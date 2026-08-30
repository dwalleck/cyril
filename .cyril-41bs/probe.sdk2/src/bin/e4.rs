#[cfg(not(unix))]
compile_error!("the E4 process-group probe requires Unix process semantics");

use std::{
    io,
    process::Command,
    str::FromStr as _,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use agent_client_protocol::{AcpAgent, AcpAgentConfig, Client, ConnectTo};
use anyhow::{Context, Result, bail};
use serde_json::json;
use tempfile::TempDir;
use tokio::time::{Instant, sleep, timeout};

fn lock_lines(lines: &Mutex<Vec<String>>) -> MutexGuard<'_, Vec<String>> {
    match lines.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn pid_state(pid: u32) -> Result<Option<(String, u32)>> {
    let output = Command::new("ps")
        .args(["-o", "stat=,pgid=", "-p", &pid.to_string()])
        .output()
        .context("spawn ps")?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fields = text.split_whitespace();
    let Some(state) = fields.next() else {
        return Ok(None);
    };
    if state.starts_with('Z') {
        return Ok(None);
    }
    let pgid = fields
        .next()
        .context("ps omitted process group")?
        .parse::<u32>()
        .context("parse process group")?;
    Ok(Some((state.to_owned(), pgid)))
}

async fn wait_for_file(path: &std::path::Path) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match tokio::fs::read_to_string(path).await {
            Ok(contents) if !contents.trim().is_empty() => return Ok(contents),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("read helper marker"),
        }
        if Instant::now() >= deadline {
            bail!("helper marker was not written: {}", path.display());
        }
        sleep(Duration::from_millis(20)).await;
    }
}

async fn wait_for_dead(pids: &[u32]) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut alive = Vec::new();
        for pid in pids {
            if pid_state(*pid)?.is_some() {
                alive.push(*pid);
            }
        }
        if alive.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("processes survived SDK guard drop: {alive:?}");
        }
        sleep(Duration::from_millis(25)).await;
    }
}

async fn connected(config: AcpAgentConfig) -> Result<(), agent_client_protocol::Error> {
    Client.builder().connect_to(AcpAgent::new(config)).await
}

#[tokio::main]
async fn main() -> Result<()> {
    let temp = TempDir::new().context("create process probe tempdir")?;

    let cwd_marker = temp.path().join("cwd.txt");
    let cwd_config = AcpAgentConfig::new("sh").args([
        "-c",
        "pwd > \"$1\"",
        "--",
        cwd_marker.to_str().context("UTF-8 cwd marker")?,
    ]);
    let serialized_config = serde_json::to_value(&cwd_config)?;
    let cwd_field_rejected = AcpAgent::from_str(r#"{"command":"sh","cwd":"/tmp"}"#).is_err();
    connected(cwd_config)
        .await
        .map_err(|error| anyhow::anyhow!("cwd helper connection failed: {error:?}"))?;
    let inherited_cwd = wait_for_file(&cwd_marker).await?.trim().to_owned();
    let expected_cwd = std::env::current_dir()?.display().to_string();

    let stderr_error = connected(
        AcpAgentConfig::new("sh").args(["-c", "printf process-probe-diagnostic >&2; exit 17"]),
    )
    .await
    .expect_err("non-zero helper must fail the connected component");
    let stderr_error_debug = format!("{stderr_error:?}");
    let stderr_error_data = serde_json::to_string(&stderr_error.data)?;
    let stderr_evidence = format!("{stderr_error_debug}\n{stderr_error_data}");

    let debug_lines = Arc::new(Mutex::new(Vec::new()));
    let debug_capture = Arc::clone(&debug_lines);
    let debug_agent = AcpAgent::new(
        AcpAgentConfig::new("sh").args(["-c", "printf clean-debug-line >&2; exit 0"]),
    )
    .with_debug(move |line, direction| {
        if direction == agent_client_protocol::LineDirection::Stderr {
            lock_lines(&debug_capture).push(line.to_owned());
        }
    });
    Client
        .builder()
        .connect_to(debug_agent)
        .await
        .map_err(|error| anyhow::anyhow!("clean debug helper failed: {error:?}"))?;
    let clean_stderr_debug = lock_lines(&debug_lines).clone();

    let group_marker = temp.path().join("group.pids");
    let group_agent = AcpAgent::new(AcpAgentConfig::new("sh").args([
        "-c",
        "printf '%s\\n' $$ > \"$1\"; sleep 30 & printf '%s\\n' $! >> \"$1\"; wait",
        "--",
        group_marker.to_str().context("UTF-8 group marker")?,
    ]));
    let (channel, group_future) =
        <AcpAgent as ConnectTo<Client>>::into_channel_and_future(group_agent);
    let group_task = tokio::spawn(group_future);
    let group_pids = wait_for_file(&group_marker)
        .await?
        .lines()
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if group_pids.len() != 2 {
        bail!("expected direct child and grandchild pids, got {group_pids:?}");
    }
    let direct_pgid = pid_state(group_pids[0])?
        .context("direct helper not alive before cancellation")?
        .1;
    let grandchild_pgid = pid_state(group_pids[1])?
        .context("grandchild helper not alive before cancellation")?
        .1;
    sleep(Duration::from_millis(1_200)).await;
    let stall_watchdog_absent = !group_task.is_finished();
    group_task.abort();
    let cancellation_join = group_task.await;
    drop(channel);
    wait_for_dead(&group_pids).await?;

    let eof_marker = temp.path().join("eof.pid");
    let eof_agent = AcpAgentConfig::new("sh").args([
        "-c",
        "printf '%s' $$ > \"$1\"; exec 1>&-; sleep 30",
        "--",
        eof_marker.to_str().context("UTF-8 eof marker")?,
    ]);
    let eof_task = tokio::spawn(connected(eof_agent));
    let eof_pid = wait_for_file(&eof_marker).await?.trim().parse::<u32>()?;
    let eof_result = timeout(Duration::from_secs(3), eof_task)
        .await
        .context("SDK did not enforce EOF shutdown grace")?
        .context("join EOF helper")?;
    wait_for_dead(&[eof_pid]).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C6"],
            "config_fields": serialized_config,
            "cwd_field_present": serialized_config.get("cwd").is_some(),
            "cwd_field_rejected": cwd_field_rejected,
            "runtime_child_cwd": inherited_cwd,
            "runtime_parent_cwd": expected_cwd,
            "runtime_inherits_parent_cwd": inherited_cwd == expected_cwd,
            "nonzero_exit_is_error": true,
            "nonzero_error_contains_status": stderr_evidence.contains("17"),
            "nonzero_error_contains_stderr": stderr_evidence.contains("process-probe-diagnostic"),
            "nonzero_error_debug": stderr_error_debug,
            "nonzero_error_data": stderr_error_data,
            "clean_exit_stderr_debug_callback": clean_stderr_debug,
            "public_stderr_tail_accessor": false,
            "direct_child_pgid": direct_pgid,
            "grandchild_pgid": grandchild_pgid,
            "same_process_group": direct_pgid == group_pids[0] && grandchild_pgid == direct_pgid,
            "stall_watchdog_absent_after_ms": 1_200,
            "stall_future_remained_pending": stall_watchdog_absent,
            "drop_cancelled_connection_future": cancellation_join.is_err(),
            "drop_killed_direct_and_grandchild": true,
            "stdout_eof_returned_ok": eof_result.is_ok(),
            "stdout_eof_killed_nonexiting_child_within_grace": true,
            "decision": "retain Cyril AgentProcess or add a custom SDK process component; AcpAgent lacks launch cwd, stall policy, and public always-on stderr tail despite matching Unix group cleanup and bounded EOF shutdown",
        }))?
    );
    Ok(())
}

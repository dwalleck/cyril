//! Throwaway public-bridge ownership probe; fixture self-exits after seven seconds.
use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::{AgentCommand, SpawnEnvironment};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

type ProbeResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ProbeResult {
    let root = tempfile::tempdir()?;
    let script = root.path().join("peer.sh");
    std::fs::write(&script, "#!/bin/sh\nIFS= read -r request\nprintf '%s\\n' \"$request\" > initialize.tmp\n/bin/mv initialize.tmp initialize.json\n/bin/sleep 7\nprintf 'natural exit\\n' > natural-exit\n")?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))?;
    let bridge = spawn_bridge(
        AgentCommand::new(script.to_str().ok_or("non-UTF-8 fixture path")?),
        SpawnConfig {
            shell: Some("bash".into()),
            environment: SpawnEnvironment::Replace(BTreeMap::new()),
            ..SpawnConfig::default()
        },
        root.path().to_owned(),
    )?;
    let (sender, _notifications, _permissions, _sources, mut completion) = bridge.split();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match std::fs::read_to_string(root.path().join("initialize.json")) {
                Ok(text) => {
                    let request: serde_json::Value = serde_json::from_str(&text)?;
                    if request["method"] != "initialize" {
                        return Err("fixture did not receive initialize".into());
                    }
                    return ProbeResult::Ok(());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }).await??;
    drop(sender);
    let started = Instant::now();
    let completed_within_three_seconds = match tokio::time::timeout(Duration::from_secs(3), &mut completion).await {
        Ok(result) => { result?; true }
        Err(_) => {
            tokio::time::timeout(Duration::from_secs(10), &mut completion).await??;
            false
        }
    };
    println!("{}", serde_json::json!({
        "initialize_received": true,
        "all_command_senders_dropped": true,
        "completion_within_three_seconds": completed_within_three_seconds,
        "peer_self_exit_observed": root.path().join("natural-exit").is_file(),
        "elapsed_seconds": started.elapsed().as_secs_f64(),
        "cleanup_completion_observed": true
    }));
    Ok(())
}

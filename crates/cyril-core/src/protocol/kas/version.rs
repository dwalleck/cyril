//! KAS wrapper spawn: version→flag resolution + the `kiro-cli acp
//! --agent-engine <flag>` command (KAS-1 Part B, cyril-evwh).

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time::timeout;

use crate::types::AgentCommand;

const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const VERSION_REAP_TIMEOUT: Duration = Duration::from_secs(1);
const VERSION_PIPE_LIMIT: u64 = 1024 * 1024;

/// Parse the leading `MAJOR.MINOR.PATCH` of a version string into a tuple,
/// ignoring any pre-release/build suffix on the patch (e.g. `2.8.1-beta` →
/// `(2,8,1)`). Compared as a tuple so ordering is true semver, NOT lexical
/// (`2.10.0` > `2.8.0`). Returns `Err` on a malformed string.
pub(crate) fn parse_semver(s: &str) -> Result<(u32, u32, u32), String> {
    let mut it = s.trim().split('.');
    // Pulls the next dotted component's leading digit-run. Captures `it` and the
    // outer `s` (for the error) — it takes no argument, so there is no shadowed
    // parameter to confuse with the version string it reports.
    let mut next = || -> Result<u32, String> {
        it.next()
            .map(|c| c.trim_start_matches(|ch: char| !ch.is_ascii_digit()))
            .map(|c| {
                c.split(|ch: char| !ch.is_ascii_digit())
                    .next()
                    .unwrap_or("")
            })
            .filter(|c| !c.is_empty())
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| format!("malformed kiro-cli version {s:?}"))
    };
    let (maj, min, pat) = (next()?, next()?, next()?);
    Ok((maj, min, pat))
}

/// Resolve cyril's `--agent-engine` flag from the installed kiro-cli version.
/// kiro-cli 2.8.0 renamed `--agent-engine kas` → `v3`; 2.7.1 accepted `kas`;
/// below 2.7.1 there is no embedded KAS engine. Probe-verified (2026-06-19).
pub(crate) fn flag_for_version(version: &str) -> Result<&'static str, String> {
    let v = parse_semver(version)?;
    if v >= (2, 8, 0) {
        Ok("v3")
    } else if v >= (2, 7, 1) {
        Ok("kas")
    } else {
        Err(format!("KAS requires kiro-cli >= 2.7.1, found {version}"))
    }
}

async fn read_limited(
    mut pipe: impl AsyncRead + Unpin,
    bytes: &mut Vec<u8>,
) -> std::io::Result<()> {
    (&mut pipe)
        .take(VERSION_PIPE_LIMIT)
        .read_to_end(bytes)
        .await?;
    // Keep draining after the capture cap so verbose probes can still exit.
    tokio::io::copy(&mut pipe, &mut tokio::io::sink()).await?;
    Ok(())
}

/// Read the installed kiro-cli version through a bounded child lifecycle.
/// One deadline covers process exit and both pipe EOFs. Cleanup gets a separate
/// bounded reap; inherited descendant pipes cannot park the bridge indefinitely.
pub(crate) async fn kiro_cli_version(
    program: &str,
    environment: &crate::types::SpawnEnvironment,
) -> Result<String, String> {
    let mut command = Command::new(program);
    environment.apply(command.as_std_mut());
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("run `{program} --version`: {e}"))?;
    #[cfg(unix)]
    let group = crate::protocol::transport::ProcessGroupGuard::new(child.id());
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture stdout from `{program} --version`"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture stderr from `{program} --version`"))?;
    // Keep captured bytes outside the cancellable reads so a deadline retains
    // diagnostics already received, without waiting again for pipe EOF.
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let output = timeout(VERSION_PROBE_TIMEOUT, async {
        tokio::try_join!(
            child.wait(),
            read_limited(stdout, &mut stdout_bytes),
            read_limited(stderr, &mut stderr_bytes)
        )
    })
    .await
    .map_err(|_| {
        format!(
            "`{program} --version` exceeded the {} second startup probe deadline",
            VERSION_PROBE_TIMEOUT.as_secs()
        )
    })
    .and_then(|result| {
        result.map_err(|error| format!("read or wait for `{program} --version`: {error}"))
    });
    // Also kill descendants when the wrapper has already exited normally.
    #[cfg(unix)]
    drop(group);
    let status = match output {
        Ok((status, (), ())) => status,
        Err(reason) => {
            match timeout(VERSION_REAP_TIMEOUT, child.kill()).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::debug!(program, %error, "failed to reap version probe"),
                Err(error) => {
                    tracing::warn!(program, %error, "version probe reap deadline elapsed")
                }
            }
            return Err(match String::from_utf8_lossy(&stderr_bytes).trim() {
                "" => reason,
                diagnostic => format!("{reason}: {diagnostic}"),
            });
        }
    };
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr_bytes);
        return Err(format!(
            "`{program} --version` exited {status}: {}",
            stderr.trim()
        ));
    }
    let text = String::from_utf8_lossy(&stdout_bytes);
    text.split_whitespace()
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()))
        .map(|v| v.to_string())
        .ok_or_else(|| format!("could not parse a version from `{program} --version`"))
}

/// Build the wrapper spawn command: the bound agent command (`kiro-cli acp`)
/// with `--agent-engine <flag>` appended, the flag resolved from the installed
/// version. Custom `agent_command` args are preserved (the flag is appended).
pub(crate) async fn build_wrapper_command(
    agent_command: &AgentCommand,
    environment: &crate::types::SpawnEnvironment,
    required_version: Option<&str>,
) -> Result<AgentCommand, String> {
    let version = kiro_cli_version(agent_command.program(), environment).await?;
    if let Some(required) = required_version
        && version != required
    {
        return Err(format!(
            "this launch requires kiro-cli {required}, found {version} at {}; install the required version or select its executable",
            agent_command.program(),
        ));
    }
    let flag = flag_for_version(&version)?;
    let mut args = agent_command.args().to_vec();
    args.push("--agent-engine".to_string());
    args.push(flag.to_string());
    Ok(AgentCommand::new(agent_command.program()).with_args(args))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    // C7 + the semver-vs-lexical stress: 2.10.0 / 2.8.10 must be v3 (a string
    // compare would put them below 2.8.0).
    #[test]
    fn flag_for_version_table() {
        for (v, want) in [
            ("2.8.1", "v3"),
            ("2.8.0", "v3"),
            ("2.8.10", "v3"),
            ("2.10.0", "v3"), // semver, NOT lexical
            ("3.0.0", "v3"),
            ("2.7.1", "kas"),
            ("2.7.9", "kas"),
        ] {
            assert_eq!(flag_for_version(v), Ok(want), "version {v}");
        }
    }

    #[test]
    fn flag_for_version_refuses_below_2_7_1() {
        assert!(flag_for_version("2.7.0").is_err()); // 2.7.0 has no embedded KAS
        assert!(flag_for_version("2.6.9").is_err());
        assert!(flag_for_version("1.29.7").is_err());
    }

    #[test]
    fn flag_for_version_rejects_malformed() {
        assert!(flag_for_version("kiro 2").is_err());
        assert!(flag_for_version("").is_err());
        assert!(flag_for_version("v3").is_err());
    }

    // Suffix tolerance: a pre-release patch still parses.
    #[test]
    fn parse_semver_tolerates_patch_suffix() {
        assert_eq!(parse_semver("2.8.1-beta.2"), Ok((2, 8, 1)));
        assert_eq!(parse_semver("2.8.1 (build 5)"), Ok((2, 8, 1)));
    }
}

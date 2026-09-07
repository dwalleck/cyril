//! KAS wrapper spawn: version→flag resolution + the `kiro-cli acp
//! --agent-engine <flag>` command (KAS-1 Part B, cyril-evwh).

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::types::AgentCommand;

const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const VERSION_POLL: Duration = Duration::from_millis(10);
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

fn read_limited(pipe: impl Read, limit: u64) -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    pipe.take(limit + 1).read_to_end(&mut bytes)?;
    bytes.truncate(limit as usize);
    Ok(bytes)
}

/// Read the installed kiro-cli version through a bounded child lifecycle.
/// This runs on the bridge thread during startup: a hung `--version` must
/// fail and complete the bridge rather than park reviewers or private trees.
pub(crate) fn kiro_cli_version(
    program: &str,
    environment: &crate::types::SpawnEnvironment,
) -> Result<String, String> {
    let mut command = Command::new(program);
    environment.apply(&mut command);
    command
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("run `{program} --version`: {e}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("run `{program} --version`: {e}"))?
        {
            break Ok(status);
        }
        if started.elapsed() >= VERSION_PROBE_TIMEOUT {
            #[cfg(unix)]
            if let Some(pgid) = i32::try_from(child.id())
                .ok()
                .and_then(std::num::NonZeroI32::new)
                && let Err(error) = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(pgid.get()),
                    nix::sys::signal::Signal::SIGKILL,
                )
            {
                tracing::debug!(program, %error, "failed to kill timed-out version probe group");
            }
            if let Err(error) = child.kill() {
                tracing::debug!(program, %error, "failed to kill timed-out version probe");
            }
            break Err(format!(
                "`{program} --version` exceeded the {} second startup probe deadline",
                VERSION_PROBE_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(VERSION_POLL);
    };
    // Reap the probe and close pipes before reading bounded output. Failure
    // here must not replace the already-decided child status/error.
    if let Err(error) = child.wait() {
        tracing::debug!(program, %error, "failed to reap version probe");
    }
    let status = status?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture stdout from `{program} --version`"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture stderr from `{program} --version`"))?;
    let stdout_bytes = read_limited(&mut stdout, VERSION_PIPE_LIMIT)
        .map_err(|e| format!("read `{program} --version` stdout: {e}"))?;
    let stderr_bytes = read_limited(&mut stderr, VERSION_PIPE_LIMIT)
        .map_err(|e| format!("read `{program} --version` stderr: {e}"))?;
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
pub(crate) fn build_wrapper_command(
    agent_command: &AgentCommand,
    environment: &crate::types::SpawnEnvironment,
    required_version: Option<&str>,
) -> Result<AgentCommand, String> {
    let version = kiro_cli_version(agent_command.program(), environment)?;
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

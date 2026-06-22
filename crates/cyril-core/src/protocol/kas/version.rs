//! KAS wrapper spawn: version→flag resolution + the `kiro-cli acp
//! --agent-engine <flag>` command (KAS-1 Part B, cyril-evwh).

use crate::types::AgentCommand;

/// Parse the leading `MAJOR.MINOR.PATCH` of a version string into a tuple,
/// ignoring any pre-release/build suffix on the patch (e.g. `2.8.1-beta` →
/// `(2,8,1)`). Compared as a tuple so ordering is true semver, NOT lexical
/// (`2.10.0` > `2.8.0`). Returns `Err` on a malformed string.
fn parse_semver(s: &str) -> Result<(u32, u32, u32), String> {
    let mut it = s.trim().split('.');
    let mut next = |s: &str| -> Result<u32, String> {
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
    let (maj, min, pat) = (next(s)?, next(s)?, next(s)?);
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

/// Read the installed kiro-cli version by running `<program> --version` and
/// pulling the first digit-leading token (`kiro-cli 2.8.1` → `2.8.1`).
fn kiro_cli_version(program: &str) -> Result<String, String> {
    let out = std::process::Command::new(program)
        .arg("--version")
        .output()
        .map_err(|e| format!("run `{program} --version`: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()))
        .map(|v| v.to_string())
        .ok_or_else(|| format!("could not parse a version from `{program} --version`"))
}

/// Build the wrapper spawn command: the bound agent command (`kiro-cli acp`)
/// with `--agent-engine <flag>` appended, the flag resolved from the installed
/// version. Custom `agent_command` args are preserved (the flag is appended).
pub(crate) fn build_wrapper_command(agent_command: &AgentCommand) -> Result<AgentCommand, String> {
    let version = kiro_cli_version(agent_command.program())?;
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

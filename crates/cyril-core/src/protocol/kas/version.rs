//! KAS wrapper spawn: version→flag resolution + the `kiro-cli acp
//! --agent-engine <flag>` command (KAS-1 Part B, cyril-evwh).

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time::timeout;

use crate::platform::path::{basename_is, is_wsl_launcher};
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

/// wsl.exe options that consume the following argv element as their value
/// (`wsl --help`, microsoft/WSL `Resources.resw`: `--distribution, -d
/// <DistroName>`, `--distribution-id <DistroGuid>` (2.4.4+), `--user, -u
/// <UserName>`, `--cd <Directory>`, `--shell-type <Type>`). Every other
/// `-`-prefixed element (`--exec`/`-e`, `--system`, `--`) is a bare flag, and
/// so is the positional `~` ("start in the user's home directory").
const WSL_VALUE_OPTIONS: [&str; 7] = [
    "-d",
    "--distribution",
    "--distribution-id",
    "-u",
    "--user",
    "--cd",
    "--shell-type",
];

/// The argv that answers "which kiro-cli will this agent command run?": the
/// agent command's prefix up to and including its kiro-cli element, plus
/// `--version` (cyril-861q).
///
/// A plain command (`kiro-cli acp`) probes `kiro-cli --version`. When the
/// program is the Windows WSL launcher (`wsl`/`wsl.exe`, any casing, bare or
/// full path — [`is_wsl_launcher`]), kiro-cli lives inside the distro and a
/// native probe would answer for the wrong binary (wsl.exe's own
/// `--version`), so the probe runs THROUGH the launcher: the launcher's
/// options, their values and its `~` shorthand stay in front of the command
/// element — the first element after them — which must itself be kiro-cli
/// ([`is_kiro_cli`]): `wsl -d Ubuntu kiro-cli acp` →
/// `wsl -d Ubuntu kiro-cli --version`. `Err` names the command when no
/// element follows the launcher's options, or when that element is something
/// else (`wsl bash -lc "kiro-cli acp"` would otherwise probe `bash --version`
/// and read bash's `5.2.21` as a kiro-cli version, silently selecting `v3`)
/// — nothing is spawned for either.
pub(crate) fn version_probe_command(agent_command: &AgentCommand) -> Result<AgentCommand, String> {
    let program = agent_command.program();
    let mut probe = Vec::new();
    if is_wsl_launcher(program) {
        let mut args = agent_command.args().iter();
        let command = loop {
            let Some(arg) = args.next() else {
                return Err(format!(
                    "cannot determine the kiro-cli version: `{}` routes through the WSL \
                     launcher but names no kiro-cli element after the launcher's options",
                    render(agent_command)
                ));
            };
            probe.push(arg.clone());
            if arg == "~" {
                continue;
            }
            if !arg.starts_with('-') {
                break arg;
            }
            if WSL_VALUE_OPTIONS.contains(&arg.as_str())
                && let Some(value) = args.next()
            {
                probe.push(value.clone());
            }
        };
        if !is_kiro_cli(command) {
            return Err(format!(
                "cannot determine the kiro-cli version: `{}` routes through the WSL launcher \
                 but the command after the launcher's options is `{command}`, not kiro-cli",
                render(agent_command)
            ));
        }
    }
    probe.push("--version".to_string());
    Ok(AgentCommand::new(program).with_args(probe))
}

/// `true` when `element` names kiro-cli — basename `kiro-cli`, optionally
/// with an `.exe` suffix, any ASCII casing, bare or as a full path — by the
/// same basename rule as [`is_wsl_launcher`] ([`basename_is`]).
fn is_kiro_cli(element: &str) -> bool {
    basename_is(element, "kiro-cli")
}

/// `program arg1 arg2 …` for diagnostics.
fn render(command: &AgentCommand) -> String {
    std::iter::once(command.program())
        .chain(command.args().iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Read the kiro-cli version the agent command would run: the probe from
/// [`version_probe_command`], executed by [`run_version_probe`].
pub(crate) async fn kiro_cli_version(
    agent_command: &AgentCommand,
    environment: &crate::types::SpawnEnvironment,
) -> Result<String, String> {
    let probe = version_probe_command(agent_command)?;
    run_version_probe(&probe, environment).await
}

/// Run a version probe through a bounded child lifecycle and parse the
/// version it prints. One deadline covers process exit and both pipe EOFs.
/// Cleanup gets a separate bounded reap; inherited descendant pipes cannot
/// park the bridge indefinitely.
async fn run_version_probe(
    probe: &AgentCommand,
    environment: &crate::types::SpawnEnvironment,
) -> Result<String, String> {
    let rendered = render(probe);
    let mut command = Command::new(probe.program());
    environment.apply(command.as_std_mut());
    command
        .args(probe.args())
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
        .map_err(|e| format!("run `{rendered}`: {e}"))?;
    #[cfg(unix)]
    let group = crate::protocol::transport::ProcessGroupGuard::new(child.id());
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture stdout from `{rendered}`"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture stderr from `{rendered}`"))?;
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
            "`{rendered}` exceeded the {} second startup probe deadline",
            VERSION_PROBE_TIMEOUT.as_secs()
        )
    })
    .and_then(|result| result.map_err(|error| format!("read or wait for `{rendered}`: {error}")));
    // Also kill descendants when the wrapper has already exited normally.
    #[cfg(unix)]
    drop(group);
    let status = match output {
        Ok((status, (), ())) => status,
        Err(reason) => {
            match timeout(VERSION_REAP_TIMEOUT, child.kill()).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::debug!(probe = %rendered, %error, "failed to reap version probe")
                }
                Err(error) => {
                    tracing::warn!(probe = %rendered, %error, "version probe reap deadline elapsed")
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
        return Err(format!("`{rendered}` exited {status}: {}", stderr.trim()));
    }
    let text = String::from_utf8_lossy(&stdout_bytes);
    text.split_whitespace()
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()))
        .map(|v| v.to_string())
        .ok_or_else(|| format!("could not parse a version from `{rendered}`"))
}

/// Build the wrapper spawn command: the bound agent command (`kiro-cli acp`)
/// with `--agent-engine <flag>` appended, the flag resolved from the version
/// of the kiro-cli that command would run (probed through a WSL launcher when
/// the command routes through one — [`version_probe_command`]). Custom
/// `agent_command` args are preserved (the flag is appended).
pub(crate) async fn build_wrapper_command(
    agent_command: &AgentCommand,
    environment: &crate::types::SpawnEnvironment,
    required_version: Option<&str>,
) -> Result<AgentCommand, String> {
    let probe = version_probe_command(agent_command)?;
    let version = run_version_probe(&probe, environment).await?;
    if let Some(required) = required_version
        && version != required
    {
        return Err(format!(
            "this launch requires kiro-cli {required}, found {version} via `{}`; install the required version or select its executable",
            render(&probe),
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

    fn argv(parts: &[&str]) -> AgentCommand {
        AgentCommand::try_from_argv(parts.iter().map(|p| p.to_string()).collect()).unwrap()
    }

    fn strings(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|p| p.to_string()).collect()
    }

    /// `(program, args)` of the probe `version_probe_command` computes.
    fn probe_of(parts: &[&str]) -> (String, Vec<String>) {
        let probe = version_probe_command(&argv(parts)).unwrap();
        (probe.program().to_string(), probe.args().to_vec())
    }

    // cyril-861q fences: the exact argv the version probe spawns. A plain
    // command keeps today's `kiro-cli --version`; a launcher-routed command
    // asks kiro-cli THROUGH the launcher instead of asking the launcher.
    #[test]
    fn version_probe_of_plain_kiro_cli_is_kiro_cli_version() {
        assert_eq!(
            probe_of(&["kiro-cli", "acp"]),
            ("kiro-cli".to_string(), strings(&["--version"]))
        );
    }

    #[test]
    fn version_probe_runs_through_a_bare_wsl_launcher() {
        assert_eq!(
            probe_of(&["wsl", "kiro-cli", "acp"]),
            ("wsl".to_string(), strings(&["kiro-cli", "--version"]))
        );
    }

    #[test]
    fn version_probe_runs_through_an_uppercase_wsl_exe_launcher() {
        assert_eq!(
            probe_of(&["WSL.EXE", "kiro-cli", "acp"]),
            ("WSL.EXE".to_string(), strings(&["kiro-cli", "--version"]))
        );
    }

    #[test]
    fn version_probe_keeps_launcher_options_and_values_before_kiro_cli() {
        assert_eq!(
            probe_of(&[
                r"C:\Windows\System32\wsl.exe",
                "-d",
                "Ubuntu",
                "kiro-cli",
                "acp"
            ]),
            (
                r"C:\Windows\System32\wsl.exe".to_string(),
                strings(&["-d", "Ubuntu", "kiro-cli", "--version"])
            )
        );
    }

    // `-e`/`--exec` and `--` take no value: the element after them is kiro-cli.
    #[test]
    fn version_probe_treats_exec_and_double_dash_as_bare_launcher_flags() {
        assert_eq!(
            probe_of(&["wsl", "-e", "kiro-cli", "acp"]).1,
            strings(&["-e", "kiro-cli", "--version"])
        );
        assert_eq!(
            probe_of(&["wsl", "-d", "Ubuntu", "--", "kiro-cli", "acp"]).1,
            strings(&["-d", "Ubuntu", "--", "kiro-cli", "--version"])
        );
    }

    #[test]
    fn version_probe_refuses_a_launcher_without_a_kiro_cli_element() {
        assert_eq!(
            version_probe_command(&argv(&["wsl", "-d", "Ubuntu"])).unwrap_err(),
            "cannot determine the kiro-cli version: `wsl -d Ubuntu` routes through the WSL \
             launcher but names no kiro-cli element after the launcher's options"
        );
        // A dangling value option is the same refusal, not a `--version` probe
        // of the launcher.
        assert!(version_probe_command(&argv(&["wsl", "-d"])).is_err());
    }

    // F1 (review of cyril-861q): wsl.exe's positional `~` ("start in the
    // home directory") is a launcher token, not the command — skipped like a
    // bare flag, or the probe degrades to `wsl ~ --version`.
    #[test]
    fn version_probe_skips_the_launcher_home_shorthand() {
        assert_eq!(
            probe_of(&["wsl", "~", "kiro-cli", "acp"]),
            ("wsl".to_string(), strings(&["~", "kiro-cli", "--version"]))
        );
    }

    // F2 (review of cyril-861q): `--distribution-id <DistroGuid>` (wsl.exe
    // 2.4.4+) takes a value; treating it as bare would make the GUID the
    // "command" and the probe `wsl --distribution-id <guid> --version`.
    #[test]
    fn version_probe_keeps_a_distribution_id_value_before_kiro_cli() {
        let guid = "{6a3b1f2c-4d5e-4f60-9a1b-2c3d4e5f6a7b}";
        assert_eq!(
            probe_of(&["wsl", "--distribution-id", guid, "kiro-cli", "acp"]).1,
            strings(&["--distribution-id", guid, "kiro-cli", "--version"])
        );
    }

    // F3 (review of cyril-861q): the element the launcher scan lands on must
    // BE kiro-cli. Probing whatever comes first would ask `env` or `bash` for
    // their versions: bash's `5.2.21` silently selects `v3`, and coreutils'
    // two-component `9.4` fails loudly as malformed — both are wrong.
    #[test]
    fn version_probe_refuses_a_launcher_whose_command_is_not_kiro_cli() {
        assert_eq!(
            version_probe_command(&argv(&["wsl", "-e", "env", "FOO=1", "kiro-cli", "acp"]))
                .unwrap_err(),
            "cannot determine the kiro-cli version: `wsl -e env FOO=1 kiro-cli acp` routes \
             through the WSL launcher but the command after the launcher's options is `env`, \
             not kiro-cli"
        );
        let error =
            version_probe_command(&argv(&["wsl", "bash", "-lc", "kiro-cli acp"])).unwrap_err();
        assert!(error.contains("is `bash`, not kiro-cli"), "{error}");
    }

    // Controls for the kiro-cli check: a full path or `.exe`/casing variant of
    // kiro-cli still probes through (no new false reject).
    #[test]
    fn version_probe_accepts_kiro_cli_by_basename_through_a_launcher() {
        assert_eq!(
            probe_of(&["wsl", "-d", "Ubuntu", "/home/u/.local/bin/kiro-cli", "acp"]).1,
            strings(&["-d", "Ubuntu", "/home/u/.local/bin/kiro-cli", "--version"])
        );
        assert_eq!(
            probe_of(&["wsl", "Kiro-CLI.exe", "acp"]).1,
            strings(&["Kiro-CLI.exe", "--version"])
        );
    }

    // The kiro-cli check applies to launcher-routed commands only: a
    // non-launcher program keeps today's `<program> --version` unchanged.
    #[test]
    fn version_probe_of_a_non_launcher_program_is_not_subject_to_the_kiro_cli_check() {
        assert_eq!(
            probe_of(&["env", "FOO=1", "kiro-cli", "acp"]),
            ("env".to_string(), strings(&["--version"]))
        );
    }

    // The refusal surfaces from the wrapper build itself, before any spawn:
    // the message is the probe computation's, not a spawn failure's.
    #[tokio::test]
    async fn wrapper_build_refuses_a_launcher_without_a_kiro_cli_element() {
        let error = build_wrapper_command(
            &argv(&["wsl", "-d", "Ubuntu"]),
            &crate::types::SpawnEnvironment::Inherit,
            None,
        )
        .await
        .unwrap_err();
        assert!(
            error.contains("names no kiro-cli element after the launcher's options"),
            "{error}"
        );
    }

    /// A fake wsl.exe: journals the argv it receives, answers its own
    /// `--version` like the real launcher, and reports `kiro-cli 2.21.1` only
    /// when `kiro-cli --version` is passed through it after its options.
    /// Returns the launcher's path (its basename is `wsl`) and the journal.
    #[cfg(unix)]
    fn fake_wsl_launcher(root: &std::path::Path) -> (String, std::path::PathBuf) {
        let journal = root.join("probe-argv.txt");
        let launcher = root.join("wsl");
        // Keep executable bytes immutable: another test's fork can retain a
        // writable descriptor even after fs::write returns in this thread.
        std::os::unix::fs::symlink(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/wsl-launcher.sh"
            ),
            &launcher,
        )
        .unwrap();
        (launcher.to_str().unwrap().to_string(), journal)
    }

    // cyril-861q reproduction at the process seam: a `wsl`-named launcher
    // answers its own `--version` (like the real wsl.exe) and only runs
    // kiro-cli when the command line is passed through it. The wrapper build
    // must resolve the flag from kiro-cli's answer, not the launcher's, and
    // must send the launcher exactly `-d Ubuntu kiro-cli --version`.
    #[cfg(unix)]
    #[tokio::test]
    async fn wrapper_probe_asks_kiro_cli_through_the_wsl_launcher() {
        let root = tempfile::tempdir().unwrap();
        let (program, journal) = fake_wsl_launcher(root.path());
        let agent_command =
            AgentCommand::new(&program).with_args(strings(&["-d", "Ubuntu", "kiro-cli", "acp"]));

        let built = build_wrapper_command(
            &agent_command,
            &crate::types::SpawnEnvironment::Inherit,
            None,
        )
        .await;
        let built = match built {
            Ok(built) => built,
            Err(error) => panic!("wrapper command should resolve from kiro-cli's version: {error}"),
        };

        assert_eq!(
            std::fs::read_to_string(&journal).unwrap(),
            "-d\nUbuntu\nkiro-cli\n--version\n",
            "the launcher must receive its own options, then `kiro-cli --version`"
        );
        assert_eq!(built.program(), program);
        assert_eq!(
            built.args(),
            strings(&["-d", "Ubuntu", "kiro-cli", "acp", "--agent-engine", "v3"])
        );
    }

    // Process-seam fence for the `~` skip (re-review N1): the fixture must
    // model `wsl ~` — an unquoted `~` in a POSIX `case` pattern is
    // tilde-expanded and never matches a literal `~` — and the whole path
    // must still reach kiro-cli behind it.
    #[cfg(unix)]
    #[tokio::test]
    async fn wrapper_probe_passes_the_launcher_home_shorthand_through() {
        let root = tempfile::tempdir().unwrap();
        let (program, journal) = fake_wsl_launcher(root.path());
        let agent_command = AgentCommand::new(&program)
            .with_args(strings(&["~", "-d", "Ubuntu", "kiro-cli", "acp"]));

        let built = build_wrapper_command(
            &agent_command,
            &crate::types::SpawnEnvironment::Inherit,
            None,
        )
        .await;
        let built = match built {
            Ok(built) => built,
            Err(error) => panic!("wrapper command should resolve through `wsl ~`: {error}"),
        };

        assert_eq!(
            std::fs::read_to_string(&journal).unwrap(),
            "~\n-d\nUbuntu\nkiro-cli\n--version\n"
        );
        assert_eq!(
            built.args(),
            strings(&[
                "~",
                "-d",
                "Ubuntu",
                "kiro-cli",
                "acp",
                "--agent-engine",
                "v3"
            ])
        );
    }

    // F4 (review of cyril-861q): a required-version mismatch names the command
    // that was actually probed, not the launcher it went through ("at wsl").
    #[cfg(unix)]
    #[tokio::test]
    async fn wrapper_version_mismatch_names_the_probed_command() {
        let root = tempfile::tempdir().unwrap();
        let (program, _journal) = fake_wsl_launcher(root.path());
        let agent_command =
            AgentCommand::new(&program).with_args(strings(&["-d", "Ubuntu", "kiro-cli", "acp"]));

        let error = build_wrapper_command(
            &agent_command,
            &crate::types::SpawnEnvironment::Inherit,
            Some("9.9.9"),
        )
        .await
        .unwrap_err();

        assert_eq!(
            error,
            format!(
                "this launch requires kiro-cli 9.9.9, found 2.21.1 via `{program} -d Ubuntu \
                 kiro-cli --version`; install the required version or select its executable"
            )
        );
    }
}

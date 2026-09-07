//! Public bridge launch boundaries, observed by real POSIX executable fixtures.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::{AgentCommand, Notification, SpawnEnvironment};
#[cfg(feature = "kas")]
use tokio::time::timeout;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn executable(root: &Path) -> TestResult<AgentCommand> {
    let path = root.join("agent space λ.sh");
    std::fs::write(
        &path,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    /usr/bin/env > "$PROBE_JOURNAL"
    printf 'kiro-cli %s\n' "${VERSION:-2.21.1}"
    exit 0
fi
/usr/bin/env > environment.txt
IFS= read -r request
printf '%s\n' "$request" > initialize.json
"#,
    )?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    Ok(AgentCommand::new(
        path.to_str().ok_or("non-UTF-8 fixture path")?,
    ))
}

async fn launch(root: &Path, config: SpawnConfig) -> TestResult<String> {
    launch_command(root, config, executable(root)?).await
}

async fn launch_command(
    root: &Path,
    config: SpawnConfig,
    command: AgentCommand,
) -> TestResult<String> {
    let bridge = spawn_bridge(command, config, root.to_owned())?;
    let (_sender, mut notifications, _permissions, _sources, completion) = bridge.split();
    let reason = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(routed) = notifications.recv().await {
            if let Notification::BridgeDisconnected { reason } = routed.notification {
                return reason;
            }
        }
        panic!("fixture must report its exit");
    })
    .await?;
    tokio::time::timeout(Duration::from_secs(10), completion).await??;
    Ok(reason)
}

#[tokio::test]
async fn isolated_spawn_does_not_inherit_parent_authority() -> TestResult {
    // Re-enter to seed a canary without mutating the test runner's environment.
    const CANARY: &str = "CYRIL_ISOLATION_PARENT_AUTHORITY";
    const COMPLETED: &str = "CYRIL_ISOLATION_COMPLETED";
    let Some(completed) = std::env::var_os(COMPLETED) else {
        let root = tempfile::tempdir()?;
        let completed = root.path().join("completed");
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "isolated_spawn_does_not_inherit_parent_authority",
                "--nocapture",
            ])
            .env(CANARY, "parent-only-secret")
            .env(COMPLETED, &completed)
            .status()?;
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(completed)?, "isolation checked");
        return Ok(());
    };
    let inherited = tempfile::tempdir()?;
    launch(inherited.path(), SpawnConfig::default()).await?;
    assert!(
        std::fs::read_to_string(inherited.path().join("environment.txt"))?
            .lines()
            .any(|line| line == "CYRIL_ISOLATION_PARENT_AUTHORITY=parent-only-secret")
    );
    let isolated = tempfile::tempdir()?;
    let mut selected =
        BTreeMap::from([(OsString::from("RUN_VALUE"), OsString::from("selected λ"))]);
    for index in 0..127 {
        selected.insert(format!("PADDING_{index}").into(), "x".repeat(512).into());
    }
    launch(
        isolated.path(),
        SpawnConfig {
            environment: SpawnEnvironment::Replace(selected),
            ..SpawnConfig::default()
        },
    )
    .await?;
    let observed = std::fs::read_to_string(isolated.path().join("environment.txt"))?;
    assert!(!observed.contains(CANARY));
    assert!(observed.lines().any(|line| line == "RUN_VALUE=selected λ"));
    assert_eq!(std::env::var(CANARY)?, "parent-only-secret");
    std::fs::write(completed, "isolation checked")?;
    Ok(())
}

#[cfg(feature = "kas")]
fn kas_config(root: &Path, thinking: bool) -> TestResult<SpawnConfig> {
    std::fs::create_dir_all(root.join("settings"))?;
    std::fs::write(
        root.join("settings/cli.json"),
        serde_json::json!({
            "chat.enableThinking": thinking,
            "unrelated-setting": "x".repeat(256 * 1024),
        })
        .to_string(),
    )?;
    Ok(SpawnConfig {
        engine: cyril_core::types::AgentEngine::Kas,
        kas_spawn: cyril_core::types::KasSpawn::Wrapper,
        kas_hooks: cyril_core::types::kas_hooks::KasHooksMode::Off,
        shell: Some("bash".into()),
        required_cli_version: Some("2.21.1".into()),
        environment: SpawnEnvironment::Replace(BTreeMap::from([
            (OsString::from("KIRO_HOME"), root.as_os_str().to_owned()),
            (
                OsString::from("PROBE_JOURNAL"),
                root.join("probe.env").into_os_string(),
            ),
        ])),
        ..SpawnConfig::default()
    })
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn concurrent_settings_are_run_local() -> TestResult {
    let a = tempfile::tempdir()?;
    let b = tempfile::tempdir()?;
    let a_config = kas_config(a.path(), true)?;
    let b_config = kas_config(b.path(), false)?;
    let original_home = std::env::var_os("KIRO_HOME");
    let (a_result, b_result) = tokio::join!(launch(a.path(), a_config), launch(b.path(), b_config));
    a_result?;
    b_result?;
    for (root, expected) in [(a.path(), true), (b.path(), false)] {
        let initialize: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("initialize.json"))?)?;
        assert_eq!(initialize["method"], "initialize");
        assert_eq!(
            initialize["params"]["clientCapabilities"]["_meta"]["kiro"]["settings"]["thinking"]["enabled"],
            expected
        );
        let probe = std::fs::read_to_string(root.join("probe.env"))?;
        assert!(
            probe
                .lines()
                .any(|line| line == format!("KIRO_HOME={}", root.display()))
        );
    }
    assert_eq!(std::env::var_os("KIRO_HOME"), original_home);
    Ok(())
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn exact_version_mismatch_never_starts_agent() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut config = kas_config(root.path(), false)?;
    if let SpawnEnvironment::Replace(values) = &mut config.environment {
        values.insert("VERSION".into(), "2.21.1-beta".into());
    }
    let reason = launch(root.path(), config).await?;
    assert!(reason.contains("2.21.1-beta"), "{reason}");
    assert!(root.path().join("probe.env").exists());
    assert!(!root.path().join("initialize.json").exists());
    assert!(!root.path().join("environment.txt").exists());
    Ok(())
}
#[cfg(feature = "kas")]
#[tokio::test]
async fn hanging_version_probe_fails_before_workbench_deadline() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().join("hanging-agent.sh");
    std::fs::write(
        &path,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'probe-auth-diagnostic' >&2; /bin/sleep 60; fi\n",
    )?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    let started = std::time::Instant::now();
    let bridge = spawn_bridge(
        AgentCommand::new(path.to_str().ok_or("fixture path")?),
        kas_config(root.path(), false)?,
        root.path().to_owned(),
    )?;
    let (_sender, mut notifications, _permissions, _sources, completion) = bridge.split();
    let reason = timeout(Duration::from_secs(10), async {
        while let Some(routed) = notifications.recv().await {
            if let Notification::BridgeDisconnected { reason } = routed.notification {
                return reason;
            }
        }
        "notifications closed before bridge failure".to_string()
    })
    .await
    .map_err(|_| "hanging version probe did not fail within deadline")?;
    timeout(Duration::from_secs(10), completion)
        .await?
        .map_err(|_| "bridge completion lost")?;
    assert!(reason.contains("probe-auth-diagnostic"), "{reason}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "probe not bounded"
    );
    Ok(())
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn free_replacement_refuses_before_inherited_discovery() -> TestResult {
    const ROOT: &str = "CYRIL_FREE_REPLACEMENT_FIXTURE";
    let Some(root) = std::env::var_os(ROOT) else {
        let root = tempfile::tempdir()?;
        let cli = root.path().join("kiro-cli");
        std::fs::write(
            &cli,
            "#!/bin/sh\nprintf '%s' \"$PARENT_ONLY_CANARY\" > \"$PROBE_JOURNAL\"\nprintf 'kiro-cli 2.21.1\\n'\n",
        )?;
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700))?;
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "free_replacement_refuses_before_inherited_discovery",
                "--nocapture",
            ])
            .env(ROOT, root.path())
            .env("HOME", root.path())
            .env("USERPROFILE", root.path())
            .env(
                "PATH",
                std::env::join_paths([root.path(), Path::new("/usr/bin"), Path::new("/bin")])?,
            )
            .env("PROBE_JOURNAL", root.path().join("probe.env"))
            .env("PARENT_ONLY_CANARY", "parent-only-authority")
            .env_remove("KIRO_KAS_SERVER_PATH")
            .env_remove("KIRO_AGENT_PATH")
            .env_remove("KIRO_HOME")
            .status()?;
        assert!(status.success(), "isolated free-path regression failed");
        assert_eq!(
            std::fs::read_to_string(root.path().join("completed"))?,
            "free discovery checked"
        );
        return Ok(());
    };
    let root = std::path::PathBuf::from(root);
    let config = SpawnConfig {
        engine: cyril_core::types::AgentEngine::Kas,
        kas_spawn: cyril_core::types::KasSpawn::Free,
        kas_hooks: cyril_core::types::kas_hooks::KasHooksMode::Off,
        shell: Some("bash".into()),
        ..SpawnConfig::default()
    };
    // Positive control: inherited Free discovery actually invokes our parent CLI.
    launch(&root, config.clone()).await?;
    let journal = root.join("probe.env");
    assert_eq!(std::fs::read_to_string(&journal)?, "parent-only-authority");
    std::fs::remove_file(&journal)?;
    launch(
        &root,
        SpawnConfig {
            environment: SpawnEnvironment::Replace(BTreeMap::new()),
            ..config
        },
    )
    .await?;
    assert!(
        !journal.exists(),
        "rejected launch executed the inherited version probe"
    );
    assert!(
        !root.join("initialize.json").exists(),
        "rejected launch spawned an agent"
    );
    // This nested process has a private HOME: ordinary inherited Host hooks
    // remain supported without loading the developer's real hook registry.
    launch(
        &root,
        SpawnConfig {
            engine: cyril_core::types::AgentEngine::Kas,
            kas_spawn: cyril_core::types::KasSpawn::Wrapper,
            kas_hooks: cyril_core::types::kas_hooks::KasHooksMode::Host,
            shell: Some("bash".into()),
            ..SpawnConfig::default()
        },
    )
    .await?;
    let initialize: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("initialize.json"))?)?;
    assert_eq!(
        initialize["params"]["clientCapabilities"]["_meta"]["kiro"]["hooks"]["enabled"],
        true
    );
    std::fs::write(root.join("completed"), "free discovery checked")?;
    Ok(())
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn verbose_version_probe_drains_both_pipes_past_capture_limit() -> TestResult {
    let root = tempfile::tempdir()?;
    let command = executable(root.path())?;
    let source = std::fs::read_to_string(command.program())?;
    // Both streams exceed the 1 MiB capture cap, not only OS pipe capacity.
    let noise = " ".repeat(2 * 1024 * 1024);
    std::fs::write(
        command.program(),
        source.replace(
            "exit 0",
            &format!("printf '%s' '{noise}'\nprintf '%s' '{noise}' >&2\nexit 0"),
        ),
    )?;
    let mut config = kas_config(root.path(), false)?;
    if let SpawnEnvironment::Replace(values) = &mut config.environment {
        values.insert(
            "PATH".into(),
            std::env::var_os("PATH").ok_or("PATH missing")?,
        );
    }
    let reason = launch_command(root.path(), config, command).await?;
    assert!(
        root.path().join("initialize.json").exists(),
        "verbose successful probe blocked agent startup: {reason}"
    );
    Ok(())
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn version_deadline_includes_exited_wrappers_inherited_pipes() -> TestResult {
    let root = tempfile::tempdir()?;
    let command = executable(root.path())?;
    let source = std::fs::read_to_string(command.program())?;
    std::fs::write(
        command.program(),
        source.replace(
            "exit 0",
            "/bin/sleep 60 &\nprintf '%s' \"$!\" > \"$DESCENDANT_PID\"\nexit 0",
        ),
    )?;
    let mut config = kas_config(root.path(), false)?;
    let pid_file = root.path().join("descendant.pid");
    if let SpawnEnvironment::Replace(values) = &mut config.environment {
        values.insert("DESCENDANT_PID".into(), pid_file.clone().into_os_string());
    }
    let started = std::time::Instant::now();
    launch_command(root.path(), config, command).await?;
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "inherited pipe bypassed deadline"
    );
    assert!(
        !root.path().join("initialize.json").exists(),
        "timed-out probe started an agent"
    );
    // The process group must be gone or contain only a killed, unreaped zombie.
    #[cfg(target_os = "linux")]
    {
        let pid = std::fs::read_to_string(pid_file)?.parse::<i32>()?;
        let stat = std::path::PathBuf::from(format!("/proc/{pid}/stat"));
        timeout(Duration::from_secs(2), async {
            loop {
                match std::fs::read_to_string(&stat) {
                    Ok(value)
                        if value
                            .rsplit_once(") ")
                            .is_some_and(|(_, fields)| fields.starts_with('Z')) =>
                    {
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                    Err(error) => return Err(error),
                    Ok(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
            Ok::<_, std::io::Error>(())
        })
        .await??;
    }
    Ok(())
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn replacement_host_hooks_refuse_before_probe_or_agent_start() -> TestResult {
    const ROOT: &str = "CYRIL_HOST_REPLACEMENT_FIXTURE";
    let Some(root) = std::env::var_os(ROOT) else {
        let root = tempfile::tempdir()?;
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "replacement_host_hooks_refuse_before_probe_or_agent_start",
                "--nocapture",
            ])
            .env(ROOT, root.path())
            .env("HOME", root.path())
            .env("USERPROFILE", root.path())
            .env_remove("KIRO_HOME")
            .env_remove("XDG_CONFIG_HOME")
            .status()?;
        assert!(status.success(), "nested hook-isolation assertions failed");
        assert_eq!(
            std::fs::read_to_string(root.path().join("completed"))?,
            "host hooks checked"
        );
        return Ok(());
    };
    let root = std::path::PathBuf::from(root);
    let mut config = kas_config(&root, false)?;
    config.kas_hooks = cyril_core::types::kas_hooks::KasHooksMode::Host;
    launch(&root, config).await?;
    assert!(!root.join("probe.env").exists());
    assert!(!root.join("environment.txt").exists());
    assert!(!root.join("initialize.json").exists());
    std::fs::write(root.join("completed"), "host hooks checked")?;
    Ok(())
}

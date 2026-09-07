//! Public bridge launch boundaries, observed by real POSIX executable fixtures.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::{AgentCommand, Notification, SpawnEnvironment};
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
    let bridge = spawn_bridge(executable(root)?, config, root.to_owned())?;
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
    if std::env::var_os(CANARY).is_none() {
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "isolated_spawn_does_not_inherit_parent_authority",
                "--nocapture",
            ])
            .env(CANARY, "parent-only-secret")
            .status()?;
        assert!(status.success());
        return Ok(());
    }
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
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then sleep 60; fi\n",
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
    assert!(reason.contains("version"), "{reason}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "probe not bounded"
    );
    Ok(())
}

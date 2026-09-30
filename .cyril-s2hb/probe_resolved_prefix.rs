use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::review::ShellDialect;
use cyril_core::types::{AgentCommand, AgentEngine, KasSpawn, SpawnEnvironment};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let shell = arguments.next().ok_or("missing configured shell choice")?;
    let expected = match arguments.next().as_deref() {
        Some("posix") => ShellDialect::Posix,
        Some("fish") => ShellDialect::Fish,
        Some("pwsh") => ShellDialect::Pwsh,
        Some("windows-powershell") => ShellDialect::WindowsPowerShell,
        _ => return Err("missing expected dialect".into()),
    };
    let workspace = tempfile::tempdir()?;
    let missing_agent = workspace.path().join("absent-agent");
    for engine in [AgentEngine::Kas, AgentEngine::V2] {
        let command = AgentCommand::try_from_argv(vec![
            missing_agent.to_str().ok_or("non-UTF8 fixture path")?.to_owned(),
        ])?;
        let bridge = spawn_bridge(
            command,
            SpawnConfig {
                engine,
                shell: Some(shell.clone()),
                kas_spawn: KasSpawn::Free,
                environment: SpawnEnvironment::Replace(Default::default()),
                ..Default::default()
            },
            workspace.path().to_owned(),
        )?;
        let expected = if cfg!(feature = "kas") && engine == AgentEngine::Kas {
            Some(expected)
        } else {
            None
        };
        assert_eq!(bridge.review_shell(), expected, "C8 resolved shell carry");
        let (sender, notifications, permissions, sources, completion) = bridge.split();
        drop((sender, notifications, permissions, sources));
        tokio::time::timeout(Duration::from_secs(3), completion).await??;
        println!("PASS C8 resolved {engine:?} {shell:?}: {expected:?}; bridge completed");
    }
    Ok(())
}

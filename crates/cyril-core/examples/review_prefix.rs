//! Qualify resolved dialect carry and generated-prefix execution on a native host.
use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::review::{CrtoolPrefix, ShellDialect};
use cyril_core::types::{AgentCommand, AgentEngine, KasSpawn, SpawnEnvironment};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const MARKER_ENV: &str = "CYRIL_REVIEW_PREFIX_MARKER";
const MARKER: &[u8; 7] = b"crtool\n";

fn main() -> Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next().ok_or("missing shell or crtool argument")?;
    let second = arguments.next();
    if arguments.next().is_some() {
        return Err("expected exactly crtool, or <shell> <dialect>".into());
    }
    // The parent's marker selects child role independently of possibly bad argv.
    if let Some(marker) = std::env::var_os(MARKER_ENV) {
        if first != "crtool" || second.is_some() {
            return Err("child invocation must contain only crtool".into());
        }
        std::fs::write(marker, MARKER)?;
        return Ok(());
    }
    let shell = first.to_str().ok_or("shell choice is not UTF-8")?;
    let expected = second.ok_or("missing independently expected dialect")?;
    let expected = match expected.to_str() {
        Some("posix") => ShellDialect::Posix,
        Some("fish") => ShellDialect::Fish,
        Some("pwsh") => ShellDialect::Pwsh,
        Some("windows-powershell") => ShellDialect::WindowsPowerShell,
        _ => return Err("unsupported expected dialect".into()),
    };
    println!("RUN C8: {shell} {expected:?}");
    match qualify(shell, expected) {
        Ok(()) => {
            println!("PASS C8: {shell} {expected:?}; exact child argv and both bridges completed");
            Ok(())
        }
        Err(error) => {
            eprintln!("FAIL C8: {error}");
            Err(error)
        }
    }
}

fn qualify(shell: &str, expected: ShellDialect) -> Result<()> {
    if !cfg!(feature = "kas") {
        return Err("native prefix qualification requires the kas feature".into());
    }
    let workspace = tempfile::tempdir()?;
    let missing_agent = workspace.path().join("absent-agent");
    let marker = workspace.path().join("prefix-marker");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        for engine in [AgentEngine::Kas, AgentEngine::V2] {
            let command = AgentCommand::try_from_argv(vec![
                missing_agent
                    .to_str()
                    .ok_or("non-UTF8 fixture path")?
                    .to_owned(),
            ])?;
            let bridge = spawn_bridge(
                command,
                SpawnConfig {
                    engine,
                    shell: Some(shell.to_owned()),
                    // This invalid combination prevents launching an agent.
                    kas_spawn: KasSpawn::Free,
                    environment: SpawnEnvironment::Replace(Default::default()),
                    ..Default::default()
                },
                workspace.path().to_owned(),
            )?;
            let actual = bridge.review_shell();
            let (sender, notifications, permissions, sources, completion) = bridge.split();
            drop((sender, notifications, permissions, sources));
            tokio::time::timeout(Duration::from_secs(3), completion).await??;
            let expected_shell = (engine == AgentEngine::Kas).then_some(expected);
            if actual != expected_shell {
                return Err(
                    format!("{engine:?} resolved {actual:?}, expected {expected_shell:?}").into(),
                );
            }
            if let Some(dialect) = actual {
                let prefix = CrtoolPrefix::current(dialect)?;
                execute_prefix(shell, dialect, prefix.as_str(), &marker)?;
            }
        }
        Ok(())
    })
}

fn execute_prefix(shell: &str, dialect: ShellDialect, prefix: &str, marker: &Path) -> Result<()> {
    let mut command = Command::new(shell);
    match dialect {
        ShellDialect::Posix | ShellDialect::Fish => {
            command.args(["-c", prefix]);
        }
        ShellDialect::Pwsh | ShellDialect::WindowsPowerShell => {
            command.args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                prefix,
            ]);
        }
    }
    let status = command.env(MARKER_ENV, marker).status()?;
    if !status.success() {
        return Err(format!("{shell} failed to execute {prefix:?}: {status}").into());
    }
    // Exact bytes and EOF avoid both newline conversion and unbounded reads.
    let mut file = std::fs::File::open(marker)?;
    let mut bytes = [0; MARKER.len()];
    file.read_exact(&mut bytes)?;
    if &bytes != MARKER || file.read(&mut [0])? != 0 {
        return Err("generated prefix did not invoke exactly the crtool child".into());
    }
    Ok(())
}

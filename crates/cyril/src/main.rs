mod app;

use std::path::PathBuf;

use clap::Parser;
use cyril_core::types::AgentEngine;

#[derive(Parser)]
#[command(
    name = "cyril",
    about = "Polished TUI for the Agent Client Protocol ecosystem"
)]
struct Cli {
    /// Working directory
    #[arg(short = 'd', long = "cwd")]
    cwd: Option<PathBuf>,

    /// Send a one-shot prompt
    #[arg(long)]
    prompt: Option<String>,

    /// Command line for the ACP agent. First value is the program; remaining
    /// values are arguments. Defaults to `kiro-cli acp`.
    #[arg(
        long = "agent-command",
        num_args = 1..,
        default_values_t = vec!["kiro-cli".to_string(), "acp".to_string()],
    )]
    agent_command: Vec<String>,

    /// Which Kiro engine to drive: `v2` (default) or `kas`. Overrides
    /// `[agent] engine` in config. KAS is not available until KAS-1.
    #[arg(long = "agent-engine")]
    agent_engine: Option<AgentEngine>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    setup_logging();

    let cwd = cli
        .cwd
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let config =
        cyril_core::types::config::Config::load_from_path(&config_dir().join("config.toml"));

    // Spawn bridge
    let agent_command = cyril_core::types::AgentCommand::try_from_argv(cli.agent_command)?;
    // The `--agent-engine` flag overrides `[agent] engine` in config; config
    // defaults to v2 (KAS-0, ADR-0002).
    let agent_engine = cli.agent_engine.unwrap_or(config.agent.engine);
    let bridge =
        cyril_core::protocol::bridge::spawn_bridge(agent_command, agent_engine, cwd.clone())?;

    // Build and run TUI
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let mut app = app::App::new(bridge, config.ui.max_messages, cwd.clone());

        // Create initial session
        app.create_initial_session(cwd).await;

        // Initialize terminal
        let mut terminal = ratatui::init();
        crossterm::execute!(
            std::io::stdout(),
            crossterm::event::EnableMouseCapture,
            crossterm::event::EnableBracketedPaste,
        )
        .map_err(|e| {
            cyril_core::Error::with_source(
                cyril_core::ErrorKind::Transport {
                    detail: "failed to enable mouse capture / bracketed paste".into(),
                },
                e,
            )
        })?;

        let result = app.run(&mut terminal).await;

        // Restore terminal
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableBracketedPaste,
        ) {
            tracing::warn!(error = %e, "failed to disable mouse capture / bracketed paste");
        }
        ratatui::restore();

        if let Err(ref e) = result {
            eprintln!("Error: {e}");
        }

        result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    })?;

    Ok(())
}

fn setup_logging() {
    let log_dir = config_dir();
    // Ensure config directory exists
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: could not create log directory: {e}");
        return;
    }

    let log_path = log_dir.join("cyril.log");

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .json()
            .init();
    }
}

fn config_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("cyril")
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home).join(".config").join("cyril")
    } else {
        PathBuf::from(".cyril")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    // Slice 5 (D7 parse table): no flag -> None (config supplies the default);
    // `--agent-engine kas` -> Some(Kas); an unknown value is REJECTED at parse
    // time, never silently defaulted.
    #[test]
    fn cli_agent_engine_flag() {
        let none = Cli::try_parse_from(["cyril"]).expect("parses with no engine flag");
        assert_eq!(none.agent_engine, None);

        let kas = Cli::try_parse_from(["cyril", "--agent-engine", "kas"])
            .expect("parses --agent-engine kas");
        assert_eq!(kas.agent_engine, Some(AgentEngine::Kas));

        assert!(
            Cli::try_parse_from(["cyril", "--agent-engine", "bogus"]).is_err(),
            "an unknown engine value is rejected, not silently defaulted"
        );
    }
}

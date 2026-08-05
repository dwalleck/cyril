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

    /// Which Kiro engine to drive: `v2` (default) or `kas` (`v3` is accepted
    /// as an alias for `kas`). Overrides `[agent] engine` in config.
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
    // KAS spawn shape (KAS-1): `[agent] kas_spawn` (free | wrapper); free default.
    let bridge = cyril_core::protocol::bridge::spawn_bridge(
        agent_command,
        cyril_core::protocol::bridge::SpawnConfig {
            engine: agent_engine,
            kas_spawn: config.agent.kas_spawn,
            shell: config.agent.shell,
            present_as: config.agent.present_as,
            kas_hooks: config.agent.kas_hooks,
        },
        cwd.clone(),
    )?;

    // Bound outside the async block: `cli` is already partially moved (cwd,
    // agent_command), and an async block would capture the whole struct.
    let oneshot_prompt = cli.prompt;

    // Build and run TUI
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        // Who answers `/hooks` depends on which side owns a hook registry
        // (cyril-gk17) — resolved once here, from the same two values the
        // bridge was spawned with.
        let hooks_source = cyril_core::commands::HooksCommandSource::resolve(
            agent_engine,
            config.agent.kas_hooks,
            cwd.clone(),
        );
        let mut app = app::App::new(bridge, &config.ui, cwd.clone(), hooks_source);

        // Create initial session; a parsed `--prompt` rides along and is
        // submitted once the session is ready (cyril-0ffy).
        app.create_initial_session(cwd, oneshot_prompt).await;

        // Initialize terminal
        let mut terminal = ratatui::init();
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste).map_err(
            |e| {
                cyril_core::Error::with_source(
                    cyril_core::ErrorKind::Transport {
                        detail: "failed to enable bracketed paste".into(),
                    },
                    e,
                )
            },
        )?;

        // Mouse capture follows `ui.mouse_capture` (cyril-nd4h). Derived from
        // App's state, NOT read from the config again: two independent reads is
        // exactly how this flag and the terminal drift apart, which shows up as
        // a first Ctrl+M press that appears to do nothing. The restore path
        // below disables unconditionally, which is a no-op when never enabled.
        //
        // Non-fatal on purpose, matching the runtime Ctrl+M handler: a terminal
        // that refuses mouse capture should not stop cyril from starting. `?`
        // here would also abandon the terminal mid-setup — past `ratatui::init()`
        // and with bracketed paste already on — skipping the restore below.
        // Rolling the flag back keeps UiState in agreement with the terminal.
        if app.mouse_captured()
            && let Err(e) =
                crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)
        {
            tracing::warn!(error = %e, "failed to enable mouse capture; continuing without it");
            app.set_mouse_captured(false);
        }

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

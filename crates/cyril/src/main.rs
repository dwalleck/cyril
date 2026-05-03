mod app;

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "cyril", about = "Polished TUI for the Agent Client Protocol ecosystem")]
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
    let bridge = cyril_core::protocol::bridge::spawn_bridge(cli.agent_command, cwd.clone())?;

    // Build and run TUI
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let mut app = app::App::new(bridge, config.ui.max_messages);

        // Create initial session
        app.create_initial_session(cwd).await;

        // Initialize terminal
        let mut terminal = ratatui::init();
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture,).map_err(
            |e| {
                cyril_core::Error::with_source(
                    cyril_core::ErrorKind::Transport {
                        detail: "failed to enable mouse capture".into(),
                    },
                    e,
                )
            },
        )?;

        let result = app.run(&mut terminal).await;

        // Restore terminal
        if let Err(e) =
            crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture,)
        {
            tracing::warn!(error = %e, "failed to disable mouse capture");
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

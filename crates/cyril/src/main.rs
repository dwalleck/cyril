mod app;
mod commands;
mod event;
mod file_completer;
mod tui;
mod ui;

use std::path::PathBuf;
use std::rc::Rc;

use agent_client_protocol::{self as acp, Agent};
use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::mpsc;

use cyril_core::client::KiroClient;
use cyril_core::event::{AppEvent, InteractionRequest, ProtocolEvent};
use cyril_core::hooks::{self, HookRegistry};
use cyril_core::path;
use cyril_core::transport::AgentProcess;

#[derive(Parser)]
#[command(name = "cyril", about = "Windows-native ACP client for Kiro CLI")]
struct Cli {
    /// Working directory (defaults to current directory)
    #[arg(short = 'd', long)]
    cwd: Option<PathBuf>,

    /// Initial prompt to send (non-interactive mode)
    #[arg(short, long)]
    prompt: Option<String>,

    /// Agent to use (e.g. "sonnet", "claude-sonnet-4-0")
    #[arg(short, long)]
    agent: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Log to file to avoid TUI conflicts
    match std::fs::OpenOptions::new().create(true).append(true).open("cyril.log") {
        Ok(file) => {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .with_writer(std::sync::Mutex::new(file))
                .with_ansi(false)
                .init();
        }
        Err(e) => {
            eprintln!("Warning: Failed to open cyril.log for logging: {e}");
            eprintln!("Diagnostics will be unavailable for this session.");
        }
    }

    let cli = Cli::parse();
    let cwd = match cli.cwd {
        Some(d) => d,
        None => std::env::current_dir()
            .context("Failed to determine current directory. Specify one with --cwd.")?,
    };

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            if let Some(prompt) = cli.prompt {
                run_oneshot(cwd, prompt, cli.agent).await
            } else {
                run_tui(cwd, cli.agent).await
            }
        })
        .await
}

/// Non-interactive mode: send a single prompt and print the response.
async fn run_oneshot(cwd: PathBuf, prompt_text: String, agent: Option<String>) -> Result<()> {
    let (conn, event_rx, _agent) = connect(agent.as_deref()).await?;

    let agent_cwd = path::to_agent(&cwd);
    let session_response = conn
        .new_session(acp::NewSessionRequest::new(agent_cwd))
        .await
        .context("Failed to create session")?;

    let mut event_rx = event_rx;
    let printer = tokio::task::spawn_local(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AppEvent::Protocol(ProtocolEvent::AgentMessage { chunk, .. }) => {
                    if let acp::ContentBlock::Text(text) = &chunk.content {
                        eprint!("{}", text.text);
                    }
                }
                AppEvent::Interaction(InteractionRequest::Permission { request, responder }) => {
                    let option_id = request
                        .options
                        .iter()
                        .find(|o| matches!(o.kind, acp::PermissionOptionKind::AllowOnce))
                        .or_else(|| request.options.first())
                        .map(|o| o.option_id.clone());

                    let outcome = match option_id {
                        Some(id) => acp::RequestPermissionOutcome::Selected(
                            acp::SelectedPermissionOutcome::new(id),
                        ),
                        None => {
                            eprintln!("Warning: permission request with no options, cancelling");
                            acp::RequestPermissionOutcome::Cancelled
                        }
                    };
                    let _ = responder.send(acp::RequestPermissionResponse::new(outcome));
                }
                _ => {}
            }
        }
    });

    let result = conn
        .prompt(acp::PromptRequest::new(
            session_response.session_id,
            vec![acp::ContentBlock::Text(acp::TextContent::new(prompt_text))],
        ))
        .await
        .context("Prompt failed")?;

    eprintln!("\n[{:?}]", result.stop_reason);
    let _ = printer.await;
    Ok(())
}

/// Interactive TUI mode.
async fn run_tui(cwd: PathBuf, agent: Option<String>) -> Result<()> {
    let (conn, event_rx, _agent) = connect(agent.as_deref()).await?;
    let conn = Rc::new(conn);

    let mut terminal = tui::init()?;
    let mut app = app::App::new(conn.clone(), cwd.clone(), event_rx);
    app.toolbar.selected_agent = agent;
    app.load_project_files().await;

    let agent_cwd = path::to_agent(&cwd);
    let session_response = conn
        .new_session(acp::NewSessionRequest::new(agent_cwd))
        .await
        .context("Failed to create session")?;

    if let Some(ref modes) = session_response.modes {
        app.session.set_modes(modes);
    }

    if let Some(config_options) = session_response.config_options {
        tracing::info!(
            "NewSessionResponse config_options: {}",
            serde_json::to_string_pretty(&config_options).unwrap_or_default()
        );
        app.session.set_config_options(config_options);
    }

    app.session.set_session_id(session_response.session_id);

    let result = app.run(&mut terminal).await;

    tui::restore()?;

    result
}

/// Connect to the agent subprocess and perform the ACP handshake.
/// Returns (connection, event_rx, agent_handle).
/// The agent handle must be kept alive for the duration of the session.
async fn connect(
    agent_name: Option<&str>,
) -> Result<(
    acp::ClientSideConnection,
    mpsc::UnboundedReceiver<AppEvent>,
    AgentProcess,
)> {
    let mut agent = AgentProcess::spawn(agent_name)?;
    agent.check_startup().await?;

    let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut hook_registry = HookRegistry::new();

    // Load user-configured hooks from hooks.json in cwd
    let hooks_path = std::env::current_dir()
        .unwrap_or_default()
        .join("hooks.json");
    if hooks_path.exists() {
        match hooks::load_hooks_config(&hooks_path) {
            Ok(loaded) => {
                tracing::info!("Loaded {} hooks from {}", loaded.len(), hooks_path.display());
                for hook in loaded {
                    hook_registry.register(hook);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to load hooks config: {e}");
            }
        }
    }

    let client = KiroClient::new(event_tx, hook_registry);

    let stdin = agent.take_stdin()?;
    let stdout = agent.take_stdout()?;

    let (conn, handle_io) = acp::ClientSideConnection::new(
        client,
        stdin,
        stdout,
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );
    tokio::task::spawn_local(async move {
        if let Err(e) = handle_io.await {
            tracing::error!("ACP I/O error: {e}");
        }
    });

    let init_response = conn
        .initialize(
            acp::InitializeRequest::new(acp::ProtocolVersion::V1)
                .client_capabilities(
                    acp::ClientCapabilities::new()
                        .fs(
                            acp::FileSystemCapability::new()
                                .read_text_file(true)
                                .write_text_file(true),
                        )
                        .terminal(true),
                )
                .client_info(
                    acp::Implementation::new("cyril", env!("CARGO_PKG_VERSION"))
                        .title("Cyril"),
                ),
        )
        .await
        .context("ACP initialize failed")?;

    if let Some(ref info) = init_response.agent_info {
        tracing::info!("Connected to {} v{}", info.name, info.version);
    }

    Ok((conn, event_rx, agent))
}

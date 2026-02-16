mod app;
mod event;
mod tui;
mod ui;

use std::path::PathBuf;
use std::rc::Rc;

use agent_client_protocol::{self as acp, Agent};
use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::mpsc;

use win_kiro_core::client::KiroClient;
use win_kiro_core::event::AppEvent;
use win_kiro_core::hooks::HookRegistry;
use win_kiro_core::path;
use win_kiro_core::transport::AgentProcess;

#[derive(Parser)]
#[command(name = "win-kiro", about = "Windows-native ACP client for Kiro CLI")]
struct Cli {
    /// Working directory (defaults to current directory)
    #[arg(short = 'd', long)]
    cwd: Option<PathBuf>,

    /// Initial prompt to send (non-interactive mode)
    #[arg(short, long)]
    prompt: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Log to file to avoid TUI conflicts
    if let Ok(file) = std::fs::File::create("win-kiro.log") {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .init();
    }

    let cli = Cli::parse();
    let cwd = cli.cwd.unwrap_or_else(|| std::env::current_dir().expect("Failed to get cwd"));

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            if let Some(prompt) = cli.prompt {
                run_oneshot(cwd, prompt).await
            } else {
                run_tui(cwd).await
            }
        })
        .await
}

/// Non-interactive mode: send a single prompt and print the response.
async fn run_oneshot(cwd: PathBuf, prompt_text: String) -> Result<()> {
    let (conn, event_rx, _agent) = connect().await?;

    let wsl_cwd = path::win_to_wsl(&cwd);
    let session_response = conn
        .new_session(acp::NewSessionRequest::new(wsl_cwd))
        .await
        .context("Failed to create session")?;

    let mut event_rx = event_rx;
    let printer = tokio::task::spawn_local(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AppEvent::AgentMessage { chunk, .. } => {
                    if let acp::ContentBlock::Text(text) = &chunk.content {
                        eprint!("{}", text.text);
                    }
                }
                AppEvent::PermissionRequest { request, responder } => {
                    let option_id = request
                        .options
                        .iter()
                        .find(|o| matches!(o.kind, acp::PermissionOptionKind::AllowOnce))
                        .map(|o| o.option_id.clone())
                        .unwrap_or_else(|| request.options[0].option_id.clone());
                    let _ = responder.send(acp::RequestPermissionResponse::new(
                        acp::RequestPermissionOutcome::Selected(
                            acp::SelectedPermissionOutcome::new(option_id),
                        ),
                    ));
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
async fn run_tui(cwd: PathBuf) -> Result<()> {
    let (conn, event_rx, _agent) = connect().await?;
    let conn = Rc::new(conn);

    let mut terminal = tui::init()?;
    let mut app = app::App::new(conn.clone(), event_rx);

    let wsl_cwd = path::win_to_wsl(&cwd);
    let session_response = conn
        .new_session(acp::NewSessionRequest::new(wsl_cwd))
        .await
        .context("Failed to create session")?;

    app.set_session_id(session_response.session_id);

    let result = app.run(&mut terminal).await;

    tui::restore()?;

    result
}

/// Connect to the WSL agent and perform the ACP handshake.
/// Returns (connection, event_rx, agent_handle).
/// The agent handle must be kept alive for the duration of the session.
async fn connect() -> Result<(
    acp::ClientSideConnection,
    mpsc::UnboundedReceiver<AppEvent>,
    AgentProcess,
)> {
    let mut agent = AgentProcess::spawn()?;
    agent.check_startup().await?;

    let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let hooks = HookRegistry::new();
    let client = KiroClient::new(event_tx, hooks);

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
                    acp::Implementation::new("win-kiro", env!("CARGO_PKG_VERSION"))
                        .title("Win Kiro"),
                ),
        )
        .await
        .context("ACP initialize failed")?;

    if let Some(ref info) = init_response.agent_info {
        tracing::info!("Connected to {} v{}", info.name, info.version);
    }

    Ok((conn, event_rx, agent))
}

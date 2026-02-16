use std::path::PathBuf;

use agent_client_protocol::{self as acp, Agent};
use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::mpsc;

use win_kiro_core::client::KiroClient;
use win_kiro_core::event::AppEvent;
use win_kiro_core::hooks::HookRegistry;
use win_kiro_core::path;
use win_kiro_core::session::SessionState;
use win_kiro_core::transport::AgentProcess;

#[derive(Parser)]
#[command(name = "win-kiro", about = "Windows-native ACP client for Kiro CLI")]
struct Cli {
    /// Working directory (defaults to current directory)
    #[arg(short = 'd', long)]
    cwd: Option<PathBuf>,

    /// Initial prompt to send (if omitted, runs interactively)
    #[arg(short, long)]
    prompt: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let cwd = cli.cwd.unwrap_or_else(|| std::env::current_dir().expect("Failed to get cwd"));

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            run(cwd, cli.prompt).await
        })
        .await
}

async fn run(cwd: PathBuf, initial_prompt: Option<String>) -> Result<()> {
    // 1. Spawn the WSL agent process
    eprintln!("Spawning wsl kiro-cli acp...");
    let mut agent = AgentProcess::spawn()?;
    agent.check_startup().await?;

    // 2. Set up the event channel
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    // 3. Create the Client impl
    let hooks = HookRegistry::new();
    let client = KiroClient::new(event_tx, hooks);

    // 4. Create the ACP connection
    let (conn, handle_io) = acp::ClientSideConnection::new(
        client,
        agent.stdin,
        agent.stdout,
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );
    tokio::task::spawn_local(async move {
        if let Err(e) = handle_io.await {
            eprintln!("ACP I/O error: {e}");
        }
    });

    // 5. Initialize
    eprintln!("Initializing ACP connection...");
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
                .client_info(acp::Implementation::new(
                    "win-kiro",
                    env!("CARGO_PKG_VERSION"),
                ).title("Win Kiro")),
        )
        .await
        .context("ACP initialize failed")?;

    if let Some(ref info) = init_response.agent_info {
        eprintln!(
            "Connected to agent: {} v{}",
            info.name,
            info.version
        );
    }

    let mut session = SessionState::new(cwd.clone());
    session.agent_info = init_response.agent_info;
    session.agent_capabilities = Some(init_response.agent_capabilities);

    // 6. Create a new session
    let wsl_cwd = path::win_to_wsl(&cwd);
    eprintln!("Creating session (cwd: {} -> {})...", cwd.display(), wsl_cwd.display());
    let session_response = conn
        .new_session(acp::NewSessionRequest::new(wsl_cwd))
        .await
        .context("Failed to create session")?;

    session.session_id = Some(session_response.session_id.clone());
    eprintln!("Session created: {}", session_response.session_id);

    // 7. Send prompt
    let prompt_text = initial_prompt.unwrap_or_else(|| "Hello! What can you help me with?".to_string());
    eprintln!("Sending prompt: {prompt_text}");

    // Spawn a task to drain events and print them while prompt is in flight
    let event_printer = tokio::task::spawn_local(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AppEvent::AgentMessage { chunk, .. } => {
                    if let acp::ContentBlock::Text(text) = &chunk.content {
                        eprint!("{}", text.text);
                    }
                }
                AppEvent::AgentThought { chunk, .. } => {
                    if let acp::ContentBlock::Text(text) = &chunk.content {
                        eprint!("[thought] {}", text.text);
                    }
                }
                AppEvent::ToolCallStarted { tool_call, .. } => {
                    eprintln!("\n[tool] {} ({})", tool_call.title, tool_call.tool_call_id);
                }
                AppEvent::ToolCallUpdated { update, .. } => {
                    eprintln!(
                        "[tool update] {} -> {:?}",
                        update.tool_call_id,
                        update.fields.status
                    );
                }
                AppEvent::PermissionRequest { request, responder } => {
                    eprintln!("\n[permission] {:?}", request.tool_call);
                    // Auto-approve in MVP mode: pick first AllowOnce option
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
                _ => {
                    eprintln!("[event] {:?}", std::mem::discriminant(&event));
                }
            }
        }
    });

    let prompt_result = conn
        .prompt(acp::PromptRequest::new(
            session_response.session_id,
            vec![acp::ContentBlock::Text(acp::TextContent::new(prompt_text))],
        ))
        .await
        .context("Prompt failed")?;

    eprintln!("\nPrompt completed: {:?}", prompt_result.stop_reason);

    // Wait for event printer to finish
    let _ = event_printer.await;

    Ok(())
}

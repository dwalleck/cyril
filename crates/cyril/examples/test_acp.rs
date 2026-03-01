//! Headless test harness for ACP session methods.
//!
//! Connects to kiro-cli via WSL, creates a session, and probes:
//!   - session/set_mode
//!   - session/set_config_option (for model selection)
//!
//! Usage: cargo run --example test_acp [-- --agent <name>]

use agent_client_protocol::{self as acp, Agent};
use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::mpsc;

use cyril_core::client::KiroClient;
use cyril_core::event::{AppEvent, ExtensionEvent, ProtocolEvent};
use cyril_core::hooks::HookRegistry;
use cyril_core::transport::AgentProcess;

#[derive(Parser)]
#[command(name = "test_acp", about = "Headless ACP method tester")]
struct Cli {
    /// Agent to use (e.g. "sonnet", "dotnet-dev")
    #[arg(short, long)]
    agent: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Log to stderr so we can see tracing alongside our println output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .init();

    let cli = Cli::parse();

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move { run_tests(cli.agent.as_deref()).await })
        .await
}

/// Helper: print a SessionConfigOption's current value and options.
fn print_config_option(opt: &acp::SessionConfigOption) {
    match &opt.kind {
        acp::SessionConfigKind::Select(select) => {
            println!("    Config: id={}, name={}, current={}", opt.id, opt.name, select.current_value);
            match &select.options {
                acp::SessionConfigSelectOptions::Ungrouped(opts) => {
                    for val in opts {
                        println!("      option: {} ({})", val.value, val.name);
                    }
                }
                acp::SessionConfigSelectOptions::Grouped(groups) => {
                    for group in groups {
                        println!("      group: {}", group.name);
                    }
                }
                _ => {
                    println!("      (unknown options variant)");
                }
            }
        }
        _ => {
            println!("    Config: id={}, name={} (unknown kind)", opt.id, opt.name);
        }
    }
}

async fn run_tests(agent_name: Option<&str>) -> Result<()> {
    println!("=== ACP Method Tester ===\n");

    // --- Connect ---
    println!("[1] Spawning kiro-cli...");
    let mut agent = AgentProcess::spawn(agent_name)?;
    agent.check_startup().await?;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let client = KiroClient::new(event_tx, HookRegistry::new());

    let stdin = agent.take_stdin()?;
    let stdout = agent.take_stdout()?;

    let (conn, handle_io) = acp::ClientSideConnection::new(
        client,
        stdin,
        stdout,
        |fut| { tokio::task::spawn_local(fut); },
    );
    tokio::task::spawn_local(async move {
        if let Err(e) = handle_io.await {
            tracing::error!("ACP I/O error: {e}");
        }
    });

    // Drain events in background so the connection doesn't block
    tokio::task::spawn_local(async move {
        while let Some(event) = event_rx.recv().await {
            match &event {
                AppEvent::Protocol(ProtocolEvent::ModeChanged { mode, .. }) => {
                    println!("  [event] ModeChanged: {:?}", mode);
                }
                AppEvent::Extension(ExtensionEvent::KiroCommandsAvailable { commands }) => {
                    println!("  [event] KiroCommandsAvailable: {} commands", commands.len());
                }
                AppEvent::Extension(ExtensionEvent::KiroMetadata { context_usage_pct, .. }) => {
                    println!("  [event] KiroMetadata: context={context_usage_pct}%");
                }
                _ => {
                    tracing::debug!("  [event] {:?}", event);
                }
            }
        }
    });

    // --- Initialize ---
    println!("[2] Initializing ACP connection...");
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
                    acp::Implementation::new("test_acp", env!("CARGO_PKG_VERSION"))
                        .title("ACP Test Harness"),
                ),
        )
        .await
        .context("ACP initialize failed")?;

    if let Some(ref info) = init_response.agent_info {
        println!("    Connected to: {} v{}", info.name, info.version);
    }
    println!(
        "    Agent capabilities: {}",
        serde_json::to_string(&init_response.agent_capabilities)
            .unwrap_or_else(|_| "??".into())
    );
    println!();

    // --- Create session ---
    println!("[3] Creating new session...");
    let cwd = std::env::current_dir().unwrap_or_default();
    let wsl_cwd = cyril_core::path::win_to_wsl(&cwd);
    let session_response = conn
        .new_session(acp::NewSessionRequest::new(wsl_cwd))
        .await
        .context("Failed to create session")?;

    let session_id = session_response.session_id.clone();
    println!("    Session ID: {session_id}");

    // Print modes
    if let Some(ref modes) = session_response.modes {
        println!("    Current mode: {}", modes.current_mode_id);
        println!("    Available modes:");
        for mode in &modes.available_modes {
            println!("      - {} ({})", mode.id, mode.name);
        }
    } else {
        println!("    Modes: not advertised");
    }

    // Print config options
    if let Some(ref config_options) = session_response.config_options {
        println!("    Config options from NewSessionResponse:");
        for opt in config_options {
            print_config_option(opt);
        }
    } else {
        println!("    Config options: not advertised");
    }
    println!();

    // Give the agent a moment to send any initial notifications
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // --- Test session/set_mode ---
    println!("[4] Testing session/set_mode...");
    if let Some(ref modes) = session_response.modes {
        for mode in &modes.available_modes {
            let mode_id = mode.id.to_string();
            println!("    Trying set_mode(\"{mode_id}\")");

            match conn
                .set_session_mode(acp::SetSessionModeRequest::new(
                    session_id.clone(),
                    mode_id.clone(),
                ))
                .await
            {
                Ok(resp) => {
                    println!("    SUCCESS: set_mode(\"{mode_id}\") -> meta={:?}", resp.meta);
                }
                Err(e) => {
                    println!("    FAILED: set_mode(\"{mode_id}\") -> {e}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    } else {
        for mode_id in &["code", "architect", "ask"] {
            println!("    Trying set_mode(\"{mode_id}\") (guessing, none advertised)");
            match conn
                .set_session_mode(acp::SetSessionModeRequest::new(
                    session_id.clone(),
                    mode_id.to_string(),
                ))
                .await
            {
                Ok(resp) => {
                    println!("    SUCCESS: set_mode(\"{mode_id}\") -> meta={:?}", resp.meta);
                }
                Err(e) => {
                    println!("    FAILED: set_mode(\"{mode_id}\") -> {e}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    println!();

    // --- Test session/set_config_option ---
    println!("[5] Testing session/set_config_option...");
    if let Some(ref config_options) = session_response.config_options {
        for opt in config_options {
            if let acp::SessionConfigKind::Select(ref select) = opt.kind {
                println!("    Config '{}': current={}", opt.id, select.current_value);
                if let acp::SessionConfigSelectOptions::Ungrouped(ref opts) = select.options {
                    // Try setting to each value
                    for val in opts {
                        println!("      Trying set_config_option(id={}, value={})", opt.id, val.value);
                        match conn
                            .set_session_config_option(acp::SetSessionConfigOptionRequest::new(
                                session_id.clone(),
                                opt.id.to_string(),
                                val.value.to_string(),
                            ))
                            .await
                        {
                            Ok(resp) => {
                                println!("      SUCCESS -> returned {} config options", resp.config_options.len());
                                for updated in &resp.config_options {
                                    if let acp::SessionConfigKind::Select(ref s) = updated.kind {
                                        println!("        {} = {}", updated.id, s.current_value);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("      FAILED -> {e}");
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }
    } else {
        println!("    No config_options advertised. Trying common IDs...");
        for (config_id, value) in &[("model", "claude-sonnet-4-0"), ("mode", "code")] {
            println!("    Trying set_config_option(id={config_id}, value={value})");
            match conn
                .set_session_config_option(acp::SetSessionConfigOptionRequest::new(
                    session_id.clone(),
                    config_id.to_string(),
                    value.to_string(),
                ))
                .await
            {
                Ok(resp) => {
                    println!("    SUCCESS -> returned {} config options", resp.config_options.len());
                    for updated in &resp.config_options {
                        if let acp::SessionConfigKind::Select(ref s) = updated.kind {
                            println!("      {} = {}", updated.id, s.current_value);
                        }
                    }
                }
                Err(e) => {
                    println!("    FAILED -> {e}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    println!();

    println!("=== Done ===");
    Ok(())
}

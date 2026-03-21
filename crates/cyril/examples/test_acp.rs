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
    println!("[1] Spawning kiro-cli (verbose)...");
    let mut agent = AgentProcess::spawn_with_extra_args(agent_name, &["--verbose"])?;
    agent.check_startup().await?;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let client = KiroClient::new(event_tx, HookRegistry::new());

    let stdin = agent.take_stdin()?;
    let stdout = agent.take_stdout()?;

    let (conn, handle_io) = acp::ClientSideConnection::new(client, stdin, stdout, |fut| {
        tokio::task::spawn_local(fut);
    });
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
                AppEvent::Protocol(ProtocolEvent::ConfigOptionsUpdated { config_options, .. }) => {
                    println!("  [event] ConfigOptionsUpdated: {} options", config_options.len());
                    for opt in config_options {
                        print_config_option(opt);
                    }
                }
                AppEvent::Extension(ExtensionEvent::KiroCommandsAvailable { commands }) => {
                    println!("  [event] KiroCommandsAvailable: {} commands", commands.len());
                    for cmd in commands {
                        let meta_str = match &cmd.meta {
                            Some(meta) => {
                                let mut parts = Vec::new();
                                if let Some(ref it) = meta.input_type {
                                    parts.push(format!("inputType={it}"));
                                }
                                if let Some(ref om) = meta.options_method {
                                    parts.push(format!("optionsMethod={om}"));
                                }
                                if meta.local {
                                    parts.push("local".to_string());
                                }
                                format!(" [{}]", parts.join(", "))
                            }
                            None => String::new(),
                        };
                        let hint = cmd.input_hint.as_deref().unwrap_or("");
                        println!(
                            "    {:<20} {:30} {hint}{meta_str}",
                            cmd.name, cmd.description
                        );
                    }
                }
                AppEvent::Extension(ExtensionEvent::KiroMetadata { context_usage_pct, .. }) => {
                    println!("  [event] KiroMetadata: context={context_usage_pct}%");
                }
                AppEvent::Extension(ExtensionEvent::Unknown { method, params }) => {
                    println!("  [event] UNKNOWN EXT: method={method}");
                    println!("          params={params}");
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

    // Give the agent a moment to send initial notifications (commands, metadata)
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // --- Test kiro.dev/commands/options for model ---
    println!("[4] Testing kiro.dev/commands/options (model)...");
    {
        let params = serde_json::json!({
            "command": "model",
            "sessionId": session_id.to_string()
        });
        let raw_params = serde_json::value::RawValue::from_string(params.to_string())
            .expect("valid json");

        match conn
            .ext_method(acp::ExtRequest::new(
                "kiro.dev/commands/options",
                std::sync::Arc::from(raw_params),
            ))
            .await
        {
            Ok(resp) => {
                println!("    Raw response: {}", resp.0);
            }
            Err(e) => {
                println!("    FAILED: {e}");
            }
        }
    }
    println!();

    // --- Test commands/execute with various command types ---
    println!("[5] Testing kiro.dev/commands/execute with different commands...");

    // Format discovered from kiro-cli debug logs:
    //   command: { "command": "<name>", "args": {<args>} }
    let test_commands: Vec<(&str, serde_json::Value)> = vec![
        (
            "/context (panel, no args)",
            serde_json::json!({
                "command": { "command": "context", "args": {} },
                "sessionId": session_id.to_string()
            }),
        ),
        (
            "/model (selection, value=claude-haiku-4.5)",
            serde_json::json!({
                "command": { "command": "model", "args": { "value": "claude-haiku-4.5" } },
                "sessionId": session_id.to_string()
            }),
        ),
        (
            "/compact (simple, no args)",
            serde_json::json!({
                "command": { "command": "compact", "args": {} },
                "sessionId": session_id.to_string()
            }),
        ),
    ];

    for (label, params) in &test_commands {
        println!("    [{label}]");
        println!("      payload: {params}");

        let raw_params = serde_json::value::RawValue::from_string(params.to_string())
            .expect("valid json");

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            conn.ext_method(acp::ExtRequest::new(
                "kiro.dev/commands/execute",
                std::sync::Arc::from(raw_params),
            )),
        )
        .await;

        match result {
            Ok(Ok(resp)) => {
                println!("      SUCCESS: {}", resp.0);
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
            Ok(Err(e)) => println!("      ERROR: {e}"),
            Err(_) => println!("      TIMEOUT (5s)"),
        }
    }

    // Drain stderr from kiro-cli
    let stderr = agent.drain_stderr();
    if !stderr.is_empty() {
        println!("    kiro-cli stderr:");
        for line in stderr.lines() {
            println!("      {line}");
        }
    }
    println!();

    println!("=== Done ===");
    Ok(())
}

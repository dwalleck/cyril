//! Bridge-based test harness for verifying Kiro ACP behavior.
//!
//! Spawns the bridge, sends commands, and prints every notification
//! that comes back. Tests the full v2 pipeline: bridge → kiro-cli → bridge → notification.
//!
//! Usage:
//!   cargo run --example test_bridge
//!   cargo run --example test_bridge -- --agent sonnet

use std::time::Duration;

use clap::Parser;

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::*;

#[derive(Parser)]
#[command(name = "test_bridge", about = "Bridge-based ACP test harness")]
struct Cli {
    /// Agent to use (e.g. "sonnet", "dotnet-dev")
    #[arg(short, long, default_value = "kiro-cli")]
    agent: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .init();

    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    println!("=== Cyril v2 Bridge Test Harness ===\n");
    println!("Agent: {}", cli.agent);
    println!("CWD: {}\n", cwd.display());

    let bridge = spawn_bridge(&cli.agent, cwd.clone())?;
    let (sender, mut notification_rx, mut permission_rx) = bridge.split();
    println!("[OK] Bridge spawned\n");

    // --- Test 1: Create session ---
    println!("--- [1] Creating session ---");
    sender
        .send(BridgeCommand::NewSession { cwd: cwd.clone() })
        .await?;

    // Drain notifications for a few seconds — session creation triggers
    // several notifications (SessionCreated, CommandsUpdated, ContextUsageUpdated)
    let session_id =
        drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3))
            .await;

    let session_id = match session_id {
        Some(id) => {
            println!("\n[OK] Session created: {}\n", id.as_str());
            id
        }
        None => {
            println!("\n[FAIL] No SessionCreated notification received\n");
            return Ok(());
        }
    };

    // --- Test 2: Query model options ---
    println!("--- [2] Querying model options ---");
    sender
        .send(BridgeCommand::QueryCommandOptions {
            command: "model".into(),
            session_id: session_id.clone(),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Test 3: Query agent options ---
    println!("--- [3] Querying agent options ---");
    sender
        .send(BridgeCommand::QueryCommandOptions {
            command: "agent".into(),
            session_id: session_id.clone(),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Test 4: Execute /tools ---
    println!("--- [4] Executing /tools ---");
    sender
        .send(BridgeCommand::ExecuteCommand {
            command: "tools".into(),
            session_id: session_id.clone(),
            args: serde_json::json!({}),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Test 5: Execute /context ---
    println!("--- [5] Executing /context ---");
    sender
        .send(BridgeCommand::ExecuteCommand {
            command: "context".into(),
            session_id: session_id.clone(),
            args: serde_json::json!({}),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Test 6: Execute /usage ---
    println!("--- [6] Executing /usage ---");
    sender
        .send(BridgeCommand::ExecuteCommand {
            command: "usage".into(),
            session_id: session_id.clone(),
            args: serde_json::json!({}),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Test 7: Switch model ---
    println!("--- [7] Switching model to claude-haiku-4.5 ---");
    sender
        .send(BridgeCommand::ExecuteCommand {
            command: "model".into(),
            session_id: session_id.clone(),
            args: serde_json::json!({"value": "claude-haiku-4.5"}),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Test 8: Query prompt options ---
    println!("--- [8] Querying prompt options ---");
    sender
        .send(BridgeCommand::QueryCommandOptions {
            command: "prompts".into(),
            session_id: session_id.clone(),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Test 9: Send a prompt to trigger UsageUpdate ---
    println!("--- [9] Sending prompt (checking for UsageUpdate) ---");
    sender
        .send(BridgeCommand::SendPrompt {
            session_id: session_id.clone(),
            content_blocks: vec!["Say hello in one word.".into()],
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(15)).await;
    println!();

    // --- Test 10: Query chat sessions ---
    println!("--- [10] Querying chat session options ---");
    sender
        .send(BridgeCommand::QueryCommandOptions {
            command: "chat".into(),
            session_id: session_id.clone(),
        })
        .await?;
    drain_notifications(&mut notification_rx, &mut permission_rx, Duration::from_secs(3)).await;
    println!();

    // --- Shutdown ---
    println!("--- Shutting down ---");
    sender.send(BridgeCommand::Shutdown).await?;
    println!("[OK] Done\n");

    Ok(())
}

/// Drain all notifications for `duration`, printing each one.
/// Returns the session ID if a SessionCreated notification was received.
async fn drain_notifications(
    notification_rx: &mut tokio::sync::mpsc::Receiver<Notification>,
    permission_rx: &mut tokio::sync::mpsc::Receiver<PermissionRequest>,
    duration: Duration,
) -> Option<SessionId> {
    let deadline = tokio::time::Instant::now() + duration;
    let mut session_id = None;

    loop {
        tokio::select! {
            biased;

            Some(notification) = notification_rx.recv() => {
                print_notification(&notification);
                if let Notification::SessionCreated { session_id: ref id, .. } = notification {
                    session_id = Some(id.clone());
                }
            }

            Some(permission) = permission_rx.recv() => {
                println!("  [PERMISSION] {}", permission.message);
                for opt in &permission.options {
                    println!("    - {} (id={}, destructive={})", opt.label, opt.id, opt.is_destructive);
                }
                // Auto-approve with first non-destructive option
                let response = if permission.options.iter().any(|o| !o.is_destructive) {
                    PermissionResponse::AllowOnce
                } else {
                    PermissionResponse::Cancel
                };
                let _ = permission.responder.send(response);
            }

            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    session_id
}

fn print_notification(n: &Notification) {
    match n {
        Notification::SessionCreated {
            session_id,
            current_mode,
        } => {
            println!("  [SessionCreated]");
            println!("    session_id: {}", session_id.as_str());
            println!(
                "    current_mode: {}",
                current_mode.as_deref().unwrap_or("(none)")
            );
        }
        Notification::TurnCompleted => {
            println!("  [TurnCompleted]");
        }
        Notification::BridgeDisconnected { reason } => {
            println!("  [BridgeDisconnected] {reason}");
        }
        Notification::AgentMessage(msg) => {
            let preview: String = msg.text.chars().take(80).collect();
            println!(
                "  [AgentMessage] streaming={} text={preview}...",
                msg.is_streaming
            );
        }
        Notification::AgentThought(thought) => {
            let preview: String = thought.text.chars().take(80).collect();
            println!("  [AgentThought] {preview}...");
        }
        Notification::ToolCallStarted(tc) => {
            println!(
                "  [ToolCallStarted] id={} title={:?} kind={:?}",
                tc.id().as_str(),
                tc.title(),
                tc.kind()
            );
        }
        Notification::ToolCallUpdated(tc) => {
            println!(
                "  [ToolCallUpdated] id={} title={:?} status={:?}",
                tc.id().as_str(),
                tc.title(),
                tc.status()
            );
        }
        Notification::PlanUpdated(plan) => {
            println!("  [PlanUpdated] {} entries", plan.entries().len());
        }
        Notification::ModeChanged { mode_id } => {
            println!("  [ModeChanged] {mode_id}");
        }
        Notification::ConfigOptionsUpdated(options) => {
            println!("  [ConfigOptionsUpdated] {} options", options.len());
            for opt in options {
                println!(
                    "    key={} value={:?} choices={}",
                    opt.key,
                    opt.value,
                    opt.options.len()
                );
            }
        }
        Notification::CommandsUpdated(cmds) => {
            println!("  [CommandsUpdated] {} commands", cmds.len());
            for cmd in cmds {
                println!(
                    "    {:<20} sel={:<5} local={:<5} {}",
                    cmd.name(),
                    cmd.is_selection(),
                    cmd.is_local(),
                    cmd.description().unwrap_or("")
                );
            }
        }
        Notification::ContextUsageUpdated(usage) => {
            println!("  [ContextUsageUpdated] {:.1}%", usage.percentage());
        }
        Notification::AgentSwitched { name, welcome } => {
            println!(
                "  [AgentSwitched] name={name} welcome={:?}",
                welcome
            );
        }
        Notification::CompactionStatus { message } => {
            println!("  [CompactionStatus] {message}");
        }
        Notification::ClearStatus { message } => {
            println!("  [ClearStatus] {message}");
        }
        Notification::RateLimited { message } => {
            println!("  [RateLimited] {message}");
        }
        Notification::ToolCallChunk {
            tool_call_id,
            title,
            kind,
        } => {
            println!("  [ToolCallChunk] id={tool_call_id} title={title} kind={kind}");
        }
        Notification::CommandOptionsReceived { command, options } => {
            println!(
                "  [CommandOptionsReceived] command={command} options={}",
                options.len()
            );
            for opt in options {
                println!(
                    "    {:<30} value={:<25} current={} desc={:?} group={:?}",
                    opt.label, opt.value, opt.is_current,
                    opt.description.as_deref().unwrap_or(""),
                    opt.group.as_deref().unwrap_or("")
                );
            }
        }
        Notification::CommandExecuted { command, response } => {
            let success = response
                .get("success")
                .and_then(|s| s.as_bool())
                .unwrap_or(false);
            let message = response
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let has_data = response.get("data").is_some();
            println!("  [CommandExecuted] command={command} success={success} has_data={has_data}");
            println!("    message: {message}");
            if has_data {
                let data = response.get("data").unwrap();
                let pretty =
                    serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string());
                // Print first 500 chars of data
                let preview: String = pretty.chars().take(500).collect();
                println!("    data: {preview}");
                if pretty.len() > 500 {
                    println!("    ... ({} more chars)", pretty.len() - 500);
                }
            }
        }
    }
}

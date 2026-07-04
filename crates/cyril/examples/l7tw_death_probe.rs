//! cyril-l7tw probe: what does the bridge's notification channel emit when the
//! agent process dies mid-turn, and on a follow-up prompt against the dead
//! connection? Prediction under test: only `TurnCompleted(EndTurn)` — no
//! `BridgeError`, no `BridgeDisconnected`. Oracle: the tracing/stderr pipeline
//! (bridge-internal error detection) + OS process table, evaluated by
//! `.cyril-l7tw/oracle-death-log.sh`.
//!
//! Usage: cargo run --example l7tw_death_probe 2> /tmp/l7tw-probe.stderr

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cwd = std::env::current_dir()?;
    // Default: real kiro-cli. Override (e.g. the 2.11.0-trace replay agent when
    // kiro-cli is not logged in): L7TW_AGENT_CMD="python3 .cyril-l7tw/kiro-replay-agent.py"
    let argv: Vec<String> = std::env::var("L7TW_AGENT_CMD")
        .unwrap_or_else(|_| "kiro-cli acp".into())
        .split_whitespace()
        .map(String::from)
        .collect();
    println!("PROBE agent_command={argv:?}");
    let agent_command = AgentCommand::try_from_argv(argv)?;
    let bridge = spawn_bridge(
        agent_command,
        AgentEngine::default(),
        KasSpawn::default(),
        cwd.clone(),
    )?;
    let (sender, mut notification_rx, mut permission_rx) = bridge.split();

    sender.send(BridgeCommand::NewSession { cwd }).await?;
    let session_id = record(&mut notification_rx, &mut permission_rx, 8, "SETUP", None).await;
    let session_id = session_id.expect("no SessionCreated — is kiro-cli logged in?");
    println!("PROBE session={}", session_id.as_str());

    // Long streaming turn, then SIGKILL the agent at the first streamed chunk.
    sender
        .send(BridgeCommand::SendPrompt {
            session_id: session_id.clone(),
            content_blocks: vec!["Count from 1 to 40, one number per line, no other text.".into()],
        })
        .await?;
    record(
        &mut notification_rx,
        &mut permission_rx,
        20,
        "PHASE1-POSTKILL",
        Some(kill_agent),
    )
    .await;

    // Second prompt at the dead connection.
    sender
        .send(BridgeCommand::SendPrompt {
            session_id,
            content_blocks: vec!["Say hello.".into()],
        })
        .await?;
    record(
        &mut notification_rx,
        &mut permission_rx,
        8,
        "PHASE2-DEADCONN",
        None,
    )
    .await;
    Ok(())
}

/// SIGKILL the kiro-cli child of this process; print OS-level evidence.
fn kill_agent() {
    let me = std::process::id().to_string();
    let out = std::process::Command::new("pgrep")
        .args(["-P", &me, "-f", "kiro"])
        .output()
        .expect("pgrep");
    let pid = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("no child pid")
        .to_string();
    let status = std::process::Command::new("kill")
        .args(["-9", &pid])
        .status()
        .expect("kill");
    std::thread::sleep(Duration::from_millis(300));
    let alive = std::process::Command::new("kill")
        .args(["-0", &pid])
        .status()
        .expect("kill -0");
    println!(
        "KILL pid={pid} kill9_ok={} still_alive={}",
        status.success(),
        alive.success()
    );
}

/// Record every notification for `secs`, printing `phase`-tagged transcript
/// lines. If `on_first_chunk` is set, fire it once at the first AgentMessage
/// (then keep recording). Auto-approves permissions. Returns any SessionCreated id.
async fn record(
    notification_rx: &mut tokio::sync::mpsc::Receiver<RoutedNotification>,
    permission_rx: &mut tokio::sync::mpsc::Receiver<PermissionRequest>,
    secs: u64,
    phase: &str,
    mut on_first_chunk: Option<fn()>,
) -> Option<SessionId> {
    let start = Instant::now();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut session_id = None;
    loop {
        tokio::select! {
            Some(routed) = notification_rx.recv() => {
                let n = &routed.notification;
                let dbg = format!("{n:?}");
                let name: String = dbg.chars().take_while(|c| c.is_alphanumeric()).collect();
                let detail = match n {
                    Notification::TurnCompleted { stop_reason } => format!("stop_reason={stop_reason:?}"),
                    Notification::BridgeError { operation, message } => format!("op={operation} msg={message}"),
                    Notification::BridgeDisconnected { reason } => format!("reason={reason}"),
                    _ => String::new(),
                };
                println!("{phase} t={:.2}s {name} {detail}", start.elapsed().as_secs_f32());
                if let Notification::SessionCreated { session_id: id, .. } = n { session_id = Some(id.clone()); }
                if matches!(n, Notification::AgentMessage(_))
                    && let Some(f) = on_first_chunk.take() { f(); }
            }
            Some(p) = permission_rx.recv() => {
                let opt = p.options.iter().find(|o| !o.is_destructive).expect("no option");
                println!("{phase} t={:.2}s PermissionRequest (auto-approving)", start.elapsed().as_secs_f32());
                if p.responder.send(PermissionResponse::Selected { option_id: opt.id.clone(), trust_option: None }).is_err() {
                    eprintln!("{phase} permission response dropped (receiver closed)");
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }
    session_id
}

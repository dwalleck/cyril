// PROBE (prove-it-prototype, K1b mid-turn steering). Throwaway — lives in
// .k1b-steering/probe.rs; copied here temporarily so it compiles as a target.
// Drives cyril's REAL bridge (spawn_bridge) against real kiro-cli 2.8.0 (v2),
// through the wire-tee oracle (.k1b-steering/wire_shim.py). Builds NOTHING from
// the K1b feature-to-come — only K1a primitives (SteerSession) + the bridge.
//
// Question: when a SteerSession is enqueued 1.5s into a multi-tool turn, does
// the steer surface BEFORE TurnCompleted (bridge allows mid-turn steering) or
// AFTER (bridge command loop blocked on conn.prompt().await for the whole turn)?
//
// Run:  cargo run --example probe_steer_midturn
// Oracle: /tmp/k1b_wire.log (wire_shim timestamps) — compare C2A _session/steer
// frame time vs A2C session/prompt response time, independent of this probe.

use std::time::{Duration, Instant};

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::AgentCommand;
use cyril_core::types::SessionId;
use cyril_core::types::event::{
    BridgeCommand, Notification, PermissionRequest, PermissionResponse, RoutedNotification,
};

fn label(n: &Notification) -> String {
    match n {
        Notification::SteeringQueued { message } => format!("STEER steering_queued: {message:?}"),
        Notification::SteeringConsumed { content } => {
            format!("STEER steering_consumed: {content:?}")
        }
        Notification::SteeringCleared => "STEER steering_cleared".into(),
        Notification::SteeringUnsupported { message } => format!("STEER unsupported: {message}"),
        Notification::TurnCompleted { stop_reason } => {
            format!("===== TurnCompleted ({stop_reason:?}) =====")
        }
        Notification::ToolCallStarted(_) => "tool_call_started".into(),
        Notification::ToolCallUpdated(_) => "tool_call_updated".into(),
        Notification::AgentMessage(_) => "agent_message".into(),
        Notification::BridgeError { operation, message } => {
            format!("BridgeError[{operation}]: {message}")
        }
        _ => "·".into(),
    }
}

fn is_steer(n: &Notification) -> bool {
    matches!(
        n,
        Notification::SteeringQueued { .. }
            | Notification::SteeringConsumed { .. }
            | Notification::SteeringCleared
            | Notification::SteeringUnsupported { .. }
    )
}

async fn wait_for_session(
    notif_rx: &mut tokio::sync::mpsc::Receiver<RoutedNotification>,
    perm_rx: &mut tokio::sync::mpsc::Receiver<PermissionRequest>,
) -> Option<SessionId> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        tokio::select! {
            Some(routed) = notif_rx.recv() => {
                if let Notification::SessionCreated { session_id, .. } = routed.notification {
                    return Some(session_id);
                }
            }
            Some(p) = perm_rx.recv() => { let _ = p.responder.send(PermissionResponse::AllowOnce); }
            _ = tokio::time::sleep_until(deadline) => return None,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_writer(std::io::stderr)
        .init();

    let cwd = std::env::current_dir()?;
    let shim = cwd.join(".k1b-steering/wire_shim.py");
    let agent_command = AgentCommand::new("python3").with_args(vec![
        shim.to_string_lossy().into_owned(),
        "acp".into(),
        "--trust-all-tools".into(),
    ]);

    let bridge = spawn_bridge(agent_command, cwd.clone())?;
    let (sender, mut notif_rx, mut perm_rx) = bridge.split();

    sender
        .send(BridgeCommand::NewSession { cwd: cwd.clone() })
        .await?;
    let session_id = wait_for_session(&mut notif_rx, &mut perm_rx)
        .await
        .ok_or_else(|| anyhow::anyhow!("no SessionCreated within 20s"))?;
    eprintln!("[probe] session = {}", session_id.as_str());

    let prompt = "You have the execute_bash tool. Run these three commands one at a time, \
        waiting for each to finish before starting the next: \
        (1) sleep 2 && echo STEP_ONE  (2) sleep 2 && echo STEP_TWO  (3) sleep 2 && echo STEP_THREE. \
        After all three finish, reply with the word DONE.";

    let t0 = Instant::now();
    sender
        .send(BridgeCommand::SendPrompt {
            session_id: session_id.clone(),
            content_blocks: vec![prompt.into()],
        })
        .await?;
    println!("[+{:>6}ms] >>> SendPrompt sent", 0u128);

    // Inject the steer 1.5s into the turn.
    {
        let s = sender.clone();
        let sid = session_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            println!("[+{:>6}ms] >>> SteerSession ENQUEUED", t0.elapsed().as_millis());
            let _ = s
                .send(BridgeCommand::SteerSession {
                    session_id: sid,
                    message: "STEERING: stop running commands now and reply only with HALTED."
                        .into(),
                })
                .await;
        });
    }

    let hard = tokio::time::Instant::now() + Duration::from_secs(45);
    let mut post_turn: Option<tokio::time::Instant> = None;
    let mut turn_done_at: Option<u128> = None;
    let mut first_steer_at: Option<(u128, String)> = None;

    loop {
        let until = post_turn.unwrap_or(hard);
        tokio::select! {
            biased;
            Some(routed) = notif_rx.recv() => {
                let el = t0.elapsed().as_millis();
                let n = &routed.notification;
                println!("[+{el:>6}ms] {}", label(n));
                if is_steer(n) && first_steer_at.is_none() {
                    first_steer_at = Some((el, label(n)));
                }
                if let Notification::TurnCompleted { .. } = n {
                    if turn_done_at.is_none() {
                        turn_done_at = Some(el);
                        // keep reading 3s past turn end to catch a blocked steer
                        post_turn = Some(tokio::time::Instant::now() + Duration::from_secs(3));
                    }
                }
            }
            Some(p) = perm_rx.recv() => { let _ = p.responder.send(PermissionResponse::AllowOnce); }
            _ = tokio::time::sleep_until(until) => break,
        }
    }

    println!("\n================ VERDICT ================");
    match (first_steer_at, turn_done_at) {
        (Some((s, lab)), Some(t)) if s < t => println!(
            "steer surfaced at +{s}ms ({lab}) BEFORE TurnCompleted at +{t}ms\n\
             => bridge sends steer MID-TURN. K1b is UI-only."
        ),
        (Some((s, lab)), Some(t)) => println!(
            "steer surfaced at +{s}ms ({lab}) AFTER TurnCompleted at +{t}ms\n\
             => bridge BLOCKED the steer until the turn ended. K1b needs a bridge change."
        ),
        (None, Some(t)) => println!(
            "NO steer notification ever surfaced (TurnCompleted at +{t}ms)\n\
             => steer swallowed; investigate (likely blocked + no echo)."
        ),
        _ => println!("inconclusive (no TurnCompleted seen)"),
    }
    println!("========================================");

    sender.send(BridgeCommand::Shutdown).await.ok();
    Ok(())
}

// PROBE (prove-it-prototype, K1b idle /steer). Throwaway — copied into
// crates/cyril/examples/probe_idle_steer.rs to compile, then removed.
// Drives cyril's REAL bridge against real kiro-cli 2.8.0 (v2), through the
// wire-tee oracle (.k1b-steering/wire_shim.py, log /tmp/k1b_wire.log).
//
// Question: when a SteerSession is sent to an ACTIVE-but-IDLE session (no turn
// in flight), does kiro ACCEPT it (steering_queued echo, queued for next turn)
// or REJECT it (-32601 / silent no-op)? The mid-turn probe (probe.rs) already
// proved mid-turn acceptance; this isolates the IDLE shape, which K1b's /steer
// idle path depends on.
//
// Run: copy to crates/cyril/examples/probe_idle_steer.rs, then
//      `cargo run --example probe_idle_steer`.
// Probe signal: cyril Notification (SteeringQueued | SteeringUnsupported | none).
// Oracle (independent): /tmp/k1b_wire.log — look for an A2C steering_queued echo
// vs an A2C -32601 error on the _session/steer id.

use std::time::Duration;

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::AgentCommand;
use cyril_core::types::SessionId;
use cyril_core::types::event::{
    BridgeCommand, Notification, PermissionRequest, PermissionResponse, RoutedNotification,
};

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
    eprintln!("[probe] session = {} (IDLE — no prompt sent)", session_id.as_str());

    // No SendPrompt. Steer an idle session directly.
    sender
        .send(BridgeCommand::SteerSession {
            session_id: session_id.clone(),
            message: "IDLE-STEER: please note this for the next turn.".into(),
        })
        .await?;
    println!(">>> SteerSession sent to IDLE session");

    // Observe for 8s.
    let mut verdict = "NONE (no steering notification surfaced)".to_string();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        tokio::select! {
            biased;
            Some(routed) = notif_rx.recv() => {
                match &routed.notification {
                    Notification::SteeringQueued { message } => {
                        verdict = format!("ACCEPTED — SteeringQueued {{ message: {message:?} }}");
                        break;
                    }
                    Notification::SteeringUnsupported { message } => {
                        verdict = format!("REJECTED — SteeringUnsupported: {message}");
                        break;
                    }
                    Notification::BridgeError { operation, message } => {
                        verdict = format!("BRIDGE-ERROR [{operation}]: {message}");
                        break;
                    }
                    other => { println!("  (saw {other:?})"); }
                }
            }
            Some(p) = perm_rx.recv() => { let _ = p.responder.send(PermissionResponse::AllowOnce); }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    println!("\n================ VERDICT ================");
    println!("idle steer => {verdict}");
    println!("oracle: inspect /tmp/k1b_wire.log for A2C steering_queued vs A2C -32601");
    println!("========================================");

    sender.send(BridgeCommand::Shutdown).await.ok();
    Ok(())
}

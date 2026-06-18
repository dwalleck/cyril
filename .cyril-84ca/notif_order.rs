// PROBE (cyril-84ca C5): does cyril's REAL notification channel deliver streamed
// AgentMessages before TurnCompleted, against a real backend? Tests whether the
// notification-vs-response reordering found in the unit harness actually manifests
// in production, and whether it differs by backend (v2 vs KAS).
//
// Throwaway probe — NOT a live example target. To re-run, copy into
// crates/cyril/examples/ and (real kiro must be logged in):
//   cargo run --example notif_order -- acp                       # v2 (default engine)
//   cargo run --example notif_order -- acp --agent-engine kas    # KAS / v3
// Evidence captured in .cyril-84ca/c5-ordering-evidence.log. Race: cyril-9akh.
//
// Oracle: independent of the unit harness — this drives cyril's spawn_bridge and
// reports the order AgentMessage vs TurnCompleted arrive on the notification channel.
use std::time::Duration;

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::AgentCommand;
use cyril_core::types::event::{BridgeCommand, Notification, PermissionResponse};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent_args: Vec<String> = {
        let a: Vec<String> = std::env::args().skip(1).collect();
        if a.is_empty() { vec!["acp".into()] } else { a }
    };
    let cwd = std::env::current_dir()?;
    let cmd = AgentCommand::new("kiro-cli").with_args(agent_args.clone());
    let bridge = spawn_bridge(cmd, cwd.clone())?;
    let (sender, mut notif_rx, mut perm_rx) = bridge.split();

    sender.send(BridgeCommand::NewSession { cwd: cwd.clone() }).await?;
    let sid = {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            tokio::select! {
                Some(r) = notif_rx.recv() => {
                    if let Notification::SessionCreated { session_id, .. } = r.notification {
                        break session_id;
                    }
                }
                Some(p) = perm_rx.recv() => { let _ = p.responder.send(PermissionResponse::AllowOnce); }
                _ = tokio::time::sleep_until(deadline) => anyhow::bail!("no SessionCreated in 30s"),
            }
        }
    };

    sender
        .send(BridgeCommand::SendPrompt {
            session_id: sid,
            content_blocks: vec![
                "Reply with EXACTLY these five words and nothing else: one two three four five"
                    .into(),
            ],
        })
        .await?;

    // Record the order, and keep reading 2s past TurnCompleted to catch trailing
    // AgentMessages (the reorder symptom).
    let mut order: Vec<&str> = Vec::new();
    let mut turn_at: Option<usize> = None;
    let mut until = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        tokio::select! {
            Some(r) = notif_rx.recv() => match r.notification {
                Notification::AgentMessage(_) => order.push("msg"),
                Notification::TurnCompleted { .. } => {
                    order.push("TURN");
                    if turn_at.is_none() {
                        turn_at = Some(order.len() - 1);
                        until = tokio::time::Instant::now() + Duration::from_secs(2);
                    }
                }
                _ => {}
            },
            Some(p) = perm_rx.recv() => { let _ = p.responder.send(PermissionResponse::AllowOnce); }
            _ = tokio::time::sleep_until(until) => break,
        }
    }

    let total_msgs = order.iter().filter(|x| **x == "msg").count();
    println!("agent_args = {agent_args:?}");
    println!("order = {order:?}");
    match turn_at {
        Some(i) => {
            let before = order[..i].iter().filter(|x| **x == "msg").count();
            let after = total_msgs - before;
            println!("AgentMessages: {before} before TURN, {after} after TURN (total {total_msgs})");
            if after > 0 {
                println!("=> REORDER: TurnCompleted arrived before {after} streamed AgentMessage(s)");
            } else if total_msgs == 0 {
                println!("=> no AgentMessages streamed (can't judge ordering)");
            } else {
                println!("=> OK: all {before} AgentMessages preceded TurnCompleted");
            }
        }
        None => println!("=> no TurnCompleted seen (KAS turn-end is session_info_update, unmapped?)"),
    }

    sender.send(BridgeCommand::Shutdown).await.ok();
    Ok(())
}

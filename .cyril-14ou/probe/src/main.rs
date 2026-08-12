//! cyril-14ou probe: (Q2) does bridge teardown reap the KAS node on each exit
//! path, and (Q3) does CancelRequest mid-turn cancel a live KAS turn?
//!
//! Arms (argv[1]):
//!   shutdown — session up, then BridgeCommand::Shutdown, wait, exit 0
//!   drop     — session up, then drop the handles WITHOUT Shutdown, wait, exit 0
//!   abort    — session up, then std::process::abort() (no Drop anywhere)
//!   cancel   — send a slow prompt, CancelRequest 2s after first text,
//!              print notifications until TurnCompleted, then Shutdown
//! External wrapper does the pgrep oracle around each arm.

use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::{AgentCommand, AgentEngine, BridgeCommand, KasSpawn, Notification};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let arm = std::env::args().nth(1).unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("c14ou-{arm}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");

    let bridge = spawn_bridge(
        AgentCommand::new("unused"),
        SpawnConfig {
            engine: AgentEngine::Kas,
            kas_spawn: KasSpawn::Free,
            ..SpawnConfig::default()
        },
        dir.clone(),
    )
    .expect("spawn bridge");
    let (sender, mut notifications, mut permissions) = bridge.split();
    sender
        .send(BridgeCommand::NewSession { cwd: dir })
        .await
        .expect("send NewSession");

    let mut session = None;
    while session.is_none() {
        match notifications.recv().await {
            Some(routed) => match routed.notification {
                Notification::SessionCreated { session_id, .. } => {
                    session = Some(session_id);
                }
                other => println!("PRE-SESSION {other:?}"),
            },
            None => panic!("channel closed before SessionCreated"),
        }
    }
    println!("READY");
    use std::io::Write as _;
    std::io::stdout().flush().expect("flush");
    // Hold until the oracle wrapper has snapshotted the live node, so
    // teardown cannot race the "during" observation.
    if arm != "cancel" {
        let mut go = String::new();
        std::io::stdin().read_line(&mut go).expect("go line");
    }

    match arm.as_str() {
        "shutdown" => {
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect("shutdown");
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("EXITING-AFTER-SHUTDOWN");
        }
        "drop" => {
            drop(sender);
            drop(notifications);
            drop(permissions);
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("EXITING-AFTER-DROP");
        }
        "abort" => {
            println!("ABORTING");
            std::process::abort();
        }
        "cancel" => {
            let sid = session.expect("session id");
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid,
                    content_blocks: vec![
                        "Count from 1 to 400, one number per line, no tools, no commentary."
                            .to_string(),
                    ],
                })
                .await
                .expect("send prompt");
            let mut cancelled = false;
            loop {
                tokio::select! {
                    routed = notifications.recv() => {
                        let Some(routed) = routed else { break };
                        match routed.notification {
                            Notification::AgentMessage(m) => {
                                if !cancelled {
                                    println!("FIRST-TEXT {:?}", &m.text[..m.text.len().min(20)]);
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                    sender.send(BridgeCommand::CancelRequest).await.expect("cancel");
                                    println!("CANCEL-SENT");
                                    cancelled = true;
                                }
                            }
                            Notification::TurnCompleted { stop_reason } => {
                                println!("TURN-COMPLETED {stop_reason:?}");
                                break;
                            }
                            other => println!("NOTE {other:?}"),
                        }
                    }
                    perm = permissions.recv() => {
                        if perm.is_some() { println!("UNEXPECTED-PERMISSION"); }
                    }
                }
            }
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect("shutdown");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        other => panic!("unknown arm {other:?}"),
    }
}

use std::{path::PathBuf, time::Duration};
use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::protocol::bridge::SpawnConfig;
use cyril_core::types::kas_hooks::KasHooksMode;
use cyril_core::types::{AgentCommand, AgentEngine, BridgeCommand, KasSpawn, Notification,
    PresentAs, RoutedNotification, SessionId};
use tokio::sync::mpsc::Receiver;

fn show(n: &RoutedNotification) -> String {
    let scope = n.session_id.as_ref().map_or("global", |s| s.as_str());
    let kind = match &n.notification {
        Notification::SessionCreated { .. } => "session-created".into(),
        Notification::TurnCompleted { stop_reason } => format!("turn-completed:{stop_reason:?}"),
        Notification::BridgeError { operation, message } => format!("bridge-error:{operation}:{message}"),
        Notification::BridgeDisconnected { reason } => format!("disconnected:{reason}"),
        other => format!("other:{other:?}"),
    };
    // cyril-a71q: surface the envelope's owner stamp. `-` means the frame
    // carries no turn identity (the KAS wire turn_end), which is a meaningful
    // observation, not a missing field.
    let owner = n.turn.map_or_else(|| "-".to_string(), |t| t.get().to_string());
    format!("scope={scope} kind={kind} owner={owner}")
}

async fn next(rx: &mut Receiver<RoutedNotification>) -> Result<RoutedNotification, String> {
    tokio::time::timeout(Duration::from_secs(3), rx.recv()).await
        .map_err(|_| "notification timeout".to_string())?
        .ok_or_else(|| "notification channel closed".to_string())
}

async fn completion(rx: &mut Receiver<RoutedNotification>, label: &str) -> Result<(), String> {
    loop {
        let n = next(rx).await?;
        println!("{label} {}", show(&n));
        if matches!(n.notification, Notification::TurnCompleted { .. }) { return Ok(()); }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scenario = std::env::args().nth(1).ok_or("scenario missing")?;
    println!("scenario={scenario}");
    let cwd = std::env::current_dir()?;
    // spawn_bridge took (command, engine, kas_spawn, cwd) when this probe was
    // written; the engine/spawn arguments were folded into SpawnConfig on main
    // (ADR-0001/ADR-0006 added present_as and kas_hooks). Repaired 2026-07-26 --
    // the probe had been uncompilable since that change, so the committed
    // captures were stale rather than live.
    let config = SpawnConfig {
        engine: AgentEngine::Kas,
        kas_spawn: KasSpawn::Free,
        present_as: PresentAs::default(),
        kas_hooks: KasHooksMode::default(),
    };
    let bridge = spawn_bridge(AgentCommand::new("unused"), config, cwd)?;
    let (tx, mut rx, _permissions) = bridge.split();
    tx.send(BridgeCommand::NewSession { cwd: PathBuf::from(".") }).await?;
    let created = next(&mut rx).await?;
    println!("new {}", show(&created));
    let sid = SessionId::new("sess_main");
    tx.send(BridgeCommand::SendPrompt { session_id: sid.clone(), content_blocks: vec!["A-or-B".into()] }).await?;
    completion(&mut rx, "terminal-1").await?;
    tx.send(BridgeCommand::SendPrompt { session_id: sid.clone(), content_blocks: vec!["B-or-C".into()] }).await?;
    completion(&mut rx, "terminal-2").await?;
    tx.send(BridgeCommand::SendPrompt { session_id: sid, content_blocks: vec!["guard-probe".into()] }).await?;
    println!("guard-probe-sent");
    tokio::time::sleep(Duration::from_millis(500)).await;
    while let Ok(n) = rx.try_recv() { println!("after {}", show(&n)); }
    tx.send(BridgeCommand::Shutdown).await?;
    Ok(())
}

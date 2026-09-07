use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::*;
use cyril_core::types::kas_hooks::KasHooksMode;
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?.canonicalize()?;
    let profile = std::env::var("JLXX_PROFILE")?;
    let prompt_text = std::fs::read_to_string(std::env::var("JLXX_PROMPT")?)?;
    let mut second_prompt = std::env::var("JLXX_SECOND_PROMPT").ok()
        .map(std::fs::read_to_string).transpose()?;
    let command = AgentCommand::try_from_argv(vec![
        std::env::var("JLXX_LAUNCHER").unwrap_or_else(|_| "kiro-cli".to_owned()), "acp".to_owned(),
        "--auth-method".to_owned(), "cli".to_owned(),
    ])?;
    let bridge = spawn_bridge(command, SpawnConfig {
        engine: AgentEngine::Kas,
        kas_spawn: KasSpawn::Wrapper,
        kas_hooks: if std::env::var("JLXX_HOOKS").as_deref() == Ok("kas") { KasHooksMode::Kas } else { KasHooksMode::Off },
        ..SpawnConfig::default()
    }, cwd.clone())?;
    let (sender, mut notifications, mut permissions, mut sources, completion) = bridge.split();
    sender.send(BridgeCommand::NewSession { cwd }).await?;
    let deadline = tokio::time::sleep(Duration::from_secs(180));
    tokio::pin!(deadline);
    let mut reply = String::new();
    let mut completed = false;
    let mut failure = None;
    let mut source_open = true;
    let mut session = None;
    let mut prompt_sent = false;
    loop {
        tokio::select! {
            _ = &mut deadline => { failure = Some("bounded probe deadline elapsed"); break; }
            request = permissions.recv() => {
                if let Some(request) = request {
                    println!("PERMISSION_DENIED");
                    if request.responder.send(PermissionResponse::Cancel).is_err() {
                        eprintln!("permission responder closed");
                    }
                }
            }
            source = sources.recv(), if source_open => { source_open = source.is_some(); }
            event = notifications.recv() => {
                let Some(event) = event else { break; };
                println!("EVENT {:?}", event.notification);
                match event.notification {
                    Notification::SessionCreated { session_id, available_modes, .. } => {
                        if !available_modes.iter().any(|mode| mode.id().as_str() == profile) {
                            failure = Some("synthetic native profile not advertised");
                            break;
                        }
                        session = Some(session_id);
                        sender.send(BridgeCommand::SetMode { mode_id: profile.clone() }).await?;
                    }
                    Notification::ConfigOptionsUpdated(options) => {
                        let mode_ok = options.iter().any(|option| option.key == "mode" && option.value.as_deref() == Some(profile.as_str()));
                        let model_ok = options.iter().any(|option| option.key == "model" && option.value.as_deref() == Some("claude-sonnet-4.6") && option.options.iter().any(|model| model == "claude-sonnet-4.6"));
                        if mode_ok && model_ok && !prompt_sent {
                            if let Some(session_id) = session.clone() {
                                prompt_sent = true;
                                println!("VERIFIED_NATIVE_PROFILE_AND_MODEL");
                                sender.send(BridgeCommand::SendPrompt {
                                    session_id,
                                    prompt: PromptEnvelope::original(vec![prompt_text.clone().into()]),
                                }).await?;
                            }
                        }
                    }
                    Notification::AgentMessage(message) => {
                        reply.push_str(&message.text);
                    }
                    Notification::TurnCompleted { .. } => {
                        println!("AUTHORITATIVE_TERMINAL");
                        if let Some(text) = second_prompt.take() {
                            let session_id = session.clone().ok_or("terminal without session")?;
                            println!("SECOND_PROMPT_SAME_SESSION {session_id:?}");
                            sender.send(BridgeCommand::SendPrompt {
                                session_id,
                                prompt: PromptEnvelope::original(vec![text.into()]),
                            }).await?;
                        } else {
                            completed = true;
                            break;
                        }
                    }
                    Notification::BridgeDisconnected { .. } => { failure = Some("bridge disconnected"); break; }
                    _ => {}
                }
            }
        }
    }
    if let Err(error) = sender.send(BridgeCommand::Shutdown).await {
        eprintln!("shutdown send: {error}");
    }
    println!("SHUTDOWN {:?}", tokio::time::timeout(Duration::from_secs(15), completion).await);
    if let Some(reason) = failure { return Err(reason.into()); }
    if !completed || reply.is_empty() { return Err("no completed authenticated reply".into()); }
    println!("COMPLETED_MODEL_BOUND_OPERATION; inspect operation outcomes separately");
    Ok(())
}

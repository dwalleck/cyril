//! Slash commands for subagent management: /sessions, /spawn, /kill, /msg.

use crate::commands::{Command, CommandContext, CommandResult};
use crate::types::{BridgeCommand, SessionId, SubagentStatus};

/// `/sessions` — list active subagents and pending stages from the tracker.
pub struct SessionsCommand;

#[async_trait::async_trait]
impl Command for SessionsCommand {
    fn name(&self) -> &str {
        "sessions"
    }

    fn description(&self) -> &str {
        "List active subagents and pending stages"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, _args: &str) -> crate::Result<CommandResult> {
        let tracker = match ctx.require_tracker() {
            Ok(t) => t,
            Err(msg) => return Ok(msg),
        };

        let subagents = tracker.subagents();
        let pending = tracker.pending_stages();

        if subagents.is_empty() && pending.is_empty() {
            return Ok(CommandResult::system_message(
                "No active subagents or pending stages.".into(),
            ));
        }

        let mut lines = Vec::new();
        if !subagents.is_empty() {
            lines.push(format!("Active subagents ({}):", subagents.len()));
            // Sort for deterministic display
            let mut sorted: Vec<_> = subagents.values().collect();
            sorted.sort_by(|a, b| a.session_name().cmp(b.session_name()));
            for info in sorted {
                let status = match info.status() {
                    SubagentStatus::Working { message } => {
                        format!("● working — {}", message.as_deref().unwrap_or("Running"))
                    }
                    SubagentStatus::Terminated => "◆ terminated".to_string(),
                };
                lines.push(format!(
                    "  {} ({}) {status}",
                    info.session_name(),
                    info.agent_name()
                ));
            }
        }
        if !pending.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!("Pending stages ({}):", pending.len()));
            let mut sorted: Vec<_> = pending.iter().collect();
            sorted.sort_by(|a, b| a.name().cmp(b.name()));
            for stage in sorted {
                let deps = if stage.depends_on().is_empty() {
                    "no dependencies".to_string()
                } else {
                    format!("depends on: {}", stage.depends_on().join(", "))
                };
                lines.push(format!("  ○ {} ({deps})", stage.name()));
            }
        }

        Ok(CommandResult::system_message(lines.join("\n")))
    }
}

/// `/spawn <name> <task>` — spawn a new subagent session.
pub struct SpawnCommand;

#[async_trait::async_trait]
impl Command for SpawnCommand {
    fn name(&self) -> &str {
        "spawn"
    }

    fn description(&self) -> &str {
        "Spawn a new subagent: /spawn <name> <task>"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        let Some((name, task)) = args.split_once(' ') else {
            return Ok(CommandResult::system_message(
                "Usage: /spawn <name> <task description>".into(),
            ));
        };
        let name = name.trim();
        let task = task.trim();
        if name.is_empty() || task.is_empty() {
            return Ok(CommandResult::system_message(
                "Usage: /spawn <name> <task description>".into(),
            ));
        }

        ctx.bridge
            .send(BridgeCommand::SpawnSession {
                task: task.to_string(),
                name: name.to_string(),
            })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

/// `/kill <name>` — terminate a subagent by its session name.
pub struct KillCommand;

#[async_trait::async_trait]
impl Command for KillCommand {
    fn name(&self) -> &str {
        "kill"
    }

    fn description(&self) -> &str {
        "Terminate a subagent: /kill <name>"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        let name = args.trim();
        if name.is_empty() {
            return Ok(CommandResult::system_message(
                "Usage: /kill <subagent-name>   (run /sessions to list active subagents)".into(),
            ));
        }

        let tracker = match ctx.require_tracker() {
            Ok(t) => t,
            Err(msg) => return Ok(msg),
        };

        let Some(info) = tracker.find_by_name(name) else {
            return Ok(CommandResult::system_message(format!(
                "No subagent named '{name}'. Run /sessions to list active subagents."
            )));
        };

        let session_id: SessionId = info.session_id().clone();
        ctx.bridge
            .send(BridgeCommand::TerminateSession { session_id })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

/// `/msg <name> <text>` — send a message to a subagent.
pub struct MsgCommand;

#[async_trait::async_trait]
impl Command for MsgCommand {
    fn name(&self) -> &str {
        "msg"
    }

    fn description(&self) -> &str {
        "Send a message to a subagent: /msg <name> <text>"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        let Some((name, content)) = args.split_once(' ') else {
            return Ok(CommandResult::system_message(
                "Usage: /msg <subagent-name> <message text>".into(),
            ));
        };
        let name = name.trim();
        let content = content.trim();
        if name.is_empty() || content.is_empty() {
            return Ok(CommandResult::system_message(
                "Usage: /msg <subagent-name> <message text>".into(),
            ));
        }

        let tracker = match ctx.require_tracker() {
            Ok(t) => t,
            Err(msg) => return Ok(msg),
        };

        let Some(info) = tracker.find_by_name(name) else {
            return Ok(CommandResult::system_message(format!(
                "No subagent named '{name}'. Run /sessions to list active subagents."
            )));
        };

        let session_id: SessionId = info.session_id().clone();
        ctx.bridge
            .send(BridgeCommand::SendMessage {
                session_id,
                content: content.to_string(),
            })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::commands::CommandResultKind;
    use crate::protocol::bridge::BridgeSender;
    use crate::session::SessionController;
    use crate::subagent::SubagentTracker;
    use crate::types::{Notification, PendingStage, SubagentInfo};

    fn make_tracker() -> SubagentTracker {
        let mut tracker = SubagentTracker::new();
        let info = SubagentInfo::new(
            SessionId::new("sub-1-id"),
            "reviewer",
            "code-reviewer",
            "Review the code",
            SubagentStatus::Working {
                message: Some("Running".into()),
            },
        )
        .with_group(Some("crew-a".into()));
        let stage = PendingStage::new(
            "summary",
            None,
            Some("crew-a".into()),
            None,
            vec!["reviewer".into()],
        );
        tracker.apply_notification(&Notification::SubagentListUpdated {
            subagents: vec![info],
            pending_stages: vec![stage],
        });
        tracker
    }

    fn make_ctx<'a>(
        session: &'a SessionController,
        sender: &'a BridgeSender,
        tracker: Option<&'a SubagentTracker>,
    ) -> CommandContext<'a> {
        CommandContext {
            session,
            bridge: sender,
            subagent_tracker: tracker,
        }
    }

    #[tokio::test]
    async fn sessions_command_lists_active_and_pending() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = make_tracker();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = SessionsCommand.execute(&ctx, "").await.unwrap();
        let CommandResultKind::SystemMessage(text) = result.kind else {
            panic!("expected SystemMessage");
        };
        assert!(text.contains("reviewer"));
        assert!(text.contains("code-reviewer"));
        assert!(text.contains("summary"));
        assert!(text.contains("depends on: reviewer"));
    }

    #[tokio::test]
    async fn sessions_command_handles_empty_tracker() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = SubagentTracker::new();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = SessionsCommand.execute(&ctx, "").await.unwrap();
        let CommandResultKind::SystemMessage(text) = result.kind else {
            panic!("expected SystemMessage");
        };
        assert!(text.contains("No active"));
    }

    #[tokio::test]
    async fn sessions_command_handles_missing_tracker() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let ctx = make_ctx(&session, &sender, None);

        let result = SessionsCommand.execute(&ctx, "").await.unwrap();
        assert!(matches!(result.kind, CommandResultKind::SystemMessage(_)));
    }

    #[tokio::test]
    async fn spawn_command_requires_name_and_task() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let ctx = make_ctx(&session, &sender, None);

        let result = SpawnCommand.execute(&ctx, "").await.unwrap();
        let CommandResultKind::SystemMessage(text) = result.kind else {
            panic!("expected SystemMessage on missing args");
        };
        assert!(text.contains("Usage"));

        let result = SpawnCommand.execute(&ctx, "only-name").await.unwrap();
        assert!(matches!(result.kind, CommandResultKind::SystemMessage(_)));
    }

    #[tokio::test]
    async fn spawn_command_dispatches_bridge_command() {
        let session = SessionController::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let ctx = make_ctx(&session, &sender, None);

        let result = SpawnCommand
            .execute(&ctx, "reviewer Review the pending PR")
            .await
            .unwrap();
        assert!(matches!(result.kind, CommandResultKind::Dispatched));

        let bridge_cmd = rx.recv().await.unwrap();
        if let BridgeCommand::SpawnSession { task, name } = bridge_cmd {
            assert_eq!(name, "reviewer");
            assert_eq!(task, "Review the pending PR");
        } else {
            panic!("expected SpawnSession, got {bridge_cmd:?}");
        }
    }

    #[tokio::test]
    async fn kill_command_requires_name() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = make_tracker();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = KillCommand.execute(&ctx, "").await.unwrap();
        assert!(matches!(result.kind, CommandResultKind::SystemMessage(_)));
    }

    #[tokio::test]
    async fn kill_command_reports_unknown_name() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = make_tracker();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = KillCommand.execute(&ctx, "ghost").await.unwrap();
        let CommandResultKind::SystemMessage(text) = result.kind else {
            panic!("expected SystemMessage for unknown name");
        };
        assert!(text.contains("ghost"));
        assert!(text.contains("No subagent"));
    }

    #[tokio::test]
    async fn kill_command_dispatches_terminate_for_known_name() {
        let session = SessionController::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = make_tracker();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = KillCommand.execute(&ctx, "reviewer").await.unwrap();
        assert!(matches!(result.kind, CommandResultKind::Dispatched));

        let bridge_cmd = rx.recv().await.unwrap();
        if let BridgeCommand::TerminateSession { session_id } = bridge_cmd {
            assert_eq!(session_id.as_str(), "sub-1-id");
        } else {
            panic!("expected TerminateSession, got {bridge_cmd:?}");
        }
    }

    #[tokio::test]
    async fn msg_command_requires_name_and_text() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = make_tracker();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = MsgCommand.execute(&ctx, "").await.unwrap();
        assert!(matches!(result.kind, CommandResultKind::SystemMessage(_)));

        let result = MsgCommand.execute(&ctx, "reviewer").await.unwrap();
        assert!(matches!(result.kind, CommandResultKind::SystemMessage(_)));
    }

    #[tokio::test]
    async fn msg_command_dispatches_send_message() {
        let session = SessionController::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = make_tracker();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = MsgCommand
            .execute(&ctx, "reviewer please check tests")
            .await
            .unwrap();
        assert!(matches!(result.kind, CommandResultKind::Dispatched));

        let bridge_cmd = rx.recv().await.unwrap();
        if let BridgeCommand::SendMessage {
            session_id,
            content,
        } = bridge_cmd
        {
            assert_eq!(session_id.as_str(), "sub-1-id");
            assert_eq!(content, "please check tests");
        } else {
            panic!("expected SendMessage, got {bridge_cmd:?}");
        }
    }

    #[tokio::test]
    async fn msg_command_reports_unknown_name() {
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = BridgeSender::from_sender(tx);
        let tracker = make_tracker();
        let ctx = make_ctx(&session, &sender, Some(&tracker));

        let result = MsgCommand.execute(&ctx, "ghost hello").await.unwrap();
        let CommandResultKind::SystemMessage(text) = result.kind else {
            panic!("expected SystemMessage for unknown name");
        };
        assert!(text.contains("ghost"));
    }
}

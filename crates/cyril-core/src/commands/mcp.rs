//! KAS owns MCP through its management dialect; v2 keeps its advertised command.

use super::{Command, CommandContext, CommandResult, CommandResultKind};
use crate::types::AgentEngine;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum McpCommandSource {
    #[default]
    Agent,
    Kas,
}

impl McpCommandSource {
    #[must_use]
    pub fn resolve(engine: AgentEngine) -> Self {
        match engine {
            AgentEngine::Kas => Self::Kas,
            AgentEngine::V2 => Self::Agent,
        }
    }
}

pub(super) struct McpCommand;

#[async_trait::async_trait]
impl Command for McpCommand {
    fn name(&self) -> &str {
        "mcp"
    }
    fn description(&self) -> &str {
        "Inspect MCP servers, reconnect/authenticate, or preview advertised data"
    }
    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        if !args.is_empty() {
            return Ok(CommandResult::system_message(
                "Usage: /mcp — select a server and action in the panel.".into(),
            ));
        }
        Ok(CommandResult {
            kind: CommandResultKind::ShowMcp {
                state: ctx.session.mcp(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{
        CommandRegistry, ConfigCommandSource, HooksCommandSource, UsageAccountCommandSource,
        WorkflowCommandSource,
    };
    use crate::test_support::must_succeed;
    use crate::types::{AgentCommandRoute, CommandInfo};

    #[tokio::test]
    async fn kas_owns_live_panel_and_help_while_v2_agent_mcp_remains_unshadowed() {
        assert_eq!(
            McpCommandSource::resolve(AgentEngine::Kas),
            McpCommandSource::Kas
        );
        assert_eq!(
            McpCommandSource::resolve(AgentEngine::V2),
            McpCommandSource::Agent
        );
        let kas = CommandRegistry::with_builtin_sources(
            HooksCommandSource::Agent,
            WorkflowCommandSource::None,
            UsageAccountCommandSource::None,
            ConfigCommandSource::Agent,
            McpCommandSource::Kas,
        );
        let (handle, mut command_rx) =
            crate::protocol::bridge::BridgeHandle::for_tests_with_command_rx();
        let sender = handle.sender();
        let session = crate::session::SessionController::new();
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };
        let (command, args) = kas
            .parse("/mcp")
            .unwrap_or_else(|| panic!("KAS registers /mcp"));
        let result = must_succeed(command.execute(&ctx, args).await, "MCP command");
        assert!(
            matches!(result.kind, CommandResultKind::ShowMcp {state} if state.snapshot.is_none())
        );
        let result = must_succeed(
            command.execute(&ctx, "unexpected").await,
            "MCP argument error",
        );
        assert!(
            matches!(result.kind, CommandResultKind::SystemMessage(message) if message.contains("Usage: /mcp"))
        );
        let (help, args) = kas.parse("/help").unwrap_or_else(|| panic!("help"));
        assert!(
            matches!(must_succeed(help.execute(&ctx,args).await,"help").kind,
            CommandResultKind::SystemMessage(message) if message.contains("/mcp"))
        );
        assert!(
            command_rx.try_recv().is_err(),
            "opening or previewing a panel never starts model/tool work"
        );
        let mut v2 =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        assert!(v2.parse("/mcp").is_none());
        v2.replace_agent_commands(&[CommandInfo::new(
            "mcp",
            "MCP",
            Some("v2 MCP"),
            false,
            false,
            false,
            AgentCommandRoute::BridgeExecute,
        )]);
        let (command, _) = v2
            .parse("/mcp")
            .unwrap_or_else(|| panic!("v2 agent command"));
        assert!(!command.is_local());
    }
}

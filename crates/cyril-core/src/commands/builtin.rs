use crate::commands::{Command, CommandContext, CommandResult};
use crate::types::BridgeCommand;

/// /help — show available commands
pub struct HelpCommand {
    command_names: Vec<String>,
}

impl HelpCommand {
    pub fn new(command_names: &[&str]) -> Self {
        Self {
            command_names: command_names.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[async_trait::async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "Show available commands"
    }

    async fn execute(
        &self,
        _ctx: &CommandContext<'_>,
        _args: &str,
    ) -> crate::Result<CommandResult> {
        let mut lines = vec!["Available commands:".to_string()];
        for name in &self.command_names {
            lines.push(format!("  /{name}"));
        }
        Ok(CommandResult::system_message(lines.join("\n")))
    }
}

/// /clear — clear the chat
pub struct ClearCommand;

#[async_trait::async_trait]
impl Command for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "Clear the chat"
    }

    async fn execute(
        &self,
        _ctx: &CommandContext<'_>,
        _args: &str,
    ) -> crate::Result<CommandResult> {
        Ok(CommandResult::system_message("__clear__".to_string()))
    }
}

/// /quit — quit the application
pub struct QuitCommand;

#[async_trait::async_trait]
impl Command for QuitCommand {
    fn name(&self) -> &str {
        "quit"
    }

    fn aliases(&self) -> &[&str] {
        &["q", "exit"]
    }

    fn description(&self) -> &str {
        "Quit the application"
    }

    async fn execute(
        &self,
        _ctx: &CommandContext<'_>,
        _args: &str,
    ) -> crate::Result<CommandResult> {
        Ok(CommandResult::quit())
    }
}

/// /new — create a new session
pub struct NewCommand;

#[async_trait::async_trait]
impl Command for NewCommand {
    fn name(&self) -> &str {
        "new"
    }

    fn description(&self) -> &str {
        "Start a new session"
    }

    async fn execute(
        &self,
        ctx: &CommandContext<'_>,
        _args: &str,
    ) -> crate::Result<CommandResult> {
        let cwd = std::env::current_dir().unwrap_or_default();
        ctx.bridge.send(BridgeCommand::NewSession { cwd }).await?;
        Ok(CommandResult::dispatched())
    }
}

/// /load <id> — load a session
pub struct LoadCommand;

#[async_trait::async_trait]
impl Command for LoadCommand {
    fn name(&self) -> &str {
        "load"
    }

    fn description(&self) -> &str {
        "Load a session by ID"
    }

    async fn execute(
        &self,
        ctx: &CommandContext<'_>,
        args: &str,
    ) -> crate::Result<CommandResult> {
        if args.is_empty() {
            return Ok(CommandResult::system_message(
                "Usage: /load <session-id>".to_string(),
            ));
        }
        ctx.bridge
            .send(BridgeCommand::LoadSession {
                session_id: args.to_string(),
            })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

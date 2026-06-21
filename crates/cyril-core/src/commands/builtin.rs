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

/// /steer — queue a mid-turn steer (ROADMAP K1b, cyril-bm1j). The explicit path;
/// Enter-while-busy is the implicit one. Works busy (steer this turn) and idle
/// (backend queues for the next turn — probe-confirmed). Returns a `Steer` result
/// the App routes through `dispatch_steer`; the command itself never touches the
/// bridge or UI.
pub struct SteerCommand;

#[async_trait::async_trait]
impl Command for SteerCommand {
    fn name(&self) -> &str {
        "steer"
    }

    fn description(&self) -> &str {
        "Steer the agent mid-turn (advisory; the agent may decline)"
    }

    async fn execute(&self, _ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        // Load-bearing: an empty arg must NOT produce an empty steer to the
        // backend — return usage instead. Enforced at runtime (survives release),
        // not a debug_assert, because the wrong output would reach the wire.
        let msg = args.trim();
        if msg.is_empty() {
            Ok(CommandResult::system_message(
                "Usage: /steer <message>".to_string(),
            ))
        } else {
            Ok(CommandResult::steer(msg.to_string()))
        }
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

/// /voice — toggle voice input (push-to-talk speech-to-text). The App owns the
/// voice engine handle, so this just signals intent; the App flips capture
/// state and reports if voice support isn't compiled in (ROADMAP CN2 / V1a).
pub struct VoiceToggleCommand;

#[async_trait::async_trait]
impl Command for VoiceToggleCommand {
    fn name(&self) -> &str {
        "voice"
    }

    fn description(&self) -> &str {
        "Toggle voice input (speech-to-text)"
    }

    async fn execute(
        &self,
        _ctx: &CommandContext<'_>,
        _args: &str,
    ) -> crate::Result<CommandResult> {
        Ok(CommandResult::toggle_voice())
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

    async fn execute(&self, ctx: &CommandContext<'_>, _args: &str) -> crate::Result<CommandResult> {
        let cwd = std::env::current_dir().map_err(|e| {
            crate::Error::with_source(
                crate::ErrorKind::CommandFailed {
                    detail: "could not determine current working directory".into(),
                },
                e,
            )
        })?;
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

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        if args.is_empty() {
            return Ok(CommandResult::system_message(
                "Usage: /load <session-id>".to_string(),
            ));
        }
        ctx.bridge
            .send(BridgeCommand::LoadSession {
                session_id: crate::types::SessionId::new(args),
            })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

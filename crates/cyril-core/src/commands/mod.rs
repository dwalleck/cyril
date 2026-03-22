pub mod builtin;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::protocol::bridge::BridgeSender;
use crate::session::SessionController;
use crate::types::CommandOption;

/// Context provided to commands during execution.
pub struct CommandContext<'a> {
    pub session: &'a SessionController,
    pub bridge: &'a BridgeSender,
}

/// Result of executing a command.
pub struct CommandResult {
    pub kind: CommandResultKind,
}

pub enum CommandResultKind {
    /// Display a system message in chat.
    SystemMessage(String),
    /// The input wasn't a command — send as prompt.
    NotACommand(String),
    /// Open a picker for user selection.
    ShowPicker {
        title: String,
        options: Vec<CommandOption>,
    },
    /// Command dispatched to bridge (already sent).
    Dispatched,
    /// Quit the application.
    Quit,
}

impl CommandResult {
    pub fn system_message(text: String) -> Self {
        Self {
            kind: CommandResultKind::SystemMessage(text),
        }
    }

    pub fn not_a_command(text: String) -> Self {
        Self {
            kind: CommandResultKind::NotACommand(text),
        }
    }

    pub fn show_picker(title: String, options: Vec<CommandOption>) -> Self {
        Self {
            kind: CommandResultKind::ShowPicker { title, options },
        }
    }

    pub fn dispatched() -> Self {
        Self {
            kind: CommandResultKind::Dispatched,
        }
    }

    pub fn quit() -> Self {
        Self {
            kind: CommandResultKind::Quit,
        }
    }
}

/// Trait for a slash command.
#[async_trait::async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn aliases(&self) -> &[&str] {
        &[]
    }
    fn description(&self) -> &str;
    fn is_local(&self) -> bool {
        true
    }
    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult>;
}

/// Registry of available slash commands.
pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, cmd: Arc<dyn Command>) {
        self.commands.insert(cmd.name().to_string(), cmd.clone());
        for alias in cmd.aliases() {
            self.commands.insert((*alias).to_string(), cmd.clone());
        }
    }

    /// Parse a slash command. Returns None if input doesn't start with '/'.
    pub fn parse<'a>(&'a self, input: &'a str) -> Option<(&'a dyn Command, &'a str)> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }
        let (name, args) = match trimmed.find(' ') {
            Some(pos) => (&trimmed[1..pos], trimmed[pos + 1..].trim()),
            None => (&trimmed[1..], ""),
        };
        self.commands.get(name).map(|cmd| (cmd.as_ref(), args))
    }

    /// Create a registry pre-populated with all builtin commands.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        let names: Vec<&str> = vec!["help", "clear", "quit", "new", "load"];
        registry.register(Arc::new(builtin::HelpCommand::new(&names)));
        registry.register(Arc::new(builtin::ClearCommand));
        registry.register(Arc::new(builtin::QuitCommand));
        registry.register(Arc::new(builtin::NewCommand));
        registry.register(Arc::new(builtin::LoadCommand));
        registry
    }

    /// Register commands advertised by the agent.
    /// These are forwarded to the bridge as ext methods when executed.
    pub fn register_agent_commands(&mut self, cmds: &[crate::types::CommandInfo]) {
        for cmd in cmds {
            let name = cmd.name().to_string();
            // Skip if a builtin already covers this name
            if self.commands.contains_key(&name) {
                continue;
            }
            self.commands.insert(
                name.clone(),
                Arc::new(AgentCommand {
                    name,
                    description: cmd
                        .description()
                        .unwrap_or_else(|| cmd.label())
                        .to_string(),
                }),
            );
        }
    }

    /// All registered commands (deduplicated — aliases don't count as separate).
    pub fn all_commands(&self) -> Vec<&dyn Command> {
        let mut seen = HashSet::new();
        self.commands
            .values()
            .filter(|cmd| seen.insert(Arc::as_ptr(cmd) as *const () as usize))
            .map(|cmd| cmd.as_ref())
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A command forwarded to the agent via ext method.
struct AgentCommand {
    name: String,
    description: String,
}

#[async_trait::async_trait]
impl Command for AgentCommand {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn is_local(&self) -> bool {
        false
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        let params = serde_json::json!({
            "command": self.name,
            "args": args,
        });
        ctx.bridge
            .send(crate::types::BridgeCommand::ExtMethod {
                method: format!("kiro.dev/commands/{}", self.name),
                params,
            })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A simple test command
    struct EchoCommand;

    #[async_trait::async_trait]
    impl Command for EchoCommand {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echo back input"
        }
        async fn execute(
            &self,
            _ctx: &CommandContext<'_>,
            args: &str,
        ) -> crate::Result<CommandResult> {
            Ok(CommandResult::system_message(format!("Echo: {args}")))
        }
    }

    struct AliasedCommand;

    #[async_trait::async_trait]
    impl Command for AliasedCommand {
        fn name(&self) -> &str {
            "quit"
        }
        fn aliases(&self) -> &[&str] {
            &["q", "exit"]
        }
        fn description(&self) -> &str {
            "Quit the app"
        }
        async fn execute(
            &self,
            _ctx: &CommandContext<'_>,
            _args: &str,
        ) -> crate::Result<CommandResult> {
            Ok(CommandResult::quit())
        }
    }

    #[test]
    fn empty_registry_returns_none() {
        let registry = CommandRegistry::new();
        assert!(registry.parse("/unknown").is_none());
    }

    #[test]
    fn registered_command_found_by_name() {
        let mut registry = CommandRegistry::new();
        registry.register(std::sync::Arc::new(EchoCommand));
        let result = registry.parse("/echo hello");
        assert!(result.is_some());
        let (cmd, args) = result.unwrap();
        assert_eq!(cmd.name(), "echo");
        assert_eq!(args, "hello");
    }

    #[test]
    fn aliases_resolve_to_command() {
        let mut registry = CommandRegistry::new();
        registry.register(std::sync::Arc::new(AliasedCommand));
        assert!(registry.parse("/quit").is_some());
        assert!(registry.parse("/q").is_some());
        assert!(registry.parse("/exit").is_some());
    }

    #[test]
    fn non_slash_input_returns_none() {
        let registry = CommandRegistry::new();
        assert!(registry.parse("hello world").is_none());
        assert!(registry.parse("").is_none());
    }

    #[test]
    fn command_with_no_args() {
        let mut registry = CommandRegistry::new();
        registry.register(std::sync::Arc::new(EchoCommand));
        let (_, args) = registry.parse("/echo").unwrap();
        assert_eq!(args, "");
    }

    #[test]
    fn all_commands_deduplicates_aliases() {
        let mut registry = CommandRegistry::new();
        registry.register(std::sync::Arc::new(AliasedCommand));
        let all = registry.all_commands();
        // "quit", "q", "exit" all point to same command — should appear once
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn command_result_variants() {
        let msg = CommandResult::system_message("hello".into());
        assert!(matches!(msg.kind, CommandResultKind::SystemMessage(_)));

        let quit = CommandResult::quit();
        assert!(matches!(quit.kind, CommandResultKind::Quit));

        let dispatched = CommandResult::dispatched();
        assert!(matches!(dispatched.kind, CommandResultKind::Dispatched));
    }

    #[tokio::test]
    async fn execute_command_returns_result() {
        let cmd = EchoCommand;
        let session = crate::session::SessionController::new();
        // Create a dummy bridge sender
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };
        let result = cmd.execute(&ctx, "test").await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(
            matches!(result.kind, CommandResultKind::SystemMessage(ref s) if s == "Echo: test")
        );
    }

    #[tokio::test]
    async fn help_command_returns_system_message() {
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let result = builtin::HelpCommand::new(&[]).execute(&ctx, "").await;
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap().kind,
            CommandResultKind::SystemMessage(_)
        ));
    }

    #[tokio::test]
    async fn clear_command_returns_system_message() {
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let result = builtin::ClearCommand.execute(&ctx, "").await;
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap().kind,
            CommandResultKind::SystemMessage(_)
        ));
    }

    #[tokio::test]
    async fn quit_command_returns_quit() {
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let result = builtin::QuitCommand.execute(&ctx, "").await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().kind, CommandResultKind::Quit));
    }

    #[tokio::test]
    async fn new_command_sends_bridge_command() {
        let session = crate::session::SessionController::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let result = builtin::NewCommand.execute(&ctx, "").await;
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap().kind,
            CommandResultKind::Dispatched
        ));

        // Verify bridge received the command
        let cmd = rx.recv().await;
        assert!(matches!(
            cmd,
            Some(crate::types::BridgeCommand::NewSession { .. })
        ));
    }

    #[test]
    fn default_registry_has_builtins() {
        let registry = CommandRegistry::with_builtins();
        assert!(registry.parse("/help").is_some());
        assert!(registry.parse("/clear").is_some());
        assert!(registry.parse("/quit").is_some());
        assert!(registry.parse("/q").is_some());
        assert!(registry.parse("/new").is_some());
    }
}

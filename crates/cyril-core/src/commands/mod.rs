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
#[derive(Debug)]
pub struct CommandResult {
    pub kind: CommandResultKind,
}

#[derive(Debug)]
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
    ///
    /// Skips commands that are local-only and not selection commands.
    /// Local selection commands (e.g., `/chat`) are kept because they
    /// still need picker UI on the client side.
    pub fn register_agent_commands(&mut self, cmds: &[crate::types::CommandInfo]) {
        for cmd in cmds {
            // QRK-010: skip local-only commands that aren't selection pickers
            if cmd.is_local() && !cmd.is_selection() {
                continue;
            }
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
                    is_selection: cmd.is_selection(),
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
    is_selection: bool,
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
        let session_id = ctx
            .session
            .id()
            .ok_or_else(|| crate::Error::from_kind(crate::ErrorKind::NoSession))?;

        // Selection command without args: dispatch options query (non-blocking)
        if self.is_selection && args.is_empty() {
            ctx.bridge
                .send(crate::types::BridgeCommand::QueryCommandOptions {
                    command: self.name.clone(),
                    session_id: session_id.clone(),
                })
                .await?;
            return Ok(CommandResult::dispatched());
        }

        // Execute command via kiro.dev/commands/execute with TuiCommand format
        let cmd_args = if args.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::json!({"value": args})
        };

        let params = serde_json::json!({
            "sessionId": session_id.as_str(),
            "command": {
                "command": self.name,
                "args": cmd_args,
            }
        });

        ctx.bridge
            .send(crate::types::BridgeCommand::ExtMethod {
                method: "kiro.dev/commands/execute".into(),
                params,
            })
            .await?;

        Ok(CommandResult::dispatched())
    }
}

/// Parse a `kiro.dev/commands/options` response into `CommandOption`s.
///
/// Handles two response shapes:
/// - Object with `"options"` array: `{"options": [...]}`
/// - Bare array: `[...]`
pub(crate) fn parse_options_response(response: &serde_json::Value) -> Vec<CommandOption> {
    let options_arr = response
        .get("options")
        .and_then(|v| v.as_array())
        .or_else(|| response.as_array());

    let Some(opts) = options_arr else {
        return Vec::new();
    };

    opts.iter()
        .filter_map(|opt| {
            let value = opt.get("value").and_then(|v| v.as_str())?.to_string();
            let label = opt
                .get("label")
                .or_else(|| opt.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or(&value)
                .to_string();
            let description = opt
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);
            let group = opt
                .get("group")
                .and_then(|v| v.as_str())
                .map(String::from);
            let is_current = opt
                .get("current")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(CommandOption {
                label,
                value,
                description,
                group,
                is_current,
            })
        })
        .collect()
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
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

    // --- parse_options_response tests ---

    #[test]
    fn parse_options_response_with_options_key() {
        let response = serde_json::json!({
            "options": [
                {"value": "claude-sonnet", "label": "Claude Sonnet", "description": "Fast", "current": true},
                {"value": "claude-haiku", "label": "Claude Haiku"}
            ]
        });
        let opts = parse_options_response(&response);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].value, "claude-sonnet");
        assert_eq!(opts[0].label, "Claude Sonnet");
        assert_eq!(opts[0].description.as_deref(), Some("Fast"));
        assert!(opts[0].is_current);
        assert!(!opts[1].is_current);
    }

    #[test]
    fn parse_options_response_bare_array() {
        let response = serde_json::json!([
            {"value": "auto", "label": "auto"}
        ]);
        let opts = parse_options_response(&response);
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].value, "auto");
    }

    #[test]
    fn parse_options_response_empty() {
        let response = serde_json::json!({});
        let opts = parse_options_response(&response);
        assert!(opts.is_empty());
    }

    #[test]
    fn parse_options_response_label_fallback_to_value() {
        let response = serde_json::json!({
            "options": [{"value": "claude-sonnet"}]
        });
        let opts = parse_options_response(&response);
        assert_eq!(opts[0].label, "claude-sonnet");
    }

    #[test]
    fn parse_options_response_label_fallback_to_name() {
        let response = serde_json::json!({
            "options": [{"value": "sonnet", "name": "Claude Sonnet"}]
        });
        let opts = parse_options_response(&response);
        assert_eq!(opts[0].label, "Claude Sonnet");
    }

    #[test]
    fn parse_options_response_skips_entries_without_value() {
        let response = serde_json::json!({
            "options": [
                {"label": "No value field"},
                {"value": "valid", "label": "Valid"}
            ]
        });
        let opts = parse_options_response(&response);
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].value, "valid");
    }

    #[test]
    fn parse_options_response_with_groups() {
        let response = serde_json::json!({
            "options": [
                {"value": "sonnet", "label": "Sonnet", "group": "Anthropic"},
                {"value": "haiku", "label": "Haiku", "group": "Anthropic"}
            ]
        });
        let opts = parse_options_response(&response);
        assert_eq!(opts[0].group.as_deref(), Some("Anthropic"));
        assert_eq!(opts[1].group.as_deref(), Some("Anthropic"));
    }

    // --- AgentCommand execution tests ---

    #[tokio::test]
    async fn agent_command_fails_without_session() {
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let cmd = AgentCommand {
            name: "compact".into(),
            description: "Compact".into(),
            is_selection: false,
        };
        let result = cmd.execute(&ctx, "").await;
        assert!(result.is_err(), "should fail with no active session");
        assert!(matches!(
            result.unwrap_err().kind(),
            crate::ErrorKind::NoSession
        ));
    }

    #[tokio::test]
    async fn agent_command_execute_sends_correct_method_and_format() {
        let mut session = crate::session::SessionController::new();
        session.set_session(
            crate::types::SessionId::new("sess_test"),
            crate::types::SessionStatus::Active,
        );
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let cmd = AgentCommand {
            name: "compact".into(),
            description: "Compact context".into(),
            is_selection: false,
        };
        let result = cmd.execute(&ctx, "").await;
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap().kind,
            CommandResultKind::Dispatched
        ));

        let bridge_cmd = rx.recv().await.unwrap();
        if let crate::types::BridgeCommand::ExtMethod { method, params } = bridge_cmd {
            assert_eq!(method, "kiro.dev/commands/execute");
            assert_eq!(params["sessionId"], "sess_test");
            assert_eq!(params["command"]["command"], "compact");
            assert_eq!(params["command"]["args"], serde_json::json!({}));
        } else {
            panic!("expected ExtMethod, got {bridge_cmd:?}");
        }
    }

    #[tokio::test]
    async fn agent_command_execute_with_args_sends_value_field() {
        let mut session = crate::session::SessionController::new();
        session.set_session(
            crate::types::SessionId::new("sess_test"),
            crate::types::SessionStatus::Active,
        );
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let cmd = AgentCommand {
            name: "model".into(),
            description: "Switch model".into(),
            is_selection: true,
        };
        let result = cmd.execute(&ctx, "claude-sonnet").await;
        assert!(result.is_ok());

        let bridge_cmd = rx.recv().await.unwrap();
        if let crate::types::BridgeCommand::ExtMethod { method, params } = bridge_cmd {
            assert_eq!(method, "kiro.dev/commands/execute");
            assert_eq!(params["command"]["command"], "model");
            assert_eq!(params["command"]["args"]["value"], "claude-sonnet");
        } else {
            panic!("expected ExtMethod, got {bridge_cmd:?}");
        }
    }

    #[tokio::test]
    async fn agent_command_selection_no_args_sends_query_command_options() {
        let mut session = crate::session::SessionController::new();
        session.set_session(
            crate::types::SessionId::new("sess_test"),
            crate::types::SessionStatus::Active,
        );
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            session: &session,
            bridge: &sender,
        };

        let cmd = AgentCommand {
            name: "model".into(),
            description: "Switch model".into(),
            is_selection: true,
        };

        let result = cmd.execute(&ctx, "").await.unwrap();
        assert!(
            matches!(result.kind, CommandResultKind::Dispatched),
            "selection command without args should return Dispatched"
        );

        // Verify the bridge received a QueryCommandOptions command
        let bridge_cmd = rx.recv().await.unwrap();
        if let crate::types::BridgeCommand::QueryCommandOptions {
            command,
            session_id,
        } = bridge_cmd
        {
            assert_eq!(command, "model");
            assert_eq!(session_id.as_str(), "sess_test");
        } else {
            panic!("expected QueryCommandOptions, got {bridge_cmd:?}");
        }
    }
}

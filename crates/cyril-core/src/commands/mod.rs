pub mod builtin;
pub mod subagent;
pub mod workflow;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::protocol::bridge::BridgeSender;
use crate::session::SessionController;
use crate::types::CommandOption;

/// Context provided to commands during execution.
pub struct CommandContext<'a> {
    /// Canonical workspace bound at application startup.
    pub workspace: &'a std::path::Path,
    pub session: &'a SessionController,
    pub bridge: &'a BridgeSender,
    /// Optional subagent tracker for commands that need to look up subagents
    /// by name (e.g., `/kill`, `/msg`). `None` in tests that don't exercise
    /// subagent commands.
    pub subagent_tracker: Option<&'a crate::subagent::SubagentTracker>,
    /// Optional workflow tracker for `/workflow status` (no-arg), which
    /// renders known runs without a wire round-trip (cyril-0qe6 C14).
    /// `None` in tests that don't exercise workflow commands.
    pub workflow_tracker: Option<&'a crate::workflow::WorkflowTracker>,
    /// Current process-global memory status for the local `/memory status`.
    /// `None` means caller wiring is incomplete; the command reports it.
    pub memory_status: Option<&'a crate::types::MemoryStatusView>,
}

impl<'a> CommandContext<'a> {
    /// Require the subagent tracker to be present. Returns a graceful
    /// `CommandResult` system message when absent, which subagent commands
    /// can propagate via `?` using the `Result<T, CommandResult>` convention
    /// defined below.
    ///
    /// Absence at runtime is a programming error — the App always wires the
    /// tracker. We log at error level so it shows up in `cyril.log` if it
    /// ever fires.
    pub fn require_tracker(&self) -> Result<&'a crate::subagent::SubagentTracker, CommandResult> {
        match self.subagent_tracker {
            Some(tracker) => Ok(tracker),
            None => {
                tracing::error!("CommandContext.subagent_tracker is None — wiring error in App");
                Err(CommandResult::system_message(
                    "Subagent tracker unavailable.".into(),
                ))
            }
        }
    }
}

/// Which registry `/hooks` reads, decided once at startup from the bound
/// engine and the hooks mode (cyril-gk17).
///
/// The two engines disagree about who *owns* a hook registry, so they also
/// disagree about who should answer `/hooks`:
///
/// - **v2** advertises its own `hooks` command, which arrives with
///   `commands/available` and is registered as an agent command. Cyril must
///   not register a builtin of the same name — it would shadow the agent's.
/// - **KAS with `kas_hooks = "kas"`** advertises no TUI commands at all (its
///   command surface is skills-only), and owns a file-watched registry
///   reachable only through `_kiro/hooks/*`. Cyril supplies the command.
/// - **KAS with `kas_hooks = "host"`** is the third case, and it maps to
///   [`Agent`](Self::Agent) too: there *cyril* owns the registry and serves
///   `_kiro/hooks/list` to the agent, so querying the agent would ask about a
///   registry it does not have.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HooksCommandSource {
    /// Cyril registers no `/hooks`; the agent's own command (v2) handles it,
    /// or there is none.
    #[default]
    Agent,
    /// Cyril registers [`builtin::KasHooksCommand`], which queries the agent's
    /// registry over `_kiro/hooks/*`.
    ///
    /// Carries the workspace root to send as `workspacePaths`. It rides on the
    /// variant rather than being re-derived at call time because the root is
    /// `--cwd` when the user passed one, which is NOT
    /// `std::env::current_dir()`; querying the wrong root returns an empty
    /// listing that reads as "you have no hooks". The `Agent` variant has no
    /// root because it needs none.
    Kas { workspace_root: std::path::PathBuf },
}

impl HooksCommandSource {
    /// Resolve from the bound engine, the configured hooks mode, and the
    /// session's workspace root. Engine and mode must BOTH be KAS: the KAS
    /// engine alone is not enough, because in `host` mode the registry lives
    /// on cyril's side of the wire and the agent has none to query.
    #[must_use]
    pub fn resolve(
        engine: crate::types::AgentEngine,
        kas_hooks: crate::types::kas_hooks::KasHooksMode,
        workspace_root: std::path::PathBuf,
    ) -> Self {
        match (engine, kas_hooks) {
            (crate::types::AgentEngine::Kas, crate::types::kas_hooks::KasHooksMode::Kas) => {
                Self::Kas { workspace_root }
            }
            _ => Self::Agent,
        }
    }
}

/// Whether cyril registers its native `/workflow` family (cyril-0qe6,
/// ADR-0011). Unlike `/hooks` this depends on the engine alone: workflows
/// are KAS-only (no v2 surface exists, even dark-flagged), and cyril owns
/// the whole control plane — there is no agent-supplied alternative to
/// defer to in any mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum WorkflowCommandSource {
    /// No `/workflow`: the bound engine has no workflow surface (v2).
    #[default]
    None,
    /// Cyril registers [`workflow::WorkflowCommand`], driving
    /// `_kiro/workflow/*` with the gate off.
    ///
    /// Carries the workspace root for `workspacePaths` — the session's
    /// `--cwd`, for the same wrong-root reason as
    /// [`HooksCommandSource::Kas`].
    Kas { workspace_root: std::path::PathBuf },
}

impl WorkflowCommandSource {
    /// Resolve from the bound engine and the session's workspace root.
    #[must_use]
    pub fn resolve(engine: crate::types::AgentEngine, workspace_root: std::path::PathBuf) -> Self {
        match engine {
            crate::types::AgentEngine::Kas => Self::Kas { workspace_root },
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UsageAccountCommandSource {
    #[default]
    None,
    Kas,
}

impl UsageAccountCommandSource {
    #[must_use]
    pub fn resolve(engine: crate::types::AgentEngine) -> Self {
        match engine {
            crate::types::AgentEngine::Kas => Self::Kas,
            crate::types::AgentEngine::V2 => Self::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryCommandAction {
    Teach { text: String },
    Replace { lesson_id: String, text: String },
    List,
    Inspect { lesson_id: String },
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
    /// Queue-steer the user's message (ROADMAP K1b, cyril-bm1j). The App routes
    /// this through its async `dispatch_steer` (optimistic echo + `SteerSession`),
    /// because the command layer has no UI access and must not touch the bridge
    /// directly — same split as `ShowPicker`.
    Steer { text: String },
    /// Drop every queued steer (`/steer clear` → `_session/steer/clear`;
    /// cyril-vgcm C10). The App routes this through `dispatch_clear_steer` —
    /// same command-layer split as `Steer`. No payload: the wire method clears
    /// the whole queue; no per-id clear exists on either engine.
    ClearSteer,
    /// Toggle voice input on/off (ROADMAP CN2 / V1a). The command layer has no
    /// access to the voice engine handle (which the App owns), so it returns
    /// this and the App flips capture state — same split as `Steer`/`ShowPicker`.
    ToggleVoice,
    /// Open Cyril's local usage dashboard; records whether an async KAS
    /// account query was dispatched before returning.
    ShowUsage { account_query_started: bool },
    /// Return Cyril's current typed memory runtime status.
    MemoryStatus(crate::types::MemoryStatusView),
    /// Execute one typed project-memory operation in the binary orchestrator.
    MemoryAction(MemoryCommandAction),
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

    pub fn steer(text: String) -> Self {
        Self {
            kind: CommandResultKind::Steer { text },
        }
    }

    pub fn clear_steer() -> Self {
        Self {
            kind: CommandResultKind::ClearSteer,
        }
    }

    pub fn toggle_voice() -> Self {
        Self {
            kind: CommandResultKind::ToggleVoice,
        }
    }

    pub fn show_usage(account_query_started: bool) -> Self {
        Self {
            kind: CommandResultKind::ShowUsage {
                account_query_started,
            },
        }
    }
    pub fn memory_status(status: crate::types::MemoryStatusView) -> Self {
        Self {
            kind: CommandResultKind::MemoryStatus(status),
        }
    }

    pub fn memory_action(action: MemoryCommandAction) -> Self {
        Self {
            kind: CommandResultKind::MemoryAction(action),
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
    ///
    /// `hooks` decides whether cyril supplies its own `/hooks` — see
    /// [`HooksCommandSource`]. `workflows` decides whether the native
    /// `/workflow` family exists — see [`WorkflowCommandSource`].
    pub fn with_builtins(hooks: HooksCommandSource, workflows: WorkflowCommandSource) -> Self {
        Self::with_builtins_and_usage(hooks, workflows, UsageAccountCommandSource::None)
    }

    pub fn with_builtins_and_usage(
        hooks: HooksCommandSource,
        workflows: WorkflowCommandSource,
        usage_account: UsageAccountCommandSource,
    ) -> Self {
        let mut registry = Self::new();
        let mut names: Vec<&str> = vec![
            "help", "clear", "quit", "new", "load", "steer", "voice", "usage", "memory",
            "sessions", "spawn", "kill", "msg",
        ];
        if let HooksCommandSource::Kas { workspace_root } = hooks {
            names.push("hooks");
            registry.register(Arc::new(builtin::KasHooksCommand::new(workspace_root)));
        }
        if let WorkflowCommandSource::Kas { workspace_root } = workflows {
            names.push("workflow");
            registry.register(Arc::new(workflow::WorkflowCommand::new(workspace_root)));
        }
        registry.register(Arc::new(builtin::HelpCommand::new(&names)));
        registry.register(Arc::new(builtin::ClearCommand));
        registry.register(Arc::new(builtin::QuitCommand));
        registry.register(Arc::new(builtin::NewCommand));
        registry.register(Arc::new(builtin::LoadCommand));
        registry.register(Arc::new(builtin::SteerCommand));
        registry.register(Arc::new(builtin::VoiceToggleCommand));
        registry.register(Arc::new(builtin::UsageCommand::new(usage_account)));
        registry.register(Arc::new(builtin::MemoryCommand));
        registry.register(Arc::new(subagent::SessionsCommand));
        registry.register(Arc::new(subagent::SpawnCommand));
        registry.register(Arc::new(subagent::KillCommand));
        registry.register(Arc::new(subagent::MsgCommand));
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
            // Kiro quirk: /chat is marked local but needs picker UI, so keep local+selection commands.
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
                    description: cmd.description().unwrap_or_else(|| cmd.label()).to_string(),
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

        // Execute command via bridge — response comes back as CommandExecuted notification
        let cmd_args = if args.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::json!({"value": args})
        };

        ctx.bridge
            .send(crate::types::BridgeCommand::ExecuteCommand {
                command: self.name.clone(),
                session_id: session_id.clone(),
                args: cmd_args,
            })
            .await?;

        Ok(CommandResult::dispatched())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
#[expect(clippy::expect_used)]
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
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };
        let result = cmd.execute(&ctx, "test").await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(
            matches!(result.kind, CommandResultKind::SystemMessage(ref s) if s == "Echo: test")
        );
    }

    // cyril-bm1j Slice 12 / claims C10, C11: /steer parses + rejects empty.
    #[tokio::test]
    async fn steer_command_parses_message_and_rejects_empty() {
        let cmd = crate::commands::builtin::SteerCommand;
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };

        // C10: a message -> Steer{text}.
        let r = cmd.execute(&ctx, "fix tests").await.unwrap();
        assert!(
            matches!(r.kind, CommandResultKind::Steer { ref text } if text == "fix tests"),
            "got {:?}",
            r.kind
        );
        // Trim outer whitespace, preserve inner spaces.
        let r = cmd.execute(&ctx, "  a b  ").await.unwrap();
        assert!(matches!(r.kind, CommandResultKind::Steer { ref text } if text == "a b"));

        // C11: empty / whitespace-only -> usage SystemMessage, NEVER a Steer
        // (an empty steer must not reach the backend).
        for empty in ["", "   "] {
            let r = cmd.execute(&ctx, empty).await.unwrap();
            assert!(
                matches!(r.kind, CommandResultKind::SystemMessage(ref s) if s.contains("Usage")),
                "empty arg {empty:?} must be usage, got {:?}",
                r.kind
            );
        }
    }

    // cyril-vgcm C10: `/steer clear` — trimmed EXACT case-sensitive match only.
    // One assert per design input shape. Bug classes: starts_with("clear")
    // (would eat "clear the tests"), case-folding (would eat "Clear"),
    // missing trim (would miss "clear ").
    #[tokio::test]
    async fn steer_clear_subcommand_parses() {
        let cmd = crate::commands::builtin::SteerCommand;
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };

        // Exact word, with and without surrounding whitespace -> ClearSteer.
        for arg in ["clear", " clear ", "clear "] {
            let r = cmd.execute(&ctx, arg).await.unwrap();
            assert!(
                matches!(r.kind, CommandResultKind::ClearSteer),
                "{arg:?} must be ClearSteer, got {:?}",
                r.kind
            );
        }
        // Case variation and multi-word args stay steerable TEXT (D2).
        for arg in ["Clear", "CLEAR", "clear the tests", "clear now"] {
            let r = cmd.execute(&ctx, arg).await.unwrap();
            assert!(
                matches!(r.kind, CommandResultKind::Steer { ref text } if text == arg.trim()),
                "{arg:?} must steer as text, got {:?}",
                r.kind
            );
        }
        // Empty still returns usage (unchanged fence, now naming both forms).
        let r = cmd.execute(&ctx, "").await.unwrap();
        assert!(
            matches!(r.kind, CommandResultKind::SystemMessage(ref s)
                if s.contains("Usage") && s.contains("clear")),
            "usage must mention the clear form, got {:?}",
            r.kind
        );
    }

    // cyril-bm1j Slice 12: /steer is registered and routes its args through parse().
    #[test]
    fn steer_command_registered_and_parses_args() {
        let registry =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        let (cmd, args) = registry.parse("/steer go now").unwrap();
        assert_eq!(cmd.name(), "steer");
        assert_eq!(args, "go now");
    }

    #[tokio::test]
    async fn help_command_returns_system_message() {
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
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
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
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
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };

        let result = builtin::QuitCommand.execute(&ctx, "").await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().kind, CommandResultKind::Quit));
    }

    #[tokio::test]
    async fn voice_command_returns_toggle_voice() {
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };

        let result = builtin::VoiceToggleCommand.execute(&ctx, "").await;
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap().kind,
            CommandResultKind::ToggleVoice
        ));
    }

    #[test]
    fn voice_command_registered_and_parses() {
        let registry =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        let (cmd, args) = registry.parse("/voice").expect("/voice is registered");
        assert_eq!(cmd.name(), "voice");
        assert_eq!(args, "");
    }

    #[tokio::test]
    async fn usage_command_is_local_without_a_session() {
        let session = crate::session::SessionController::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };
        let registry =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        let (command, args) = registry.parse("/usage").expect("/usage is registered");
        let result = command.execute(&ctx, args).await.expect("/usage executes");
        assert!(matches!(
            result.kind,
            CommandResultKind::ShowUsage {
                account_query_started: false
            }
        ));
        assert!(
            matches!(
                rx.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ),
            "local usage command must not send to the ACP bridge"
        );
    }

    #[tokio::test]
    async fn kas_usage_marks_account_query_for_app_dispatch() {
        let mut session = crate::session::SessionController::new();
        session.set_session(
            crate::types::SessionId::new("sess_kas"),
            crate::types::SessionStatus::Active,
        );
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };
        let registry = CommandRegistry::with_builtins_and_usage(
            HooksCommandSource::Agent,
            WorkflowCommandSource::None,
            UsageAccountCommandSource::Kas,
        );
        let (command, args) = registry.parse("/usage").expect("/usage is registered");
        let result = command.execute(&ctx, args).await.expect("/usage executes");
        assert!(matches!(
            result.kind,
            CommandResultKind::ShowUsage {
                account_query_started: true
            }
        ));
        assert!(matches!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn new_command_sends_bridge_command() {
        let session = crate::session::SessionController::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
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
    fn register_agent_commands_skips_local_non_selection() {
        let mut registry = CommandRegistry::new();
        let cmds = vec![
            crate::types::CommandInfo::new("quit", "Quit", None::<&str>, false, false, true), // local, not selection → skip
            crate::types::CommandInfo::new("compact", "Compact", None::<&str>, false, false, false), // not local → register
        ];
        registry.register_agent_commands(&cmds);
        assert!(
            registry.parse("/compact").is_some(),
            "non-local command should register"
        );
        assert!(
            registry.parse("/quit").is_none(),
            "local non-selection should be skipped"
        );
    }

    #[test]
    fn register_agent_commands_keeps_local_selection() {
        let mut registry = CommandRegistry::new();
        let cmds = vec![
            crate::types::CommandInfo::new("chat", "Chat", None::<&str>, true, true, true), // local AND selection → keep
        ];
        registry.register_agent_commands(&cmds);
        assert!(
            registry.parse("/chat").is_some(),
            "local selection command should be kept for picker UI"
        );
    }

    #[test]
    fn register_agent_commands_skips_builtin_names() {
        let mut registry =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        let cmds = vec![crate::types::CommandInfo::new(
            "help",
            "Agent Help",
            None::<&str>,
            false,
            false,
            false,
        )];
        registry.register_agent_commands(&cmds);
        // Should still be the builtin help, not agent help
        let (cmd, _) = registry.parse("/help").unwrap();
        assert!(
            cmd.is_local(),
            "builtin should not be overwritten by agent command"
        );
    }

    #[test]
    fn default_registry_has_builtins() {
        let registry =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        assert!(registry.parse("/help").is_some());
        assert!(registry.parse("/clear").is_some());
        assert!(registry.parse("/quit").is_some());
        assert!(registry.parse("/q").is_some());
        assert!(registry.parse("/new").is_some());
    }

    // --- AgentCommand execution tests ---

    #[tokio::test]
    async fn agent_command_fails_without_session() {
        let session = crate::session::SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
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
    async fn agent_command_execute_sends_correct_command_and_format() {
        let mut session = crate::session::SessionController::new();
        session.set_session(
            crate::types::SessionId::new("sess_test"),
            crate::types::SessionStatus::Active,
        );
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
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
        if let crate::types::BridgeCommand::ExecuteCommand {
            command,
            session_id,
            args,
        } = bridge_cmd
        {
            assert_eq!(command, "compact");
            assert_eq!(session_id.as_str(), "sess_test");
            assert_eq!(args, serde_json::json!({}));
        } else {
            panic!("expected ExecuteCommand, got {bridge_cmd:?}");
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
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };

        let cmd = AgentCommand {
            name: "model".into(),
            description: "Switch model".into(),
            is_selection: true,
        };
        let result = cmd.execute(&ctx, "claude-sonnet").await;
        assert!(result.is_ok());

        let bridge_cmd = rx.recv().await.unwrap();
        if let crate::types::BridgeCommand::ExecuteCommand {
            command,
            session_id: _,
            args,
        } = bridge_cmd
        {
            assert_eq!(command, "model");
            assert_eq!(args["value"], "claude-sonnet");
        } else {
            panic!("expected ExecuteCommand, got {bridge_cmd:?}");
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
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
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
    #[tokio::test]
    async fn memory_status_command_is_local_and_typed() {
        let registry =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        let session = SessionController::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let status = crate::types::MemoryStatusView::ready(
            "instance",
            1,
            crate::types::MemoryStoreVersions::new(1, 1),
        );
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: Some(&status),
        };
        let (command, args) = registry.parse("/memory status").expect("memory command");
        let result = command.execute(&ctx, args).await.expect("memory status");
        match result.kind {
            CommandResultKind::MemoryStatus(actual) => assert_eq!(actual, status),
            other => panic!("expected typed memory status, got {other:?}"),
        }
        assert!(matches!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        let (_, invalid_args) = registry.parse("/memory other").expect("memory command");
        let invalid = command
            .execute(&ctx, invalid_args)
            .await
            .expect("memory usage");
        assert!(matches!(
            &invalid.kind,
            CommandResultKind::SystemMessage(text)
                if text == "Usage: /memory status | teach <text> | teach --replace <lesson-id> <text> | list | inspect <lesson-id>"
        ));
        let unavailable_ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };
        let unavailable = command
            .execute(&unavailable_ctx, "status")
            .await
            .expect("missing provider");
        assert!(matches!(
            &unavailable.kind,
            CommandResultKind::SystemMessage(text) if text == "Memory status unavailable."
        ));
    }

    #[tokio::test]
    async fn memory_commands_emit_typed_actions() {
        let registry =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        let session = SessionController::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let sender = crate::protocol::bridge::BridgeSender::from_sender(tx);
        let ctx = CommandContext {
            workspace: std::path::Path::new("/bound-workspace"),
            session: &session,
            bridge: &sender,
            subagent_tracker: None,
            workflow_tracker: None,
            memory_status: None,
        };
        let rows = [
            (
                "/memory teach prefer boring Rust",
                MemoryCommandAction::Teach {
                    text: "prefer boring Rust".to_owned(),
                },
            ),
            (
                "/memory teach --replace 00112233445566778899aabbccddeeff prefer explicit errors",
                MemoryCommandAction::Replace {
                    lesson_id: "00112233445566778899aabbccddeeff".to_owned(),
                    text: "prefer explicit errors".to_owned(),
                },
            ),
            ("/memory list", MemoryCommandAction::List),
            (
                "/memory inspect 00112233445566778899aabbccddeeff",
                MemoryCommandAction::Inspect {
                    lesson_id: "00112233445566778899aabbccddeeff".to_owned(),
                },
            ),
            // Any whitespace run separates tokens: a double space or a
            // pasted tab must not turn a replacement into a new lesson.
            (
                "/memory teach  --replace 00112233445566778899aabbccddeeff new text",
                MemoryCommandAction::Replace {
                    lesson_id: "00112233445566778899aabbccddeeff".to_owned(),
                    text: "new text".to_owned(),
                },
            ),
            (
                "/memory teach\t--replace\t00112233445566778899aabbccddeeff\tnew text",
                MemoryCommandAction::Replace {
                    lesson_id: "00112233445566778899aabbccddeeff".to_owned(),
                    text: "new text".to_owned(),
                },
            ),
            ("/memory   list  ", MemoryCommandAction::List),
            (
                "/memory teach   keep  internal   spacing ",
                MemoryCommandAction::Teach {
                    text: "keep  internal   spacing".to_owned(),
                },
            ),
        ];
        for (input, expected) in rows {
            let (command, args) = registry.parse(input).expect("memory command");
            let result = command.execute(&ctx, args).await.expect("typed action");
            assert!(matches!(
                result.kind,
                CommandResultKind::MemoryAction(actual) if actual == expected
            ));
        }
        for input in [
            "/memory",
            "/memory teach",
            "/memory teach   ",
            "/memory teach --replace",
            "/memory teach  --replace",
            "/memory teach --replace id",
            "/memory teach\t--replace\tid\t",
            "/memory inspect",
            "/memory inspect too many",
            "/memory list extra",
            "/memory status extra",
        ] {
            let (command, args) = registry.parse(input).expect("memory command");
            let result = command.execute(&ctx, args).await.expect("usage");
            assert!(matches!(result.kind, CommandResultKind::SystemMessage(_)));
        }
    }
}

#[cfg(test)]
mod hooks_source_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::types::AgentEngine;
    use crate::types::kas_hooks::KasHooksMode;

    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from("/workspace")
    }

    /// cyril-0qe6 C11: `/workflow` exists exactly when the engine is KAS —
    /// the buggy implementation (unconditional registration) hands v2 users
    /// a command whose bridge arm can only answer errors.
    #[test]
    fn workflow_command_is_kas_engine_only() {
        assert_eq!(
            WorkflowCommandSource::resolve(AgentEngine::Kas, root()),
            WorkflowCommandSource::Kas {
                workspace_root: root()
            }
        );
        assert_eq!(
            WorkflowCommandSource::resolve(AgentEngine::V2, root()),
            WorkflowCommandSource::None
        );

        let v2 =
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None);
        assert!(
            v2.parse("/workflow list").is_none(),
            "no /workflow under v2 — nothing would answer it"
        );

        let kas = CommandRegistry::with_builtins(
            HooksCommandSource::Agent,
            WorkflowCommandSource::Kas {
                workspace_root: root(),
            },
        );
        let (cmd, args) = kas
            .parse("/workflow status wf_1")
            .expect("registered under KAS");
        assert_eq!(cmd.name(), "workflow");
        assert_eq!(args, "status wf_1");
    }

    #[test]
    fn only_kas_engine_with_kas_hooks_gets_the_builtin() {
        // The full matrix. The load-bearing cell is (Kas, Host): cyril OWNS the
        // registry there and serves `_kiro/hooks/list` to the agent, so asking
        // the agent would query a registry it does not have. A resolver written
        // as `engine == Kas` alone passes every other cell and fails this one.
        assert_eq!(
            HooksCommandSource::resolve(AgentEngine::Kas, KasHooksMode::Kas, root()),
            HooksCommandSource::Kas {
                workspace_root: root()
            },
            "the resolved source carries the workspace root VERBATIM — re-deriving \
             it from the process cwd is the --cwd bug"
        );
        for (engine, mode) in [
            (AgentEngine::Kas, KasHooksMode::Host),
            (AgentEngine::Kas, KasHooksMode::Off),
            (AgentEngine::V2, KasHooksMode::Kas),
            (AgentEngine::V2, KasHooksMode::Host),
            (AgentEngine::V2, KasHooksMode::Off),
        ] {
            assert_eq!(
                HooksCommandSource::resolve(engine, mode, root()),
                HooksCommandSource::Agent,
                "{engine:?} + {mode:?} must leave /hooks to the agent"
            );
        }
    }

    #[test]
    fn builtin_hooks_registered_only_for_the_kas_source() {
        // Registering `hooks` under the Agent source would SHADOW v2's own
        // agent-advertised command until `commands/available` arrived to
        // overwrite it — a race the user would see as a broken /hooks at
        // startup. Absence is the guarantee.
        assert!(
            CommandRegistry::with_builtins(HooksCommandSource::Agent, WorkflowCommandSource::None)
                .parse("/hooks")
                .is_none()
        );
        let kas = CommandRegistry::with_builtins(
            HooksCommandSource::Kas {
                workspace_root: root(),
            },
            WorkflowCommandSource::None,
        );
        let (cmd, args) = kas.parse("/hooks disable audit").expect("registered");
        assert_eq!(cmd.name(), "hooks");
        assert_eq!(args, "disable audit");
    }
}

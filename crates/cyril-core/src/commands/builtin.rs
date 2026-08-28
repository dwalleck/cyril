use crate::commands::{
    Command, CommandContext, CommandResult, MemoryCommandAction, UsageAccountCommandSource,
};
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
                "Usage: /steer <message> | /steer clear".to_string(),
            ))
        } else if msg == "clear" {
            // `/steer clear` drops the queued steers (cyril-vgcm C10, D2).
            // EXACT trimmed match only, case-sensitive: "Clear" and
            // "clear the tests" stay steerable text — the carve-out is the
            // single bare lowercase word (a vanishingly rare steer).
            Ok(CommandResult::clear_steer())
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

/// /usage — open Cyril's local live-usage dashboard.
pub struct UsageCommand {
    account_source: UsageAccountCommandSource,
}

impl UsageCommand {
    pub fn new(account_source: UsageAccountCommandSource) -> Self {
        Self { account_source }
    }
}

#[async_trait::async_trait]
impl Command for UsageCommand {
    fn name(&self) -> &str {
        "usage"
    }

    fn description(&self) -> &str {
        "Show live token, cost, model, provider, and tool usage"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, _args: &str) -> crate::Result<CommandResult> {
        let account_query_requested =
            self.account_source == UsageAccountCommandSource::Kas && ctx.session.id().is_some();
        Ok(CommandResult::show_usage(account_query_requested))
    }
}
/// `/memory` — typed project lesson operations. The binary owns persistence.
pub struct MemoryCommand;

const MEMORY_USAGE: &str = "Usage: /memory status | teach <text> | teach --replace <lesson-id> <text> | list | inspect <lesson-id> | turns | inspect-turn <source-turn-id>";

#[async_trait::async_trait]
impl Command for MemoryCommand {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Show memory status, lessons, or captured source turns"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        // Tokenized on any whitespace run, never on a literal single space:
        // `teach  --replace <id> <text>` (double space, or a pasted tab) must
        // be a replacement, not a new lesson whose text starts with
        // `--replace`.
        let (subcommand, rest) = split_token(args);
        if subcommand == "status" && rest.is_empty() {
            let Some(status) = ctx.memory_status else {
                tracing::error!("CommandContext.memory_status is None — wiring error in App");
                return Ok(CommandResult::system_message(
                    "Memory status unavailable.".to_owned(),
                ));
            };
            return Ok(CommandResult::memory_status(status.clone()));
        }
        let action = match (subcommand, rest) {
            ("list", "") => Some(MemoryCommandAction::List),
            ("turns", "") => Some(MemoryCommandAction::Turns),
            ("inspect-turn", rest) => match split_token(rest) {
                (source_turn_id, "") if !source_turn_id.is_empty() => {
                    Some(MemoryCommandAction::InspectTurn {
                        source_turn_id: source_turn_id.to_owned(),
                    })
                }
                _ => None,
            },
            ("inspect", rest) => match split_token(rest) {
                (lesson_id, "") if !lesson_id.is_empty() => Some(MemoryCommandAction::Inspect {
                    lesson_id: lesson_id.to_owned(),
                }),
                _ => None,
            },
            ("teach", rest) => match split_token(rest) {
                ("--replace", replacement) => match split_token(replacement) {
                    (lesson_id, text) if !lesson_id.is_empty() && !text.is_empty() => {
                        Some(MemoryCommandAction::Replace {
                            lesson_id: lesson_id.to_owned(),
                            text: text.to_owned(),
                        })
                    }
                    _ => None,
                },
                (first, _) if !first.is_empty() => Some(MemoryCommandAction::Teach {
                    text: rest.to_owned(),
                }),
                _ => None,
            },
            _ => None,
        };
        Ok(action.map_or_else(
            || CommandResult::system_message(MEMORY_USAGE.to_owned()),
            CommandResult::memory_action,
        ))
    }
}

/// Split the leading whitespace-delimited token from `input`.
///
/// Returns `(token, remainder)` with the remainder trimmed at both ends so
/// internal spacing of lesson text is preserved while surrounding whitespace
/// (including a tab or a double space after the token) is not.
fn split_token(input: &str) -> (&str, &str) {
    let input = input.trim();
    match input.split_once(char::is_whitespace) {
        Some((token, rest)) => (token, rest.trim()),
        None => (input, ""),
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
        ctx.bridge
            .send(BridgeCommand::NewSession {
                cwd: ctx.workspace.to_path_buf(),
            })
            .await?;
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

/// `/hooks` on the KAS engine (cyril-gk17).
///
/// KAS advertises no `hooks` command of its own — its command surface is
/// skills-only — so on v2 this command is *not* registered and the agent's own
/// `hooks` command handles the slash instead. See
/// [`HooksCommandSource`](crate::commands::HooksCommandSource) for that split.
///
/// Under `kas_hooks = "kas"` the agent owns a file-watched `.kiro/hooks`
/// registry and executes those hooks itself, so this is cyril's only window
/// onto them:
///
/// - `/hooks` — list the registry (including disabled hooks).
/// - `/hooks enable|disable <name-or-id>` — flip a hook's `enabled` flag. The
///   agent rewrites the flag in the backing **file**, so the change persists
///   past the session; the command re-lists afterwards so the user sees it.
pub struct KasHooksCommand {
    /// The session's workspace root, sent as `workspacePaths`.
    ///
    /// Supplied at construction from the SAME value the bridge was spawned
    /// with, never re-derived from `std::env::current_dir()`: cyril accepts a
    /// `--cwd` flag, so the process cwd and the workspace root genuinely
    /// differ, and querying the wrong root returns an empty listing that
    /// reads as "you have no hooks".
    workspace_root: std::path::PathBuf,
}

impl KasHooksCommand {
    #[must_use]
    pub fn new(workspace_root: std::path::PathBuf) -> Self {
        Self { workspace_root }
    }

    fn workspace_paths(&self) -> Vec<std::path::PathBuf> {
        vec![self.workspace_root.clone()]
    }
}

#[async_trait::async_trait]
impl Command for KasHooksCommand {
    fn name(&self) -> &str {
        "hooks"
    }

    fn description(&self) -> &str {
        "List KAS hooks; /hooks enable|disable <name> toggles one"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        let Some(session_id) = ctx.session.id().cloned() else {
            return Ok(CommandResult::system_message(
                "No active session — hooks are session-scoped.".into(),
            ));
        };
        let workspace_paths = self.workspace_paths();

        let mut parts = args.split_whitespace();
        let enabled = match parts.next() {
            None => {
                ctx.bridge
                    .send(BridgeCommand::ListKasHooks {
                        session_id,
                        workspace_paths,
                    })
                    .await?;
                return Ok(CommandResult::dispatched());
            }
            Some("enable") => true,
            Some("disable") => false,
            Some(other) => {
                return Ok(CommandResult::system_message(format!(
                    "Unknown /hooks action {other:?}. Usage: /hooks | /hooks enable <name> | /hooks disable <name>"
                )));
            }
        };
        let Some(reference) = parts.next() else {
            return Ok(CommandResult::system_message(
                "Which hook? Usage: /hooks enable <name> | /hooks disable <name>".into(),
            ));
        };

        // Resolution needs a listing to have landed. Saying so beats sending a
        // name the agent will reject as an unknown hookId.
        let hook_id = match ctx.session.resolve_kas_hook_id(reference) {
            Ok(id) => id,
            Err(crate::session::HookRefError::NotFound) => {
                let known = ctx.session.kas_hooks().len();
                return Ok(CommandResult::system_message(if known == 0 {
                    "No hooks known yet — run /hooks first.".into()
                } else {
                    format!("No hook named {reference:?} in the {known} known hooks.")
                }));
            }
            Err(crate::session::HookRefError::Ambiguous(candidates)) => {
                return Ok(CommandResult::system_message(format!(
                    "{reference:?} is ambiguous across {} hooks — use the full id:\n  {}",
                    candidates.len(),
                    candidates
                        .iter()
                        .map(crate::types::hook::HookId::as_str)
                        .collect::<Vec<_>>()
                        .join("\n  ")
                )));
            }
        };

        ctx.bridge
            .send(BridgeCommand::SetKasHookEnabled {
                session_id,
                hook_id,
                enabled,
                workspace_paths,
            })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

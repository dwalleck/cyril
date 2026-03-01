use std::rc::Rc;
use std::sync::Arc;

use agent_client_protocol::{self as acp, Agent};
use anyhow::Result;
use serde_json::value::RawValue;
use tokio::sync::mpsc;

use cyril_core::path;
use cyril_core::session::{CONFIG_KEY_MODEL, SessionContext};

use crate::file_completer;
use crate::ui::{chat, input, picker, toolbar};

/// Channels used by command execution to communicate with the event loop.
pub struct CommandChannels {
    pub prompt_done_tx: mpsc::UnboundedSender<()>,
    pub cmd_response_tx: mpsc::UnboundedSender<String>,
}

/// Result of command execution -- signals whether the app should continue or quit.
pub enum CommandResult {
    Continue,
    Quit,
}

/// Stateless executor for slash commands and prompts.
///
/// Each method is an associated function that takes the dependencies it needs
/// as parameters, keeping App as a thin coordinator.
pub struct CommandExecutor;

impl CommandExecutor {
    /// Execute a parsed slash command, returning whether the app should continue.
    ///
    /// `agent_commands` is the only piece of input state needed (for `/help` display).
    /// The caller should call `take_input()` before invoking this method.
    pub async fn execute(
        cmd: ParsedCommand,
        session: &mut SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
        agent_commands: &[AgentCommand],
        toolbar: &mut toolbar::ToolbarState,
        picker: &mut Option<picker::PickerState>,
        channels: &CommandChannels,
    ) -> Result<CommandResult> {
        match cmd {
            ParsedCommand::Quit => Ok(CommandResult::Quit),
            ParsedCommand::Clear => {
                *chat = chat::ChatState::default();
                chat.add_system_message("Chat cleared.".to_string());
                Ok(CommandResult::Continue)
            }
            ParsedCommand::Help => {
                let mut help = String::from("Local commands:\n");
                for cmd in COMMANDS {
                    help.push_str(&format!("  {:<14} {}\n", cmd.name, cmd.description));
                }
                if !agent_commands.is_empty() {
                    help.push_str("\nAgent commands:\n");
                    for cmd in agent_commands {
                        help.push_str(&format!("  {:<20} {}\n", cmd.display_name(), cmd.description));
                    }
                }
                help.push_str("\nKeyboard shortcuts:\n");
                help.push_str("  Ctrl+C/Q     Quit\n");
                help.push_str("  Ctrl+M       Toggle mouse (off = copy mode)\n");
                help.push_str("  Esc          Cancel current request\n");
                help.push_str("  Tab          Accept autocomplete suggestion\n");
                help.push_str("  Shift+Enter  Newline in input\n");
                chat.add_system_message(help);
                chat.scroll_to_bottom();
                Ok(CommandResult::Continue)
            }
            ParsedCommand::New => {
                Self::create_new_session(session, conn, chat).await?;
                Ok(CommandResult::Continue)
            }
            ParsedCommand::Load(session_id) => {
                if session_id.is_empty() {
                    chat.add_system_message(
                        "Usage: /load <session-id>\nUse /sessions to list available sessions."
                            .to_string(),
                    );
                } else {
                    Self::load_session(session, conn, chat, &session_id).await?;
                }
                Ok(CommandResult::Continue)
            }
            ParsedCommand::Mode(mode_id) => {
                Self::set_mode(session, conn, chat, &mode_id).await?;
                Ok(CommandResult::Continue)
            }
            ParsedCommand::ModelSelect(model_id) => {
                Self::set_model(session, conn, chat, picker, channels, &model_id).await?;
                Ok(CommandResult::Continue)
            }
            ParsedCommand::Agent { name, input: arg } => {
                let command = if let Some(input_text) = arg {
                    format!("/{name} {input_text}")
                } else {
                    format!("/{name}")
                };
                Self::execute_agent_command(session, conn, chat, toolbar, channels, &command).await?;
                Ok(CommandResult::Continue)
            }
            ParsedCommand::Unknown(cmd) => {
                chat.add_system_message(format!(
                    "Unknown command: {cmd}\nType /help for available commands."
                ));
                Ok(CommandResult::Continue)
            }
        }
    }

    /// Send a user prompt to the agent.
    pub async fn send_prompt(
        session: &SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
        input: &mut input::InputState,
        toolbar: &mut toolbar::ToolbarState,
        channels: &CommandChannels,
    ) -> Result<()> {
        if input.is_empty() || toolbar.is_busy {
            return Ok(());
        }

        let session_id = match &session.id {
            Some(id) => id.clone(),
            None => {
                chat.add_system_message("No active session. Use /new to start one.".to_string());
                return Ok(());
            }
        };

        let text = input.take_input();
        chat.add_user_message(text.clone());
        chat.begin_streaming();
        chat.scroll_to_bottom();
        toolbar.is_busy = true;

        // Build content blocks: user text + any @-referenced file contents
        let mut content_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(text.clone()))];

        if let Some(ref completer) = input.file_completer {
            for path in file_completer::parse_file_references(&text, completer) {
                match completer.read_file(&path) {
                    Ok(contents) => {
                        let file_block = format!("<file path=\"{path}\">\n{contents}\n</file>");
                        content_blocks
                            .push(acp::ContentBlock::Text(acp::TextContent::new(file_block)));
                        tracing::info!("Attached @-referenced file: {path}");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read @-referenced file {path}: {e}");
                    }
                }
            }
        }

        let conn = conn.clone();
        let done_tx = channels.prompt_done_tx.clone();
        let response_tx = channels.cmd_response_tx.clone();
        tokio::task::spawn_local(async move {
            let result = conn
                .prompt(acp::PromptRequest::new(session_id, content_blocks))
                .await;

            if let Err(e) = result {
                tracing::error!("Prompt error: {e}");
                let _ = response_tx.send(format!("[Error] Prompt failed: {e}"));
            }
            let _ = done_tx.send(());
        });

        Ok(())
    }

    /// Create a new session, replacing the current one.
    pub async fn create_new_session(
        session: &mut SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
    ) -> Result<()> {
        let agent_cwd = path::to_agent(&session.cwd);
        match conn.new_session(acp::NewSessionRequest::new(agent_cwd)).await {
            Ok(response) => {
                session.set_session_id(response.session_id);
                if let Some(ref modes) = response.modes {
                    session.set_modes(modes);
                }
                if let Some(config_options) = response.config_options {
                    session.set_config_options(config_options);
                }
                *chat = chat::ChatState::default();
                chat.add_system_message("New session started.".to_string());
                chat.scroll_to_bottom();
            }
            Err(e) => {
                chat.add_system_message(format!("Failed to create session: {e}"));
            }
        }
        Ok(())
    }

    /// Load a previous session by ID.
    pub async fn load_session(
        session: &mut SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
        session_id_str: &str,
    ) -> Result<()> {
        let agent_cwd = path::to_agent(&session.cwd);
        let session_id = acp::SessionId::from(session_id_str.to_string());

        match conn
            .load_session(acp::LoadSessionRequest::new(
                session_id.clone(),
                agent_cwd,
            ))
            .await
        {
            Ok(_) => {
                session.set_session_id(session_id);
                *chat = chat::ChatState::default();
                chat.add_system_message(format!("Loaded session: {session_id_str}"));
                chat.scroll_to_bottom();
            }
            Err(e) => {
                chat.add_system_message(format!("Failed to load session: {e}"));
            }
        }
        Ok(())
    }

    /// Execute a Kiro agent command via the extension method.
    pub async fn execute_agent_command(
        session: &SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
        toolbar: &mut toolbar::ToolbarState,
        channels: &CommandChannels,
        command: &str,
    ) -> Result<()> {
        let session_id = match &session.id {
            Some(id) => id.clone(),
            None => {
                chat.add_system_message("No active session. Use /new to start one.".to_string());
                return Ok(());
            }
        };

        let params = serde_json::json!({
            "sessionId": session_id.to_string(),
            "command": command
        });

        let raw_params = RawValue::from_string(params.to_string())
            .map_err(|e| anyhow::anyhow!("Failed to serialize command params: {e}"))?;

        chat.begin_streaming();
        chat.scroll_to_bottom();
        toolbar.is_busy = true;

        let conn = conn.clone();
        let done_tx = channels.prompt_done_tx.clone();
        let response_tx = channels.cmd_response_tx.clone();
        let cmd_str = command.to_string();
        tokio::task::spawn_local(async move {
            let result = conn
                .ext_method(acp::ExtRequest::new(
                    "kiro.dev/commands/execute",
                    Arc::from(raw_params),
                ))
                .await;

            match result {
                Ok(resp) => {
                    tracing::info!("Command {cmd_str} response: {}", resp.0);
                    let displayed = if let Ok(val) = serde_json::from_str::<serde_json::Value>(resp.0.get()) {
                        if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                            let _ = response_tx.send(msg.to_string());
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if !displayed {
                        let _ = response_tx.send(resp.0.to_string());
                    }
                }
                Err(e) => {
                    tracing::error!("Command {cmd_str} error: {e}");
                    let _ = response_tx.send(format!("[Error] Command {cmd_str} failed: {e}"));
                }
            }
            let _ = done_tx.send(());
        });

        Ok(())
    }

    /// Switch the agent mode via session/set_mode.
    pub async fn set_mode(
        session: &mut SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
        mode_id: &str,
    ) -> Result<()> {
        if mode_id.is_empty() {
            if session.available_modes.is_empty() {
                chat.add_system_message(
                    "No modes available. The agent did not advertise any modes.".to_string(),
                );
            } else {
                let mut msg = String::from("Available modes:\n");
                for mode in &session.available_modes {
                    let current = session
                        .current_mode_id
                        .as_deref()
                        .is_some_and(|c| c == mode.id);
                    let marker = if current { " (active)" } else { "" };
                    msg.push_str(&format!("  {:<20} {}{}\n", mode.id, mode.name, marker));
                }
                msg.push_str("\nUsage: /mode <id>");
                chat.add_system_message(msg);
            }
            return Ok(());
        }

        let session_id = match &session.id {
            Some(id) => id.clone(),
            None => {
                chat.add_system_message("No active session.".to_string());
                return Ok(());
            }
        };

        match conn
            .set_session_mode(acp::SetSessionModeRequest::new(
                session_id,
                mode_id.to_string(),
            ))
            .await
        {
            Ok(_) => {
                session.current_mode_id = Some(mode_id.to_string());
                chat.add_system_message(format!("Switched to mode: {mode_id}"));
            }
            Err(e) => {
                chat.add_system_message(format!("Failed to set mode: {e}"));
            }
        }
        Ok(())
    }

    /// Switch the model via Kiro extension commands.
    pub async fn set_model(
        session: &SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
        picker: &mut Option<picker::PickerState>,
        channels: &CommandChannels,
        model_id: &str,
    ) -> Result<()> {
        let session_id = match &session.id {
            Some(id) => id.clone(),
            None => {
                chat.add_system_message("No active session.".to_string());
                return Ok(());
            }
        };

        if model_id.is_empty() {
            let params = serde_json::json!({
                "command": CONFIG_KEY_MODEL,
                "sessionId": session_id.to_string()
            });
            let raw_params = RawValue::from_string(params.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to serialize params: {e}"))?;

            match conn
                .ext_method(acp::ExtRequest::new(
                    "kiro.dev/commands/options",
                    Arc::from(raw_params),
                ))
                .await
            {
                Ok(resp) => {
                    Self::open_model_picker(chat, picker, resp.0.get());
                }
                Err(e) => {
                    chat.add_system_message(format!("Failed to query models: {e}"));
                }
            }
            return Ok(());
        }

        let conn = conn.clone();
        let response_tx = channels.cmd_response_tx.clone();
        let model_str = model_id.to_string();
        tokio::task::spawn_local(async move {
            match conn
                .set_session_config_option(acp::SetSessionConfigOptionRequest::new(
                    session_id,
                    CONFIG_KEY_MODEL.to_string(),
                    model_str.clone(),
                ))
                .await
            {
                Ok(_) => {
                    tracing::info!("Set model to: {model_str}");
                    let _ = response_tx.send(format!("Switched to model: {model_str}"));
                }
                Err(e) => {
                    tracing::error!("Failed to set model: {e}");
                    let _ = response_tx.send(format!("Failed to set model: {e}"));
                }
            }
        });
        Ok(())
    }

    /// Parse model options from a `_kiro.dev/commands/options` response and open a picker.
    pub fn open_model_picker(
        chat: &mut chat::ChatState,
        picker: &mut Option<picker::PickerState>,
        raw_json: &str,
    ) {
        let val: serde_json::Value = match serde_json::from_str(raw_json) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse model options: {e}");
                chat.add_system_message("Failed to parse model options.".to_string());
                return;
            }
        };

        let options = val
            .get("options")
            .and_then(|v| v.as_array())
            .or_else(|| val.as_array());

        match options {
            Some(opts) if !opts.is_empty() => {
                let picker_options: Vec<picker::PickerOption> = opts
                    .iter()
                    .map(|opt| {
                        let value = opt
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let name = opt
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&value)
                            .to_string();
                        let active = opt
                            .get("current")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        picker::PickerOption {
                            label: name,
                            value,
                            active,
                        }
                    })
                    .collect();
                *picker = Some(picker::PickerState::new(
                    "Select Model",
                    picker_options,
                    picker::PickerAction::SetModel,
                ));
            }
            _ => {
                tracing::info!("Model options raw response: {raw_json}");
                chat.add_system_message(
                    "No model options returned. Try /model <model-id> directly.".to_string(),
                );
            }
        }
    }

    /// Handle a confirmed picker selection.
    pub fn handle_picker_confirm(
        session: &SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        channels: &CommandChannels,
        state: picker::PickerState,
    ) {
        let value = match state.selected_value() {
            Some(v) => v.to_string(),
            None => {
                tracing::warn!("Picker confirmed with no selection");
                return;
            }
        };

        match state.action {
            picker::PickerAction::SetModel => {
                let session_id = match &session.id {
                    Some(id) => id.clone(),
                    None => {
                        tracing::warn!("Picker confirmed but no active session");
                        return;
                    }
                };

                let conn = conn.clone();
                let response_tx = channels.cmd_response_tx.clone();
                tokio::task::spawn_local(async move {
                    match conn
                        .set_session_config_option(acp::SetSessionConfigOptionRequest::new(
                            session_id,
                            CONFIG_KEY_MODEL.to_string(),
                            value.clone(),
                        ))
                        .await
                    {
                        Ok(_) => {
                            tracing::info!("Set model to: {value}");
                            let _ = response_tx.send(format!("Switched to model: {value}"));
                        }
                        Err(e) => {
                            tracing::error!("Failed to set model: {e}");
                            let _ = response_tx.send(format!("Failed to set model: {e}"));
                        }
                    }
                });
            }
        }
    }

    /// Send the next queued hook feedback as a prompt.
    pub fn flush_next_hook_feedback(
        session: &SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut chat::ChatState,
        toolbar: &mut toolbar::ToolbarState,
        channels: &CommandChannels,
        pending_hook_feedback: &mut Vec<String>,
    ) {
        if pending_hook_feedback.is_empty() {
            return;
        }

        let session_id = match &session.id {
            Some(id) => id.clone(),
            None => {
                pending_hook_feedback.clear();
                return;
            }
        };

        let text = pending_hook_feedback.remove(0);
        toolbar.is_busy = true;
        chat.begin_streaming();

        let conn = conn.clone();
        let done_tx = channels.prompt_done_tx.clone();
        let response_tx = channels.cmd_response_tx.clone();
        tokio::task::spawn_local(async move {
            let result = conn
                .prompt(acp::PromptRequest::new(
                    session_id,
                    vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                ))
                .await;

            if let Err(e) = result {
                tracing::error!("Hook feedback prompt error: {e}");
                let _ = response_tx.send(format!("[Error] Hook feedback failed: {e}"));
            }
            let _ = done_tx.send(());
        });
    }
}

/// Built-in slash commands.
#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub takes_arg: bool,
}

/// An agent-provided command from AvailableCommandsUpdate.
#[derive(Debug, Clone)]
pub struct AgentCommand {
    pub name: String,
    pub description: String,
    pub input_hint: Option<String>,
}

impl AgentCommand {
    pub fn from_available(cmd: &acp::AvailableCommand) -> Self {
        let input_hint = cmd.input.as_ref().map(|input| match input {
            acp::AvailableCommandInput::Unstructured(u) => u.hint.clone(),
            _ => String::new(),
        });
        Self {
            name: cmd.name.clone(),
            description: cmd.description.clone(),
            input_hint,
        }
    }

    /// Display name with / prefix for autocomplete.
    pub fn display_name(&self) -> String {
        format!("/{}", self.name)
    }

    pub fn takes_arg(&self) -> bool {
        self.input_hint.is_some()
    }
}

/// All available slash commands.
pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/clear",
        description: "Clear the chat history",
        takes_arg: false,
    },
    SlashCommand {
        name: "/help",
        description: "Show available commands",
        takes_arg: false,
    },
    SlashCommand {
        name: "/load",
        description: "Load a previous session by ID",
        takes_arg: true,
    },
    SlashCommand {
        name: "/mode",
        description: "Switch agent mode (e.g. /mode dotnet-dev)",
        takes_arg: true,
    },
    SlashCommand {
        name: "/model",
        description: "Switch model (e.g. /model claude-sonnet-4-6)",
        takes_arg: true,
    },
    SlashCommand {
        name: "/new",
        description: "Start a new session",
        takes_arg: false,
    },
    SlashCommand {
        name: "/quit",
        description: "Exit the application",
        takes_arg: false,
    },
];

/// Parsed slash command from user input.
#[derive(Debug)]
pub enum ParsedCommand {
    /// Built-in local commands.
    Clear,
    Help,
    Load(String),
    Mode(String),
    ModelSelect(String),
    New,
    Quit,
    /// Agent-provided command (name, optional input).
    Agent { name: String, input: Option<String> },
    Unknown(String),
}

/// Try to parse a slash command from input text.
pub fn parse_command(input: &str, agent_commands: &[AgentCommand]) -> Option<ParsedCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim().to_string();

    // Check built-in commands first
    match cmd {
        "/clear" => return Some(ParsedCommand::Clear),
        "/help" => return Some(ParsedCommand::Help),
        "/load" => return Some(ParsedCommand::Load(arg)),
        "/mode" => return Some(ParsedCommand::Mode(arg)),
        "/model" => return Some(ParsedCommand::ModelSelect(arg)),
        "/new" => return Some(ParsedCommand::New),
        "/quit" => return Some(ParsedCommand::Quit),
        _ => {}
    }

    // Check agent commands (strip the leading /)
    let cmd_name = &cmd[1..];
    if agent_commands.iter().any(|ac| ac.name == cmd_name) {
        return Some(ParsedCommand::Agent {
            name: cmd_name.to_string(),
            input: if arg.is_empty() { None } else { Some(arg) },
        });
    }

    Some(ParsedCommand::Unknown(cmd.to_string()))
}

/// A suggestion entry for autocomplete (can be local or agent command).
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub display_name: String,
    pub description: String,
    pub takes_arg: bool,
}

/// Return commands matching the given prefix (for autocomplete).
pub fn matching_suggestions(prefix: &str, agent_commands: &[AgentCommand]) -> Vec<Suggestion> {
    if prefix.is_empty() || !prefix.starts_with('/') {
        return Vec::new();
    }

    let mut suggestions: Vec<Suggestion> = Vec::new();

    // Built-in commands
    for cmd in COMMANDS {
        if cmd.name.starts_with(prefix) {
            suggestions.push(Suggestion {
                display_name: cmd.name.to_string(),
                description: cmd.description.to_string(),
                takes_arg: cmd.takes_arg,
            });
        }
    }

    // Agent commands
    for cmd in agent_commands {
        let display = cmd.display_name();
        if display.starts_with(prefix) {
            suggestions.push(Suggestion {
                display_name: display,
                description: cmd.description.clone(),
                takes_arg: cmd.takes_arg(),
            });
        }
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_agent_commands() -> Vec<AgentCommand> {
        Vec::new()
    }

    fn agent_commands_with_compact() -> Vec<AgentCommand> {
        vec![AgentCommand {
            name: "compact".to_string(),
            description: "Compact the context".to_string(),
            input_hint: Some("instructions".to_string()),
        }]
    }

    #[test]
    fn parse_slash_quit() {
        let cmd = parse_command("/quit", &no_agent_commands());
        assert!(matches!(cmd, Some(ParsedCommand::Quit)));
    }

    #[test]
    fn parse_slash_clear() {
        let cmd = parse_command("/clear", &no_agent_commands());
        assert!(matches!(cmd, Some(ParsedCommand::Clear)));
    }

    #[test]
    fn parse_slash_help() {
        let cmd = parse_command("/help", &no_agent_commands());
        assert!(matches!(cmd, Some(ParsedCommand::Help)));
    }

    #[test]
    fn parse_slash_new() {
        let cmd = parse_command("/new", &no_agent_commands());
        assert!(matches!(cmd, Some(ParsedCommand::New)));
    }

    #[test]
    fn parse_slash_load_with_arg() {
        let cmd = parse_command("/load abc-123", &no_agent_commands());
        match cmd {
            Some(ParsedCommand::Load(arg)) => assert_eq!(arg, "abc-123"),
            other => panic!("Expected Load(\"abc-123\"), got {other:?}"),
        }
    }

    #[test]
    fn parse_slash_load_no_arg() {
        let cmd = parse_command("/load", &no_agent_commands());
        match cmd {
            Some(ParsedCommand::Load(arg)) => assert_eq!(arg, ""),
            other => panic!("Expected Load(\"\"), got {other:?}"),
        }
    }

    #[test]
    fn parse_slash_mode_with_arg() {
        let cmd = parse_command("/mode code", &no_agent_commands());
        match cmd {
            Some(ParsedCommand::Mode(arg)) => assert_eq!(arg, "code"),
            other => panic!("Expected Mode(\"code\"), got {other:?}"),
        }
    }

    #[test]
    fn parse_slash_model_with_arg() {
        let cmd = parse_command("/model claude-sonnet", &no_agent_commands());
        match cmd {
            Some(ParsedCommand::ModelSelect(arg)) => assert_eq!(arg, "claude-sonnet"),
            other => panic!("Expected ModelSelect(\"claude-sonnet\"), got {other:?}"),
        }
    }

    #[test]
    fn parse_non_slash_returns_none() {
        let cmd = parse_command("hello world", &no_agent_commands());
        assert!(cmd.is_none());
    }

    #[test]
    fn parse_unknown_command() {
        let cmd = parse_command("/foobar", &no_agent_commands());
        match cmd {
            Some(ParsedCommand::Unknown(name)) => assert_eq!(name, "/foobar"),
            other => panic!("Expected Unknown(\"/foobar\"), got {other:?}"),
        }
    }

    #[test]
    fn parse_agent_command() {
        let cmd = parse_command("/compact", &agent_commands_with_compact());
        match cmd {
            Some(ParsedCommand::Agent { name, input }) => {
                assert_eq!(name, "compact");
                assert!(input.is_none());
            }
            other => panic!("Expected Agent {{ name: \"compact\" }}, got {other:?}"),
        }
    }

    #[test]
    fn parse_agent_command_with_input() {
        let cmd = parse_command("/compact reduce context", &agent_commands_with_compact());
        match cmd {
            Some(ParsedCommand::Agent { name, input }) => {
                assert_eq!(name, "compact");
                assert_eq!(input.as_deref(), Some("reduce context"));
            }
            other => panic!("Expected Agent with input, got {other:?}"),
        }
    }

    #[test]
    fn matching_suggestions_filters_by_prefix() {
        let suggestions = matching_suggestions("/cl", &no_agent_commands());
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].display_name, "/clear");
    }

    #[test]
    fn matching_suggestions_empty_for_non_slash() {
        let suggestions = matching_suggestions("hello", &no_agent_commands());
        assert!(suggestions.is_empty());
    }

    #[test]
    fn matching_suggestions_includes_agent_commands() {
        let suggestions = matching_suggestions("/com", &agent_commands_with_compact());
        let names: Vec<&str> = suggestions.iter().map(|s| s.display_name.as_str()).collect();
        assert!(names.contains(&"/compact"));
    }
}

use agent_client_protocol as acp;

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
        description: "Select model (not supported by kiro-cli)",
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
    ModelSelect,
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
        "/model" => return Some(ParsedCommand::ModelSelect),
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

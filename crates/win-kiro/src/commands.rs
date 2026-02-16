/// Built-in slash commands.
#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub takes_arg: bool,
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
    Clear,
    Help,
    Load(String),
    New,
    Quit,
    Unknown(String),
}

/// Try to parse a slash command from input text.
pub fn parse_command(input: &str) -> Option<ParsedCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim().to_string();

    Some(match cmd {
        "/clear" => ParsedCommand::Clear,
        "/help" => ParsedCommand::Help,
        "/load" => ParsedCommand::Load(arg),
        "/new" => ParsedCommand::New,
        "/quit" => ParsedCommand::Quit,
        _ => ParsedCommand::Unknown(cmd.to_string()),
    })
}

/// Return commands matching the given prefix (for autocomplete).
pub fn matching_commands(prefix: &str) -> Vec<&'static SlashCommand> {
    if prefix.is_empty() || !prefix.starts_with('/') {
        return Vec::new();
    }
    COMMANDS
        .iter()
        .filter(|cmd| cmd.name.starts_with(prefix))
        .collect()
}

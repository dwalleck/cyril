/// Types specific to Kiro extension notifications and commands.
/// These are deserialized from `kiro.dev/commands/available` and related
/// extension notifications that are not part of the standard ACP protocol.

/// A command received from the `kiro.dev/commands/available` extension notification.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct KiroExtCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_hint: Option<String>,
    #[serde(default)]
    pub meta: Option<KiroCommandMeta>,
}

/// Metadata for a Kiro command.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroCommandMeta {
    /// "selection" requires a dropdown, "panel" needs special rendering.
    pub input_type: Option<String>,
    /// Extension method to call for options (e.g. `_kiro.dev/commands/model/options`).
    pub options_method: Option<String>,
    /// If true, the command is purely local (e.g. /quit).
    #[serde(default)]
    pub local: bool,
}

impl KiroExtCommand {
    /// Whether this command can be executed via `kiro.dev/commands/execute`.
    /// Panel commands (like /context, /help) are allowed — they return structured
    /// data that we display in chat. Only selection commands and local-only
    /// commands are excluded.
    pub fn is_executable(&self) -> bool {
        match &self.meta {
            None => true,
            Some(meta) => !meta.local && meta.input_type.as_deref() != Some("selection"),
        }
    }
}

/// Payload for `kiro.dev/commands/available` ext_notification.
/// We try multiple shapes since the format isn't documented.
#[derive(serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum KiroCommandsPayload {
    /// `{ "commands": [...] }`
    Wrapped { commands: Vec<KiroExtCommand> },
    /// `{ "availableCommands": [...] }` (ACP-style)
    AcpStyle {
        #[serde(rename = "availableCommands")]
        commands: Vec<KiroExtCommand>,
    },
    /// Top-level array `[...]`
    Bare(Vec<KiroExtCommand>),
}

impl KiroCommandsPayload {
    pub(crate) fn commands(self) -> Vec<KiroExtCommand> {
        match self {
            Self::Wrapped { commands } => commands,
            Self::AcpStyle { commands } => commands,
            Self::Bare(commands) => commands,
        }
    }
}

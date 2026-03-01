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

#[cfg(test)]
mod tests {
    use super::*;

    // -- KiroExtCommand::is_executable() --

    #[test]
    fn is_executable_no_meta_returns_true() {
        let cmd = KiroExtCommand {
            name: "/compact".into(),
            description: String::new(),
            input_hint: None,
            meta: None,
        };
        assert!(cmd.is_executable());
    }

    #[test]
    fn is_executable_local_command_returns_false() {
        let cmd = KiroExtCommand {
            name: "/quit".into(),
            description: String::new(),
            input_hint: None,
            meta: Some(KiroCommandMeta {
                input_type: None,
                options_method: None,
                local: true,
            }),
        };
        assert!(!cmd.is_executable());
    }

    #[test]
    fn is_executable_selection_input_type_returns_false() {
        let cmd = KiroExtCommand {
            name: "/model".into(),
            description: String::new(),
            input_hint: None,
            meta: Some(KiroCommandMeta {
                input_type: Some("selection".into()),
                options_method: Some("_kiro.dev/commands/model/options".into()),
                local: false,
            }),
        };
        assert!(!cmd.is_executable());
    }

    #[test]
    fn is_executable_panel_input_type_returns_true() {
        let cmd = KiroExtCommand {
            name: "/context".into(),
            description: String::new(),
            input_hint: None,
            meta: Some(KiroCommandMeta {
                input_type: Some("panel".into()),
                options_method: None,
                local: false,
            }),
        };
        assert!(cmd.is_executable());
    }

    // -- KiroCommandsPayload deserialization --

    #[test]
    fn payload_wrapped_shape() {
        let json = r#"{ "commands": [{ "name": "/compact" }] }"#;
        let payload: KiroCommandsPayload = serde_json::from_str(json).unwrap();
        let cmds = payload.commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "/compact");
    }

    #[test]
    fn payload_acp_style_shape() {
        let json = r#"{ "availableCommands": [{ "name": "/help" }] }"#;
        let payload: KiroCommandsPayload = serde_json::from_str(json).unwrap();
        let cmds = payload.commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "/help");
    }

    #[test]
    fn payload_bare_array_shape() {
        let json = r#"[{ "name": "/context" }]"#;
        let payload: KiroCommandsPayload = serde_json::from_str(json).unwrap();
        let cmds = payload.commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "/context");
    }

    // -- KiroExtCommand serde defaults --

    #[test]
    fn command_minimal_deserialization() {
        let json = r#"{ "name": "/compact" }"#;
        let cmd: KiroExtCommand = serde_json::from_str(json).unwrap();
        assert_eq!(cmd.name, "/compact");
        assert_eq!(cmd.description, "");
        assert!(cmd.input_hint.is_none());
        assert!(cmd.meta.is_none());
    }

    #[test]
    fn command_full_deserialization() {
        let json = r#"{
            "name": "/model",
            "description": "Switch AI model",
            "input_hint": "Choose a model",
            "meta": {
                "inputType": "selection",
                "optionsMethod": "_kiro.dev/commands/model/options",
                "local": true
            }
        }"#;
        let cmd: KiroExtCommand = serde_json::from_str(json).unwrap();
        assert_eq!(cmd.name, "/model");
        assert_eq!(cmd.description, "Switch AI model");
        assert_eq!(cmd.input_hint.as_deref(), Some("Choose a model"));
        let meta = cmd.meta.unwrap();
        assert_eq!(meta.input_type.as_deref(), Some("selection"));
        assert_eq!(
            meta.options_method.as_deref(),
            Some("_kiro.dev/commands/model/options")
        );
        assert!(meta.local);
    }
}

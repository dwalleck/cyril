/// Metadata about an available slash command (from Kiro or builtin).
#[derive(Debug, Clone)]
pub struct CommandInfo {
    name: String,
    label: String,
    description: Option<String>,
    has_options: bool,
    is_selection: bool,
    is_local: bool,
}

impl CommandInfo {
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        description: Option<impl Into<String>>,
        has_options: bool,
        is_selection: bool,
        is_local: bool,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            description: description.map(Into::into),
            has_options: has_options || is_selection,
            is_selection,
            is_local,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn has_options(&self) -> bool {
        self.has_options
    }

    pub fn is_selection(&self) -> bool {
        self.is_selection
    }

    pub fn is_local(&self) -> bool {
        self.is_local
    }
}

/// An option for a selection command (e.g., model picker).
#[derive(Debug, Clone)]
pub struct CommandOption {
    pub label: String,
    pub value: String,
    pub description: Option<String>,
    pub group: Option<String>,
    pub is_current: bool,
}

/// A session configuration option (e.g., model, mode).
#[derive(Debug, Clone)]
pub struct ConfigOption {
    pub key: String,
    pub label: String,
    pub value: Option<String>,
    pub options: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_info_accessors() {
        let cmd = CommandInfo::new(
            "model",
            "Switch model",
            Some("Change the active model"),
            true,
            true,
            false,
        );
        assert_eq!(cmd.name(), "model");
        assert_eq!(cmd.label(), "Switch model");
        assert_eq!(cmd.description(), Some("Change the active model"));
        assert!(cmd.has_options());
        assert!(cmd.is_selection());
        assert!(!cmd.is_local());
    }

    #[test]
    fn command_info_no_description() {
        let cmd = CommandInfo::new("quit", "Quit", None::<&str>, false, false, true);
        assert_eq!(cmd.description(), None);
        assert!(!cmd.has_options());
        assert!(!cmd.is_selection());
        assert!(cmd.is_local());
    }

    #[test]
    fn command_option_fields() {
        let opt = CommandOption {
            label: "Claude Sonnet".into(),
            value: "claude-sonnet-4".into(),
            description: Some("Fast model".into()),
            group: Some("Anthropic".into()),
            is_current: true,
        };
        assert_eq!(opt.label, "Claude Sonnet");
        assert_eq!(opt.value, "claude-sonnet-4");
        assert!(opt.is_current);
    }

    #[test]
    fn command_info_selection_implies_has_options() {
        let cmd = CommandInfo::new("model", "Model", None::<&str>, false, true, false);
        assert!(cmd.has_options(), "is_selection should imply has_options");
    }

    #[test]
    fn config_option_fields() {
        let opt = ConfigOption {
            key: "model".into(),
            label: "Model".into(),
            value: Some("claude-sonnet-4".into()),
            options: vec!["claude-sonnet-4".into(), "claude-haiku-4.5".into()],
        };
        assert_eq!(opt.key, "model");
        assert_eq!(opt.options.len(), 2);
    }
}

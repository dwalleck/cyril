/// A prompt argument definition.
#[derive(Debug, Clone)]
pub struct PromptArgument {
    name: String,
    description: Option<String>,
    required: bool,
}

impl PromptArgument {
    pub fn new(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
        required: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(Into::into),
            required,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn required(&self) -> bool {
        self.required
    }

    pub fn hint(&self) -> String {
        if self.required {
            format!("<{}>", self.name)
        } else {
            format!("[{}]", self.name)
        }
    }
}

/// Metadata about an available prompt.
#[derive(Debug, Clone)]
pub struct PromptInfo {
    name: String,
    description: Option<String>,
    server_name: Option<String>,
    arguments: Vec<PromptArgument>,
}

impl PromptInfo {
    pub fn new(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
        server_name: Option<impl Into<String>>,
        arguments: Vec<PromptArgument>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(Into::into),
            server_name: server_name.map(Into::into),
            arguments,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    pub fn arguments(&self) -> &[PromptArgument] {
        &self.arguments
    }

    pub fn argument_hints(&self) -> String {
        self.arguments
            .iter()
            .map(|a| a.hint())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_hint_formatting() {
        let required = PromptArgument::new("target", Some("file to review"), true);
        assert_eq!(required.hint(), "<target>");

        let optional = PromptArgument::new("depth", None::<String>, false);
        assert_eq!(optional.hint(), "[depth]");
    }

    #[test]
    fn prompt_argument_hints() {
        let prompt = PromptInfo::new(
            "review",
            Some("Review code"),
            Some("file-prompts"),
            vec![
                PromptArgument::new("branch", None::<String>, true),
                PromptArgument::new("scope", None::<String>, false),
            ],
        );
        assert_eq!(prompt.argument_hints(), "<branch> [scope]");
    }

    #[test]
    fn prompt_no_arguments() {
        let prompt = PromptInfo::new("dg", Some("Code review"), Some("global"), vec![]);
        assert_eq!(prompt.argument_hints(), "");
    }
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigOption {
    pub key: String,
    pub label: String,
    pub value: Option<String>,
    pub options: Vec<ConfigOptionValue>,
}

/// One offered value of a select [`ConfigOption`] (cyril-vh69).
///
/// `value`/`name`/`description` are engine-neutral ACP fields. `effort` is
/// `Some` only when the KAS `_meta.kiro` block on the wire value carried a
/// usable effort spec — it stays `None` on the v2 engine, on options that are
/// not the model select, and whenever the block was absent or malformed (the
/// converter warns on malformed; absence is normal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigOptionValue {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub effort: Option<EffortMetadata>,
}

impl ConfigOptionValue {
    /// A plain value with no description and no effort metadata.
    pub fn new(value: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            name: name.into(),
            description: None,
            effort: None,
        }
    }

    /// Whether this value carries a usable effort spec.
    pub fn has_effort(&self) -> bool {
        self.effort.is_some()
    }
}

/// The effort levels a KAS model accepts, from the model select option's
/// `_meta.kiro` block (cyril-vh69).
///
/// Constructed only with a non-empty `levels` — `has_effort` is therefore
/// derived (`Option<EffortMetadata>::is_some`), never a stored flag, so the
/// wire's `hasEffort: true` paired with missing/empty `effortLevels` cannot
/// create a levels-less "effort model". A `defaultEffortLevel` outside
/// `levels` is warned and dropped to `None` rather than trusted (the picker
/// then marks nothing — no sentinel default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffortMetadata {
    levels: Vec<String>,
    default_level: Option<String>,
}

impl EffortMetadata {
    /// Build metadata from parsed wire fields. `None` when `levels` is empty:
    /// an effort spec without levels is unusable, and the caller (the KAS
    /// converter) has already warned with wire context.
    pub fn new(levels: Vec<String>, default_level: Option<String>) -> Option<Self> {
        if levels.is_empty() {
            return None;
        }
        let default_level = match default_level {
            Some(level) if levels.contains(&level) => Some(level),
            Some(level) => {
                tracing::warn!(
                    default = %level,
                    ?levels,
                    "KAS defaultEffortLevel is not one of the offered effortLevels; dropping it"
                );
                None
            }
            None => None,
        };
        Some(Self {
            levels,
            default_level,
        })
    }

    /// The levels the model accepts, in wire order. Never empty.
    pub fn levels(&self) -> &[String] {
        &self.levels
    }

    /// The model's `defaultEffortLevel`, guaranteed to be one of `levels`
    /// when present.
    pub fn default_level(&self) -> Option<&str> {
        self.default_level.as_deref()
    }

    /// Whether `level` is one of the offered levels — exact string equality,
    /// because KAS "succeeds" on an unoffered level and silently keeps the
    /// old one.
    pub fn offers(&self, level: &str) -> bool {
        self.levels.iter().any(|offered| offered == level)
    }
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
            options: vec![
                ConfigOptionValue::new("claude-sonnet-4", "Claude Sonnet 4"),
                ConfigOptionValue::new("claude-haiku-4.5", "Claude Haiku 4.5"),
            ],
        };
        assert_eq!(opt.key, "model");
        assert_eq!(opt.options.len(), 2);
        assert_eq!(opt.options[0].value, "claude-sonnet-4");
        assert!(!opt.options[0].has_effort());
    }

    #[test]
    fn effort_metadata_invariants() {
        // Empty levels are unconstructible (has_effort is derived, never a flag).
        assert!(EffortMetadata::new(vec![], Some("high".into())).is_none());
        assert!(EffortMetadata::new(vec![], None).is_none());

        // A default outside the offered levels is dropped, never trusted.
        let metadata = EffortMetadata::new(vec!["low".into(), "high".into()], Some("turbo".into()))
            .expect("non-empty levels construct");
        assert_eq!(metadata.default_level(), None);
        assert_eq!(metadata.levels(), ["low", "high"]);

        // A default inside the levels survives verbatim.
        let metadata = EffortMetadata::new(vec!["low".into(), "high".into()], Some("high".into()))
            .expect("non-empty levels construct");
        assert_eq!(metadata.default_level(), Some("high"));
    }
}

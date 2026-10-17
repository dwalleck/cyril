//! The KAS session `configOptions` catalog as domain state (cyril-vh69).
//!
//! On the KAS engine, model, effort, and mode are select options of the
//! session config catalog rather than advertised `kiro.dev` commands
//! (docs/kiro-2.31.1-wire-audit.md § B3). The catalog always arrives complete
//! — every `config_option_update` and every `session/set_config_option`
//! result rebuilds it wholesale — so the snapshot here replaces, never
//! merges, and every query helper reads the latest catalog as the only source
//! of truth. The wire contract for WHY nothing is trusted (unoffered models
//! accepted silently, invalid effort levels "succeeding" with an unchanged
//! `currentValue`) lives in `convert/kas.rs`; this module only holds and
//! queries what conversion already validated.

use crate::types::command::{CommandOption, ConfigOption, EffortMetadata};

/// The KAS `configOptions` id of the model select (docs/kiro-2.31.1-wire-audit.md
/// § B3). Effort is NOT a separate model knob: the merged surface sets the
/// level through [`EFFORT_CONFIG_ID`] after the model switch.
pub const MODEL_CONFIG_ID: &str = "model";
/// The KAS `configOptions` id of the session mode select.
pub const MODE_CONFIG_ID: &str = "mode";
/// The KAS `configOptions` id of the effort-level select. KAS only carries it
/// while the current model has effort support; the offered levels come from
/// the model option's own [`EffortMetadata`], not from this option's values.
pub const EFFORT_CONFIG_ID: &str = "effortLevel";

/// The latest KAS config catalog, replaced wholesale per notification.
///
/// Wraps the `Vec<ConfigOption>` every config notification carries and
/// answers the queries the `/model` `/effort` `/mode` commands and the App's
/// confirm path need — option presence, current values, picker rows, per-model
/// effort metadata, and confirm-time membership. Pure queries only: no
/// notification handling (that is `SessionController`'s job), no async, no
/// bridge access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KasConfigSnapshot {
    options: Vec<ConfigOption>,
}

impl KasConfigSnapshot {
    pub fn new(options: Vec<ConfigOption>) -> Self {
        Self { options }
    }

    /// The raw catalog, in wire order.
    pub fn options(&self) -> &[ConfigOption] {
        &self.options
    }

    /// The option named `config_id`, when the catalog offers it.
    pub fn option(&self, config_id: &str) -> Option<&ConfigOption> {
        self.options.iter().find(|option| option.key == config_id)
    }

    /// The option's reported `currentValue`, or `None` when the option is
    /// absent or carries no value.
    pub fn current_value(&self, config_id: &str) -> Option<&str> {
        self.option(config_id)
            .and_then(|option| option.value.as_deref())
    }

    /// The current model id — `current_value` of the model select.
    pub fn current_model(&self) -> Option<&str> {
        self.current_value(MODEL_CONFIG_ID)
    }

    /// Whether `value` is one of the values `config_id` currently offers.
    /// Exact string equality: KAS accepts unoffered values without complaint
    /// (and then fails the next prompt), so confirm-time revalidation must be
    /// precise, never prefix or case-folded.
    pub fn contains_value(&self, config_id: &str, value: &str) -> bool {
        self.option(config_id)
            .is_some_and(|option| option.options.iter().any(|o| o.value == value))
    }

    /// The effort metadata of the offered model `model_id`, when that model
    /// carries a usable effort spec. Models without effort support (`auto`,
    /// `hasEffort:false`) yield `None` — the wire carries no `effortLevel`
    /// option for them either, so offering a picker would be offering a
    /// silent no-op.
    pub fn effort_for_model(&self, model_id: &str) -> Option<&EffortMetadata> {
        self.option(MODEL_CONFIG_ID)?
            .options
            .iter()
            .find(|o| o.value == model_id)?
            .effort
            .as_ref()
    }

    /// Picker rows for `config_id`: one [`CommandOption`] per offered value,
    /// label/description from the wire, `is_current` on the option's
    /// `currentValue`, `group` always `None` (credit-rate badges are
    /// cyril-lxuo, out of scope). `None` when the catalog does not offer the
    /// option at all — the command layer's "not offered by this session"
    /// case, which a present-but-empty list deliberately shares (a picker
    /// with zero options is never opened).
    pub fn picker_options(&self, config_id: &str) -> Option<Vec<CommandOption>> {
        let option = self.option(config_id)?;
        if option.options.is_empty() {
            return None;
        }
        let current = option.value.as_deref();
        Some(
            option
                .options
                .iter()
                .map(|o| CommandOption {
                    label: o.name.clone(),
                    value: o.value.clone(),
                    description: o.description.clone(),
                    group: None,
                    is_current: current == Some(o.value.as_str()),
                })
                .collect(),
        )
    }

    /// Effort picker rows for the offered model `model_id`: one row per level
    /// of the model's [`EffortMetadata`], `is_current` on the model's
    /// `defaultEffortLevel` — the acceptance criteria's "default marked".
    /// `None` when the model has no effort metadata.
    pub fn effort_picker_options(&self, model_id: &str) -> Option<Vec<CommandOption>> {
        let effort = self.effort_for_model(model_id)?;
        let default = effort.default_level();
        Some(
            effort
                .levels()
                .iter()
                .map(|level| CommandOption {
                    label: level.clone(),
                    value: level.clone(),
                    description: None,
                    group: None,
                    is_current: default == Some(level.as_str()),
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::command::ConfigOptionValue;

    fn value(value: &str, name: &str, effort: Option<EffortMetadata>) -> ConfigOptionValue {
        ConfigOptionValue {
            value: value.into(),
            name: name.into(),
            description: None,
            effort,
        }
    }

    fn model_catalog() -> KasConfigSnapshot {
        KasConfigSnapshot::new(vec![
            ConfigOption {
                key: MODEL_CONFIG_ID.into(),
                label: "Model".into(),
                value: Some("auto".into()),
                options: vec![
                    value("auto", "Auto", None),
                    value(
                        "claude-sonnet-5.5",
                        "Claude Sonnet 5.5",
                        EffortMetadata::new(
                            vec![
                                "low".into(),
                                "medium".into(),
                                "high".into(),
                                "xhigh".into(),
                                "max".into(),
                            ],
                            Some("high".into()),
                        ),
                    ),
                    value("claude-haiku-4.5", "Claude Haiku 4.5", None),
                ],
            },
            ConfigOption {
                key: MODE_CONFIG_ID.into(),
                label: "Mode".into(),
                value: Some("vibe".into()),
                options: vec![
                    ConfigOptionValue {
                        description: Some("General coding assistance".into()),
                        ..value("vibe", "Default", None)
                    },
                    ConfigOptionValue {
                        description: Some("Structured feature development".into()),
                        ..value("spec", "Spec", None)
                    },
                ],
            },
        ])
    }

    /// Oracle: the helper contract each consumer (commands, App confirm path)
    /// leans on, hand-written from design § Supporting types.
    #[test]
    fn snapshot_queries_read_the_catalog() {
        let snapshot = model_catalog();

        assert_eq!(snapshot.options().len(), 2);
        assert_eq!(
            snapshot.option(MODEL_CONFIG_ID).map(|o| o.key.as_str()),
            Some(MODEL_CONFIG_ID)
        );
        assert!(snapshot.option("autopilot").is_none());

        assert_eq!(snapshot.current_value(MODEL_CONFIG_ID), Some("auto"));
        assert_eq!(snapshot.current_value(MODE_CONFIG_ID), Some("vibe"));
        assert_eq!(snapshot.current_value("missing"), None);
        assert_eq!(snapshot.current_model(), Some("auto"));

        assert!(snapshot.contains_value(MODEL_CONFIG_ID, "claude-haiku-4.5"));
        assert!(!snapshot.contains_value(MODEL_CONFIG_ID, "claude-haiku"));
        assert!(!snapshot.contains_value("missing", "auto"));

        let effort = snapshot
            .effort_for_model("claude-sonnet-5.5")
            .expect("sonnet 5.5 carries effort");
        assert_eq!(effort.levels().len(), 5);
        assert_eq!(effort.default_level(), Some("high"));
        assert!(snapshot.effort_for_model("auto").is_none());
        assert!(snapshot.effort_for_model("unknown-model").is_none());
    }

    #[test]
    fn picker_options_mark_the_current_value() {
        let snapshot = model_catalog();

        let models = snapshot
            .picker_options(MODEL_CONFIG_ID)
            .expect("model option present");
        assert_eq!(models.len(), 3);
        assert!(
            models.iter().all(|o| o.group.is_none()),
            "badges are cyril-lxuo, out of scope"
        );
        let current: Vec<&str> = models
            .iter()
            .filter(|o| o.is_current)
            .map(|o| o.value.as_str())
            .collect();
        assert_eq!(current, ["auto"], "exactly the currentValue is marked");
        assert_eq!(models[1].label, "Claude Sonnet 5.5");
        assert_eq!(models[1].value, "claude-sonnet-5.5");

        // Wire labels AND descriptions ride through.
        let modes = snapshot
            .picker_options(MODE_CONFIG_ID)
            .expect("mode option present");
        assert_eq!(modes[0].label, "Default");
        assert_eq!(
            modes[0].description.as_deref(),
            Some("General coding assistance")
        );

        assert!(snapshot.picker_options("missing").is_none());
    }

    #[test]
    fn effort_picker_marks_the_model_default() {
        let snapshot = model_catalog();
        let rows = snapshot
            .effort_picker_options("claude-sonnet-5.5")
            .expect("sonnet 5.5 has effort levels");
        assert_eq!(rows.len(), 5);
        let marked: Vec<&str> = rows
            .iter()
            .filter(|o| o.is_current)
            .map(|o| o.value.as_str())
            .collect();
        assert_eq!(marked, ["high"], "the model's defaultEffortLevel is marked");

        assert!(snapshot.effort_picker_options("auto").is_none());
        assert!(snapshot.effort_picker_options("unknown-model").is_none());
    }

    #[test]
    fn picker_options_never_opens_empty() {
        let snapshot = KasConfigSnapshot::new(vec![ConfigOption {
            key: "empty".into(),
            label: "Empty".into(),
            value: None,
            options: vec![],
        }]);
        assert!(
            snapshot.picker_options("empty").is_none(),
            "a present-but-empty option is the same refusal as an absent one"
        );
    }
}

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

use crate::types::BridgeCommand;
use crate::types::command::{CommandOption, ConfigOption, EffortMetadata};

/// The three KAS config selects cyril sets from a picker
/// (docs/kiro-2.31.1-wire-audit.md § B3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KasConfigId {
    /// `model` — the model select. Effort is NOT a separate model knob: the
    /// merged surface sets the level through [`KasConfigId::Effort`] after
    /// the model switch.
    Model,
    /// `mode` — the session mode select.
    Mode,
    /// `effortLevel` — present only while the current model has effort
    /// support. The offered levels come from the model option's own
    /// [`EffortMetadata`]; this option supplies the level in use.
    Effort,
}

impl KasConfigId {
    const ALL: [Self; 3] = [Self::Model, Self::Mode, Self::Effort];

    /// The wire `configId`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Mode => "mode",
            Self::Effort => "effortLevel",
        }
    }

    /// The id for a wire `configId`, or `None` for selects cyril does not
    /// set from a picker (`thinking`, `autopilot`, …).
    pub fn from_wire(config_id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.as_str() == config_id)
    }

    /// The setting's name in user-facing text ("Model set to …").
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Mode => "Mode",
            Self::Effort => "Effort",
        }
    }
}

/// What a config picker sets. The effort target names the model its levels
/// came from, so "an effort picker always knows its model" is a type fact
/// rather than a comment (review round 4, F11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigTarget {
    Model,
    Mode,
    /// The effort level of `model_id`: rows are that model's
    /// [`EffortMetadata`] levels, valid only while it is the current model.
    Effort {
        model_id: String,
    },
}

impl ConfigTarget {
    /// The config select this target writes.
    pub fn id(&self) -> KasConfigId {
        match self {
            Self::Model => KasConfigId::Model,
            Self::Mode => KasConfigId::Mode,
            Self::Effort { .. } => KasConfigId::Effort,
        }
    }
}

/// One openable config picker: what it sets and its rows, `is_current`
/// already marked from the snapshot.
#[derive(Debug, Clone)]
pub struct ConfigPicker {
    target: ConfigTarget,
    options: Vec<CommandOption>,
}

impl ConfigPicker {
    pub fn new(target: ConfigTarget, options: Vec<CommandOption>) -> Self {
        Self { target, options }
    }

    pub fn target(&self) -> &ConfigTarget {
        &self.target
    }

    pub fn options(&self) -> &[CommandOption] {
        &self.options
    }

    pub fn into_parts(self) -> (ConfigTarget, Vec<CommandOption>) {
        (self.target, self.options)
    }
}

/// One dispatched `session/set_config_option` (review round 4, F14): the
/// request side of the read-back the `SessionController` keeps while the ack
/// is in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KasConfigRequest {
    id: KasConfigId,
    value: String,
}

impl KasConfigRequest {
    pub fn new(id: KasConfigId, value: impl Into<String>) -> Self {
        Self {
            id,
            value: value.into(),
        }
    }

    pub fn id(&self) -> KasConfigId {
        self.id
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// The bridge command that performs this request.
    pub fn to_command(&self) -> BridgeCommand {
        BridgeCommand::SetConfigOption {
            config_id: self.id.as_str().to_owned(),
            value: self.value.clone(),
        }
    }
}

/// The latest KAS config catalog, replaced wholesale per notification.
///
/// Wraps the `Vec<ConfigOption>` every config notification carries and
/// answers the queries the `/model` `/effort` `/mode` commands, the App's
/// confirm path and the session's post-ack chain need — option presence,
/// current values, pickers, per-model effort metadata, and confirm-time
/// acceptance. Pure queries only: no notification handling (that is
/// `SessionController`'s job), no async, no bridge access.
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

    /// The `id` select, when the catalog offers it.
    pub fn option(&self, id: KasConfigId) -> Option<&ConfigOption> {
        self.options.iter().find(|option| option.key == id.as_str())
    }

    /// The select's reported `currentValue`, or `None` when the option is
    /// absent or carries no value.
    pub fn current_value(&self, id: KasConfigId) -> Option<&str> {
        self.option(id).and_then(|option| option.value.as_deref())
    }

    /// The current model id — `current_value` of the model select.
    pub fn current_model(&self) -> Option<&str> {
        self.current_value(KasConfigId::Model)
    }

    /// The effort metadata of the offered model `model_id`, when that model
    /// carries a usable effort spec. Models without effort support (`auto`,
    /// `hasEffort:false`) yield `None` — the wire carries no `effortLevel`
    /// option for them either, so offering a picker would be offering a
    /// silent no-op.
    pub fn effort_for_model(&self, model_id: &str) -> Option<&EffortMetadata> {
        self.option(KasConfigId::Model)?
            .options
            .iter()
            .find(|o| o.value == model_id)?
            .effort
            .as_ref()
    }

    /// The picker for `target`, or `None` when this catalog cannot offer it
    /// (a zero-row picker is never opened).
    ///
    /// - `Model`/`Mode`: one row per offered value, label/description from
    ///   the wire, `is_current` on the select's `currentValue`, `group`
    ///   always `None` (credit-rate badges are cyril-lxuo, out of scope).
    ///   An absent and a present-but-empty select are the same refusal —
    ///   the command layer's "not offered by this session".
    /// - `Effort { model_id }`: one row per level of the model's
    ///   [`EffortMetadata`], `is_current` on the level in use — the
    ///   `effortLevel` select's `currentValue` while `model_id` is current and
    ///   that value is one of its levels (review round 4, F19) — else on its
    ///   `defaultEffortLevel`. The post-ack chain is built right after KAS
    ///   applied the default, so it reads "default marked" either way.
    ///   `None` when the model has no effort metadata.
    pub fn picker(&self, target: ConfigTarget) -> Option<ConfigPicker> {
        let options = match &target {
            ConfigTarget::Model | ConfigTarget::Mode => {
                let option = self.option(target.id())?;
                let current = option.value.as_deref();
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
                    .collect::<Vec<_>>()
            }
            ConfigTarget::Effort { model_id } => {
                let effort = self.effort_for_model(model_id)?;
                let in_use = (self.current_model() == Some(model_id.as_str()))
                    .then(|| self.current_value(KasConfigId::Effort))
                    .flatten()
                    .filter(|level| effort.offers(level));
                let marked = in_use.or(effort.default_level());
                effort
                    .levels()
                    .iter()
                    .map(|level| CommandOption {
                        label: level.clone(),
                        value: level.clone(),
                        description: None,
                        group: None,
                        is_current: marked == Some(level.as_str()),
                    })
                    .collect()
            }
        };
        if options.is_empty() {
            return None;
        }
        Some(ConfigPicker::new(target, options))
    }

    /// Whether confirming `value` for `target` is still valid against THIS
    /// catalog — the confirm-time revalidation (review round 4, F10). The
    /// picker holds a point-in-time copy, and KAS accepts unoffered values
    /// silently and fails the next prompt, so the check is exact and against
    /// the same source the rows came from:
    ///
    /// - `Model`/`Mode`: `value` is one of the select's offered values.
    /// - `Effort { model_id }`: `model_id` is still the current model (a
    ///   delayed model ack must not let an old picker apply its level to a
    ///   different model — review round 2, F6) and offers `value`.
    pub fn accepts(&self, target: &ConfigTarget, value: &str) -> bool {
        match target {
            ConfigTarget::Model | ConfigTarget::Mode => self
                .option(target.id())
                .is_some_and(|option| option.options.iter().any(|o| o.value == value)),
            ConfigTarget::Effort { model_id } => {
                self.current_model() == Some(model_id.as_str())
                    && self
                        .effort_for_model(model_id)
                        .is_some_and(|effort| effort.offers(value))
            }
        }
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
        catalog_with_model("auto")
    }

    fn catalog_with_model(current: &str) -> KasConfigSnapshot {
        KasConfigSnapshot::new(vec![
            ConfigOption {
                key: KasConfigId::Model.as_str().into(),
                label: "Model".into(),
                value: Some(current.into()),
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
                key: KasConfigId::Mode.as_str().into(),
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

    fn marked(picker: &ConfigPicker) -> Vec<&str> {
        picker
            .options()
            .iter()
            .filter(|o| o.is_current)
            .map(|o| o.value.as_str())
            .collect()
    }

    #[test]
    fn config_ids_round_trip_the_wire() {
        for id in KasConfigId::ALL {
            assert_eq!(KasConfigId::from_wire(id.as_str()), Some(id));
        }
        assert_eq!(KasConfigId::Effort.as_str(), "effortLevel");
        assert_eq!(KasConfigId::from_wire("thinking"), None);
        assert_eq!(KasConfigId::from_wire("Model"), None, "exact wire ids only");
        let effort = ConfigTarget::Effort {
            model_id: "m".into(),
        };
        assert_eq!(effort.id(), KasConfigId::Effort);
        assert!(matches!(
            KasConfigRequest::new(KasConfigId::Mode, "plan").to_command(),
            BridgeCommand::SetConfigOption { config_id, value }
                if config_id == "mode" && value == "plan"
        ));
    }

    /// Oracle: the helper contract each consumer (commands, App confirm path)
    /// leans on, hand-written from design § Supporting types.
    #[test]
    fn snapshot_queries_read_the_catalog() {
        let snapshot = model_catalog();

        assert_eq!(snapshot.options().len(), 2);
        assert_eq!(
            snapshot.option(KasConfigId::Model).map(|o| o.key.as_str()),
            Some("model")
        );
        assert!(snapshot.option(KasConfigId::Effort).is_none());

        assert_eq!(snapshot.current_value(KasConfigId::Model), Some("auto"));
        assert_eq!(snapshot.current_value(KasConfigId::Mode), Some("vibe"));
        assert_eq!(snapshot.current_value(KasConfigId::Effort), None);
        assert_eq!(snapshot.current_model(), Some("auto"));

        let effort = snapshot
            .effort_for_model("claude-sonnet-5.5")
            .expect("sonnet 5.5 carries effort");
        assert_eq!(effort.levels().len(), 5);
        assert_eq!(effort.default_level(), Some("high"));
        assert!(effort.offers("max"));
        assert!(!effort.offers("Max"));
        assert!(snapshot.effort_for_model("auto").is_none());
        assert!(snapshot.effort_for_model("unknown-model").is_none());
    }

    #[test]
    fn select_pickers_mark_the_current_value() {
        let snapshot = model_catalog();

        let models = snapshot
            .picker(ConfigTarget::Model)
            .expect("model option present");
        assert_eq!(models.target(), &ConfigTarget::Model);
        assert_eq!(models.options().len(), 3);
        assert!(
            models.options().iter().all(|o| o.group.is_none()),
            "badges are cyril-lxuo, out of scope"
        );
        assert_eq!(
            marked(&models),
            ["auto"],
            "exactly the currentValue is marked"
        );
        assert_eq!(models.options()[1].label, "Claude Sonnet 5.5");
        assert_eq!(models.options()[1].value, "claude-sonnet-5.5");

        // Wire labels AND descriptions ride through.
        let modes = snapshot
            .picker(ConfigTarget::Mode)
            .expect("mode option present");
        assert_eq!(modes.options()[0].label, "Default");
        assert_eq!(
            modes.options()[0].description.as_deref(),
            Some("General coding assistance")
        );
    }

    #[test]
    fn effort_picker_marks_the_model_default() {
        let snapshot = model_catalog();
        let target = ConfigTarget::Effort {
            model_id: "claude-sonnet-5.5".into(),
        };
        let picker = snapshot
            .picker(target.clone())
            .expect("sonnet 5.5 has effort levels");
        assert_eq!(picker.target(), &target);
        assert_eq!(picker.options().len(), 5);
        assert_eq!(
            marked(&picker),
            ["high"],
            "the model's defaultEffortLevel is marked"
        );

        for model_id in ["auto", "unknown-model"] {
            assert!(
                snapshot
                    .picker(ConfigTarget::Effort {
                        model_id: model_id.into()
                    })
                    .is_none(),
                "{model_id} offers no effort picker"
            );
        }
    }

    /// F19: the tick follows the `effortLevel` select's `currentValue` only
    /// for the current model and only when it names an offered level.
    #[test]
    fn effort_picker_marks_the_level_in_use() {
        let with_effort = |current_model: &str, in_use: &str| {
            let mut options = catalog_with_model(current_model).options().to_vec();
            options.push(ConfigOption {
                key: KasConfigId::Effort.as_str().into(),
                label: "Effort".into(),
                value: Some(in_use.into()),
                options: vec![],
            });
            KasConfigSnapshot::new(options)
        };
        let sonnet = || ConfigTarget::Effort {
            model_id: "claude-sonnet-5.5".into(),
        };
        for (current_model, in_use, want, why) in [
            ("claude-sonnet-5.5", "max", "max", "the level in use"),
            (
                "claude-sonnet-5.5",
                "none",
                "high",
                "an unoffered level falls back",
            ),
            (
                "auto",
                "max",
                "high",
                "another model's level is not this one's",
            ),
        ] {
            let picker = with_effort(current_model, in_use)
                .picker(sonnet())
                .expect("sonnet 5.5 has effort levels");
            assert_eq!(marked(&picker), [want], "{why}");
        }
    }

    #[test]
    fn pickers_never_open_empty() {
        let snapshot = KasConfigSnapshot::new(vec![ConfigOption {
            key: "mode".into(),
            label: "Mode".into(),
            value: None,
            options: vec![],
        }]);
        assert!(
            snapshot.picker(ConfigTarget::Mode).is_none(),
            "a present-but-empty option is the same refusal as an absent one"
        );
        assert!(snapshot.picker(ConfigTarget::Model).is_none(), "absent");
    }

    /// F10: the confirm-time rule, per target.
    #[test]
    fn accepts_revalidates_against_the_row_source() {
        let snapshot = model_catalog();
        assert!(snapshot.accepts(&ConfigTarget::Model, "claude-haiku-4.5"));
        assert!(
            !snapshot.accepts(&ConfigTarget::Model, "claude-haiku"),
            "exact equality, never prefix"
        );
        assert!(snapshot.accepts(&ConfigTarget::Mode, "spec"));
        assert!(!snapshot.accepts(&ConfigTarget::Mode, "plan"));

        let sonnet = ConfigTarget::Effort {
            model_id: "claude-sonnet-5.5".into(),
        };
        assert!(
            !snapshot.accepts(&sonnet, "max"),
            "the effort model must be the current model (F6)"
        );
        let switched = catalog_with_model("claude-sonnet-5.5");
        assert!(switched.accepts(&sonnet, "max"));
        assert!(!switched.accepts(&sonnet, "none"), "unoffered level");
        assert!(
            !switched.accepts(
                &ConfigTarget::Effort {
                    model_id: "auto".into()
                },
                "max"
            ),
            "a model without effort metadata accepts no level"
        );
    }
}

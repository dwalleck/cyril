//! Extended-thinking on/off state for the main session's current model
//! (cyril-k3lz).
//!
//! Both engines gained a per-model thinking toggle in kiro-cli 2.23.0, each
//! with its own lever and its own report:
//!
//! - **v2** reports `reasoning{support, thinkingEnabled?}` on every
//!   `_kiro.dev/metadata` frame ([`ReasoningInfo`], cyril-q1xs) and is set with
//!   the `reasoning` TUI command.
//! - **KAS** (0.66.8) advertises a `thinking` select option in `configOptions`
//!   only while the current model is toggleable, and is set with
//!   `session/set_config_option`.
//!
//! [`ThinkingState`] is the single owner of deriving one engine-neutral state
//! from either report. `SessionController` (command gating) and `UiState`
//! (toolbar) both delegate to [`ThinkingState::apply_notification`] rather
//! than re-deriving it. The lever to use is a fact of the snapshot that
//! reported toggleability, so nothing here matches on the bound engine.

use crate::types::command::ConfigOption;
use crate::types::event::Notification;
use crate::types::session::{ReasoningInfo, ReasoningSupport};

/// The KAS `configOptions` id of the thinking toggle (KAS 0.66.8,
/// `docs/kiro-2.24.0-wire-audit.md` § 7.1).
pub const THINKING_CONFIG_ID: &str = "thinking";

const CONFIG_VALUE_ON: &str = "on";
const CONFIG_VALUE_OFF: &str = "off";

/// Which engine mechanism changes thinking for the current model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingLever {
    /// v2: `kiro.dev/commands/execute {command:"reasoning", args:{thinkingEnabled}}`.
    ReasoningCommand,
    /// KAS: `session/set_config_option {configId:"thinking", value:"on"|"off"}`.
    ConfigOption,
}

impl ThinkingLever {
    /// The KAS `thinking` option value for a requested state. KAS does not
    /// validate the value (anything but `"on"` means off), so cyril only
    /// ever produces these two literals.
    pub fn config_value(enabled: bool) -> &'static str {
        if enabled {
            CONFIG_VALUE_ON
        } else {
            CONFIG_VALUE_OFF
        }
    }
}

/// Thinking state of the main session's current model, re-derived from each
/// snapshot and never carried across one (cyril-838u).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ThinkingState {
    /// No snapshot for the current session yet (before any session, after a
    /// new one starts, after disconnect).
    #[default]
    Unreported,
    /// v2: `reasoning.support == "toggleable"`. `enabled` is `None` when the
    /// wire omits `thinkingEnabled` — seen before the first explicit change.
    /// Unknown is never defaulted to on.
    ToggleableByReasoning { enabled: Option<bool> },
    /// KAS: the snapshot carries a `thinking` option with value `on`/`off`.
    ToggleableByConfigOption { enabled: bool },
    /// v2: `reasoning.support == "alwaysOn"`.
    AlwaysOn,
    /// The model has no thinking toggle: v2 `unavailable` or an unrecognized
    /// support value, or a KAS snapshot without a usable `thinking` option.
    NotToggleable,
}

impl ThinkingState {
    /// Derive from a v2 `reasoning` snapshot. `support` decides
    /// toggleability; a `thinkingEnabled` on a non-toggleable model (kept by
    /// the tolerant parser) is ignored.
    pub fn from_reasoning(info: &ReasoningInfo) -> Self {
        match info.support() {
            ReasoningSupport::Toggleable => Self::ToggleableByReasoning {
                enabled: info.thinking_enabled(),
            },
            ReasoningSupport::AlwaysOn => Self::AlwaysOn,
            ReasoningSupport::Unavailable | ReasoningSupport::Other(_) => Self::NotToggleable,
        }
    }

    /// Derive from a KAS `configOptions` snapshot. Absence of the `thinking`
    /// option means the current model is not toggleable (KAS emits it only
    /// for toggleable models). A value other than `on`/`off` is warned and
    /// treated as not toggleable rather than guessed.
    pub fn from_config_options(options: &[ConfigOption]) -> Self {
        let Some(option) = options.iter().find(|o| o.key == THINKING_CONFIG_ID) else {
            return Self::NotToggleable;
        };
        match option.value.as_deref() {
            Some(CONFIG_VALUE_ON) => Self::ToggleableByConfigOption { enabled: true },
            Some(CONFIG_VALUE_OFF) => Self::ToggleableByConfigOption { enabled: false },
            other => {
                tracing::warn!(
                    value = ?other,
                    "KAS `thinking` config option has an unrecognized value; treating the model as not toggleable"
                );
                Self::NotToggleable
            }
        }
    }

    /// The lever that changes thinking, when the current model is toggleable.
    pub fn lever(&self) -> Option<ThinkingLever> {
        match self {
            Self::ToggleableByReasoning { .. } => Some(ThinkingLever::ReasoningCommand),
            Self::ToggleableByConfigOption { .. } => Some(ThinkingLever::ConfigOption),
            Self::Unreported | Self::AlwaysOn | Self::NotToggleable => None,
        }
    }

    /// Whether thinking is on, only when the model is toggleable AND the
    /// state is known. `AlwaysOn` is deliberately `None`: there is nothing
    /// to toggle, so there is no on/off to display.
    pub fn enabled(&self) -> Option<bool> {
        match self {
            Self::ToggleableByReasoning { enabled } => *enabled,
            Self::ToggleableByConfigOption { enabled } => Some(*enabled),
            Self::Unreported | Self::AlwaysOn | Self::NotToggleable => None,
        }
    }

    /// Apply a main-session notification. Snapshots replace the state; a new
    /// session or a disconnect resets it; a metadata frame without a
    /// `reasoning` block (pre-2.23.0, or a corrupt block already warned by
    /// the parser) leaves it unchanged. Returns whether the state changed.
    pub fn apply_notification(&mut self, notification: &Notification) -> bool {
        let next = match notification {
            Notification::MetadataUpdated {
                reasoning: Some(info),
                ..
            } => Self::from_reasoning(info),
            Notification::ConfigOptionsUpdated(options)
            | Notification::ConfigOptionSet { options, .. } => Self::from_config_options(options),
            Notification::SessionCreated { .. } | Notification::BridgeDisconnected { .. } => {
                Self::Unreported
            }
            _ => return false,
        };
        if *self == next {
            return false;
        }
        *self = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::session::{EffortLevel, EffortUpdate, SessionId};

    fn reasoning(support: ReasoningSupport, enabled: Option<bool>) -> ReasoningInfo {
        ReasoningInfo::new(support, enabled, Some(EffortLevel::High), vec![])
    }

    fn option(key: &str, value: Option<&str>) -> ConfigOption {
        ConfigOption {
            key: key.into(),
            label: key.into(),
            value: value.map(Into::into),
            options: vec!["on".into(), "off".into()],
        }
    }

    fn metadata(reasoning: Option<ReasoningInfo>) -> Notification {
        Notification::MetadataUpdated {
            context_usage: None,
            metering: None,
            tokens: None,
            duration_ms: None,
            effort: EffortUpdate::Unchanged,
            reasoning,
            session_id: None,
            refusal: None,
        }
    }

    /// C2 — every support × thinkingEnabled cell (oracle: spec "Thinking
    /// state" definitions, hand-written).
    #[test]
    fn from_reasoning_matrix() {
        use ReasoningSupport::{Other, Toggleable, Unavailable};
        use ThinkingState::{NotToggleable, ToggleableByReasoning};
        let other = || Other("futureValue".into());
        let cases = [
            (
                Toggleable,
                Some(true),
                ToggleableByReasoning {
                    enabled: Some(true),
                },
            ),
            (
                Toggleable,
                Some(false),
                ToggleableByReasoning {
                    enabled: Some(false),
                },
            ),
            (Toggleable, None, ToggleableByReasoning { enabled: None }),
            (ReasoningSupport::AlwaysOn, None, ThinkingState::AlwaysOn),
            (
                ReasoningSupport::AlwaysOn,
                Some(true),
                ThinkingState::AlwaysOn,
            ),
            (Unavailable, None, NotToggleable),
            (Unavailable, Some(false), NotToggleable),
            (other(), None, NotToggleable),
            (other(), Some(true), NotToggleable),
        ];
        for (support, enabled, want) in cases {
            let label = format!("{}/{enabled:?}", support.as_str());
            assert_eq!(
                ThinkingState::from_reasoning(&reasoning(support, enabled)),
                want,
                "reasoning cell {label}"
            );
        }
    }

    /// C2 — every configOptions cell (oracle: spec definitions).
    #[test]
    fn from_config_options_matrix() {
        use ThinkingState::*;
        let model = option("model", Some("claude-sonnet-4.6"));
        let cases: [(&str, Vec<ConfigOption>, ThinkingState); 6] = [
            ("empty", vec![], NotToggleable),
            ("no thinking", vec![model.clone()], NotToggleable),
            (
                "on",
                vec![model.clone(), option("thinking", Some("on"))],
                ToggleableByConfigOption { enabled: true },
            ),
            (
                "off",
                vec![option("thinking", Some("off")), model.clone()],
                ToggleableByConfigOption { enabled: false },
            ),
            (
                "bogus",
                vec![option("thinking", Some("bogus"))],
                NotToggleable,
            ),
            (
                "value absent",
                vec![option("thinking", None)],
                NotToggleable,
            ),
        ];
        for (label, options, want) in cases {
            assert_eq!(
                ThinkingState::from_config_options(&options),
                want,
                "config cell {label}"
            );
        }
    }

    /// C3 — replacement, reset and no-op rules (oracle: spec B8). Every row
    /// starts from a known non-default state so a reset is observable.
    #[test]
    fn apply_notification_rules() {
        let start = ThinkingState::ToggleableByReasoning {
            enabled: Some(false),
        };
        let cases: [(&str, Notification, ThinkingState, bool); 7] = [
            (
                "metadata with reasoning replaces",
                metadata(Some(reasoning(ReasoningSupport::Unavailable, None))),
                ThinkingState::NotToggleable,
                true,
            ),
            (
                "metadata without reasoning keeps",
                metadata(None),
                start.clone(),
                false,
            ),
            (
                "config options update replaces",
                Notification::ConfigOptionsUpdated(vec![option("thinking", Some("on"))]),
                ThinkingState::ToggleableByConfigOption { enabled: true },
                true,
            ),
            (
                "config option set replaces",
                Notification::ConfigOptionSet {
                    config_id: "model".into(),
                    options: vec![option("model", Some("gpt-5.6-luna"))],
                },
                ThinkingState::NotToggleable,
                true,
            ),
            (
                "session created resets",
                Notification::SessionCreated {
                    session_id: SessionId::new("s"),
                    current_mode: None,
                    current_model: None,
                    available_modes: vec![],
                    available_models: vec![],
                },
                ThinkingState::Unreported,
                true,
            ),
            (
                "disconnect resets",
                Notification::BridgeDisconnected {
                    reason: "gone".into(),
                },
                ThinkingState::Unreported,
                true,
            ),
            (
                "unrelated notification keeps",
                Notification::TurnCompleted {
                    stop_reason: crate::types::session::StopReason::EndTurn,
                },
                start.clone(),
                false,
            ),
        ];
        for (label, notification, want, want_changed) in cases {
            let mut state = start.clone();
            let changed = state.apply_notification(&notification);
            assert_eq!(state, want, "{label}: state");
            assert_eq!(changed, want_changed, "{label}: changed flag");
        }

        // An identical snapshot is not a change.
        let mut state = start.clone();
        assert!(
            !state.apply_notification(&metadata(Some(reasoning(
                ReasoningSupport::Toggleable,
                Some(false)
            )))),
            "identical snapshot must report unchanged"
        );
    }

    #[test]
    fn lever_and_enabled_per_state() {
        use ThinkingState::*;
        let cases = [
            (Unreported, None, None),
            (
                ToggleableByReasoning { enabled: None },
                Some(ThinkingLever::ReasoningCommand),
                None,
            ),
            (
                ToggleableByReasoning {
                    enabled: Some(true),
                },
                Some(ThinkingLever::ReasoningCommand),
                Some(true),
            ),
            (
                ToggleableByConfigOption { enabled: false },
                Some(ThinkingLever::ConfigOption),
                Some(false),
            ),
            (AlwaysOn, None, None),
            (NotToggleable, None, None),
        ];
        for (state, lever, enabled) in cases {
            assert_eq!(state.lever(), lever, "{state:?}: lever");
            assert_eq!(state.enabled(), enabled, "{state:?}: enabled");
        }
        assert_eq!(ThinkingLever::config_value(true), "on");
        assert_eq!(ThinkingLever::config_value(false), "off");
    }
}

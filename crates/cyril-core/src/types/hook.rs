use serde::{Deserialize, Serialize};

/// A KAS v2 hook's registry identifier: the composite
/// `"<absolute file path>#hook-<n>"`, not a bare name.
///
/// A newtype rather than a `String` because a hook has **two** addressable
/// strings — this id and its declared `name` — and only this one is accepted by
/// `_kiro/hooks/setEnabled`, which rewrites a file on the user's disk.
/// `SessionController::resolve_kas_hook_id` exists precisely to turn a name
/// into one of these, which is the evidence that the two are confusable;
/// carrying both as `String` leaves the distinction to convention.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HookId(String);

impl HookId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HookId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Metadata about a single hook configured in the agent, as returned by the
/// `hooks` command's response at `data.hooks[]`.
///
/// This is Kiro's display-oriented projection of its backend `HookConfig` —
/// no execution details. Hooks themselves run entirely inside
/// `kiro-cli-chat`; Cyril's role is display-only via the `/hooks` command.
///
/// Wire format from Kiro 1.29.6+:
/// ```json
/// {
///   "trigger": "PreToolUse",
///   "command": "echo hello",
///   "matcher": "read"
/// }
/// ```
///
/// The three optional fields below carry what the **KAS v2 hooks registry**
/// reports and this v2 projection does not (cyril-gk17). They are `Option`
/// rather than defaulted because absent and known-false are different facts:
/// a v2 hook whose `enabled` is `None` is not "disabled", it comes from a
/// registry that does not model enablement at all. All three are skipped when
/// serializing, so a v2 `HookInfo` still round-trips as exactly the original
/// three fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookInfo {
    /// Trigger name from Kiro's `HookTrigger` enum. Observed values:
    /// `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`, `AgentSpawn`.
    /// Kept as a raw `String` so new variants don't require a Cyril release.
    pub trigger: String,

    /// Shell command the hook executes on trigger.
    pub command: String,

    /// Optional tool name matcher (e.g., `"read"`). Kiro supports tool
    /// aliases so `"read"` matches both `read` and `fs_read`. `None` means
    /// the hook runs for every tool of that trigger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,

    /// KAS v2 only: the registry's hook id, a **composite**
    /// `"<absolute file path>#hook-<n>"` — not a bare name, and the only
    /// value `_kiro/hooks/setEnabled` accepts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<HookId>,

    /// KAS v2 only: the hook's declared name, which is what a user will type
    /// to address it rather than the composite [`id`](Self::id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// KAS v2 only: whether the hook is currently enabled. Listings request
    /// `includeDisabled`, so disabled hooks are present and must not be
    /// displayed as if they were live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl HookInfo {
    /// A hook from the **v2** projection, which carries only these three fields.
    ///
    /// The KAS-only fields being `Option` means every v2 construction site would
    /// otherwise spell out `id: None, name: None, enabled: None` — one type
    /// serving two registries, with half its fields inert in each. This
    /// constructor keeps that noise in one place, so adding a fourth KAS field
    /// touches this file rather than every caller.
    pub fn v2(
        trigger: impl Into<String>,
        command: impl Into<String>,
        matcher: Option<String>,
    ) -> Self {
        Self {
            trigger: trigger.into(),
            command: command.into(),
            matcher,
            id: None,
            name: None,
            enabled: None,
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_with_matcher() {
        let json = r#"{"trigger":"PreToolUse","command":"echo hi","matcher":"read"}"#;
        let hook: HookInfo = serde_json::from_str(json).unwrap();
        assert_eq!(hook.trigger, "PreToolUse");
        assert_eq!(hook.command, "echo hi");
        assert_eq!(hook.matcher.as_deref(), Some("read"));
    }

    #[test]
    fn deserialize_without_matcher_field() {
        let json = r#"{"trigger":"Stop","command":"notify-send done"}"#;
        let hook: HookInfo = serde_json::from_str(json).unwrap();
        assert_eq!(hook.trigger, "Stop");
        assert_eq!(hook.command, "notify-send done");
        assert!(hook.matcher.is_none());
    }

    #[test]
    fn deserialize_null_matcher() {
        let json = r#"{"trigger":"AgentSpawn","command":"foo","matcher":null}"#;
        let hook: HookInfo = serde_json::from_str(json).unwrap();
        assert!(hook.matcher.is_none());
    }

    #[test]
    fn deserialize_hooks_array() {
        let json = r#"[
            {"trigger":"PreToolUse","command":"pre","matcher":"read"},
            {"trigger":"PostToolUse","command":"post"}
        ]"#;
        let hooks: Vec<HookInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].matcher.as_deref(), Some("read"));
        assert!(hooks[1].matcher.is_none());
    }

    #[test]
    fn roundtrip_serialization_omits_null_matcher() {
        let hook = HookInfo::v2("Stop", "foo", None);
        let json = serde_json::to_string(&hook).unwrap();
        // matcher should not appear in the output at all, not even as null
        assert!(!json.contains("matcher"));
    }
}

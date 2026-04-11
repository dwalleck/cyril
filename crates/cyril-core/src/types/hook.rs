use serde::{Deserialize, Serialize};

/// Metadata about a single hook configured in the agent, as returned by the
/// `hooks` command's response at `data.hooks[]`.
///
/// This is Kiro's display-oriented projection of its backend `HookConfig` —
/// only three fields, no execution details. Hooks themselves run entirely
/// inside `kiro-cli-chat`; Cyril's role is strictly display-only via the
/// `/hooks` command.
///
/// Wire format from Kiro 1.29.6+:
/// ```json
/// {
///   "trigger": "PreToolUse",
///   "command": "echo hello",
///   "matcher": "read"
/// }
/// ```
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
        let hook = HookInfo {
            trigger: "Stop".into(),
            command: "foo".into(),
            matcher: None,
        };
        let json = serde_json::to_string(&hook).unwrap();
        // matcher should not appear in the output at all, not even as null
        assert!(!json.contains("matcher"));
    }
}

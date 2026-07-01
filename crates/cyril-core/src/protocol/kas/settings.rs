//! KAS settings handshake (cyril-nhzw): marshal the user's kiro-cli settings
//! (`~/.kiro/settings/cli.json`) into an `AgentSettings` object attached as
//! `clientCapabilities._meta.kiro.settings` at `initialize`. Without it, KAS runs
//! with its bare fallback flags (no toolSearch, no thinking, `invoke_sub_agent`
//! instead of `orchestrate_subagent`, …).
//!
//! The mapping is a **verbatim replica of v2's `zme()`** (extracted from
//! `kiro-tui-2.8.1.js`) — respecting the user's config and giving v2 parity rather
//! than inventing cyril's own posture. Prove-it confirmed KAS reads this handshake
//! (`experiments/conductor-spike/probe-kas-settings-subagent-orchestration-2.10.0.py`).
//!
//! KAS-only: v2 advertises no `_meta.kiro.settings` (`V2Engine` stays empty).

use agent_client_protocol as acp;
use serde_json::{Map, Value};

/// Read the global kiro-cli settings map at `path`.
///
/// An **absent** file yields an empty map (a KAS session with no user overrides is
/// legitimate). A **corrupt** file (unreadable, or not a JSON object) also yields an
/// empty map, but logs a `warn!` — missing and corrupt are distinct, and a parse
/// failure must never abort the `initialize` handshake. Load-bearing (CLAUDE.md:
/// don't collapse missing/corrupt; log before falling back), so this is real
/// runtime behavior, not a `debug_assert!`.
fn read_settings_at(path: &std::path::Path) -> Map<String, Value> {
    match std::fs::read_to_string(path) {
        Ok(s) => match serde_json::from_str::<Value>(&s) {
            Ok(Value::Object(m)) => m,
            Ok(_) => {
                tracing::warn!(path = %path.display(), "kiro cli.json is not a JSON object; using KAS defaults");
                Map::new()
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "kiro cli.json parse failed; using KAS defaults");
                Map::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "kiro cli.json read failed; using KAS defaults");
            Map::new()
        }
    }
}

/// Read the global kiro-cli settings (`$HOME/.kiro/settings/cli.json`). Missing
/// home or file → empty map (see [`read_settings_at`]). Workspace-scoped overlay is
/// out of scope for v1 (tracked: cyril-sa39).
pub(crate) fn read_cli_settings() -> Map<String, Value> {
    match crate::kiro_agent_config::home_dir() {
        Some(home) => read_settings_at(&home.join(".kiro/settings/cli.json")),
        None => Map::new(),
    }
}

/// The v2 `zme()` boolean mapping: `cli.json` key → `AgentSettings` key. A present
/// **boolean** value becomes `{enabled: <bool>}`; a non-boolean value is ignored.
/// Verbatim from `kiro-tui-2.8.1.js` (the independent source of truth).
const BOOL_MAP: &[(&str, &str)] = &[
    ("chat.enableThinking", "thinking"),
    ("chat.enableKnowledge", "knowledge"),
    ("chat.enableCodeIntelligence", "codeIntelligence"),
    ("chat.enableTodoList", "todoList"),
    ("chat.enableCheckpoint", "checkpoint"),
    ("chat.enableTangentMode", "tangentMode"),
    ("chat.disableAutoCompaction", "disableAutoCompaction"),
    ("chat.enableSubagent", "_subagent"),
    ("chat.enableDelegate", "_delegate"),
];

/// `AgentSettings` keys defaulted to `{enabled: true}` when absent after mapping.
/// `subagentOrchestration` has no `cli.json` source key — it is always default-on
/// (which selects the `orchestrate_subagent` tool; prove-it confirmed).
const DEFAULTS_ON: &[&str] = &[
    "codeIntelligence",
    "knowledge",
    "thinking",
    "subagentOrchestration",
];

/// Marshal a kiro-cli settings map into an `AgentSettings` JSON object, replicating
/// v2's `zme()`. Pure and total: any map in → a valid object out. Nested
/// `toolSearch`/`compaction` are added in slice B.
pub(crate) fn marshal_agent_settings(e: &Map<String, Value>) -> Value {
    let mut n = Map::new();
    for (src, dst) in BOOL_MAP {
        if let Some(Value::Bool(b)) = e.get(*src) {
            n.insert((*dst).to_string(), serde_json::json!({ "enabled": b }));
        }
    }
    for k in DEFAULTS_ON {
        n.entry((*k).to_string())
            .or_insert_with(|| serde_json::json!({ "enabled": true }));
    }
    Value::Object(n)
}

/// Build the `_meta` object cyril attaches to the KAS `initialize`
/// `clientCapabilities`: `{"kiro": {"settings": <AgentSettings>}}`. Reads the live
/// global cli.json each call (initialize is once per session).
pub(crate) fn kiro_settings_meta() -> acp::Meta {
    let settings = marshal_agent_settings(&read_cli_settings());
    let mut meta = acp::Meta::new();
    meta.insert(
        "kiro".to_string(),
        serde_json::json!({ "settings": settings }),
    );
    meta
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn absent_file_defaults() {
        // Claim 7: a nonexistent settings file yields an empty map, never an error
        // or panic (a KAS session with no user cli.json is normal). Fails if the
        // reader errors/propagates on NotFound instead of returning empty.
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope/cli.json");
        assert!(read_settings_at(&missing).is_empty());
    }

    #[test]
    fn malformed_file_defaults() {
        // Claim 8 (stress: missing ≠ corrupt): a file that exists but is invalid
        // JSON must yield an empty map, NOT panic and NOT abort initialize. Fails
        // under `serde_json::from_str(..).unwrap()` or `?`-propagation.
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("cli.json");
        std::fs::write(&bad, "{ this is not json").unwrap();
        assert!(read_settings_at(&bad).is_empty());
        // A JSON value that isn't an object (e.g. an array) is also tolerated.
        let arr = dir.path().join("arr.json");
        std::fs::write(&arr, "[1,2,3]").unwrap();
        assert!(read_settings_at(&arr).is_empty());
    }

    fn map(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }
    fn on() -> Value {
        serde_json::json!({ "enabled": true })
    }

    #[test]
    fn defaults_only() {
        // Claim 4: an empty settings map yields EXACTLY the four default-on keys,
        // nothing else. Fails if a default is missing or a non-default key leaks.
        let got = marshal_agent_settings(&Map::new());
        let obj = got.as_object().unwrap();
        assert_eq!(obj.len(), 4, "only the 4 defaults: {got}");
        for k in [
            "codeIntelligence",
            "knowledge",
            "thinking",
            "subagentOrchestration",
        ] {
            assert_eq!(obj.get(k), Some(&on()), "default {k}");
        }
    }

    #[test]
    fn all_bool_keys_map() {
        // Claim 2: each of the 9 bool cli.json keys present=true maps to its
        // AgentSettings key as {enabled:true}, per the zme table (no renamed key).
        let src: Vec<(&str, Value)> = BOOL_MAP
            .iter()
            .map(|(s, _)| (*s, Value::Bool(true)))
            .collect();
        let got = marshal_agent_settings(&map(&src));
        let obj = got.as_object().unwrap();
        for (_, dst) in BOOL_MAP {
            assert_eq!(obj.get(*dst), Some(&on()), "mapped key {dst}: {got}");
        }
    }

    #[test]
    fn nonbool_ignored() {
        // Claim 3 (stress): a non-default key with a NON-boolean value must be
        // omitted (no coercion), and a default key with a non-bool value falls back
        // to its default. Fails under `as_bool().unwrap_or(true)`-style coercion.
        let got = marshal_agent_settings(&map(&[
            ("chat.enableCheckpoint", Value::String("yes".into())), // non-default → omit
            ("chat.enableThinking", serde_json::json!(1)),          // default key, non-bool
        ]));
        let obj = got.as_object().unwrap();
        assert_eq!(
            obj.get("checkpoint"),
            None,
            "non-bool non-default omitted: {got}"
        );
        assert_eq!(
            obj.get("thinking"),
            Some(&on()),
            "thinking falls to default-on: {got}"
        );
    }

    #[test]
    fn false_preserved() {
        // Claim 4b (stress): a default key explicitly set to `false` stays
        // {enabled:false} — the default-on pass must NOT overwrite it. Fails under
        // `if v { insert }` (which drops false, then default-on re-adds it as true).
        let got = marshal_agent_settings(&map(&[("chat.enableThinking", Value::Bool(false))]));
        assert_eq!(
            got.as_object().unwrap().get("thinking"),
            Some(&serde_json::json!({ "enabled": false })),
            "explicit false preserved, not defaulted-on: {got}"
        );
    }
}

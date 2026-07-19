//! Hook registry for the KAS hooks host (cyril-jiyn, KAS-7).
//!
//! With `[agent] kas_hooks = "host"` cyril owns the hooks: this module loads
//! the user's on-disk hook files and serves them to KAS's `_kiro/hooks/list`
//! queries. Execution lives in the executor half of this module (slices
//! 5a/5b); wire dispatch in `client.rs` (slice 7).

use std::path::Path;

use agent_client_protocol as acp;

/// The acp-stripped method name for `_kiro/hooks/list` (the acp library strips
/// the leading underscore, per the `SHELL_TYPE_METHOD` precedent).
pub(crate) const LIST_METHOD: &str = "kiro/hooks/list";

/// A loaded, servable hook: one `runCommand` entry from a `.kiro/hooks/*.json`
/// file, keyed by the wire-side (camelCase) trigger it answers to.
#[derive(Debug, Clone)]
pub(crate) struct HookDef {
    /// Namespaced `<file-stem>:<name>` — duplicate names across files stay
    /// distinct and traceable to their source file.
    pub id: String,
    pub name: String,
    /// The wire trigger this hook answers (`promptSubmit`, `preToolUse`,
    /// `postToolUse`, `agentStop`, `sessionStart`) — mapped from the file's
    /// PascalCase (`.cyril-jiyn/findings.md` Q2: the two vocabularies differ).
    pub wire_trigger: &'static str,
    /// Optional tool-name matcher (regex, matching Kiro's own matcher
    /// semantics — a substring downgrade would silently never-fire patterns
    /// like `fs_.*`). Applied against `toolId` on `list`.
    pub matcher: Option<regex::Regex>,
    pub command: String,
}

/// The on-disk file schema (kasHookFileSchema shape; hooksBlock carve in
/// `.cyril-0wyn/`): `{version: "v1", hooks: [{name, trigger, matcher?,
/// action: {type, command?}}]}`.
#[derive(Debug, serde::Deserialize)]
struct HookFile {
    version: String,
    hooks: Vec<HookFileEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct HookFileEntry {
    name: String,
    trigger: String,
    #[serde(default)]
    matcher: Option<String>,
    action: HookAction,
}

#[derive(Debug, serde::Deserialize)]
struct HookAction {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    command: Option<String>,
}

/// PascalCase file trigger → camelCase wire trigger. `None` for triggers the
/// host wire model cannot serve (IDE file events, task events) or unknowns.
fn wire_trigger(file_trigger: &str) -> Option<&'static str> {
    match file_trigger {
        "UserPromptSubmit" => Some("promptSubmit"),
        "Stop" => Some("agentStop"),
        "PreToolUse" => Some("preToolUse"),
        "PostToolUse" => Some("postToolUse"),
        "SessionStart" => Some("sessionStart"),
        _ => None,
    }
}

/// The loaded hook set for one bridge lifetime (no hot-reload: cyril-2adk).
#[derive(Debug, Default)]
pub(crate) struct HookRegistry {
    hooks: Vec<HookDef>,
}

impl HookRegistry {
    /// Load hooks from the workspace root's `.kiro/hooks/` and the global
    /// `~/.kiro/hooks/`. Every per-file and per-entry problem is a `warn` +
    /// skip — one bad file must never take down the rest (the load runs at
    /// bridge startup on user-authored content).
    pub(crate) fn load(workspace_root: &Path, global_kiro_home: Option<&Path>) -> Self {
        let mut hooks = Vec::new();
        let mut dirs = vec![workspace_root.join(".kiro").join("hooks")];
        if let Some(home) = global_kiro_home {
            dirs.push(home.join("hooks"));
        }
        for dir in dirs {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    tracing::warn!(dir = %dir.display(), error = %e, "hooks dir unreadable; skipping");
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                Self::load_file(&path, &mut hooks);
            }
        }
        tracing::info!(count = hooks.len(), "KAS hooks host: registry loaded");
        Self { hooks }
    }

    fn load_file(path: &Path, out: &mut Vec<HookDef>) {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("hooks")
            .to_string();
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "hook file unreadable; skipped");
                return;
            }
        };
        let file: HookFile = match serde_json::from_str(&text) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "hook file is not valid hook JSON; skipped");
                return;
            }
        };
        if file.version != "v1" {
            tracing::warn!(file = %path.display(), version = %file.version, "unknown hook file version; skipped");
            return;
        }
        for entry in file.hooks {
            let Some(trigger) = wire_trigger(&entry.trigger) else {
                tracing::warn!(
                    file = %path.display(), hook = %entry.name, trigger = %entry.trigger,
                    "trigger not servable in host mode; hook skipped"
                );
                continue;
            };
            if entry.action.kind != "command" {
                // agent-type actions need a prompt-injection vehicle: cyril-n03f.
                tracing::warn!(
                    file = %path.display(), hook = %entry.name, kind = %entry.action.kind,
                    "non-command hook action not executed in host mode; hook skipped"
                );
                continue;
            }
            let Some(command) = entry.action.command.filter(|c| !c.is_empty()) else {
                tracing::warn!(file = %path.display(), hook = %entry.name, "command action without a command; skipped");
                continue;
            };
            let matcher = match entry.matcher.as_deref() {
                None => None,
                Some(m) => match regex::Regex::new(m) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        tracing::warn!(
                            file = %path.display(), hook = %entry.name, matcher = %m, error = %e,
                            "invalid matcher regex; hook skipped"
                        );
                        continue;
                    }
                },
            };
            out.push(HookDef {
                id: format!("{stem}:{}", entry.name),
                name: entry.name,
                wire_trigger: trigger,
                matcher,
                command,
            });
        }
    }

    /// Answer the `_kiro/hooks/list` ext request from its params, replying
    /// `{hooks: [...]}`. A missing `trigger` yields an empty list (the agent
    /// always sends one; a malformed frame should not error the turn).
    pub(crate) fn respond_list(&self, params: &serde_json::Value) -> acp::Result<acp::ExtResponse> {
        let trigger = params.get("trigger").and_then(|t| t.as_str()).unwrap_or("");
        let tool_id = params.get("toolId").and_then(|t| t.as_str());
        let hooks = self.list(trigger, tool_id);
        let body = serde_json::to_string(&serde_json::json!({ "hooks": hooks }))
            .map_err(|e| acp::Error::new(-32603, format!("serialize hooks/list reply: {e}")))?;
        let raw = serde_json::value::RawValue::from_string(body)
            .map_err(|e| acp::Error::new(-32603, format!("hooks/list raw value: {e}")))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }

    /// Answer `_kiro/hooks/list {trigger, toolId?}`: the hooks whose wire
    /// trigger equals `trigger`, honoring each hook's optional tool-name
    /// matcher against `tool_id`. A matcher-carrying hook is excluded when
    /// `tool_id` is absent (there is nothing to match) or does not match; an
    /// unknown trigger simply yields an empty list, never an error.
    pub(crate) fn list(&self, trigger: &str, tool_id: Option<&str>) -> Vec<serde_json::Value> {
        self.hooks
            .iter()
            .filter(|h| h.wire_trigger == trigger)
            .filter(|h| match &h.matcher {
                None => true,
                Some(rx) => tool_id.is_some_and(|t| rx.is_match(t)),
            })
            .map(|h| {
                serde_json::json!({
                    "id": h.id,
                    "name": h.name,
                    "action": {"type": "runCommand", "command": h.command},
                    "approved": true,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), body).unwrap();
    }

    // cyril-jiyn claim 4 fence (mapping half): a valid load maps PascalCase
    // file triggers to the wire vocabulary and namespaces ids by file stem.
    #[test]
    fn hook_registry_loads_and_maps() {
        let ws = tempfile::tempdir().unwrap();
        let hooks_dir = ws.path().join(".kiro/hooks");
        write(
            &hooks_dir,
            "team.json",
            r#"{"version":"v1","hooks":[
                {"name":"lint","trigger":"PreToolUse","matcher":"fs_.*",
                 "action":{"type":"command","command":"echo lint"}},
                {"name":"greet","trigger":"UserPromptSubmit",
                 "action":{"type":"command","command":"echo hi"}}
            ]}"#,
        );
        // A global root with one Stop hook — both sources merge.
        let global = tempfile::tempdir().unwrap();
        write(
            &global.path().join("hooks"),
            "personal.json",
            r#"{"version":"v1","hooks":[
                {"name":"bye","trigger":"Stop","action":{"type":"command","command":"echo bye"}}
            ]}"#,
        );

        let reg = HookRegistry::load(ws.path(), Some(global.path()));
        let mut got: Vec<(&str, &str)> = reg
            .hooks
            .iter()
            .map(|h| (h.id.as_str(), h.wire_trigger))
            .collect();
        got.sort_unstable();
        assert_eq!(
            got,
            vec![
                ("personal:bye", "agentStop"),
                ("team:greet", "promptSubmit"),
                ("team:lint", "preToolUse"),
            ]
        );
        assert!(
            reg.hooks.iter().any(|h| h.matcher.is_some()),
            "the matcher survived the load"
        );
    }

    // cyril-jiyn claim 4 fence (skip half): the stress-fixture load — one dir
    // containing an invalid-JSON file, an unknown trigger, an agent action,
    // an invalid matcher regex, and a duplicate name in a second file.
    // Expected: exactly the servable hooks load; nothing aborts.
    #[test]
    fn hook_registry_skips_invalid_without_aborting() {
        let ws = tempfile::tempdir().unwrap();
        let dir = ws.path().join(".kiro/hooks");
        write(&dir, "broken.json", "{ not json !!!");
        write(
            &dir,
            "mixed.json",
            r#"{"version":"v1","hooks":[
                {"name":"good","trigger":"PostToolUse","action":{"type":"command","command":"echo ok"}},
                {"name":"filey","trigger":"PostFileSave","action":{"type":"command","command":"echo nope"}},
                {"name":"agenty","trigger":"Stop","action":{"type":"agent"}},
                {"name":"badrx","trigger":"PreToolUse","matcher":"fs_(","action":{"type":"command","command":"echo nope"}}
            ]}"#,
        );
        write(
            &dir,
            "second.json",
            r#"{"version":"v1","hooks":[
                {"name":"good","trigger":"UserPromptSubmit","action":{"type":"command","command":"echo dup"}}
            ]}"#,
        );
        write(
            &dir,
            "oldver.json",
            r#"{"version":"v2","hooks":[
                {"name":"future","trigger":"Stop","action":{"type":"command","command":"echo nope"}}
            ]}"#,
        );

        let reg = HookRegistry::load(ws.path(), None);
        let mut ids: Vec<&str> = reg.hooks.iter().map(|h| h.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            vec!["mixed:good", "second:good"],
            "exactly the servable hooks load; duplicates across files both survive under distinct ids"
        );
    }

    // cyril-jiyn claim 5 fence: list honors trigger + matcher-vs-toolId, and an
    // unknown trigger is empty (not an error). The matcher hook and the
    // no-matcher hook share a trigger so the two toolId cases differ only by
    // the matcher — a matcher-ignoring impl makes the first two asserts equal.
    #[test]
    fn hooks_list_filtering() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "h.json",
            r#"{"version":"v1","hooks":[
                {"name":"fsonly","trigger":"PreToolUse","matcher":"fs_.*",
                 "action":{"type":"command","command":"echo fs"}},
                {"name":"always","trigger":"PreToolUse",
                 "action":{"type":"command","command":"echo any"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);

        let names = |v: &[serde_json::Value]| -> Vec<String> {
            let mut n: Vec<String> = v
                .iter()
                .map(|h| h["id"].as_str().unwrap().to_string())
                .collect();
            n.sort();
            n
        };

        // fs_write matches the matcher → both hooks.
        assert_eq!(
            names(&reg.list("preToolUse", Some("fs_write"))),
            vec!["h:always", "h:fsonly"]
        );
        // execute_bash misses the matcher → only the no-matcher hook.
        assert_eq!(
            names(&reg.list("preToolUse", Some("execute_bash"))),
            vec!["h:always"]
        );
        // No toolId → matcher hooks can't match → only the no-matcher hook.
        assert_eq!(names(&reg.list("preToolUse", None)), vec!["h:always"]);
        // Unknown trigger → empty, not an error.
        assert!(reg.list("bogusTrigger", Some("fs_write")).is_empty());
    }
}

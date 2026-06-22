//! Persisting Kiro command-trust grants to the active agent's config file.
//!
//! When the user grants "Always allow → \<tier\>" in the approval dialog, Kiro
//! honors it for the current session but reprompts next session — over ACP it
//! never writes the grant to disk (its native TUI does). To make trust persist,
//! cyril writes the chosen tier's regex patterns into the agent's own config at
//! `toolsSettings.<tool>.allowedCommands`, which a future `kiro-cli acp` reads.
//!
//! This is Kiro-specific and side-effecting, so it lives in `cyril-core` rather
//! than the UI. cyril only ever **appends** to a config the user authored — it
//! never creates one, and it refuses to touch Kiro's built-in agents.
//!
//! Wire facts (probed against kiro-cli 2.5.1):
//! - Grants live at `toolsSettings.execute_bash.allowedCommands` (array of regex
//!   strings); the `setting_key` on the wire (`allowedCommands`) names the field.
//! - Agent configs resolve from a workspace `<cwd>/.kiro/agents/<name>.json` that
//!   **fully overrides** the global `~/.kiro/agents/<name>.json` (not a merge), so
//!   cyril writes to whichever Kiro actually reads — workspace first.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Kiro's built-in agents. cyril did not author these and must not write trust
/// grants into them — they keep their session-scoped behavior.
const BUILTIN_AGENTS: &[&str] = &["kiro_default", "kiro_planner", "kiro_guide"];

/// Map a trust `setting_key` (from `_meta.trustOptions[].setting_key`) to the
/// `toolsSettings` tool key it belongs under in the agent config.
fn tool_for_setting_key(setting_key: &str) -> Option<&'static str> {
    match setting_key {
        "allowedCommands" => Some("execute_bash"),
        // "runtime_read_paths" => Some("fs_read"), // out-of-workspace reads — future
        _ => None,
    }
}

/// Errors from persisting a trust grant. Persistence failure is non-fatal — the
/// session-scoped grant already succeeded — but it must be visible, never silent.
#[derive(Debug, thiserror::Error)]
pub enum TrustPersistError {
    #[error("agent name '{0}' is not a plain filename component")]
    InvalidAgentName(String),
    #[error("agent '{0}' is a Kiro built-in; trust is not persisted for it")]
    BuiltinAgent(String),
    #[error("unknown trust setting_key '{0}'")]
    UnknownSettingKey(String),
    #[error("no on-disk config for agent '{0}' (workspace or global)")]
    NoConfig(String),
    #[error("reading agent config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("agent config {path} is not a JSON object")]
    NotAnObject { path: PathBuf },
    #[error("parsing agent config {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("serializing agent config: {source}")]
    Serialize { source: serde_json::Error },
    #[error("writing agent config {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// True only if `name` is a single, normal path component — no separators, no
/// `.`/`..`, not empty, not absolute. `agent_name` is server-supplied (it comes
/// from Kiro's `currentModeId` / `AgentSwitched.name`, ultimately derived from a
/// config filename), so it must never be trusted to build a path: a value like
/// `../../foo` would resolve outside `.kiro/agents/`. Validate at the boundary.
fn is_plain_agent_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(std::path::Component::Normal(_)), None)
    )
}

/// The user's home directory: `HOME`, falling back to `USERPROFILE` so it
/// behaves the same inside WSL and on Linux. Shared by the agent-config path
/// resolution and the KAS-engine free-path spawn discovery.
pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// The directory holding global agent configs (`~/.kiro/agents`), if a home
/// directory can be determined.
fn global_agents_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join(".kiro").join("agents"))
}

/// Resolve the agent config file Kiro actually reads for `agent_name` when
/// spawned in `cwd`: the workspace file shadows the global one, so prefer
/// `<cwd>/.kiro/agents/<name>.json`, then `~/.kiro/agents/<name>.json`.
///
/// Returns `None` when neither exists — cyril only appends to a config the user
/// authored, so there is nothing to write.
pub fn resolve_agent_config_path(agent_name: &str, cwd: &Path) -> Option<PathBuf> {
    resolve_in_dirs(agent_name, cwd, global_agents_dir().as_deref())
}

/// Resolution core with an injectable global dir, so tests don't mutate `HOME`
/// (env mutation is `unsafe` in Rust 2024, which the workspace forbids).
fn resolve_in_dirs(agent_name: &str, cwd: &Path, global_dir: Option<&Path>) -> Option<PathBuf> {
    if !is_plain_agent_name(agent_name) {
        tracing::warn!(
            agent = agent_name,
            "refusing to resolve a config path from a non-plain agent name"
        );
        return None;
    }
    let file = format!("{agent_name}.json");

    let workspace = cwd.join(".kiro").join("agents").join(&file);
    if workspace.is_file() {
        return Some(workspace);
    }

    if let Some(global) = global_dir.map(|d| d.join(&file))
        && global.is_file()
    {
        return Some(global);
    }

    tracing::debug!(
        agent = agent_name,
        "no workspace or global agent config found; trust grant not persisted"
    );
    None
}

/// Append `patterns` into `toolsSettings.<tool>.allowedCommands` of the active
/// agent's config, so a future `kiro-cli acp` auto-approves matching commands.
///
/// - Refuses Kiro built-in agents and unknown `setting_key`s.
/// - Re-reads the file immediately before writing (a parallel native-TUI write
///   may have landed) and preserves every other field.
/// - Writes atomically (temp + rename in the same directory).
///
/// Returns the path written on success. A no-op success (all patterns already
/// present) still returns the path without rewriting the file.
pub fn persist_trust_grant(
    agent_name: &str,
    cwd: &Path,
    setting_key: &str,
    patterns: &[String],
) -> Result<PathBuf, TrustPersistError> {
    if !is_plain_agent_name(agent_name) {
        return Err(TrustPersistError::InvalidAgentName(agent_name.to_string()));
    }
    if BUILTIN_AGENTS.contains(&agent_name) {
        return Err(TrustPersistError::BuiltinAgent(agent_name.to_string()));
    }
    let tool = tool_for_setting_key(setting_key)
        .ok_or_else(|| TrustPersistError::UnknownSettingKey(setting_key.to_string()))?;
    let path = resolve_agent_config_path(agent_name, cwd)
        .ok_or_else(|| TrustPersistError::NoConfig(agent_name.to_string()))?;

    // Re-read fresh to shrink the window where a concurrent native-TUI write is
    // lost (temp+rename narrows but cannot fully close it).
    let content = std::fs::read_to_string(&path).map_err(|source| TrustPersistError::Read {
        path: path.clone(),
        source,
    })?;
    let mut root: serde_json::Value =
        serde_json::from_str(&content).map_err(|source| TrustPersistError::Parse {
            path: path.clone(),
            source,
        })?;

    let added = merge_allowed_commands(&mut root, tool, patterns)
        .ok_or_else(|| TrustPersistError::NotAnObject { path: path.clone() })?;

    if added == 0 {
        tracing::debug!(
            agent = agent_name,
            "trust patterns already present; no write"
        );
        return Ok(path);
    }

    let serialized = serde_json::to_string_pretty(&root)
        .map_err(|source| TrustPersistError::Serialize { source })?;
    write_atomic(&path, &serialized).map_err(|source| TrustPersistError::Write {
        path: path.clone(),
        source,
    })?;

    tracing::info!(
        agent = agent_name,
        path = %path.display(),
        added,
        "persisted trust grant to agent config"
    );
    Ok(path)
}

/// Merge `patterns` into `root.toolsSettings.<tool>.allowedCommands`, creating
/// the nested objects/array as needed. Returns the count of newly-added patterns,
/// or `None` if `root` (or a node that must be an object) is not an object.
fn merge_allowed_commands(
    root: &mut serde_json::Value,
    tool: &str,
    patterns: &[String],
) -> Option<usize> {
    let obj = root.as_object_mut()?;
    let tools_settings = obj
        .entry("toolsSettings")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()?;
    let tool_obj = tools_settings
        .entry(tool)
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()?;
    let arr = tool_obj
        .entry("allowedCommands")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()?;

    // Owned snapshot of existing values so the dedup set doesn't borrow `arr`
    // while we push into it.
    let existing: HashSet<String> = arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let mut added = 0;
    for p in patterns {
        if !existing.contains(p) {
            arr.push(serde_json::Value::String(p.clone()));
            added += 1;
        }
    }
    Some(added)
}

/// Write `content` to `path` atomically: write a sibling temp file, then rename
/// over the target (rename is atomic within a directory on POSIX).
fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "agent config path has no parent directory",
        )
    })?;
    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "agent config path has no file name",
        )
    })?;
    let tmp = dir.join(format!(".{file_name}.cyril.tmp"));

    // Best-effort cleanup so a partial write or a failed rename never strands the
    // temp file next to the user's config.
    let cleanup = || {
        if let Err(rm) = std::fs::remove_file(&tmp) {
            tracing::debug!(tmp = %tmp.display(), error = %rm, "could not remove temp file after a failed atomic write");
        }
    };

    if let Err(e) = std::fs::write(&tmp, content) {
        cleanup();
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        cleanup();
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write_agent(dir: &Path, name: &str, body: &str) -> PathBuf {
        let agents = dir.join(".kiro").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        let path = agents.join(format!("{name}.json"));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn persist_creates_toolssettings_and_dedups() {
        let tmp = tempfile::tempdir().unwrap();
        write_agent(
            tmp.path(),
            "myagent",
            r#"{"name":"myagent","prompt":null,"tools":["execute_bash"],"allowedTools":["fs_read"]}"#,
        );

        let written = persist_trust_grant(
            "myagent",
            tmp.path(),
            "allowedCommands",
            &["echo hello".into(), "echo( .*)?".into()],
        )
        .unwrap();

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&written).unwrap()).unwrap();
        let cmds = json["toolsSettings"]["execute_bash"]["allowedCommands"]
            .as_array()
            .unwrap();
        assert_eq!(cmds.len(), 2);
        // Sibling fields preserved.
        assert_eq!(json["allowedTools"][0], "fs_read");
        assert_eq!(json["name"], "myagent");

        // Re-running with one overlapping + one new pattern only adds the new one.
        persist_trust_grant(
            "myagent",
            tmp.path(),
            "allowedCommands",
            &["echo hello".into(), "ls( .*)?".into()],
        )
        .unwrap();
        let json2: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&written).unwrap()).unwrap();
        let cmds2 = json2["toolsSettings"]["execute_bash"]["allowedCommands"]
            .as_array()
            .unwrap();
        assert_eq!(cmds2.len(), 3, "dedup keeps echo hello, adds only ls");
    }

    #[test]
    fn persist_rejects_traversal_agent_names() {
        let tmp = tempfile::tempdir().unwrap();
        for bad in ["../evil", "..", "a/b", "/abs", "", "."] {
            let err =
                persist_trust_grant(bad, tmp.path(), "allowedCommands", &["x".into()]).unwrap_err();
            assert!(
                matches!(err, TrustPersistError::InvalidAgentName(_)),
                "expected InvalidAgentName for {bad:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn persist_errors_on_corrupt_config_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_agent(tmp.path(), "broken", "{ not json");
        let original = std::fs::read(&path).unwrap();

        let err = persist_trust_grant("broken", tmp.path(), "allowedCommands", &["echo".into()])
            .unwrap_err();
        assert!(
            matches!(err, TrustPersistError::Parse { .. }),
            "got {err:?}"
        );
        // The corrupt file must be left exactly as it was — never overwritten.
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn persist_errors_on_non_object_config_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_agent(tmp.path(), "arr", "[]");
        let original = std::fs::read(&path).unwrap();

        let err = persist_trust_grant("arr", tmp.path(), "allowedCommands", &["echo".into()])
            .unwrap_err();
        assert!(
            matches!(err, TrustPersistError::NotAnObject { .. }),
            "got {err:?}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn persist_leaves_no_temp_file_behind() {
        let tmp = tempfile::tempdir().unwrap();
        write_agent(tmp.path(), "clean", "{}");
        persist_trust_grant("clean", tmp.path(), "allowedCommands", &["echo".into()]).unwrap();

        let agents = tmp.path().join(".kiro").join("agents");
        let strays: Vec<_> = std::fs::read_dir(&agents)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".cyril.tmp"))
            .collect();
        assert!(strays.is_empty(), "temp file left behind: {strays:?}");
    }

    #[test]
    fn persist_refuses_builtin_agents() {
        let tmp = tempfile::tempdir().unwrap();
        let err = persist_trust_grant(
            "kiro_default",
            tmp.path(),
            "allowedCommands",
            &["echo hello".into()],
        )
        .unwrap_err();
        assert!(matches!(err, TrustPersistError::BuiltinAgent(_)));
    }

    #[test]
    fn persist_errors_on_unknown_setting_key() {
        let tmp = tempfile::tempdir().unwrap();
        // Unknown key is rejected before any filesystem access.
        let err =
            persist_trust_grant("myagent", tmp.path(), "mysteryKey", &["x".into()]).unwrap_err();
        assert!(matches!(err, TrustPersistError::UnknownSettingKey(_)));
    }

    #[test]
    fn resolve_prefers_workspace_over_global() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tempfile::tempdir().unwrap();
        // Global config exists...
        std::fs::write(global.path().join("dual.json"), "{}").unwrap();
        // ...and a workspace config shadows it.
        let ws = write_agent(tmp.path(), "dual", "{}");

        let resolved = resolve_in_dirs("dual", tmp.path(), Some(global.path())).unwrap();
        assert_eq!(resolved, ws, "workspace config must win over global");
    }

    #[test]
    fn resolve_falls_back_to_global_then_none() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tempfile::tempdir().unwrap();
        // Only the global config exists → resolves to it.
        std::fs::write(global.path().join("only.json"), "{}").unwrap();
        let resolved = resolve_in_dirs("only", tmp.path(), Some(global.path())).unwrap();
        assert_eq!(resolved, global.path().join("only.json"));

        // Neither workspace nor global has the agent → None (nothing to write).
        assert!(resolve_in_dirs("ghost", tmp.path(), Some(global.path())).is_none());
    }
}

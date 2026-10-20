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
//!   **fully overrides** the global `<kiro-home>/agents/<name>.json` (not a merge), so
//!   cyril writes to whichever Kiro actually reads — workspace first.

use std::collections::HashSet;
use std::ffi::OsStr;
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

/// Kiro's validated profile root, shared by agents, hooks, settings and sidecar.
pub(crate) fn kiro_home_dir() -> Option<PathBuf> {
    let home = home_dir();
    let value = std::env::var_os("KIRO_HOME");
    resolve_kiro_home(home.as_deref(), value.as_deref())
}

/// Resolve the profile root from explicit environment inputs, so a replacement
/// launch environment never falls back to the host's HOME or KIRO_HOME.
/// Mirrors KAS `P8o`/`O9u`/`A8o` (2.31.0+) and the current TUI: real paths are
/// used only for rejection checks; an accepted path retains its lexical spelling.
pub(crate) fn resolve_kiro_home(home: Option<&Path>, value: Option<&OsStr>) -> Option<PathBuf> {
    let default = || home.map(|home| home.join(".kiro"));
    let Some(value) = value.map(OsStr::to_string_lossy) else {
        return default();
    };
    // ECMAScript trim includes BOM, but not the Unicode NEL character.
    let value =
        value.trim_matches(|c: char| c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}'));
    if value.is_empty() {
        return default();
    }
    let input = Path::new(value);
    // Node's Windows isAbsolute accepts rooted /foo and \foo; drive-relative
    // C:foo is still rejected. std::path::absolute supplies the current drive.
    if !(input.is_absolute() || (cfg!(windows) && input.has_root())) {
        tracing::warn!(
            value,
            reason = "relative",
            "ignoring KIRO_HOME; using the default Kiro home"
        );
        return default();
    }
    let candidate = match lexical_absolute(input) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(value, %error, "cannot resolve KIRO_HOME; using the default Kiro home");
            return default();
        }
    };
    let normalized = nearest_existing_real_path(&candidate);
    let reason = if normalized.has_root() && normalized.parent().is_none() {
        Some("root")
    } else if let Some(home) = home {
        let home = match lexical_absolute(home) {
            Ok(path) => nearest_existing_real_path(&path),
            Err(error) => {
                tracing::warn!(value, %error, "cannot resolve HOME for KIRO_HOME validation; using the default Kiro home");
                return default();
            }
        };
        kiro_path_contains_home(&normalized, &home).then_some("contains-home")
    } else {
        None
    };
    if let Some(reason) = reason {
        tracing::warn!(
            value,
            reason,
            "ignoring KIRO_HOME; using the default Kiro home"
        );
        return default();
    }
    Some(candidate)
}

/// Node path.resolve removes dot components before realpath validation.
fn lexical_absolute(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut resolved = PathBuf::with_capacity(absolute.as_os_str().len());
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                resolved.pop();
            }
            component => resolved.push(component.as_os_str()),
        }
    }
    Ok(resolved)
}

/// Like Kiro's A8o: an absent/unreadable leaf does not prevent detecting a
/// symlink in its existing ancestors. Failure at every ancestor retains the
/// lexical path, matching upstream rather than creating or requiring a leaf.
fn nearest_existing_real_path(path: &Path) -> PathBuf {
    let mut ancestor = path;
    let mut suffix = Vec::new();
    loop {
        if let Ok(mut real) = std::fs::canonicalize(ancestor) {
            real.extend(suffix.into_iter().rev());
            return real;
        }
        match (ancestor.parent(), ancestor.file_name()) {
            (Some(parent), Some(name)) => {
                suffix.push(name);
                ancestor = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Equivalent to Node relative(candidate, home)'s empty-or-not-".." test,
/// not a textual prefix test (/home/u is not an ancestor of /home/user).
fn kiro_path_contains_home(candidate: &Path, home: &Path) -> bool {
    let mut home = home.components();
    for component in candidate.components() {
        let Some(next) = home.next() else {
            return false;
        };
        #[cfg(windows)]
        let equal = {
            let component = component.as_os_str().to_string_lossy();
            let next = next.as_os_str().to_string_lossy();
            component.eq_ignore_ascii_case(&next)
                || (!(component.is_ascii() && next.is_ascii())
                    && component.to_lowercase() == next.to_lowercase())
        };
        #[cfg(not(windows))]
        let equal = component == next;
        if !equal {
            return false;
        }
    }
    home.next()
        .is_none_or(|component| !component.as_os_str().as_encoded_bytes().starts_with(b".."))
}

/// The global agent directory follows the same validated profile as Kiro.
fn global_agents_dir() -> Option<PathBuf> {
    kiro_home_dir().map(|home| home.join("agents"))
}

/// Resolve the agent config file Kiro actually reads for `agent_name` when
/// spawned in `cwd`: the workspace file shadows the global one, so prefer
/// `<cwd>/.kiro/agents/<name>.json`, then `<kiro-home>/agents/<name>.json`.
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

    #[test]
    fn profile_defaults_are_silent_and_valid_overrides_are_trimmed() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let (_guard, capture, dispatch) = crate::test_support::capture_json_subscriber();
        tracing::dispatcher::with_default(&dispatch, || {
            for value in [
                None,
                Some(OsStr::new("")),
                Some(OsStr::new(" \t\u{2003}\u{feff} ")),
            ] {
                assert_eq!(
                    resolve_kiro_home(Some(&home), value),
                    Some(home.join(".kiro"))
                );
                assert_eq!(resolve_kiro_home(None, value), None);
            }
            let expected = root.path().join("profile ü");
            let input = format!(" \t\u{feff}{} \u{2003}", expected.display());
            for home in [Some(home.as_path()), None] {
                assert_eq!(
                    resolve_kiro_home(home, Some(OsStr::new(&input))),
                    Some(expected.clone())
                );
            }
            assert!(!expected.exists(), "resolving must not create a profile");
        });
        assert!(
            capture
                .captured()
                .iter()
                .all(|event| event["level"] != "WARN"),
            "default and accepted roots must not warn"
        );
    }

    #[test]
    fn profile_rejects_relative_root_and_home_ancestors_with_reasons() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let filesystem_root = root.path().ancestors().last().unwrap().to_path_buf();
        let values = [
            (PathBuf::from("relative/profile"), "relative"),
            (PathBuf::from("."), "relative"),
            (filesystem_root, "root"),
            (home.clone(), "contains-home"),
            (root.path().to_path_buf(), "contains-home"),
            (home.join("child/.."), "contains-home"),
        ];
        let (_guard, capture, dispatch) = crate::test_support::capture_json_subscriber();
        for (value, reason) in values {
            let before = capture.captured().len();
            assert_eq!(
                tracing::dispatcher::with_default(&dispatch, || {
                    resolve_kiro_home(Some(&home), Some(value.as_os_str()))
                }),
                Some(home.join(".kiro")),
                "value={value:?}"
            );
            let events = capture.captured();
            let warnings: Vec<_> = events[before..]
                .iter()
                .filter(|event| event["level"] == "WARN")
                .collect();
            assert_eq!(warnings.len(), 1, "value={value:?}: {events:?}");
            assert_eq!(warnings[0]["fields"]["reason"], reason);
            assert_eq!(
                warnings[0]["fields"]["value"],
                value.to_string_lossy().as_ref()
            );
        }
        assert_eq!(resolve_kiro_home(None, Some(OsStr::new("relative"))), None);
    }

    #[test]
    fn profile_normalizes_lexically_without_using_textual_home_prefixes() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("username");
        for candidate in [
            root.path().join("user"),
            home.join(".kiro"),
            root.path().join("profiles/unused/../selected"),
        ] {
            let expected = if candidate.ends_with("selected") {
                root.path().join("profiles/selected")
            } else {
                candidate.clone()
            };
            assert_eq!(
                resolve_kiro_home(Some(&home), Some(candidate.as_os_str())),
                Some(expected)
            );
        }
        // Preserve Kiro's precise relative-path predicate: "..hidden" is not
        // considered a contained relative path because it starts with "..".
        assert!(!kiro_path_contains_home(
            root.path(),
            &root.path().join("..hidden")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn profile_validation_rejects_symlink_aliases_but_retains_accepted_spelling() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("real/home");
        std::fs::create_dir_all(&home).unwrap();
        let home_alias = root.path().join("home-alias");
        let parent_alias = root.path().join("parent-alias");
        let root_alias = root.path().join("root-alias");
        symlink(&home, &home_alias).unwrap();
        symlink(root.path().join("real"), &parent_alias).unwrap();
        symlink("/", &root_alias).unwrap();
        for candidate in [&home_alias, &parent_alias, &root_alias] {
            assert_eq!(
                resolve_kiro_home(Some(&home), Some(candidate.as_os_str())),
                Some(home.join(".kiro")),
                "alias={candidate:?}"
            );
        }
        assert_eq!(
            resolve_kiro_home(Some(&home_alias), Some(home.as_os_str())),
            Some(home_alias.join(".kiro")),
            "HOME itself must also be normalized through its symlink"
        );
        let absent_home = root.path().join("real/not-created/user");
        let absent_alias = parent_alias.join("not-created");
        assert_eq!(
            resolve_kiro_home(Some(&absent_home), Some(absent_alias.as_os_str())),
            Some(absent_home.join(".kiro")),
            "existing symlink ancestors must be resolved even with an absent suffix"
        );
        let real_profile = root.path().join("real/profile");
        std::fs::create_dir_all(&real_profile).unwrap();
        let profile_alias = root.path().join("profile-alias");
        symlink(&real_profile, &profile_alias).unwrap();
        assert_eq!(
            resolve_kiro_home(Some(&home), Some(profile_alias.as_os_str())),
            Some(profile_alias),
            "accepted roots retain the user's lexical spelling, not realpath"
        );
    }

    #[cfg(windows)]
    #[test]
    fn profile_windows_native_rooted_case_and_unc_boundaries() {
        assert_eq!(
            resolve_kiro_home(None, Some(OsStr::new(r"C:relative"))),
            None
        );
        for value in [
            r"\cyril-d7ve-owned-nonexistent",
            "/cyril-d7ve-owned-nonexistent",
        ] {
            let path = resolve_kiro_home(None, Some(OsStr::new(value))).unwrap();
            assert!(
                path.is_absolute(),
                "root-relative paths acquire the current drive"
            );
            assert!(path.ends_with("cyril-d7ve-owned-nonexistent"));
        }
        for (candidate, home, expected) in [
            (r"C:\Users\PROFILE", r"c:\users\profile\child", true),
            (r"C:\Users\Ü", r"c:\users\ü\child", true),
            (r"C:\Users", r"D:\Users\child", false),
            (
                r"\\SERVER\Share\Profiles",
                r"\\server\share\profiles\User",
                true,
            ),
            (
                r"\\server\share\Profiles",
                r"\\server\other\Profiles\User",
                false,
            ),
        ] {
            assert_eq!(
                kiro_path_contains_home(Path::new(candidate), Path::new(home)),
                expected
            );
        }
        // Lexical classification only: never contact a real UNC server.
        for root in [r"C:\", r"\\server\share\"] {
            let root = lexical_absolute(Path::new(root)).unwrap();
            assert!(root.has_root() && root.parent().is_none());
        }
    }

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

//! Agent↔native path translation across the Windows/WSL boundary.
//!
//! Two path families translate on Windows (no-ops on Linux):
//!
//! - **Drive mounts:** `/mnt/c/...` ↔ `C:\...`, unconditional.
//! - **WSL-internal paths** (`/home/...`, `/tmp/...`, non-drive `/mnt`
//!   entries): `/...` ↔ `\\wsl$\<distro>\...`, only when the process WSL
//!   distro is known — [`resolve_wsl_distro`] runs once per process
//!   (`CYRIL_WSL_DISTRO` env, else a `\\wsl$`-rooted cwd). Unknown distro
//!   means passthrough: the pre-cyril-8tq6 behavior, failing downstream as an
//!   honest NotFound rather than guessing a distro.
//!
//! UNC semantics follow Microsoft's own `wslpath` conformance tests
//! (microsoft/WSL `test/linux/unit_tests/wslpath.c`): both `\\wsl$\` and
//! `\\wsl.localhost\` accepted inbound, `\\wsl$` emitted, exact
//! distro-segment matching. Probe evidence: `.cyril-8tq6/`.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Translate an agent-provided path to the native filesystem path.
/// On Windows (WSL bridge), converts `/mnt/c/...` to `C:\...`.
/// On Linux (direct), returns the path unchanged.
pub fn to_native(path: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        wsl_to_win(&path.to_string_lossy())
    } else {
        path.to_path_buf()
    }
}

/// Translate a native filesystem path to an agent-compatible path.
/// On Windows (WSL bridge), converts `C:\...` to `/mnt/c/...`.
/// On Linux (direct), returns the path unchanged.
pub fn to_agent(path: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        win_to_wsl(path)
    } else {
        path.to_path_buf()
    }
}

/// Direction of path translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    WinToWsl,
    WslToWin,
}

/// Convert a Windows path to a WSL path.
///
/// `C:\Users\foo\bar` becomes `/mnt/c/Users/foo/bar`
/// `D:\project` becomes `/mnt/d/project`
/// `\\?\C:\Users\foo` becomes `/mnt/c/Users/foo` (extended-length prefix stripped)
/// `\\wsl$\<distro>\...` becomes `/...` when `<distro>` matches the process
/// WSL distro (see [`resolve_wsl_distro`]).
pub fn win_to_wsl(path: &Path) -> PathBuf {
    win_to_wsl_in(path, process_wsl_distro())
}

/// The pre-UNC `win_to_wsl` behavior: drive letters, `\\?\` stripping, and the
/// legacy backslash→slash conversion for everything else. Fallback for
/// [`win_to_wsl_in`] on non-WSL inputs — never called for WSL UNC paths.
fn legacy_win_to_wsl(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    // Strip the \\?\ extended-length path prefix that canonicalize() produces on Windows.
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    // Handle drive letter paths like C:\ or C:/
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let drive = s.as_bytes()[0].to_ascii_lowercase() as char;
        let rest = &s[2..];
        let rest = rest.replace('\\', "/");
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() {
            PathBuf::from(format!("/mnt/{drive}"))
        } else {
            PathBuf::from(format!("/mnt/{drive}/{rest}"))
        }
    } else {
        // Already a unix-style path or relative — return as-is with forward slashes
        PathBuf::from(s.replace('\\', "/"))
    }
}

/// Convert a WSL path to a Windows path.
///
/// `/mnt/c/Users/foo/bar` becomes `C:\Users\foo\bar`
/// `/mnt/d/project` becomes `D:\project`
/// `/home/...` (any WSL-internal path) becomes `\\wsl$\<distro>\...` when the
/// process WSL distro is known (see [`resolve_wsl_distro`]); unchanged otherwise.
pub fn wsl_to_win(path: &str) -> PathBuf {
    let distro = process_wsl_distro();
    if cfg!(target_os = "windows")
        && distro.is_none()
        && path.starts_with('/')
        && drive_mount_to_win(path).is_none()
    {
        // Once per process, not per callback: the untranslatable-path condition
        // repeats for every fs/* call of a session.
        static WARN_ONCE: std::sync::Once = std::sync::Once::new();
        WARN_ONCE.call_once(|| {
            tracing::warn!(
                path = %path,
                "WSL-internal path cannot be translated: no WSL distro configured \
                 (set CYRIL_WSL_DISTRO or launch cyril from a \\\\wsl$ workspace)"
            );
        });
    }
    wsl_to_win_in(path, distro)
}

/// The WSL distro used by [`wsl_to_win`] / [`win_to_wsl`], resolved once per
/// process from `CYRIL_WSL_DISTRO` and the process cwd. Always `None` off
/// Windows — Linux translation stays a no-op (load-bearing: enforced by the
/// `cfg!` below and fenced by `tests/win_wsl_wiring.rs`).
fn process_wsl_distro() -> Option<&'static str> {
    static DISTRO: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    DISTRO
        .get_or_init(|| {
            if !cfg!(target_os = "windows") {
                return None;
            }
            let env = std::env::var("CYRIL_WSL_DISTRO").ok();
            let cwd = std::env::current_dir().ok();
            resolve_wsl_distro(env.as_deref(), cwd.as_deref())
        })
        .as_deref()
}

/// Convert a WSL path to a Windows path, translating WSL-internal paths to
/// `\\wsl$\<distro>\...` UNC form when a distro is known.
///
/// Drive mounts keep the existing translation (`/mnt/c/...` → `C:\...`); any
/// other `/`-rooted path — `/home/...`, `/tmp/...`, and non-drive `/mnt`
/// entries like `/mnt/data` — becomes `\\wsl$\<distro>\<path>` with `/`
/// converted to `\` (root maps to a trailing `\`, matching `wslpath`).
/// With no distro (`None`, or defensively `Some("")`) every non-drive path is
/// returned unchanged — the pre-UNC behavior.
pub fn wsl_to_win_in(path: &str, distro: Option<&str>) -> PathBuf {
    if let Some(win) = drive_mount_to_win(path) {
        return win;
    }
    let distro = match distro {
        // Load-bearing guard: an empty distro would silently emit
        // `\\wsl$\<nothing>\...`, a path no host can open.
        Some("") => {
            tracing::debug!("WSL distro is an empty string; treating as unset");
            None
        }
        d => d,
    };
    if let Some(d) = distro
        && path.starts_with('/')
    {
        return PathBuf::from(format!(r"\\wsl$\{d}{}", path.replace('/', "\\")));
    }
    PathBuf::from(path)
}

/// Convert a Windows path to a WSL path, additionally mapping WSL UNC paths
/// (`\\wsl$\<distro>\...` / `\\wsl.localhost\<distro>\...`) back to their
/// POSIX form when the distro segment exactly matches `distro`.
///
/// The distro segment must match EXACTLY — `\\wsl$\Ubuntu-other\foo` does NOT
/// match distro `Ubuntu` (Microsoft's wslpath treats prefix-colliding names as
/// errors). A WSL UNC path that doesn't match (foreign distro, blank segment,
/// or no distro configured) is returned UNCHANGED — never handed to the legacy
/// generic-UNC branch, whose slash-flip would corrupt it. Non-WSL inputs fall
/// back to [`legacy_win_to_wsl`] (drive letters, `\\?\`, generic).
pub fn win_to_wsl_in(path: &Path, distro: Option<&str>) -> PathBuf {
    let s = path.to_string_lossy();
    if is_wsl_unc(&s) {
        let distro = match distro {
            // Same load-bearing guard as `wsl_to_win_in`: an empty distro must
            // not match anything (and can't — segments are non-empty — but be
            // explicit).
            Some("") => {
                tracing::debug!("WSL distro is an empty string; treating as unset");
                None
            }
            d => d,
        };
        match distro {
            Some(d) => {
                if let Some(posix) = wsl_unc_tail(&s, d) {
                    return posix;
                }
                tracing::warn!(
                    path = %s,
                    distro = %d,
                    "WSL UNC path names a different distro; passing through untranslated"
                );
            }
            None => {
                tracing::debug!(path = %s, "WSL UNC path seen with no distro configured");
            }
        }
        return path.to_path_buf();
    }
    legacy_win_to_wsl(path)
}

/// `true` when `s` starts with a WSL UNC prefix (`\\wsl$\` or
/// `\\wsl.localhost\`), whatever follows.
fn is_wsl_unc(s: &str) -> bool {
    WSL_UNC_PREFIXES.iter().any(|p| s.starts_with(p))
}

const WSL_UNC_PREFIXES: [&str; 2] = [r"\\wsl.localhost\", r"\\wsl$\"];

/// `Some(<posix path>)` when `s` is a `\\wsl$\` / `\\wsl.localhost\` UNC path
/// whose distro segment equals `distro` exactly; `None` otherwise (foreign
/// distro, blank segment, or not a WSL UNC path). The bare root forms
/// (`\\wsl$\<d>` and `\\wsl$\<d>\`) map to `/`, matching wslpath.
fn wsl_unc_tail(s: &str, distro: &str) -> Option<PathBuf> {
    let rest = WSL_UNC_PREFIXES.iter().find_map(|p| s.strip_prefix(p))?;
    let rest = rest.replace('\\', "/");
    let (seg, tail) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest.as_str(), ""),
    };
    if seg != distro {
        return None;
    }
    Some(PathBuf::from(if tail.is_empty() { "/" } else { tail }))
}

/// Resolve the WSL distro name used for `\\wsl$` UNC translation.
///
/// Precedence: a non-empty `env` value (the `CYRIL_WSL_DISTRO` variable) wins;
/// otherwise a `cwd` sitting under a WSL UNC prefix donates its distro segment
/// (cyril launched *from* the WSL-native workspace); otherwise `None`.
///
/// An empty env value is treated as unset. The value is otherwise taken
/// literally — no trimming — because a wrong name degrades to passthrough,
/// the same failure mode as unset. The returned name is never empty.
pub fn resolve_wsl_distro(env: Option<&str>, cwd: Option<&Path>) -> Option<String> {
    if let Some(e) = env
        && !e.is_empty()
    {
        return Some(e.to_string());
    }
    let cwd = cwd?.to_string_lossy();
    let rest = WSL_UNC_PREFIXES.iter().find_map(|p| cwd.strip_prefix(p))?;
    let rest = rest.replace('\\', "/");
    let seg = rest.split('/').next().unwrap_or("");
    if seg.is_empty() {
        tracing::debug!(cwd = %cwd, "WSL UNC cwd has a blank distro segment; no distro resolved");
        None
    } else {
        Some(seg.to_string())
    }
}

/// The `/mnt/<drive-letter>` translation shared by [`wsl_to_win`] and
/// [`wsl_to_win_in`]. `None` when `path` is not a single-letter drive mount
/// (including WSL-internal `/mnt` entries like `/mnt/data` or bare `/mnt`).
fn drive_mount_to_win(path: &str) -> Option<PathBuf> {
    if let Some(rest) = path.strip_prefix("/mnt/")
        && !rest.is_empty()
    {
        let drive = rest.as_bytes()[0].to_ascii_uppercase() as char;
        let after_drive = &rest[1..];
        if after_drive.is_empty() || after_drive.starts_with('/') {
            let suffix = after_drive.strip_prefix('/').unwrap_or("");
            let win_path = if suffix.is_empty() {
                format!("{drive}:\\")
            } else {
                format!("{drive}:\\{}", suffix.replace('/', "\\"))
            };
            return Some(PathBuf::from(win_path));
        }
    }
    None
}

/// Recursively translate paths in a JSON value.
/// Looks for string values that look like paths and translates them, using
/// the process WSL distro for `\\wsl$` UNC forms.
pub fn translate_paths_in_json(value: &mut Value, direction: Direction) {
    translate_paths_in_json_in(value, direction, process_wsl_distro());
}

/// As [`translate_paths_in_json`], with an explicit distro (testable off-Windows).
///
/// `WinToWsl` recognizes drive-letter strings AND WSL UNC strings (a string
/// starting `\\wsl$\` is unambiguously a path). `WslToWin` deliberately stays
/// drive-mount-only: a bare `/`-rooted JSON string can be file CONTENT
/// (`"content": "/etc/hosts is..."`), and blind translation would corrupt
/// writes — WSL-internal paths are translated only at the typed fs boundary
/// (`to_native`), never heuristically.
pub fn translate_paths_in_json_in(value: &mut Value, direction: Direction, distro: Option<&str>) {
    match value {
        Value::String(s) => {
            let translated = match direction {
                Direction::WinToWsl => {
                    if looks_like_windows_path(s) || is_wsl_unc(s) {
                        win_to_wsl_in(Path::new(s.as_str()), distro)
                            .to_string_lossy()
                            .into_owned()
                    } else {
                        return;
                    }
                }
                Direction::WslToWin => {
                    if looks_like_wsl_mount_path(s) {
                        wsl_to_win_in(s, distro).to_string_lossy().into_owned()
                    } else {
                        return;
                    }
                }
            };
            *s = translated;
        }
        Value::Array(arr) => {
            for item in arr {
                translate_paths_in_json_in(item, direction, distro);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                translate_paths_in_json_in(v, direction, distro);
            }
        }
        _ => {}
    }
}

fn looks_like_windows_path(s: &str) -> bool {
    // Strip \\?\ extended-length prefix so the drive-letter check below still works.
    let s = s.strip_prefix(r"\\?\").unwrap_or(s);
    s.len() >= 3
        && s.as_bytes()[0].is_ascii_alphabetic()
        && s.as_bytes()[1] == b':'
        && (s.as_bytes()[2] == b'\\' || s.as_bytes()[2] == b'/')
}

fn looks_like_wsl_mount_path(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix("/mnt/") {
        !rest.is_empty() && rest.as_bytes()[0].is_ascii_alphabetic()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_win_to_wsl_c_drive() {
        assert_eq!(
            win_to_wsl(Path::new(r"C:\Users\foo\bar")),
            PathBuf::from("/mnt/c/Users/foo/bar")
        );
    }

    #[test]
    fn test_win_to_wsl_d_drive() {
        assert_eq!(
            win_to_wsl(Path::new(r"D:\project\src")),
            PathBuf::from("/mnt/d/project/src")
        );
    }

    #[test]
    fn test_win_to_wsl_root() {
        assert_eq!(win_to_wsl(Path::new(r"C:\")), PathBuf::from("/mnt/c"));
    }

    #[test]
    fn test_win_to_wsl_forward_slashes() {
        assert_eq!(
            win_to_wsl(Path::new("C:/Users/foo")),
            PathBuf::from("/mnt/c/Users/foo")
        );
    }

    #[test]
    fn test_wsl_to_win_basic() {
        assert_eq!(
            wsl_to_win("/mnt/c/Users/foo/bar"),
            PathBuf::from(r"C:\Users\foo\bar")
        );
    }

    #[test]
    fn test_wsl_to_win_d_drive() {
        assert_eq!(wsl_to_win("/mnt/d/project"), PathBuf::from(r"D:\project"));
    }

    #[test]
    fn test_wsl_to_win_root() {
        assert_eq!(wsl_to_win("/mnt/c"), PathBuf::from(r"C:\"));
    }

    #[test]
    fn test_wsl_to_win_non_mount_path() {
        assert_eq!(
            wsl_to_win("/home/user/.config"),
            PathBuf::from("/home/user/.config")
        );
    }

    #[test]
    fn test_roundtrip_win_wsl_win() {
        let original = r"C:\Users\dwall\repos\project\src\main.rs";
        let wsl = win_to_wsl(Path::new(original));
        let back = wsl_to_win(&wsl.to_string_lossy());
        assert_eq!(back, PathBuf::from(original));
    }

    #[test]
    fn test_translate_json_wsl_to_win() {
        let mut val = serde_json::json!({
            "path": "/mnt/c/Users/foo/file.txt",
            "content": "hello world",
            "nested": {
                "file": "/mnt/d/project/src/main.rs"
            }
        });
        translate_paths_in_json(&mut val, Direction::WslToWin);
        assert_eq!(val["path"], r"C:\Users\foo\file.txt");
        assert_eq!(val["content"], "hello world");
        assert_eq!(val["nested"]["file"], r"D:\project\src\main.rs");
    }

    #[test]
    fn test_translate_json_win_to_wsl() {
        let mut val = serde_json::json!({
            "path": r"C:\Users\foo\file.txt",
            "count": 42
        });
        translate_paths_in_json(&mut val, Direction::WinToWsl);
        assert_eq!(val["path"], "/mnt/c/Users/foo/file.txt");
        assert_eq!(val["count"], 42);
    }

    // ── \\?\ extended-length prefix tests ──

    #[test]
    fn test_win_to_wsl_strips_extended_prefix() {
        assert_eq!(
            win_to_wsl(Path::new(r"\\?\C:\Users\foo\bar")),
            PathBuf::from("/mnt/c/Users/foo/bar")
        );
    }

    #[test]
    fn test_win_to_wsl_strips_extended_prefix_d_drive() {
        assert_eq!(
            win_to_wsl(Path::new(r"\\?\D:\project\src")),
            PathBuf::from("/mnt/d/project/src")
        );
    }

    #[test]
    fn test_win_to_wsl_extended_prefix_root() {
        assert_eq!(win_to_wsl(Path::new(r"\\?\C:\")), PathBuf::from("/mnt/c"));
    }

    #[test]
    fn test_roundtrip_extended_prefix() {
        let original = r"\\?\C:\Users\dwall\repos\project\src\main.rs";
        let wsl = win_to_wsl(Path::new(original));
        assert_eq!(
            wsl,
            PathBuf::from("/mnt/c/Users/dwall/repos/project/src/main.rs")
        );
        let back = wsl_to_win(&wsl.to_string_lossy());
        // Roundtrip produces the canonical form without \\?\ prefix
        assert_eq!(
            back,
            PathBuf::from(r"C:\Users\dwall\repos\project\src\main.rs")
        );
    }

    #[test]
    fn test_translate_json_extended_prefix() {
        let mut val = serde_json::json!({
            "path": r"\\?\C:\Users\foo\file.txt",
            "normal": r"D:\project\src\main.rs"
        });
        translate_paths_in_json(&mut val, Direction::WinToWsl);
        assert_eq!(val["path"], "/mnt/c/Users/foo/file.txt");
        assert_eq!(val["normal"], "/mnt/d/project/src/main.rs");
    }

    // ── wsl_to_win_in: WSL-internal → \\wsl$ UNC (cyril-8tq6, claims C2/C5) ──
    // Expected values follow Microsoft's own wslpath conformance tests
    // (microsoft/WSL test/linux/unit_tests/wslpath.c), \\wsl$ emission.

    #[test]
    fn wsl_internal_to_unc_home_tmp_root() {
        let d = Some("Ubuntu");
        assert_eq!(
            wsl_to_win_in("/home/u/f.txt", d),
            PathBuf::from(r"\\wsl$\Ubuntu\home\u\f.txt")
        );
        assert_eq!(
            wsl_to_win_in("/tmp/x", d),
            PathBuf::from(r"\\wsl$\Ubuntu\tmp\x")
        );
        assert_eq!(
            wsl_to_win_in("/root", d),
            PathBuf::from(r"\\wsl$\Ubuntu\root")
        );
    }

    #[test]
    fn wsl_internal_to_unc_root_and_trailing_separator() {
        let d = Some("Ubuntu");
        assert_eq!(wsl_to_win_in("/", d), PathBuf::from(r"\\wsl$\Ubuntu\"));
        assert_eq!(
            wsl_to_win_in("/proc/1/", d),
            PathBuf::from(r"\\wsl$\Ubuntu\proc\1\")
        );
    }

    #[test]
    fn wsl_internal_to_unc_non_drive_mnt_entries() {
        let d = Some("Ubuntu");
        // Multi-char /mnt entries are WSL-internal, NOT drive "d"/"m".
        assert_eq!(
            wsl_to_win_in("/mnt/data/x", d),
            PathBuf::from(r"\\wsl$\Ubuntu\mnt\data\x")
        );
        assert_eq!(
            wsl_to_win_in("/mnt", d),
            PathBuf::from(r"\\wsl$\Ubuntu\mnt")
        );
        assert_eq!(
            wsl_to_win_in("/mnt/", d),
            PathBuf::from(r"\\wsl$\Ubuntu\mnt\")
        );
    }

    #[test]
    fn wsl_internal_to_unc_drive_branch_still_wins() {
        assert_eq!(
            wsl_to_win_in("/mnt/c/Users", Some("Ubuntu")),
            PathBuf::from(r"C:\Users")
        );
    }

    #[test]
    fn wsl_internal_to_unc_unicode_and_spaces() {
        assert_eq!(
            wsl_to_win_in("/home/ü ser/f x.txt", Some("Ubuntu")),
            PathBuf::from(r"\\wsl$\Ubuntu\home\ü ser\f x.txt")
        );
    }

    #[test]
    fn wsl_internal_to_unc_relative_and_empty_unchanged() {
        let d = Some("Ubuntu");
        assert_eq!(wsl_to_win_in("rel/path", d), PathBuf::from("rel/path"));
        assert_eq!(wsl_to_win_in("", d), PathBuf::from(""));
    }

    #[test]
    fn no_distro_is_passthrough_forward() {
        // Distro None (and defensively Some("")) preserves today's behavior —
        // the 4 distinct WSL-internal paths from the real 2.10.0 KAS capture.
        for p in [
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m",
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/README.md",
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/scratch.txt",
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/summary.txt",
        ] {
            assert_eq!(wsl_to_win_in(p, None), PathBuf::from(p));
            assert_eq!(wsl_to_win_in(p, Some("")), PathBuf::from(p));
        }
        // Drive translation is distro-independent.
        assert_eq!(wsl_to_win_in("/mnt/c/x", None), PathBuf::from(r"C:\x"));
    }

    // ── win_to_wsl_in: \\wsl$ UNC → POSIX (cyril-8tq6, claims C3/C4/C5) ──

    #[test]
    fn unc_to_wsl_both_prefixes_and_slash_kinds() {
        let d = Some("Ubuntu");
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\wsl$\Ubuntu\home\u"), d),
            PathBuf::from("/home/u")
        );
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\wsl.localhost\Ubuntu\proc\stat"), d),
            PathBuf::from("/proc/stat")
        );
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\wsl$\Ubuntu/proc/stat"), d),
            PathBuf::from("/proc/stat")
        );
        // Mixed separators in the tail normalize too.
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\wsl$\Ubuntu\proc/stat"), d),
            PathBuf::from("/proc/stat")
        );
    }

    #[test]
    fn unc_to_wsl_root_forms() {
        let d = Some("Ubuntu");
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\wsl$\Ubuntu"), d),
            PathBuf::from("/")
        );
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\wsl$\Ubuntu\"), d),
            PathBuf::from("/")
        );
    }

    #[test]
    fn foreign_distro_passthrough() {
        // Exact-segment guard: prefix-colliding distro names must NOT match
        // (MS wslpath conformance), nor a blank segment.
        let d = Some("Ubuntu");
        for p in [
            r"\\wsl$\Ubuntu-other\foo",
            r"\\wsl$\UbuntuX\foo",
            r"\\wsl$\",
        ] {
            assert_eq!(win_to_wsl_in(Path::new(p), d), PathBuf::from(p));
        }
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\wsl.localhost\Ubuntu-other\foo"), d),
            PathBuf::from(r"\\wsl.localhost\Ubuntu-other\foo")
        );
    }

    #[test]
    fn no_distro_is_passthrough_reverse() {
        for p in [r"\\wsl$\Ubuntu\home\u", r"\\wsl.localhost\Ubuntu\x"] {
            assert_eq!(win_to_wsl_in(Path::new(p), None), PathBuf::from(p));
            assert_eq!(win_to_wsl_in(Path::new(p), Some("")), PathBuf::from(p));
        }
    }

    #[test]
    fn unc_to_wsl_legacy_branches_untouched() {
        let d = Some("Ubuntu");
        // Generic UNC keeps the pre-existing (legacy) forward-slash behavior.
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\server\share\f"), d),
            PathBuf::from("//server/share/f")
        );
        // Drive letters and \\?\ still translate as before.
        assert_eq!(
            win_to_wsl_in(Path::new(r"C:\Users\u"), d),
            PathBuf::from("/mnt/c/Users/u")
        );
        assert_eq!(
            win_to_wsl_in(Path::new(r"\\?\C:\Users\u"), d),
            PathBuf::from("/mnt/c/Users/u")
        );
    }

    #[test]
    fn roundtrip_wsl_internal_capture_paths() {
        // POSIX → UNC → POSIX over the 4 distinct paths from the real 2.10.0
        // KAS capture (plus wslpath conformance shapes), claim C4.
        let d = Some("Ubuntu");
        for p in [
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m",
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/README.md",
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/scratch.txt",
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/summary.txt",
            "/",
            "/proc/1/",
        ] {
            let unc = wsl_to_win_in(p, d);
            let back = win_to_wsl_in(&unc, d);
            assert_eq!(back, PathBuf::from(p), "round-trip failed for {p}");
        }
        // UNC → POSIX → UNC lands on the canonical \\wsl$ emission, from
        // either accepted prefix.
        for (unc, canonical) in [
            (r"\\wsl$\Ubuntu\home\u", r"\\wsl$\Ubuntu\home\u"),
            (r"\\wsl.localhost\Ubuntu\home\u", r"\\wsl$\Ubuntu\home\u"),
        ] {
            let posix = win_to_wsl_in(Path::new(unc), d);
            let again = wsl_to_win_in(&posix.to_string_lossy(), d);
            assert_eq!(again, PathBuf::from(canonical));
        }
    }

    // ── resolve_wsl_distro (cyril-8tq6, claim C6) ──

    #[test]
    fn resolve_distro_env_wins_over_cwd() {
        assert_eq!(
            resolve_wsl_distro(Some("Ubuntu"), None),
            Some("Ubuntu".into())
        );
        assert_eq!(
            resolve_wsl_distro(Some("Ubuntu"), Some(Path::new(r"\\wsl$\Debian\home\u"))),
            Some("Ubuntu".into())
        );
    }

    #[test]
    fn resolve_distro_cwd_derivation_both_prefixes() {
        assert_eq!(
            resolve_wsl_distro(None, Some(Path::new(r"\\wsl$\Debian\home\u"))),
            Some("Debian".into())
        );
        assert_eq!(
            resolve_wsl_distro(None, Some(Path::new(r"\\wsl.localhost\Debian\home\u"))),
            Some("Debian".into())
        );
        // Root cwd with no tail, and a forward-slash tail.
        assert_eq!(
            resolve_wsl_distro(None, Some(Path::new(r"\\wsl$\Ubuntu"))),
            Some("Ubuntu".into())
        );
        assert_eq!(
            resolve_wsl_distro(None, Some(Path::new(r"\\wsl$\Ubuntu/sub"))),
            Some("Ubuntu".into())
        );
    }

    #[test]
    fn resolve_distro_none_paths() {
        assert_eq!(resolve_wsl_distro(None, None), None);
        // Empty env is unset; a drive cwd derives nothing.
        assert_eq!(
            resolve_wsl_distro(Some(""), Some(Path::new(r"C:\Users\u"))),
            None
        );
        // Blank distro segment resolves nothing.
        assert_eq!(resolve_wsl_distro(None, Some(Path::new(r"\\wsl$\"))), None);
    }

    #[test]
    fn resolve_distro_env_taken_literally() {
        // No trimming: a wrong name degrades to passthrough, same as unset.
        assert_eq!(
            resolve_wsl_distro(Some(" Ubuntu "), None),
            Some(" Ubuntu ".into())
        );
    }

    // ── JSON translation with WSL UNC awareness (cyril-8tq6, claim C7) ──

    #[test]
    fn json_win_to_wsl_unc() {
        let mut val = serde_json::json!({
            "path": r"\\wsl$\Ubuntu\home\u",
            "alt": r"\\wsl.localhost\Ubuntu\x",
            "drive": r"C:\Users\u",
            "keep": r"\\server\share",
            "foreign": r"\\wsl$\Debian\y",
            "nested": [{ "p": r"\\wsl$\Ubuntu\tmp\f" }]
        });
        translate_paths_in_json_in(&mut val, Direction::WinToWsl, Some("Ubuntu"));
        assert_eq!(val["path"], "/home/u");
        assert_eq!(val["alt"], "/x");
        assert_eq!(val["drive"], "/mnt/c/Users/u");
        // Generic UNC is not path-shaped enough to translate — unchanged.
        assert_eq!(val["keep"], r"\\server\share");
        // Foreign distro passes through untranslated (exact-segment guard).
        assert_eq!(val["foreign"], r"\\wsl$\Debian\y");
        assert_eq!(val["nested"][0]["p"], "/tmp/f");
    }

    #[test]
    fn json_wsl_to_win_ignores_bare_posix() {
        // Content safety: even with a distro configured, a bare /-rooted JSON
        // string may be file CONTENT and must never be translated. Only
        // /mnt/<drive> strings translate in the WslToWin direction.
        let mut val = serde_json::json!({
            "path": "/mnt/c/f",
            "content": "/etc/hosts is a file\n",
            "posix": "/home/u"
        });
        translate_paths_in_json_in(&mut val, Direction::WslToWin, Some("Ubuntu"));
        assert_eq!(val["path"], r"C:\f");
        assert_eq!(val["content"], "/etc/hosts is a file\n");
        assert_eq!(val["posix"], "/home/u");
    }

    #[test]
    fn test_unc_path_not_mangled() {
        // UNC paths (\\server\share) should pass through without prefix stripping
        let result = win_to_wsl(Path::new(r"\\server\share\file.txt"));
        assert_eq!(result, PathBuf::from("//server/share/file.txt"));
    }

    #[test]
    fn test_translate_json_unc_path_not_translated() {
        let mut val = serde_json::json!({
            "path": r"\\?\UNC\server\share\file.txt"
        });
        translate_paths_in_json(&mut val, Direction::WinToWsl);
        // \\?\UNC\... after prefix stripping becomes UNC\server\share\file.txt
        // which doesn't match drive-letter pattern, so it should not be translated
        assert_eq!(val["path"], r"\\?\UNC\server\share\file.txt");
    }
}

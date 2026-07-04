//! Free-path KAS spawn discovery (KAS-1 Part A, cyril-evwh).
//!
//! Resolves the argv that spawns the bundled `@kiro/agent` ACP server directly
//! over stdio with **no `--auth` flag** — KAS then uses its own tier-5 file-auth
//! (reads `~/.aws/sso/cache/kiro-auth-token.json`, self-refreshing), so cyril
//! needs zero credential code on this path (prove-it-prototype verified the turn
//! completes with `_kiro/auth/getAccessToken` fired 0×). Reuses [`AgentCommand`]
//! as the spawn description so `AgentProcess::spawn` consumes it unchanged.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::types::AgentCommand;

/// A precondition for the free-path spawn that was not satisfied. Each variant
/// names the specific missing item so the bridge can emit an actionable
/// `BridgeDisconnected` (spec B6) instead of a silent hang or a v2 fallback.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KasMissing {
    /// Neither `HOME` nor `USERPROFILE` is set and no path override was given.
    NoHome,
    /// The KAS server bundle (`acp-server.js`) is not a file at the resolved path.
    Server(PathBuf),
    /// No `node` runtime (`KIRO_AGENT_PATH` unset/missing and none on `PATH`).
    Node,
    /// The auth token file is absent — the user has not run `kiro-cli login`.
    NotLoggedIn(PathBuf),
}

impl KasMissing {
    /// A user-facing, actionable reason for the `BridgeDisconnected` (spec B6).
    pub(crate) fn reason(&self) -> String {
        match self {
            KasMissing::NoHome => "cannot locate the KAS bundle: no home directory (HOME unset). \
                 Set KIRO_KAS_SERVER_PATH to the acp-server.js path."
                .to_string(),
            KasMissing::Server(p) => format!(
                "KAS bundle not found at {}. Run `kiro-cli acp --agent-engine v3` \
                 once to self-extract it, or set KIRO_KAS_SERVER_PATH.",
                p.display()
            ),
            KasMissing::Node => "node runtime not found. Install Node.js (on PATH) or set \
                 KIRO_AGENT_PATH to the node binary."
                .to_string(),
            KasMissing::NotLoggedIn(p) => format!(
                "not authenticated for KAS: token file {} is absent. Run `kiro-cli login`.",
                p.display()
            ),
        }
    }
}

/// `<home>`-relative path of the KAS self-extraction root. kiro ≥2.10.0
/// extracts into versioned `<semver>-<sha256>/` dirs under it; older releases
/// extracted the bundle directly at the root (the legacy layout).
const KAS_ROOT_REL: &str = ".local/share/kiro-cli/kas";
/// Path of the ACP server entry inside one extraction (versioned dir or the
/// legacy root itself).
const SERVER_IN_ROOT_REL: &str = "node_modules/@kiro/agent/dist/server/acp-server.js";
/// `<home>`-relative path of the tier-5 file-auth token (login precheck).
const TOKEN_FILE_REL: &str = ".aws/sso/cache/kiro-auth-token.json";

/// Strictly parse a versioned-extraction dir name — `<MAJOR.MINOR.PATCH>-<sha>`
/// with exactly 64 lowercase hex sha digits (e.g. `2.11.0-05e9…`) — into its
/// semver tuple. Deliberately NOT [`super::version::parse_semver`]: that parser
/// is lenient by design (it tolerates `kiro-cli 2.8.1-beta` CLI output), and
/// leniency here would admit non-extraction dirs (`v2.10.0-…`, `2.10-…`) as
/// spawn candidates.
fn dir_version(name: &str) -> Option<(u32, u32, u32)> {
    let (ver, sha) = name.split_once('-')?;
    if sha.len() != 64
        || !sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut parts = ver.split('.');
    let mut num = || -> Option<u32> {
        let p = parts.next()?;
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse().ok()
    };
    let v = (num()?, num()?, num()?);
    if parts.next().is_some() {
        return None;
    }
    Some(v)
}

/// Pick the versioned-extraction dir to spawn from `entries`
/// (`(dir_name, has_server_entry)` pairs listed from the kas root):
/// a dir matching `cli_version` exactly beats the newest by semver-tuple
/// order; dirs failing the name grammar or lacking the inner `acp-server.js`
/// (partial extractions) are never candidates; version ties resolve to the
/// greatest dir name, so duplicate re-extractions pick deterministically.
/// `None` means no versioned candidate — the caller falls back to the legacy
/// unversioned layout.
fn select_server(entries: &[(String, bool)], cli_version: Option<(u32, u32, u32)>) -> Option<&str> {
    let mut exact: Option<&str> = None;
    let mut newest: Option<((u32, u32, u32), &str)> = None;
    for (name, has_server) in entries {
        if !has_server {
            continue;
        }
        let Some(v) = dir_version(name) else { continue };
        if Some(v) == cli_version && exact.is_none_or(|b| name.as_str() > b) {
            exact = Some(name);
        }
        if newest.is_none_or(|(bv, bn)| (v, name.as_str()) > (bv, bn)) {
            newest = Some((v, name));
        }
    }
    exact.or(newest.map(|(_, n)| n))
}

/// Treat an env value as "not provided" when unset or empty/whitespace-only —
/// so `KIRO_AGENT_PATH=""` falls back to PATH rather than spawning the empty
/// string as a binary.
fn nonempty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.trim().is_empty())
}

/// First `node<EXE_SUFFIX>` found in a PATH-style variable, if any. Uses
/// [`std::env::split_paths`] so the separator (`:` vs `;`) is correct per
/// platform; `EXE_SUFFIX` adds `.exe` on Windows.
fn find_on_path(path_var: Option<&OsStr>, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let exe = format!("node{}", std::env::consts::EXE_SUFFIX);
    let path_var = path_var?;
    std::env::split_paths(path_var)
        .map(|dir| dir.join(&exe))
        .find(|cand| exists(cand))
}

/// Resolve the free-path spawn argv from explicit inputs, using `exists` to test
/// file presence. Pure (no env reads, no real filesystem), so the precheck
/// matrix is unit-testable. Preconditions are load-bearing for correctness — a
/// missing item must surface as the right typed `Err`, not a later opaque spawn
/// failure — so they are enforced as runtime returns, not `debug_assert!`.
fn resolve(
    home: Option<&Path>,
    server_override: Option<&str>,
    node_override: Option<&str>,
    path_var: Option<&OsStr>,
    kas_entries: &[(String, bool)],
    cli_version: Option<(u32, u32, u32)>,
    exists: impl Fn(&Path) -> bool,
) -> Result<AgentCommand, KasMissing> {
    // 1. server.js — override, else the selected versioned dir under
    //    <home>/<kas root>, else the legacy unversioned layout; must exist.
    let server: PathBuf = match server_override {
        Some(s) => PathBuf::from(s),
        None => {
            let root = home.ok_or(KasMissing::NoHome)?.join(KAS_ROOT_REL);
            match select_server(kas_entries, cli_version) {
                Some(dir) => root.join(dir).join(SERVER_IN_ROOT_REL),
                None => root.join(SERVER_IN_ROOT_REL),
            }
        }
    };
    if !exists(&server) {
        return Err(KasMissing::Server(server));
    }

    // 2. node — override (must exist), else the first `node` on PATH.
    let node: PathBuf = match node_override {
        Some(n) => {
            let p = PathBuf::from(n);
            if !exists(&p) {
                return Err(KasMissing::Node);
            }
            p
        }
        None => find_on_path(path_var, &exists).ok_or(KasMissing::Node)?,
    };

    // 3. login precheck — the tier-5 token file must exist (KAS reads+refreshes it).
    let token = home.ok_or(KasMissing::NoHome)?.join(TOKEN_FILE_REL);
    if !exists(&token) {
        return Err(KasMissing::NotLoggedIn(token));
    }

    Ok(
        AgentCommand::new(node.to_string_lossy().into_owned()).with_args(vec![
            "--experimental-wasm-modules".to_string(),
            server.to_string_lossy().into_owned(),
            "--transport=stdio".to_string(),
        ]),
    )
}

/// Resolve the free-path KAS spawn from the real environment + filesystem.
/// `KIRO_KAS_SERVER_PATH` / `KIRO_AGENT_PATH` override the defaults.
pub(crate) fn resolve_kas_command() -> Result<AgentCommand, KasMissing> {
    let home = crate::kiro_agent_config::home_dir();
    let server_override = nonempty(std::env::var("KIRO_KAS_SERVER_PATH").ok());
    let node_override = nonempty(std::env::var("KIRO_AGENT_PATH").ok());
    let path_var = std::env::var_os("PATH");
    // Versioned-dir listing + installed-CLI version are inert here until the
    // C5–C7 wiring lands (design .cyril-dcc6/design.md): an empty listing makes
    // `select_server` decline and `resolve` use the legacy layout, exactly the
    // pre-versioned-discovery behavior.
    resolve(
        home.as_deref(),
        server_override.as_deref(),
        node_override.as_deref(),
        path_var.as_deref(),
        &[],
        None,
        |p| p.is_file(),
    )
}

/// The tier-5 file-auth token path (`~/.aws/sso/cache/kiro-auth-token.json`), if
/// a home directory is known. Shared with the Part B auth responder — the same
/// file the free-path precheck verifies and that KAS/kiro-cli maintain.
pub(crate) fn default_token_path() -> Option<std::path::PathBuf> {
    crate::kiro_agent_config::home_dir().map(|h| h.join(TOKEN_FILE_REL))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::collections::HashSet;
    use std::ffi::OsString;

    /// Build an `exists` closure that returns true only for the given paths.
    /// `use<>` (precise capturing): the closure owns `set` and borrows nothing,
    /// so the return type captures none of the input lifetimes (edition 2024).
    fn exists_set(paths: &[&str]) -> impl Fn(&Path) -> bool + use<> {
        let set: HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }

    const HOME: &str = "/home/u";
    fn default_server() -> String {
        format!("{HOME}/{KAS_ROOT_REL}/{SERVER_IN_ROOT_REL}")
    }
    /// `resolve` with no versioned-dir candidates and no CLI version — the
    /// legacy layout the pre-slice tests exercise.
    fn resolve_legacy(
        home: Option<&Path>,
        server_override: Option<&str>,
        node_override: Option<&str>,
        path_var: Option<&OsStr>,
        exists: impl Fn(&Path) -> bool,
    ) -> Result<AgentCommand, KasMissing> {
        resolve(
            home,
            server_override,
            node_override,
            path_var,
            &[],
            None,
            exists,
        )
    }
    fn default_token() -> String {
        format!("{HOME}/{TOKEN_FILE_REL}")
    }
    /// A `<dir>/node` candidate carrying the platform exe suffix (`node.exe` on
    /// Windows), so the injected `exists` set matches what [`find_on_path`]
    /// actually looks for. `PathBuf` hashing/equality is separator-agnostic, so
    /// the forward slashes here still match `find_on_path`'s `Path::join` output.
    fn node(dir: &str) -> String {
        format!("{dir}/node{}", std::env::consts::EXE_SUFFIX)
    }

    // C2: happy path — defaults resolve to the exact probe-proven argv.
    #[test]
    fn resolve_happy_path_builds_probe_argv() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&default_server(), &default_token(), &node("/usr/bin")]);
        let cmd = resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists)
            .expect("all preconditions present");
        // Compare as paths, not strings: on Windows `find_on_path`/`Path::join`
        // yield `\` separators and a `.exe` suffix that an exact string compare
        // against a Unix literal would fail.
        assert_eq!(Path::new(cmd.program()), Path::new(&node("/usr/bin")));
        let args = cmd.args();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "--experimental-wasm-modules");
        assert_eq!(Path::new(&args[1]), Path::new(&default_server()));
        assert_eq!(args[2], "--transport=stdio");
    }

    // C3a: server missing while node IS present → Server, NOT Node (the
    // wrong-missing-item bug: checking node first / reporting the wrong path).
    #[test]
    fn resolve_missing_server_reports_server_not_node() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&default_token(), &node("/usr/bin")]); // server absent
        let err =
            resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists).unwrap_err();
        assert_eq!(err, KasMissing::Server(PathBuf::from(default_server())));
    }

    // C3b: server+token present, no node anywhere → Node.
    #[test]
    fn resolve_missing_node() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&default_server(), &default_token()]); // no node on PATH
        let err =
            resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists).unwrap_err();
        assert_eq!(err, KasMissing::Node);
    }

    // C3c: server+node present, token absent → NotLoggedIn.
    #[test]
    fn resolve_missing_token_is_not_logged_in() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&default_server(), &node("/usr/bin")]); // token absent
        let err =
            resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists).unwrap_err();
        assert_eq!(err, KasMissing::NotLoggedIn(PathBuf::from(default_token())));
    }

    // Stress: a node override that does not exist → Node (not a silent spawn).
    #[test]
    fn resolve_node_override_missing_errors() {
        let exists = exists_set(&[&default_server(), &default_token()]);
        let err = resolve_legacy(
            Some(Path::new(HOME)),
            None,
            Some("/no/such/node"),
            None,
            exists,
        )
        .unwrap_err();
        assert_eq!(err, KasMissing::Node);
    }

    // Stress: a server-override path WITH SPACES is preserved as ONE arg, not split.
    #[test]
    fn resolve_server_path_with_spaces_is_one_arg() {
        let spaced = "/opt/my kas/acp-server.js";
        let exists = exists_set(&[spaced, &default_token(), &node("/usr/bin")]);
        let path = OsString::from("/usr/bin");
        let cmd = resolve_legacy(
            Some(Path::new(HOME)),
            Some(spaced),
            None,
            Some(&path),
            exists,
        )
        .expect("override + token + node present");
        assert!(
            cmd.args().contains(&spaced.to_string()),
            "spaced server path must be one arg, got {:?}",
            cmd.args()
        );
    }

    // Stress: no home and no override → NoHome (can't build the default path).
    #[test]
    fn resolve_no_home_no_override_errors() {
        let err = resolve_legacy(None, None, None, None, |_| true).unwrap_err();
        assert_eq!(err, KasMissing::NoHome);
    }

    // Stress: empty / whitespace env value is treated as unset.
    #[test]
    fn nonempty_treats_blank_as_unset() {
        assert_eq!(nonempty(None), None);
        assert_eq!(nonempty(Some(String::new())), None);
        assert_eq!(nonempty(Some("   ".to_string())), None);
        assert_eq!(nonempty(Some("/x".to_string())), Some("/x".to_string()));
    }

    // find_on_path returns the FIRST matching dir's node (PATH order honored).
    #[test]
    fn find_on_path_returns_first_match() {
        let path = std::env::join_paths(["/a", "/b"]).unwrap();
        let exists = exists_set(&[&node("/b")]); // only /b has node
        assert_eq!(
            find_on_path(Some(&path), exists),
            Some(PathBuf::from(node("/b")))
        );
    }

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    fn e(name: &str, has_server: bool) -> (String, bool) {
        (name.to_string(), has_server)
    }

    // C1 fence: a dir matching the CLI version wins even when a NEWER dir
    // exists (kills the always-pick-newest implementation).
    #[test]
    fn picks_exact_version_match() {
        let entries = [
            e(&format!("2.10.0-{SHA_A}"), true),
            e(&format!("2.11.0-{SHA_B}"), true),
        ];
        let want = format!("2.10.0-{SHA_A}");
        assert_eq!(
            select_server(&entries, Some((2, 10, 0))),
            Some(want.as_str())
        );
    }

    // C2 fence: with no exact match, ordering is SEMVER-tuple, not lexical —
    // 2.10.0 > 2.9.0 (a lexicographic max picks "2.9.0" and fails here).
    #[test]
    fn picks_newest_semver_not_lex() {
        let entries = [
            e(&format!("2.9.0-{SHA_A}"), true),
            e(&format!("2.10.0-{SHA_B}"), true),
        ];
        let got = select_server(&entries, Some((2, 11, 0))).expect("candidates exist");
        assert!(
            got.starts_with("2.10.0-"),
            "lexicographic-max bug: got {got}"
        );
    }

    // C3 fence: CLI version unavailable → newest, not an error (kills the
    // err-when-kiro-cli-missing implementation).
    #[test]
    fn no_cli_version_falls_back_newest() {
        let entries = [
            e(&format!("2.10.0-{SHA_A}"), true),
            e(&format!("2.11.0-{SHA_B}"), true),
        ];
        let got = select_server(&entries, None).expect("candidates exist");
        assert!(got.starts_with("2.11.0-"), "got {got}");
    }

    // C4 fence: a dir without the inner acp-server.js (partial extraction) is
    // never a candidate (kills the name-glob-only implementation).
    #[test]
    fn partial_extraction_skipped() {
        let entries = [
            e(&format!("2.10.0-{SHA_A}"), true),
            e(&format!("2.11.0-{SHA_B}"), false),
        ];
        let got = select_server(&entries, Some((2, 11, 0))).expect("complete candidate exists");
        assert!(
            got.starts_with("2.10.0-"),
            "picked a partial extraction: {got}"
        );
        // All candidates partial → None (falls back to legacy at the caller).
        assert_eq!(
            select_server(&[e(&format!("2.11.0-{SHA_B}"), false)], None),
            None
        );
    }

    // C4 grammar fence: names outside `<semver>-<sha64 lowercase hex>` are
    // ignored — 63-char sha, non-hex, uppercase hex, `v` prefix, two-part
    // version, trailing component, lock-file suffix.
    #[test]
    fn malformed_dir_names_ignored() {
        let bad = [
            format!("2.10.0-{}", &SHA_A[..63]),
            format!("2.10.0-{}", "z".repeat(64)),
            format!("2.10.0-{}", SHA_A.to_uppercase()),
            format!("v2.10.0-{SHA_A}"),
            format!("2.10-{SHA_A}"),
            format!("2.10.0.1-{SHA_A}"),
            format!("2.10.0-{SHA_A}.lock"),
        ];
        let entries: Vec<_> = bad.iter().map(|n| e(n, true)).collect();
        assert_eq!(select_server(&entries, Some((2, 10, 0))), None);
    }

    // Duplicate re-extractions of the SAME version pick deterministically
    // (greatest dir name), independent of listing order.
    #[test]
    fn duplicate_same_version_is_deterministic() {
        let a = format!("2.10.0-{SHA_A}");
        let b = format!("2.10.0-{SHA_B}");
        for entries in [[e(&a, true), e(&b, true)], [e(&b, true), e(&a, true)]] {
            assert_eq!(select_server(&entries, Some((2, 10, 0))), Some(b.as_str()));
            assert_eq!(select_server(&entries, None), Some(b.as_str()));
        }
    }

    // Empty listing → None (the caller's legacy fallback path).
    #[test]
    fn empty_listing_selects_nothing() {
        assert_eq!(select_server(&[], Some((2, 10, 0))), None);
        assert_eq!(select_server(&[], None), None);
    }

    // Each KasMissing variant yields a non-empty, actionable reason.
    #[test]
    fn missing_reasons_are_actionable() {
        assert!(KasMissing::NoHome.reason().contains("KIRO_KAS_SERVER_PATH"));
        assert!(
            KasMissing::Server(PathBuf::from("/x"))
                .reason()
                .contains("kiro-cli acp")
        );
        assert!(KasMissing::Node.reason().contains("KIRO_AGENT_PATH"));
        assert!(
            KasMissing::NotLoggedIn(PathBuf::from("/x"))
                .reason()
                .contains("kiro-cli login")
        );
    }
}

//! Free-path KAS spawn discovery (KAS-1 Part A, cyril-evwh; versioned dirs +
//! callback auth, cyril-dcc6).
//!
//! Resolves the argv that spawns the bundled `@kiro/agent` ACP server directly
//! over stdio with `--auth=acp-callback` — byte-identical to kiro-cli's own
//! spawn (probe-verified against /proc). KAS then requests credentials via
//! `_kiro/auth/getAccessToken`, answered by [`super::auth`] from kiro-cli's
//! sqlite store; the SSO-cache token file plays no role on any path. Reuses
//! [`AgentCommand`] as the spawn description so `AgentProcess::spawn` consumes
//! it unchanged.

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
    /// No home directory to locate the credential store — distinct from
    /// [`KasMissing::NoHome`] because it is reachable with the bundle already
    /// resolved via `KIRO_KAS_SERVER_PATH` (dcc6 review F6), where "set the
    /// override" would be misleading advice.
    NoHomeForStore,
    /// The credential store cannot serve a login right now; `why` carries the
    /// precise diagnostic (absent/locked/corrupt store, logged out, or expired
    /// token — dcc6 review F4), so a locked store is never misreported as
    /// "run `kiro-cli login`".
    StoreUnservable { store: PathBuf, why: String },
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
            KasMissing::NoHomeForStore => {
                "cannot locate the kiro credential store: no home directory (HOME unset). \
                 KAS auth is served from kiro-cli's login store (`kiro-cli login`)."
                    .to_string()
            }
            KasMissing::StoreUnservable { store, why } => {
                format!("KAS auth not servable from {}: {why}", store.display())
            }
        }
    }
}

/// `<home>`-relative kiro-cli data dir — the shared prefix of the KAS
/// extraction root and the credential store ([`default_store_path`]); a unit
/// test pins [`KAS_ROOT_REL`] to this prefix so the two cannot drift apart
/// (dcc6 review F19b).
const KIRO_DATA_DIR_REL: &str = ".local/share/kiro-cli";
/// `<home>`-relative path of the KAS self-extraction root. kiro ≥2.10.0
/// extracts into versioned `<semver>-<sha256>/` dirs under it; older releases
/// extracted the bundle directly at the root (the legacy layout).
const KAS_ROOT_REL: &str = ".local/share/kiro-cli/kas";
/// Path of the ACP server entry inside one extraction (versioned dir or the
/// legacy root itself).
const SERVER_IN_ROOT_REL: &str = "node_modules/@kiro/agent/dist/server/acp-server.js";

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
    if exact.is_some() {
        return exact;
    }
    let (_, name) = newest?;
    if let Some((maj, min, pat)) = cli_version {
        // An entry matching the CLI version would have set `exact`, so reaching
        // here with a known version means a MISMATCHED bundle is about to spawn
        // (fresh upgrade not yet self-extracted, or the matching extraction is
        // partial) — leave a breadcrumb (dcc6 review F5).
        tracing::warn!(
            cli_version = format_args!("{maj}.{min}.{pat}"),
            selected = name,
            "no KAS extraction matches the installed kiro-cli; spawning the newest"
        );
    }
    Some(name)
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

    Ok(
        AgentCommand::new(node.to_string_lossy().into_owned()).with_args(vec![
            "--experimental-wasm-modules".to_string(),
            server.to_string_lossy().into_owned(),
            "--transport=stdio".to_string(),
            "--auth=acp-callback".to_string(),
        ]),
    )
}

/// List the kas extraction root: one `(dir_name, has_server_entry)` pair per
/// directory entry. A missing root yields an empty list (normal on a machine
/// where kiro-cli has never self-extracted — `resolve` then reports the
/// actionable `Server` error); an unreadable root warns before the same
/// fallback (dcc6 review F13 — the `Server` reason's "run kiro-cli acp to
/// self-extract" remedy is wrong for a permissions problem, so the log line
/// must carry the real cause). Individual entries that can't be read or hold
/// non-UTF-8 names are skipped with a debug line (F12) — a skipped entry
/// could otherwise silently cost the exact-match dir.
fn list_kas_entries(root: &Path) -> Vec<(String, bool)> {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            tracing::warn!(root = %root.display(), error = %e, "kas root unreadable; treating as never extracted");
            return Vec::new();
        }
    };
    entries
        .filter_map(|entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::debug!(root = %root.display(), error = %e, "unreadable kas dir entry skipped");
                    return None;
                }
            };
            let name = match entry.file_name().into_string() {
                Ok(n) => n,
                Err(raw) => {
                    tracing::debug!(root = %root.display(), name = ?raw, "non-UTF-8 kas dir entry skipped");
                    return None;
                }
            };
            let has_server = root.join(&name).join(SERVER_IN_ROOT_REL).is_file();
            Some((name, has_server))
        })
        .collect()
}

/// The installed kiro-cli's semver tuple, from `kiro-cli --version` on PATH.
/// `None` (binary absent, non-zero exit, unparseable output) is warn-logged —
/// selection then prefers the newest extraction instead of an exact match,
/// which is the best available guess when the CLI can't be asked.
fn installed_cli_version() -> Option<(u32, u32, u32)> {
    let step =
        super::version::kiro_cli_version("kiro-cli").and_then(|v| super::version::parse_semver(&v));
    match step {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(error = %e, "kiro-cli version unavailable; selecting newest KAS extraction");
            None
        }
    }
}

/// Resolve the free-path KAS spawn from the real environment + filesystem.
/// `KIRO_KAS_SERVER_PATH` / `KIRO_AGENT_PATH` override the defaults.
pub(crate) fn resolve_kas_command() -> Result<AgentCommand, KasMissing> {
    let home = crate::kiro_agent_config::home_dir();
    let server_override = nonempty(std::env::var("KIRO_KAS_SERVER_PATH").ok());
    let node_override = nonempty(std::env::var("KIRO_AGENT_PATH").ok());
    let path_var = std::env::var_os("PATH");
    // The dir scan + `kiro-cli --version` subprocess only run when they can
    // influence selection: an override names the server directly (and must win
    // without paying that cost), and no home means no root to scan (`resolve`
    // reports `NoHome`). The match makes the home-is-present invariant of the
    // scanning arm structural (dcc6 review F14 — no dead unwrap_or_default).
    let (kas_entries, cli_version) = match (server_override.as_deref(), home.as_deref()) {
        (Some(_), _) | (None, None) => (Vec::new(), None),
        (None, Some(h)) => (
            list_kas_entries(&h.join(KAS_ROOT_REL)),
            installed_cli_version(),
        ),
    };
    let cmd = resolve(
        home.as_deref(),
        server_override.as_deref(),
        node_override.as_deref(),
        path_var.as_deref(),
        &kas_entries,
        cli_version,
        |p| p.is_file(),
    )?;
    // Login gate (C14a): with `--auth=acp-callback` the responder is
    // load-bearing for every turn, so an unservable credential store — absent,
    // locked, corrupt, logged out, or holding an expired token — fails the
    // spawn here with its precise diagnostic, instead of as a dead first turn.
    let db = default_store_path().ok_or(KasMissing::NoHomeForStore)?;
    if let Some(why) = super::auth::store_unservable_reason(&db, super::auth::now_epoch()) {
        return Err(KasMissing::StoreUnservable { store: db, why });
    }
    Ok(cmd)
}

/// kiro-cli's credential store (`~/.local/share/kiro-cli/data.sqlite3`) — the
/// sqlite database `kiro-cli login` maintains (IdC token in `auth_kv`, active
/// profile in `state`). The auth responder's source: unlike the SSO-cache
/// token file, this is refreshed by every login and deleted-row on logout.
pub(crate) fn default_store_path() -> Option<std::path::PathBuf> {
    crate::kiro_agent_config::home_dir().map(|h| h.join(KIRO_DATA_DIR_REL).join("data.sqlite3"))
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
        let exists = exists_set(&[&default_server(), &node("/usr/bin")]);
        let cmd = resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists)
            .expect("all preconditions present");
        // Compare as paths, not strings: on Windows `find_on_path`/`Path::join`
        // yield `\` separators and a `.exe` suffix that an exact string compare
        // against a Unix literal would fail.
        assert_eq!(Path::new(cmd.program()), Path::new(&node("/usr/bin")));
        let args = cmd.args();
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "--experimental-wasm-modules");
        assert_eq!(Path::new(&args[1]), Path::new(&default_server()));
        assert_eq!(args[2], "--transport=stdio");
        assert_eq!(args[3], "--auth=acp-callback");
    }

    // C3a: server missing while node IS present → Server, NOT Node (the
    // wrong-missing-item bug: checking node first / reporting the wrong path).
    #[test]
    fn resolve_missing_server_reports_server_not_node() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&node("/usr/bin")]); // server absent
        let err =
            resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists).unwrap_err();
        assert_eq!(err, KasMissing::Server(PathBuf::from(default_server())));
    }

    // C3b: server+token present, no node anywhere → Node.
    #[test]
    fn resolve_missing_node() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&default_server()]); // no node on PATH
        let err =
            resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists).unwrap_err();
        assert_eq!(err, KasMissing::Node);
    }

    // Stress: a node override that does not exist → Node (not a silent spawn).
    #[test]
    fn resolve_node_override_missing_errors() {
        let exists = exists_set(&[&default_server()]);
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
        let exists = exists_set(&[spaced, &node("/usr/bin")]);
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

    // C5 fence: when BOTH the legacy layout and a versioned dir exist, the
    // versioned dir wins (kills the legacy-checked-first implementation).
    #[test]
    fn versioned_beats_legacy() {
        let dir = format!("2.10.0-{SHA_A}");
        let versioned = format!("{HOME}/{KAS_ROOT_REL}/{dir}/{SERVER_IN_ROOT_REL}");
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[
            &default_server(), // legacy present too
            &versioned,
            &node("/usr/bin"),
        ]);
        let cmd = resolve(
            Some(Path::new(HOME)),
            None,
            None,
            Some(&path),
            &[e(&dir, true)],
            Some((2, 10, 0)),
            exists,
        )
        .expect("all preconditions present");
        assert_eq!(
            Path::new(&cmd.args()[1]),
            Path::new(&versioned),
            "legacy-first bug: picked {}",
            cmd.args()[1]
        );
    }

    // C7 fence: KIRO_KAS_SERVER_PATH bypasses the glob — the override wins
    // even when versioned candidates exist (kills the glob-anyway impl).
    #[test]
    fn override_beats_versioned() {
        let dir = format!("2.11.0-{SHA_B}");
        let versioned = format!("{HOME}/{KAS_ROOT_REL}/{dir}/{SERVER_IN_ROOT_REL}");
        let override_path = "/opt/custom/acp-server.js";
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[override_path, &versioned, &node("/usr/bin")]);
        let cmd = resolve(
            Some(Path::new(HOME)),
            Some(override_path),
            None,
            Some(&path),
            &[e(&dir, true)],
            Some((2, 11, 0)),
            exists,
        )
        .expect("override + token + node present");
        assert_eq!(cmd.args()[1], override_path);
    }

    // C6 fence: nothing found anywhere → Server whose reason names the
    // searched kas root (kills the wrong-variant / path-less-message impl).
    #[test]
    fn nothing_found_names_search_root() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&node("/usr/bin")]);
        let err = resolve(
            Some(Path::new(HOME)),
            None,
            None,
            Some(&path),
            &[],
            Some((2, 11, 0)),
            exists,
        )
        .unwrap_err();
        let KasMissing::Server(p) = &err else {
            panic!("expected Server, got {err:?}");
        };
        assert!(
            p.starts_with(format!("{HOME}/{KAS_ROOT_REL}")),
            "got {}",
            p.display()
        );
        assert!(err.reason().contains(KAS_ROOT_REL));
    }

    // Real-filesystem listing: complete extraction, partial extraction, and a
    // `.lock` FILE sibling (as kiro-cli actually leaves them) pair correctly;
    // a missing root lists empty.
    #[test]
    fn list_kas_entries_real_fs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("kas");
        let complete = format!("2.10.0-{SHA_A}");
        let partial = format!("2.11.0-{SHA_B}");
        let server_dir = root
            .join(&complete)
            .join("node_modules/@kiro/agent/dist/server");
        std::fs::create_dir_all(&server_dir).expect("mkdir complete");
        std::fs::write(server_dir.join("acp-server.js"), "//").expect("write entry");
        std::fs::create_dir_all(root.join(&partial)).expect("mkdir partial");
        std::fs::write(root.join(format!("{complete}.lock")), "").expect("write lock");

        let mut entries = list_kas_entries(&root);
        entries.sort();
        assert_eq!(
            entries,
            vec![
                (complete.clone(), true),
                (format!("{complete}.lock"), false),
                (partial.clone(), false),
            ]
        );
        // The full pipeline over this real layout picks the complete dir.
        assert_eq!(
            select_server(&entries, Some((2, 11, 0))),
            Some(complete.as_str()),
            "partial/lock entries must not be candidates"
        );
        // Missing root → empty, not an error.
        assert!(list_kas_entries(&root.join("nope")).is_empty());
    }

    // C8 fence: the non-path flags are byte-equal to kiro-cli's own KAS
    // spawn as captured from /proc (prove-it-prototype oracle) — a missing
    // `--auth=acp-callback` (the pre-dcc6 argv) fails here by construction.
    #[test]
    fn argv_matches_kiro_cli_own_spawn() {
        let path = OsString::from("/usr/bin");
        let exists = exists_set(&[&default_server(), &node("/usr/bin")]);
        let cmd = resolve_legacy(Some(Path::new(HOME)), None, None, Some(&path), exists)
            .expect("all preconditions present");
        let flags: Vec<&str> = cmd
            .args()
            .iter()
            .map(String::as_str)
            .filter(|a| a.starts_with("--"))
            .collect();
        assert_eq!(
            flags,
            [
                "--experimental-wasm-modules",
                "--transport=stdio",
                "--auth=acp-callback"
            ],
            "flag drift vs the /proc-captured kiro-cli spawn"
        );
    }

    // F19b drift fence: the extraction root and the credential store must
    // share the kiro-cli data dir — a path change that touches only one of
    // them fails here.
    #[test]
    fn kas_root_shares_kiro_data_dir() {
        assert_eq!(KAS_ROOT_REL.strip_suffix("/kas"), Some(KIRO_DATA_DIR_REL));
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
        // F6: the store-side no-home reason must NOT point at the bundle
        // override (the user may already have set it) — it names the store.
        let store_reason = KasMissing::NoHomeForStore.reason();
        assert!(store_reason.contains("credential store"), "{store_reason}");
        assert!(
            !store_reason.contains("KIRO_KAS_SERVER_PATH"),
            "misdirects to the bundle override: {store_reason}"
        );
        // F4: the gate's reason carries the underlying diagnostic verbatim.
        let unservable = KasMissing::StoreUnservable {
            store: PathBuf::from("/x"),
            why: "kiro token row absent — logged out; run `kiro-cli login`".to_string(),
        };
        assert!(unservable.reason().contains("kiro-cli login"));
        assert!(unservable.reason().contains("/x"));
    }
}

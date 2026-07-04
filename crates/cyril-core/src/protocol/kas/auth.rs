//! KAS auth responder (KAS-1 Part B, cyril-evwh; sqlite source, cyril-dcc6).
//!
//! Answers the `_kiro/auth/getAccessToken` server→client request KAS sends
//! under `--auth=acp-callback` — both spawn modes: the wrapper (kiro-cli
//! forwards the callback to its ACP client) and the free path (the directly
//! spawned server asks its host). The source is kiro-cli's **sqlite credential
//! store** (`data.sqlite3`), the only store `kiro-cli login` refreshes; the
//! SSO-cache token file is deliberately not consulted (it can hold a dead
//! identity that self-refreshes — the cyril-dcc6 bug). cyril is a **custodian**
//! of the credential: read-only, held for one reply, redacted in `Debug`,
//! never logged, and the store's refresh token is never extracted or
//! transmitted (the row containing it is read whole; only the access fields
//! are taken from it).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_client_protocol as acp;

/// The method string cyril receives for the wire request `_kiro/auth/getAccessToken`.
/// The ACP library strips the single leading `_` before dispatch (same as it
/// does for the `_kiro.dev/*` ext notifications cyril already handles as
/// `kiro.dev/*`), so the `_kiro` namespace arrives as `kiro`.
pub(crate) const GET_ACCESS_TOKEN_METHOD: &str = "kiro/auth/getAccessToken";

/// KAS rejects a token within this pre-expiry window (it validates
/// `expiresAt > now + ~3min`), so cyril treats such a token as stale.
const EXPIRY_BUFFER_SECS: i64 = 180;

/// JSON-RPC error codes the responder replies with: internal (store, task, or
/// serialize failures) and server-defined (stale token — the user must re-login).
const JSONRPC_INTERNAL_ERROR: i32 = -32603;
const JSONRPC_STALE_TOKEN: i32 = -32000;

/// A redacted access-token wrapper: its `Debug` never prints the secret, so a
/// stray `{:?}` or a tracing of any struct containing it cannot leak the
/// credential (spec SC4 custodian).
struct AccessToken(String);

impl std::fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessToken(***redacted***)")
    }
}

/// The validated `getAccessToken` reply payload.
#[derive(Debug)]
struct AuthReply {
    access_token: AccessToken,
    expires_at: String,
    profile_arn: String,
}

/// The credential-store rows kiro-cli maintains in `data.sqlite3`: the IdC
/// token JSON (snake_case fields; deleted on logout) and the active
/// CodeWhisperer profile JSON (`{arn, profile_name}` — the token row stopped
/// carrying `profile_arn`, so the `state` row is mandatory).
const TOKEN_ROW_SQL: &str = "SELECT value FROM auth_kv WHERE key = 'kirocli:odic:token'";
const PROFILE_ROW_SQL: &str = "SELECT value FROM state WHERE key = 'api.codewhisperer.profile'";

/// Read kiro-cli's sqlite credential store into a reply. Synchronous
/// (rusqlite) — the live-callback path wraps it in `spawn_blocking`; the
/// spawn-time gate ([`store_unservable_reason`]) calls it directly, which is
/// safe only because that runs once at startup, before any session traffic.
/// The store is opened READ_ONLY (never created, never written; kiro-cli
/// writes it concurrently) with a short busy timeout. Every failure mode is
/// distinguished: absent store / absent row (logged out — actionable), locked,
/// and corrupt (parse errors name the row). The error is the diagnostic,
/// never the token.
fn read_sqlite_store(db: &Path) -> Result<AuthReply, String> {
    if !db.is_file() {
        return Err(format!(
            "kiro credential store {} is absent; run `kiro-cli login`",
            db.display()
        ));
    }
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("open kiro credential store: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(250))
        .map_err(|e| format!("set kiro credential store busy timeout: {e}"))?;
    let row = |sql: &str, what: &str| -> Result<serde_json::Value, String> {
        use rusqlite::OptionalExtension;
        let raw = conn
            .query_row(sql, [], |r| r.get::<_, String>(0))
            .optional()
            .map_err(|e| format!("query {what}: {e}"))?
            .ok_or_else(|| format!("{what} row absent — logged out; run `kiro-cli login`"))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse {what} row: {e}"))
    };
    let token = row(TOKEN_ROW_SQL, "kiro token")?;
    let field = |k: &str| -> Result<String, String> {
        match token.get(k).and_then(|x| x.as_str()) {
            Some(s) if !s.is_empty() => Ok(s.to_string()),
            _ => Err(format!("kiro token row missing `{k}`")),
        }
    };
    let access_token = AccessToken(field("access_token")?);
    let expires_at = field("expires_at")?;
    let profile = row(PROFILE_ROW_SQL, "kiro profile")?;
    let profile_arn = match profile.get("arn").and_then(|x| x.as_str()) {
        // Load-bearing: a reply with an absent/empty arn 400s at the backend
        // ("profileArn is required"), so it is an error, not a default.
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return Err("kiro profile row missing `arn`".to_string()),
    };
    Ok(AuthReply {
        access_token,
        expires_at,
        profile_arn,
    })
}

/// Parse an RFC3339 **UTC** timestamp (`YYYY-MM-DDTHH:MM:SS[.fff]Z`) to unix
/// epoch seconds; sub-second precision is ignored (second precision suffices for
/// a 180s buffer). A trailing `Z` is **required** — a numeric offset (`+02:00`)
/// would otherwise be silently read as UTC and mis-date the token, so a non-`Z`
/// stamp returns `None` (which [`is_stale`] treats as stale, fail safe). Strict
/// on layout. Uses the canonical days-from-civil algorithm (Howard Hinnant).
fn rfc3339_to_epoch(s: &str) -> Option<i64> {
    if !s.ends_with('Z') {
        return None;
    }
    let b = s.as_bytes();
    if b.len() < 19
        || b.get(4) != Some(&b'-')
        || b.get(7) != Some(&b'-')
        || b.get(10) != Some(&b'T')
    {
        return None;
    }
    let f = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse().ok() };
    let (y, mo, d) = (f(0..4)?, f(5..7)?, f(8..10)?);
    let (h, mi, se) = (f(11..13)?, f(14..16)?, f(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    // days_from_civil: epoch days for a Gregorian (y, mo, d).
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = (if yy >= 0 { yy } else { yy - 399 }) / 400;
    let yoe = yy - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + h * 3600 + mi * 60 + se)
}

/// True when the token is expired or within the pre-expiry buffer (KAS would
/// reject it), or when the expiry can't be parsed, or the clock is unreadable
/// (all three fail safe → stale, so a token cyril cannot time-check is never
/// forwarded). Pure.
fn is_stale(expires_at: &str, now_epoch: Option<i64>) -> bool {
    match (now_epoch, rfc3339_to_epoch(expires_at)) {
        (Some(now), Some(exp)) => exp <= now + EXPIRY_BUFFER_SECS,
        _ => true,
    }
}

/// Current unix epoch seconds, or `None` if the system clock predates
/// `UNIX_EPOCH` (a broken/backwards clock). `None` makes [`is_stale`] fail safe
/// rather than masking the error as epoch 0 — which would make every real,
/// future-dated token look *fresh* and forward a credential cyril never
/// validated against the real time.
pub(super) fn now_epoch() -> Option<i64> {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => Some(d.as_secs() as i64),
        Err(e) => {
            tracing::warn!(error = %e, "system clock before UNIX_EPOCH; treating KAS token as stale");
            None
        }
    }
}

/// Build the `{accessToken, expiresAt, profileArn}` ExtResponse KAS expects.
fn build_response(reply: &AuthReply) -> Result<acp::ExtResponse, String> {
    let body = serde_json::json!({
        "accessToken": reply.access_token.0,
        "expiresAt": reply.expires_at,
        "profileArn": reply.profile_arn,
    });
    let raw = serde_json::value::RawValue::from_string(body.to_string())
        .map_err(|e| format!("serialize getAccessToken reply: {e}"))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

/// Why the credential store cannot serve `getAccessToken` right now — absent/
/// locked/corrupt store, the logged-out row shape, or a token already expired
/// or expiring (dcc6 review F3/F4) — or `None` when a callback made now would
/// succeed. The free-path spawn gate (C14a): with `--auth=acp-callback` the
/// responder is load-bearing for every turn, so an unservable store must fail
/// the spawn up front with the precise diagnostic instead of as a dead first
/// turn. `now` is injected so gate tests never race the fixture's expiry.
pub(crate) fn store_unservable_reason(db: &Path, now: Option<i64>) -> Option<String> {
    match read_sqlite_store(db) {
        Ok(reply) if is_stale(&reply.expires_at, now) => {
            Some("kiro token expired; run `kiro-cli login`".to_string())
        }
        Ok(_) => None,
        Err(e) => Some(e),
    }
}

/// Answer `getAccessToken` from the store at `db` against clock `now`: the
/// store is re-read on EVERY call, so a mid-session `kiro-cli login` is served
/// on the next request without restarting cyril; a stale token gets an
/// actionable error instead of a known-bad reply. Every failure is warn-logged
/// locally before the JSON-RPC error travels to KAS — KAS surfaces a failed
/// callback as a mute/opaque turn (cyril-l7tw), so the log line is the only
/// cyril-side breadcrumb.
fn get_access_token_from(db: &Path, now: Option<i64>) -> acp::Result<acp::ExtResponse> {
    let reply = read_sqlite_store(db).map_err(|e| {
        tracing::warn!(store = %db.display(), error = %e, "getAccessToken failed: store not servable");
        acp::Error::new(JSONRPC_INTERNAL_ERROR, e)
    })?;
    if is_stale(&reply.expires_at, now) {
        tracing::warn!(store = %db.display(), "getAccessToken refused: kiro token expired or expiring");
        return Err(acp::Error::new(
            JSONRPC_STALE_TOKEN,
            "kiro token expired; run `kiro-cli login`",
        ));
    }
    build_response(&reply).map_err(|e| {
        tracing::warn!(error = %e, "getAccessToken reply serialization failed");
        acp::Error::new(JSONRPC_INTERNAL_ERROR, e)
    })
}

/// Answer `_kiro/auth/getAccessToken` from kiro-cli's sqlite credential store
/// (cyril-dcc6). The stale-policy evolution (a re-login affordance in the UI)
/// is **cyril-taba**; surfacing callback failures as App notifications rather
/// than log lines is **cyril-l7tw**. cyril never extracts or transmits the
/// store's refresh token.
pub(crate) async fn respond_get_access_token() -> acp::Result<acp::ExtResponse> {
    let db = crate::protocol::kas::discovery::default_store_path().ok_or_else(|| {
        tracing::warn!(
            "getAccessToken failed: no home directory to locate the kiro credential store"
        );
        acp::Error::new(
            JSONRPC_INTERNAL_ERROR,
            "no home directory to locate the kiro credential store",
        )
    })?;
    // spawn_blocking: rusqlite is synchronous, and the bridge is a
    // single-threaded runtime whose executor must not stall on I/O.
    tokio::task::spawn_blocking(move || get_access_token_from(&db, now_epoch()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "getAccessToken store-read task failed");
            acp::Error::new(
                JSONRPC_INTERNAL_ERROR,
                format!("credential store read task: {e}"),
            )
        })?
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Build a fixture credential store shaped like kiro-cli's `data.sqlite3`
    /// — including EXTRA rows in both tables, so a first-row-instead-of-keyed-
    /// row implementation fails the fences.
    fn fixture_store(dir: &Path) -> std::path::PathBuf {
        let db = dir.join("data.sqlite3");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE auth_kv (key TEXT PRIMARY KEY, value TEXT);
            CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO auth_kv VALUES ('kirocli:odic:device-registration', '{"unrelated":true}');
            INSERT INTO auth_kv VALUES ('kirocli:odic:token',
              '{"access_token":"AT-sqlite","expires_at":"2026-07-04T03:35:26.917713429Z","refresh_token":"RT-never-read","region":"us-east-1"}');
            INSERT INTO state VALUES ('aaa.first.row', '{"arn":"arn:aws:wrong"}');
            INSERT INTO state VALUES ('api.codewhisperer.profile',
              '{"arn":"arn:aws:codewhisperer:us-east-1:1:profile/X","profile_name":"p"}');
            "#,
        )
        .unwrap();
        db
    }

    // C9 fence: reply fields come from the KEYED sqlite rows — snake_case
    // token fields (the retired file used camelCase), profile arn from the
    // state row, 9-digit sub-second expiry parses via rfc3339_to_epoch.
    #[test]
    fn reply_from_sqlite_rows() {
        let dir = tempfile::tempdir().unwrap();
        let reply = read_sqlite_store(&fixture_store(dir.path())).expect("valid store");
        assert_eq!(reply.access_token.0, "AT-sqlite");
        assert_eq!(reply.expires_at, "2026-07-04T03:35:26.917713429Z");
        assert_eq!(
            reply.profile_arn,
            "arn:aws:codewhisperer:us-east-1:1:profile/X"
        );
        assert!(
            rfc3339_to_epoch(&reply.expires_at).is_some(),
            "9-digit subsecond must parse"
        );
    }

    // C10 fence: the LOGOUT shape — store present, token row deleted — is an
    // actionable error, never an empty/partial reply.
    #[test]
    fn logged_out_row_absent_errors() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        rusqlite::Connection::open(&db)
            .unwrap()
            .execute("DELETE FROM auth_kv WHERE key = 'kirocli:odic:token'", [])
            .unwrap();
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(err.contains("kiro-cli login"), "not actionable: {err}");
    }

    // C11 fence: an absent profile row (or arn key) is an error — a null/empty
    // profileArn reply would 400 at the backend.
    #[test]
    fn missing_profile_arn_errors() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE state SET value = '{\"profile_name\":\"p\"}' WHERE key = 'api.codewhisperer.profile'",
            [],
        )
        .unwrap();
        assert!(read_sqlite_store(&db).unwrap_err().contains("arn"));
        conn.execute(
            "DELETE FROM state WHERE key = 'api.codewhisperer.profile'",
            [],
        )
        .unwrap();
        assert!(read_sqlite_store(&db).unwrap_err().contains("profile"));
    }

    // C12 fence: a missing store errors actionably AND is never created (a
    // default read-write open would create an empty db here).
    #[test]
    fn readonly_never_creates_db() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("data.sqlite3");
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(err.contains("kiro-cli login"), "not actionable: {err}");
        assert!(!db.exists(), "read path CREATED the credential store");
    }

    // C13 fence: the store is re-read per call — a mid-session re-login (row
    // replaced) is served on the next read; a cache-at-startup impl fails.
    #[test]
    fn store_reread_per_callback() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        assert_eq!(read_sqlite_store(&db).unwrap().access_token.0, "AT-sqlite");
        rusqlite::Connection::open(&db)
            .unwrap()
            .execute(
                "UPDATE auth_kv SET value = '{\"access_token\":\"AT-relogin\",\"expires_at\":\"2027-01-01T00:00:00Z\"}' \
                 WHERE key = 'kirocli:odic:token'",
                [],
            )
            .unwrap();
        assert_eq!(read_sqlite_store(&db).unwrap().access_token.0, "AT-relogin");
    }

    // dcc6 review F17a: a store that is garbage BYTES (not a sqlite file at
    // all) errors at the query layer and is never misreported as logged out.
    #[test]
    fn corrupt_store_file_is_not_a_fake_logout() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("data.sqlite3");
        std::fs::write(&db, b"definitely not sqlite").unwrap();
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(
            !err.contains("logged out"),
            "corrupt file misdiagnosed as logout: {err}"
        );
        assert!(err.contains("kiro token"), "names the failing step: {err}");
    }

    // dcc6 review F17a: a valid but SCHEMALESS db (no auth_kv/state tables)
    // errors at the query layer, distinct from the logged-out row shape.
    #[test]
    fn schemaless_store_errors_as_query() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("data.sqlite3");
        drop(rusqlite::Connection::open(&db).unwrap()); // create empty db
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(err.contains("query"), "wrong failure mode: {err}");
        assert!(!err.contains("logged out"), "misdiagnosed: {err}");
    }

    // dcc6 review F17b: the token-row field guards — a row missing
    // `access_token`, or holding an empty `expires_at`, errors naming the
    // field (a partial write / schema drift shape).
    #[test]
    fn token_row_field_guards() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE auth_kv SET value = '{\"expires_at\":\"2027-01-01T00:00:00Z\"}' \
             WHERE key = 'kirocli:odic:token'",
            [],
        )
        .unwrap();
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(err.contains("access_token"), "{err}");
        conn.execute(
            "UPDATE auth_kv SET value = '{\"access_token\":\"AT\",\"expires_at\":\"\"}' \
             WHERE key = 'kirocli:odic:token'",
            [],
        )
        .unwrap();
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(err.contains("expires_at"), "{err}");
    }

    // Executable slice-7 fence (dcc6 review F7c): the retired SSO-cache token
    // path must never be consulted again anywhere in the workspace's crates.
    // The needle is assembled at compile time so this file's source never
    // contains the joined form.
    #[test]
    fn sso_token_path_never_resurrected() {
        fn scan(dir: &Path, needle: &str, hits: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("readable dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    scan(&path, needle, hits);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && std::fs::read_to_string(&path).is_ok_and(|s| s.contains(needle))
                {
                    hits.push(path);
                }
            }
        }
        let needle = concat!("kiro-auth", "-token");
        let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/ parent dir");
        let mut hits = Vec::new();
        scan(crates_root, needle, &mut hits);
        assert!(hits.is_empty(), "SSO token path resurrected in: {hits:?}");
    }

    // Corrupt (unparseable) token row is distinguished from absent — the
    // error names the parse, not a fake logout.
    #[test]
    fn corrupt_token_row_errors_as_parse() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        rusqlite::Connection::open(&db)
            .unwrap()
            .execute(
                "UPDATE auth_kv SET value = 'not json' WHERE key = 'kirocli:odic:token'",
                [],
            )
            .unwrap();
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(
            err.contains("parse"),
            "corrupt collapsed into another mode: {err}"
        );
    }

    // C14a fence: the spawn gate keys on the sqlite store — servable rows
    // pass; the logout shape, a corrupt row, a missing store, AND a stale
    // token (dcc6 review F3) all decline, each with its own diagnostic (F4).
    // No file input exists in this path (the SSO-file gate is structurally
    // gone; the sso_token_path_never_resurrected fence forbids it).
    #[test]
    fn gate_is_sqlite_not_file() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let fresh = Some(fixture_expiry_epoch() - EXPIRY_BUFFER_SECS - 60);
        assert_eq!(store_unservable_reason(&db, fresh), None);
        // Review F3: an EXPIRED login passes row checks but must not pass the
        // gate — it would die on the first callback instead of at spawn.
        let why = store_unservable_reason(&db, Some(fixture_expiry_epoch() + 1))
            .expect("stale token must not pass the gate");
        assert!(why.contains("expired"), "wrong diagnostic: {why}");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE auth_kv SET value = 'corrupt' WHERE key = 'kirocli:odic:token'",
            [],
        )
        .unwrap();
        // Review F4: corrupt is diagnosed as corrupt, not as a fake logout.
        let why = store_unservable_reason(&db, fresh).expect("corrupt row must not pass the gate");
        assert!(why.contains("parse"), "corrupt collapsed: {why}");
        conn.execute("DELETE FROM auth_kv WHERE key = 'kirocli:odic:token'", [])
            .unwrap();
        let why = store_unservable_reason(&db, fresh).expect("logout shape must not pass");
        assert!(why.contains("kiro-cli login"), "not actionable: {why}");
        assert!(store_unservable_reason(&dir.path().join("absent.sqlite3"), fresh).is_some());
    }

    // C11 custodian: neither the AccessToken's nor the AuthReply's Debug leaks
    // the secret, so logging a struct containing it cannot expose the token.
    #[test]
    fn access_token_debug_is_redacted() {
        let reply = AuthReply {
            access_token: AccessToken("SECRET-abc123".to_string()),
            expires_at: "2026-06-22T03:13:22Z".to_string(),
            profile_arn: "arn".to_string(),
        };
        let dbg = format!("{reply:?}");
        assert!(
            !dbg.contains("SECRET-abc123"),
            "token leaked in Debug: {dbg}"
        );
        assert!(dbg.contains("redacted"));
    }

    // rfc3339_to_epoch matches a reference value (cross-checked: `date -u -d
    // 2026-06-22T03:13:22Z +%s` == 1782098002).
    #[test]
    fn rfc3339_epoch_reference() {
        assert_eq!(
            rfc3339_to_epoch("2026-06-22T03:13:22.609Z"),
            Some(1_782_098_002)
        );
        // The unix epoch itself.
        assert_eq!(rfc3339_to_epoch("1970-01-01T00:00:00Z"), Some(0));
        // Malformed layouts -> None.
        assert_eq!(rfc3339_to_epoch("not-a-date"), None);
        assert_eq!(rfc3339_to_epoch("2026/06/22 03:13:22"), None);
        assert_eq!(rfc3339_to_epoch("2026-13-22T03:13:22Z"), None); // month 13
        // A non-UTC offset is rejected (not silently read as UTC) -> fail safe.
        assert_eq!(rfc3339_to_epoch("2026-06-22T03:13:22+02:00"), None);
        assert_eq!(rfc3339_to_epoch("2026-06-22T03:13:22"), None); // no zone
    }

    // C9 (the deterministic part): the stale boundary is exactly now + buffer.
    #[test]
    fn is_stale_boundary() {
        let exp = "2026-06-22T03:13:22Z"; // epoch 1_782_098_002
        let base = 1_782_098_002;
        // Expiring exactly at now+buffer is STALE (<=), one second later is fresh.
        assert!(is_stale(exp, Some(base - EXPIRY_BUFFER_SECS)));
        assert!(!is_stale(exp, Some(base - EXPIRY_BUFFER_SECS - 1)));
        // Already past -> stale; far future -> fresh.
        assert!(is_stale(exp, Some(base + 10)));
        assert!(!is_stale(exp, Some(base - 100_000)));
        // Unparseable expiry -> stale (fail safe).
        assert!(is_stale("garbage", Some(base)));
        // Unreadable clock -> stale (fail safe), never reported fresh.
        assert!(is_stale(exp, None));
    }

    /// Epoch of the fixture store's `expires_at` — the responder tests pin
    /// `now` relative to this so they never race the real clock.
    fn fixture_expiry_epoch() -> i64 {
        rfc3339_to_epoch("2026-07-04T03:35:26.917713429Z").expect("fixture expiry parses")
    }

    // dcc6 review F2: the COMPOSED responder path — a fresh store serves the
    // 3-key reply (the pieces were fenced; the glue was not).
    #[test]
    fn responder_serves_fresh_store() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let now = fixture_expiry_epoch() - EXPIRY_BUFFER_SECS - 60;
        let resp = get_access_token_from(&db, Some(now)).expect("fresh store serves");
        let v: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        assert_eq!(v["accessToken"], "AT-sqlite");
        assert_eq!(
            v["profileArn"],
            "arn:aws:codewhisperer:us-east-1:1:profile/X"
        );
    }

    // dcc6 review F2: a stale token is REFUSED with the stale code and the
    // actionable re-login hint — deleting/inverting the is_stale gate fails here.
    #[test]
    fn responder_refuses_stale_token() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let err = get_access_token_from(&db, Some(fixture_expiry_epoch() + 1)).unwrap_err();
        assert_eq!(err.code, acp::Error::new(JSONRPC_STALE_TOKEN, "").code);
        assert!(
            err.message.contains("kiro-cli login"),
            "not actionable: {}",
            err.message
        );
    }

    // dcc6 review F2: a store failure maps to the INTERNAL code — distinct
    // from the stale code, so KAS-side triage can tell the modes apart.
    #[test]
    fn responder_store_error_is_internal() {
        let dir = tempfile::tempdir().unwrap();
        let err = get_access_token_from(&dir.path().join("absent.sqlite3"), Some(0)).unwrap_err();
        assert_eq!(err.code, acp::Error::new(JSONRPC_INTERNAL_ERROR, "").code);
        assert!(err.message.contains("kiro-cli login"), "{}", err.message);
    }

    // build_response emits exactly the three camelCase keys KAS validates.
    #[test]
    fn build_response_has_three_camel_case_keys() {
        let reply = AuthReply {
            access_token: AccessToken("AT".to_string()),
            expires_at: "2026-06-22T03:13:22Z".to_string(),
            profile_arn: "arn:aws:x".to_string(),
        };
        let resp = build_response(&reply).unwrap();
        let v: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        assert_eq!(v["accessToken"], "AT");
        assert_eq!(v["expiresAt"], "2026-06-22T03:13:22Z");
        assert_eq!(v["profileArn"], "arn:aws:x");
    }
}

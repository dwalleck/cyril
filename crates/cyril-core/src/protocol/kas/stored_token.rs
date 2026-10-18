//! Stored-token FALLBACK for the KAS auth responder (cyril-zbu7 amendment,
//! 2026-10-18; reader from cyril-dcc6 / cyril-y14u).
//!
//! Used ONLY when the bound kiro-cli lacks the hidden `chat _ get-kas-token`
//! subcommand (clap's unrecognized-subcommand / unexpected-argument refusal —
//! [`super::auth`] detects it precisely and caches it per process). Every
//! other helper failure stays a hard error and never reaches this module.
//! The stored token is never renewed here: a KAS session on such a kiro-cli
//! expires with its access token (~1 h), and the user is warned once.
//!
//! The source is kiro-cli's **sqlite credential store** (`data.sqlite3`), read
//! read-only; cyril is a custodian of the credential: held for one reply,
//! redacted in `Debug`, never logged, and the store's refresh token is never
//! extracted or transmitted (the row containing it is read whole; only the
//! access fields are taken from it).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::auth::{AccessToken, KasToken, LOGIN_HINT};

/// KAS rejects a token within this pre-expiry window (it validates
/// `expiresAt > now + ~3min`), so the fallback treats such a token as stale.
const EXPIRY_BUFFER_SECS: i64 = 180;

/// Why the stored token cannot be served.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum StoredTokenError {
    /// The store is absent/locked/corrupt, logged out, or missing a field;
    /// the string is the precise diagnostic (never the token).
    Unservable(String),
    /// The stored token is expired or inside KAS's pre-expiry buffer.
    Stale,
}

/// Read the stored token from the store at `db` and refuse it when stale
/// against clock `now` (`None` = unreadable clock → stale, fail safe).
pub(super) fn read_stored_token(db: &Path, now: Option<i64>) -> Result<KasToken, StoredTokenError> {
    let reply = read_sqlite_store(db).map_err(StoredTokenError::Unservable)?;
    if is_stale(&reply.expires_at, now) {
        return Err(StoredTokenError::Stale);
    }
    Ok(KasToken::from_store(
        reply.access_token,
        reply.expires_at,
        reply.profile_arn,
    ))
}

/// The validated stored-token payload.
#[derive(Debug)]
struct AuthReply {
    access_token: AccessToken,
    expires_at: String,
    profile_arn: String,
}
/// The credential-store rows kiro-cli maintains in `data.sqlite3`. The token
/// row is PER LOGIN METHOD (cyril-y14u): Builder-ID/IdC logins write
/// `kirocli:odic:token`, social logins (`accountType: SocialGitHub/…`) write
/// `kirocli:social:token` — both snake_case `{access_token, expires_at, …}`,
/// deleted on logout; the social row additionally carries an inline
/// `profile_arn`. The active CodeWhisperer profile JSON (`{arn,
/// profile_name}`) lives in the `state` table and is preferred for the arn
/// (the odic token row stopped carrying `profile_arn`); the social row's
/// inline arn is the fallback when no state profile row exists.
const TOKEN_ROW_SQL: &str = "SELECT value FROM auth_kv WHERE key = 'kirocli:odic:token'";
const SOCIAL_TOKEN_ROW_SQL: &str = "SELECT value FROM auth_kv WHERE key = 'kirocli:social:token'";
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
            "kiro credential store {} is absent; {LOGIN_HINT}",
            db.display()
        ));
    }
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("open kiro credential store: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(250))
        .map_err(|e| format!("set kiro credential store busy timeout: {e}"))?;
    let opt_row = |sql: &str, what: &str| -> Result<Option<serde_json::Value>, String> {
        use rusqlite::OptionalExtension;
        conn.query_row(sql, [], |r| r.get::<_, String>(0))
            .optional()
            .map_err(|e| format!("query {what}: {e}"))?
            .map(|raw| serde_json::from_str(&raw).map_err(|e| format!("parse {what} row: {e}")))
            .transpose()
    };
    // Per-login-method token rows (cyril-y14u): read both; absent-both is the
    // logged-out shape. Both present (a login-method switch leftover): the
    // freshest expiry wins — a stale leftover must not shadow the live
    // credential, and an unparseable expiry loses to a parseable one
    // (None < Some, fail safe).
    let odic = opt_row(TOKEN_ROW_SQL, "kiro token")?;
    let social = opt_row(SOCIAL_TOKEN_ROW_SQL, "kiro social token")?;
    let token = match (odic, social) {
        (None, None) => {
            return Err(format!(
                "kiro token row absent (no builder-id or social login) — logged out; {LOGIN_HINT}"
            ));
        }
        (Some(t), None) | (None, Some(t)) => t,
        (Some(o), Some(s)) => {
            // Log-before-None: a PRESENT but unparseable expiry is corrupt,
            // not missing — without the breadcrumb, a malformed live
            // credential silently losing the tiebreak is undiagnosable.
            let exp = |t: &serde_json::Value, which: &str| {
                let stamp = t.get("expires_at").and_then(|x| x.as_str());
                let parsed = stamp.and_then(rfc3339_to_epoch);
                if stamp.is_some() && parsed.is_none() {
                    tracing::debug!(
                        row = which,
                        "token-row tiebreak: expires_at present but unparseable; row loses"
                    );
                }
                parsed
            };
            // Equal or both-unparseable expiries prefer builder-id — the
            // pre-y14u behavior, kept deterministic rather than meaningful.
            let (chosen, name) = if exp(&s, "social") > exp(&o, "builder-id") {
                (s, "social")
            } else {
                (o, "builder-id")
            };
            tracing::debug!(
                chosen = name,
                "both kiro token rows present; serving the freshest expiry"
            );
            chosen
        }
    };
    let field = |k: &str| -> Result<String, String> {
        match token.get(k).and_then(|x| x.as_str()) {
            Some(s) if !s.is_empty() => Ok(s.to_string()),
            _ => Err(format!("kiro token row missing `{k}`")),
        }
    };
    let access_token = AccessToken(field("access_token")?);
    let expires_at = field("expires_at")?;
    // Load-bearing: a reply with an absent/empty arn 400s at the backend
    // ("profileArn is required"), so it is an error, not a default. The state
    // row is kiro's ACTIVE profile and wins; the token row's inline
    // `profile_arn` (social logins) is the fallback.
    let state_arn = opt_row(PROFILE_ROW_SQL, "kiro profile")?
        .and_then(|p| p.get("arn").and_then(|x| x.as_str()).map(str::to_string))
        .filter(|s| !s.is_empty());
    let inline_arn = token
        .get("profile_arn")
        .and_then(|x| x.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty());
    let profile_arn = state_arn.or(inline_arn).ok_or_else(|| {
        "kiro profile row missing `arn` (and no token-inline profile_arn)".to_string()
    })?;
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
/// forwarded). An unparseable expiry is warn-logged (dcc6 re-review N2): the
/// caller-facing outcome says "expired; re-login", which for a kiro-cli
/// serialization drift would be a permanent loop re-login can't fix — the log
/// line is the only clue the stamp, not the token, is the problem.
fn is_stale(expires_at: &str, now_epoch: Option<i64>) -> bool {
    match (now_epoch, rfc3339_to_epoch(expires_at)) {
        (Some(now), Some(exp)) => exp <= now + EXPIRY_BUFFER_SECS,
        (_, None) => {
            tracing::warn!(
                expires_at,
                "unparseable kiro token expiry; treating as stale"
            );
            true
        }
        (None, Some(_)) => true, // broken clock — now_epoch already warned
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

    // cyril-y14u fence: a SOCIAL login (kirocli:social:token, no odic row) is
    // served — shape verified against a live SocialGitHub store 2026-08-02.
    // Fails against the pre-y14u reader, which knew only the odic row and
    // reported a logged-in user as logged out.
    #[test]
    fn social_login_row_is_served() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute("DELETE FROM auth_kv WHERE key = 'kirocli:odic:token'", [])
            .unwrap();
        conn.execute(
            "INSERT INTO auth_kv VALUES ('kirocli:social:token',
              '{\"access_token\":\"AT-social\",\"expires_at\":\"2026-07-04T03:35:26.765158732Z\",\"refresh_token\":\"RT-never-read\",\"provider\":\"github\",\"profile_arn\":\"arn:aws:codewhisperer:us-east-1:1:profile/inline\"}')",
            [],
        )
        .unwrap();
        let reply = read_sqlite_store(&db).expect("social login is servable");
        assert_eq!(reply.access_token.0, "AT-social");
        // The state profile row is kiro's ACTIVE profile and wins over the
        // token-inline arn when both exist.
        assert_eq!(
            reply.profile_arn,
            "arn:aws:codewhisperer:us-east-1:1:profile/X"
        );
    }

    // cyril-y14u fence: the state profile row absent → the social row's
    // INLINE profile_arn is the fallback (a social-only machine may have no
    // state profile row). An empty inline arn on top of no state row is still
    // an error — never an empty profileArn (400 at the backend).
    #[test]
    fn social_inline_arn_is_the_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute("DELETE FROM auth_kv WHERE key = 'kirocli:odic:token'", [])
            .unwrap();
        conn.execute(
            "DELETE FROM state WHERE key = 'api.codewhisperer.profile'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO auth_kv VALUES ('kirocli:social:token',
              '{\"access_token\":\"AT-social\",\"expires_at\":\"2026-07-04T03:35:26Z\",\"profile_arn\":\"arn:aws:codewhisperer:us-east-1:1:profile/inline\"}')",
            [],
        )
        .unwrap();
        let reply = read_sqlite_store(&db).expect("inline arn serves");
        assert_eq!(
            reply.profile_arn,
            "arn:aws:codewhisperer:us-east-1:1:profile/inline"
        );

        conn.execute(
            "UPDATE auth_kv SET value = '{\"access_token\":\"AT\",\"expires_at\":\"2026-07-04T03:35:26Z\"}'
             WHERE key = 'kirocli:social:token'",
            [],
        )
        .unwrap();
        let err = read_sqlite_store(&db).unwrap_err();
        assert!(err.contains("arn"), "empty-arn reply forbidden: {err}");
    }

    // cyril-y14u fence: BOTH token rows present (login-method switch
    // leftover) → the freshest expiry is served; a stale leftover must not
    // shadow the live credential. Designed to fail under prefer-odic-always.
    #[test]
    fn freshest_token_row_wins_when_both_present() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path()); // odic expires 2026-07-04
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO auth_kv VALUES ('kirocli:social:token',
              '{\"access_token\":\"AT-social-fresher\",\"expires_at\":\"2027-01-01T00:00:00Z\"}')",
            [],
        )
        .unwrap();
        let reply = read_sqlite_store(&db).expect("servable");
        assert_eq!(reply.access_token.0, "AT-social-fresher");

        conn.execute(
            "UPDATE auth_kv SET value = '{\"access_token\":\"AT-social-stale\",\"expires_at\":\"2020-01-01T00:00:00Z\"}'
             WHERE key = 'kirocli:social:token'",
            [],
        )
        .unwrap();
        let reply = read_sqlite_store(&db).expect("servable");
        assert_eq!(
            reply.access_token.0, "AT-sqlite",
            "the stale social leftover must not shadow the live odic row"
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

    // The composed fallback read: a fresh stored token is served (store
    // fields carried over), a stale one is `Stale`, a broken store is
    // `Unservable` with its diagnostic — never a known-bad token.
    #[test]
    fn read_stored_token_serves_fresh_and_refuses_stale() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_store(dir.path());
        let fresh = Some(fixture_expiry_epoch() - EXPIRY_BUFFER_SECS - 60);
        let token = read_stored_token(&db, fresh).expect("fresh store serves");
        assert_eq!(token.expires_at(), "2026-07-04T03:35:26.917713429Z");
        // Exactly at the buffer edge is stale (KAS would reject it).
        assert_eq!(
            read_stored_token(&db, Some(fixture_expiry_epoch() - EXPIRY_BUFFER_SECS)).unwrap_err(),
            StoredTokenError::Stale
        );
        let absent = read_stored_token(&dir.path().join("absent.sqlite3"), fresh).unwrap_err();
        assert!(
            matches!(&absent, StoredTokenError::Unservable(why) if why.contains("kiro-cli login")),
            "{absent:?}"
        );
    }
}

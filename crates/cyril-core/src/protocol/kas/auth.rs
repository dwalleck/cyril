//! KAS auth responder (KAS-1 Part B, cyril-evwh).
//!
//! Answers the `_kiro/auth/getAccessToken` server→client request that KAS sends
//! in **wrapper** mode (`--auth=acp-callback`) by reading kiro-cli's own tier-5
//! token file. cyril is a **custodian** of that credential: the token is
//! read-only, held only for the duration of one reply, redacted in `Debug`, and
//! never logged. cyril does NOT reimplement OIDC refresh — it delegates to the
//! file kiro-cli/KAS maintain. The free path needs none of this (KAS reads the
//! file itself); this is for the wrapper lifecycle + non-file-refreshing setups.

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

/// A redacted access-token wrapper: its `Debug` never prints the secret, so a
/// stray `{:?}` or a tracing of any struct containing it cannot leak the
/// credential (spec SC4 custodian).
#[derive(Clone)]
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

/// Parse `~/.aws/sso/cache/kiro-auth-token.json` into a reply. Returns `Err`
/// (the diagnostic, never the token) if the file is missing/unparseable or ANY
/// of the three required fields is absent or empty. `profileArn` is load-bearing
/// — KAS 400s "profileArn is required" — so an absent field is an error, not a
/// silent empty default.
fn read_token_file(path: &Path) -> Result<AuthReply, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read kiro token file: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse kiro token file: {e}"))?;
    let field = |k: &str| -> Result<String, String> {
        match v.get(k).and_then(|x| x.as_str()) {
            Some(s) if !s.is_empty() => Ok(s.to_string()),
            _ => Err(format!("kiro token file missing `{k}`")),
        }
    };
    Ok(AuthReply {
        access_token: AccessToken(field("accessToken")?),
        expires_at: field("expiresAt")?,
        profile_arn: field("profileArn")?,
    })
}

/// Parse an RFC3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SS[.fff]Z`) to unix epoch
/// seconds; sub-second precision and the trailing `Z` are ignored (second
/// precision suffices for a 180s buffer). Strict on layout — returns `None` on
/// any shape mismatch, which [`is_stale`] treats as stale (fail safe). Uses the
/// canonical days-from-civil algorithm (Howard Hinnant).
fn rfc3339_to_epoch(s: &str) -> Option<i64> {
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
/// reject it), or when its timestamp can't be parsed (fail safe). Pure.
fn is_stale(expires_at: &str, now_epoch: i64) -> bool {
    match rfc3339_to_epoch(expires_at) {
        Some(exp) => exp <= now_epoch + EXPIRY_BUFFER_SECS,
        None => true,
    }
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

/// Answer `_kiro/auth/getAccessToken` from kiro-cli's own token file. On a stale
/// token returns an actionable error instead of a known-bad reply; automatic
/// refresh-on-stale is deferred to **cyril-taba** (the token was fresh
/// throughout the KAS-1 build, so the stale path can't be live-verified yet).
pub(crate) fn respond_get_access_token() -> acp::Result<acp::ExtResponse> {
    let path = crate::protocol::kas::discovery::default_token_path().ok_or_else(|| {
        acp::Error::new(-32603, "no home directory to locate the kiro token file")
    })?;
    let reply = read_token_file(&path).map_err(|e| acp::Error::new(-32603, e))?;
    if is_stale(&reply.expires_at, now_epoch()) {
        return Err(acp::Error::new(
            -32000,
            "kiro token expired; run `kiro-cli login` (auto-refresh: cyril-taba)",
        ));
    }
    build_response(&reply).map_err(|e| acp::Error::new(-32603, e))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn write_token(dir: &Path, body: &str) -> std::path::PathBuf {
        let p = dir.join("kiro-auth-token.json");
        std::fs::write(&p, body).unwrap();
        p
    }

    // C8/C10: a well-formed token file yields all three fields.
    #[test]
    fn read_token_file_extracts_three_fields() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_token(
            dir.path(),
            r#"{"accessToken":"AT","expiresAt":"2026-06-22T03:13:22.609Z","profileArn":"arn:aws:codewhisperer:::profile/X","refreshToken":"RT","provider":"Github"}"#,
        );
        let reply = read_token_file(&p).expect("valid token file");
        assert_eq!(reply.access_token.0, "AT");
        assert_eq!(reply.expires_at, "2026-06-22T03:13:22.609Z");
        assert!(reply.profile_arn.starts_with("arn:aws:"));
    }

    // C10 (the load-bearing field): a token file missing profileArn is an Err,
    // NOT a reply with an empty profileArn (which would 400 at KAS).
    #[test]
    fn read_token_file_missing_profile_arn_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_token(
            dir.path(),
            r#"{"accessToken":"AT","expiresAt":"2026-06-22T03:13:22Z"}"#,
        );
        let err = read_token_file(&p).unwrap_err();
        assert!(err.contains("profileArn"), "got {err}");
    }

    // An empty-string field is also "missing".
    #[test]
    fn read_token_file_empty_field_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_token(
            dir.path(),
            r#"{"accessToken":"","expiresAt":"2026-06-22T03:13:22Z","profileArn":"arn"}"#,
        );
        assert!(read_token_file(&p).unwrap_err().contains("accessToken"));
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
    // 2026-06-22T03:13:22Z +%s` == 1781061202).
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
    }

    // C9 (the deterministic part): the stale boundary is exactly now + buffer.
    #[test]
    fn is_stale_boundary() {
        let exp = "2026-06-22T03:13:22Z"; // epoch 1_782_098_002
        let base = 1_782_098_002;
        // Expiring exactly at now+buffer is STALE (<=), one second later is fresh.
        assert!(is_stale(exp, base - EXPIRY_BUFFER_SECS));
        assert!(!is_stale(exp, base - EXPIRY_BUFFER_SECS - 1));
        // Already past -> stale; far future -> fresh.
        assert!(is_stale(exp, base + 10));
        assert!(!is_stale(exp, base - 100_000));
        // Unparseable -> stale (fail safe).
        assert!(is_stale("garbage", base));
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

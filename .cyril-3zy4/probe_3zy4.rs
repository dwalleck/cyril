//! Probe (cyril-3zy4): does the KAS-dialect `_kiro/error/rate_limit` route to
//! `Notification::RateLimited`, or is it dropped by the unknown-method arm?
//!
//! Wire-naming (verified in source):
//!   - ACP crate strips the single leading `_` inbound (kas/auth.rs:20-24).
//!   - `_kiro/error/rate_limit` (KAS)  -> arrives as `kiro/error/rate_limit`
//!   - `_kiro.dev/error/rate_limit` (v2) -> arrives as `kiro.dev/error/rate_limit`
//!   - KasEngine delegates ext frames to kiro::to_ext_notification (engine.rs:143)
//!
//! Run: cargo test -p cyril-core --test probe_3zy4 -- --nocapture
//! Expected BEFORE fix: KAS-dialect assertion FAILS (routes to Ok(None)).

#[cfg(feature = "kas")]
use cyril_core::protocol::engine::{AgentEngine, KasEngine};
use cyril_core::types::Notification;

const KAS_PAYLOAD: &str = r#"{"message": "Rate limit exceeded. Please wait a moment before trying again."}"#;

#[test]
fn kas_dialect_rate_limit_is_dropped() {
    // The acp-stripped KAS method name.
    let params: serde_json::Value = serde_json::from_str(KAS_PAYLOAD).unwrap();
    let r = cyril_core::protocol::convert::kiro::to_ext_notification(
        "kiro/error/rate_limit",
        &params,
    );
    println!("KAS dialect `kiro/error/rate_limit` -> {r:?}");
    assert!(
        matches!(r, Ok(Some(Notification::RateLimited { .. }))),
        "KAS-dialect rate_limit must convert to RateLimited, got {r:?}"
    );
}

#[test]
fn v2_dialect_rate_limit_still_converts() {
    // Control: the legacy arm must keep working (no regression).
    let params: serde_json::Value = serde_json::from_str(KAS_PAYLOAD).unwrap();
    let r = cyril_core::protocol::convert::kiro::to_ext_notification(
        "kiro.dev/error/rate_limit",
        &params,
    );
    println!("v2 dialect `kiro.dev/error/rate_limit` -> {r:?}");
    assert!(
        matches!(r, Ok(Some(Notification::RateLimited { .. }))),
        "v2-dialect rate_limit must keep converting, got {r:?}"
    );
}

#[cfg(feature = "kas")]
#[test]
fn kas_engine_routes_rate_limit() {
    // Engine-level: KasEngine must not drop the frame (engine.rs:143 delegates).
    let params: serde_json::Value = serde_json::from_str(KAS_PAYLOAD).unwrap();
    let r = KasEngine.convert_ext_notification("kiro/error/rate_limit", &params);
    println!("KasEngine `kiro/error/rate_limit` -> {r:?}");
    assert!(
        matches!(r, Ok(Some(Notification::RateLimited { .. }))),
        "KasEngine must route rate_limit to RateLimited, got {r:?}"
    );
}

//! KAS (`_kiro/*`) `session/update` specifics — the KAS analogue of
//! [`super::kiro`] for the v2 dialect. KAS rides standard ACP `session/update`
//! frames whose KAS-specific payload lives entirely in `_meta.kiro`.
//!
//! KAS-2a (cyril-j16p) Slice 1: recognise the `turn_end` lifecycle frame — the
//! signal that drives turn completion under KAS, in place of v2's prompt
//! response — and map it to [`Notification::TurnCompleted`].

use agent_client_protocol as acp;

use crate::types::{ContextBreakdown, ContextBucket, Notification, StopReason};

/// Convert a KAS `session_info_update` to an internal notification.
///
/// KAS multiplexes turn lifecycle, metering, and context telemetry through one
/// `session_info_update` envelope, discriminated by `_meta.kiro.kind`. Two
/// sub-kinds surface today:
/// - **`turn_end`** — the terminal lifecycle signal → [`Notification::TurnCompleted`]
///   (KAS-2a), stop reason from `_meta.kiro.stopReason`.
/// - **`context_usage`** — the proactively-pushed per-category breakdown
///   (KAS-2b, cyril-5et2) → [`Notification::ContextBreakdownUpdated`].
///
/// Every other sub-kind (`turn_completion` metering, `user_message_id_assigned`,
/// …) returns `None`. Completion keys on the `kind == "turn_end"` value, never on
/// frame ordering — a `context_usage` frame trails `turn_end` on the wire.
///
/// A `turn_end` whose `_meta.kiro.stopReason` is missing or unparseable still
/// completes the turn (defaults [`StopReason::EndTurn`]): silently returning
/// `None` would strand the UI in the busy state forever, so this is a runtime
/// fallback that survives release builds, not a `debug_assert!`.
pub(crate) fn session_info_to_notification(siu: &acp::SessionInfoUpdate) -> Option<Notification> {
    let kiro = siu.meta.as_ref()?.get("kiro")?;
    match kiro.get("kind").and_then(serde_json::Value::as_str) {
        Some("turn_end") => Some(Notification::TurnCompleted {
            stop_reason: turn_end_stop_reason(kiro),
        }),
        Some("context_usage") => {
            // A context_usage frame missing its required `usagePercentage` carries
            // nothing to show → drop (unlike turn_end, which must complete). When
            // present, ALWAYS return Some even if the breakdown is absent/malformed
            // — the scalar `Context: N%` must still update (the bars retain-last in
            // UiState). No unwrap; a malformed breakdown degrades to `None`.
            let usage_percentage = kiro
                .get("usagePercentage")
                .and_then(serde_json::Value::as_f64)?;
            Some(Notification::ContextBreakdownUpdated {
                usage_percentage,
                breakdown: parse_breakdown(kiro.get("breakdown")),
            })
        }
        _ => None,
    }
}

/// Stop reason for a `turn_end` frame, defaulting [`StopReason::EndTurn`] when
/// `_meta.kiro.stopReason` is missing or unparseable (a dropped turn_end would
/// strand the UI busy forever — a runtime fallback, not a `debug_assert!`).
fn turn_end_stop_reason(kiro: &serde_json::Value) -> StopReason {
    let raw_stop_reason = kiro.get("stopReason");
    raw_stop_reason
        .and_then(serde_json::Value::as_str)
        .and_then(|s| {
            serde_json::from_value::<acp::StopReason>(serde_json::Value::String(s.to_owned())).ok()
        })
        .map(super::to_stop_reason)
        .unwrap_or_else(|| {
            // Distinguish "missing" from "corrupt" (CLAUDE.md): log the offending
            // value (`None` = absent, `Some(..)` = present-but-unparseable) so a
            // wire drift is diagnosable, not hidden behind a generic message.
            tracing::warn!(
                stop_reason = ?raw_stop_reason,
                "KAS turn_end `_meta.kiro.stopReason` missing or unparseable; defaulting to EndTurn"
            );
            StopReason::EndTurn
        })
}

/// Parse the `_meta.kiro.breakdown` object into a [`ContextBreakdown`]. Returns
/// `None` (treated as "no breakdown this frame") if the object is absent or any
/// of the five named buckets is missing/malformed — never an error, never a
/// panic. O(1): five fixed buckets.
fn parse_breakdown(breakdown: Option<&serde_json::Value>) -> Option<ContextBreakdown> {
    let bd = breakdown?;
    Some(ContextBreakdown::new(
        parse_bucket(bd.get("contextFiles"))?,
        parse_bucket(bd.get("sessionFiles"))?,
        parse_bucket(bd.get("tools"))?,
        parse_bucket(bd.get("yourPrompts"))?,
        parse_bucket(bd.get("kiroResponses"))?,
    ))
}

/// Parse one breakdown bucket `{tokens, percent}`. `None` if absent or either
/// field is missing/the wrong type — so a malformed bucket degrades the whole
/// breakdown to absent rather than fabricating a sentinel zero.
fn parse_bucket(bucket: Option<&serde_json::Value>) -> Option<ContextBucket> {
    let b = bucket?;
    let tokens = b.get("tokens").and_then(serde_json::Value::as_u64)?;
    let percent = b.get("percent").and_then(serde_json::Value::as_f64)?;
    Some(ContextBucket::new(tokens, percent))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::collections::HashMap;
    use std::path::Path;

    use serde_json::json;

    use super::*;
    use crate::protocol::engine::{Engine, KasEngine};

    /// Deserialize a captured fixture into a `SessionNotification` — the exact
    /// layer the acp Client parses a `session/update` at (mirrors the
    /// `schema_deserializes_captured_kas_session_updates` loader in `mod.rs`).
    fn load(name: &str) -> (serde_json::Value, acp::SessionNotification) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kas")
            .join(name);
        let raw = std::fs::read_to_string(&path).expect("read fixture");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("fixture is JSON");
        let parsed: acp::SessionNotification =
            serde_json::from_value(value.clone()).expect("fixture deserializes");
        (value, parsed)
    }

    fn info_update(sn: &acp::SessionNotification) -> &acp::SessionInfoUpdate {
        match &sn.update {
            acp::SessionUpdate::SessionInfoUpdate(siu) => siu,
            other => panic!("fixture is not a session_info_update: {other:?}"),
        }
    }

    #[test]
    fn turn_end_maps_to_turn_completed_endturn() {
        let (value, sn) = load("session_info_update_turn_end.json");
        let result = session_info_to_notification(info_update(&sn));

        // Oracle: the converter reads the FLAT `_meta.kiro.stopReason`; this
        // independently reads the MIRRORED `_meta.kiro.turnEnd.stopReason` path
        // (the capture showed they agree) and maps it via the acp deserializer.
        let mirrored = value["update"]["_meta"]["kiro"]["turnEnd"]["stopReason"]
            .as_str()
            .expect("mirrored stopReason");
        let oracle = super::super::to_stop_reason(
            serde_json::from_value::<acp::StopReason>(json!(mirrored)).expect("oracle parses"),
        );
        assert_eq!(oracle, StopReason::EndTurn, "oracle precondition");
        assert!(
            matches!(result, Some(Notification::TurnCompleted { stop_reason }) if stop_reason == oracle),
            "turn_end must map to TurnCompleted with the mirrored stop reason, got {result:?}"
        );
    }

    #[test]
    fn turn_completion_metering_is_not_a_turn_end() {
        // Guards confusing metering for completion — the exact ambiguity the
        // cheapest-falsifier resolved (turn_completion fires BEFORE turn_end).
        let (_v, sn) = load("session_info_update_turn_completion.json");
        assert!(session_info_to_notification(info_update(&sn)).is_none());
    }

    #[test]
    fn other_sub_kind_is_ignored() {
        // user_message_id_assigned — guards "every session_info_update is a turn end".
        let (_v, sn) = load("session_info_update.json");
        assert!(session_info_to_notification(info_update(&sn)).is_none());
    }

    fn context_usage_frame(kiro: serde_json::Value) -> acp::SessionNotification {
        serde_json::from_value(json!({
            "sessionId": "sess_x",
            "update": { "sessionUpdate": "session_info_update", "_meta": { "kiro": kiro } }
        }))
        .expect("frame deserializes")
    }

    #[test]
    fn context_usage_maps_breakdown() {
        // Slice 3 / claim C1. The real 2.10.0 frame maps to ContextBreakdownUpdated
        // with the 5 buckets' exact tokens/percent. Expected values are the
        // independent jq oracle's (.cyril-5et2/oracle.sh on the same fixture).
        let (_v, sn) = load("session_info_update_context_usage.json");
        let result = session_info_to_notification(info_update(&sn));
        let Some(Notification::ContextBreakdownUpdated {
            usage_percentage,
            breakdown,
        }) = result
        else {
            panic!("expected ContextBreakdownUpdated, got {result:?}");
        };
        assert!((usage_percentage - 4.3).abs() < f64::EPSILON);
        let bd = breakdown.expect("breakdown present");
        for (bucket, tokens, percent) in [
            (bd.context_files(), 0u64, 0.0),
            (bd.tools(), 4662, 2.3),
            (bd.your_prompts(), 4096, 2.0),
            (bd.kiro_responses(), 0, 0.0),
            (bd.session_files(), 0, 0.0),
        ] {
            assert_eq!(bucket.tokens(), tokens);
            assert!((bucket.percent() - percent).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn context_usage_reads_flat_usage_not_nested() {
        // Slice 3 / claim C2. Flat `_meta.kiro.usagePercentage` (9.9) wins over the
        // nested `contextUsage.usagePercentage` (1.1). Fails if the converter reads
        // the nested wrapper.
        let sn = context_usage_frame(json!({
            "kind": "context_usage",
            "usagePercentage": 9.9,
            "contextUsage": { "usagePercentage": 1.1 }
        }));
        let result = session_info_to_notification(info_update(&sn));
        let Some(Notification::ContextBreakdownUpdated {
            usage_percentage, ..
        }) = result
        else {
            panic!("expected ContextBreakdownUpdated, got {result:?}");
        };
        assert!(
            (usage_percentage - 9.9).abs() < f64::EPSILON,
            "got {usage_percentage}"
        );
    }

    #[test]
    fn context_usage_breakdown_absent_still_carries_scalar() {
        // Slice 3 / claim C3. No `breakdown` key → Some with breakdown None, scalar
        // intact. Fails under `breakdown.unwrap()` or returning None (which would
        // drop the % update and freeze the toolbar).
        let sn = context_usage_frame(json!({ "kind": "context_usage", "usagePercentage": 12.5 }));
        let result = session_info_to_notification(info_update(&sn));
        assert!(
            matches!(
                result,
                Some(Notification::ContextBreakdownUpdated { usage_percentage, breakdown: None })
                    if (usage_percentage - 12.5).abs() < f64::EPSILON
            ),
            "got {result:?}"
        );
    }

    #[test]
    fn context_usage_malformed_breakdown_degrades_to_none() {
        // Slice 3 / claim C3. A breakdown missing a bucket (here `tools`) degrades
        // the whole breakdown to None — never a fabricated sentinel-zero bucket —
        // while the scalar still updates.
        let sn = context_usage_frame(json!({
            "kind": "context_usage", "usagePercentage": 3.0,
            "breakdown": {
                "contextFiles": { "tokens": 0, "percent": 0 },
                "sessionFiles": { "tokens": 0, "percent": 0 },
                "yourPrompts": { "tokens": 1, "percent": 1 },
                "kiroResponses": { "tokens": 0, "percent": 0 }
                // tools missing
            }
        }));
        let result = session_info_to_notification(info_update(&sn));
        assert!(
            matches!(
                result,
                Some(Notification::ContextBreakdownUpdated {
                    breakdown: None,
                    ..
                })
            ),
            "malformed breakdown must degrade to None, got {result:?}"
        );
    }

    #[test]
    fn turn_end_without_stop_reason_still_completes() {
        // Load-bearing fallback: a turn_end missing stopReason must NOT be
        // dropped (that strands the UI busy) — defaults EndTurn.
        let value = json!({
            "sessionId": "sess_x",
            "update": { "sessionUpdate": "session_info_update",
                        "_meta": { "kiro": { "kind": "turn_end" } } }
        });
        let sn: acp::SessionNotification = serde_json::from_value(value).unwrap();
        assert!(matches!(
            session_info_to_notification(info_update(&sn)),
            Some(Notification::TurnCompleted {
                stop_reason: StopReason::EndTurn
            })
        ));
    }

    #[test]
    fn kas_engine_routes_turn_end_to_completion() {
        let (_v, sn) = load("session_info_update_turn_end.json");
        let n = KasEngine.convert_session_update(&sn, &HashMap::new());
        assert!(matches!(
            n,
            Some(Notification::TurnCompleted {
                stop_reason: StopReason::EndTurn
            })
        ));
    }

    #[test]
    fn kas_engine_still_renders_agent_text() {
        // Slice 1 must NOT break text rendering: non-turn_end updates delegate
        // to the generic converter (agent_message_chunk -> AgentMessage).
        let (_v, sn) = load("agent_message_chunk.json");
        let n = KasEngine.convert_session_update(&sn, &HashMap::new());
        assert!(
            matches!(n, Some(Notification::AgentMessage(_))),
            "agent_message_chunk must still render via the generic path, got {n:?}"
        );
    }

    #[test]
    fn kas_engine_drops_unknown_ext_frame() {
        // KAS-2a (cyril-j16p) Slice 3 — unknown-variant tolerance: an
        // unrecognised `_kiro/*` frame (arriving as `kiro/*` once the acp crate
        // strips the leading underscore) drops to `Ok(None)` — no error, no hang.
        // KasEngine delegates ext frames to the v2 `kiro::` handler, whose
        // unknown-variant arm owns this; this fences the KAS engine path.
        let r = KasEngine.convert_ext_notification("kiro/does/not/exist", &json!({}));
        assert!(
            matches!(r, Ok(None)),
            "unknown _kiro/* frame must drop to Ok(None), got {r:?}"
        );
    }
}

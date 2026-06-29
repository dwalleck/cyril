//! KAS (`_kiro/*`) `session/update` specifics — the KAS analogue of
//! [`super::kiro`] for the v2 dialect. KAS rides standard ACP `session/update`
//! frames whose KAS-specific payload lives entirely in `_meta.kiro`.
//!
//! KAS-2a (cyril-j16p) Slice 1: recognise the `turn_end` lifecycle frame — the
//! signal that drives turn completion under KAS, in place of v2's prompt
//! response — and map it to [`Notification::TurnCompleted`].

use agent_client_protocol as acp;

use crate::types::{Notification, StopReason};

/// Convert a KAS `session_info_update` to an internal notification.
///
/// KAS multiplexes turn lifecycle, metering, and context telemetry through one
/// `session_info_update` envelope, discriminated by `_meta.kiro.kind`. KAS-2a
/// surfaces exactly one sub-kind: **`turn_end`**, the terminal lifecycle signal,
/// mapped to [`Notification::TurnCompleted`] with the stop reason from
/// `_meta.kiro.stopReason`. Every other sub-kind (`turn_completion` metering,
/// `user_message_id_assigned`, `context_usage`, …) returns `None` and stays
/// dormant until KAS-2b — including frames that arrive *after* `turn_end` (a
/// `context_usage` trails it on the wire), so completion keys on the
/// `kind == "turn_end"` value, never on frame ordering.
///
/// A `turn_end` whose `_meta.kiro.stopReason` is missing or unparseable still
/// completes the turn (defaults [`StopReason::EndTurn`]): silently returning
/// `None` would strand the UI in the busy state forever, so this is a runtime
/// fallback that survives release builds, not a `debug_assert!`.
pub(crate) fn session_info_to_notification(siu: &acp::SessionInfoUpdate) -> Option<Notification> {
    let kiro = siu.meta.as_ref()?.get("kiro")?;
    if kiro.get("kind").and_then(serde_json::Value::as_str) != Some("turn_end") {
        return None;
    }
    let stop_reason = kiro
        .get("stopReason")
        .and_then(serde_json::Value::as_str)
        .and_then(|s| {
            serde_json::from_value::<acp::StopReason>(serde_json::Value::String(s.to_owned())).ok()
        })
        .map(super::to_stop_reason)
        .unwrap_or_else(|| {
            tracing::warn!(
                "KAS turn_end without a parseable `_meta.kiro.stopReason`; defaulting to EndTurn"
            );
            StopReason::EndTurn
        });
    Some(Notification::TurnCompleted { stop_reason })
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

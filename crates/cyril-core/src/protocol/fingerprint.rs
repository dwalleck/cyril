//! Engine-identity fingerprinting at the ACP handshake (cyril-6iek).
//!
//! `AgentEngine` is bound at spawn from flag/config alone; nothing else
//! verifies the subprocess speaks the assumed dialect. These pure checks
//! compare the bound engine against the wire's own evidence at the two
//! handshake points — the `initialize` response and session creation/load —
//! so a mismatch fails loud (`BridgeDisconnected` with a remedy) instead of
//! cascading into cryptic mid-turn errors.
//!
//! This module compiles **unconditionally** — never move it behind the `kas`
//! cargo feature. The asymmetric failure ADR-0002 accepts (a default build
//! meeting a KAS wire) is exactly the case that needs a diagnostic, so the
//! detection half must exist in every build. Verification only, never
//! selection: a mismatch is reported, the engine is never silently rebound
//! (ADR-0001 startup-only binding).
//!
//! Evidence (probe-proven, `.cyril-6iek/findings.md`): KAS `initialize`
//! always advertises an `agentCapabilities._meta.kiro` **object** (key set
//! drifts per release — 3 keys at 2.7.1, 5 at 2.10.0+ — so only the object's
//! presence is meaningful); v2 sends no `_meta` at all (2.4.1→2.12.0). KAS
//! session ids are `sess_`-prefixed; v2 ids are bare UUIDs.

use agent_client_protocol as acp;

use crate::types::AgentEngine;

/// Whether the `initialize` response carries the KAS signature: a `kiro`
/// **object** under `agentCapabilities._meta`. Non-object `kiro` values and
/// `_meta` without `kiro` are generic ACP extensibility, not KAS evidence.
fn wire_shows_kas(init: &acp::InitializeResponse) -> bool {
    init.agent_capabilities
        .meta
        .as_ref()
        .and_then(|meta| meta.get("kiro"))
        .is_some_and(serde_json::Value::is_object)
}

/// Compare the bound engine against the `initialize` response's evidence.
/// `Some(reason)` — an actionable, user-facing disconnect reason — exactly
/// when the wire contradicts the binding. `kas_available` is
/// `cfg!(feature = "kas")` at the call site, passed as a parameter so both
/// message variants stay testable in one build.
pub(crate) fn init_mismatch(
    bound: AgentEngine,
    init: &acp::InitializeResponse,
    kas_available: bool,
) -> Option<String> {
    match (bound, wire_shows_kas(init)) {
        (AgentEngine::V2, true) => Some(mismatch_reason(
            bound,
            "the agent advertised `_meta.kiro` capabilities at initialize (a KAS signature)",
            kas_available,
        )),
        (AgentEngine::Kas, false) => Some(mismatch_reason(
            bound,
            "the agent advertised no `_meta.kiro` capabilities at initialize (a v2 signature)",
            kas_available,
        )),
        _ => None,
    }
}

/// The remedy message for a fingerprint contradiction. The `Kas`-bound arm
/// ignores `kas_available` — a Kas binding implies the feature is compiled in
/// (`engine_for` refuses it otherwise), so the only remedy is re-selection.
fn mismatch_reason(bound: AgentEngine, evidence: &str, kas_available: bool) -> String {
    match bound {
        AgentEngine::V2 if kas_available => format!(
            "engine mismatch: cyril is driving the v2 engine but {evidence}; \
             restart cyril with `--agent-engine kas`, or spawn a v2 agent \
             (`kiro-cli acp`)"
        ),
        AgentEngine::V2 => format!(
            "engine mismatch: cyril is driving the v2 engine but {evidence}; \
             this build has no KAS support — rebuild with `--features kas` and \
             run with `--agent-engine kas`, or spawn a v2 agent (`kiro-cli acp`)"
        ),
        AgentEngine::Kas => format!(
            "engine mismatch: cyril was started with the KAS engine but \
             {evidence}; restart with `--agent-engine v2` (or drop the \
             engine selection)"
        ),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Build an `InitializeResponse` from wire-shaped JSON so fixtures mirror
    /// the committed traces (the oracle), not the detector's vocabulary.
    fn init_response(agent_capabilities: serde_json::Value) -> acp::InitializeResponse {
        serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "agentCapabilities": agent_capabilities,
        }))
        .expect("fixture InitializeResponse deserializes")
    }

    /// The live KAS shape (2.10.0–2.12.0): `_meta.kiro` object with sibling
    /// keys and the full drifted key set.
    fn kas_shaped() -> acp::InitializeResponse {
        init_response(serde_json::json!({
            "loadSession": true,
            "_meta": {
                "kiro": {
                    "checkpoints": true,
                    "sessionList": true,
                    "policyNotifications": true,
                    "extensionMethods": ["_kiro/knowledge"],
                    "logging": { "logDir": "/tmp/kiro-logs" }
                }
            }
        }))
    }

    /// The live v2 shape (2.4.1–2.12.0): no `_meta` at all.
    fn v2_shaped() -> acp::InitializeResponse {
        init_response(serde_json::json!({ "loadSession": true }))
    }

    // C3 fn-level: V2-bound + KAS wire ⇒ mismatch naming the evidence.
    #[test]
    fn v2_bound_on_kas_wire_stops() {
        let reason = init_mismatch(AgentEngine::V2, &kas_shaped(), false)
            .expect("KAS signature under V2 binding is a contradiction");
        assert!(
            reason.contains("_meta.kiro"),
            "names the evidence: {reason}"
        );
        assert!(
            reason.contains("KAS"),
            "names the detected engine: {reason}"
        );
    }

    // C4: no false positives — v2 wire and generic `_meta` shapes all pass.
    // Stress fixtures target the presence-only-detector bug: `_meta` without
    // `kiro`, and a non-object `kiro`, are NOT KAS evidence.
    #[test]
    fn no_false_positive_on_v2_and_generic_meta() {
        for (label, init) in [
            ("plain v2 (no _meta)", v2_shaped()),
            (
                "_meta without kiro",
                init_response(serde_json::json!({ "_meta": { "vendor": {} } })),
            ),
            (
                "non-object kiro",
                init_response(serde_json::json!({ "_meta": { "kiro": true } })),
            ),
        ] {
            assert_eq!(
                init_mismatch(AgentEngine::V2, &init, false),
                None,
                "{label} must not stop a v2 binding"
            );
        }
    }

    // C5: Kas-bound + v2 wire ⇒ mismatch naming v2 and the re-selection remedy.
    #[test]
    fn kas_bound_on_v2_wire_stops() {
        let reason = init_mismatch(AgentEngine::Kas, &v2_shaped(), true)
            .expect("v2 signature under Kas binding is a contradiction");
        assert!(
            reason.contains("v2 signature"),
            "names the evidence: {reason}"
        );
        assert!(
            reason.contains("--agent-engine v2"),
            "names the remedy: {reason}"
        );
    }

    // C6: Kas-bound + KAS wire ⇒ proceed.
    #[test]
    fn kas_bound_on_kas_wire_proceeds() {
        assert_eq!(init_mismatch(AgentEngine::Kas, &kas_shaped(), true), None);
    }

    // C9: the remedy is keyed to the build — a default build points at the
    // rebuild, a kas build points at the flag, never both.
    #[test]
    fn mismatch_reason_names_remedy_per_build() {
        let without_feature = init_mismatch(AgentEngine::V2, &kas_shaped(), false)
            .expect("mismatch regardless of feature");
        assert!(
            without_feature.contains("--features kas"),
            "default build names the rebuild: {without_feature}"
        );

        let with_feature = init_mismatch(AgentEngine::V2, &kas_shaped(), true)
            .expect("mismatch regardless of feature");
        assert!(
            with_feature.contains("--agent-engine kas"),
            "kas build names the flag: {with_feature}"
        );
        assert!(
            !with_feature.contains("--features kas"),
            "kas build must not tell the user to rebuild: {with_feature}"
        );
    }
}

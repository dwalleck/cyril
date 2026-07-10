//! The Kiro-scoped `Engine` trait (ADR-0001) and the `V2Engine` port.
//!
//! An `Engine` converts wire notifications to internal [`Notification`]s and
//! declares the client capabilities to advertise at the ACP handshake.
//!
//! Engine is bound once at agent-subprocess spawn; the bridge holds one
//! `Rc<dyn Engine>` for its life (ADR-0001). KAS-0 shipped the core trait +
//! `V2Engine` (a behavior-identical port of today's `convert::` calls); KAS-1
//! adds `KasEngine` behind the `kas` cargo feature (ADR-0002) for the
//! free-path direct spawn.
//! Optional capability sub-traits (`AuthResponder`≈KAS-1 Part B, `HostIo`≈KAS-5,
//! …), queried through defaulted `as_*` accessors, land **with their first
//! consumer** — a consumer-less stub would be dead code under the workspace's
//! `-D warnings`.

use std::collections::HashMap;

use agent_client_protocol as acp;

use crate::protocol::convert;
use crate::types::{AgentEngine, Notification};

/// A Kiro agent engine — **v2** (Rust, `kiro.dev/*` dialect) or **KAS**
/// (`_kiro/*`). The core surface is small (ADR-0001): convert the two wire
/// notification dialects and declare capabilities. Optional capability
/// sub-traits arrive as defaulted `as_*` accessors with their first consumer
/// (KAS-1, cyril-evwh) — not stubbed empty in KAS-0.
pub(crate) trait Engine {
    /// Which [`AgentEngine`] this impl embodies — the bound identity the
    /// handshake fingerprint verifies the wire against (cyril-6iek,
    /// `protocol::fingerprint`).
    fn kind(&self) -> AgentEngine;

    /// Client capabilities advertised at the ACP `initialize` handshake.
    fn client_capabilities(&self) -> acp::ClientCapabilities;

    /// Convert a standard `session/update` notification to an internal one.
    /// Returns `None` for updates this engine does not surface to the UI.
    fn convert_session_update(
        &self,
        args: &acp::SessionNotification,
        cached_inputs: &HashMap<String, serde_json::Value>,
    ) -> Option<Notification>;

    /// Convert an engine-dialect ext notification (v2: `kiro.dev/*`) to an
    /// internal one. `Err` on a malformed-but-recognized frame; `Ok(None)` for
    /// recognized-but-not-surfaced frames.
    fn convert_ext_notification(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> crate::Result<Option<Notification>>;
}

/// The v2 (Rust, `kiro.dev/*`) engine — cyril's default. Delegates to the
/// existing `convert::` functions verbatim, so behavior is byte-identical to
/// pre-KAS-0 (the milestone's strict-parity acceptance criterion).
pub(crate) struct V2Engine;

impl Engine for V2Engine {
    fn kind(&self) -> AgentEngine {
        AgentEngine::V2
    }

    fn client_capabilities(&self) -> acp::ClientCapabilities {
        acp::ClientCapabilities::new()
    }

    fn convert_session_update(
        &self,
        args: &acp::SessionNotification,
        cached_inputs: &HashMap<String, serde_json::Value>,
    ) -> Option<Notification> {
        convert::session_update_to_notification(args, cached_inputs)
    }

    fn convert_ext_notification(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> crate::Result<Option<Notification>> {
        convert::kiro::to_ext_notification(method, params)
    }
}

/// The KAS engine (TypeScript/LangGraph, `_kiro/*` dialect), reached via the
/// free-path direct spawn (KAS-1, cyril-evwh). Gated behind the `kas` cargo
/// feature (ADR-0002) so a default build links no KAS code.
///
/// KAS-2a (cyril-j16p) renders the KAS dialect incrementally. Slice 1:
/// `convert_session_update` maps the `session_info_update` → `turn_end`
/// lifecycle frame to `TurnCompleted` (the KAS turn-completion signal, in place
/// of v2's prompt response) and delegates every other `session/update` — agent
/// text, tool calls — to the generic `convert::` fns. `convert_ext_notification`
/// still delegates to the v2 `kiro::` handler, so unrecognized `_kiro/*` frames
/// fall to the existing unknown-variant drop (dormant until KAS-2b).
/// Advertises `fs` read+write (KAS-5a, cyril-7bdu) and `terminal` (KAS-5b,
/// cyril-ufie) capabilities so KAS delegates file I/O and shell execution to
/// cyril's host-io responders instead of running them in-process.
#[cfg(feature = "kas")]
pub(crate) struct KasEngine;

#[cfg(feature = "kas")]
impl Engine for KasEngine {
    fn kind(&self) -> AgentEngine {
        AgentEngine::Kas
    }

    fn client_capabilities(&self) -> acp::ClientCapabilities {
        // KAS-5a (cyril-7bdu): advertise fs read+write; KAS-5b (cyril-ufie):
        // advertise terminal — so KAS routes file I/O and shell execution back to
        // cyril's host-io/terminal responders. v2 stays empty (V2Engine).
        // cyril-nhzw: attach `_meta.kiro.settings` (AgentSettings marshaled from the
        // user's kiro-cli cli.json) so KAS honors the same feature flags v2 would.
        acp::ClientCapabilities::new()
            .fs(acp::FileSystemCapabilities::default()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(true)
            .meta(super::kas::settings::kiro_settings_meta())
    }

    fn convert_session_update(
        &self,
        args: &acp::SessionNotification,
        cached_inputs: &HashMap<String, serde_json::Value>,
    ) -> Option<Notification> {
        // KAS-2a (cyril-j16p) Slice 1: the `turn_end` lifecycle frame is a
        // KAS-specific `session_info_update` sub-kind that drives turn
        // completion (v2 derives it from the prompt response instead). All
        // other updates — agent text, tool calls — delegate to the generic
        // converter unchanged.
        if let acp::SessionUpdate::SessionInfoUpdate(siu) = &args.update {
            return convert::kas::session_info_to_notification(siu);
        }
        convert::session_update_to_notification(args, cached_inputs)
    }

    fn convert_ext_notification(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> crate::Result<Option<Notification>> {
        convert::kiro::to_ext_notification(method, params)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn v2_client_capabilities_match_handshake_default() {
        // Parity with the old hardcoded handshake (bridge.rs:320): V2Engine must
        // advertise the SAME empty capabilities, or the init request changes.
        assert_eq!(
            format!("{:?}", V2Engine.client_capabilities()),
            format!("{:?}", acp::ClientCapabilities::new()),
        );
    }

    #[cfg(feature = "kas")]
    #[test]
    fn kas_advertises_fs_and_terminal_v2_empty() {
        // KAS-5b / claim C1 (fixture Q). KasEngine advertises fs read+write AND
        // terminal (go-live), so KAS delegates shell execution to cyril's terminal
        // responders. Stress fixture: V2Engine must STILL be empty — designed to fail
        // if the KAS caps body is copy-pasted into V2 (the parity-break bug).
        let caps = KasEngine.client_capabilities();
        assert!(
            caps.fs.read_text_file,
            "KAS must advertise fs.read_text_file"
        );
        assert!(
            caps.fs.write_text_file,
            "KAS must advertise fs.write_text_file"
        );
        assert!(
            caps.terminal,
            "KAS must advertise terminal (KAS-5b go-live, cyril-ufie)"
        );
        assert_eq!(
            format!("{:?}", V2Engine.client_capabilities()),
            format!("{:?}", acp::ClientCapabilities::new()),
            "V2Engine must stay empty (no fs/terminal caps leaked from the KAS path)"
        );
    }

    #[cfg(feature = "kas")]
    #[test]
    fn kas_sets_kiro_settings_meta_v2_none() {
        // cyril-nhzw claim 1: KasEngine attaches `_meta.kiro.settings` (an
        // AgentSettings object); V2Engine attaches no `_meta`. Stress fixture: the
        // parity-break bug (KAS meta leaking into V2) fails the v2 assertion.
        let kas = KasEngine.client_capabilities();
        let settings = kas
            .meta
            .as_ref()
            .expect("KAS must set _meta")
            .get("kiro")
            .and_then(|k| k.get("settings"))
            .and_then(|s| s.as_object())
            .expect("_meta.kiro.settings must be an object");
        // subagentOrchestration is default-on regardless of the user's cli.json, so
        // this is hermetic; its presence is what flips KAS to orchestrate_subagent.
        assert_eq!(
            settings.get("subagentOrchestration"),
            Some(&serde_json::json!({ "enabled": true })),
            "KAS settings must carry subagentOrchestration"
        );
        assert!(
            V2Engine.client_capabilities().meta.is_none(),
            "V2Engine must attach no _meta"
        );
    }

    // Slice 1 oracle + stress fixture: V2Engine routes BOTH a generic
    // `session/update` AND a `_kiro.dev/*` ext frame IDENTICALLY to the direct
    // `convert::` calls. Designed to FAIL if V2Engine drops or miswires the ext
    // path (e.g. stubs `convert_ext_notification` to `None` or to the generic fn).
    #[test]
    fn v2_routes_generic_and_ext_identically() {
        let cache = HashMap::new();

        // Generic: agent_message_chunk -> AgentMessage.
        let generic = acp::SessionNotification::new(
            acp::SessionId::new("sess"),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::from(
                "hello",
            ))),
        );
        let via_engine = V2Engine.convert_session_update(&generic, &cache);
        let direct = convert::session_update_to_notification(&generic, &cache);
        assert_eq!(
            format!("{via_engine:?}"),
            format!("{direct:?}"),
            "generic path must route identically to the direct convert fn"
        );
        assert!(
            via_engine.is_some(),
            "generic frame must produce a Notification"
        );

        // Ext: _kiro.dev steering_queued -> SteeringQueued (must NOT be dropped).
        let method = "kiro.dev/session/update";
        let params = json!({"update": {"sessionUpdate": "steering_queued"}});
        let via_engine = V2Engine.convert_ext_notification(method, &params);
        let direct = convert::kiro::to_ext_notification(method, &params);
        assert_eq!(
            format!("{via_engine:?}"),
            format!("{direct:?}"),
            "ext path must route identically to the direct convert fn"
        );
        assert!(
            matches!(via_engine, Ok(Some(_))),
            "ext frame must NOT be dropped — V2Engine wires the _kiro.dev path"
        );
    }
}

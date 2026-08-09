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
//! Host-callback availability is declared by the engine's **capability
//! adapter set** ([`Adapters`], ADR-0001 amendment, cyril-dn91): dispatch
//! consults it, and inbound advertisement is derived from it by the
//! [`client_capabilities`] free function — engines cannot hand-write (and
//! therefore cannot desynchronize) their advertised capability set.

use agent_client_protocol as acp;

use crate::protocol::convert;
use crate::types::{AgentEngine, Notification};

/// The bound engine's capability-adapter set (ADR-0001 amendment): which
/// host-callback families this engine installs. `KiroClient` dispatch consults
/// it — a family with no adapter is refused with JSON-RPC method-not-found,
/// never the protocol-default null — and [`client_capabilities`] derives the
/// inbound advertisement from the same datum.
///
/// The presence fields exist only under the `kas` cargo feature (the
/// `InternalChannels` cfg-field precedent): a default build **cannot
/// construct** presence, making ADR-0002's "default build links no KAS code"
/// a type-system fact rather than a convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Adapters {
    /// Inbound `_kiro/auth/getAccessToken`. Auth has no capability
    /// advertisement (KAS initiates the callback unconditionally under
    /// `--auth=acp-callback`); the adapter gates execution only.
    #[cfg(feature = "kas")]
    pub(crate) auth: Option<AuthAdapter>,
    /// Bare-ACP typed fs/terminal callbacks, the `_kiro/fs/*` dialect, and
    /// `_kiro/terminal/shell_type`.
    #[cfg(feature = "kas")]
    pub(crate) host_io: Option<HostIoAdapter>,
    /// Per-direction hooks (ADR-0010) — the one family where cyril is both
    /// server and client of the same method names.
    pub(crate) hooks: HooksAdapter,
}

impl Adapters {
    /// The all-absent set — the only value a default (non-kas) build can
    /// construct, and `V2Engine`'s answer.
    pub(crate) const NONE: Self = Self {
        #[cfg(feature = "kas")]
        auth: None,
        #[cfg(feature = "kas")]
        host_io: None,
        hooks: HooksAdapter::None,
    };
}

/// Marker: the engine answers `_kiro/auth/getAccessToken` from the host
/// credential store. Unit today; the g9vt mediator may grow it into a real
/// interface.
#[cfg(feature = "kas")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuthAdapter;

/// Marker: the engine delegates file I/O and shell execution to cyril's
/// host-io/terminal responders.
#[cfg(feature = "kas")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostIoAdapter;

/// Which side of the bidirectional `_kiro/hooks/*` surface cyril occupies
/// (CONTEXT.md "Hook generation"; ADR-0010). Not a bool pair: `Outbound`
/// advertises `{enabled, v2}` with NO inbound serving, and must not be
/// reconciled by an empty inbound registry (the sentinel the no-sentinel rule
/// forbids).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HooksAdapter {
    /// No hooks capability (v2; KAS with `kas_hooks = "off"`).
    None,
    /// cyril serves `list`/`executeHook`/`sessionStart` host-side
    /// (`kas_hooks = "host"`).
    #[cfg(feature = "kas")]
    Inbound,
    /// The agent runs its own registry; cyril only advertises
    /// (`kas_hooks = "kas"`).
    #[cfg(feature = "kas")]
    Outbound,
}

/// Derive the handshake capability set from the engine's adapters — the ONLY
/// constructor of inbound advertisement (ADR-0001 amendment). Presence comes
/// from [`Engine::adapters`]; engines contribute only the opaque
/// `_meta.kiro.settings` extra, nested under a fixed key so it cannot collide
/// with presence-derived keys.
pub(crate) fn client_capabilities(engine: &dyn Engine) -> acp::ClientCapabilities {
    let adapters = engine.adapters();
    let mut caps = acp::ClientCapabilities::new();

    #[cfg(feature = "kas")]
    if adapters.host_io.is_some() {
        // cyril-kf2g: the `_kiro/fs/*` dialect gate is `fs._meta.kiro`, NOT
        // top-level `_meta.kiro`; the flags derive from `kiro_fs::FS_OPS`.
        caps = caps
            .fs(acp::FileSystemCapabilities::default()
                .read_text_file(true)
                .write_text_file(true)
                .meta(super::kas::kiro_fs::capabilities_meta()))
            .terminal(true);
    }

    let mut kiro = serde_json::Map::new();
    if let Some(settings) = engine.settings_extra() {
        kiro.insert("settings".to_string(), settings);
    }
    match adapters.hooks {
        HooksAdapter::None => {}
        #[cfg(feature = "kas")]
        HooksAdapter::Inbound => {
            kiro.insert("hooks".to_string(), serde_json::json!({ "enabled": true }));
        }
        #[cfg(feature = "kas")]
        HooksAdapter::Outbound => {
            kiro.insert(
                "hooks".to_string(),
                serde_json::json!({ "enabled": true, "v2": true }),
            );
        }
    }
    if !kiro.is_empty() {
        let mut meta = acp::Meta::new();
        meta.insert("kiro".to_string(), serde_json::Value::Object(kiro));
        caps = caps.meta(meta);
    }
    caps
}

/// A Kiro agent engine — **v2** (Rust, `kiro.dev/*` dialect) or **KAS**
/// (`_kiro/*`). The core surface is small (ADR-0001): convert the two wire
/// notification dialects and declare the capability-adapter set
/// ([`Adapters`]) from which advertisement is derived. (The original ADR's
/// `as_*` capability accessors were withdrawn by its 2026-07-30 amendment —
/// the adapter set replaces them.)
pub(crate) trait Engine {
    /// Which [`AgentEngine`] this impl embodies — the bound identity the
    /// handshake fingerprint verifies the wire against (cyril-6iek,
    /// `protocol::fingerprint`).
    fn kind(&self) -> AgentEngine;

    /// The capability-adapter set this engine installs (ADR-0001 amendment,
    /// cyril-dn91). Dispatch consults it; [`client_capabilities`] derives the
    /// handshake advertisement from it. Deliberately no default: an engine
    /// that inherits the wrong set either refuses live callbacks (missing
    /// adapter) or answers surfaces it cannot back (phantom adapter). Each
    /// engine answers explicitly.
    fn adapters(&self) -> Adapters;

    /// The opaque `_meta.kiro.settings` object attached to the handshake
    /// advertisement, if any. Content-only: presence keys are derived from
    /// [`Engine::adapters`] and cannot be smuggled here (the extra nests under
    /// a fixed `settings` key).
    fn settings_extra(&self) -> Option<serde_json::Value> {
        None
    }

    /// Does this engine stream a wire `turn_end` in addition to the prompt
    /// response? (cyril-b4y4)
    ///
    /// Terminal-source authority is an Engine fact (CONTEXT.md "Turn-end"):
    /// only the bound engine may declare a turn over, and turn mediation asks
    /// this question instead of matching on `kind()` — the enum-match pattern
    /// ADR-0001 rejected. `true` means every turn ends with TWO terminal
    /// signals, so a release still owes the companion terminal; `false` means
    /// the prompt response is the sole terminal and the companion ledger must
    /// stay empty after release (cyril-upjh).
    ///
    /// Deliberately no default: an engine that inherits the wrong answer here
    /// either freezes turns (missing companion) or eats live ones (phantom
    /// companion). Each engine answers explicitly.
    fn emits_wire_turn_end(&self) -> bool;

    /// Convert a standard `session/update` notification to an internal one.
    /// Returns `None` for updates this engine does not surface to the UI.
    fn convert_session_update(&self, args: &acp::SessionNotification) -> Option<Notification>;

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

    /// v2 installs no capability adapters: it keeps the standard permission
    /// path and never sends host callbacks (client.rs:23-24), so a kas-feature
    /// build bound to v2 refuses them (cyril-dn91).
    fn adapters(&self) -> Adapters {
        Adapters::NONE
    }

    /// v2's sole terminal is the prompt response — no wire `turn_end`
    /// (`convert::kas` is the only unstamped-terminal producer).
    fn emits_wire_turn_end(&self) -> bool {
        false
    }

    fn convert_session_update(&self, args: &acp::SessionNotification) -> Option<Notification> {
        convert::session_update_to_notification(args)
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
/// text and tool calls — to the generic `convert::` functions. Exact normalized
/// `kiro/workflow/*` lifecycle methods route through the KAS-only workflow
/// adapter; every other extension keeps the existing Kiro conversion path.
/// Advertises `fs` read+write (KAS-5a, cyril-7bdu) and `terminal` (KAS-5b,
/// cyril-ufie) capabilities so KAS delegates file I/O and shell execution to
/// cyril's host-io responders instead of running them in-process.
#[cfg(feature = "kas")]
#[derive(Default)]
pub(crate) struct KasEngine {
    /// The decided hooks advertisement (cyril-jiyn); carried by the engine so
    /// `client_capabilities()` stays parameterless on the trait.
    pub(crate) hooks_mode: crate::types::kas_hooks::KasHooksMode,
}

#[cfg(feature = "kas")]
impl Engine for KasEngine {
    fn kind(&self) -> AgentEngine {
        AgentEngine::Kas
    }

    /// KAS ends every turn with BOTH terminals — the streamed
    /// `session_info_update → turn_end` first, the prompt response 0–1 ms
    /// behind (live-confirmed 2026-08-01 on 2.16.0, turn-end-ordering
    /// captures). A release therefore still owes the companion terminal.
    fn emits_wire_turn_end(&self) -> bool {
        true
    }

    /// KAS installs auth (KAS-1, cyril-evwh) and host I/O (KAS-5a/5b,
    /// cyril-7bdu/cyril-ufie) — delegating file I/O and shell execution to
    /// cyril's responders — plus the hooks direction its `hooks_mode` decides
    /// (cyril-jiyn, ADR-0010). The bare-ACP fs read/write flags this derives
    /// stay advertised alongside the `_kiro/fs/*` dialect: they are the
    /// fallback if a future KAS drops a Kiro flag (cyril-kf2g).
    fn adapters(&self) -> Adapters {
        use crate::types::kas_hooks::KasHooksMode;
        Adapters {
            auth: Some(AuthAdapter),
            host_io: Some(HostIoAdapter),
            hooks: match self.hooks_mode {
                KasHooksMode::Off => HooksAdapter::None,
                KasHooksMode::Host => HooksAdapter::Inbound,
                KasHooksMode::Kas => HooksAdapter::Outbound,
            },
        }
    }

    /// cyril-nhzw: `_meta.kiro.settings` (AgentSettings marshaled from the
    /// user's kiro-cli cli.json) so KAS honors the same feature flags v2 would.
    fn settings_extra(&self) -> Option<serde_json::Value> {
        Some(super::kas::settings::settings_extra_value())
    }

    fn convert_session_update(&self, args: &acp::SessionNotification) -> Option<Notification> {
        // KAS-2a (cyril-j16p) Slice 1: the `turn_end` lifecycle frame is a
        // KAS-specific `session_info_update` sub-kind that drives turn
        // completion (v2 derives it from the prompt response instead). All
        // other updates — agent text, tool calls — delegate to the generic
        // converter unchanged.
        if let acp::SessionUpdate::SessionInfoUpdate(siu) = &args.update {
            return convert::kas::session_info_to_notification(siu);
        }
        convert::session_update_to_notification(args)
    }

    fn convert_ext_notification(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> crate::Result<Option<Notification>> {
        if let Some(notification) = convert::kas::workflow_to_notification(method, params) {
            return Ok(notification);
        }
        convert::kiro::to_ext_notification(method, params)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use serde_json::json;

    /// REGRESSION FENCE (cyril-b4y4 C6, engine half; supersedes the engine
    /// side of cyril-upjh's `wire_companion_is_owed_only_under_kas`).
    /// Terminal-source shape is an Engine fact: KAS streams a wire `turn_end`
    /// alongside the prompt response; v2's prompt response is the sole
    /// terminal. Both engines asserted in ONE test so a trait default that
    /// silently made them identical fails here.
    #[test]
    fn terminal_source_matrix() {
        assert!(
            !V2Engine.emits_wire_turn_end(),
            "a v2 turn has one terminal source — its release owes no companion"
        );
        #[cfg(feature = "kas")]
        assert!(
            KasEngine::default().emits_wire_turn_end(),
            "a KAS turn has two terminal sources — its release still owes the wire one"
        );
    }

    // cyril-dn91 slice 1 stress fixture: the mode→direction mapping matrix.
    // All three cells asserted — the swapped-arms bug (Host↔Kas) type-checks
    // fine — plus the V2 cell exercised under `--features kas` (the dn91 trap:
    // a cfg-keyed implementation would give V2 adapters here).
    #[cfg(feature = "kas")]
    #[test]
    fn adapters_mapping_matrix() {
        use crate::types::kas_hooks::KasHooksMode;

        assert_eq!(
            V2Engine.adapters(),
            Adapters::NONE,
            "V2 installs no adapters even in a kas build"
        );
        let hooks_of = |mode| KasEngine { hooks_mode: mode }.adapters().hooks;
        assert_eq!(hooks_of(KasHooksMode::Off), HooksAdapter::None);
        assert_eq!(hooks_of(KasHooksMode::Host), HooksAdapter::Inbound);
        assert_eq!(hooks_of(KasHooksMode::Kas), HooksAdapter::Outbound);
        assert!(
            KasEngine::default().adapters().host_io.is_some(),
            "KAS installs host I/O"
        );
    }

    // The default-build cell of the same matrix: Adapters::NONE is the only
    // constructible value without the kas feature (presence fields cfg'd out).
    #[test]
    fn v2_adapters_are_none_in_every_build() {
        assert_eq!(V2Engine.adapters(), Adapters::NONE);
    }

    #[test]
    fn v2_client_capabilities_match_handshake_default() {
        // Parity with the old hardcoded handshake (bridge.rs:320): V2Engine must
        // advertise the SAME empty capabilities, or the init request changes.
        assert_eq!(
            format!("{:?}", client_capabilities(&V2Engine)),
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
        let caps = client_capabilities(&KasEngine::default());
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
            format!("{:?}", client_capabilities(&V2Engine)),
            format!("{:?}", acp::ClientCapabilities::new()),
            "V2Engine must stay empty (no fs/terminal caps leaked from the KAS path)"
        );
    }

    // cyril-kf2g: the fs dialect gate is `clientCapabilities.fs._meta.kiro`,
    // NOT top-level `clientCapabilities._meta.kiro`. That distinction is the
    // entire finding — probe-kas-rpc-sweep-2.16.0.py advertised the resolved
    // capability name at the TOP level, moved nothing, and concluded the
    // trigger was "unknown". So this asserts the placement, not just the
    // presence: the top-level assertion is what fails if the object drifts up
    // a level, which is otherwise invisible (KAS ignores it silently and keeps
    // using the bare-ACP dialect).
    #[cfg(feature = "kas")]
    #[test]
    fn kas_advertises_kiro_fs_dialect_nested_under_fs() {
        let caps = client_capabilities(&KasEngine::default());

        let fs_meta =
            serde_json::to_value(caps.fs.meta.as_ref().expect("fs._meta present")).unwrap();
        assert_eq!(
            fs_meta.get("kiro"),
            Some(&json!({
                "readFile": true,
                "writeFile": true,
                "stat": true,
                "readDirectory": true,
                "delete": true,
            })),
            "all five wire flags, under fs._meta.kiro"
        );

        // The wrong placement must stay empty. `_meta.kiro` legitimately holds
        // settings/hooks, so this checks for the fs keys specifically.
        let top = serde_json::to_value(caps.meta.as_ref().expect("top _meta present")).unwrap();
        let top_kiro = top.get("kiro").expect("_meta.kiro");
        for flag in ["readFile", "writeFile", "stat", "readDirectory", "delete"] {
            assert!(
                top_kiro.get(flag).is_none(),
                "{flag} must NOT sit at top-level _meta.kiro (the sweep's mistake)"
            );
        }

        // The bare-ACP flags stay on: they are the fallback if a future KAS
        // drops a Kiro flag, and are what `fs/write_text_file` still rides.
        assert!(caps.fs.read_text_file && caps.fs.write_text_file);

        // Parity-break guard: V2 advertises no fs capability at all, so it
        // cannot acquire an fs._meta by copy-paste.
        assert!(client_capabilities(&V2Engine).fs.meta.is_none());
    }

    // cyril-jiyn claim 2 fence: the mode×engine advertisement matrix. The V2
    // cells run under `--features kas` too — a cfg-keyed implementation (the
    // cyril-dn91 trap) fails them. Absent-key asserts enforce the no-sentinel
    // rule: Off means NO hooks key (not enabled:false), Host means NO v2 key
    // (not v2:false) — oracle is the serialized JSON vs the covenant §2
    // shapes, not the constructor's enums.
    #[cfg(feature = "kas")]
    #[test]
    fn kas_hooks_advertisement_matrix() {
        use crate::types::kas_hooks::KasHooksMode;

        let hooks_json = |mode: KasHooksMode| -> Option<serde_json::Value> {
            let caps = client_capabilities(&KasEngine { hooks_mode: mode });
            let meta = serde_json::to_value(caps.meta.expect("KAS meta present")).unwrap();
            meta.get("kiro").and_then(|k| k.get("hooks")).cloned()
        };

        assert_eq!(
            hooks_json(KasHooksMode::Host),
            Some(json!({"enabled": true})),
            "Host advertises enabled only — no v2 key at all"
        );
        assert_eq!(
            hooks_json(KasHooksMode::Kas),
            Some(json!({"enabled": true, "v2": true})),
            "Kas advertises the standalone loader"
        );
        assert_eq!(
            hooks_json(KasHooksMode::Off),
            None,
            "Off omits the hooks key entirely (absence, not enabled:false)"
        );

        // V2-engine cells: no _meta at all regardless of the knob — bound-engine
        // keying, exercised in the kas-feature build (the dn91 trap fence).
        assert!(
            client_capabilities(&V2Engine).meta.is_none(),
            "V2Engine advertises no _meta.kiro whatever the knob says"
        );
    }

    #[cfg(feature = "kas")]
    #[test]
    fn kas_sets_kiro_settings_meta_v2_none() {
        // cyril-nhzw claim 1: KasEngine attaches `_meta.kiro.settings` (an
        // AgentSettings object); V2Engine attaches no `_meta`. Stress fixture: the
        // parity-break bug (KAS meta leaking into V2) fails the v2 assertion.
        let kas = client_capabilities(&KasEngine::default());
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
            client_capabilities(&V2Engine).meta.is_none(),
            "V2Engine must attach no _meta"
        );
    }

    // Slice 1 oracle + stress fixture: V2Engine routes BOTH a generic
    // `session/update` AND a `_kiro.dev/*` ext frame IDENTICALLY to the direct
    // `convert::` calls. Designed to FAIL if V2Engine drops or miswires the ext
    // path (e.g. stubs `convert_ext_notification` to `None` or to the generic fn).
    #[test]
    fn v2_routes_generic_and_ext_identically() {
        // Generic: agent_message_chunk -> AgentMessage.
        let generic = acp::SessionNotification::new(
            acp::SessionId::new("sess"),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::from(
                "hello",
            ))),
        );
        let via_engine = V2Engine.convert_session_update(&generic);
        let direct = convert::session_update_to_notification(&generic);
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

    // Probe (cyril-3zy4): KasEngine must route `_kiro/error/rate_limit` to
    // RateLimited, not delegate-drop it. engine.rs:143 delegates to
    // kiro::to_ext_notification, so this fences the KAS engine path end-to-end.
    #[cfg(feature = "kas")]
    #[test]
    fn probe_kas_engine_routes_rate_limit() {
        let params = json!({ "message": "Rate limit exceeded" });
        let r = KasEngine::default().convert_ext_notification("kiro/error/rate_limit", &params);
        assert!(
            matches!(r, Ok(Some(crate::types::Notification::RateLimited { .. }))),
            "KasEngine must route rate_limit to RateLimited, got {r:?}"
        );
    }

    #[cfg(feature = "kas")]
    #[test]
    fn kas_alone_routes_normalized_workflow_extensions() {
        let params = json!({
            "workflowId": "workflow",
            "workflowName": "recipe",
            "inputs": {},
            "nodeTree": [{"nodeId": "step", "type": "step", "agentName": "agent"}]
        });
        let kas = KasEngine::default().convert_ext_notification("kiro/workflow/run_start", &params);
        assert!(
            matches!(
                kas,
                Ok(Some(crate::types::Notification::Workflow(event)))
                    if event.method_name() == "run_start"
            ),
            "KAS must route the normalized workflow method"
        );

        let v2 = V2Engine.convert_ext_notification("kiro/workflow/run_start", &params);
        assert!(
            matches!(v2, Ok(None)),
            "V2 must preserve the pre-workflow unknown-extension result, got {v2:?}"
        );
        let raw =
            KasEngine::default().convert_ext_notification("_kiro/workflow/run_start", &params);
        assert!(
            matches!(raw, Ok(None)),
            "raw underscore spelling must not bypass ACP normalization, got {raw:?}"
        );
    }
}

# cyril-dn91 — falsifiable design: engine-selected adapter set, derived capabilities

Implements the ADR-0001 amendment (2026-07-30): host-callback availability is
decided by the **bound engine's adapter set**, not the cargo feature. Probe
evidence: `.cyril-dn91/findings.md` (19 handled variants, 0 engine consults on
any dispatch path; a V2-bound kas build answers auth/fs/hooks and *executes
arbitrary hook commands*).

## Purpose

One rule: **a callback family answers iff the bound engine installs its
adapter; advertisement is derived from the same datum.** Absent adapter →
JSON-RPC `method_not_found` (-32601), never the protocol-default null (which
the agent reads as success-with-empty-result).

## Architecture and placement (step 2c)

**Owner: `protocol/engine.rs`** — the module the 2026-07-30 review's deletion
test said to deepen. New vocabulary (all `pub(crate)`, engine-neutral data):

```rust
// engine.rs — un-gated
pub(crate) struct Adapters {
    pub auth: Option<AuthAdapter>,      // inbound _kiro/auth/getAccessToken
    pub host_io: Option<HostIoAdapter>, // fs/* typed + _kiro/fs/* + terminal/*
    pub hooks: HooksAdapter,            // per-direction (ADR-0010)
}
// Unit markers under kas; UNINHABITED in a default build — a non-kas build
// CANNOT construct presence. ADR-0002 becomes a type-system fact.
#[cfg(feature = "kas")]     pub(crate) struct AuthAdapter;
#[cfg(not(feature = "kas"))] pub(crate) enum AuthAdapter {}
#[cfg(feature = "kas")]     pub(crate) struct HostIoAdapter;
#[cfg(not(feature = "kas"))] pub(crate) enum HostIoAdapter {}
pub(crate) enum HooksAdapter {
    None,
    #[cfg(feature = "kas")] Inbound,   // host mode: serve list/execute/sessionStart
    #[cfg(feature = "kas")] Outbound,  // kas mode: advertise {enabled,v2}; agent runs hooks
}

pub(crate) trait Engine {
    fn adapters(&self) -> Adapters;    // REQUIRED, no default (like emits_wire_turn_end)
    fn settings_extra(&self) -> Option<serde_json::Value> { None } // opaque _meta.kiro.settings
    // fn client_capabilities()  ← REMOVED from the trait
}

/// The ONLY constructor of the handshake capability set. Presence derives from
/// adapters(); engines contribute only the opaque settings extra.
pub(crate) fn client_capabilities(engine: &dyn Engine) -> acp::ClientCapabilities
```

- `V2Engine::adapters()` → all-absent. `KasEngine::adapters()` → auth+host_io
  present, hooks from `hooks_mode` (Off→None, Host→Inbound, Kas→Outbound). The
  trait-level `#[cfg] fn hooks_mode()` is **deleted**; its one consumer
  (registry construction) switches to `adapters().hooks`.
- **`KiroClient` consults, never decides**: each dispatch arm checks the
  engine's adapter for its family; absent → `acp::Error::method_not_found()`.
  Typed overrides (fs ×2, terminal ×5) gate the same way. `kas/settings.rs`
  loses the hooks-key branch (`kiro_client_meta` → settings marshaling only);
  the hooks/fs advertisement JSON is assembled by the derivation fn from
  presence data.
- Hooks registry field becomes `Option<Rc<HookRegistry>>`, `Some` **iff**
  `HooksAdapter::Inbound` — the empty-registry-as-absent sentinel is
  structurally gone. `hooks/cancel` is consumed only under Inbound;
  `hooks/didChange` only under Inbound|Outbound (None → debug-log + consume).
- **Forbidden:** engines hand-writing capability structs (no capability method
  exists to override); `client.rs` deciding availability from `cfg` alone;
  anything outside `cyril-core::protocol` seeing these types (`pub(crate)`);
  reconciling Outbound hooks with an empty registry (sentinel).
- **No new seam** — `Adapters` slots behind the existing `Engine` trait
  (extend-existing; `design-an-interface` not required).

## Input shapes (step 2)

1. **Build × engine:** default+V2, kas+V2, kas+KAS — 3 reachable configs (KAS
   in default build is refused at spawn, bridge.rs:496).
2. **HooksAdapter:** None / Inbound / Outbound (V2 always None; KAS per
   `kas_hooks` config Off/Host/Kas).
3. **Adapters presence matrix:** reachable cells: V2=(None,None,None),
   KAS=(Some,Some,{None|Inbound|Outbound}). Type-admitted-but-unshipped cells
   (e.g. auth-only) are covered generically by the C9 matrix test, which walks
   whatever `adapters()` returns — future engines inherit coverage.
4. **Inbound method space:** 19 handled variants (7 typed, 5 ext arms, 5
   `_kiro/fs/*`, 2 control notifications) + unknown ext methods (null-default
   preserved, C14).
5. **didChange payload:** with `hooks[]` (Outbound) / without (Inbound) /
   either under None.
6. **executeHook params** (command/operationId presence): unchanged under
   Inbound (existing tests); refused before parsing under absent adapter.
7. **Out of scope shapes:** malformed params on refused methods (refusal
   precedes parsing); permission requests (standard ACP path, not an adapter —
   ADR amendment).

## Removed-invariant sweep (step 2b)

The change is **subtractive**: it removes "a kas build's dispatch answers
regardless of bound engine". What that unconditional dispatch silently
guaranteed:

- *"KAS-bound clients always answer these families"* — must STILL hold → C6
  (parity, live-confirmed per AC6).
- *"getAccessToken always produces a store-read attempt, so any Err is a real
  auth failure"* — `notify_if_auth_failure` relies on this; after gating, a V2
  refusal is an `Err` and would emit a spurious `BridgeError("auth")` → C13.
- *"A hook registry Rc always exists"* — field becomes `Option`; sole
  consumers are the dispatch arms being gated (grep-verified: `self.hooks`
  only in `handle_ext_request` + construction) → C10.
- *"V2 terminal callbacks refuse with the responder's host-shell error"* —
  shape changes to -32601. Safe: v2 never sends terminal callbacks in
  production (client.rs:23-24; probe fired them synthetically), and no code
  matches on that error string (grep: only the client.rs unit test asserts
  it, updated in-slice).
- *"didChange with a hooks[] payload always emits HooksChanged"* — now gated
  to Inbound|Outbound; V2 never receives didChange in production → C12 fences
  the gate.

## Claims and falsification

Refusal fences C1–C5 are the probe's characterization tests **inverted** —
they fail against today's code by construction (maximal non-vacuity).

| # | Claim | Falsifier (input → expected; falsified by) | Oracle | Cost | Status | Regression fence |
|---|-------|--------------------------------------------|--------|------|--------|------------------|
| C1 | kas build + V2 bound: `getAccessToken` answers -32601, never null, never a store read | fire the ext request at a V2-bound client; any non-32601 outcome falsifies. Buggy impl: today's ungated arm | JSON-RPC error code on the wire reply | test | pending | `client.rs` test `v2_refuses_auth_callback` |
| C2 | kas build + V2 bound: typed `fs/read_text_file`+`write_text_file` answer -32601; write leaves NO file | call overrides on tempdir; content returned or file created falsifies. Buggy impl: gate after side effect | fs state via `std::fs` (independent of client) | test | pending | `v2_refuses_typed_fs` |
| C3 | kas build + V2 bound: all 5 `_kiro/fs/*` answer -32601, walked over `FS_OPS` | iterate FS_OPS; any non-32601 falsifies. Buggy impl: gate on 4 of 5 arms | FS_OPS table as method census | test | pending | `v2_refuses_kiro_fs_all_ops` |
| C4 | kas build + V2 bound: 5 typed `terminal/*` + `shell_type` answer -32601 (replacing the host-shell responder error) | call each; responder-shaped error or success falsifies. Buggy impl: only the ext arm gated, typed overrides forgotten | error code + message ≠ "no resolved host shell" | test | pending | `v2_refuses_terminal_family` |
| C5 | Inbound-absent hooks (V2; KAS Outbound/Off): list/execute/sessionStart answer -32601; executeHook runs NO command | executeHook with a file-creating command; file exists falsifies. Buggy impl: probe slice 2's current behavior | fs side-effect check | test | pending | `hooks_inbound_absent_refuses` |
| C6 | KAS+Host bound: every family answers with today's result shapes (fs content, stat object, list serves registry, executeHook runs, shell_type, auth attempt) | rerun probe-slice-1 calls against KAS-bound client; any refusal falsifies. Buggy impl: gate keyed on cfg or inverted | probe slice 1 result shapes (pre-change capture) + AC6 live session | test+live | pending | `kas_bound_families_still_answer` + live parity check |
| C7 | Advertisement is fully determined by (host_io presence, hooks direction, settings extra) | reconstruct all 4 configs from those inputs; byte-diff falsifies | hand-assembled JSON literal (independent of constructors) | 15m | **passed** (probe slice 3) | `advertisement_is_fully_determined_by_presence_direction_extras` (kept) |
| C8 | The Engine trait exposes no capability method; the free fn is the only constructor | structural: method deleted; an engine re-adding one desyncs advertise/execute | C9 matrix (behavioral net for any desync) | build | pending | C9's test |
| C9 | ONE test walks `adapters()` per constructible engine × family asserting advertised(f) == answers(f) via real client calls | any cell where the two facts differ falsifies. Buggy impl: advertise-without-adapter (today's V2 inverse: answer-without-advertise) | matrix over live `KiroClient` calls + serialized caps JSON | test | pending | `adapter_matrix_advertise_iff_answer` |
| C10 | HookRegistry is constructed iff Inbound (`Option`, no empty-registry sentinel) | construct clients in all 3 directions; Some under Outbound/None falsifies. Buggy impl: keep always-constructing | field inspection in client.rs unit test | test | pending | `registry_present_iff_inbound` |
| C11 | Default build compiles + passes with un-gated `Adapters`; presence UNCONSTRUCTIBLE (uninhabited markers); caps byte-identical to `ClientCapabilities::new()` | `cargo check`/`test` default features; compile failure or caps diff falsifies. Buggy impl: un-gated code referencing kas module | rustc (default-features CI leg) | 5m | pending | existing default CI leg + `v2_client_capabilities_match_handshake_default` |
| C12 | didChange with `hooks[]` under a hooks-less engine (V2) emits NO HooksChanged; Outbound still emits | send didChange to V2-bound client; notification on channel falsifies. Buggy impl: today's ungated arm | channel try_recv (empty vs frame) | test | pending | `did_change_gated_by_hooks_direction` |
| C13 | A refusal (-32601) from getAccessToken does NOT emit `BridgeError("auth")` | V2-bound getAccessToken; BridgeError on channel falsifies. Buggy impl: gate inside handle_ext_request with notify path untouched | channel try_recv | test | pending | `auth_refusal_emits_no_bridge_error` |
| C14 | Methods outside the five families still answer the protocol-default null with breadcrumb (dcc6 F15), under BOTH engines | unknown method → null; -32601 falsifies (over-broad refusal net). Buggy impl: refuse-by-default rewrite | probe slice 1's NullDefault assert + KAS twin | test | pending | probe slice 1 assert + matrix unknown row |

Cheapest falsifier (C7) ran before this document was presented: **passed**
(probe slice 3, committed).

## Negative space (what this deliberately does NOT do)

1. **No mediator / loop routing** — dispatch stays in `KiroClient`; the
   `accept()`-style mediator type and run_loop arm are cyril-g9vt (verified
   open, blocked on this).
2. **No vendor-neutral adapter seam** — Kiro-scoped per the ADR amendment; a
   second vendor with host callbacks triggers that design, not this one.
3. **No auth responder internals change** — store injection is cyril-5db7,
   token refresh is cyril-taba (both verified open).
4. **No hooks hardening** — registry hot-reload is cyril-2adk; executeHook
   command-echo verification is cyril-qr6l (both verified open; qr6l's threat
   is *narrowed* by C5 since only Inbound engines execute at all).
5. **No permission-path change** — permission stays the standard ACP
   human-decision path (ADR amendment, settled rationale).
6. **No live engine re-bind** — one adapter set per subprocess lifetime,
   matching the immutable Engine binding (settled rationale).
7. **No KAS refusal-object rendering** — cyril-ker1 (verified open).

## Consequences for existing artifacts

- Probe characterization tests (slices 1–2) are inverted into C1/C5 fences by
  the build; slice 3 survives verbatim as C7's fence.
- ADR-0001 amendment's "shape sketch" is realized with `client_capabilities`
  as a free fn (stronger than the sketch: no overridable method exists at
  all). ADR text needs no edit — the sketch says "not a pinned signature".
- Issue AC5 (stale AuthResponder text) was already done 2026-08-01.

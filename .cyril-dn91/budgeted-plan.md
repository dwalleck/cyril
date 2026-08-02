# cyril-dn91 — budgeted plan (7 slices)

Design: `.cyril-dn91/falsifiable-design.md` (approved; C7 cheapest falsifier
passed). Build order keeps every slice green: each gating slice flips its own
family's characterization asserts in `probe_dn91.rs` in the same commit.
Existing KAS-bound tests (fs/terminal/hooks in `client.rs`) are the standing
KAS-parity fences and must pass untouched through every slice.

No slice writes to stdout; all diagnostics are `tracing` events (stderr sink) —
the output-stream rule is satisfied vacuously and re-checked in self-review.

---

## Slice 1: `Adapters` data type + `Engine::adapters()` on both engines

**Claim:** C11 (partial: presence unconstructible in default build) + the data
substrate for C1–C10.
**Oracle:** rustc on BOTH feature configs (a default build that can construct
presence fails the uninhabited-marker design); adapters-mapping test asserted
against the KasHooksMode table in `settings.rs` (independent: the mapping is
read off ADR-0010, not off the impl).
**Stress fixture:** mapping test asserts all three `hooks_mode → HooksAdapter`
cells (Off→None, Host→Inbound, Kas→Outbound) — designed to fail under the
swapped-arms bug (Host↔Kas), which type-checks fine. V2 cell asserted in the
same test under `--features kas` (the dn91 trap: cfg-keyed impl).
**Loop budget:** none (pure data; derive Copy/PartialEq).
**Wall budget:** n/a (no always-on phase).
**Files:** `crates/cyril-core/src/protocol/engine.rs` only.

**Code (advisory):** types as in the design's architecture block; `adapters()`
REQUIRED on the trait (no default). `KasEngine::adapters()` maps its
`hooks_mode` field; `V2Engine::adapters()` returns all-absent via
`Adapters::NONE` const (usable in default builds — `None`/`HooksAdapter::None`
are always constructible; only *presence* is kas-gated).

**Verification:**
- [ ] Unit tests pass (both feature configs)
- [ ] Stress fixture (mapping matrix incl. V2-under-kas cell) passes
- [ ] probe_dn91 slices 1–3 still pass (no behavior change yet)
- [ ] Budgets hold (vacuous)

## Slice 2: derived `client_capabilities` free fn; capability method off the trait

**Claim:** C7 (derivation is the code now), C8 (no overridable method), C9
substrate.
**Oracle:** probe slice 3 (`advertisement_is_fully_determined_...`) — the
hand-assembled JSON literal predates this slice and must keep passing
byte-identically; plus `v2_client_capabilities_match_handshake_default`.
**Stress fixture:** probe slice 3 catches the double-hooks-key bug (extras
smuggling a `hooks` key while derivation also inserts one → duplicate/overwrite
diverges from the literal). The V2 assert catches presence leaking into the
empty set.
**Loop budget:** none new (FS_OPS map already exists, O(5)).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/engine.rs`,
`crates/cyril-core/src/protocol/kas/settings.rs` (+ one-line call-site swap in
`bridge.rs:880`, noted here so the slice diff is honest).

**Code (advisory):** `pub(crate) fn client_capabilities(engine: &dyn Engine)`
in engine.rs assembles: host_io present → fs(read+write+`kiro_fs`
meta)+terminal (kas-cfg'd block; unreachable-by-type in default builds); hooks
direction → `_meta.kiro.hooks` key; `settings_extra()` → `_meta.kiro.settings`.
`kiro_client_meta` shrinks to settings-marshaling only (rename to
`settings_extra_value` or similar); trait method `client_capabilities` DELETED;
engine.rs capability tests switch to the free fn. `settings_extra` doc states
"extras must not contain presence keys" — sanity-hint tier (in-crate impls,
C7 fence catches): `debug_assert!` in the derivation fn.

**Verification:**
- [ ] Unit tests pass (both configs)
- [ ] Probe slice 3 passes byte-identically (unchanged file)
- [ ] Existing advertisement fences (hooks matrix, kiro_fs placement, settings meta) pass, rewired to the free fn
- [ ] Budgets hold (vacuous)

## Slice 3: gate the Auth family

**Claim:** C1 (V2 refuses getAccessToken with -32601, never null/store-read) +
C13 (refusal emits no `BridgeError("auth")`).
**Oracle:** JSON-RPC error code on the reply (wire-shape fact, independent of
the responder); channel `try_recv` emptiness for C13.
**Stress fixture:** C13's fixture IS the plausible-bug fixture: gate placed
inside `handle_ext_request` with `notify_if_auth_failure` untouched turns every
V2 refusal into a spurious UI auth error — test fails under exactly that impl.
C1 asserts code == -32601 specifically (not just "is error"), failing the
null-default and responder-error impls.
**Loop budget:** none (one `Option::is_some` check per request).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs`,
`crates/cyril-core/src/protocol/probe_dn91.rs` (flip the auth row of slice 1 to
Refused).

**Code (advisory):** auth arm: adapter present → respond; absent → debug-log
breadcrumb + `Err(acp::Error::method_not_found())`. `notify_if_auth_failure`
early-returns when `e.code == MethodNotFound` (refusals are not auth
failures — comment states the constraint).

**Verification:**
- [ ] Unit tests pass; new fences `v2_refuses_auth_callback`, `auth_refusal_emits_no_bridge_error`
- [ ] Probe slice 1 auth row flipped and passing
- [ ] Existing l7tw C11 auth-failure tests (KAS-bound) still pass
- [ ] Budgets hold (vacuous)

## Slice 4: gate the Hooks family (per-direction) + registry de-sentinel

**Claim:** C5 (Inbound-absent refuses list/execute/sessionStart; executeHook
runs NOTHING) + C10 (registry `Option`, Some iff Inbound) + C12 (didChange
gated to Inbound|Outbound).
**Oracle:** fs side-effect probe for C5 (executeHook told to create a file; fs
checked via `std::fs`, independent of the client); field inspection for C10;
channel `try_recv` for C12.
**Stress fixture:** three, each targeting a named bug: (a) gate-after-execute —
C5's command is file-creating, file existence fails the test even if the reply
is an error; (b) empty-registry sentinel — C10 asserts `hooks.is_none()` under
Outbound AND Off, failing the keep-constructing impl; (c) today's ungated
didChange — C12 sends `hooks:[…]` payload to a V2-bound client and fails if
HooksChanged appears (this is current behavior, so the fence is non-vacuous by
construction).
**Loop budget:** none new.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs`,
`crates/cyril-core/src/protocol/engine.rs` (delete trait `hooks_mode` — its
sole consumer, registry construction, now reads `adapters().hooks`), plus
probe flips in `probe_dn91.rs` (slice 2 inverted; hooks rows of slice 1).

**Code (advisory):** `hooks: Option<Rc<HookRegistry>>` — `Some(load(..))` iff
`HooksAdapter::Inbound`; list/sessionStart use `as_ref()` or refuse;
execute/cancel gated on Inbound; didChange arm matches `Inbound | Outbound`
(None → `debug!` + consume). KasEngine keeps its `hooks_mode` FIELD (feeds
`adapters()`); only the trait method dies.

**Verification:**
- [ ] Unit tests pass; new fences `hooks_inbound_absent_refuses`, `registry_present_iff_inbound`, `did_change_gated_by_hooks_direction`
- [ ] Probe slice 2 inverted (Outbound refuses inbound execution) and passing
- [ ] Existing Host-mode hooks tests (list routes, slow-hook, sessionStart) pass unchanged
- [ ] Budgets hold (vacuous)

## Slice 5: gate the Host I/O family (typed fs + `_kiro/fs/*` + terminal)

**Claim:** C2 (typed fs refuse, write side-effect-free) + C3 (all 5 `_kiro/fs/*`
refuse, walked) + C4 (5 typed terminal + shell_type refuse with -32601, not the
host-shell responder error).
**Oracle:** fs state via `std::fs` (C2); the `FS_OPS` table as the method
census (C3 — the walk is over the same table the advertisement derives from,
so a 6th op added later is fenced automatically); error message ≠ "no resolved
host shell" (C4 — distinguishes the adapter refusal from the registry's).
**Stress fixture:** (a) gate-after-side-effect — C2's write target must not
exist after the call; (b) gate-on-4-of-5 — C3 iterates FS_OPS so a missed arm
fails its named op; (c) ext-arm-only gating — C4 calls the TYPED overrides
directly; an impl that only gates `handle_ext_request` passes shell_type but
fails create/wait/output/release/kill.
**Loop budget:** C3 walk O(5) ops × O(1) — trivial; no production loop added
(per-request `Option` check).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs`,
`crates/cyril-core/src/protocol/probe_dn91.rs` (flip fs/terminal/kiro_fs rows;
slice 1 becomes all-refused + unknown-null and is renamed to the C1–C4 fence).

**Verification:**
- [ ] Unit tests pass; new fences `v2_refuses_typed_fs`, `v2_refuses_kiro_fs_all_ops`, `v2_refuses_terminal_family`
- [ ] Existing KAS-bound fs/terminal tests pass unchanged (parity)
- [ ] `every_advertised_fs_flag_is_dispatched` (KAS-bound) still passes
- [ ] Budgets hold (O(5) walk)

## Slice 6: the advertise⇔answer matrix (one test, walked from data)

**Claim:** C9 (advertised(family) == answers(family), both facts in one test,
walked from `adapters()`) + C14 (unknown methods stay null under BOTH engines)
+ C8's regression net.
**Oracle:** serialized capabilities JSON (advertisement side) vs live
`KiroClient` call dispositions (execution side) — two independent observation
channels compared per family.
**Stress fixture:** the matrix is walked from `adapters()` data, not
hardcoded per-engine expectations — a future engine (or a mode change)
inherits coverage; the unknown-method row fails an over-broad refuse-by-default
rewrite (the C14 bug). Non-vacuity: remove any single family gate from slices
3–5 and its matrix cell fails with the family name in the assert message
(per-claim localization).
**Loop budget:** O(engines × families) = 2 × 4 rows (auth, host_io-fs,
host_io-terminal, hooks) + unknown row — ≤10 client calls, test-only.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs` (test module; or
`probe_dn91.rs` if it reads better as the probe's final form — implementer's
call, one file either way).

**Verification:**
- [ ] Matrix test passes on both engines under `--features kas`
- [ ] Killing any one family gate makes exactly that cell fail (spot-check one)
- [ ] Budgets hold (≤10 calls)

## Slice 7: default-build posture + live parity, both engines (AC6)

**Claim:** C11 (default build: compiles, tests pass, caps byte-identical to
empty; presence unconstructible) + C6 (live `kiro-cli acp` session per engine
behaves at parity).
**Oracle:** rustc + the default CI leg (C11); a real kiro-cli process (C6) —
the ultimate independent oracle. v2: `cargo run --example test_bridge --
--agent-command "kiro-cli acp"` completes a turn; handshake capabilities in the
recorded/logged init are the SAME empty set as before this branch. KAS: a live
session exercising an fs read + terminal command + hooks list still round-trips
(host-io families answer), per the KAS spawn config.
**Stress fixture:** the live KAS turn is chosen to force host callbacks (a
prompt that reads a file and runs a command) — an over-gated impl (the
inverted-gate bug) refuses live KAS traffic and the turn visibly fails; the v2
session catches any accidental capability advertisement (v2 handshake must stay
empty on the wire).
**Loop budget:** none.
**Wall budget:** live checks are one-shot manual (minutes), not always-on.
**Files:** none (verification only) + `.cyril-dn91/build-audit.md` records the
live evidence (session transcripts/log excerpts).

**Verification:**
- [ ] `cargo test -p cyril-core` (default) + `cargo test -p cyril-core --features kas` green
- [ ] `cargo clippy --all-targets` both configs `-D warnings`; `cargo fmt --check`
- [ ] Live v2 session: turn completes; init capabilities empty on the wire
- [ ] Live KAS session: fs/terminal/hooks callbacks answered; turn completes
- [ ] build-audit.md updated with evidence

---

## Plan Self-Review

1. **Loops:** S5's FS_OPS walk O(5); S6's matrix ≤10 calls — both test-only,
   no production loop added anywhere (every gate is an O(1) Option/enum check
   per request). No gaps.
2. **Fixtures:** every logic slice names its bug class: swapped mode-mapping
   (S1), double-hooks-key/presence-leak (S2), gate-after-notify (S3),
   gate-after-execute + sentinel + ungated-didChange (S4),
   gate-after-side-effect + 4-of-5 + ext-arm-only (S5), hardcoded-matrix +
   over-broad-refusal (S6), inverted-gate live + wire-visible caps (S7). No
   happy-path-only fixtures.
3. **Doc preconditions:** one — `settings_extra` must not carry presence keys:
   sanity-hint tier, `debug_assert!` in the derivation fn + C7 fence as the
   release-build net (S2). Refusal contracts are runtime checks by
   construction. No unenforced contracts.
4. **Write targets:** no stdout writes in any slice; refusal breadcrumbs are
   `tracing::debug!` diagnostics (matching `unhandled_ext_response`). No gaps.
5. **Tracker references:** mediator routing → cyril-g9vt; auth store/refresh →
   cyril-5db7 / cyril-taba; hooks hot-reload → cyril-2adk; executeHook echo
   hardening → cyril-qr6l; KAS refusal rendering → cyril-ker1. All verified
   open in `.cyril-dn91/related-issues.md` (2026-08-02 sweep). No uncited
   deferrals.

Claim coverage: C1(S3) C2(S5) C3(S5) C4(S5) C5(S4) C6(S7) C7(S2) C8(S2+S6)
C9(S6) C10(S4) C11(S1+S7) C12(S4) C13(S3) C14(S6) — all 14 covered.

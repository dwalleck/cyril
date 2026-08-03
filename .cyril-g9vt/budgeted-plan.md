# cyril-g9vt — budgeted plan (9 slices)

Design: `.cyril-g9vt/falsifiable-design.md` (approved; C13 cheapest falsifier
passed both configs). Cutover order per the issue: substrate → Auth → Host I/O
→ Hooks, parity-preserving, deleting each direct path in its own slice. No
slice writes stdout; all diagnostics are `tracing` (output-stream rule
satisfied vacuously; re-checked in self-review). No production loop anywhere
exceeds O(in-flight callbacks) with in-flight ≈ single digits; census/test
loops are O(19). Every slice gates on: tests both configs, clippy both
configs `-D warnings`, fmt, and the standing probe fences.

Staging rule (dn91 lesson): a new module lands `#[cfg(test)]`-declared OR in
the same commit as its first production consumer — never as unconsumed
production code under `-D warnings`.

---

## Slice 1: HostMediator pure state machine (tests-only staging)

**Claim:** C1 (accept() unit-testable sans async harness) + C12 substrate.
**Oracle:** the b4y4 TurnMediator pattern as structural reference (module
compiles with zero `protocol::kas` imports — the default build is the
mechanical proof); unit tests drive every `Accept` outcome synchronously.
**Stress fixture:** three bug classes: (a) register-after-return — a probe
callback accepted then immediately cancelled must abort (registration must
happen INSIDE accept, not in the spawned job); (b) duplicate cancel-key —
second registration with the same key is a distinct lifecycle entry (keys are
(kind, id)-scoped, not global); (c) cancel-unknown-key — log-drop, no panic,
state unchanged.
**Loop budget:** O(1)–O(log n) map ops per accept, n = in-flight ≤ ~8.
**Wall budget:** n/a (no always-on phase).
**Files:** `crates/cyril-core/src/protocol/host_mediator.rs` (new),
`crates/cyril-core/src/protocol/mod.rs` (`#[cfg(test)] mod host_mediator;`
until Slice 3 promotes it).

**Code (advisory):** `HostMediator<C: CallbackMeta>` holding
`HashMap<CancelKey, Entry>`; `accept(Envelope<C>) -> Accept<C>` with
`Accept::{Spawn(Job<C>), Abort(CancelKey), Consume}`; `Job::run(dispatch,
notify_tx)` does notify-then-resolve. `CallbackMeta { fn cancel_key(&self) ->
Option<CancelKey>; fn scope(&self) -> Option<SessionId>; fn kind(&self) ->
&'static str; }`.

**Verification:**
- [ ] Unit tests pass both configs (module is test-only this slice)
- [ ] Stress fixtures (a)(b)(c) pass
- [ ] probe_g9vt_c13 + concurrency probe still pass (untouched paths)
- [ ] Budgets hold (map ops)

## Slice 2: typed callbacks — enum, parse, meta (kas, tests-only staging)

**Claim:** C11 substrate (typed + exhaustive at the seam; no raw JSON
crosses).
**Oracle:** the dn91 census (19 variants, `.cyril-dn91/findings.md`) as the
completeness reference — the enum's variant count must equal it; parse
outputs compared against the covenant param shapes (docs/kiro-kas-acp-covenant.md).
**Stress fixture:** malformed params per family (executeHook without command,
fs read with non-string path, kiro_fs stat without sessionId): parse returns a
typed error that the client maps to invalid-params — NOT the old
`parse_ext_params` Null fallback (the bug class: Null-tolerant parsing
surviving into the typed seam). Plus field-preservation: fs read `line`/`limit`
round-trip through the typed struct (dropped-optional-field bug).
**Loop budget:** none (per-variant parse, O(1)).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/kas/callbacks.rs` (new),
`crates/cyril-core/src/protocol/kas/mod.rs` (`#[cfg(test)]`-staged decl).

**Verification:**
- [ ] Unit tests pass (kas config)
- [ ] Malformed-params + field-preservation fixtures pass
- [ ] Variant count == 19 asserted against a census table in the test
- [ ] Budgets hold (vacuous)

## Slice 3: ingress channel + loop arm + AUTH family cutover (promotes both modules)

**Claim:** C3 partial (auth crosses; concurrency preserved), C6 (failure
ordering now mediator-owned), C13 realized in run_loop.
**Oracle:** the committed concurrency probe (must still pass end-to-end);
the l7tw auth-failure fences migrated to the inline-mediator test seam
(BridgeError before error reply — the fence's channel/oneshot order is the
observable); acp per-request-spawn census for the off-loop property.
**Stress fixture:** (a) the migrated `auth_callback_err_emits_bridge_error` +
`auth_hint_not_doubled` + `non_auth_ext_err_emits_nothing` (their assertions
unchanged — parity); (b) notify-channel at capacity during a failing auth
resolve: ordering still holds and nothing deadlocks (bounded-send-inside-job
bug class); (c) dn91's `v2_refuses_auth_callback` + `auth_refusal_emits_no_bridge_error`
UNCHANGED (refusals never cross — C9's first cell).
**Loop budget:** O(1) per accepted callback; channel capacity const
(HOST_CAPACITY ≈ 16, documented).
**Wall budget:** n/a (arm is non-blocking by C4's contract, fenced in S7).
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (InternalChannels +
arm + mediator instantiation + dispatch ctx), `crates/cyril-core/src/protocol/client.rs`
(auth parse→send→await-oneshot; direct `respond_get_access_token` call
deleted) — module promotions in mod.rs files ride along (one-line each).
Test-support inline mediator lands here (client.rs test module).

**Verification:**
- [ ] All auth fences (migrated + dn91 refusal) pass
- [ ] Concurrency probe passes end-to-end
- [ ] grep: client.rs no longer calls kas::auth::respond_get_access_token
- [ ] Budgets hold

## Slice 4: HOST I/O cutover part 1 — typed fs + `_kiro/fs/*` (7 variants)

**Claim:** C10 partial (fs family parity with direct paths deleted), C11
partial.
**Oracle:** migrated dn91/kf2g fences (`read_text_file_override_returns_content`,
`write_text_file_override_writes_file`, `every_advertised_fs_flag_is_dispatched`,
`kiro_fs_ext_requests_route_to_responders`) — written pre-mediator, so they
are independent of it; fs side effects verified via `std::fs`.
**Stress fixture:** kiro_fs `delete` through the mediator still deletes (the
side effect IS the wiring proof — a mis-wired variant answers typed-but-wrong);
FS_OPS walk stays exhaustive (a 6th op added to the table without a callbacks.rs
variant must fail the census, not silently null); refusal cells (dn91
`v2_refuses_typed_fs`, `v2_refuses_kiro_fs_all_ops`) UNCHANGED.
**Loop budget:** O(1) per op.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs`,
`crates/cyril-core/src/protocol/kas/callbacks.rs` (dispatch arms) — kiro_fs.rs
itself unchanged (responders stay; only their call site moves).

**Verification:**
- [ ] Migrated fs fences pass; refusal fences unchanged-pass
- [ ] grep: client.rs no longer calls kas::host_io::* nor kiro_fs::dispatch
- [ ] Budgets hold

## Slice 5: HOST I/O cutover part 2 — terminal family (6 variants) + registry relocation

**Claim:** C10 partial; the 3lh8 `Rc` escape deleted (ADR escape #1).
**Oracle:** the existing terminal fences (create/wait/output/release/kill,
lw67 silent-no-op fences, 2z9g pipeline-cancel, 3lh8 reap-on-cancel, ba5x
shutdown sweep, ho7o wait-bound, cb93 command-line) — all written against
registry behavior, independent of mediation; `ps`-based liveness (portable).
**Stress fixture:** cancel-reaps-through-the-new-path: a session cancel with a
live terminal child must reap it via the mediator/ctx route (the 3lh8 fence
re-pointed — the bug class is the reap silently lost in the ownership move);
release-during-pending-wait (lw67) through mediation.
**Loop budget:** O(session's terminals) per cancel reap (existing bound).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (ctx owns the registry;
CancelRequest arm rewired; InternalChannels sheds the terminals field),
`crates/cyril-core/src/protocol/client.rs` (typed overrides parse→mediate;
terminals field removed from KiroClient).

**Verification:**
- [ ] Full terminal fence suite passes (ho7o/cb93/2z9g/lw67/3lh8/ba5x named)
- [ ] grep: KiroClient has no `terminals` field; no `client.terminals()` callers
- [ ] Budgets hold

## Slice 6: HOOKS cutover — 3 requests + 2 controls; inline arms deleted

**Claim:** C14 (control semantics survive), C10 partial, ADR escape #2
deleted.
**Oracle:** migrated hooks fences (list-routes-to-registry, sessionStart,
slow-hook-does-not-serialize, dn91 `registry_present_iff_inbound`,
`did_change_gated_by_hooks_direction`, outbound-refuses) — pre-mediator
authorship = independent.
**Stress fixture:** cancel-mid-execute through the mediator aborts the child
(kill_on_drop; fs side-effect absence is the observable — the bug class is
the op-registry lost in the hook_ops relocation); didChange under None still
dropped, Outbound still surfaces HooksChanged (direction gate survives the
control-notification rewire).
**Loop budget:** O(1) per control (map lookup by cancel-key).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs` (ext_notification hook
arms deleted; requests parse→mediate; hooks/hook_ops fields move),
`crates/cyril-core/src/protocol/kas/callbacks.rs` (dispatch arms + ctx).

**Verification:**
- [ ] Migrated hooks fences + dn91 direction fences pass
- [ ] grep: ext_notification contains no hooks arms; client has no hook_ops field
- [ ] Budgets hold

## Slice 7: seam scenarios — the mediator's behavioral contract

**Claim:** C2 (cancel-after-accept aborts unpolled + midflight), C4 (loop
live while a resolve is parked), C5 (bounded lossless backpressure), C7
(responder drop clean), C8 (shutdown aborts in-flight).
**Oracle:** per-scenario independent observables: fs side-effect ABSENCE
(C2's gated command would create a file), TurnCompleted arrival with a gate
still held (C4), oneshot resolution COUNT k-of-k at channel capacity 2 (C5),
mediator introspection + no-panic (C7), `ps` child liveness (C8). None of
these read mediator internals to define expected values.
**Stress fixture:** each scenario IS its named bug class (see design C2/C4/
C5/C7/C8 buggy-impl column); C5 additionally interleaves a cancel INTO the
backpressure queue (ordered acceptance under pressure — queue-jump bug).
**Loop budget:** test-only loops O(6).
**Wall budget:** parked-resolve tests bound their gates at ≤2s (CI-safe).
**Files:** `crates/cyril-core/src/protocol/host_mediator.rs` (unit half: C7 +
accept-level C5), `crates/cyril-core/src/protocol/bridge.rs` (harness half:
C2, C4, C5 end-to-end, C8).

**Verification:**
- [ ] All five seam fences pass, each failing under its named mutation
  (spot-check one mutation per the checkpointed-build rule)
- [ ] Budgets hold

## Slice 8: census + deletion fences

**Claim:** C11 (all 19 variants cross — accept-log census), C10's deletion
census (`client_no_longer_resolves_directly`), C9 re-affirmed.
**Oracle:** the dn91 19-variant census table (probe-derived, pre-mediator);
source text for the deletion census (grep over client.rs for responder-call
patterns — independent of the runtime path).
**Stress fixture:** mutation spot-check: re-route ONE variant (shell_type)
back to direct resolution — the census must fail exactly that variant's
accept-log cell (localization), and the deletion grep must fire.
**Loop budget:** census O(19) client calls, test-only.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (census harness test)
or `probe_dn91.rs`-style module — implementer's call, one file; the deletion
census as a source-reading test alongside.

**Verification:**
- [ ] Census passes; mutation spot-check localizes
- [ ] Deletion census green (zero direct responder calls in client.rs)
- [ ] Budgets hold

## Slice 9: live parity both engines + audit (C15, AC6)

**Claim:** C15; C13 final re-verify.
**Oracle:** real kiro-cli 2.16.0. v2: full test_bridge harness sequence.
KAS: `--agent-engine kas` with a prompt forcing fs read + shell command
(`echo g9vt-live-ok` — terminals work live post-cb93) + the workspace stop-hook
firing; every family answers through the mediator; turn completes.
**Stress fixture:** the live KAS turn exercises auth (spawn gate + callback),
fs, terminal, and hooks in ONE session — an over-gated or deadlocked mediator
visibly fails the turn; the v2 session catches any accidental traffic (v2
must send nothing across the host channel — assert via the accept-log left
empty in a debug run or absence of host-arm tracing).
**Loop budget:** none.
**Wall budget:** live checks one-shot manual (minutes).
**Files:** none (verification) + `.cyril-g9vt/build-audit.md`.

**Verification:**
- [ ] `cargo test` both configs; clippy both configs; fmt — all green
- [ ] Live v2 session: full sequence, no host-channel traffic
- [ ] Live KAS session: auth+fs+terminal+hooks all answer; turn completes
- [ ] build-audit.md records evidence + any deviations

---

## Plan Self-Review

1. **Loops:** mediator map ops O(1)-O(log n) at n≤~8 in-flight; cancel reap
   O(session terminals) (pre-existing bound); census/backpressure test loops
   O(6)-O(19). All far under budget; no always-on phases added.
2. **Fixtures:** every slice names bug classes: register-after-return /
   dup-key / unknown-key (S1); Null-tolerant parse + dropped-optional-field
   (S2); ordering-under-full-channel + refusal-cells-unchanged (S3);
   side-effect-as-wiring-proof + table-drift (S4); reap-lost-in-relocation +
   release-mid-wait (S5); op-registry-lost + direction-gate-lost (S6); the
   five named seam mutations + queue-jump (S7); re-routed-variant
   localization (S8); over-gated-live + v2-accidental-traffic (S9). No
   happy-path-only fixtures.
3. **Doc preconditions:** two identified: (a) "cancel keys are (kind,
   id)-scoped" — load-bearing (a global-keyed cancel could abort a stranger):
   runtime-enforced by the key TYPE (CancelKey carries kind), not an assert;
   (b) "Envelope must carry a live responder for request-kind callbacks" —
   sanity-hint tier (in-crate construction, C7 fences the drop path):
   `debug_assert!` at accept. No unenforced contracts.
4. **Write targets:** none to stdout; all diagnostics `tracing` events.
5. **Tracker references:** stages gates → ADR-0003/Phase 2 (roadmap-anchored,
   settled rationale — the seam ships, consumers are future phases per the
   ADR itself); store injection → cyril-5db7 (verified open); new surfaces →
   cyril-nk4o, cyril-3ald (verified open); refusal rendering → cyril-ker1
   (verified open). No uncited deferrals.

Claim coverage: C1(S1) C2(S7) C3(S3) C4(S7) C5(S7) C6(S3) C7(S7) C8(S7)
C9(S3+S8) C10(S4-S6,S8) C11(S2,S8) C12(S1, continuous) C13(S3, continuous)
C14(S6) C15(S9) — all 15 covered.

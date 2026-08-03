# cyril-g9vt — falsifiable design: the host-callback mediator

Implements the ADR-0004 amendment: every **handled** host-callback request and
control notification crosses one bridge-internal mediator type; the direct
KiroClient resolution paths it replaces are deleted. Probe evidence:
`.cyril-g9vt/findings.md` (concurrent resolution proven at the connection
layer; callbacks do not nest inside turns; symmetric underscore escape;
19-variant census carried from dn91).

## Purpose

One seam: KiroClient parses ACP payloads into **typed callbacks**, the
**HostMediator** owns their lifecycle (ordered acceptance, concurrent
resolution, cancellation, responder drop, shutdown, failure ordering), and the
**adapter-side dispatch** resolves them against the Engine-selected adapter
set (ADR-0001). Adding a capability touches the mediator zero times.

## Architecture and placement (step 2c)

Three owners, all `pub(crate)`:

1. **`protocol/host_mediator.rs`** (new; sibling and structural twin of
   `turn_mediator.rs`, the b4y4 precedent). `HostMediator<C: CallbackMeta>` —
   a pure state machine: `accept(envelope) -> Accept` where `Accept` says
   what the caller does next (`Spawn(job)` / `Abort{key}` / `Drop{reason}`).
   Registration happens inside `accept`, BEFORE any resolution: ordered
   acceptance is the single-consumer channel order. The mediator is generic
   over the callback type via a small `CallbackMeta` trait (cancel key,
   session scope, kind label) and an injected dispatch function — it imports
   nothing capability-specific, which the DEFAULT build proves mechanically
   (kas types don't exist there, yet the module compiles and the loop arm
   matches exhaustively on an uninhabited item — falsifier C13, **passed**).
2. **`protocol/kas/callbacks.rs`** (new, kas-gated): the typed
   `HostCallback` enum (17 request variants + 2 control variants, the dn91
   census), per-variant **parsed** params (no raw `serde_json::Value` crosses
   the channel), `CallbackMeta` impl, and `dispatch(callback, ctx) ->
   Outcome` — an exhaustive match resolving against the adapter-side
   responders (auth, host_io fs/kiro_fs/terminals, hooks). `Outcome` carries
   `{ notify: Option<Notification>, reply: TypedReply }` so failure ordering
   is data, not control flow. The terminal registry and hook-op table move
   INTO the dispatch context owned by the loop side — deleting the 3lh8 `Rc`
   escape and the inline `ext_notification` hook-cancel arm.
3. **`protocol/bridge.rs`**: `InternalChannels` grows the bounded host
   channel (`#[cfg(kas)]` producer side in KiroClient; the loop-side item is
   uninhabited in default builds); `run_loop` gains ONE thin arm:
   `Some(env) = host_rx.recv() => match mediator.accept(env) { Spawn(job) =>
   spawn_local(job.run()), … }` — no callback policy inline.

**Refusals stay parse-time in KiroClient** (design interpretation, surfaced
for approval): an un-adaptered family has no adapter to dispatch to, so it is
refused at the protocol adapter exactly as dn91 built it — same wire shape
(-32601 pre-side-effect), and every dn91 refusal fence passes VERBATIM
(those tests construct KiroClient with no loop; routing refusals through
mediation would hang them). "Handled" in AC1 = has an adapter.

**Forbidden:** `run_loop` holding callback policy inline; `host_mediator.rs`
importing `protocol::kas` or naming capability specifics beyond
`CallbackMeta`; raw JSON or opaque closures crossing the channel; any of
these types escaping `cyril-core::protocol`; KiroClient resolving a handled
callback directly (per-family deletion is part of each cutover slice).

**Test seam:** KiroClient unit tests that exercise HANDLED paths get a
test-support inline mediator (a `LocalSet`-local consumer draining the host
channel through the real accept/dispatch), so existing fences migrate with
minimal edits; seam-scenario tests use the FakeAgent harness.

## Input shapes (step 2)

1. **Variants:** 17 requests (auth ×1; fs typed ×2; `_kiro/fs/*` ×5; terminal
   ×5 + shell_type; hooks list/execute/sessionStart) + 2 controls (hooks
   cancel, didChange) — each gets a parse + dispatch arm (C11 walks all).
2. **Engine × family:** KAS with adapter (crosses), V2-in-kas-build
   (refused parse-time, never crosses — C9), default build (uninhabited item,
   no traffic — C13).
3. **Channel states:** empty / mid / FULL (backpressure, C5) / closed
   (shutdown, C8).
4. **Lifecycle interleavings:** cancel-before-accept (no target: log-drop;
   wire order = acceptance order, so a true pre-cancel means the agent
   cancelled the unsent), cancel-after-accept-before-resolve (aborts, C2),
   cancel-mid-resolve (aborts; kill_on_drop reaps — existing 2z9g/lw67/3lh8
   fences), cancel-after-resolve (no-op), responder-dropped (C7),
   resolution-outliving-turn_end (probe finding — normal, C3's harness shape),
   shutdown-with-inflight (C8).
5. **Outcome shapes:** reply-only; notify+error (auth failure, C6);
   notify-only (didChange→HooksChanged); neither (cancel consumed).
6. **Out of scope:** permission requests (standard ACP path, ADR settled);
   unknown ext methods (protocol-default null at parse, dn91 C14 fence);
   malformed params on refused families (refusal precedes parsing).

## Removed-invariant sweep (step 2b)

Subtractive move: resolution leaves the acp per-request task's sole custody
(parse→resolve inline) and splits into enqueue → accept → spawned resolve.

- *"Resolution starts the instant the request arrives"* — now bounded by loop
  acceptance latency; must NOT become "loop awaits resolution" or a gated
  callback starves everything → C4 (loop stays live while a resolve is
  parked) + C3 (concurrency preserved end-to-end).
- *"Cancel-before-register can't happen"* (single-thread spawn order made
  execute's sync register always precede the cancel task) — the mediator makes
  this STRUCTURAL (register in accept, channel-ordered) instead of
  incidental → C2.
- *"The loop can reach terminals via the 3lh8 Rc"* — replaced by mediator/
  dispatch-context ownership; reap-on-cancel must still hold → existing 3lh8
  fence + C8.
- *"Client tests run without a loop"* — broken for handled paths; restored by
  the inline-mediator test seam → C10 (migrated fences green).
- *Auth notify-then-reply code order in ext_method* — becomes mediator-owned
  outcome ordering → C6.

## Claims and falsification

| # | Claim | Falsifier (input → expected; buggy impl that fails it) | Oracle | Cost | Status | Regression fence |
|---|-------|--------------------------------------------------------|--------|------|--------|------------------|
| C1 | HostMediator is a pure-state-machine type: `accept()` unit-tested with NO async harness; run_loop's arm only delegates | in-module unit tests drive accept through every `Accept` outcome synchronously; buggy: policy written into the select! arm — the unit tests could not exist | tests compile+run without tokio harness (b4y4 pattern precedent) | build | pending | `host_mediator::tests::*` |
| C2 | Acceptance order = channel order, registration precedes resolution: a cancel accepted after its target aborts it even if resolution hasn't polled yet | send execute(gated)+cancel back-to-back through the harness; hook must NOT run; buggy: register inside the spawned job (post-accept) — cancel finds nothing | fs side-effect absence (gated command would create a file) + accept-log order | test | pending | `cancel_after_accept_aborts_unpolled_and_midflight` |
| C3 | Concurrent resolution survives mediation end-to-end | the committed probe (slow 1s + fast 334µs, non-null bodies) reruns UNCHANGED through the mediated loop; buggy: loop awaits the job → fast≈slow | probe timings vs acp per-request-spawn census (`.cyril-g9vt/oracle-acp-rpc.txt`) | test | pending (passes pre-cutover) | `probe_g9vt_concurrent_callback_resolution` (existing) |
| C4 | run_loop never awaits resolution: with one resolve parked on a gate, a full prompt turn still completes | harness: park a hook resolve on Notify; drive NewSession+prompt to TurnCompleted; buggy: accept().await-ing job completion — turn never completes | TurnCompleted arrival while gate still held | test | pending | `loop_processes_turns_while_callback_parked` |
| C5 | Ingress is bounded and lossless: at capacity N, N+k callbacks all resolve, none dropped, producers await | test-constructed mediator channel with N=2, 6 concurrent requests; buggy: try_send/drop — some oneshots never resolve | all k oneshots resolve with typed replies (count) | test | pending | `backpressure_awaits_capacity_losslessly` |
| C6 | Failure ordering: outcome{notify, err} sends the App notification BEFORE the responder resolves | injected failing dispatch (the seam makes auth failure testable without the real store); buggy: resolve-then-notify — order observably inverted | channel-recv vs oneshot-completion order under a controlled LocalSet | test | pending | `failure_notification_precedes_error_reply` |
| C7 | A dropped responder mid-resolve cleans the lifecycle entry, logs, never panics | drop the oneshot rx before resolve completes; buggy: `.expect()` on the send / entry leaked (visible via mediator introspection) | mediator state introspection + no-panic | test | pending | `responder_drop_is_clean` |
| C8 | Shutdown aborts in-flight resolutions and reaps terminal children | shutdown with a parked resolve + live terminal child; buggy: shutdown ignores in-flight — child survives, loop hangs | `ps` liveness (portable) + existing ba5x/3lh8 fences | test | pending | `shutdown_aborts_inflight` + existing ba5x |
| C9 | Refusal parity: un-adaptered families refuse at parse time and never cross the mediator — every dn91 refusal fence passes VERBATIM | run the dn91 suite unchanged; buggy: refusals routed through mediation — the loop-less unit tests hang/time out | existing dn91 fences (written pre-mediator, independent) | test | pending | dn91 suite (unchanged files) |
| C10 | Handled-path parity: every migrated KAS-bound fence (fs/kiro_fs/hooks/terminal semantics, matrix, auth BridgeError) passes with the direct client paths DELETED | run migrated suite; grep client.rs for direct responder calls = zero; buggy: a family left on the direct path — grep fence fires | migrated fences + `grep` deletion census | test | pending | migrated suite + `client_no_longer_resolves_directly` (source census test) |
| C11 | Exhaustive wiring: all 19 variants parse→accept→dispatch, walked from one census table | census test drives every variant through the client entry points and asserts each reached its family responder (typed non-null outcome) + the mediator accept-log saw each kind; buggy: one typed override resolving directly — its accept-log entry is missing | accept-log (test-mode) + typed outcomes | test | pending | `every_handled_variant_crosses_the_mediator` |
| C12 | Zero-touch depth: the mediator module names no capability — it is generic over `CallbackMeta` and compiles in a default build where the kas callback type does not exist | default-build compile IS the mechanical proof (no un-cfg'd kas import can exist); buggy: mediator matching on capability variants — default build breaks or cfg spreads | rustc, default CI leg | 5m | **passed-in-principle** (C13 scratch) | default CI leg + module review |
| C13 | The default-build loop arm compiles with an uninhabited item and drains as closed | scratch test `probe_g9vt_c13` (both configs); buggy: cfg'd run_loop signature fork — the single-arm pattern wouldn't compile | rustc both configs | 10m | **passed** | `uninhabited_channel_arm_compiles_and_recv_is_none` (kept) |
| C14 | Control-notification semantics survive: didChange direction-gate (None drop / Outbound→HooksChanged) and op-keyed cancel behave as the dn91 fences specify | migrated `did_change_gated_by_hooks_direction` + C2; buggy: controls left inline in ext_notification — C11's census misses them | migrated fences | test | pending | migrated didChange fence + C2 |
| C15 | Live parity, both engines (AC6): a live v2 session and a live KAS session (fs read + shell command + hooks — terminals now WORK live post-cb93) behave identically through the mediator | test_bridge harness runs, before/after transcripts; buggy: over-gated or deadlocked mediation visibly fails the live turn | real kiro-cli 2.16.0 | 30m manual | pending | CI proxy: C3+C11; live evidence in build-audit (dn91 precedent) |

Cheapest falsifier (C13) ran before presenting: **passed both configs**
(committed as `probe_g9vt_c13.rs`).

## Negative space

1. **Permission requests do not move** — the standard ACP human-decision path
   stays as-is (ADR amendment, settled rationale).
2. **No stages/proxy gate is built** — this creates the interception seam
   ADR-0003's Phase 2 consumers will use; the gates themselves are that
   phase's work, not this PR's.
3. **No vendor-neutral mediation** — Kiro-scoped per ADR-0001/0004; a second
   vendor triggers that design.
4. **Refusal policy unchanged** — parse-time, dn91's shapes; only handled
   callbacks cross (interpretation surfaced for approval).
5. **No auth store injection** — cyril-5db7 (verified open); C6's injected
   dispatch is a mediator-level seam, not a store seam.
6. **Turn mediation untouched** — TurnMediator (b4y4) keeps sole ownership of
   turn state; the two mediators do not merge.
7. **No new KAS-8 surfaces** — `_kiro/secret/*`, `_kiro/mcp/*` (cyril-nk4o,
   verified open), `_kiro/safety/*` (cyril-3ald, verified open) arrive later
   as new adapters + variants; C12 is what makes that a zero-mediator-touch
   event.

## Consequences for existing artifacts

- The g9vt concurrency probe becomes the C3 fence unchanged; `probe_g9vt_c13`
  survives as the C13 fence.
- dn91's refusal fences are untouched (C9); its handled-path client fences
  migrate to the inline-mediator seam per family slice (C10).
- ADR-0004 amendment needs no text change — this implements it; CONTEXT.md
  gains the mediator vocabulary at the glossary step.

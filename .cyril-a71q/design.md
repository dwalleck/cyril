# Falsifiable design — cyril-a71q (re-anchored)

Date: 2026-07-12
Status: **DESIGN GATE PASSED; review approval is not claimed**
Supersedes: `design-superseded-sole-turn-end.md` (voided contract) and
`design-superseded-either-source.md` (impossibility valid only for the over-generalized
input space — see `timing-audit.md`).

## Purpose

Associate every accepted prompt with a process-local `TurnId(u64)` owner so only that
turn's terminal observation can release it, while **retaining first-source-wins release**
(cyril-j16p). The second same-turn terminal signal is an *expected companion*: absorbed,
its `{source, stop_reason}` recorded for cyril-pnwb, never forwarded as a second
completion. Preserve synthesized global v1/v2 completion, scoped foreign routing,
fail-stop visibility, exactly-once App effects, checked `u64` identity, and
bounded-channel delivery.

The signed contract (spec.md, requester sign-off "I confirm these consequences",
2026-07-12) anchors KAS input on the researched wire order: one scoped `turn_end`
followed by one RPC response per accepted prompt, both normally present. Single-source
turns are degenerate drift handled safely (liveness first); correctness must be
receipt-order-independent.

## Input shapes

| Input family | Production shapes | Required disposition | Claims |
| --- | --- | --- | --- |
| Engine | `V2`; `Kas` | v2: one id-stamped global terminal. KAS: id-stamped synthesized response + identity-free scoped `turn_end`, first-wins. | C1, C2 |
| `SendPrompt` guard | no active owner; active same-session; active distinct-session | Accept only the no-active case after owner allocation; active cases emit Busy, start zero RPCs. | C1, C3 |
| Owner allocator | initial `0`; interior; final unused `u64::MAX`; exhausted | Allocate each value once; exhaustion starts zero turns, fails closed visibly. | C8 |
| KAS receipt order | `turn_end`→response (researched); response→`turn_end` (jitter); single source (drift) | Identical observable outcome for both orders: 1 completion, correct owner released, all arrived observations recorded. Single source releases (liveness); companion expectation dangles safely. | C1, C2, C6 |
| Synthesized completion owner | active owner; expected companion owner; stale owner; v2/KAS | Active id-match releases; expected id-match absorbs+records; stale drops. Never session-matched. | C1, C2 |
| Scoped `turn_end` scope | expected-companion session; active KAS owner's session; foreign; no-active same-session | Absorb-first: dangling expectation absorbs+records; else active session-match releases; foreign routes once; no-active drops. | C1, C3, C6 |
| `turn_end.stopReason` | valid; missing; malformed | Valid preserved; missing/malformed defaults `EndTurn`, still releases. | C2, C6 |
| Repeated scoped `turn_end` | 2+ frames per accepted prompt | Unsupported drift; no ownership-safety claim (identity-free). Best-effort absorption when an expectation dangles. | C1 (boundary), B10 |
| Companion ledger | empty; one entry; entry replaced by newer release | At most 1 entry; registering a new expectation replaces a dangling one; absorbs at most one signal. | C6 |
| Cancellation | no active; active T; mid-turn session retarget; both sources after cancel | Cancel targets T's immutable owner session; T releases via first source exactly once; both reasons recorded (live capture: both `cancelled`). | C4 |
| Prompt `Err` / process death | idle; active T; race with terminal sources | `BridgeError` → owner-keyed `TurnCompleted` → `BridgeDisconnected`; no B accepted in the failed lifetime; stale/companion signals cannot satisfy another owner's deferred disconnect. | C5 |
| Shutdown | 0, 1, or 2 live bridge-owned prompt futures | Abort all; 0 required completions; none survives `run_loop` exit holding the connection. | C4 |
| Rate-limit observation | before/after terminal sources; one/repeated | Forwards 0 completions, releases 0 turns by itself; the turn still releases via its first terminal source (restored cyril-3zy4 criterion). | C10 |
| Routed scope | `None` (global); `Some(main)`; `Some(foreign)` | Global matched by stamp only; foreign forwarded once, zero main mutation. | C2, C3, C7 |
| Backpressure | 0–255; 256 full; blocked terminal 257th; resumed receiver | All 257 delivered once in order; release/absorption applied in receipt order. | C9 |

## Removed invariants

The change is **subtractive** relative to shipped code: it removes "any `TurnCompleted`
while a session is in flight may clear it" and removes "first-wins dedup by bare
`Option<SessionId>` presence." It does NOT remove prompt-response release authority (the
voided design did; reversed by the signed re-anchor).

| Sweep target | Removed/exposed assumption | Still-holds replacement | Claim |
| --- | --- | --- | --- |
| `turn_in_flight` | `Option<SessionId>` treated as terminal epoch | Active record `{owner, engine, session}`; Busy/cancel/release read it. | C1, C3 |
| First-wins dedup | "no turn in flight ⇒ duplicate" breaks once same-session B starts | Companion ledger keyed to the released owner absorbs the one remaining signal; id-stamp catches stale synthesized signals with no expectation. | C1, C6 |
| Late global response | generic completion could clear B | Owner stamped at dispatch; id mismatch drops/absorbs, never releases B. | C1, C2 |
| Scoped foreign `turn_end` | bridge guard could clear before App routes foreign (split-brain) | Foreign routes once, mutates zero main state. | C3, C7 |
| `prompt_task` single handle | accepting B could orphan A's still-resolving task (voided design aborted it instead) | Bridge owns ≤2 futures (active + companion-pending); shutdown/death aborts all; none outlives `run_loop`. | C4 |
| Pending disconnect | any completion could satisfy it | Keyed to the dying owner. | C5 |
| Cancellation target | mutable current session | Immutable owner session from the active record. | C4 |
| App/session/UI effects | every forwarded main completion assumed applicable | Only the owned release forwards; absorbed/stale/foreign/rate-limit cause zero main completion transitions. | C7 |
| Source/reason provenance | first-wins dedup erased the second source's reason | Companion absorption records it; ≤2 observations per turn, both orders. | C6 |
| Identity uniqueness | counter wrap would recreate an owner | Checked allocation; fail-closed exhaustion. | C8 |
| 256+1 delivery | classification could be lost at capacity | Awaited lossless forwarding; semantics in receipt order. | C9 |

## Architecture

### Owner allocation and active state

- `TurnId(u64)` newtype (CLAUDE.md newtype rule; mirror `TerminalRegistry`'s counter
  idiom but with **checked** — not saturating — allocation), allocated at `SendPrompt`
  accept, before RPC dispatch.
- Active record `{owner, engine, session}`; prompt futures keyed by owner (≤2 live).
- Synthesized completions (v2 response, KAS response, error-path) carry `Some(owner)`;
  the KAS wire `turn_end` conversion carries `None` (no native id — prototype-proven).
  A legitimate `Option`-for-absent, not a sentinel.

### Mediation policy (the falsified model)

On id-stamped completion:
1. matches the expected companion's owner → absorb: record `{source, reason}`, clear
   ledger, forward 0;
2. matches the active owner → release: forward exactly 1, record, register the wire
   companion expectation (KAS only);
3. otherwise → stale: drop, forward 0.

On scoped `turn_end` (identity-free), **absorb-first**:
1. session matches a dangling wire-companion expectation → absorb: record, clear, 0;
2. session matches the active KAS owner → release: forward 1, record, register the
   synthesized companion expectation (id-keyed);
3. foreign session → route once to that consumer, zero main mutation;
4. otherwise → drop.

Absorb-first vs release-first: observationally identical under supported input (the
researched order + same-thread FIFO mean a wire expectation never survives to meet the
next turn's `turn_end` — timing-audit §2); under single-drift, absorb-first is safe
(stale frame absorbed) where release-first wrongly clears the newer turn (falsifier
mutation M3). Under double-drift (two simultaneous unsupported producer omissions),
absorb-first defers liveness to the fail-stop lifecycle — the signed residual.

### Lifecycle

- Cancel targets the active owner's immutable session; release still via first source.
- Prompt `Err`/death: `BridgeError` → owner-keyed `TurnCompleted` → `BridgeDisconnected`;
  the failed lifetime accepts no further prompt.
- Shutdown aborts every bridge-owned future (≤2); fresh process = fresh allocator,
  channels, ledger.

### Evidence seam (cyril-pnwb)

Per turn, ≤2 recorded observations `{source: turn_end|response|failure, reason}` —
populated on release AND on absorption, in either receipt order. The forwarded completion
carries the first source's reason without authority. No precedence is selected.

## Claims

1. **C1 — ownership safety:** A late id-stamped completion or an expectation-matched
   stale `turn_end` forwards 0 completions and releases 0 newer turns; each accepted turn
   forwards at most 1 completion; B is accepted only after A releases.
2. **C2 — engine compatibility and order independence:** v2's id-stamped global releases
   its owner (no session comparison — no freeze); KAS yields identical observable outcome
   for both receipt orders and releases on a single source (liveness).
3. **C3 — scope isolation:** Foreign scoped completions forward once to their routed
   consumer with zero main mutation; unowned same-session/no-active signals drop.
4. **C4 — task/cancel/shutdown ownership:** ≤2 bridge-owned futures; cancel targets the
   immutable owner session; shutdown aborts all with 0 required completions and none
   outlives `run_loop`.
5. **C5 — lifecycle fail-stop:** Error/death preserves `BridgeError` → owner-keyed
   `TurnCompleted` → `BridgeDisconnected`; no B in the failed lifetime; only the dying
   owner's marker satisfies its deferred disconnect.
6. **C6 — evidence completeness:** Both sources' `{source, reason}` are recorded whenever
   both arrive, in either order (absorption records, never discards); missing/malformed
   `turn_end` reason defaults `EndTurn`; ledger holds ≤1 entry.
7. **C7 — consumer effects:** Exactly one owned release causes one SessionController and
   UiState completion transition; absorbed/stale/foreign/rate-limit observations cause
   zero.
8. **C8 — exhaustion:** The final unused `u64` allocates once; the next request starts 0
   turns, emits 1 visible fail-closed signal.
9. **C9 — backpressure:** 256 queued + blocked terminal 257th all deliver once in order
   on resume; release/absorption applied in receipt order.
10. **C10 — rate-limit nonauthority with restored liveness:** Rate-limit observations
    alone release 0 turns; a rate-limited turn still releases via its first terminal
    source (the response), accepting the next prompt.

## Falsification

| # | Claim | Falsifier and falsifying result | Independent oracle | Concrete buggy implementation | Cost | Status | Deterministic fence / distinct output | Blindness |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C1 | Ownership safety | Traces T3 (late stamped stale during B) and T4 (late wire `turn_end` during B): any B release, completion count ≠1 per turn, or early B acceptance falsifies. | Hardcoded per-trace disposition tables + hidden turn labels (harness-only). | Session-only matching (M1); no ledger (M2); release-first (M3). | <1s | **passed** | `design_reanchored_falsifier.py`; `T3.*`/`T4.*` | B1, B11 |
| C2 | Engine compat / order independence | T1 vs T2 (both orders) must produce identical completion count, released owner, evidence set; T7 v2 id-release; single-source trace releases. | Same hardcoded tables; v2 freeze detected by accept-rejection. | v2 session-matching (M4) — the tracker's named freeze constraint. | <1s | **passed** | `T1.*`/`T2.*`/`T7.*` | B1, B8 |
| C3 | Scope isolation | T6: foreign scoped completion during main B — main release, foreign loss, or B un-busy falsifies. | Routed-consumer count ledger. | Clear-on-variant before scope routing. | <1s (model) + 3m (impl fence) | **passed at model seam** | `T6.*`; impl fence `terminal_scope_owner_matrix` | B2, B9 |
| C4 | Task/cancel/shutdown | Park A's companion-pending future, accept B, shutdown: any surviving future, >2 live, or wrong cancel session falsifies. | Task-drop sentinels + wire cancel transcript. | Single overwritten handle orphaning A's task. | 4m | pending implementation | `active_prompt_futures_bounded`; `C4-TASK` | B4, B11 |
| C5 | Lifecycle fail-stop | T5 tail: fail-stop order + B rejected after; implementation fence injects death racing terminal sources. | Ordered channel recorder; model asserts exact order. | Resume command loop after error+completion. | <1s (model) + 4m (impl) | **passed at model seam** | `T5.lifecycle_order`; impl fence `kas_error_is_failstop` | B4, B17 |
| C6 | Evidence completeness | T1/T2 evidence-set assertions: lost companion tuple, wrong fallback, or >1 ledger entry falsifies. | Expected evidence sets encoded separately from policy. | First-wins dedup that drops instead of absorbs (M2 fails `T1.both_evidence` distinctly). | <1s | **passed** | `T1.both_evidence`/`T2.both_evidence` | B3, B10, B11 |
| C7 | Consumer effects | Snapshot SessionController/UiState around owned, absorbed, stale, foreign, rate-limit events; any non-owned main delta falsifies. | Public-state delta table + routed receiver identity. | Forward absorbed companion and rely on consumers to dedupe. | 4m | pending implementation | `only_owned_completion_mutates_main`; `C7-EFFECTS` | B2, B13 |
| C8 | Exhaustion | Inject final-unused and exhausted allocator states; skip/reuse/second-signal falsifies. | Arithmetic boundary fixture + server prompt count. | `wrapping_add` / `saturating_add` / same-process reset. | 1m | pending implementation | `turn_owner_exhaustion_fails_closed`; `C8-EXHAUSTION` | B6, B12 |
| C9 | Backpressure | Pause receiver, fill 256, block terminal 257th, resume, reconcile order/ownership. | Independently generated ID range + receiver-order ledger. | `try_send` drop or clear-before-delivery. | 3m | pending implementation | `owned_terminal_survives_256_backlog`; `C9-BACKPRESSURE` | B7, B12 |
| C10 | Rate-limit | Inject rate limits around both receipt orders; any observation-alone release, or a rate-limited turn left Busy after its response, falsifies. | App completion count + fake-server prompt transcript. | Map `RateLimited` to release; or (voided-design bug) response never releases. | 2m | pending implementation | `rate_limited_turn_releases_via_response`; `C10-RATE-LIMIT` | B5, B12 |

All pending fences are deterministic CI obligations for implementation, not manual
measurements or a build plan.

## Cheapest falsifier result

Persisted artifacts:

- `.cyril-a71q/probes/design_reanchored_falsifier.py`
- `.cyril-a71q/probes/output/design-reanchored-cheapest-falsifier.txt`

Command: `python .cyril-a71q/probes/design_reanchored_falsifier.py`

Output (verbatim tail):

```text
REANCHOR correct_policy_failures=[]
REANCHOR M1_session_only failed=['T7.B_still_busy', 'T7.stale_stamped_drops']
REANCHOR M2_no_ledger failed=['T1.both_evidence', 'T1.companion_absorbed', 'T2.both_evidence', 'T2.companion_absorbed', 'T4.B_not_wrongly_released', 'T4.B_releases_via_own_synth', 'T4.ambiguous_absorbed', 'T5.same_action_as_T4', 'T5.signed_residual_busy']
REANCHOR M3_release_first failed=['T4.B_not_wrongly_released', 'T4.B_releases_via_own_synth', 'T4.ambiguous_absorbed', 'T5.same_action_as_T4', 'T5.signed_residual_busy']
REANCHOR M4_v2_session_match failed=['T2.both_evidence', 'T2.companion_absorbed', 'T2.release_on_response', 'T4.B_not_wrongly_released', 'T4.B_releases_via_own_synth', 'T4.ambiguous_absorbed', 'T4.safety', 'T5.same_action_as_T4', 'T5.signed_residual_busy', 'T7.B_still_busy', 'T7.v2_not_frozen', 'T7.v2_releases_on_id']
REANCHOR distinct_mutation_signatures=4/4
REANCHORED-CHEAPEST-PASSED
```

Non-vacuity: the correct policy fails 0/34 assertions; each of four concrete buggy
policies fails a non-empty, pairwise-distinct assertion set. Notable structure: the
ledger alone masks session-only matching in T3 (absorption fires before the buggy release
rule can) — M1 is instead caught by the no-expectation stale case (T7), showing the
id-stamp and the ledger are independently load-bearing, not redundant.

**Resolution of the superseded impossibility:** T4 (History 1) and T5 (History 2) present
identical visible input at the ambiguous `turn_end`; the policy takes the SAME action
(absorb) in both, and the outcome is safe in both — A's stale frame cannot clear B (T4),
and B still completes via its own id-stamped response when the producer honors the
contract. The superseded proof assumed a per-frame release-or-drop decision was required;
the ledger plus the always-identified second source removes that requirement. The one
residual (T5: double-drift leaves B Busy until the fail-stop lifecycle) is signed in
spec.md and requires two simultaneous unsupported producer omissions.

## Material-boundary accounting

Every material boundary in `prototype.md` is carried here.

| Prototype boundary | Design disposition |
| --- | --- |
| Live KAS terminal wire shape | C1/C2/C6 use the two pinned no-turn-id frames; researched order justifies bounds (ledger size), never correctness. |
| KAS terminal conversion/routing entry | C1/C3/C6 cross converter → scoped route → mediator. |
| KAS fixed-pair liveness/current dedup | C1/C2/C6 replace first-wins-by-session with first-wins-by-owner + companion absorption; liveness retained. |
| Notification backpressure substrate | C9 covers 256+1. |
| Active-turn guard | Sweep + C1/C3 replace session-presence semantics. |
| Completion release guard | C1/C2 require owner/scope classification. |
| Global v1/v2 path | C2 preserves synthesized global completion via id-match. |
| Scoped KAS path | C1/C2/C3 cover owned, expected-companion, foreign, no-active scopes. |
| App foreign-session boundary | C3/C7 preserve early-return routing. |
| Same-session stale ownership | C1: id mismatch or expectation absorption; never a newer-turn release. |
| Cross-session ownership | C3/C7. |
| KAS distinct-source ownership | C1/C6: one release + one absorbed companion per turn, both orders. |
| KAS prompt-response-only authority | C2/C10: the response releases (liveness restored); the dangling expectation degrades safely. |
| Shutdown/process lifetime | C4: ≤2 futures, all aborted, fresh-domain isolation. |
| Prompt error/process death | C5 fail-stop before B. |
| Cancellation | C4 preserves target; C6 preserves both reasons without precedence. |
| Rate-limit consumer | C10: zero authority for the observation, restored release via response; payload/render/retry remain cyril-3zy4. |
| Ownership identity mechanism | C1–C5 define dispatch-stamped `TurnId(u64)`. |
| Identity exhaustion | C8. |
| 257-notification backlog | C9. |

## Oracle blindness ledger

| ID | Erased/unseen difference | Disposition |
| --- | --- | --- |
| B1 | Scripted traces cannot see unrepresented scheduler interleavings or undocumented live frames. | Named risk; C2 requires order-independence so unscripted orders change bounds, not correctness. |
| B2 | Routing tests cannot see consumers bypassing `RoutedNotification`. | Named risk; C7 pairs runtime deltas with consumer source review. |
| B3 | Evidence tests cannot decide reason precedence. | cyril-pnwb owns it; C6 records tuples only. |
| B4 | Lifecycle fixtures cannot see OS-specific death timing. | Named risk; C5 claims ordering, not timing coverage. |
| B5 | Rate-limit tests cannot see the live payload, rendering, retry timing. | cyril-3zy4; C10 asserts release counts only. |
| B6 | Counter injection cannot see a natural 2^64 lifetime. | Named risk; C8 covers injected boundaries. |
| B7 | The 257 fixture cannot see an indefinitely wedged consumer. | Named risk; C9 conditions on a live resumed receiver. |
| B8 | Workspace tests cannot see live wire drift or production scheduling. | Named risk; captured fixtures supplement. |
| B9 | Sanitized session IDs erase original opaque values. | Equality fixtures cover routing; accepted risk. |
| B10 | Two-turn capture cannot prove every KAS version honors order or at-most-one `turn_end`; duplicates carry no identity. | Named unsupported drift; order-independence (C2) and absorb-first degradation bound the damage; duplicate `turn_end` remains the one named unsafety. |
| B11 | The model falsifier is not the bridge: channel mechanics, task lifetimes, and select-arm scheduling are abstracted. | Every model-passed claim carries a deterministic implementation fence (C3–C5 named; C1/C2/C6 land as bridge harness tests in the plan). |
| B12 | The model does not observe the allocator, rate-limit consumer, or 257 backlog. | C8–C10 are separate implementation fences. |
| B13 | Hidden turn labels are unavailable to production. | Deliberate: production creates the owner at dispatch and never infers one from content. |
| B14 | Double-drift liveness (T5) defers to the fail-stop lifecycle. | Signed residual in spec.md; requires two simultaneous unsupported omissions. |
| B15 | The prior pipeline's Node mock probes were not rerun. | Production substrate unchanged since `prototype.md`; runtime defect reproductions remain valid per its post-correction note. |

## Negative space

1. Repeated scoped KAS `turn_end` frames are unsupported drift; no identity or safety
   claim is invented for them.
2. Stop-reason precedence is not selected; **cyril-pnwb** owns it (and now inherits
   complete two-source evidence plus the answered cancel-capture ACTION item).
3. Rate-limit payload/render/retry is excluded under **cyril-3zy4**; its busy-release
   requirement is restored, not revised.
4. Stream/completion ordering remains **cyril-9akh**; reconnect remains **cyril-gua0**;
   steering and terminal-child reap are unchanged (**cyril-2vcc**, **cyril-3lh8**).
5. No FIFO guess, timeout, unbounded task registry, or completed-turn history supplies
   ownership; the ledger holds at most one entry.
6. No production code or build plan is included in this design.
7. The in-flight prompt RPC is never aborted on `turn_end` (voided-design behavior).

## Tracker audit

Verified against repository-local `.rivets/issues.jsonl`: **cyril-a71q** (open target;
notes updated with the timing audit), **cyril-j16p** (closed substrate; first-wins
retained), **cyril-pnwb** (open; evidence seam preserved; cancel capture noted),
**cyril-3zy4** (open; release-via-response restored), **cyril-l7tw** (closed; ordering
preserved), **cyril-9akh**/**cyril-gua0** (open; excluded), **cyril-3lh8**/**cyril-2vcc**
(closed; unchanged). No phantom references.

## Self-review

- **Claim count:** 10, within 3–15.
- **Input coverage:** both engines, both receipt orders, single-source drift,
  double-drift residual, stale/foreign/no-active scopes, cancellation, error/death,
  shutdown, exhaustion, rate limit, 256+1 — all mapped to claims.
- **Removed-invariant coverage:** guard, dedup, late global, foreign split-brain, task
  handles, pending disconnect, cancel target, consumer effects, provenance, identity,
  backpressure — swept.
- **Independence/non-vacuity:** hardcoded disposition tables + hidden labels; four
  mutations exercised with distinct failure signatures; 0/34 correct-policy failures.
- **Impossibility resolution:** T4/T5 demonstrate same-action safety on identical visible
  input; the residual is signed, bounded, and requires double unsupported drift.
- **Cost:** model falsifier <1s; every implementation fence ≤4 minutes, deterministic.
- **Material boundaries:** all 20 prototype rows accounted for.
- **Blindness:** 15 entries including the new B11 (model≠bridge), B14 (double-drift
  residual), B15 (mock probes not rerun).
- **Gate result:** passed and ready for requester review. No approval, production change,
  or plan is claimed.

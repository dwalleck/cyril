# Timing audit — cyril-a71q spec/design vs. the end-of-turn wire research

Date: 2026-07-12
Status: **AUDIT FINDING — the pending spec sign-off should be refused; spec/design re-anchor required**
Auditor: takeover session (main repo), independent of the pipeline that produced `spec.md`/`design.md`

## Summary

The current `spec.md` (sole-`turn_end` release, abort-on-`turn_end`, discard-late-response,
Busy-forever-without-`turn_end`) rests on an input-space premise — *"either KAS terminal
source, in either order, either absent indefinitely"* — that the repository's own wire
research does not support. The superseded design's impossibility proof
(`design-superseded-either-source.md` C1) is valid **for that over-generalized space**, but
the space itself was the error. Every choice-A decision downstream of it (workflow.md
decisions dated 2026-07-12, lines 40–45) inherits the confused premise.

## What the research actually established

### 1. The live capture: both signals, `turn_end` first, response back-to-back

`experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl` (the only committed
genuine KAS capture; the same file the prototype pinned its fixtures from) contains two
prompt turns. Extracted lifecycle ordering (frame index, capture timestamp ms, direction):

```
turn 1 (user cancel):
   37  1783041444400  out  session/prompt REQUEST id=5
   43  1783041444429  in   session_info_update kind=turn_start
   79  1783041453782  in   session_info_update kind=turn_completion
   83  1783041453801  in   session_info_update kind=turn_end   stopReason=cancelled
   84  1783041453802  in   PROMPT RESPONSE id=5                stopReason=cancelled

turn 2 (normal):
   91  1783041473217  out  session/prompt REQUEST id=8
   94  1783041473238  in   session_info_update kind=turn_start
  522  1783042242918  in   session_info_update kind=turn_completion
  524  1783042242922  in   session_info_update kind=turn_end   stopReason=end_turn
  525  1783042242922  in   PROMPT RESPONSE id=8                stopReason=end_turn
```

On every observed turn — including the cancel — **both** terminal signals are present,
`turn_end` is emitted **first**, and the RPC response follows as the next inbound frame
(1 ms / 0 ms later). "The response may be late" (cyril-j16p) means *milliseconds after
`turn_end`*, not *may never come*.

### 2. Transport and bridge internals make receipt order deterministic

- ACP is JSON-RPC over one ordered stdio stream: the response frame cannot be received
  before the `turn_end` frame KAS emitted ahead of it.
- Inside cyril, both signals feed the **same** FIFO mpsc (`InternalChannels` doc,
  `bridge.rs:574-587`): the KiroClient enqueues `turn_end`'s `TurnCompleted` at frame
  receipt; the off-loop prompt task enqueues the synthesized `TurnCompleted` only after
  the RPC resolves — causally after the response frame, hence after `turn_end`'s frame.
  Both producers run on the same `LocalSet` thread. Mediator receipt order on a normal
  KAS turn is therefore `turn_end` → response; response-first is reachable only under
  channel-backpressure scheduling jitter and is a **defensive** case, not a normal input
  class.

### 3. First-source-wins dedup is a liveness property, not a cardinality contract

The shipped comment (`bridge.rs:1639-1646`) states the design intent of cyril-j16p:
both signals, "in either order," clear and forward only the first, **"so … a
non-returning prompt response can't freeze the turn (turn_end completes it)"** — and
symmetrically, a missing `turn_end` can't freeze the turn because the response completes
it (fenced by `kas_turn_end_completes_without_prompt_response`, bridge.rs:3199, and
`kas_turn_end_and_prompt_response_dedupe_to_one`, bridge.rs:3142). Either-order handling
exists so that liveness never depends on which signal shows up.

### 4. "Response-only" was never observed on the wire

The prototype's response-only trace came from the pipeline's **own Node mock**
(`prototype.md` line 29: `mock_kas_server.js` — "its `response_only` branch responds to
prompts 1 and 2 and emits no `turn_end`"). It is a scripted synthetic fixture proving how
*current cyril* behaves on that input; it is **not** evidence that a live KAS producer
emits response-only turns. No committed capture shows a response-first or
either-signal-missing normal turn.

### 5. Bonus finding — the cyril-pnwb ACTION item is already answered

cyril-pnwb says a KAS cancel `turn_end` is "currently UNOBSERVED; no cancel turn_end has
been captured." Turn 1 above **is** a captured live cancel: `turn_end` carries
`stopReason=cancelled` and the response agrees (`cancelled`). Per pnwb's own framing
("If KAS already reports 'cancelled' in turn_end, there is no bug"), the hazard did not
manifest in the observed capture.

## Where the pipeline departed from the research

1. **Over-generalization.** The first spec promoted "either order / secondary may be
   late" into "either source may be absent indefinitely, in any order" — a strictly
   larger input space than researched, with the synthetic mock's response-only branch
   treated as a production input class.
2. **A correct proof about the wrong space.** `design-superseded-either-source.md` C1
   correctly proved that space unresolvable without wire turn identity. Rather than
   re-anchoring the space on the researched ordering, the pipeline escalated menu
   choices (A/B) whose shared premise was the over-generalized space.
3. **Choice-A consequences measured against the research:**
   - `turn_end` arrives first on every observed turn, so *"abort the prompt RPC on
     `turn_end`, discard any later response"* fires on **every normal KAS turn** — abort
     becomes the hot path and the RPC result is systematically discarded.
   - That discards exactly the evidence cyril-pnwb needs. "Preserve response evidence
     only if observed before `turn_end`" preserves **nothing** in practice, contradicting
     the a71q tracker note ("the identity work must preserve both source/reason inputs so
     that later decision remains possible").
   - *"Missing `turn_end` → Busy until failure/disconnect"* inverts cyril-j16p's
     non-blocking criterion and directly conflicts with P2 **cyril-3zy4** ("a
     rate-limited turn must release the busy guard"). The pipeline resolved the conflict
     backwards — by requiring 3zy4's product requirement to be "revised" to fit the model
     (workflow.md decision line 41).
   - It also conflicts with baseline ACP, where turn completion **is** the
     `session/prompt` response (docs/kiro-acp-protocol.md); a response-only turn is legal
     ACP, and freezing on it is protocol-hostile.

## Recommended re-anchor (for requester re-gate)

- **Keep first-source-wins release** (shipped j16p behavior): liveness never depends on
  which signal arrives; a rate-limited or turn_end-less turn still releases via the
  response, satisfying cyril-3zy4.
- **Anchor the KAS contract on the researched ordering:** normal input per accepted
  prompt is scoped `turn_end` followed by the RPC response — ordered, both present.
  Absence of either is a degenerate case handled by first-wins liveness plus the existing
  fail-stop lifecycles — not a normal input class, and never Busy-forever.
- **Fix the actual a71q defects exactly as the tracker sketched:** `TurnId(u64)`
  allocated at `SendPrompt` accept; stamped on the synthesized `TurnCompleted`
  (`Option<TurnId>`, dual matching — id-match for synthesized, scoped-session match for
  the unstampable wire `turn_end`); after a first-source release, track *"one companion
  signal still expected for (session, owner)"* and absorb it (tracker note option (b));
  a scoped completion for a non-active session routes to its consumer without touching
  the main guard (cross-session split-brain fix).
- Under the researched ordering, the failed design's History-1/History-2 ambiguity
  dissolves: if A released via its response, A's `turn_end` was already received (it
  precedes the response on the wire) — it either already won or is the one expected
  companion; the ledger disambiguates deterministically with no FIFO guess, no timeout.
- **Preserve both source/reason observations** at the observer seam for cyril-pnwb
  regardless of arrival order.

## Disposition

- `spec.md`'s consequence sign-off is recorded as **PENDING — NOT PASSED** (spec.md line
  205). Recommend the requester refuse it, supersede `spec.md`/`design.md` (the pipeline's
  existing `*-superseded-*` convention), and re-run the design falsifier against the
  researched input space above.
- Workflow decisions predicated on the either-source/indefinite-absence premise
  (workflow.md lines 40, 41, 43–45) need re-signing against the corrected contract.
- The prototype's *evidence* (pinned frames, no native turn id, runtime defect
  reproductions) remains valid and reusable; only the contract layered on top of it is
  wrong.

# cyril-b4y4 — related issues (prove-it step 0)

Tracker searched 2026-08-02 (`rivets list -n 200`, keywords: turn, mediat,
companion, absorb, bridge, a71q, g9vt). Matches that bear on this extraction:

## Direct lineage

- **cyril-a71q** (closed, ★) — turn-seq dedup; its pre-PR review filed this
  issue. All artifacts in `.cyril-a71q/` (spec, design with blindness table
  B1–B18, plan, review-decisions). The a71q fences are the safety net this
  refactor runs under (AC3).
- **cyril-g9vt** (open, P2, ★) — callback-mediator ADR-0004 amendment.
  BLOCKED BY this issue; the sequencing rationale (one coherent ADR-0004
  amendment instead of two overlapping ones) is recorded in b4y4's update.

## Blindnesses this extraction can retire (AC4)

- **cyril-ri8q** (open, P3) — KAS companion ledger has near-zero behavioural
  coverage: absorb and drop both `continue`, observationally identical on the
  notification channel. Root cause is exactly that the ledger is a `run_loop`
  local. Its option (a) — an observation seam — IS the mediator type this
  issue builds. Landing b4y4 with disposition-returning `observe()` makes
  absorb vs drop directly assertable and should close or largely close ri8q.
  Design must name it.
- **cyril-ns0o** (open, P3) — a71q follow-up: four bridge-level fences the
  spec named but the build substituted. Item 1 (injectable turn allocator so
  exhaustion can be driven through the loop) intersects this extraction: a
  mediator that OWNS the allocator is the natural injection seam. Design
  should say which ns0o items (if any) ride along and which stay deferred.

## Adjacent, explicitly out of scope

- **cyril-pnwb** (open, P3, needs-info) — which of the two same-turn terminal
  signals carries the authoritative stop_reason on KAS cancel. The Companion
  ledger already records both `{source, reason}` observations FOR pnwb; the
  extraction must preserve that recording, not resolve the precedence
  question (needs a live cancel capture first).
- **cyril-9akh** (open, P3) — streamed text after TurnCompleted (ACP
  notification-vs-response ordering race). Different layer (App-side
  commit), not mediation.
- **cyril-0o7e** (open, P3) — consume KAS turn_completion summary — new
  wire consumption, not mediation policy.
- **cyril-79df** (open, P3) — model KAS turn_completion requestIds[] (2.16.0
  fixture gap). Wire-shape modeling, not mediation.

## Constraints inherited from notes/ADRs

- Issue note 2026-08-02: the module owns the STATE MACHINE; terminal-source
  AUTHORITY is an Engine fact — ask the bound Engine, never branch on
  `engine.kind()` inside the module (ADR-0001's rejected enum-match pattern).
  `ActiveTurn::owes_wire_companion()` currently does `self.engine ==
  AgentEngine::Kas` — the extraction is where that becomes an Engine question.
- CONTEXT.md "Turn-end": only the bound engine may declare a turn over; both
  terminals arrive every KAS turn, turn_end first, response 0–1 ms behind
  (live-confirmed 2026-08-01 on 2.16.0).

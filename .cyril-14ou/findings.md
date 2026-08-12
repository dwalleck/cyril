# cyril-14ou prove-it findings (2026-08-12)

## Q1 — What silence distinguishes a healthy turn from a stall?

Probe: `probe_gaps.py` over the bh7g tap captures (11 healthy turns + the stall;
committed copies: `kas-turn-healthy-{a,b}-2.16.2.jsonl`, `kas-turn-stall-run-2.16.2.jsonl`).
Oracle: KAS's persisted `messages.jsonl` timestamps — an independent recorder.

- Probe and oracle **agree to the decimal** on the shared quantity (per-turn
  duration: `[27.9, 30.1, 21.1]`s both sides). Their max-gap numbers differ by
  design (wire streams chunks + context_usage; transcript persists milestones)
  — reconciled, not papered over.
- **Healthy wire silence never exceeded 8.2s** (most gaps ≤4s) across the 11
  healthy turns measured (3 in the stall run + 4 + 4 in the clean runs), up
  to 96s long — `context_usage` frames are a de-facto heartbeat during model
  legs. The stall: last frame at +3.0s, then unbounded silence (oracle's
  internal gap: 983.3s).
- ⇒ >100× separation. Any stalled-turn threshold in ~30–120s is unambiguous;
  the choice is UX preference, not risk calibration.

## Q2 — Does bridge teardown reap the KAS node?

Probe: `probe/` (rust, real `spawn_bridge` KAS free path) with arms
shutdown/drop/abort, held at READY so the oracle can observe the live node.
Oracle: `run_reap_arms.sh` pgrep set-diff (before/during/after).

| arm | node during | node after |
|---|---|---|
| `BridgeCommand::Shutdown` | alive | **reaped** |
| bare handle drop (no Shutdown) | alive | **reaped** |
| `std::process::abort()` (no Drop) | alive | **orphaned** |

**Scope item 4 is implemented and fenced for UNIX CLEAN teardown paths**:
`ProcessGroupGuard` (transport.rs, cyril-0pms) group-SIGKILLs on drop;
regression test `dropped_agent_process_kills_process_group` exists; both the
Shutdown and bare-drop arms reaped live. Scoping (PR #94 review SP2): the
guard is `cfg(unix)` — on native Windows `kill_on_drop` covers free mode (node
is the direct child) but wrapper mode leaks the grandchild; and Drop-skipping
death (SIGKILL/abort) orphans on every platform — unfixable in-process, a
supervisor/subreaper could close it. Both gaps → **cyril-jlw9**. The bh7g-era
orphans came from python probes bypassing cyril's transport and from
SIGKILL'd consumers. Item 4 in THIS PR ⇒ documentation.

## Q3 — Does CancelRequest cancel a live KAS turn?

Probe: `cancel` arm — 400-count streamed turn, `CancelRequest` 2s after first
text. Result: `TurnCompleted { Cancelled }` promptly.
Oracle: KAS's persisted transcript for that session independently records
`session_event{aborted}` + `turn_end{cancelled}` 4.3s after the prompt.

- Cancel-on-a-streaming-turn works end-to-end through cyril's own plumbing
  (mediator `cancel_target` → `session/cancel` → cancelled terminals).
- First attempt (40-count) lost a race — turn completed `EndTurn` before the
  cancel landed. Lesson: cancel is best-effort against turn completion; the
  design must treat `EndTurn`-after-cancel as a legal outcome (feeds the
  cyril-pnwb evidence base: the forwarded stop_reason was correct both times).
- **Residual (not probeable on demand):** cancel-under-*stall* — whether KAS
  processes `session/cancel` while blocked on a dead backend stream. The bh7g
  harness's cancel-on-wedge arm stays armed for the next stall window; the
  design must not ASSUME cancel works there (offer kill-and-respawn as the
  second-tier escape).

## Structural notes for the design

- `run_loop`'s `tokio::select!` has **no time source** — the liveness clock is
  a new select arm (interval), stamping last-inbound-frame time per active turn.
- The mediator already knows the active turn + its session (`is_busy`,
  `cancel_target`); it is the natural home for "which turn is silent".
- A stalled-turn signal to the App must be a new `Notification` variant riding
  the normal channel (routed; NOT a terminal — never synthesize TurnCompleted).

## What I learned (the sentence)

Healthy KAS turns are never wire-silent for more than ~8 seconds even during
minute-long work — so a stalled-turn detector can fire in tens of seconds with
100× margin — and half the ticket (teardown reaping) turned out to be already
shipped and fenced, reducing scope item 4 to documentation.

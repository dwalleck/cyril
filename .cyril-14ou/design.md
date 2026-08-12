# cyril-14ou design — turn-liveness: stalled-turn signal + cancel affordance

## Purpose

A KAS turn can sit open indefinitely on a stalled backend stream emitting nothing
(bh7g: 16 min, later completed). The consumer cannot distinguish "thinking",
"stalled", and "terminal lost". This design adds a bridge-level liveness clock and
a UI stalled-turn state. It never synthesizes a terminal (the cyril-a71q/pnwb
hazard: a genuine terminal can arrive minutes late).

Probe grounding (`findings.md`): healthy turns are never wire-silent >8.2s
(context_usage is a de-facto heartbeat); the stall is unbounded silence — >100×
separation. Cancel of a live streaming turn works end-to-end today (Q3). Clean
teardown already reaps the KAS node (Q2, `ProcessGroupGuard`, fenced) — scope
item 4 of the ticket is documentation only.

## Architecture

**Mechanism (cyril-core, bridge):**
- `TurnLiveness` — a new pure state machine owned by `run_loop` (SessionController
  mold: no async, no I/O; time is an *input*). State: `last_inbound: Option<Instant>`,
  `outstanding_host_replies: usize`, `armed: bool`.
- Inputs, fed from existing loop arms:
  - inbound `RoutedNotification` whose scope is the active turn's session or
    global → `stamp(now)`; re-arms after a stall emission
  - host-callback dispatched → `outstanding += 1`; reply sent → `outstanding -= 1`,
    `stamp(now)` (cyril owes the agent work — silence is ours, not theirs)
  - turn begin/end (mediator's existing begin/release points) → arm/disarm
- A new `tokio::select!` interval arm (5s period, polled only while
  `mediator.is_busy()`): `liveness.check(now, threshold)` → at most one
  `Notification::TurnStalled { quiet: Duration }` per quiet period, scoped to the
  active turn's session, sent on the normal notification channel.
- Threshold: `SpawnConfig::stall_threshold: Duration`, default **30s** (probe: ≥3×
  the observed healthy ceiling). SpawnConfig is the bridge's existing config
  surface; no additional config surface exists for any other bridge knob, so none
  is added for this one.
- `TurnStalled` is a non-terminal: the mediator's `observe` treats it as
  `Forward` (it only special-cases `TurnCompleted`), and it must not touch
  `is_busy`.

**Policy (cyril-ui + App):**
- `UiState` gains `stall: Option<StallState { quiet: Duration, cancel_sent: bool }>`.
  `apply_notification`: `TurnStalled` sets it; any *other* notification clears it;
  `TurnCompleted` clears it (busy already clears).
- Toolbar renders an amber "agent quiet {N}s — Esc cancels" chip while
  `stall.is_some() && is_busy`, suppressed while the approval overlay is active
  (an outstanding approval explains the silence; UiState already knows overlay
  state). After Esc, App marks `cancel_sent` → chip text becomes
  "cancel sent — agent unresponsive" if the stall persists.
- Esc behavior is unchanged (existing cancel chain); no new key handling layer.

**Deliberately no recovery mechanics:** cancel-under-stall is unverifiable on
demand (no stall window can be summoned); the second-tier escape
(kill-and-respawn + session/load) is **cyril-w9oi**. Teardown reaping is already
shipped and fenced (`dropped_agent_process_kills_process_group`); this PR adds a
doc pointer, no code.

## Input shapes

Turn/event stream (the feature's input):
1. no active turn (clock disarmed) — C5
2. active turn, frames < T apart (healthy) — C11
3. active turn, quiet ≥ T, nothing outstanding (the stall) — C1
4. quiet then traffic resumes, quiet again (two stalls, one turn) — C2
5. quiet with ≥1 outstanding host reply (long local terminal/fs work) — C3
6. foreign-session frames during main-turn quiet (subagent/peer traffic) — C4
7. turn ends while stalled / before threshold — C5, C6
8. UiState: TurnStalled with/without approval overlay; clear on next
   notification; idempotent clear — C7, C8
9. Esc during stall — C9

Out of scope shapes: `stall_threshold == Duration::ZERO` (not producible by
cyril's own wiring; SpawnConfig default is const 30s and no caller passes zero);
v2 `KIRO_MOCK` turns (mock completes instantly; covered structurally by the same
engine-agnostic loop code); subagent/workflow-step stall tracking (their streams
have their own activity UI and no busy-guard wedge — no known problem to solve).

## Subtractive sweep (2b)

Purely additive: a new interval select arm (reads liveness state, emits ≤1
notification with the same channel/backpressure rules as every other send), a new
notification variant, a new UiState field. No lock, guard, ordering, or
serialization point is removed. One risk inspected: the interval arm shares the
loop with command dispatch — it performs no awaits other than the bounded channel
send already used by every arm, so it cannot starve the loop.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| C1 | Active turn + quiet ≥30s + nothing outstanding ⇒ exactly one session-scoped `TurnStalled` | Replay the bh7g stall capture's real timings through the decision rule; 0 or ≥2 emissions falsifies | The capture (ground truth of a real stall), independent of cyril code | 5m | **passed** (falsifier_c11.py) | unit `turn_liveness::stall_emits_once` + replay fixture test from capture-derived timing table |
| C2 | Traffic after a stall re-arms; a second quiet period emits a second TurnStalled | Synthetic timing sequence quiet→traffic→quiet; 1 emission falsifies | Hand-computed expected emission count | 10m | pending | unit `turn_liveness::rearm_after_traffic` |
| C3 | Outstanding host reply parks the clock | Sequence: dispatch callback, 40s quiet, reply; any emission falsifies | Hand-computed | 10m | pending | unit `turn_liveness::outstanding_reply_parks` |
| C4 | Foreign-session frames don't reset the main turn's clock | Quiet main turn + foreign frames every 5s; zero emission falsifies | Hand-computed | 10m | pending | bridge harness `foreign_traffic_does_not_mask_stall` |
| C5 | Turn end disarms; no TurnStalled for completed/absent turns | End turn at 25s then 40s more quiet; any emission falsifies | Hand-computed | 10m | pending | unit `turn_liveness::disarm_on_turn_end` |
| C6 | TurnStalled is non-terminal: mediator forwards it, `is_busy` unchanged | Feed TurnStalled to mediator mid-turn; DropStale/Absorb/release falsifies | Mediator disposition enum output | 10m | pending | unit `turn_mediator::stalled_is_forwarded_nonterminal` |
| C7 | UiState sets stall on TurnStalled, clears on any other notification | Apply TurnStalled then AgentMessage; stall surviving falsifies | UiState field asserts | 10m | pending | unit `state::stall_set_and_cleared` |
| C8 | Stall chip suppressed while approval overlay active | Render with stall+overlay to TestBackend; chip text present falsifies | Rendered buffer text | 15m | pending | render test `toolbar::stall_suppressed_during_approval` |
| C9 | Esc during stall sends cancel; healthy-turn cancel yields Cancelled end-to-end | Live probe (Q3, done): 400-count turn + CancelRequest → TurnCompleted{Cancelled}; KAS transcript records turn_end{cancelled} | KAS persisted transcript (independent recorder) | done | **passed** (live 2026-08-12) | App key-chain test `esc_marks_cancel_sent_during_stall` (plumbing); live evidence archived in findings.md |
| C10 | Clean teardown reaps the KAS node (docs-only item) | Live arms shutdown/drop/abort (Q2, done) | pgrep set-diff | done | **passed** | existing `transport::dropped_agent_process_kills_process_group` |
| C11 | T=30s produces zero false stalls on real healthy traffic | Replay 12 healthy captured turns; any emission falsifies | The captures (real traffic), independent | 5m | **passed** (falsifier_c11.py) | replay fixture test with capture-derived gap table (asserts 0 emissions at T=30s AND ≥1 at T=8s — the tight-bound guard) |

Non-vacuity — named buggy implementations per fence:
- C1: clock stamped only on frame arrival and *checked* only on frame arrival
  (the horizon bug my own replay had) → no emission ever. The interval arm exists
  precisely to defeat this; the unit fixture advances time with no events.
- C2: `armed` never reset on resumed traffic → second stall missed.
- C3: forgetting `outstanding` (or decrementing on dispatch) → false stall during
  a 40s local command.
- C4: stamping on any `RoutedNotification` regardless of scope → foreign traffic
  masks a real stall.
- C5: interval arm not gated on `is_busy`/active turn → phantom stall between turns.
- C6: adding TurnStalled to the mediator's terminal match arm → turn released early.
- C7: clearing only on TurnCompleted → stale chip through a healthy resumed turn.
- C8: rendering the chip whenever stall.is_some() → chip over the approval overlay.
- C11: threshold constant edited below the healthy ceiling (e.g. 5s) → replay
  fixture's 0-emission assert fails; the ≥1-at-8s assert defeats a fixture whose
  timing table was regenerated too coarsely to observe anything.

## Negative space

1. **Never synthesizes `TurnCompleted`** on silence — the stalled turn can complete
   minutes later (bh7g proved it); terminal authority stays with the engine.
2. **No automatic recovery** — no auto-cancel, no auto-respawn. Second-tier
   kill-and-respawn escape is cyril-w9oi.
3. **No KAS-server-side changes** — the missing converse-stream timeout is
   upstream's defect; cyril observes, it does not patch the agent.
4. **No subagent/workflow-step stall tracking** — main turn only; subagent streams
   carry their own activity indicators and cannot wedge the busy guard.
5. **No new config surface** — threshold lives in SpawnConfig like every other
   bridge knob; no slash command, no settings file key.

## Open decisions (for the design pause)

1. Threshold default: **30s** (recommended; ≥3× observed ceiling) vs 60s
   (more conservative, slower to inform).
2. Chip copy: "agent quiet {N}s — Esc cancels" → escalating to "cancel sent —
   agent unresponsive". Wording preferences welcome.
3. `TurnStalled` emission cadence: once per quiet period (recommended; UI derives
   a live counter from its own clock) vs re-emitting every interval tick
   (chattier channel, no UI clock needed).

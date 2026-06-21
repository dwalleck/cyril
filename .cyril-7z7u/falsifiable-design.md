# cyril-7z7u — Optimistic steer chip (falsifiable design)

**Issue:** cyril-7z7u (P2). Scope chosen by user: **optimistic chip** (not minimal cleanup / wontfix).
**Builds on:** `.cyril-7z7u/findings.md` (prove-it; backend contract pinned). Cheapest falsifier run + passed (see table).

## Verified backend contract (from the probe — the design may not contradict this)

Wire `steering_queued` + `steering_consumed` arrive as a **pair within one turn** (the turn that processes the steer). A steer sent at a turn's tail defers the **whole pair** to the next turn. There is no wire state where `steering_queued` is outstanding (un-`consumed`) across a turn boundary.

## Root cause (probe + code)

The chip (`UiState::steering_queued: usize`) is **wire-driven** (`+1` on `SteeringQueued`, `-1` on `SteeringConsumed`, reset on `TurnCompleted`/`Cleared`/`SessionCreated`); `add_steer_echo` does **not** touch it. Because the wire pairs same-turn, the count is always 0 at `TurnCompleted`, so the reset (state.rs:432) is a **no-op** on a false premise. The *real* defect: for a late steer the wire `SteeringQueued` is deferred to N+1, so during turn N the user sees the optimistic `Queued` echo but the chip shows **0** — the chip and echo disagree.

## Fix

Drive the chip **optimistically**, in lockstep with the echo, so the invariant **`steering_queued` == count of `SteerEcho{Queued}`** holds:

| Event | Count change | (echo — unchanged from today) |
|---|---|---|
| `add_steer_echo` (user-send) | **`+1`** (NEW) | push `Queued` |
| `SteeringConsumed` (wire) | `-1` saturating | flip oldest `Queued`→`Applied` |
| `SteeringQueued` (wire) | **none** (was `+1`; REMOVED — avoid double-count) | (none) |
| `TurnCompleted` | **none** (was `=0`; REMOVED) | (none — echo survives, steer still pending) |
| `SteeringUnsupported` | **`=0`** (NEW) | flip all `Queued`→`Unsupported` |
| `SteeringCleared` | `=0` (kept) | flip all `Queued`→`Cleared` |
| `SessionCreated` | `=0` (kept) | flip all `Queued`→`Cleared` |

Files: `crates/cyril-ui/src/state.rs` only. The echo logic (`flip_queued_steer_echoes`) is untouched; only the count's data source changes.

## Subtractive sweep (step 2b)

This change is **subtractive** — it removes two guarantees:

1. **Removed: "`steering_queued` == 0 after every `TurnCompleted`."** Who relied on it: the chip-clear-at-turn-end. **Still-holds claim (3):** after a turn whose steer is consumed, the count returns to 0 via `SteeringConsumed`, not the reset. New failure mode it exposes: a steer the backend *silently drops* (no `consumed`/`cleared`/`unsupported`) would now leave the count stuck — the probe shows the backend always consumes (no silent drop), so accepted; the reset previously *masked* such a leak every turn.
2. **Removed: "every count increment has a matching wire `SteeringQueued`."** The count is now incremented optimistically at send, not at wire confirm. **Still-holds claim (4):** a wire `SteeringQueued` must not also increment (no double-count). Consequence: observer-originated steers (wire-only, no local echo) won't count — single-client model today, tracked **cyril-8lfs**.

## Input shapes (the steer-lifecycle sequences applied to `UiState`)

`add_steer_echo` × {0,1,2}; then some order of `SteeringQueued`(wire) / `SteeringConsumed` / `SteeringCleared` / `SteeringUnsupported` / `TurnCompleted` / `SessionCreated`. Production-reachable sequences below each get ≥1 claim. Out-of-scope: a `Queued` echo evicted by the 500-message cap while the count stays (HUD approximation under pathological eviction — negative space).

## Claims & Falsification

| # | Claim | Falsifier (sequence → falsifying result) | Oracle | Cost | Status | Regression fence |
|---|-------|-------------------------------------------|--------|------|--------|------------------|
| 1 | `add_steer_echo` increments the chip (reflects the steer at send, pre-wire). | `add_steer_echo("a")` → if `steering_queued()≠1`, false. | `steering_queued()` | 2m | pending | `add_steer_echo_increments_chip` (new) |
| 2 | `SteeringConsumed` decrements + flips oldest `Queued`→`Applied`. | send, consume → if count≠0 OR echo≠Applied, false. | count + `messages()` | 2m | **passed** (preserved; `consumed_flips_oldest_queued_echo` green now) | `consumed_decrements_and_flips` (extend existing) |
| 3 | `TurnCompleted` does NOT reset the count or echo — a late steer stays count 1 / `Queued`, then a next-turn `SteeringConsumed` drains it to 0 / `Applied`. | send, `TurnCompleted` → if count≠1 OR echo≠Queued; then consume → if count≠0/echo≠Applied, false. | count + `messages()` | 3m | pending | `turn_completed_does_not_reset_pending_steer` (REWRITES `turn_completed_resets_steering_queued`) |
| 4 | A wire `SteeringQueued` does not change the count (optimistic add already counted it). | send, wire `SteeringQueued` → if `steering_queued()≠1`, false. | count | 2m | pending | `wire_queued_does_not_double_count` (new; REWRITES `steering_queue_mirror`) |
| 5 | `SteeringUnsupported` zeroes the count and flips all `Queued`→`Unsupported`. | send×2, `SteeringUnsupported` → if count≠0 OR any echo still Queued, false. | count + `messages()` | 2m | pending | `unsupported_clears_chip` (new) |
| 6 | `SteeringCleared` zeroes the count and flips all `Queued`→`Cleared`. | send×2, `SteeringCleared` → if count≠0 OR any Queued, false. | count + `messages()` | 2m | pending | `cleared_zeroes_chip` (from `steering_queue_mirror` rewrite) |
| 7 | `SessionCreated` zeroes the count and flips all `Queued`→`Cleared`. | send, `SessionCreated` → if count≠0, false. | count | 2m | pending | `session_created_zeroes_chip` (from `steering_queue_mirror` rewrite) |
| 8 | FIFO: two sends then two consumes → count 0, both echoes `Applied` oldest-first. | send("a"),send("b"),consume,consume → if count≠0 OR statuses≠[Applied,Applied], false. | count + `messages()` | 2m | pending | `fifo_multi_steer_drains` (new) |
| 9 | No underflow: `SteeringConsumed`/`TurnCompleted` at count 0 leaves 0. | consume at 0 → if count≠0 (panic/underflow), false. | count | 2m | pending | `consumed_at_zero_no_underflow` (new) |

### Non-vacuity (buggy impl per claim)
- **1**: current code (`add_steer_echo` doesn't count) → count 0 after send → fails. *(This is the change.)*
- **2**: preserved behavior; a no-flip / wrong-sign impl fails it (current code passes — verified).
- **3**: keeping the `TurnCompleted` reset → count 0 after turn-end → fails. *(Inverts the old `turn_completed_resets_steering_queued`.)*
- **4**: keeping the wire `SteeringQueued` `+1` → count 2 → fails.
- **5**: current `SteeringUnsupported` (no count clear) + optimistic add → count 1 after unsupported → fails.
- **6/7**: dropping the explicit reset → count 1 → fails. **8**: LIFO/no-flip → wrong status vector. **9**: non-saturating `-1` → underflow panic.

Each fence asserts a distinct `(count, echo-status-vector)` after a distinct sequence → failures localize to the exact claim.

## Negative space (deliberately NOT done)

1. Does **not** count/echo observer-originated steers (wire `SteeringQueued` without a local echo) — single-client model; tracked **cyril-8lfs**.
2. Does **not** finalize the optimistic echo at turn-end — the probe shows a late steer is still pending, so flipping it at `TurnCompleted` would be *wrong* (rejects issue option (b)).
3. Does **not** change `flip_queued_steer_echoes` or the echo status enum — only the count's data source.
4. Does **not** guarantee `count == #Queued-echoes` under 500-message-cap eviction of a `Queued` echo (HUD approximation in that pathological case).
5. Does **not** add steer correlation ids — `content` stays advisory; reconciliation stays positional FIFO.

## Tracker references

- cyril-7z7u — this issue (verified `rivets show`; status in_progress).
- cyril-8lfs — multi-observer steer-counting gap (verified: filed `2026-06-21`, `related` to cyril-7z7u).

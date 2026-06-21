# cyril-7z7u — Budgeted plan

**Design:** `.cyril-7z7u/falsifiable-design.md` (approved; cheapest falsifier passed).
**Shape:** **one atomic slice.** The 9 claims verify a single coherent change — switching `steering_queued`'s data source from wire-driven to optimistic. It cannot be split: any partial application violates the invariant `steering_queued == #SteerEcho{Queued}` (optimistic `+1` without removing the wire `+1` → double-count; switching the normal path without the `SteeringUnsupported` clear → a leaked count on an unsupported steer). Splitting would ship a known-broken intermediate, which "a slice is done when its tests pass" forbids.

---

## Slice 1: Make `steering_queued` optimistic (chip == #Queued echoes)

**Claim:** all 9 design claims. The chip count is incremented at `add_steer_echo` (`+1`), decremented at `SteeringConsumed` (`-1` saturating), zeroed at `SteeringCleared`/`SteeringUnsupported`/`SessionCreated`; the wire `SteeringQueued` and `TurnCompleted` no longer touch it. Invariant after every notification (modulo message-cap eviction): `steering_queued() == count of SteerEcho{Queued}`.

**Oracle:** the probe-pinned wire contract (`.cyril-7z7u/findings.md`: `steering_queued`+`steering_consumed` pair same-turn; a late steer defers the pair to N+1) + the `count == #Queued echoes` spec. Expected `(count, echo-status-vector)` values are hand-derived from that contract, compared against `UiState::steering_queued()` + `messages()`. Independent of the production reconciliation code (the expected side comes from the probe + spec, not from running the SUT).

**Stress fixtures** (each designed to fail a *plausible* implementation bug; expected output fixed before coding):

1. **Double-count — `[add_steer_echo("a"), wire SteeringQueued]`.** Most plausible bug: add the optimistic `+1` but forget to remove the wire `+1`. **Expected `steering_queued()==1`** (not 2). (claim 4)
2. **Late-steer reset regression — `[add_steer_echo("a"), TurnCompleted]`.** Bug: leave the `TurnCompleted` reset in. **Expected `count==1` and echo still `Queued`**; then `SteeringConsumed` → `count==0`, echo `Applied`. (claim 3)
3. **Unsupported leak — `[add_steer_echo("a"), add_steer_echo("b"), SteeringUnsupported]`.** Bug: forget to add `=0` to `SteeringUnsupported` (was a no-op on the count under the old wire-driven model). **Expected `count==0`, both echoes `Unsupported`.** (claim 5)
4. **Same-content FIFO — `[add_steer_echo("x"), add_steer_echo("x"), SteeringConsumed, SteeringConsumed]`.** Bug: a flip that matches on `content` instead of position would mis-resolve two identical steers. **Expected `count==0`, statuses `[Applied, Applied]`** (positional oldest-first; `content` advisory). (claim 8)
5. **Underflow — `SteeringConsumed` at `count==0`.** Bug: non-saturating `-1` → underflow panic/wrap. **Expected `count==0`, no panic.** (claim 9)

**Loop budget:** **no new loop.** The count edits are O(1). The only loop touched-adjacent is the existing `flip_queued_steer_echoes` — **unchanged by this slice** — which iterates `messages` (O(n), n ≤ the 500-message cap = O(1)-bounded), once per `Consumed`/`Cleared`/`Unsupported`/`SessionCreated`. Within budget (≤500 ops, 0 syscalls).

**Wall budget:** n/a (per-notification state transitions, not an always-on phase).

**Doc-comment-as-contract:** the invariant `steering_queued == #Queued echoes` is **code-maintained**, not a caller precondition — no "callers must X" doc, so no `debug_assert`/runtime-check obligation there. The underflow guard stays **`saturating_sub`** (a runtime guard that survives release — a `debug_assert` would let release wrap/panic; load-bearing per claim 9). Update the misleading `TurnCompleted` comment ("a steer un-consumed by turn-end can't drain") to state the real contract (the probe disproved it; the count drains via `SteeringConsumed`, optionally next turn).

**Output stream:** no stdout/stderr writes. State transitions bump `messages_version` for redraw (existing pattern). N/a.

**Files:** `crates/cyril-ui/src/state.rs` only.

**Code (advisory — implementer may deviate if the invariant holds + budgets pass):**

```rust
// add_steer_echo (was: no count change) — optimistic:
pub fn add_steer_echo(&mut self, text: &str) {
    self.flush_streaming_agent_text();
    self.flush_streaming_thought();
    self.messages.push(ChatMessage::steer_echo(text.to_string()));
    self.steering_queued = self.steering_queued.saturating_add(1); // NEW (chip == #Queued echoes)
    self.messages_version += 1;
    self.enforce_message_limit();
}

// SteeringQueued (wire): drop the count++ — the optimistic add already counted it.
// Single-client model; observer-originated steers are cyril-8lfs.
Notification::SteeringQueued { .. } => false, // no state change (echo + count both optimistic)

// TurnCompleted: REMOVE `self.steering_queued = 0;` + the false-premise comment.
// (count drains via SteeringConsumed — possibly next turn, per the probe.)

// SteeringConsumed / SteeringCleared / SessionCreated: count logic UNCHANGED
//   (-1 saturating / =0 / =0); echo flips unchanged.

// SteeringUnsupported: ADD count clear (the optimistic count must not leak):
Notification::SteeringUnsupported { message } => {
    self.add_system_message(message.clone());
    self.steering_queued = 0;                               // NEW
    self.flip_queued_steer_echoes(SteerEchoStatus::Unsupported, false);
    true
}
```

**Verification (all 9 design fences):**

- [ ] **Unit tests pass.** In `crates/cyril-ui/src/state.rs` `mod tests`:
  - `add_steer_echo_increments_chip` — claim 1.
  - `consumed_decrements_and_flips` — claim 2 (extend `consumed_flips_oldest_queued_echo` to also assert the count; the echo half is the passed cheapest falsifier).
  - `turn_completed_does_not_reset_pending_steer` — claim 3 (**REWRITES** `turn_completed_resets_steering_queued`, which asserts the now-removed reset).
  - `wire_queued_does_not_double_count` — claim 4 (**REWRITES** `steering_queue_mirror`, which drives the count via wire `SteeringQueued`; rebuild it to drive via `add_steer_echo`).
  - `unsupported_clears_chip` — claim 5.
  - `cleared_zeroes_chip` — claim 6 (from the `steering_queue_mirror` rewrite).
  - `session_created_zeroes_chip` — claim 7 (from the rewrite; cf. existing `session_created_finalizes_prior_session_steer_echoes`).
  - `fifo_multi_steer_drains` — claim 8 (use **identical content** per stress fixture 4).
  - `consumed_at_zero_no_underflow` — claim 9.
- [ ] **Stress fixtures produce expected outcome** — fixtures 1–5 ARE the tests above (4↔#4, 3↔#3, 5↔#5, 8↔#4-same-content, 9↔#9).
- [ ] **prove-it-prototype oracle still agrees** — n/a re-run; the probe pinned the backend wire sequence the fences replay. The expected values are derived from it.
- [ ] **Budgets hold** — O(1) count edits, no new loop.
- [ ] `cargo test -p cyril-ui` green + `cargo clippy --all-targets -- -D warnings` clean + `cargo fmt --check` clean (only the touched lines must be fmt-clean).

---

## Plan Self-Review

**1. Every loop — complexity + budget.** No new loop. Existing `flip_queued_steer_echoes` (unchanged) is O(messages ≤ 500) per Consumed/Cleared/Unsupported/SessionCreated = O(1)-bounded, 0 syscalls. ✅
**2. Every fixture — bug class.** (1) double-count via forgotten wire-`++` removal; (2) late-steer reset regression; (3) unsupported count leak; (4) content-keyed flip on identical steers; (5) underflow. All adversarial, not happy-path. ✅
**3. Every doc-comment precondition.** Invariant is code-maintained (no caller precondition → no enforcement obligation). Underflow guard = runtime `saturating_sub` (load-bearing, survives release; not `debug_assert`). False `TurnCompleted` comment corrected. ✅
**4. Every write target.** UI state only (`messages`, `steering_queued`, `messages_version`); no stdout/stderr. ✅
**5. Every tracker reference.** cyril-7z7u (this, in_progress) ✓; cyril-8lfs (multi-observer gap — verified filed, `related` to cyril-7z7u) ✓. No un-tracked deferrals. ✅

No gaps in any of the five lists.

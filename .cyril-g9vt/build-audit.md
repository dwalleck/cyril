# cyril-g9vt — checkpointed-build audit

Plan: `budgeted-plan.md` (9 slices). Every slice gated on: tests both feature
configs, clippy `-D warnings` both, fmt, and the standing probe fences. No
production loop exceeds O(in-flight callbacks) ≈ single digits; the mediator's
map ops are O(1)–O(log n).

## Deviations from plan (all noted in commit messages)

1. **Enum variants STAGED per family** (not all 15 in slice 2) — lib-target
   dead_code under `-D warnings` forbids unconsumed variants. The full shape
   was authored at b36c3a8 and re-landed family by family (slices 3/4/5/6).
   `finish`/`resolve_err`/`WIRE_METHODS`/`CancelKey::new` carried matching
   `#[cfg]` stage gates that tightened as consumers landed.
2. **Slices 1+2+3 module promotions merged** — a module can't ship
   `#[cfg(test)]`-staged and then be imported by production in the same slice
   without a dead-code gap; the promotion rode slice 3's loop wiring.
3. **cfg'd-out presence instead of uninhabited markers** for the default-build
   channel item (`NeverCallback`) — same C13 unconstructibility, and it needs
   no `CallbackMeta` impl (the default drain matches it exhaustively).
4. **THE BIG ONE — drain moved OFF `run_loop`'s select! (slice 9, live-caught).**
   The design and slices 3–8 put the host-channel drain as a `run_loop` select
   arm. Live KAS session creation then DEADLOCKED: `run_loop` blocks on
   `conn.new_session().await` inside its command arm, KAS issues
   `getAccessToken` DURING new_session, and the in-loop drain can't run while
   the loop is blocked — new_session waits for the auth reply, the auth reply
   needs the loop to drain, the loop is blocked on new_session. Fix: a
   DEDICATED `spawn_local` drain task owns acceptance + resolution, sharing the
   mediator via `Rc<RefCell>` with `run_loop`'s CancelRequest (cancel_scope)
   and Shutdown hooks. This is the ADR's "concurrent resolution, off the loop"
   taken literally. Design C4 ("run_loop never awaits resolution") was true but
   INSUFFICIENT — the loop also can't be the resolver's scheduler.

## Live-drift STOP (slice 9) — the checkpointed-build gate working as designed

The unit + census fences (787 kas tests) ALL PASSED with the deadlocking
in-loop drain — because no test drives a real `conn.new_session` that triggers
a mid-RPC callback. Only the live oracle (real kiro-cli 2.16.0) exposed it.
Parity experiment: baseline main (detached worktree) created the session and
ran fs+terminal callbacks; the branch hung at "No SessionCreated." Root cause
diagnosed, drain relocated, live re-run PASSES with identical behavior to main.

## Slice 9 — live parity evidence (AC6), 2026-08-03, kiro-cli 2.16.0

- **v2, kas-feature build**: full harness sequence green; turn streamed and
  `TurnCompleted`. v2 sends no host callbacks — nothing crosses the mediator
  (the correct null result).
- **KAS free path** (`--agent-engine kas`, Host hooks): session created (auth
  `getAccessToken` mediated during new_session — the fixed deadlock), `File
  Search` + `Read File` Completed (fs host callbacks through the mediator),
  `/bin/echo g9vt-live-ok` Run Command Completed (terminal host callback
  through the mediator), permission prompt flowed (standard ACP path), and
  `TurnCompleted`. Every host-callback family crossed the mediator in one
  live turn.

## Claims → outcomes

| Claim | Outcome |
|---|---|
| C1 pure state machine | `host_mediator::tests::*` (8 sync unit tests, no async harness) |
| C2 cancel-after-accept aborts unpolled | `cancel_after_accept_signals_unpolled_job` + kind-scoped/dup-key fences |
| C3 concurrent resolution | `probe_g9vt_concurrent_callback_resolution` (slow 1s / fast 334µs) |
| C4 loop never awaits resolution | drain is its own task (slice-9 fix); live session proves the loop stays free during new_session |
| C5 bounded lossless ingress | `backpressure_awaits_capacity_losslessly` |
| C6 failure ordering | `finish_notifies_before_resolving` + `auth_failure_notification_*` |
| C7 responder drop clean | `resolve_err_reaches_receiver_and_survives_drop` |
| C8 shutdown aborts in-flight | `shutdown_signals_all_and_clears` |
| C9 refusal parity | dn91 refusal suite unchanged-passing |
| C10 handled-path parity, direct paths deleted | migrated fs/terminal/hooks suites + client deletion census (zero direct responder calls) |
| C11 exhaustive wiring | `every_handled_variant_crosses_the_mediator` (19-method walk, mutation-checked) |
| C12 zero-touch depth | default build compiles; mediator names no capability |
| C13 default-build posture | `probe_g9vt_c13` + default CI leg |
| C14 control semantics | `did_change_gated_by_hooks_direction` (None drop / Outbound HooksChanged) |
| C15 live parity both engines | this audit's evidence |

## Discovered during the build

- The mid-RPC-callback deadlock (fixed in-branch) is a general property of any
  loop-issued RPC that can trigger a host callback — documented at the drain
  construction so a future maintainer keeps the drain off the loop. No separate
  ticket: the fix is complete and fenced by the live check.

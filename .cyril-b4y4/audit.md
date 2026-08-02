# cyril-b4y4 — slice 8 audit: mutations + manual fences

Date: 2026-08-02. Tree at slice 7 (`1fef44a`); every mutation reverse-edited
(never `git checkout`), tree verified byte-identical to HEAD afterward
(`git diff --stat` empty), suites re-run green (758/758 kas, 551/551 default).

## Mutation M2 — absorb without clearing the ledger (design C4, ex-B16)

Mutant: `absorb_if` reads the companion by reference instead of `take()`.

Result: **killed, 2/758 failed, 756 green** — perfect localization.

- `turn_mediator::tests::mediator_matrix_all_dispositions` FAILED at **f3**
  (`left: Absorb{owner: 0, ...}` where DropStale expected) — one cell EARLIER
  than the designed f9→f10 pair: the uncleared ledger absorbs turn#0's stale
  duplicate the moment it should have been stale-dropped. Discrimination is
  stronger than designed.
- `bridge::tests::turn_mediation_probe_scenario` FAILED (loop level).

Pre-extraction, this exact mutant passed every bridge fixture (signed
blindness B16, re-measured in cyril-ri8q). B16 is now a red test. PROMOTED.

## Mutation M3 — release-before-absorb in the unstamped arm (a71q falsifier)

Mutant: the Wire-companion absorb is skipped whenever a turn is live
(equivalent to checking the active turn first).

Result: **killed, 2/758 failed, 756 green.**

- `mediator_matrix_all_dispositions` FAILED at **f9** with its designed
  message ("M3: a release here means absorb-first is broken").
- `turn_mediation_probe_scenario` FAILED (loop level, same cell).

## Manual fences (approved at the design pause)

| Claim | Check | Result |
|---|---|---|
| C6 | `grep -cE 'AgentEngine\|engine\.kind\|agent_client_protocol' turn_mediator.rs` | **0** — the module names no engine kind and imports no acp |
| C8 | `companion.take()` sites | **1** (`absorb_if`) |
| C8 | `"absorbed expected companion"` log sites | **1** (`PendingAbsorb::second`) |
| C8 | `after_arrival(` occurrences | **3** = definition + exactly the two release call sites |
| C11 | `"dropping unowned terminal"` log sites | **1** — the ex-silent drop now logs |

## B17/B18 re-assessment (AC4)

- **B16 → PROMOTED** (M2 above; fences: matrix f3/f9→f10 pair + loop scenario).
- **B18 → policy half PROMOTED**: `Disposition::ForwardTurnComplete` is
  returned only for owned/scoped releases — matrix cell f4 asserts a foreign
  terminal is `Forward`, not a completion. The loop-wiring half (deferred
  disconnect keys off the variant) remains harness-level, same residual
  rationale as a71q signed.
- **B17 → remains a signed blindness**: prompt-task abort lives with the
  JoinHandles in `run_loop` (approved narrow scope, design negative-space #3);
  the a71q rationale (LocalSet teardown makes abort deterministic) is
  unchanged by this extraction.

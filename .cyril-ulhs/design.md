# Design: cyril-ulhs

## Route and inputs

- Route: **Empirical** (`route.md`, T1 fired on P1–P3). `spec.md`: `N/A — behavior fully explicit`; the behavior set is `route.md` T4's B1–B5, amended by the requester's option-A decision (Approval section below): B4 is re-scoped from "restore `.expect()` in the `render_open_picker` helper" to "relocate the `.expect()` into the three `#[test]` fns of `picker_active_marker.rs` and delete the let-else helper"; B5's forced deletion of the 17 `#[expect(clippy::expect_used)]` attributes is confirmed as a technical correction the approved change determines.
- Empirical premises and results (`evidence.md`): P1 FALSE — the config does not reach a non-`#[test]` helper fn in a `tests/` crate; P1b — the issue's asymmetry is `is_never_like(Infallible)`, not Option-vs-Result; P2 PASS — non-test `.expect()` still trips with the config; P3 PASS — all 17 `#[expect(clippy::expect_used)]` attributes on `#[cfg(test)]` items become `unfulfilled_lint_expectations`; P4 PASS — `#[test]` fns and `#[cfg(test)]` modules ARE covered.
- Independent oracles established there: (1) the standalone crate `.cyril-ulhs/probe.oracle/` (own workspace root, own lint table, `--keep-going`, explicit shadowing `clippy.toml`); (2) the pinned tool source at `rust-1.94.0` (`is_in_test`, `unwrap_expect_used::check`, `is_never_like`) and ratatui-core 0.1.0's `TestBackend::Error = Infallible`.
- Comparisons: `evidence.md` § Comparisons, P1–P4 all agree.

## Input shapes

Inputs the design touches and every production-reachable shape, each with a status.

| Shape | Status |
|---|---|
| Root `clippy.toml` absent (today) | Covered — C1 mutation M1 (absence → red) |
| Root `clippy.toml` present with exactly `allow-expect-in-tests = true` | Covered — C1, C3, C6 |
| `clippy.toml` additionally carrying `allow-unwrap-in-tests = true` | Covered — C4 mutation M3 (red); unapproved, must never land |
| `.expect()` receiver `Option` inside a `#[test]` fn | Covered — C1 (the three picker sites) |
| `.expect()` receiver `Result<_, E>` with inhabited `E` inside a `#[test]` fn | Covered — C1 via oracle `tests/helper.rs:39` (P4); no cyril site changes |
| `.expect()` receiver `Result<_, Infallible>` (picker `render_text` lines 48/57) | `N/A — reason: never linted in any configuration (evidence P1b); unchanged by this change` |
| `.expect()` in a non-`#[test]` helper fn of a `tests/*.rs` crate | `N/A — reason: permanent non-goal, the config cannot reach it (P1); the picker helper is deleted, other helpers keep their let-else by requester constraint (3)` |
| `.expect()` inside a `#[cfg(test)]` module WITH `#[expect(clippy::expect_used)]` (17 sites) | Covered — C3 (attribute deleted; M2 re-adds one → red) |
| `.expect()` inside a `#[cfg(test)]` module WITH `#![allow(clippy::unwrap_used, clippy::expect_used)]` (~50 sites) | `N/A — reason: requester constraint (2) — untouched, report only; \`allow\` never errors when unused so the gate stays green` |
| `.expect()` in non-test production code | Covered — C5 (receipt P2, re-run on the final tree) |
| `.unwrap()` anywhere | Covered — C4 (`unwrap_used = "deny"` unchanged; the 8 `#[expect(clippy::unwrap_used)]` attributes stay fulfilled) |
| Attribute form: outer `#[expect(clippy::expect_used)]` (13 sites) | Covered — C3, C6 |
| Attribute form: outer multi-line `#[expect(\n clippy::expect_used,\n reason = "…"\n)]` (`cyril-core/src/voice.rs:75-78`, `cyril-voice/src/lib.rs:138-141`) | Covered — C3, C6 (all four lines deleted) |
| Attribute form: inner `#![expect(clippy::expect_used)]` (`source_observer.rs:458`, `source_turn.rs:1207`, `capture_forwarder.rs:204`) | Covered — C3, C6 (attribute line plus its trailing blank deleted) |
| Attribute stacked with `#[expect(clippy::unwrap_used)]` (`commands/mod.rs:514-515`) | Covered — C3, C4, C6 (only line 515 deleted; 514 stays and must remain fulfilled) |
| `commands/mod.rs` sibling-PR hunk disjointness (cyril-6bwr edits lines 170–250) | Covered — C6 (hunks confined to ~515 and ~1659) |

Decision cells:

| Cell | Status |
|---|---|
| clippy `is_in_test` dispatch: `#[test]` fn leg | Covered — C1 |
| `is_in_test` dispatch: `cfg(test)`-attribute leg | Covered — C3 (the suppressed sites are exactly those the deleted attributes guarded) |
| `is_in_test` dispatch: neither leg (helper fn, production fn) | Covered — C5 (production) ; `N/A — permanent non-goal` (helper fn, above) |
| `-D warnings` promotion of `expect_used = "warn"` and of `unfulfilled_lint_expectations` (warn-by-default) | Covered — C1, C3 (both mutations rely on it; the gate command is CI's verbatim) |
| Bound imposed on callers: none | `N/A — reason: no numeric bound in this change` |

## Placement

Capability: **test-scoped `expect_used` policy**.

- **Owner:** root `clippy.toml` (clippy configuration) beside root `Cargo.toml` `[workspace.lints.clippy]` (lint levels). The two files together are the single policy owner; every member crate inherits via `[lints] workspace = true`.
- **New seam:** none — clippy's documented configuration surface, an existing interface.
- **Forbidden:** per-crate lint overrides (CLAUDE.md "never override lints per-crate"); any `#[allow(...)]`; any new `#[expect(clippy::expect_used)]`; `allow-unwrap-in-tests`; any edit to `Cargo.toml` lint levels; any edit beyond attribute lines in the 15 forced files (17 sites; the requester's "16" repeats the hand-back's miscount); any edit to the ~50 `#![allow]` test modules or the let-else workarounds outside the picker helper (requester constraints 1–3).

## Module shape

`N/A — route.md T2: no public interface, schema, seam, dependency direction, or responsibility owner changes; touched files are lint configuration, one test crate (shrinks), and attribute-only deletions; no repository file-length gate exists (T2 evidence) and projected growth is negative.`

## Claims

- **C1** — With root `clippy.toml` (`allow-expect-in-tests = true`), a `.expect()` inside a `#[test]` fn emits no `clippy::expect_used`, so the CI gate is green with `picker_active_marker.rs`'s three tests calling `ui.picker().expect("show_picker did not open a picker")` directly, and red the moment the config is absent.
- **C3** — With the config present, every `#[expect(clippy::expect_used)]` / `#![expect(clippy::expect_used)]` on a `#[cfg(test)]` item is an `unfulfilled_lint_expectations` error under `-D warnings`; deleting all 17 is necessary and sufficient for green, and re-adding any one turns the gate red at that line.
- **C4** — `clippy::unwrap_used` behavior is unchanged: the 8 `#[expect(clippy::unwrap_used)]` attributes remain fulfilled (gate green), and adding `allow-unwrap-in-tests = true` would turn the gate red at exactly those 8 sites.
- **C5** — Non-test discipline is unchanged: `Cargo.toml` is byte-identical to `origin/main`, and a `.expect()` in non-test code still trips `clippy::expect_used` with the config present.
- **C6** — The footprint is exactly the approved one: `clippy.toml` holds one key; `picker_active_marker.rs` loses the `render_open_picker` helper and gains three inline `.expect()` calls; the 15 forced files lose only attribute lines (with `commands/mod.rs` hunks at ~515 and ~1659); no `#[allow]`/`#[expect]` is added; nothing else in the tree changes.

(C2 was folded into C1 during drafting: the picker tests' pass/fail meaning is unchanged because their assertions are untouched; the affected-unit-test gate item carries that proof.)

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | config present ⇒ `#[test]`-fn `.expect()` unlinted; absent ⇒ linted | `#[test]` fn Option-`.expect()` ×3 in `picker_active_marker.rs`; config present/absent | Run `cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings` on the final tree → exit 0 and zero `clippy::expect_used` diagnostics; falsified by any `expect_used` at the picker file. Other cause of a green: the test target not scheduled — excluded by `--keep-going` and by listing compiled units (`test picker_active_marker`). Positive control for absence: M1. | Standalone crate `probe.oracle/tests/helper.rs:39` (direct `#[test]`-fn expect: fires with `allow-expect-in-tests = false`, silent with `true`, same targets compiled) — evidence P4, retained; plus tool source `is_in_test_function` (`#[rustc_test_marker]` name match). Different mechanism: a different crate, no cyril workspace machinery, and a static reading. | **M1:** `rm clippy.toml` → gate red: `error: used \`expect()\` on an \`Option\` value` at `crates/cyril-ui/tests/picker_active_marker.rs` on each of the three test lines (plus `expect_used` in every formerly-`#[expect]`-guarded module — expected collateral). Restore: recreate the file → green. | The three `#[test]` fns of `crates/cyril-ui/tests/picker_active_marker.rs` calling `ui.picker().expect(...)`, linted by CI's clippy leg on every push. | 3 min (one workspace clippy) | PASS — discharged at the slice 1 checkpoint 2026-09-27 (`plan.md` § Slice 1 checkpoint; `receipts/{GREEN,M1,M2,M3,C5}.diag`, `oracles/footprint.sh`) |
| C3 | 17 attributes deleted ⇒ green; any one re-added ⇒ red | 17 `#[expect(clippy::expect_used)]` sites in the three attribute forms | Same gate run as C1 → zero `unfulfilled_lint_expectations`; falsified by any such diagnostic. Positive control: M2. | `probe.oracle/src/lib.rs:15` (expectation fulfilled with `false`, unfulfilled with `true`) — evidence P3, retained; plus the hand list of 17 sites (`grep -rn "expect(clippy::expect_used\|clippy::expect_used,$" crates/`) matched item-by-item against B-keepgoing's 17 diagnostics. | **M2:** re-insert the line `#[expect(clippy::expect_used)]` between `#[cfg(test)]` and `mod tests {` in `crates/cyril-memory/src/paths.rs` (line 213 on main) → gate red: `error: this lint expectation is unfulfilled` at `crates/cyril-memory/src/paths.rs:213`. Restore: delete the line → green. | CI's clippy leg: rustc's `unfulfilled_lint_expectations` (warn-by-default) under `-D warnings` — permanent and already in place. | 3 min | PASS — discharged at the slice 1 checkpoint 2026-09-27 (`plan.md` § Slice 1 checkpoint; `receipts/{GREEN,M1,M2,M3,C5}.diag`, `oracles/footprint.sh`) |
| C4 | `unwrap_used` unchanged; 8 `#[expect(clippy::unwrap_used)]` stay fulfilled | `.unwrap()` in test and non-test code; the extra config key | Same gate run → zero `unfulfilled_lint_expectations` and zero `clippy::unwrap_used`; falsified by either. Positive control: M3. | Hand count of `#[expect(clippy::unwrap_used)]` sites = 8 (`grep -rn "expect(clippy::unwrap_used)" crates/`): M3's diagnostic count and file:line set must equal that list item-by-item. Different mechanism from the fence: a text census, not rustc. | **M3:** append `allow-unwrap-in-tests = true` to `clippy.toml` → gate red with `unfulfilled_lint_expectations` at exactly `kiro_agent_config.rs:288`, `commands/subagent.rs:202`, `commands/mod.rs:514`, `types/hook.rs:114`, `types/config.rs:139`, `cyril-ui/src/stream_buffer.rs:69`, `cyril-memory/src/config.rs:318`, `cyril-memory/src/store.rs:2082` (post-deletion line numbers shift by −1 in `commands/mod.rs` only). Restore: remove the key → green. | The 8 `#[expect(clippy::unwrap_used)]` attributes themselves, under CI's clippy leg. | 3 min | PASS — discharged at the slice 1 checkpoint 2026-09-27 (`plan.md` § Slice 1 checkpoint; `receipts/{GREEN,M1,M2,M3,C5}.diag`, `oracles/footprint.sh`) |
| C5 | non-test discipline unchanged | non-test production fn `.expect()`; `Cargo.toml` | (a) `git diff origin/main -- Cargo.toml` → empty; falsified by any hunk. (b) Re-run probe step C on the final tree: append `pub fn ulhs_nontest_probe() -> u8 { Some(1u8).expect("probe") }` to `crates/cyril-core/src/lib.rs`, run the gate → `clippy::expect_used` at that line; revert; `git diff` clean. Falsified by silence at that line. | Standalone crate `src/bin/nontest.rs:7,12` fire with the config (evidence P2, retained) and tool source: neither `is_in_test` leg holds for a free fn. | `N/A — approved risk: no fence to mutate` | `N/A — approved risk: requester specified "Non-test discipline unchanged (receipt, not a commit)"; the receipt (C.txt on main, re-run on the final tree) is the proof; the standing enforcement is Cargo.toml's unchanged \`expect_used = "warn"\` under CI's \`-D warnings\`` | 3 min | PASS — discharged at the slice 1 checkpoint 2026-09-27 (`plan.md` § Slice 1 checkpoint; `receipts/{GREEN,M1,M2,M3,C5}.diag`, `oracles/footprint.sh`) |
| C6 | footprint exactly as approved | the diff | Run `bash .cyril-ulhs/oracles/footprint.sh` (diff vs `git merge-base HEAD origin/main`) → every check prints `C6 PASS`: file set ⊆ {`clippy.toml`, picker test, 15 forced files, `.cyril-ulhs/**`}; each forced file has zero `+` lines and only attribute-form `-` lines; `commands/mod.rs` hunks start within 505–525 and 1650–1670; `clippy.toml` has exactly one non-comment line `allow-expect-in-tests = true`; no added line anywhere matches `#[allow(` / `#![allow(` / `#[expect(clippy::expect_used`; `Cargo.toml` untouched. Falsified by any `C6 FAIL`. | `git diff --stat` read by hand against the approved list (a human read of the same diff, independent of the script's regexes). | `N/A — approved risk: no fence to mutate` | `N/A — approved risk: a one-time diff-scope obligation of this PR (coordinator constraints 1–3); nothing permanent to fence` | 1 min | PASS — discharged at the slice 1 checkpoint 2026-09-27 (`plan.md` § Slice 1 checkpoint; `receipts/{GREEN,M1,M2,M3,C5}.diag`, `oracles/footprint.sh`) |

## Non-goals and future work

- **Permanent non-goal:** making `.expect()` legal in non-`#[test]` helper fns of `tests/*.rs` crates through configuration — clippy 0.1.94 has no knob (`is_in_test` has exactly two legs, evidence P1); such sites either keep `let … else { panic!() }` or move the `.expect()` into a `#[test]` fn, decided per site.
- **Permanent non-goal for this change:** `allow-unwrap-in-tests` — explicitly unapproved by the requester ("stays UNAPPROVED"); C4/M3 fence it out.
- **Permanent non-goal:** touching `render_text`'s `Result<_, Infallible>` `.expect()`s — never linted, nothing to fix.
- **Intended future work (report only, per requester constraints 2–3; tracker is read-only for this session, so the orchestrator owns filing — no verified ID can be cited here and this deviation is recorded in the hand-back):** (a) the ~50 `#![allow(clippy::unwrap_used, clippy::expect_used)]` test-module attributes that violate the zero-`#[allow]` rule, whose `expect_used` half is redundant once this lands; (b) the let-else-panic workarounds in the other 9 test files listed in `evidence.md` § Related issues, which the config cannot help when they sit in helper fns.

## Falsifier run log

Cheapest row by cost is C6 (1 min), but it needs the final diff, so the cheapest runnable-before-approval falsifier is C1's oracle half, already run and retained under Evidence validity: `bash .cyril-ulhs/oracle.sh` (2026-09-27, `probe-out/oracle-no-config-keepgoing.diag` vs `oracle-with-config-keepgoing.diag`) — `tests/helper.rs:39` (`#[test]`-fn `Option`/`Result` expect) present with `allow-expect-in-tests = false`, absent with `true`, all three targets (`bin nontest`, `lib ulhs_oracle`, `test helper`) compiled in both runs → **PASS** (claim C1's mechanism survives; the cyril half is discharged at the slice gate).

## Approval

Requester (orchestrator relaying the user's decision), 2026-09-27, verbatim:

> "Decision: option A. Rationale to record in route.md/design.md: the approved behavior (tests may use `.expect()` without tripping the CI clippy gate; mechanism = clippy.toml `allow-expect-in-tests = true`) is unchanged; deleting the 17 `#[expect(clippy::expect_used)]` / `#![expect(...)]` attributes that the config renders unfulfilled is a technical correction the approved change determines (they are dead once the config lands), and relocating the fence's `.expect()` into the three `#[test]` fns of picker_active_marker.rs (deleting the let-else helper) preserves the fence's meaning under the probe-verified scope of the config. `allow-unwrap-in-tests` stays UNAPPROVED. Constraints: (1) touch ONLY the attribute lines in the 16 extra files — no other edits there — and in crates/cyril-core/src/commands/mod.rs keep your edits confined to the two attribute lines (~515 and ~1659), because a sibling PR (#cyril-6bwr) is editing that file around lines 170-250 and the hunks must stay disjoint; (2) leave the ~50 `#![allow(clippy::unwrap_used, clippy::expect_used)]` test-module attributes alone (report them, do not touch); (3) do not touch the other let-else workarounds in the 10 test files you listed (report only). Then run the full CI-mirror gate (with `--keep-going` on the clippy legs as an extra receipt), push, and open the PR with an explicit "Footprint" section listing the 16 forced files and why."

Original scope decisions (task brief, 2026-09-27) carried forward: `clippy.toml` contains ONLY `allow-expect-in-tests = true` with a short comment; red-then-green proof; non-test receipt "receipt, not a commit".

Risk acceptances approved: (1) C5 — no committed fence for non-test discipline; the receipt stands in ("receipt, not a commit"). (2) C6 — no permanent fence for the one-time footprint constraints (1)–(3); the diff-inspection oracle runs at the checkpoint and in the PR.

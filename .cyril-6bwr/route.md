# Route: cyril-6bwr

Change: remove the producer-less `CommandResultKind::ShowPicker` variant, its `CommandResult::show_picker` constructor, and the unreachable dispatch arm in `App::handle_command_result` (issue option (a), no replacement).
Date: 2026-09-27
Base revision: `f9bc81d8` (main) — every line number below is anchored there.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The only premise is "the variant has zero producers", and it is covered by current repository evidence, not by any external or system behavior. (a) `grep -rn 'ShowPicker\|CommandResult::show_picker\|show_picker(' crates/ docs/ CLAUDE.md CONTEXT.md` at `f9bc81d8`: the variant name occurs only at its definition (`crates/cyril-core/src/commands/mod.rs:177`), three doc comments citing it as the "same split" example (`mod.rs:184`, `:192`, `:201`), the constructor (`mod.rs:239`, `:241`) and the dispatch arm (`crates/cyril/src/app.rs:2043`). Every other `show_picker(` hit is `UiState::show_picker` (`crates/cyril-ui/src/state.rs:2375`), its tests, or the live notification wiring at `app.rs:1462`. Zero production producers; zero tests of the variant. (b) `tethys index` (exit 0) then `tethys callers "CommandResult::show_picker" --lsp` → "No callers found" (rust-analyzer reported degraded — "Failed to load workspaces" — so the result is the indexed-callers set; grep (a) is the primary receipt, tethys the corroborating one). (c) Orphaning commit: `git log -S'ShowPicker' -- crates/cyril-core/src/commands/mod.rs` shows `ed13a75c` ("refactor: replace blocking ExtMethodWithResponse with notification-driven flow", 2026-03-22) removed the last producer (`return Ok(CommandResult::show_picker(self.name.clone(), options));`) and its only test (`if let CommandResultKind::ShowPicker { title, options } = result.kind`), adding `self.ui_state.show_picker(command.clone(), options.clone())` under `Notification::CommandOptionsReceived` instead. The variant has been dead since that commit. | no |
| 2 | Structural module shape | **Interface narrows.** `CommandResultKind` and `CommandResult` are `pub` items of `cyril-core` (`crates/cyril-core/src/commands/mod.rs:165-224`, not `#[non_exhaustive]`), consumed by the `cyril` binary's exhaustive `match result.kind` in `App::handle_command_result` (`crates/cyril/src/app.rs:2031-2118`) and pattern-matched in `cyril-core` tests (`commands/workflow.rs`, `commands/subagent.rs` — none names `ShowPicker`). Removing a public enum variant and a public constructor is a public-API change of a workspace library crate, so this test fires even though no caller exists. **Responsibility ownership is unchanged:** the command layer (`cyril-core/commands`) still returns typed intents; the App (`cyril/app.rs`) still projects them onto `UiState`; the one live picker path — `Notification::CommandOptionsReceived` → `App` (`app.rs:1453-1465`) → `UiState::show_picker` (`state.rs:2375`) — is not touched. No seam, dependency direction, or module is created, split, merged, or moved. Affected modules: `crates/cyril-core/src/commands/mod.rs` (owner of `CommandResult`, 1938 lines) and `crates/cyril/src/app.rs` (orchestrator / protected parent, 7619 lines); candidate owners: none, both `retain`. **Length review:** no repository length gate exists — `grep -rn -i 'too.many.lines\|too_many_lines\|max.lines\|length gate\|line.count.*threshold\|file-length' Cargo.toml clippy.toml .github scripts docs/agents` → no hits, and `clippy.toml` is absent. The change only removes lines (projected: `mod.rs` −~13, `app.rs` −3; uncertainty ±2), so no threshold can be approached; trigger verdict: none. | yes |
| 3 | Production-scale risk | None. The removed arm was unreachable (T1), so no runtime path changes: no latency, throughput, memory, concurrency, or data-volume dimension is touched. The change is resolved entirely at compile time. | no |
| 4 | Explicit behavior | Fully explicit; the requester fixed every decision (issue option (a)). Behavior contract (the behavior source, since `spec.md` is N/A): **B1** Given the workspace after the change, when `crates/` is censused for `ShowPicker` / `CommandResult::show_picker`, then there are zero hits (definition, constructor, dispatch arm, and the three doc-comment citations are all gone). **B2** Given the workspace after the change, when `cargo clippy --workspace --all-targets --all-features -- -D warnings` runs, then it exits 0 — in particular the now-unused `use crate::types::CommandOption;` at `mod.rs:10` (its only other uses were the variant field and the constructor) is removed rather than left to trip `unused_imports`. **B3** Given `Notification::CommandOptionsReceived { command, options }` with non-empty `options` reaches `App::handle_notification`, when it is handled, then `UiState::show_picker(command, options)` opens the picker exactly as before — `app.rs:1453-1465` and `UiState::show_picker` are byte-for-byte unchanged, and the existing `UiState::show_picker` tests (`crates/cyril-ui/tests/picker_active_marker.rs`, `picker_viewport.rs`, `state.rs` tests) still pass. **B4** Given the doc comments on `ShowThemePicker`, `Steer`, and `ToggleVoice` in `commands/mod.rs`, when read, then each "same split as" phrase names only variants that still exist and still describes the command/App responsibility split (command layer returns the intent; App owns the cross-module effect). **B5** Given `docs/` and `CLAUDE.md`, when read, then no living document presents `CommandResult::ShowPicker` as an existing path: `docs/omp-review-command-analysis.md:96` ("`CommandResult::ShowPicker` … exists") is corrected to name the live notification path; `CLAUDE.md:190` already names the live path and is unchanged; the live `.agents/summary/` surfaces (`architecture.md:119`, `components.md:86`, `interfaces.md:182` — found by review round 1, F1) drop their bare `ShowPicker` mentions; the dated, status-stamped historical records `docs/plans/2026-03-21-cyril-v2-architecture-design.md:398` and `docs/plans/2026-03-22-v2-known-regressions.md:39` describe the pre-`ed13a75c` design and are left as history. | yes |

Unknown tests: none

## Selected route

Structural — T2 fires: a public enum variant and a public constructor leave `cyril-core`'s surface (interface narrows), with no empirical premise (T1 no), no scale risk (T3 no), and fully explicit behavior (T4 yes). Precedence Empirical > Structural > Local selects Structural.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict yes; contract B1–B5 recorded above) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict no) |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural route

## Downstream sequence

falsifiable-design → budgeted-plan → checkpointed-build (interrogated-spec and prove-it-prototype skipped per the N/A rows above).

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate.

Result: 2026-09-27 | `design.md` (approved, five claims all `PASS`), `plan.md` (one slice, one increment), slice 1 checkpoint record in `plan.md` — eleven gate items with no `FAIL` (8 `PASS`, 3 contract-backed `N/A`), five named mutations red then restored green, clean full gate re-run `GATE-OK` ×8 | PASS. Final T2 recheck: every module shrank (mod.rs 1938→1926, app.rs 7619→7616); no length trigger; no interface beyond the approved narrowing.

# Evidence: cyril-ulhs

Revision under test: `f9bc81d8` (branch `chore/cyril-ulhs-clippy-expect-in-tests`, tree unmodified between runs — every probe edit is reverted by the script and `git status --porcelain` for the touched paths was empty after each run). Toolchain: `clippy 0.1.94 (4a4ef493e3 2026-03-02)`, `cargo 1.94.0`. Gate command = CI's: `cargo clippy --workspace --all-targets --all-features -- -D warnings` (run with `env -u CARGO_TARGET_DIR` because this shell exports it empty).

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | A root `clippy.toml` with `allow-expect-in-tests = true` suppresses `clippy::expect_used` for the `Option::expect` in `render_open_picker` — a non-`#[test]` helper fn in the integration-test crate `crates/cyril-ui/tests/picker_active_marker.rs` (line 70 once restored). | With the config present and the test target actually compiled, does clippy emit `expect_used` at `picker_active_marker.rs:70`? | **PASS — discharged, premise is FALSE.** It still fires. Probe (cyril, `--keep-going`) and oracle (standalone crate, `tests/helper.rs:11/15/19`) agree; the tool source explains it (see Validated / learned). |
| P1b | The asymmetry the issue reports — `Result::expect` at lines 48/57 of the same helper file never fires while `Option::expect` does — is an Option-vs-Result behavior of the lint. | Why are lines 48/57 silent under the same gate that flags line 70? | **PASS — discharged, premise is FALSE.** Not Option-vs-Result: those receivers are `Result<_, Infallible>` (ratatui-core `TestBackend::Error = core::convert::Infallible`), and the lint returns early for a never-like error type. Oracle: an `io::Result<u8>` helper in the same position fires. |
| P2 | With the config present, a `.expect()` in NON-test code still trips `clippy::expect_used` (the `expect_used = "warn"` level promoted by `-D warnings`). | Does `expect_used` fire at a deliberate non-test `.expect("probe")` site with the config present? | **PASS — validated.** cyril `crates/cyril-core/src/lib.rs:23` (probe step C) and oracle `src/bin/nontest.rs:7` (Option) / `:12` (Result) all fire with the config. |
| P3 | The 17 `#[expect(clippy::expect_used)]` / `#![expect(clippy::expect_used)]` attributes on `#[cfg(test)]` items go unfulfilled once the config suppresses the lint inside them, turning `-D warnings` red (`unfulfilled_lint_expectations`). | With the config present and every unit built, does `unfulfilled_lint_expectations` fire at each of the 17 sites? | **PASS — validated.** All 17 fire in cyril (step B `--keep-going`); oracle `src/lib.rs:15` reproduces the shape. |
| P4 | The config DOES suppress `expect_used` inside `#[test]` fns and inside `#[cfg(test)]`-attributed modules (the coverage the config is documented to give). | Do `#[test]`-fn and `#[cfg(test)]`-module sites fire without the config and go silent with it? | **PASS — validated.** Oracle `tests/helper.rs:39` (inside `#[test] fn`) and `src/lib.rs:32/33` (`#[cfg(test)] mod plain_tests`) fire in the `false` run and are absent in the `true` run with the same three targets compiled; in cyril the 17 unfulfilled expectations are themselves proof that the lint was suppressed inside those `#[cfg(test)]` modules. |
| — | Requested behavior B1–B5 (`route.md` T4) | N/A — behavior contract, not an external premise; it is what the design must satisfy. | `N/A — spec territory` |
| — | `allow-unwrap-in-tests` | N/A — explicitly not approved by the requester; the analogous `#[expect(clippy::unwrap_used)]` fallout is noted under Related issues only. | `N/A — out of scope` |

## Data

- Source: production-shaped — the repository's own source at `f9bc81d8` plus the pinned toolchain; the only inputs are the workspace files and a 3-target standalone crate written for the oracle.
- Shape: the exact CI gate over the exact workspace; the oracle mirrors the three code shapes at stake (non-`#[test]` helper in a `tests/` crate, `#[test]` fn, `#[cfg(test)]` module with and without `#[expect]`, non-test fn) with the same per-package lint levels.
- Safety: probe edits (picker helper, `cyril-core/src/lib.rs`, root `clippy.toml`) are applied on the working tree and reverted by an `EXIT` trap; each script refuses to start on a dirty tree for those paths; verified empty `git status --porcelain` after every run. Nothing is committed from a probe; `target/` is build cache. Approval: none needed (no production state, no snapshot).

## Probe

- Files: `probe.sh` (steps A/B/C), `probe-keepgoing.sh` (step B repeated with `--keep-going`), `probe-diag.py` (lists `expect_used` / `unwrap_used` / `unfulfilled_lint_expectations` per primary span from cargo JSON). Outputs under `probe-out/` (`<step>.diag`, `<step>.exit`, `<step>.txt` rendered, `A.picker.diff`, `C.core_lib.diff`, run logs; the raw cargo JSON is not committed).
- Mechanism: restore `render_open_picker` to `let state = ui.picker().expect("show_picker did not open a picker");` (the edit the issue names), toggle the root `clippy.toml`, run the CI gate with `--message-format=json`, and read diagnostics by file:line. Step C appends a `pub fn ulhs_nontest_probe() -> u8 { Some(1u8).expect("probe") }` to `crates/cyril-core/src/lib.rs` (non-test) with the config present.
- Runs: `bash .cyril-ulhs/probe.sh` (2026-09-27), then `bash .cyril-ulhs/probe-keepgoing.sh` (2026-09-27) after learning that cargo stops scheduling units after the first failing one, so step B could not show whether the cyril-ui test target had been linted at all.

Probe outputs (all exits 101):

| Step | Config | `expect_used` | `unfulfilled_lint_expectations` | Units |
|---|---|---|---|---|
| A | none | `picker_active_marker.rs:70` only — lines 48/57 (`Result::expect`) silent | none | scheduling stopped after the first error |
| B | `true` | none listed — **but `cyril-ui`'s test target was never scheduled** (see B-keepgoing) | 14 of 17 (the 3 `crates/cyril/` sites unscheduled) | partial |
| C | `true` + non-test probe | `crates/cyril-core/src/lib.rs:23` | 6 (cyril-core only; downstream unscheduled) | partial |
| B-keepgoing | `true`, `--keep-going` | **`picker_active_marker.rs:70` still fires** | **all 17**: `cyril-core/src/commands/mod.rs:515,1659`, `cyril-core/src/voice.rs:76`, `cyril-core/src/protocol/source_observer.rs:458`, `cyril-core/src/types/memory.rs:734`, `cyril-memory/src/{client.rs:278, lesson.rs:293, paths.rs:213, permissions.rs:155, project.rs:240, runtime.rs:421, source_turn.rs:1207, wire.rs:1357}`, `cyril-voice/src/lib.rs:139`, `cyril/src/capture_forwarder.rs:204`, `cyril/src/memory_runtime.rs:800,928` | every workspace target compiled, including `test picker_active_marker cyril-ui/tests/picker_active_marker.rs` |

Red text captured from step A (the requester's "capture the lint text" item), `probe-out/A.txt`:

```
error: used `expect()` on an `Option` value
  --> crates/cyril-ui/tests/picker_active_marker.rs:70:17
   |
70 |     let state = ui.picker().expect("show_picker did not open a picker");
   |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: if this value is `None`, it will panic
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.94.0/index.html#expect_used
   = note: `-D clippy::expect-used` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::expect_used)]`
```

Non-test receipt from step C (`probe-out/C.txt`, config present; `C.core_lib.diff` holds the probe edit, reverted afterwards):

```
error: used `expect()` on an `Option` value
  --> crates/cyril-core/src/lib.rs:23:5
   |
23 |     Some(1u8).expect("probe")
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: if this value is `None`, it will panic
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.94.0/index.html#expect_used
   = note: `-D clippy::expect-used` implied by `-D warnings`
```

## Oracle

- Mechanism 1 — `probe.oracle/`, a standalone crate that is its own `[workspace]` root, declares `unwrap_used = "deny"` / `expect_used = "warn"` per-package (no inheritance), has no dependencies or features, its own `--target-dir`, `--keep-going`, and its own `clippy.toml` written explicitly (`false` for the "no-config" run, `true` for the "with-config" run — clippy walks parent directories and takes the first config it meets, so the crate must shadow the worktree root). Targets: `src/lib.rs` (`#[cfg(test)]` module with `#[expect]` = P3; `#[cfg(test)]` module without = P4), `src/bin/nontest.rs` (non-test sites = P2, kept out of the lib so a lint error cannot block the test target's compilation), `tests/helper.rs` (non-`#[test]` helpers with `Option`, `Result<_, &str>`, `io::Result` receivers = P1/P1b; `#[test]` fn sites = P4). It shares none of cyril's workspace machinery, feature flags, or dep-info state with the probe.
- Mechanism 2 — static reading of the tool's own source at tag `rust-1.94.0` (`clippy_utils/src/lib.rs`: `is_in_test = is_in_test_function || is_in_cfg_test`; `is_in_test_function` walks enclosing `ItemKind::Fn` items and matches their names against the module's `#[rustc_test_marker]` consts; `is_in_cfg_test` looks for a `cfg(test)` attribute on a parent; `clippy_lints/src/methods/unwrap_expect_used.rs`: `if allow_*_in_tests && is_in_test(...) { return }`, preceded by `if is_never_like(t_or_e_ty) { return }` on the `Result` error type; `clippy_utils/src/ty/mod.rs`: `is_never_like = ty.is_never() || (enum with zero variants)`), and of `~/.cargo/registry/src/*/ratatui-core-0.1.0/src/backend/test.rs:238` (`type Error = core::convert::Infallible;`) with `terminal/terminal.rs:157` (`new -> Result<Self, B::Error>`) and `:373` (`draw -> Result<CompletedFrame<'_>, B::Error>`). No lint is executed; the answer comes from reading definitions.
- Runs: `bash .cyril-ulhs/oracle.sh` (2026-09-27; outputs `probe-out/oracle-{no-config,with-config}*.diag/.exit`; the final clean runs are `oracle-no-config-keepgoing.*` and `oracle-with-config-keepgoing.*`, both with all three targets compiled: `bin nontest`, `lib ulhs_oracle`, `test helper`).

Oracle outputs (both exits 101):

| Run | `expect_used` sites | `unfulfilled_lint_expectations` |
|---|---|---|
| `allow-expect-in-tests = false` | `src/bin/nontest.rs:7,12` · `src/lib.rs:32,33` (plain `#[cfg(test)]`) · `tests/helper.rs:11` (Option helper) `:15` (`Result<_, &str>` helper) `:19` (`io::Result` helper) `:39` ×2 (inside `#[test] fn`) | none (`src/lib.rs:15` expectation fulfilled) |
| `allow-expect-in-tests = true` | `src/bin/nontest.rs:7,12` · **`tests/helper.rs:11,15,19` (helpers still fire)** | `src/lib.rs:15` |

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | B-keepgoing: `expect_used` at `picker_active_marker.rs:70` with the config present, target compiled | Mechanism 1: `tests/helper.rs:11` (Option helper) fires with `true`; Mechanism 2: `render_open_picker` is neither a `#[rustc_test_marker]`-named fn nor under a `cfg(test)` attribute, so `is_in_test` is false and the early return never happens | PASS — agree: premise FALSE |
| P1b | A: lines 48/57 silent, line 70 fires, same file, same run | Mechanism 1: `io::Result<u8>` helper (`:19`) fires, so `Result::expect` in that position is linted; Mechanism 2: `TestBackend::Error = Infallible`, `is_never_like(Infallible)` = true (zero-variant enum) → return before the lint | PASS — agree: never-like `E`, not Option-vs-Result |
| P2 | C: `cyril-core/src/lib.rs:23` fires with the config | `src/bin/nontest.rs:7` and `:12` fire with `true`; Mechanism 2: non-test code fails both `is_in_test` legs | PASS — agree |
| P3 | B-keepgoing: 17/17 sites `unfulfilled_lint_expectations` | `src/lib.rs:15` unfulfilled with `true`, fulfilled with `false`; Mechanism 2: `is_in_cfg_test` true inside the module → lint skipped → expectation unmet | PASS — agree |
| P4 | B-keepgoing: zero `expect_used` inside any `#[cfg(test)]` module carrying `#[expect]` (they are unfulfilled precisely because suppressed) | `tests/helper.rs:39` and `src/lib.rs:32,33` present with `false`, absent with `true`, same targets compiled | PASS — agree |

## Validated / learned

- P1: **learning.** Believed (issue text + requester brief): `allow-expect-in-tests` makes `.expect()` legal in test code generally, so the picker helper's let-else could become `.expect()`. Observed: the config covers only (a) functions whose name matches a `#[rustc_test_marker]` test item in the same module — i.e. `#[test]` fns — and (b) items under a `cfg(test)` attribute. A helper fn in a `tests/*.rs` integration crate is neither, so `render_open_picker`'s `Option::expect` keeps firing with the config present. The approved "restore the `.expect()` in the helper, red then green" fence is unachievable by this mechanism; the config's real coverage is `#[test]` fns and `#[cfg(test)]` modules.
- P1b: **learning.** The issue's "asymmetric firing" is real but misattributed: `expect_used` skips any `Result` whose error type is never-like. `Terminal::new`/`draw` on `TestBackend` return `Result<_, Infallible>`, hence silent; an `io::Result` in the same helper position is linted. This is why `render_text`'s two `Result::expect` calls have passed CI since PR #75.
- P2: validated prior understanding — non-test discipline is unchanged by the config (cyril line 23 probe; oracle bin 7/12).
- P3: validated with a forced consequence — with the config, all 17 `#[expect(clippy::expect_used)]` attributes on `#[cfg(test)]` items become `unfulfilled_lint_expectations` errors under `-D warnings`; they must be deleted for any green gate. (`#![allow(clippy::expect_used)]` variants, ~50 sites, are unaffected: `allow` never complains when unused.)
- P4: validated prior understanding — the documented coverage (`#[test]` fns, `#[cfg(test)]` modules) holds on both instruments.
- Instrument learnings (recorded so the next probe does not repeat them): (1) cargo stops scheduling new units after the first failing one; an unscheduled test target looks exactly like a suppressed lint — probe with `--keep-going` and list compiled units. (2) clippy searches for `clippy.toml` upward from the crate manifest dir; a probe crate nested under the worktree inherits the root config (or a concurrent probe's) unless it shadows it with its own file. (3) A lint error in a lib's non-test code blocks every test target of that package; keep non-test probe sites in a bin.

## Related issues

- Consulted (read-only `rivets list --json`, keywords `expect_used|unwrap_used|clippy|allow-expect|unfulfilled|let-else`): `cyril-ulhs` (this issue); `cyril-gu8a` (closed — Windows clippy cfg-gate of a test import, unrelated mechanism); `cyril-ykkc` (open — local gate never compiles kas-gated code; adjacent in that gate coverage ≠ what compiled, same trap as instrument learning 1); `cyril-3ck6` (keyword hit only, unrelated). No prior art on `allow-expect-in-tests`.
- Filed: none — the tracker is read-only for this session (orchestrator constraint); the items below are reported in the hand-back for the orchestrator to file: (a) 17 `#[expect(clippy::expect_used)]` attributes become dead/erroring the moment the config lands (P3 list above); (b) the let-else-panic workaround appears in 10 test files (`crates/cyril-ui/tests/{modal_theme,picker_active_marker,picker_viewport,widget_theme_sources}.rs`, `crates/cyril/tests/{architecture_tests,powers_source_fence,memory_runtime,nd4h_source_fences}.rs`, `crates/cyril-core/tests/spawn_isolation.rs`, `crates/cyril-workbench/tests/reviewer.rs`) — those inside non-`#[test]` helpers are NOT helped by the config (P1); (c) `allow-unwrap-in-tests`, if ever approved, would likewise unfulfill the 8 `#[expect(clippy::unwrap_used)]` sites (`cyril-core/src/{commands/subagent.rs:202, kiro_agent_config.rs:288, commands/mod.rs:514, types/hook.rs:114, types/config.rs:139}`, `cyril-memory/src/{config.rs:318, store.rs:2082}`, `cyril-ui/src/stream_buffer.rs:69`; exact `grep -rn "expect(clippy::unwrap_used)" crates/` count — the hand-back's "9" was a miscount, as was its "16 files" for the 17 `expect_used` sites, which span 15 files).

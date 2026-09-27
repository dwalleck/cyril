# Review decisions: cyril-3br3 — round 1

Source: independent review of PR #135 (Opus, xhigh), relayed by the
coordinator 2026-09-27; verdict "approved with nits", and the moved env read
(read-before-host-gate) judged correct to keep. Reviewed revision: `c6988820`.
Workflow state: Local route (`route.md` present, `plan.md` N/A), so per
assessing-review-feedback §5 this round used the repository's normal
reproduce → fix → focused-verification path, not checkpointed-build.

Blocking rows first: none — every row below is `non-blocking` (F1 is a
warning-only defect on a job that does not run with `-D warnings`; the branch
was landable throughout).

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Bug: the top-level `use cyril_core::platform::path::{…, wsl_to_win}` in `crates/cyril-core/tests/win_wsl_wiring.rs:23` is ungated while its only use is inside the `#[cfg(unix)]` fence, so the Windows test build emits `warning: unused import: wsl_to_win` (CI run 36297591628, job 108559200217); CLAUDE.md's CI-triage rule requires gating helpers/imports only a gated test uses. Fix: drop it from the shared import and add a `#[cfg(unix)]`-gated import or call fully qualified. | PR #135 review (Opus, xhigh) via coordinator | Verified | (a) Observed fact: the named CI job log (`/tmp/cyril-3br3-ci-windows.log`, 2721 lines) lines 385–393 — `warning: unused import: `wsl_to_win`` at `crates\cyril-core\tests\win_wsl_wiring.rs:23:5`, then ``cyril-core` (test "win_wsl_wiring") generated 1 warning`. (b) Mechanism reproduced locally on `c6988820` (a Windows target check dies in `libsqlite3-sys` per CLAUDE.md): flipping the fence's `#[cfg(unix)]` to `#[cfg(windows)]` compiles it out on Linux exactly as Windows does; `env -u CARGO_TARGET_DIR cargo check -p cyril-core --test win_wsl_wiring` → `warning: unused import: `wsl_to_win`` at `win_wsl_wiring.rs:23:5` (`/tmp/cyril-3br3-f1-mutation-check.log`). The second warning in that log, `struct CaptureWriter is never constructed`, is an artifact of the surgical flip (the writer kept its `cfg(unix)` while its consumer was compiled out) — real Windows compiles both out, which is why CI reports exactly 1 warning. | Accept | `crates/cyril-core/tests/win_wsl_wiring.rs`: `wsl_to_win` removed from the shared `use cyril_core::platform::path::{…}` block (line 22–24); `use cyril_core::platform::path::wsl_to_win;` added inside `distro_non_unicode_warns_and_treats_as_unset_via_child_process` beside the sibling-idiom fn-local `use std::os::unix::ffi::OsStrExt;` (lines 152–156), with a two-line comment naming why. This is the reviewer's "gated import" alternative: the fn's `#[cfg(unix)]` gates its body, and fn-local placement mirrors how the sibling fence already scopes its unix-only import. Receipt — same mutation, fix applied: `cargo check -p cyril-core --test win_wsl_wiring` → 0 `unused import` warnings (`/tmp/cyril-3br3-f1-fixed-under-mutation.log`; only the flip artifact remains). Restored `#[cfg(unix)]` → wiring fences 5/5. | non-blocking — warning-only on a job without `-D warnings`; fixed because the repository rule (CLAUDE.md CI-triage: gate what only the gated test uses) determines the answer. |
| F2 | Bug: the `process_wsl_distro` doc comment (`path.rs:266–271`) and the PR body's Fix section say the warning "fires on every host", but production reaches `process_wsl_distro` only through `win_to_wsl`/`wsl_to_win`, which `to_native`/`to_agent` call behind `translation_active(cfg!(windows), ..)`; off Windows it is observable only through the public entry points / tests. Fix: reword to "observable on every host through the public `wsl_to_win`/`win_to_wsl` entries; production reaches it only on Windows". | PR #135 review (Opus, xhigh) via coordinator | Verified | Caller census on `c6988820`: `grep -rn "wsl_to_win\|win_to_wsl\|translate_paths_in_json\|process_wsl_distro" crates/*/src` → no hit outside `crates/cyril-core/src/platform/path.rs`. In-file non-test call sites: `wsl_to_win` only from `to_native` (line 174) and `win_to_wsl` only from `to_agent` (line 186), both under `translation_active(cfg!(target_os = "windows"), agent_location())` (line 148 returns `false` when the host is not Windows); `translate_paths_in_json` (line 455) calls `process_wsl_distro()` directly but has no production caller (its own doc says so; the census confirms). So production never reaches the read off Windows; the reviewer's claim holds and the old wording overstated it. | Accept | `crates/cyril-core/src/platform/path.rs` `process_wsl_distro` doc comment (lines 266–274) reworded: warns and is treated as unset; "Read-before-gate makes the warning observable on every host through the public [`wsl_to_win`] / [`win_to_wsl`] entries (the Linux fence's path); production reaches this function only on Windows, behind the agent-location gate in [`to_native`] / [`to_agent`]." PR #135 body Fix section reworded to match (`gh pr edit`). | non-blocking — documentation accuracy; no code path changed. |
| F3 | Note, no fix proposed: on Windows, the composite behavior "corrupt `CYRIL_WSL_DISTRO` → warned, treated as unset → cwd donation still applies" (route.md T4 contract (3)) has no dedicated fence. | PR #135 review (Opus, xhigh) via coordinator | Verified | Code reading on `c6988820`: the `windows` module of `tests/win_wsl_wiring.rs` has three fences (`env_distro_wiring_via_child_process` with the valid value `Ubuntu`, `native_agent_no_rewrite_via_child_process`, `wsl_location_without_distro_via_child_process`) and none injects a non-Unicode value (`grep -n "from_wide\|non_unicode\|NotUnicode"` → only the unix fence). The composite is covered piecewise: the unix fence proves corrupt → warning + env treated as `None`; `platform::path::tests` at `path.rs:1030–1043` prove `resolve_wsl_distro(None, Some(\\wsl$\<d>\…))` donates `<d>`. | Reject | N/A — no change, per the coordinator's explicit acceptance. | non-blocking — permanent non-goal for this change, not deferred work: a Windows fence would need unpaired UTF-16 surrogates via `OsStringExt::from_wide` in a `cfg(windows)` test that is CI-verified only (CLAUDE.md: a Windows target check dies locally in `libsqlite3-sys`), and the two existing proofs already cover both halves of the composition. The coordinator accepted this without a tracker issue; this agent is read-only on the tracker, so if the orchestrator instead classifies it as intended future work, a rivets ID is theirs to file. |

## Review errors

None found. Every anchor checked out: `win_wsl_wiring.rs:23` (import), the
fence's use near line 162 (exact: 162 on `c6988820`), `path.rs:266–271` (the
doc lines), and CI run 36297591628 / job 108559200217 (the warning is at log
lines 385–393).

## Repair re-review

N/A — the round changed no behavior: F1 moves an import inside the function
that is its only consumer (test-binary compilation on Windows loses one
warning; the fence's assertions and the production path are unchanged), F2 is
doc wording, F3 applies nothing.

## Receipts (round 1, working tree = `c6988820` + F1 + F2, committed as the
round-1 commit on `fix/cyril-3br3-wsl-distro-non-unicode`)

Coordinator-specified gate, each with `env -u CARGO_TARGET_DIR`, own log at
`/tmp/cyril-3br3-review-<name>.log`, real exit code echoed as `GATE-OK-*`:

- `cargo nextest run -p cyril-core --test win_wsl_wiring` → PASS (5 run, 5 passed, 0 skipped)
- `cargo fmt --all -- --check` → PASS
- `cargo clippy -p cyril-core --all-targets --all-features -- -D warnings` → PASS

Evidence disposition: the round-0 full CI-mirror gate on `08b0c6bc`
(`route.md` Terminal criterion) is retained for the production behavior — the
production change since then is a doc comment, and the test change is import
placement that the fence run above re-exercises; the fence's own
red/green/mutation proof (`red-receipt.md`) is retained unchanged because the
fence's assertions and the production `match` are byte-identical to `08b0c6bc`.

Commit grouping: F1 and F2 land in one commit listing both IDs. They are
separate non-behavioral atomic changes in different files, but the only state
verified above is the combined one, and an F1-only or F2-only working tree
could not be reconstructed without `git stash` (forbidden in this shared-stash
worktree environment); one commit of the checked state is preferred over two
commits with one unchecked intermediate state.

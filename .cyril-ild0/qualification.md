# cyril-ild0 — qualification of native `/review`

Status: **partially qualified.** Linux was exercised live through the real TUI and the
KAS workflow; Windows has not been run live. Each row below says whether its
evidence is **live** (the real binary driving a real KAS session) or **fixture** (tests
on the App harness, the bridge harness or real git and process fixtures).

## Hosts

| Host | OS | Shells | kiro-cli / KAS | Live `/review` |
|---|---|---|---|---|
| Linux workstation | CachyOS, Linux 7.2.8 x86_64 | fish (host shell; Posix-family crtool prefix) | 2.26.0 (KAS bundle `2.26.0-6857cdb6…`) | **yes** (runs below), until the Kiro token expired |
| Windows VM (resourcefs) | Windows, PowerShell 5.1.26100.9444 | Windows PowerShell 5.1 only — **pwsh not installed** | 2.26.0 (`kiro-cli-chat 2.26.0`); **node not on PATH**, auth state unknown | **no** |

## Live runs (Linux, 2026-10-02, `kiro-cli` 2.26.0)

The driver is `smoke-review.py` in this directory: the real `cyril` binary on a pty,
`HOME` isolated so the review assets install into a throwaway `~/.kiro`, the real
credential store reused. The fixture repository is one Rust file with planted bugs
(an emptied guard, an underflow, three `unwrap`s), edited but not committed.

| Run (local time) | cyril (branch build; merged as) | Observed |
|---|---|---|
| 1 (23:49) | cyril-iowg pre-review build (PR #154, main `c5fb98a4`) | Form shown (target auto, 1 file, "no check configured", the permission list). Enter installed the assets, created `.code-review/<stamp>-<hex>/` and gathered. **`workflow/new` failed: "custom agent 'cyril-review-clerk' … is not registered"** — KAS registers `~/.kiro/agents` through a file watcher, so agents installed mid-session are not usable at once. Fixed in PR #154 (wait 2 s after an install that wrote agents, retry with doubling waits while KAS reports the agent unregistered). |
| 2 (23:56) | PR #154 with the agent-load fix | Complete path: form → install → gather → Minted → `run.json` → Invoke → the full workflow → `findings.json`/`report.md`, **14 CONFIRMED findings** covering every planted bug, about 24 minutes. The policy **denied three step-session reads outside the workspace** (`~/.claude/CLAUDE.md`, `~/.kiro/steering/rust.md`) and logged them to `denied.log`; the run **finished with nobody at the keyboard**: a step whose permission request had gone to an ordinary prompt would have waited for an answer, so every step request was decided by the policy. (The driver then in use could not detect a prompt on screen; the committed driver detects the " Permission Required " title.) The parent session got KAS's completion follow-up turn. Found: cyril dropped the recipe's `run_start` as malformed (`onMaxIterations: "continue"` unknown) — fixed in PR #154. |
| 3 (00:29) | PR #154 with both fixes | `run_start` parsed. The authorization decided **5 allowed / 2 denied** step requests with no prompt. The Kiro token expired mid-run; the run failed and cyril reported one line, `review failed — <run dir>`, and disarmed the run (`review: run ended … status: Failed` in `cyril.log`). |
| 4 (01:17) | cyril-305w build (PR #155, main `3d664c3f`) | Not started: the Kiro token had expired before the session was created. |

`run.json` from run 2 recorded `crtool_prefix: "\"…/target/debug/cyril\" crtool"`:
every crtool step ran the native binary. No Python or WSL is in the execution path.

## Acceptance criteria

| Criterion | Evidence | Status |
|---|---|---|
| Actual TUI and native KAS workflow on Linux; no Python or WSL in production execution | Live runs 2–3 | **met (Linux)** |
| …on Windows with Windows PowerShell 5.1 and pwsh | — | **unmet**: the VM has no pwsh, and kiro-cli's KAS (node) and auth are not set up for a live run |
| Informed launch | Live runs 1–3; App tests | met |
| Target/scope input | App tests on real git history (`each_target_counts_gathers_and_sends_its_own_diff`, scope checkbox tests) | fixture only |
| No-check behavior | Live runs 1–3 ("no check configured", nothing ran) | met |
| Configured check | App tests with real processes (clean, red, timed out, cannot start, cancelled) | fixture only |
| Permission allows and denials, no prompts | Live run 2 (completed unattended) and run 3 (5 allowed / 2 denied in `cyril.log`); App tests; shared policy vectors | met |
| Unrelated main-session approvals and chat stay usable | App tests (`route_review_permission` returns main-session and foreign requests) | fixture only |
| Completion report | App tests (`summary`, missing findings) ; live run 2 completed but its summary line was not captured | fixture only |
| Check cancellation | App tests killing a real sleeping process (`esc_during_the_check…`, `cancel_during_the_check…`) | fixture only |
| Workflow cancellation | App tests (every phase, PR 158) | fixture only |
| Paused continuation; restart recovery without re-gathering or re-checking | App tests (PR 157: manifest bytes unchanged, Load → Retry/Resume) | fixture only |
| Executable paths with spaces quoted for the resolved shell; forbidden characters refused | `review::tests` (spaces, Unicode, every hazard, verbatim paths); `generated_posix_prefix_runs_as_a_native_shell_command` runs a prefix with a space under `/bin/sh`; CI `review_prefix` example qualifies `powershell` and `pwsh` on Windows | met |
| CI: crtool goldens on Linux and Windows; shared policy vectors; recipe generator parity | CI "Code Review Tooling" (ubuntu, windows, macOS): `cargo test -p cyril-review`, `crtool_cli`, `selftest_crtool.py`; "Generated review assets are in sync" | met |
| Same-run ownership established before the first step permission; foreign ownership never inherits | Live run 2 (completed unattended, so no step request reached an ordinary prompt) and run 3 (policy decisions logged); App tests (foreign and unclaimed sessions get ordinary approval; late requests denied) | met (Linux) |
| Rejection behavior; no persisted trust | App tests (only AllowOnce/RejectOnce, Cancel when neither is offered; never AllowAlways or trust) | fixture only — the live homes were reset before the agent files were compared |
| Foreign-version refusal; missing/corrupt findings and run records never reported as clean | App and unit tests (PR 157; `missing_findings_are_an_error_not_zero_findings`) | fixture only |

## To finish

1. Linux: `kiro-cli login`, then rerun `smoke-review.py` for: a configured check
   (`.cyril/config.toml` `check_cmd`), `--command "/review base main -- src"`, a
   completed run (`--until end`, capture `summary.txt`), `/review cancel` during the
   check and during the run, and a restart followed by `/review resume`. Compare the
   isolated home's `~/.kiro/agents/*.md` with `crates/cyril-review/assets/agents/`
   afterwards (no persisted trust).
2. Windows: install pwsh 7; set up and authenticate kiro-cli's KAS for the test user;
   build `cyril` natively; run the same checks under Windows PowerShell 5.1 and pwsh.

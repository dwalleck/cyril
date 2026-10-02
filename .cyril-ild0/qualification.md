# cyril-ild0 — qualification of native `/review`

Status: **Linux live-qualified; Windows moved to cyril-22y6.** Linux was exercised live
through the real TUI and the KAS workflow (runs 1–12). Live runs cover every Linux row's
main path; what stays fixture-only is what a healthy live run cannot provoke — rejecting
a permission, refusing a foreign-version or corrupt run record, and a check that fails,
times out or cannot start (each row says which). Windows has not been run live; the operator split
it into cyril-22y6 on 2026-10-02. Each row below says whether its
evidence is **live** (the real binary driving a real KAS session) or **fixture** (tests
on the App harness, the bridge harness or real git and process fixtures).

## Hosts

| Host | OS | Shells | kiro-cli / KAS | Live `/review` |
|---|---|---|---|---|
| Linux workstation | CachyOS, Linux 7.2.8 x86_64 | fish (host shell; Posix-family crtool prefix) | 2.26.0 (KAS bundle `2.26.0-6857cdb6…`) | **yes** (runs 1–12) |
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
| 5 (06:54, 06:55) | main `a48acb34` | **Esc during a configured check.** A copy of the fixture with `check_cmd = "sleep 120"`: form → gather → `review: running check…`; Esc → `review: abandoned — anything already written stays under .code-review/`. A snapshot of cyril's descendants taken after Esc and before the driver's cleanup shows only cyril and its KAS node. On Unix the check is exec'd without a shell, so `sleep` was the check's only process and the kill reached it; a multi-process check would orphan its workers (cyril-kb1j). No workflow was minted. (Run twice: the first time had no process snapshot.) |
| 6 (06:56) | main `a48acb34` | **Prefilled target and scope, configured check, `/review cancel` during the run.** `--command "/review uncommitted -- src"`: the form showed `Target uncommitted changes (HEAD)`, `Scope [x] src (1) — 1 file`, `Check will run: git diff --stat HEAD (timeout 60s)`. The check ran clean; the run started; 20 s later `/review cancel` → `review: wf_… cancelled`, and no step process descended from cyril afterwards (a descendants-only snapshot: a reparented orphan would not have shown). **Found:** KAS's auto-wake (cyril-lki9) then opened the labelled `workflow "cyril-review" aborted · agent follow-up` turn on the main session, and the model **took up the cancelled review itself** (listed the run directory, spawned a `cyril-review-finder` sub-agent) — billed work after an explicit stop, outside the run's authorization. Filed as **cyril-oswq** (needs a human decision; cyril-lki9 chose to keep the wake). |
| 7 (06:58) | main `a48acb34` | **Restart, part 1.** Default `/review` with the configured check (`review: check clean — recorded for the reviewers`); 60 s after the run started, **cyril was quit (Ctrl+Q)**: `cyril.log` shows `review: disarmed … why: "cyril is exiting"`, and cyril shut KAS down itself. (The driver then in use meant this action as a SIGKILL, but its cleanup sent Ctrl+Q first; the committed driver kills every process in cyril's session.) The run directory's gather and check artifacts were fingerprinted (SHA-256 of `manifest.json`, `diff.patch`, `changed-files.txt`, `run.json`, `facts/*`, `patches/*`). |
| 8 (07:00) | main `a48acb34` | **Restart, part 2: a new cyril process, `/review resume`.** KAS reported the run, interrupted by the quit, as **paused**; the form showed `Resume 20261002-115906-65e7 (paused)`, `Target auto`, `Scope src — 1 file`, `Check not rerun — the run keeps the results it recorded`. Enter continued it to the end in 27.8 minutes: `review complete: 15 finding(s) — 15 CONFIRMED` with the report path, covering every planted bug. Every fingerprinted artifact was **byte-identical** afterwards (nothing re-gathered or re-checked). No approval prompt appeared at any point of the run. |
| 9 (07:29) | main `a48acb34` | Not started: the Kiro access token expired about an hour after the login (`KAS auth not servable … kiro token expired`). cyril serves the stored token to KAS and never renews it, so a live session lasts until the token's expiry; this run was to have shown a main-session approval during a review. |
| 10 (09:40) | main `a48acb34` | **Restart after a crash, part 1.** Fresh login. Default `/review` with the configured check (clean); 64 s after the run started the driver sent SIGKILL to every process in cyril's session. `cyril.log` stops mid-step (a `_kiro/fs/write_file` for a step session) with no `disarmed` or exit line. A system-wide `ps` after the driver exited found no KAS node process and nothing from the run's home (the driver then in use took no process list right after the kill; the committed one writes `processes.txt` there). The run directory's gather and check artifacts were fingerprinted as in run 7. |
| 11 (09:41) | main `a48acb34` | **Restart after a crash, part 2.** A new cyril process, `/review resume`: KAS reported the crashed run as **paused**, the same as after a quit; the form showed `Resume 20261002-144034-2b48 (paused)` and `Check not rerun`. Enter → `review: wf_7c85219f767502ae continues — …`, then `review complete: 14 finding(s) — 14 CONFIRMED` after 19.6 minutes. Four of the five planted bugs are findings of their own (the emptied guard, the underflow, both `parse` unwraps); the `split_once('.').unwrap()` panic appears only inside another finding's write-up in `report.md`, where run 8 had listed it separately — the reviewers' output varies run to run; no step's results were lost (`verified.json` cites it). Every fingerprinted artifact was byte-identical; no approval prompt appeared. |
| 12 (10:02, 10:03) | main `a48acb34` | **Main-session approval during a run.** 20 s after the review started, the main agent was asked to run `echo cyril-ild0-chat`. An ordinary ` Permission Required ` prompt for the **main session** (no session suffix in the title) appeared over the running review, offering KAS's `Allow` / `Deny`; `Allow` (KAS's allow-once option; no "always" option was offered) approved that one request and the chat answered `cyril-ild0-chat`, the review still running (read from `chat.txt`; the committed driver now checks the answer, the closed prompt and the running review itself). (The first attempt at 10:02 found the prompt but the driver misread its options: a wide glyph left of the popup shifted that row; fixed.) Afterwards the home still held only the byte-identical assets and no trust-bearing file. |

After runs 5–8 (and again after runs 10–12) the isolated homes held only the installed assets outside KAS's session
transcripts: `~/.kiro/agents/cyril-review-{clerk,commenter,finder,verifier}.md` and
`~/.kiro/workflows/cyril-review.workflow.json`, each **byte-identical** to
`crates/cyril-review/assets/`, and no file outside KAS's session transcripts and logs naming trust (searched for
`allowAlways`, `allow_always`, `trustedTools`, `allowedCommands`, `alwaysAllow`).

Three KAS node processes from earlier spawns (23:47, 23:54, 06:52) were found still
running afterwards, each with its cyril gone: a cyril that dies without running its
cleanup leaves KAS behind (cyril-jlw9; the harness side is cyril-fklu). They were killed.

`run.json` from run 2 recorded `crtool_prefix: "\"…/target/debug/cyril\" crtool"`:
every crtool step ran the native binary. No Python or WSL is in the execution path.

## Acceptance criteria

| Criterion | Evidence | Status |
|---|---|---|
| Actual TUI and native KAS workflow on Linux; no Python or WSL in production execution | Live runs 2–3 | **met (Linux)** |
| …on Windows with Windows PowerShell 5.1 and pwsh | — | **moved to cyril-22y6**: the VM has no pwsh, and kiro-cli's KAS (node) and auth are not set up for a live run |
| Informed launch | Live runs 1–3, 6, 8 (resume form); App tests | met |
| Target/scope input | Live run 6 (`/review uncommitted -- src` prefilled target and checked scope); App tests on real git history (`each_target_counts_gathers_and_sends_its_own_diff`, scope checkbox tests) | met |
| No-check behavior | Live runs 1–3 ("no check configured", nothing ran) | met |
| Configured check | Live runs 6–7 and 10 (clean, recorded), runs 8 and 11 (not rerun on resume); App tests with real processes (clean, red, timed out, cannot start, cancelled) | met (clean path live; the rest fixture) |
| Permission allows and denials, no prompts | Live runs 2, 8 and 11 (completed unattended, no prompt on screen), run 3 (5 allowed / 2 denied in `cyril.log`); App tests; shared policy vectors | met |
| Unrelated main-session approvals and chat stay usable | Live run 12 (a main-session prompt over a running review, approved once, chat answered); App tests (`route_review_permission` returns main-session and foreign requests) | met |
| Completion report | Live run 8 (`review complete: 15 finding(s) — 15 CONFIRMED`, report path); App tests (`summary`, missing findings) | met |
| Check cancellation | Live run 5 (Esc killed a real `sleep 120` check; no workflow minted); App tests (`esc_during_the_check…`, `cancel_during_the_check…`) | met |
| Workflow cancellation | Live run 6 (`/review cancel` mid-run; no step process left); App tests (every phase, PR 158) | met — but the KAS abort wake then lets the main agent redo the review (cyril-oswq) |
| Paused continuation; restart recovery without re-gathering or re-checking | Live runs 7–8 (quit mid-run) and 10–11 (SIGKILL mid-run): restart, `/review resume` of the paused run to completion, gather and check artifacts byte-identical; App tests (PR 157) | met |
| Executable paths with spaces quoted for the resolved shell; forbidden characters refused | `review::tests` (spaces, Unicode, every hazard, verbatim paths); `generated_posix_prefix_runs_as_a_native_shell_command` runs a prefix with a space under `/bin/sh`; CI `review_prefix` example qualifies `powershell` and `pwsh` on Windows | met |
| CI: crtool goldens on Linux and Windows; shared policy vectors; recipe generator parity | CI "Code Review Tooling" (ubuntu, windows, macOS): `cargo test -p cyril-review`, `crtool_cli`, `selftest_crtool.py`; "Generated review assets are in sync" | met |
| Same-run ownership established before the first step permission; foreign ownership never inherits | Live run 2 (completed unattended, so no step request reached an ordinary prompt) and run 3 (policy decisions logged); App tests (foreign and unclaimed sessions get ordinary approval; late requests denied) | met (Linux) |
| Rejection behavior; no persisted trust | No persisted trust: live runs 5–8 and 10–12 — including run 12's granted main-session approval — (installed assets byte-identical to the shipped ones; the key-name search above found nothing). Rejection: App tests only (only AllowOnce/RejectOnce, Cancel when neither is offered; never AllowAlways or trust) | no persisted trust met; rejection fixture only |
| Foreign-version refusal; missing/corrupt findings and run records never reported as clean | App and unit tests (PR 157; `missing_findings_are_an_error_not_zero_findings`) | fixture only |

## Follow-ups

- **cyril-22y6** — Windows live qualification (Windows PowerShell 5.1 and pwsh).
- **cyril-oswq** — what a `/review cancel` should do about KAS's abort wake (run 6).
- **cyril-kb1j** — a cancelled or timed-out check leaves a multi-process check's workers running.

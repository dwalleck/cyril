# Review decisions: cyril-861q (PR #139, round 1)

Reviewer: independent review of PR #139 (Opus, xhigh), relayed by the sweep
orchestrator with the requester-delegated approval for F3's rule change.
Reviewed revision: e7f29438. Pre-repair baseline for every red proof below:
the e7f29438 production tree with only the round's fences added (run
`cyril-861q-review-red.log`). Post-repair: the repair diff on top of it.

## Decision log

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | `version_probe_command`'s launcher scan does not skip wsl.exe's positional `~`, so `wsl ~ kiro-cli acp` probes `wsl ~ --version` — WSL's own version, the original cyril-861q failure. Fix: treat `~` as a bare launcher token in the launcher-option scan; fence `["wsl","~","kiro-cli","acp"]`. | PR #139 review (Opus) | Verified | Red run at the pre-repair baseline: `version_probe_skips_the_launcher_home_shorthand` FAIL with `left: ("wsl", ["~", "--version"])`; Microsoft Learn "Basic commands for WSL" documents `wsl ~`. | Accept | R1 — `if arg == "~" { continue; }` in the launcher scan of `version_probe_command` (`crates/cyril-core/src/protocol/kas/version.rs`); fence `version_probe_skips_the_launcher_home_shorthand`; process-seam fence `wrapper_probe_passes_the_launcher_home_shorthand_through` (round 1b). | blocking — a documented launcher form reproduced the very defect this PR fixes. |
| F2 | `--distribution-id <GUID>` (wsl.exe 2.4.4+) is missing from the value-taking option table, so the GUID becomes the "command" and the probe is `wsl --distribution-id <guid> --version`. Fix: add it; fence with a GUID value. | PR #139 review (Opus) | Verified | Primary sources: microsoft/WSL release 2.4.4 note "Implement wsl.exe --distribution-id"; `localization/strings/en-US/Resources.resw` usage text `--distribution-id <DistroGuid>  Run the specified distribution ID.` (value-taking). Red run: `version_probe_keeps_a_distribution_id_value_before_kiro_cli` FAIL with `left: ["--distribution-id", "{…}", "--version"]`. | Accept | R1 — `WSL_VALUE_OPTIONS` gains `"--distribution-id"` (now 7 entries) with the source cited in its doc comment; fence `version_probe_keeps_a_distribution_id_value_before_kiro_cli`. | blocking — same defect surface (a documented value option mis-scanned); the failure is loud (wsl runs a command named `--version`) but wrong for a supported launcher form. |
| F3 | The rule probes the FIRST non-flag element without checking it is kiro-cli: `wsl -e env FOO=1 kiro-cli acp` probes `env --version`, `wsl bash -lc "kiro-cli acp"` probes `bash --version`, and a foreign version string can silently select a flag. Fix (requester-delegated approval, orchestrator): for launcher-routed commands only, require the located element's basename to be `kiro-cli`/`kiro-cli.exe` (any casing, bare or full path); otherwise the existing named-command `Err`, no spawn. Non-launcher commands keep today's behavior. | PR #139 review (Opus); rule change approved by the orchestrator as the requester's delegate | Verified | Red run: `version_probe_refuses_a_launcher_whose_command_is_not_kiro_cli` FAIL at the `.unwrap_err()` (the pre-repair code returned `Ok(wsl -e env --version)`). Silent-flag path confirmed by reading `run_version_probe` + `parse_semver`: `bash --version` → `5.2.21(1)-release` → `(5,2,21)` → `v3` with no error (see Review errors for the coreutils example). Controls green on both sides: `version_probe_accepts_kiro_cli_by_basename_through_a_launcher`, `version_probe_of_a_non_launcher_program_is_not_subject_to_the_kiro_cli_check`. | Accept | R1 — `let command = loop { … break arg; }` then `if !is_kiro_cli(command) { return Err(…"the command after the launcher's options is `{command}`, not kiro-cli") }`; `is_kiro_cli` applies the shared basename rule (`platform::path::basename_is`, round 1b); fence `version_probe_refuses_a_launcher_whose_command_is_not_kiro_cli` + the two controls. Approved-rule change recorded in `route.md` (Amendment, review round 1). | blocking — a launcher-routed command can select `--agent-engine` from a foreign binary's version with no error (CLAUDE.md silent-failure rule). |
| F4 | `build_wrapper_command`'s required-version mismatch says "found {version} at {program}" — "at wsl" for launcher-routed commands. Fix: render the probe argv so the message names the binary actually probed; adjust any exact-text fence. | PR #139 review (Opus) | Verified | Red run: `wrapper_version_mismatch_names_the_probed_command` FAIL with `left: "… found 2.21.1 at <tmp>/wsl; …"`. No pre-existing exact-text fence for this message (grep of `crates/` for "install the required version" / "this launch requires" at e7f29438: only the production site). | Accept | R2 — `build_wrapper_command` computes `probe` via `version_probe_command` and runs `run_version_probe` (the executor split out of `kiro_cli_version`, whose public behavior is unchanged: it now composes the two); message reads "found {version} via `{render(&probe)}`". Fence `wrapper_version_mismatch_names_the_probed_command` (exact text). | non-blocking — diagnostic wording only; fixed in this round because the launcher-routed message named the wrong binary. |
| N1 | (note, no change requested) A wrapper script run under `CYRIL_AGENT_LOCATION=wsl` is not the WSL launcher, so it still probes `<wrapper> --version`. | PR #139 review (Opus) | Verified | Code reading of `version_probe_command`'s non-launcher branch (program is not `is_wsl_launcher` → `<program> --version`, args dropped) and the control `version_probe_of_a_non_launcher_program_is_not_subject_to_the_kiro_cli_check`; `CYRIL_AGENT_LOCATION` is read only by `platform::path::bind_agent_location` (path translation), never by spawn resolution. | Reject | N/A — permanent non-goal: a wrapper script is the user's own launcher with an unknowable argv grammar; a pass-through wrapper (`wsl -d X kiro-cli "$@"`) forwards `--version` to kiro-cli correctly, and a non-pass-through one cannot be probed generically. `CYRIL_AGENT_LOCATION` governs path translation, not spawn resolution, by design (cyril-jxmv). | non-blocking — out of scope by construction; rationale recorded, no tracker issue. |

## Review errors

- F3's coreutils example ("`env --version` … coreutils '9.x' silently picks the v3 flag") is not silent on real coreutils: `env (GNU coreutils) 9.4` yields the two-component `9.4`, which `parse_semver` rejects as "malformed kiro-cli version" — a loud (if confusing) refusal. The silent path the finding describes is real for the finding's other example: `bash --version` prints `GNU bash, version 5.2.21(1)-release`, parsed as `(5,2,21)` → `v3` with no error. The bug claim stands; the example that demonstrates its silent form is bash, not env.
- F2 dates the option to "wsl.exe 2.4+"; the release note is on 2.4.4 specifically. No effect on the fix.

## Compact repair records

### R1 — launcher scan: `~`, `--distribution-id`, kiro-cli requirement (F1, F2, F3)

- **Finding IDs and ownership:** F1, F2, F3. No `design.md`/`plan.md` exist (Local route); the covering contract is `route.md` T4 (behavior), amended this round for F3 with the orchestrator's requester-delegated approval. F1 and F2 are technical corrections within the approved rule ("skip the launcher and its `-`-prefixed flags plus their values"): they complete the launcher grammar against its primary source.
- **Root-cause change and affected paths:** `crates/cyril-core/src/protocol/kas/version.rs` — `WSL_VALUE_OPTIONS` (+`--distribution-id`), `version_probe_command` (skip `~`; capture the located element; refuse unless `is_kiro_cli`), new `is_kiro_cli`. Round 1b: `is_kiro_cli` delegates to the new `crates/cyril-core/src/platform/path.rs::basename_is` (R3). Test-side: the process-seam fixture was extracted into `fake_wsl_launcher` (shared by the reproduction and the F4/N1 fences), gained `--distribution-id` in its value-option list and a `'~'` launcher-token branch (quoted — see R3/N1).
- **Commands and results:** `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas --no-fail-fast -- kas::version` — pre-repair baseline: 18 run, 14 passed, 4 failed (F1–F4 fences red, `cyril-861q-review-red.log`); post-repair: 18 run, 18 passed (`cyril-861q-review-green.log`); after round 1b: 19 run, 19 passed (`cyril-861q-rereview-green.log`). Expected: exactly the four new fences flip red→green; every prior fence and both controls stay green. Observed: as expected.
- **Fence mutation receipts:** `version_probe_skips_the_launcher_home_shorthand` — red at baseline (`["~", "--version"]`), green post-repair. `version_probe_keeps_a_distribution_id_value_before_kiro_cli` — red at baseline (GUID taken as the command), green post-repair. `version_probe_refuses_a_launcher_whose_command_is_not_kiro_cli` — red at baseline (`Ok` where `Err` expected), green post-repair. Controls `version_probe_accepts_kiro_cli_by_basename_through_a_launcher` and `version_probe_of_a_non_launcher_program_is_not_subject_to_the_kiro_cli_check` green on both sides (the tightened predicate introduces no false reject for paths, `.exe`/casing variants, or non-launcher programs). The eight pre-existing cyril-861q fences remain green on both sides (retained). Round 1b adds `wrapper_probe_passes_the_launcher_home_shorthand_through` — see R3.
- **Evidence disposition:** the e852b199 gate results recorded in `route.md` are invalidated for the kas-feature legs (changed production path) and re-run below; after round 1b (path.rs changed) the default-features `cyril-core` test leg is re-run as well; the doc-test leg is retained (no doc tests in the changed files; no doc comment carries a code block).

### R2 — mismatch message names the probed command (F4)

- **Finding IDs and ownership:** F4. Technical correction of a diagnostic under the existing approved behavior ("Err naming the command"); no rule change.
- **Root-cause change and affected paths:** `crates/cyril-core/src/protocol/kas/version.rs` — `kiro_cli_version` now composes `version_probe_command` + the new private `run_version_probe` (body moved verbatim; its lifecycle doc comment moved with it; `rendered = render(probe)`); `build_wrapper_command` computes the probe itself, runs it, and formats "found {version} via `{probe}`".
- **Commands and results:** same focused run as R1 — `wrapper_version_mismatch_names_the_probed_command` red at baseline ("at <tmp>/wsl"), green post-repair. The message has one production site; `kiro_cli_version`'s only remaining caller (`discovery.rs::installed_cli_version`) sees identical behavior (composition) — covered by the full kas nextest leg below.
- **Fence mutation receipts:** `wrapper_version_mismatch_names_the_probed_command` — red at baseline, green post-repair (exact-text assertion).
- **Evidence disposition:** as R1.

## Repair re-review (round 1 diff)

- **Who / isolation:** a fresh-context `rust-code-reviewer` agent given only the repair diff (`cyril-861q-review-repair.diff`, 312 lines), this log, the post-repair `version.rs`, `is_wsl_launcher` in `path.rs`, and CLAUDE.md's standards sections — not the original reviewer's findings. It ran no cargo commands.
- **Scope:** launcher-scan correctness (ordering of `~` vs value options, dangling value option, `--`), no `Ok` without `is_kiro_cli`, non-launcher passthrough, standards in non-test code, fence discrimination, diff-vs-log reconciliation, Windows dead-code.
- **Result:** APPROVE WITH NITS. Scan correct, no blocking findings; every fence judged discriminating (table in the report); non-test code free of unwrap/expect/`let _`/`#[allow]`/sentinels; no dead helpers on Windows. Nits became round 1b below.

## Round 1b — re-review findings

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| N1r | The fake launcher's `case` pattern `-*|~)` never matches a literal `~`: an unquoted `~` pattern is tilde-expanded to `$HOME`, so the fixture does not model `wsl ~`; it also lacks `--distribution-id`. Fix: quote the pattern (`'~'`), add `--distribution-id`, and add a process-seam case for `wsl ~ kiro-cli acp`. | repair re-review (rust-code-reviewer) | Verified | Direct check: `sh -c 'case "~" in -*|~) …'` → `nomatch`; with `'~'` → `match` (`/bin/sh` → bash here; POSIX 2.6.1 tilde expansion applies to `case` patterns). Red run: the new `wrapper_probe_passes_the_launcher_home_shorthand_through` FAIL against the unquoted fixture — the fake launcher rejected `~ -d Ubuntu kiro-cli --version` with exit 3 (`cyril-861q-rereview-red.log`); its journal assertion passed, so the production `~` skip already sent the right argv and only the fixture was wrong. | Accept | R3 — fixture pattern `-*|'~')`, `--distribution-id` added to its value-option list, new process-seam fence `wrapper_probe_passes_the_launcher_home_shorthand_through` (green after the fix, `cyril-861q-rereview-green.log`). | non-blocking — test-fixture fidelity; no production behavior involved. |
| N2r | The doc comment on `version_probe_command` and the F3 test comment repeat the coreutils example this log's Review errors section calls wrong (coreutils fails loudly; bash is the silent case). Fix: use the bash example. | repair re-review (rust-code-reviewer) | Verified | Reading the two comments against the Review errors entry above. | Accept | R3 — doc comment now cites `wsl bash -lc "kiro-cli acp"` → `bash --version` → `5.2.21` → `v3` silently; the F3 test comment states both outcomes (bash silent, coreutils loud). | non-blocking — comments only. |
| N3r | `is_kiro_cli` duplicates `is_wsl_launcher`'s basename body; a shared `basename_is(element, name)` in `platform::path` keeps one definition, as the path.rs comment argues. | repair re-review (rust-code-reviewer) | Not-applicable | N/A — design preference, no factual claim beyond the (true) duplication. | Accept | R3 — `pub(crate) fn basename_is(program, name)` in `platform/path.rs` (body = the former `is_wsl_launcher` body, byte-identical logic); `is_wsl_launcher` → `basename_is(program, "wsl")`; `is_kiro_cli` → `basename_is(element, "kiro-cli")`. | non-blocking — behavior-preserving extraction; coverage retained by `agent_location_heuristic_table` (path.rs, 14 launcher/non-launcher cases through `resolve_agent_location`) and this round's launcher fences and controls (19/19 green). |

### R3 — round 1b repairs (N1r, N2r, N3r)

- **Finding IDs and ownership:** N1r, N2r, N3r. Technical corrections under the existing approved behavior; no rule change.
- **Root-cause change and affected paths:** `crates/cyril-core/src/platform/path.rs` (new `basename_is`; `is_wsl_launcher` delegates), `crates/cyril-core/src/protocol/kas/version.rs` (`is_kiro_cli` delegates; two comments; test fixture pattern + value list; new process-seam fence).
- **Commands and results:** focused run before the fixture fix: 19 run, 18 passed, 1 failed (only the new fence, as predicted); after: 19 run, 19 passed. Gate table below.
- **Fence mutation receipts:** `wrapper_probe_passes_the_launcher_home_shorthand_through` — red against the unquoted fixture (exit 3 from the fake launcher), green with `'~'` quoted; baseline on each side = the post-round-1 tree ± the one-token fixture change. No other fence added or changed; `basename_is` is fenced by the retained `agent_location_heuristic_table` and the launcher/kiro-cli fences, all green after the extraction.
- **Evidence disposition:** kas legs re-run (R3 gate table); default-features `cyril-core` nextest and workspace clippy re-run because `path.rs` compiles in every feature set.
- **Repair re-review of round 1b:** N/A — round 1b changed no behavior: a test-fixture fix, two comments, and a behavior-preserving extraction whose predicate body is byte-identical, with retained fences green on both sides.

## Gate

Each `GATE-OK-*` echo is bound to its cargo exit code (no pipes); logs are id-named in the shared scratchpad.

Post-round-1 (R1 + R2):

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (`GATE-OK-fmt-861q`) |
| `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | PASS (`cyril-861q-review-clippy-kas.log`) |
| `cargo nextest run -p cyril-core --features kas -- kas::version` | PASS — 18 run, 18 passed (`cyril-861q-review-green.log`) |
| `cargo nextest run -p cyril-core --features kas version` | PASS — 30 run, 30 passed (`cyril-861q-review-nextest-version.log`) |
| `cargo nextest run -p cyril -p cyril-core --features kas` | PASS — 1264 run, 1264 passed, 10 skipped (`cyril-861q-review-nextest-kas.log`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (`cyril-861q-review-clippy-default.log`) |

Post-round-1b (R3), the state being committed:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (`GATE-OK-fmt-861q-r2`) |
| `cargo nextest run -p cyril-core --features kas -- kas::version` | PASS — 19 run, 19 passed (`cyril-861q-rereview-green.log`) |
| `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | PASS (`cyril-861q-rereview-clippy-kas.log`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (`cyril-861q-rereview-clippy-default.log`) |
| `cargo nextest run -p cyril-core` (default features; path.rs) | PASS — 782 run, 782 passed, 4 skipped (`cyril-861q-rereview-nextest-core-default.log`) |
| `cargo nextest run -p cyril -p cyril-core --features kas` | PASS — 1265 run, 1265 passed, 10 skipped (`cyril-861q-rereview-nextest-kas.log`) |

## Commit grouping

R1, R2 and R3 share `version.rs` and its test module (a common fixture helper and adjacent hunks in `build_wrapper_command`); no intermediate state with only some of them applied was ever built or run as a commit candidate, so they land in one commit whose message lists each repair and its finding IDs, per the stage's shared-file rule. The compact records above stay one per atomic repair.

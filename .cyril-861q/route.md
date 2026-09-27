# Route: cyril-861q

Change: `build_wrapper_command` probes `<program> --version`, so a wsl-prefixed
wrapper command (`--agent-command wsl kiro-cli acp`) resolves the
`--agent-engine` flag from WSL's own version string. Probe the kiro-cli
element *through* the launcher instead.
Date: 2026-09-27

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The only external premise is that `wsl.exe [launcher options] kiro-cli --version` runs `kiro-cli --version` inside the distro that `wsl.exe [same options] kiro-cli acp` spawns the agent in. That is wsl.exe's documented command-line passthrough (Microsoft Learn, "WSL interop — Command-line interop": `wsl ls -la /home`, `wsl -d Ubuntu-22.04 -- cat /etc/os-release`; "Basic commands for WSL": `wsl --distribution <Distro> --user <User>`, `wsl ~`) and it is the exact mechanism the production spawn already relies on (CLAUDE.md "Platform Constraints": Windows spawns `wsl kiro-cli acp`; `platform/path.rs::is_wsl_launcher`, cyril-jxmv). The launcher option grammar used for flag skipping (value-taking: `-d/--distribution`, `-u/--user`, `--cd`, `--shell-type`; bare: `-e/--exec`, `--system`, `--`) is from `wsl --help` as reproduced in those docs. The defect itself was observed during the cyril-jxmv probe (`.cyril-jxmv/findings.md` lines 58–63). No live WSL host is available here, and none is needed for a mechanism the existing spawn already depends on; the requester approved the probe rule explicitly. | no |
| 2 | Structural module shape | Interfaces touched: (a) `kas/version.rs::kiro_cli_version` — `pub(crate)`, two in-crate callers (`version.rs::build_wrapper_command`, `discovery.rs:287 installed_cli_version`; census by grep, which beats tethys's index-only result of one caller while rust-analyzer is degraded in this worktree) — its `program: &str` parameter becomes `&AgentCommand`; (b) `platform/path.rs::is_wsl_launcher` — private today, one caller (`resolve_agent_location`) — widened to `pub(crate)` for reuse (the requester named this reuse; `platform::path` is `pub mod` but `pub(crate)` stays crate-internal, so no public API change); (c) new `pub(crate) fn version_probe_command` in `kas/version.rs` — the pure "which argv answers the version question" computation. Responsibility ownership is unchanged: version.rs already owns "what command answers the version question" (it hard-codes `--version` today), and launcher detection stays owned by `platform/path.rs` (reused, not duplicated). No module is split, merged, created, or deleted; no dependency direction changes (`kas` already depends on `platform::path` — `discovery.rs:418` imports `resolve_agent_location`); `build_wrapper_command`'s signature and its single caller (`bridge.rs:346 resolve_spawn_command`) are untouched. Length: `kas/version.rs` is 221 lines (≈155 production + 43 test) → projected ≈350 lines (+≈60 production, +≈70 test, ±20 margin). No repository length gate exists (`grep -rni 'max.lines\|line.count\|length gate' scripts/ .github/` → no hits); no `.cyril-*` manifest declares one for this file. Trigger: none. | no |
| 3 | Production-scale risk | One bounded subprocess at spawn time, same count as today (the probe already existed; only its argv changes). No latency, throughput, memory, concurrency, or data-volume dimension. | no |
| 4 | Explicit behavior | Given `["kiro-cli","acp"]`, when the wrapper version probe is computed, then it is program `kiro-cli`, args `["--version"]` (unchanged). Given `["wsl","kiro-cli","acp"]` → program `wsl`, args `["kiro-cli","--version"]`. Given `["WSL.EXE","kiro-cli","acp"]` → program `WSL.EXE`, args `["kiro-cli","--version"]`. Given `["C:\Windows\System32\wsl.exe","-d","Ubuntu","kiro-cli","acp"]` → program `C:\Windows\System32\wsl.exe`, args `["-d","Ubuntu","kiro-cli","--version"]` (launcher flags and their values are kept in front of the kiro-cli element; a value-taking flag consumes exactly its next element; a bare flag such as `-e` or `--` consumes none). Given a launcher argv with no non-flag element after its flags (`["wsl","-d","Ubuntu"]`), when the probe is computed, then `build_wrapper_command` returns `Err` naming the command (the existing `Result<_, String>` path that `resolve_spawn_command` maps to `InvalidConfig`), and no process is spawned. Given `build_wrapper_command` on a wsl-launched command whose probed kiro-cli reports 2.21.1, when the version resolves, then the result is the original program and args with `--agent-engine v3` appended (args preserved, program unchanged). Given a non-launcher program, when the probe is computed, then the agent args are not forwarded to the probe (unchanged from today's `<program> --version`). Non-goal (permanent, out of the approved rule): wsl.exe's positional `~` argument is not a `-`-prefixed flag and is not skipped; reported as discovered-not-fixed. | yes |

Unknown tests: none

## Selected route

Local — behavior fully explicit; the only external premise is the launcher passthrough the production spawn already relies on; no public interface, module-shape, ownership, or scale change.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict); the requester's approved rule is the spec |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict) |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD (red first: a `#[cfg(unix)]` reproduction that drives `build_wrapper_command` through a fake `wsl` launcher script which answers `--version` with a WSL-style string and answers `… kiro-cli --version` with `kiro-cli 2.21.1`; then the pure-function fences the requester named).

## Terminal criterion

Local — focused behavioral verification: `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas -- kas::version` (the fence tests in `crates/cyril-core/src/protocol/kas/version.rs`) plus the full CI-mirroring gate listed in the PR body; after the hand-off append `Result: <YYYY-MM-DD> | <command> | <PASS or FAIL>` and final T2 evidence here.

Red (before product code: the branch base f9bc81d8 plus the reproduction test only):
`cargo nextest run -p cyril-core --features kas -- wrapper_probe_asks_kiro_cli_through_the_wsl_launcher`
→ FAIL: the fake launcher's argv journal was `--version` alone (expected
`-d Ubuntu kiro-cli --version`) — the launcher was asked for its own
version and kiro-cli never ran.

Result: 2026-09-27 | `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas -- kas::version` | PASS (12 run, 12 passed: 6 `version_probe_*` fences, `wrapper_build_refuses_a_launcher_without_a_kiro_cli_element`, the unix reproduction `wrapper_probe_asks_kiro_cli_through_the_wsl_launcher`, and the 4 pre-existing flag/semver tests)

Full gate on this commit's crate sources (the gate ran at pre-amend
e852b199; the amend that produced the final commit added only this
route result, so the crate tree is byte-identical). Each `GATE-OK-*`
echo is bound to its cargo exit code and was recorded in the per-task
output files, since the session scratchpad is shared with sibling agents
and same-named logs there interleaved:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo nextest run --workspace --all-features` | PASS (2065 run, 2065 passed, 13 skipped) |
| `cargo test --doc --workspace --all-features` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo nextest run --workspace` | PASS (2063 run, 2063 passed, 13 skipped) |
| `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | PASS |
| `cargo nextest run -p cyril -p cyril-core --features kas` | PASS (1258 run, 1258 passed, 10 skipped; a first attempt tripped the pre-existing 100 ms wall-clock budget in `cyril::capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown` at 0.865 s under a 12-agent build load — it passed alone at 0.137 s and in the full re-run; not a file this change touches) |

Final T2 recheck: `crates/cyril-core/src/protocol/kas/version.rs` is 450
lines (243 production, 207 test) — production grew by ≈65 lines inside the
existing "which command answers the version question" responsibility;
`is_wsl_launcher` stays owned by `platform/path.rs` (visibility widened to
`pub(crate)`, body untouched); no interface beyond the crate-internal
`kiro_cli_version` parameter changed; no length gate exists for the file.
No new trigger.

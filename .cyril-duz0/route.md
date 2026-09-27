# Route: cyril-duz0

Change: Correct three docs/comment sites that claim Windows spawns `wsl kiro-cli acp`; the code spawns `kiro-cli acp` natively on every platform. No behavior change.
Date: 2026-09-27
Base revision: f9bc81d8 (main), branch `docs/cyril-duz0-native-windows-spawn`

## Claim verification (requester decision: verify first)

The issue's claim was checked against the code at f9bc81d8 before any edit:

| Fact | Evidence | Result |
|---|---|---|
| `--agent-command` defaults to `["kiro-cli","acp"]` on every platform | `crates/cyril/src/main.rs:25-32` — `#[arg(long = "agent-command", num_args = 1.., default_values_t = vec!["kiro-cli".to_string(), "acp".to_string()])]`, no `cfg` attribute on the field or the default | CONFIRMED |
| `AgentProcess::spawn` execs the argv verbatim | `crates/cyril-core/src/protocol/transport.rs:213-233` — `Command::new(cmd.program())` + `.args(cmd.args())`; the only platform-conditional line is `#[cfg(unix)] command.process_group(0);` (transport.rs:230-231), which adds no wrapper | CONFIRMED |
| The bridge passes the resolved command straight to spawn | `crates/cyril-core/src/protocol/bridge.rs:380-385` — `resolve_spawn_command(...)` (engine resolution only) → `bind_agent_location(command.program())` → `AgentProcess::spawn(&command, ...)` | CONFIRMED |
| No `cfg(windows)` spawn wrapping anywhere in the workspace | `grep -rn 'cfg(windows)\|cfg(target_os = "windows")' crates/` — hits are in `spawn_environment.rs:50` (env-var key case-folding, not the command), `kas/terminal_io.rs`, `kas/hooks.rs`, `cyril-memory` IPC/paths/permissions, `cyril-workbench` runtime, and test gates. None wraps the agent argv. `grep -rn -i wsl crates --include='*.rs'` hits only path translation (`platform/path.rs`, `tests/win_wsl_wiring.rs`), `kas/discovery.rs:412-431` (a test proving a resolved KAS spawn classifies Native even when the CLI said `wsl`), doc comments, and the transport.rs:224-225 comment this change fixes | CONFIRMED — no STOP condition |
| WSL is reachable only by explicit user choice | `platform/path.rs` `resolve_agent_location` (per CLAUDE.md Path Translation, cyril-jxmv): program basename `wsl`/`wsl.exe` ⇒ WSL, else Native; `CYRIL_AGENT_LOCATION` overrides. The user must pass `--agent-command wsl kiro-cli acp` to get there | CONFIRMED |

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The only premise is "what does the code spawn", answered by reading the code at f9bc81d8 (table above) — current repository evidence, no external/system behavior involved. The one external fact stated in the new text ("`kiro-cli` on PATH resolves to the native `kiro-cli.exe` from the Kiro MSI") is the issue's own description and matches memory `feedback_cyril_native_windows_not_wsl_ceiling` and ROADMAP.md:13 ("Kiro CLI now ships a native Windows binary"); the doc states it as PATH resolution, which is standard `CreateProcess` behavior, not a cyril premise | no |
| 2 | Structural module shape | Files touched: `CLAUDE.md` (Platform Constraints prose), `crates/cyril-core/src/protocol/transport.rs:224-226` (a `//` comment inside `AgentProcess::spawn`; no token outside the comment changes), and this artifact. No public interface, schema, seam, dependency direction, or responsibility owner changes; no module split/merge; no orchestrator growth. Length gates: repository has no `module-shape` gate file (`ls .claude/skills/gilfoyle/references/module-shape.md` is outside this worktree; no repo policy references a length threshold for transport.rs), and the transport.rs line count is unchanged (+1 comment line) | no |
| 3 | Production-scale risk | Docs and a comment; nothing executes differently. No latency/throughput/memory/concurrency/data-volume surface | no |
| 4 | Explicit behavior | Given a reader of CLAUDE.md "Platform Constraints", when they read the Windows bullet, then it states the default `kiro-cli acp` spawn is native on every platform, that Windows resolves `kiro-cli` on PATH to the native `kiro-cli.exe`, that WSL is reachable only via an explicit `--agent-command wsl kiro-cli acp` (requiring kiro-cli installed and authenticated inside WSL), and one line defers the path-translation caveat to the Path Translation section (WSL-located agent only; identity passthrough for native; no-op on Linux), while the terminal-commands and log-file bullets are unchanged. Given a reader of `transport.rs` at `kill_on_drop(true)`, when they read the comment, then it explains the backstop without claiming Windows spawns `wsl`: Windows has no Unix process groups so `kill_on_drop` is the only cleanup the native spawn gets there, and on Unix it backs up `ProcessGroupGuard`. Given the README Prerequisites, when read, then no WSL-only line is present. Given docs/ROADMAP.md, when the Phase 1 README item is read, then it is marked complete consistently with the document's convention. No unresolved decisions — the requester enumerated all three targets and the wording constraints | yes |

Unknown tests: none

## Selected route

Local — behavior is explicit and enumerated by the requester; no premise beyond the code read above; no interface/shape/scale change (a comment edit and prose).

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict); the requester's decisions enumerate the three sites and wording constraints |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict); the claim was verified by reading the code, recorded above |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD

## The three sites (requester scope)

1. **CLAUDE.md "Platform Constraints"** — the `Linux`/`Windows` bullets and the path-translation caveat replaced (see T4). The Path Translation section (cyril-jxmv / cyril-8tq6) is the owner of the translation rules; the caveat now defers to it in one line instead of duplicating it.
2. **transport.rs:224-226** — comment restated (see T4). Comment-only; the Rust gate below proves the file still compiles and its tests pass.
3. **README Prerequisites / ROADMAP Phase 1 item** — **already done at head, no edit required.** `README.md:27` reads "Kiro CLI installed and authenticated (`kiro-cli login`) — Linux, macOS, and Windows native binaries are all supported"; `git log -S"WSL" -- README.md` shows the line was dropped in `f2725b0c docs: drop WSL prerequisite from README`. `docs/ROADMAP.md` tracks completed phases with a `✅` header plus a `**Status:** complete` paragraph (Phase 0 at line 28-30, Phase 1 at line 34-40), not per-bullet strikethrough (the `~~…~~ shipped` idiom at line 395 belongs to the KAS-8 deferred list, a different section); Phase 1's status paragraph (line 36-40, added 2026-08-08, after this issue was filed on 2026-08-02) already records "README carries no WSL-only line. Residual doc drift elsewhere is tracked on **cyril-duz0**, not here." The bullet at line 47 is the completed phase's original task list, exactly as Phase 0's bullets remain. The item is therefore already marked done in the document's own convention; editing it would be inconsistent with Phase 0.

## Discovered, not fixed (same false claim, outside the issue's three named sites)

- `AGENTS.md:503` — verbatim copy of the old CLAUDE.md line (`AGENTS.md` is a separately tracked, diverged copy of CLAUDE.md, not a symlink: `diff -q` differs, last touched in 08d16183).
- `.agents/summary/components.md:79` — "`AgentProcess::spawn()` — launches `kiro-cli acp` (or `wsl kiro-cli acp` on Windows)".
- `docs/kiro-acp-protocol.md:36` — "Spawn command: `kiro-cli acp` (Linux/macOS). On Windows: `wsl kiro-cli acp`."

## Terminal criterion

Local — focused behavioral verification: (a) `grep -n "wsl kiro-cli acp" CLAUDE.md crates/cyril-core/src/protocol/transport.rs README.md` returns only the opt-in `--agent-command wsl kiro-cli acp` mention in CLAUDE.md (no "spawns `wsl`" claim); (b) `cargo fmt --all -- --check`; (c) `cargo clippy --workspace --all-targets --all-features -- -D warnings`; (d) `cargo nextest run -p cyril-core --all-features`. Results appended below after the hand-off.

Result: 2026-09-27 | (a) `grep -n "wsl kiro-cli acp\|spawns \`wsl" CLAUDE.md crates/cyril-core/src/protocol/transport.rs README.md` → single hit `CLAUDE.md:520` (the opt-in `--agent-command wsl kiro-cli acp` sentence); transport.rs and README.md clean | PASS
Result: 2026-09-27 | `env -u CARGO_TARGET_DIR cargo fmt --all -- --check` → exit 0, receipt `GATE-OK-fmt` | PASS
Result: 2026-09-27 | `env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0 (Finished dev profile in 44.39s), receipt `GATE-OK-clippy-all` | PASS
Result: 2026-09-27 | `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --all-features` → exit 0, `1064 tests run: 1064 passed, 9 skipped` across 9 binaries, receipt `GATE-OK-nextest-core` | PASS

All four gates ran against the working tree containing the two edits (CLAUDE.md, transport.rs) on branch `docs/cyril-duz0-native-windows-spawn` at base f9bc81d8. The `CARGO_TARGET_DIR` unset is an environment correction (the shell exported it as an empty string), not a change to what is checked.

Final T2 recheck: diff is `CLAUDE.md | 6 +++---` and `transport.rs | 5 +++--` (one added comment line); no module, interface, or length-gate trigger reached. No new trigger.

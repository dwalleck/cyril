# Audit 2.31.1 — run state (started 2026-10-14)
OLD: 2.29.1  NEW: 2.31.1  folds: 2.30.0, 2.31.0  focus: default (v3 backend, workflows, unreleased features); 2026-10-14 user: v3/KAS becomes cyril's DEFAULT engine (v2 backend retiring) — KAS is "cyril's current path" for priorities; see v3-retriage.md  tier: constrained → kas, tui, host static lanes run in parallel at user request; live lane last

## Releases
| ver | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS class | TUI class | host class | headline |
|---|---|---|---|---|---|---|---|---|
| 2.30.0 | 2026-10-12T20:15Z | a1561bc5 | 11ee5ab2… | 0.66.33 | A (== 2.29.1, diff -rq empty) | distinct (13,651,231 B) | distinct | [V3] model refusal/unavailable fallback notices; V2→V3 switch offer at chat start; agent-upgrade permission refresh |
| 2.31.0 | 2026-10-12T23:56Z | a1561bc5 (same train, trap 20) | b3ae5daf… | 0.66.34 | B (acp-server.js +32,694 B, package.json version only) | distinct (13,656,146 B) | distinct | [V3] `/sandbox` + local `/autonomous` GA (off by default); MCP tools on-demand for all; `KIRO_HOME` relocates MCP/permissions/sessions/powers/agent state; 15 s service-request timeout |
| 2.31.1 | 2026-10-14T01:00Z | d6fb1e94 | 46d26b88… (== latest manifest) | 0.66.34 | B (== 2.31.0, diff -rq empty) | distinct (13,692,707 B) | distinct | Mermaid terminal diagrams; fallback handles empty refusals; compaction refusal fix; [security] workflow recipe text escape sequences in `/workflow` |

## Established facts
- Installed kiro-cli = 2.31.1 (mtime 2026-10-14 17:24); installed tui.js sha256 105848c6a805aa55e6341c2719c34b3ad4d319fb6fa68ed685772d679c6a6ee2 snapshotted to `snapshot-installed/tui-installed-2.31.1.js` before any lane ran.
- Self-extracted KAS tree present: `~/.local/share/kiro-cli/kas/2.31.1-5763e7b9…` (no 2.30.0/2.31.0 trees — host was not launched at those versions).
- Changelogs: `feed-<ver>.json` per release (remote feed, trap 22).
- KAS identity: class A = 0.66.33 (2.29.1 == 2.30.0, `diff -rq` empty incl. licenses); class B = 0.66.34 (2.31.0 == 2.31.1). The ONLY KAS delta in the range is `dist/server/acp-server.js` 12,669,464 → 12,702,158 B (+32,694) and `package.json` (version). No `.d.ts`, externals, or node_modules change. ⇒ every 2.30.0 `[V3]` changelog line is NOT KAS code (TUI, host, or backend).
- KAS carve 2.31.1 == self-extracted `~/.local/share/kiro-cli/kas/2.31.1-5763e7b9…` (`diff -rq` empty).
- TUI: four distinct bundles (2.29.1 13,643,233 → 2.30.0 13,651,231 → 2.31.0 13,656,146 → 2.31.1 13,692,707 B). Carve 2.31.1 == installed tui.js (`cmp` silent).
- Host: all three binaries distinct in every release (sha256 differs; sizes in acquire-<ver>.log). Installed kiro-cli / kiro-cli-chat / kiro-cli-term == archived 2.31.1 (`cmp` silent). 2.30.0 and 2.31.0 share BUILD_HASH a1561bc5 yet differ in every binary (trap 20).
- Frames (csize/dsize): KAS tar 27,801,781/183,224,320 (2.30.0) → 27,808,488/183,255,040 (2.31.0) → 27,808,487/183,255,040 (2.31.1); Node ELF 32,031,786/124,679,552 and Bun ELF 29,887,616/79,500,640 unchanged all three; 62 nested magics each.
- Lane plan deviation: the KAS delta is one minified file, so `kas-core` + `kas-workflow` run as ONE lane `kas` (dir `kas/`) whose FINDINGS carries a Workflows subsection.

- kas lane orchestrator spot-check (2026-10-14): KIRO_HOME 0→1, AB_TOOL_LOAD 4→0, KIRO_TOOL_SEARCH_THRESHOLD 1→0 CONFIRMED by grep; `_kiro/environment/start_ssh_session` is 0→5 (lane said 4); `throwOnRequestTimeout` is 5→5 — the lane's "15 s timeouts now throw" cannot be a new token, so it is a call-site change at most: treat as LEAD, not finding. cyril `kiro_home_dir` (crates/cyril-core/src/kiro_agent_config.rs:93-98) accepts any non-empty KIRO_HOME — confirms the ignore-rule divergence.
- `kas/bundle/{old,new}.{raw,norm}` (~50 MB) are regenerable in 7 s by static-kas-bundle-diff-2.31.1.py: DO NOT COMMIT; commit norm.diff, hunks*.txt, part-0*.

- host lane spot-check (2026-10-14), counts in kiro-cli-chat 2.29.1/2.30.0/2.31.0/2.31.1: "The model declined to summarize" 0/0/0/1, KIRO_DISABLE_TOOLKIT_TELEMETRY 0/0/0/2, agent/util/directories.rs 0/0/2/2, "ignoring KIRO_HOME" 0/0/1/1, managed_settings 0/0/0/5, KIRO_ENABLE_LOCAL_SANDBOX 1/1/0/0, auth/partition.rs 0/0/0/4 — ALL CONFIRMED. `unsummarized_dropped` lands in the warn!-logged unknown-status arm crates/cyril-core/src/protocol/convert/kiro.rs:803-808 (DROPPED with log, not silent).
- `host/raw/` (76 MB) is regenerable: DO NOT COMMIT; commit FINDINGS/host-findings.md and the scripts.

- tui lane spot-check (2026-10-14), counts tui 2.29.1/2.30.0/2.31.0/2.31.1 | kas 0.66.33/0.66.34: model_fallback_promoted 0/4/4/4 | 0/0 (TUI-only dark kind); managed_settings 0/0/0/5 | 0/0; HERDR_BIN_PATH 1/1/1/1; report-agent 2/2/2/2; KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED 1/1/1/1 (unchanged ⇒ local_sandbox GA is the host rollout row, not TUI); validationError 11 all tui, 18/18 kas — CONFIRMED. cyril `WireRecipe` (crates/cyril-core/src/protocol/convert/kas/workflow.rs:610-617) has no validationError field ⇒ DROPPED confirmed.
- Cross-lane agreement: KIRO_HOME divergence reported independently by kas, host and tui lanes (one issue, not three).

- v3-coverage spot-check (2026-10-14): blocker #2 STATIC-CONFIRMED — KAS filter `suppress_workflow_gate_commands` (crates/cyril-core/src/protocol/convert/kas.rs:267-300) drops only the 4 workflow-gate names; all other advertised commands reach `register_agent_commands` (crates/cyril-core/src/commands/mod.rs:449-469) whose dispatch is v2 `kiro.dev/commands/execute` (domain_mediator/commands/extensions.rs:30). Its "LIVE frame 146" citation is WRONG: frame 146 = `_kiro/session/rename`; 145 = -32603 for `_kiro/session/setWorkflowNotificationDelivery`, which only the PROBE sends (0 hits in crates/) and 0.66.33 answers identically (sweep-2.29.1 frame 145) — not a delta, not cyril. ⇒ blocker #2 stays STATIC until live V1.
- `ToolKind::Other` is NOT filtered (crates/cyril-ui/src/state.rs:536-558 commits every ToolCallStarted; widgets/chat.rs:353 renders Other by title) — CLAUDE.md ACP-notes sentence is stale (docs issue).
- `orchestrate_subagent` removal in 0.66.33 was already recorded by the 2.29.1 audit; CLAUDE.md KAS-subagent paragraph still describes it (docs issue).

- CORRECTION to v3-coverage blocker #3 (user challenge 2026-10-14, verified LIVE from live/kas-live-sweep-2.31.1.jsonl): KAS DOES support model/effort/mode switching — model+effort are MERGED: `config_option_update` (frame 24, after session/new) carries configId `model` with 21 options, each `_meta.kiro.{hasEffort, effortLevels[], defaultEffortLevel, effortSchemaPath, rateMultiplier, thinkingToggleable}`; effort is written as `set_config_option{configId:"effortLevel"}` (TUI 2.31.1 `sessionWrites:[{configId:"effortLevel",value}]`), thinking as configId `thinking` (write-only; cyril /thinking already uses it), mode as configId `mode` (vibe/spec/quick-spec/bug-fix/plan/autonomous/semantic_reviewer). session/new's own configOptions in this capture omit `model` (frame 8: mode, memoryReflection, autopilot, contentCollection, outputStyle) — read model from the update, never from session/new alone. KAS accepts an unoffered model unvalidated (frame 160 current=bogus-model-zz9). The real gap is cyril-side only: no /model, /effort, /mode command on KAS (builtins: help clear quit new load steer voice usage memory theme thinking sessions spawn kill msg [+hooks workflow review on KAS]); v2 got them from advertised kiro.dev commands. Receive path (crates/cyril-ui/src/state.rs:1023) and send path (BridgeCommand::SetConfigOption) both exist. ⇒ NEW P1 parent "KAS /model picker with per-model effort (+ /mode)" over cyril-cxwb, cyril-lxuo, cyril-838u and a KAS twin of cyril-fj6j; blocks cyril-4c4d. Wiring, not protocol.

- Framing rule (user, 2026-10-14): v2 commands are EXPECTED to fail on KAS; never spend a probe proving it. Flip-blocker findings are about cyril's own routing (engine-agnostic registry dispatching KAS-advertised commands over v2 `kiro.dev/commands/execute`; v2-only /model,/effort), stated as "cyril lacks a KAS-native route", not "KAS rejects X". Live V1 re-scoped to the KAS-native skill invocation frame; V3 adds effortLevel validation vs the model's effortLevels.

- live-lane spot-check (2026-10-15): (b) trust grants persist ONLY as V2 `toolsSettings.execute_bash.allowedCommands` (crates/cyril-core/src/kiro_agent_config.rs:146-251) — LIVE: KAS 0.66.34 honours only V3 `permissions.rules` ⇒ CONFIRMED blocker; (c) lone `tool_call_update` with an unseen id updates nothing and returns true (crates/cyril-ui/src/state.rs:560-575) ⇒ silent drop CONFIRMED; (d) `session/set_model` -32603 on KAS is MOOT for cyril today — BridgeCommand::SetModel has NO production producer (only dispatcher domain_mediator/commands/mod.rs:64 + test event.rs:1062); constraint for the new picker: use set_config_option model/effortLevel. V1 KAS-native answer: `/skill args` as session/prompt text works.
- DO NOT COMMIT `live/tmp/` (191 MB).

## Steps
- [x] 0 locate   - [x] 1 acquire+carve   - [x] 2 lanes   - [x] 3 fold   - [x] 4 file   - [x] 5 remember+commit

## Lanes
| lane | agent | status | FINDINGS sections complete | leads forwarded to live |
|---|---|---|---|---|
| kas (core+workflow) | bg agent | complete 2026-10-14 (14m, 209-line FINDINGS) | all (Summary, Deltas, Workflows, Unreleased, Leads, Changelog, Method, Proposed) | L1–L5 held for live brief |
| tui | bg agent | complete 2026-10-14 (22m, 203-line FINDINGS) | all 6 (+Proposed, Progress log) | L1–L7 held for live brief |
| host | bg agent | complete 2026-10-14 (21m, 177-line FINDINGS; agent reported residual bg work) | all 6 | L1–L7 held for live brief |
| live | bg agent | complete 2026-10-15 02:17 UTC (legs 0–4 + A–D + V1–V5; 489-line FINDINGS; 3 void legs re-run; v2 pair 2.29.1 vs 2.31.1 ran, 320=320) | all (Summary, Legs, Deltas, Lead verdicts, sweep-new-fields, Method) | resumed once for effortLevel validation only |
| v3-coverage | bg agent | complete 2026-10-14 (22m, 449-line FINDINGS, ~180 matrix rows) | all (Summary, Matrix 1–9, Flip blockers, Follow-ups, Proposed, Leads, Method) | V1–V5 forwarded to live |

## Dropped rows (lane finding -> reason not in the document)
| kas 22-24 MCP, WebP, web_fetch, reasoning block, read_file text | engine-internal / model-facing; summarized in doc §3 rows |
| tui T4-T6, T23-T24 (screen reader, tmux, glyphs, changelog, disableWrap) | TUI-local chrome; doc §6 last bullet |
| host serde "flicker", whole-string hashdiff/strdiff | parsing/linker-glue artifacts (host method notes) |
| v3-coverage P3/P4 customAgent errors, session/close step sessions | deferred as checklist items in epic cyril-c6hv |

## Outcome (2026-10-15)
Doc docs/kiro-2.31.1-wire-audit.md; rivets commit 9dd432b9 (epic cyril-c6hv + 12 children; 7 reprioritised; 22 notes); memory 2634481 (reference_kiro_2_31_1_diff.md, index, methodology addendum) pushed; traps.md 24-28.

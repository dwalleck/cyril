# 2.31.1 audit — v3-default re-triage (orchestrator, 2026-10-14)

User direction (2026-10-14): cyril moves to KAS/v3 as its DEFAULT engine because the v2 backend is
being retired soon (cyril-4c4d P1, cyril-a7nk P1). Every delta row from the kas, tui and host lanes is
re-ranked here with **KAS as "cyril's current path"** for the priority rules in fold-and-file.md
(P1 live-proven regression or billed/unsolicited work on the default path; P2 dropped field or missing
route on the default engine; P3 opt-in surface; P4 cosmetic / TUI-only / v2-only-while-retiring).
All rows STATIC until the live lane (refocused the same day on legs A–D) confirms.
The standing (non-delta) parity inventory is the separate `v3-coverage/` lane.

## Tier 1 — every default (KAS) session is affected

| # | finding (lane row) | v3-default consequence in cyril | cyril evidence | proposed action | P (was) |
|---|---|---|---|---|---|
| R1 | Settings cyril sends that 0.66.34 ignores: `subagentOrchestration`, `todoList` (unknown-key since 0.66.33) + `toolSearch.enabled` consumer REMOVED in 0.66.34 (kas `toolSearch` 2→1); `neverDefer` is the only live toolSearch key and cyril does not send it | every session's handshake carries dead keys; the user's `toolSearch` setting silently does nothing; no way to keep an MCP tool eager | protocol/kas/settings.rs:82,97,120-131 | NOTE cyril-53qx (+toolSearch.enabled, +neverDefer); live leg A proves the intake list | P2 (P2) |
| R2 | MCP deferral unconditional (`AB_TOOL_LOAD` 4→0, `tool_search` tool deleted) — `tool_load` calls appear whenever MCP servers exist; TUI 2.31.0 unwraps rawInput `{tool_id, arguments}` | tool-call rows show the loader wrapper, not the real tool/args, for every MCP-using default session | no `tool_load`/`tool_id` handling in crates/ (kas lane) | NOTE cyril-2yn8; NEW issue only if live L3 confirms the shape | P2 (—) |
| R3 | `session_info_update` coverage: 8/25 kinds handled, 17 dropped **unlogged** at convert/kas.rs:525 incl. `model_fallback`, which Kiro's TUI renders since 2.30.0 (fields from/to/trigger/scope/agentSubtaskId; dark `model_fallback_promoted`) | on the default engine a model fallback changes the answering model with no notice and a stale model badge | convert/kas.rs:352,525 | NOTE cyril-58uv (P1 umbrella) + cyril-aoby field map; **raise cyril-aoby P3→P2** (dropped field on default engine) | P2 (P3) |
| R4 | KAS memory default flipped to `{disabled, reflection:false}` when no layer sets `memory` | KAS native memory is now OFF for every cyril session (cyril sends no `memory` key); needs a product decision vs cyril-memory | settings.rs BOOL_MAP/DEFAULTS_ON (no memory key) | NOTE cyril-0na7 (decision: send explicit memory or document off); live leg C | P2 (P2) |
| R5 | KAS enforces org governance itself ("Admin enforcement is not gated: KAS enforces governance itself", host rollout text); `_kiro/governance/state` carries the reason (7 features + disabledReason) | enterprise users on the default engine lose MCP / web tools with no explanation | convert/kiro.rs:1284 `_ => Ok(None)` via engine.rs:345-371 | NOTE cyril-0o7e; **raise cyril-0o7e P3→P2** for the governance/state part | P2 (P3) |

## Tier 2 — default engine, conditional or opt-in

| # | finding | consequence | evidence | action | P |
|---|---|---|---|---|---|
| R6 | KIRO_HOME: KAS 0.66.34 relocates agents, hooks, MCP, sessions, memories, steering, recipes under `userConfigDir`; Kiro (TUI, host, KAS) ignores relative / root / home-or-ancestor values | with KIRO_HOME set, cyril's trust grants are written to `~/.kiro/agents` which KAS no longer reads (silent no-op), cyril's hook registry reads `~/.kiro/hooks`, and cyril accepts KIRO_HOME values Kiro ignores | kiro_agent_config.rs:93-98,101-104; kas/settings.rs:50-59; domain_mediator/mod.rs:262-266; usage/kiro_sidecar.rs:513 | NEW P3 "KIRO_HOME parity with Kiro 2.31.0 (one resolver: ignore rules, agents, hooks)" — all three lanes | P3 (—) |
| R7 | Workflow: `validationError` dropped (invalid recipe lists as valid) | `/workflow list` misleads on the default engine | convert/kas/workflow.rs:610-617 | NEW P3 | P3 |
| R8 | Workflow: `_kiro/workflow/new` accepts `modelId`/`effortLevel` (and `runLabel`) | cyril cannot launch a run on a chosen model/effort | domain_mediator/commands/kas.rs:349-356 | NOTE cyril-4u4a | P3 |
| R9 | Local `/sandbox` + `/autonomous` GA (host rollout `local_sandbox` → all); TUI passes `--sandbox=auto --sandbox-network-mode=…` on KAS spawn; 0.66.34 sandbox grant follows userConfigDir | Kiro users get a sandbox cyril cannot offer | no sandbox argv in cyril spawn | NOTE cyril-be7e (spawn parity) + cyril-6vo6 | P3 |
| R10 | agentCapabilities: TUI parses 7 keys (executionTargets, extensionMethods, replayMarking, sessionListScopes, sessionSources, sourceProviders, strictSessionLoad); cyril parses none (`extensionMethods` only in fingerprint.rs:138 fixture). 0.66.34 adds `_kiro/environment/start_ssh_session` to extensionMethods | cyril cannot feature-detect `config/template`, `codeIntelligence`, SSH | fingerprint.rs:138 | NOTE cyril-tikf (enabler for cyril-lyrx, codeIntelligence) | P3 |
| R11 | `session/load` suppresses abandoned-prompt replay when messages arrive during load | resume behaviour change on the default engine | — | NOTE cyril-rtrh; live leg D | P3 |
| R12 | Workflow recipe text raw on the wire (TUI 2.31.1 sanitises description/validationError, not name); ratatui drops control graphemes, but newlines/bidi/printable CSI tails pass | cosmetic spoofing of `/workflow list` rows | cyril-ui/src/workflow_format.rs:88-98 | NEW P4 (may merge into R7's issue) | P4 |
| R13 | Herdr pane-state reporting (TUI execs `$HERDR_BIN_PATH pane report-agent … --state idle\|working\|blocked`) | optional integration | — | NEW P4 needs-triage feature | P4 |

## Demoted — v2-only while v2 retires

| finding | was | proposal |
|---|---|---|
| v2 compaction refusal → `unsummarized_dropped` (dropped with warn at convert/kiro.rs:803-808) | cyril-0f4e P3 | NOTE only; **lower to P4** (v2-only) |
| v2 `kiro.dev/session/update` model_fallback (refusals with no text) | cyril-ci3j P2 | NOTE only; **lower to P4** — superseded by the KAS path (R3) |
| v3 ease-in prompt on the v2 launcher (`v3_prompt` 10% ext / 50% int) | cyril-a7nk P1 | NOTE only (evidence of Kiro's own migration push) |
| v2 KIRO_HOME counterpart, v2 acp_agent.rs growth, v2 paired capture | live leads | dropped from the live lane |

## Unchanged-safe (no action)
15 s service timeouts (call-site change; `throwOnRequestTimeout` 5→5), WebP→PNG, MCP streamable-HTTP recovery
(arrives as tool-result text), web_fetch `rendered` removed, reasoning block, headless compaction settings
(cyril already sends them), auth partition / social refresh (cyril reads, never refreshes, the store).

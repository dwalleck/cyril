# KAS live session trace — kiro-cli 2.11.0

Companion to [`kas-live-session-trace-2.11.0.jsonl`](kas-live-session-trace-2.11.0.jsonl) — a **real** KAS (v3) session captured 2026-07-02 from kiro-cli 2.11.0's own TUI client, not a probe. This is the richest KAS wire sample we have; use it as a reference corpus for KAS notification shapes, ordering, and interaction/subagent flows.

## Capture method

Kiro's built-in ACP recorder: `KIRO_ACP_RECORD_PATH=~/acp-trace.jsonl`. Each line is `{ts, dir, msg}` — `ts` epoch-ms, `dir` = `out` (client→agent) / `in` (agent→client), `msg` = the raw JSON-RPC frame. **TUI-only feature** (the v2 TUI's ACP client records its own traffic to KAS); the Rust `kiro-cli acp` path cyril spawns does *not* honor it — so this is a *reference* for how the same backend behaves, captured at near-zero gap (in-process, source-timestamped), not a capture of cyril's own stream.

**Redaction:** `accessToken` → `<redacted>` (1 occurrence); `profileArn` account-id masked to `<account-id>`. No other secret-keyed fields present.

## Shape

- **527 frames, ~880s**, dialect KAS (`_kiro/*`), workspace `~/repos/rivets`.
- **2 turns:** prompt id=5 (cancelled at 90.4s via `session/cancel`), prompt id=8 (completed at 879.5s after ~13 min of heavy `agent-subtask` + tool activity).
- 1 `_kiro/auth/getAccessToken` callback → reply `{accessToken, expiresAt, profileArn, provider}`, `provider: "Enterprise"` (note: prior audits assumed this user's *non*-enterprise token — this TUI session used an Enterprise profile).

**Methods** (excl. `session/update`): `session/request_permission` ×10, `_kiro/customAgent/config_error` ×8, `_kiro/sessions/changed` ×6, `session/set_config_option` ×5, `_kiro/mcp/status` ×4, `_kiro/tools/didChange` ×3, `_kiro/progressive_context/items_changed` ×2, `_kiro/policy/changed` ×2, `session/prompt` ×2, and singletons: `initialize`, `session/new`, `session/cancel`, `_kiro/auth/getAccessToken`, `_kiro/governance/state`, `_kiro/steering/documents_changed`, `_kiro/powers/items_changed`, `_kiro/hooks/cancel`.

**`session/update`** (bulk): `agent_message_chunk` ×166, `tool_call_update` ×112, `agent_thought_chunk` ×55, `tool_call` ×45; `session_info_update` kinds: `context_usage` ×21, `pending_interaction` ×10, `interaction_resolved` ×10, `steering_inclusion` ×9, `user_message_id_assigned` ×2, `turn_start` ×2, `turn_completion` ×2, `turn_end` ×2, `focus_update` ×1; plus `agent-subtask`-tagged `tool_call`/`tool_call_update` (KAS subagents — no `list_update`).

## Notable findings

1. **No new wire.** Every `_kiro/*` method and `session_info_update` kind here is already in the [covenant](../../docs/kiro-kas-acp-covenant.md) / audit catalog. A real 8-min KAS session on 2.11.0 stays entirely within the documented surface.
2. **Turn-boundary ordering (cyril-9akh evidence).** On BOTH turns, the only `session/update` trailing `turn_end` within 5s is a single `context_usage` (~10ms after the prompt response). **No `agent_message_chunk`/`tool_call` arrives after `turn_end`** on either turn. So in this KAS session the "streamed text after TurnCompleted" race did not manifest on the wire; only benign context telemetry trails. (Caveat: KAS/v3, 2 turns, one cancelled — one sample, not proof; cyril-9akh may also concern the v2 path.)
3. **`pending_interaction`/`interaction_resolved` directly observed** (10 each) — the 2.7.1 audit could only see these indirectly. Good corpus if the KAS interaction/elicitation path is ever built.
4. **Environment artifact (not a Kiro/cyril bug):** 8 `_kiro/customAgent/config_error` "No front matter found" for `~/repos/rivets/.kiro/agents/prompts/*.md` — frontmatter-less prompt files that KAS's recursive `.kiro/agents/**` scanner tries to load as agents. Env-specific; noted only because it's ~8 error notifications per session there.

## Full agent→client message inventory (systematic pass 2026-07-02)

Every distinct message kind the agent sends, with cyril status. **cyril drops all unknown `_kiro/*` + unknown `session_info_update` kinds to `Ok(None)`** (test `kas_engine_drops_unknown_ext_frame`), so "drops" = safe-but-unused, not a crash.

| kind (×count) | carries | cyril status |
|---|---|---|
| `session/update::agent_message_chunk` (166) | `content.{type,text}` | **handled** |
| `session/update::agent_thought_chunk` (55) | thinking text | **handled** |
| `session/update::tool_call` (45) / `tool_call_update` (112) | `rawInput`, `content[]` (diff/text), `rawOutput`, `_meta.kiro.{agentSubtaskId, toolId, toolOrigin, preview, checkpoint}` | handled; **drops** agentSubtaskId (KAS-3/cyril-fjfu), checkpoint/preview (snapshot memory) |
| `…::agent-subtask` (5 / 11) | `OrchestrateSubAgent` input `{name, preset, prompt, contextFiles}` | KAS-3 (cyril-fjfu) |
| `su::context_usage` (21) | aggregate % + **per-file `items[].{name,uri,tokens,matched,percent,progressivelyLoaded}`** (47 files) | cyril-5et2 (aggregate) / **cyril-1116** (per-file) |
| `su::pending_interaction` / `interaction_resolved` (10/10) | clarifying-question flow | **cyril-qo13** |
| `su::turn_end` (2) | `stopReason` | **handled** (TurnCompleted) |
| `su::turn_completion` (2) | **`{elapsedTime, status, promptTurnSummaries:[{usage, unit:"credit", usedTools[]}]}`** | **DROPS — richer than MetadataUpdated** |
| `su::focus_update` (1) | `{focus.title}` = agent's current focus | **DROPS** |
| `su::turn_start` (2), `user_message_id_assigned` (2) | lifecycle markers | drops |
| `su::steering_inclusion` (9) | `{steeringDocuments[], agentSubtaskId}` | documented |
| `su::available_commands_update` (3) / `config_option_update` (1) | commands + `_meta.kiro.{contextQuery,originalName,type}` | handled (drops typing) |
| `req session/request_permission` (10) | tool-approval + `_meta.kiro.consent{capability,scope,resource,askType,consentRound}` | **cyril-qo13** |
| `_kiro/governance/state` (1) | **`{isEnterprise, features:{mcpEnabled, webToolsEnabled, usageAnalytics, contentCollection, promptLogging, codeReferenceTracker, autonomousAgents}}`** | **DROPS — feature gating** |
| `_kiro/sessions/changed` (6) | `{upserted[], deleted}` observer CDC | documented |
| `_kiro/mcp/status` (4) | full MCP roster (per-server tool schemas) | richer than `McpServerInitialized`; drops |
| `_kiro/progressive_context/items_changed` (2) | 45 skill/context items `{name,description,scope,type,uri}` | drops |
| `_kiro/powers/items_changed` (1) | powers roster `{name,description,keywords[]}` | drops |
| `_kiro/tools/didChange` (3) | tool-search tags `{tag,description,source}` | drops |
| `_kiro/steering/documents_changed` (1) | steering docs (content inline) | documented |
| `_kiro/policy/changed` (2), `_kiro/hooks/cancel` (1) | Cedar policy / hook-cancel | drops |
| `_kiro/customAgent/config_error` (8) | `{path, error}` | env artifact (finding #4) |

**New cyril-relevant surface** (tracked → cyril-0o7e): `_kiro/governance/state` (feature gating), `turn_completion` (per-turn cost + usedTools + elapsedTime, richer than the flat metering), `focus_update` (status line). Per-file `context_usage` includes `progressivelyLoaded` (→ cyril-1116).

**Structural note for the KAS converter:** several `session_info_update` kinds **double-encode** — a typed sub-object AND flattened fields (`pendingInteraction.{question,options}` + flat `question`/`options`; `turnEnd.stopReason` + flat `stopReason`; `focus.title` + flat `title`; `contextUsage.usagePercentage` + flat `usagePercentage`). Read the flat `_meta.kiro.X` consistently rather than mixing.

---

# v2 side-by-side (`v2-live-session-trace-2.11.0.jsonl`)

A real tool-using **v2** (default `kiro-cli acp`) session, same capture method (`KIRO_ACP_RECORD_PATH`, `kiro-tui` 2.11.0), 391 frames / ~667s, 14 prompt turns. Dialect is `_kiro.dev/*` (dotted), vs KAS `_kiro/*`. This exercises the v2 tool/permission surface the earlier trivial proxy capture didn't.

## v2 tool lifecycle (cyril handles all of it)
- `session/update::tool_call` / `tool_call_update`: `rawInput` (`operations[]` for fs_read, `pattern`/`symbol_name`/`output_mode` for search), `_meta.kiro.toolName`, structured `rawOutput.items[].{Json:{numFiles,numMatches,results[].{file,count,matches[]},truncated}, Text}`, `content[]` diffs, `locations[].{path,line}`.
- `session/request_permission` with **`_meta.trustOptions[].{display, label, patterns[], setting_key}`** — v2's command-pattern trust model. The response echoes the chosen `outcome._meta.trustOption.{patterns, setting_key}`. cyril handles trust options.
- `_kiro.dev/metadata` (×16): `meteringUsage[].{unit,value}` + `turnDurationMs` — **cyril parses both** (`convert/kiro.rs` → `TurnMetering{credits, duration}`).
- `_kiro.dev/settings/list`: full roster incl. `chat.modelDefaults.<model>.output_config.effort` (per-model effort default → cyril-lxuo) and `toolSearch.{enabled,minPct,minTokens}`.
- `_kiro.dev/subagent/list_update` `{subagents, pendingStages}` — the `agent_crew` model cyril's `SubagentTracker` is built for (empty this session).

## New observation: v2 client→agent telemetry channel
The TUI emits **`_kiro.dev/telemetry/*` (dir out, client→agent)**: `processHealth` (×11 — `{cpuUserPct, eventLoopP99Ms, heapUsedMb, inputLatencyP95Ms, rendersPerMin, rssMb, yogaNodeCount, …}`), **`uiModeSessionStart` (UNDOCUMENTED — confirms the binary's `ui_mode_*` telemetry rides ACP)**, `chatSlashCommand`. cyril emits none — correctly; it's frontend self-telemetry to AWS, and omitting it is a privacy plus ([[reference_kiro_acp_telemetry]]).

## v2 vs KAS — the shapes differ
| | v2 (`_kiro.dev/*`) | KAS (`_kiro/*`) |
|---|---|---|
| tool metadata | `_meta.kiro.toolName`; `rawOutput.items[].{Json,Text}` | `_meta.kiro.{toolId, agentSubtaskId, preview, checkpoint, toolOrigin}` — subtask grouping + snapshot |
| trust/consent | `trustOptions[].{patterns, setting_key}` (command-pattern) | `consent{capability, scope, resource}` (Cedar) — cyril-qo13 |
| per-turn cost | `_kiro.dev/metadata{meteringUsage[], turnDurationMs}` — **cyril handles** | `turn_completion{promptTurnSummaries[usage, usedTools], elapsedTime}` — dropped (cyril-0o7e); `usedTools[]` is KAS-only |
| subagents | `subagent/list_update` (SubagentTracker) | `agent-subtask` tool_calls, no list_update (KAS-3) |
| client→agent telemetry | `_kiro.dev/telemetry/*` | none observed |
| governance/checkpoint/interaction/focus | absent | present |

Net: the v2 tool-turn surfaced no cyril bug — it confirmed the v2 tool/permission/trust surface is handled. KAS carries *more* per-tool metadata (subagent grouping + checkpoints); the two engines have genuinely different trust models (v2 command-pattern vs KAS Cedar consent), both of which cyril already represents.

## Concrete tool-lifecycle field diff (both traces)

The two engines **share only the ACP `tool_call` skeleton** (`toolCallId`, `title`, `status`, `locations[].path`, nested `content[].content.{text,type}`). Everything semantic is engine-specific — cyril needs a distinct converter path per engine (which the `convert/kiro.rs` vs `convert/kas.rs` split already anticipates):

- **`rawInput` — different tools entirely:** KAS `{text, paths[], start_line, end_line, offset, query, includePattern, caseSensitive, depth}` + subagent `{name, preset, prompt, contextFiles}`; v2 `{operation, operations[].{path,mode,offset,limit}, output_mode, pattern, symbol_name, include_source}`. Concrete confirmation of [[reference_kiro_tool_input_schemas]] (silent-serde-fail risk). cyril survives it because `client.rs` caches `rawInput` as **raw JSON without typing it** — engine-agnostic by not deserializing.
- **`rawOutput` — typed enum vs loose object:** KAS `{message, content, properties, requirementsPath}`; v2 `{items[].{Json:{numFiles,numMatches,results[].{file,count,matches[]},truncated}, Text}}` (a serde tagged enum — the Rust `ToolOutput`).
- **Tool metadata:** KAS `_meta.kiro.{agentSubtaskId, preview{file,local,modified,originalContent,modifiedContent}, checkpoint{local,modified}, toolOrigin, toolId}`; v2 `_meta.kiro.toolName` only.
- **Diff delivery:** KAS emits explicit `content[].{type:"diff", newText, oldText, path}`; v2 `tool_call_update` content was `content`-only (no `diff` type) this session.
- **Trust model (no field overlap):** v2 permission `_meta.trustOptions[].{patterns[], setting_key, label, display}` → response `outcome._meta.trustOption.{patterns[], setting_key}` (command-pattern trust, persisted to a setting). KAS permission `_meta.kiro.consent{capability, resource, askType}` + `consentRound` → response `outcome._meta.kiro.consent{capability, scope, workspaceRoot}` (Cedar capability+scope grant). ⇒ cyril's trust UI/persistence is **engine-conditional**; KAS side = cyril-qo13.

# kiro-cli 2.29.1 wire audit — model fallback goes live, recipe includes, and two settings cyril still sends that KAS dropped (covers 2.28.1, 2.29.0, 2.29.1)

Audited 2026-10-11. Baseline: 2.28.0 ([kiro-2.28.0-wire-audit.md](kiro-2.28.0-wire-audit.md)).
Scope: **discovery, not only compatibility**, with the user's priority on the **v3/KAS backend,
workflows, and unreleased (gated) features**. Five lanes: KAS core static, KAS workflow static,
Rust host static, TUI-bundle static (Kiro's own TUI as the reference ACP client), and one live lane
with paired captures of KAS 0.66.26 vs 0.66.33.

**Verdict: SAFE for cyril on today's default (v2) path — no wire break, and every v2 changelog line
is either already handled or engine-internal. Three things need work on the KAS path: cyril still
sends two `AgentSettings` keys KAS 0.66.33 removed (`subagentOrchestration`, `todoList` — they now
land as `configuration.intake.unknown-key`), cyril does not advertise the new
`_meta.kiro.inactiveItems` capability or call `_kiro/configuration/getState`, and the new
`model_fallback` session_info kind is silently dropped. Workflows are unchanged except two
unannounced additions (recipe `include`, `consentRound` propagation).**

The three findings that matter most:

1. **Model fallback is now a first-class KAS notification** — a 25th `session_info_update` kind
   `model_fallback`, an agent capability `_meta.kiro.modelFallbackReporting`, three flags and a
   server-pushed `server_cascade_config`. cyril drops the notification at
   `crates/cyril-core/src/protocol/convert/kas.rs:525`. This is the same feature the 2.28.0 audit
   captured live on the v2 engine (§3a there); on 0.66.33 it has a KAS wire shape of its own.
2. **cyril's KAS settings handshake is now partly dead.** `subagentOrchestration` and `inlineAgents`
   were removed from the `AgentSettings` schema in 0.66.33 (the `orchestrate_subagent` delegation
   tool is gone with them), and `todoList` has been stale since 0.66.22. cyril still sends both
   (`crates/cyril-core/src/protocol/kas/settings.rs:82,93-98`).
3. **Two unannounced workflow items**: a sixth recipe node type `include` (recipe composition) and
   the propagation of `consentRound` onto the `pending_interaction` / `interaction_resolved`
   session_info kinds. The latter is the wire mechanism behind the open ticket for "Always allow"
   looping past round 20.

This document carries **every** delta row, negative result and dark candidate from all five lanes.
Per-lane detail: [`experiments/conductor-spike/audit-2.29.1/`](../experiments/conductor-spike/audit-2.29.1/).

## 1. Releases and artifacts

| ver | build (UTC) | BUILD_HASH | `@kiro/agent` (KAS) | host / TUI | headline |
|---|---|---|---|---|---|
| 2.28.1 | feed 2026-10-07 | — | — (feed-only) | — | `kiro-cli agent upgrade <DIR>` upgrades a directory of agent configs for V2 and V3 — **located in the 2.29.x host**, absent from 2.28.0 |
| 2.29.0 | 2026-10-09T03:12Z | 31e6bb3f | **0.66.33** | changed | `/memories`, `kiro-cli acp` uses the CLI login unless `--auth-method=client`, stdio MCP inherits the full env, model fallback without restart, images resized, `KIRO_API_KEY` home-region auth |
| 2.29.1 | 2026-10-10T03:23Z | 31e6bb3f (same) | 0.66.33 (license file only) | changed | MCP OAuth issuer fix (classic + Rust V2) |

- **2.28.1 is a changelog entry with no artifact.** Every artifact on the origin (Linux, Windows,
  mac) answers 403; its one line folds into this audit and is proven in the 2.29.0 host binary
  (`agent upgrade` is `unrecognized subcommand` in 2.28.0 and a full subcommand in 2.29.0/2.29.1).
- 2.29.1's tarball sha256 `edd84abc…` matches `stable/latest/manifest.json`; 2.29.0 (`324185fb…`)
  has no versioned manifest (the origin serves no `/<ver>/manifest.json`). The installed
  `kiro-cli` and `kiro-cli-chat` are byte-identical to the archived 2.29.1 binaries, and the
  installed `tui.js` is byte-identical to the 2.29.1 carve.
- **`BUILD_HASH` is a release-train stamp, not a content digest.** 2.29.0 and 2.29.1 share
  `31e6bb3f…` while `BUILD_DATE`, `BUILD_VERSION`, all three binary sizes (chat 215,572,200 →
  215,787,048 B; launcher +1,024; term +2,560), ELF sections (`.text` +190,656, `.rodata` +4,096,
  `.gcc_except_table` +9,920, `.eh_frame` +6,624, `.rela.dyn` +2,184, `.data.rel.ro` +2,624,
  `.got` −72, `.data` −64) and the source trees differ (new
  `crates/chat-cli/src/launch/notification_order.rs`; `oauth_util.rs` tracing call sites 47→46 and
  34→33; line shifts in `fs_write.rs`, `tool_manager.rs`, `launch.rs`; reordered adjacent
  `.rodata` literals proving code motion). The two embedded payload ELFs — node (sha `81925c09…`)
  and bun (sha `a83d2637…`) — are **byte-identical** in all three builds.
- **Identity collapse.** KAS 2.29.0 ≡ 2.29.1 except `_preserved-licenses/@types/node/package.json`,
  so the KAS lanes compare 2.28.0 → 2.29.1 and cover both. TUI 2.28.0 → 2.29.1 differs (2.29.0 ≡
  2.29.1 ≡ installed). Host binaries differ in all three. The 2.29.1 KAS carve is byte-identical to
  the self-extracted `~/.local/share/kiro-cli/kas/2.29.1-32dbef42…/`.
- Embedded zstd payload layout unchanged (4 top-level frames; the 62 nested magics are Bun's own
  modules showing through RAW blocks and are present in 2.28.0 too):

  | frame | 2.28.0 | 2.29.0 | 2.29.1 |
  |---|---|---|---|
  | KAS `kas-bundle.tar` | 180,664,320 | 183,224,320 | 183,224,320 |
  | Node ELF | 124,679,552 | same csize + sha | same csize + sha |
  | Bun ELF | 79,500,640 | same csize + sha | same csize + sha |
  | TUI bundle | 13,633,741 | 13,643,233 | 13,643,233 |

  The KAS frame moves by exactly 200 compressed bytes between 2.29.0 and 2.29.1 (the `@types/node`
  license file); node csize 32,031,786 and bun csize 29,887,616 are unchanged in all three.
- KAS file-level 0.66.26 → 0.66.33 diff: `acp-server.js` 12,295,918 → 12,669,464 B;
  `acp-server.externals.json`; NEW `api-key-region-probe.d.ts`; changed `client-connections`,
  `env-auth-provider`, `file-auth-provider`, `multiplex-stream`, `select-auth-provider` `.d.ts`;
  NEW runtime dep `@silvia-odwyer/photon-node@0.3.4` (`sharp` demoted to a devDependency).
- TUI bundle sha256: 2.28.0 `6733ecdbd8a72ecd…`, 2.29.1 `256b00c17fdd301d…` (== 2.29.0 == installed).
- Changelog source: `prod.download.cli.kiro.dev/stable/<ver>/feed.json`, copied to the evidence
  directory as `feed-2.29.1.json` and `feed-2.28.1.json`. `kiro-cli version --changelog` is remote
  since 2.28.0, so an archived binary prints the current feed, never its own.
- Archive: `~/.local/share/kiro-research/{binaries,kas-carves,tui-bundles}/`. Carve tooling:
  `experiments/conductor-spike/carve-frames-2.29.1.py`.

### 1a. Changelog reconciliation — every line

`feed-2.29.1.json` (2.29.1, 2.29.0), `feed-2.28.1.json` (2.28.1, 2.28.0). Every line is either
located or marked not exercised by cyril.

**2.29.1 (1 line)**

| line | location |
|---|---|
| MCP OAuth sign-in in classic and Rust V2 now works with servers that require an issuer | **host**: engine-internal `crates/agent/src/agent/mcp/oauth_util.rs` 47→46 and `crates/chat-cli/src/mcp_client/oauth_util.rs` 34→33 tracing call sites, zero new literals. KAS MCP OAuth server-metadata schema identical (34/34, 3/3). cyril NOT-SENT (it only surfaces `oauthUrl`). **TUI**: located for its own line 10 |

**2.28.1 (1 line, feed-only, folds into 2.29.x)**

| line | location |
|---|---|
| `kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3 | **host**: `crates/chat-cli/src/cli/agent/upgrade.rs` (new), registered in `root_command_args.rs` (+23), engine in `crates/agent/src/agent/agent_config/migration/io.rs` (+23). 0 hits in both KAS bundles. cyril NOT-SENT. **TUI**: `/upgrade-agent` report side (D10, line 24) |

**2.29.0 (22 lines)**

| # | line | location |
|---|---|---|
| 1 | [V3] `/memories` configures memory access and automatic updates for local sessions | **TUI**: the `/memories` entry is unchanged (×10==×10); the substance is the handshake D1–D4 plus new panel copy. **KAS**: no wire change (memory schema and `_kiro/memory/*` 20==20). **host**: rollout `memory`/`memory_controls` internal→all |
| 2 | V2 TUI hook matchers such as `read` cover more tools | **host/TUI**: `tool_tags` payload key (see 4); no matcher literal changed; TUI report text only. KAS `_kiro/hooks/list` already carried `toolTags` in 2.28.0 |
| 3 | V2 TUI tool hooks receive `tool_tags` | **host**: new host hook-payload key `tool_tags` (0→3) + telemetry dimension; **not** on the ACP wire. cyril DROPPED |
| 4 | [V3] `kiro-cli acp` uses your CLI login unless you pass `--auth-method=client` | **host**: `AUTH_METHOD` help rewrite + `client` variant + `(default)` on `cli`. cyril HANDLED (cli, `reviewer/runtime.rs:231`) / NOT-SENT (client) |
| 5 | [V3] Custom agents use their tool settings to decide which `mcp.json` servers to start | **KAS**: `requiredMcpServers(t,r)`, `runtimeServers.clientRequiresReady(...)`, `poolServersFor`/`restateRuntimeServers` replace `desiredServersForSession(s)`, `includeMcpJson` gating |
| 6 | [V3] Stdio MCP servers now inherit the full agent environment | **KAS**: OLD `o3r()` curated allow-list → NEW `Tlu()` = all of `process.env`, merged by `xlu` |
| 7 | Classic `fs_write` no longer auto-approves symlink writes outside `allowedPaths` | **host**: no new literal (`allowedPaths` 3/3/3, `auto-approve` 5/5/5); call-site lines shift −1 at the allow/deny check; `Updating:`/`Appending to:` reorder. cyril DROPPED (engine-internal) |
| 8 | Model fallback changes take effect in the current session without restarting | **KAS**: `model_fallback` kind + flags (this audit's headline). **host**: `chat/cli/model.rs` −40 lines; cyril HANDLED via `kiro.rs:1209` |
| 9 | MCP OAuth now shows the authorization URL so you can open it from SSH or tmux sessions | **TUI**: 4 new copy strings + 3 removed; **host/TUI**: cyril HANDLED `kiro.rs:1123-1143` |
| 10 | [V3] Non-interactive runs respect `telemetry.enabled` and `KIRO_DISABLE_TELEMETRY` | **KAS**: both identifiers 0 hits; telemetry defaults unchanged. **host**: settings key set unchanged (119). cyril HANDLED |
| 11 | [V3] `/upgrade-agent` rewrites legacy hook matchers such as `fs_read` so they fire on V3 | **TUI**: report side only (`Hook matcher widened`, `unportable-hook-matcher`, `hooks[].matcher`). The CLI twin is the 2.28.1 line. `agent upgrade` is 0 hits in KAS |
| 12 | Knowledge search limits apply across all bases, with combined pagination and file-type filtering | **host**: `semantic-search-client/.../context_manager.rs` 2→1 calls, `bm25_index.rs` 2→1, `semantic_context.rs` shifts. cyril NOT-SENT |
| 13 | [V3] Sessions no longer unexpectedly close under heavy load | **KAS**: no wire delta; candidate locus is the mux ask-lifecycle rework (row 16 below) |
| 14 | [V3] Custom agents dispatched as subagents now load their own `mcpServers` | **KAS**: `acquireSubagentMcp`, `profileAgentServers`, `runtimeSessionOf`, `restateRuntimeServers`, `subagentMcpScopes` (all 0 in 2.28.0) |
| 15 | [V3] Effort selector is offered at session start for an effort-capable default model | **TUI**: no TUI delta (effort token counts identical); **KAS** `effortLevels` 29→33 ⇒ engine advertisement change |
| 16 | [V3] Images are resized to fit the selected model instead of failing the request | **KAS**: `@silvia-odwyer/photon-node` pipeline (photon 0→12, `maxBase64Bytes` 0→13, placeholder texts 0→3) |
| 17 | [V3] `KIRO_API_KEY` keys minted outside `us-east-1` now authenticate in their home region | **KAS**: `api-key-region-probe.d.ts` + region discovery (row 10 below). **host**: `auth/social.rs` shifts only |
| 18 | [V3] Readable retry message when the backend returns a non-JSON error page | **KAS**: no token delta (`retryErrorType` 75→75, `display_error` 10→10, `non-JSON` 2→2) — SDK/formatting, not a wire shape. **host**: AWS-SDK/refresh churn only |
| 19 | [V3] Resumed sessions keep `UserPromptSubmit` hook output in the conversation | **KAS**: `_kiro/hooks/*` shapes unchanged; the new compaction-carry fields (`summaryKind`/`summaryUserRequests`/`noteUserRequests`/`agent_note`) are the likely mechanism |
| 20 | [V3] Stop now cancels a prompt sent while MCP servers are still starting | **KAS**: no wire delta; the prompt path already raced `settleSessionModel` against the MCP wait in 2.28.0; only the required-server predicate changed. **host**: KAS-spawn block line shifts |
| 21 | [V3] A prompt sent just as a turn ends now includes that turn in the model's history | **KAS**: no wire delta (turn lifecycle kinds unchanged); TUI `turn_end` handling unchanged |
| 22 | [V3] Manual steering files now load even when their slash command appears mid-prompt | **KAS**: no wire delta (steering kinds unchanged) |

**2.28.0 baseline block (11 lines, in `feed-2.28.1.json`)** — reconciled by the previous audit
(`audit-2.28.0/host-findings.md`); listed here for completeness: full-screen copy-on-select,
classic→3.0 offer, `[V3]` hook-matcher tool tags, `/model`+`/effort` mid-turn pickers, V2 TUI
sub-agent stall fix, activity-tray rows, Windows V2 stdio MCP argv, voice-setting hint,
agent-monitor return, `[V3]` interrupted-tool-call outcomes, `[V3]` long-session memory growth.

**No changelog line maps to a workflow-lane delta** — the two workflow findings (`include`,
`consentRound` propagation) are unannounced.

## 2. KAS 0.66.26 → 0.66.33 (core)

Nothing was removed from the `_kiro/*` method set or the `sessionUpdate` variant set. Adds are one
client→agent method, one client capability, one agent capability, five flags and one session_info
kind. Counts are N in 2.28.0 → 2.29.1. cyril refs are `file:line`; `protocol/engine.rs:97` is
`client_capabilities` — the complete outbound `_meta.kiro` surface — so a NOT-SENT capability row is
proven by that function, and `protocol/kas/settings.rs:111,116` are the only `AgentSettings` writers.

| # | item | version | gate/default | direction | S/L | cyril verdict + `file:line` |
|---|---|---|---|---|---|---|
| 1 | `_kiro/configuration/getState` (client→agent) | 2.29.0 | advertised unconditionally in `initialize._meta.kiro.extensionMethods`; params `{observer:"connection"}` \| `{observer:"session",sessionId}` \| `{observer:"workspace",roots[]}` | ADD (0→8) | STATIC | **NOT-SENT** — no call site; outbound shape `domain_mediator/commands/extensions.rs:262` |
| 2 | `clientCapabilities._meta.kiro.inactiveItems` | 2.29.0 | literal `===true` declares the client wants ALL resource items with `standing`; absent ⇒ `resourcesFor` keeps only `items.filter(n=>n.standing.kind==="active")` | ADD (0→2) | STATIC | **NOT-SENT** — `protocol/engine.rs:97` builds `_meta.kiro` with only `settings` + fs flags |
| 3 | `initialize` result `_meta.kiro.modelFallbackReporting:true` | 2.29.0 | agent capability, unconditional | ADD (0→1) | STATIC | **DROPPED** — cyril ignores initialize-result `_meta.kiro` extras (no reader in `protocol/`) |
| 4 | `session_info_update` kind `model_fallback` | 2.29.0 | emitted on fallback; payload `_meta.kiro={kind:"model_fallback", from, to?, scope?, trigger, reason?, agentSubtaskId?}` (+ persisted `abandonedMessageIds`) | ADD (kinds 24→25; token 0→8) | STATIC | **DROPPED** — `convert/kas.rs:525` `_ => None` |
| 5 | flag `model_fallback_refusal` | 2.29.0 | default `false`; env `KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED` | ADD | STATIC | NOT-SENT (`protocol/engine.rs:97`) |
| 6 | flag `model_fallback_availability` | 2.29.0 | default `false`; env `KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED` | ADD | STATIC | NOT-SENT (`protocol/engine.rs:97`) |
| 7 | flag `server_cascade_config` | 2.29.0 | default `""`; no env; precedence env > `server_cascade_config` > `defaults` | ADD | STATIC | NOT-SENT |
| 8 | flag `vibe_to_default_v2_routing_enabled` | 2.29.0 | default `false`; AB `AB_VIBE_TO_DEFAULT_V2_ROUTING`; no env; hidden setting `vibeToDefaultV2Routing` | ADD | STATIC | NOT-SENT (`kas/settings.rs:116`) |
| 9 | flag `skill_name_qualifiers` | 2.29.0 | default `false`; env `KIRO_FEATURE_SKILL_NAME_QUALIFIERS_ENABLED`; AB `SKILL_NAME_QUALIFIERS` | ADD | STATIC | NOT-SENT |
| 10 | `KIRO_API_KEY` home-region discovery | 2.29.0 | applies only on the apiKey path AND NOT (`--region` or (`--endpoint` AND `--control-plane-endpoint`)); probes `[us-east-1, eu-central-1, us-gov-east-1, us-gov-west-1]` via CPS `GetProfileCommand`, 10 s/region, retry cooldown 5–60 s | ADD (apiKeyRegion 0→5; regionSource 0→6) | STATIC | NOT-SENT — no wire field; agent-side routing. cyril spawns KAS without `--region`/`--endpoint` (`protocol/kas/discovery.rs`) ⇒ discovery active |
| 11 | `EnvAuthProvider.region` getter + `resolveIdentity()` awaits region | 2.29.0 | `readonly region` → `get region()`; identity read resolves it before the first request | CHANGE | STATIC | NOT-SENT (`protocol/kas/auth.rs` answers `getAccessToken` only) |
| 12 | `--auth=machine` `--token-path`/default token path absolutised | 2.29.0 | `isAbsolute(t)?t:resolve(t)` around `DEFAULT_MACHINE_TOKEN_PATH="/tmp/access_token"` | CHANGE | STATIC | NOT-SENT (agent-side) |
| 13 | `.d.ts` growth: `AuthSelectionParams.apiKeyRegionSource?`, `EnvAuthProviderOptions.regionSource?`, `FileAuthProviderOptions.tokenPath` doc | 2.29.0 | new optional fields | ADD | STATIC | NOT-SENT |
| 14 | advertised `authMethods` `[aws-builder-id, aws-iam-identity-center]` | — | unchanged (1:1) | — | STATIC | HANDLED (`protocol/kas/auth.rs`) |
| 15 | `--auth` mode set | — | `["user","machine","acp-callback"]`, identical parser in both | — | STATIC | HANDLED (`protocol/kas/auth.rs`) |
| 16 | mux: `interaction_resolved` broadcast to EVERY live client (incl. `selectedOption`) + `resendOpenAsks` after `initialize` + `askOpened`/`askResolved`/`markAskResolved`/`sendToEveryLiveClient` | 2.29.0 | every connected client (not just subscribers); replayed updates (`_meta.kiro.replay===true`) exempt | ADD (askResolved 0→2, askOpened 0→4, resendAsks 0→5, `replay===!0` read 0→1) | STATIC | **DROPPED** — 0 grep hits for `interaction_resolved`/`selectedOption` in cyril; single-stdio client ⇒ broadcast ≡ subscriber set |
| 17 | `pending_interaction` / `interaction_resolved` gain `consentRound` | 2.29.0 | optional field | ADD | STATIC | **DROPPED** (same arm as 16) |
| 18 | multi-client config-state: `declaresInactiveItems`, `configurationStateFor(id,state)`, `handshakeOf(id)`, `resourcesFor(initialize,resources)`; push guard `this.observes(r,recipient)` | 2.29.0 | per-client handshake | CHANGE | STATIC | NOT-SENT (`protocol/engine.rs:97`) |
| 19 | `AgentSettings`: REMOVE `subagentOrchestration`, `inlineAgents`; ADD `vibeToDefaultV2Routing`; the known-outside-schema list drops `subagentOrchestration` | 2.29.0 | schema `catchall(unknown)` | REMOVE (14→0, 11→0) / ADD (0→5) | STATIC | **DROPPED** (cyril still sends) — `kas/settings.rs:97` `DEFAULTS_ON` → `configuration.intake.unknown-key` |
| 20 | `AgentSettings` `todoList` | ≤2.27 | not in schema, not in known-outside-schema | (already stale) | STATIC | **DROPPED** (cyril still sends) — `kas/settings.rs:82` BOOL_MAP `chat.enableTodoList`→`todoList` |
| 21 | delegation tool `orchestrate_subagent` (+selector) removed; only `invoke_sub_agent` | 2.29.0 | `tne="invoke_sub_agent"` only | REMOVE (12→0) | STATIC | NOT-SENT (cyril sends no tool name); stale doc refs at `kas/settings.rs:5,92` |
| 22 | MCP stdio env: allow-list → full parent env | 2.29.0 | OLD `o3r()` = curated `V9c` allow-list of `i3r.env`; NEW `Tlu()` = all of `process.env` (skip undefined/`"()"`), merged by `xlu` | CHANGE | STATIC | N/A — KAS owns MCP servers; cyril contributes nothing (`convert/kas.rs` has no `mcpServers`) |
| 23 | subagent MCP scopes: `acquireSubagentMcp`, `profileAgentServers`, `runtimeSessionOf`, `restateRuntimeServers`, `subagentMcpScopes` | 2.29.0 | per-subagent registry; OLD had `desiredServersForSession(s)` | ADD (all 0→N) | STATIC | N/A (KAS-internal) |
| 24 | `requiredMcpServers(t)` → `requiredMcpServers(t, requiredSet)`; `clientMcpServers.some(waitForReady)` → `runtimeServers.clientRequiresReady(...)` | 2.29.0 | predicate generalised; the abort race already existed in OLD | CHANGE | STATIC | N/A |
| 25 | tool-tag map: −`orchestrate_subagent`, −`searchMemories`; +`memory:"context"`, +`validate_spec_format:"fs_read"` | 2.29.0 | feeds hook `toolTags` | CHANGE | STATIC | HANDLED generically (`protocol/kas/hooks.rs`) |
| 26 | remote tool alias `searchMemories` → `memory` | 2.29.0 | `$Ao="searchMemories"` → `"memory"` | RENAME (3→0) | STATIC | NOT-SENT |
| 27 | `ClientCustomAgent` frontmatter field `steeringAfterPromptBody` (boolean) | 2.29.0 | conditional spread | ADD (0→28) | STATIC | NOT-SENT — cyril never sends `customAgents` (0 hits in `crates/cyril-core/src`) |
| 28 | image-fitting pipeline (external `@silvia-odwyer/photon-node`) | 2.29.0 | worker decode/resize/encode; `maxBase64Bytes` cap; placeholder on `no_vision`/`undecodable`/`unfittable` | ADD (photon 0→12, maxBase64Bytes 0→13, placeholder texts 0→3) | STATIC | NOT-SENT (agent-side) |
| 29 | `_meta.kiro.replay===true` read (ask-resolution suppression on replay) | 2.29.0 | marker set in BOTH versions (`replay:!0`), only READ in NEW | ADD (read 0→1) | STATIC | NOT-SENT (cyril sets no such marker) |
| 30 | config-state zod keys: `configuration`/`envelope`/`manifest`/`stamp`/`versionId`/`hash`/`ino`/`mtimeMs`/`lastReadAt`/`fetchedAt`/`confirmedAt`/`generatedAt`/`identityKind`/`policy`/`roots`/`sweep`/`composeBehavior`/`includedFrom`/`enterpriseManagementStatus` | 2.29.0 | resources + control-plane envelope schema | ADD | STATIC | NOT-SENT (no config-state model; `convert/kas.rs:525` fallback) |
| 31 | compaction/summary keys `summaryKind`/`summaryUserRequests`/`noteUserRequests`/`agent_note` | 2.29.0 | persisted-message schema; carries user requests across compaction | ADD (15/28/5/…) | STATIC | **DROPPED** — no model (`convert/kas.rs:525`) |
| 32 | `KIRO_DISABLE_RECAP` | 2.29.0 | direct `process.env.KIRO_DISABLE_RECAP` → generic `process.env[killSwitchEnv]==="true"` | **NOT A REMOVE** (env-read 1→0, literal 0→1; behaviour identical) | STATIC | NOT-SENT |
| 33 | `display_error.retryErrorType` / retry-message formatting | 2.29.0 | `retryErrorType` 75→75, `display_error` 10→10 | unchanged | STATIC | DROPPED (`convert/kas.rs:525`) |
| 34 | sessionUpdate variant set | — | literal `sessionUpdate:"…"` 1:1 both: `agent_message_chunk, available_commands_update, config_option_update, current_mode_update, session_info_update, tool_call, tool_call_update, user_message_chunk`; `agent_thought_chunk` bare literal 8→8 | unchanged | STATIC | HANDLED |
| 35 | `session/set_config_option` / `config_option_update` | — | 2→2, 5→5 | unchanged | STATIC | HANDLED |
| 36 | `configOptions` count | — | 21→21 | unchanged | STATIC | HANDLED |
| 37 | `_kiro/hooks/list` agent→client request params | — | `{trigger, sessionId, toolId?, toolTags?}` — `toolTags` present in BOTH | unchanged | STATIC | HANDLED (`protocol/kas/hooks.rs:587`) |
| 38 | `_kiro/memory/*` method set + memory schema (`mode: disabled\|read_only\|read_write`, `reflection`) + per-client `settings.memory`→`memorySettings` | — | unchanged (20→20; memorySettings 4→4) | unchanged | STATIC | HANDLED (memory schema unmodelled but not sent by cyril) |
| 39 | telemetry defaults | — | `telemetryEnabled`, `kutsEnabled`, `standaloneTelemetryConfig`, content-collection unchanged; `KIRO_DISABLE_TELEMETRY`/`telemetry.enabled` 0 hits both | unchanged | STATIC | HANDLED |
| 40 | background-execution env vars | — | `KIRO_BACKGROUND_STALL_SECONDS`/`KIRO_BACKGROUND_TIMEOUT_DEFAULT_SECONDS`/`KIRO_MONITOR_TIMEOUT_DEFAULT_SECONDS` all 1→1 | unchanged | STATIC | HANDLED/partial |
| 41 | MCP OAuth server-metadata schema (`issuer`, `authorization_response_iss_parameter_supported`) | — | identical modulo renames (34/34, 3/3) | unchanged | STATIC | HANDLED |

### 2a. Standing coverage — every emitted kind vs cyril

KAS 0.66.33 emits 25 `session_info_update` kinds (converter anchor: the brace-balanced `switch`
containing `case"pending_interaction":return{pendingInteraction:`). Cyril dispatch is
`session_info_to_notification` (`convert/kas.rs:352`); anything not matched falls to `_ => None` at
`convert/kas.rs:525` (DROPPED).

| kind | cyril | file:line |
|---|---|---|
| `turn_start` | HANDLED | `convert/kas.rs:397` |
| `turn_end` | HANDLED | `convert/kas.rs:398` |
| `turn_completion` | HANDLED | `convert/kas.rs:390` |
| `context_usage` | HANDLED | `convert/kas.rs:401` |
| `hook_update` | HANDLED | `convert/kas.rs:371` |
| `steering_queued` | HANDLED | `convert/kas.rs:464` |
| `steering_injected` | HANDLED | `convert/kas.rs:472` |
| `steering_cleared` | HANDLED | `convert/kas.rs:503` |
| `user_message_id_assigned` | DROPPED | `convert/kas.rs:525` |
| `focus_update` | DROPPED | `convert/kas.rs:525` |
| `steering_inclusion` | DROPPED | `convert/kas.rs:525` |
| `display_error` | DROPPED | `convert/kas.rs:525` |
| `summarization_started` | DROPPED | `convert/kas.rs:525` |
| `summarization_separator` | DROPPED | `convert/kas.rs:525` |
| `summarization_completed` | DROPPED | `convert/kas.rs:525` |
| `summarization_failed` | DROPPED | `convert/kas.rs:525` |
| `summary_message` | DROPPED | `convert/kas.rs:525` |
| `recap` | DROPPED | `convert/kas.rs:525` |
| `queued` | DROPPED | `convert/kas.rs:525` |
| `pending_interaction` | DROPPED | `convert/kas.rs:525` |
| `interaction_resolved` | DROPPED | `convert/kas.rs:525` |
| `checkpoint_restore_availability` | DROPPED | `convert/kas.rs:525` |
| `repositories_update` | DROPPED | `convert/kas.rs:525` |
| `model_routed` | DROPPED | `convert/kas.rs:525` |
| **`model_fallback` (NEW)** | **DROPPED** | **`convert/kas.rs:525`** |

`_kiro/*` ext methods/notifications: all 122 `_kiro/` literals in the 2.29.1 bundle were checked
against `crates/cyril-core/src` (the acp crate strips the leading `_`, so cyril matches the `kiro/`
form). Fallback for unhandled notifications is `to_ext_notification` `_ => Ok(None)` at
`convert/kiro.rs:1284`.

**Agent→client notifications / pushes**

| method | cyril | file:line |
|---|---|---|
| `_kiro/error/rate_limit` | HANDLED | `convert/kiro.rs:1074` |
| `_kiro/system/notify` | HANDLED | `convert/kiro.rs:1085` |
| `_kiro.dev/metadata` (v2 family, shared arm) | HANDLED | `convert/kiro.rs:547` |
| `_kiro.dev/session/update` (steering echoes) | HANDLED | `convert/kiro.rs:957` |
| `_kiro.dev/mcp/server_init_failure` | HANDLED | `convert/kiro.rs:1103` |
| `_kiro.dev/mcp/oauth_request` | HANDLED | `convert/kiro.rs:1123` |
| `_kiro.dev/session/inbox_notification` | HANDLED | `convert/kiro.rs:1241` |
| `_kiro/powers/items_changed` | HANDLED | `convert/kas/powers.rs:48` |
| `_kiro/workflow/*` (run_start/run_complete/node_*/paused/steps_queued/watch_poll/loop_iteration) | HANDLED | `convert/kas/workflow.rs` |
| `_kiro/configuration/state` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/session/notify` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/sessions/changed` | DROPPED (test-only mention) | `domain_mediator/tests/serial.rs:38` |
| `_kiro/mcp/status`, `_kiro/mcp/governance_disabled` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/sandbox/status` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/policy/{changed,error}`, `_kiro/policy/ignore_files_changed` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/hooks/{cancel,didChange}` | HANDLED | `protocol/kas/hooks.rs:35,39` |
| `_kiro/spec/taskStatusChanged` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/steering/documents_changed` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/progressive_context/items_changed` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/customAgent/{not_found,config_error}` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/governance/state` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/safety/statusChanged`, `_kiro/safety/propertiesChanged` (Infrastructure Safety gate — can block tool calls) | DROPPED — 0 grep hits in `crates/cyril-core/src` | `convert/kiro.rs:1284` |
| `_kiro/tools/didChange`, `_kiro/code_references` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/c2s/view/changed`, `_kiro/knowledge/indexing*`, `_kiro/terminal/settings_changed`, `_kiro/terminal/write`, `_kiro/tools/content_chunk` | DROPPED | `convert/kiro.rs:1284` |

**Agent→client requests (host callbacks cyril must answer)**

| method | cyril | file:line |
|---|---|---|
| `_kiro/auth/getAccessToken` | HANDLED | `protocol/kas/auth.rs:24` |
| `_kiro/hooks/list` | HANDLED | `protocol/kas/hooks.rs:25` |
| `_kiro/hooks/executeHook` | HANDLED | `protocol/kas/hooks.rs:32` |
| `_kiro/hooks/sessionStart` | HANDLED | `protocol/kas/hooks.rs:42` |
| `_kiro/fs/{read_file,write_file,stat,read_directory,delete}` | HANDLED | `protocol/kas/kiro_fs.rs:96-100` |
| `_kiro/terminal/shell_type` | HANDLED | `protocol/kas/terminal_io.rs:612` |
| `_kiro/{userInput,mcp/elicitation,secret/*,openExternalUrl,tool/semantic_rename,tool/smart_relocate,tool/get_diagnostics,search/*}` | NOT-SENT — cyril never advertises these capabilities, so KAS never calls them | `protocol/engine.rs:97` |

**Client→agent requests (cyril callable, advertised in `extensionMethods`)**

| method | cyril | file:line |
|---|---|---|
| `_kiro/account/getUsage` | HANDLED | `domain_mediator/commands/extensions.rs:262` |
| `_kiro/session/context` | HANDLED | `convert/kas.rs` (context arm) |
| `_kiro/hooks/setEnabled` | HANDLED | `protocol/kas/hooks.rs:29` |
| `_kiro/workflow/*` (invoke/list/listRecipes/new/inspect/cancel/resume/load/retry/pause) | HANDLED | `commands/workflow.rs` + `convert/kas/workflow.rs` |
| `_kiro/configuration/getState` (NEW) | NOT-SENT | unhandled (outbound shape `domain_mediator/commands/extensions.rs:262`) |
| `_kiro/{session/delete,rename,compact,export,history,checkpoint/*,spec/*,policy/check,permissions/*,codeIntelligence,sourceProviders/*,mcp/resetServer,mcp/getPrompt,mcp/getResource,mcp/toggle,memory/*,sandbox/applyConfig,terminal/...}` | NOT-SENT | no call sites |

## 3. Workflows (v3)

**Control plane unchanged; two unannounced additions.** All 25 `_kiro/workflow/*` literals (14
methods + 11 lifecycle events), the 9-event lifecycle map, the 3 watch handlers, the 9 `wf-*`
bundled agent ids, the bundled recipe registry (autoresearch, feature-pipeline, goal, investigate,
publish-pr, ralph, semantic-review-multi-model), the recipe-catalog state, `recipes_changed`,
`resume`/`extendRepeat` messages, the `run_workflow`/`save_workflow_definition` schemas,
`_kiro/session/notify`, the `extensionMethods` array and the persistence classification are
**identical with matching counts (0 added / 0 removed)**. Caveat: this is string-literal and schema
evidence; handler *bodies* are minified and were not compared, so start/stop/approve/resume, the
`load`-vs-`inspect` subscription and the auto-wake bodies are flagged **body-unproven**.

| # | item | version | gate/default | direction | S/L | cyril verdict + `file:line` |
|---|---|---|---|---|---|---|
| 1 | `_kiro/workflow/*` method set (25 distinct literals; 14 methods) + the `extensionMethods` array (`v$r`≡`v9r`) | 2.29.1 | advertised unconditionally in the `initialize` result; NOT gated by `settings.workflows` | unchanged (all counts equal: `/new` 16→16, `/resume` 10→10, `/listRecipes` 10→10, `/inspect` 8→8) | STATIC | HANDLED |
| 2 | Lifecycle events `node_start/node_complete/node_paused/run_start/run_complete/loop_iteration/watch_poll/paused/steps_queued` (map `hBc`→`aou`) | 2.29.1 | gate-independent | unchanged (`node_paused` 20→20, `run_complete` 13→13, `run_start` 4→4) | STATIC (2.28.0 LIVE) | HANDLED |
| 3 | Recipe node union `{step,sequence,repeat,parallel,watch}` + enums `completionSignal`/`onMaxIterations`/`joinPolicy` | 2.29.1 | — | unchanged (5 types both; zod-key raw counts shift ±1 — bundler/format only) | STATIC | HANDLED |
| 4 | Watch handlers `github-pr`/`command`/`background-process`; `_kiro/workflow/listWatchHandlers` | 2.29.1 | registered at run creation | unchanged (`background-process` 1→1, `github-pr` 6→6, `listWatchHandlers` 6→6) | STATIC (2.28.0 LIVE) | HANDLED |
| 5 | Bundled `wf-*` agent ids (9) + bundled recipe registry (7) | 2.29.1 | — | unchanged sets (quoted counts shift from embedding/renames) | STATIC | N/A (KAS-side resources) |
| 6 | **NEW recipe node `include`**: `oKc=k.object({type:k.literal("include"),id:k.string().regex(/^[A-Za-z0-9_-]+$/),path:k.string().min(1),inputs:k.record(k.string()).optional().default({}),includedFrom:wfe.optional()}).strict()` | 2.29.0 | none (fully wired: `includeResolver` 0→13; bundled cap `Gho=50`; included recipes may not include) | **added** (`"include"` 3→6) | STATIC | N/A at runtime (expanded to a `sequence` before the wire) — but cyril's pinned schema lacks it: `docs/kiro-kas-workflow-recipe-schema-2.26.0.json` `$defs.WorkflowNode` has 5 variants |
| 7 | **NEW `includedFrom:{name,path}`** on `sequence` descriptors: `case"sequence":return{nodeId:e.id,type:e.type,steps:…,…e.includedFrom!==void 0&&{includedFrom:e.includedFrom}}` | 2.29.0 | none | **added** (0→8) | STATIC | **DROPPED** — `convert/kas/workflow.rs:340-346` (`WireNodeDescriptor::Sequence{node_id,steps}`) and the re-projection at `:1410` never emit it; `WorkflowNodeType` has no `include` (`types/workflow.rs:172-178`) |
| 8 | **`consentRound` propagation**: `pending_interaction`/`interaction_resolved` carry it, persisted `interaction_resolved` zod carries it, round-aware dedup `uPt(e,t){return e.toolCallId===t.toolCallId&&e.consentRound===t.consentRound}` | 2.29.0 | none (per-approval) | **added** (`consentRound` raw 1→17; zod-key 0→2) | STATIC | **DROPPED** — `convert/kas.rs:525` (neither kind is dispatched); no `consentRound` in `crates/` |
| 9 | `session/request_permission._meta.kiro.consentRound` | — | — | **NOT new** — baseline in 0.66.26 | STATIC | (cyril kas requestPermission path; kas-core lane) |
| 10 | `agent_note` payload `+noteUserRequests?:string[]`; summary payloads `+summaryUserRequests/summaryNoteTag/summaryNoteKind/summaryKeys` | 2.29.0 | — | **added** (`noteUserRequests` 0→5, `summaryUserRequests` 0→9) | STATIC | DROPPED (session-message payloads; no field in `crates/`) — cross-ref kas-core rows 31 |
| 11 | `_kiro/workflow/new` recipe-source gate: `{prefix:"_kiro/workflow/new: ",field:"workflowPath",notInForce:e=>"the workflow recipe at '<e>' is not available in this session's configuration."}`; `recipeInForce`/`inForce` 0→7/0→34 | 2.29.0 | session recipe scope/admission | **added** (error strings 0→2) | STATIC | NOT-SENT (cyril never resolves recipes; server-side text only) |
| 12 | `run_workflow` schema (`Itu`≡OLD `t4c`) and `save_workflow_definition` schema (`Xtu`≡OLD `_4c`) | 2.29.1 | tool pool gated by `settings.workflows` | unchanged (`save_workflow_definition` 10→10, `run_workflow` 45→45) | STATIC | HANDLED |
| 13 | `_kiro/session/notify` payload (`cAt`≡OLD `v0t`) | 2.29.1 | — | unchanged (`{sessionId,callerSessionId?,message,severity,sender,workflowId?,nodeId?,agentName?}`; literal 2→2) | STATIC | DROPPED (kas-core lane; unchanged) |
| 14 | `recipes_changed {sessionId,workspacePaths,recipes[]}` + recipe-catalog `stateRecipes` | 2.29.1 | **gate ON only** (`settings.value(Di.workflows)==="on"`; 2.28.0 LIVE confirmed 2× vs 0×) | unchanged (2→2) | STATIC | HANDLED |
| 15 | Auto-wake (`sendParentNote`, `parent_note`, `network-give-up`/`network-waiting`, `wakePolicy`, `pendingWakePrompts`, `deliverRuntimeNoteToParent`) | 2.29.1 | gate ON and OFF (2.28.0 LIVE) | unchanged literals (3→3, 3→3, 1→1/1→1, 4→4, 6→6); body unproven | STATIC | HANDLED |
| 16 | `_kiro/workflow/load` vs `/inspect` (subscription/persistence classification; both in the `transient`/method group) | 2.29.1 | — | unchanged classification (both in the OLD group at `11559637` and the NEW group at `11888711`, identical member lists); handler bodies unproven | STATIC | HANDLED |

### 3a. Workflow dark candidates

1. **Recipe includes.** A sixth node type `include` (`{type:"include", id:<slug>, path:<file>,
   inputs?}`) splices another recipe's steps in place of the node, expanding it to a `sequence`
   whose id is the include id and prefixing every included node id / artifact name with `<id>/`
   (`prefix:`${i.id}${Jho}``). Machinery: schema `oKc` (`.strict()`), cycle/limit validation
   (`include cycle`; "included recipes cannot contain include nodes"), `includeCount`, cap `Gho=50`,
   three resolver kinds (`bundled://<name>`, `generated://<id>`, file path), and a default resolver
   `Ube={resolve(){return Promise.resolve({ok:!1,error:"include paths cannot be resolved here"})}}`
   overridden by `includeResolverFor`. Counts: `includedFrom` 0→8, `includeResolver` 0→13,
   `inForce` 0→34, `recipeInForce` 0→7. **Unannounced in `feed-*.json`, absent from cyril's pinned
   schema, but documented in the bundled agent prompt**: "Puts another recipe's steps into this
   workflow in place of the node. Required: type, id, path. Optional: inputs." and "the include
   becomes a `sequence` whose id is the include id, and every included node id and artifact name
   gets the prefix `<include id>/`".
2. **`consentRound` propagation.** The persisted pending/interaction pair gains a round counter so
   a client can correlate the Nth re-prompt of the same tool call. New zod:
   `…consentRound:k.number().optional()` on both kinds, plus the round-aware dedup
   `uPt(e,t){return e.toolCallId===t.toolCallId&&e.consentRound===t.consentRound}`. OLD carried
   `consentRound` only in the `request_permission` meta. Directly relevant to **cyril-w55l** — the
   `allow_always` option builder `t0o`≡OLD `ouo` and `maxRounds` 6→6 are unchanged, so the loop is
   a client-side round-correlation gap.
3. **Recipe source "in force" gating.** KAS refuses a `_kiro/workflow/new` recipe whose source is
   not in the session's configuration ("not available in this session's configuration") and
   validates `injectOriginalUserRequest` homogeneously across includes. Host-side policy, no new
   wire field.

## 4. v2 (Rust) engine and launcher

**The visible CLI surface is frozen**: `--help-all` for all three binaries is byte-identical
(md5 `3a49e5a2…` launcher, `b551f29a…` chat, `58ee94de…` term) across 2.28.0/2.29.0/2.29.1; the
settings-key set is 119/119/119; the `_kiro.dev/*` method set, the 14 `TuiCommand` args structs, the
embedded doc manifest (164 documents / 37 `category:feature`, byte-identical) and the
Classic-deprecation wording (`classic` 52/52/52, `deprecated` 129/129/129, `sunset` 1/1/1) are all
unchanged.

| # | item | version | gate/default | direction | S/L | cyril verdict + `file:line` |
|---|---|---|---|---|---|---|
| 1 | Same `BUILD_HASH` `31e6bb3f…`, different binaries | 2.29.0 vs 2.29.1 | n/a (build stamp) | change | STATIC | N/A — cyril gates on semver only, `protocol/kas/version.rs:46,271-274` |
| 2 | `--auth-method` `client` variant + `(default)` on `cli` | 2.29.0 | acp only with `--agent-engine=v3`; default `cli` | add | STATIC | **HANDLED (cli)** — `cyril-workbench/src/reviewer/runtime.rs:231`; **NOT-SENT (client)** — no `client` literal in `crates/`, though the callback responder exists `protocol/kas/auth.rs:24,300` |
| 3 | `AUTH_METHOD` help rewritten ("Defaults to \"cli\"… use \"client\" to answer the engine's access-token requests from the ACP client") | 2.29.0 | n/a | change | STATIC | N/A — cyril parses no kiro help text |
| 4 | env `KIRO_CLI_IS_CHILD_PROCESS` | 2.29.0 | unset default; chat-cli only | add | STATIC | **NOT-SENT** — 0 hits in `crates/`; cyril's env set is HOME/USERPROFILE/KIRO_HOME/XDG_*/TMP* (`runtime.rs:216-225`) + `KIRO_KAS_SERVER_PATH`/`KIRO_AGENT_PATH` (`discovery.rs:415,421`) |
| 5 | rollout `standalone_toolbox` | 2.29.0 | `treatment_percent=0` (0%), segment=all, +1 cohort-override entry (digest `8f95b89bc47f4a04`) | add | STATIC | NOT-SENT (cyril reads no rollout table) |
| 6 | rollout `memory`, `memory_controls` | 2.29.0 | internal → **all** (GA) | change | STATIC | KAS lane's feature; cyril has no table |
| 7 | rollout rows 2.29.0→2.29.1 | 2.29.1 | — | none (0 row deltas, overrides unchanged) | STATIC | N/A |
| 8 | hidden `_ cleanup-autocomplete-artifacts` | 2.29.0 | hidden ("Hidden helpers run by installers. Not for direct use") | add (0→4) | STATIC | NOT-SENT |
| 9 | hidden `_ standalone-cohort` | 2.29.0 | hidden ("Print this user's standalone cohort as one JSON line") | add (0→2) | STATIC | NOT-SENT |
| 10 | `acp` help sentence: "Accepts and ignores the autocomplete bundle's `init` calls…" | 2.29.0 | n/a | add | STATIC | NOT-SENT (cyril never sends `init`) |
| 11 | **`agent upgrade <DIR>`** (the folded 2.28.1 line) | 2.29.0 | visible; `--no-backup`, `--dry-run`, `-v` | add | STATIC | NOT-SENT (0 hits) |
| 12 | new source paths (chat) | 2.29.0 | — | add | STATIC | NOT-SENT |
| 13 | `tool_tags` in the V2 hook payload + telemetry dimension | 2.29.0 | V2 TUI hooks only | add (0→3) | STATIC | **DROPPED** — cyril's hook surface is the KAS registry `protocol/kas/hooks.rs:25-42`; payload hooks are the host's |
| 14 | Classic `fs_write` symlink auto-approve fix | 2.29.0 | classic engine only | change | STATIC (logic) / LIVE (claim) | **DROPPED** — engine-internal; cyril only converts the resulting permission frames `convert/mod.rs:240,291` |
| 15 | model fallback applies without restart | 2.29.0 | n/a | change | STATIC | **HANDLED** `convert/kiro.rs:1209` (`model/not_found` + `fallbackModel`), `:1168` (`agent/not_found` + `fallbackAgent`); `chat/cli/model.rs` −40 lines |
| 16 | MCP OAuth `oauthUrl` surfaced (2.29.0) + issuer fix (2.29.1) | both | n/a | add / fix | STATIC | **HANDLED** `kiro.rs:1123-1143` → `Notification::McpOAuthRequest { server_name, url }`; issuer fix is engine-side |
| 17 | knowledge-search limits across bases | 2.29.0 | n/a | fix | STATIC | NOT-SENT — cyril embeds no semantic-search client |
| 18 | settings key set | both | 119 keys | none | STATIC | N/A (`settings.log`: 119/119/119, 0 added/removed) |
| 19 | `_kiro.dev/*` + `_kiro/*` method set | both | — | none | STATIC | N/A — trap-3 set diff identical, incl. `_kiro/auth/getAccessToken`, `_kiro/session/delete` (present in 2.28.0 too) |
| 20 | `TuiCommand` args (`*Args` structs) | both | — | none | STATIC | N/A (14 structs, identical) |
| 21 | doc manifest | both | — | none | STATIC | N/A (164 docs / 37 `category:feature`, byte-identical) |
| 22 | Classic deprecation wording | both | unchanged | none | STATIC | N/A (`classic` 52/52/52; `deprecated` 129/129/129; `sunset` 1/1/1) |
| 23 | serde shape churn (host) | 2.29.0 / 2.29.1 | — | change | STATIC | NOT-SENT (internal session-state/AWS-SDK shapes; no ACP wire surface) |
| 24 | launcher `crates/fig_request/src/reqwest_client.rs` +1 tracing call | 2.29.0 → reverted 2.29.1 | — | change | STATIC | NOT-SENT |

### 4a. Host detail for the non-obvious rows

**Same-BUILD_HASH (row 1).** `BUILD-INFO` per version:

    2.28.0 BUILD_DATE=2026-10-05T23:47:51.564044+00:00 BUILD_HASH=877b466eda188c6dc44136df230f7e193a4edbfa
    2.29.0 BUILD_DATE=2026-10-09T03:12:17.353320+00:00 BUILD_HASH=31e6bb3fad4a1fff9ba23aee436cef61c8579a58
    2.29.1 BUILD_DATE=2026-10-10T03:23:23.018134+00:00 BUILD_HASH=31e6bb3fad4a1fff9ba23aee436cef61c8579a58

Two builds one day apart share the hash while everything else moved: sizes (chat +214,848),
ELF sections, source-level call-site counts, a new source path, and reordered `.rodata` literals.
The payloads (node ELF sha `81925c09…`, bun ELF sha `a83d2637…`) are byte-identical in all three
builds, so the differing bytes are the host Rust code plus the version/`BUILD_DATE` strings.
**`BUILD_HASH` is a build-configuration/release-train stamp, not a digest of the produced
artifact** [INFERENCE].

**`agent upgrade` (row 11).** `kiro-cli-chat agent upgrade --help` on the archived binaries: 2.28.0
→ `error: unrecognized subcommand 'upgrade'` (tip: `migrate`); 2.29.0/2.29.1 → full help: "Upgrade
agent configs in a directory to the universal format both engines load. Rewrites top-level \*.json
files in place (subdirectories are not searched); already-universal, V3-only, out-of-sync, and
externally managed agents are left untouched", args `<DIR>` + `--no-backup` + `--dry-run`.
Implementation: new `crates/chat-cli/src/cli/agent/upgrade.rs`; registration in
`root_command_args.rs` (+23 lines, 108→131); `cli/agent/mod.rs` (+1);
`crates/agent/src/agent/agent_config/migration/io.rs` (+23). Agent-side literals (2.29.x only)
refuse symlinked configs ("would fail: config is a symlink; the linked target would be modified in
place"), skip "already universal" / "not an agent config" / "managed externally" / "symlink to a
file outside the directory" / "not a regular file" / "out of sync (not rebuilt)", refuse when
"too many existing backups", and print "Upgrading agents in" / "Dry run … no files written" /
"Originals backed up next to each upgraded file". The identical migration machinery explains the
`/upgrade-agent` line; that slash-command half is V3, not host.

**Doc manifest (row 21).** Byte-identical across all three versions — 164 documents, 37
`category:feature`, 0 added/removed/changed (embed stamps `2026-09-02T18:38:18.266523` (103 docs)
and `2026-09-28T19:09:06.985120` (140 docs)). The standing trap-12 check against
`https://kiro.dev/sitemap.xml` (1,192 `<loc>`) finds 26 of 37 feature nodes whose file stem appears
in **no** sitemap path (`voice-mode`, `trust-configuration`, `paste-chips`, `mcp-registry`,
`planning-agent`, `turn-usage-summary`, …) — but the count is **26/26/26**, i.e. pre-existing URL
consolidation, not this cycle's finding. The sitemap does carry `/docs/cli/v3/` and `/docs/acp/`
(lastmod 2026-10-09) with no matching manifest node.

**Hidden helper roster** (chat binary): `upgrade-agent` (7/7/7), `refresh-feed`, `get-kas-token`,
`get-telemetry-credentials`, `test-seed-v1`, `import-session`, `export-session`,
`windows-sandbox`, `windows-kas-stop`, `acp-client`, `set-default`, `api-key-lineage`,
`diagnostics`, `print-system-prompt`. Only `cleanup-autocomplete-artifacts` (0→4) and
`standalone-cohort` (0→2) are new.

## 5. What Kiro's own TUI now sends and consumes (reference client)

**The TUI method surface is frozen**: the `_kiro/*` + `kiro.dev/*` literal family is 92==92, the
gates-script method table 113==113, the outbound call literals 23==23, the slash-command names
74==74 — added ∅, removed ∅ (and `/memories` already existed in 2.28.0, ×10==×10). The whole TUI
delta is **44 added / 3 removed string literals**. The `initialize` `clientMeta` object is
byte-identical modulo renames: `{telemetryEnabled, telemetry?, knowledge:!0,
hooks:{enabled:!0,v2:!0}, requirementsAnalysis:!0, specPhaseCheckpoints:!0, streamingShellContent:!0,
infrastructureSafety?, settings?}`.

| # | item | version | gate/default | direction | S/L | cyril verdict + `file:line` |
|---|---|---|---|---|---|---|
| D1 | `_meta.kiro.settings.memory` = `{mode,reflection}` sent **always** | 2.29.0 | none (2.28.0 required rollout `memory_controls`) | outbound settings | STATIC | **NOT-SENT** — `protocol/kas/settings.rs:109` has no `memory` key |
| D2 | `settings.memoryEnable={enabled:true}` narrowed | 2.29.0 | `Fn("memory") && process.env.KIRO_INTERNAL==="1" && mode!=="disabled"` (2.28.0: `Fn("memory")` only) | outbound settings | STATIC | **NOT-SENT** — `settings.rs:78-98` |
| D3 | `settings.userMemoryOptIn={enabled: mode==="read_write"&&reflection}` computed; also sent when `mode==="disabled"` | 2.29.0 | `!Fn("memory_controls") \|\| mode==="disabled"` (2.28.0: flat `cli.json memory.enabled` row) | outbound settings | STATIC | **NOT-SENT** — `settings.rs:78-98`; the 2.28.0 row cyril-q8gq asks for no longer exists |
| D4 | default memory mode/reflection source | 2.29.0 | `KIRO_INTERNAL ? "read_write" : "disabled"` (2.28.0 froze `{mode:"read_write",reflection:!0}`) | outbound settings | STATIC | **N/A** (cyril derives no memory default) — `settings.rs:109` |
| D5 | flat row `["memory.enabled","userMemoryOptIn"]` removed from the bool table | 2.29.0 | 2.28.0-only (`Pn("memory_controls")` guard gone) | outbound settings table | STATIC | **cyril still maps it** — `settings.rs:78-88` (cyril-q8gq is now the opposite direction and stale) |
| D6 | `session/new` request `_meta.kiro.overrideSessionId` = `sess_<uuid>` | 2.29.0 | only when the wrapper is called with `{onSessionUpdate}` (no in-bundle caller found) | outbound request meta | STATIC | **NOT-SENT** — `domain_mediator/commands/session.rs:50-52` sends no `_meta` |
| D7 | new consumption of `session_info_update._meta.kiro.kind:"interaction_resolved"` → abort the matching pending `session/request_permission` | 2.29.0 | `agentCapabilities._meta.kiro.replayMarking===!0` | inbound consumption (permission lifecycle) | STATIC | **DROPPED** — `convert/kas.rs:352` (no arm) via `protocol/engine.rs:329-330` |
| D8 | `sessionUpdate()` settles open permission requests (replay-safe: `_meta.kiro.replay===!0` frames ignored) | 2.29.0 | `replayMarking` | inbound plumbing | STATIC | **DROPPED** (same arm as D7) |
| D9 | KAS-spawn env `KIRO_CLI_IS_CHILD_PROCESS="1"` | 2.29.0 | unconditional in the engine-launch env map | spawn env | STATIC | **NOT-SENT** — `protocol/kas/version.rs` / engine launch |
| D10 | `/upgrade-agent` AIM-managed report: `reasonCode:"cli_only_agent" && externallyManaged` → "managed by AIM…" | 2.28.1/2.29.0 | `externallyManaged` derived **locally** from the agent config's `description` | TUI-local rendering | STATIC | **N/A** (not ACP). KAS has 0 hits for `externallyManaged`/`skippedManaged` in both carves |
| D11 | MCP-OAuth/copy-url notice: `{message, success:!0, persistent:!0, linkUrl}` (2.28.0: `success:!1`) and `case"system_notice"` renders `linkUrl` | 2.29.0 | TUI-local (`linkUrl` 0→11; KAS 0 hits both) | TUI-local event | STATIC | **HANDLED at the ACP input** — `convert/kiro.rs:1134`; `linkUrl` never crosses the wire |
| D12 | rollout gate names the TUI checks | 2.29.0 | **+`memory`** (8 → 9) | client rollout gate | STATIC | **N/A** (client-side; registry rows are host lane) |
| D13 | `/memories` panel gate `e==="kas"&&!t&&Fn("memory_controls")`; default now `m9(cli.json settings, Js())` where `Js(){return process.env.KIRO_INTERNAL==="1"}` | 2.29.0 | `memory_controls` | client UI | STATIC | **NOT-SENT** (panel-only; the wire part is D1–D4) |
| D14 | MCP-sandboxing UI (`mcpSandboxing`, "…policy keeps MCP sandboxing on.", "Also isolate stdio MCP server processes") + `kiro_cli_sandbox_session_outcome_total` telemetry | 2.29.0 | TUI-local | client UI/telemetry | STATIC | **N/A** — KAS owns sandbox/MCP-env behaviour (kas-core) |
| D15 | `--auth-method=client` (changelog line) | 2.29.0 | — | host/launch flag | STATIC | **not in this lane** — `--auth-method` 0 hits in both bundles; `auth-method` 1==1 is an ispell word-list entry; `authMethod` 4==4 is the ACP `GetAccessTokenResponse` field |
| D16 | V2 hook-matcher widening + `tool_tags` on V2 tool hooks | 2.29.0 | — | host/V2 engine | STATIC | **not in this lane** — `tool_tags`/`toolTags` 0 hits in both bundles; the only TUI trace is `/upgrade-agent` report text |
| D17 | effort selector at session start | 2.29.0 | — | engine advert | STATIC | **no TUI delta** — every effort token count equal, plumbing byte-equivalent modulo renames; KAS `effortLevels` 29→33 |
| D18 | duplicate `127.0.0.1` loopback helper (`Ptt`/`URn` == `J5n`/`pwt`) | 2.29.0 | — | client misc | STATIC | **N/A** cosmetic duplication (2 → 3 occurrences) |
| D19 | `replayMarking` was already parsed+used in 2.28.0 (workflow child-session replay buffering, 14 hits) | 2.22–2.28 | — | client capability | STATIC | cyril does not parse it either — `replayMarking` 0 hits in `crates/**/*.rs` |

Everything else in the 44-literal delta is TUI chrome: `/upgrade-agent` report strings,
`components.contextBreakdown.memory` (i18n), AIM/`skippedManaged`, `clearTranscriptForShellClear`,
`agentMarksReplay`, `sandbox_backend`/`sandbox_fallback_reason`/`sandbox_session_outcome`,
`not_applied`/`not_applicable`/`user_bypass`/`fallback_unsandboxed`/`backend_unavailable`.

### 5a. TUI negative results (equally load-bearing)

| check | 2.28.0 | 2.29.1 | evidence |
|---|---|---|---|
| `_kiro/*` + `_kiro.dev/*` + `kiro.dev/*` method literals | 92 | 92 | added ∅ removed ∅ |
| method list (gates script `methodKeys`, wider table incl. core ACP names) | 113 | 113 | `gates-2.28.0-2.29.1.txt` |
| slash-command name literals | 74 | 74 | symdiff ∅ |
| `initialize.clientCapabilities._meta.kiro` clientMeta object | — | — | byte-identical modulo renames |
| KAS spawn argv | `[prefix,"--experimental-wasm-modules",server,"--transport=stdio","--auth=acp-callback",…]` | same | quote in lane file |
| KAS spawn env set (gates) | `KIRO_CUSTOM_USER_AGENT, NODE_CHANNEL_FD, NODE_CHANNEL_SERIALIZATION_MODE, NODE_USE_SYSTEM_CA` | same | `gates-*.txt` |
| capability keys advertised (`getAccessToken,userInput,copyUrlToClipboard,secretStorage`) | 4 | 4 | bodies identical |
| `session/prompt` `_meta.kiro` keys (`displayText`,`outputStyle`) | 2 | 2 | `persistDisplayText` sites identical |
| permission `_meta` (`provenance` 1, `consentContext` 28, `outcome:"selected"` 6) | equal | equal | rename only |
| `subagentOrchestration` in the TUI settings | **0** | **0** | cyril still sends it (`settings.rs:93-98`) — divergence stands; cyril-53qx says keep |
| session_info_update main mapper kinds + silent set `{background_processes, background_process_event}` | equal | equal | identical modulo renames |
| `memory_config_update` observer, `memoryReflection` config option, `mode` config option | present | present | identical modulo renames |

### 5b. TUI outbound-vs-cyril parity (the check a version diff cannot make)

Extracting every literal passed to the TUI's ext helpers gives 23 outbound calls, byte-identical
across the two bundles. cyril sends three of them plus the workflow family.

| TUI outbound literal | cyril sender | verdict |
|---|---|---|
| `_kiro/account/getUsage` | `domain_mediator/commands/extensions.rs:262` | HANDLED |
| `kiro.dev/settings/list` | `domain_mediator/commands/extensions.rs:216` | HANDLED |
| `kiro.dev/session/terminate` | `domain_mediator/commands/subagents.rs:60` | HANDLED |
| `_kiro/hooks/list` | answered, not called — `protocol/kas/hooks.rs` (responder) | reversed direction |
| `_kiro/session/context` | only a `#[cfg(test)]` near-miss at `protocol/engine.rs:777` | NOT-SENT |
| `_kiro/session/compact` | no sender (it is a slash-command name, `commands/mod.rs:1153`) | NOT-SENT |
| `_kiro/session/history` | no sender | NOT-SENT (cyril-a3th) |
| `_kiro/memory/list`, `_kiro/memory/get` | no sender | NOT-SENT (cyril-0na7) |
| `_kiro/session/rename`, `_kiro/session/delete` | no sender | NOT-SENT |
| `_kiro/config/template` | no sender | NOT-SENT |
| `_kiro/sourceProviders/list`, `…/listResources` | no sender | NOT-SENT |
| `_kiro/mcp/resetServer` | no sender | NOT-SENT |
| `_kiro/spec/resolveSession`, `_kiro/spec/invoke` | no sender (no `_kiro/spec/*` anywhere in `crates/`) | NOT-SENT |
| `_kiro/codeIntelligence` | only the setting `chat.enableCodeIntelligence` (`kas/settings.rs:81`) | NOT-SENT |
| `_kiro/knowledge` | advertised in `fingerprint.rs:138`, never called | NOT-SENT |
| `_kiro/policy/ignore_files_changed` (notification) | no sender | NOT-SENT |
| `_kiro/powers/refresh` (notification) | **deliberately unused** — `convert/kas/powers.rs:29-31` | NOT-SENT (by design) |
| `kiro.dev/session/list` | no sender | NOT-SENT |
| `kiro.dev/settings/set` | no sender (cyril reads settings, never writes them) | NOT-SENT |

The workflow family (reached through a dedicated client, so not in the 23):
`_kiro/workflow/{new,invoke,list,load,inspect,cancel,pause,resume,retry,…}` — cyril covers all but
`pause` (`cyril-jxw3`/`4u4a`). **None of this is a 2.29.x delta.**

Reverse direction: cyril answers 19 requests (`protocol/kas/callbacks.rs:531` `WIRE_METHODS`); the
2.29.1 TUI advertises and answers more — `secretStorage`→`_kiro/secret/{get,store,delete}`,
`userInput`→`_kiro/userInput`, `openExternalUrl`→`_kiro/openExternalUrl`, plus
`copyUrlToClipboard`. The literal-absence test is a lead, not a proof: the only dynamic families in
cyril are `_kiro/workflow/{…}` and `_kiro/powers/{…}`; no `format!`/`strip_prefix` builds a
`_iro/session`, `_iro/memory`, `_iro/secret`, `_iro/spec`, `_iro/sandbox` or `kiro.dev/` name.

### 5c. TUI `_meta.kiro.settings` keys vs cyril

★ = new or newly-shaped in 2.29.1. cyril's marshaller is `protocol/kas/settings.rs:109-179`.

| key | TUI literal | cyril |
|---|---|---|
| ★ `memory` | `t.memory=s` (`m9`: `{mode:"disabled"\|"read_only"\|"read_write", reflection:bool}`) | not emitted |
| ★ `memoryEnable` | `t.memoryEnable={enabled:!0}` | not emitted |
| ★ `userMemoryOptIn` | `t.userMemoryOptIn={enabled:s.mode==="read_write"&&s.reflection}` | not emitted (and the dropped 2.28.0 flat row is absent from cyril too) |
| `largeToolOutputHandler` | `["chat.enableLargeToolOutputHandler","largeToolOutputHandler"]` + default `!0` | **not emitted** — cyril maps `chat.enableTodoList→todoList` instead (a dead KAS key) |
| `c2s` | `if(Fn("c2s"))t.c2s={enabled:e[rt.CHAT_ENABLE_C2S]===!0}` | not emitted |
| `workflowNotifications` | `t.workflowNotifications={enabled:n.enabled,delivery:d2(…)}` | not emitted |
| `backgroundExecution` | `PTn=[["background_execution","backgroundExecution"]]` → `{enabled:!0}` | not emitted |
| `workflows`, `goal` | `GTn(t,n){if(n.available)e.workflows={enabled:t.enabled},e.goal={enabled:t.enabled}}` | deliberately **never** emitted (ADR-0011; fenced by `settings.rs:466-487`) |
| `infraSafetyMonitor`, `infraSafetyEnforce` | `if(process.env.KIRO_INFRA_SAFETY_ROLLOUT_ENABLED==="1")…` | not emitted (env-gated dark) |
| (`todoList`) | **absent from the 2.29.1 TUI table** | cyril still emits it (`settings.rs:82`) — stale, cyril-53qx |

Also TUI-sent and cyril-absent: `session/new` `_meta.kiro.overrideSessionId` (D6) and the KAS-spawn
env `KIRO_CLI_IS_CHILD_PROCESS="1"` (D9). Conversely cyril sends
`subagentOrchestration:{enabled:true}` (`settings.rs:93-98`) which the 2.29.1 TUI does **not** —
KAS 0.66.33 no longer accepts it either (kas-core row 19).

## 6. Unannounced or dark features spotted

1. **Recipe includes** (`include` node + `includedFrom`) — in no changelog, in no cyril doc,
   documented only in KAS's bundled agent prompt. Evidence in §3a.
2. **`consentRound` propagation** — the fix mechanism for an open cyril bug (cyril-w55l). §3a.
3. **Model fallback chain** — `model_fallback` kind, `modelFallbackReporting`, flags
   `model_fallback_refusal`/`model_fallback_availability` (both default `false`, env-reachable),
   `server_cascade_config` (default `""`, no env, server-pushable), reason literals
   (`chainExhausted`, `noChainConfigured`, `winnerChanged`, `explicitPick`,
   `explicitPickDuringWrite`, `sameLayer`, `notEnabled`, `noServedTarget`, `staleActivityStatus`,
   `refusalFallback`) and a `source: server_cascade_config|defaults|env` resolver. Dormant unless
   the backend pushes the config.
4. **Configuration layers, pull side** — `_kiro/configuration/getState` advertised in
   `extensionMethods` (never called by cyril); `declaresInactiveItems` requires a literal `true`;
   `resourcesFor` filters to `standing.kind==="active"` otherwise; opt-in is per-client from that
   client's handshake.
5. **`vibeToDefaultV2Routing`** — hidden setting + AB flag (default false, **no env**):
   `Q0t(e,t){return e==="vibe"&&t.vibeToDefaultV2Routing?{modeId:RO,routedFrom:e}:{modeId:e}}`.
   No client-set path.
6. **`skill_name_qualifiers`** — default false, env `KIRO_FEATURE_SKILL_NAME_QUALIFIERS_ENABLED`,
   AB `SKILL_NAME_QUALIFIERS`, wired to the session setting `skillNameQualifiers` alongside
   `validation`; no client-set path.
7. **`standalone_toolbox`** — rollout row at `treatment_percent=0`, segment `all`, with one live
   cohort-override entry (digest `8f95b89bc47f4a04`). Evidence text: "Autocomplete decoupling
   cohort: the bundled Toolbox post-install hook asks this flag whether to move a user onto the
   standalone channel. It does not gate update routing … Starts at 0%; ramp `treatment_percent` in
   later releases." Client machinery shipped in the same release: hidden
   `_ cleanup-autocomplete-artifacts` (0→4), hidden `_ standalone-cohort` (0→2), the `acp` help
   line about ignoring the autocomplete bundle's `init` calls, and
   `crates/chat-cli/src/cli/internal.rs` (new). Live: the helper is present **only** in
   `kiro-cli-chat` (`strings` 0 hits in the launcher, 4 in the chat binary; `_ cleanup-autocomplete-artifacts --dry-run`
   reports `would remove: …/ssh_inner`), so it is new in 2.29.1 and dark — no changelog line mentions it.
8. **2.29.1 ships host code beyond its feed line** — new module
   `crates/chat-cli/src/launch/notification_order.rs` (only in 2.29.1, no tracing call sites) is
   unmentioned by the changelog.
9. **Image-fitting placeholder texts** — three compiled-in strings
   (`[image omitted: the selected model does not read images]`, `…could not be reduced to the
   selected model's size limit`, `…could not be decoded`) that replace a hard request failure with
   an in-conversation note. `sharp` moved to a devDependency.
10. **MCP sandboxing UI** (TUI) — `mcpSandboxing`, "Global/Project policy keeps MCP sandboxing on.",
    "Also isolate stdio MCP server processes", `kiro_cli_sandbox_session_outcome_total` (0→1).
11. **`_kiro/openExternalUrl` link-notice UX** — "[copy-url] Copied URL to clipboard and showed
    it", `addLinkNotice` 0→7; `linkUrl` never crosses the wire.
12. **Replay-aware permission cancel** (TUI) — gated on `agentCapabilities._meta.kiro.replayMarking`;
    `largeToolOutputHandler` is 0 hits in the KAS bundles but present as a TUI settings row.
13. **`local_sandbox` rollout** still env-forced (`KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`), identical
    in both bundles.
14. **`memory` / `memory_controls` are no longer dark** — both went `internal` → `all` (GA) in
    2.29.0.

## 7. Live results (KAS 0.66.26 vs 0.66.33, same day)

Same-day, strictly serial, paired runs of both KAS carves with one driver copy
(`probe-kas-live-2.29.1.py`: the 2.28.0 driver with the handshake moved to
`initialize.clientCapabilities._meta.kiro.settings` per trap 5 and `getState`/`memory/get` added to
the census). Every spawn proved by `/proc/<pid>/exe` plus the KAS log line
`kas.server.starting {"version":"0.66.26"|"0.66.33"}`. Fresh temp `HOME`, real `XDG_DATA_HOME`, no
`login`/`logout`, captures redacted. Frame index = 1-based line in the named `.jsonl`.

| leg | captures | result |
|---|---|---|
| 0 smoke/`--help` | `tmp/help-{2.28.0,2.29.1}-*` | `acp-server.js` has no help surface (ignores argv, starts normally). NEW reads the recipe catalog at boot for the `user` **and `cloud`** tiers (`fs.workflow.recipes.readDir.ok`, `recipeCatalog.refreshShared.ok {"trigger":"startup"}`) — OLD emits none |
| 0b init auth matrix | `kas-init-{2.28.0,2.29.1}-{callback,none}.jsonl` | initialize result byte-identical between `--auth=acp-callback` and no `--auth`; the only OLD→NEW deltas are `modelFallbackReporting:true` and `extensionMethods += _kiro/configuration/getState` (26→27). In callback mode `_kiro/auth/getAccessToken` is invoked at initialize, before any prompt |
| 1 `SCENARIO=sweep` | `kas-live-sweep-{2.28.0,2.29.1}.jsonl` (165 / 183 frames) | `getState` implemented; `consentRound` on the interaction kinds; `tool_call._meta.kiro.resources[]`; **steering slash commands gone** |
| 2 workflow gate off/on | `kas-workflow-gate{off,on}-{2.28.0,2.29.1}.jsonl` | control plane **live-identical** — gate-on all 10 workflow names match event-for-event, gate-off all 6; `recipes_changed` 0/0 off and 1/1 on; 18 recipes both |
| 2b W-series | `kas-wseries-{2.28.0,2.29.1}.jsonl` | `include` accepted on NEW / rejected `invalid_union_discriminator` on OLD; dedicated include-cycle detector; **20× re-ask + `failed` → 1× ask + `completed`** |
| 3 leads + T-series | `kas-lead-*.jsonl`, `kas-t-*-{2.28.0,2.29.1}.jsonl` | L2/L6/L7/L8 confirmed, L1/L3/L4/L5 unproven, `largeToolOutputHandler` refuted; T-memory/T-override IDENTICAL; T-interaction differs only by `consentRound` |
| 4 `sweep-new-fields.py --diff` | `sweep-new-fields-diff-{sweep,wfoff,wfon}.txt` | sweep: 679 NEW vs 618 OLD paths, 62 only-NEW / 1 only-OLD; the single "removed" path is the trap-11 artifact of the steering-command difference |
| 5 host/CLI + v2 pair + telemetry | `v2-acpmethod-{client,cli}.jsonl`, `v2-live-sweep-{2.28.0,2.29.1}.jsonl`, `kas-telemetry-{2.28.0,2.29.1}.jsonl`, `tmp/h3-*.log`, `tmp/h4b-*.log` | H1 confirmed at the wire level; H3 confirmed; H4 **present in `kiro-cli-chat`** (the first pass asked the launcher — wrong binary); v2 sweep IDENTICAL (313/313 paths); H5 UNPROVEN; telemetry dials unchanged |

**The added wire surface, live.** `_kiro/configuration/getState` answers (OLD returned `-32603
[PersistenceClassification] Ext method "_kiro/configuration/getState" has no persistence
classification`); the pull is **ungated** while `configurationState:true` gates only the push
(control: push 0/0 without it, 1/1 with). Connection view = 9 layers, session/workspace = 12; the
session view is the only one with settings (`workflow: "off"`); `resources[]` = 3 typed bundles
(steering/agent/workflow), 22 items, each `{krn, layer, standing:{kind:"active"}, value}`. The
`inactiveItems` capability produced **no observable difference** — this workspace has no
non-active items (22/22 active), so its filter is UNPROVEN, not contradicted.

**cyril's settings handshake is live-dead for two keys.** At `KIRO_LOG_LEVEL=debug`,
`initialize.clientCapabilities._meta.kiro.settings = {"subagentOrchestration":…,"todoList":…}`
(what cyril sends) logs exactly:

    configuration.intake.unknown-key {"key":"setting/subagentOrchestration","layer":"client-connection"}
    configuration.intake.unknown-key {"key":"setting/todoList","layer":"client-connection"}

and the control arm (`{"thinking":{"enabled":true}}`) logs nothing. The info-level run showed 0 hits
— a log-level artifact worth remembering.

**The workflow 20×-re-ask bug is fixed upstream.** Identical client answer
(`{"outcome":{"selected"},"optionId":"always-accept"}`), same single-node recipe: 0.66.26 issues
`session/request_permission` **20×** with `consentRound` absent on the interaction kinds and the node
ends `failed` ("blocked by the approval flow and never executed"); 0.66.33 issues it **1×**, carries
`consentRound:1` on `request_permission` + `pending_interaction` + `interaction_resolved`, and the node
completes. The correlation is captured; attributing it specifically to `consentRound` is [INFERENCE].

**Steering slash commands disappear (regression).** Same driver, same settle windows, both frames of
each run: 0.66.26 advertises `architecture-selection, quick-spec, bug-fix, general-task-execution,
context-gatherer` (5); 0.66.33 advertises `general-task-execution, context-gatherer` (2). The three
`_meta.kiro.type:"steering"` commands (`contextQuery`) are gone from `available_commands_update`
while the same three docs still arrive in `_kiro/steering/documents_changed` and `getState` lists them
`standing.kind:"active"`. cyril consumes `available_commands_update` generically
(`convert/mod.rs:491`) and drops the `_meta`, so the user-visible effect is identical: `/quick-spec`,
`/bug-fix`, `/architecture-selection` vanish on 2.29.1. Same-day/different-binary ⇒ attributed to the
binary (trap 1). **Re-confirmed on a second, fresh single-writer pair** (nothing else running, one
clean run per engine): 5 vs 2, `_meta.kiro.type` =
`[steering,steering,steering,custom-agent,custom-agent]` vs `[custom-agent,custom-agent]`, uniform
across **both** `available_commands_update` frames (15 and 23) and every other 0.66.33 capture in the
lane. The feature is alive — `_kiro/steering/documents_changed` delivers the identical three-doc set
(`inclusion:"manual"`, `uri:"builtin:…"`) on both engines and `getState` lists all three
`standing.kind:"active"` — and the surviving `general-task-execution._meta.kiro` is byte-identical, so
this is a **registration loss, not a schema change** (the `user`/`cloud` startup reads are the recipe
catalog, a different subsystem). The `sweep-new-fields` "removed"
`availableCommands[]._meta.kiro.contextQuery` is a trap-11 array-walk artifact of this same difference.

**Host legs.** H1 (`--auth-method`): `kiro-cli acp --auth-method client` produced **32**
`_kiro/auth/getAccessToken` requests to the ACP client; `--auth-method cli` produced **0**. Ownership
confirmed; end-to-end client-mode success UNPROVEN (the probe recorded no replies and the turn still
reported "not signed in"). H3 (`agent upgrade <dir>`): dry-run prints per-file decisions and writes
nothing; the real run upgrades only the legacy file, prints
`⚠ widened-hook-matcher  hooks: Hook 'preToolUse-0' matcher fs_read -> read now also matches grep,
glob, code, introspect, tool_search`, skips the symlink and the non-agent JSON, leaves the universal
config unchanged, and backs the original up as `.json.bak` — the only live proof of the folded 2.28.1
line. **H4 CORRECTED — the helper exists; the first probe asked the wrong binary.**
`kiro-cli _ cleanup-autocomplete-artifacts --dry-run` fails on the **launcher**
(`error: unrecognized subcommand 'cleanup-autocomplete-artifacts'`; the launcher's `internal`/`_`
namespace is the fig/shell set, 25 subcommands, no `upgrade-agent` either), but
`kiro-cli-chat _ cleanup-autocomplete-artifacts --dry-run` **succeeds** and prints
`would remove: ~/.local/share/kiro-cli/ssh_inner` / `Dry run; nothing was removed.` The chat binary
has no `internal` name at all (`unrecognized subcommand 'internal'`, tip `init`) — its hidden
namespace is the `_` alias. The first pass's "absent in 2.29.1" was a wrong-binary artifact, the same
class as the PATH trap (traps 8).

**Confirmed negatives.** `tool_tags` is not on the ACP wire (0 hits in every capture). The 2.28.0
audit's `largeToolOutputHandler` initialize-setting hypothesis is **refuted**: it logs
`configuration.intake.unknown-key` on 0.66.26 *and* 0.66.33, and it is not a cyril key (0 hits in
`crates/**/*.rs`) — so it was never a working positive control.

**v2 host pair (the legacy path, same-day).** First attempt voided (the driver was invoked from the
lane directory while the shared probe lives one level up — both legs produced empty captures); re-run
with the archive `bin/` prepended to `PATH`, the capture's `proc_tree` proving both legs ran the
archived binaries (`2.28.0` / `2.29.1`). `sweep-new-fields.py --diff`:
**313 paths each, 0 only-NEW / 0 only-OLD — IDENTICAL field sets, no shape change.** Both legs
complete the same three turns with the same outcomes (`p1_read` `end_turn` 7.8 s, `p2_shell` `end_turn`
7.6 s, `p3_after_bogus_model` `-32603 Encountered an error in the response stream`). **H5** (the
changelog's "model fallback changes take effect in the current session without restarting"): a bogus
model set mid-session fails identically on both engines — **UNPROVEN**, it needs a served fallback
chain, exactly like the KAS-side L1. **H6** (MCP OAuth URL over SSH/tmux) was not run: no
OAuth-requiring MCP server was available in the lane.

**Telemetry (dial-vs-suppression).** Three arms × two engines with an in-process CONNECT-logging
proxy (`default`, `KIRO_FEATURE_KUTS_TELEMETRY_ENABLED=false`, `init_telemetryEnabled:false`):
**zero `telemetry*.kiro.dev` dials in every arm on BOTH engines**, while the same arms do show the
`management`/`runtime` dials (the proxy is not blind). `clientCapabilities._meta.kiro.telemetryEnabled`
is on the wire in every initialize on both engines. The v3 line's surface is the host/TUI, not the
standalone KAS process.

### 7a. Live delta rows

| item | version | gate/default | direction | S/L | cyril verdict + `file:line` |
|---|---|---|---|---|---|
| `_kiro/configuration/getState` implemented (was `-32603`) | 2.29.0 | pull ungated; push gated by `configurationState:true` | ADD | LIVE | **NOT-SENT** — 0 call sites in `crates/cyril-core/src` |
| `extensionMethods` += `_kiro/configuration/getState` (26→27) | 2.29.0 | default | ADD | LIVE | n/a (cyril never reads the list) |
| `agentCapabilities._meta.kiro.modelFallbackReporting:true` | 2.29.0 | default | ADD | LIVE | **NOT-SENT** — 0 hits in `crates/**/*.rs` |
| `consentRound` on `pending_interaction`/`interaction_resolved` `_meta.kiro` | 2.29.0 | default | ADD | LIVE | **DROPPED** — `convert/kas.rs:525` |
| workflow-step approval: 20× re-ask + `node_complete:failed` → 1× ask + `completed` | 2.29.0 | default | FIX | LIVE | engine-side; cyril relays workflow verbs opaquely |
| `tool_call`/`tool_call_update` `_meta.kiro.resources[]` (`{provider,owner,name}`) | 2.29.0 | default | ADD | LIVE | **DROPPED** — only the `agentInitiated` tag is read from the tool `_meta` (`kas.rs:531-540`) |
| `configuration/state` push + `getState` result `resources[].{type,compose,assertions[],pureAssertions,items[]}` populated | 2.29.0 | push opt-in | ADD | LIVE | **NOT-SENT** — cyril never pulls and has no resource model |
| recipe `include` nodes accepted + `includedFrom` on `nodeTree` + nested `nodePath` | 0.66.33 (no changelog line) | default | ADD | LIVE | relayed opaquely (no recipe/nodeTree model) |
| include-cycle negative `has invalid include nodes` | 0.66.33 | default | ADD | LIVE | n/a |
| bundled steering slash commands dropped from `available_commands_update` (5→2) | 2.29.0 | default | REMOVE (**regression**) | LIVE | HANDLED generically (`convert/mod.rs:491`); user-visible command loss |
| `configuration.intake.unknown-key` for `setting/subagentOrchestration` + `setting/todoList` | 0.66.33 | debug log | ADD (diagnostic) | LIVE | cyril **SENDS** both: `protocol/kas/settings.rs:82,93-97` |
| startup recipe-catalog reads for the `user` + `cloud` tiers | 0.66.33 | always-on boot | ADD | LIVE | n/a |
| stdio MCP child inherits the full parent env | 2.29.0 | default | ADD | LIVE | n/a (KAS spawns MCP) |
| memory handshake (`session/new._meta.memoryConfig`, `configOption memoryReflection`) | — | default | UNCHANGED | LIVE | HANDLED |
| `session/new._meta.kiro.overrideSessionId` honoured (and not idempotent) | — | default | UNCHANGED | LIVE | n/a (cyril sends no `session/new` `_meta`) |
| `kiro-cli acp --auth-method <cli\|client>` (v3 auth owner) | 2.29.0 | default `cli` | ADD | LIVE | n/a on the wire; cyril *is* the client that must answer when `client` is chosen (`protocol/kas/auth.rs:24,300`) |
| `kiro-cli agent upgrade <dir>` (V2/V3 agent-config migration, `fs_read`→`read`) | 2.28.1 (folded) | default | ADD | LIVE | n/a (host CLI) |
| `_ cleanup-autocomplete-artifacts` | — | — | **PRESENT in `kiro-cli-chat`** (dry-run: removes `~/.local/share/kiro-cli/ssh_inner`); absent from the launcher's `internal`/`_` namespace | LIVE | n/a (host CLI) |
| `largeToolOutputHandler` at the initialize handshake | — | — | NEGATIVE (unknown on both) | LIVE | not a cyril key (0 hits) |
| v2 sweep workload field set (`v2-live-sweep-*`, 3 turns) | 2.28.0 vs 2.29.1 | — | **UNCHANGED** (313 paths each, 0 only-NEW / 0 only-OLD) | LIVE | HANDLED — no v2 wire change |
| v2 mid-session model fallback (bogus model → prompt) | 2.29.0 | — | NEGATIVE (identical `-32603` on both) | LIVE | the changelog line is UNPROVEN (needs a served chain) |
| KAS telemetry dials (3 arms × 2 engines) | 2.29.0 | `telemetryEnabled` on the wire both | **UNCHANGED** (0 `telemetry*.kiro.dev` connects; control dials present) | LIVE | HANDLED |
| memory handshake arms (`memory`, `memoryEnable`, `userMemoryOptIn`) | — | default | UNCHANGED (normalized IDENTICAL) | LIVE | cyril sends none (`settings.rs:78-98,109`) |
| `interaction_resolved` on an unanswered-then-cancelled ask | — | default | exists on BOTH; NEW adds `consentRound:1` | LIVE | **DROPPED** — `convert/kas.rs:525`; a pre-existing gap, sharpened on 2.29.1 |

### 7b. Static rows confirmed and corrected live

**Confirmed:** kas-core L2 (`getState`) — extended: pull ungated, push gated, 9/12 layers, 3 typed
resource bundles. kas-core L7 (settings intake) — exactly two unknown-key lines at debug. kas-core L8
(stdio MCP full env) — an unlisted `MY_MARKER` reached the child. kas-core L6 (cascade precedence) —
env > defaults (`model_handoff.config.resolved {"source":"env"|"defaults"}`); the
`server_cascade_config` tier stays UNPROVEN (server-pushed, trap 13). kas-workflow W1/W2/W3 —
confirmed, W1 stronger than stated (`include` is a **new step type**, not just a field change).
host/tui H7 (`tool_tags` off the ACP wire) — confirmed negative.

**Corrected:** kas-core **L1** (`model_fallback`) is **not live-provable** — the kind needs a *served*
fallback chain; an unknown model is a `CLIENT_ERROR` so the SDK neither retries nor falls back, and
neither flag manufactures a chain. What is live is `modelFallbackReporting:true` (ADD). Status:
UNPROVEN, not contradicted. kas-core **L5** (image placeholder) — the compiled placeholder string does
**not** reach the client wire; a text-only model returns `end_turn` prose and the log carries only
`fs.loadImageCodec.ok`. The fix's observable behaviour is confirmed; the placeholder's surface is not.
The 2.28.0 audit's `largeToolOutputHandler` hypothesis is corrected to a confirmed negative.

### 7c. Still unproven after this run

1. **`model_fallback` (KAS)** — needs a real over-quota/refused primary with a configured chain, or a
   server push. Backend fault injection is off limits (trap 13).
2. **Multi-client `interaction_resolved` fanout** — `acp-server.js` is one process per stdio pipe, so
   two clients need the multiplex/ws transport the Rust host owns (`server/multiplex-stream.d.ts`).
3. **`KIRO_API_KEY` home-region auth** — no API key in this environment; the lane does not obtain
   credentials.
4. **Image-placeholder surface** — not client-visible; needs backend instrumentation.
5. **`inactiveItems` filter** — no non-active resource items exist here; needs a user-level override of
   a bundled steering doc.
6. **Steering-command regression causality** — the absence is confirmed; *why* (catalog change vs
   advertisement filter) is not.
7. **`--auth-method=client` end-to-end** — requests observed, handshake not completed by the probe.
8. **v2 mid-session model fallback (H5)** — the same error on both engines; needs a served fallback
   chain, as does the KAS twin (item 1).
9. **MCP OAuth authorization URL over SSH/tmux (H6)** — no OAuth-requiring MCP server was available
   to the lane; untouched.

## 8. Issues

New, filed under epic **cyril-90hi** (KAS 2.29 wire coverage, KAS 0.66.33), labelled `kas`,
`kiro-2-29-1` plus a triage label:

| id | P | kind | title |
|---|---|---|---|
| cyril-2nz7 | P2 | bug | KAS 0.66.33 drops the three bundled steering slash commands from `available_commands_update` (live regression 5→2) |
| cyril-aoby | P3 | feature | KAS: handle the `model_fallback` session_info kind + `_meta.kiro.modelFallbackReporting` |
| cyril-f4tt | P3 | feature | KAS: model `tool_call._meta.kiro.resources[]` repository stamping |
| cyril-ny8z | P2 | task | v3 launcher auth owner: `kiro-cli acp --agent-engine v3` now defaults to `--auth-method cli` |
| cyril-jf71 | P4 | feature | KAS: adopt `session/new _meta.kiro.overrideSessionId` (client-preallocated session ids) |

Notes added to existing issues (16) — each records what this audit adds:

| issue | what the audit adds |
|---|---|
| cyril-iq2c | `getState` is implemented; pull ungated / push gated; 9/12 layers; the resource-bundle model; `inactiveItems` untestable |
| cyril-53qx | live `configuration.intake.unknown-key` for `subagentOrchestration` + `todoList`; both keys removed from the 0.66.33 schema |
| cyril-w55l | root cause corroborated; the 20× loop is **fixed upstream** in 0.66.33 via `consentRound` |
| cyril-inwg | new recipe node `include` + `includedFrom`; startup recipe-catalog reads for the `user`/`cloud` tiers |
| cyril-be7e | the one new spawn env: `KIRO_CLI_IS_CHILD_PROCESS=1` |
| cyril-0na7 | the TUI now sends the memory keys unconditionally; the engine handshake is wire-identical |
| cyril-q8gq | its target row (`memory.enabled → userMemoryOptIn`) was **removed** from the 2.29.x TUI table — re-scope |
| cyril-99ds | the TUI's replay-gated `interaction_resolved` permission cancel; cyril drops the kind |
| cyril-qo13 | interaction kinds now carry `consentRound` (live payload on cancel) |
| cyril-58uv | the drop census extended to 25 kinds incl. the new `model_fallback` |
| cyril-3ald | `_kiro/safety/*` still unhandled on 0.66.33 |
| cyril-nk4o | `_kiro/mcp/*` still unhandled on 0.66.33 |
| cyril-cxd2 | the v3 launcher's new default auth owner (tracks cyril-ny8z) |
| cyril-6bha | the 2.29 cycle supersedes new-work scope; children listed |
| cyril-ci3j | the KAS twin ticket exists (cyril-aoby) and is static-only |
| cyril-79ox | the lost steering commands are exactly the `_meta.kiro`-typed entries that ticket renders |

**Rows deliberately filed as no-issue** (with the reason):

- `KIRO_API_KEY` home-region discovery + `EnvAuthProvider.region` getter + `--auth=machine` token-path
  absolutisation + the `.d.ts` growth (kas-core rows 10-13): agent-internal routing; no wire field.
  Live-unrun (no API key available).
- `ClientCustomAgent.steeringAfterPromptBody` (row 27): cyril sends no `customAgents`; nothing to
  model until custom-agent support lands.
- Image-fitting pipeline (row 28): agent-side; the only observable change is "the request no longer
  fails", and the placeholder surface is not client-visible.
- `_meta.kiro.replay` read (29), config-state zod keys (30), compaction carry (31): folded into the
  cyril-99ds / cyril-iq2c / cyril-58uv notes respectively rather than duplicated.
- `agent upgrade` (host row 11), hidden subcommands (8-10), `standalone_toolbox` (5), knowledge-search
  limits (17), Classic `fs_write` (14): host/engine internals with no cyril surface.
- All unchanged/negative rows (kas-core 32-41, workflow 1-5 and 12-16, TUI D10-D19 and 5a, host
  18-24): no delta, no issue. `todoList`/`tool_tags` confirmations ride the notes above.

## 9. Method notes for the next audit

- **Minifier renames break count-based scripts silently.** The 2.28.0 `static-kas-flags` script's
  env-map regex matched the flag-const map by identifier; the minifier renamed it `Gn`→`$n` in
  0.66.33, so the script reported "0 env-reachable" — a pure artefact. Widen to `[\w$]+` and
  sanity-check the OLD bundle before believing a count change.
- **A bare count diff invents removals.** `KIRO_DISABLE_RECAP`'s direct `process.env` read went
  1→0 while the literal went 0→1 (refactored to a generic `process.env[killSwitchEnv]`); the
  behaviour is identical. Same class: `_meta.kiro.replay` exists in both bundles and is only *read*
  in the new one; a raw substring count of `_kiro/workflow/` read 189→186, which a diff of the
  non-exact occurrences proved to be minifier identifiers inside error strings, not a removed
  literal.
- **Trap 5's example positive control is stale.** `largeToolOutputHandler` is **0 hits in both KAS
  bundles**; use `_meta.kiro.configurationState` / `inactiveItems` or `settings.memory` instead.
- **The TUI's 55k "statement-split diff lines" are minifier churn**, not behaviour: the literal
  delta is 44/3 and the method delta is zero. Diff *sets of quoted literals* (and build an
  identifier→`I`/number→`N`/string→`S(content)` skeleton diff), not statements.
- **Do not read `--help-all` as the whole CLI surface.** The visible help is byte-identical across
  these three builds while the binary grows ~215 KB; the changes are in hidden helpers,
  `crates/*/src/*.rs` paths, tracing call-site counts and the rollout table.
- **Ask the binary that owns the namespace.** `kiro-cli internal --help` lists 25 fig/shell helpers
  and omits the hidden agent helpers that live in `kiro-cli-chat`: `kiro-cli-chat _
  cleanup-autocomplete-artifacts --dry-run` succeeds (`would remove: …/ssh_inner`) while the launcher
  errors `unrecognized subcommand` (and the chat binary rejects the *name* `internal`). A "subcommand
  absent" verdict from the wrong binary is an artifact — the same class as the v2 PATH trap (traps 8).
- **Glue artefacts.** Env-string extraction glues neighbours
  (`KIRO_NO_AUTO_UPDATEKIRO_CLI_IS_CHILD_PROCESS…`); substring-check each candidate against the
  other version before reporting an env add (real adds this cycle: exactly one).
- **Never diff or claim from a capture another leg is still writing.** The gate-on NEW capture was
  diffed mid-write; every leg-2 look, all leg-2 diffs and the run-parity table had to be re-run on the
  final files. Close a leg, then measure it.
- **One KAS chain at a time.** Two chains ~40 s apart collided (both trees killed); the corrupted
  `kas-wseries-2.28.0.jsonl` is parked at `tmp/CORRUPT-kas-wseries-2.28.0.jsonl.bak` and a single
  clean chain was re-run. Serial legs are not a budget preference here — they are a correctness rule.
- **Copy a shared driver into the lane before using it.** Invoking `./probe-….py` from the lane
  directory failed (the shared probe lives one level up), producing empty captures for both legs; the
  re-run proves the archived binaries ran via the capture's `proc_tree` plus `--version`.
- **Per-file line-number set diffs produce false removals** when a raw digit run absorbs a
  neighbouring token (e.g. `[44,371]` vs `[44,371,40]`); re-derive per-file call counts first.
- **`static-rollout`'s per-key `summary` prints `len()` of a list-valued override field**, not a
  treatment value — only `treatment_percent` in a registry row is a percentage.
- **Mask the embedded payloads** before reading host `.rodata` contexts (blank the four zstd
  frames) or the payload bytes pollute every literal comparison.
- **Carve**: do not bound a zstd frame at the next magic candidate; incompressible payloads (Bun)
  store RAW blocks that expose nested magics. Decompress from the magic to EOF with `zstandard`
  `decompressobj` to get the exact consumed size.
- **Re-run the TUI parity script every audit** (`tui-vs-cyril-methods.py`): "no new method" is a
  version-to-version result and does **not** prove cyril parity — 19 of the TUI's 23 outbound calls
  have no cyril sender, all pre-dating this range.

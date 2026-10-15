# kiro-cli 2.31.1 wire audit — KAS barely moves, and the v3-default parity list (covers 2.30.0, 2.31.0)

Audited 2026-10-14/15. Baseline: 2.29.1 ([kiro-2.29.1-wire-audit.md](kiro-2.29.1-wire-audit.md)).
Scope: discovery, with the user's priority on the **v3/KAS backend, workflows, and unreleased
features**. Midway the user set the frame for this and every later audit: **KAS/v3 becomes cyril's
default engine because the v2 backend is being retired soon** (cyril-4c4d). Priorities below treat
KAS as "cyril's current path"; v2-only findings are demoted. Six lanes: KAS static (core + workflow
merged, because the KAS delta is one file), TUI-bundle static, Rust-host static, a new **v3-coverage**
lane (a standing parity inventory, not a delta), and one live lane (KAS 0.66.33 vs 0.66.34 paired,
plus the v3-default checks).

**Verdict: SAFE for cyril — no wire break on either engine.** The v2 wire is frozen (320 = 320 live
field paths, 2.29.1 vs 2.31.1). KAS 0.66.33 → 0.66.34 is one changed file, and live it matches
0.66.33 on the handshake, the sweep census, all 17 workflow runs, session/load, sandbox and mode
switching. The release adds little; the **v3-default parity inventory** is what this audit is for.
Framing used throughout: v2 commands are *expected* to fail on KAS, so every gap below is "cyril
lacks a KAS-native route", never "KAS rejects X".

The three findings that matter most:

1. **Four P1 blockers for making KAS the default, all confirmed against cyril's code and three live.**
   (a) Every KAS-advertised skill, saved prompt and custom agent is dispatched over v2's
   `kiro.dev/commands/execute` (18/18 calls fail; the KAS-native route, proven live, is
   `/name args` as prompt text) — **cyril-oieu**. (b) "Always allow" grants are written in V2 format,
   which KAS ignores — **cyril-auxt**. (c) No `/model`, `/effort` or `/mode` on KAS: KAS merged model
   and effort into one `configId:"model"` option with per-model `effortLevels`, and cyril has no
   command over it — **cyril-vh69**. (d) Free-path KAS sessions die ~1 h after the last kiro-cli
   token refresh — **cyril-zbu7**. All four are now prerequisites of cyril-4c4d; epic **cyril-c6hv**.
2. **`KIRO_HOME` became one validated resolver across Kiro** (host, TUI and KAS 0.66.34), and KAS now
   relocates agents, hooks, MCP config, sessions, memory and recipes with it. cyril accepts values Kiro
   ignores and hard-codes `~/.kiro` for agents (where trust grants go) and hooks — **cyril-d7ve**.
3. **KAS 0.66.34 turns native memory off by default** for new sessions when the client sends no
   `memory` setting. cyril sends none, so KAS memory is now off for every new cyril session
   (decision needed, cyril-0na7).

Evidence: [`experiments/conductor-spike/audit-2.31.1/`](../experiments/conductor-spike/audit-2.31.1/)
(`RUN-STATE.md`, `v3-retriage.md`, `<lane>-findings.md`), captures `experiments/conductor-spike/*-2.31.1.jsonl`.

## 1. Releases and artifacts

| ver | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS | TUI bundle | host | headline |
|---|---|---|---|---|---|---|---|---|
| 2.30.0 | 2026-10-12 20:15 | a1561bc5 | 11ee5ab2… | 0.66.33 | == 2.29.1 | 13,651,231 B | distinct | [V3] model refuse/unavailable notices; V2→V3 switch offer; agent-upgrade permission refresh |
| 2.31.0 | 2026-10-12 23:56 | a1561bc5 | b3ae5daf… | 0.66.34 | `acp-server.js` +32,694 B | 13,656,146 B | distinct | [V3] `/sandbox` + local `/autonomous` GA (off by default); MCP on-demand for all; `KIRO_HOME` relocation; 15 s service timeout |
| 2.31.1 | 2026-10-14 01:00 | d6fb1e94 | 46d26b88… (== latest manifest) | 0.66.34 | == 2.31.0 | 13,692,707 B | distinct | Mermaid diagrams; fallback handles empty refusals; compaction refusal fix; [security] workflow recipe escape sequences |

- Installed `kiro-cli`, `kiro-cli-chat`, `kiro-cli-term` and `tui.js` == archived 2.31.1 (`cmp` silent);
  the KAS carve == the self-extracted `kas/2.31.1-5763e7b9…` tree (`diff -rq` empty).
- The **only KAS delta** in the range: `dist/server/acp-server.js` 12,669,464 → 12,702,158 B, plus the
  `package.json` version. No `.d.ts`, externals or `node_modules` change. So no 2.30.0 `[V3]` line can be
  KAS code; each one was found in the TUI, the host or the backend.
- 2.30.0 and 2.31.0 share BUILD_HASH `a1561bc5` yet differ in every binary (chat `.text` +47,168 B, two
  new source files): a release-train stamp, not a content hash.
- Frames: Node ELF (32,031,786 / 124,679,552) and Bun ELF (29,887,616 / 79,500,640) are byte-identical in
  all three releases; 62 nested magics each. KAS tar 27,801,781 → 27,808,488 → 27,808,487 B compressed.
- Changelogs from `stable/<ver>/feed.json` (`audit-2.31.1/feed-<ver>.json`).

## 2. cyril on KAS by default: the parity list (cyril-4c4d)

The v3-coverage lane compared Kiro's TUI 2.31.1 (the reference v3 client), KAS 0.66.34 and cyril HEAD
`5ee8bc07` in nine matrices of ~180 rows (`v3-coverage-findings.md`). The KAS core turn is **handled**:
chunks, tool calls, turn_start/turn_end, metering, context %, steering, rate limit, workflows, and the
fs/terminal/auth/hooks host role. What is missing is mostly the surface around the turn.

### 2a. Flip blockers (P1, prerequisites of cyril-4c4d)

| # | gap | evidence | issue |
|---|---|---|---|
| B1 | **Agent commands.** KAS advertises skills, saved prompts and custom agents (`_meta.kiro.type` `skill`/`prompt`/`custom-agent`). cyril registers each (`commands/mod.rs:449-469`) and dispatches over `_kiro.dev/commands/execute`/`options` (`domain_mediator/commands/extensions.rs:13-47`) — a v2-only family. The KAS filter drops only the four workflow-gate names (`convert/kas.rs:267-300`). | LIVE `kas-v3c-v1-*`: 18/18 fail -32603 on both engines. KAS-native route LIVE: `session/prompt "/probe-skill hello"` → `tool_call "Load skill: probe-skill"` → answer. TUI sends `prompt:[{type:"text"}]` + optional `_meta.kiro{outputStyle,displayText}` | cyril-oieu (P3→P1) |
| B2 | **Trust grants inert.** cyril writes "Always allow" to V2 `toolsSettings.execute_bash.allowedCommands` (`kiro_agent_config.rs:146-251`); KAS reads only V3 `permissions.rules`. `agent upgrade` copies V2 into V3 (its fingerprint ignores key order, so cyril's re-serialisation is safe), but once the user edits V3 permissions the upgrade skips the file ("out of sync") and grants are stranded. | LIVE `kas-v3c-l9-2.31.1.jsonl`: V3-ruled command runs unasked; cyril's appended command is asked; `h5-agent-upgrade-2.31.1.json` arms A–E | **cyril-auxt** (NEW) |
| B3 | **No model, effort or mode switching.** On KAS, model and effort are merged: `config_option_update` carries `configId:"model"` (21 options, each `_meta.kiro.{hasEffort, effortLevels[], defaultEffortLevel, effortSchemaPath, rateMultiplier, thinkingToggleable}`); effort is `set_config_option{configId:"effortLevel"}`; mode is `configId:"mode"`. cyril has the receive path (`cyril-ui/src/state.rs:1023`) and the setter (`BridgeCommand::SetConfigOption`, used by `/thinking`) but no command. v2's `/model`/`/effort` came from Kiro's advertised commands, which KAS does not have. | LIVE `kas-v3c-v3-*`, sweep frame 24. KAS accepts an unoffered model unvalidated; `effortLevel` exists only once a model with effort is current, and an invalid level "succeeds" while `currentValue` stays put; an unknown mode silently becomes `vibe`; `session/set_model` is -32603 on KAS (cyril's `SetModel` has no production producer, so it is a constraint, not a failure) | **cyril-vh69** (NEW) over cyril-cxwb, -lxuo, -838u |
| B4 | **Token renewal.** On the free-path spawn cyril serves the stored access token and never renews it (`protocol/kas/auth.rs:210-268`): ~1 h per `kiro-cli login`. | STATIC + prior live history (cyril-taba) | cyril-zbu7 (P2→P1); cyril-taba stopgap |

### 2b. P2 parity (default engine, dropped or missing route)

| gap | evidence | issue |
|---|---|---|
| Sub-agent output interleaves into the main transcript: every `agentSubtaskId` frame, including the child's own tool calls, arrives on the **parent** sessionId; the child streams no text. `orchestrate_subagent` is gone (0.66.33); only `invoke_sub_agent` remains. cyril reads no `agentSubtaskId`. | LIVE `kas-v3c-v2-2.31.1.jsonl` frames 37-46 | cyril-ebqu (premise corrected) |
| Orphan `tool_call_update` (id never started) is dropped silently (`cyril-ui/src/state.rs:560-575`) — KAS sends "Tool Load" (MCP on-demand) and "Subagent Response" (the child's answer) this way. | LIVE `kas-lead-mcp-*` 43-58, `kas-v3c-v2` 46 | **cyril-1v25** (NEW) |
| Most of v2's slash surface has no KAS route: `/context`, `/compact`, `/tools`, `/mcp`, `/knowledge`, `/code`, `/rewind`, … Kiro's TUI maps them to `_kiro/*` methods (Matrix 7). | STATIC | **cyril-1aq0** (NEW umbrella) |
| 17 of 25 `session_info_update` kinds dropped **unlogged** (`convert/kas.rs:525`), incl. `model_fallback` (Kiro's TUI renders it since 2.30.0) and `summarization_*` (compaction invisible; v2's compaction status was handled). | STATIC; fallback decline path seen in the KAS debug log | cyril-58uv (P1 umbrella), cyril-aoby (P3→P2) |
| Org governance silent: `_kiro/governance/state` is pushed unsolicited at session/new (both engines), and "KAS enforces governance itself". | LIVE sweep frame 17 | cyril-0o7e (P3→P2) |
| MCP failures and remote-MCP OAuth invisible: `_kiro/mcp/status`, `display_error{mcp_connection_error}` dropped; `openExternalUrl` not advertised, so KAS never sends the sign-in URL. | LIVE V5 status shape; OAuth server not exercised | cyril-nk4o (P3→P2), **cyril-tqr8** (NEW) |

### 2c. Handshake as KAS sees it (LIVE, cyril-exact initialize, KAS debug log)

| key under `initialize.clientCapabilities._meta.kiro.settings` | cyril | TUI 2.31.1 | KAS intake (0.66.33 = 0.66.34) |
|---|---|---|---|
| `subagentOrchestration` | sends (`settings.rs:93-98`) | no | `unknown-key` (dead since 0.66.33) |
| `todoList` | sends when enabled (`settings.rs:82`) | no | `unknown-key` |
| `toolSearch{enabled,minPct,minTokens}` | sends (`settings.rs:120-131`) | sends | accepted; `enabled` has no effect on either engine; only `neverDefer` (not sent) works |
| `thinking` | sends | sends | `unread-key` at the connection layer |
| `memory` | **no** | `{mode:"disabled",reflection:true}` | consumed |
| `workflows` / `workflowNotifications` | no (ADR-0011) | sends | `unread-key` / `unknown-key` at initialize |
| `largeToolOutputHandler` | no | sends | `unknown-key` — not a valid positive control |

`clientInfo` `cyril` is logged `clientAttribution:"unrecognized"` and run with KAS's **kiro-ide**
behaviour profile (`types/present_as.rs:43-47`) — **cyril-po2z**. Notes on cyril-53qx.

### 2d. Smaller parity items filed

`/spawn`, `/kill`, `/msg`, `/sessions` send v2-only methods on KAS (**cyril-a8se**, P3);
`turn_end.stopReason` `error`/`content_filtered` map to EndTurn (`convert/kas.rs:717-734`, **cyril-pyko**, P3);
`extensionMethods` and the other six agent capabilities the TUI parses are unread (cyril-tikf).
Deferred in the epic: `_kiro/customAgent/{not_found,config_error}`, closing loaded workflow-step sessions.

## 3. KAS 0.66.33 → 0.66.34

| item | gate | live | cyril | issue |
|---|---|---|---|---|
| `_kiro/environment/start_ssh_session` (0→5 literals; `extensionMethods` 27→28). `{}` → -32602; any sessionId → -32000 `SshSessionUnavailableError` without `KIRO_REMOTE_SESSIONS_ENDPOINT`; with it `{outcome: ready{ssmSessionId,streamUrl,tokenValue} \| environmentNotReady \| notFound}` | dark | CONFIRMED (0.66.33: -32603) | NOT-SENT | note cyril-tikf |
| `KIRO_HOME` → `userConfigDir` (`userConfigDir` 0→147): agents, hooks, steering/skills/prompts, `settings/mcp.json`, sessions, logs, memories, knowledge, recipes, tasks, cloud-cache, sandbox grant. Ignored if relative, root, or `$HOME`/ancestor (`agent.userConfigDir.ignored{reason}`) | env | CONFIRMED (0.66.33 ignores the variable); hooks relocation STATIC (v2Hooks off) | diverges (§2, finding 2) | **cyril-d7ve** |
| Memory default `{read_write, true}` → `{disabled, false}` for new sessions with no `memory` layer; a loaded 0.66.33 session keeps `read_write`. Surfaces in session/new `_meta.memoryConfig`, `memoryReflection` configOption, `_kiro/config/template` | — | CONFIRMED | NOT-SENT | note cyril-0na7 |
| MCP deferral unconditional (`AB_TOOL_LOAD` 4→0, `tool_search` tool and `KIRO_TOOL_SEARCH_THRESHOLD` deleted) | was A/B | **REFUTED as a visible change**: this account was already deferred on 0.66.33; `tool_load` rawInput `{tool_id, arguments}` | not unwrapped; orphan update dropped | notes cyril-2yn8, cyril-53qx; cyril-1v25 |
| Service timeouts: three control-plane clients move to a shared handler that throws on request timeout (`throwOnRequestTimeout` 5→5: call-site change) | always | not reached (no forging) | n/a | — |
| Animated WebP sent as PNG; RIFF/ANMF validation | always | — | n/a | — |
| MCP streamable-HTTP session recovery (reset queue, approval-bound server identity, refusal text as tool result) | always | — | arrives as tool-result text | — |
| `web_fetch` `rendered` mode removed; `reasoning:{effort:"none"}` special case removed | always | — | n/a | — |
| `session/load` suppresses abandoned-prompt replay when messages arrive during load (`arrivedDuringLoad` 0→4) | always | replay identical; race not reached | killed turn = synthetic `turn_end{cancelled, replay:true}`, handled `convert/kas.rs:398-400` | note cyril-rtrh |
| Sandbox seatbelt `local ip` → `remote ip "localhost:*"`; write grant follows `userConfigDir` | `sandboxConfig` | sandbox behaviour identical | NOT-SENT | notes cyril-6vo6, cyril-be7e |
| `AGENTS.md` steering watch moved to workspace-only paths | always | — | n/a | — |

Unchanged (set-diffed): 25 `session_info_update` kinds, 15 `_meta.kiro` keys, 11 `settings` keys,
AgentSettings zod keys, 25 ACP method literals, auth, configOptions. `model_fallback_refusal` and
`model_fallback_availability` flags still default off, yet the live debug log shows the availability
path evaluated (`modelFallback.declined{trigger:"availability", reason:"notModelAttributable"}`), so
the backend A/B decides. `_kiro/governance/state` (pushed on both engines) and `_kiro/config/template`
(returns `{modes, configOptions}`, no `agents`) are not new.

## 4. Workflows (v3)

| item | live | cyril | issue |
|---|---|---|---|
| `_kiro/workflow/new` launch defaults `modelId`/`effortLevel`: validated before path resolution, override the recipe defaults, carried on state and nodes | CONFIRMED: `modelId:5` → "Invalid workflow shape" on NEW, path error on OLD; step ran on the given model | NOT-SENT (`domain_mediator/commands/kas.rs:349-356`; no `runLabel` either) | note cyril-4u4a |
| `run_workflow` tool schema gains `modelId`, `effortLevel` | model-facing | n/a | — |
| Bundled `feature-pipeline`: `plan` and `code-aggregate` gain `effortLevel:"high"` | CONFIRMED (sweep 136) | n/a | — |
| Unsavable step `send_message` signal now parks the step awaiting the user (`unrecordedStepSignals` 0→10) | not reached | expected via existing `node_paused` (`workflow.rs:508/:667`) | — |
| Concurrency: `load` coalescing, `invoke`/`retry` post-claim re-checks, same-process claim queue, new error texts | — | surfaces as a generic workflow failure (`commands/kas.rs:362-374`) | — |
| Recipe text raw on the wire (only `runLabel` validated); the 2.31.1 `[security]` fix is **TUI-only**: it sanitises description/validationError/inputs, not `name` | STATIC (KAS escape counts unchanged) | ratatui drops control graphemes, so escapes cannot reach the terminal; newlines, bidi controls and printable CSI tails pass (`cyril-ui/src/workflow_format.rs:88-98`); `validationError` dropped (`convert/kas/workflow.rs:610-617`), so an invalid recipe lists as valid | **cyril-n8pr** |

Unchanged: `_kiro/workflow/*` method set, lifecycle literals, snapshot fields, `recipes_changed`,
`load`/`inspect`, `resume`/`extendRepeat`, auto-wake, watch handlers, `_kiro/session/notify`,
`consentRound`; recipe schema only renamed by the minifier, so
`docs/kiro-kas-workflow-recipe-schema-2.26.0.json` stays valid. Live: 17/17 runs match event-for-event
gate-off and gate-on.

## 5. v2 (Rust) engine and launcher — demoted (v2 retiring)

- **v2 ACP wire frozen**: `_kiro.dev/*` set, 42 `*Args` structs, 50 tagged enums, every `--help` md5
  identical; LIVE v2 sweep 2.29.1 vs 2.31.1 = 320 = 320 paths. `acp` still defaults to v2.
- v2 compaction refusal ("The model declined to summarize the conversation", 0/0/0/1) ends in the existing
  `unsummarized_dropped` status, dropped with a warn at `convert/kiro.rs:803-808` — cyril-0f4e (P3→P4).
  v2 fallback now handles empty refusals, wire unchanged — cyril-ci3j (P2→P4).
- `KIRO_HOME`: new resolver `crates/agent/src/agent/util/directories.rs:97` ("ignoring KIRO_HOME; using the
  default Kiro home"); v2 agent-config loading already read `KIRO_HOME` in 2.29.1.
- Rollout registry: `v3_prompt` 35% internal → 10% external / 50% internal (new field
  `internal_treatment_percent`), with the 2.30.0 copy "You're using Kiro CLI V2. Switch to V3 today."
  (`launch/v3_ease_in.rs`; no evidence it reaches `kiro-cli acp`; live check stopped at the auth gate);
  `local_sandbox` → GA (2.31.0, `KIRO_ENABLE_LOCAL_SANDBOX` removed); **`managed_settings`** added dark at
  0% (2.31.1); `standalone_toolbox` cohort overrides 1 → 7. Note on cyril-a7nk.
- Auth: social-login refresh tolerates a temporary auth-service error (2.31.0); new
  `chat-cli-v2/src/auth/partition.rs` builds per-partition OIDC endpoints (2.31.1). No cyril impact:
  cyril reads, never refreshes, the store.
- Headless V3 launch now sends `disableAutoCompaction`/`minPct`/`minTokens` (`launch.rs:2601`), which
  cyril already sends.
- Env: + `KIRO_DISABLE_TOOLKIT_TELEMETRY` (2.31.1, unannounced), + `KIRO_TELEMETRY_EXPLICIT_HISTOGRAMS`
  (2.31.0); settings + `chat.screenReader` (120 keys). `agent upgrade` writes
  `kiroUpgrade{revision, fingerprint, source}` (2.30.0) — the fingerprint is semantic (LIVE, §2a B2).
- Doc manifest: 164 docs / 37 `category:feature` in all four builds; only `slash-commands/settings.md`
  keywords changed. No unannounced feature.

## 6. What Kiro's own TUI now sends and consumes

The TUI's `_kiro/*` (63) and `kiro.dev/*` (38) literal sets, 74 slash literals, the initialize clientMeta,
session/new, prompt and load `_meta`, and the KAS spawn argv/env are identical across all four bundles.
What changed is rendering and gated surface:

- **2.30.0 model-fallback notices** — the transcript notice the changelog tags `[V3]` is TUI rendering of
  the existing `model_fallback` kind (fields in §2b), plus a dark `model_fallback_promoted` kind that no
  engine emits. The fallback renderer's outcome `unanswered` became `faulted`, and
  `turn_end.stopReason` `error`/`content_filtered` now count as faulted. Notes on cyril-aoby.
- **2.31.0** `KIRO_HOME` validation; one-time migration of powers and secrets into `KIRO_HOME`;
  `_kiro/codeIntelligence` status gated on `extensionMethods`; tool display unwraps `{tool_id, arguments}`;
  `/autonomous` now says "not available" instead of returning silently.
- **2.31.1** `managed_settings` (see §7); Herdr pane-state reporting; the workflow-text sanitiser;
  Mermaid terminal diagrams; `/changelog` per-line entries.
- Local `/sandbox`/`/autonomous` gate is still `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1"`, which the host
  sets from the now-GA rollout row. On KAS spawn the TUI adds `--sandbox=auto
  --sandbox-network-mode=<mode>`.
- TUI-local only: screen reader (`chat.screenReader`, `KIRO_SCREEN_READER`), tmux/offset fixes, glyphs,
  `chat.disableWrap` copy.

## 7. Unannounced or dark features

| feature | gate | evidence |
|---|---|---|
| **Org-managed settings**: `_kiro/governance/state` parser (7 features + `disabledReason`), `/config managed`, `organization` provenance on hooks/powers/steering/MCP, MCP registry `accessMode` hidden servers, pre-session `_kiro/config/template` | rollout `managed_settings`, 0%, internal nightly; opt-in `KIRO_ENABLED_FEATURES='["managed_settings"]'`. Enforcement is not gated: KAS enforces governance itself | host rollout row text; TUI `Bn("managed_settings")` 0→5 |
| **Cloud-session SSH** `_kiro/environment/start_ssh_session` | `KIRO_REMOTE_SESSIONS_ENDPOINT` | §3 |
| **Herdr** pane-state reporting: TUI execs `$HERDR_BIN_PATH pane report-agent … --state idle\|working\|blocked` | `HERDR_ENV=1` + bin/pane/socket paths | **cyril-v2u6** (needs-triage) |
| `model_fallback_promoted` session-info kind | consumed by the TUI, emitted by no engine | §6 |
| `standalone_toolbox` cohort enrolment | 0%, overrides 1→7 | host rollout |
| `coerceStringifiedArgs` MCP option | wired off | KAS |

## 8. Live results

All legs serial, 2026-10-15 01:33–02:20 UTC, direct carves of 0.66.33 and 0.66.34, temp `HOME`, real
`XDG_DATA_HOME`, no forging, no login/logout. Full log: `audit-2.31.1/live-findings.md`.

| leg | capture(s) | outcome |
|---|---|---|
| 0 `--help` + initialize matrix | `kas-init-2.31.1-*` | `extensionMethods` +1 |
| 1 paired sweep | `kas-live-sweep-2.31.1.jsonl` (bundle) | 695 vs 693 paths; only `recipes[].plan[].steps[].effortLevel` is real. First NEW sweep void (auth wait) and re-run |
| 2 workflow gate off/on | `kas-workflow-gate{off,on}-2.31.1.jsonl` | 17/17 runs identical; `workflowWatch.*` "removed" is a sweep-tool artifact |
| 3 leads | `kas-lead-*`, `kas-v3d-*`, `kas-v3c-*` | §2–§4 rows marked CONFIRMED / REFUTED |
| 4 v2 pair | `v2-live-sweep-2.31.1.jsonl` | 320 = 320 |

Corrections to static rows: MCP deferral was not a visible change; `toolSearch.enabled` is inert on
both engines; `_kiro/governance/state` and `_kiro/config/template` are not new and the latter has no
`agents`; the KAS log line for `KIRO_HOME` is `agent.userConfigDir.*`; `memoryConfigSource` appears only
in workflow state; the coverage lane's "live frame 146" for B1 was a mis-citation, superseded by V1.

Not reached: the `model_fallback` frame itself, compaction, the 15 s timeout, an OAuth MCP server, the
`arrivedDuringLoad` race and the unsavable `send_message` pause. One natural 301 s silent backend stall
occurred on 0.66.33 (cf. cyril-14ou).

## 9. Issues

New (epic **cyril-c6hv**, labels `kas`, `kiro-2-31-1`):

| id | P | title |
|---|---|---|
| cyril-c6hv | P1 | KAS default-engine parity — 2.31.1 audit (cyril-4c4d parity list) |
| cyril-vh69 | P1 | KAS: /model picker with per-model effort, plus /mode, driven by session configOptions |
| cyril-auxt | P1 | KAS: "Always allow" grants are inert — persist them as V3 permissions.rules |
| cyril-1v25 | P2 | UiState silently drops a tool_call_update whose toolCallId never started |
| cyril-1aq0 | P2 | KAS slash-command parity umbrella |
| cyril-tqr8 | P2 | KAS remote-MCP OAuth never reaches the user |
| cyril-d7ve | P3 | KIRO_HOME parity with Kiro 2.31.0 |
| cyril-n8pr | P3 | /workflow list: show recipe validationError and single-line-sanitise recipe text |
| cyril-a8se | P3 | KAS: /spawn, /kill, /msg, /sessions send v2-only methods |
| cyril-pyko | P3 | KAS turn_end stopReason error/content_filtered is mapped to EndTurn |
| cyril-po2z | P3 | KAS treats clientInfo cyril as unrecognized (kiro-ide profile) |
| cyril-o3s9 | P4 | CLAUDE.md: stale KAS notes (ToolKind::Other, orchestrate_subagent) |
| cyril-v2u6 | P4 | Herdr pane-state reporting |

Reprioritised: cyril-oieu and cyril-zbu7 → P1 and made prerequisites of cyril-4c4d (with cyril-vh69 and
cyril-auxt); cyril-aoby, cyril-0o7e, cyril-nk4o → P2; cyril-0f4e, cyril-ci3j → P4.
Notes: cyril-4c4d, -oieu, -zbu7, -taba, -ebqu, -aoby, -0o7e, -nk4o, -0f4e, -ci3j, -58uv, -53qx, -0na7,
-2yn8, -4u4a, -tikf, -lyrx, -6vo6, -be7e, -rtrh, -a7nk, -w55l.

Changelog reconciliation (31 lines): every line is located in a lane write-up (`kas`, `tui`, `host`
"Changelog reconciliation" sections). Not observable in Linux artifacts: 2.31.1 Windows `/sandbox`
temp-folder fix. Not exercised live: knowledge-search chunk fix, IAM Identity Center partitions.

## 10. Method notes for the next audit

- **Treat KAS as cyril's current path** (user, 2026-10-14). Run a standing **coverage** lane (reference
  TUI × KAS × cyril) every cycle, not only delta lanes; drop v2 live legs unless budget remains.
- **v2 commands failing on KAS is expected.** Don't spend probes proving it; probe the KAS-native route
  (the frame Kiro's TUI sends) and phrase findings as "cyril lacks a route".
- **Look for a moved feature before calling it missing.** "No `/model` on KAS" was a protocol move (model
  and effort folded into `configId:"model"` options), not an absence.
- **Spot-check every lane citation.** A frame index was off by one and pointed at a probe-only method;
  frame numbers in FINDINGS refer to lane-dir captures, not the trimmed `*-2.31.1.jsonl` bundles.
- `largeToolOutputHandler` is not a valid handshake positive control (KAS logs it `unknown-key`); use
  `configurationState:true`, which makes KAS push `_kiro/configuration/state`.
- KAS debug lines go to `$HOME/.kiro/logs/<ts>/kiro.log`, not stderr.
- `session/new` can return without the `model` option (it arrives in a later `config_option_update`);
  timing, not a version change.
- KAS sends `tool_call_update`s whose id never appeared in a `tool_call`.
- A one-file KAS delta collapses `kas-core` and `kas-workflow` into one lane.
- Large regenerable raw outputs (`kas/bundle` 50 MB, `host/raw` 78 MB, `live/tmp` 191 MB) stay uncommitted.

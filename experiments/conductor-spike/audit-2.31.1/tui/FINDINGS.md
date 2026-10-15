# TUI-bundle static lane: kiro-cli 2.29.1 -> 2.31.1 (folds 2.30.0, 2.31.0)

Inputs (sha256 checked against the `.sha256` sidecars this run; archive copies only, never `~/.local/share/kiro-cli/`, trap 18):

| bundle | sha256 | bytes |
|---|---|---|
| kiro-tui-2.29.1.js | 256b00c1…7d6 | 13,643,233 |
| kiro-tui-2.30.0.js | 62376896…be2 | 13,651,231 |
| kiro-tui-2.31.0.js | a4a773c6…36a | 13,656,146 |
| kiro-tui-2.31.1.js | 105848c6…ee2 (== installed snapshot) | 13,692,707 |

KAS carves were grepped read-only for cross-reference only (kas lane owns them). Nothing from Kiro was executed.

---

## Summary

1. **The wire vocabulary is frozen in all three releases.** Trap-3 set diffs, 2.29.1 -> 2.30.0 -> 2.31.0 -> 2.31.1, all equal: `_kiro/*` 63, `kiro.dev/*` 38, slash-command literals 74, `name:"/x"` entries 37, hidden `chat _` subcommands 11, ext-call literals 20, gates method table 113. The `initialize` clientMeta, the `settings` builder, the `session/new`, `session/prompt`, permission and `session/load` `_meta`, and the KAS spawn argv and env are byte-equivalent modulo renames. **No new method and no new handshake key in this range.**
2. **2.30.0: the TUI starts rendering KAS model fallback.** `session_info_update._meta.kiro.kind` ∈ {`model_fallback`, `model_fallback_promoted`} feeds the existing v2 fallback renderer. That renderer prints the transcript notices: "X refused / was unavailable / yielded", "answered by Y for this turn", "Y is now this conversation's model", "produced no answer". KAS 0.66.33 already emitted `model_fallback`, so the 2.30.0 `[V3]` notice line is a **TUI change**. KAS has **0** `model_fallback_promoted`, so that kind is a forward-compat branch in the TUI. cyril DROPS both at `convert/kas.rs:525` (cyril-aoby).
3. **2.31.1: org-managed settings, dark behind the new rollout gate `managed_settings`.** It adds a full `_kiro/governance/state` parser (7 features plus `disabledReason`), a `/config managed` tab, a pre-session `_kiro/config/template` call, `resource.source.provenance.scope:"organization"`, and the MCP registry `accessMode:"registry"`/`accessModeFilteredServers` hidden-server list. cyril drops `_kiro/governance/state` and `_kiro/mcp/status` (`convert/kiro.rs:1284`).
4. **2.31.1 [security] workflow sanitiser.** Calls to the sanitiser `Xr` go 24 -> 27 and calls to the strip function `kr` go 26 -> 27. The new calls cover the recipe picker `description` (`validationError ?? description ?? source`), two `is invalid: ${validationError}` alerts, and the inputs-panel `description`. The recipe `name` and the input keys stay unsanitised. cyril emits no escapes (ratatui drops control graphemes). But cyril drops `validationError` and splices raw newlines from `description` into `/workflow list`.
5. **2.31.1 Herdr.** Herdr is an external pane/agent manager (CLI plus unix socket, `HERDR_*` env), not ACP. The TUI execs `$HERDR_BIN_PATH pane report-agent … --state idle|working|blocked`, and `blocked` means a permission or question prompt is pending. Before 2.31.1 the TUI only reported resume ids. cyril could implement the same CLI contract.
6. **2.31.0 is small:** `KIRO_HOME` validation (relative, root and home-containing values are ignored and the value is trimmed), plus a one-time copy of powers and secrets into `KIRO_HOME`. It also adds a cloud-aware code-intelligence indicator gated on `extensionMethods`, unwraps the `{tool_id, arguments}` rawInput shape (MCP on-demand), and replaces a silent return with the message "Autonomous mode is not available in this session". The **local `/sandbox`/`/autonomous` gate is unchanged** (`KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1"`), so the 2.31.0 GA line must be host-side.
7. **Not in the TUI:** the V2->V3 switch offer at chat start (`V3` count 24 == 24 == 24 == 24, no new strings), the agent-upgrade permission refresh (only the report copy is TUI-side), knowledge-chunk loss, the 2.31.1 refusal and compaction fixes (no TUI delta, and KAS 2.31.1 == 2.31.0), `KIRO_HOME` for MCP, permissions and sessions, `sandbox.json` headless, WebP, and the 15 s timeout. All of these go to the host or kas lanes.
8. **cyril divergences found in passing:** `KIRO_HOME` resolution accepts values Kiro 2.31.0 now ignores, and the hooks registry and the global agents directory ignore `KIRO_HOME`. `extensionMethods` is not parsed at all.

Legend: **STATIC** = read off the bundle (a lead); **LIVE** = needs a probe. Every row in this file is STATIC.

---

## Deltas

Columns: S/L = STATIC/LIVE. In the cyril column, H = HANDLED, D = DROPPED, NS = NOT-SENT, N/A = TUI-local with no wire part.

| # | item | version | gate/default | direction | S/L | cyril verdict + file:line | proposed issue |
|---|---|---|---|---|---|---|---|
| T1 | KAS `session_info_update._meta.kiro.kind:"model_fallback"` consumed. Fields read: `from`, `to?`, `trigger` (`refusal`->refused, `availability`->unavailable, else "unknown"->"yielded"), `scope` (`session` => second event `promoted:true`; `turn` => none; other values warn), `agentSubtaskId` (a non-empty value is ignored as a sub-agent notice). `reason` is **not** read. Ids are validated by `^[A-Za-z0-9._:-]{1,1024}$`. Counts: `model_fallback` 6 -> 19, `Unrecognized model_fallback trigger from KAS` 0 -> 1 | 2.30.0 | none (always on for kas) | inbound | STATIC | **D**: `crates/cyril-core/src/protocol/convert/kas.rs:352` (`session_info_to_notification`) falls to `_ => None` at `:525` | NOTE on cyril-aoby (field map + render copy below) |
| T2 | `kind:"model_fallback_promoted"` (needs `to` and `trigger==="refusal"`, else dropped with a warn) -> `{type:"model_fallback", cause:"refused", promoted:true}`. Count 0 -> 4. **KAS 0.66.33/0.66.34: 0 hits** | 2.30.0 | engine never emits it today | inbound (forward-compat) | STATIC | **D** (same arm, `kas.rs:525`) | NOTE on cyril-aoby (dark kind) |
| T3 | Fallback renderer reworked (shared by v2 and KAS): outcome `unanswered` -> `faulted` (`unanswered` 2 -> 0, `faulted` 4 -> 7). `turn_end.stopReason` `error`/`content_filtered` counts as faulted. New copy: `` `${o}; ${n(a.to)} produced no answer.` `` and `` `${n(e.promotedTo)} is now this conversation's model.` `` (0 -> 1 each). Same-model retry shows `couldn't complete this response. Retrying…` | 2.30.0 | — | TUI render | STATIC | v2 twin **D**: `convert/kiro.rs:1058` (unhandled `kiro.dev/session/update` variant) | NOTE on cyril-ci3j / cyril-pouu |
| T4 | Screen reader: setting `chat.screenReader` (`auto`\|`on`\|`off`, default `auto`), env override `KIRO_SCREEN_READER` (0 -> 2). macOS VoiceOver detection via `ps -axo comm=` with a 500 ms timeout ("process listing timed out"). `streamTailCursor`/`yieldCursorWhenEmpty` park the cursor at the stream tail. `/settings display` gains a "Screen reader" row | 2.30.0 | setting, default auto | TUI-local | STATIC | N/A | — |
| T5 | tmux and launch-offset fixes: APC cursor markers `\x1B_ki\x07`, `\x1B_kt\x07` (0 -> 1), clear sequence `\x1B[2K\x1B7\x1B[1B\x1B[J\x1B8` (0 -> 1), `\r\x1B[J` removed (1 -> 0) | 2.30.0 | — | TUI-local | STATIC | N/A | — |
| T6 | Icon glyphs: clipboard `📋` -> `⌸`, sparkle `✨` -> `✧`, `🔧` and `☁️` removed, `⚒` added | 2.30.0 | — | TUI-local | STATIC | N/A (cyril chooses its own glyphs) | — |
| T7 | `/upgrade-agent` report: new diagnostic `skill-read-restrictions` ("V3 doesn't apply read restrictions to skill loading yet", attribute `toolsSettings deniedPaths`, 0 -> 1). Label "V2 or earlier upgrade" / "Upgrade or refresh the universal (V2 + V3) config" (was "Upgrade to universal …") | 2.30.0 | — | TUI render of the host subcommand | STATIC | N/A (the upgrade itself is host `chat _ upgrade-agent`, set 11 == 11) | — |
| T8 | `KIRO_HOME` validation `L9t`: value is trimmed; a relative, root or home-containing value is ignored with warn `[kiro-home] ignoring KIRO_HOME=${r} (${o}); using ${n}` (0 -> 1). In 2.30.0 any non-empty value was used verbatim | 2.31.0 | — | TUI path resolution (KAS inherits env) | STATIC | **divergent**: cyril uses any non-empty `KIRO_HOME` verbatim, untrimmed: `crates/cyril-core/src/protocol/kas/settings.rs:50-58`, `crates/cyril-core/src/kiro_agent_config.rs:93-97`. `global_agents_dir` ignores `KIRO_HOME` (`kiro_agent_config.rs:102-104`), and so does the hooks registry (`crates/cyril-core/src/protocol/domain_mediator/mod.rs:264-266`) | **NEW** (see below) |
| T9 | One-time migration of legacy powers and TUI secret-store (`secrets.json`) into `KIRO_HOME`, with markers `.powers-migrated` and `.secrets-migrated` (0 -> 1 each) | 2.31.0 | `KIRO_HOME` set and valid | TUI-local storage | STATIC | N/A (cyril implements no `_kiro/secret/*` store; powers are read-only push, `convert/kas/powers.rs:48`) | — |
| T10 | Code intelligence: `codeIntelligenceStatus` (0 -> 4) calls `_kiro/codeIntelligence {subcommand:"status"}` only if `extensionMethods` is empty or includes `_kiro/codeIntelligence`. Re-queried on cloud-session switch and readiness. `rootPath`/`configPath` are omitted for remote | 2.31.0 | `extensionMethods` capability | outbound (existing method) | STATIC | **NS**: cyril never calls `_kiro/codeIntelligence`. `extensionMethods` is unparsed (only a test fixture, `crates/cyril-core/src/protocol/fingerprint.rs:138`) | NOTE on cyril-tikf (`extensionMethods` = 5th capability key) |
| T11 | Tool rawInput display unwraps `{tool_id:string, arguments:object}` (`tool_id` 0 -> 1). This is the MCP on-demand invocation shape | 2.31.0 | — | inbound render | STATIC | **gap risk**: cyril's raw_input readers would see only `tool_id`/`arguments`. Cross-ref kas lane (`tool_load`) | NOTE on cyril-2yn8 (pending kas-lane shape) |
| T12 | `/autonomous` local path: `if(!Ka(n.agentEngine)){n.addSystemMessage("Autonomous mode is not available in this session",!0);return}` (0 -> 1); was a silent `return`. Gate `Ka(e)=e==="kas"&&Fn("local_sandbox")` is unchanged | 2.31.0 | `local_sandbox` = `process.env.KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1"` (1 == 1 == 1 == 1) | TUI-local | STATIC | N/A | NOTE on cyril-6vo6 (GA is host-side) |
| T13 | Telemetry: `KIRO_TELEMETRY_EXPLICIT_HISTOGRAMS` opt-out of exponential histograms (0 -> 1) | 2.31.0 | env | telemetry | STATIC | N/A | — |
| T14 | **New rollout gate `managed_settings`** (`Bn("managed_settings")`, 0 -> 5 sites; gate function `Fn` -> `Bn` rename; `KIRO_ROLLOUT_FEATURES` JSON array) | 2.31.1 | off unless the host puts it in `KIRO_ROLLOUT_FEATURES` (registry value: host lane) | client gate | STATIC | N/A | — |
| T15 | `_kiro/governance/state` full parser `J1e`: `{sessionId:string, isEnterprise:boolean, features{mcpEnabled, webToolsEnabled, autonomousAgents, usageAnalytics, promptLogging, codeReferenceTracker, contentCollection}: bool, disabledReason?∈{admin_disabled, api_failure, no_endpoint}}`, mapped to internal `governance_state_update` (0 -> 3). It feeds init errors `mcp_governance_disabled`/`web_tools_governance_disabled`. With the gate off, only legacy `X1e` (`webToolsEnabled===false`) runs | 2.31.1 | `managed_settings` | inbound | STATIC | **D**: normalised `kiro/governance/state` reaches `crates/cyril-core/src/protocol/engine.rs:345-371`, then `convert/kiro.rs:1284` `_ => Ok(None)` | NOTE on cyril-0o7e |
| T16 | `/config managed` tab: title `"/config — managed"` (0 -> 2), columns Setting / Value / "Managed by", footers "Could not read your organization profile; …", "No organization control plane is configured; …", "Managed by your organization. Contact your administrator …" | 2.31.1 | `managed_settings` | TUI-local | STATIC | N/A | (part of cyril-0o7e note) |
| T17 | Pre-session `_kiro/config/template {}` (2nd call site; `"_kiro/config/template"` 1 -> 2) -> `vb(o.configOptions).agents` -> `agents_update`. Runs in `afterInitialize(extensionMethods)` only if `extensionMethods` includes it | 2.31.1 | `managed_settings` | outbound | STATIC | **NS** (no sender in `crates/`) | NOTE on cyril-lyrx |
| T18 | Resource provenance: `_meta.kiro.resource.source.provenance.scope==="organization"` makes the source "organization" (hooks, powers, steering, MCP `markCloudConfigSource`). `provenance` 1 -> 2 | 2.31.1 | `managed_settings` (else "cloud") | inbound meta | STATIC | **D** (cyril reads no `resource.source`) | NOTE on cyril-0o7e |
| T19 | `_kiro/mcp/status` `accessMode==="registry"` + `accessModeFilteredServers[]` -> `mcpRegistryHidden` (0 -> 17) and the copy "hidden: your organization allows only servers from its MCP registry." KAS has had both fields since 0.66.33 (3 == 3) | 2.31.1 | `managed_settings` | inbound | STATIC | **D**: `_kiro/mcp/status` unhandled (`convert/kiro.rs:1284`) | NOTE on cyril-nk4o |
| T20 | Herdr state reporting: `report-agent-session` (1 -> 0) replaced by `report-agent` (0 -> 1) with `--state idle\|working\|blocked --seq N` and optional `--agent-session-id <id> -- kiro-cli chat --agent-engine=v3 [--cloud] --resume-id <id>`. `PPn(e){if(e.pendingApproval\|\|e.pendingQuestion)return"blocked";…}`. Exit claims `release`/`release-local` (0 -> 6), idle on exit | 2.31.1 | `HERDR_ENV==="1"` + `HERDR_BIN_PATH` + `HERDR_PANE_ID` + `HERDR_SOCKET_PATH`, first reporter only (`KIRO_HERDR_REPORTER_PID`) | local IPC (exec), not ACP | STATIC | N/A (cyril has no Herdr integration) | **NEW** (needs-triage, feature) |
| T21 | Workflow sanitiser `Xr` 24 -> 27 calls, `kr` 26 -> 27 (see Method notes). New wraps: `description:Xr(r.validationError??r.description??r.source??"")`, `is invalid: ${Xr(n.validationError)}` ×2, and inputs panel `l(kr(e.description))`. Not wrapped: `r.name` (picker label, alerts, "Run workflow · ${e.name}") and input keys | 2.31.1 | — | inbound text hygiene | STATIC | **H for escapes**: ratatui-core 0.1.0 drops `char::is_control` graphemes (`buffer/buffer.rs:350`, `text/span.rs:314`) and zero-width graphemes. **D for `validationError`**: `crates/cyril-core/src/protocol/convert/kas/workflow.rs:611-617` (`WireRecipe` = name/description/source only), so an invalid recipe lists as valid. **Newline splice**: `crates/cyril-ui/src/workflow_format.rs:88-98` puts the raw `description` into the `/workflow list` system message (`crates/cyril-ui/src/state.rs:1185`) | **NEW** (small) |
| T22 | Mermaid terminal diagrams (flowchart, sequence, state, ER): a code-fence language `mermaid` gets an in-process renderer ("Drawing diagram…" 0 -> 1, `stateDiagram-v2` 0 -> 1, `diagramColor` 0 -> 3). Falls back to the source if too wide or unsupported, and is skipped when ASCII glyphs are active | 2.31.1 | none | TUI-local | STATIC | N/A (no wire field) | — |
| T23 | `/changelog` per-line entries: format `markdown` -> `release-notes` (0 -> 2) with `entryMarkers` (0 -> 5). 2.30.0 had already dropped the `What's new in $1 (` header (1 -> 0) | 2.30.0 / 2.31.1 | — | TUI-local | STATIC | N/A | — |
| T24 | `chat.disableWrap` copy fix: user-prompt `paddingLeft:n==="developer"?zA:0` removed from the wrap-disabled branch (1 -> 0), and the table indent `blockquoteDepth*2+listIndent*2` was dropped | 2.31.1 | — | TUI-local | STATIC | N/A | — |

### Negative results (load-bearing)

| check | 2.29.1 | 2.30.0 | 2.31.0 | 2.31.1 | evidence |
|---|---|---|---|---|---|
| `_kiro/*` / `kiro.dev/*` literal sets | 63/38 | 63/38 | 63/38 | 63/38 | `sets/sets-diff.txt` |
| slash literals / `name:` entries | 74/37 | 74/37 | 74/37 | 74/37 | symmetric diff empty |
| hidden `chat _` subcommands | 11 | 11 | 11 | 11 | `ensure-session … windows-sandbox` |
| settings rows, defaults, mentions, rollout checks | = | = | = | = | `gates-2.29.1-2.31.1.txt`; 9 kB builder window rename-normalised diff shows no logic change |
| agentCaps parsed | 7 keys | = | = | = | `executionTargets, extensionMethods, replayMarking, sessionListScopes, sessionSources, sourceProviders, strictSessionLoad` |
| prompt `_meta.kiro` | displayText, outputStyle | = | = | = | |
| `overrideSessionId` / `requireExisting` / `consentContext` / `replayMarking` / `interaction_resolved` | 1/6/28/15/1 | = | = | = | token counts |
| `subagentOrchestration` / `todoList` in TUI | 0/0 | 0 | 0 | 0 | cyril still sends both (`settings.rs:82,97`) — cyril-53qx unchanged |
| KAS spawn argv | `[prefix,"--experimental-wasm-modules",server,"--transport=stdio","--auth=acp-callback",...Ent(policy),--endpoint/--control-plane-endpoint/--region]` | = | = | = | rename-normalised region diff: renames only |
| KAS spawn env | `{...process.env, NODE_USE_SYSTEM_CA:??"1", NODE_CHANNEL_FD/SERIALIZATION_MODE:undefined, KIRO_OWNED_KAS_PARENT/CONTROL (ownsProcess), KIRO_CUSTOM_USER_AGENT}` | = | = | = | `KIRO_HOME` inherited, never set by the TUI |
| rollout gate names | 9 | 9 | 9 | **10** (+`managed_settings`) | `Fn(`/`Bn(` census |
| V2->V3 offer strings (`V3` token) | 24 | 24 | 24 | 24 | no "switch to V3" family in any bundle |

---

## Unreleased / dark candidates

| candidate | gate | default | evidence |
|---|---|---|---|
| **Org managed settings** (governance parser, `/config managed`, provenance `organization`, MCP registry hidden servers, pre-session `config/template` agents) | rollout `managed_settings` via `KIRO_ROLLOUT_FEATURES` | off for anyone whose host does not list it | `function qrt({client:e,…}){let i=Bn("managed_settings");return{subscribe:()=>{if(!i)return;t(Grt(e,n,a))},afterInitialize:(s)=>{if(!i){t($rt(e,o));return}Prt(e,s,r,a)}}}` |
| **`model_fallback_promoted`** session-info kind | none in TUI; **KAS never emits it** (0 hits in 0.66.33/0.66.34) | latent | `function FJe(e){return e?.kind==="model_fallback"\|\|e?.kind==="model_fallback_promoted"}` … `Dropping model_fallback_promoted notice with a non-refusal trigger` |
| Local `/sandbox` + `/autonomous` | `process.env.KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1"` (TUI special case, not the registry) | the TUI alone keeps it off; the 2.31.0 GA must come from the host exporting the env | `function Bn(e){if(e==="local_sandbox")return process.env.KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1";if(!Znn().has(e))return!1;…}` |
| KAS sandbox argv | `local_sandbox` + policy `requestedState!=="off"` | off | `function Ent(e){if(!Bn("local_sandbox")\|\|!e\|\|e.requestedState==="off")return[];return["--sandbox=auto",`--sandbox-network-mode=${aA(e.networkAccess)}`]}` (unchanged since 2.29.1) |
| Screen-reader auto-detect | `chat.screenReader` | `auto` (VoiceOver detection on macOS only) | `defaultValue:"auto",cycle:[...CY]`; `KIRO_SCREEN_READER` override |
| Herdr `blocked` state | `HERDR_ENV==="1"` + three paths | on only inside Herdr panes | `function PPn(e){if(e.pendingApproval\|\|e.pendingQuestion)return"blocked";…}` |

---

## Leads for the live lane

Every probe uses the full path `initialize.clientCapabilities._meta.kiro[.settings]` (trap 5), `HOME=<tmp>` with the real `XDG_DATA_HOME` (trap 7), and a direct `node acp-server.js` from the 2.31.1 carve (trap 8). No forged backend responses (trap 13).

**L1 — `_kiro/governance/state` shape (T15).** Inbound, no request. After `session/new`, record every `_kiro/governance/state` frame. Expected: `{sessionId, isEnterprise:false, features:{mcpEnabled, webToolsEnabled, autonomousAgents, usageAnalytics, promptLogging, codeReferenceTracker, contentCollection}, disabledReason?}`, with all 7 keys boolean. Record whether `disabledReason` appears (`no_endpoint` is likely for a personal account). Positive control: `_kiro/mcp/status` arrives in the same session (known pushed method, cyril-7q8u).

**L2 — `agentCapabilities._meta.kiro.extensionMethods` (T10, T17).** Request: a plain `initialize` with `clientCapabilities._meta.kiro.settings:{largeToolOutputHandler:{enabled:true}}`. Capture `result.agentCapabilities._meta.kiro.extensionMethods` in full. Does it list `_kiro/config/template` and `_kiro/codeIntelligence`? The 2.31.x TUI gates both calls on it. Positive control: the same object carries `replayMarking:true` and `modelFallbackReporting:true` (LIVE in 2.29.1), and `largeToolOutputHandler` shows its known effect on a large tool output.

**L3 — pre-session `_kiro/config/template {}` (T17, cyril-lyrx).** Send it after `initialize` and before `session/new`. Expected: `{configOptions:…}` whose parsed form exposes `agents[]` (the TUI reads `vb(o.configOptions).agents`). Positive control: the same call after `session/new` must return the output-style catalog the TUI already reads (`uet`/`ynt`, `-32601` handled).

**L4 — `_kiro/mcp/status.accessMode` (T19).** Configure one stdio MCP server and record `_kiro/mcp/status`. Expected for a non-org account: `accessMode` absent or `!=="registry"`, and no `accessModeFilteredServers`. Record the key set for cyril-nk4o. Positive control: `servers[]` lists the configured server with a `status`.

**L5 — KAS `model_fallback` (T1/T2, cyril-aoby).** It cannot be provoked (2.29.1 live lane). Keep `KIRO_ACP_RECORD_PATH` on during every leg and grep for `"kind":"model_fallback"`. If one appears, verify the TUI-read fields `from`, `to`, `trigger`, `scope`, `agentSubtaskId` and the unread `reason`. Check that no `model_fallback_promoted` appears. Positive control: `initialize` result `_meta.kiro.modelFallbackReporting===true`.

**L6 — sandbox config without the argv flag (T12, cyril-6vo6).** The TUI applies sandbox **two ways**: spawn argv `--sandbox=auto --sandbox-network-mode=<m>`, and `session/set_config_option {sessionId, configId:"sandbox", value:"enabled"}` plus `{configId:"mcpSandboxing", value:"enabled"|"disabled"}`. Legs: (a) no argv flag, `set_config_option sandbox=enabled`; (b) argv flag only; (c) both. Expected signal: a `_kiro/sandbox/status` push and `configOptions` listing `sandbox`. Positive control: `session/new` `configOptions` must contain the `mode` option, which is known.

**L7 — `KIRO_HOME` rejection in KAS (T8).** Spawn KAS with `KIRO_HOME=relative/dir`, then with `KIRO_HOME=$HOME` (contains-home). Seed a distinct `settings/cli.json` under the tmp `~/.kiro` and under the bogus path. Expected (if KAS mirrors the TUI): KAS reads `~/.kiro`, and at debug level it logs an ignore warning. Positive control: a valid absolute `KIRO_HOME=<tmp>/kh` must be honoured, observable through a seeded `mcp.json` server appearing in `_kiro/mcp/status`.

---

## Changelog reconciliation (feed-2.30.0, feed-2.31.0, feed-2.31.1; 31 lines)

`L` = located in the TUI bundle.

| # | rel | line | location |
|---|---|---|---|
| 1 | 2.30.0 | [V3] Notices in the transcript when a model refuses or is unavailable and the turn moves to another model | **L, T1–T3**: new KAS `session_info_update` consumer (`FJe`/`PJe`), shared renderer copy. KAS is byte-identical, so this is a TUI change |
| 2 | 2.30.0 | [V3] Agent upgrades refresh permissions after V2 setting changes until you edit them | **not in my lane** (host `chat _ upgrade-agent`). TUI part = report label "Upgrade or refresh the universal (V2 + V3) config" (T7) |
| 3 | 2.30.0 | Kiro CLI may now offer to switch you to V3 when you start a chat | **not in the TUI**: no new strings, `V3` 24 == 24, no engine-preference key added (only `chat.screenReader` added to the `chat.*` key set). Host lane |
| 4 | 2.30.0 | Screen readers follow streaming replies … `/settings display` | **L, T4** |
| 5 | 2.30.0 | [V3] Agent upgrades preserve V2 workspace read access and `fs_read` skill trust | **not in my lane** (host upgrade). TUI part = the `skill-read-restrictions` diagnostic (T7) |
| 6 | 2.30.0 | Scrolling back in `tmux` no longer shows blank rows | **L, T5** (TUI-local) |
| 7 | 2.30.0 | Launching after output without a trailing newline no longer draws at an offset | **L, T5** (cursor-position extraction rewrite, TUI-local) |
| 8 | 2.30.0 | Knowledge search no longer silently drops a chunk | **not in my lane**: no TUI delta, KAS identical, so host v2 |
| 9 | 2.30.0 | Garbled text and stray characters around icons | **L, T6** |
| 10 | 2.31.0 | [V3] `/sandbox` and local `/autonomous` available to all users, off by default | **TUI gate unchanged** (T12, env `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`). TUI adds only the "not available" message. GA must be the host exporting the env or the registry. Host lane |
| 11 | 2.31.0 | [V3] MCP tools load on demand for every user | **L (display only), T11** (`{tool_id, arguments}` unwrap). Engine side = kas lane (`tool_load`) |
| 12 | 2.31.0 | [V3] `KIRO_HOME` also relocates MCP config, permissions, sessions, powers, agent state | **L partly, T9**: TUI-owned powers and secrets migrate. The rest is KAS/host |
| 13 | 2.31.0 | [V3] saved `sandbox.json` `enabled:true` applies to headless runs | **not in the TUI** (`sandbox.json` 7 == 7, no `headless` change). KAS/host |
| 14 | 2.31.0 | `--no-interactive` on V1 exits non-zero … | **not in my lane** (V1 Rust) |
| 15 | 2.31.0 | [V3] MCP sign-ins saved under `KIRO_HOME`; existing copied once | **L partly, T9**: `[secret-storage] copied legacy secrets into KIRO_HOME` (the TUI secret store backs `_kiro/secret/*` for MCP OAuth) |
| 16 | 2.31.0 | [V3] Animated WebP sent as PNG | **not in the TUI** (`image/webp` 0 in all). KAS lane |
| 17 | 2.31.0 | [V3] Kiro service requests fail after 15 s | **not in the TUI** (no new 15000 timeout). KAS/host |
| 18 | 2.31.0 | [V3] streamable-HTTP MCP tools survive a session drop | **not in my lane** (KAS) |
| 19 | 2.31.0 | Relative, root or home-folder `KIRO_HOME` ignored | **L, T8** (`L9t`/`O9t`, returns `relative`\|`root`\|`contains-home`) |
| 20 | 2.31.0 | [V3] Cloud sessions: code intelligence indicator and `/code` reflect the cloud workspace | **L, T10** |
| 21 | 2.31.0 | GitHub/Google sign-ins survive a temporary auth error during refresh | **not in my lane** (host auth) |
| 22 | 2.31.1 | Mermaid diagrams drawn as terminal art | **L, T22** (TUI-local, no wire field) |
| 23 | 2.31.1 | [V3] Non-interactive runs apply saved compaction and `toolSearch` settings | **not in the TUI** (`toolSearch` 7 == 7, settings builder unchanged). Host/KAS headless path |
| 24 | 2.31.1 | `chat.disableWrap` / lite panes copy without two leading spaces | **L, T24** |
| 25 | 2.31.1 | [V3] Herdr shows a session as blocked while a permission or question prompt waits | **L, T20** |
| 26 | 2.31.1 | Model fallback handles refusals that return no response text | **no 2.31.1 TUI delta** (no fallback hunk in 2.31.0 -> 2.31.1; the `faulted`/"produced no answer" path shipped in 2.30.0). KAS 2.31.1 == 2.31.0, so this is host/v2 or a late-listed 0.66.34 change (kas lane) |
| 27 | 2.31.1 | Compaction no longer saves the model's refusal as the summary | **not in the TUI** (no compaction hunk). Host/v2 or KAS 0.66.34 |
| 28 | 2.31.1 | [V3] `/sandbox` setup on Windows survives temp-folder policy | **not in the TUI** (no windows/temp hunk). Host `chat _ windows-sandbox` |
| 29 | 2.31.1 | `/changelog` shows each entry on its own line | **L, T23** |
| 30 | 2.31.1 | IAM Identity Center sign-in in isolated partitions | **not in my lane** (host auth) |
| 31 | 2.31.1 | [security] Workflow recipe text can no longer emit terminal escape sequences in `/workflow` | **L, T21**: four new sanitiser call sites. Recipe `name` and input keys stay raw (presumably schema-constrained by KAS; kas lane to confirm) |

Also located, with no changelog line: the `managed_settings` org-governance surface (T14–T19). That is the largest 2.31.1 TUI delta, and it is dark.

---

## Proposed issues (checked against `.rivets/issues.jsonl`)

- **NOTE cyril-aoby**: the 2.30.0 TUI consumer is the reference for this issue. Add the field map (T1), the dark `model_fallback_promoted` kind (T2), the `scope:"session"` promotion and the `agentSubtaskId` drop. Quote the render copy (T3).
- **NOTE cyril-ci3j / cyril-pouu**: the shared renderer gained the `unknown`->"yielded" cause and the `faulted` outcome.
- **NOTE cyril-0o7e**: the full `_kiro/governance/state` contract per the 2.31.1 parser, the `managed_settings` gate, `/config managed`, and provenance `scope:"organization"`.
- **NOTE cyril-nk4o**: `_kiro/mcp/status.accessMode`/`accessModeFilteredServers`.
- **NOTE cyril-lyrx**: the TUI now calls `config/template` pre-session for `agents`, gated on `extensionMethods`.
- **NOTE cyril-tikf**: `extensionMethods` is the capability the TUI uses to feature-detect `_kiro/codeIntelligence` and `_kiro/config/template`. cyril does not parse it.
- **NOTE cyril-6vo6**: the local-sandbox gate is env-forced in the TUI. Record the two-way sandbox application (L6).
- **NOTE cyril-2yn8** (provisional, pending the kas lane): the `{tool_id, arguments}` rawInput wrapper.
- **NEW: Workflow recipe text hygiene + `validationError`.** Model `listRecipes[].validationError` (`convert/kas/workflow.rs:611-617`) and render invalid recipes as invalid. Collapse `[\t\n\r]+` in recipe `description`/`source` before splicing them into `/workflow list` (`cyril-ui/src/workflow_format.rs:88-98`). Escapes are already neutralised by ratatui. Related: cyril-0asq.
- **NEW: `KIRO_HOME` resolution parity with 2.31.0.** Trim the value, and ignore relative, root and home-containing values with a warn. Apply it in `kas/settings.rs:50-58` and `kiro_agent_config.rs:93-97`. Also route `global_agents_dir` (`kiro_agent_config.rs:102-104`) and the hooks registry home (`domain_mediator/mod.rs:264-266`) through `kiro_home_dir`.
- **NEW (needs-triage, feature): Herdr pane-state reporting.** When `HERDR_ENV=1` and the bin, pane and socket are set, exec `pane report-agent <pane> --source <src> --agent <agent> --state idle|working|blocked --seq <n>`. `blocked` = an approval or question overlay is open. Also exec `pane release-agent` on exit. Whether Herdr accepts a non-`kiro-cli` `--source` is unknown (static lead).

---

## Method notes

- **Scripts** (new and reusable, in `experiments/conductor-spike/`; the 2.29.1 originals are untouched):
  - `static-tui-vocab-2.31.1.py`: copy of the 2.29.1 lexer differ.
  - `static-tui-gates-2.31.1.py`: copy of the 2.29.1 gates extractor.
  - `static-tui-sets-2.31.1.py`: trap-3 families for N bundles plus consecutive and overall diffs. Adds `case"x_y"`, `kind===`, `type===`, rollout-call and `name:"/x"` families.
  - `static-tui-region-2.31.1.py`: anchored region print plus a rename-normalised `--diff`.
  - `static-tui-stmtdiff-2.31.1.py`: whole-bundle statement dump with identifiers of 4 characters or fewer replaced by `_`, for GNU `diff`.
  - The lane-local `stmt-filter.py` drops hunks whose sides are equal after normalising every identifier of 6 characters or fewer. That leaves 119/44/154 real hunks out of 593/324/681.
- **Raw outputs** (this directory): `vocab/` (consecutive pairs), `vocab-overall/` (2.29.1 -> 2.31.1: +145/−12 literals), `sets/`, `gates-2.29.1-2.31.1.txt`, `stmt/d-*.diff` and `stmt/d-*.real.txt` (the 13 MB per-version statement dumps were deleted after diffing; regenerate with `static-tui-stmtdiff-2.31.1.py`), `progress.log.md`.
- **Sanitiser facts (T21).**
  - `function kr(e){if(e==="")return"";return e.replace(eTe,"").replace(tTe,"").replace(grn,"")}`.
  - `eTe=QE()` is the ANSI/OSC regex. `tTe=bM()` is `[\x00-\x08\x0b\x0c\x0e-\x1a\x1c-\x1f\x7f\x84\x85\x88\x8d\x8e\x8f\x9c]`. `grn=/[\r؜‎‏‪-‮⁦-⁩]/gu`.
  - `Xr` truncates at `hrn=4096` (surrogate-safe), then applies `kr`, then `.replace(/[\t\n]+/gu," ")`.
  - The function bodies were matched by pattern in every bundle (`ya`/`vr` in 2.29.1, `wa`/`xr` in 2.30.0 and 2.31.0, `Xr`/`kr` in 2.31.1). Call counts are 24/24/24/27 and 26/26/26/27.
- **Fallback consumer location (T1).** In the KAS session-update mapper (`tee` in 2.30.0, `jee` in 2.31.1), `r=C_(n)` with `function C_(e){return e._meta?.kiro}`. The branch `if(FJe(r)){for(let a of PJe(r))t.emitSideEffect(a);return null}` sits after `user_message_id_assigned` and the steering switch, and before `turn_start`.
  - The KAS emitter (cross-ref, read-only) is `function tIt(e){return{from:e.from,...e.to!==void 0&&{to:e.to},...e.scope!==void 0&&{scope:e.scope},trigger:e.trigger,...e.reason!==void 0&&{reason:e.reason},...e.agentSubtaskId!==void 0&&{agentSubtaskId:e.agentSubtaskId}}}`. It is also replayed from session metadata key `modelFallback`.
- **Trap compliance.**
  - Every add or remove above carries a count in all four bundles (§Deltas, §Negative results), backed by verification greps run this session.
  - Minifier renames are not counted as changes: `Fn`->`Bn`, `Kn`->`Wn`, `ya`->`wa`->`Xr`, `vr`->`xr`->`kr`, `VRn`->`GOn`, `met`->`Ent`, `$tt`->`aat`, `Htt`->`oat`.
  - The 2.31.1 statement-diff volume (4,262 diff lines) is mostly the Mermaid renderer (one ~970-line hunk). Do not read it as a behaviour estimate.
  - No row is LIVE. Every cyril verdict cites `file:line`, or is marked N/A because it is TUI-local with no wire part.
## Progress log
- [P1] vocab differ (static-tui-vocab-2.31.1.py -> vocab/, vocab-overall/): 2.29.1->2.30.0 +41/-10 literals; 2.30.0->2.31.0 +18/-0; 2.31.0->2.31.1 +86/-3; overall +145/-12. env reads +KIRO_TELEMETRY_EXPLICIT_HISTOGRAMS (2.31.0).
- [P2] trap-3 sets (static-tui-sets-2.31.1.py -> sets/): _kiro/* 63==63==63==63, kiro.dev/* 38 all, slash literals 74 all, slash name: entries 37 all, chat _ subcmds 11 all, ext_method 20 all. New: KIRO_SCREEN_READER (2.30.0), KIRO_TELEMETRY_EXPLICIT_HISTOGRAMS (2.31.0), kind=== model_fallback/model_fallback_promoted (2.30.0), case governance_state_update (2.31.1), rollout gate managed_settings (2.31.1, 5 call sites; gate fn Fn->Bn rename).
- [P3] gates extractor: settings rows/defaults/mentions, agentcaps, spawn argv/env, prompt meta, subcmds, method list 113 — IDENTICAL in all four bundles. Settings builder (9 kB window) rename-normalised diff: no logic change.
- [P4] local_sandbox gate unchanged in all four: `function Bn(e){if(e==="local_sandbox")return process.env.KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1";if(!Znn().has(e))return!1;...}` (Znn = KIRO_ROLLOUT_FEATURES JSON array). /autonomous local path still `Ka(engine)= e==="kas"&&Fn("local_sandbox")`; 2.31.0 only adds the message "Autonomous mode is not available in this session" (0->1) instead of a silent return. => the 2.31.0 "/sandbox + local /autonomous GA" line is NOT a TUI gate change; it must be the host setting KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED=1 / rollout registry (host lane).
- [P5] stmt diffs (static-tui-stmtdiff-2.31.1.py + stmt-filter.py -> stmt/*.real.txt): real (non-rename) hunks 119 (2.30.0), 44 (2.31.0), 154 (2.31.1).
- [P6] model_fallback (2.30.0): new KAS consumer in the session_info_update mapper `tee` (r = n._meta.kiro): `if(FJe(r)){for(let a of PJe(r))t.emitSideEffect(a);return null}`; FJe = kind==="model_fallback"||"model_fallback_promoted"; fields read from, to?, trigger ("refusal"->refused, "availability"->unavailable, else "unknown"), scope ("session" => extra promoted:true event; "turn" => none), agentSubtaskId (non-empty => ignored). `reason` NOT read. KAS 0.66.33/34 emitter `tIt` = {from,to?,scope?,trigger,reason?,agentSubtaskId?}; KAS has 0 `model_fallback_promoted` (TUI forward-compat). cyril: no arm, convert/kas.rs:525 `_ => None` (cyril-aoby).
- [P7] governance (2.31.1): new rollout gate `managed_settings` (Bn 5 sites). ON: `_kiro/governance/state` -> J1e {sessionId,isEnterprise,features{mcpEnabled,webToolsEnabled,autonomousAgents,usageAnalytics,promptLogging,codeReferenceTracker,contentCollection},disabledReason∈{admin_disabled,api_failure,no_endpoint}} -> internal governance_state_update -> "/config — managed" panel; afterInitialize calls `_kiro/config/template` if extensionMethods includes it; resource `_meta.kiro.resource.source.provenance.scope==="organization"`; MCP `accessMode==="registry"` + `accessModeFilteredServers[]` -> hidden servers. OFF: legacy X1e (webToolsEnabled===false only).
- [P8] herdr (2.31.1): exec `$HERDR_BIN_PATH pane report-agent $HERDR_PANE_ID --source kiro-cli --agent kiro --state idle|working|blocked --seq N [--agent-session-id <id> -- kiro-cli chat --agent-engine=v3 [--cloud] --resume-id <id>]`; was `report-agent-session` (resume only) in <=2.31.0. blocked = pendingApproval||pendingQuestion. Gate: HERDR_ENV=1 + HERDR_BIN_PATH + HERDR_PANE_ID + HERDR_SOCKET_PATH, not under another reporter (KIRO_HERDR_REPORTER_PID).
- [P9] workflow sanitiser (2.31.1): `Xr` (truncate 4096, strip ANSI via QE(), C0/C1 via bM(), CR+bidi via /[\r؜‎‏‪-‮⁦-⁩]/, [\t\n]+ -> " ") call sites 24 -> 27: recipe picker `description:Xr(r.validationError??r.description??r.source??"")` + two `is invalid: ${Xr(n.validationError)}` alerts. recipe `name` NOT wrapped. cyril: renders via ratatui (ratatui-core 0.1.0 buffer.rs:350 / span.rs:314 drop is_control graphemes; zero-width bidi graphemes filtered by width>0) — no escape emission; but cyril drops `validationError` (convert/kas/workflow.rs:610-617) and splices raw description newlines into the /workflow list text (cyril-ui/src/workflow_format.rs:93-98).

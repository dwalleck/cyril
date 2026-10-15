# KAS lane (kas-core + kas-workflow) — kiro-cli 2.31.1 audit, KAS 0.66.33 -> 0.66.34

Status: COMPLETE (static). Every row is STATIC: nothing from Kiro was executed. Baseline carve
2.29.1 (== 2.30.0) vs carve 2.31.1 (== 2.31.0); the only file delta is
`dist/server/acp-server.js` (12,669,464 -> 12,702,158 B) + `package.json` version.

Method / artefacts (all under `experiments/conductor-spike/`):
- `static-kas-bundle-diff-2.31.1.py` (NEW): lexes both bundles (strings, template chunks, regex
  literals), splits on `; { }`, normalises renameable identifiers to `$` -> `audit-2.31.1/kas/bundle/{old,new}.{raw,norm}`;
  `diff old.norm new.norm` = `bundle/norm.diff` (433 hunks, 783 old / 1812 new statements);
  token-level render `bundle/hunks.txt` (`static-kas-hunks-2.31.1.py`), rename-only swaps filtered in `bundle/hunks-f.txt` (+`part-0*`).
  Every hunk in `hunks-f.txt` was read.
- `static-kas-literals-2.31.1.py` (NEW): lexer-exact string-literal multiset diff (trap 2/3) ->
  `audit-2.31.1/kas/literals-delta.txt` (347 literals changed count; 173 new-only; 56 old-only).
  Supersedes the regex `static-kas-identifiers` output (`identifiers/`), which mis-reported 3
  pre-existing workflow literals (`workflow.inspect.liveness_failed`, ...) as added — verified
  present in OLD at old.raw:290463/290494/290508.
- Reused read-only 2.29.1 scripts: surface (`surface/`), flags (`flags-old.txt`/`flags-new.txt`),
  settings (`settings.txt`), session-info kinds (`session-info-kinds.txt`).
- `bundle/{old,new}.{raw,norm}` are ~50 MB and regenerable from the carves (one command, 7 s): do not commit them;
  commit `norm.diff`, `hunks*.txt`, `part-0*`.
- Counts below are `grep -oF` occurrences in OLD -> NEW bundles unless noted.

## Summary

- KAS 0.66.34 is a **KIRO_HOME + MCP-reliability + workflow-hardening** release. No `session_info_update` kind,
  `sessionUpdate` variant, `_meta.kiro` client key, `AgentSettings` key, configOption, auth mode or workflow event literal changed.
- **One new method: `_kiro/environment/start_ssh_session`** (advertised unconditionally in `extensionMethods`, functional only
  with `--remote-sessions-endpoint`/`KIRO_REMOTE_SESSIONS_ENDPOINT`) — dark cloud-session SSH (SSM) credential minting.
- **`KIRO_HOME` is now read by KAS** (0 -> 1; `userConfigDir` 0 -> 147): agents, hooks, settings/mcp.json, sessions, logs, memories,
  knowledge, powers, steering/skills/prompts, user workflow recipes, tasks, cloud-cache, sandbox grants. Relative / root /
  home-containing values are ignored (fallback `~/.kiro`). cyril now matches KAS for sessions + settings, but **diverges for
  hooks and global agents** (hard-coded `~/.kiro`) and for the rejection rules.
- **MCP deferral is now unconditional**: `AB_TOOL_LOAD` / `KIRO_FEATURE_TOOL_LOAD_ENABLED` / the `tool_search` tool /
  `KIRO_TOOL_SEARCH_THRESHOLD` are deleted; every session runs `"mcp-only"` deferral through `tool_load`.
  `toolSearch.enabled` (which cyril marshals) is now dead; only `toolSearch.neverDefer` is consumed.
- **Memory default flipped** `{mode:"read_write",reflection:true}` -> `{mode:"disabled",reflection:false}` when no layer sets `memory`.
- **Workflows**: `_kiro/workflow/new` + `run_workflow` accept launch defaults `modelId` / `effortLevel` (cyril NOT-SENT);
  concurrency guards on load/invoke/retry/send_message; bundled `feature-pipeline` steps gain `effortLevel:"high"`.
- **Escape-sequence [security] line is not KAS code** (KAS 2.31.0 == 2.31.1; no sanitiser added). KAS emits recipe text verbatim;
  cyril is protected from terminal escapes by ratatui-core's control-grapheme filter, residual risk is newline/markdown row spoofing.
- Every 2.30.0 and 2.31.1 changelog line is NOT KAS code (KAS identical across each pair); only 2.31.0 lines can be KAS.

## Deltas (core)

| item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| `_kiro/environment/start_ssh_session` client->agent method. Literal 0 -> 4; `_kiro/environment/` 0 -> 5. Added to the initialize `_meta.kiro.extensionMethods` list (new.raw:330338: `extensionMethods:["_kiro/knowledge",..._rr,"_kiro/codeIntelligence","_kiro/environment/start_ssh_session","_kiro/session/context",…`) and to both dispatch case lists (new.raw:313056, 315838) | 0.66.34 (2.31.0) | Functional only when a remote-sessions endpoint is set (`UT("remote-sessions-endpoint")??process.env.KIRO_REMOTE_SESSIONS_ENDPOINT`, pre-existing gate, count 1 -> 1); otherwise rejects `-32000` `SshSessionUnavailableError` | agent capability + new method | STATIC | NOT-SENT (cyril never calls it; cyril does not read `extensionMethods` — only fixture at protocol/fingerprint.rs:138) | NOTE on cyril-tikf (remote sessions / executionTargets family); no issue (dark) |
| `KIRO_HOME` -> `userConfigDir` resolution `P8o` (new.raw:324167): `KIRO_HOME` 0 -> 1, `userConfigDir` 0 -> 147, `agent.userConfigDir.{ignored,resolved,createFailed}` 0 -> 1 each, `contains-home` 0 -> 1. Rule: trimmed empty -> `~/.kiro`; not absolute -> ignored `relative`; filesystem root -> ignored `root`; equal to / ancestor of `$HOME` -> ignored `contains-home`; else `path.resolve(KIRO_HOME)`, `mkdirSync(recursive, mode)` | 0.66.34 | env, default `~/.kiro` | config relocation | STATIC | sessions: HANDLED-by-accident — cyril already reads KAS session dirs at `kiro_home_dir()/sessions` (usage/kiro_sidecar.rs:513-514 via kiro_agent_config.rs:93-98); 0.66.33 KAS ignored KIRO_HOME, so this was a latent mismatch that 0.66.34 fixes. settings: HANDLED (protocol/kas/settings.rs:50-59 reads `$KIRO_HOME/settings/cli.json`). DIVERGES: hooks loaded from `home_dir()/.kiro` (protocol/domain_mediator/mod.rs:262-266) while KAS 0.66.34 reads `<userConfigDir>/hooks` (new: `let r=t.userConfigDir;this.homeGlobalHooksDir=B0t.join(r,O$)`); global agents `~/.kiro/agents` (kiro_agent_config.rs:102-104) while KAS loads `Rd.join(this.userConfigDir,_I)` (new.raw:331137); cyril `kiro_home_dir` accepts relative/root/home KIRO_HOME verbatim (kiro_agent_config.rs:93-98, settings.rs:50-59) | NEW issue: "KIRO_HOME parity with KAS 0.66.34 userConfigDir: hooks + global agents + ignore rules" (P3) |
| Paths moved under `userConfigDir` (old `join(homeDir,".kiro",X)` -> new `join(userConfigDir,X)`): `sessions` (old `sessionsPath=…join(this.homeDir,".kiro","sessions")`), `logs`, `memories`, `knowledge_bases`, `models`, `cache` (unless `XDG_CACHE_HOME`), `tasks`, `spec-sessions`, `cloud-cache/<scope>/{config,staging}`, `enterprise-management`, `settings/{mcp.json,kiroignore,knowledge.json}`, `sandbox-state`, prompts/skills/steering/powers/agents/hooks/workflows watchers. `".kiro"` literal 43 -> 28 | 0.66.34 | follows KIRO_HOME | relocation | STATIC | n/a except rows above | covered by KIRO_HOME issue |
| Sandbox (macOS seatbelt / Linux bind): `(allow file-write* (subpath (string-append (param "HOME_DIR") "/.kiro")))` 1 -> 0, replaced by a per-sandbox `(allow file-write* (subpath "${this.userConfigDir}"))`; Linux `bindIfExists(this.userConfigDir)`; seatbelt rule `(allow network-outbound (local ip "localhost:*"))` 1 -> 0, `(remote ip "localhost:*")` 0 -> 1 | 0.66.34 | `sandboxConfig` (pre-existing; 22 -> 22) | behaviour fix | STATIC | NOT-SENT (cyril sends no sandbox config; `_kiro/sandbox/{applyConfig,status}` unchanged) | NOTE on cyril-be7e / cyril-6vo6 |
| Deny-write protection extended to the relocated config dir (`setDeniedWritePaths([...mcu(t.authority),...T0o(t.userConfigDir)])`; `kiro-scope` evaluator now keys `home` role on `userConfigDir`) | 0.66.34 | always | permission engine | STATIC | n/a (engine-side; permission requests arrive via the unchanged `session/request_permission`) | none |
| MCP deferral unconditional: `AB_TOOL_LOAD` 4 -> 0, `abToolLoadEnabled` 5 -> 0, `tool_load_enabled` 1 -> 0, `KIRO_FEATURE_TOOL_LOAD_ENABLED` 1 -> 0 (flag table 31 -> 30 flags, 16 -> 15 env-reachable); `pD=Rt?"all-tools":Ze?"mcp-only":"off"` -> `$h=J?"all-tools":"mcp-only"` (new.raw:305392). `tool_search` tool deleted (`"tool_search"` 7 -> 3, remaining 3 = OpenAI server-tool parsing + tool-name map; `ToolSearch` 1 -> 0; `KIRO_TOOL_SEARCH_THRESHOLD` 1 -> 0; `available-deferred-tools` 5 -> 0); zod key `max_results` removed | 0.66.34 | was AB-gated default off, now ON for all | tool surface | STATIC | `tool_load` tool_calls will now appear whenever MCP tools exist; cyril has no `tool_load` special-casing (no match in crates/) — rendered as an ordinary tool call (or filtered if kind maps to Other) | live lead L3; no issue until rendered |
| `toolSearch` settings: schema unchanged (`toolSearch:JXs.optional()`, JXs = `{enabled,minPct,minTokens,neverDefer}`), but the `toolSearch.enabled` consumer `B=!q&&cR(oe,"toolSearch")` is gone (`"toolSearch"` literal 2 -> 1); `minPct`/`minTokens` consumed nowhere in either version (1 -> 1 = schema only); `neverDefer` still read (`new Set(oe.toolSearch?.neverDefer??[])`, 2 -> 2) | 0.66.34 | n/a | dead setting | STATIC | NOT-EFFECTIVE: cyril marshals `toolSearch:{enabled,minPct,minTokens}` (protocol/kas/settings.rs:120-131); `neverDefer` NOT-SENT | NOTE on cyril-q8gq / cyril-ea67 |
| Memory default `Object.freeze({mode:"read_write",reflection:!0})` 1 -> 0 -> `Object.freeze({mode:"disabled",reflection:!1})`; used by `captureMemoryConfig` (new.raw:333323: `memoryConfig:V2n(Q3(r),Q3(i)),memoryConfigSource:o(r)||o(i)?"explicit":"legacy"`) and `sx()` for workflow step sessions | 0.66.34 | applies when neither session request `settings.memory` nor `initialize.clientCapabilities._meta.kiro.settings.memory` sets mode/reflection | default flip | STATIC | NOT-SENT: cyril sends no `memory` setting (settings.rs BOOL_MAP/DEFAULTS_ON have no memory key) → KAS native memory is now off for cyril sessions even if AB_MEMORY_* opens | NOTE on cyril-0na7, cyril-s21o, cyril-l4e7 |
| Memory-extraction background prompt rewritten ("Capture durable, cross-session memories per the bar below…", DURABLE/USEFUL LATER/NON-OBVIOUS/GROUNDED/ACTIONABLE/STANDALONE bar) | 0.66.34 | AB_MEMORY_* (dark) | prompt text | STATIC | n/a | NOTE on cyril-s21o |
| 15 s service timeout: three `KiroControlPlaneBearerClient`s (`listAvailableModels`, `getEnterpriseManagement`, `getProfile`) switch from inline `NodeHttpHandler({requestTimeout:15e3,connectionTimeout:15e3,…})` (`requestTimeout:15e3` 3 -> 0) to shared `BG(l)` -> `Fat(e,{connectionTimeoutMs:15e3,requestTimeoutMs:15e3})` which sets `throwOnRequestTimeout:!0` (`requestTimeoutMs:15e3` 0 -> 1; `BG(` call sites 1 (`getUsageLimits`, already) -> 4). Before, smithy only warned on request timeout, so these calls could hang | 0.66.34 | always | reliability | STATIC | n/a (failure surfaces through existing error paths) | none; passive live lead only (trap 13: no forging) |
| Animated WebP: format choice `u==="jpeg"||u==="webp"?"jpeg":"png"` -> `!l&&(u==="jpeg"||u==="webp")?"jpeg":"png"` (`l`=animated; new.raw:215387); new RIFF chunk walker + ANMF frame validation (`webp animation has no canvas` 0 -> 2, 11 new `webp … cut short`/mismatch errors); photon worker downscale rewritten (alpha-weighted box filter replaces `photon.resize(…Triangle)`) | 0.66.34 | always | image pipeline | STATIC | n/a (cyril passes image bytes through) | none |
| Backend request reasoning block: `jHc` -> `L9c` now emits `reasoning:{type?,effort?,summary?}` from model config `thinking.type` / `effort` / `thinking.display`; the special case `reasoning:{effort:"none"}` (1 -> 0) is removed | 0.66.34 | per model schema | outbound-to-backend only | STATIC | n/a (not on ACP) | none |
| MCP streamable-HTTP session recovery: session-loss classifier (`json-rpc`/`session-status`/`worded-status`), serialized per-server reset queue (`inFlightResets`, triggers `session-loss`/`sandbox-reconnect`/`user-retry`), server-identity hash (`LPt`, sha256 of normalized config) bound at approval time, refusal reports returned as `HandledError` tool results ("…MCP server configuration changed while this call was waiting…", "…MCP session expired during this call…") ; new metrics `mcpToolCallRefused`, `mcpSessionRecovery`, `resetServerOutcome` (0 -> 1 each); `_kiro/mcp/resetServer` now awaits a bounded reset ceiling | 0.66.34 | always | reliability | STATIC | HANDLED as ordinary tool-call result text (no new wire field) | none |
| web_fetch tool: `"rendered"` mode removed from enum (`Rendered mode is not available…` 1 -> 0, `WebFetchRendererError` 1 -> 0); default user agent `"KiroIDE"` 3 -> 2 (now explicit `userAgent` per origin, server = `KiroServer/1.0`) | 0.66.34 | always | tool schema (model-facing) | STATIC | n/a | none |
| read_file truncation notes reworded; `readPath` dropped from result object | 0.66.34 | always | tool text | STATIC | n/a | none |
| Session load: `loadMessagesWithRaceGuard` returns `{persisted, arrivedDuringLoad}`; abandoned-prompt-closure replay suppressed when messages arrived during the load (`arrivedDuringLoad` 0 -> 4) | 0.66.34 | always | `session/load` behaviour | STATIC | n/a | NOTE on cyril-rtrh |
| Steering watcher: `AGENTS.md` moved to `workspaceOnlyPaths` (`workspaceOnlyPaths:[` 0 -> 1); watchers now throw if a user watch path does not start with `.kiro/` | 0.66.34 | always | hot-reload scope | STATIC | n/a | none |
| Unchanged (set-diffed, zero delta): `session_info_update` kinds 25 -> 25; `_meta.kiro` keys 15 -> 15; `settings` keys 11 -> 11; AgentSettings zod keys +0/-0 and known-extra-keys list identical; ACP method literals 25 -> 25; snake_case lifecycle literals 44 -> 44; auth (`authMethod` hunks are code moves); `model_fallback_refusal`/`model_fallback_availability` still default `false`; telemetry flag `kuts_telemetry` default `true` | — | — | — | STATIC | session_info coverage: cyril handles 8/25 (`hook_update` convert/kas.rs:371, `turn_completion` :390, `turn_start` :397, `turn_end` :398, `context_usage` :401, `steering_queued` :464, `steering_injected` :472, `steering_cleared` :503); the other 17 incl. `model_fallback` DROPPED unlogged at convert/kas.rs:525 (`_ => None`) | NOTE on cyril-58uv, cyril-aoby |

## Workflows

| item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| `_kiro/workflow/new` launch defaults: `p=mcu(c)` validates `{modelId,effortLevel}` via `of.pick({modelId:!0,effortLevel:!0})` (recipe schema) BEFORE path resolution; `h=wxo(h,p)` overrides the recipe's top-level `modelId`/`effortLevel` (`effortLevel:""` = absent); debug log `workflow.new.launch_defaults` 0 -> 1. Invalid shape -> `Invalid workflow shape: …` | 0.66.34 | optional params | new params | STATIC | NOT-SENT: cyril sends only `workflowPath,inputs,parentSessionId,workspacePaths` (crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs:349-356; also no `runLabel`) | NOTE on cyril-4u4a (or new P3 "/workflow run --model/--effort/--label") |
| `run_workflow` tool schema +`modelId`, `effortLevel` ("Default model for steps that don't set their own. Omit to inherit this session's." 0 -> 1, same for effort); description drops "Model and effort inherit from the parent session."; re-dispatch hint now includes `, modelId: …, effortLevel: …` | 0.66.34 | model-facing | tool schema | STATIC | n/a (model-side tool) | none |
| Bundled recipe `feature-pipeline`: steps `plan` (agent `wf-planner`) and `code-aggregate` (`wf-review-aggregator`) gain `effortLevel:"high"` (`effortLevel:"high"` 0 -> 2; other levels unchanged: xhigh 5, low 2, cascade 1) | 0.66.34 | bundled | recipe content | STATIC | n/a | none |
| Recipe schema (`of`, step `V7`, state `K7`, node `$rt`): no change beyond minifier rename `Dg` -> `of`; `docs/kiro-kas-workflow-recipe-schema-2.26.0.json` remains valid | — | — | unchanged | STATIC | n/a | none |
| `send_message` completion signal: rewritten into `completionSignalTarget` (`kind:"answered"|"write"`); if the signal cannot be persisted, `noteUnrecordedStepSignal` makes the step park awaiting user at turn end with reason "Step '<id>' sent a send_message <success/warning/error> signal that could not be saved, so it is paused awaiting user input instead of finishing." (`unrecordedStepSignals` 0 -> 10; log `workflow.step.signal_unrecorded` 0 -> 1) | 0.66.34 | always | new pause cause (existing event literals) | STATIC | HANDLED (STATIC inference) via the existing `node_paused` path: crates/cyril-core/src/workflow.rs:508 / :667 `apply_node_paused`; assumes the park emits the pre-existing node_paused event (live-verify) | none |
| Liveness: same-process claim queue `takeLocalClaimTurn`/`waitForLocalClaimant` (`localClaimTurns` 0 -> 5; logs `workflow.liveness.claim_queued_local`, `…claim_local_holder_overstayed`); `forgetRemovedMarker` on `remove` | 0.66.34 | always | concurrency | STATIC | n/a | none |
| `_kiro/workflow/load` coalescing (`loadsInProgress` 0 -> 4): concurrent loads of one id share a promise; new error "Workflow '<id>' became <status> while this load waited for its ownership claim; it was not loaded." | 0.66.34 | always | new error text | STATIC | surfaced as a generic workflow failure (commands/kas.rs:362-374 `workflow_extension` maps error) | none |
| `_kiro/workflow/invoke` post-claim re-check: "Workflow '<id>' was restarted by another request while this one waited for its ownership claim; it is <status> now."; `_kiro/workflow/retry` running/terminal checks moved after `acquireRunOwnership` | 0.66.34 | always | new error text | STATIC | same generic failure path | none |
| Unchanged: `_kiro/workflow/*` method set (method_literals diff = only `start_ssh_session`), lifecycle event literals, snapshot fields, `recipes_changed`, `load` vs `inspect`, `resume`/`extendRepeat`, auto-wake, watch handlers, `_kiro/session/notify` (0 diff lines each); step-session `consentRound` untouched (the only `nextConsentRound` change is the MCP wrapper refactor) | — | — | — | STATIC | — | — |

**Escape-sequence verdict ([security] 2.31.1 "Workflow recipe text can no longer emit terminal escape sequences in /workflow"):**
- Not KAS code: KAS 2.31.0 == 2.31.1 (`diff -rq` empty), and 0.66.33 -> 0.66.34 adds no sanitiser
  (`\x1B` 43 -> 43, `\x00-\x1f` 3 -> 3, `x00-\x1F` 1 -> 1, no `stripVTControlCharacters`/`strip-ansi`). The fix is TUI-side (tui lane).
- KAS emits recipe text verbatim: recipe schema `of=k.object({name:k.string(),description:k.string().optional(),…})`
  has no character constraint; only `runLabel` is validated (`/^[^\p{Cc}\p{Zl}\p{Zp}"]*$/u`, 1 -> 1, pre-existing).
  Recipe `name`/`description`, step ids, `workflowName`, `failureReason`, `capturedOutput` reach the client raw.
- cyril: `format_recipes` interpolates `recipe.name` / `recipe.description` verbatim (crates/cyril-ui/src/workflow_format.rs:94-98)
  into a system message, but rendering goes through ratatui, which drops every grapheme containing a control char
  (ratatui-core 0.1.0 `buffer/buffer.rs:350` `.filter(|symbol| !symbol.contains(char::is_control))`, `text/span.rs:314` same filter);
  cyril has no direct crossterm `Print` of workflow text. So ESC/CSI/OSC bytes cannot reach the terminal (STATIC).
  Residual: the printable tail of a sequence (`[31m`) shows as literal text, and an embedded newline or markdown in a
  recipe name/description can fake extra rows in the `/workflow list` output; `\p{Cf}` bidi controls (U+202E) are not `Cc` and pass.
  Proposed: NEW P4 issue "single-line-sanitise KAS recipe text in workflow_format (strip Cc/Cf, collapse newlines)".

## Unreleased/dark candidates

| candidate | gate | default | evidence |
|---|---|---|---|
| Cloud-session SSH via SSM: `_kiro/environment/start_ssh_session {sessionId}` -> `{outcome:"ready",ssmSessionId,streamUrl,tokenValue}` / `{outcome:"environmentNotReady",message}` (also fires a throttled 30 s `LoadSessionCommand` resume) / `{outcome:"notFound",message}` | `--remote-sessions-endpoint` or `KIRO_REMOTE_SESSIONS_ENDPOINT` (KiroWebBearerService `StartSshSessionCommand`, 4 -> 5) | off: default source rejects "Starting an SSH session is not available: this agent has no remote sessions endpoint configured, so there is no backend to mint credentials." (`-32000`, `data.errorType:"SshSessionUnavailableError"`) | new.raw:264905-264945 (handler `hVn`), 336472-336553 (`TBt`), 337606 (`sshSession:new TBt({client:r})`), log ids `environment.startSshSession.{ready,notReady,notFound}` |
| KAS native memory now default-disabled | `AB_MEMORY_INTERNAL` (`memory_internal_enabled` default `disabled`) / `AB_MEMORY_EXTERNAL` (`KIRO_FEATURE_MEMORY_EXTERNAL_ENABLED`, false) | `{mode:"disabled",reflection:false}` | see Deltas row; extraction prompt rewritten |
| `coerceStringifiedArgs` MCP option (JSON.parse string args before dispatch) | constructor option, wired `coerceStringifiedArgs:!1` | off | `coerceStringifiedArgs` 0 -> 2 |
| `model_fallback_refusal` / `model_fallback_availability` | AB / `KIRO_FEATURE_MODEL_FALLBACK_{REFUSAL,AVAILABILITY}_ENABLED` | false (unchanged) | 2.30.0 "[V3] notices…" is therefore TUI rendering of the pre-existing kind, possibly backed by an AB rollout (live lane) |

## Leads for the live lane

All legs: fresh temp `HOME`, real `XDG_DATA_HOME`, direct `node acp-server.js` from the carve (trap 7/8), OLD = carve 2.29.1,
NEW = carve 2.31.1, same day. Positive control on every leg: `initialize.clientCapabilities._meta.kiro.largeToolOutputHandler`
(connection-level gate, visible effect) to prove handshake placement (trap 5).

- **L1 KIRO_HOME relocation (zero turns).** Arm A: `KIRO_HOME=<tmp>/kh` (absolute, outside and not an ancestor of HOME);
  `initialize` + `session/new`. Expect NEW: `<tmp>/kh` created, session dir under `<tmp>/kh/sessions/…`, KAS log under
  `<tmp>/kh/logs` with `agent.userConfigDir.resolved {source:"env"}`; OLD: everything under `$HOME/.kiro`. Arm B: `KIRO_HOME=kh`
  (relative) -> NEW logs `agent.userConfigDir.ignored {reason:"relative"}` and uses `$HOME/.kiro`. Arm C: `KIRO_HOME=$HOME` ->
  `reason:"contains-home"`. Seed `<tmp>/kh/hooks/x.json` and `<tmp>/kh/agents/a.json` and confirm NEW lists them (agent in
  `session/new` modes/agents; hook via `_kiro/hooks/list`, which needs v2Hooks enabled) and OLD does not — this is the cyril divergence.
- **L2 `_kiro/environment/start_ssh_session` (zero turns).** NEW: check the method in the initialize response's
  `_meta.kiro.extensionMethods`; call with `{"sessionId":"x"}` -> expect JSON-RPC error `-32000`, `data.errorType:"SshSessionUnavailableError"`;
  call with `{}` -> `-32602` `Invalid params: _kiro/environment/start_ssh_session requires sessionId as a string of 1-128 characters` (`_n` = InvalidParamsError, new.raw:75686). OLD: unknown-method error (record the exact code; earlier audits found KAS avoids `-32601`). Do NOT set
  `KIRO_REMOTE_SESSIONS_ENDPOINT` (would hit a real backend).
- **L3 MCP on-demand (one turn).** Configure one stdio MCP server in `$HOME/.kiro/settings/mcp.json`; send
  `initialize.clientCapabilities._meta.kiro.settings.toolSearch = {"enabled": true}` on both. Expect NEW: no `tool_search`
  tool registered (debug log / `KIRO_DUMP_CONFIGURATION` dump), MCP tools deferred behind `tool_load`; OLD: `tool_search`
  present (toolSearch on) — then the same OLD leg with `toolSearch` absent shows no deferral (`"off"`). One prompt asking to
  use the MCP tool should produce a `tool_load` tool_call on NEW: record its `kind`, `title`, `_meta.kiro` for cyril rendering.
- **L4 memory default (zero turns where possible).** No `settings.memory` in `initialize.clientCapabilities._meta.kiro.settings`.
  Read `memoryConfig` from `KIRO_LOG_LEVEL=debug` intake and from the `session/new` result `_meta` (wire location of
  `memoryConfig` on session/new is UNVERIFIED statically; the verified surface is the workflow state `memoryConfig` field
  returned by `_kiro/workflow/inspect`, which needs a run).
  Expect NEW `{mode:"disabled",reflection:false}` source `legacy`; OLD `{mode:"read_write",reflection:true}`. Control: set
  `initialize.clientCapabilities._meta.kiro.settings.memory={"mode":"read_write","reflection":true}` -> source `explicit` on both.
- **L5 `_kiro/workflow/new` launch defaults (zero turns).** Params `{workflowPath:"/nonexistent/x.workflow.json",
  parentSessionId:<sid>, workspacePaths:[<ws>], modelId: 5}`. Expect NEW error `Invalid workflow shape: …` (validation precedes path
  resolution); OLD path/not-found error (param ignored). Positive control: same call without `modelId` -> path error on both.
- **Passive only:** 15 s timeout on `listAvailableModels`/`getEnterpriseManagement`/`getProfile` — capture if it occurs
  naturally (trap 13 forbids forging).
- **Backend axis:** whether `model_fallback` frames now fire on a refusal (2.30.0 notice line) is a backend/AB question; KAS code
  is unchanged — record any `session_info_update` `kind:"model_fallback"` seen during other legs.

## Changelog reconciliation

KAS identity: 2.29.1 == 2.30.0 (0.66.33) and 2.31.0 == 2.31.1 (0.66.34). Only 2.31.0 lines can be KAS code.

| ver | line | location |
|---|---|---|
| 2.30.0 | [V3] Notices when a model refuses or is unavailable and the turn moves to another model | Not KAS code (KAS unchanged). KAS already emits `model_fallback` (flags default false); rendering = tui lane; possible backend/AB rollout = live lane |
| 2.30.0 | [V3] Agent upgrades refresh permissions after V2 setting changes | not my lane (host/TUI agent-upgrade) |
| 2.30.0 | Kiro CLI may offer to switch you to V3 at chat start | not my lane (TUI/host) |
| 2.30.0 | Screen readers follow streaming replies | not my lane (TUI) |
| 2.30.0 | [V3] Agent upgrades preserve V2 workspace read access and `fs_read` skill trust | not my lane (host/TUI) |
| 2.30.0 | tmux blank rows / launch offset / garbled icons | not my lane (TUI) |
| 2.30.0 | Knowledge search no longer drops a chunk | not KAS (KAS unchanged); host/v2 knowledge |
| 2.31.0 | [V3] `/sandbox` and local `/autonomous` available to all users, off by default | Not gated in KAS (no SANDBOX/AUTONOMOUS flag or env; `"autonomous"` 20 -> 20; "workflows are unavailable in autonomous mode" pre-existing) → availability gate is TUI/host/backend. KAS-side sandbox changes = userConfigDir grant + localhost network rule (Deltas) |
| 2.31.0 | [V3] MCP tools load on demand for every user | KAS: AB_TOOL_LOAD removed, deferral `"mcp-only"` unconditional, `tool_search` deleted (Deltas) |
| 2.31.0 | [V3] `KIRO_HOME` relocates MCP config, permissions, sessions, powers, agent state | KAS: `P8o` userConfigDir (Deltas) |
| 2.31.0 | [V3] `sandbox.json` `enabled:true` applies to headless runs | not in KAS (`sandbox.json` 0 -> 0, `headless` 0 -> 0); host/TUI loads the file and passes `sandboxConfig` |
| 2.31.0 | `--no-interactive` V1 exits non-zero | not my lane (host V1) |
| 2.31.0 | [V3] MCP sign-ins saved under KIRO_HOME; copied once | not in KAS (no copy/migrate in diff; OAuth secrets are host-held via `_kiro/secret/*`) → host/TUI |
| 2.31.0 | [V3] Animated WebP sent to model as PNG | KAS: format choice + ANMF validation (Deltas) |
| 2.31.0 | [V3] Cloud config and Kiro service requests fail after 15 s | KAS: `BG`/`Fat` `throwOnRequestTimeout` for 3 control-plane clients (Deltas); host side may also apply |
| 2.31.0 | [V3] MCP on streamable-HTTP keeps working after server drops session | KAS: session-loss recovery + reset queue (Deltas) |
| 2.31.0 | Relative, root, or home-folder `KIRO_HOME` ignored | KAS: `O9u` reasons `relative`/`root`/`contains-home` (Deltas); host likely mirrors |
| 2.31.0 | [V3] Cloud sessions: code intelligence indicator and `/code` panel reflect cloud workspace | no KAS diff touching codeIntelligence (only dispatch-list neighbours) → TUI |
| 2.31.0 | GitHub/Google sign-ins not signed out on temporary auth error | no KAS auth diff → host |
| 2.31.1 | Mermaid diagrams as terminal art | not my lane (TUI) |
| 2.31.1 | [V3] Non-interactive runs apply saved compaction and `toolSearch` settings | Not KAS code (KAS 2.31.0 == 2.31.1) → host/TUI settings marshal for headless; note KAS 0.66.34 ignores `toolSearch.enabled` anyway |
| 2.31.1 | `chat.disableWrap` copy spacing | not my lane (TUI) |
| 2.31.1 | [V3] Herdr shows session blocked while a prompt waits | Not KAS code → TUI/host |
| 2.31.1 | Model fallback handles refusals with no response text | Not KAS code → host V2 engine or TUI |
| 2.31.1 | Compaction no longer saves the model's refusal as summary | Not KAS code → host V2 engine |
| 2.31.1 | [V3] `/sandbox` setup on Windows temp-folder policy | Not KAS code → host/TUI |
| 2.31.1 | `/changelog` one entry per line | not my lane (TUI) |
| 2.31.1 | IAM Identity Center in isolated partitions | Not KAS code → host auth |
| 2.31.1 | [security][V3] Workflow recipe text can no longer emit escape sequences in `/workflow` | Not KAS code; verdict above |
| 2.29.1 (in feeds) | MCP OAuth sign-in with issuer (classic/V2) | out of range / not KAS |

## Method notes

- `_kiro/environment/start_ssh_session` — params `{sessionId: string, 1..128 chars}` (else `-32602` InvalidParamsError, class `_n` at new.raw:75686:
  "`_kiro/environment/start_ssh_session requires sessionId as a string of 1-128 characters`"). Result discriminated by
  `outcome`: `ready {ssmSessionId, streamUrl, tokenValue}` | `environmentNotReady {message}` | `notFound {message}`.
  `environmentNotReady` triggers at most one `LoadSessionCommand` resume per session per 30 s. Without a remote-sessions
  endpoint the source is `xct`, which rejects with `-32000` `SshSessionUnavailableError` (`data.errorType`). The covenant doc
  (docs/kiro-kas-acp-covenant.md) has no `_kiro/environment/*` entry — sync it when the covenant package next updates.
- `_kiro/workflow/new` — new optional `modelId` (string) and `effortLevel` (string; `""` = absent), validated against the recipe
  schema pick before the recipe is resolved, merged over the recipe's top-level defaults; `runLabel` unchanged (string, trimmed,
  1..100, no Cc/Zl/Zp/`"`). Mode guard `autonomous` and folder guard unchanged.
- `_kiro/workflow/load` — concurrent loads of the same id coalesce; a load that waited for the ownership claim re-reads disk and
  refuses if the run went terminal meanwhile.
- `_kiro/workflow/invoke` / `retry` — status checks now run after the ownership claim (race-safe); new invoke error text.
- `_kiro/mcp/resetServer` — now `resetServer(name,{startOAuth,coalesce:false,trigger:"user-retry"})` and the response waits at
  most a bounded ceiling (`mcp.resetServer.responseCeilingExpired` warn) instead of the full reset.
- `send_message` (step tool) — completion-signal persistence failures no longer silently drop the signal; the step pauses
  awaiting the user with an explicit reason; result `warning` texts reworded ("Message delivered, but …").

## Proposed issues / notes (search of .rivets/issues.jsonl done)

- NEW P3: KIRO_HOME parity with KAS 0.66.34 `userConfigDir` — hooks loader (protocol/domain_mediator/mod.rs:262-266) and global
  agent config (kiro_agent_config.rs:102-104) hard-code `~/.kiro`; `kiro_home_dir` (kiro_agent_config.rs:93-98,
  protocol/kas/settings.rs:50-59) must ignore relative / root / home-or-ancestor values like KAS `O9u`. (No existing issue;
  cyril-nhzw closed, cyril-ss5i unrelated.)
- NEW P4: single-line-sanitise KAS recipe text in crates/cyril-ui/src/workflow_format.rs:94-98.
- NOTE cyril-4u4a: `_kiro/workflow/new` `modelId`/`effortLevel` launch defaults (+ `runLabel`) NOT-SENT at commands/kas.rs:349-356.
- NOTE cyril-q8gq / cyril-ea67: `toolSearch.enabled` dead in 0.66.34; only `neverDefer` consumed; `minPct`/`minTokens` never consumed.
- NOTE cyril-0na7 / cyril-s21o / cyril-l4e7: memory default flipped to disabled.
- NOTE cyril-58uv / cyril-aoby: kind set unchanged (25), cyril handles 8, 17 dropped unlogged at convert/kas.rs:525.
- NOTE cyril-tikf: dark `_kiro/environment/start_ssh_session` joins the remote-sessions family.
- NOTE cyril-be7e / cyril-6vo6: sandbox write grant follows `userConfigDir`; seatbelt localhost rule fixed; GA gate not in KAS.
- NOTE cyril-rtrh: `session/load` abandoned-prompt replay suppressed when messages arrive during load.

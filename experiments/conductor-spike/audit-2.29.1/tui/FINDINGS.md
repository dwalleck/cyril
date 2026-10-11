# TUI-bundle static lane — kiro-cli 2.28.0 → 2.29.1 (folds 2.28.1 feed-only + 2.29.0)

Inputs (sha256 verified this run):
`/home/dwalleck/.local/share/kiro-research/tui-bundles/kiro-tui-2.28.0.js` = `6733ecdbd8a72ecd1d6c7a2bbfc763b8b81a00e8cd097febcc01dc2152fede4f`
`…/kiro-tui-2.29.1.js` = `256b00c17fdd301d9aed3a2b7885222d914a5499fd994dd00de58297afdcd7d6`
(identical hash to `kiro-tui-2.29.0.js` → **TUI 2.29.0 == 2.29.1**, as established; this lane therefore compares 2.28.0 → 2.29.1 only and its rows cover both.)
Cross-references (read-only, cross-check only — the KAS bundle is kas-core's lane, the binaries are host's): KAS carves
`/home/dwalleck/.local/share/kiro-research/kas-carves/{2.28.0,2.29.1}/node_modules/@kiro/agent/dist/server/acp-server.js` (12,295,918 / 12,669,464 B).

---

## Summary

1. **Method/vocabulary surface is frozen.** Trap-3 set diff: the method-literal family `"_kiro/[A-Za-z0-9_./-]*"` ∪ `"(?:_kiro\.dev|kiro\.dev)/…"` is **92 == 92, added ∅ removed ∅**; the gates extractor's method list **113 == 113**; slash-command name literals **74 == 74, symdiff empty**; `/memories` (`count 10 == 10`) already existed in 2.28.0. **No new `_kiro/*` method, no new slash command, no removed method.**
2. **The whole TUI delta is 44 added / 3 removed string literals** (vocab differ) and the deltas that matter are concentrated in four places: the KAS `settings` builder (memory), a new `session/new` meta key, a new replay-aware permission-request cancel, and the MCP-OAuth URL notice.
3. **initialize handshake `clientMeta` is byte-identical** (modulo renames): `{telemetryEnabled, telemetry?, knowledge:!0, hooks:{enabled:!0,v2:!0}, requirementsAnalysis:!0, specPhaseCheckpoints:!0, streamingShellContent:!0, infrastructureSafety?, settings?}` — same in 2.28.0 and 2.29.1.
4. **`settings` builder memory change (STATIC, D1–D5):** `memory` is now sent **always** (2.28.0: only when rollout `memory_controls`); `memoryEnable` narrowed to `Fn("memory") && KIRO_INTERNAL && mode!=="disabled"`; `userMemoryOptIn` is now computed and also sent when `mode==="disabled"`; the flat row `["memory.enabled","userMemoryOptIn"]` is **gone**; the default memory mode became `KIRO_INTERNAL ? "read_write" : "disabled"` (was the constant `{mode:"read_write",reflection:true}`). cyril sends none of it → NOT-SENT.
5. **New wire send (STATIC, D6):** the TUI's ACP client can attach `_meta.kiro.overrideSessionId` = `sess_<uuid>` to `session/new` (0 hits in 2.28.0, `1` in 2.29.1). KAS 0.66.26 **and** 0.66.33 both already read it (`"overrideSessionId"`, 3 hits each carve) — engine-side support predates the TUI's use. cyril sends no `_meta.kiro` on `session/new` at all → NOT-SENT.
6. **New consumption (STATIC, D7–D8):** on every `sessionUpdate`, when `agentCapabilities._meta.kiro.replayMarking === true`, the TUI cancels a still-open `session/request_permission` whose `toolCallId` matches a `session_info_update{_meta.kiro.kind:"interaction_resolved"}` (skipping `_meta.kiro.replay === true` frames). cyril has no arm for `interaction_resolved` → DROPPED.
7. **Spawn: only one env var added** — `KIRO_CLI_IS_CHILD_PROCESS="1"` (0 → 1). KAS argv is unchanged (`--transport=stdio --auth=acp-callback …`), `NODE_USE_SYSTEM_CA`/`KIRO_CUSTOM_USER_AGENT` unchanged.
8. **`--auth-method=client` has 0 hits in the bundle**; `tool_tags` has **0 hits in both** — both changelog lines are host-side (not my lane).
9. **No client-side `_meta.kiro` change** in `session/prompt` (`displayText`/`outputStyle`), permission responses (`provenance:"human"` v2-only, `consentContext` 28/28) or `session/load` (`requireExisting`) — counts equal in both, code byte-equivalent modulo renames.
10. **§"Explicit answer" separates three things the earlier TUI audits blurred:** (A) the *version* delta (no new `_kiro/*` method — 92 == 92, outbound 23 == 23); (B) TUI→cyril *outbound* parity (19 of the TUI's 23 calls have no cyril sender, all pre-dating 2.29.x); (C) cyril→KAS *inbound* parity (cyril answers 19; the TUI's capability table answers more, e.g. `_kiro/secret/{get,store,delete}`, `_kiro/userInput`, `_kiro/openExternalUrl`). Each is listed with the literal quoted from the bundle or the `file:line` in cyril.

Legend: **STATIC** = read off the bundle (a lead); **LIVE** = needs a probe (a verdict). No LIVE row is claimed here.

---

## Deltas

`gate/default` is the TUI-side gate read off the bundle; **the rollout *registry values* are the host lane's** (`memory_controls`, `memory`, `local_sandbox`, `session_dashboard`, `remote_sandbox`, `c2s`, `cloud_config`, `workflows`, `background_execution` — the nine names this TUI checks).

| # | item | version | gate/default | direction | S/L | cyril verdict + `file:line` | proposed issue |
|---|---|---|---|---|---|---|---|
| D1 | `_meta.kiro.settings.memory` = `{mode,reflection}` sent **always** | 2.29.0 | none (2.28.0 required rollout `memory_controls`) | outbound settings | STATIC | **NOT-SENT** — `crates/cyril-core/src/protocol/kas/settings.rs:109` `marshal_agent_settings` has no `memory` key | cyril-0na7 |
| D2 | `settings.memoryEnable={enabled:true}` narrowed | 2.29.0 | `Fn("memory") && process.env.KIRO_INTERNAL==="1" && mode!=="disabled"` (2.28.0: `Fn("memory")` only) | outbound settings | STATIC | **NOT-SENT** — `settings.rs:78-98` (BOOL_MAP/DEFAULTS_ON, no `memoryEnable`) | cyril-0na7 |
| D3 | `settings.userMemoryOptIn={enabled: mode==="read_write"&&reflection}` computed; also sent when `mode==="disabled"` | 2.29.0 | `!Fn("memory_controls") \|\| mode==="disabled"` (2.28.0: flat `cli.json memory.enabled` row, only when `!memory_controls`) | outbound settings | STATIC | **NOT-SENT** — `settings.rs:78-98` (no `userMemoryOptIn`); the 2.28.0 row is what cyril-q8gq asks for and it no longer exists | amendment to cyril-q8gq (see §Changelog) |
| D4 | default memory mode/reflection source | 2.29.0 | `KIRO_INTERNAL ? "read_write" : "disabled"` (2.28.0: `Object.freeze({mode:"read_write",reflection:!0})`) | outbound settings | STATIC | **N/A** (cyril derives no memory default) — `settings.rs:109` | cyril-0na7 |
| D5 | flat row `["memory.enabled","userMemoryOptIn"]` removed from the bool table | 2.29.0 | 2.28.0-only (`Pn("memory_controls")` guard gone entirely) | outbound settings table | STATIC | **cyril still maps it** — `settings.rs:78-88` (`BOOL_MAP` has no `memory.enabled`; cyril-q8gq is the *opposite* direction and is now stale) | cyril-q8gq (re-scope) |
| D6 | `session/new` request `_meta.kiro.overrideSessionId` = `sess_<uuid>` | 2.29.0 | only when the wrapper is called with `{onSessionUpdate}` (no in-bundle caller found) | outbound request meta | STATIC | **NOT-SENT** — `crates/cyril-core/src/protocol/domain_mediator/commands/session.rs:50-52` (`NewSessionRequest::new`, no `_meta`) | NEW |
| D7 | new consumption of `session_info_update._meta.kiro.kind:"interaction_resolved"` → abort the matching pending `session/request_permission` | 2.29.0 | `agentCapabilities._meta.kiro.replayMarking===!0` | inbound consumption (permission lifecycle) | STATIC | **DROPPED** — `crates/cyril-core/src/protocol/convert/kas.rs:352` (no arm; falls through to `None`) via `crates/cyril-core/src/protocol/engine.rs:329-330` | cyril-99ds (extend) / NEW |
| D8 | `sessionUpdate()` settles open permission requests (replay-safe: `_meta.kiro.replay===!0` frames ignored) | 2.29.0 | `replayMarking` | inbound plumbing | STATIC | **DROPPED** (same arm as D7) | cyril-99ds (extend) |
| D9 | KAS-spawn env `KIRO_CLI_IS_CHILD_PROCESS="1"` | 2.29.0 | unconditional in the engine-launch env map | spawn env | STATIC | **NOT-SENT** — cyril has no such var; spawn env is assembled in `crates/cyril-core/src/protocol/kas/version.rs` / engine launch (see `be7e`) | cyril-be7e |
| D10 | `/upgrade-agent` AIM-managed report: `reasonCode:"cli_only_agent" && externallyManaged` → "managed by AIM…" | 2.28.1/2.29.0 | `externallyManaged` derived **locally** from the agent config file containing `description … includes("managed by AIM")` | TUI-local rendering | STATIC | **N/A** (not ACP; no `_kiro/*`, no settings key). KAS has 0 hits for `externallyManaged`/`skippedManaged` in both carves | — |
| D11 | MCP-OAuth/copy-url notice payload: `{message, success:!0, persistent:!0, linkUrl}` (2.28.0: `{message, success:!1, persistent:!0}`) and `case"system_notice"` renders `linkUrl` | 2.29.0 | TUI-local (`linkUrl` count 0 → 11; **KAS 0.66.26/0.66.33 have 0 `linkUrl`**) | TUI-local event | STATIC | **HANDLED at the ACP input** — cyril parses the agent's `mcp/oauth_request.oauthUrl`: `crates/cyril-core/src/protocol/convert/kiro.rs:1134`. `linkUrl` itself never crosses the wire | — |
| D12 | rollout gate names the TUI checks | 2.29.0 | **+`memory`** (8 → 9: `local_sandbox, background_execution, c2s, cloud_config, memory_controls, remote_sandbox, session_dashboard, workflows` + `memory`) | client rollout gate | STATIC | **N/A** (client-side gate; registry rows are host lane) | — |
| D13 | `/memories` panel unchanged in shape: `tM(e,t)= e==="kas"&&!t&&Fn("memory_controls")`, memory default now `m9(cli.json settings, Js())` where `Js(){return process.env.KIRO_INTERNAL==="1"}` | 2.29.0 | `memory_controls` | client UI | STATIC | **NOT-SENT** (panel-only; the wire part is D1–D4) | cyril-0na7 |
| D14 | MCP-sandboxing UI (`mcpSandboxing`, "…policy keeps MCP sandboxing on.", "Also isolate stdio MCP server processes") + `kiro_cli_sandbox_session_outcome_total` telemetry | 2.29.0 | TUI-local | client UI/telemetry | STATIC | **N/A** — KAS owns sandbox/MCP-env behaviour (kas-core) | — |
| D15 | `--auth-method=client` (2.29.0 changelog) | 2.29.0 | — | host/launch flag | STATIC | **not in my lane** — `--auth-method` = **0 hits in both**; the only `auth-method` substring is **1 == 1**, an ispell word-list entry (`mtpmail-try-auth-methods`); `authMethod` 4 == 4 is the ACP `GetAccessTokenResponse` field | — |
| D16 | V2 hook-matcher widening + `tool_tags` on V2 tool hooks | 2.29.0 | — | host/V2 engine | STATIC | **not in my lane** — `tool_tags`/`toolTags`/`toolTag` = **0 hits in both bundles**; the only TUI trace is `/upgrade-agent` report *text* ("Legacy tool name rewritten to its tag group…") | cyril-5nep (kas-side), host lane |
| D17 | effort selector at session start (2.29.0 fixed line) | 2.29.0 | — | engine advert | STATIC | **no TUI delta found**: every effort token count equal (226/59/8/4/35 == same), the `wA`/`GDa` selector plumbing and `launchEffort` sites are byte-equivalent modulo renames; KAS `effortLevels` 29 → **33** | live lead L5; kas-core |
| D18 | duplicate `127.0.0.1` loopback helper (`Ptt`/`URn` == `J5n`/`pwt`) | 2.29.0 | — | client misc | STATIC | **N/A** cosmetic duplication (2 → 3 occurrences) | — |
| D19 | `replayMarking` was **already** parsed+used in 2.28.0 (workflow child-session replay buffering, 14 hits) | 2.22–2.28 | — | client capability | STATIC | cyril does not parse it either — grep for `replayMarking` in `crates/**/*.rs` = **0** | cyril-99ds |

Everything else in the bundle's 44-literal delta is TUI chrome: `/upgrade-agent` report strings, `components.contextBreakdown.memory` (i18n), AIM/`skippedManaged`, `clearTranscriptForShellClear`, `agentMarksReplay`, `sandbox_backend`/`sandbox_fallback_reason`/`sandbox_session_outcome`, `not_applied`/`not_applicable`/`user_bypass`/`fallback_unsandboxed`/`backend_unavailable`.

### Negative results (equally load-bearing)

| check | 2.28.0 | 2.29.1 | evidence |
|---|---|---|---|
| `_kiro/*` + `_kiro.dev/*` + `kiro.dev/*` method literals | 92 | 92 | added ∅ removed ∅ (`sets-*.txt`) |
| method list (gates script `methodKeys`, a wider table incl. core ACP names) | 113 | 113 | `gates-2.28.0-2.29.1.txt` |
| slash-command name literals | 74 | 74 | symdiff ∅ |
| `initialize.clientCapabilities._meta.kiro` clientMeta object | — | — | byte-identical modulo renames |
| KAS spawn argv | `[prefix,"--experimental-wasm-modules",server,"--transport=stdio","--auth=acp-callback",…krs/cps/region]` | same | quote below |
| KAS spawn env set (gates) | `KIRO_CUSTOM_USER_AGENT, NODE_CHANNEL_FD, NODE_CHANNEL_SERIALIZATION_MODE, NODE_USE_SYSTEM_CA` | same | `gates-*.txt` |
| capability keys advertised (`getAccessToken,userInput,copyUrlToClipboard,secretStorage`) | 4 | 4 | `XX`/`qX` bodies identical |
| `session/prompt` `_meta.kiro` keys (`displayText`,`outputStyle`) | 2 | 2 | `persistDisplayText` sites identical |
| permission `_meta` (`provenance` 1, `consentContext` 28, `outcome:"selected"` 6) | equal | equal | `I_`→`D_` rename only |
| `subagentOrchestration` in the TUI settings | **0** | **0** | cyril still sends it (`settings.rs:93-98`) — divergence stands, `53qx` says keep |
| session_info_update main mapper kinds + silent set `{background_processes, background_process_event}` | equal | equal | `BX`/`OX` identical modulo renames |
| `memory_config_update` observer, `memoryReflection` config option, `mode` config option | present | present | identical modulo renames |

---

## Unreleased / dark candidates

| candidate | gate | default | evidence (bundle literal or count) |
|---|---|---|---|
| **`session/new._meta.kiro.overrideSessionId`** — client-preallocated `sess_<uuid>` (KAS uses it verbatim: `d=o?.overrideSessionId??wLt()`) | none in the send path (`CTn` generates it unconditionally whenever the 2-arg wrapper is used) | no in-bundle caller passes `{onSessionUpdate}` → **unreachable from the shipped TUI flow** | `function CTn(e){…let r=n[UJe];if(typeof r==="string")return{request:e,sessionId:r};let a=`${ETn}${crypto.randomUUID()}`…} … ETn="sess_",UJe="overrideSessionId"` (0→1 hit) |
| **Replay-aware permission cancel** (`interaction_resolved`) | `agentCapabilities._meta.kiro.replayMarking===!0` (KAS marks session/load frames `_meta.kiro.replay=!0` (stamper `sxt` 0.66.26 / `YAt` 0.66.33; `replay:!0` = 1 in each carve)) | off for engines that do not advertise `replayMarking` | `async sessionUpdate(e){if(this.agentMarksReplay())this.openPermissionRequests.settle(e);…}`; `wTn`: `if(t?.kind!=="interaction_resolved")return;if("replay"in t&&t.replay===!0)return;return t.toolCallId` |
| **`settings.memoryEnable`** — the "always-on native memory" switch | `Fn("memory")` **and** `process.env.KIRO_INTERNAL==="1"` | on only for internal builds, and only when `memory.mode!=="disabled"` | `if(Fn("memory")&&i&&!c)t.memoryEnable={enabled:!0}` |
| **`settings.memory` default = disabled outside internal builds** | `KIRO_INTERNAL` env | `{mode:"disabled",reflection:false}` for a normal user with no memory config | `function LTn(e){return e?"read_write":"disabled"}` + `Js(){return process.env.KIRO_INTERNAL==="1"}`; `let i=Js(),s=m9(e,i)` (e = parsed cli.json) |
| **`_kiro/openExternalUrl` link-notice UX** (URL + clipboard + dedup) | TUI-local | active | `[copy-url] Copied URL to clipboard and showed it`; `addLinkNotice` 0→7 |
| **MCP sandboxing UI** (`mcpSandboxing`, "Global/Project policy keeps MCP sandboxing on.", "Also isolate stdio MCP server processes") | TUI-local sandbox config | displayed from policy | strings in the 44-literal delta; `kiro_cli_sandbox_session_outcome_total` 0→1 |
| **`local_sandbox` rollout** still env-forced (`KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`) | env | off | `function Fn(e){if(e==="local_sandbox")return process.env.KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1";…}` (identical in both) |

---

## Leads for the live lane

Rules applied: full `initialize.clientCapabilities._meta.kiro` path (trap 5), positive control for every handshake-field probe, `HOME=<tmp>` with the real `XDG_DATA_HOME` (trap 7), KAS spawned as `node acp-server.js` from the carve (trap 8).

**L1 — does KAS honour `settings.memory` / `memoryEnable`, and does it echo `_meta.memoryConfig`?** (proves D1–D4)
* Handshake: `initialize.clientCapabilities._meta.kiro.settings` = `{"memory":{"mode":"read_only","reflection":false},"memoryEnable":{"enabled":true},"largeToolOutputHandler":{"enabled":true},"codeIntelligence":{"enabled":true}}` (`memoryEnable` only meaningful under `KIRO_INTERNAL=1`).
* Positive control (trap 5): `largeToolOutputHandler:{enabled:true}` is the documented connection-level gate — the run MUST show its effect (the tool-output truncation/`[kas-settings]`-style acknowledgement path) or the handshake placement is wrong again and every conclusion below is void.
* Expected signal: `session/new` result `_meta.memoryConfig` (top-level **or** `_meta.kiro.memoryConfig`; the TUI reads `t?.memoryConfig??n?.memoryConfig`) ≠ null and echoing the requested `{mode,reflection}`; `configOptions` carries a `memoryReflection` option; then `session/set_config_option {sessionId, configId:"memoryReflection", value:"on"}` → a `memory_config_update` push (the TUI's `pee` observer keys on `e.type==="memory_config_update"` carrying `memoryConfig`).
* Legs: (a) `KIRO_INTERNAL` unset + no `memory` key (2.28.0 TUI equivalent) — expect today's behaviour; (b) `memory:{mode:"read_only"}`; (c) `memory:{mode:"disabled"}` + `userMemoryOptIn:{enabled:false}`; (d) `memoryEnable:{enabled:true}` with/without `KIRO_INTERNAL=1` — the TUI only ever pairs it with `KIRO_INTERNAL`.
* Cost: free (no prompt needed); leave `_kiro/memory/list` untouched (it is a paid/local read of the record store).
* KAS-side cross-ref (kas-core lane, count-identical in both carves): `resolveMemoryEnabled` = 4 hits each, `memoryConfig` = 101 each, `AB_MEMORY_{INTERNAL,EXTERNAL}` present in both — the **engine** memory surface did not change 0.66.26 → 0.66.33, so the `/memories` release line is the client handshake D1–D4 plus panel defaults, not a KAS behaviour change. That is why a client-only probe (L1 legs a–d) can settle it.

**L2 — `session/new._meta.kiro.overrideSessionId`** (proves D6; KAS-side truth for kas-core)
* Request: `session/new {cwd, mcpServers:[], _meta:{kiro:{overrideSessionId:"sess_<uuid4>"}}}`.
* Positive control: a second `session/new` **without** the field must return a KAS-minted `sess_…` id, and `crates/cyril-core/src/protocol/fingerprint.rs:71-83` already asserts that shape.
* Expected signal: the response `sessionId` **equals** the supplied `sess_<uuid4>` (KAS: `let d=o?.overrideSessionId??wLt()` — used verbatim; `idempotent` in the carve is only MCP-tool/AWS-Smithy metadata, **no** dedupe of repeat `session/new`). Control: a second identical `session/new` with the same `sess_` id → expect a fresh/admitted-but-colliding session, i.e. the preallocation is a *client* identity, not a server idempotency key.
* Static cross-check already done: KAS 0.66.26 and 0.66.33 each contain **3** hits for `overrideSessionId` (`BRt="overrideSessionId"` / `pLt="overrideSessionId"`), read as `d=i?.overrideSessionId??VRt()` / `d=o?.overrideSessionId??wLt()` inside `newSessionAdmitted` — the field is not new engine-side; what is new is the TUI sending it.

**L3 — `interaction_resolved` cancels a pending permission request** (proves D7–D8)
* Setup: `_meta.kiro.hooks={enabled:true}` not required; drive one shell tool call that triggers `session/request_permission` and **do not answer it**.
* Request: nothing — this is inbound. Watch for `session/update {sessionUpdate:"session_info_update", _meta:{kiro:{kind:"interaction_resolved", toolCallId:"<the pending one>", outcome:"…"}}}` (KAS `ZYs` schema: `{type:"interaction_resolved", toolCallId, outcome, selectedOption?, executionId, consentRound?}`).
* Positive control: the same session with the request **answered** normally must show the ordinary `selectedPermissionOutcome` path — otherwise the leg proves nothing about the cancel branch.
* Expected signal for cyril: today the frame is converted to nothing (`kas.rs:352` fall-through) so cyril's UI stays in the approval prompt forever; the fix is an arm that cancels the matching pending permission. Also record whether the frame carries `_meta.kiro.replay:true` (stamped by `session/load` only: `sxt` 0.66.26 / `YAt` 0.66.33) — that is the flag the TUI skips.

**L4 — spawn-env parity surface** (D9)
* Compare `env` of the TUI-spawned KAS (`KIRO_CLI_IS_CHILD_PROCESS="1"`, plus the pre-existing `NODE_USE_SYSTEM_CA`, `KIRO_CUSTOM_USER_AGENT`, `NODE_CHANNEL_FD`, `NODE_CHANNEL_SERIALIZATION_MODE`) with a cyril-spawned KAS. No new flag: the deliverable is the delta list for cyril-be7e.

**L5 — effort selector at session start** (D17)
* Configure a default model whose `configOptions` model entry carries `defaultEffortLevel` + `thinkingToggleable`, start a session, and observe which picker claims focus and what `set_config_option {configId:"effortLevel"}` (if any) the TUI sends at start.
* Positive control: a model without `effortLevels` must NOT offer the picker.
* Static prior: TUI code byte-equivalent; KAS 0.66.26 → 0.66.33 `effortLevels` **29 → 33** — expect the fix to be an advertisement change (kas-core).

**L6 — MCP OAuth URL reachability from a non-TTY** (D11)
* With an OAuth-requiring MCP server, confirm the `_kiro/openExternalUrl` request still arrives and that `mcp/oauth_request.oauthUrl` reaches the client; the TUI's change is purely how it *prints* the URL (`linkUrl` never appears in KAS). Positive control: the request must carry a non-empty `oauthUrl` (cyril drops the frame otherwise, `kiro.rs:1140`).

---

## Changelog reconciliation (feed-2.29.1.json + feed-2.28.1.json, 24 lines)

`L` = located in the TUI bundle this lane owns.

| # | release | line | location |
|---|---|---|---|
| 1 | 2.29.1 | MCP OAuth sign-in … works with servers that require an issuer | **not in my lane** (classic/Rust V2 engine); TUI-visible part is #8 |
| 2 | 2.29.0 | [V3] `/memories` configures memory access and automatic updates for local sessions | **L** — the entry `{name:"/memories",description:"Manage repo-scoped memories from previous sessions",feature:"memory",meta:{inputType:"panel"}}` is **unchanged** (×10 == ×10); the release substance is (i) the handshake D1–D4, (ii) the `m9(<cli.json>, Js())` memory default (`Js()` = `KIRO_INTERNAL`), (iii) **new panel copy** (3 of the 44 added strings): `"…it is shown above"`, `"Automatic memory updates are off for this session. Turn them on in config."`, `` `Memory access is ${mode==="disabled"?"off":"read only"} for this session. Changes in config apply to new sessions.` `` (renderer `yVn(e)`, first branch unchanged: `"Kiro creates them automatically while you work…"`). Panel gate `tM(e,t)= e==="kas"&&!t&&Fn("memory_controls")` is unchanged from 2.28.0 |
| 3 | 2.29.0 | V2 TUI hook matchers such as `read` cover more tools, widening any matching blocking hooks | **not in my lane** (V2/Rust hooks). TUI trace = report text `"Legacy tool name rewritten to its tag group, which covers more tools on V2"` (`widened-hook-matcher`, `hooks[].matcher`); `fs_read` 13 == 13 (telemetry canonicaliser, unchanged) |
| 4 | 2.29.0 | V2 TUI tool hooks receive `tool_tags` … | **not in my lane** — `tool_tags`/`toolTags`/`toolTag` **0 hits in both bundles** |
| 5 | 2.29.0 | [V3] `kiro-cli acp` uses your CLI login unless you pass `--auth-method=client` | **not in my lane** (host/launch). `--auth-method` = 0 hits in both bundles (`auth-method` = 1, an ispell entry); the KAS spawn keeps `--auth=acp-callback` unchanged |
| 6 | 2.29.0 | [V3] Custom agents use their tool settings to decide which `mcp.json` servers to start | **not in my lane** (KAS; kas-core) |
| 7 | 2.29.0 | [V3] Stdio MCP servers now inherit the full agent environment | **not in my lane** (KAS). TUI neighbour = MCP-sandboxing UI (D14) |
| 8 | 2.29.0 | Classic `fs_write` no longer auto-approves symlink writes outside `allowedPaths` | **not in my lane** (classic engine) |
| 9 | 2.29.0 | Model fallback changes take effect in the current session without restarting | **not in my lane** (v2 Rust); TUI `model_fallback` count 6 == 6 (2.27.1 parser, unchanged) |
| 10 | 2.29.0 | MCP OAuth now shows the authorization URL so you can open it from SSH or tmux sessions | **L** — `Open this URL to … (also copied to your clipboard)`, `Couldn't copy to the clipboard. Open this URL to …`, `[copy-url] Copied URL to clipboard and showed it`, `[copy-url] No clipboard tool found; showed the URL only`; 3 literals removed (`Clipboard copy failed. Open this session-specific OAuth URL manually; do not share it:` …) |
| 11 | 2.29.0 | [V3] Non-interactive runs respect `telemetry.enabled` and `KIRO_DISABLE_TELEMETRY` | **not in my lane** (KAS); TUI telemetry switch `KIRO_DISABLE_TELEMETRY` 1 == 1, unchanged |
| 12 | 2.29.0 | [V3] `/upgrade-agent` rewrites legacy hook matchers such as `fs_read` so they fire on V3 | **L (report side only)** — `Hook matcher not portable` / `Hook matcher widened` / `unportable-hook-matcher` / `widened-hook-matcher` / `hooks[].matcher` / `Use read, write, shell, web, @builtin, @mcp, @server, @server/tool, or *`; the rewrite itself is host/V2 |
| 13 | 2.29.0 | Knowledge search limits apply across all bases, with combined pagination and file-type filtering | **not in my lane** (KAS); `_kiro/knowledge` literal present in both |
| 14 | 2.29.0 | [V3] Sessions no longer unexpectedly close under heavy load | **not in my lane** (KAS) |
| 15 | 2.29.0 | [V3] Custom agents dispatched as subagents now load their own `mcpServers` | **not in my lane** (KAS) |
| 16 | 2.29.0 | [V3] Effort selector is offered at session start for an effort-capable default model | **no TUI delta found** (D17): effort token counts identical, selector plumbing byte-equivalent modulo renames; KAS `effortLevels` 29→33 ⇒ engine-side → kas-core + live L5 |
| 17 | 2.29.0 | [V3] Images are resized to fit the selected model instead of failing the request | **not in my lane** (KAS): new dep `@silvia-odwyer/photon` (WASM image processing), `resize` 40→48, `photon` 0→12 in `acp-server.js` |
| 18 | 2.29.0 | [V3] `KIRO_API_KEY` keys minted outside `us-east-1` now authenticate in their home region | **not in my lane** (KAS: new `api-key-region-probe.d.ts`; host) |
| 19 | 2.29.0 | [V3] Readable retry message when the backend returns a non-JSON error page | **not in my lane** (KAS) — no new literal in the TUI bundle |
| 20 | 2.29.0 | [V3] Resumed sessions keep `UserPromptSubmit` hook output in the conversation | **not in my lane** (KAS hooks) |
| 21 | 2.29.0 | [V3] Stop now cancels a prompt sent while MCP servers are still starting | **not in my lane** (KAS) |
| 22 | 2.29.0 | [V3] A prompt sent just as a turn ends now includes that turn in the model's history | **not in my lane** (KAS); TUI `turn_end` handling unchanged |
| 23 | 2.29.0 | [V3] Manual steering files now load even when their slash command appears mid-prompt | **not in my lane** (KAS steering) |
| 24 | 2.28.1 | `kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3 | **not in my lane** as a CLI subcommand (host); **L** for its result rendering — the `/upgrade-agent` report (`No agents to upgrade. …`, `No agents to upgrade — every agent is already in sync or V3-only`, `skippedManaged`, `externallyManaged` (D10), `bNn` classifier) — and `upgrade-agent` remains one of the 11 hidden `chat _` subcommands (unchanged set) |

Line-level note for #2: the TUI's own `description` string is unchanged (`"Manage repo-scoped memories from previous sessions"`), so the feature line's substance is in the handshake (D1–D4), not the command.

---

## Explicit answer — `_meta.kiro.settings` keys and `_kiro/*` methods the 2.29.x TUI sends that cyril does not

**A. Version delta (what this audit adds): NO new method, in either direction.** The method-literal family is 92 == 92 (added ∅ removed ∅), and the *outbound call list* — the only place a TUI-sent method is unambiguous — is identical: `sendExtMethod(...)` yields 12 literals and `sendExtNotification(...)` 2, byte-for-byte the same set in 2.28.0 and 2.29.1 (`outbound-methods.txt`). The gates script's wider 113-entry table is also identical. The only new connection-scoped literal anywhere is `replayMarking`, which the TUI *parses*, not calls.

**B. TUI → cyril OUTBOUND parity (the check the version diff cannot make): the TUI sends 19 methods cyril never sends, and every one of them predates 2.29.x.** The TUI reaches `_kiro/*`/`kiro.dev/*` through ext-request helpers; extracting every literal passed to `sendExtMethod`/`sendExtNotification`/`callExtMethod`/`extMethod(...)` (plus `this.ext("…")`) yields **23 literals, identical in 2.28.0 and 2.29.1** (`outbound-methods.txt`). cyril's production senders (`crates/`, tests excluded) are: `_kiro/account/getUsage` (`protocol/domain_mediator/commands/extensions.rs:262`), `kiro.dev/settings/list` (`…/extensions.rs:216`), `kiro.dev/session/terminate` (`…/commands/subagents.rs:60`), `kiro.dev/commands/{options,execute}` and the workflow family (`kiro/workflow/{invoke,cancel,list,inspect,load,new,resume,retry}`). `_kiro/knowledge` is advertised but never called (`protocol/fingerprint.rs:138`). So of the 23:

| TUI outbound literal | cyril sender | verdict |
|---|---|---|
| `_kiro/account/getUsage` | `domain_mediator/commands/extensions.rs:262` | HANDLED |
| `kiro.dev/settings/list` | `domain_mediator/commands/extensions.rs:216` | HANDLED |
| `kiro.dev/session/terminate` | `domain_mediator/commands/subagents.rs:60` | HANDLED |
| `_kiro/hooks/list` | answered, not called — `protocol/kas/hooks.rs` (responder) | reversed direction |
| `_kiro/session/context` | neither sent nor handled — the only hit is a *near-miss negative case* in `protocol/engine.rs:777` (inside `#[cfg(test)] mod tests`) | NOT-SENT |
| `_iro/session/compact` | no sender (it is a *slash command* name in `commands/mod.rs:1153`, not a wire call) | NOT-SENT |
| `_kiro/session/history` | no sender | NOT-SENT (cyril-a3th) |
| `_kiro/memory/list`, `_kiro/memory/get` | no sender | NOT-SENT (cyril-0na7) |
| `_kiro/session/rename`, `_kiro/session/delete` | no sender | NOT-SENT |
| `_kiro/config/template` | no sender | NOT-SENT |
| `_kiro/sourceProviders/list`, `…/listResources` | no sender | NOT-SENT |
| `_kiro/mcp/resetServer` | no sender | NOT-SENT |
| `_kiro/spec/resolveSession`, `_kiro/spec/invoke` | no sender (no `_kiro/spec/*` anywhere in `crates/`) | NOT-SENT |
| `_kiro/codeIntelligence` | no method sender — only the *setting* `chat.enableCodeIntelligence` (`kas/settings.rs:81`) | NOT-SENT |
| `_kiro/knowledge` | advertised in `fingerprint.rs:138`, never called | NOT-SENT |
| `_kiro/policy/ignore_files_changed` (notification) | no sender | NOT-SENT |
| `_kiro/powers/refresh` (notification) | **deliberately unused** — `convert/kas/powers.rs:29-31` says the pull methods are unverifiable; cyril only *receives* `_kiro/powers/items_changed` | NOT-SENT (by design) |
| `kiro.dev/session/list` | no sender | NOT-SENT |
| `kiro.dev/settings/set` | no sender (cyril reads settings, never writes them) | NOT-SENT |

Plus the workflow family, reached through a dedicated client rather than the ext helpers (so not in the 23): the TUI calls `_kiro/workflow/{new,invoke,list,load,inspect,cancel,pause,resume,retry,…}`; cyril covers all but `pause` (`convert/kas/workflow.rs`, `cyril-jxw3`/`4u4a`). **None of this is a 2.29.x delta** — the outbound set is byte-identical between the two bundles.

**C. cyril → KAS INBOUND parity (reverse direction): cyril answers 19 of the requests the TUI answers.** cyril's responder census is `crates/cyril-core/src/protocol/kas/callbacks.rs:531` `WIRE_METHODS` (19: `auth/getAccessToken`, bare `fs/read_text_file|write_text_file`, `_kiro/fs/{read_file,write_file,stat,read_directory,delete}`, `terminal/{create,wait_for_exit,output,release,kill}`, `_kiro/terminal/shell_type`, `_kiro/hooks/{list,executeHook,sessionStart,cancel,didChange}`). The 2.29.1 TUI advertises and *answers* — via its capability table (`{type:"other",key:"…",method:"…",handler}`) and `onExtNotification` subscriptions — `secretStorage` → `_kiro/secret/{get,store,delete}`, `userInput` → `_kiro/userInput`, `openExternalUrl` → `_kiro/openExternalUrl`, `copyUrlToClipboard`, and it handles `_kiro/tools/content_chunk`, `_kiro/mcp/status`, `_kiro/customAgent/*`, `_kiro/sandbox/status`, `_kiro/diagnostics/changed`, `_kiro/steering/documents_changed`, `_kiro/spec/{phaseCheckpoint,taskStatusChanged}`, `kiro.dev/goal/status`, `kiro.dev/telemetry/{identityChanged,modeChanged,processHealth,uiModeChanged,…}`, `kiro.dev/{session/list,settings/set}` — **none of which is a literal anywhere in `crates/`** (`tui-vs-cyril-methods.py`; corpus = `find crates -name '*.rs' | xargs cat`). Those are the names where cyril is not yet a capable counterpart: a KAS-emitted server→client request for one of them gets `MethodNotFound` from cyril. Several are already documented as missing by the covenant (`_kiro/search/*`, `_kiro/safety/*`, `_kiro/sessions/changed`) — still not a 2.29.x delta, but it is the honest answer to "what the TUI does that cyril does not", and it is far larger than the settings delta.

The literal-absence test is a *lead*, not a proof of absence: cyril could build a name dynamically. I checked for that — the only dynamic families in cyril are `_kiro/workflow/{…}` (via the workflow module's own literals) and `_kiro/powers/{…}`; no `format!`/`strip_prefix` builds a `_iro/session`, `_iro/memory`, `_iro/secret`, `_iro/spec`, `_iro/sandbox` or `kiro.dev/` name (grep for `starts_with("kiro.dev/` / `strip_prefix("_kiro/` = 0 hits).

**`_meta.kiro.settings` keys** (literal quoted from `kiro-tui-2.29.1.js`; cyril's marshaller is `crates/cyril-core/src/protocol/kas/settings.rs:109-179`). New or newly-shaped in 2.29.1 marked ★:

| key | TUI literal | cyril |
|---|---|---|
| ★ `memory` | `t.memory=s` (shape from `m9`: `{mode:"disabled"\|"read_only"\|"read_write", reflection:bool}`) | not emitted |
| ★ `memoryEnable` | `t.memoryEnable={enabled:!0}` | not emitted |
| ★ `userMemoryOptIn` | `t.userMemoryOptIn={enabled:s.mode==="read_write"&&s.reflection}` | not emitted (and the 2.28.0 flat row the TUI dropped is absent from cyril too) |
| `largeToolOutputHandler` | `["chat.enableLargeToolOutputHandler","largeToolOutputHandler"]` + `{…,largeToolOutputHandler:!0}` default | **not emitted** (cyril maps `chat.enableTodoList→todoList` instead — a dead KAS key) |
| `c2s` | `if(Fn("c2s"))t.c2s={enabled:e[rt.CHAT_ENABLE_C2S]===!0}` | not emitted |
| `workflowNotifications` | `t.workflowNotifications={enabled:n.enabled,delivery:d2(…)}` | not emitted |
| `backgroundExecution` | `PTn=[["background_execution","backgroundExecution"]]` → `{enabled:!0}` | not emitted |
| `workflows`, `goal` | `GTn(t,n){if(n.available)e.workflows={enabled:t.enabled},e.goal={enabled:t.enabled}}` | deliberately **never** emitted (ADR-0011; fenced by `settings.rs:466-487`) |
| `infraSafetyMonitor`, `infraSafetyEnforce` | `if(process.env.KIRO_INFRA_SAFETY_ROLLOUT_ENABLED==="1")a.push(["chat.enableInfraSafetyMonitor","infraSafetyMonitor"],…["chat.enableInfraSafetyEnforce","infraSafetyEnforce"])` | not emitted (env-gated dark) |
| (`todoList`) | **absent from the 2.29.1 TUI table** | cyril still emits it (`settings.rs:82`) — stale, cyril-53qx |

Not a settings key but also TUI-sent and cyril-absent: `session/new` `_meta.kiro.overrideSessionId` (D6) and the KAS-spawn env `KIRO_CLI_IS_CHILD_PROCESS="1"` (D9). Conversely, cyril sends `subagentOrchestration:{enabled:true}` (`settings.rs:93-98`) which the 2.29.1 TUI does **not** (`subagentOrchestration` = 0 hits in both bundles) — keep it per cyril-53qx.

---

## Method notes

*Scripts (this lane, read-only, re-runnable):* `static-tui-vocab-2.29.1.py` (copy of the 2.28.0 lexer; adapt-and-copy only, the 2.28.0 originals untouched), `static-tui-gates-2.29.1.py`, `extract-sets.py` (trap-3 broad token families: `_kiro/*`, `_kiro.dev/*`, `KIRO_*`, `AB_*`, `AMAZON_Q_*`/`AWS_*`/`Q_*`, slash commands, `sessionUpdate:"…"`, `kind:"…"`, `sendExtMethod("…")`), `skeleton-diff.py` (identifier→`I`/number→`N`/string→`S(content)`/template→`T(normalised)` token multiset — a minifier-invariant check that no large logic block was added; net −137 tokens, +46 vs −57 distinct tokens, all accounted for above).

*Raw outputs in this directory:* `vocab/` (per-version literal+prop sets, `2.28.0__2.29.1.{strings,props}.{added,removed}`, `.buckets.txt`, `.context.txt`), `gates-2.28.0-2.29.1.txt`, `sets-diff.txt` + `sets-<ver>.<family>.txt`, `skeleton-diff.txt`, `token-counts-both.txt`, `settings-builder.{2.28.0,2.29.1}.{txt,wrapped.txt}`, `interaction-resolved.2.29.1.txt`, `outbound-methods.txt`, `tui-vs-cyril-methods.txt`.

*Parity script (reusable):* `tui-vs-cyril-methods.py` — extracts the outbound call literals from both bundles and checks each against a `crates/` string-literal corpus (both `_kiro/x` and `kiro/x` spellings), then does the same for the 2.29.1 capability-table/`onExtNotification` names. This is the check the version diff cannot make; re-run it every audit instead of re-deriving "no new method" by hand.

*Trap compliance notes:* every add/remove claim above carries a count in **both** bundles plus ≤200-char quoted context (§Explicit answer, §Negative results, §Unreleased). Renames are called out rather than counted as changes (e.g. `Pn`→`Fn`, `w1`→`C1`, `s9`→`m9`, `J1n`→`UTn`, `Z1n`→`PTn`, `qX`→`XX`, `wtn`→`pwt`, `NJe`→`LTn`). The 55k "statement-split diff lines" reported in RUN-STATE are minifier churn: the *literal* delta is 44/3 and the *method* delta is zero — do not read the line count as a behaviour estimate. No claim in this file is LIVE; every cyril verdict cites `file:line`, and anything without a line is marked N/A or "not in my lane" rather than "harmless". `tui.js` was read from the archived research copy, never from `~/.local/share/kiro-cli/` (trap 18). Nothing was executed from Kiro (trap 6); the KAS carves were grepped read-only for cross-reference only.

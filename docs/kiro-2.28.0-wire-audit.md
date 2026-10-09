# kiro-cli 2.28.0 wire audit — Classic deprecation, configuration layers, model fallback (covers 2.26.1, 2.27.0, 2.27.1, 2.28.0)

Audited 2026-10-08. Baseline: 2.26.0 ([kiro-2.26.0-wire-audit.md](kiro-2.26.0-wire-audit.md)).
Scope: **discovery, not only compatibility.** We looked for new ACP routes, new params,
new response/notification fields, feature flags and unannounced features. There were
four lanes: KAS static, Rust host static, TUI-bundle static (Kiro's own TUI as the
reference ACP client), and live same-day captures on both engines, 2.26.0 vs 2.28.0.

**Verdict: SAFE for cyril.** No existing cyril path broke on either engine, and the
workflow control plane is byte-for-byte the same. The *new* surface is large but
almost all of it is opt-in, and cyril drops or doesn't send any of it today. The
three findings that matter most:

1. **Classic (v2) is being deprecated with Kiro CLI 3.0 in October 2026**
   (`launch/classic_nudge.rs`). cyril still defaults to v2. Decision taken: KAS becomes
   the default (cyril-4c4d; watch: cyril-a7nk).
2. **"Always allow" on a workflow *step* approval loops 20 times, then fails the step.**
   This is live on 0.66.15 and 0.66.26. KAS doesn't honour step-session always-grants;
   the TUI works around it client-side (cyril-w55l, P1).
3. **v2 model fallback is default-on from 2.27.1.** cyril drops `model_fallback`
   (cyril-ci3j). The live capture (§3a) narrowed the impact: a capacity fallback only
   moves a turn when a fallback target is configured, and cyril sessions can't configure
   one. So under cyril the only possible silent move is the unverified refusal path.
   ci3j was re-scoped to a minimal P2, and `/model fallback` (cyril-lnxg) was deferred
   because v2 is retiring.

Per-lane evidence, with quoted contexts, HANDLED/DROPPED tables and exact probe recipes:
[`experiments/conductor-spike/audit-2.28.0/`](../experiments/conductor-spike/audit-2.28.0/)
(`kas-findings.md`, `host-findings.md`, `tui-findings.md`, `live-findings.md`).

## 1. Releases and artifacts

| ver | build | `@kiro/agent` (KAS) | host / TUI | headline |
|---|---|---|---|---|
| 2.26.1 | 2026-09-30 | 0.66.15 (**byte-identical** to 2.26.0) | changed | v2 `stopReason: refusal`; `NODE_USE_SYSTEM_CA`; herdr pane integration (TUI) |
| 2.27.0 | 2026-10-01 | **0.66.22** | changed | configuration layers, `backgroundExecution`, `ftaVibe→validation`, saved-prompt commands, memory controls (TUI) |
| 2.27.1 | 2026-10-02 | 0.66.22 (license file only) | changed | **model fallback on**, trust-classifier shadow, `/tangent merge`, `/kvim` (dark), `_kiro/session/history` used by TUI |
| 2.28.0 | 2026-10-05 | **0.66.26** | changed | Classic→3.0 nudge, `_kiro/terminal/write`, cascade routing, `contextBreakdown`, hook tool tags, Windows KAS supervisor |

The 2.28.0 tarball matched `latest/manifest.json` sha256 `f48ef68d…`, and its
`kiro-cli-chat` is byte-identical to the installed binary. Archived at
`~/.local/share/kiro-research/binaries/<ver>/`. KAS was carved (first zstd frame) to
`kas-carves/<ver>/` and the TUI bundles (the `#!/usr/bin/env bun` zstd frame, about 96 MB
into the binary) to `tui-bundles/kiro-tui-<ver>.js`.

**Changelog lane change:** `kiro-cli version --changelog` is now fetched **remotely**,
so every archived binary prints the current feed. That made the 2.26.1/2.27.0 entries
look missing. The authoritative per-version source is
`prod.download.cli.kiro.dev/stable/<ver>/feed.json` (all four are in `host-findings.md` §7).

## 2. KAS 0.66.15 → 0.66.22 → 0.66.26

The `initialize` result, `agentCapabilities`, the ACP `sessionUpdate` variants and the
workflow event literals are unchanged. `@kiro/acp-type-covenant` is still **not shipped**:
only a version pin plus an erased type import; the zod schemas are inlined at about
2 MB in `acp-server.js`.

| item | ver | direction / gate | live | cyril | issue |
|---|---|---|---|---|---|
| `_kiro/configuration/state` | .22 | agent→client; opt-in `clientCapabilities._meta.kiro.configurationState` | ✅ shape `{view, layers, settings[{krn, value, compose, assertions, contributions}], resources}`. Static guessed a different row shape. (The live claim that initialize settings do not appear was **invalid**, because the probe mis-placed them; see §9) | NOT-SENT | cyril-iq2c |
| `_kiro/configuration/contribute` | .22 | client→agent | unreachable standalone (`enableConfigurationContributeMethod` never set) | N/A | — |
| `KIRO_DUMP_CONFIGURATION(_DIR)` | .22 | env debug dump of resolved layers | — | — | — |
| multi-client server (`SharedInitialization`, `ClientConnections`, `single-client-stream`) | .22 | formalises the `kiro-cli serve` ws mux; first initialize result shared with every client | no change for one stdio client | N/A | cyril-5g2o note |
| `settings.backgroundExecution` | .22 | session/new setting / flag `background_execution` / env | — | NOT-SENT | cyril-7p7n |
| `ftaVibe` → `validation` (rename) | .22 | `ftaVibe` now silently ignored | — | stale map | cyril-53qx |
| `todoList` setting and `todo_list` tool **removed** | .22 | — | — | stale map | cyril-53qx, cyril-5onw |
| workflow step **parent notes** (network-waiting/give-up) | .22 | `deliverSendMessage(…, wake=true)` + `_kiro/session/notify {sender:"step"}` | ✅ socket-reset fault → 6 retry-waits → give-up → notify + **unprompted parent turn** | DROPPED; billed wake | cyril-lki9, cyril-fb1m |
| saved prompts as slash commands | .22 | `available_commands` `_meta.kiro.{type:"prompt", resource, arguments}` | ✅ only `${1}` / `$ARGUMENTS` substitute (bare `$1` raw) | _meta DROPPED | cyril-79ox |
| steering `#[[file:…]]` / `#[[folder:…]]` refs | .22 | agent-side | — | N/A | — |
| permission engine fails closed if cedar-wasm can't load | .22 | — | — | N/A | — |
| `_kiro/terminal/write {sessionId, terminalId, input}` | .26 | agent→client; opt-in `clientCapabilities._meta.kiro.terminalInput` | ❌ not provoked (model never chose interactive) | NOT-SENT | cyril-7p7n |
| background execution v2 (monitor, interactive, adopt, stall detection; bg `terminal/create` + `outputByteLimit`) | .26 | — | — | partial | cyril-7p7n, cyril-1rpv |
| model routing "cascade" (`auto*` models) | .26 | `settings.cascade` / `KIRO_FEATURE_CASCADE_CONFIG` / AB | ✅ **hidden classifier model call every turn, nothing on the wire**, no `model_routed` seen | NOT-SENT | cyril-ojum |
| `session_info_update` kind `model_routed` | .26 | off unless `clientNotificationEnabled` | not seen | DROPPED | cyril-ojum |
| `turn_completion.contextBreakdown` (schema 1) | .26 | `session/prompt _meta.kiro.contextBreakdown:"summary"\|"detailed"` | ✅ "detailed" == "summary" output | NOT-SENT | cyril-1116, cyril-0s9x |
| `_kiro/hooks/list` `toolTags` | .26 (filter); agent→client param ≥.15 | hook matchers on tool tags (`shell`, `read`, `@builtin`, `@mcp`) | — | **ignored** in `kas/callbacks.rs` | cyril-5nep |
| interrupted tool calls | .26 | text only: "This tool was interrupted before it reported a result…" / "…cancelled before it ran." | — | HANDLED | cyril-fjfu note |
| dynamic delegation (`invoke_sub_agent` + `modelCategory`) | .26 | AB/JSON env, default off | — | rawInput only | cyril-ojum |
| KUTS telemetry flag (default **true**) | .26 | `telemetry.*.kiro.dev`, `ClientType:"acp-other"` | ✅ **no egress** in any variant (incl. 240 s idle) | no policy sent | cyril-zjm5 |
| `KIRO_LEGACY_RESPONSES_REPLAY` | .26 | GPT Responses reasoning replay; not on the wire | — | N/A | — |
| `validate_workflow` tool **removed** (folded into `save_workflow_definition`); `creator_override_refused` | .26 | — | ✅ removed | skill stale | cyril-4u4a, cyril-0asq |
| watch handler `background-process` | .26 | runs **inside KAS** (no client terminal) | ✅ 0.66.15 rejects at `new` | opaque | cyril-4u4a |

Flags: 0.66.22 added `validation` and `background_execution` and removed `fta_vibe`;
0.66.26 added `kuts_telemetry` (default true), `cascade_config` and `dynamic_delegation_config`.
The JSON-valued flags need the fixed extractor `static-kas-flags-2.28.0.py`.

## 3. v2 (Rust) engine and launcher

**The v2 ACP surface is statically frozen across the range.** The `_kiro.dev/*` method set,
the TuiCommand args structs and the `acp::schema` notification types are unchanged, and
the contexts around the `initialize`/`session/new`/`metadata`/`commands/available` anchors
are byte-identical. The protocol crates are unchanged (agent-client-protocol 0.10.4,
sacp 11.0.0). This is consistent with maintenance mode ahead of the deprecation.

| item | ver | live | cyril | issue |
|---|---|---|---|---|
| **model fallback switched on** (rollout row removed, gate strings gone; engine code predates 2.26.0) | 2.27.1 | `/model fallback` → `{success:true,"Fallback for X set to Y"}` on 2.28.0; 2.26.0 → "Model fallback is not available." Options gain `fallback{eligible, vendorDefault[], configured}`. A bogus `set_model` still fails the next prompt with `-32603` and no frame (the `InvalidModelId` path). **Frame captured live afterwards — see §3a** | DROPPED / NOT-SENT | cyril-ci3j (P1), cyril-lnxg (P1) |
| `stopReason: refusal` for content-filtered turns | 2.26.1 | — | HANDLED (`convert/mod.rs:134`) | — |
| `_kiro.dev/agent/not_found` fields | 2.27.1 (static) | **live contradicts static**: `{sessionId, requestedAgent, fallbackAgent}` on both versions | `sessionId` ignored | cyril-6i02 (P4) |
| trust-classifier shadow (`trust_shadow.rs`, `trust_shadow_host.rs`; hidden SAFE/UNSAFE/ABSTAIN model call per approval) | 2.27.1 | env set on `kiro-cli acp`: **no effect** (no log, no wire) | N/A | cyril-8q8p |
| permission response `_meta.provenance:"human"` (TUI) | 2.28.0 | no visible effect | NOT-SENT | cyril-8q8p |
| `kiro-cli acp`/`serve` warn that `sandbox.json` isolation is **not applied**; `SandboxFileConfig.backend` removed; `KIRO_ENABLE_LOCAL_SANDBOX` | 2.28.0 | no wire trace | N/A | cyril-be7e |
| Windows KAS supervisor/sandbox (`windows_kas_{control,process}.rs`, `windows_sandbox.rs`, hidden `windows-sandbox` / `windows-kas-stop`) | 2.28.0 | — | cyril spawns node directly | cyril-be7e |
| **Classic → 3.0 nudge** (`classic_nudge.rs`, `KIRO_CLASSIC_NUDGE_RELAUNCHED`) | 2.28.0 | launcher only | **default engine is v2** | cyril-a7nk, cyril-4c4d |
| `KIRO_TEST_DROP_FIRST_END_TURN` (test hook) | 2.28.0 | no visible effect on `acp` | — | — |
| standing gap (not a delta): `goal/status`, `mcp`/`webTools` `governance_disabled`, `settings/set`, `telemetry/*` unhandled | ≤2.26.0 | — | DROPPED | cyril-bmek |

### 3a. Addendum — `model_fallback` captured live (same day)

The user captured this with a TLS-terminating proxy that answered the primary model's
`GenerateAssistantResponse` with errors. Evidence:
[`experiments/conductor-spike/v2-model-fallback-2.28.0.md`](../experiments/conductor-spike/v2-model-fallback-2.28.0.md),
the frames in `v2-model-fallback-2.28.0.jsonl`, and the fixture
`crates/cyril-core/tests/fixtures/v2/model-fallback-live-2.28.0.json`. Engine:
`kiro-cli-chat` sha256 `ae5e172a905bd007`.

```json
{"sessionId":"…","update":{"sessionUpdate":"model_fallback","from":"claude-sonnet-5",
 "to":"claude-haiku-4.5","cause":"unavailable","source":"user","promoted":false}}
```

- **Shape:** `{from, to, cause, source, promoted}`. **`source` (`"user"` | `null`) is
  missing from the TUI-derived shape**, and it is non-null exactly when a target existed.
  `promoted` was present and `false` in every frame. `promoted:true` was never observed;
  the binary strings suggest it happens on refusals only.
- **Trigger:** HTTP 429 with `x-amzn-errortype: ThrottlingException`, reason
  `INSUFFICIENT_MODEL_CAPACITY`. There are 3 engine × 3 smithy attempts, i.e. 9 requests,
  and 8 `retry_warning` frames before the move. The capacity ones carry `capacity:true`
  and `modelId`. The turn then completes with `end_turn` and is billed on the fallback model.
- **Corrects §3's static inference:** a **503 `ServiceUnavailableException` does NOT fall
  back**. It is a separate retry class ("Retrying in 3s after a server error", no capacity
  flag), and the turn fails.
- **No configured target means no move:** the frame arrives with `to:null, source:null`,
  then `_kiro.dev/error/rate_limit`, then `-32603` "The model you've selected is
  temporarily unavailable…". `claude-opus-5.5`'s `vendorDefault` was **not** used on this
  capacity/unavailable path. The binary labels vendor defaults "refusals only", so
  refusal behaviour stays unverified. In practice, `/model fallback` is what makes
  capacity fallback work at all.
- **Non-promoted moves are turn-scoped:** the session model stays `[active]` and the next
  turn runs on it.
- **Refusal arm not producible:** forged refusal fields were accepted on the stream but not
  decoded.

**Rollout registry deltas:**
- 2.27.0: `background_execution` description now states the client contract.
- 2.27.1: added `kvim` and `trust_classifier_shadow`; **removed `model_fallback`**; `v3_prompt` went 25→35 %.
- 2.28.0: `memory` channel nightly→any; `memory_controls` went 0→100 % and nightly→any; **removed `v2_non_interactive`**.

**Env added:**
- 2.26.1: `NODE_USE_SYSTEM_CA`.
- 2.27.0: `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED`, `KIRO_ROLLOUT_FEATURES`.
- 2.27.1: `KIRO_TRUST_CLASSIFIER_SHADOW[_LOG]`, `KIRO_HEADLESS_WORKFLOW_TIMEOUT_SECS`, `KIRO_TELEMETRY_IS_ENTERPRISE`, `KIRO_TEST_KAS_TURN_LOG_PATH`.
- 2.28.0: `KIRO_CLASSIC_NUDGE_RELAUNCHED`, `KIRO_ENABLE_LOCAL_SANDBOX`, `KIRO_TEST_DROP_FIRST_END_TURN`.

**Settings added:**
- 2.27.0: `memory.mode`, `memory.reflection`, `chat.keybindings.moveToBackground`.
- 2.27.1: `trust.classifier.shadow[LogPath]`.
- 2.28.0: `chat.fullscreenCopyOnSelect`.

**Embedded doc manifests:** no node added or removed. Only `slash-commands/memories.md`
changed, and that page is public. So this cycle gave **no unannounced-feature signal from
the manifest lane**. The unannounced items came from the rollout table and the bundles (§5).

## 4. What Kiro's own TUI now sends and consumes (reference client)

Across all five bundles the set of wire-method literals grew by exactly one,
`_kiro/session/history` (2.27.1).

| TUI behaviour | live | cyril | issue |
|---|---|---|---|
| no longer sends `subagentOrchestration` (setting and env removed, 2.28.0) | ~~no effect either way~~ **INVALID** (probe mis-placed the setting; see §9). Code: when active, the orchestration tool builder replaces `invoke_sub_agent`, and with workflows on the chat delegation tool is kept | still sends `{enabled:true}`, correctly placed; **keep it** | cyril-53qx |
| memory controls `memory:{mode, reflection}` at initialize; `memoryReflection` config option; reads `session/new._meta.memoryConfig` | ~~initialize memory settings ignored~~ **INVALID** (mis-placed; see §9) | NOT-SENT | cyril-0na7 |
| `_kiro/session/history {sessionId, beforeMessageId, limit}` | **empty unless `beforeMessageId`** | NOT-SENT | cyril-a3th |
| `session/prompt _meta.kiro.displayText` | ✅ persisted and shown on reload | NOT-SENT | cyril-hzkb |
| child-consent grant table auto-approves matching step-session approvals | needed: KAS loops 20× otherwise | absent | cyril-w55l, cyril-gn07 |
| `/workflow new` canned prompt: refuse rather than hand-author if `wf-workflow-creator` is unavailable | — | — | cyril-0asq |
| `session/close` on finished step sessions | — | — | cyril-4u4a |
| KAS spawn env `NODE_USE_SYSTEM_CA=1`, `KIRO_CUSTOM_USER_AGENT`; Windows supervisor argv | — | absent | cyril-be7e |
| renders shell rawOutput `agent_notes` (`$AGENT_CONTEXT_OUT`) | — | DROPPED | cyril-4pte |
| `session/load _meta.kiro.requireExisting` (capability `strictSessionLoad`) | **ignored by 0.66.26**: loading a missing id creates it (TUI is ahead of the engine) | — | — |
| `_kiro/terminal/{moveToBackground, output, stopBackground, stopAllBackground}` | still unserved by KAS | — | — |
| mid-turn `/model` and `/effort` pickers | queued client-side as `/reasoning` and sent after the turn | already handled | — |

## 5. Unannounced or dark features spotted

- **Configuration layers** (`_kiro/configuration/*`, KRN addressing, 11 layers including "Your Organization" and "Cloud Config"). This is the substrate for org-managed settings, and only the opt-in read side is reachable.
- **Model routing / dynamic delegation** over `auto-fast|auto-balanced|auto-smart` categories (AB-gated, default off). Hidden extra billed calls.
- **Trust-classifier shadow** on v2 approvals (internal nightly), i.e. data collection for automated approvals.
- **`/kvim`**, embedded Neovim (rollout `kvim`, internal), and **herdr pane integration** (2.26.1).
- **Windows KAS supervisor + sandbox** and the local-sandbox rework.
- **Classic deprecation** with 3.0 in October, plus the `v3_prompt` rollout at 35 %.

## 6. Workflows (v3) — live, gate on and off × 0.66.15 and 0.66.26

- **Control plane unchanged:**
  - all 14 `_kiro/workflow/*` methods, every recipe shape and the event ordering are identical;
  - shapes covered: linear with steer/pause/resume; repeat with update/extendRepeat/cancel/retry/delete; command watch; interactive step; YAML; user tier; `agent://`; a custom agent calling `run_workflow`;
  - guessed method names return `-32603`, never `-32601`.
- `recipes_changed` fires only with the gate on. `workflow/new` without `inputs` gives `-32602`; cyril already sends `{}`.
- **Auto-wake:** every terminal `run_complete` wakes the parent into a billed turn, on both versions, gate on or off. The wake `turn_completion` carries credit usage on both, so the 2.27.1 "wake usage accounting" is TUI-side. New since 0.66.22: **network give-up parent notes also wake the parent** (cyril-lki9).
- **Approvals:**
  - "Always allow" on a step approval loops through `consentRound` 1→20 and is then rejected (cyril-w55l).
  - Watch-command approvals arrive on the **parent** session.
- New `background-process` watch handler (runs inside KAS). New state fields `backgroundExecution` and `memoryConfigSource` (DROPPED). `validate_workflow` is gone.
- The workflow ext methods **are** listed in `agentCapabilities._meta.kiro.extensionMethods`. The `kiro-workflow-authoring` skill says otherwise, so the skill is stale; that and the recipe schema need a 2.28.0 refresh (cyril-4u4a).

## 7. Issues

New:

| issue | P | title |
|---|---|---|
| cyril-4c4d | 1 | Make KAS (v3) cyril's default engine |
| cyril-w55l | 1 | "Always allow" on a workflow step approval loops 20× then fails the step |
| cyril-ci3j | 2 | v2: handle `model_fallback` (re-scoped minimal; v2 retiring) |
| cyril-a7nk | 1 | Kiro CLI 3.0 deprecates Classic/v2: watch and sunset |
| cyril-lnxg | 3 | v2 `/model fallback` — DEFERRED (v2 retiring; revive only if cyril-4c4d slips) |
| cyril-53qx | 2 | KAS settings marshal drift |
| cyril-0na7 | 2 | KAS memory controls |
| cyril-ojum | 2 | KAS cascade routing / `model_routed` |
| cyril-7p7n | 2 | `_kiro/terminal/write` + `terminalInput` |
| cyril-5nep | 2 | `hooks/list` `toolTags` |
| cyril-be7e | 2 | KAS spawn parity (Windows supervisor, system CA, UA) |
| cyril-zjm5 | 3 | KAS telemetry policy |
| cyril-iq2c | 3 | `_kiro/configuration/state` |
| cyril-79ox | 3 | saved-prompt commands |
| cyril-a3th | 3 | `_kiro/session/history` paging |
| cyril-hzkb | 3 | `displayText` on synthetic prompts |
| cyril-4pte | 3 | render `agent_notes` |
| cyril-bmek | 3 | v2 standing ext-notification coverage gap |
| cyril-66tl | 3 | `sweep-new-fields.py` walks only 5 array elements |
| cyril-6i02 | 4 | `agent/not_found` `sessionId` |
| cyril-8q8p | 4 | permission `provenance:"human"` |

Notes were added to: cyril-lki9, fb1m, 4u4a, 0asq, gn07, 1116, 0s9x, 1rpv, 5onw, fjfu and 7q8u.

## 8. Method notes for the next audit

- The changelog is remote now. Use `stable/<ver>/feed.json`, not `version --changelog` on an archived binary.
- **Static predictions were wrong twice:** the `agent/not_found` 4th field and the `configuration/state` row shape. Keep the live lane, but see §9: **the live lane was wrong three times too**, all from one mis-placed handshake field.
- `sweep-new-fields.py` samples 5 array elements, which produced a false "removed `promptScoped`" (cyril-66tl).
- KAS legs spawned `node acp-server.js` straight from the carve, so no Rust host ran and the stale-engine cleanup never fired. v2 legs put the archive `bin/` first on PATH and recorded child exe proofs.
- Subagents may be refused the Write tool for findings files. Have them write via Bash, and return full findings in the hand-back.
- Scripts in `experiments/conductor-spike/`:
  - `static-{kas,host,tui,rollout,doc-manifests}-*-2.28.0.py`
  - `probe-{v2,kas}-*-2.28.0.py`
- Captures in the same directory: `{v2,kas}-live-sweep-{2.26.0,2.28.0}.jsonl` and `kas-workflow-live-{2.26.0,2.28.0}.jsonl` (trimmed; tokens redacted).

## 9. Correction (same day): initialize settings were mis-placed in the live probe

The live probe's `init()` helper (`probe-kas-leads-2.28.0.py`) sent "initialize settings"
at the **initialize request's top-level `_meta.kiro.settings`**. KAS 0.66.15 and 0.66.26
read **only** `initialize.clientCapabilities._meta.kiro`
(`let r=t.clientCapabilities?._meta?.kiro … this.clientMeta=r`). That is also where cyril
sends them (`engine::client_capabilities`). Earlier audits and CLAUDE.md wrote
"`initialize._meta.kiro.settings`" as shorthand for the `clientCapabilities` form, and the
2.28.0 probe took the shorthand literally. These three conclusions are therefore
**invalid**, and are being re-measured with `probe-kas-initsettings-2.28.0.py`
(results go in `audit-2.28.0/live-findings.md` §7):

1. "`subagentOrchestration` has no effect." Code says otherwise:
   `subagentOrchestrationActive()` reads `clientMeta.settings`; when active, the
   orchestration tool builder is used, and with workflows on the chat delegation tool
   is **not** suppressed. cyril-53qx was corrected to **keep** sending it.
2. "KAS memory settings at initialize are ignored" (cyril-0na7).
3. "Settings sent at initialize don't appear in `_kiro/configuration/state`" (cyril-iq2c).

Rule for future probes: write the full path,
`initialize.clientCapabilities._meta.kiro.settings`, and assert in the probe that a
known connection-level gate (e.g. `largeToolOutputHandler`) takes effect, as a control.


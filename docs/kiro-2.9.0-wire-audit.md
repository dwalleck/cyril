# Kiro CLI 2.9.0 — wire audit (diff vs 2.8.1)

**Analyzed:** 2026-06-23 · **Method:** downloaded + SHA-verified 2.9.0 headless tarball (archived to `~/.local/share/kiro-research/binaries/2.9.0/`) vs archived 2.8.1; same-day live v2 surface capture from both chat binaries; same-day live KAS `initialize` + `session/new` capture from both bundles (direct-spawn free path); `nm`+`rustfilt` module-path diff of `kiro-cli-chat`; embedded `@kiro/agent` version probe; self-extracted KAS 0.3.299 bundle + covenant `.d.ts` diff vs on-disk 0.3.257; embedded doc-manifest delta; embedded `feed.json` changelog extraction; `/lite` deep-dive in the carved tui.js region. Single environment (this user's social/GitHub token, non-enterprise).

**Verdict for cyril: SAFE — nothing changed on the v2 path cyril drives.** The v2 (`kiro-cli acp`) wire surface is **binary-identical** to 2.8.1 (24 commands, 15 tools — proven same-day from two chat binaries). No cyril code change is warranted by 2.9.0; safe to upgrade. The two real, undocumented changes are both off cyril's current path: (1) a new **frontend "lite mode"** rendering subsystem (`/lite` / `/tui` / `/verbosity`), gated behind a gradual-rollout flag, **telemetry-only — never on the ACP wire**; and (2) the embedded KAS bundle jumped **`@kiro/agent` 0.3.257 → 0.3.299** with **field-level (not method-level) additive refinements** to the policy/safety/hooks subsystems.

Build: `BUILD_VERSION=2.9.0`, `BUILD_HASH=e542597866f6eab50d3fcf12268544dc544ef1bb`, built 2026-06-23. Tarball (tar.zst) SHA-256 `ee9ef0a203f7e1d73585c377cb993b506828bd3121d7f037390e06efbc054267` (from `latest/manifest.json`). Released 2026-06-23 (feed.json dates the entry 2026-06-22).

---

## Size deltas (2.8.1 → 2.9.0) — breaks the recent "always grow" pattern

| Binary | 2.8.1 | 2.9.0 | Δ |
|---|---|---|---|
| `kiro-cli` | 119,012,528 | 118,982,224 | **−30,304** (noise) |
| `kiro-cli-chat` | 729,676,608 | 728,826,384 | **−850,224** (shrank!) |
| `kiro-cli-term` | 86,822,256 | 86,886,320 | +64,064 (minor) |

Every recent release *grew* `kiro-cli-chat` via KAS bundle bumps (2.8.1 added ~335 KB). 2.9.0 is the first minor in a while that **shrinks** it — a net result of the telemetry-crate extraction + image-codec churn below offsetting the KAS bump. Not the "big additive" a `.9.0` number might suggest.

## Changelog (the embedded `feed.json`)

> The CLI `version --changelog` snapshot is **stale — it tops out at 2.6.0** (2.6.1…2.9.0 absent). Since 2.6.1 the changelog ships as the network/`feed.json`-backed `/changelog` panel (`inputType:"panel"`). The 2.9.0 entry was extracted from the embedded `feed.json` (`"$schema":"./feed-schema.json"`).

**Version 2.9.0** (7 entries, 4 `[V3]`/KAS-tagged):
- *fixed*: External IdP token refresh now replays scopes, fixing session expiration for Entra ID (Azure AD) users
- *fixed*: Custom agents no longer load a file twice when it is both listed in resources and matched by an inherited resource glob
- *fixed*: Arrow keys now navigate correctly in tool approval prompts instead of cancelling edit mode
- *fixed*: [V3] Agent-crew sub-agent activity no longer appears duplicated in the main conversation
- *fixed*: [V3] Show the credits column in the `/model` picker, matching v2
- *fixed*: [V3] Compound shell commands (e.g. `git status && echo done`) no longer loop on the approval prompt; the trust exact-match and pattern options now target the gated sub-command
- *changed*: [V3] Sub-agent tool cards show a one-line prompt preview (expand with `ctrl+o`) instead of full arguments

**`/lite` is not mentioned anywhere in the changelog** — see below.

## v2 (default Rust engine) — unchanged, proven same-day

Live `commands/available` capture against **both** chat binaries, same backend, same hour:

- **24 slash commands** (identical): `agent chat clear code compact context effort feedback goal guide help hooks knowledge mcp model paste plan prompts quit reply rewind stats tools usage`
- **15 tools** (identical): `code glob goal grep introspect knowledge read shell subagent todo_list tool_search use_aws web_fetch web_search write`

**`tool_search` is a backend rollout, not a 2.9.0 binary change.** The 2.8.1 audit (2026-06-18) recorded **14** tools; today both 2.8.1 *and* 2.9.0 binaries report **15** (adds `tool_search`). Since both binaries agree today, this is an AWS backend rollout between 2026-06-18 and 2026-06-23 — the binary↔backend axis distinction (see `reference_kiro_wire_audit_methodology`). The 2.8.1↔2.9.0 *binary* delta for the v2 surface is **zero**. (Binary-side, `tool_search` was also promoted to a first-class `agent::agent::tools::tool_search` module — see symbol diff.)

## The headline undocumented change: frontend "lite mode" (`/lite` / `/tui` / `/verbosity`)

Three new **local** TUI commands (all `source:"local", meta:{local:!0}` — handled client-side in the React/Ink layer, **never crossing the ACP wire**):

```
{name:"/lite",      description:"Switch to lite mode"}
{name:"/tui",       description:"Switch to TUI mode"}
{name:"/verbosity", description:"Configure lite-mode rendering: tool args, reasoning, output
                                 filters, density, subagent sections.", meta:{liteOnly:!0}}
```

**What lite mode is:** a **scrollback-based, non-alternate-screen rendering mode** — an alternative to the full-screen Ink "TUI mode." The smoking gun is `liteScrollbackClearToken` (lite renders into the terminal's native scrollback rather than taking over the screen). State lives in a `uiMode ∈ {"lite","tui"}` store (`setUiMode`), persisted to the `chat.ui.mode` setting.

**How it ships:**
- **Gradual rollout, flag-gated.** New Rust module `chat_cli::rollout` / `chat_cli_v2::rollout` (`Rollout::{init,is_enabled,variation}`, `FeatureRollout`/`Channel`/`Segment` deserialized from JSON) = a percentage/segment feature-flag framework. Lite mode's local override env var is **`KIRO_LITE_ROLLOUT_ENABLED`** (seen adjacent to `KIRO_VERSION`/`KIRO_VERSION_OVERRIDE` in the data section). This is *why* lite mode is absent from the changelog and doc-manifest — it's staged, not launched.
- **First-launch choice.** `firstLaunchUiModeRequested` + `confirmFirstLaunchUiMode(s)` prompt eligible new users to pick lite vs TUI and persist the choice; the server can flip the *default* (`UiModeDefaultChangedNotification`).
- **KAS = lite-only.** The local-command builder filters out `/tui` when `agentEngine==="kas"` (`.filter((s)=>n!=="kas"||s.name!=="/tui")`) — the v3 engine ships without the full TUI.
- **Telemetry-instrumented (off-wire).** `kiro.dev/telemetry/uiMode{SessionStart,Changed,DefaultChanged}` with Rust structs `UiMode{SessionStart,Changed,DefaultChanged}Notification` carrying `from_mode`/`to_mode`/`ui_mode_source`/**`memory_mb`** — Kiro is A/B-measuring adoption *and memory footprint*. These go to AWS telemetry, **not** to ACP clients.

**Cyril impact: none.** Lite mode is entirely a Kiro-frontend + telemetry concern; cyril is its own frontend and renders its own way. Strategic read: Kiro is converging on an inline/scrollback model (closer to how cyril already streams) and defaulting KAS to it.

## Binary symbol diff (`nm`+`rustfilt` module paths) — 36 added / 23 removed

Meaningful this time (not the zero-delta of recent releases), but **no v2 wire surface change**:

**Refactors (no wire impact):**
- **Telemetry extracted to a new crate `kiro_telemetry_observer`** (`context::{AcpClientInfo,TelemetryContext}`, `observer::{TelemetryObserver,TelemetryObserverHandle}`) — the same symbols left `chat_cli_v2::telemetry::observer::*`. Pure move.
- `agent::agent::tools::glob` removed but `glob` still on the wire → implementation moved (new `agent::agent::tools::fs_read::file`); tool unchanged.
- `chat_cli::cli::chat::tools::switch_to_execution` removed (old spec/execution-mode switch).

**New modules:** `chat_cli::rollout` (lite-mode rollout, above), `chat_cli::util::resource_permission` (matches the changelog "custom agents…inherited resource glob" fix), `agent::agent::tools::tool_search` (tool_search promoted to a built-in tool module), `chat_cli::cli::agent::legacy::hooks`, `chat_cli_v2::telemetry::{host_config,metadata_provider}`.

**ACP schema-crate reshape (flag for cyril's own ACP tracking):** `agent_client_protocol_schema::agent` **added** `SessionModelState` + `SessionModeState`, **removed** `SessionModeId`, `SessionConfigValueId`, and `ext::ExtRequest`. The ACP schema types are evolving (consistent with the sacp→ACP-v1 merge tracked in `reference_sacp_acp_v1_merge`). No observed v2 wire change resulted, but cyril should watch its own `agent-client-protocol`/schema dependency for the same renames.

**Image/codec dependency churn (added):** `png::text_metadata`, `tiff::encoder::tiff_value::TiffValue`, `exr::io::Tracking`, `zip::crc32::Crc32Reader`, `zstd::stream::read::Decoder`, `deflate64`, `liblzma::stream`, `flate2::deflate`, `sha1::compress`, `html2text::render::text_renderer`. **Removed:** `amzn_codewhisperer_streaming_client::types::_image_block::ImageBlockBuilder`, `tiff::encoder::{TiffEncoder,writer}`. Net read: **image handling was refactored**, not newly added — KAS already advertised `promptCapabilities.image:true` in 2.8.1 (see KAS section). Purpose of the codec set is not wire-confirmed (likely host-side image decode for paste/attachments and/or `html2text` for web_fetch rendering); **not a v2 ACP wire change**.

## KAS / V3 — `@kiro/agent` 0.3.257 → 0.3.299 (42 versions), but additive field-level only

Embedded version string `"@kiro/agent":"0.3.257"` → `"0.3.299"`. Self-extracted the 0.3.299 bundle (direct-spawn free path into a throwaway HOME, leaving the installed 0.3.257 untouched) and diffed the authoritative `@kiro/acp-type-covenant` `.d.ts`:

**Covenant is structurally STABLE** — 32 capability dirs, 69 `.d.ts`, 69 `_kiro/*` method strings — **zero methods added or removed** (unlike 2.8.1, which added `_kiro/sessions/changed` + `_kiro/hooks/setEnabled`). Agent `.d.ts` 664 → 665 (+1: `spec/empty-workspace-messages.d.ts`, spec-UX strings).

**Four covenant type files changed — all additive field refinements to policy/safety/hooks:**

| File | Change | Meaning |
|---|---|---|
| `capabilities/hooks/types.d.ts` | +`handlesFileHooks?: boolean` | Client capability flag: IDE clients handle PostFile* (save/create/delete) hooks via VS Code file events + the agent's `_kiro/hooks/didChange` list; CLI/headless leave it unset → the agent runs file hooks itself (no double-fire). |
| `capabilities/safety/status-changed.d.ts` | `SafetyGateStatus` +`'blocked'`; +`toolId?`, +`blockedProperties?` | The 2.8.0 safety gate can now actively **block** a tool and report which tool + which properties triggered it. |
| `session/schemas/index.d.ts` | +`policyDenial` on tool_call payloads | Structured denial metadata (`capability`, `resource`, `matchedRule{capability,match?,exclude?,effect}`, `scope`, `source`, `effect:"deny"`) now **persists across session reload** so denial explanations survive. |
| `capabilities/mcp/status-changed.d.ts` | +`errorCode?: 'timeout_too_low'` | Structured MCP timeout error code. |

**KAS `initialize` handshake: IDENTICAL 2.8.1 → 2.9.0** (same-day capture from both bundles). `extensionMethods` (6: `_kiro/knowledge`, `_kiro/codeIntelligence`, `_kiro/session/{context,compact,export,history}`), `promptCapabilities{image,embeddedContext}`, `sessionCapabilities{list:{}, fork:{_meta.kiro.messageId}}`, `_meta.kiro{checkpoints,sessionList,policyNotifications}` — all unchanged. *(Note: `image`/`messageId` were already present in 0.3.257; the 2.8.1 audit prose just didn't enumerate them — captured fresh here to avoid a false "new capability" claim.)*

**KAS `session/new`: IDENTICAL 2.8.1 → 2.9.0** (modulo run-specific timestamps/session-id/log-path). Same 7 modes (`vibe`/`spec`/`quick-spec`/`bug-fix`/`plan`/`autonomous`/`semantic_reviewer`, default `vibe`), same 3 `configOptions` (`mode`/`autopilot`=on/`contentCollection`=disabled), same `_meta` (`ftaEnabled:false`, `semanticReviewEnabled:true`, `schemaVersion:"1.0.0"`, `sess_`-prefixed id). **KAS-4 (configOptions/modes) is unchanged.**

**`@kiro/agent` implementation diff (genuine 0.3.257 vs 0.3.299 `dist`).** The raw file diff is large (373 "added"/375 "removed"/227 "differ") but **dominated by content-hashed chunk filenames** (`acp-CrMcm-bn.js` etc.) that re-hash on every bundler rebuild — the JS analog of Rust LTO noise; **no feature count is readable from it.** The real semantic signal is the non-hashed `.d.ts` contracts that changed, which are **internal orchestration/policy logic, not new wire types**:
- `graphs/{sub-agent-graph, chat-agent-graph, custom-agent-graph, stateful-graph-state}.d.ts` — the internal LangGraph orchestration graphs churned (logic, not wire).
- `acp/permission-options.d.ts`: `resolvePermissionOutcome(...)` gained a **`homeDir?` parameter** — concrete evidence the changelog's "[V3] trust exact-match/pattern options now target the gated sub-command" approval fix landed **agent-side**.
- `acp/acp-event-adapter.d.ts`: reworked `AgentExecutionSummarizationComplete` → `session_info_update` bridging + a deliberate "do not overwrite the session's real title" fix. **Verified on the wire (compact A/B below): the `summarization_completed` sub-type already exists in 0.3.257 and its wire payload is byte-identical in 0.3.299 — the rework is internal logic, not a wire change.**
- `acp/infrastructure-safety/{context,types}.d.ts`, `hooks/{triggers/post-file,types}.d.ts` — mirror the covenant additions (safety `blocked`, `handlesFileHooks`).

> **Attribution caveat (corrected):** the 2.9.0 changelog `[V3]` items span *both* `@kiro/agent` *and* the tui.js frontend — they are **not** a manifest of the agent-bundle diff. Verified placements: approval/trust fix = **agent** (`homeDir` param above); "tool-card one-line preview / ctrl+o" = **frontend** (tui.js has collapsed/expanded tool-card state + "Press Tab to expand"). "Agent-crew sub-agent activity duplicated" could **not** be located in either bundle from string search — left **unattributed** rather than guessed.

**Theme:** 2.8.1 *widened* the KAS method surface; 2.9.0 *deepens existing subsystems* — orchestration graphs + policy enforcement churned internally (persisted denials, granular safety blocking, `homeDir`-aware trust, client-delegated file hooks) while the **ACP wire surface stayed frozen**. All covenant changes additive → a KAS converter's unknown-field tolerance absorbs them.

## KAS live wire capture — orchestration/subagents + command set (genuine A/B, 2026-06-23)

Beyond the static covenant/handshake diff, ran a **live KAS turn** that triggers subagent orchestration (direct-spawn free path; `stop_reason: end_turn`; **0** `_kiro/auth/getAccessToken` callbacks — free path reconfirmed). Prompt asked for two parallel subagents (write `alpha.txt`/`beta.txt`) + a review.

> **Methodology note — first A/B was contaminated, redone.** The KAS extractor is **version-gated and self-healing**: it re-extracts to match the *running* binary's embedded version. An initial 0.3.299 extraction into a throwaway HOME was silently **reverted to 0.3.257** when a 2.8.1 binary was later pointed at that same HOME (for the 2.8.1 handshake capture). So the first "0.3.257 vs 0.3.299" A/B actually compared 0.3.257 to itself. **Fix:** re-extracted 0.3.299 into a *protected* HOME never touched by a 2.8.1 binary (version verified 0.3.299 before *and* after each probe), and re-ran. The numbers below are the **genuine** A/B (`/tmp/kas-orch-257.jsonl` = installed 0.3.257 server; `/tmp/kas-orch-299-REAL.jsonl` = protected 0.3.299 server). Lesson for future audits: **never point two different kiro-cli binaries at one KAS HOME; verify the on-disk `@kiro/agent` version immediately before *and* after every KAS probe.**

**Orchestration/subagent wire model (0.3.299, unchanged from the documented v1/v2-era KAS model):**
- Subagent invocations are plain **`tool_call`s tagged `_meta.kiro.kind: "agent-subtask"`** — **NO `kiro.dev/subagent/list_update`** (that's the v2 engine's mechanism; KAS emits none, confirmed by the inbound-method set).
- Grouping key is **`agentSubtaskId`** (UUID); **`subExecutionId` was never sent** (0 occurrences). Shape: parent emits `Sub-agent: <role>` (tagged `agent-subtask`, subtaskId = the invoke tool-use id), then an update remaps it to a subtask UUID; each child's inner work (`Write File`, `Subagent Response`) carries that UUID as `agentSubtaskId` *without* its own `agent-subtask` meta.kind. This is the parent-stream grouping KAS-3 must render (no separate roster — `_kiro/sessions/changed` excludes subagents, per the 2.8.1 audit).
- Turn progress streams via **`session_info_update`** (~23–25×/turn) — the KAS analog of v2's `kiro.dev/metadata`. Commands arrive as the ACP-standard **`available_commands_update`** session/update variant (not v2's `kiro.dev/commands/available`).
- The agent spawned 2 parallel `general-task-execution` subagents; it did **not** auto-spawn `semantic_reviewer`/`functional_task_alignment` (those are mode/opt-in, not vibe-mode default).

**Genuine A/B (0.3.257 vs real 0.3.299) — structurally identical.** Field-path differ over the two raw logs:
- inbound method **set**: identical (9 methods: `session/update`, `session/request_permission`, `_kiro/{mcp/status, governance/state, tools/didChange, steering/documents_changed, sessions/changed, progressive_context/items_changed, powers/items_changed}`).
- `session/update` **kind set**: identical (6: `available_commands_update`, `session_info_update`, `config_option_update`, `tool_call`, `tool_call_update`, `agent_message_chunk`).
- `_meta.kiro.kind` value set: identical (10 shared values incl. `agent-subtask`).
- `session/update` key-paths: **139 shared**; the only paths unique to the 0.3.299 run (`rawInput.{limit,offset}`) are **run-specific tool arguments** (that run's agent read files with limit/offset; the contaminated earlier run had `rawInput.{paths,start_line,end_line}` — different args each run, proving it's argument noise). `rawInput` is a free-form echo of whatever args the agent passed, not a protocol field. **No wire-format delta.**
- Also re-verified on genuine versions: KAS `initialize` agentCapabilities (logging stripped) **byte-identical**; `session/new` `modes` + `configOptions` **sha-identical**.

  > cyril note: do **not** schema-validate `rawInput` on KAS tool_calls — it mirrors arbitrary per-tool arguments and varies run to run.

**Compaction / `summarization_completed` — chased down live (genuine A/B), wire-identical.** The `acp-event-adapter.d.ts` diff *reworked* summarization emission (incl. a deliberate "don't overwrite the real session title" fix), so I triggered it directly: built 4 trivial turns, then `_kiro/session/compact {sessionId}` (→ `{success:true}`). It emits one `session/update`/`session_info_update` with `_meta.kiro.kind:"summarization_completed"`, carrying **both** a structured `summarization:{status,summary:{conversationSummary,truncated}}` *and* flat `kind`/`conversationSummary`/`truncated`. **Same compact run on genuine 0.3.257 vs 0.3.299: payload structure byte-identical, and `summarization_completed` already exists in 0.3.257 (NOT new in 2.9.0).** Neither version carries `sessionTitle` — the adapter's title fix is internal logic, not a wire change. (Probe: `probe-kas-compact-summarization-2.9.0.py`; logs `/tmp/kas-compact-{257,299}.jsonl`.)

**Full `session_info_update` sub-type taxonomy (via `_meta.kiro.kind`, identical both versions — useful for cyril's KAS-2 converter):** `context_usage` (token-% breakdown: contextFiles/tools/kiroResponses/yourPrompts/sessionFiles), `turn_start`, `turn_completion` (per-turn metering — see below), `turn_end`, `user_message_id_assigned`, `focus_update`, `display_error`, `summarization_completed`.

**`turn_completion` metering — NO token counts; the metering unit is backend-passthrough.** Wire schema (`tool-call-emitter`): `promptTurnSummaries:[{usedTools?:string[], unit?:string, unitPlural?:string, usage?:number}]` + `elapsedTime` + `status`. `unit` is a **free string (not an enum)** populated verbatim from the backend model-response metadata (`model-response-metrics.js`: `additional_kwargs.usageSummaryEntry.{usage,unit}` → `meteringUnit`/`meteringUsage`). On this (credit-billed) account it's `unit:"credit"`; a token-metered backend could emit `unit:"token"` through the same field — so the unit/value is a **backend decision, not a binary constant** (binary×backend axis). **Per-turn `inputTokens`/`outputTokens` exist internally but go to telemetry only** (`reportCountMetrics`/`RecapMeteringUsage` in `orchestrate-subagent.js`), **never the ACP wire**. The only token data on the wire is `context_usage` = *cumulative context-window occupancy* by bucket, NOT per-turn I/O. **Cyril cannot get per-turn token counts from the KAS wire** — only credits (`turn_completion`) + context occupancy (`context_usage`); per-turn tokens are off-wire (telemetry + the on-disk session sidecar, see `reference_kiro_session_sidecar_metering`).

**The `/stats` question (user-noticed "gone"):** `/stats` is a **v2-only backend command**. It is present on the v2 ACP surface in **both** 2.8.1 and 2.9.0 (in the 24-command list), and **absent from the KAS/v3 command set in both 0.3.257 and 0.3.299**. KAS exposes a different, smaller invokable set (8, captured identically across versions: `architecture-selection bug-fix code-testing-agent codebase-summary context-gatherer general-task-execution quick-spec rust-best-practices` — bundled roles + this user's custom file-agents). So `/stats` was **not removed in 2.9.0** — it simply never existed on KAS; seeing it "gone" means running `--v3`. (Probes: `experiments/conductor-spike/probe-kas-commands-tools-2.9.0.py`, `probe-kas-orchestrate-wire-2.9.0.py`.)

## Doc-manifest delta — essentially nil

Embedded product-doc index (two manifests: 82 + 118 docs, 134 merged) — **134 → 134, zero added, zero removed**. Only 3 `validated`-date bumps (`features/mid-turn-steering.md`, `settings/default-interrupt-mode.md`, `settings/key-bindings-settings.md`: 2026-06-08 → 2026-06-15). **No doc mentions `lite` or `verbosity`** — confirming lite-mode is below even the doc-manifest's usual pre-announcement-superset radar (it's a flagged rollout). Artifacts: `docs/kiro-docs-index-2.9.0-{82,118,merged}.json`.

## Cyril impact

- **None on the current v2 path.** Stay on the default engine; `use_aws` + all 24 cmds / 15 tools unchanged. Safe to upgrade `kiro-cli` 2.8.1 → 2.9.0.
- **`/lite` (the user's question):** a frontend rendering-mode toggle, telemetry-only, **zero ACP wire impact** — invisible to cyril. Strategic signal only: Kiro is moving toward inline/scrollback rendering and defaulting KAS to it.
- **KAS-track signals (additive, no removals):**
  - The KAS-2 converter's unknown-field tolerance must accept the new `policyDenial` (tool_call), `SafetyGateStatus:'blocked'` (+`toolId`/`blockedProperties`), `handlesFileHooks` (hooks), and `errorCode:'timeout_too_low'` (mcp) — same posture as the 2.8.0 `_kiro/safety/*` and 2.8.1 `_kiro/sessions/changed` additions.
  - `policyDenial` persistence + granular safety blocking are directly useful when cyril renders KAS policy/permission state (KAS-4/KAS-5). A KAS permission UI can show *why* a tool was denied (which rule matched) and survive reload.
  - `handlesFileHooks` is an IDE-vs-CLI distinction: cyril is a headless/CLI client → leave it unset → the agent runs file hooks. No responder needed.
- **ACP schema watch:** the `SessionModeId`/`SessionConfigValueId`/`ExtRequest` → `SessionModeState`/`SessionModelState` reshape in `agent_client_protocol_schema` is the kind of rename cyril's own `agent-client-protocol` dependency will see on the road to 1.0 (`reference_sacp_acp_v1_merge`).
- **Trajectory:** v2 is frozen-stable (three consecutive zero-binary-wire-delta releases: 2.8.0, 2.8.1, 2.9.0). All wire motion is in the fast KAS bundle (0.3.224 → 234 → 257 → **299** across 2.7.1/2.8.0/2.8.1/2.9.0) and now the flag-gated frontend (lite mode). Track the embedded `@kiro/agent` version every release.

## Reproduce

```sh
# fetch + verify (SHA from latest/manifest.json)
curl -sSLo /tmp/k290.tar.zst https://desktop-release.q.us-east-1.amazonaws.com/2.9.0/kirocli-x86_64-linux.tar.zst
echo "ee9ef0a203f7e1d73585c377cb993b506828bd3121d7f037390e06efbc054267  /tmp/k290.tar.zst" | sha256sum -c

# v2 surface identical (same-day), against each archived chat binary
python3 experiments/conductor-spike/probe-v2-surface-2.8.0.py ~/.local/share/kiro-research/binaries/2.8.1/kiro-cli-chat
python3 experiments/conductor-spike/probe-v2-surface-2.8.0.py ~/.local/share/kiro-research/binaries/2.9.0/kiro-cli-chat

# embedded KAS version bump
grep -aoP '"@kiro/agent":"[0-9.]+' ~/.local/share/kiro-research/binaries/2.8.{1,}/kiro-cli-chat ~/.local/share/kiro-research/binaries/2.9.0/kiro-cli-chat

# /lite (frontend, off-wire): the local-command array + rollout gate live in the embedded tui.js
grep -abo '/lite' ~/.local/share/kiro-research/binaries/2.9.0/kiro-cli-chat   # last hit = tui.js
grep -ao 'KIRO_LITE_ROLLOUT_ENABLED' ~/.local/share/kiro-research/binaries/2.9.0/kiro-cli-chat

# KAS covenant diff (after self-extracting 0.3.299 to a throwaway HOME via direct-spawn):
#   diff -rq <0.3.257 covenant>/dist <0.3.299 covenant>/dist   (4 .d.ts differ, all additive)

# KAS session/new identical (direct-spawn free path), both bundles:
KIRO_AGENT_PATH=~/.local/share/kiro-cli/node \
  python3 experiments/conductor-spike/probe-kas-session-new-2.9.0.py <path-to-acp-server.js>

# doc-manifest delta (nil)
python3 experiments/conductor-spike/extract_doc_manifest.py ~/.local/share/kiro-research/binaries/2.9.0/kiro-cli-chat /tmp/k290-docs
```

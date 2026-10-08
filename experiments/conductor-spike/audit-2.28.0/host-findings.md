# Rust-host static lane: kiro-cli 2.26.0 → 2.26.1 → 2.27.0 → 2.27.1 → 2.28.0

Audited 2026-10-08. The lane was static only: no engine was spawned and no auth command ran.
The only binaries executed were `kiro-cli --help-all`, `kiro-cli-chat [sub] --help` (with acp, chat, login, whoami, crew and update skipped), `kiro-cli-term --help` and
`kiro-cli version --changelog`, all run under a temp `HOME` with the archive `bin/` first on `PATH`. Inputs were
`~/.local/share/kiro-research/binaries/<ver>/extracted/kirocli/bin/{kiro-cli,kiro-cli-chat,kiro-cli-term}`.

## 0. Method and tooling (new durable scripts, `experiments/conductor-spike/`)

| script | what it does |
|---|---|
| `static-rollout-2.28.0.py` | Extracts the rollout registry and the FeatureOverride cohort table across an N-build chain, and reports every row delta |
| `static-host-paths-2.28.0.py` | Lists `crates/*/src/**.rs` for **all** crates in all 3 binaries, plus module tokens, `kiro.dev/` leads and env-shaped tokens. zstd payloads are masked |
| `static-host-env-2.28.0.py` | Env census that splits glued env tables (`KIRO_PARENTQ_SET_PARENT…`), so a lowercase prefix may precede a name. Every add/remove is substring-checked against the other build |
| `static-host-serde-2.28.0.py` | Inventories serde `expecting` strings (`struct X with N elements`, tagged enums). A changed element count means a field was added or removed. Glue-resistant |
| `static-host-novel-words-2.28.0.py` | Glue-proof novel-word census. Each word must be absent as a raw substring from the old build, and must not segment into the old build's delimited words. Stage 2 removes LTO re-glue |
| `static-host-vocab-2.28.0.py` | Full identifier-token diff (raw, noisy: about ±600 tokens per patch release, nearly all glue). Kept as the unfiltered evidence base |
| `static-host-settings-2.28.0.py` | Settings-key census (`chat.*`, `memory.*`, `trust.*` …) |
| `static-doc-manifests-2.28.0.py` | Chain diff of the embedded doc manifests `{generated_at,total_docs,documents[]}` |

Payload masking: in every build the large zstd frames are KAS tar (~6.9 MB @ ~6.85 MB), Node v22 ELF, Bun ELF and the TUI bundle (~2.1 MB → 13.6 MB).
Node and Bun frames are byte-size-identical across all five builds, so the runtimes were not bumped. A gzip tar at ~98.1 MB
holds the **embedded docs knowledge index** (`kiro-autodocs-semantic`/`-bm25`, HNSW+BM25, 140 docs). It explained the
"random-word" churn in 2.27.0, which came from index regeneration and not from code.

Trap observed this cycle: `kiro-cli version --changelog=<ver>` is now **remote-fed** (`remote_changelog` rollout,
`https://prod.download.cli.kiro.dev/stable/<ver>/feed.json`, cached into `$HOME/.local/share/kiro-cli/feed-cache.json`).
The embedded feed is an empty placeholder (`0.0.0`). So every archived binary prints the **current** feed (2.28.0 + 2.27.1),
and that is why 2.26.1 and 2.27.0 were "missing". The per-version feeds were fetched directly from
`stable/<ver>/feed.json`; see §7.

## 1. Headline (CONFIRMED)

1. **The v2 ACP wire surface is statically unchanged from 2.26.0 to 2.28.0.** The `_kiro.dev/*` method set is identical once glue is stripped.
   No new `chat-cli-v2/src/agent/acp/commands/*.rs`. TuiCommand args structs are identical (`FallbackArgs`, `ReasoningArgs`, …).
   `acp::schema` notification types are identical (CommandsAvailable, Metadata, ModeChanged, UiModeChanged, UiModeDefaultChanged,
   UiModeSessionStart). ±120-char contexts around `contextUsagePercentage`, `meteringUsage`,
   `availableCommands`, `promptCapabilities`, `agentCapabilities` and `currentModelId` are byte-identical 2.26.0 vs 2.28.0.
   `agent-client-protocol-0.10.4` / `sacp-11.0.0` are unchanged.
2. **v2 model fallback was switched on, not added, in 2.27.1.** The engine side predates the range: the `_kiro.dev/session/update`
   `model_fallback{cause,…}` variant, `FallbackArgs{targetModelId,fallbackModelId}` and the model-options `fallback{eligible,readOnly,configured,
   configuredUnreadable,vendorDefault}` are all present in 2.26.0. In 2.27.1 the engine-side gate strings
   `"Model fallback is not available."` and the `ModelTiepointTag … fallback_enabled` tag disappear, and the rollout row
   `model_fallback` (0 % internal nightly) is **removed**. The changelog says "a refused or over-capacity turn now retries on
   another model by default". Cyril **DROPS** `model_fallback` (convert/kiro.rs logs "unhandled session/update variant").
   This is tracked as cyril-ci3j, but **correct its premise**: the row was removed in **2.27.1**, not 2.28.0. 2.28.0 removed
   `v2_non_interactive`.
3. **`_kiro.dev/agent/not_found` params went from 3 to 4 fields in 2.27.1.** In 2.27.0 `struct AgentNotFoundParams with 3 elements` has fields
   `requestedAgent, fallbackAgent, skipped`; in 2.27.1 it has 4 elements. The new field name is deduplicated with another literal. The
   serializer's inline immediates gain `sessionI…`, so the fourth field is **very likely `sessionId`** (LEAD L3).
   Cyril HANDLES this method only partly: it reads `requestedAgent` and `fallbackAgent` and ignores `skipped` and the new field.
   No rivets issue was found.
4. **Permission-classifier shadow mode (V2 engine) arrives in 2.27.1.** New modules `crates/agent/src/agent/trust_shadow.rs` and
   `crates/chat-cli-v2/src/agent/acp/trust_shadow_host.rs` (**in the v2 ACP host**). There is a new rollout row
   `trust_classifier_shadow` (100 % internal nightly), env `KIRO_TRUST_CLASSIFIER_SHADOW` and `KIRO_TRUST_CLASSIFIER_SHADOW_LOG`, and settings
   `trust.classifier.shadow` (default true) and `trust.classifier.shadowLogPath`. On every approval prompt the engine makes an
   **extra hidden model call** to "permission-classifier" (prompt: "You are the approval classifier… Reply with exactly one word:
   SAFE, UNSAFE, or ABSTAIN"). It records the verdict beside the human decision and never changes the prompt. Telemetry fields:
   `humanDecision, humanLatencyMs, queuePosition, stage2RationaleChars, classifier_session_id_hash`. A `provenance: human`
   check exists ("shadow pair dropped: answer not vouched for as human"). The rollout row says "today only the V2 engine reads it".
   The launcher exports the env, and the ACP server does not consult the registry (2.22.0 § 4b). So under cyril (`kiro-cli acp`) it is **off unless
   KIRO_TRUST_CLASSIFIER_SHADOW is set in cyril's spawn env**. It is N/A for non-internal users. It matters because it adds paid calls if ever on.
5. **2.28.0: `kiro-cli acp` now warns when sandbox.json asks for a sandbox.** The new literal is "Warning: sandbox.json requests a
   sandbox, but `kiro-cli acp` does not apply sandbox isolation on this launch path." (the same exists for `kiro-cli serve`).
   It is emitted from `crates/chat-cli/src/cli/mod.rs` next to `kas_acp_relay.rs`. Cyril's acp path therefore runs unsandboxed
   even when a workspace `sandbox.json` asks for isolation. The local-sandbox family is also moving: `SandboxFileConfig` fields go from
   `{enabled,backend,networkAccess,mcpSandboxing}` to `{enabled,networkAccess,mcpSandboxing}` (**`backend` removed** in 2.28.0),
   new env `KIRO_ENABLE_LOCAL_SANDBOX`, and new modules `cli/windows_sandbox.rs`, `cli/windows_kas_control.rs`,
   `cli/windows_kas_process.rs`, with hidden subcommands `windows-sandbox` and `windows-kas-stop` plus
   "Owned Windows KAS supervision…" and `Local\KiroOwnedKas-` literals. This lines up with cyril-be7e, which is TUI-lane evidence.
6. **2.28.0 adds the Classic → 3.0 nudge** (`crates/chat-cli/src/launch/classic_nudge.rs`, env `KIRO_CLASSIC_NUDGE_RELAUNCHED`):
   "Classic is being deprecated with the Kiro CLI 3.0 release in October." It also offers to auto-upgrade agent configs. This is TUI/launcher only and
   does not apply to `kiro-cli acp`, but it is a **strategic signal: 3.0 (KAS default) ships in October**.

## 2. Per-pair delta tables

### 2.26.0 → 2.26.1 (build 2026-09-30)

| item | kind | evidence | cyril | significance |
|---|---|---|---|---|
| `NODE_USE_SYSTEM_CA` | env (host → KAS node spawn) | new literal in launch path: `failed to launch Bun TUI runtime at NODE_USE_SYSTEM_CA event crates/chat-cli/src/launch.rs` | N/A (cyril spawns node itself and does not set it). See cyril-be7e | changelog "Trusts the OS certificate store … corporate TLS proxies" |
| `session/set_model` literal | v2 ACP method string | absent in 2.26.0. In 2.26.1 it sits glued after `session/set_config_option` in the schema/error region (`…PoisonErrorsession/set_config_optionsession/set_model`). `set_config_option` there is the known -32601 `data` | LEAD L1 (cyril-xdll, cyril-fj6j) | v2 now names set_model explicitly. Its behaviour may have changed |
| `SetSessionModelResponse` | serde shape | `struct SetSessionModelResponse with 1 element` newly materialized | same lead | same |
| stopReason refusal on v2 ACP | behaviour (changelog) | "ACP clients on the V2 engine now get `stopReason: refusal` for content-filtered responses, not `end_turn`" | **HANDLED** (`convert/mod.rs:134` maps `acp::StopReason::Refusal`; bridge.rs maps it to Failed) | v2 refusals now end turns as Refusal |
| rollout / docs / settings / source paths | none | no row or path delta | — | — |

### 2.26.1 → 2.27.0 (build 2026-10-01)

| item | kind | evidence | cyril | significance |
|---|---|---|---|---|
| `chat-cli/src/owner_only_sweep.rs` | module | "owner-only sweep refused a tree that does not resolve inside the Kiro home" | N/A | changelog: `~/.kiro` and transcripts chmod owner-only, retroactively |
| `chat-cli-v2/src/auth/credential_generation.rs`, `chat-cli-v2/src/telemetry/host_config.rs` | modules | token structs +1 field: `BuilderIdToken 7→8`, `ExternalIdpToken 7→8`, `SocialToken 5→6` = `telemetry_generation`. Hidden subcommand `get-telemetry-credentials --expected-generation` ("Lineage (64 lowercase hex) the exporter pinned at startup"), `api-key-lineage`, "KUTS delivery queue" | N/A (no ACP) | telemetry-exporter credential lineage. Token file schema grew a field (relevant only if cyril ever reads the token store) |
| `SessionTool::ValidateAgents` / `UnknownAgents{names}` | v2 internal tool enum | new serde variant plus new internal enum arm `UnknownAgents names` | N/A (agent-side tool) | changelog: unknown subagent role now errors visibly |
| `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED`, `KIRO_ROLLOUT_FEATURES` | env | new host literals. `background_execution` row text now says the TUI sends `backgroundExecution:{enabled:true}` on session/new, and "KAS reads no KIRO_ENABLED_FEATURES; a KAS reached without the CLI is enabled by its own operator override KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED=true" | NOT-SENT (cyril sends no backgroundExecution). Overlaps cyril-7p7n | how a third-party client (cyril) turns KAS background execution on |
| settings `memory.mode`, `memory.reflection`, `chat.keybindings.moveToBackground` | settings | absent in 2.26.1, present in 2.27.0 | N/A (KAS/TUI) | /memories access mode + automatic updates. Embedded doc `slash-commands/memories.md` rewritten (see §5) |
| rollout `background_execution` | row text changed | full before/after in §3 | — | still 0 %. Describes the session/new setting contract |
| embedded docs index regenerated | docs | 140-doc manifest `generated_at` 2026-09-18 → 2026-09-28. Only `memories.md` changed | — | — |

### 2.27.0 → 2.27.1 (build 2026-10-02)

| item | kind | evidence | cyril | significance |
|---|---|---|---|---|
| model fallback ON by default | gate removal | rollout row `model_fallback` REMOVED. Engine strings `Model fallback is not available.` and `ModelTiepointTag…fallback_enabled` removed. Telemetry enum gains `modelFallback`; `kiro_cli_model_fallback_total{fallback_cause=refusal|unavailable, moved|stayed, interactive_cli|noninteractive_cli|external_acp}`. Engine prose: "moving the rest of this turn to another model", "a refused turn was answered elsewhere, so that model becomes the conversation's primary" | **DROPPED** `session/update model_fallback` (cyril-ci3j). **NOT-SENT** `commands/execute fallback` (cyril-lnxg) | **HIGH**: turn and billing can silently run on another model under cyril. The `external_acp` metric label shows the ACP path is in scope |
| `_kiro.dev/agent/not_found` 3→4 fields | v2 ACP ext notification shape | `struct AgentNotFoundParams with 3 elements` → `with 4 elements`. Inline immediates add `sessionI…` | PARTIAL (requested/fallback only) | LEAD L3. Pairs with changelog "non-interactive exits 4 when named agent unavailable" |
| `trust_shadow.rs`, `acp/trust_shadow_host.rs` | modules (v2 ACP host) | see §1.4 | N/A unless env set | hidden extra model call per approval (internal nightly) |
| env `KIRO_TRUST_CLASSIFIER_SHADOW`, `…_SHADOW_LOG`, `KIRO_HEADLESS_WORKFLOW_TIMEOUT_SECS`, `KIRO_TELEMETRY_IS_ENTERPRISE`, `KIRO_TEST_KAS_TURN_LOG_PATH`, `KIRO_TEST_DISABLE_SUBAGENT_ORCHESTRATION` (gone again in 2.28.0) | env | split env table | N/A | headless `--no-interactive` workflow drain bound (6 h default) |
| settings `trust.classifier.shadow`, `trust.classifier.shadowLogPath` | settings | "Run the permission classifier in shadow mode beside approval prompts on internal nightly builds (boolean, default true)" | N/A | — |
| rollout: `+kvim` (100 % internal), `+trust_classifier_shadow` (100 % internal nightly), `-model_fallback`, `v3_prompt 25→35 %` | rollout | §3 | — | V3 ease-in prompt reaches 35 % of internal |
| workflow monitor statuses | TUI-side literals in host | `step started/step completed/step paused/looping/paused/steps queued/aborted…`, "the KAS connection closed while workflows were active; workflow activity did not settle within" | N/A | headless workflow attach (changelog) |

### 2.27.1 → 2.28.0 (build 2026-10-05)

| item | kind | evidence | cyril | significance |
|---|---|---|---|---|
| sandbox.json warning on `kiro-cli acp` / `serve` | launcher behaviour | §1.5 | N/A (cyril KAS Free spawns node directly; cyril v2 via `kiro-cli acp` gets the stderr warning) | stderr noise on cyril's v2 spawn if a workspace has sandbox.json. No isolation on the ACP path |
| `SandboxFileConfig` −`backend` | config schema | `enabledbackendnetworkAccessmcpSandboxing … with 4` → `enablednetworkAccessmcpSandboxing … with 3` | N/A | sandbox backend no longer user-selectable |
| Windows KAS supervisor + sandbox | modules / hidden subcommands | `cli/windows_{kas_control,kas_process,sandbox}.rs`; `windows-sandbox`, `windows-kas-stop`; "Owned Windows KAS supervision is only available on Windows"; `Local\KiroOwnedKas-` | relevant to native-Windows cyril (cyril-be7e) | Windows-only code (cfg-stubbed in the Linux build) |
| `launch/classic_nudge.rs`, `KIRO_CLASSIC_NUDGE_RELAUNCHED` | module/env | §1.6 | N/A | 3.0 default in October |
| `KIRO_ENABLE_LOCAL_SANDBOX`, `KIRO_TEST_DROP_FIRST_END_TURN` | env | the latter sits in `chat-cli-v2/src/os/mod.rs` env list beside `KIRO_TEST_MODE`/`KIRO_TEST_LIVE_API` | — | v2 has a test hook that drops the first `end_turn`, which implies v2 now recovers from a missing end_turn. Pairs with changelog "Conversations no longer stop responding after a sub-agent finishes in V2 TUI sessions". LEAD L5 |
| setting `chat.fullscreenCopyOnSelect` | setting | — | N/A | changelog |
| rollout: `memory` channel nightly→any, `memory_controls` 0→100 % + channel nightly→any, `-v2_non_interactive` | rollout | §3 | — | KAS memory now on for **all internal users on every channel** |
| telemetry outcome enum | literals | `switch/switch_failed/snooze/dismiss/suppressed/ease_in/classic/moved/stayed/acp_injected/resumed/hard_cancelled/fault/interactive_cli/noninteractive_cli/external_acp/runtime_setup`; `RecoveredMovedToAnotherModel` | — | — |

Launcher `kiro-cli` and `kiro-cli-term`: **no source-path, settings or help change** in any pair. Their vocab churn is all
re-glue (novel-word stage 2 leaves only fragments). `kiro-cli --help-all`, `kiro-cli-chat --help` (+23 subcommand pages) and
`kiro-cli-term --help` are byte-identical across all five builds.

## 3. Rollout registry (21 → 21 → 21 → 22 → 21 rows). FeatureOverride cohort unchanged (`v3_prompt` treatment ×135, digest `3d29b950321fdd59`)

| release | row | change |
|---|---|---|
| 2.26.1 | — | none |
| 2.27.0 | `background_execution` | description only. New text: "…whenever it is on, the TUI sends backgroundExecution: { enabled: true } on session/new … KAS reads no KIRO_ENABLED_FEATURES; a KAS reached without the CLI is enabled by its own operator override, KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED=true … Dark-shipped at 0% until the telemetry layer ramps it for internal users on the nightly channel" (was "…while the rendering layer is unbuilt") |
| 2.27.1 | `kvim` | **ADDED** 100 % internal (all channels): "/kvim: Neovim in a pane above the fullscreen chat…" |
| 2.27.1 | `trust_classifier_shadow` | **ADDED** 100 % internal nightly (§1.4) |
| 2.27.1 | `model_fallback` | **REMOVED** (was 0 % internal nightly, "Moving a turn to another model when the one it is using refuses it or is over capacity") |
| 2.27.1 | `v3_prompt` | treatment 25 → **35** (internal) |
| 2.28.0 | `memory` | channel nightly → **any**. Text: "Persistent agent memory and the /memories command. Ramped to all internal users on every channel; KAS's memory experiment still decides per-user eligibility." (treatment already 100, internal) |
| 2.28.0 | `memory_controls` | treatment **0 → 100**, channel nightly → **any**. Text: "KAS-only /memories controls: saved memory access and automatic-update defaults, plus record browsing" |
| 2.28.0 | `v2_non_interactive` | **REMOVED** (was 100 %, "Default non-interactive and piped-stdin to V2 engine instead of V1"). Rollout gate retired, presumably hard-defaulted |

Full 2.28.0 table (treatment/segment/channel): background_execution 0/internal/nightly · c2s 100/internal/nightly ·
cloud_config 100/all · infra_safety 100/internal · kvim 100/internal · lite 100/internal · local_sandbox 100/internal/nightly ·
memory 100/internal · memory_controls 100/internal · remote_changelog 100/–/stable · remote_sandbox 100/all ·
session_dashboard 100/all · tangent 100/all · test 100 · test_internal_only 100/internal · test_nightly_only 100/internal/nightly ·
trust_classifier_shadow 100/internal/nightly · tui 50/internal · v3_prompt 35/internal · voice 100/all · workflows 100/all.
Raw JSON: `out/rollout/rollout.json`, `out/rollout/rollout-delta.json`.

## 4. Env-var delta (host, payload-masked; `out/env/host-env-delta.json`)

| release | added | removed |
|---|---|---|
| 2.26.1 | `NODE_USE_SYSTEM_CA` | — |
| 2.27.0 | `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED`, `KIRO_ROLLOUT_FEATURES` | — |
| 2.27.1 | `KIRO_TRUST_CLASSIFIER_SHADOW`, `KIRO_TRUST_CLASSIFIER_SHADOW_LOG`, `KIRO_HEADLESS_WORKFLOW_TIMEOUT_SECS`, `KIRO_TELEMETRY_IS_ENTERPRISE`, `KIRO_TEST_KAS_TURN_LOG_PATH`, `KIRO_TEST_DISABLE_SUBAGENT_ORCHESTRATION` | — |
| 2.28.0 | `KIRO_CLASSIC_NUDGE_RELAUNCHED`, `KIRO_ENABLE_LOCAL_SANDBOX`, `KIRO_TEST_DROP_FIRST_END_TURN` | `KIRO_TEST_DISABLE_SUBAGENT_ORCHESTRATION` |

Each add was verified by raw substring count (0 in the old build). The remaining census lines are glue re-splits (`…C`, `…R`, `PPPP…`).
`KIRO_OWNED_KAS_PARENT/CONTROL` are **not** in the Linux host binary. They appear only in the TUI evidence of cyril-be7e or in cfg(windows) code.
`KIRO_CUSTOM_USER_AGENT` was already present at 2.26.0.

## 5. Embedded doc manifests and doc corpus

Two manifests in every build: 103 docs (`generated_at 2026-09-02`, unchanged) and 140 docs (`2026-09-18` → **`2026-09-28` in 2.27.0**).
Delta across the whole chain: **0 added, 0 removed, 1 changed**. That one is `slash-commands/memories.md` (2.27.0). The description goes from
"Browse and preview memories…" to "Browse stored memories and change memory access and automatic updates", and the keywords gain
`settings, reflection, access`. The body now lives in the gzip docs index (`kiro-autodocs-semantic`). It describes a
config/list menu, access modes `read & write | read only | off`, "Automatic memory updates … including after a session
ends", and the `memory_controls` rollout gate. Public status: kiro.dev/docs/memory/ already documents `/memories` →
config → access level, so this is **public**, not unannounced. No new `category:feature` node, so **this cycle has no
unannounced-feature signal from the manifest**. The 2.27.0 and 2.28.0 doc corpora are byte-identical.
`/model fallback` and `/tangent merge` are already on the public kiro.dev/docs/reference/slash-commands page
(fallback "in V1 or V2").

## 6. Cyril coverage summary (v2 wire items touched by this range)

| wire item | status | issue |
|---|---|---|
| `_kiro.dev/session/update` `model_fallback` (live by default since 2.27.1) | DROPPED (debug "unhandled") | cyril-ci3j (fix the 2.28.0 → 2.27.1 attribution) |
| `_kiro.dev/commands/execute` `fallback` + options `fallback{}` | NOT-SENT / not parsed | cyril-lnxg (engine side pre-dates 2.26.0; 2.27.1 = enablement) |
| `_kiro.dev/agent/not_found` `skipped` + 4th field (2.27.1) | PARTIAL | **none found**. Candidate new issue |
| `session/prompt` stopReason `refusal` (v2, 2.26.1) | HANDLED | cyril-h8zb/pz51 family |
| `session/set_model` (literal appears 2.26.1) | unknown behaviour change | cyril-xdll / cyril-fj6j |
| `retry_warning`, `stream_stall_notice` session/update | DROPPED (standing, not new) | cyril-svi2 |
| `_kiro.dev/goal/status`, `_kiro.dev/mcp/governance_disabled`, `_kiro.dev/webTools/governance_disabled`, `_kiro.dev/settings/set`, `_kiro.dev/telemetry/*` | not matched in convert/kiro.rs (standing, all exist at 2.26.0). Not a delta of this range | — (coverage-lane follow-up) |
| `initialize` / `session/new` / `_kiro.dev/metadata` / `commands/available` | no static change | — |

## 7. Changelogs (per-version feeds `https://prod.download.cli.kiro.dev/stable/<ver>/feed.json`)

### 2.28.0 (2026-10-05)
- added: `/settings` Display > Full Screen lets you turn off Copy on select for terminal or tmux copying
- added: Classic sessions offer to switch you to 3.0 and upgrade your agent configs
- added: [V3] Hook matchers accept tool tags such as `shell`, `read`, `@builtin` and `@mcp`
- changed: `/model` and `/effort` pickers open mid-turn, with selections applied after the turn ends
- fixed: Conversations no longer stop responding after a sub-agent finishes in V2 TUI sessions
- fixed: Expanded activity trays no longer limit task lists to six rows, with a screen-sized limit in fullscreen
- fixed: Windows V2 stdio MCP servers preserve `|`, `&`, `%`, and spaces in arguments when resolved to `.exe`, `.com`, `.bat`, or `.cmd` without a configured `PATHEXT`
- fixed: Voice recording hints point to `kiro-cli settings voice.*` instead of the nonexistent `/settings voice`
- fixed: Returning from the agent monitor or session dashboard no longer misplaces the chat view on some terminals
- fixed: [V3] Interrupted tool calls report accurate outcomes, live and after reload
- fixed: [V3] Reduced memory growth during very long agent sessions

### 2.27.1 (2026-10-02)
- added: [V3] `/tangent merge [dest]` to fold a tangent's findings into its parent or a named session
- added: [V3] Non-interactive runs support hooks, knowledge, code intelligence
- fixed: [V3] `/chat save -f <path>` now overwrites an existing file like `--force` instead of failing
- fixed: [V3] `/chat save` now works when Kiro is launched from a different directory than the one the session was started in
- changed: Non-interactive runs now exit 4 when the named agent (`--agent`, or the v3 `chat.defaultAgent`) is unavailable, instead of answering on the default agent
- fixed: [V3] Reopening a tangent after `/tangent merge` shows its summarization line, not the full prompt
- fixed: Remote MCP servers that answer the path-based OAuth discovery URL with their root issuer (such as self-managed GitLab) no longer fail sign-in with an issuer mismatch on the V2 engine
- fixed: [V2] Batched `Directory` operations in the `read` tool no longer run into each other
- changed: Non-interactive runs exit 1, instead of warning and answering, when `--agent` fails to apply for a reason other than the agent being unavailable
- fixed: [V3] `/upgrade-agent diagnostics` no longer blanks the screen and freezes input a few seconds after opening
- fixed: [V3] New and resumed sessions, once the model list is known, tell you when their model is unavailable or was changed and point to `/model`; an unavailable saved default falls back to the default model
- fixed: [V3] Resuming a session with many workflow runs no longer waits to load every finished step's credits
- added: [V3] Workflows in `--no-interactive` runs now stay attached through completion wakes and late `--trust-all-tools` approvals; set `KIRO_HEADLESS_WORKFLOW_TIMEOUT_SECS` to lower the six-hour drain bound
- added: [V3] Workflow progress and complete wake-turn usage accounting in `--no-interactive` runs
- fixed: [V3] Make `--no-interactive` workflows honor feature settings, isolate child output, and report incomplete outcomes
- added: Model fallback: a refused or over-capacity turn now retries on another model by default; `/model fallback` picks the target
- fixed: Pasting text that contains terminal escape sequences, such as colored output or hyperlinks, no longer leaves sequence debris in the prompt
- fixed: Failed shell commands show both stdout and stderr alongside their exit status, and shell output no longer ends with a blank row

### 2.27.0 (2026-10-01)
- fixed: A failed shell command shows its output and the failure together under one `output:` label
- fixed: Long text in an indented row keeps every wrapped line; the row below no longer paints over it
- fixed: Transcript spacing no longer shifts up by a row when an entry is committed to scrollback
- fixed: Pass `--effort` to the engine in non-interactive runs
- fixed: Subagent spawned with an unknown agent role now returns a visible error instead of silently using the default agent's tools
- fixed: Selective mode in web_fetch no longer returns unbounded output that caused every subsequent turn to fail with "Improperly formed request" on large documents
- fixed: Prevent a CLI shutdown stall during slow telemetry exports handled by a native flush worker
- fixed: [V3] `/context` no longer lists global steering files twice when run from the home directory
- fixed: [V3] Steering files now expand `#[[file:...]]` and `#[[folder:...]]` references
- security: `~/.kiro` and the session transcripts, logs, prompt history, settings, agents and prompts every engine keeps in it are now readable only by you, including files from earlier releases
- fixed: [V3] Workflow steps now retry through network outages for up to 24 hours instead of pausing
- changed: [V3] Output style now lives under `/settings display` instead of the top-level `/settings` menu
- added: [V3] Herdr can restore existing cloud conversations after a server restart
- added: [V3] Workflows sub-agent tool toggle in `/settings` features to limit chat delegation to workflows
- fixed: Agent configs saved by renaming a file into an agents directory now hot-reload without a restart
- security: MCP registry mode now always launches a registry server's own command or URL, even when a local config reuses its name
- fixed: [V3] `--resume` now picks the most recent session matching the launch environment, local or cloud
- fixed: `agent list` now includes agents that use the v3 config format instead of hiding them
- fixed: Write cards in the full TUI show only a diff's changed lines with context, not the whole file
- fixed: Copy works in `/fullscreen`: dragging over a message copies what was said, not where it wrapped
- changed: Write cards in the full TUI cap a diff at 40 rows; `ctrl+o` or `chat.autoExpandToolOutput` shows the rest while the card is live
- added: [V3] Saved prompt files and MCP server prompts now appear as slash commands with argument hints
- changed: [V3] Sessions start up to ~0.5s faster with large `permissions.yaml` files
- fixed: [V3] A workflow step answered after the agent restarts now reports its progress and completion
- fixed: [V3] Recovering from a stalled model response keeps completed tool results instead of rerunning them
- fixed: [V3] The context-gatherer subagent no longer finishes without returning its findings
- removed: [V3] The `todo_list` tool that `chat.enableTodoList` turned on
- fixed: [V3] Trust-all no longer repeats tool approval requests when the agent engine omits a persistable capability

### 2.26.1 (2026-09-30)
- added: [V3] Herdr can restore existing local conversations after a server restart
- fixed: Enter starts authentication for the selected OAuth server in `/mcp`
- fixed: ACP clients on the V2 engine now get `stopReason: refusal` for content-filtered responses, not `end_turn`
- fixed: Sessions no longer become unrecoverable when a preToolUse hook blocks one of multiple requested tools
- fixed: `--model` is honored in `--no-interactive` runs on the V2 engine instead of the default model
- fixed: [V3] Trusts the OS certificate store, so corporate TLS-inspecting proxies no longer cause "No model available"
- fixed: TUI reports invalid or locally missing `--resume-id` values instead of starting a new conversation

("Herdr": zero case-insensitive hits in the host binary. It lives in KAS/TUI and belongs to another lane.)

## 8. LEADS (unconfirmed) and the live probe that would settle each

| id | lead | probe |
|---|---|---|
| L1 | v2 `session/set_model` changed behaviour in 2.26.1 (the literal now sits beside the -32601 `set_config_option` data and `SetSessionModelResponse` materialized). It may now be rejected as -32601 or validated | Re-run `experiments/conductor-spike/probe-v2-set-model-2.20.1.py` against archived 2.26.0 and 2.28.0 v2 (prepend archive `bin/` to PATH and record `/proc/<child>/exe`). Send a served id + a bogus id and compare the result codes. Zero turns needed |
| L2 | v2 `model_fallback` session/update fires on the `kiro-cli acp` path (metric label `external_acp` says yes). Exact keys `{from,to,cause,promoted}` come from cyril-ci3j / TUI evidence, not from the host serde shape | Live v2 turn on a model the account cannot serve, or one with refusal. Needs a reproducible refusal/over-capacity trigger. Alternatively, `/model fallback` on a self-picked unusable model id, then one prompt, then capture `_kiro.dev/session/update` frames |
| L3 | `AgentNotFoundParams` 4th field = `sessionId` (from inline immediates) | v2 `session/new` with `_meta` agent / `--agent nonexistent` on `kiro-cli acp` 2.28.0. Capture `_kiro.dev/agent/not_found` params. Zero turns |
| L4 | `KIRO_TRUST_CLASSIFIER_SHADOW` honoured by `kiro-cli acp` (trust_shadow_host is in the acp module) | v2 acp with `KIRO_TRUST_CLASSIFIER_SHADOW=1` + `KIRO_TRUST_CLASSIFIER_SHADOW_LOG=/abs/tmp.jsonl`, one turn that triggers a shell approval. Check the JSONL and AWS prompt log for a second model call. Costs one turn + one classifier call |
| L5 | v2 recovers from a dropped first `end_turn` (`KIRO_TEST_DROP_FIRST_END_TURN`) | v2 acp with that env + one short prompt. Observe whether the `session/prompt` response still arrives and the stopReason. Relevant to cyril's turn-stall chip |
| L6 | sandbox.json warning goes to stderr (not the ACP stream) on `kiro-cli acp` | `kiro-cli acp` (2.28.0) in a temp workspace with `.kiro/sandbox.json {"enabled":true}`, initialize only. Check that stderr has the warning and stdout stays clean JSON-RPC. Zero turns |

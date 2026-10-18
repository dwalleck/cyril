# Host lane — kiro-cli 2.31.1 → 2.32.0 (static)

Lane: **host** (Rust launcher `kiro-cli`, host binary `kiro-cli-chat`, term `kiro-cli-term`). Audited 2026-10-18.
Static only. Executions: `--help` on the **archived** binaries under `env -i HOME=<tmp> XDG_DATA_HOME=<tmp>/xdg
PATH=<archive bin>:/usr/bin:/bin` (`raw/help.sh`). Nothing authenticated; installed launcher never run.
Inputs: `~/.local/share/kiro-research/binaries/{2.31.1,2.32.0}/extracted/kirocli/bin/*` + `BUILD-INFO`.
String evidence = zstd-frame-masked strings (`raw/mkstrings.py` → `tmp/strings/<ver>/<bin>.txt`; KAS tar, node ELF,
bun ELF and TUI frames zeroed, so KAS/TUI/Node/Bun text never counts). Counts are `[2.31.1, 2.32.0]` substring
counts in `kiro-cli-chat` unless a binary is named (`raw/cnt.py`).

## Summary
1. **Size change explained.** chat 216,000,680 → 207,496,744 B (−8,503,936): KAS tar frame −6,982,045 (sharp/@huggingface unbundled, kas lane), TUI frame +8,921, Node/Bun frames byte-size-identical; the remaining −1.53 MB is real Rust code loss — `.text` −887,200, `.rela.dyn` −350,256, `.data.rel.ro` −249,536, non-frame `.rodata` −26,940. No first-party source file and no third-party crate was removed (`gemm-0.19.0` panic path gone, rustls .45→.46); srcloc shows −1…−80-line shifts across V1 `chat-cli` and `code-agent-sdk` ⇒ deletion spread over many files [INFERENCE]. Launcher/term: rebuilds (`.text` +5,184 / +1,920, no new user-facing literal).
2. **KAS-path headline: the host's non-interactive V3 client now runs cloud sessions itself.** `chat --cloud` help: "Forwarded to the TUI" → "interactively or with --no-interactive. Sends _meta.kiro.executionTarget on session/new". The host now parses KAS `initialize` `executionTargets` [0,1] and gates on it ("--cloud is not available: the agent did not advertise cloud sandbox sessions for this account or build" [0,1]); `--repo` is gated on `sourceProviders` [0,2] + a new host call to `_kiro/sourceProviders/list` [0,1]. cyril reads neither capability and never sends `executionTarget` (open cyril-tikf).
3. **Non-interactive UX (host `launch.rs`, +468 lines after :3748):** plain-text "Error: the model refused to continue the turn" / "Error: the turn was cancelled before it finished" [0,1] each; cloud disconnect/resume messages; `--resume-id` of a cloud session; closed-stdout nonzero exit is logic-only (no literal).
4. **Rollout registry:** 23 rows both; ONLY change `v3_prompt.internal_treatment_percent` 50 → 75 (external stays 10%, segment all). Overrides table unchanged. No new row for cloud, OSC 7501 or memory.
5. **Env:** one genuine new name, `KIRO_STANDALONE_CUTOVER` [0,1] (chat env table, next to `KIRO_ROLLOUT_FORCE_INTERNAL`; autocomplete standalone-channel cutover, unannounced). `KIRO_NO_PROGRAM_STATUS` / OSC 7501 = 0 in all three host binaries → TUI (tui lane KIRO_ENV set 94→95).
6. **Settings keys 120 → 123:** +`chat.disabledModelFallbacks` (v2 per-model fallback off switch), +`chat.modelPicker.sortBy` ('last-used'|'cost'|'company'), +`chat.modelPicker.lastUsed` (model id → epoch ms). Doc manifests byte-equivalent (no trap-12 delta).
7. **v2 (legacy, ranked low):** new source `chat-cli-v2/src/api_client/fallback_choice.rs`; TuiCommand `FallbackArgs` 2 → 3 fields (third = `disabled`, the "None" fix); subagent spawn record +`inherited_model_fallback`; crew `DiscardUnstartedSpawn`; new inbound telemetry method `_kiro.dev/telemetry/modeChanged`. Every other `_kiro.dev/*` literal count-identical.
8. **Unchanged:** `acp --help` byte-identical (default engine `v2`, `--auth-method cli|client`, default `cli`); KAS token path (`kas_token.rs` no shift), hidden helpers (`get-kas-token` = wire surface of `_kiro/auth/getAccessToken`, `export-session`, `import-session`, `windows-sandbox`, `get-telemetry-credentials`) byte-identical; Classic deprecation copy count-identical; BUILD_HASH d6fb1e94 shared with 2.31.1 while all three binaries differ (trap 20).

## Deltas

Legend: STATIC = binary evidence only (trap 4). cyril verdicts grepped this session against `crates/*/src`.

| item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| Host non-interactive V3 client sends `_meta.kiro.executionTarget` on `session/new` for `--cloud` (help text; `_meta.kiro.executionTarget` [1,1], `executionTargets` [0,1] in `chat_cli::launch`) | 2.32.0 | rollout `remote_sandbox` 100% all (unchanged) + KAS must advertise a cloud target | add (host becomes a cloud client) | STATIC | NOT-SENT — cyril builds no `executionTarget` (0 hits in `crates/*/src`) | NOTE cyril-tikf |
| `--cloud` gate on KAS `initialize` `agentCapabilities._meta.kiro.executionTargets`: "--cloud is not available: the agent did not advertise cloud sandbox sessions for this account or build" [0,1] | 2.32.0 | per account/build (standalone KAS advertises `["local"]`) | add | STATIC | DROPPED — cyril only tests `_meta.kiro` is an object `crates/cyril-core/src/protocol/fingerprint.rs:30-36`; `executionTargets` unread | NOTE cyril-tikf |
| `--repo` gate: `sourceProviders` [0,2] + host calls `_kiro/sourceProviders/list` (fragment `_kiro/sourceProv…` [0,1], `kiro/sourceProviders/list` [0,1]); "Attaching a repository (--repo) requires a connected source provider. Connect one at …, then re-run." [0,1] | 2.32.0 | KAS `sourceProviders:true` + method in `extensionMethods` (same rule as TUI) | add | STATIC | NOT-SENT (no `_kiro/sourceProviders/*` call in cyril; capability unread) | NOTE cyril-tikf |
| Cloud session lifecycle text: "[cloud] Disconnected. The cloud session keeps running in its sandbox; continue it with `kiro-cli chat --resume-id …" [0,1]; "…its id could not be displayed, so find it with `kiro-cli chat --list-sessions`." [0,1]; "the run was ended by a signal; the cloud session keeps running in its sandbox" [0,1] | 2.32.0 | `--cloud` | add | STATIC | N/A (host CLI output) | no |
| Cloud flag validation: "--cloud/--repo start a new cloud session and cannot be combined with --resume/--resume-id; to continue a cloud session pass only --resume-id <id>" [0,1]; "--sandbox applies to local sessions and cannot be combined with --cloud" [0,1]; "Warning: sandbox.json requests a local sandbox, which does not apply to a cloud session…" [0,1]; "[warn] --trust-tools has no effect on cloud sessions: the cloud sandbox applies its own tool policy, which runs tools without asking for approval" [0,1] | 2.32.0 | `--cloud` | add | STATIC | N/A | no |
| `--resume-id` cloud resume: "…was not found in any local session store and would be loaded from the cloud store, which --sandbox cannot isolate" [0,1] | 2.32.0 | non-interactive V3 | fix | STATIC | N/A (cyril resumes via `session/load`, no cloud store) | no |
| Non-interactive plain-text outcomes: "Error: the model refused to continue the turn" [0,1], "Error: the turn was cancelled before it finished" [0,1] | 2.32.0 | text output mode | add | STATIC | N/A (host CLI) | no |
| Non-interactive closed-stdout nonzero exit | 2.32.0 | — | fix | STATIC (logic only; no literal: "stdout closed"/"stdout is closed" 0/0) | N/A | no |
| Host V3 `launch.rs` region grows +468 lines (tracing tail :4828/:4839 → :5296/:5307; 27 = 27 call sites) | 2.32.0 | — | change | STATIC | N/A | no |
| rollout `v3_prompt.internal_treatment_percent` 50 → 75 | 2.32.0 | 10% external unchanged | change | STATIC | NOT-SENT (cyril spawns `kiro-cli acp`/`node acp-server.js`, not `chat`) | NOTE cyril-a7nk |
| env `KIRO_STANDALONE_CUTOVER` [0,1] (launcher 0/0, term 0/0) | 2.32.0 | unset | add | STATIC | NOT-SENT (distribution plumbing) | no |
| settings `chat.disabledModelFallbacks` [0,4] "Models whose fallback the user switched off, per profile (object of profile ARN …" | 2.32.0 | unset | add | STATIC | NOT-SENT — not in `BOOL_MAP` `crates/cyril-core/src/protocol/kas/settings.rs:78-88`; v2-only | no |
| settings `chat.modelPicker.sortBy` [0,2] "'last-used', 'cost' or 'company'", `chat.modelPicker.lastUsed` [0,2] "model id to epoch ms" | 2.32.0 | unset | add | STATIC | NOT-SENT (TUI picker display state; no cyril reader — 0 hits for `modelPicker` in `crates/*/src`) | no |
| v2 new source `crates/chat-cli-v2/src/api_client/fallback_choice.rs` [0,4] ("ignoring chat.disabledModelFallbacks", "leaving chat.disabledModelFallbacks as it is") | 2.32.0 | default | add | STATIC | N/A | no |
| v2 TuiCommand `struct FallbackArgs with 2 elements` → `with 3 elements` (`targetModelId`, `fallbackModelId`, + `disabled` per TUI 2.32.0 `{targetModelId:e,disabled:!0}`); "Switched off the fallback for " [0,1], "Cannot switch off the fallback for " [0,2], "A fallback cannot name a model and be switched off at once." [0,1] (old "A model cannot be its own fallback." [2,0]) | 2.32.0 | `/model fallback` | change | STATIC | NOT-SENT — cyril never sends `fallback` via `kiro.dev/commands/execute` (`crates/cyril-core/src/protocol/domain_mediator/commands/extensions.rs:30`, no fallback args anywhere) | no |
| v2 session-manager commands `SetUserFallbackTarget` → `SetUserFallbackTarget{choice}`, +`GetModelFallbackStrategy` [0,3] | 2.32.0 | — | change | STATIC | N/A (internal) | no |
| v2 subagent spawn record +`inherited_model_fallback` [0,1] (beside `skip_history_replay`, `initial_agent_name`, `subagent_info`) | 2.32.0 | default | add | STATIC | `model_fallback` session/update still DROPPED via the unhandled-variant arm `crates/cyril-core/src/protocol/convert/kiro.rs:1057-1063` | NOTE cyril-ci3j |
| v2 crew `DiscardUnstartedSpawn` [0,1] (after `FailGroup{stage_name,error}`), "Cannot spawn under session " [0,3]; `session_manager.rs` sites 61 → 63 | 2.32.0 | — | fix | STATIC | N/A (cyril observes `kiro.dev/subagent/list_update` only) | no |
| v2 inbound method `_kiro.dev/telemetry/modeChanged` [0,1] (const table `…_kiro.dev/mcp/startup_status_kiro.dev/telemetry/modeChanged_kiro.dev/session/terminate…`) | 2.32.0 | TUI→v2 telemetry | add | STATIC | NOT-SENT (cyril sends no `_kiro.dev/telemetry/*`) | no |
| v2 session import: "File looks like a zip archive but cannot be read. It was probably changed after it was saved. Save the session again." [0,1] replaces "File has zip magic bytes but is not a valid zip archive" [1,0] (`chat-cli-v2/src/agent/acp/acp_agent.rs` ~:5674-5698, used by the host `import-session` helper) | 2.32.0 | `/chat load` | change | STATIC | N/A | no |
| new tracing site `crates/chat-cli/src/rollout.rs:276` [0,1] (rollout code previously had no tracing path) | 2.32.0 | — | add | STATIC | N/A | no |
| third-party: −`gemm-0.19.0/src/gemm.rs` path [1,0] (gemm-common/f32/f16 kept), rustls 0.23.45 → 0.23.46, −socket2 0.6.3 path | 2.32.0 | — | change | STATIC | N/A | no |
| `acp --help`, `_ --help`, launcher `--help-all`, `login`/`user`/`settings`/`agent`/`mcp` help | 2.32.0 | — | none (cmp identical) | STATIC | N/A | no |
| Classic deprecation ("Classic is being deprecated" 3/3, `classic_nudge` 9/9, `v3_ease_in` 9/9, "Switch to V3" 3/3, "Kiro CLI 3.0" 2/2), `--agent-engine` default `v2` | 2.32.0 | — | none | STATIC | cyril default engine unchanged `crates/cyril/src/main.rs:40` | NOTE cyril-a7nk |
| KAS auth path (`kas_token.rs` no line shift; `get-kas-token` helper text identical; `--auth-method` unchanged) | 2.32.0 | — | none | STATIC | N/A | no |
| doc manifests (2) | 2.32.0 | — | none | STATIC | N/A | no |

## Rollout registry and feature flags

**Registry** (`static-rollout-2.32.0.py` → `raw/rollout/`, `raw/rollout.log`): 23 rows in both, registry sha256 `355ba99c…` → `5ba2a13d…` (2 copies each). Field diff of every row (description included):

| row | 2.31.1 | 2.32.0 |
|---|---|---|
| `v3_prompt` | treatment 10, internal_treatment 50, segment all | treatment 10, **internal_treatment 75**, segment all |
| all other 22 rows (`background_execution` 0 int/nightly, `c2s` 100 int/nightly, `cloud_config` 100 all, `infra_safety` 100 int, `kvim` 100 int, `lite` 100 int, `local_sandbox` 100 all, `managed_settings` 0 int/nightly, `memory` 100 all, `memory_controls` 100 all, `remote_changelog` 100 stable, `remote_sandbox` 100 all, `session_dashboard` 100 all, `standalone_toolbox` 0 all, `tangent` 100 all, `test*` ×3, `trust_classifier_shadow` 100 int/nightly, `tui` 50 int, `voice` 100 all, `workflows` 100 all) | — | identical |

Overrides (`rollout_overrides`): unchanged (`overrides changed: False`; `v3_prompt.treatment` 135 cohort ids, `standalone_toolbox.treatment` 7).
`remote_sandbox` row text (both): "Remote/cloud sandbox sessions (--cloud/--repo flags). Ramped to 100% for all users on every channel." — so the 2.32.0 non-interactive cloud path is gated only by the rollout (on) and by what KAS advertises per account.

**Env-var set** (`static-host-env-2.32.0.py` → `raw/env/`, `raw/env.log`; every candidate re-counted): launcher 150 → 149 and term 63 → 63 tokens, all differences are glue (`Q_SET_PARENT_CHET` etc.). chat 225 → 230 tokens; the only add whose substring count in OLD is 0 and survives re-count is **`KIRO_STANDALONE_CUTOVER` [0,1]**: `…KIRO_NO_REMOTE_CHANGELOGKIRO_ROLLOUT_FORCE_INTERNALKIRO_STANDALONE_CUTOVERASBX_KIRO_MANDATORY_MCPS…`. Rejected glue (both-version counts equal): `KIRO_API_KEY` 10/10, `KIRO_CLI_CLIENT_APPLICATION` 2/2, `KIRO_KAS_SERVER_PATH` 2/2, `KIRO_KAS_NODE_PATH` 2/2, `KIRO_TELEMETRY_IS_INTERNAL_AMAZON` 1/1, `KIRO_TEST_MODE` 3/3, `KIRO_TRUST_CLASSIFIER_SHADOW` 5/5, `KIRO_CLI_TOOL_SEARCH_MATCHING_THRESHOLD` 1/1. KAS endpoint overrides unchanged (`KIRO_KAS_ENDPOINT`, `KIRO_KAS_CONTROL_PLANE_ENDPOINT`, `KIRO_KAS_REGION` 1/1, `kas_endpoint_overrides.rs` 5/5; `KIRO_REMOTE_SESSIONS_ENDPOINT` 1/1). `KIRO_NO_PROGRAM_STATUS` 0/0 in all three host binaries (TUI-only).

**Settings-key set** (`static-host-settings-2.32.0.py`, `raw/settings.log`): 120 → 124 raw, 123 real.
- + `chat.disabledModelFallbacks` [0,4] — "Models whose fallback the user switched off, per profile (object of profile ARN …" (read by v2 `fallback_choice.rs`).
- + `chat.modelPicker.sortBy` [0,2] — "Model picker sort: 'last-used', 'cost' or 'company' (string)".
- + `chat.modelPicker.lastUsed` [0,2] — "Model picker record of explicit picks, model id to epoch ms (object)".
- `mcp.rsNo` = glue (`chat_cli_v2::agent::acp::commands::mcpcrates/chat-cli-v2/src/agent/acp/commands/mcp.rsNo MCP servers configured`), rejected.

## Doc-manifest delta
`static-doc-manifests-2.32.0.py` → `raw/docman/`, `raw/docman.log`: both builds embed the same two manifests — `(generated_at 2026-10-12T02:11:59.964722, 103 docs)` and `(2026-09-28T19:09:06.985120, 140 docs)`; added 0, removed 0, changed 0 (keyed by `path`, compared on title/validated/status/category/description/file/keywords). No unannounced `category:feature` node this cycle; no sitemap check needed. None of the 2.32.0 features (cloud non-interactive, program status, output styles, memory What's-new) has a host doc node update.

## Unreleased/dark candidates

| candidate | gate | default | evidence |
|---|---|---|---|
| Cloud sessions for non-interactive runs (host-side cloud client) | rollout `remote_sandbox` 100% + KAS `executionTargets` advertisement per account/build | off for any account whose KAS advertises only `["local"]` (standalone default) | gate literal "--cloud is not available: the agent did not advertise cloud sandbox sessions for this account or build" [0,1]; `executionTargets` [0,1] |
| `KIRO_STANDALONE_CUTOVER` | env | unset | chat env table only; no prose; pairs with `standalone_toolbox` 0% row + `_ standalone-cohort` helper (unchanged) |
| V3 ease-in to more internal users | `v3_prompt` | 10% external / **75% internal** | rollout field diff |
| Model-picker sort/recency | settings `chat.modelPicker.sortBy`/`lastUsed` | unset | setting descriptions; not in feed |
| `_kiro.dev/telemetry/modeChanged` (v2) | TUI→v2 notification | — | const-table literal [0,1]; not in feed |
| unchanged dark rows | `managed_settings` 0% int/nightly, `background_execution` 0% int/nightly | 0% | identical rows |

## Leads for the live lane

All legs: archived 2.32.0 `bin/` FIRST on PATH (trap 8), record child `/proc/<pid>/exe`, fresh temp `HOME`, real `XDG_DATA_HOME` (trap 7), auth single-flight by the live driver (trap 6). The host has no `KIRO_ACP_RECORD_PATH` (0/0); capture with `--output-format stream-json` ("The run's ACP events as JSON Lines on stdout").

- **L1 (KAS, near-zero cost, highest) cloud gate + initialize shape.** `kiro-cli chat --no-interactive --agent-engine v3 --cloud --output-format stream-json "reply ok"`. Expected for a non-entitled account: nonzero exit, stderr "--cloud is not available: the agent did not advertise cloud sandbox sessions for this account or build", no `session/new` (no billed turn); stream shows the KAS `initialize` result with `_meta.kiro.executionTargets` (expect `["local"]`) and `sourceProviders`. If entitled: record the `session/new` `_meta.kiro.executionTarget` request object exactly (field names, repo list shape) — that is the shape cyril-tikf needs. Positive control: `kiro-cli chat --list-models --agent-engine v3` (no turn) proves the same auth/env reaches KAS.
- **L2 (KAS, zero cost) flag validation is pre-spawn.** `kiro-cli chat --no-interactive --agent-engine v3 --cloud --resume-id 00000000-0000-0000-0000-000000000000 "x"` and `… --cloud --sandbox "x"`. Expected: immediate argument errors ("--cloud/--repo start a new cloud session and cannot be combined with --resume/--resume-id; …", "--sandbox applies to local sessions and cannot be combined with --cloud") with no node child spawned (check `/proc` for `acp-server.js`).
- **L3 (KAS) `--repo` gate.** `kiro-cli chat --no-interactive --agent-engine v3 --cloud --repo owner/name "x"` on an entitled account: expect a `_kiro/sourceProviders/list` request only if `initialize` advertised `sourceProviders:true` and the method in `extensionMethods`; otherwise "Attaching a repository (--repo) requires a connected source provider. Connect one at <url>, then re-run."
- **L4 (KAS, one turn) plain-text refusal/cancel + closed stdout.** (a) `kiro-cli chat --no-interactive --agent-engine v3 "reply ok" | head -c 0; echo ${pipestatus}` → expect nonzero exit from kiro-cli and no `turn_end` telemetry; (b) SIGINT mid-turn in text mode → stderr "Error: the turn was cancelled before it finished". Control: same run with stdout to a file → exit 0.
- **L5 (v2, low; drop unless budget).** v2 `kiro.dev/commands/execute {command:"fallback", args:{targetModelId:<id>, disabled:true}}` → response text "Switched off the fallback for …" and `$HOME/.kiro/settings/cli.json` gains `chat.disabledModelFallbacks`; then spawn a subagent and confirm it inherits (`inherited_model_fallback`).
- **Negative leads (do not chase):** OSC 7501 / `KIRO_NO_PROGRAM_STATUS` and the memory "What's new" are TUI (0 host hits); MCP OAuth toast is TUI (`_kiro/mcp/oauthCompleted`, tui lane); registry `oauth`/`oauthScopes` is KAS (host v2 `RegistryMcpServerConfig` 7/7 unchanged).

## Changelog reconciliation

`feed-2.32.0.json` (34 lines):

| # | line | location / verdict |
|---|---|---|
| 1 | `--cloud`/`--repo` in non-interactive runs, print cloud session id | **host** `chat-cli/src/launch.rs` non-interactive V3 client: help text change; `executionTargets` [0,1]; gate literal; `_kiro/sourceProviders/list`; "[cloud] Disconnected … `kiro-cli chat --resume-id`" |
| 2 | OSC 7501 program status; `KIRO_NO_PROGRAM_STATUS` | not in my lane — 0 hits in all three host binaries; TUI lane (+`KIRO_NO_PROGRAM_STATUS` in TUI KIRO_ENV set) |
| 3 | [V3] write approvals custom folder at session/workspace/user scope | not in my lane (KAS/TUI) |
| 4 | [V3] `/workflow run` confirmation form | not in my lane |
| 5 | [V3] Proactive/Explanatory/Learning output styles | not in my lane; host `chat.outputStyle` key 1/1 unchanged, "Proactive"/"Explanatory" 0/0 |
| 6 | [V3] workflow steps use Powers | not in my lane (KAS) |
| 7 | [V3] design steps use MCP | not in my lane (KAS) |
| 8 | Startup "What's new" introduces memory | not in my lane — host "What's new" 0/0; rollout `memory`/`memory_controls` rows unchanged |
| 9 | [V3] Always Allow scope page `s` | not in my lane (TUI) |
| 10 | V2 subagents follow session's model fallback | **host (v2)**: `inherited_model_fallback` [0,1] in subagent spawn record |
| 11 | [V3] step session opens on its task | not in my lane (KAS) |
| 12 | [V3] prompt-created workflow shows plan for approval | not in my lane |
| 13 | [V3] no `sharp`/libvips | **host size**: KAS tar frame −6,982,045 B compressed; content = kas lane |
| 14 | [V3] running workflows accept plain-language changes | not in my lane (KAS) |
| 15 | [V3] saved-session lists refresh without blocking | not in my lane (KAS/TUI) |
| 16 | [V3] review loops pause after two rejections | not in my lane (KAS) |
| 17 | [V3] `/chat save` non-`.zip` name says zip archive | not in my lane ("zip session archive" 0/0 in host) |
| 18 | [V3] `/chat load` explains altered archive | **host** (session import used by the `import-session` helper): "File looks like a zip archive but cannot be read. It was probably changed after it was saved. Save the session again." [0,1] replaces "File has zip magic bytes but is not a valid zip archive" [1,0] |
| 19 | [V3] non-interactive `--resume-id` of a cloud session resumes it | **host** `launch.rs`: "…would be loaded from the cloud store, which --sandbox cannot isolate" [0,1]; "to continue a cloud session pass only --resume-id <id>" [0,1] |
| 20 | MCP OAuth links disappear; server named in toast | not in my lane (TUI `_kiro/mcp/oauthCompleted`); host v2 `_kiro.dev/mcp/oauth_request` 1/1 unchanged |
| 21 | plain-text non-interactive explains cancel/refusal/start failure | **host** `launch.rs`: "Error: the model refused to continue the turn" [0,1], "Error: the turn was cancelled before it finished" [0,1] |
| 22 | non-interactive exits nonzero when stdout is closed | **host** logic only (no literal); L4 |
| 23 | V2 `/model` fallback None turns fallback off | **host (v2)**: `FallbackArgs` 2 → 3 fields (`disabled`), `fallback_choice.rs`, `chat.disabledModelFallbacks`, "Switched off the fallback for " |
| 24 | `/model` fallback row names vendor default | **host (v2)**: "vendor default" 2 → 4; new row text "(refusals only):fallback switched off on this machine" [0,1] |
| 25 | V2 crew stage that cannot start fails with its error | **host (v2)**: `DiscardUnstartedSpawn` [0,1], "Cannot spawn under session " [0,3] |
| 26 | [V3] paused step in parallel branch finishes | not in my lane (KAS) |
| 27 | [V3] turn not ended early when output budget spent on reasoning | not in my lane (KAS) |
| 28 | [V3] buffered workflow messages survive restart | not in my lane (KAS) |
| 29 | [V3] command approval shell syntax incl. fish | not in my lane (KAS); host v2 `shell_permission/decider.rs` −1 shift only |
| 30 | [V3] completed workflow steps send follow-up | not in my lane (KAS) |
| 31 | [V3] org MCP registry `oauth`/`oauthScopes` | not in my lane (KAS); host v2 registry structs unchanged (`oauth_scopes` 4/4, `RegistryMcpServerConfig` 7/7) |
| 32 | [V3] oversized images retry at provider limit | not in my lane (KAS) |
| 33 | [V3] resumed sessions show saved model/effort early | not in my lane (TUI/KAS) |
| 34 | [V3] workflow pause messages | not in my lane (KAS) |

## Proposed issues/notes
No new issue: every KAS-path host finding extends an open issue; v2 findings are legacy (traps 24).
- **NOTE cyril-tikf (P3, KAS-4 — keep P3 unless L1 shows the user's account advertises a cloud target):** kiro-cli 2.32.0 makes the host a second Kiro client that consumes the initialize capabilities: `executionTargets` gates `chat --cloud` (now also non-interactive) with "--cloud is not available: the agent did not advertise cloud sandbox sessions for this account or build", and `sourceProviders` + `extensionMethods` gate `--repo` via a new host call to `_kiro/sourceProviders/list`. The cloud request itself is `_meta.kiro.executionTarget` on `session/new`. cyril reads neither key (`fingerprint.rs:30-36` checks only that `_meta.kiro` is an object) and sends no `executionTarget`. Evidence: audit-2.32.0/host.
- **NOTE cyril-a7nk:** 2.32.0 ramps the V3 ease-in to 75% of internal users (`v3_prompt.internal_treatment_percent` 50 → 75; external stays 10%); `acp --agent-engine` default still `v2`; Classic deprecation copy unchanged.
- **NOTE cyril-ci3j (v2, low):** 2.32.0 v2 fallback adds a per-model off switch (`chat.disabledModelFallbacks`, TuiCommand `FallbackArgs.disabled`) and makes subagents inherit the parent's fallback (`inherited_model_fallback`); the `model_fallback` session/update vocabulary is unchanged and still dropped at `kiro.rs:1057-1063`.
- Not filed: `KIRO_STANDALONE_CUTOVER`, `_kiro.dev/telemetry/modeChanged`, `chat.modelPicker.*` (no cyril consumer, no wire effect on cyril's path).

## Method notes
- **Masked strings first** (`raw/mkstrings.py`, copied from 2.31.1): frames zeroed per binary — chat 2.31.1 `[(6957565, 27808487), (34767626, 32031786), (66799989, 29887616), (96687669, 2124271)]`, 2.32.0 `[(6715617, 20826442), (27543487, 32031786), (59575694, 29887616), (89463374, 2133192)]` (offset, csize). `raw/ctx.py <regex>` = context per version; `raw/cnt.py <lit>…` = `[old,new]` counts; `raw/sentdiff.py <regex>` = one-sided prose fragments.
- **Size accounting** from `readelf -SW`: chat `.rodata` −7,000,064 = KAS −6,982,045 + TUI +8,921 + other −26,940; the rest of the shrink is `.text`/`.rela.dyn`/`.data.rel.ro`.
- **New reusable scripts** (`experiments/conductor-spike/`): `static-host-deps-2.32.0.py` (cargo-registry crate-version set diff from panic/tracing paths — answers "did a dependency leave?"), `static-host-methods-2.32.0.py` (`_kiro.dev/*`, `_kiro/*`, `kiro.dev/*`, `kiro/*` token set + segment-prefix count diff). Its lookbehind misses tokens glued after a lowercase letter (`…jsonkiro/sourceProviders/list`): confirm such methods with `raw/cnt.py`. Copies of the 2.31.1 env/paths/srcloc/serde/settings/vocab/novel-words/prose/idents/hashdiff/strdiff/rollout/doc-manifest scripts carry the 2.32.0 name, logic unchanged; `raw/run-all.sh` runs them.
- **Trap 2/3:** every add/remove above is re-counted in both builds; glue rejected: `mcp.rsNo`, `_kiro.dev/clear/statusFailed`, `_kiro.dev/compaction/statusFailed`, `kiro/skills/config`, the launcher "Logged in with …" and PID-file runs (same literals, reordered), most of `raw/prose-*.log` and `raw/idents-*.log` (+322/−827 prose runs, +782/−991 idents on chat are LTO re-glue).
- **Trap 8:** help sweep with archive `bin/` first; `_ get-kas-token`/`export-session`/`import-session`/`windows-sandbox`/`get-telemetry-credentials` are rejected by `_` in BOTH builds (they live in another hidden namespace; their text is byte-identical in the strings, so no verdict depends on the namespace).
- **Cross-lane references (read, not analysed):** `_kiro/sourceProviders/list`/`listResources` exist in KAS 0.66.34 and 0.66.43 (kas lane `surface/static-kas-surface.json`); TUI 2.32.0 builds `FallbackArgs` as `{targetModelId:e,disabled:!0}` and gates source providers on `kiroCapabilities.sourceProviders===!0 && extensionMethods.includes(m)` (`~/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js`).
- **Trap 6/13:** no backend contact, no auth, no session.
- Raw evidence: `raw/` (logs, rollout/, docman/, env/, help/, methods.log, deps.log) and `tmp/` (strings, paths, srcloc, serde, novel, running-notes.md; git-ignored).

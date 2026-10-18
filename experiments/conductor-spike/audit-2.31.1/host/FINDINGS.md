# Host lane — kiro-cli 2.29.1 → 2.30.0 → 2.31.0 → 2.31.1 (static)

Lane: **host** (Rust launcher `kiro-cli`, host binary `kiro-cli-chat`, term `kiro-cli-term`).
Audited 2026-10-14. Static only. Executions: `--help` on the **archived** binaries under
`env -i HOME=<tmp> XDG_DATA_HOME=<tmp>/xdg PATH=<archive bin>:/usr/bin:/bin` (`raw/help.sh`).
Nothing authenticated, no `acp`/`chat`/`login`/`whoami` session, installed launcher never run.
Inputs: `~/.local/share/kiro-research/binaries/<ver>/extracted/kirocli/bin/*` + `BUILD-INFO`.
All string evidence comes from zstd-frame-masked strings (`raw/mkstrings.py` → `raw/strings/<ver>/<bin>.txt`,
KAS tar / node ELF / bun ELF / 2.1 MB TUI-adjacent frame zeroed), so KAS/TUI/Node/Bun text never counts.
Counts below are always `[2.29.1, 2.30.0, 2.31.0, 2.31.1]` substring counts in `kiro-cli-chat` unless noted.

## Summary
1. **v2 ACP surface frozen.** `_kiro.dev/*` method set, TuiCommand `*Args` structs (42) and tagged-enum set (50) count-identical in all four builds; `acp --help` byte-identical (default engine still `v2`, `--auth-method cli|client` unchanged). No new v2 wire field/notification for refusal/unavailable.
2. **v2 engine behaviour changes without new wire names (2.31.1):** compaction now treats a refusal as "no summary" — new literal `The model declined to summarize the conversation` [0,0,0,1] and new notice `History was reduced to fit the context window ... because no summary of them could be produced.` [0,0,0,1]. That routes into the existing `unsummarized_dropped` compaction status, which cyril still drops (NOTE on cyril-0f4e).
3. **KIRO_HOME (2.31.0):** new `crates/agent/src/agent/util/directories.rs:97` "ignoring KIRO_HOME; using the default Kiro home" (v2 engine). v2 agent-config load already honored KIRO_HOME in 2.29.1 (`load.rs:100 ... KIRO_HOME.kiro`); 2.31.0 moved the `.kiro` fallback into the shared resolver. cyril resolves KIRO_HOME **verbatim** (no relative/root/home rejection) and ignores it entirely for global agents + hooks → new issue proposed.
4. **Rollout table:** 2.30.0 `v3_prompt` internal-35% → **all users 10% / internal 50%** (new field `internal_treatment_percent`); 2.31.0 `local_sandbox` internal-nightly → **GA all**; 2.31.1 **new dark row `managed_settings` 0% internal nightly** (`_kiro/governance/state` subscription + pre-session `_kiro/config/template`); `standalone_toolbox` cohort overrides 1 → 7 entries.
5. **"Offer to switch to V3 at chat start" (2.30.0) is host:** `launch/v3_ease_in.rs` copy rewritten ("CLI 3.0 is becoming the default experience" → "You're using Kiro CLI V2. Switch to V3 today.") + the v3_prompt ramp. No evidence it reaches `kiro-cli acp`; live lead L2.
6. **Same BUILD_HASH a1561bc5 (2.30.0/2.31.0) = release-train stamp:** every binary differs; chat `.text` +47,168 B, `.rodata` +20,480 B, two new source files, new literals, different KAS frame (0.66.33→0.66.34).
7. **Auth:** 2.31.0 social refresh (`Failed to reach the social auth service to refresh the token` [0,0,2,2]); 2.31.1 new `chat-cli-v2/src/auth/partition.rs:159` used by `kas_token.rs` (the host's KAS token path) — region→partition OIDC endpoint, `https://oidc.invalid` refusal. cyril never refreshes tokens → N/A.
8. **Env:** +`KIRO_DISABLE_TOOLKIT_TELEMETRY` (2.31.1, all three binaries, unannounced), +`KIRO_TELEMETRY_EXPLICIT_HISTOGRAMS` (2.31.0), −`KIRO_ENABLE_LOCAL_SANDBOX` (2.31.0, superseded by GA). Settings: +`chat.screenReader` (2.30.0) → 120 keys.
9. **Doc manifest:** 164 docs / 37 `category:feature` all four; only change = 2.31.0 restamp of manifest 1 with `slash-commands/settings.md` validated 2026-07-01→2026-10-08, keywords +`verbosity` +`output style` (existing settings). No unannounced feature node.
10. cyril verdicts: settings-handshake parity HANDLED (`settings.rs:85,120-130`); `model_fallback` and `unsummarized_dropped` still DROPPED (open cyril-ci3j, cyril-0f4e); KIRO_HOME divergence = 1 new issue.

## Deltas

Legend: STATIC = binary evidence only (trap 4). cyril verdicts against `crates/` with `file:line` grepped this session.

| item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| BUILD_HASH a1561bc5 shared, binaries differ | 2.30.0 vs 2.31.0 | n/a | change | STATIC | N/A — cyril gates on semver, not hash (`crates/cyril-core/src/protocol/kas/version.rs`) | no |
| rollout `v3_prompt` 35%/internal → 10%/all + `internal_treatment_percent: 50` | 2.30.0 | rollout row | change | STATIC | NOT-SENT (cyril reads no rollout table; spawns `kiro-cli acp`, not `chat`) | NOTE on cyril-a7nk |
| `launch/v3_ease_in.rs` prompt copy: "You're using Kiro CLI V2. Switch to V3 today." / "Switch to V3 and upgrade my configs" | 2.30.0 | v3_prompt bucket | change | STATIC | NOT-SENT; acp reachability unproven → L2 | NOTE on cyril-a7nk |
| serde `FeatureRollout` 4 → 5 fields (`internal_treatment_percent` [0,8,8,8]) | 2.30.0 | n/a | add | STATIC | N/A | no |
| settings key `chat.screenReader` [0,2,2,2] (119→120) | 2.30.0 | off | add | STATIC | NOT-SENT — not in cyril's `BOOL_MAP` (`crates/cyril-core/src/protocol/kas/settings.rs:78-88`); display-only TUI setting | no |
| source `crates/agent/src/agent/agent_config/migration/fingerprint.rs` + agent-JSON field `kiroUpgrade` / `fingerprint` `sha256:` [0,1,1,1] | 2.30.0 | agent upgrade | add | STATIC | NOT-SENT; possible interaction with cyril trust writes `crates/cyril-core/src/kiro_agent_config.rs:156-196` (JSON round-trip) → L5 | maybe (after L5) |
| "Skills will ask for approval on V3: read restrictions aren't applied to skill loading yet." [0,1,1,1] | 2.30.0 | agent upgrade | add | STATIC | NOT-SENT | no |
| `semantic-search-client/.../semantic_context.rs` line shifts [+1,+9], no literal | 2.30.0 | n/a | change | STATIC | NOT-SENT (no knowledge client) | no |
| new source `crates/agent/src/agent/util/directories.rs:97` "ignoring KIRO_HOME; using the default Kiro home" | 2.31.0 | KIRO_HOME relative/root/home | add | STATIC | **DIVERGES**: cyril uses KIRO_HOME verbatim when non-empty `crates/cyril-core/src/kiro_agent_config.rs:92-98`, `crates/cyril-core/src/protocol/kas/settings.rs:51-59` | **yes (new)** |
| v2 `agent_config/load.rs` `.kiro` fallback moved out (`load.rs:100 ...KIRO_HOME.kiro{` → `load.rs:98 KIRO_HOME{`) | 2.31.0 | n/a | change | STATIC | **DIVERGES (pre-existing)**: cyril global agents dir ignores KIRO_HOME `kiro_agent_config.rs:101-104`; KAS hooks dir ignores it `crates/cyril-core/src/protocol/domain_mediator/mod.rs:262-266` | same new issue |
| `chat-cli-v2/src/util/paths.rs` −5 lines, `chat-cli/src/util/paths.rs` −2 | 2.31.0 | n/a | change | STATIC | sidecar `kiro_home_dir()` `crates/cyril-core/src/usage/kiro_sidecar.rs:80,402-416,513-514` — v2 sessions dir behaviour under KIRO_HOME → L1 | same new issue |
| rollout `local_sandbox` internal/nightly → all/any channel | 2.31.0 | GA, sandbox off by default | change | STATIC | NOT-SENT | no |
| env `KIRO_ENABLE_LOCAL_SANDBOX` [1,1,0,0] removed; "--sandbox is not enabled for this build" [0,0,1,1] replaces "LocalSandbox requires internal nightly eligibility" [1,1,0,0] | 2.31.0 | n/a | remove/change | STATIC | NOT-SENT | no |
| env `KIRO_TELEMETRY_EXPLICIT_HISTOGRAMS` [0,0,2,2] | 2.31.0 | unset | add | STATIC | NOT-SENT | no |
| new source `crates/kiro-telemetry-observer/src/work.rs`; metrics `kiro_cli_work_turn_total`, `kiro_cli_time_to_validated_result` [0,0,2,2], `work_outcome` [0,0,4,4], build/test "validation command table" regex | 2.31.0 | telemetry | add | STATIC | NOT-SENT | no |
| social token refresh: "Failed to reach the social auth service to refresh the token" [0,0,2,2]; `social.rs` 15→16 / 23→24 call sites | 2.31.0 | n/a | fix | STATIC | N/A — cyril reads, never refreshes, the store `crates/cyril-core/src/protocol/kas/auth.rs` | no |
| v2 `chat-cli-v2/src/agent/acp/acp_agent.rs` +3 lines ≤1379, +14 in 1781–2063, +13 more by 2402; no new literal | 2.31.0 | n/a | change | STATIC | unknown → L7 | no |
| V1 `chat-cli/src/cli/chat/mod.rs` large shifts; `NonInteractiveToolApproval` [2,2,2,2] | 2.31.0 | V1 only | fix | STATIC | NOT-SENT (cyril never runs V1 `--no-interactive`) | no |
| doc manifest 1 restamp 2026-09-02→2026-10-12; `slash-commands/settings.md` validated + keywords `verbosity`, `output style` | 2.31.0 | n/a | change | STATIC | N/A | no |
| rollout **`managed_settings`** ADDED treatment 0 / internal / nightly | 2.31.1 | **dark**, opt-in `KIRO_ENABLED_FEATURES='["managed_settings"]'` | add | STATIC | `_kiro/governance/state` DROPPED (open cyril-0o7e); `_kiro/config/template` NOT-SENT (0 hits in crates/) | NOTE on cyril-0o7e |
| rollout overrides `standalone_toolbox.treatment` 1 → 7 cohort entries (digest 8f95b89b… → 3e9c2d34…) | 2.31.1 | row still 0% | change | STATIC | NOT-SENT | no |
| new source `crates/chat-cli-v2/src/auth/partition.rs:159` (also compiled into chat-cli) "refusing to build an OIDC endpoint: sign-in in this region is not supported" [0,0,0,2] next to `kas_token.rs:192` | 2.31.1 | n/a | add | STATIC | N/A (cyril never builds OIDC endpoints; 0 `oidc` hits in crates/) | no |
| login controls: "Kiro does not support sign-in in . Check the spelling..." [0,0,0,1], "managed setting is not a region Kiro can sign in to" [0,0,0,1], "Unusable sso_region" [0,0,0,1] | 2.31.1 | managed `idc_region` | add | STATIC | NOT-SENT | no |
| v2 compaction: "The model declined to summarize the conversation" [0,0,0,1]; "compaction produced no summary - applying brute-force history reduction" [0,0,0,1] (replaces "compaction request overflowed - applying brute-force history reduction" [1,1,1,0]) | 2.31.1 | default | change | STATIC | `failed{error}` HANDLED `crates/cyril-core/src/protocol/convert/kiro.rs:796-802`; `unsummarized_dropped` [2,2,2,2] DROPPED at `kiro.rs:803-808` | NOTE on cyril-0f4e |
| v2 notice "History was reduced to fit the context window: the oldest N messages were dropped without summarization because no summary of them could be produced." [0,0,0,1] (old text "…after repeated context-window overflows…because the summarization request itself exceeded the context window." [1,1,1,0]) | 2.31.1 | default | change | STATIC | carrier unknown → L3 | NOTE on cyril-0f4e |
| v2 model-fallback walk literals ("moving the rest of this turn to another model", "a refused turn was answered elsewhere…") [1,1,1,1]; `agent_loop/mod.rs` +6 at :1193→:1199 | 2.31.1 | default-on (GA 2.28.0) | logic only | STATIC | `model_fallback` session/update DROPPED `kiro.rs:1057-1062` (open cyril-ci3j) | NOTE on cyril-ci3j |
| host non-interactive V3 launch sends `disableAutoCompaction`/`minPct`/`minTokens` (literal [3,3,3,4]/[4,4,4,5]/[4,4,4,5], new at `launch.rs:2601` beside "kiro-cli-non-interactive") | 2.31.1 | V3 headless | add | STATIC | **HANDLED** (cyril already marshals them every spawn) `crates/cyril-core/src/protocol/kas/settings.rs:85,120-130` | no |
| env `KIRO_DISABLE_TOOLKIT_TELEMETRY` [launcher 0,0,0,1 / chat 0,0,0,2 / term 0,0,0,1] at `fig_telemetry/src/util.rs:75` | 2.31.1 | unset | add | STATIC | NOT-SENT | no |
| settings key set | 2.31.0/2.31.1 | 120 | none | STATIC | N/A | no |
| `_kiro.dev/*` method set, `*Args` structs, tagged enums | all | — | none | STATIC | N/A (`raw/methods-v2.txt`, `raw/tuicommand-args.txt`) | no |
| `--help`/`--help-all` all three binaries + `acp`, `chat`, `agent`, `agent upgrade`, `_`, `login` | all | — | none (byte-identical md5) | STATIC | N/A (`raw/help/`) | no |
| Classic deprecation wording (`classic` 52, `deprecated` 129, "being deprecated" 3, `sunset` 1) | all | — | none | STATIC | N/A | no |

### Detail

**Same BUILD_HASH (trap 20).** `BUILD-INFO`: 2.30.0 `BUILD_DATE=2026-10-12T20:15:21Z`, 2.31.0 `2026-10-12T23:56:54Z`,
both `BUILD_HASH=a1561bc58200f7614b7cfb9761868e19adf812c4`; 2.31.1 `d6fb1e94…`. Sizes/sha256 (`raw/identity.txt`):
chat 215,863,336 `488a7442…` → 215,947,368 `e88692b8…` (+84,032); launcher 43,927,712 → 43,926,688; term 34,056,224 → 34,058,528.
Chat ELF (`raw/elf-sections-chat.txt`): `.text` +47,168, `.rodata` +20,480, `.eh_frame` +5,268, `.gcc_except_table` +3,204.
Source-level proof of a different revision: two new source paths (`agent/util/directories.rs`, `kiro-telemetry-observer/src/work.rs`),
`social.rs` +1 call site, `otel.rs` 4→5, v2 `acp_agent.rs` +27 lines, new literals (`Failed to reach the social auth service…`,
`--sandbox is not enabled for this build`, `ignoring KIRO_HOME…`), rollout row `local_sandbox` flipped, KAS frame
27,801,781 → 27,808,488 csize (0.66.33 → 0.66.34). Node and Bun payload frames are byte-identical in all four builds
(32,031,786 / 29,887,616 csize). [INFERENCE] the hash names a release train/branch config, not the artifact.

**KIRO_HOME — what moves, and cyril.** Host evidence:
- 2.29.1 `@5054876`: `event crates/agent/src/agent/agent_config/load.rs:100KIRO_HOME.kiro{` (v2 engine agent-config load already read KIRO_HOME, `.kiro` fallback inline).
- 2.31.0 `@5059451`: `…agent_config/load.rs:98KIRO_HOME{` and `@5094909`: `event crates/agent/src/agent/util/directories.rs:97 … ignoring KIRO_HOME; using the default Kiro home` — the fallback and validation moved to one resolver.
- `KIRO_HOME` 6→7 occurrences is exactly that new warning; the env table (`…KIRO_TEST_HOMEBREW_PREFIXKIRO_HOMEKIRO_DATA_DIR…`) and the embedded doc keyword list are unchanged. The host does not pass any new path to KAS; the `[V3]` relocation (MCP config, permissions, sessions, powers, agent state, MCP sign-ins) is KAS 0.66.34 code → kas lane.
- Which v2 paths route through `directories.rs` cannot be enumerated statically (stripped). Proven: agent configs. Probable: everything under the Kiro home (`chat-cli-v2/src/util/paths.rs` lost 5 lines in the same release; it holds `KIRO_TEST_SESSIONS_DIR` + `cli` + `.kiro`, i.e. `<home>/sessions/cli`). → L1.
- cyril today: (a) `kiro_home_dir()` = non-empty KIRO_HOME verbatim (`kiro_agent_config.rs:92-98`; same rule `kas/settings.rs:51-59`), so a relative, `/`, or `$HOME` KIRO_HOME makes cyril read `<that>/settings/cli.json` and `<that>/sessions[/cli]` while Kiro ≥2.31.0 uses `~/.kiro`; (b) `global_agents_dir()` = `~/.kiro/agents` ignoring KIRO_HOME (`kiro_agent_config.rs:101-104`) — trust grants are written to a file Kiro does not read whenever KIRO_HOME is set (pre-existing since ≤2.29.1, now also true for KAS per the 2.31.0 line); (c) KAS inbound hooks load `~/.kiro/hooks` (`domain_mediator/mod.rs:262-266`). Reviewer is safe: it sets an absolute `KIRO_HOME=<root>/home/.kiro` (`crates/cyril-workbench/src/reviewer/runtime.rs:207`). KAS discovery uses the data dir, not KIRO_HOME (`crates/cyril-core/src/protocol/kas/discovery.rs:112-124,168-188`) → unaffected.

**V3 ease-in (2.30.0 line "may now offer to switch you to V3").** 2.29.1 `@100234314`: `…v3_ease_in.rs:187CLI 3.0 is becoming the default experience.` / `It brings cloud sessions, tangents, specs…` / `Switch to 3.0 and upgrade my configs`.
2.30.0 `@100236266`: `…v3_ease_in.rs:187You're using Kiro CLI V2. Switch to V3 today.` / `V3 adds workflows, cloud sessions, and specs. To try it in a new session, run kiro-cli --v3.` / `Switch to V3 and upgrade my configs`.
Same six tracing lines (`v3_ease_in.rs:104,121,141,151,161,187`) in all four builds → copy + rollout change, not new logic. The rollout row now says *"Internal users ramp at internal_treatment_percent and everyone else at treatment_percent… a user is never defaulted onto V3 without being asked."*

**Compaction refusal (2.31.1).** `@5107254` (2.31.1): `handling compaction loop eventCompaction response unexpectedly contained tool usescompaction completed successfully…The model declined to summarize the conversationcompaction failed due to context overflow, retrying with aggressive strategy`. `@5107015`: `compaction produced no summary - applying brute-force history reductionHistory was reduced to fit the context window: the oldest  messages were dropped without summarization because no summary of them could be produced.` The v2 wire vocabulary around compaction is unchanged (`@100526951`: `…startedcompletedunsummarized_droppedfailedsessionUpdatetool_call_chunk…retry_warning…stream_stall_noticemodel_fallbackcause…`).

**managed_settings (2.31.1, dark).** Row text: *"Organization-managed settings display on V3: the /config managed category and subcommand, organization Source labels on /config, /mcp and /hooks rows, the organization-disabled/unavailable/hidden MCP row, the registry-hidden /mcp notice, the _kiro/governance/state subscription feeding them (off: the TUI reads only the web-tools toggle from it, as before), and the pre-session _kiro/config/template mode seed. Admin enforcement is not gated: KAS enforces governance itself. Dark-shipped at 0%…"* `_kiro/governance/state` / `_kiro/config/template` occur **only** inside this description in the host ([0,0,0,2] each) — the host is not a party to those methods.

## Unreleased/dark candidates

| candidate | gate | default | evidence |
|---|---|---|---|
| `managed_settings` (org-managed settings UI + `_kiro/governance/state` subscription + pre-session `_kiro/config/template` mode seed) | rollout row, internal + nightly; opt-in `KIRO_ENABLED_FEATURES='["managed_settings"]'` | 0% | `raw/rollout.log` 2.31.0→2.31.1 ADDED; KAS methods cross-ref kas lane; cyril-0o7e already tracks governance/state drop |
| `standalone_toolbox` cohort enrolment | rollout 0% + override table | 0%, overrides 1 → 7 hashed cohort ids | `raw/rollout.log` line "override standalone_toolbox.treatment: 1 -> 7 entries" (six new sha256 ids) |
| V3 ease-in to external users | `v3_prompt` | 10% external, 50% internal | 2.30.0 row change; prompt copy |
| `background_execution` | rollout internal nightly | 0% (unchanged) | row present all four builds |
| `KIRO_DISABLE_TOOLKIT_TELEMETRY` | env | unset | 2.31.1, `fig_telemetry/src/util.rs:75` region `telemetry.enabledKIRO_DISABLE_TOOLKIT_TELEMETRYaccess_key_id…`; not in any feed line |
| work-turn telemetry (`kiro-telemetry-observer`) | telemetry | on | 2.31.0 metrics `kiro_cli_work_turn_total`, `kiro_cli_time_to_validated_result`, `work_outcome`; validation-command regex table (cargo/npm/pytest/go/mvn/gradle/brazil-build/make/tsc/jest/vitest/dotnet/swift) |
| `KIRO_TELEMETRY_EXPLICIT_HISTOGRAMS` | env | unset | 2.31.0 env table |
| doc manifest | — | — | no new `category:feature` node in any build (164/37 constant) → no trap-12 unannounced feature this cycle |

## Leads for the live lane

All v2 legs: archived binary, **its `bin/` first on PATH** (trap 8), record child `/proc/<pid>/exe`, temp `HOME`, real `XDG_DATA_HOME` (trap 7), auth single-flight via the live driver (trap 6).

- **L1 (v2, highest) KIRO_HOME resolution, 2.30.0 vs 2.31.1.** Spawn `~/.local/share/kiro-research/binaries/<ver>/extracted/kirocli/bin/kiro-cli acp` (v2 default) four times per version with `KIRO_HOME` = (a) `$TMPHOME/kh` absolute, (b) `kh` relative, (c) `/`, (d) `$TMPHOME`; plus control (e) unset. Seed `probe.json` agent in `$TMPHOME/kh/agents/` and in `$TMPHOME/.kiro/agents/` with different `description`. Send `initialize`, `session/new {cwd, mcpServers:[]}` with `--agent probe`, one cheap `session/prompt`. Expected: 2.31.1 (a) → `$TMPHOME/kh/sessions/cli/<sid>.json` and the kh agent; (b)(c)(d) → `$TMPHOME/.kiro/...` + log line `ignoring KIRO_HOME; using the default Kiro home` in `$XDG_RUNTIME_DIR/kiro-log/` at `KIRO_LOG_LEVEL=debug`; 2.30.0 (b)-(d) use KIRO_HOME verbatim. Positive control: (e) writes `$TMPHOME/.kiro/sessions/cli/<sid>.json`. Answers whether cyril's sidecar (`kiro_sidecar.rs:402-416`) and trust writes (`kiro_agent_config.rs:101-104`) are pointed at the right files.
- **L2 (v2, zero-auth) ease-in never touches `acp`.** Archived 2.31.1 `kiro-cli acp` with `KIRO_ENABLED_FEATURES='["v3_prompt"]'` and separately `KIRO_ROLLOUT_FORCE_INTERNAL=1`; stdin = pipe; send `initialize` only. Expected: first stdout byte `{`, no `You're using Kiro CLI V2` bytes on stdout or stderr, no `chat.agentEngine` write in `$TMPHOME/.kiro/settings/cli.json`. Positive control: same env, `kiro-cli chat` in a pty shows the prompt (or log `V3 ease-in prompt failed, skipping`).
- **L3 (v2, opportunistic) compaction refusal.** Only if it occurs naturally (trap 13): watch for `_kiro.dev/compaction/status {status:{type:"failed", error:"The model declined to summarize the conversation"}}` followed by `{status:{type:"unsummarized_dropped"}}` and find which frame carries `History was reduced to fit the context window…`. Positive control: a normal `/compact` → `completed` with `summary`.
- **L4 (v2, opportunistic) empty refusal → fallback.** `_kiro.dev/session/update {update:{sessionUpdate:"model_fallback", cause:"refused", from, to, promoted?}}` on a refusal that streamed no text (2.31.1 fix). Control: a text-bearing refusal on 2.30.0.
- **L5 (zero-auth) agent-upgrade fingerprint vs cyril trust writes.** Fixture dir with a V2 agent JSON having `toolsSettings.execute_bash.allowedCommands`. Run archived 2.31.1 `kiro-cli-chat agent upgrade <dir>`; record the written `kiroUpgrade` object (expect `fingerprint: "sha256:…"`). Then apply cyril's write path (append one pattern via `serde_json::Value` round-trip, which may reorder keys) and change a V2 setting; re-run upgrade. Expected signal: whether the agent is now treated as user-edited (refresh skipped) — decides if cyril's trust persistence silently freezes Kiro's permission refresh.
- **L6 (V3) managed_settings.** Opt-in `KIRO_ENABLED_FEATURES='["managed_settings"]'` affects the TUI only; for cyril the useful capture is the KAS side: does a direct `node acp-server.js` emit `_kiro/governance/state` without the TUI subscription, and what does `_kiro/config/template` return pre-session. Handshake path `initialize.clientCapabilities._meta.kiro…` (trap 5); positive control `largeToolOutputHandler`. Coordinate with kas lane.
- **L7 (v2) acp_agent.rs +27 lines (2.31.0).** Same-day paired v2 capture 2.30.0 vs 2.31.0 with the field sweep, diff with `diff_fields.py`; the edit sits between the init-time "Agent event channel lagged" block and session restore ("Failed to set initial model / restore saved session model").
- **Negative leads (do not chase):** `_kiro/governance/state` and `_kiro/config/template` are not host methods (description text only); partition sign-in cannot be exercised without an isolated-partition IdC account; Windows `/sandbox` temp-folder fix is not in the Linux artifacts (`AppLocker` 2/2/2/2, no temp-policy literal).

## Changelog reconciliation

`feed-2.30.0.json` (9 lines):
| # | line | location / verdict |
|---|---|---|
| 1 | [V3] Notices when a model refuses/unavailable and turn moves | not in my lane (KAS == 2.29.1 → TUI); host v2 fallback literals unchanged [1,1,1,1] |
| 2 | [V3] Agent upgrades refresh permissions after V2 setting changes until you edit them | **host**: new `agent_config/migration/fingerprint.rs`, agent-JSON `kiroUpgrade`/`fingerprint` `sha256:`; `load.rs` shifts; L5 |
| 3 | Kiro CLI may now offer to switch you to V3 when you start a chat | **host**: `launch/v3_ease_in.rs` copy rewrite + rollout `v3_prompt` 10% all / 50% internal; L2 |
| 4 | Screen readers follow streaming replies (`/settings display`) | **host part**: settings key `chat.screenReader` [0,2,2,2] + description text; rendering = TUI lane |
| 5 | [V3] Agent upgrades preserve V2 workspace read access and `fs_read` skill trust | **host**: "Skills will ask for approval on V3: read restrictions aren't applied to skill loading yet." [0,1,1,1], `.kiro/skills/` 3→4 |
| 6 | tmux scroll-back blank rows | not in my lane (TUI) |
| 7 | Launch after output without trailing newline | not in my lane (TUI); `launch.rs` shifts are not attributable |
| 8 | Knowledge search no longer drops a chunk | **host**: `semantic-search-client/src/client/context/semantic_context.rs` line shifts [+1,+9], no literal |
| 9 | Garbled text around icons | not in my lane (TUI) |

`feed-2.31.0.json` (12 lines):
| # | line | location / verdict |
|---|---|---|
| 1 | [V3] `/sandbox` + local `/autonomous` GA, off by default | **host**: rollout `local_sandbox` → all; `KIRO_ENABLE_LOCAL_SANDBOX` removed; "--sandbox is not enabled for this build"; feature itself KAS/TUI |
| 2 | [V3] MCP tools load on demand for everyone | not in my lane (KAS); no host rollout row for tool search; `toolSearch` 15/15/15/15 |
| 3 | [V3] `KIRO_HOME` relocates MCP config, permissions, sessions, powers, agent state | KAS lane for V3; **host v2 counterpart**: `agent/util/directories.rs` resolver; cyril divergence (new issue) |
| 4 | [V3] saved `sandbox.json enabled:true` applies to headless runs | not in my lane (KAS/launcher headless); `sandbox.json` 3/3/3/3, `sandbox_args` 1/1/1/1 unchanged |
| 5 | `--no-interactive` V1 exits non-zero with the real error | **host**: V1 `chat-cli/src/cli/chat/mod.rs` large shifts, no new literal |
| 6 | [V3] MCP sign-ins saved under `KIRO_HOME`, copied once | not in my lane (KAS) |
| 7 | [V3] Animated WebP sent as PNG | not in my lane (KAS); `webp` 41/41/41/41 |
| 8 | [V3] 15 s service-request timeout | not in my lane (KAS); launcher `fig_request/reqwest_client.rs` :441→:401 shift is unrelated launcher code |
| 9 | [V3] streamable-HTTP MCP session drop | not in my lane (KAS) |
| 10 | relative/root/home `KIRO_HOME` ignored | **host**: `crates/agent/src/agent/util/directories.rs:97` "ignoring KIRO_HOME; using the default Kiro home" |
| 11 | [V3] cloud sessions code-intel indicator | not in my lane (TUI) |
| 12 | GitHub/Google not signed out on temporary refresh error | **host**: `chat-cli{,-v2}/src/auth/social.rs` +1 call site; "Failed to reach the social auth service to refresh the token" [0,0,2,2] |

`feed-2.31.1.json` (10 lines):
| # | line | location / verdict |
|---|---|---|
| 1 | Mermaid diagrams as terminal art | not in my lane (TUI); host `mermaid` 8/8/8/8 |
| 2 | [V3] Non-interactive runs apply saved compaction + `toolSearch` | **host**: `launch.rs:2601` non-interactive client now emits `disableAutoCompaction`/`minPct`/`minTokens`; cyril HANDLED |
| 3 | `chat.disableWrap` copy leading spaces | not in my lane (TUI) |
| 4 | [V3] Herdr shows session blocked | not in my lane (TUI/KAS); `herdr` 0 in host |
| 5 | Model fallback handles refusals with no response text | **host (v2 engine)**: logic only, `agent/src/agent/agent_loop/mod.rs` +6 lines; wire vocab unchanged; cyril DROPPED model_fallback (cyril-ci3j) |
| 6 | Compaction no longer saves the refusal as the summary | **host (v2 engine)**: "The model declined to summarize the conversation", "compaction produced no summary…", new reduction notice; cyril-0f4e |
| 7 | [V3] `/sandbox` Windows temp-folder policy | not visible in Linux artifacts; KAS identical 2.31.0→2.31.1, so TUI/Windows host build |
| 8 | `/changelog` one entry per line | not in my lane (TUI); host `changelog` 38/38/38/38 |
| 9 | IAM Identity Center sign-in in isolated partitions | **host**: new `chat-cli-v2/src/auth/partition.rs:159` (used by `kas_token.rs`), region/partition literals; `builder_id.rs` +6/+10 shifts, `login_controls.rs` 23→25, `portal.rs` +10/+43 |
| 10 | [security] [V3] workflow recipe text escape sequences | not in my lane (TUI/KAS) |

## Method notes
- **Masked strings first.** `raw/mkstrings.py` zeroes every zstd frame >50 KB before string extraction (four frames in chat: KAS tar, node ELF, bun ELF, a ~2.1 MB frame), then all greps run on `raw/strings/<ver>/<bin>.txt`. `raw/ctx.py <regex> [-v vers] [-b bin] [-w width]` prints count + context per version.
- **Trap 2/3:** every add/remove above is a substring count in all four builds (counts quoted inline). Glue artefacts rejected: `AMAZON_Q_CHAT_SHELLF`, `RUST_MIN_STACKR`, `KIRO_TRUST_CLASSIFIER_SHADOWI`, `KIRO_TEST_KAS_TURN_LOG_PATH` (1/1/1/1), `KIRO_TELEMETRY_IS_INTERNAL_AMAZON` (1/1/1/1), `_kiro.dev/mcp/stGetSessionHandle*` (1/1/1/1), `_kiro.dev/settinxt`, serde `SessionTool::*`/`TodoList::*`/`ResourceContents*` flicker (anchor artefacts of `static-host-serde`).
- **New reusable scripts** (`experiments/conductor-spike/`): `static-host-prose-2.31.1.py` (glue-proof novel-prose diff on inner prose trigrams, source paths/digits stripped), `static-host-idents-2.31.1.py` (camel/snake identifier substring-count diff); `static-rollout-2.31.1.py` now stores each override object and itemises per-feature/arm override entry changes (2.29.1 printed only a digest). Copies of the 2.29.1 env/paths/srcloc/serde/settings/vocab/novel-words/hashdiff/strdiff/doc-manifest scripts carry the 2.31.1 name unchanged in logic.
- **Noise budget:** `static-host-hashdiff`/`strdiff` whole-run set diffs (+145/−134 "prose literals" on the launcher for 2.30.0→2.31.0) are LTO re-glue; not used for findings. `static-host-prose` reduced chat to ~120 runs/release, read by hand.
- **Trap 8:** help sweep ran the archived binaries with their own `bin/` first on a minimal PATH; every help text (launcher/chat/term, `acp`, `chat`, `agent`, `agent upgrade`, `_`, `login`) is md5-identical across the four builds; `kiro-cli-chat --help-all` is rejected by that binary (rc 2, use `--help`).
- **Trap 12:** manifests `(2026-09-02T18:38:18, 103)` → `(2026-10-12T02:11:59, 103)` in 2.31.0, second manifest `(2026-09-28T19:09:06, 140)` constant; 164 documents keyed by path, diffed on title/validated/status/category/description/file/keywords. No sitemap re-check: no new feature node to test.
- **Trap 13/6:** no backend contact, no auth, no session. L1/L3/L4/L7 need the live lane.
- Raw evidence: `raw/` (76 MB): `rollout/`, `docman/`, `paths/`, `srcloc/`, `env/`, `prose/`, `idents-*.txt`, `methods-v2.txt`, `identity.txt`, `elf-sections-chat.txt`, `help/`, `changelog-lines.txt`.

## Proposed rivets actions
- **NEW** "Mirror Kiro's KIRO_HOME resolution everywhere cyril reads the Kiro home" — (1) reject relative, `/` and `$HOME` values exactly as kiro-cli ≥2.31.0 does (`directories.rs:97`), (2) use KIRO_HOME for the global agents dir (`kiro_agent_config.rs:101-104`) and KAS hooks (`domain_mediator/mod.rs:262-266`), (3) keep one resolver for settings/sidecar (`kiro_agent_config.rs:92-98`, `kas/settings.rs:51-59`). Gate on L1 for the v2 sessions-dir half. No existing issue (grep `KIRO_HOME` in `.rivets/issues.jsonl`: only cyril-nhzw closed, cyril-ss5i unrelated).
- **NOTE cyril-0f4e**: 2.31.1 routes compaction refusals into the brute-force reduction path (new "compaction produced no summary" and new user notice text), so the dropped `unsummarized_dropped` status now also hides refusal-driven history loss.
- **NOTE cyril-ci3j**: 2.31.1 extends v2 fallback to refusals with empty text; wire vocabulary unchanged; still dropped at `kiro.rs:1057-1062`.
- **NOTE cyril-a7nk**: 2.30.0 ramps the V3 ease-in to 10% of external users (50% internal) with new "Switch to V3 today" copy; acp default still v2; Classic deprecation strings unchanged.
- **NOTE cyril-0o7e**: 2.31.1 dark row `managed_settings` makes `_kiro/governance/state` the feed for org-managed /config, /mcp, /hooks labels, plus a pre-session `_kiro/config/template` mode seed.

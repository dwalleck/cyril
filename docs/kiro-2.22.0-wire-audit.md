# kiro-cli 2.22.0 ACP audit (delta from 2.21.2, covering the unaudited 2.21.4)

**Audit status:** complete for the stated paired scope. The installed binary
was already 2.22.0 when the audit began; 2.21.2 and 2.21.4 were compared from
the research archive. **2.21.4 (2026-09-11) was never audited and 2.21.3 has
no changelog**, so this document covers the two-release hop 2.21.2 → 2.22.0
and attributes each finding to the release that introduced it. Probes isolated
`HOME` to protect real `~/.kiro` state. Cost: 26 real turns across the v3
turn sweep, four workflow runs, four large-output legs, six watchdog legs and
the v2 sweep; the SDK-method probe sent no prompt.

**Conclusion:** **SAFE for cyril — no code change required.** Unlike 2.21.2,
**KAS is not frozen**: it moved 0.58.7 → 0.63.3 (2.21.4) → 0.66.0 (2.22.0),
and 0.63.3 carries a major ACP SDK bump (`@agentclientprotocol/sdk` ^0.19 →
^1.3.0). The `_kiro/*` method census is unchanged at 110 = 110 = 110, no new
`session/update` variant is emitted, and the field sweep finds **exactly one
new-field family** on both the ordinary-turn and workflow workloads, all in
0.66.0: `session_info_update{kind:"focus_update"}` now carries
`_meta.kiro.activity{turnActive, runningWorkflows, pausedWorkflows}` and a
`focus.status` (§ 6, § 7). Three further behavior changes are live-confirmed:
large tool results are always offloaded to a session file with a preview
(§ 8), the SDK bump **inverted the error codes** KAS returns for
`session/set_model` and `session/delete` (§ 9), and the pre-`session/new`
`fetch_cloud_config` tool call disappears when cloud sync is not enabled
(§ 6). The v2 engine is wire-identical across all three releases (§ 11).
Workflow event vocabulary, payload keys, per-step turn-end ordering and the
artifact channel are unchanged (§ 7).

**Scope.** Compare the released x86_64 Linux `kiro-cli` 2.22.0 artifact with
the archived 2.21.2 and 2.21.4 artifacts across the v2 ACP engine, the three
self-extracted KAS runtimes, the Rust host rollout registry and the embedded
TUI bundle. Static evidence is hypothesis-generating; behavior claims use the
paired live captures listed in § 14. Every v3 leg runs through the **installed
2.22.0 host** with `KIRO_KAS_SERVER_PATH` pinning the bundle, so the KAS-binary
axis is isolated against one same-day backend.

## 1. Acquisition and integrity

The S3-direct `/latest/manifest.json` reported `2.22.0`, Linux x86_64 headless
`tarXz` SHA-256 `16111e91e58cbccebe97070253dc0ad2f5da5f1c3e3c7dc432d8329b27102ad4`
(292,704,924 bytes). Verified before extraction. **The tarball layout changed:**
binaries now live under `kirocli/bin/` beside `kirocli/{BUILD-INFO,README,
install.sh}` (2.21.2 and earlier unpacked flat). Versioned manifests do not
exist for 2.21.3/2.21.4; the 2.21.4 tarball was fetched by URL and its
`BUILD-INFO` confirms the version (no published hash to check against; the
self-extracted KAS tree from the user's own 2.21.4 install matches it).

| release | `BUILD_HASH` | `kiro-cli` | `kiro-cli-chat` | `kiro-cli-term` |
|---|---|---:|---:|---:|
| 2.21.2 | `a4729fa2…` | 113,931,888 | 838,969,752 | 86,904,936 |
| 2.21.4 | `57c33e90…` (2026-09-11) | 113,833,552 | 839,206,888 | 86,909,008 |
| 2.22.0 | `b5aa4876…` (2026-09-15) | 113,901,280 | **462,078,288** | 86,961,584 |

SHA-256 (2.22.0): `kiro-cli` `d0315a97…`, `kiro-cli-chat` `5bd70965…`,
`kiro-cli-term` `2950d21d…` — identical to the installed `~/.local/bin`
binaries. **`kiro-cli-chat` shrank by 45 %.** The `kas-bundle.tar` gzip stream
is still embedded (FNAME-verified at offset 6.69 MB) and the KAS runtime is the
same 11.5 MB `acp-server.js`; a single 339 MB xz stream now sits where the
~450 MB of KAS externals (cedar/z3/onnx/tree-sitter) used to be raw, so the
carve recipe in the methodology memory still works and the externals are just
compressed now.

Embedded changelog: 2.22.0 has 20 entries (identical to the public
kiro.dev changelog, headline "Fullscreen Chat and Streamlined Sessions");
2.21.4 has 6; 2.21.3 returns *"No changelog information available"*.
`kiro-cli --help` and `kiro-cli acp --help` are byte-identical 2.21.2 → 2.22.0.

## 2. Methods and evidence discipline

* Archive and per-binary hashes recorded before interpretation.
* KAS vocabulary compared as **quoted** `_kiro/*` literals per bundle; the
  2.21.2 linear literal scanner was replaced by a regex scanner after it
  derailed on a regex literal and inflated "added" from ~130 to ~3,800.
  **Every static lead below was count-checked in all three bundles** before
  being called new (the `_kiro/workflow/*` literals looked new because 0.66.0
  spells them out; `steps_queued`/`watch_poll` exist in all three).
* Live lanes: self-contained Python raw JSON-RPC probes, temporary `HOME`,
  `XDG_DATA_HOME` at the real share dir, auth redacted, host spawned with
  `start_new_session`. The v3 turn sweep was run **twice** on 0.66.0 because
  the 2.21.2 audit proved a single sample reports the effort-metadata timing
  nondeterminism as a regression — it did so again here (§ 6).
* Controls: every large-output and watchdog leg has a 0.58.7 pin; the SDK
  probe sends the same six methods to both builds.

## 3. Static — KAS 0.58.7 → 0.63.3 → 0.66.0

| lane | 0.58.7 | 0.63.3 | 0.66.0 |
|---|---:|---:|---:|
| `acp-server.js` bytes / lines | 11,397,530 / 17,577 | 11,477,334 / 17,576 | 11,499,652 / 17,546 |
| quoted `_kiro/*` method literals | 110 | 110 | 110 |
| `process.env` reads | 65 | 65 | 65 |
| `KIRO_*` env literals | 49 | 50 | 51 |
| feature-config flags (env-reachable) | 17 (9) | — | 20 (11) |
| `@agentclientprotocol/sdk` | `^0.19.0` | **`^1.3.0`** | `^1.3.0` |

**No new or removed `_kiro/*` method on either hop.** The 8 `dist/server/*.d.ts`
files (the readable island) differ only by an auth-provider rename
`RequestCredential` → `BffIdentity` with the note that `provider`/`authMethod`
strings "are parsed before they reach a header" — semantics, not wire.

### 3a. 2.21.4 hop (0.58.7 → 0.63.3)

* **ACP SDK ^0.19 → ^1.3.0.** The SDK's method tables now list
  `mcp/connect`, `mcp/message`, `mcp/disconnect`, `$/cancel_request`,
  `session/delete`, and the `session/update` union gains `plan_update`,
  `plan_removed`, `usage_update`. **KAS emits none of them** — each occurs
  exactly once in the bundle (the schema), so cyril's typed-variant hard-fail
  (cyril-ai1y) is not triggered. `session/set_model` **left** the SDK's
  agent-method table — see § 9 for the live consequence.
* New flag `tool_load_enabled` (default **false**, env
  `KIRO_FEATURE_TOOL_LOAD_ENABLED`): deferred tool disclosure — tools the
  model calls by name before they are loaded get a hint error and are
  resolved (`process-chunk-stream.deferred-tool-called-by-name`). Dark.
* Agent model pin validation (`agent.model.pin.{cold_registry,corrected,
  unservable}`): an agent file's declared model is checked against the served
  registry and corrected or refused.
* Workflow internals: a **user-park vs system-pause** distinction
  (`workflow.invoke.system_resume_refused_user_park`,
  `workflow.node.user_stop_paused`/`user_pause_paused`, `stopInitiator`) — a
  user-parked run is no longer auto-resumed by system resumes; a
  routing/reconcile layer (`workflow.route.*` with a hydration store);
  `WorkflowNotFoundError`, `WorkflowSessionRootlessError`; and per-event
  telemetry names (`workflowRun.*`, `workflowStep.*`, `workflowPlan.update`
  for `steps_queued`). **None of it changed the event payloads (§ 7).**
* `syntheticUserMessageReason` and a `refusal{category, explanation,
  recommendedModel}` block appear in the persisted message `_meta.kiro`
  schema — the host-side `model_fallback` experiment's substrate (§ 4).
  Not observed on the wire.
* Removed literal: `session/set_model` (SDK table), `update_workflow.error`.

### 3b. 2.22.0 hop (0.63.3 → 0.66.0)

* **`stall_tool_continuation`** — new flag, default **true**, env
  `KIRO_FEATURE_STALL_TOOL_CONTINUATION_ENABLED`. When the stream-idle
  watchdog fires *after a complete trailing tool block and before the
  stopReason frame* (`lastToolReceivedStop && toolBlockIsTrailing &&
  !stopReason`), the turn is completed with continuation `stallToolComplete`
  and a `_kiroStallToolContinuation:{idleMs}` marker chunk, so the tool runs
  and the agent continues. Caps: 3 consecutive / 10 per turn; exhaustion
  throws the `StreamIdleTimeoutError` subclass with message *"Model stream
  stalled at a complete tool block N times in this turn (no data received for
  Xms each); giving up instead of continuing again"*. This is the
  "[V3] A turn that stalls after a complete tool call now runs the tool and
  continues" note. Live status: § 10.
* **`largeToolOutputHandler` removed** (2 → 0 occurrences, gone from the
  `initialize._meta.kiro.settings` schema): the large-output handler is now
  unconditional. Threshold 30,000 chars, 500-char head/tail preview, applies
  to `execute_bash`, `get_process_output`, `web_fetch`, `remote_web_search`,
  `invoke_sub_agent`, `orchestrate_subagent` and every `mcp_*`/`subagent_*`
  tool. Cyril never sent the key. Live shape: § 8.
* **Session activity tracker** (`session.activity.*`, 0 → 11): per-session
  `{status, activity:{turnActive, runningWorkflows, pausedWorkflows}}`
  snapshots emitted as `session_info_update{kind:"focus_update"}` on every
  transition, merged into the roster status behind `_kiro/sessions/changed`,
  fed by execution begin/end events and workflow lifecycle events, and
  **rehydrated from the workflow state directory on session load**
  (`workflow.activity.rehydrate_scan`) so a resumed parent session learns its
  non-terminal runs. Live shape: § 6, § 7.
* `mom_config` — new experiment-only flag, default `"disabled"`. Name only.
* `cloudConfig.pull.silent` — the pull card is emitted only when the outcome
  is `served`/`unknown`; `notEnabled`/`none` are silent. Live: § 6.
* Removed: the `*RetryRecovered` / `*RetryExhausted` metric-name literals
  (the four retry flags themselves remain, defaults unchanged) and
  `WorkflowAgentNotFoundError`.

## 4. Static — Rust host

**Rollout registry: 17 → 18 entries, three changed.**

| experiment | 2.21.2 | 2.22.0 |
|---|---|---|
| `model_fallback` | — | **new**, 0 % internal nightly: *"Moving a turn to another model when the one it is using refuses it or is over capacity."* |
| `v3_prompt` | 0 % | **5 % internal** — the V3 ease-in prompt is now offered to internal users |
| `v2_non_interactive` | 10 % | **50 %** |

`cloud_config`, `session_dashboard`, `remote_sandbox`, `tangent`, `voice` stay
at 100 % all users; `workflows` and `memory` stay internal-only.

**Host env tokens** (strict-boundary counts, glue noise discarded): new
`KIRO_SKIP_BINARY_PINNING` (the changelog item; `pinned_bin` module is
pre-existing — sessions pin the binary they started with, the env disables it),
`KIRO_REQUIRE_MCP_STARTUP_TIMEOUT_SECS` (companion to the
`--require-mcp-startup` fix), and a `KIRO_TEST_MAX_*`/`KIRO_TEST_HEAP_SNAPSHOT_DIR`
family behind the "reduced memory growth" fix. `KIRO_KAS_SERVER_PATH`,
`KIRO_KAS_NODE_PATH` and `KIRO_ACP_RECORD_PATH` all survive (the raw diff
showed them "removed" only because the glued string they sit in was re-cut).
A `Qwen3` model-name literal is present in both 2.21.2 and 2.22.0.

### 4b. Live — `KIRO_ROLLOUT_FORCE_INTERNAL` works, but only where the registry is consulted

Both override names live in `kiro-cli-chat`'s `chat_cli::rollout` module (the
`kiro-cli` launcher has zero hits) and the result of `Rollout::enabled_features`
is exported to the TUI child as a JSON `KIRO_ENABLED_FEATURES` array plus
per-feature `KIRO_<NAME>_ROLLOUT_ENABLED` flags, so the observable is the child
process environment. `probe-rollout-force-internal-2.22.0.py` spawns each path
under an isolated `HOME`, completes the handshake (or waits for the TUI to
paint), then walks the process tree and dumps every `KIRO_*` variable. Eleven
legs, no prompts sent.

| path | override | `KIRO_ENABLED_FEATURES` | `INFRA_SAFETY` / `LITE` flags | ACP handshake |
|---|---|---|---|---|
| `kiro-cli chat` (TUI) | none | `voice, remote_sandbox, tangent, remote_changelog, cloud_config, session_dashboard` | 0 / 0 | — |
| `kiro-cli chat` (TUI) | `FORCE_INTERNAL=1` | **+ `lite`, `infra_safety`, `workflows`** | **1 / 1** | — |
| `kiro-cli chat` (TUI) | `FORCE_INTERNAL=1` + `FORCE_NIGHTLY=1` | same as internal-only | 1 / 1 | — |
| `kiro-cli chat` (TUI) | `FORCE_NIGHTLY=1` only | unchanged | 0 / 0 | — |
| `kiro-cli chat` (TUI) | `FORCE_INTERNAL=true` or **`=0`** | same as `=1` | 1 / 1 | — |
| `kiro-cli acp --agent-engine kas` | none / internal / both | **not exported at all** (node child gets only `CLOUD_CONFIG_ENDPOINT`) | — | 206 = 206 paths, 5 = 5 commands, identical |
| `kiro-cli acp` (v2) | none / both | not exported | — | 75 = 75 paths, 25 = 25 commands, identical |

* **It works, and it is presence-tested.** Any value, including `0`, forces
  the internal segment. Every `segment: internal, channel: any` row flips:
  `lite`, `infra_safety`, `workflows`. The bucketed rows (`tui` 50 %,
  `v3_prompt` 5 %) did not land for this user's hash and `model_fallback` is
  0 %, so nothing can be said about them from one account.
* **`KIRO_ROLLOUT_FORCE_NIGHTLY` produced no observable change.** The two
  `channel: nightly` rows (`c2s`, `memory`) never appeared, alone or combined,
  so either nightly is decided from the build's version string rather than
  the env, or those two are exported through a path this probe does not see.
  Treated as not working until a nightly build is examined.
* **The telemetry flag is separate.** `KIRO_TELEMETRY_IS_INTERNAL_AMAZON`
  stays `false` under every override, and `KIRO_INTERNAL` (which the TUI
  tests as `=== "1"`) is never exported on this account. The override changes
  rollout bucketing, not identity.
* **Cyril's path is unaffected.** On both `acp` paths nothing rollout-related
  reaches the engine child and the handshake, command list and every frame
  path are byte-identical with and without the override. The registry is a
  TUI-and-launcher concern; the ACP server never consults it.

**A second table sits right after the registry: a per-user cohort override.**
`chat_cli::rollout::FeatureOverride {treatment, control}` is a compiled-in
`HashMap<feature, {treatment: [sha256…], control: [sha256…]}>`. In all three
releases it holds exactly one entry — `v3_prompt` with **135 hashed
identifiers in `treatment`** and none in `control`, unchanged 2.21.2 → 2.22.0.
Those users were being offered the V3 ease-in prompt while its
`treatment_percent` was still 0 %. This account's telemetry user id and
client id hash to none of them. Which identifier is hashed was not
determined.

**Embedded TUI bundle:** the set of `onExtNotification("…")` registrations is
**14 = 14 = 14** and the quoted `_kiro`/`kiro.dev` literal set is 82 = 82 = 82
across 2.21.2 / 2.21.4 / 2.22.0 — no new first-party consumer of any KAS
frame. The two embedded doc manifests carry identical `generated_at`
(2026-09-02, 103 docs; 2026-08-20, 139 docs) in 2.21.2 and 2.22.0, so the
doc-manifest lane reports no delta.

## 5. Release notes → evidence

| note | engine | evidence |
|---|---|---|
| Large tool results always saved with a preview | V3 | § 3b, **§ 8 live** |
| Turn that stalls after a complete tool call continues | V3 | § 3b flag; **§ 10 live: window not reached** |
| Cloud-config pull card only when sync is enabled | V3 | **§ 6 live**: pre-`session/new` `fetch_cloud_config` frame absent on 0.66.0 |
| `--require-mcp-startup` in non-interactive runs | V3 host | env token only; not exercised |
| Proxy auth error instead of "Internal error" | V3 | `ProxyAuth` 1 → 2 literals; not exercised |
| Custom agents told their cwd; `/context` steering listing; symlinked `file://` prompt | V3 | system-prompt/host-side; invisible on ACP |
| `/fullscreen`, `/sessions` refresh, settings navigation, compaction UI, Ctrl+O, image mislabel, memory growth | TUI | not on the ACP wire |
| `KIRO_SKIP_BINARY_PINNING` | host | § 4 |
| 2.21.4: session search scope, `--v2` flag, settings keyboard, dead-modal dismissal, tangent indicator | TUI/host | 0.63.3 wire-identical to 0.58.7 (§ 6, § 7, § 11) |

## 6. Live — v3 paired turn sweep

`probe-kas-turn-sweep-2.22.0.py` (`SCENARIO=turn`), the 2.21.2 workload
(prompt → file-read tool call → `end_turn`), one leg per bundle plus a repeat
of 0.66.0:

| leg | paths | stop | turn | `session_info_update` frames |
|---|---:|---|---:|---:|
| 0.66.0 run a | 286 | `end_turn` | 11.7 s | 14 |
| 0.66.0 run b | 292 | `end_turn` | — | 14 |
| 0.63.3 | 290 | `end_turn` | — | 12 |
| 0.58.7 | 290 | `end_turn` | — | 12 |

**0.63.3 ≡ 0.58.7 (290 = 290, identical sets).** The whole 2.21.4 KAS hop,
SDK bump included, produced zero wire-shape change on this workload.

**0.66.0 vs 0.58.7 — five new paths, all one family:**

```
+ params.update._meta.kiro.activity
+ params.update._meta.kiro.activity.pausedWorkflows
+ params.update._meta.kiro.activity.runningWorkflows
+ params.update._meta.kiro.activity.turnActive
+ params.update._meta.kiro.focus.status
```

Per turn 0.66.0 adds two `focus_update` frames — `{status:"in_progress",
activity:{turnActive:true, runningWorkflows:0, pausedWorkflows:0}}` immediately
before `turn_start`, and `{status:"idle", activity:{turnActive:false,…}}`
immediately **after** `turn_end`. `turn_end` remains the terminal signal and
its position relative to the prompt response is unchanged.

**Paths only under the pins (9)** are two known effects, not regressions:
(a) `_meta.kiro.toolId`, `rawOutput.kind`, `rawOutput.retracted` — the
launcher-path `fetch_cloud_config` tool call that 0.58.7/0.63.3 emit before
the `session/new` response and **0.66.0 no longer emits** for this
non-entitled account (`rawOutput.kind` was `notEnabled` on the pins; 0 frames
on both 0.66.0 legs and both 0.66.0 workflow legs). (b) the six
`configOptions[].options[]._meta.kiro.{defaultEffortLevel,…}` effort paths —
absent from run a's `session/new` result and **present in run b**, the
arrival-timing nondeterminism the 2.21.2 audit documented; run a's
`session/new` result in fact omitted the `model` option entirely (3 options
vs 4) and it arrived via `config_option_update`, exactly the 2.19.0 "LATE via
update" shape. **A single 0.66.0 sample would have reported both a missing
frame and a missing `model` option.**

`initialize` and `session/new` result values are otherwise identical across
builds: `agentCapabilities._meta.kiro` keys, `_meta` session flags,
`modes` (7), and the `_kiro/sessions/changed` frame sequence (5 per turn,
`idle → in_progress → idle`, titles included).

## 7. Live — workflows: turn and artifact handling between steps

`probe-kas-workflow-channels-2.22.0.py` (the 2.21.1 two-step recipe: s1 writes
`{{token}}` to a file and either restates it or stops terse; s2 reports
`{{s1.output}}` and the `{{artifacts.value}}` file), four legs:

| leg | status | events | capturedOutputs | channel A (template) | channel B (file) | run |
|---|---|---:|---|---|---|---:|
| 0.66.0 restate | completed | 8 | `{s1:"ALPHA", s2:"DONE"}` | `ALPHA` ✓ | `ALPHA` ✓ | 14.9 s |
| 0.58.7 restate | completed | 8 | same | `ALPHA` ✓ | `ALPHA` ✓ | 15.5 s |
| 0.63.3 restate | completed | 8 | same | `ALPHA` ✓ | `ALPHA` ✓ | 14.8 s |
| 0.66.0 terse | completed | 8 | **`{s1:"Done.", …}`** | wrapper copied verbatim | `ALPHA` ✓ | 17.0 s |

**Unchanged on every build:** event kinds and counts (`run_start`, `node_start`
×4 — still double per node, `node_complete` ×2, `run_complete`), every
payload key set (`node_start`, `node_complete`, `run_complete.finalState`
including `rootConversationId`, `planRevision`, `completionSignalSource`),
per-step ordering `turn_completion → turn_end → node_complete` in the same
millisecond, `parentSessionId` stamped on every event, and the artifact
registry (`finalState.artifacts.value` correct 4/4). **cyril-srp6 stands**:
the terse step still captures the sign-off `"Done."` under
`status:"completed"`.

**Field sweep across the workflow traces:** 0.63.3 ≡ 0.58.7 (418 = 418);
0.66.0 adds the same five `activity`/`focus.status` paths and drops the same
three `fetch_cloud_config` paths as § 6 — nothing workflow-specific.

**What the new activity frames look like on a run (0.66.0 only):**

```
@1.15 [PARENT] focus_update {status:"in_progress", activity:{turnActive:false, runningWorkflows:1, pausedWorkflows:0}}
@1.22 [s1]     focus_update {status:"in_progress", activity:{turnActive:true,  runningWorkflows:0, pausedWorkflows:0}}   then turn_start
@6.92 [s1]     focus_update {status:"idle",        activity:{turnActive:false, …}}   then turn_completion, turn_end, node_complete
@7.03 [s2]     … same pair …
```

The parent session is told, by the engine, that a workflow is running under it
(`runningWorkflows:1`) with no turn active — the first time that fact has
been pushed rather than derived client-side. No trailing parent `idle` frame
was captured because the probe exits on `run_complete`; whether one follows is
unmeasured. Step sessions keep the pre-existing title-only `focus_update`
(`"audit-channels-2.22.0 · s1"`) as well.

**`{{id.output}}` is injected wrapped, and has been since at least 2.20.1.**
The terse leg made it visible: s2 saw
`<prior_step_output_69c8c6ccc034abb4 id="s1">\nDone.\n</prior_step_output_…>`
and copied it verbatim. The resolver (`prior_step_output_${frameNonce}`, nonce
≥ 8 hex, nested `prior_step_output` tags entity-escaped) is present in all
three bundles and in every committed 2.20.1/2.21.0/2.21.1 trace (2 occurrences
each — the rendered s2 prompt). `{{artifacts.name}}` is **not** wrapped. Not a
2.22.0 change; recorded because the restate legs hid it (the model unwrapped
it) and the kiro-workflow-authoring skill documents bare interpolation.

## 8. Live — large tool results are always offloaded

`probe-kas-turn-sweep-2.22.0.py` (`SCENARIO=large`): the shell tool runs
`seq 1 20000` (108,916 chars), four legs — each bundle with cyril's
`terminal:true` and with the capability omitted (KAS runs the shell itself).

| | 0.58.7 | 0.66.0 |
|---|---|---|
| `rawOutput.message` (what the model sees) | `Output:\n…[truncated 29075 chars]…\n\nExit Code: 0` (30k cap) | `The output from "execute_bash" (108,916 chars) exceeded the 30,000-char limit; here is a preview of the tool's result:` + `--- HEAD (first 500 chars) ---` … `--- 107,916 chars omitted ---` … `--- TAIL (last 500 chars) ---` … `The full output was saved to a file under '<HOME>/.kiro/sessions/<ws>/<sess>/tool-outputs/execute_bash-f8e976e7.txt'. Inspect it with targeted shell commands…` |
| file on disk | none | `tool-outputs/execute_bash-<hash>.txt`, **108,916 bytes = the complete output** |
| transcript `messages.jsonl` | 61.7 KB (capped output inline) | 26.9 KB (preview only) |
| `rawOutput.output` (wire) | 1,032 chars: head 500 + `...[truncated 107894 chars]...` + tail 500 | identical |
| `content[0].text` (wire, terminal omitted) | 1,032-char excerpt of the JSON-stringified rawOutput; tail = end of the output + `Exit Code: 0` | 1,032-char excerpt; **tail = the offload instructions** |
| `content` (wire, `terminal:true`) | `[{type:"terminal", terminalId}]` | identical |

Two corrections to standing knowledge fall out: the "30k wire cap" behind
cyril-7cnh is really two mechanisms — a 30,000-char cap on the **model-facing
message** and a separate ~1 KB head/tail **wire excerpt** on `rawOutput.output`
and `content.text` that both builds apply; and 0.66.0 keeps the wire excerpt
but replaces the model-facing message with the preview. With `terminal:true`
(cyril's configuration) only `rawOutput.message` changes; cyril's
`output_text()` never reads `message`, so nothing renders differently.
The `terminal:true` legs also show a **probe artifact**: the probe's
`terminal/output` handler read the pipe only after `wait_for_exit`, so the
64 KB pipe buffer stalled `seq` at 65,559 chars and the 60 s wait killed it
(`Exit Code: -1`, 71 s turns). That is the harness, not KAS; the
terminal-omitted legs are the clean measurement.

## 9. Live — the SDK 1.x bump inverted two error codes

`probe-kas-sdk-methods-2.22.0.py` (no prompt sent) after `session/new`:

| method | 0.58.7 | 0.66.0 |
|---|---|---|
| `session/set_model {modelId: claude-opus-5}` | **-32601** `"Method not found": session/set_model` | **-32603** `Internal error`, data.details `[PersistenceClassification] Ext method "session/set_model" has no persistence classification…` |
| `session/set_model {modelId: bogus}` | -32601 | -32603 (same detail) |
| `session/set_config_option {model}` | OK `{configOptions}` | OK `{configOptions}` |
| `session/list` | OK `{sessions}` | OK `{sessions}` |
| `session/delete {unknown id}` | **-32603** persistence classification | **-32601** `"Method not found": session/delete` |
| `$/cancel_request` | -32603 persistence classification | -32603 persistence classification |

The mechanism is the SDK table: a method the SDK knows but KAS does not
implement is refused with a proper `-32601`; a method the SDK does not know
falls through KAS's ext-method router, whose first gate is the persistence
classifier, producing `-32603` with a leaked internal filename. `session/
set_model` moved from the first class to the second and `session/delete` the
other way. Two standing facts need amending: the KAS gate-surface note "zero
`-32601` across the whole surface, so a client cannot feature-detect" is now
only true for **ext** methods, and any client that keys a fallback on `-32601`
for `session/set_model` would silently stop falling back on 0.66.0.

**Cyril impact: none today.** `domain_mediator/commands/session.rs:284` does
send `session/set_model` untyped, but nothing in production constructs
`BridgeCommand::SetModel` (only tests; `cyril-workbench` uses
`SetConfigOption`), and the model path on KAS is `session/set_config_option`,
which works on both builds. The mediator's error handling surfaces any error
as a `BridgeError` regardless of code. The `MethodNotFound` special-cases in
`domain_mediator/commands/subagents.rs` target `_kiro.dev/subagent/*`, which
this release does not touch.

## 10. Live — stall-tool continuation: gate proven, window not reached

`probe-kas-stall-continuation-2.22.0.py` collapses the watchdog with
`KIRO_STREAM_IDLE_WARN_MS` / `KIRO_STREAM_IDLE_TIMEOUT_MS` on the file-read
workload, three timeouts × two builds:

| timeout | 0.58.7 | 0.66.0 |
|---|---|---|
| 1500 ms (warn 100) | `end_turn`; 2 soft `_kiro/system/notify` | `end_turn`; 1 soft notify |
| 700 ms (warn 100) | `end_turn`; 2 soft notify | `end_turn`; 1 soft notify |
| 250 ms (warn 60) | **-32000 `StreamIdleTimeoutError`**, 11 notify (paused ×, "connection interrupted. Retrying" ×), `tool_call_update:failed[Read File]` ×3, `display_error`, `turn_completion`, `turn_end` | **-32000 `StreamIdleTimeoutError`**, 6 notify, `tool_call_update:failed` ×2, `display_error`, `turn_completion`, `turn_end` |

At 250 ms the stream stalled with `emissions.tool:true, streamPosition:
BeforeContent` on both builds — **inside** the tool block, not after a
complete one — so 0.66.0 took the ordinary `stall_retry` → `stream_error_retry`
→ exhausted path exactly like 0.58.7 (KAS log: `q.converse.stall_retry.exhausted
attempts:2`, no `stall_tool_continuation` line). The continuation's trigger
(`lastToolReceivedStop && toolBlockIsTrailing && !stopReason`) is the gap
between a tool block's stop marker and the stream's stopReason, which on a
healthy backend is milliseconds and was never the longest gap in any leg.
**The feature is static-only.** What the legs did establish: the turn
terminates on both builds at every threshold (`turn_end` then `-32000`, the
2.20.1 contract), a stalled tool block surfaces as `tool_call_update
status:failed` on the wire on both builds, and 0.66.0 sends fewer soft
warnings per turn (1 vs 2 at 700/1500 ms; n = 2 each).

## 11. Live — v2 paired sweep

`probe-v2-turn-sweep-2.22.0.py` against the archived 2.21.2, 2.21.4 and
2.22.0 launchers (each spawning its own `kiro-cli-chat`): **118 = 118 = 118
paths, identical sets**, identical frame families (`agent_message_chunk`,
`tool_call`, `tool_call_update`, `_kiro.dev/metadata` ×4,
`_kiro.dev/commands/available`, `_kiro.dev/session/update`,
`_kiro.dev/subagent/list_update`), `end_turn` in 3.1–3.3 s, 19 models with
`currentModelId: auto` and the same id list on all three. The v2 engine did
not change on the ACP wire across the two releases.

## 12. Cyril impact

**No code change is required by 2.21.4 or 2.22.0.**

* **`focus_update` activity block (§ 6, § 7):** cyril's
  `session_info_to_notification` matches kinds exactly and returns `None`
  for `focus_update`, so the new frames are dropped — silently, the
  cyril-58uv class. Nothing regresses because `turn_end` is untouched. It is,
  however, the engine-pushed per-session busy / running-workflows /
  paused-workflows state the W-track renderer (cyril-zd8u) and the approvals
  queue (cyril-z4eo) would otherwise derive. Filed **cyril-otpi**.
* **Large-output offload (§ 8):** no rendering change under `terminal:true`;
  `output_text()` ignores `message`. The full output now exists on disk at a
  path the frame names. Filed **cyril-f8w6**; cyril-7cnh's premise corrected
  in a note.
* **Error-code inversion (§ 9):** no production caller of
  `session/set_model`; the mediator does not branch on code. Recorded on the
  gate-surface memory; no issue.
* **`fetch_cloud_config` gated (§ 6):** cyril's direct-node spawn never saw
  the frame (cyril-0b8t); the launcher-path premise behind the closed
  cyril-68ag is now version-conditional. Noted on both.
* **Workflows (§ 7):** tracker, converter and the `{{id.output}}` doctrine
  are unaffected; the wrapper finding is documentation for the
  kiro-workflow-authoring skill and cyril-srp6, not a code change.
* **SDK bump (§ 3a):** no emitted `session/update` variant changed, so
  cyril-ai1y's latent hard-fail is not triggered; `plan_update`,
  `plan_removed`, `usage_update` exist only in KAS's schema table.

## 13. Coverage boundary

The stall-tool continuation was not reached live (§ 10); its wire shape when it
does fire — whether anything beyond the ordinary tool frames reaches the client
— is unverified. `--require-mcp-startup`, the proxy-auth error, `tool_load`
(default off), `model_fallback` (0 %), and the user-park semantics of
workflow pause/resume were not exercised. The parent session's trailing
`focus_update` after `run_complete` was not captured. The v3 legs pinned three
KAS builds under one host on one same-day backend, isolating the KAS axis; the
host axis for v3 was not paired (cyril spawns `node acp-server.js` directly, so
host-side v3 changes do not reach it). Frame families never driven — plans,
crews, cancellation, compaction, `repeat`/`watch`/`parallel` nodes — remain
outside the swept path sets.

## 14. Artifacts

Live probes and captures (`experiments/conductor-spike/`):

* `probe-kas-turn-sweep-2.22.0.py` — `SCENARIO=turn|large`, `TERM_CAP=on|off`,
  `KAS=` pin; captures `kas-turn-sweep-{0660a,0660b,0633,0587}-2.22.0.jsonl`,
  `kas-large-{0660,0587}[-noterm]-2.22.0.jsonl`.
* `probe-kas-workflow-channels-2.22.0.py` — `LABEL`, `WF_STYLE`,
  `KIRO_KAS_SERVER_PATH`; captures `kas-workflow-channels-{0660-restate,
  0587-restate,0633-restate,0660-terse}-2.22.0.jsonl` + `-verdict.json`.
* `analyze-workflow-trace-2.22.0.py` — event vocabulary, payload keys,
  per-session `session_info_update` ordering, activity frames, capture verdict.
* `probe-kas-sdk-methods-2.22.0.py` — `kas-sdk-methods-{0660,0587}-2.22.0.jsonl`.
* `probe-kas-stall-continuation-2.22.0.py` — `WD_TIMEOUT`/`WD_WARN`;
  `kas-stall-{0660,0587}-{1500,700,250}-2.22.0.jsonl`.
* `probe-v2-turn-sweep-2.22.0.py` — `v2-turn-sweep-{2.21.2,2.21.4,2.22.0}-2.22.0.jsonl`.
* `probe-rollout-force-internal-2.22.0.py` — `MODE=acp-kas|acp-v2|tui`,
  `FORCE_INTERNAL` / `FORCE_NIGHTLY`; outputs `rollout-force-*-2.22.0.json`
  (per-user identifiers redacted).
* Sweeps via `sweep-new-fields.py` in inventory and `--diff` mode.

Static scripts and outputs: `static-kas-surface-2.22.0.py` (N-tree census),
`static-kas-identifiers-2.22.0.py` (regex literal diff), `static-rollout-2.22.0.py`,
`static-host-paths-2.22.0.py`, `extract-kas-feature-flags.py`; outputs under
`static-2.22.0/` (surface, identifiers delta, rollout + delta, host paths
delta, flag tables, changelog and help captures).

Bundles compared in place from the self-extracted trees under
`~/.local/share/kiro-cli/kas/{2.21.2-…,2.21.4-…,2.22.0-…}/` (0.58.7, 0.63.3,
0.66.0); binaries under `~/.local/share/kiro-research/binaries/{2.21.4,2.22.0}/`.

Captures were checked for unredacted `accessToken`, `refreshToken`, `idToken`,
`clientSecret`, `profileArn` and `arn:aws:codewhisperer` values before commit;
auth keys carry `<REDACTED>`. Bounded check, not a general secret scan.

# kiro-cli 2.24.0 ACP audit (delta from 2.22.0, covering the unaudited 2.23.0 and 2.23.1)

**Audit status:** complete for the stated paired scope. The installed binary
was already 2.24.0 when the audit began. **2.23.0 (2026-09-21) and 2.23.1
(2026-09-23) were never audited**, so this document covers the three-release
hop 2.22.0 → 2.24.0 and attributes each finding to the release that introduced
it. Emphasis is the KAS (v3) engine per the request; the v2 lane was run
because it is cyril's default path, and it produced the only regression.
Probes isolated `HOME` to protect real `~/.kiro` state (verified afterwards:
`~/.kiro/settings/cli.json` untouched).

**Conclusion: ONE SILENT REGRESSION on cyril's default (v2) engine; KAS is
SAFE with four new opportunities.**

* **v2, since 2.23.0 — the effort badge is dead.** `_kiro.dev/metadata` no
  longer carries top-level `effort`; it moved into a new
  `reasoning{support, thinkingEnabled?, effort?, effortLevels}` block (§ 10).
  Cyril reads only top-level `effort` (`convert/kiro.rs:452`) and the metadata
  `Set` arm is the *only* writer of the badge (`cyril-ui/src/state.rs:591`), so
  on 2.23.0+ the badge never appears. No error, no crash — the new key is
  logged at `debug` as unrecognized. Filed as a P1 bug.
* **KAS 0.66.0 → 0.66.8 (the 2.23.0 hop; byte-identical across 2.23.0 /
  2.23.1 / 2.24.0).** The field sweep finds **exactly two new families**,
  replicated ×2 and on the workflow workload (§ 6): a per-model **`thinking`
  configOption** with `_meta.kiro.{thinkingToggleable, defaultThinkingEnabled}`
  on model options, and **`sessionCapabilities.delete`** (`session/delete` is
  now implemented). Live-confirmed new client levers: **`rejectionReason`** on
  `reject_once` reaches the model (§ 7.4), and the inbound
  **`_kiro/terminal/settings_changed`** notification sets a shell command
  timeout (§ 7.3). No new `session/update` variant, no new agent→client ext
  method, workflow vocabulary unchanged (`node_failed` is a relay-only literal,
  never emitted — § 8). Every KAS deserialization path is tolerant of the new
  shapes (§ 9).
* **Methodology correction (§ 2):** the `kiro-cli` launcher resolves
  `kiro-cli-chat` **via `PATH`**, not as its own sibling. An archived launcher
  silently spawns the *installed* chat binary. The first v2 pairing in this
  audit compared 2.24.0 with itself (121 = 121, "identical") and missed the
  regression; **the 2.22.0 audit's "v2 118 = 118 = 118" was run the same way
  and is void.** Fixed in `probe-v2-turn-sweep-2.24.0.py` (prepends the
  archive's `bin/` and records the child's `/proc/<pid>/exe`).

**Scope.** Compare the released x86_64 Linux `kiro-cli` 2.24.0 artifact with
the archived 2.22.0 and the newly fetched 2.23.0 / 2.23.1 artifacts across the
v2 ACP engine, the embedded KAS runtimes, the Rust host rollout registry and
the embedded TUI bundle. Every v3 leg runs through the **installed 2.24.0
host** with `KIRO_KAS_SERVER_PATH` pinning the bundle (0.66.0 from a
scratchpad snapshot, 0.66.8 from the carved tree), isolating the KAS-binary
axis against one same-day backend.

## 1. Acquisition and integrity

S3-direct `/latest/manifest.json` reported `2.24.0`, Linux x86_64 headless
`tarXz` SHA-256 `e4ba8862a7a480b508dbdcf4655d34df1c3f03b1a7acc2115a308cd01e8082a6`
(136,579,196 bytes) — verified, and the extracted binaries are byte-identical
to the installed `~/.local/bin` ones. 2.23.0 / 2.23.1 tarballs fetched by URL
(no versioned manifest exists); `BUILD-INFO` confirms each. All three archived
under `~/.local/share/kiro-research/binaries/<ver>/`.

| release | `BUILD_HASH` | `kiro-cli` | `kiro-cli-chat` | `kiro-cli-term` | tarball |
|---|---|---:|---:|---:|---:|
| 2.22.0 | `b5aa4876…` (2026-09-15) | 113,901,280 | 462,078,288 | 86,961,584 | 292.7 MB |
| 2.23.0 | `82fac3b0…` (2026-09-21) | 43,781,472 | 211,734,504 | 34,027,936 | 136.2 MB |
| 2.23.1 | `c1c98028…` (2026-09-22) | 43,781,216 | 211,795,880 | 34,028,192 | 136.2 MB |
| 2.24.0 | `240642df…` (2026-09-23) | 43,786,080 | 213,013,224 | 34,029,152 | 136.6 MB |

**The binaries are stripped from 2.23.0** ("Smaller CLI installation size"):
no `.symtab`/`.strtab`, no `.debug_*`; `.text` is unchanged (32.0 MB launcher).
**The `nm` + `rustfilt` module-path lane is dead.** Substitute: the
`crates/<crate>/src/*.rs` source-path strings (tracing + panic locations)
survive stripping (chat 360 → 376 paths, `static-host-srcpaths-delta.json`).

**`kiro-cli-chat` `.rodata` 368 → 132 MB: every embedded payload is now a
zstd frame** (2.22.0 had them raw, KAS as gzip). **The gzip carve recipe in
the methodology memory no longer works.** New layout (2.24.0):

| offset | payload |
|---|---|
| 6.84 MB | KAS bundle tar (`@kiro/agent` 0.66.8) — first `28 b5 2f fd` frame |
| 33.8 MB | Node.js v22.22.2 ELF — runs KAS |
| 65.8 MB | Bun v1.4.2 ELF — runs the TUI (2.22.0: Bun v1.3.13) |
| 95.7 MB | TUI bundle (13.27 MB) |

Carve: `off=$(first zstd magic)`; `tail -c +$((off+1)) kiro-cli-chat | zstd -dc | tar -x`.
The carved 2.24.0 tree is identical (`diff -rq` empty) to the self-extracted
`~/.local/share/kiro-cli/kas/2.24.0-0f0c0e9a…/`. The KAS tree also shrank
355 → 173 MB: `onnxruntime-web` removed entirely, `@huggingface` 48 → 4 MB,
third-party `.d.ts`/`.map` pruned. The `@kiro/agent/dist/server/*.d.ts`
readable island is intact and unchanged.

Embedded changelog: 2.24.0 carries 2.24.0 + 2.23.1 only; **2.23.0's own
binary carries its 33-entry 2.23.0 changelog**, which 2.24.0's history lacks
(`static-changelog-2.23.0.txt`). Doc manifests identical (2026-09-02/103,
2026-08-20/139). `--help-all`, `acp --help`, `chat --help` identical but for
one reordered option.

## 2. Methods and evidence discipline

* Static leads count-checked in both bundles before being called new. Two
  static tool results were **false positives** caught this way: the flag
  extractor reported "0 env-reachable flags" in 0.66.8 because the env map is
  now written `{...dLi, ...fwr()}` — the 11 `KIRO_FEATURE_*_ENABLED` literals
  are all still present; and the host lane predicted `/reasoning` would appear
  in the v2 command list — live, it is executable but not advertised (§ 10).
* Live lanes: self-contained Python raw JSON-RPC probes, temporary `HOME`,
  `XDG_DATA_HOME` at the real share dir, auth redacted, profile ARN cached so
  no probe renews the single-use refresh token concurrently.
* **The 0.66.0 tree was snapshotted to the scratchpad before any 2.24.0 host
  ran**, because 2.23.1 added "stale extracted V3 engine versions are now
  cleaned up": the 2.24.0 host may delete `kas/2.22.0-*` from the shared data
  dir. (2.21.2's extract is already gone; 2.21.4's survives for now.)
* **Launcher PATH trap (new).** Verified by walking the process tree:
  `binaries/2.22.0/.../kiro-cli acp` spawned `/home/.../.local/bin/kiro-cli-chat`
  (the installed 2.24.0); with the archive's `bin/` prepended to `PATH` it
  spawned its own sibling. Every v2 leg below was re-run with the fix and
  records the child exe in its capture (`"probe": "child_exe"`). KAS legs are
  unaffected — they deliberately use the installed host and pin the bundle.
* The v3 turn sweep was run **twice** on 0.66.8 (effort-metadata timing
  nondeterminism); both runs agree exactly.

## 3. Static — KAS 0.66.0 → 0.66.8

| lane | 0.66.0 | 0.66.8 |
|---|---:|---:|
| `acp-server.js` bytes / lines | 11,499,652 / 17,546 | 11,643,974 / 17,692 |
| quoted `_kiro/*` method literals | 110 | **112** |
| `process.env` reads | 65 | 65 |
| `KIRO_*` env literals | 51 | 52 (`+KIRO_SESSION_ID`) |
| snake-case kind literals | 34 | 34 |
| feature flags / env-reachable | 20 / 11 | **21** / 11 (+ derived) |
| `@agentclientprotocol/sdk` | `^1.3.0` | `^1.3.0` |

Only `acp-server.js` and `package.json` changed inside `@kiro/agent`;
`@cedar-policy/cedar-wasm` was rebuilt. 0.66.8 is **byte-identical** in the
2.23.0, 2.23.1 and 2.24.0 bundles, so every KAS change is a 2.23.0-hop change
and the 2.24.0 `[V3]` notes are host/TUI-side (§ 4).

New method literals:

* **`_kiro/terminal/settings_changed`** — *client→agent notification*
  (`{terminal:{enabled, commandTimeoutMs: 1000..1800000}}`, strict schema; the
  `enabled` flag comes from the `mEe` settings wrapper and **must be true** or
  the timeout is cleared). Also settable at `initialize` via
  `_meta.kiro.settings.terminal`. Feeds a new `getConfiguredCommandTimeoutMs`
  hook in the tool context. Persistence class `localOnly`. Live: § 7.3.
* **`_kiro/workflow/node_failed`** — appears once, in the set
  `{node_complete, node_paused, node_failed}` that the sandbox/remote step
  router passes to `noteLifecycleFrame`. **There is no emitter**: a failed
  step is still `node_complete{status:"failed"}` (`emitNodeComplete`). A
  forward-compat relay literal; cyril's `WorkflowTracker` needs nothing. Live: § 8.

Other additions (identifier delta `static-kas-identifiers-delta.json`, +291/−6):

* **`session/delete` implemented** (`acp.session.delete.*`, workflow cascade:
  deleting a session deletes the step sessions of its workflows,
  `SessionOwnsLiveWorkflowsError`, `WorkflowStepSessionInUseError`,
  `purgeSessionDirectory`). Advertised as `sessionCapabilities.delete`. Live: § 7.2.
* **Thinking toggle.** `thinkingToggleable` / `defaultThinkingEnabled` on the
  model registry; new configId `thinking` (`on|off`, category `"thinking"`)
  emitted only for toggleable models; `thinking=off` caps an `xhigh`/`max`
  effort down to the highest non-`{xhigh,max}` level
  (`session.effort.capped_for_thinking_off`); choosing an `xhigh`/`max` effort
  forces thinking back on. Thinking-off maps to `thinking:{type:"disabled"}`
  and on to `{type:"adaptive"}` in the model request. Live: § 7.1.
* **`rejectionReason`** — `session/request_permission` responses may carry
  `_meta.kiro.rejectionReason` (string, ignored for `reject_always`); it is
  sanitized and passed as the rejection reason. Live: § 7.4.
* **`KIRO_SESSION_ID`** — the root conversation id, now exported to tool
  children (appears in `terminal/create.env`, § 7.3).
* **Tool-schema sanitization** (`tool.schema.*`: collapse, relocate `$ref`s,
  unresolvable local references) — MCP tool schemas are normalized before the
  model sees them. Model-facing; not on ACP.
* Retry/recovery bookkeeping: `AgentExecutionRecovered`,
  `interruptedTool`, `toolBlockComplete|MidArgs|Unknown` (stall classification),
  `summarization.output_truncated.retry*`, `turnDurationMs.recovered`. No new
  wire frame — the `session_info_update` kind census is 34 = 34.
* `memory.secretScan.*` + an embedded gitleaks ruleset — the dark external
  memory (AB_MEMORY_EXTERNAL) now scans for secrets before persisting.
* Workflow: `workflow.new.original_user_request_captured` (the parent's
  original request is captured into a run; a recipe can disable it), step
  session close on completion (`workflow.step.close*`), session-write
  admission deferral. Payload keys unchanged live (§ 8).
* Flags: new `companion_shadow_mode` (default false, **no consumer** — name
  only). New `defineExperimentGate` path derives the env var name at runtime
  (`KIRO_FEATURE_${KEY}_ENABLED`) — so **future env levers may not exist as
  literals**; today it registers only `unifiedAgent`
  (`KIRO_FEATURE_UNIFIED_AGENT_ENABLED` or client setting `unifiedAgent`).
* Telemetry-only labels that look like features but are not wire:
  `mode-fallback`, `model-switch`, `session-title`, `model-settlement`.
* `acceptsModelDisplayName` — `NoResponseError` now names the model
  ("Claude Sonnet 4.6 returned no response").

## 4. Static — Rust host and TUI

Detail: `static-2.24.0/static-host-findings-2.24.0.md` (host static lane).

**Rollout registry 18 → 19.** 2.23.0: **+`local_sandbox`** (100 % internal
nightly — "CLI LocalSandbox testing for internal nightly users on V3"),
`v3_prompt` 5 → 10 %. 2.23.1: `v2_non_interactive` 50 → 75 %, `v3_prompt`
10 → 25 % internal. 2.24.0: none. `model_fallback` still 0 %. The 135-hash
`v3_prompt` FeatureOverride cohort is byte-identical in all four builds.

**Host env tokens 144 → 149, none removed** (a raw diff shows ~75 "removals"
that are TUI names now inside a zstd frame): `KIRO_KAS_REGION` (new
`launch/kas_endpoint_overrides.rs` — KAS endpoint overrides validated: https,
or http on loopback only; krs/cps region must agree — the 2.23.0
"[V3] Honor `api.krs.service`/`api.cps.service`" note),
`KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`, `KIRO_TURN_MARKER_DIR`,
`KIRO_TEST_HOMEBREW_PREFIX`. `KIRO_KAS_SERVER_PATH`, `KIRO_KAS_NODE_PATH`,
`KIRO_ACP_RECORD_PATH`, `KIRO_ROLLOUT_FORCE_INTERNAL` survive with unchanged
counts.

**New host modules:** `chat-cli-v2/src/agent/acp/commands/reasoning.rs` (§ 10),
`agent/util/image.rs` (v2 image downscale; placeholder
`[image removed: rejected by the model provider]`), `agent/util/sanitize.rs`
(exports `KIRO_SESSION_ID` to v2 shell tools),
`amzn-kiro-controlplane-bearer-rust-client` (`get/set_user_preference`,
`list_available_models`), `turn_marker.rs`, `launch/kas_endpoint_overrides.rs`.

**TUI bundle (carved: `~/.local/share/kiro-research/tui-bundles/kiro-tui-{2.22.0,2.24.0}.js`):**

* `onExtNotification` 14 → 15: `+_kiro/sandbox/status` (gated on
  `local_sandbox`). Both sandbox methods already existed in KAS 0.66.0.
* **`/tools trust-all` is TUI-local**: it reuses the pre-existing
  `--trust-all-tools` auto-approve path (select `allow_always`, attach
  `_meta.kiro.consent{capability, scope:"session", …}` on KAS). No new method,
  nothing sent to KAS. Cyril can build the same thing on its own approval
  overlay with zero protocol work.
* **First-party `rejectionReason` consumer**: the TUI sends the user's typed
  denial feedback as `_meta.kiro.rejectionReason` on `reject_once`.
* The TUI now parses the `replayMarking` capability (KAS advertised it and
  marked replayed frames `_meta.kiro.replay:true` since 0.66.0; cyril-99ds).
* `/effort [default]` is display-only; `chat.defaultModel` for non-interactive
  sessions shows no wire change; queued steering is a UI queue (no
  `_session/steer` literal).
* Not consumed by the TUI: `node_failed`, `terminal/settings_changed`,
  `focus_update.activity`.

**Project `.env` fix (2.24.0):** the launcher now passes `--no-env-file` to
the Bun TUI runtime (Bun auto-loads `.env*`; every TUI child — the KAS node
process, MCP servers, tools — inherited it). **Cyril was never exposed**: it
spawns `node acp-server.js` from Rust and Node does not auto-load `.env`.

## 5. Release notes → evidence

| note | release / engine | evidence |
|---|---|---|
| Reasoning settings (thinking and effort) in `/model` | 2.23.0 both | KAS `thinking` configOption **§ 7.1 live**; v2 `reasoning` block + command **§ 10 live** |
| Feedback typed when denying a tool reaches the model | 2.23.0 V3 | **§ 7.4 live** |
| Honor `api.krs/cps.service` overrides | 2.23.0 V3 | § 4 (`kas_endpoint_overrides.rs`) |
| Turn that ends silently after tool use now completes | 2.23.0 | recovery bookkeeping § 3; not reproduced |
| `str_replace` mixed line endings; response sanitizer; custom agents preserved on mode reapply; `chat.defaultAgent` non-interactive; agent switch by display name | 2.23.0 V3 | model/host-side; invisible on ACP |
| Smaller CLI installation size | 2.23.0 | § 1 (stripped + zstd + onnx-web removal) |
| Ask before env-prefixed allowed commands (security) | 2.23.0 | reverted in 2.23.1 ("no longer prompt on every run") |
| Extended-thinking models no longer fail after Tool Search / multi-think replies | 2.23.1 | host/classic-path fix; not exercised |
| Stale extracted V3 engines cleaned up | 2.23.1 | § 2 (snapshot precaution) |
| `/tools trust-all` | 2.24.0 V3 | § 4 — TUI-local, no wire |
| Honor `chat.defaultModel` in non-interactive; `/effort [default]` | 2.24.0 V3 | TUI/host-side; no wire change |
| Reserved built-in agent names rejected | 2.24.0 | not exercised |
| Downscale oversized images | 2.24.0 | v2 host `image.rs` (§ 4); KAS image path unchanged (0 new literals) |
| Project `.env` no longer overrides sessions/MCP/tools | 2.24.0 | § 4 — Bun `--no-env-file`; cyril unaffected |
| `/sessions` filter, Ctrl+L, scrolling, tool-details layout, Homebrew updater | 2.24.0 TUI | not on the ACP wire |

## 6. Live — v3 paired turn sweep

`probe-kas-turn-sweep-2.24.0.py`, SCENARIO=turn (file read → end_turn),
installed 2.24.0 host. Legs: 0.66.0 ×1, 0.66.8 ×2. All `end_turn`, 11.5–12.3 s.

`sweep-new-fields.py --diff` (0.66.8 vs 0.66.0): **297 vs 292 paths, 5 new,
0 removed — identical in both 0.66.8 runs**:

```
+ result.agentCapabilities.sessionCapabilities.delete
+ result.configOptions[].options[]._meta.kiro.thinkingToggleable
+ result.configOptions[].options[]._meta.kiro.defaultThinkingEnabled
+ params.update.configOptions[].options[]._meta.kiro.thinkingToggleable
+ params.update.configOptions[].options[]._meta.kiro.defaultThinkingEnabled
```

Values: `thinkingToggleable:true, defaultThinkingEnabled:true` on exactly the
Claude 4.6+ / 5 models (opus-5, sonnet-5, opus-4.8, opus-4.7, opus-4.6,
sonnet-4.6); `thinkingToggleable:false` (no default key) on `auto`, the three
GPT-5.6 models (they use `reasoning` effort), opus/sonnet-4.5, sonnet-4,
haiku-4.5 and the open-weight models. `session/new` on `auto` carries no
`effortLevel`/`thinking` option — they appear only after a model switch. The
19-model catalog and rate multipliers are unchanged.

The same 5-path delta is the **only** delta on the workflow workload (§ 8)
and on the new-surface turn legs (§ 7).

## 7. Live — v3 new surface (paired 0.66.0 / 0.66.8)

`probe-kas-new-surface-2.24.0.py`; captures `kas-new-surface-{noprompt,turns}-{0660,0668}-2.24.0.jsonl`.

### 7.1 `thinking` configOption

| step (`session/set_config_option`) | 0.66.0 | 0.66.8 |
|---|---|---|
| model → `claude-sonnet-4.6` | options gain `effortLevel` (high) | gain `effortLevel` (high) **and `thinking` (on)** |
| `effortLevel=max` | max | max, thinking on |
| `thinking=off` | **accepted silently, no effect** | **thinking off, effort capped max → high** |
| `effortLevel=max` | max | max, **thinking forced back on** |
| `thinking=bogus` | accepted silently | **accepted as off** (anything ≠ `"on"`), effort capped |
| model → `gpt-5.6-luna` | effort stays max | no `thinking` option (not toggleable) |
| `thinking=off` on luna | accepted silently | accepted silently, no effect |

`set_config_option` validates neither the configId (0.66.0 accepts
`thinking`) nor the thinking value (0.66.8 maps `bogus` to off). A client must
validate against the advertised options itself. Option shape:
`{type:"select", id:"thinking", name:"Thinking", category:"thinking",
currentValue:"on"|"off", options:[{value:"on",…},{value:"off",…}]}`.

### 7.2 `session/delete`

| call | 0.66.0 | 0.66.8 |
|---|---|---|
| `session/delete` live second session | -32601 Method not found | **`{}`; the session leaves `session/list`** |
| delete again / unknown UUID | -32601 | **-32000 "Something went wrong with the cloud session service. Please try again."** `{errorType:"RelayedUnknownError", retryErrorType:"SERVER_ERROR", faultKind:"unknown"}` |
| `_kiro/session/delete` unknown UUID | `{success:true}` | `{success:true}` |
| `session/set_model` | -32603 persistence classification | -32603 (unchanged) |

The not-found error is misleading (a local session, blamed on the "cloud
session service", flagged retryable). A client must not retry it.

### 7.3 `_kiro/terminal/settings_changed` — live-proven command timeout

* As a **request**: 0.66.0 -32603 (persistence classification), 0.66.8 -32603
  `Unknown ext method` — it is **notification-only**.
* As a notification **without** `enabled`: no effect (`sleep 20` completes;
  captures `kas-new-surface-timeout-0668-term{on,off}-2.24.0.jsonl`, kept as
  the control).
* As `{terminal:{enabled:true, commandTimeoutMs:3000}}`: **the shell command
  is killed at 3 s** on both paths — `tool_call_update status:"failed"`,
  `rawOutput.message` `"…Command timed out after 3000ms\n\nExit Code: -1"`
  (`kas-new-surface-timeout-enabled-0668-term{on,off}-2.24.0.jsonl`).
* **Client-terminal path (cyril's `terminal:true`)**: on timeout KAS sends
  `terminal/output`, `terminal/kill`, `terminal/release` **while its
  `terminal/wait_for_exit` is still outstanding**. A host that serializes
  callbacks (as this probe does) delays the timeout until the process exits
  on its own. Cyril's `TerminalRegistry` already handles kill-during-wait
  (ADR-0004 kill signal, `terminal_io.rs`), so it is correct by construction.
* `terminal/create.env` now carries `KIRO_SESSION_ID` beside
  `AWS_SDK_UA_APP_ID` (0.66.0: `AWS_SDK_UA_APP_ID` only). Cyril passes request
  env through (`terminal_io.rs:229`); the comment there ("KAS sends none @
  2.10.0") is stale.

### 7.4 `rejectionReason` — live-proven

`autopilot=off`, prompt `echo hello`, respond `reject_once` with
`_meta.kiro.rejectionReason:"Do not use echo. Use printf instead, and include
the word PURPLE in the output."`:

* 0.66.0: tool `failed`, content `"The user rejected this tool call."`; model:
  *"The user rejected the tool call with the message: 'The user rejected this
  tool call.'"* — reason dropped.
* 0.66.8: content **`"The user rejected this tool call: Do not use echo. Use
  printf instead…"`**; the model quotes the reason verbatim. The reason is
  visible on the wire in the failed `tool_call_update` content too.

The permission request shape (4 options, `_meta.kiro.{toolId, command,
consent, consentRound}`) is unchanged.

## 8. Live — workflows (paired restate legs)

`probe-kas-workflow-channels-2.24.0.py` + `analyze-workflow-trace-2.24.0.py`:
**unchanged**. Both builds: event kinds `{new, invoke, run_start,
node_start×4, node_complete×2, run_complete}`, identical payload key sets,
double `node_start`, `focus_update.activity` frames as in 2.22.0,
`turn_completion → turn_end → node_complete` same-ms, artifacts correct,
captures `{s1:"ALPHA", s2:"DONE"}` (s2's instructed reply; matches all three
2.22.0 legs). **No `node_failed`.** Field sweep: the same 5 paths as § 6 and
nothing workflow-specific.

## 9. Coverage of the SDK/deserialization boundary

Checked against the pinned `agent-client-protocol-schema` 0.11.2:
`SessionConfigOptionCategory` has an `#[serde(untagged)] Other(String)`
catch-all, so `category:"thinking"` deserializes; `SessionCapabilities` has
no `deny_unknown_fields`, so `delete` is ignored. `to_config_options` maps
selects generically, so a `thinking` entry reaches `ConfigOptionsUpdated` —
where the UI reads only the `model` key (`state.rs:994`) and drops the rest.
The v2 metadata parser tolerates unknown keys (debug log). **No hard-fail
path is reachable from any shape seen in this audit.**

## 10. Live — v2 (cyril's default engine)

### 10.1 The pairing had to be redone

The first v2 run reported 121 = 121 paths, identical 19 models and 25
commands. Process-tree inspection showed the archived 2.22.0 launcher
spawning the installed 2.24.0 `kiro-cli-chat` (§ 2). Re-run with the
archive's `bin/` on `PATH` (child exe recorded in each capture):

```
+ params.reasoning
+ params.reasoning.effortLevels
+ params.reasoning.support
```

19 = 19 models and 25 = 25 advertised commands remain identical.

### 10.2 `_kiro.dev/metadata` effort moved into `reasoning` — REGRESSION for cyril

`probe-v2-reasoning-2.24.0.py`: `session/set_model` → `claude-sonnet-4.6`,
one turn, `/effort max`, one turn. Every metadata frame:

```
2.22.0  {"contextUsagePercentage": …, "effort": "high"}                 # after set_model
        {"contextUsagePercentage": …, "effort": "max"}                  # after /effort max
2.24.0  {"contextUsagePercentage": …, "reasoning": {"support": "unavailable", "effortLevels": []}}          # on auto
        {"contextUsagePercentage": …, "reasoning": {"support": "toggleable", "effort": "high",
                                                    "effortLevels": ["low","medium","high","max"]}}
        {"contextUsagePercentage": …, "reasoning": {"support": "toggleable", "thinkingEnabled": true,
                                                    "effort": "max", "effortLevels": [...]}}
```

**Top-level `effort` never appears on 2.23.0, 2.23.1 or 2.24.0** (all three
probed; attributed to 2.23.0). `reasoning` is on every frame, including the
context-only ones. `/effort` still works (`"Effort set to max"`, options label
`"high  [active]"` unchanged) — only the frame that reports the state moved.

Cyril impact: `convert/kiro.rs:452` reads `params.get("effort")` → `None` →
`EffortUpdate::Unchanged` on every frame; `state.rs:591` is the only badge
writer; a model change clears the badge (`set_current_model`). Net: **the
effort badge is never shown on v2 2.23.0+**, and the `/effort` picker's
cyril-side current-row marker (`active_option_value`) is lost. Fix: read
`reasoning.effort` (keep the top-level read for < 2.23.0), and model the
whole block per the "model the full wire surface" rule — `support`
(`unavailable`/`toggleable`, others unseen), `thinkingEnabled`,
`effortLevels`. Tri-state semantics need re-deriving: absence of
`reasoning.effort` inside a present `reasoning` on a `toggleable` model is
not yet characterized (seen only on `unavailable`).

### 10.3 The v2 `reasoning` command — executable, not advertised

`commands/options {command:"reasoning"}` → -32700 (not in the options enum).
`commands/available` does not list it. But the `commands/execute` `TuiCommand`
enum now lists `…, effort, reasoning, goal` (2.23.0+), and
`{command:"reasoning", args:{}}` returns
`{success:true, data:{thinking:"toggleable", effortLevels:[…],
defaultThinkingEnabled:true, defaultEffort:"max"}}`. `ReasoningArgs` is
`{effort, thinkingEnabled, setAsDefault, targetModelId}` (two recovered from
unique strings, two by live trial); unknown keys are ignored silently.
`{thinkingEnabled:false}` → next metadata `reasoning.thinkingEnabled:false`;
`{effort:"low"}` → `reasoning.effort:"low"`. **This is the v2 equivalent of
KAS's `thinking` configOption** — a thinking toggle cyril can offer on both
engines. (`setAsDefault` was not exercised; it presumably writes
`chat.modelDefaults`.)

## 11. Cyril impact

| finding | engine | severity | action |
|---|---|---|---|
| metadata `effort` → `reasoning.effort`; badge dead since 2.23.0 | v2 | **P1 bug** (silent) | new issue |
| thinking toggle: KAS `thinking` configOption + `_meta.kiro.thinking*`; v2 `reasoning` command | both | feature | new issue; note on cyril-lxuo |
| `rejectionReason` on `reject_once` (first-party TUI uses it) | KAS | feature | new issue |
| `_kiro/terminal/settings_changed` command timeout (`enabled:true` required) | KAS | feature (low) | new issue |
| `session/delete` implemented; not-found = misleading retryable -32000 | KAS | feature (low) | new issue |
| `KIRO_SESSION_ID` in `terminal/create.env` | KAS | none (pass-through works) | stale comment `terminal_io.rs:228` |
| kill/output/release arrive during pending `wait_for_exit` | KAS | none (ADR-0004 handles it) | — |
| `/tools trust-all` TUI-local | — | opportunity | cyril can mirror on its approval overlay |
| `replayMarking` now consumed first-party | KAS | pre-existing gap | cyril-99ds |
| KAS `effortLevel` configOption ignored by the UI | KAS | pre-existing gap | cyril-4jt7 / cyril-cxwb |
| stripped binaries, zstd payloads, launcher PATH trap | audit tooling | methodology | memory + this doc |

## 12. Coverage boundary

Not exercised: the stall-tool-continuation window (unchanged, still
unreachable), large-output offloading (unchanged code path), `session/delete`
of a session that owns a live workflow (`SessionOwnsLiveWorkflowsError`), the
v2 `reasoning` `setAsDefault`/`targetModelId` args, image downscaling, the
`_kiro/sandbox/status` path (internal-nightly gated), `unifiedAgent`,
`KIRO_ROLLOUT_FORCE_INTERNAL` for the new `local_sandbox` row. The v2
`reasoning.effort` tri-state (absent vs present on a toggleable model) needs a
fixture before the fix lands.

## 13. Artifacts

Live probes and captures (`experiments/conductor-spike/`):

* `probe-kas-turn-sweep-2.24.0.py` — `kas-turn-sweep-{0660,0668a,0668b}-2.24.0.jsonl`.
* `probe-kas-new-surface-2.24.0.py` — `LEGS=config,terminal,delete,setmodel,reject,timeout`;
  `kas-new-surface-{noprompt,turns}-{0660,0668}-2.24.0.jsonl`,
  `kas-new-surface-timeout-0668-term{on,off}-2.24.0.jsonl` (no `enabled`, control),
  `kas-new-surface-timeout-enabled-0668-term{on,off}-2.24.0.jsonl`.
* `probe-kas-workflow-channels-2.24.0.py` + `analyze-workflow-trace-2.24.0.py` —
  `kas-workflow-channels-{0660,0668}-restate-2.24.0.jsonl` (+ `-verdict.json`, `-paths.json`).
* `probe-v2-turn-sweep-2.24.0.py` (PATH-forced) — `v2-turn-sweep-{2.22.0,2.24.0}-2.24.0.jsonl`.
* `probe-v2-reasoning-2.24.0.py` — `v2-reasoning-{2.22.0,2.23.0,2.23.1,2.24.0}-2.24.0.jsonl`,
  `v2-reasoning-args{,2}-2.24.0-2.24.0.jsonl` (`RARGS` env for arg trials).
* Sweeps via `sweep-new-fields.py --diff`.

Static scripts and outputs: `static-kas-surface-2.24.0.py`,
`static-kas-identifiers-2.24.0.py`, `extract-kas-feature-flags.py` (takes the
`acp-server.js` path; its env-map anchor needs updating for the spread form),
`static-rollout-2.24.0.py`, `static-host-env-2.24.0.py`,
`static-host-paths-2.24.0.py`, `static-doc-manifests-2.24.0.py`; outputs under
`static-2.24.0/`.

Bundles: 0.66.0 from `~/.local/share/kiro-cli/kas/2.22.0-86d896f0…/`
(snapshotted before any 2.24.0 host ran), 0.66.8 carved from each release's
`kiro-cli-chat` zstd frame; binaries under
`~/.local/share/kiro-research/binaries/{2.23.0,2.23.1,2.24.0}/`; TUI bundles
under `~/.local/share/kiro-research/tui-bundles/`.

Captures were checked for unredacted `accessToken`, `refreshToken`,
`idToken`, `clientSecret`, `profileArn`, `arn:aws:codewhisperer`, JWT-shaped
strings and the account email before commit — zero hits. Bounded check, not
a general secret scan.

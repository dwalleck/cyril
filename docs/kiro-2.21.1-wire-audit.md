# kiro-cli 2.21.1 ACP audit (delta from 2.21.0)

**Audit status:** complete for the stated paired live scope.
No installed Kiro binaries were upgraded. Probes isolated `HOME` to protect
real `~/.kiro` state.

**Conclusion:** no release-attributed ACP method or payload-schema change was
established on the exercised paths. KAS changed from 0.54.8 to 0.58.7 and
adds two internal feature flags. A paired two-turn WITH/CONTROL probe found
new durable SessionStart hook-content storage in KAS; backend prompt logging
was unavailable, so per-turn model receipt is not claimed. Large-output
offloading remains explicitly configurable on the tested ACP path in both
versions, rather than newly default-on. Coverage limits appear in §6e.

**Scope.** Compare the released x86_64 Linux `kiro-cli` 2.21.1 artifact with
the archived 2.21.0 artifact across the v2 ACP engine, the KAS ACP engine, host
rollout/configuration, embedded documentation, and the carved KAS runtime.
Static evidence is hypothesis-generating; behavior claims require the paired
raw JSON-RPC captures listed below. Binary and backend drift are kept separate
by running 2.21.0 and 2.21.1 against the same-day backend controls.

## 1. Acquisition and integrity

The official S3-direct manifest (`https://desktop-release.q.us-east-1.amazonaws.com/latest/manifest.json`)
reported version `2.21.1`, Linux x86_64 headless `tarXz`, archive size
`536509056`, and SHA-256
`7fc0564fd02295a64470c4bf52752f5475f3280be3fa4dd9db255162e07e9825`.
The archive was fetched from
`https://desktop-release.q.us-east-1.amazonaws.com/2.21.1/kirocli-x86_64-linux.tar.xz`,
verified before extraction, and extracted without installation.

| release | binary | bytes | SHA-256 | research path |
|---|---|---:|---|---|
| 2.21.0 | `kiro-cli` | 113,921,088 | `099831d2a777b851b5f94b8b3c67e7c3570ad678b50f526e4f44f0c3663bbd59` | `~/.local/share/kiro-research/binaries/2.21.0/kiro-cli` |
| 2.21.0 | `kiro-cli-chat` | 838,911,376 | `c2b03820e98d2dac1fe13a85f7c91388cac4c56afbed28f17ec3f2e2b2f20b96` | `~/.local/share/kiro-research/binaries/2.21.0/kiro-cli-chat` |
| 2.21.0 | `kiro-cli-term` | 86,906,304 | `2e5476ab941a1484483298354d9166d15e84bee2d5fc94a678311651a16808c7` | `~/.local/share/kiro-research/binaries/2.21.0/kiro-cli-term` |
| 2.21.1 | `kiro-cli` | 113,925,216 | `6880acd76a902afb4f0ba3c5d29134e6608c0b359632227105d08a0756357e21` | `~/.local/share/kiro-research/binaries/2.21.1/extracted/kiro-cli` |
| 2.21.1 | `kiro-cli-chat` | 838,626,440 | `f130f53bfa9e435d864c1b148ea27086cb3ed0399306318161a8a54dd03d1952` | `~/.local/share/kiro-research/binaries/2.21.1/extracted/kiro-cli-chat` |
| 2.21.1 | `kiro-cli-term` | 86,891,944 | `58763faf16420d6cbf53bd138e913a87671917a50a9581324644fcbdf98c8587` | `~/.local/share/kiro-research/binaries/2.21.1/extracted/kiro-cli-term` |

The version-specific archive remains at
`~/.local/share/kiro-research/binaries/2.21.1/kirocli-x86_64-linux.tar.xz`.

## 2. Methods and evidence discipline

* Raw binary and archive hashes were recorded before interpretation.
* The `kas-bundle.tar` gzip stream was carved from each `kiro-cli-chat` without
  starting KAS or logging in, then extracted with tar path filtering.
* KAS quoted wire-method literals were compared from each carved
  `acp-server.js`; KAS package manifests and feature registries were compared
  separately.
* Host-side rollout records were brace-matched as JSON from the binary. Rust
  source-path markers were compared as leads, not treated as semantic proof.
* Embedded documentation manifests were extracted and compared by document
  path/title/metadata. A changed literal or identifier is not called a wire
  behavior without a live control.
* Live lanes use self-contained Python raw JSON-RPC probes. Each spawn uses a
  temporary `HOME`, `XDG_DATA_HOME=/home/dwalleck/.local/share`, isolated
  runtime/log paths, process-group cleanup, and redacted captures. Auth token
  values and callback results are not recorded.

## 3. Static v2/host findings

The host binary embeds a 17-entry rollout registry in two engine copies. The
only 2.21.0 → 2.21.1 registry delta is:

| experiment | 2.21.0 | 2.21.1 | interpretation |
|---|---|---|---|
| `v2_non_interactive` | 1% (all channels) | 10% (all channels) | non-interactive/piped-stdin V2 ramp; not itself an ACP extension claim |

The other rollout entries, including `cloud_config`, `session_dashboard`,
`workflows`, and `v3_prompt`, are unchanged. Host source-path census is
229 → 229, and the quote-delimited ACP method-literal inventory is 81 → 81
with no add/remove. This static lane does not infer that the non-interactive
ramp changes ACP behavior; the v2 lane owns that paired runtime question.

The isolated command `kiro-cli version --changelog=2.21.1` succeeds and identifies
the release date as **2026-09-03**. Its complete output is retained in
`experiments/conductor-spike/static-2.21.1/static-changelog-2.21.1.txt`.
ACP-relevant release claims include full large-tool-output storage, resumed tool
metadata, saved model/effort defaults, steering/skills notification deduplication,
SessionStart hook persistence, provider refusal reasons, and reasoning-stall retries.

## 4. Static KAS findings

The carved KAS package changed materially while preserving the method census:

| item | 2.21.0 | 2.21.1 |
|---|---|---|
| `@kiro/agent` package | 0.54.8 | 0.58.7 |
| `acp-server.js` SHA-256 | `e5bd0a95eaf68d846ef4593fc6e401e10414eda381753c1d795952d31db43714` | `7e102d154413a7b92ed1abae0ab32d5761debfefdff4d626075e20d63f9da0cb` |
| `kas-bundle.tar` bytes | 543,436,800 | 543,528,960 |
| tar members | 3,049 | 3,049 |
| quoted `_kiro/*` method literals | 110 | 110 |
| KAS feature flags | 15 (9 env-reachable) | 17 (10 env-reachable; corrected below) |

The package declarations update the KAS-coupled packages
`@kiro/sandbox-proxy`, `@kiro/acp-type-covenant`, `@kiro/context-providers`,
and `@kiro/client` from 0.54.8 to 0.58.7. The PowerShell grammar remains
`@kiro/tree-sitter-powershell` 0.26.4-kiro.48.

The KAS feature registry keeps all existing keys/defaults/env mappings and
adds two entries:

* `stall_reasoning_retry`, default `true`, no environment override;
* `unified_agent_enabled`, default `false`, generated environment override
  `KIRO_FEATURE_UNIFIED_AGENT_ENABLED`.

**Follow-up correction:** the original static extractor counted only the
literal environment map and missed its generated spread. In 0.58.7,
`wmr({settingKey:"unifiedAgent"})` derives the wire key and environment name;
`Emr()` adds it to `SIi={...vIi,...Emr()}`, which the real environment provider
reads. Executing the extracted gate/environment initializer confirms `true`
and `false` resolve accordingly and the map has ten entries. The saved
`static-kas-flags-2.21.1.txt` is the original incomplete extractor output,
not authoritative for this generated gate.

The gate object is used to construct that map, and the generic active-AB
experiment reporter reads nondefault flags for telemetry. No consumer of
this gate selecting an agent graph, tool set, or prompt was found in the
shipped `acp-server.js`; the gate's `resolve`/`settingComponent` helpers have
no discovered runtime callers. Thus the setting/flag plumbing is present,
but the functionality implied by “unified agent” remains unestablished.

These static additions are bounded by the live controls: the unified-agent
metadata/settings control found no new config option or behavior, while no
backend `chargesReasoningStall` trigger was available for the new retry branch.
No `SAFE` conclusion is justified from the 110-method census alone.

## 5. Embedded documentation and identifier leads

The merged embedded documentation corpus is 163 documents in both releases;
there are no added or removed paths. The only metadata change is
`settings/show-thinking-tips.md`, whose validation date moves from
`2026-07-24` to `2026-09-02`.

The full identifier-shaped KAS literal set and its relevant additions/removals
are saved in `experiments/conductor-spike/static-2.21.1/static-kas-identifiers-delta.json`.
This is a lead inventory, not a semantic verdict: minification and bundle
rewrites can alter literal context, so behavior claims below require the live
probe that exercises the candidate surface.

## 6. Live wire results

### 6a. KAS surface and controls

The paired KAS surface probes used the 2.21.0 and 2.21.1 launchers with
temporary homes and both live and pinned bundles. Both releases completed the
same initialize/session/new/prompt workload, including filesystem, terminal,
permission, authentication, and turn-end callbacks. The advertised
method/surface behavior and the exercised callback families were equivalent
on the paired surface workload. Powers probes returned the same empty list
fixture and the same `-32603 Unknown ext method` refresh response; the
`items_changed` notification count was one on both. Watchdog controls on both
releases returned `-32000 StreamIdleTimeoutError` with warnings for the same
short-idle workloads. The targeted unified-agent controls showed identical
`session/new` `_meta.kiro` echoes (`settings.unifiedAgent.enabled`,
`unified_agent_enabled`, `featureConfig`, and `features`), no new
`unifiedAgent` config option (ids remained mode/model/autopilot/contentCollection),
and `stopReason: end_turn` on both; this did not enable the flag.

The ordinary restate recipe captured `ALPHA` through both the
`{{s1.output}}` template channel and the artifact channel. The terse
first-step recipe produced these **model-reported** verdict values:

| leg | template value A (`{{s1.output}}`) | artifact value B |
|---|---|---|
| 2.21.0 host + old KAS | `Done.` | `ALPHA` |
| 2.21.1 host + new KAS | `<prior_step_output_6695a27d733819d1 id="s1">\nDone.\n</prior_step_output_6695a27d733819d1>` | `ALPHA\n` |
| 2.21.1 host + old KAS | `\nDone.\n` | `ALPHA` |
| 2.21.0 host + new KAS | `<prior_step_output_715085ec969c0c73 id="s1">\nDone.\n</prior_step_output_715085ec969c0c73>` | `ALPHA` |

Both old and new KAS terse raw traces contain the same `session/update`
`user_message_chunk` markup around `Done.`; the preceding `node_start` prompt
still contains literal `{{s1.output}}`. `capturedOutputs.s1` remains
plain `Done.` on both releases. Thus the markup is downstream
interpolation/transport content, not a changed captured-output storage
representation. The crossed-leg value differences (`\nDone.\n` versus the
`prior_step_output` text, and `ALPHA` versus `ALPHA\n`) are workload/model
interpretation and whitespace variance; these probes do not establish a
2.21.1 binary-attributed workflow change. Restate streams contained ten
workflow frames (`workflow/new`, `invoke`, `run_start`, `run_complete`,
`node_start`×4, and `node_complete`×2) and eight captured events. JSONL
method/frame counts were old restate 129/144 lines, new restate 165/185, old
terse 160/180, new terse 139/154, new-host+old-KAS 174/199, and
old-host+new-KAS 133/148. The workflow result is therefore a no-new-delta
finding under this workload, with KAS interpolation behavior still opaque to
Cyril and no blanket claim beyond the exercised paths.
KAS sweep inventories were: surface 341/344 paths (new-only
`params.env/name/value` callback coverage), watchdog 265/265, powers 236/236,
and unified 270/270. Workflow restate was 419/432 paths (old-only
`params.prompt` plus six effort-metadata paths; new-only 20 terminal
callback/result paths); terse was 436/418 (old-only 18 terminal paths,
new-only none). These are workload-coverage differences, not asserted
release fields.
The 14 KAS JSONL captures total 1,406 rows with zero parse errors. Recursive
sensitive-key checks over the JSONL and verdict files found no unredacted
`accessToken`, `refreshToken`, `profileArn`, or `expiresAt` values; auth keys
are `<REDACTED>`.

### 6b. v2 surface and controls

`V2Audit` ran the same raw JSON-RPC workload against the archived 2.21.0
`kiro-cli-chat` and extracted 2.21.1 `kiro-cli-chat`, with the binary hashes
recorded in the verdict. Both completed initialize/session/new, model/mode and
config (config was null), command options, and the ten prior extension-method
requests. Extension response values were identical. The prompt workload
streamed `DONE`, read input, ran a shell `printf`, wrote the exact marker, and
received two `allow_once` permission requests on each release.

The final verdict inventories 158 agent-side field paths on each leg, with zero
additions or removals. The full bidirectional capture sweep inventories 175 on
each side. The active-turn scenario sent the correct
`session/cancel` notification (no id) while a `sleep 20; printf
CANCEL_MARKER` shell turn was running. Both releases returned the original
prompt with `stopReason: "cancelled"`; `cancel_notification_sent` and
`cancel_turn_observed` were true on both. Its sweep found 175 = 175 paths,
with no additions or removals. The earlier request-shaped control returned
`-32601` on both and is not cancellation evidence because ACP cancellation
is notification-shaped. Notification ordering differences in the baseline
workload were workload timing/stream-chunk/tool scheduling variance, not a
reproducible release delta. The static `v2_non_interactive` 1% → 10% rollout
is not itself an ACP change.

Artifacts retained after exploratory cleanup:
`experiments/conductor-spike/probe-v2-live-ab-2.21.1.py`,
`v2-live-ab-2.21.1-cancel-old.jsonl`,
`v2-live-ab-2.21.1-cancel-new.jsonl`,
`v2-live-ab-2.21.1-cancel-old-stderr.log`,
`v2-live-ab-2.21.1-cancel-new-stderr.log`,
`v2-live-ab-2.21.1-cancel-verdict.json`, and
`v2-live-ab-2.21.1-cancel-sweep.txt`.

### 6c. Release-note controls: large output and session replay

The same 32,069-character shell result was generated on each version, with a
unique terminal marker. Each session was closed, the launcher restarted with
the same isolated HOME, and the original session loaded using `session/load`.

| arm | 2.21.0 | 2.21.1 |
|---|---|---|
| Default: no explicit large-output setting | `outputTransformation.kind: clipped`; no `tool-outputs/` file | Same |
| `_meta.kiro.settings.largeToolOutputHandler.enabled: true` at initialize and session/new | `kind: offloaded`, full file saved | Same |
| Reload original tool card | Original tool ID, title and rawInput preserved | Same |

Both forced-arm files contain exactly 32,069 bytes and SHA-256
`dd71c24116ce4944de8db83ed2b90ef0af1b0bd8042f60bddea0c4bb53513fca`.
Their wire previews remain shortened; offloading does **not** imply that the
entire output is transported in the tool-card preview. The default arms
record `originalChars: 32069` but do not save a full tool-output file.

These are controlled observations of a pre-existing configurable ACP path,
not confirmation that full-output storage became default in 2.21.1.
Likewise the tested reload path already preserved the requested tool metadata
on 2.21.0. Other replay/tool families may differ.

Evidence: `probe-kas-output-replay-2.21.1.py`,
`kas-output-replay-2.21.1-{old,new}.jsonl`,
`kas-output-replay-2.21.1-verdict.json`, and
`kas-output-replay-2.21.1-default-{old,new}.jsonl` with
`kas-output-replay-2.21.1-default-verdict.json`, all under
`experiments/conductor-spike/`. Forced and default paired structural sweeps
differ only in terminal callback `params.env` coverage.

### 6d. Release-note controls: hooks, change notifications, saved defaults

**New durable hook-content behavior.** Each version ran SessionStart WITH
and CONTROL arms, each followed by two user turns. Both called the
`sessionStart` callback exactly once. The new version's KAS-owned
`messages.jsonl` session-start record retains:

```text
<HOOK_INSTRUCTION>
[Session Start Hook Output]
KAS_SESSION_START_CLAIM_MARKER_2_21_1
```

The old WITH record does not retain the marker; neither CONTROL does.
The records contain two user turns and one session-start entry. This proves
a difference in durable KAS session context after the two-turn workload,
not that the model received the hook on every turn: governance reports
`promptLogging: false`, so authoritative backend prompt capture was
unavailable. Model echo was not used as proof. The callback trigger/count
remains unchanged; the observed addition is persisted content, not a new
ACP method.

**Notifications.** After startup settled, one scratch steering-file change
produced one `_kiro/steering/documents_changed` and one
`_kiro/progressive_context/items_changed` on each version. A skill-file change
produced one progressive-context notification on each. Neither version
produced duplicate payload digests under these stimuli. The release's
deduplication fix was not distinguished by this workload.

**Saved model/effort.** Both versions exposed saved `claude-sonnet-5` / `low`
on the inheriting agent, and the explicitly pinned `claude-opus-4.8` model
won. Workflow steps used inherited Sonnet/low and explicit Opus/max
respectively. These wire-state controls did not establish a release delta;
no conclusion is drawn for other configuration-precedence paths.

Evidence under `experiments/conductor-spike/`:
`probe-kas-session-claims-2.21.1.py`,
`kas-session-claims-{old,new}-2.21.1.jsonl` with matching
`-verdict.json` and `-prompt-evidence.txt` sidecars, plus
`kas-session-claims-paired-2.21.1.json`.
Verdicts retain sanitized authoritative session-start records, callback
counts, governance state, and model/notification observations.

### 6e. Coverage boundary

The paired lanes cover the exercised ACP/KAS methods, callback families,
workflow channels, powers, watchdog, cancellation, and new-field path sets
only. They did not trigger a backend `chargesReasoningStall` response, so the
new KAS retry branch remains unverified. The unified-agent control exercised
the metadata/settings echo and found no new config option or behavior, but did
not enable the experiment. Provider refusal reasons were not safely triggered
live. These remaining branches are reported as gaps, not as safe or broken.
TUI-only rendering, non-interactive stream-JSON shutdown, auto-update token
expiry, non-commercial API-key endpoints, and enterprise MCP server behavior
are outside this ACP audit's exercised surface.

## 7. Cyril impact and limits

Cyril's KAS workflow conversion stores `captured_output` and
`capturedOutputs` as opaque JSON values; the application does not perform the
KAS `{{s1.output}}` substitution or interpret `prior_step_output` markup.
The observed channel-A wrapper and artifact-channel values are therefore
upstream KAS/host behavior, not a Cyril-side workflow parser transformation.
No 2.21.1-specific Cyril workflow regression was established by these
captures. The separate pre-session routing issue is not reclassified by this
audit; any status claim about it must use the current project integration
state rather than the old 2.21.0 report.

More generally, a Kiro launcher-only callback/order is not automatically a
Cyril direct-node behavior: record the exact spawn path and environment for
each claim. Static absence of a method or field proves only that this
binary/carved bundle did not contain the searched literal. Static presence
proves neither advertisement nor runtime use. A live leg that errors early
covers fewer paths; new-field sweep differences are leads until coverage and
controls agree.


## 8. Artifacts

Static acquisition, carving, and comparison:

* `experiments/conductor-spike/static-audit-2.21.1.py`
* `experiments/conductor-spike/static-kas-surface-2.21.1.py`
* `experiments/conductor-spike/static-kas-identifiers-2.21.1.py`
* `experiments/conductor-spike/static-rollout-2.21.1.py`
* `experiments/conductor-spike/static-2.21.1/static-2.21.1-report.json`
  — regenerable via `static-audit-2.21.1.py`; not tracked in git (60 MB raw
  static dump). Consistent with prior releases, which keep `static-*`
  outputs local and commit only probe scripts and live captures.
* `experiments/conductor-spike/static-2.21.1/static-kas-surface.json`
* `experiments/conductor-spike/static-2.21.1/static-rollout-2.21.1.json`
* `experiments/conductor-spike/static-2.21.1/static-rollout-delta.json`
* `experiments/conductor-spike/static-2.21.1/static-kas-flags-2.21.0.txt`
* `experiments/conductor-spike/static-2.21.1/static-kas-flags-2.21.1.txt`
* `experiments/conductor-spike/static-2.21.1/static-doc-delta.json`
* `experiments/conductor-spike/static-2.21.1/static-kas-identifiers-delta.json`

Live probe scripts and captures:

* `experiments/conductor-spike/probe-kas-surface-2.21.1.py`
* `experiments/conductor-spike/probe-kas-powers-2.21.1.py`
* `experiments/conductor-spike/probe-kas-watchdog-2.21.1.py`
* `experiments/conductor-spike/probe-kas-workflow-channels-2.21.1.py`
* Surface, unified-agent, and powers capture basenames:
  `kas-{surface,unified,powers}-{2.21.0,2.21.1}-2.21.1`.
* Watchdog capture basenames:
  `kas-watchdog-{2.21.0,2.21.1}-hard-2.21.1`.
* Workflow capture basenames:
  `kas-workflow-channels-{2.21.0,2.21.1}-{restate,terse}-2.21.1`,
  `kas-workflow-channels-2.21.1-host-oldkas-terse-2.21.1`, and
  `kas-workflow-channels-2.21.0-host-newkas-terse-2.21.1`.
  Each basename has `.jsonl`, `-verdict.json`, and `-stderr.log` files under
  `experiments/conductor-spike/`.
* `experiments/conductor-spike/kas-field-sweeps-2.21.1.txt` records paired
  structural sweeps and raw workflow wrapper frame references.

Carved runtime paths (outside the repository):

* `~/.local/share/kiro-research/kas-carves/2.21.1/2.21.0/tree/`
* `~/.local/share/kiro-research/kas-carves/2.21.1/2.21.1/tree/`

All live captures listed above were checked for parse errors and for
unredacted `accessToken`, `refreshToken`, `profileArn`, or `expiresAt` values;
auth keys are redacted. No captured file contains a secret callback result.

Final merged-evidence check: **22 JSONL captures, 2,573 parsed rows, and
20 parsed verdict/comparison JSON files**. A recursive scan found no
unredacted values under access-token, refresh-token, authorization,
client-secret, ID-token, or Kiro-key fields. This is a bounded sensitive-key
check, not a claim that arbitrary secret formats are mechanically detectable.

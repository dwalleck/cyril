# kiro-cli 2.21.2 ACP audit (delta from 2.21.1)

**Audit status:** complete for the stated paired scope. The installed binary
was already 2.21.2 when the audit began; 2.21.1 was compared from the research
archive. Probes isolated `HOME` to protect real `~/.kiro` state. Cost: two
tiny real turns (one per binary) for the turn-lifecycle sweep in § 6b.

**Conclusion:** **SAFE for cyril — no wire change on any exercised path**, and
a structural field sweep over both session-creation and full-turn workloads
finds **188 = 188 JSON paths with zero new and zero missing fields** (§ 6b). The
`_kiro/*` census is 110 = 110 and the KAS runtime is **byte-frozen at 0.58.7**
(`acp-server.js` hashes identically), so nothing in this release can be a KAS
change. All three ACP-relevant release notes are **host-side**. Two are
confirmed no-ops for cyril; the third — the model-list failure path — is newly
implemented, statically located, and **not inducible from outside the binary**,
so it is reported as a bounded gap rather than as verified-safe.

**Scope.** Compare the released x86_64 Linux `kiro-cli` 2.21.2 artifact with the
archived 2.21.1 artifact across the v2 ACP engine, the carved KAS runtime, the
Rust host rollout registry, and the embedded TUI bundle. Static evidence is
hypothesis-generating; behavior claims use the paired live captures listed in
§ 7. Binary and backend drift are kept separate by running both binaries
against the same-day backend.

## 1. Acquisition and integrity

The official S3-direct manifest reported version `2.21.2`, Linux x86_64
headless `tarXz`, archive size `536592360`, SHA-256
`504de388194df2e5f83d11f39a2831422842dd8061b626b24c8fd541d9e7a652`. The archive
was fetched from
`https://desktop-release.q.us-east-1.amazonaws.com/2.21.2/kirocli-x86_64-linux.tar.xz`,
verified **before** extraction, and extracted without installation.

`BUILD-INFO`: `BUILD_VERSION=2.21.2`,
`BUILD_HASH=a4729fa233d8097438cb3dcb4c9c8d2f21cc82d9`,
`BUILD_DATE=2026-09-08T19:39:39Z`, `x86_64-unknown-linux-gnu`.

| release | binary | bytes | SHA-256 |
|---|---|---:|---|
| 2.21.1 | `kiro-cli` | 113,925,216 | `6880acd76a902afb4f0ba3c5d29134e6608c0b359632227105d08a0756357e21` |
| 2.21.1 | `kiro-cli-chat` | 838,626,440 | `f130f53bfa9e435d864c1b148ea27086cb3ed0399306318161a8a54dd03d1952` |
| 2.21.1 | `kiro-cli-term` | 86,891,944 | `58763faf16420d6cbf53bd138e913a87671917a50a9581324644fcbdf98c8587` |
| 2.21.2 | `kiro-cli` | 113,931,888 | `3d60a9fcf0d6a3f8b8cec7f01d15d4fb88088b192b0fa7910911a0cce61e416d` |
| 2.21.2 | `kiro-cli-chat` | 838,969,752 | `3b99d843521fd10b9ca963472d7dee3dfc9e2ffdbdb0640a769e750afb74580c` |
| 2.21.2 | `kiro-cli-term` | 86,904,936 | `af705ab45f34b3e40d519c50ca73a7e780a30e49691c87144b4b73a4b67ccd93` |

All three binaries changed (+6,672 / +343,312 / +12,992 bytes), so 2.21.2 is a
real build rather than a repackage.

The remote changelog feed had **not** propagated 2.21.2 at audit time —
`kiro-cli version --changelog` returns *"No changelog information available for
version 2.21.2."* on the 2.21.2 binary while returning full text for 2.21.1.
The release notes used to drive this audit came from the operator, not from the
binary. `kiro-cli --help` is byte-identical between the two releases.

## 2. Methods and evidence discipline

* Archive and per-binary hashes were recorded before any interpretation.
* The `kas-bundle.tar` gzip stream was carved from each `kiro-cli-chat` without
  starting KAS or logging in, then extracted with tar path filtering.
* KAS wire vocabulary was compared as **quoted** `_kiro/*` literals from each
  carved `acp-server.js`; unquoted matches in the host binary are LTO
  string-table noise (see § 3) and are not treated as methods.
* Host-side rollout records were brace-matched as JSON from the binary.
* Live lanes use a self-contained Python raw JSON-RPC probe. Each spawn uses a
  temporary `HOME`, `XDG_DATA_HOME` pointing at the real share directory,
  and process cleanup. Auth values are redacted.
* **Controls are themselves verified.** The first agent-switch run selected its
  target using the `current` flag on `commands/options` entries, which v2 does
  not populate (cyril-imjx); the probe therefore "switched" to the already-live
  agent and observed no announcement. That false negative was caught and the
  probe re-keyed to `session/new`'s `modes.currentModeId`. Only the corrected
  run is reported.

## 3. Static findings — KAS is byte-frozen

`@kiro/agent` is **0.58.7 on both releases**, and
`dist/server/acp-server.js` is **byte-identical**
(`7e102d154413a7b92ed1abae0ab32d5761debfefdff4d626075e20d63f9da0cb`).

A recursive diff of the two carved trees reports 22 differing entries, and
**every one is a type-declaration package**: 19 `.d.ts` files plus
`@types/node/package.json`, `@types/node/README.md`, and
`undici-types/package.json` — a `@types/node` 26.4.1 → 26.5.0 bump. **No
runtime JavaScript differs.**

Censuses on the carved bundles:

| lane | 2.21.1 | 2.21.2 | delta |
|---|---:|---:|---|
| KAS quoted `_kiro/*` method literals | 110 | 110 | none |
| KAS `process.env` reads | 65 | 65 | none |
| KAS feature-flag env literals | 49 | 49 | none |

**Consequence:** any behavior change in this release is host-side. A `[V3]` tag
in the release notes marks *which engine's sessions are affected*, not which
component changed — see § 5.

The host binary's *unquoted* `_kiro.dev/*` literal diff is pure LTO noise
(entries such as `_kiro.dev/commanadditional_field` and
`_kiro.dev/telemetry/uiModeChangedProxyMessage…` are adjacent string-table runs
concatenated). It is recorded but carries no signal, consistent with the
binary-diff methodology note that only identifier-shaped literals are usable.

## 4. Static findings — the Rust host rollout registry

The compiled-in rollout registry holds **17 experiments on both releases with
zero changed entries**. Values of note, unchanged across the pair:
`v3_prompt` 0 % (the V3 switch remains an opt-in prompt only),
`v2_non_interactive` 10 %, `cloud_config` / `session_dashboard` /
`remote_sandbox` / `tangent` / `voice` at 100 % for all users,
`workflows` and `memory` internal-only.

## 5. Static findings — the three ACP release notes

### 5a. Model list on ACP session creation

> *"ACP session creation no longer advertises a hardcoded model list when the
> model-list fetch fails; it retries briefly, then reports the list as unknown."*

Four strings are **new in 2.21.2 and absent from 2.21.1** (0 → 1 occurrences
each), and together they describe the whole implementation:

```
Failed to fetch available models (attempt {}): {}; retrying in {}
Failed to fetch available models after {} attempts: {}; the model list is unknown
no model list was fetched for this session
mock model list failure armed by test
```

The last is an in-process Rust test hook reached through `IpcMockApiClient`;
**no environment variable arms it** (the full `KIRO_*` / `Q_*` / `AB_*`
inventory was searched), so the failure path is **not inducible live from
outside the binary**. That is the single coverage gap of this audit.

`no model list was fetched for this session` sits in the string table directly
among ACP request-handler validation errors — `session_id is required`,
`cwd is required`, `Invalid session ID '': must be a valid UUID`,
`Prompt already in progress`, `ExecuteCommand: flags computed`,
`GetCommandOptions: current available agents` — and immediately after
`session model:` / `Failed to set CLI model override:`. The strong reading is
that an unknown list is surfaced on the ACP path as a **handler error**, not as
an empty list.

Corroborating the removal half: one cluster of hardcoded model ids near
`crates/chat-cli-v2/src/cli/chat/legacy/model.rs` present in 2.21.1 is **gone**
in 2.21.2 (`claude-sonnet-4.5` 3 → 2, `claude-sonnet-4` 11 → 10; distinct-id
clusters 4 → 3). Current catalog ids (`claude-opus-5`, `gpt-5.6-*`, …) occur
**zero** times in either binary, re-confirming that the live catalog is
backend-served.

### 5b. Agent switches announced over ACP

> *"Announce agent switches made through the ACP protocol so the TUI stops
> showing the previous agent."*

**No wire change.** See § 6 — `_kiro.dev/agent/switched` fires on *both*
releases for an ACP-originated switch, with identical payload fields. The fix
is TUI-side consumption, not emission.

### 5c. `[V3]` shell streaming updates partial lines in place

> *"[V3] Shell tool streaming output now updates partial lines in place instead
> of expanding into multiple entries."*

KAS is byte-frozen, so this is **not** a KAS change. The host binary's quoted
literal census gained exactly one entry, 81 → 82:
**`_kiro/tools/content_chunk`** — 0 occurrences in 2.21.1, 1 in 2.21.2. The
embedded TUI bundle now registers

```js
this.kiroClient.onExtNotification("_kiro/tools/content_chunk",
                                  (t) => this.handleToolCallContentChunk(t))
```

`_kiro/tools/content_chunk` has existed in KAS since 0.48.0 (kiro-cli 2.19.1)
with **no shipped consumer**. **2.21.2 ships the first one.** The wire method is
unchanged; a first-party reference implementation appeared.

The recovered contract (two functions on *different* paths — adjacency in the
minified bundle is misleading, and the call sites were checked):

```js
// decoder for _kiro/tools/content_chunk
function VYn(e, n) {
  if (e.sessionId !== n) return null;                       // session-scoped
  const a = e.content?.type === "content" ? e.content.content : undefined;
  const i = a?.type === "text" ? a.text : undefined;
  if (typeof e.toolCallId !== "string" || typeof i !== "string") {
    debug("[content-chunk] dropped malformed notification"); return null;
  }
  if (i === "") return null;                                 // empty chunk dropped
  return { type: "tool_call_update", id: e.toolCallId,
           content: { type: "text", text: i }, merge: "append" };   // APPEND
}

// applied on the STANDARD ACP session/update path, via convertAcpUpdateToEvent
function ZGe(e) {
  if (e.type === "tool_call_update" && e.content.type === "text"
      && e.content.text !== "") e.merge = "replace";                // REPLACE
}
```

So: **streamed chunks append; the authoritative standard `tool_call_update`
then replaces the accumulated text.** That replace-on-final is what stops the
completed output from "expanding into multiple entries" beneath the partials.
Payload shape is
`{sessionId, toolCallId, content:{type:"content", content:{type:"text", text}}}`;
non-matching session, non-string `toolCallId`/`text`, and empty text are all
dropped.

## 6. Live results — paired v2 probe

`probe-v2-models-agentswitch-2.21.2.py`, run against the archived 2.21.1 and
2.21.2 `kiro-cli` binaries, same-day, same backend. No `session/prompt` sent.

| observation | 2.21.1 | 2.21.2 |
|---|---|---|
| `session/new` top-level keys | `["models","modes","sessionId"]` | identical |
| `configOptions` present | no | no |
| `models.availableModels` count | 19 | 19 |
| `models.currentModelId` | `auto` | `auto` |
| `commands/options {model}` count | 19 | 19 |
| agents advertised | `kiro_default`, `kiro_guide`, `kiro_planner` | identical |
| switch `kiro_default` → `kiro_guide` | accepted, no error | accepted, no error |
| notifications after the switch | `_kiro.dev/agent/switched`, `_kiro.dev/metadata`, `_kiro.dev/commands/available` | identical |
| `_kiro.dev/agent/switched` emitted | **yes** | **yes** |
| `session/update` `current_mode_update` | not seen | not seen |

`_kiro.dev/agent/switched` params on both releases carry `sessionId`,
`agentName`, `previousAgentName`, and `welcomeMessage`.

Two standing CLAUDE.md claims are re-confirmed on 2.21.2: `config_options` is
**absent entirely** from the v1/v2 `session/new` result, and `commands/options`
entries carry **no** `current` flag (cyril-imjx).

## 6b. Structural field sweep — new and missing data on the wire

A method census answers "did a method appear"; it cannot answer "did a field
appear inside a frame nobody asked about". `sweep-new-fields.py` collapses each
capture to its set of JSON paths (array indices flattened to `[]`) and diffs
the sets. Run on two workloads:

| workload | paths 2.21.1 | paths 2.21.2 | new | missing |
|---|---:|---:|---:|---:|
| session creation + agent switch | 149 | 149 | 0 | 0 |
| full turn (prompt → file-read tool call → `end_turn`) | 118 | 118 | 0 | 0 |
| **union across both** | **188** | **188** | **0** | **0** |

**Identical field sets on every lane. No new and no missing data on the wire.**

The turn lane was added specifically because the first live probe sent no
`session/prompt` and therefore captured none of the frames cyril actually
renders. Both binaries produced the same frame families for the same workload:

```
session/update:agent_message_chunk   1     _kiro.dev/metadata              4
session/update:tool_call             1     _kiro.dev/session/update        1
session/update:tool_call_update      1     _kiro.dev/subagent/list_update  1
                                           _kiro.dev/commands/available    1
```

`stop_reason: end_turn` on both; 3.9 s vs 3.4 s; zero permission requests (file
reads need none on v2, as documented).

### Fields cyril does not model

The sweep also answers the inverse question — not "did Kiro change" but "is
cyril dropping something that is already arriving". Cross-checking all 91
distinct leaf names against `crates/` separates serde-typed fields (which
arrive as snake_case Rust identifiers, not string literals — `availableModes`,
`currentModeId`, `locations`, `rawInput`, `rawOutput` are all handled) from
genuine drops:

* **`commands[].meta` subcommand metadata** — cyril reads only `meta.inputType`
  and `meta.local` (`convert/kiro.rs:700`). It silently drops `optionsMethod`,
  `subcommands`, `subcommandHints`, `subcommandDescriptions`, and `hint`. The
  `/agent` entry alone carries three subcommands with per-subcommand hints and
  descriptions. Filed as **cyril-j90q**.
* **`result.hasMore` on `commands/options`** — never read (`has_more` occurs
  zero times in `crates/`). `false` on every observed call, so nothing is
  truncated today, but a truncated picker would render silently as a complete
  list. Filed as **cyril-8bz8**.

Neither is a 2.21.2 regression — the field sets are identical across the pair.
Both are pre-existing drops that the sweep made visible, and both are below the
project bar for received payloads (model every field, or explicit-ignore plus a
debug log).

## 7. Cyril impact

**No code change is required by this release.**

* **Agent switches (§ 5b, § 6): no impact.** Cyril already handles
  `kiro.dev/agent/switched` at
  `crates/cyril-core/src/protocol/convert/kiro.rs:649`, reading all four fields
  the frame carries. The frame arrives on the raw-JSON ext-notification path,
  which is immune to the `SessionUpdate` serde hard-fail (that enum is
  `#[serde(tag = "sessionUpdate")]` with no `#[serde(other)]`). This release
  adds no typed `session/update` variant, so the latent hard-fail tracked by
  cyril-ai1y is **not** triggered by 2.21.2.

* **Model list (§ 5a): the picker path is already correct.** Cyril drives
  `/model` from `_kiro.dev/commands/options`, and
  `domain_mediator/commands/extensions.rs:74` converts a failed RPC into a
  `BridgeError` notification rather than an empty picker (cyril-tr0a: *"a
  failed RPC must not masquerade as a command with no options"*). If an unknown
  list surfaces as a handler error, the user sees the reason.

  The residual weakness is the **cached** catalog, not the picker:
  `SessionController` stores `available_models` as a plain `Vec<ModelInfo>`
  (`session.rs:298`), so "the list is unknown" and "there are no models"
  collapse to the same empty vector. `parse_model_catalog`
  (`convert/kiro.rs:1141`) already warns on each degraded shape and reads
  `currentModelId` independently, so nothing misreports today — but the
  distinction is unrepresentable in the cached type. This is a latent
  `Option`-vs-sentinel gap, filed rather than fixed here.

  Relatedly, the removed hardcoded fallback is a **plausible upstream source**
  of the phantom models behind cyril-fj6j (P1: the picker offers a model
  kiro-cli accepts, then every turn fails `-32603`). 2.21.2 likely narrows that
  bug's blast radius. It does **not** close it: an entitlement guard is still
  needed, because a correct backend list can still contain models the account
  is not entitled to. No claim is made that fj6j is fixed.

* **Shell streaming (§ 5c): no breakage, new design input.** Cyril has no
  `content_chunk` consumer at all, so nothing regresses. cyril-2mo0 now has a
  first-party reference implementation and an exact merge contract to build
  against — including the detail that chunks **append** while the standard
  update **replaces**, which is easy to get backwards.

## 8. Coverage boundary

The model-list *failure* path was not exercised: its only trigger is an
in-process Rust test mock with no environment lever. Everything asserted about
it in § 5a is static string and symbol evidence plus string-table
neighbourhood, not a live capture. The exact wire representation of an unknown
list — whether `session/new` omits `models`, returns it empty, or errors, and
what `commands/options {model}` returns — is **unverified**. Treat § 7's
model-list reasoning as conditional on that shape.

Also outside this audit's exercised surface: TUI-only rendering behavior (the
scrollback, spinner, settings-footer, and reasoning-hint notes), non-interactive
stream-JSON, auto-update, and enterprise MCP behavior. The `[V3]` shell
streaming contract in § 5c is recovered from the shipped bundle, not from a live
KAS session with `streamingShellContent` advertised.

The § 6b sweep covers the v2 engine only, and within it the two workloads
listed. Frame families never exercised — permission prompts, plans, subagent
crews, cancellation, compaction, KAS `_kiro/*` turns — are outside the swept
path set, so "zero new fields" is scoped to what was driven, not to the whole
protocol.

## 9. Artifacts

Live probe and captures:

* `experiments/conductor-spike/probe-v2-models-agentswitch-2.21.2.py`
* `experiments/conductor-spike/v2-models-agentswitch-2.21.1-2.21.2.jsonl`
* `experiments/conductor-spike/v2-models-agentswitch-2.21.2-2.21.2.jsonl`
* `experiments/conductor-spike/probe-v2-turn-sweep-2.21.2.py` — the turn lane
* `experiments/conductor-spike/v2-turn-sweep-2.21.1-2.21.2.jsonl`
* `experiments/conductor-spike/v2-turn-sweep-2.21.2-2.21.2.jsonl`
* `experiments/conductor-spike/v2-field-sweep-2.21.2.txt` — sweep output
* Sweeps run with `sweep-new-fields.py` in both inventory and `--diff` mode.

Static scripts (outputs stay local and regenerable, per prior releases):

* `experiments/conductor-spike/static-audit-2.21.2.py`
* `experiments/conductor-spike/static-kas-surface-2.21.2.py`
* `experiments/conductor-spike/static-kas-identifiers-2.21.2.py`
* `experiments/conductor-spike/static-rollout-2.21.2.py`
* `experiments/conductor-spike/static-host-paths-2.21.2.py`
* `experiments/conductor-spike/static-2.21.2/` — KAS surface census, rollout
  registry + delta, changelog and help captures. The 60 MB
  `static-2.21.2-report.json` raw dump is not tracked.

Carved runtimes (outside the repository):

* `~/.local/share/kiro-research/kas-carves/2.21.2/2.21.1/tree/`
* `~/.local/share/kiro-research/kas-carves/2.21.2/2.21.2/tree/`

Captures were checked for unredacted `accessToken`, `refreshToken`, `idToken`,
`clientSecret`, and `profileArn` values before commit; auth keys carry
`<REDACTED>`. This is a bounded sensitive-key check, not a claim that arbitrary
secret formats are mechanically detectable.

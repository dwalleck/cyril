# kiro-cli 2.16.0 wire audit (2026-07-30, vs 2.15.0)

**Verdict: SAFE for cyril's current v2 path — but this is the biggest KAS release since
2.7.1.** The v2 (Rust) surface cyril consumes is unchanged at settle (zero field-path delta), and
the one v2 response that *did* change (`/context`) is **purely additive**. On the KAS side,
`@kiro/agent` jumped **0.25.17 → 0.27.8** and shipped a **complete, functional, multi-agent
workflow engine** — the `_kiro/workflow/*` emitter that has been missing since the protocol was
first spotted at 2.14.1. It is **live and answering today**, gated behind an opt-in setting.

Baselines: archived 2.15.0 (`~/.local/share/kiro-research/binaries/2.15.0/`, KAS 0.25.17 at
`~/.local/share/kiro-cli/kas/2.15.0-5dc82f4d.../`). 2.16.0 binaries archived to
`~/.local/share/kiro-research/binaries/2.16.0/`. All A/B captures below were taken **same-day
(2026-07-30)** against both binaries, so field deltas isolate the binary axis, not the backend.

## Embedded changelog

**2.16.0** (2026-07-30):
- Added: [V3] `/tangent` to spin off multiple named side-conversations & switch between any of
  them in a visual picker
- Added: Per-tool token breakdown in `/context`, grouped by source (built-in, MCP server, agent)
- Fixed: Context usage percentage now recalculates when switching models via `/model`

The changelog badly undersells this release. It mentions none of the workflow engine.

## Headline: the `_kiro/workflow/*` emitter shipped

`cyril-6beh` has tracked this protocol since the 2.14.1 audit with the standing note *"DEFERRED
until an emitter ships — do not build UI against it yet."* **That condition is now met.**

### Attribution — this is genuinely new, not a bundler artifact

Per the methodology's bundler-rename trap, occurrences were counted in **both** versions:

| Token | 2.13.0 | 2.14.1 | 2.14.2 | 2.15.0 | 2.16.0 |
|---|---|---|---|---|---|
| `_kiro/workflow` in `acp-server.js` | 0 | 0 | 0 | **0** | **118** |
| `workflow-progress` | — | — | — | 0 | 4 |
| `nodeTree` | — | — | — | 0 | 13 |

The earlier "workflow route exists" observation was about **tui.js (client-side)** — a
parse-and-drop converter with no renderer. The **server** never had it. Now it does.

- **`_kiro/*` method literals: 83 → 109.** 26 added, **0 removed**.
- **Bundle modules: 686 → 729.** 47 added, 4 removed — **26 of the 47 are `src/workflow/**`**.

### The 26 new methods

Control (client → agent): `invoke` `cancel` `pause` `resume` `resumeAll` `retry` `list`
`listRecipes` `listWatchHandlers` `load` `new` `delete` `inspect` `update`

Lifecycle events (agent → client) — the emitter, from `src/workflow/workflow-notification-bridge.ts`:

```js
var KIND_TO_METHOD = {
  run_start:      "_kiro/workflow/run_start",
  node_start:     "_kiro/workflow/node_start",
  node_complete:  "_kiro/workflow/node_complete",
  node_paused:    "_kiro/workflow/node_paused",
  loop_iteration: "_kiro/workflow/loop_iteration",
  watch_poll:     "_kiro/workflow/watch_poll",
  paused:         "_kiro/workflow/paused",
  run_complete:   "_kiro/workflow/run_complete",
  steps_queued:   "_kiro/workflow/steps_queued"
};
```

Each event emits `{...event.payload, parentSessionId?}`. The bridge auto-unsubscribes when
`run_complete` carries a terminal status. Pending lifecycle events buffered before the bridge
attaches are flushed on subscribe — so a client attaching late does not miss the run's opening.

### The gate — opt-in, surfaced on the wire

```js
function resolveWorkflows(parsed, persistedDefault) {
  return parsed.data.workflows?.enabled ?? persistedDefault ?? false;
}
```

Default **false**. It is now reported in the handshake as `session/new._meta.workflowsEnabled`
(new field this release — see the KAS delta table below). A client opts in by passing settings at
session creation:

```json
{"method":"session/new","params":{"cwd":"…","mcpServers":[],
  "_meta":{"kiro":{"settings":{"workflows":{"enabled":true}}}}}}
```

**`_kiro/workflow/*` is NOT advertised in `extensionMethods`** — that list is unchanged at 7. The
methods route regardless; discovery is out-of-band.

### Live functional verification (schema-accepted ≠ functional)

`experiments/conductor-spike/probe-kas-workflow-runtime-2.16.0.py` flips the gate and calls the
read-only methods. No prompt turn, so zero credits. Result: **`workflowsEnabled: true`** and the
runtime answers.

- **`_kiro/workflow/list`** → `{"runs": []}`. Requires an explicit `workspacePaths` array; omitting
  it returns `-32603 "workspacePaths is not iterable"` (an unguarded destructure — real, minor).
- **`_kiro/workflow/listRecipes`** → **7 bundled recipes**, each with a real executable plan:

  | Recipe | Nodes | Node types | Inputs |
  |---|---|---|---|
  | `autoresearch` | 1 | `repeat` | `benchmark_path`, `research_directions` |
  | `feature-pipeline` | 6 | `repeat`,`step` | `task`, `workdir` |
  | `goal` | 1 | `repeat` | `prompt`, `max_iterations` |
  | `investigate` | 1 | `step` | `brief`, `report_path` |
  | `publish-pr` | 2 | `repeat`,`step` | `branch` |
  | `ralph` | 1 | `repeat` | `goal`, `prd_path` |
  | `semantic-review-multi-model` | 3 | `parallel`,`step` | `target`, `workdir` |

  Node types are `step`, `repeat`, and **`parallel`** — a real DAG scheduler
  (`src/workflow/parallel-scheduler.ts`), not a linear chain. `autoresearch` runs `maxIterations:
  1000` with `onMaxIterations: "pause"`.

- **`_kiro/workflow/listWatchHandlers`** → 2 handlers with JSON-Schema configs:
  - `github-pr` — polls a GitHub PR for comments/reviews/check status via the `gh` CLI
    (`prRef`, `url`, `pollIntervalSec`, `includeOwnActivity`, `ignoreAuthors`)
  - `crux-cr` — polls an Amazon Crux code review via the `cr` CLI (`crRef`, `crId`, …)

  Both `defaultPollIntervalSec: 60`, `minPollIntervalSec: 30`. These make workflows
  **externally-triggered and long-running**, not just fire-and-forget.

### Architecture — workflow steps are full sessions, not subagents

From `buildWorkflowDriverHost()`, a workflow step calls `this.host.newSession(...)`:

```js
createWorkflowStepSession: async (input) => {
  const response = await this.host.newSession({
    cwd: input.workspacePath, mcpServers: [],
    _meta: { kiro: {
      modeId: input.agentDefinition.id,
      settings: { workflows: { enabled: true } },   // step sessions need the workflow tools
      ...this.host.getShellType() ? { shellType: this.host.getShellType() } : {},
      workflow: { workflowId: …, workflowName: … },
```

**This independently validates cyril's own recorded design decision**
(`project_cyril_session_level_workflows`: *"workflow engine at session level (N peers), not
subagent"*). Kiro landed on the same model. Consequence for cyril: a running workflow produces
**N concurrent top-level sessions**, each emitting ordinary `session/update` streams, correlated
by `_meta.kiro.workflow.workflowId` — not by the subagent tracker.

Also note the inline comment: step sessions inherit the connection's resolved shell type
specifically to skip a `_kiro/terminal/shell_type` round-trip. Relevant to **cyril-6bol**.

### Agent-facing tools

Five new tools registered when the gate is on: `run_workflow`, `inspect_workflow`,
`update_workflow`, `validate_workflow`, `save_workflow_definition`. `validate_workflow` and
`save_workflow_definition` are conditionally registered on `workflowsEnabled`; the agent authors
workflows into `.kiro/workflows/` and steering swaps to `WORKFLOW_STEP_COMPLETION_PROTOCOL` when
`session.workflowId` is set.

## v2 (Rust) — frozen at settle, one additive command response

- **Crate pins unchanged:** `agent-client-protocol-0.10.4`, `sacp-11.0.0`.
- **Live offline A/B** (`probe-v2-surface-ab-2.11.0.py`, HOME-isolated, same-day):
  **24 commands / 14 tools identical, zero field-path delta** across all 5 message types that
  reach cyril (`R:initialize`, `R:session/new`, `_kiro.dev/commands/available`,
  `_kiro.dev/metadata`, `_kiro.dev/subagent/list_update`).
  - `tool_search` still absent (14 not 15) — same backend-axis observation as 2.13.0/2.14.1/2.15.0.
    Notable because the doc manifest revalidated `tools/tool-search.md` on 2026-07-29 and added
    `settings/tool-search-settings.md`. Docs moved; the wire did not.

### `/context` — additive `groups[]` (live-verified)

The nm diff surfaced three new types under **`chat_cli_v2::agent::acp::commands::context`** —
`ToolBreakdownItem`, `ToolCategoryBreakdown`, `ToolGroupBreakdown` — i.e. on the ACP commands path
cyril drives, not TUI-only. Settle captures cannot see this, so
`probe-v2-context-breakdown-ab-2.16.0.py` executes `_kiro.dev/commands/execute` for `context`
against both binaries. Field-path diff — **9 added, 0 removed**:

```
+ data.breakdown.tools.groups[]
+ data.breakdown.tools.groups[].name        e.g. "Built-in"
+ data.breakdown.tools.groups[].source      e.g. "built-in"  (also: MCP server, agent)
+ data.breakdown.tools.groups[].tokens
+ data.breakdown.tools.groups[].percent
+ data.breakdown.tools.groups[].items[].{name,tokens,percent}
```

`breakdown.tools.{tokens,percent}` are preserved, and the other four buckets
(`contextFiles`, `kiroResponses`, `yourPrompts`, `sessionFiles`) are untouched.

**cyril impact: none — it is forward-compatible by construction.**
`format_command_response` (`crates/cyril/src/app.rs:1193`) iterates a fixed `categories` array
reading `tokens`/`percent` per bucket, so `groups[]` is simply ignored. Cyril does not break; it
just does not yet *show* the new per-tool detail. Filed as an enhancement.

Also new: `recompute_context_usage_after_model_change` — the third changelog line. Cyril caches
model and context usage separately, so worth confirming the refreshed percentage propagates.

### nm module-path diff (2.15.0 → 2.16.0)

+64 / −41 module paths. Nothing ACP-facing beyond the `/context` types above. Two things worth
recording:

- **`agent::agent::tui_commands::command::*Args` wholesale removal** (ModelArgs, EffortArgs,
  ClearArgs, HooksArgs, …) with `chat_cli::cli::chat::cli::{context,clear,paste,reply,usage,
  changelog,experiment,lite}::*Args` appearing. This is the `TuiCommand` adjacently-tagged enum
  CLAUDE.md documents as load-bearing for `commands/execute`. **The live A/B proves the reshuffle
  is wire-neutral** — `commands/available` is byte-identical and `/context` executes correctly
  with the documented `{"command":"context","args":{}}` object form. Internal refactor only.
- Telemetry churn: `kiro_telemetry` crate appears, `StartupTelemetry`, `KasMeteringUsage`,
  `KasTurnCompletion`, `emit_kas_noninteractive_turn`, `send_metering_event_for_engine` — the host
  now meters KAS turns per-engine. Off-wire for ACP clients.

## KAS handshake deltas (live A/B, `probe-kas-hostinit-2.15.0.py` on both binaries)

`extensionMethods` (7), `authMethods`, `modes` (7), `configOptions` ids, and all seven unsolicited
session-start pushes are **unchanged**. Four field-level additions:

| Message | Added field | Value | Notes |
|---|---|---|---|
| `initialize` | `agentCapabilities._meta.kiro.replayMarking` | `true` | Replayed `session/load` updates now carry `_meta.kiro.replay: true`, letting a client separate the replay stream from live updates that interleave during load |
| `session/new` | `_meta.workflowsEnabled` | `false` | The workflow gate, reported |
| `_kiro/sessions/changed` | `upserted[].source` | `"local"` | Cloud/relay groundwork surfacing |
| `_kiro/sessions/changed` | `upserted[].executionTarget.kind` | `"local"` | ditto |
| `session/update` | `configOptions[].options[]._meta.kiro.effortLevels` | `["low","medium","high","xhigh","max"]` | **Per-model effort levels now advertised** |
| `session/update` | `configOptions[].options[]._meta.kiro.defaultEffortLevel` | `"high"` | Only on models with `hasEffort: true` |

The effort pair is the most immediately useful: cyril previously had to infer the effort ladder.
KAS now publishes it per model alongside the existing `hasEffort` / `effortSchemaPath`. Bears on
**cyril-8yka** (stale effort badge) and **cyril-imjx** (picker never marks the active option) —
`defaultEffortLevel` gives a real default to render against.

`sourceProviders: false`, `executionTargets: ["local"]`, `sessionSources: ["local"]` — cloud/relay
still dormant on a local run.

## Two other new `_kiro/*` methods

- **`_kiro/session/notify`** — emitted by the new `send_message` tool
  (`src/tools/send-message.ts`). Delivers a message into a target session's *steering buffer* and
  notifies the client for UI rendering. Payload: `{sessionId, callerSessionId, message, severity,
  workflowId?}` plus node identity. Carries a **top-level `sessionId`** so the mux routes it
  directly rather than parent-broadcasting. This is the peer-to-peer session messaging primitive
  (cf. `reference_kiro_message_send_semantics`) now wired to workflow nodes.
- **`_kiro/policy/ignore_files_changed`** — client → agent notification, `{files: string[]}`,
  validated (non-array or non-string members are warn-logged and dropped).

## `/tangent` — no new wire surface

Despite the `[V3]` tag, `/tangent` adds **no method**. `createdReason: "tangent"` already existed
in 2.15.0's `CreatedReasonSchema` (`["human","rewind","subagent","tangent"]`). The 2.16.0 delta is
two lines of session metadata:

```js
// A named tangent fork carries a title the user deliberately chose, so mark it user-set …
// so the chosen name is not overwritten on the tangent's first turn. Scoped to tangent forks
// so rewind/subagent forks (which may also pass a title) are unaffected.
...params.createdReason === "tangent" && params.title != null && { titleSetByUser: true }
```

So: named tangents ride `session/fork` (KAS advertises `sessionCapabilities {list, fork}`) with a
user-supplied `title` + `titleSetByUser: true`. The "visual picker" is entirely client-side in
tui.js. Nothing for cyril to model beyond `titleSetByUser` if it ever renders a session list.

## Doc-manifest delta (embedded feature inventory)

138 → 139 documents. Two manifests (88 + 118), `generated_at` 2026-07-30T21:37Z.

- **Added (1):** `settings/tool-search-settings.md` — `[setting] toolSearch`, validated 2026-07-29
- **Removed:** none
- **Revalidated:** `tools/tool-search.md` (2026-04-24 → 2026-07-29), `commands/login.md`,
  `commands/logout.md` (both → 2026-07-17)

**No `/tangent` doc and no workflow doc** — consistent with workflows being an unannounced,
default-off preview. Worth re-diffing next release; a workflow doc appearing is the signal that
Kiro intends to make it public.

Artifacts: `docs/kiro-docs-index-2.16.0-{88,118,merged}.json`.

## What this means for cyril

1. **No breakage.** v2 is frozen at settle; the one changed v2 response is additive and cyril's
   parser skips unknown keys by construction. Nothing to fix to stay working on 2.16.0.
2. **`cyril-6beh` is unblocked.** Its explicit deferral condition — "until an emitter ships" — is
   satisfied, and the shapes are now observable rather than inferred.
3. **The session-level workflow bet was right.** Kiro's engine spawns peer sessions correlated by
   `workflowId`, exactly the model recorded in `project_cyril_session_level_workflows`. Rendering
   a workflow run is *not* a subagent-tracker problem.
4. **Watch handlers change the interaction model.** `github-pr` / `crux-cr` polling means a KAS
   session can sit long-running and wake on external events — a TUI that assumes turn-shaped
   interaction will need an always-on affordance.

## Reproduction

```sh
# v2 settle A/B (offline, HOME-isolated, free)
probe-v2-surface-ab-2.11.0.py <bin> v2-surface-<ver>.jsonl

# v2 /context command A/B (no prompt turn, free)          [NEW this audit]
probe-v2-context-breakdown-ab-2.16.0.py <bin> v2-context-<ver>.jsonl

# KAS host-init A/B (no prompt turn, zero credits)
probe-kas-hostinit-2.15.0.py <bin> kas-hostinit-<ver>.jsonl

# KAS workflow runtime, gate flipped on (no prompt turn)  [NEW this audit]
probe-kas-workflow-runtime-2.16.0.py <bin> kas-workflow-<ver>.jsonl

# doc manifest
extract_doc_manifest.py <bin> docs/kiro-docs-index-<ver>
```

# kiro-cli 2.16.0 wire audit (2026-07-30, vs 2.15.0)

**Verdict: SAFE for cyril's current v2 path — but this is the biggest KAS release since
2.7.1.** The v2 (Rust) surface cyril consumes is unchanged at settle (zero field-path delta) and
across **all 19 command responses swept except `/context`**, whose change is **purely additive**.
Turn traffic carries two further v2 changes, both **additive and both improvements cyril gets for
free**: the post-`/model`-switch `_kiro.dev/metadata` push now carries a recomputed
`contextUsagePercentage` (it carried none on 2.15.0, leaving clients stale), and the `/model`
execute response gained the same field. On the KAS side,
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

### Event payload shapes

Reconstructed from the constructed object literals at the emit sites in
`src/workflow/workflow-runner.ts` and `src/workflow/run-liveness.ts` — the bundle is compiled JS,
so the TypeScript types are erased and these are the literals themselves, not a declared schema.

**Envelope.** Every event is a plain JSON-RPC **notification** (no `id`, no reply). The bridge
flattens the payload straight into `params` and injects `parentSessionId`:

```js
const payload = parentSessionId !== undefined
  ? { ...event.payload, parentSessionId }
  : event.payload;
void emit(KIND_TO_METHOD[event.kind], payload);
```

So `params` = the payload below **plus `parentSessionId`** when the run has one. There is **no
top-level `sessionId`** — these are workflow-scoped, not session-scoped, so cyril's
`RoutedNotification` session-matching will not route them. **`workflowId` is the correlation key
on every event.**

```jsonc
// _kiro/workflow/run_start
{ workflowId, workflowName, inputs, nodeTree: NodeDescriptor[], parentSessionId }

// _kiro/workflow/run_complete
{ workflowId, status, finalState: WorkflowState }

// _kiro/workflow/node_start
{ workflowId, nodeId, nodePath: string[], type,
  agentName?,     // step nodes only
  sessionId?,     // see the double-emit note below
  prompt?,        // step nodes carrying an inline prompt
  iteration?,     // present inside a repeat
  branchId? }     // present inside a parallel

// _kiro/workflow/node_complete
{ workflowId, nodeId, nodePath, status,
  artifacts?,       // omitted when empty
  capturedOutput?,
  failureReason? }

// _kiro/workflow/node_paused
{ workflowId, nodeId, nodePath, reason }

// _kiro/workflow/paused
{ workflowId, pauseReason }

// _kiro/workflow/steps_queued
{ workflowId, pendingSteps: NodeDescriptor[],
  resolution?: { outcome: "applied" | "rejected" | "dropped", reason? } }

// _kiro/workflow/loop_iteration
{ workflowId, loopId, iteration, stopConditionMet: boolean }

// _kiro/workflow/watch_poll
{ workflowId, nodeId, nodePath, outcome, at }   // `at` = deps.clock.now()
```

**`NodeDescriptor`** — appears in `run_start.nodeTree` and `steps_queued.pendingSteps`, built by
`nodeToDescriptor()` (`src/workflow/recipe-plan.ts`). Discriminated on `type`, five variants:

```jsonc
{ nodeId, type: "step",     agentName, modelId?, effortLevel? }
{ nodeId, type: "sequence", steps: NodeDescriptor[] }
{ nodeId, type: "repeat",   steps: NodeDescriptor[], maxIterations, onMaxIterations,
                            stopCondition?, stopWhen? }
{ nodeId, type: "parallel", branches: NodeDescriptor[] }
{ nodeId, type: "watch",    agentName }   // agentName carries the HANDLER id (github-pr / crux-cr)
```

Note the engine implements **five** node types; the seven bundled recipes only exercise `step`,
`repeat`, and `parallel`. `sequence` and `watch` are real but unused by the shipped recipes.

**`WorkflowState`** — the whole persisted run, shipped verbatim in `run_complete.finalState`
(`initState()`, `src/workflow/workflow-state.ts`):

```jsonc
{ workflowId, workflowName, status, inputs, artifacts, capturedOutputs,
  root: NodeState,          // synthetic root, type "sequence", nodeId === workflowId
  createdAt, planRevision,
  parentSessionId,          // observed live; not visible in initState()
  workspacePath }           // observed live; not visible in initState()
```

`NodeState` mirrors `NodeDescriptor` plus runtime fields: `status`, `children[]`, and per-node
`sessionId`, `artifacts`, `capturedOutput`, `failureReason`, `iteration`, `branchId`,
`completionSignal`, `startedAt`, `endedAt`.

**`nodePath`** — array from `nodePathTo()` (`src/workflow/node-tree.ts`), rooted at `workflowId`.
Inside a `repeat`, the segment is **`iter-<n>`, not the child nodeId**:

```
[workflowId, "review-loop", "iter-2", "run-tests"]
```

**Enum domains** (from status assignments across the workflow modules):

| Field | Values |
|---|---|
| workflow `status` | `running` `paused` `completed` `failed` `aborted` — terminal set is `completed`/`failed`/`aborted` per `isTerminalWorkflowStatus()` |
| node `status` | `pending` `running` `paused` `completed` `failed` `aborted` `skipped` |
| `watch_poll.outcome` | `new-activity` `idle` `idle-timeout` `terminal-state` |
| `steps_queued.resolution.outcome` | `applied` `rejected` `dropped` |
| node `type` | `step` `sequence` `repeat` `parallel` `watch` |

Do **not** confuse these with `success`/`warning`/`fault`, which are `send_message` *severity*
values landing on `nodeState.completionSignal` — see the send_message hazard below.

**Eight of the nine shapes are confirmed against live runs** across two probes — see "Live
end-to-end verification" and "Live verification — repeat + watch" below, which between them list
six corrections the runs forced. Only **`node_paused` remains static-only**: it did not fire on
the `repeat`-exhaustion path (see the pairing correction below), and its other emit conditions
(mid-node pause request, `send_message` severity `warning`, throttle retry, stale-run liveness)
were not reachable from a scripted probe.

### Six hazards for any consumer

1. **`run_complete` does not mean the run finished.** It is emitted with
   **`status: "paused"`** as well as with terminal statuses (observed live on the `repeat`
   exhaustion path). Only `completed`/`failed`/`aborted` are terminal, and only those unsubscribe
   the bridge. **Check `status` — never treat the arrival of `run_complete` as end-of-run**, or a
   client will tear down a workflow that is still live and resumable.
2. **`node_start` fires twice per step node.** The first emission precedes session creation (no
   `sessionId`); `executeStep`'s `onSessionCreated` callback re-emits the *same* `nodeId`/
   `nodePath` with `sessionId` populated. The bundle comment says this is deliberate — it lets
   clients map `nodeId → sessionId` for drill-in and per-step approvals without a later signal.
   Structurally this is the same merge-update hazard as `ToolCallUpdate`: merge on
   `(workflowId, nodeId, nodePath)` and guard partial fields, never append. **On the resume path
   the paused node already has a `sessionId` at re-entry, so it rides the first emission and the
   re-emit does not happen — the emission count is not fixed.**
3. **`steps_queued` is overloaded.** It is both "here are the pending steps" (`pendingSteps`
   populated, **no `resolution` key at all** — confirmed live) and a bare acknowledgement of a
   queued-step proposal (`pendingSteps: []` with `resolution.outcome`). Treating empty
   `pendingSteps` as "queue drained" is wrong.
4. **Step completion rides `send_message`, not the workflow protocol.**
   `WORKFLOW_STEP_COMPLETION_PROTOCOL` steering tells the step agent every turn must end with a
   `send_message` call carrying severity `success` (done), `warning` (needs user input → node
   pauses), or `fault` (failed); that severity lands on `nodeState.completionSignal`. A step's
   outcome is therefore decided by a **model-issued tool call** — if the agent omits it, the node
   sits `paused` with reason `"Awaiting next user message on step session."` instead of
   completing. `src/workflow/run-liveness.ts` exists to force-pause stale `running` nodes for
   exactly this reason, emitting `node_paused` + `paused` with `STALE_RUNNING_PAUSE_REASON`. Do
   not treat `node_complete` as a reliable liveness signal.
5. **`paused` and `node_paused` are independent, not paired** (live-verified: `repeat` exhaustion
   emitted `paused` only, `node_paused` zero times). Do not wait for one on the strength of the
   other.
6. **`nodePath` is the only stable node identity.** `node_start` carries `iteration` but
   `node_complete` does not, and a `repeat` re-uses the same `nodeId` every pass — so keying on
   `nodeId` (or on `(nodeId, iteration)` read off `node_complete`) collapses iterations together.
   Note also that `finalState` names iteration wrappers `loop#0`/`loop#1` while `nodePath` uses
   `iter-0`/`iter-1`; **the two schemes do not match** and must be translated.

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

  The recipes exercise `step`, `repeat`, and **`parallel`** — a real DAG scheduler
  (`src/workflow/parallel-scheduler.ts`), not a linear chain. (The engine implements five node
  types; `sequence` and `watch` are real but unused by the bundled set — see the payload-shapes
  section.) `autoresearch` runs `maxIterations: 1000` with `onMaxIterations: "pause"`.

- **`_kiro/workflow/listWatchHandlers`** → 2 handlers with JSON-Schema configs:
  - `github-pr` — polls a GitHub PR for comments/reviews/check status via the `gh` CLI
    (`prRef`, `url`, `pollIntervalSec`, `includeOwnActivity`, `ignoreAuthors`)
  - `crux-cr` — polls an Amazon Crux code review via the `cr` CLI (`crRef`, `crId`, …)

  Both `defaultPollIntervalSec: 60`, `minPollIntervalSec: 30`. These make workflows
  **externally-triggered and long-running**, not just fire-and-forget.

### Live end-to-end verification — a CLIENT-authored DAG executes

Everything above is static (bundle literals + read-only calls). Per
`feedback_kiro_schema_vs_runtime`, that only proves schema-acceptance. So
`probe-kas-custom-dag-live-2.16.0.py` **runs a DAG the client wrote inline** — no model involved
in constructing it — and captures the real notifications. Capture:
`experiments/conductor-spike/kas-custom-dag-2.16.0.jsonl`.

The DAG: one `parallel` node, `joinPolicy: "all"`, two `wf-coder` steps whose prompts forbid tool
use, with a `{{token}}` template variable. **This costs credits** (two real agent sessions); the
whole run took ~4.7s wall-clock.

**Result: it works.** `_kiro/workflow/new` accepted the inline object and
`_kiro/workflow/invoke` ran it to `status: "completed"`. Ten notifications, in order:

```
run_start
node_start   fan    parallel  (no sessionId, no branchId, no agentName)
node_start   alpha  step      (no sessionId)  branchId=alpha
node_start   beta   step      (no sessionId)  branchId=beta
node_start   alpha  step      sessionId=sess_a3d8bb37…  branchId=alpha
node_start   beta   step      sessionId=sess_fd35dac1…  branchId=beta
node_complete alpha  status=completed  capturedOutput=""
node_complete beta   status=completed  capturedOutput=""
node_complete fan    status=completed
run_complete         status=completed
```

Confirmed against the documented shapes:

- **The double `node_start` is real** — `alpha` and `beta` each emitted twice, the second carrying
  `sessionId`. Note both branches emit their *first* `node_start` before either receives a
  session, so a consumer cannot assume start/session pairs arrive adjacently.
- **Steps are peer sessions** — three distinct sessionIds appeared on `session/update`: the parent
  plus one per step. Confirms the architecture section empirically.
- **`nodePath`** came back exactly as modelled: `["wf_a02797…", "fan", "alpha"]`.
- **Optional-field guards behave as read** — the `parallel` node's `node_start` carried no
  `agentName` and no `branchId`; step nodes carried both.
- **`parentSessionId` is injected on every event**, including ones whose payload never sets it.
- **`completionSignal: "success"`** appeared on both step NodeStates in `finalState`, confirming
  the `send_message`-severity mechanism.
- `node_paused`, `paused`, `steps_queued`, `loop_iteration` and `watch_poll` did **not** fire —
  expected, since the DAG has no `repeat`/`watch` node and nothing paused. **Those five payload
  shapes remain static-only, not live-verified.**

Three corrections the live run forced:

1. **`_kiro/workflow/new` returns `{workflowId, initialState}`** — the full `WorkflowState`, not
   the `{workflowId, status}` that the agent-facing `run_workflow` tool returns. A client gets the
   whole initial tree up front and does not need `run_start` to learn the plan.
2. **`WorkflowState` also carries `parentSessionId` and `workspacePath`**, neither of which is set
   in `initState()` — they are attached later, so a static read of that function under-reports.
3. **`NodeState` carries `startedAt`** alongside `endedAt`.

One behaviour worth flagging: **`node_start.prompt` carries the RAW, un-interpolated template.**
The emitted value was literally `"… reporting exactly: alpha-{{token}}"`, not `alpha-OK42`.
Interpolation happens downstream of the notification, so a client rendering `prompt` shows
template source, not what the agent received.

### Live verification — `repeat` + `watch`, and where the model was wrong

The custom-DAG run left five shapes unexercised. `probe-kas-workflow-repeat-watch-2.16.0.py`
drives four of them in three phases; capture:
`experiments/conductor-spike/kas-repeat-watch-2.16.0.jsonl`.

- **Phase A — `watch`.** A single `watch` node on `github-pr` pointed at a **merged** PR. Watch
  nodes are explicitly non-LLM, so **this phase costs zero credits.** Merged ⇒ `terminal-state` on
  the first poll. (`idleTimeoutSec: 35` was set as a bounded safety net, since a failing `gh`
  returns `idle` and would otherwise re-poll forever.)
- **Phase B — `repeat`.** `maxIterations: 2`, `onMaxIterations: "pause"`, with a `fileCheck`
  stopCondition on a file that never exists. Costs 2 trivial agent turns.
- **Phase C — `steps_queued`.** `_kiro/workflow/update` with `action: "replace_remaining"` against
  the paused run from phase B.

Verified exactly as documented:

```jsonc
// watch_poll  (phase A)
{ "workflowId": "wf_286a2d82…", "nodeId": "pr-watch",
  "nodePath": ["wf_286a2d82…", "pr-watch"],
  "outcome": "terminal-state", "at": "2026-07-31T03:54:37.084Z", "parentSessionId": "sess_…" }

// loop_iteration  (phase B) — note the iteration counter is ZERO-INDEXED
{ "workflowId": "wf_6d86e090…", "loopId": "loop", "iteration": 0, "stopConditionMet": false, … }
{ "workflowId": "wf_6d86e090…", "loopId": "loop", "iteration": 1, "stopConditionMet": false, … }

// paused  (phase B)
{ "workflowId": "wf_6d86e090…", "pauseReason": "Repeat 'loop' reached maxIterations.", … }

// steps_queued  (phase C) — the POPULATED-LIST form; no `resolution` key at all
{ "workflowId": "wf_6d86e090…",
  "pendingSteps": [{ "nodeId": "final", "type": "step", "agentName": "wf-coder" }], … }
```

`nodePath`'s `iter-<n>` rule is confirmed verbatim:
`["wf_6d86e090…", "loop", "iter-0", "tick"]`. The double `node_start` also holds inside a
`repeat`.

**Three more corrections the run forced:**

4. **`run_complete` does NOT mean the run finished.** Phase B emitted
   `run_complete` with **`status: "paused"`** — which is not in the terminal set, so the bridge
   stayed subscribed. `run_complete` signals "the runner's execution pass returned", and pausing
   is one way that happens. A consumer treating it as end-of-run will tear down a live workflow.
5. **`paused` and `node_paused` are NOT paired.** The `maxIterations` exhaustion path emitted
   `paused` only — `node_paused` fired **zero** times across the whole probe. The two are
   independent signals, so do not wait for one on the strength of the other.
6. **`node_complete` omits `iteration`.** `node_start` carries `iteration` (0, 1), `node_complete`
   does not. Combined with the repeat re-using the same `nodeId` (`tick`) every pass, **`nodePath`
   is the only reliable node identity across the start/complete pair** — keying on `nodeId`, or on
   `(nodeId, iteration)` taken from `node_complete`, collapses iterations together.

Also worth knowing: in `finalState` the repeat's per-iteration wrappers are synthetic `sequence`
nodes with nodeIds **`loop#0`, `loop#1`** — the `#N` form, whereas the same iteration appears in
`nodePath` as `iter-0`, `iter-1`. **The two identifier schemes do not match**, so a client
correlating `finalState` against streamed events must translate between them.

**A real trap in the documented recovery path.** `run_workflow`'s own description says a
maxIterations-paused run can be advanced by `update_workflow(replace_remaining)`. Live, that call
returned:

```json
{ "workflowId": "wf_6d86e090…", "updated": true, "queued": true,
  "message": "Queued: the remaining steps will be replaced after the current step completes." }
```

…and the replacement **never applied**: the run was paused with no step in flight, so nothing ever
"completed" to trigger the swap. The `final` step never ran and the workflow stayed `paused`. So
`replace_remaining` alone does **not** resume an exhausted loop — it only queues intent. Recovery
needs `resume`/`retry` (or cancel-then-retry) as well.

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
still dormant on a local run. See the next section for what moved underneath.

## Cloud / remote-agent stack (`_kiro/sourceProviders/*`) — wire-frozen, internals churning

**The wire surface did not change in 2.16.0.** Both methods are present and unmodified in the
109-literal set, and all four capability keys still report their dormant local-only values. What
changed is underneath, plus one field pair that finally surfaced.

Full characterization lives in **[docs/kiro-2.12.3-wire-audit.md](kiro-2.12.3-wire-audit.md)**
(where the stack landed, in KAS 0.17.2) — not repeated here. The short version, for context:

- `_kiro/sourceProviders/list` (connection-scoped, session-less, no request fields) and
  `listResources` (`{providerType, cursor?, limit?}`, paged) expose an **account-scoped catalog of
  repo providers — `GITHUB` / `GITLAB` / `MIDWAY`** (MIDWAY = internal-Amazon), each with
  `connectionStatus` + `setupUrl` for OAuth handoff. It is the discovery half of running an agent
  against a cloud-hosted repo in a `cloud-sandbox` execution target (placement `relayed` rather
  than `executedHere`).
- **One env var gates all of it**: `KIRO_REMOTE_SESSIONS_ENDPOINT` (or
  `--remote-sessions-endpoint`) flips **all four caps at once** — `sourceProviders true`,
  `sessionSources` +`remote`, `sessionListScopes` +`user`, `executionTargets` +`cloud-sandbox`.
  No per-cap flags.
- **Dispatch is unconditional**: calling either method without a wired catalog returns a typed
  `-32000 "no source provider catalog is configured"`, **not `-32601`** — so a client cannot
  feature-detect the cloud stack by probing for method-not-found.
- Backend is a **distinct service** from the CodeWhisperer streaming endpoint that serves turns:
  `@amzn/kiro-web-portal-service`, Smithy **RPC-v2-CBOR**, `POST
  /service/KiroWebPortalService/operation/<Op>`, with operation names that do **not** match the
  ACP method names (`sourceProviders/list` → `ListAvailableProviders`, `listResources` →
  `ListProviderResources`). Auth contract is three headers: `Authorization: Bearer`,
  `X-Kiro-Idp`, `X-Kiro-Profile-Arn`.

### What 2.16.0 changed

1. **The cloud vocabulary reached the wire on a purely local run.** `_kiro/sessions/changed`
   now carries `upserted[].source` (`"local"`) and `upserted[].executionTarget.kind` (`"local"`)
   — see the handshake table above. These are the cloud stack's persisted per-session fields; up
   to 2.15.0 they stayed internal. Nothing is enabled by their appearance, but a cyril that
   consumes `sessions/changed` will now see them, and the same fields are what carry `remote` /
   `cloud-sandbox` once the endpoint is set.
2. **The cloud-config transport moved to a generated Smithy client.**
   `src/cloud-config/source/http-cloud-config-transport.ts` and
   `src/cloud-config/source/inert-cloud-config-source.ts` were **removed**, replaced by
   `src/cloud-config/source/smithy-cloud-config-transport.ts` — a `KiroWebBearerServiceClient`
   with the same `addKiroAuthHeaders` middleware, paging `GetConfigManifestCommand` via
   `nextToken` (`MAX_PAGES 1000`, 30s request timeout). Nearly every `src/session/bff-*.ts` module
   changed alongside it (`bff-source-provider-catalog`, `bff-remote-session-source`,
   `bff-remote-agent-link`, `bff-provider-resources`, `remote-sessions-composition`, …).

**Keep these two features distinct:** cloud-**config** (org-pushed configuration manifests) is not
the same surface as source**Providers** (repo-provider catalog). They share the web-portal backend
and the auth contract, which is why they move together, but they are separate features.

Deliberately *not* concluding anything from the removal of `inert-cloud-config-source.ts`. "Inert"
names a no-op source and its deletion is suggestive, but the composition module's bundler-renamed
identifiers did not let us confirm what replaced it — and the capability values are still dormant,
which is the fact that matters. Treat this as churn, not as a launch signal.

**cyril status: unchanged — KAS-8 / deferred.** `cyril-tikf` still stands: model or
explicitly-ignore the four capability keys per the model-full-wire-surface rule. There is no
reason to speak the protocol until there is a cloud feature to speak it for.

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

## Coverage — what this audit checked, and what it did NOT

Recorded so the next audit knows where the holes are rather than re-deriving them.

### Gap-closure passes run after the first draft

- **Every advertised v2 command response, not just `/context`.** The settle A/B cannot see command
  responses at all, and the first pass only executed `/context` because the nm diff pointed there.
  `probe-v2-all-commands-ab-2.16.0.py` executes **16 read-only commands plus 3
  `commands/options` queries** on both binaries and structurally diffs every response. Result:
  **`/context` is the only one that changed** — the other 18 are field-path identical. That
  upgrades "v2 is frozen" from an inference to a measurement.
  Skipped as unsafe/stateful: `quit` `clear` `compact` `rewind` `paste` `reply` `chat` `feedback`.
- **Dark-shipped feature flags.** `KIRO_TEST_MODE` flips Kiro's 0%-rollout flags to treatment, and
  the flag registry names one **`workflows`** (alongside `remote_sandbox`, `c2s`) — so the obvious
  question is whether the v2 Rust engine hides a workflow surface too. Ran the full command sweep
  with it set, on **both** binaries. Findings, all **pre-existing, none 2.16.0-attributable**:
  - **No `workflows` command and no workflow tool appear on v2, even with the flag on.** The
    workflow engine is **KAS-only**. (This also refutes the older note that `KIRO_TEST_MODE` might
    be the only way to reach workflows on the ACP path — the real lever is the KAS
    `settings.workflows.enabled` gate.)
  - `voice` appears as a 25th command under test mode — identically on 2.15.0 and 2.16.0.
  - Test mode deterministically **suppresses backend-served data**: `usage` returns no `data` and
    `commands/options` for `model` returns 0 options (19 without it). Identical both versions, so
    these are test-mode artifacts, not flakiness and not a release delta.
  - `contextUsagePercentage` shifts (3.574 → 2.991 on 2.16.0), confirming the flag really does
    change the prompt/toolset. **Anyone benchmarking cyril must keep `KIRO_TEST_MODE` unset.**

### Turn traffic, via the free mock backend

`probe-v2-turn-traffic-ab-2.16.0.py` closes the biggest hole: no probe in this audit had ever
driven a **turn**, so `_kiro.dev/metadata` was only ever seen in its empty pre-turn form and no
`session/update` turn variant had been observed at all.

`KIRO_MOCK_CHAT_RESPONSE` makes this free and deterministic — it is a **file path** to a JSON
array-of-arrays, one outer element per turn, inner strings streamed as `agent_message_chunk`s.
Two turns plus a model switch, on both binaries, zero credits and no network.
Mock is **strings-only** (object entries panic kiro-cli at `initialize`), so this run still yields
**no tool calls, no permission prompts and no `meteringUsage`** — see the residual gaps below.

Turn-shaped message inventory (identical on both): `session/update:agent_message_chunk` ×4,
`_kiro.dev/metadata` ×7, `_kiro.dev/commands/available`, `_kiro.dev/subagent/list_update`,
`session/prompt` → `stopReason: "end_turn"`. Streamed chunks concatenated identically.

**This is where 2.16.0's third changelog line actually lives** — "context usage percentage now
recalculates when switching models via `/model`" — and it lands in **two** places, both on cyril's
path:

1. **The pushed `_kiro.dev/metadata` after a model switch.** This is the real fix:

   ```jsonc
   // 2.15.0 — post-/model-switch push carries NO percentage; client stays stale
   {"sessionId": "b4d10c45…"}
   // 2.16.0 — recomputed value is pushed
   {"sessionId": "bcc78d36…", "contextUsagePercentage": 0.715399980545044}
   ```

   On ≤2.15.0 a client's cached percentage stayed at the pre-switch value (3.39%) until the user
   manually ran `/context`. On 2.16.0 it self-corrects.
2. **The `/model` execute response gained `data.contextUsagePercentage`.**

**cyril inherits the fix for free and needs no change.** `convert/kiro.rs:271` maps an absent
`contextUsagePercentage` to `None` (deliberately — duration/effort-only frames are a real wire
shape), and `session.rs:132` guards the assignment `if let Some(u) = context_usage`, so the cached
value is retained on sparse frames and updated on populated ones. The stale-toolbar behaviour on
≤2.15.0 was Kiro's, not cyril's.

**Two methodology lessons, both worth carrying forward:**

- **Field-path set diffing across aggregated frames hides per-frame behavioural changes.** The
  aggregate `_kiro.dev/metadata` path set is **identical** (3 paths) on both versions — because
  2.15.0 does emit `contextUsagePercentage`, just on a *different frame*. The differ reported
  "same" and the change was only visible by looking at **which frame** carried the field. A
  set-union diff answers "can this field ever appear", not "does it appear when it should".
- **Executing a command with empty args does not cover its mutating form.** The all-commands
  sweep ran `/model` with `args: {}` (a query) and found no delta; the `data.contextUsagePercentage`
  addition only appears when `/model` is invoked *with a value* to actually switch.

### KAS real-turn traffic (paid A/B) — one turn-only field, plus a falsifier closed

`probe-kas-turn-traffic-ab-2.16.0.py` is the KAS lane that never existed: every other KAS probe
in the tree is targeted at one feature, so nothing captured the whole turn surface in a
binary-parameterized run. There is **no mock backend for KAS** (`KIRO_MOCK_CHAT_RESPONSE` is read
by the two Rust crates; KAS has its own TS client), so **every turn is a paid model call** — this
is 4 turns × 2 binaries, run on cyril's real path (`kiro-cli-chat acp --agent-engine kas`).

Scenario, chosen to maximise surface per paid turn: **read** (fs tool calls + `fs/*` callbacks) →
**exit-code** (non-zero terminal exit) → **write** (fs_write + checkpoints) → **subtask** (the
`agent-subtask` path). Output is written directly in `diff-acp-wire.py`'s record format, so the
legs diff with no adapter.

Surface actually exercised (identical inventory on both binaries): `tool_call` /
`tool_call_update`, including the `_meta.kiro.kind: "agent-subtask"` variants; the full
`session_info_update` kind set — `turn_start`, `turn_end`, `turn_completion`, `context_usage`,
`user_message_id_assigned`, `pending_interaction`, `interaction_resolved`, `focus_update`;
`config_option_update`; `available_commands_update`; and 9 distinct host callbacks
(`fs/read_text_file` ×3, `fs/write_text_file`, `terminal/{create,output,wait_for_exit,release}`,
`session/request_permission` ×2, `_kiro/auth/getAccessToken`, `_kiro/terminal/shell_type`).

**The one genuinely new turn-only field:**

```jsonc
// session/update :: session_info_update, _meta.kiro.kind == "turn_completion"
{"promptTurnSummaries": [{"unit":"credit","unitPlural":"credits",
                          "usage":0.10456622736318409,"usedTools":["read_file"]}],
 "elapsedTime": 4442, "status": "success",
 "requestIds": ["fdc505a9-…","9edf2750-…"]}          // <-- NEW in 2.16.0
```

`requestIds[]` appears on **`turn_completion` only** — 4/4 such frames on 2.16.0, **0/4 on
2.15.0** — and on no other kind (`context_usage`, `turn_end`, `turn_start`,
`user_message_id_assigned`, `pending_interaction`, `interaction_resolved`, `focus_update` all
carry none, on both). It sits beside the existing metering payload, so it reads as backend
request correlation for the turn.

**cyril action:** `crates/cyril-core/tests/fixtures/kas/session_info_update_turn_completion.json`
models `promptTurnSummaries`/`elapsedTime`/`status` but **not `requestIds`**. Per the
model-full-wire-surface rule that fixture is now an incomplete sample of the real frame.

Everything else the differ reported was already known from the cheaper probes — `sessions/changed`
`source`+`executionTarget.kind`, `replayMarking`, `_meta.workflowsEnabled`,
`effortLevels`/`defaultEffortLevel` — which is itself a useful result: **the expensive lane
confirmed the cheap lanes rather than contradicting them.** `tool_call`, `tool_call_update`,
`agent_message_chunk`, `turn_end` and every host-callback shape are **byte-stable** across the two
binaries.

One detail the turn A/B added to the effort story: the advertised `effortLevels` ladder includes
**`"none"`**, and only for the GPT models — `GPT 5.6 Sol` / `Terra` / `Luna` all report
`["none","low","medium","high","xhigh","max"]` with `defaultEffortLevel: "high"`. That is direct
evidence for **cyril-8yka** (stale effort badge when a GPT model reports `effort: "none"`):
`none` is a *legitimate advertised level*, not a degenerate value to paper over.

**Residual falsifier CLOSED.** `reference_kiro_terminal_wait_exit_reply_shape` recorded an
outstanding live check: reply to `terminal/wait_for_exit` with the **flat** typed shape and prove
KAS surfaces a non-zero code. Turn 2 does exactly that — host exits **3**, we reply
`{"exitCode": 3, "signal": null}` (flat, no `exitStatus` wrapper), and the agent answers *"The
exit code was 3."* on **both** binaries. The nested shape in
`probe-kas-fs-terminal-host-2.10.0.py` remains the trap that memory describes.

**A live-confirmed KAS quirk, and a cyril bug reproduced.** `terminal/create` sends the command as
a **single shell string with no `args` array**:

```jsonc
{"sessionId":"sess_…","command":"sh -c 'exit 3'","cwd":"/…"}
```

A host that exec's `[command, *args]` argv-style looks for a file literally named `sh -c 'exit 3'`
and fails `ENOENT`. The first run of this probe did exactly that and produced exit 1 — which is
the defect **cyril-6bol** already tracks ("create runs argv with no shell"). A correct host runs
the string through a shell when `args` is absent. Worth noting the first run's falsifier *looked*
like a pass (the agent said "3" by reasoning about the intended command) — the verdict logic now
requires the **host** to have actually produced the code before it will report PASS.

### Residual gaps — genuinely not covered

- **Turn traffic is only PARTLY covered** (see the mock-backend section above). Agent text and
  turn lifecycle are now verified on v2; what the strings-only mock cannot reach remains
  unchecked: **tool-call notifications** (`session/update` `tool_call`/`tool_call_update`),
  **`session/request_permission`**, **thought chunks**, and the **`meteringUsage[]`** /
  `refusal` fields on `_kiro.dev/metadata` (mock turns are zero-credit, so no metering frame is
  ever produced). Reaching those needs a real turn — i.e. credits — or a fake bridge.
  `_kiro.dev/subagent/list_update` was still only ever seen in its empty form.
- **KAS turn traffic is now covered** (paid A/B, see above) for text, tool calls, subtasks,
  permissions, fs + terminal callbacks and all eight `session_info_update` kinds. Still unexercised
  there: MCP tool calls, thought/reasoning chunks, `_kiro/workflow/*` during a *prompted* turn (the
  workflow runs were invoked directly), error/throttle paths, and `node_paused`.
- **`nm` cannot detect new fields on existing structs** — a documented limit of the method. Only
  the live A/B can, and only for messages the probe actually elicits. Anything on a message shape
  not exercised above is unchecked by construction.
- **Backend axis untested.** All captures were same-day, which isolates the *binary* axis by
  design. A backend rollout can add fields with no version change at all (this is exactly how
  `meteringUsage[]` appeared in 2026-05). Detecting that needs a **same-binary re-capture weeks
  apart**, which no single release audit can do.
- **KAS `session_info_update` kinds unverified.** `context_usage` and `turn_end` are pushed
  during turns; the host-init probe stops before the first prompt. `src/utils/context-usage-
  breakdown.ts` being `cmp`-identical is good static evidence, but it is not a wire observation.
- **Five `_kiro/workflow/*` shapes are live-verified, `node_paused` is not** — see the repeat +
  watch section.

## The KAS gate surface — the single most productive lane

Written down because it is the through-line of this whole audit. **Almost every large
finding came from flipping a gate, not from spending more turns.** The workflow engine, the
hooks subsystem, and the paginated fs dialect were each one capability or settings key away,
and each was found by accident. The gates are enumerable, so they should be enumerated.

### Where the gates live (four distinct channels — they are not interchangeable)

| Channel | Read as | Gates |
|---|---|---|
| `initialize.clientCapabilities.fs._meta.kiro.*` | `readFile`, `writeFile`, `stat`, `readDirectory`, `delete` | the `_kiro/fs/*` dialect (paginated) vs plain ACP `fs/read_text_file` |
| `initialize.clientCapabilities._meta.kiro.*` | `secretStorage`, `openExternalUrl`, `knowledge`, `infrastructureSafety`, `c2sViews`, `hooks:{enabled,v2}` | whole subsystems |
| `initialize._meta.kiro.settings` | 21 keys, `{enabled: bool}` | **overrides BACKEND feature flags** via `bridgeFeatureFlags()` |
| `session/new._meta.kiro.settings` | same keys | session-scoped only — does **not** reach connection-scoped gates |

The last two being different is load-bearing: `workflows` works from either, but anything read
through `getModelConfigProvider().isFeatureEnabled()` only responds to the **initialize** form.
An early probe sent `subagentOrchestration` at `session/new` and wrongly concluded the feature
was backend-gated.

Two naming traps cost real probe time:

- **`hooks` must be an object.** `{enabled: true, v2: true}` — the bundle reads
  `kiroMeta?.hooks?.enabled`, so the boolean `true` makes it `undefined` and silently skips
  the entire block. Two probes concluded the hooks path was dead on that basis.
- **The capability wire name ≠ the resolved field name.** `resolveCapabilities()` maps
  `fs._meta.kiro.readFile` → `kiroFsReadFile`. Advertising the *resolved* name at top level
  does nothing.

### Settings sweep result (21 keys, one arm each, free)

Five change the handshake:

| Flag | Effect |
|---|---|
| `workflows` | `workflowsEnabled` **plus four slash commands**: `workflow-run`, `workflow-status`, `workflow-cancel`, `workflow-resume` |
| `fta` | adds mode `functional_task_alignment` (7 → 8 modes) |
| `goal` | adds the `goal` command |
| `specPlan` | `specPlanEnabled` |
| `steeringSupervisor` | `steeringSupervisorEnabled` — a `session/new._meta` field that **does not exist at all** by default |

The workflow slash commands are worth dwelling on: every workflow probe in this audit drove
`_kiro/workflow/*` directly and **never noticed the user-facing command surface appear**.
Driving a protocol directly can hide the product feature built on top of it.

No handshake delta from the other 13 (`checkpoint`, `codeIntelligence`, `compaction`,
`disableAutoCompaction`, `inlineAgents`, `knowledge`, `largeToolOutputHandler`,
`semanticReview`, `sessionEviction`, `tangentMode`, `thinking`, `todoList`, `toolSearch`) —
but a handshake sweep cannot see in-turn behaviour, so that is "not observed here", not
"no effect".

### Client persona — `clientInfo.name`, not `_meta.kiro.clientName`

`resolveAgentContext()` reads the **standard ACP `initialize.params.clientInfo.name`**;
unrecognised or absent falls through to **`kiro-ide`**. Every capture in this repo was taken
under the IDE persona, because no probe ever sent `clientInfo`.

Measured across four arms (free): the **advertised surface is persona-invariant** — identical
`authMethods`, modes, `configOptions`, `extensionMethods`, commands and pushes. What changes
is the **system prompt** (context usage on an empty session: `kiro-ide` 0.9%, `kiro-cli` 0.8%,
`kiro-web` 0.8%) and, for `kiro-web` only, **two tools run at session start**
(`get_learnings_for_prompt`, `get_steering_files` — the `honorsRepositories()` branch).

Because the surface is persona-invariant, persona does **not** explain
`OrchestrateSubAgent`/`userInput`/`openExternalUrl` being absent. Those conclusions stand.
cyril should send `clientInfo {name: "kiro-cli"}` → **cyril-df5l**.

### Two results this audit records as INCONCLUSIVE, not negative

Both are limits of the probe, not evidence about the feature — flagged so a later reader does
not cite them as settled:

- **`subagentOrchestration`** showed no handshake delta even through the initialize bridge,
  but the surface it selects is chosen during *prompt construction*
  (`getDelegationToolId(getModelConfigProvider().isFeatureEnabled(…))`), which a
  handshake-only sweep cannot observe. One paid turn with the flag set at initialize would
  settle **cyril-ucii**.
- **`infraSafetyMonitor` / `infraSafetyEnforce`** showed no delta, but the real gate is
  `clientSupportsSafety && (monitor || enforce)` and the sweep arms **did not advertise
  `infrastructureSafety`** — half the condition was missing. Re-run before concluding
  anything for **cyril-3ald**.

### The standing limit: elective mechanisms

Three separate surfaces could not be provoked by prompting — `OrchestrateSubAgent`,
workflow `node_paused`, and `_kiro/userInput` / `_kiro/openExternalUrl`. In each case the
mechanism is **elected by the model**, so no prompt reliably forces it. Treat this as a
methodology limit rather than re-attempting per method. (Hooks *looked* like a fourth case and
was not — it was a client-advertisement bug on our side.)

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

# EVERY read-only v2 command + options queries (free)     [NEW this audit]
probe-v2-all-commands-ab-2.16.0.py <bin> v2-allcmd-<ver>.jsonl
# ...and again with KIRO_TEST_MODE=1 to sweep dark-shipped flags

# v2 TURN traffic via the free mock backend (free, offline)  [NEW this audit]
probe-v2-turn-traffic-ab-2.16.0.py <bin> v2-turn-<ver>.jsonl

# KAS full-surface REAL turns — COSTS CREDITS (4 turns/binary) [NEW this audit]
probe-kas-turn-traffic-ab-2.16.0.py <bin> kas-turn-<ver>.jsonl

# --- KAS surface mapping (all FREE unless noted) ---------------------------
probe-kas-rpc-sweep-2.16.0.py          <bin> kas-rpc-sweep-<ver>.jsonl   # 1 turn
probe-kas-rpc-corrections-2.16.0.py    <bin> kas-rpc-corrections.jsonl
probe-kas-pushed-methods-2.16.0.py     <bin> kas-pushed-<ver>.jsonl      # 3 turns
probe-kas-v2hooks-2.16.0.py            <bin> kas-v2hooks-<ver>.jsonl     # 1 turn
probe-kas-client-persona-2.16.0.py     <bin> kas-persona-<ver>
probe-kas-settings-sweep-2.16.0.py     <bin> kas-settings-<ver>
probe-kas-safety-fork-load-2.16.0.py   <bin> kas-safety-fork-load.jsonl  # 1 turn
diff-acp-wire.py kas-turn-2.15.0.jsonl kas-turn-2.16.0.jsonl \
    --label-old 2.15.0 --label-new 2.16.0

# KAS host-init A/B (no prompt turn, zero credits)
probe-kas-hostinit-2.15.0.py <bin> kas-hostinit-<ver>.jsonl

# KAS workflow runtime, gate flipped on (no prompt turn)  [NEW this audit]
probe-kas-workflow-runtime-2.16.0.py <bin> kas-workflow-<ver>.jsonl

# LIVE client-authored DAG execution (COSTS CREDITS — two real agent sessions)
probe-kas-custom-dag-live-2.16.0.py <bin> kas-custom-dag-<ver>.jsonl

# LIVE repeat + watch + steps_queued (watch phase is FREE; repeat costs 2 turns)
probe-kas-workflow-repeat-watch-2.16.0.py <bin> kas-repeat-watch-<ver>.jsonl

# doc manifest
extract_doc_manifest.py <bin> docs/kiro-docs-index-<ver>

# ADDENDUM: v2 /voice ACP command behavior + gate (free, no turn)
probe-v2-voice-acp-2.16.0.py <bin> v2-voice-2.16.0.jsonl
```

## Addendum (2026-08-01) — the test-mode `/voice` command, followed up

The flag sweep above noted `/voice` appearing as a 25th advertised command under
`KIRO_TEST_MODE` and parked it as "pre-existing, identical on 2.15.0". Following up
(`probe-v2-voice-acp-2.16.0.py` + binary/bundle archaeology) reframes the voice picture the
project had on record:

- **The advertised entry**: `{"name": "/voice", "description": "Voice input mode for hands-free
  interaction", "meta": {"subcommands": ["start", "stop", "status"]}}`.
- **The gate is advertise-only.** `kiro.dev/commands/execute` with `{"command": "voice"}` is
  answered identically with and without `KIRO_TEST_MODE` — the handler is always wired on the
  ACP path; the flag only controls whether it is listed. Every shape and subcommand returns
  `{"success": false, "message": "Voice mode is not supported on this platform"}` — a clean
  structured refusal, no crash, so cyril needs nothing defensive here.
- **"Platform" means the Linux build, and always has.** The Linux `kiro-cli-chat` contains zero
  Whisper inference machinery at 2.8.1 *and* 2.16.0 (no onnxruntime tokens, no model/tokenizer
  markers), and no binary of the three (`kiro-cli`, `kiro-cli-chat`, `kiro-cli-term`) parses a
  `voice` subcommand. tui.js's local spawn path (`kiro-cli voice --ptt`, via
  `KIRO_CHAT_CLI_BIN`) is dead code on Linux. This corrects the earlier 2.8.1 voice audit, which
  inferred a functional local path on Linux from strings evidence (the "~2919 `ort` symbols"
  claim was substring contamination — sort/port/abort).
- **Verified on the Windows-native build: the engine is really there.** Downloaded the 2.16.0
  MSI (sha256-matched against `prod.download.cli.kiro.dev/stable/latest/manifest.json`,
  extracted `kiro-cli.exe` with `msiextract`): **2,419 onnxruntime tokens** (statically linked
  `ort`, build paths `D:\a\ort-artifacts\...`), compiled-in `voice.rs` / `voice_handler.rs` /
  **`voice_serve.rs`**, and the "Voice mode is not supported on this platform" string is
  **absent entirely** — the refusal is the Linux stub's message, not a shared runtime check.
  This is a per-platform `cfg` split: Linux ships a stub; Windows (and macOS) ship the engine,
  including the `voice-serve` server side.
- **Two hidden, auth-gated router subcommands missed by every prior audit** (present since
  **2.6.0**): `voice-cloud-setup` — "Set up voice mode for a cloud desktop" — and `voice-serve`
  — "Start a voice recording server". tui.js's remote-path error message ties them together:
  "Voice server not reachable. Run `kiro-cli voice-cloud-setup <hostname>` on your local
  machine first." The intended topology: the machine with the microphone runs `voice-serve`
  (the plain-HTTP `POST /voice/record/stream` SSE server tui.js consumes via
  `KIRO_VOICE_SERVER_URL`), and a cloud desktop's Kiro connects to it. On Linux both
  subcommands **dead-forward**: the router accepts them (auth precheck runs first) and forwards
  to `kiro-cli-chat`, which rejects them as unrecognized — verified logged-in on 2.16.0. Prior
  "voice is FROZEN" re-checks (2.11.0, 2.13.0) grepped a fixed vocabulary of known tokens and
  so could never see these — a fixed-vocabulary blind spot to avoid in future audits.
- **Kiro instruments voice over ACP.** The telemetry enum blob
  `V1V2ACPLocalWhisperRemoteServerSlashCommandPTTContinuousVoiceStandalone` names **ACP** as a
  voice-frontend dimension alongside V1/V2/Standalone — the intent for voice-over-ACP exists
  even though the Linux handler refuses today. (The apparent `LocalWhisper` 2→1 string drop at
  2.15.0 is an LTO re-glue artifact in an MCP/elicitation type blob, not a removal — same class
  of artifact as 2.11.0's phantom `voiceShare`.)

**Cyril impact:** cyril is a native Windows app; its current WSL spawn of `kiro-cli` is a legacy
stopgap from before Kiro shipped a Windows binary, and the ROADMAP explicitly declares that
mission obsolete — native-any-OS spawn is the direction. That splits the voice picture by spawn
path, not by "cyril's platform matrix": through the WSL (Linux-binary) path Kiro voice is a
structured refusal, but a native-Windows cyril spawning `kiro-cli.exe acp` talks to a build that
carries the full voice engine — so if the rollout flag flips (or under `KIRO_TEST_MODE`), `/voice
start|stop|status` over ACP plausibly *works* there (unverified at runtime; needs a Windows
host). CN2 (`cyril-voice`) remains justified for Linux and for non-Kiro agents, and gains an
interop target: Kiro's remote-voice wire contract (`POST ${url}/voice/record/stream` SSE +
`/voice/record/stop`) is plain HTTP with a real server implementation (`voice-serve`) on
desktop builds — a cyril transcriber speaking that contract could serve both ecosystems.
**Open probe for the Windows track:** on a Windows host, run `kiro-cli.exe acp` with
`KIRO_TEST_MODE=true` and execute `/voice start` — if it streams, cyril gets voice-over-ACP on
native Windows for free.

```sh
# (reproduction for this addendum)
probe-v2-voice-acp-2.16.0.py ~/.local/share/kiro-research/binaries/2.16.0/kiro-cli-chat v2-voice-2.16.0.jsonl
```

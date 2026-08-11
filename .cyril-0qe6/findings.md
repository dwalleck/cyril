# cyril-0qe6 — probe findings (2026-08-11, kiro-cli 2.16.2 / KAS)

Two live probes + bundle carving, closing the gaps the issue flagged: the
run-persistence semantics behind reattach-on-demand (the 2.16.2
`run-disk-operations.ts` hazard note) and the response shapes the command
family must model. Probes:

- `experiments/conductor-spike/probe-kas-workflow-reattach-2.16.2.py`
  (capture `logs/kas-workflow-reattach-2.16.2.jsonl`) — persistence + list +
  load/inspect + resume-vs-orphan.
- `experiments/conductor-spike/probe-kas-workflow-reattach2-2.16.2.py`
  (capture `logs/kas-workflow-reattach2-2.16.2.jsonl`) — whole-tree kill +
  immediate resume from a fresh process.

## Oracle

The run objects on disk, read directly with the filesystem, against the live
JSON-RPC answers — independent mechanisms. A before/after snapshot diff over
the fake HOME + workspace located the store; the state files' workflowIds and
statuses were then compared item-by-item against `_kiro/workflow/list`.
Agreement on every slice: round 1 `failed`=`failed` for the completed-failed
run and `running`=`running` for the orphan-owned run; round 2 `completed` on
disk = `completed` from RPC after the cross-process resume. The round-2 kill
also used an independent check (`ps -g <pgid>`) to prove the owner tree was
dead before the resume that succeeded.

## What I learned (that I did not know before)

**A KAS workflow run is a per-user, workspace-hashed directory of four files,
guarded by a pid-stamped heartbeat; a dead owner is claimable instantly, a
live one refuses with its pid — and killing the spawned `kiro-cli` does NOT
kill the engine, so "crashed client" and "dead run" are different states.**

## Findings

### F1 — runs persist at `~/.kiro/sessions/<workspace-hash>/workflows/<wf_id>/`

Four files per run: `workflow-state.json` (the WorkflowState `list`/`load`
serve), `workflow-definition.json`, `sessions.json` (step-session map),
`run.beat` (liveness heartbeat). Per-user HOME storage keyed by a hash of the
workspace path — NOT in the workspace's own `.kiro/`. Two different temp
workspaces produced two different hash dirs; `list {workspacePaths:[cwd]}`
scoped correctly to each.

### F2 — reattach-on-demand is real, and better than spec'd

Round 2: `killpg(SIGKILL)` of the whole agent tree mid-run (only a zombie
left, verified via ps), then a FRESH process on the same workspace:

- `list` shows the abandoned run as **`"paused"`** (the startup sweep
  reconciles a stale running state — `workflow.sweep.reconciling_stale_run`).
- `resume {workflowId}` succeeded **immediately** — first attempt, 0s wait.
  The liveness assessor short-circuits on a dead pid
  (`workflow.liveness.stale_dead_pid` → verdict "stale", no beat-age wait).
- The resumed run streamed its full lifecycle to the late-attached client
  (`run_start`, `node_start`, `node_complete`, `run_complete`) and reached
  `completed`. AC4's flush-on-subscribe holds cross-process.

### F3 — a live foreign owner refuses resume, naming itself

Round 1 killed only the spawned `kiro-cli` wrapper. The
`kiro-cli-chat → node acp-server.js` chain survives as an orphan that keeps
beating AND keeps driving the run. `resume` from a fresh process is then
refused: `-32603` with details `"Workflow 'wf_…' appears to be running in
another process (owner pid <pid>, liveness verdict: live); refusing to load
it here. Retry after that process releases it or its run goes stale."`
`load` and `inspect` still answer read-only against a foreign-owned run.
Ownership mechanics (carved from the 2.16.2 bundle): `run.beat` mtime +
`{pid, instanceId, stampedAt}` stamp; verdict live/stale/defer;
`staleAfterMs = beatIntervalMs × 4.5`, beat default 30s (liveness module's
`validateBeatIntervalMs(3e4)`), env override `KIRO_WORKFLOW_BEAT_INTERVAL_MS`;
dead pid → instant stale.

**Design consequence:** `/workflow resume` must surface the refusal text
verbatim (it is precise and actionable), and cyril must kill the WHOLE spawn
tree on shutdown or its own crashed sessions will hold runs hostage as live
orphans (cyril-0pms added reaping for clean shutdown; SIGKILL of cyril still
orphans the chain — the refusal is then correct behavior, not a bug).

### F4 — the verb catalog and shapes (2.16.2, all gate-off)

14 request verbs carved from the bundle: `list`, `listRecipes`,
`listWatchHandlers`, `new`, `invoke`, `load`, `inspect`, `resume`,
`resumeAll`, `pause`, `cancel`, `retry`, `delete`, `update`. (`inspect` is
the status verb the issue's `/workflow status` maps to.) Shapes observed
live:

- `list {sessionId, workspacePaths}` → `{runs: [{workflowId, name, status,
  createdAt, updatedAt, startedAt, endedAt?, parentSessionId}]}`.
  Omitting `workspacePaths` → `-32603 "workspacePaths is not iterable"`
  (re-confirmed on 2.16.2).
- `load {workflowId}` / `inspect {workflowId}` → `{workflowId, state:
  WorkflowState, nodePlan, stepSessions}` (state = the cyril-6beh shape incl.
  `root` tree, `capturedOutputs`, `workspacePath`).
- `resume {workflowId}` / `invoke {workflowId}` → `{workflowId, status:
  "running"}`.
- run statuses observed: `running`, `paused`, `completed`, `failed`.

### F5 — the auth-expiry landmine hits workflow steps first (cyril-taba)

Round 1's trivial run FAILED: the probe served the stored token verbatim and
it was past expiry — the step's node_complete carried
`failureReason: "Authentication token is invalid: Host refresh callback
returned token already inside 180000ms refresh buffer"`. The control plane
(new/invoke/list/load) never touches the backend and works with a stale
token; the STEP is what dies. cyril's own responder has the same landmine
(filed: cyril-taba). Not this issue's to fix, but `/workflow` error surfacing
must show per-node `failureReason` — it is precise when it happens.

### F6 — substrate claims verified in cyril's code

- `BridgeCommand::ExtMethod` discards success responses
  (`bridge.rs` `ExtMethod` arm: only the `Err` half of `conn.ext_method()`
  is inspected) — AC2's gap is real.
- `ListKasHooks` (bridge.rs ~1420-2090 region) is the response-carrying
  template: dedicated variant with `session_id` + `workspace_paths`, awaited
  `ext_method`, typed parse, success → typed notifications, failure →
  `BridgeError`, and a `#[cfg(not(feature = "kas"))]` arm that answers
  instead of dangling.

## Residual unknowns (deliberate, scoped)

- `pause`, `cancel`, `retry`, `delete`, `update`, `resumeAll` still not
  individually exercised gate-off (ADR-0011's caveat stands: treat
  `-32601/-32603` as "re-probe the gate").
- The `"paused"` status a swept run shows is indistinguishable in `list`
  from a deliberately-paused run (no "abandoned" marker surfaced) — status
  display should not over-interpret it.
- Watch-parked runs auto-resume at sweep (`auto_resuming_watch_parked_run`)
  — a fresh cyril session on a workspace with a parked watch run may see a
  run start WITHOUT any client invoke. The tracker must tolerate unsolicited
  run_start (cyril-6beh's state machine already accepts events for unknown
  runs).

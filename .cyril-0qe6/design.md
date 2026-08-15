# cyril-0qe6 — `/workflow` command family: falsifiable design

Implements ROADMAP W1's user-facing half per ADR-0011: cyril's own `/workflow`
commands driving `_kiro/workflow/*` directly, gate never set, runs as
persisted workspace objects reattached on demand. Grounded in
`.cyril-0qe6/findings.md` (probes F1–F6); every wire fact below is
live-verified on kiro-cli 2.16.2 unless marked otherwise.

## Purpose

Seven subcommands, all KAS-only, all client-driven (the model never gains
`run_workflow`):

| Subcommand | Wire verb(s) | Notes |
|---|---|---|
| `/workflow recipes` | `listRecipes` | 7 bundled + workspace `.workflow.json` files |
| `/workflow list` | `list` | always sends `workspacePaths: [workspace_root]` |
| `/workflow run <ref> [k=v …]` | `new` → `invoke` | `invoke` only after `new` succeeds |
| `/workflow attach <id>` | `inspect` | seeds the tracker with the run's snapshot |
| `/workflow status [<id>]` | none / `inspect` | no-arg renders tracker state, no wire call |
| `/workflow cancel <id>` | `cancel` | reply is `{ok, previousStatus}`; run lists `aborted` after |
| `/workflow resume <id>` | `resume` | refusal text (live foreign owner) surfaces verbatim |

## Architecture

One new response-carrying bridge command, following the `ListKasHooks`
template exactly (awaited `ext_method`, typed parse, notify on every path,
`#[cfg(not(feature = "kas"))]` arm answers `BridgeError`):

```
WorkflowCommand (commands/, KAS-only registration)
  └─ BridgeCommand::Workflow { session_id, workspace_paths, op: WorkflowOp }
       └─ bridge: ext_method(verb, params) → parse reply
            ├─ ok, carries full state  → Notification::WorkflowSnapshot(Box<WorkflowSnapshot>)   [1st]
            │                            + Notification::WorkflowCommand(Box<WorkflowCommandOutcome>) [2nd]
            ├─ ok, summary-only        → Notification::WorkflowCommand(outcome)
            └─ error (any path)        → Notification::WorkflowCommand(Failed{op, code, details})
```

- **`Notification::WorkflowSnapshot`** is consumed exactly once by the App —
  `WorkflowTracker::apply_snapshot()` — and never forwarded, mirroring the
  `Notification::Workflow` doctrine. Sent *before* the display notification
  (hooks precedent: state lands before the message that references it).
- **`Notification::WorkflowCommand`** rides the normal dual routing;
  `SessionController` ignores it, `UiState` renders it via a pure formatter
  (all display text lives in cyril-ui).
- `/workflow run` performs two awaited calls in one bridge op (`new`, then
  `invoke {workflowId}` only on success); each failure path notifies.
  Lifecycle events of the launched run then arrive via the existing
  `Notification::Workflow` pipeline (cyril-6beh) — this feature adds no
  second consumer.
- **Run-ref mapping** (`/workflow run <ref>`): `bundled://…`/`generated://…`
  pass verbatim as `workflowPath`; a ref containing `/` or ending
  `.workflow.json` is absolutized against the workspace root; a bare word
  becomes `bundled://<word>`. Trailing `key=value` tokens become string
  inputs (`{inputs: {key: "value"}}`).
- **AC5 suppression** lives in `convert/kas.rs`: the four gate-advertised
  command names (`workflow-run`, `workflow-status`, `workflow-cancel`,
  `workflow-resume`) are dropped from available-commands conversion with a
  debug log (Kiro dialect quirk → Kiro converter, per CLAUDE.md).
- **Error surfacing**: the bridge extracts JSON-RPC `error.data.details`
  when present (the live-owner refusal and the `workspacePaths` error both
  put their actionable text there; `error.message` is just
  "Internal error").
- Reply parsing extends `convert/kas/workflow.rs` (`WireSnapshot` →
  `WorkflowSnapshot` already exists for `run_complete.finalState`; the
  `inspect.state` payload is the same shape).

## Input shapes

1. **Subcommand token**: absent, `recipes`, `list`, `run`, `attach`,
   `status`, `cancel`, `resume`, unknown → usage message for absent/unknown.
2. **Id/ref argument**: absent where required (usage), present-valid,
   present-unknown-to-agent (agent error surfaces), run-ref forms: bare word,
   `bundled://`, `generated://`, relative path, absolute path, `k=v` tail
   (zero, one, many; duplicate keys last-wins).
3. **Session**: `None` (no active session → message, no send) / `Some`.
4. **Engine/feature**: KAS engine (registered), v2 engine (not registered),
   `kas` feature off (bridge arm answers `BridgeError`).
5. **Reply**: success; JSON-RPC error with `data.details`; error without
   `details`; transport error; malformed success payload (parse error →
   `Failed` outcome, never silent).
6. **`list` runs**: empty, one, many; entries with/without `startedAt`
   (never-invoked run lacks it — live-observed) and `endedAt` (only terminal
   runs have it) → both `Option`, no sentinels.
7. **Run status strings**: `running`, `paused`, `completed`, `failed`,
   `aborted` (all live-observed), unknown string → tolerated per-entry
   (warn + skip that entry), never kills the whole listing.
8. **Recipes**: bundled-only (7), bundled + workspace file (source = absolute
   path), defensive empty.
9. **Snapshot vs tracker**: run unknown to tracker; run active in tracker;
   run terminal in tracker with same status; terminal with different status
   (conflict error surfaces, state unchanged).

Out of scope shapes: inline DAG objects for `new` (authoring goes through
`.workflow.json` files / the `kiro-workflow-authoring` skill — boundary, see
Negative space); non-string input values in `k=v` (strings only in v1 — the
engine coerces; revisit only if a bundled recipe demands it, tracked with
the extended-verb surface in cyril-2ibk).

Subtractive sweep (2b): the change is purely additive — it adds a new
`BridgeCommand` variant and awaits its replies inline in the single-consumer
bridge loop exactly as `ListKasHooks` already does, removing no
serialization point, guard, or ordering; concurrent peer-session approval
contention pre-exists and is tracked at cyril-z4eo.

## Claims and falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | All seven wire verbs route with the gate off on 2.16.2 (`workflowsEnabled=false`) | live A/B probes; any `-32601`/gate error falsifies | live agent + captures (`kas-workflow-reattach*-2.16.2.jsonl`, `kas-workflow-cancel-gateoff-2.16.2.jsonl`) | done | **passed** | `manual` — re-probe per kiro release audit (needs approval; environment claim, not cyril code) |
| 2 | Every `list` request cyril sends carries non-empty `workspacePaths` | omit it → agent answers `-32603 "workspacePaths is not iterable"` (probe Q0, passed); unit asserts the built params | capture + serde of built request | 5m | probe passed | unit `bridge` test asserting `workspacePaths == [root]` in built params |
| 3 | Every `Workflow` bridge op emits ≥1 notification on every path (success, serialize-fail, transport-fail, agent-error, malformed-reply) | drive each path in a bridge unit harness; a path with zero notifications falsifies | mpsc receiver contents | 30m | pending | unit tests, one per path (buggy impl: log-and-continue on error — the current `ExtMethod` behavior) |
| 4 | An `inspect` reply parses to `WorkflowSnapshot` and seeds the tracker (`/workflow status` sees it afterward) | feed the captured live reply bytes; tracker.get(wid) `None` or wrong status falsifies | capture fixture (real bytes, not hand-built) | 30m | pending | unit test on fixture `workflow-inspect-reply-2.16.2.json` (buggy impl: dropping `root` tree or mis-mapping status) |
| 5 | A snapshot conflicting with a terminal tracker run surfaces an error and changes nothing | seed tracker terminal `completed`, apply `failed` snapshot; silent acceptance or state change falsifies | tracker state + emitted warn/message | 15m | pending | unit test (buggy impl: ignoring `apply_snapshot`'s `Err`) |
| 6 | `/workflow run` calls `new` then `invoke` only on success, and cyril never sends `workflows.enabled` anywhere | unit: force `new` error → assert no `invoke` sent; grep-test initialize/session-new builders for `"workflows"` | recorded outgoing frames in harness | 20m | pending | unit tests (buggy impl: unconditional invoke; settings crept into handshake) |
| 7 | A JSON-RPC error with `data.details` surfaces the details text verbatim to the user | feed the captured live-owner refusal frame; a message reading only "Internal error" falsifies | captured error frame | 15m | pending | unit test on error formatter (buggy impl: `e.to_string()` only — today's `ExtMethod` behavior) |
| 8 | The four gate-advertised `workflow-*` commands never reach autocomplete under KAS | feed available-commands containing them + 3 others; any of the four surviving, or a 5th name dropped, falsifies | filtered set contents | 15m | pending | unit test in `convert/kas.rs` (buggy impl: no filter, or prefix-match dropping `workflow-creator`-style names) |
| 9 | `/workflow recipes` renders every recipe's name + source (7 bundled fixture; workspace file shows its path) | render captured listRecipes reply; a missing name or pathless workspace recipe falsifies | capture fixture (7 bundled + diskrecipe capture) | 20m | pending | unit test on formatter (buggy impl: rendering only `name`, dropping `source`) |
| 10 | Run-ref mapping: bare→`bundled://`, scheme-refs verbatim, path-refs absolutized, `k=v` tail → string inputs | table test over the 6 ref forms + input tails; any wrong `workflowPath`/`inputs` falsifies | expected-value table (from the bundle's own ref documentation, F4) | 15m | pending | unit table test (buggy impl: sending bare names as file paths — the engine then errors "not found") |
| 11 | `/workflow` exists only under KAS engine; `kas`-featureless builds answer `BridgeError`, never dangle | registry test per engine; feature-off bridge arm test; `cargo test` both feature sets (AC6) | compiled test matrix | 15m | pending | unit tests + CI `--features kas` leg (buggy impl: unconditional registration — v2 users get a dead command) |
| 12 | One unknown run-status string warns and skips that entry; the rest of the listing renders | fixture: 2 good + 1 bogus status; whole-listing failure or silent full success falsifies | fixture + log capture | 15m | pending | unit test (buggy impl: strict serde enum over the whole `Vec` — one bad entry kills all) |
| 13 | `cancel` reply `{ok, previousStatus}` is modeled and rendered; run lists `aborted` afterward | parse captured cancel reply; treating it as `{workflowId, status}` shape falsifies (fields are absent) | cancel capture (`kas-workflow-cancel-gateoff-2.16.2.jsonl`) | 10m | live passed | unit test on fixture (buggy impl: reusing the invoke-reply struct — serde fails or nulls) |
| 14 | No-arg `/workflow status` renders from tracker only — zero bridge sends | unit: command with mock bridge; any sent `BridgeCommand` falsifies | mock bridge send log | 10m | pending | unit test (buggy impl: status always round-trips, breaking offline status after attach) |

Cheapest falsifier run before approval: **claim 1's last open verb —
`cancel` — probed live today** (`probe-kas-workflow-cancel-gateoff-2.16.2.py`,
zero-credit: `new` without `invoke`, then `cancel` → `{ok: true,
previousStatus: "running"}`, run listed `aborted`). Claims 2 and 13 also
already hold against live captures. Result: **passed** — the entire v1 verb
surface is individually verified gate-off; ADR-0011's per-verb caveat is
discharged for this feature.

Verification distinctness: each claim's fence is its own named unit test (or
probe section) — a failure names its claim.

## Negative space

1. **No rendering of run progress** — no panel, no drill-in; that is
   cyril-zd8u. This feature's output is system messages + tracker state.
2. **No model-launched runs, ever** — ADR-0011's core trade; the gate stays
   off and `run_workflow` is never registered. Not a deferral; a boundary.
3. **No proxying of Kiro's four advertised `workflow-*` commands** — they
   dispatch to a v2-only method KAS lacks (cyril-oieu); cyril suppresses
   them (claim 8).
4. **No `pause`/`retry`/`delete`/`update`/`resumeAll`** — filed cyril-2ibk
   with the reply-shape caveat.
5. **No startup probe of persisted runs** — reattach is on demand, per the
   issue; a watch-parked run auto-resumed by the agent's own sweep will
   announce itself via `run_start`, which the tracker already accepts.
6. **No inline-DAG authoring surface** — `.workflow.json` + the authoring
   skill own that; `new {workflow: {...}}` stays probe-only.
7. **No approvals-queue work** — concurrent peer-session permission
   contention is cyril-z4eo.
8. **No auth-token refresh fix** — the stale-token landmine that fails
   workflow *steps* (findings F5) is cyril-taba; this feature only surfaces
   the per-node `failureReason` it produces.

## Open decisions (for approval)

1. **`attach` vs `status <id>`**: both map to `inspect` + snapshot seeding,
   differing only in the rendered confirmation. Recommend keeping both
   (distinct user intents; trivial cost). Alternative: collapse to `status`.
2. **`k=v` input parsing on `run`**: the issue's table says
   `/workflow run <recipe>` only, but bundled recipes take inputs; without
   them `run` can only launch input-free recipes. Recommend including
   (string values only).
3. **Claim 1's regression fence is `manual`** (re-probe on kiro version
   bumps, which the standing release-audit routine already does) — the skill
   requires explicit approval for a manual fence.
4. **Suppression location**: `convert/kas.rs` (chosen; dialect quirk lives
   with the dialect) vs `SessionController`.
5. **Notification doctrine extension**: `Notification::WorkflowSnapshot`
   becomes the second consumed-exactly-once variant (alongside
   `Notification::Workflow`). CLAUDE.md's routing table gains one row.

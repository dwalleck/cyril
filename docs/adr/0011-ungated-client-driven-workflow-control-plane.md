# Cyril drives `_kiro/workflow/*` with the workflow gate OFF, and owns the control plane natively

Status: superseded by [ADR-0013](0013-workflows-follow-the-users-kiro-setting.md) (2026-10-18) — was accepted (2026-08-08); resolved the workflow half of [ADR-0003](0003-defer-proxy-stack-for-host-callbacks.md)

## Context

kiro-cli 2.16.0 ships a workflow engine in KAS. Its documented gate is
`resolveWorkflows()` = `settings.workflows.enabled ?? false`, reported back as
`session/new._meta.workflowsEnabled`, and every probe in the 2.16.0 wire audit
flipped that gate ON before calling anything. Read naively, the gate looks
mandatory.

It is not. Flipping it does two things at once:

1. routes the `_kiro/workflow/*` methods, and
2. registers five agent-facing tools (`run_workflow`, `inspect_workflow`,
   `update_workflow`, `validate_workflow`, `save_workflow_definition`) **and four
   slash commands** (`workflow-run`, `workflow-status`, `workflow-cancel`,
   `workflow-resume`).

Only (2) is actually gated. Live A/B on one connection, 2026-08-08
(`experiments/conductor-spike/probe-kas-workflow-gateoff-2.16.0.py`, capture
`logs/kas-workflow-gateoff-2.16.0.jsonl`), with an arm that sent no workflow
settings at either `initialize` or `session/new`:

| Method | gate off | control (gate on) |
|---|---|---|
| `listRecipes` | 7 recipes | 7 recipes |
| `list` | ok | ok |
| `listWatchHandlers` | 2 handlers | 2 handlers |
| `new` | minted a `workflowId` | minted a `workflowId` |
| `invoke` | **ran to `run_complete status=completed`** | (control not needed) |

The gate-off run also delivered the full lifecycle stream to the gate-off parent
(`run_start`, `node_start` ×2 — the documented double-emit — `node_complete`,
`run_complete`) and created its step as a peer session. So discovery, authoring,
execution and progress reporting are all ungated.

This matters because of what (2) costs. Registering `run_workflow` lets the
**model** start a workflow run mid-turn. Workflow step outcomes are already
model-decided (a step ends by the agent issuing `send_message` with a severity;
omit it and the node sits paused), so the model is unavoidably in the *execution*
path. Letting it into the *launch* path as well makes runs non-deterministic from
the client's side for no gain. Separately, the four slash commands would appear
in cyril's autocomplete and dispatch to `kiro.dev/commands/execute` — a v2-only
method KAS does not implement.

## Decision

- **Cyril never sets `workflows.enabled`.** The gate stays off; cyril calls
  `_kiro/workflow/*` directly.
- **Cyril owns the control plane natively** — its own `/workflow` command family
  driving the wire methods, rather than proxying Kiro's four advertised commands.
- **The five workflow tools are never registered**, so the model cannot launch,
  author, or mutate a run. Cyril decides what runs.

## Considered options

- **Flip the gate at `initialize`** — rejected. It is the only channel that
  overrides backend feature flags, so it is connection-wide: every session on the
  subprocess gains five tools and a changed prompt. It would also have been
  cyril's first non-parity entry in `kas/settings.rs`, which is otherwise a
  verbatim replica of v2's `zme()`.
- **Flip the gate per-session at `session/new`** — rejected for the same tool
  exposure, plus it needs a `_meta`-carrying session-creation path cyril does not
  have today (it sends a bare `NewSessionRequest`).
- **Pass through to Kiro's four slash commands** — rejected: broken by
  construction (v2-only dispatch under KAS), and it puts the model back in the
  control plane.

## What is unchanged by this decision

Nothing about the engine or the authoring format moves. Workflows remain **KAS-only**
(the 2.16.0 dark-flag sweep found no workflow surface on the v2 Rust engine even with
`KIRO_TEST_MODE`), so W1 ships only when cyril is running KAS — today behind the
default-off `kas` cargo feature, and not the default engine.

Recipes remain `.kiro/workflows/<name>.workflow.json`, and the `kiro-workflow-authoring`
skill remains the authoring reference. Verified gate-off 2026-08-08
(`probe-kas-workflow-diskrecipe-gateoff-2.16.0.py`, capture
`logs/kas-workflow-diskrecipe-2.16.0.jsonl`):

- a workspace `.workflow.json` appears in `listRecipes` alongside the seven
  `bundled://` recipes, with `source` = its absolute path;
- `_kiro/workflow/new {workflowPath}` — the **file** form, not just the inline
  `{workflow: {...}}` object — accepts it and mints a `workflowId`;
- the `.workflow.json` suffix is genuinely enforced: a plain `.json` sibling is ignored.

So both ways of handing the engine a workflow work with the gate off: a file path, or an
inline DAG object.

## Consequences

- Cyril must **suppress the four workflow commands from autocomplete** if they
  ever appear, so users are never offered a path that cannot dispatch. The
  underlying KAS command-dispatch gap is filed separately; it is pre-existing and
  affects every KAS-advertised command, not just these.
- **The ten mutating verbs beyond `invoke`** (`cancel`, `pause`, `resume`,
  `resumeAll`, `retry`, `load`, `delete`, `update`, and the two list variants) are
  **not** individually verified gate-off. `invoke` was the load-bearing one and it
  passed; treat a `-32601`/`-32603` from any other verb as "re-probe the gate",
  not as a cyril bug.
- **The real cost, stated plainly: the model loses two abilities.** It cannot
  *launch* a run (`run_workflow`), and it cannot *author* one through the engine's
  own tool (`save_workflow_definition` / `validate_workflow`). Authoring itself is
  not lost — a user, the `kiro-workflow-authoring` skill, or cyril can write the
  `.workflow.json`, and `_kiro/workflow/new` **is** the engine's validation path
  (it validates including agent resolution, and costs nothing until `invoke`), so
  cyril can offer validation ungated. What is lost is "ask Kiro in-session to write
  and register me a workflow" as a first-class tool call; the model can still write
  the file with ordinary `fs_write`, just without the dedicated tool's checks.
  If that trade stops being worth it, the fix is an opt-in that flips the gate —
  a separate, deliberate decision, not a default.
- This is a **deliberate deviation from the documented gate**. Anyone reading
  Kiro's engine source will conclude the gate is required; the table above is why
  it is not. Do not "fix" this by enabling workflows.

## Amendment: workflow-initiated follow-up turns (cyril-lki9, 2026-10-12)

Decision approved 2026-09-29, correlation amended 2026-09-30; recorded here
2026-10-12. The approval and exact behavior are in
[`spec.md`](../../.cyril-lki9/spec.md#decisions); implementation landed in
[PR #145](https://github.com/dwalleck/cyril/pull/145) and
[PR #147](https://github.com/dwalleck/cyril/pull/147).

Gate-off execution does **not** prevent KAS from starting a billed model turn
when a workflow completes, fails, or is aborted. The committed
[`-tail` capture](../../experiments/conductor-spike/kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl)
records a parent `turn_start`, tool calls, agent text, and `turn_end` after
`run_complete`, without a client `session/prompt`. The launch/control-plane
decision above therefore does not imply that the model is absent from the
post-run path.

**Keep the parent link; accept, model, and label the follow-up.** Cyril retains
`parentSessionId` on `_kiro/workflow/new`, preserving parent-scoped relays,
permissions, and workflow discovery. It neither suppresses the wake nor
diverts it to another session. Omitting the parent or introducing a dedicated
control session would change those responsibilities and is not this decision.

- An idle main-session `turn_start` begins a first-class agent-initiated turn.
  Busy state and the stall watchdog start immediately; permission requests,
  Esc cancellation, and mid-turn steering use the normal turn machinery.
  Its `turn_end` completes the turn without waiting for a prompt response.
- The first frame tagged as agent-initiated gets one transcript header.
  A workflow-completion wake names the newest completion queued for that
  session when the turn starts, with its status; a missing correlation gets a
  nameless workflow header. Other reasons get a generic agent-initiated
  header. A silent turn with no tagged frame remains busy but has no
  invented header.
- Engine-injected `notify-*` messages appear inline at their arrival point.
  They never consume or clear the operator's queued steers (cyril-5n75).
  Workflow injections use completions arriving after the current turn
  started, not the completion the wake already took.

This deliberately **does not remove the billed turn**. It makes the turn
visible and controllable without reconstructing the hidden wake prompt.
Bridge ownership is documented in
[ADR-0004](0004-bridge-loop-mediates-both-acp-directions.md).
The capture replay and lifecycle checks are recorded in
the [completion record](../../.cyril-lki9/plan.md#resumed-completion-2026-10-12).

The later KAS network-give-up parent-note trigger has separate correlation
work in **cyril-pd4b**, dependent on **cyril-fb1m** (`_kiro/session/notify`).
It is not qualified by the run-completion capture and is not claimed fixed
by this amendment. `/workflow new` authoring remains **cyril-0asq**.

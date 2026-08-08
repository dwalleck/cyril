# Cyril drives `_kiro/workflow/*` with the workflow gate OFF, and owns the control plane natively

Status: accepted (2026-08-08) — resolves the workflow half of [ADR-0003](0003-defer-proxy-stack-for-host-callbacks.md)

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
- Users who want the model to launch workflows conversationally cannot, by
  design. If that becomes a wanted feature it is an opt-in that flips the gate,
  and it should be a separate, deliberate decision rather than a default.
- This is a **deliberate deviation from the documented gate**. Anyone reading
  Kiro's engine source will conclude the gate is required; the table above is why
  it is not. Do not "fix" this by enabling workflows.

# cyril-0qe6 — prior art (tracker sweep 2026-08-11)

Upstream spec: the issue itself is the interrogated artifact — produced by the
2026-08-08 grill-with-docs session, carries the command surface, the ADR-0011
contract, the response-plumbing gap, and six ACs. No separate spec.md needed.

## Directly load-bearing

- **ADR-0011** — the contract this issue implements. Revalidated on 2.16.2
  (commit 36290d1): method set byte-identical, gate-off invoke still runs to
  completion. NEW in 2.16.2: `NodeState.completionSignalSource` field;
  lifecycle-events.ts / node-state-transforms.ts / run-disk-operations.ts are
  new internal modules.
- **cyril-6beh** (closed, PR #92) — WorkflowTracker + 9-event state machine.
  The tracker this command family reads.
- **cyril-jxfu** (closed, PR #93) — peer-session routing + late-claim
  re-parent. The stream substrate `/workflow attach` output rides on.
- **cyril-oieu** (open) — KAS-advertised commands dispatch to v2-only
  `kiro.dev/commands/execute`. Why AC5 suppresses Kiro's four `workflow-*`
  commands instead of proxying them. Out of scope here; do not fix.
- **cyril-w0vy** (open, P1) — bridge wedge when an expected response never
  arrives. Cautionary for AC2: the new response-carrying ext-method path must
  fail loud (timeout or error notification), never park `is_busy`.

## Adjacent, not in scope

- **cyril-z4eo** (open) — approvals queue + attribution for concurrent peer
  sessions. Separate W-track issue.
- **cyril-zd8u** (open, P3) — run panel / drill-in renderer. Explicitly out of
  scope for 0qe6 (`related` link).
- **cyril-kzke** (open, P3) — SubagentTracker→SessionTracker rename; touches
  the same files, serialize after this ships.
- **cyril-7sjs / cyril-sinu** (open, P3) — workflow test tripwires + converter
  diagnostics polish.
- **cyril-ea67** (open) — settings sweep; documents the 4 slash commands the
  gate would add (what AC5 suppresses).
- **cyril-saf4** (open) — KAS RPC sweep; 28 response shapes captured, method
  param-discovery patterns.
- **cyril-ykkc** (open, P3) — `--features kas` must be in the local gate (AC6
  explicitly requires it; this issue is why).
- **cyril-nn85** (open, P3) — `_kiro/session/list` response-shape change;
  distinct surface from `_kiro/workflow/list`, no overlap.

## Existing probes/captures reused

- `experiments/conductor-spike/probe-kas-workflow-gateoff-2.16.0.py` — harness
  (spawn, auth responder, gate-off session, follow loop) reused verbatim.
- `logs/kas-workflow-gateoff-2.16.2.jsonl` + `logs/kas-workflow-diskrecipe-2.16.2.jsonl`
  — 2.16.2 revalidation captures (commit 36290d1). Both `list` calls in them
  return `{"runs": []}` and both PRECEDE the invoke — no capture anywhere shows
  a run in `list`, and none crosses a process boundary. That gap is this
  probe's target.

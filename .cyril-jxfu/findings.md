# cyril-jxfu — prove-it-prototype findings

Probes run 2026-08-10 against `experiments/conductor-spike/kas-custom-dag-2.16.0.jsonl`
(82-line live capture, 2.16.0, two-step parallel DAG: parent + steps alpha/beta).

## Probe 1 — routing replay (`probe.py`)

Simulates `classify_notification_route` (app.rs:1184) over every `session/update`
in the capture. main = `sess_2bc0cfdc…` (session/new response, line 9).

| session | frames | route under current classifier |
|---|---|---|
| parent `…23a63d60` | 3 | Main |
| step alpha `…c0877fb6` | 15 | **Subagent (misfiled)** |
| step beta `…d68523d9` | 15 | **Subagent (misfiled)** |

**30 of 33 scoped frames misfile.** Claim timeline: `node_start` fires 3× with
NO sessionId (lines 19-21: group node `fan` + both steps, pre-session), then
re-emits WITH sessionId at lines 46 (alpha) and 48 (beta).

**Late claim is the dominant path, not an edge case.** Alpha's stream starts at
line 33, its claim lands at line 46 — six frames early. Beta: 39 vs 48. In this
capture 100% of step sessions emit before their claim.

## Probe 2 — meta coverage (`probe2_meta_coverage.py`)

Does `_meta.kiro.workflow` ride step frames, making them self-identifying?

**Only on tool-call frames: 4 of 33** (one `tool_call` + one `tool_call_update`
per step, carrying workflowId/workflowName/nodeId/nodePath/type/branchId in
`update._meta.kiro.workflow`). The bulk — `session_info_update` ×18,
`config_option_update` ×5, `available_commands_update` ×3,
`user_message_chunk` ×2 — carries **nothing**. Per-frame self-identification
cannot replace the registry; tool-call meta is corroboration only.

Bonus observation: step bootstrap traffic is shaped exactly like a main
session's (config options, session info, commands list, driver-injected
`user_message_chunk` prompt). A phantom "subagent" stream would absorb a full
session bootstrap per step.

## Probe 3 — registry feasibility (shipped tracker replay)

`oracle-replay-expected.json` (the committed projection of replaying this same
capture through the SHIPPED `to_notification` + `WorkflowTracker`) holds both
step ids at `final[0].nodes[2].data.sessionId` / `nodes[3].data.sessionId`.
cyril-6beh's merge-not-append `apply_node_started` already lands the claim in
tracker state — the registry can be an index/query over the tracker, not new
parallel state.

## Oracle

Text-only pipeline (`oracle.sh`: grep -o / awk over raw bytes — no JSON
parsing, no routing simulation) recomputed: frame counts per sessionId,
node_start claim lines, first-appearance lines, workflowId-bearing update
count. **Two initial disagreements, both resolved:**

1. Oracle counted 5 main-sid occurrences on session/update vs probe's 3 frames:
   lines 63/68 are STEP-scoped `tool_call_update`s whose `rawOutput` embeds the
   parent sessionId — the steps message the parent via a "Send Message" tool.
   Occurrences ≠ frames; probe correct, oracle count explained.
2. Oracle's `"result":{[^}]*"sessionId"` regex missed the session/new response:
   `[^}]*` can't cross the nested `_meta` object. Oracle bug, fixed
   understanding against raw line 9.

After reconciliation: item-by-item agreement on all three slices
(33 frames = 3+15+15; claims = exactly {alpha@46, beta@48}; workflowId-bearing
updates = 4).

## What I learned (that I didn't know before)

The late claim is not a rare ordering hazard to tolerate but the ONLY observed
path — every step session in the live capture streams its entire bootstrap
(6+ frames) before the sessionId-bearing node_start arrives, and those
bootstrap frames carry no workflow meta whatsoever, so re-parenting an
already-populated stream is a mainline requirement, not a fence for a corner
case.

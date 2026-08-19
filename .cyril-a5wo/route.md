# Route: cyril-a5wo

Change: Verify and fence Cyril's KAS tool-call cancel-recovery lifecycle against current source and six committed captures
Date: 2026-08-18

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The revised request depends on a current-system premise not yet covered by applicable evidence: whether every `@kiro/agent` ACP `tool_call` / `tool_call_update` emission serializes only complete `rawInput`, making absent or partial `rawInput` unreachable at the ACP seam. The six committed 2.16.2 and 2.18.1 captures show complete `rawInput` in every observed frame, but observation alone does not establish wire representability. Current extracted-source coverage and citations are required. | yes |
| 2 | Structural boundary | The requested work audits an external implementation and adds behavioral fences around Cyril's existing KAS conversion, `UiState` tool-call merge/commit path, and existing `TrackedToolCall` display helpers only if the source audit proves partial input representable. It does not require a public API, schema, module-boundary, or cross-module placement change. | no |
| 3 | Production-scale risk | The change adds source evidence and deterministic replay tests over six bounded captures. It does not change production latency, throughput, memory, concurrency, or data-volume behavior. | no |
| 4 | Explicit behavior | Given the current extracted `@kiro/agent` KAS source, when every ACP `tool_call` and `tool_call_update` emission path is traced, then the evidence records with exact source citations whether absent or partial `rawInput` is wire-representable. Given each of the six committed 2.16.2 and 2.18.1 cancel captures, when its same-`toolCallId` `pending` → `in_progress` → `failed` lifecycle is replayed through Cyril's production conversion and UI state path and the turn ends, then exactly one committed tool call remains, fields preserved by guarded merge are not clobbered, and no in-progress entry remains. Given the source audit finds absent or partial `rawInput` unrepresentable, when criterion 3 is evaluated, then it closes as unreachable-by-construction with citation; otherwise the source-derived fixture is converted and its `TrackedToolCall::primary_path` and `command_text` display behavior is asserted without panic or incorrect rendering. | yes |

Unknown tests: none

## Selected route

Empirical — wire representability depends on current extracted KAS source behavior not established by the existing live captures.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — revised behavior is fully explicit (T4 yes); the existing file records the superseded live-capture request and is not the behavior source for this route |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 yes); existing captures and probes may be adopted where current |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Empirical route

## Downstream sequence

prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Empirical — `prove-it-prototype` records `PASS` for the current-source wire-representability premise, every later artifact satisfies its owning stage's completion criterion, and `checkpointed-build` records no `FAIL`.

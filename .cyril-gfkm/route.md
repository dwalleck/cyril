# Route: cyril-gfkm

Change: Capture standard ACP turn usage, persist engine-neutral records, aggregate them, and render a usage dashboard.
Date: 2026-08-21

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The schema-driving premise is current and covered: `experiments/conductor-spike/omp-usage-update-2turn.jsonl` is a live two-turn omp 17.3.5 capture. It proves per-turn standard ACP `Usage` is on each `session/prompt` response, cumulative context/cost is on `usage_update`, and model/provider is the `model` config option. `docs/usage-observer-design.md` records the field semantics and deltas. The workspace already enables `unstable_session_usage`; `convert_session_update` currently converts `UsageUpdate` context counts but intentionally drops cost, while `bridge.rs` drops `PromptResponse.usage`. No dependency or wire path named by that evidence has changed since the same-day capture. | no |
| 2 | Structural boundary | The change extends the core domain notification/turn schema, ACP conversion and bridge response boundary, adds persistent log and engine-neutral aggregation modules, exposes read-only usage state across the core/UI boundary, and adds a modal TUI surface plus input routing. Those are public schema and cross-module placement decisions. | yes |
| 3 | Production-scale risk | The append-only usage history is process-persistent and can grow across sessions/folders. Aggregation and rendering must remain bounded in memory and latency as record count grows; the design and checkpoint require a stress fixture rather than assuming a small log. | yes |
| 4 | Explicit behavior | Given Cyril drives an ACP agent whose prompt response carries standard `Usage` and whose `usage_update` carries cumulative context/cost, when a turn completes, then Cyril records one typed turn with token/cache counts, the cost delta, session/folder/model/provider/agent identity, duration, TTFT, tool calls, stop/error outcome, and timestamp. Given persisted records from multiple turns, when the usage view opens, then it shows overview tokens/cost/cache rate/TTFT/tokens-per-second plus model/provider/tool/folder breakdowns and recent/error records, matching omp stats overview within rounding for the captured fixture. Given records from any engine, when aggregation runs, then no engine-specific branch participates. Given malformed/absent optional usage fields, when conversion/aggregation runs, then absence remains explicit and invalid values do not become plausible zeroes. Given a large history, when the view opens and aggregates, then work and retained detail remain within the bounds approved in `design.md`. | yes |

Unknown tests: none

## Selected route

Structural — current live evidence covers the wire premise, but the change crosses protocol/domain/UI boundaries and introduces persistent data-volume risk.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior is fully explicit in T4; viewer placement and storage bounds are design decisions, not unresolved requester scope |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise; the current committed live probe and capture cover T1 |
| design.md | falsifiable-design | required — Structural route |
| plan.md | budgeted-plan | required — Structural route |

Oracle checkpoint in `checkpointed-build`: required — Structural route

## Downstream sequence

falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate.

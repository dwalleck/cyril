# Route: cyril-kryv

Change: Add Kiro v2/KAS credits-degraded capture and rendering to the usage observer.
Date: 2026-08-21

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Current repository evidence covers every Kiro premise: `crates/cyril-core/tests/fixtures/kas/session_info_update_turn_completion.json` captures per-turn KAS credits/duration/status; `session_info_update_context_usage.json` captures KAS context percentage/buckets; `docs/kiro-2.19.1-wire-audit.md` records the six-model backend-gated token result; existing `kiro.dev/metadata` conversion/tests cover v2 credits/context; workflow JSONL fixtures include `_kiro/account/getUsage`; existing live observer timestamps define TTFT/duration independently of OTLP. The request's OTLP decision is resolved in tracker evidence. | no |
| 2 | Structural boundary | The change crosses the Kiro conversion boundary, public domain record/snapshot types, SQLite schema/queries, observer correlation, App wiring, and usage-modal rendering. It also adds KAS account-usage request/response ownership and historical Kiro sidecar ingestion. These require cross-module placement decisions. | yes |
| 3 | Production-scale risk | Historical JSONL backfill can encounter many sessions and large tool payloads; ingestion must be incremental/idempotent and must not retain full archives in memory. Existing snapshot bounds remain applicable. | yes |
| 4 | Explicit behavior | The acceptance criteria establish live v2/KAS credits-mode, explicit backend-gated placeholders, client-measured TTFT/duration, tool counts, identity groupings, and no omp regression. Material behavior remains unresolved: whether historical sidecars are required delivery or only evidence; when/how import runs and deduplicates live records; whether the Costs page presents credits and monetary cost together or separately; which context-breakdown and compaction statistics are displayed; whether `_kiro/account/getUsage` is automatically queried and where plan/overage data renders; and whether exact tool-name/argument/result attribution is required for live turns, imported history, or both. | no |

Unknown tests: none

## Selected route

Structural — repository evidence is current, but unresolved dashboard/import semantics require interrogation before cross-module design.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — T4 has unresolved behavior decisions |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict) |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural route

## Downstream sequence

interrogated-spec → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate.

Result: 2026-08-22 | PASS — `cargo test` (1,415 passed), three consecutive `cargo test --features kas` runs (1,721 passed each), default/KAS `cargo clippy -- -D warnings`, and `cargo fmt --all --check` all succeeded after the review-fix slice. The tracing gate now captures events structurally with callsites registered as always enabled, and the previously hanging usage-command test asserts App-owned dispatch without awaiting an impossible command-layer receive.

## PR increments

- PR #97 — Live metering substrate (`feat/cyril-kryv-kiro-usage`, draft).
- PR #98 — Usage detail and enrichment (`feat/cyril-kryv-usage-detail`, draft).
- PR #99 — Account and modal completion (`feat/cyril-kryv-usage-modal`, draft), plus exact-wire review fence commit `99496ca`.

Live acceptance: KAS authenticated on this host, completed a no-tool turn (`OK`), and the open `/usage` panel refreshed from 4→5 turns with `Provider requests 1`, `Retries 0`, and `0.1698 credits`; Costs also showed live account plan/limit/overage fields. v2 remains unavailable because the host is not logged in, so no live v2 claim is made; committed v2 fixtures and default-suite coverage remain the evidence for that path.

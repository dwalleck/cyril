# Route: cyril-n3j7

Change: Capture authoritative completed ACP turns and recall scoped episodes in a later session
Date: 2026-08-25

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The design requires SQLite FTS5 virtual tables and literal-token ranking through workspace `rusqlite 0.39` with `default-features = false, features = ["bundled"]`. Current `crates/cyril-memory/src/store.rs` has no FTS table or capability probe, and no existing `evidence.md` verifies that this exact bundled dependency exposes FTS5 or pins the required project-filtered ranking behavior. The bridge lifecycle premise is covered by current `run_loop`, `TurnMediator`, normalized `Notification` types, and fake-agent tests; FTS5 remains unverified. | yes |
| 2 | Structural boundary | The change adds source-turn and episode domain types, authenticated runtime protocol operations, memory-store schema and FTS index, a deliberate bridge-to-memory observer/fan-out seam, first-prompt query input, `/memory` command actions, core-neutral UI projection types, and binary orchestration across `cyril-memory`, `cyril-core`, `cyril`, and `cyril-ui`. | yes |
| 3 | Production-scale risk | Durable source turns and their FTS rows grow with completed ACP turns. Retrieval must remain project-filtered and bounded to three episodes, 1,200 characters per episode, and 3,600 episode characters total; capture must not backpressure the bridge or UI event loop. These data-volume, latency, and concurrency dimensions require explicit budgets and stress fixtures. | yes |
| 4 | Explicit behavior | **Capture:** Given a project-bound session accepts an original outbound prompt, when normalized assistant chunks, tool lifecycle updates, and an authoritative terminal disposition arrive, then a pre-UI observer emits typed events keyed by session and turn, excludes injected context, and the memory runtime assembles one coherent source turn with project, provenance, timestamps, content hash, and terminal state. **Completion:** Given a turn completes, fails, is interrupted, is abandoned, or remains incomplete, when its lifecycle is persisted or replayed after restart, then only authoritative completion is recall-eligible; identical `(session, turn)` replay is idempotent and a content conflict is a typed integrity failure and audit event. **Recall:** Given a completed episode exists, when a fresh same-project session sends its first prompt, then the original first text block drives literal-token FTS5 retrieval, at most three deterministic results are rendered after lessons in a separate bounded `CYRIL_EPISODES` derived-data section with source session/turn/timestamp provenance, and the original prompt blocks remain unchanged apart from the optional context prefix. **Isolation:** Given an equivalent query from another project or a later prompt in the same session, when prompt preparation runs, then the episode is not retrieved or reinjected. **Inspection:** Given UI transcript retention or truncation no longer contains the turn, when `/memory` inspects the stored turn, then typed bounded source content, terminal state, and provenance remain available from durable storage. **Responsiveness:** Given capture/runtime failure or a full UI notification queue, when bridge mediation continues, then capture does not consume the UI channel, block the event loop, fabricate completion, or change the agent turn. **Negative space:** No working-memory table/tool/extractor/TTL/prompt section, LLM consolidation, semantic extraction, embeddings, MCP tools, automatic scope promotion, proxy adapter, dual-write production path, or generalized backend abstraction is added. | yes |

Unknown tests: none

## Selected route

Empirical — the FTS5 capability and ranking premise is unverified; the change is also structural and production-scale-sensitive.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — issue description, Design field, acceptance criteria, and ADR-0003 make behavior fully explicit (T4 yes) |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 yes) |
| design.md | falsifiable-design | required — Empirical route |
| plan.md | budgeted-plan | required — Empirical route |

Oracle checkpoint in `checkpointed-build`: required — Empirical route

## Downstream sequence

prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Empirical — `evidence.md` records PASS for the exact bundled-rusqlite FTS5 capability and project-filtered literal-token ranking premise, every later artifact satisfies its owning stage's completion criterion, and checkpointed-build records no FAIL.

## Terminal result

**PASS — 2026-08-25.** `evidence.md` records PASS for both empirical premises.
`design.md`, `plan.md`, and `checkpoints.md` satisfy their owning gates.
Checkpointed build records all eight gate items as PASS or the plan-backed
one-off-phase N/A, with no FAIL. Claims C1–C13, their independent oracles,
falsifiers, permanent regression fences, production budgets, the full Rust
suite, formatting, and warning-denied Clippy are green.

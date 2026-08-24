# Route: cyril-ezgo

Change: Teach, persist, inspect, and inject one explicit project lesson
Date: 2026-08-24

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Stable project identity depends on resolving a Git worktree to its canonical Git common directory on every supported host. Current source has no project-identity resolver (`crates/cyril-memory/src` only canonicalizes the memory data root), no applicable `evidence.md` exists, and the exact main-worktree, linked-worktree, and non-Git path behavior is unverified. | yes |
| 2 | Structural boundary | The change expands the public `cyril-memory` domain protocol beyond admin-only `Health`/`Shutdown`, adds durable memory-store schema, introduces a scoped client consumed by the binary and bridge, changes `/memory` command result projection, and adds a prompt-adapter seam across `cyril-memory`, `cyril-core`, `cyril`, and `cyril-ui`. | yes |
| 3 | Production-scale risk | No. M1 is local single-user storage; first-prompt output has a fixed 4,000-character budget and truncates only between lessons. It adds no transcript ingestion, embeddings, background jobs, network service, or unbounded prompt output. | no |
| 4 | Explicit behavior | **Teach:** Given memory is ready and a canonical session workspace is bound, when the user teaches non-empty lesson text through `/memory teach`, then Cyril validates it, redacts secrets, persists an active `user_explicit`/`instruction` lesson under the bound project, records an audit event, and returns a visible typed confirmation; exact normalized duplicates in the same project are idempotent. **List/inspect:** Given active lessons exist for the bound project, when the user runs `/memory list` or `/memory inspect <id>`, then Cyril returns only that project's typed read-only projection, including provenance/trust/status needed to distinguish explicit instructions, without UI-side row formatting policy. **Persistence:** Given a lesson was taught, when Cyril and the ACP session restart in the same canonical project, then the lesson remains listable and injectable. **Project identity:** Given two linked Git worktrees share one Git common directory, when sessions bind either workspace, then both resolve the same project ID while retaining their own display path; given a non-Git workspace, the canonical workspace path is the identity; after binding, process cwd is never consulted as fallback. **First prompt:** Given a fresh bound ACP session with active lessons, when its first prompt is accepted, then the bridge requests one complete memory-context block with a deadline and 4,000-character budget, prepends that block as a separate text content block, and forwards every original prompt block unchanged and in original order. The block uses a dedicated `CYRIL_LESSONS` instruction section, deterministic newest-first lesson order, whole-lesson truncation, and an omitted-count marker. **Transcript separation:** Given first-prompt injection occurs, when the TUI commits the user's message, then it displays only original user content and never the injected block. **Exactly once:** Given a first prompt has already been accepted for a session, when later prompts are sent, then no automatic block is requested or prepended. **Fallback:** Given memory is disabled, unavailable, fails, or exceeds its deadline, when any prompt is sent, then the wire request contains byte-equivalent original prompt blocks and the turn remains usable. **Trust separation:** Given future derived/document rows may exist, when prompt context is prepared, then only active `user_explicit` lessons with `instruction` trust can enter `CYRIL_LESSONS`; derived/document content cannot. **Regression proof:** Given the bridge fake-agent harness, when success, repeat-prompt, disabled, failure, and timeout scenarios run, then captured wire blocks prove ordering, exactly-once injection, transcript separation, and exact fallback. | yes |

Unknown tests: none

## Selected route

Empirical — the cross-platform Git-common-directory identity premise lacks current applicable evidence; the change is structural regardless.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit in the issue Design and acceptance criteria (T4 yes) |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 yes) |
| design.md | falsifiable-design | required — Empirical route |
| plan.md | budgeted-plan | required — Empirical route |

Oracle checkpoint in `checkpointed-build`: required — Empirical route

## Downstream sequence

prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Empirical — `evidence.md` records PASS for the Git-common-directory project-identity premise, every later artifact satisfies its owning stage's completion criterion, and checkpointed-build records no FAIL.

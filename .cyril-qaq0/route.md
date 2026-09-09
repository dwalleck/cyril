# Route: cyril-qaq0

Change: Activate bundled themes and terminal color modes.
Date: 2026-09-09

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | No external terminal guarantee is assumed: automatic detection is a policy over injected environment values, and projection uses the existing semantic resolver. Current repository inspection establishes ownership and missing activation seams. July prototype results are historical, not current PASS evidence; no persistence mechanism is assumed pending scope resolution. | no |
| 2 | Structural module shape | UiConfig in crates/cyril-core/src/types/config.rs gains configuration fields; commands/mod.rs gains local command behavior; UiState in cyril-ui/src/state.rs needs theme mutation and picker lifecycle interfaces. App in crates/cyril/src/app.rs and main.rs are protected orchestration parents. Current startup parser is cyril-memory::load_config_report, not the historical direct Config loader. Candidate owners: core configuration/command intent, UI theme policy and preview state, binary startup environment probing and routing only. No theme policy belongs in memory. | yes |
| 3 | Production-scale risk | Fixed-size theme catalog and one theme resolution per startup/selection event. No new unbounded workload intended; existing transcript caches must remain theme-correct. | no |
| 4 | Explicit behavior | Current ticket requires session-local Enter acceptance without configuration writes. Existing spec.md records verbatim prior approval for an optional persistence dialog. These are conflicting scope authorities requiring requester resolution. Also cyril-fkke remains an open declared blocker and ThemeId::ALL contains only CyrilDark; adding five palettes is expressly outside qaq0. | no |

Unknown tests: none. T4 identifies unresolved decisions rather than unavailable repository facts.

## Selected route

Structural — configuration and UI interfaces change; prior approved persistence behavior conflicts with the current ticket.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — reconcile historical sign-off with current ticket and palette dependency scope |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified external premise selected; historical prototype files do not establish current gates |
| design.md | falsifiable-design | required after specification approval |
| plan.md | budgeted-plan | required after design approval |

Oracle checkpoint in checkpointed-build: required — Structural route.

## Downstream sequence

interrogated-spec → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate. Not satisfied: scope resolution and subsequent gates remain outstanding.

## Source state and evidence

Feature worktree: /home/dwalleck/repos/cyril-wt-feat-cyril-qaq0, branch feat/cyril-qaq0, created from main at 7fe7f5ed.
Issue claimed in primary and feature tracker via rivets update cyril-qaq0 -s in_progress.
Read-only source mapping: agent://ThemeBoundaries (2026-09-09).
Historical contract: spec.md, especially lines 47–58 and 152–159.
Current tracker: rivets show cyril-qaq0 --json; rivets show cyril-fkke --json (2026-09-09). All other declared blockers are closed.

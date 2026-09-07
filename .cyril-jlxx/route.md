# Route: cyril-jlxx

Change: Run an isolated, inspection-only KAS reviewer through Cyril's public bridge.
Date: 2026-09-06
Source: Cyril Rivets cyril-jlxx and authoritative parent cyril-ukmu.
Branch: feat/cyril-jlxx; upstream discovered as refs/remotes/origin/main.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Parent explicitly distinguishes native configuration/policy evaluation from configured-operation proof. Need current CLI 2.21.1 KAS evidence for native restricted profile, inherited hooks/MCP/subagents/workflows, actual permitted model, credential separation and fresh process/session isolation. Existing working ACP and Jira access are established and are not premises to rediscover. Installed CLI reports 2.21.1; supported CLI catalog advertises claude-sonnet-4.6, but KAS selection is not yet verified. | yes |
| 2 | Structural module shape | Current Cargo.toml has core, memory, UI, voice and TUI binary only. Core owns public runtime/protocol/process lifetime; workbench review operations are a new owner consuming core, not a new responsibility in core's mediator or TUI App. Candidate owner is a workbench backend reviewer module; final placement depends on probe. Protect existing core bridge/domain mediator and TUI App from review-specific policy. Reusable runtime enforcement is allowed only for a demonstrated integration gap. | yes |
| 3 | Production-scale risk | Child process ownership, bounded event/permission drains and isolation across successive reviews are concurrency/resource risks. No throughput or scale expansion is requested. Design must cover shutdown and stalled-side-effect behavior. | yes |
| 4 | Explicit behavior | Given supported CLI 2.21.1 KAS and its advertised Sonnet 4.6, when a Review Run starts, select and verify that model or fail without substitution. Given a workbench-owned restricted native profile and mediated authorized evidence, when inspection executes, authorized reads succeed and mutation, PR-code execution, arbitrary shell, unauthorized reads, publication and escalation are denied; unexpected permissions fail closed. Given inherited configuration and exposed hooks/MCP/subagents/workflows, when actual configured operations are attempted, outcomes must agree with independent synthetic-file observations and safe permissive controls, not just model refusal or bypassed executors. Given backend publication credentials and ResourceFS access, when a reviewer is launched, neither authority is available to it; permitted runtime/auth state writes are distinguished from forbidden resource writes. Given consecutive reviews and other Cyril apps, when runs start/end, each has its own process/session and cannot share live review state; record platform and leave native Windows acceptance to separately verified cyril-6y1s. | yes |

Unknown tests: none.

## Selected route

Empirical — actual configured native authority enforcement remains unverified and determines the implementation boundary.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit in child and authoritative parent (T4 yes) |
| evidence.md, probe.* | prove-it-prototype | required — empirical native policy/runtime premises |
| design.md | falsifiable-design | required after every empirical premise passes; user approval required |
| plan.md | budgeted-plan | required after design approval |

Oracle checkpoint in checkpointed-build: required — Empirical route.

## Downstream sequence

prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build.

## Terminal criterion

Every empirical premise records PASS, every downstream artifact satisfies its owning stage's completion criterion, and checkpointed-build records no FAIL. Implementation is not complete at a probe or design handoff. Native Windows gate remains separately owned by verified ticket cyril-6y1s; Linux evidence must not claim it.

Result: 2026-09-06 | cargo test --workspace --features cyril-core/kas (1890 passed) + cargo clippy -D warnings + cargo fmt --check + python3 .cyril-jlxx/oracles/shape.py --complete --mutation-check | PASS

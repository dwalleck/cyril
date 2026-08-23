# Route: cyril-j7um

Change: Add Cyril M0 local memory-runtime health, storage initialization, authenticated IPC, and status reporting.
Date: 2026-08-22

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The design depends on companion-process discovery, OS-released exclusive data-root locking, owner-only filesystem and IPC permissions, Unix socket and Windows named-pipe behavior, SQLite WAL reopen behavior after graceful/forced termination, and timeout/reaping behavior. The current repository has no `cyril-memory` crate, memory runtime, IPC implementation, or applicable `evidence.md`; these system/platform premises are unverified. | yes |
| 2 | Structural boundary | The issue explicitly adds a `cyril-memory` domain/runtime crate, a dedicated runtime executable, a versioned request/response protocol, a `[memory]` configuration schema, typed cross-crate status/events, `/memory status`, and a TUI projection. These are public schema, module-boundary, and cross-module placement changes. | yes |
| 3 | Production-scale risk | IPC input is untrusted and bounded at 1 MiB; process startup/shutdown is concurrent with ACP lifecycle and must remain bounded; SQLite ownership, migrations, and reconnect/reopen behavior affect correctness under concurrent or abnormal termination. These require stress/boundary checks beyond a Local route. | yes |
| 4 | Explicit behavior | Given no `[memory]` table or `enabled = false`, when Cyril starts, then memory remains disabled and existing ACP/TUI startup is unchanged. Given valid enabled configuration, when Cyril starts, then exactly one absolute-path runtime child owns the canonical data root, creates owner-only versioned memory and knowledge SQLite stores in WAL mode, binds private authenticated IPC, passes health, and reports Ready. Given invalid or whole-file-unparseable memory configuration, inaccessible storage, startup timeout, child exit, protocol mismatch, migration failure, permission failure, or a second owner, when startup is attempted, then no runtime is treated as ready, status carries a typed actionable diagnostic, and ACP session creation/prompting remain usable. Given missing/invalid credentials, malformed/oversized frames, unknown operations, or unsupported protocol versions, when a client connects, then the offending connection receives a typed safe error or is closed without authorization or runtime failure. Given running memory, when Cyril shuts down, then it requests authenticated shutdown, waits at most two seconds, force-reaps if needed, restores the terminal without hanging, and either shutdown mode leaves both stores reopenable without destructive reinitialization. Given `/memory status` or TUI rendering, when status changes, then disabled, starting, ready, degraded, and failed are distinguished from typed immutable state without UI persistence/process access. Given M0 schema/API inspection, then only version metadata plus consumed health/shutdown operations exist: no memory/knowledge/vector/job tables, queue contract, backend selector/trait, remote/no-op adapter, retrieval, embeddings, MCP registration, transcript capture, prompt injection, or agent tools. | yes |

Unknown tests: none

## Selected route

Empirical — multiple operating-system, process-lifecycle, SQLite, and IPC premises lack current applicable evidence; Empirical takes precedence over the structural boundary and scale-risk verdicts.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior is fully explicit in the issue and T4 verdict |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 verdict) |
| design.md | falsifiable-design | required — Empirical route |
| plan.md | budgeted-plan | required — Empirical route |

Oracle checkpoint in `checkpointed-build`: required — Empirical route

## Downstream sequence

prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Empirical — `evidence.md` records PASS for every empirical premise, every later artifact satisfies its owning stage completion criterion, and checkpointed-build records no FAIL.

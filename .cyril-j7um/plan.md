# Plan: cyril-j7um

## Partition arithmetic

| Slice | Diff estimate |
|---|---:|
| 1. Config, paths, permissions, minimal stores | 1,850 lines |
| 2. Authenticated runtime and real-process lifecycle | 2,450 lines |
| 3. Cyril process orchestration and fail-open startup | 1,850 lines |
| 4. Typed command and TUI status projection | 1,450 lines |
| **Slice sum** | **7,600 lines** |
| Churn margin | **1,900 lines (25%)** |
| **Projected total** | **9,500 lines** |

The 25% margin covers cross-platform cfg branches, Cargo lock/workflow churn, and fixture expansion discovered only when native Windows/Unix behavior is fenced. The projected total exceeds the 4,000-line review gate, so the work is partitioned into three independently mergeable increments against the repository's discovered upstream/default branch.

### PR increments

1. **runtime-storage-foundation** — Slice 1. Mergeable definition: `cyril-memory` strictly loads memory config, resolves/private-hardens roots, and initializes/reopens minimal stores behind an exclusive lock. Its unit and temporary-root tests verify without a runtime process or Cyril wiring.
2. **runtime-process-protocol** — Slice 2, based on increment 1. Mergeable definition: the dedicated binary and AdminClient communicate through the one authenticated protocol, enforce endpoint privacy, expose health/shutdown, and pass real-process restart/ownership tests without App startup wiring.
3. **cyril-memory-status-integration** — Slices 3-4, based on increment 2. Mergeable definition: Cyril starts/stops the runtime without blocking ACP, and `/memory status` plus TUI projection report all five states. App/core/UI and end-to-end tests verify without any later increment.

## Slice 1: Add strict config, private paths, locking, and minimal stores

**Claim IDs:** C1, C2, C3

**Expected behavior:** `cyril-memory` returns ordinary config defaults/values independently from an exact memory presence state; rejects invalid-present memory even when disabled; resolves an absolute platform data root outside cwd; owner-hardens it; obtains one OS-released exclusive lock; and initializes/reopens separate WAL memory/knowledge databases whose only table is the singleton schema-version table at version 1.

**Oracle:** Python `tomllib` computes the config presence/type matrix; expected platform paths come from the issue's literal table rather than the production resolver; `stat(1)`/native Windows ACL enumeration inspect privacy; `sqlite3` CLI independently checks table names, row value, `journal_mode`, `foreign_keys`, and integrity.

**Stress fixture:** A well-formed config has invalid ordinary UI data plus valid enabled memory rooted at an absolute Unicode/spaces path; ordinary config must fall back while memory remains Valid. Additional roots cover an existing file, symlink, partial one-store initialization, a corrupt version row, and a held lock; no case reads HOME.

**Regression fence:** `crates/cyril-memory/src/config.rs` table tests; `crates/cyril-memory/src/paths.rs` platform/private-path tests; `crates/cyril-memory/src/store.rs` exact-schema/reopen/corruption/lock tests; existing `crates/cyril-core/tests/nd4h_legacy_config_compat.rs` remains green.

**Named mutation:** C1 — route memory through the broad `Config` deserializer so disabled-invalid becomes Absent; `disabled_invalid_memory_is_rejected` must turn red. C2 — remove Unix chmod or Windows protected-DACL application; `paths_and_permissions_are_private` must turn red. C3 — add a `jobs` table; the exact `sqlite_master` fence must turn red.

**Complexity/production scale:** Config parse is O(config bytes), one pass over an ordinary user config expected below 1 MiB; accepted cost ≤100 ms at 1 MiB, because startup parsing is local CPU only. Migration is O(fixed schema statements) with exactly two stores/one table each; accepted cost ≤1 s on a temporary local filesystem. Lock acquisition is one nonblocking O(1) system call.

**Wall budget/phase:** N/A — reason: config resolution, hardening, lock, and migration are one-off runtime-start phases; no always-on loop is introduced.

**Files:** `Cargo.toml`, `Cargo.lock`, `crates/cyril-memory/Cargo.toml`, `crates/cyril-memory/src/lib.rs`, `crates/cyril-memory/src/config.rs`, `crates/cyril-memory/src/error.rs`, `crates/cyril-memory/src/paths.rs`, `crates/cyril-memory/src/permissions.rs`, `crates/cyril-memory/src/store.rs`; target-specific dependency sections for safe Windows SID/ACL work.

**Estimate:** 1 implementation day.

**Diff estimate:** 1,850 changed lines including tests.

**PR increment:** runtime-storage-foundation

**Commands and expected results:**
- `cargo test -p cyril-memory config` → every absent/valid/invalid/unreadable/unparseable and field-shape row matches the independent matrix; legacy ordinary fallback is preserved.
- `cargo test -p cyril-memory paths` → explicit temporary roots canonicalize and harden; relative/file/symlink/permission cases return typed errors; platform cfg tests assert literal default mappings.
- `cargo test -p cyril-memory store` → `sqlite_master` contains only `schema_version`, both rows equal 1, WAL survives reopen/killed writer, corruption fails distinctly, and the second lock owner gets `already_running`.
- At checkpoint, apply each named mutation above → its named fence goes red; restore → all three focused commands return green and agree with the listed oracles.

## Slice 2: Add the authenticated runtime binary and real-process protocol

**Claim IDs:** C4, C5a, C5b, C6, C13

**Expected behavior:** `cyril-memory-runtime` binds one private local endpoint, obtains root ownership, opens both stores, and serves only correlated v1 health/shutdown responses through a 1 MiB authenticated frame. Ready reports instance/protocol/store versions only after every gate. Offender connections fail independently. A second runtime reports `already_running`; graceful/forced restart reopens version 1.

**Oracle:** The independent Python framing client computes every wire frame/error outcome; `stat(1)` and native Windows ACL access checks inspect endpoint privacy; child/lock facts and `sqlite3` inspect readiness and restart; dependency/source checks independently confirm current-user SID→DACL→CreateNamedPipe and Job Object interfaces.

**Stress fixture:** A real temporary runtime receives, in order, a truncated header, exact-cap malformed JSON, cap+1 header, missing/wrong-length/wrong-value auth, duplicate/zero IDs, version 2, unknown operation, then valid health on a fresh connection. The listener must remain alive and return the exact error table. A second runtime contends on the same Unicode/spaces data root.

**Regression fence:** `crates/cyril/tests/memory_runtime.rs` real-process suites for protocol invalid-input survival, endpoint privacy, readiness, exact health shape, second-owner failure, graceful restart, and forced restart; platform modules compile under workspace `unsafe_code = "forbid"` and native Windows CI exercises DACL denial.

**Named mutation:** C4 — accept auth unconditionally or remove max-frame check; the protocol matrix turns red. C5a — replace safe descriptor creation with Tokio raw security attributes; unsafe-forbid/Windows compile turns red. C5b — create a Windows pipe with null descriptor or omit Unix chmod; privacy fence turns red. C6 — emit Ready before authenticated health; readiness ordering fence turns red. C13 — replace retained OS lock with a deletable PID file; concurrent-owner fence turns red.

**Complexity/production scale:** Frame read/deserialize is O(n), n≤1,048,576, with one allocation capped to the announced length; accepted cost ≤100 ms per local 1 MiB frame and ≤1.1 MiB transient memory. Duplicate-ID tracking is O(1) because requests are sequential and monotonic per connection. Accept loop handles production M0's orchestration client plus bounded offender connections; no task/job queue exists.

**Wall budget/phase:** Always-on request phase: every request is bounded by configured `request_timeout_ms` (default 2,000 ms); production default maximum accepted wall cost is 2 s because a hung local peer must not pin orchestration. Runtime initialization is one-off and must settle within the caller's default 10 s startup deadline.

**Files:** `crates/cyril-memory/src/protocol.rs`, `crates/cyril-memory/src/wire.rs`, `crates/cyril-memory/src/client.rs`, `crates/cyril-memory/src/runtime.rs`, `crates/cyril-memory/src/ipc/mod.rs`, `crates/cyril-memory/src/ipc/unix.rs`, `crates/cyril-memory/src/ipc/windows.rs`, `crates/cyril/src/bin/cyril-memory-runtime.rs`, `crates/cyril/Cargo.toml`, `crates/cyril/tests/memory_runtime.rs`, `.github/workflows/ci.yml` or the existing platform workflow paths that enumerate binaries/tests.

**Estimate:** 1.5 implementation days.

**Diff estimate:** 2,450 changed lines including real-process tests.

**PR increment:** runtime-process-protocol

**Commands and expected results:**
- `cargo test -p cyril --test memory_runtime` → the real binary reaches Ready with version 1/1, returns the exact invalid-input error table, remains alive after offenders, denies a second owner, and reopens unchanged after graceful/forced exits.
- `cargo run --quiet --manifest-path .cyril-j7um/probe-platform/Cargo.toml` and `./.cyril-j7um/probe-platform-oracle.py` → item-by-item framing/process agreement remains identical to evidence.
- `cargo check --manifest-path .cyril-j7um/probe-platform/Cargo.toml --target x86_64-pc-windows-msvc` → safe current-user SID, protected-DACL pipe, and Job Object paths compile under Rust 1.94 with no warning.
- At checkpoint, each C4/C5a/C5b/C6/C13 named mutation turns only its named fence red; restoring returns all focused commands green.

## Slice 3: Wire fail-open startup, typed lifecycle, secret-safe launch, and bounded shutdown

**Claim IDs:** C7, C8, C12

**Expected behavior:** Main always starts the ACP bridge independently of memory readiness; enabled memory launches exactly one canonical absolute companion with a fresh credential in environment only; App receives typed lifecycle events; invalid/spawn/timeout/migration/IPC failures leave session creation and prompting usable; quit requests authenticated shutdown, waits two seconds, force-kills the contained tree if needed, waits under a hard outer bound, then returns for terminal restoration.

**Oracle:** A deterministic fake ACP process produces the same SessionCreated/prompt sequence with memory disabled and every injected memory failure; OS argv/process inspection and sentinel scans independently prove absolute singular launch and secret absence; `/proc`/Windows process enumeration plus independent elapsed clock prove shutdown/tree bounds.

**Stress fixture:** Runtime executable/data paths contain spaces and Unicode; a sentinel credential is searched across argv, config, logs, status, and health. A fixture runtime spawns a grandchild and ignores shutdown. Expected: one runtime process, no sentinel outside child environment/in-memory handle, ACP prompt still returns, and both descendants are absent before App returns within the recorded outer bound.

**Regression fence:** `crates/cyril/src/memory_runtime.rs` lifecycle tests; `crates/cyril/tests/memory_fail_open.rs` bridge/session/prompt matrix; `crates/cyril/tests/memory_shutdown.rs` real ignored-shutdown/process-tree bound; launch resolver tests for every executable path shape.

**Named mutation:** C7 — propagate `start_memory(...)?` from main; failure fixture exits before SessionCreated and turns red. C8 — call direct-child kill rather than wrapped group/job kill; grandchild remains and turns red. C12 — put the credential in argv or allow relative executable; sentinel/absolute-path fences turn red.

**Complexity/production scale:** Status forwarding is O(events) over a bounded channel; M0 emits ≤4 lifecycle events per runtime, maximum queue 16. Startup connect retries are O(startup_timeout/retry_interval), default ≤100 attempts at 100 ms and ≤10 s total; accepted cost is the configured startup deadline. Process launch and shutdown hold one child tree only.

**Wall budget/phase:** Startup is one-off with default 10 s deadline and never blocks bridge/session startup. Shutdown is one-off: request attempt uses configured default 2 s, graceful child-exit window is 2 s, forced kill/wait gets 500 ms; accepted hard outer bound 4.5 s before App returns, justified by terminal restoration safety.

**Files:** `crates/cyril/src/memory_runtime.rs`, `crates/cyril/src/main.rs`, `crates/cyril/src/app.rs`, `crates/cyril/Cargo.toml`, `crates/cyril/tests/memory_fail_open.rs`, `crates/cyril/tests/memory_shutdown.rs`, release workflow/script files that currently build/stage only `cyril`, and install documentation that names produced binaries.

**Estimate:** 1 implementation day.

**Diff estimate:** 1,850 changed lines including fixtures/tests.

**PR increment:** cyril-memory-status-integration

**Commands and expected results:**
- `cargo test -p cyril memory_runtime` → executable resolution accepts only canonical absolute runnable files, starts exactly one child, emits typed lifecycle order, and the sentinel secret appears only in the injected environment capture.
- `cargo test -p cyril --test memory_fail_open` → for disabled, invalid config, missing binary, startup timeout, migration failure, and IPC failure, SessionCreated and ordinary prompt output match the no-memory oracle.
- `cargo test -p cyril --test memory_shutdown` → normal shutdown exits gracefully; ignored shutdown is force-reaped with its grandchild and App returns within 4.5 s.
- At checkpoint, each C7/C8/C12 named mutation turns its fence red; restore returns all focused tests green.

## Slice 4: Add typed `/memory status` and TUI projection

**Claim IDs:** C9, C10, C11

**Expected behavior:** Core exports only immutable engine-neutral memory status/event/view types and an always-present local `/memory status`; App supplies the current view without a bridge/runtime round trip; UiState atomically retains process-global status across ACP session resets; UI formats safe details/versions and text-distinguishes all states, omitting only the default Disabled chrome chip to preserve the existing disabled surface.

**Oracle:** A literal five-row expected status/label/detail/version table is independent from formatter/renderer code; a closed bridge receiver proves local execution; `cargo metadata` plus an allowlisted public-symbol scan independently verify dependency direction and the M0-only request surface.

**Stress fixture:** Five states are rendered in no-color mode with absent/present 512-byte diagnostics and versions, then SessionCreated is applied. Expected: exact state survives, stale detail never leaks across replacement, non-disabled states have unique text, disabled `/memory status` says disabled while default full-frame snapshot remains unchanged, and no bridge send occurs.

**Regression fence:** `crates/cyril-core/src/types/memory.rs` public type/accessor/Send+Sync tests; command registration/execution/no-send tests; `crates/cyril-ui/src/state.rs` projection/reset tests; `crates/cyril-ui/src/memory_format.rs` five-state table; toolbar/status no-color tests and required chrome fixtures; dependency/public-contract test.

**Named mutation:** C9 — reset memory in the SessionCreated arm; persistence test turns red. C10 — dispatch `/memory` through `BridgeCommand`; closed-receiver no-send fence turns red. C11 — expose `rusqlite::Connection` or add `Recall`; dependency/public-symbol allowlist turns red.

**Complexity/production scale:** Status replacement is O(detail bytes), detail capped at 512 bytes; toolbar and command formatting are O(1) state fields with ≤512-byte output. Accepted cost ≤1 ms per status event/render-format call at maximum detail; redraw cadence remains owned by existing App activity logic.

**Wall budget/phase:** Always-on render projection adds constant work per redraw with accepted ≤1 ms at the 512-byte detail bound, preserving the existing frame budget. Status event/command formatting are one-off discrete events.

**Files:** `crates/cyril-core/src/types/memory.rs`, `crates/cyril-core/src/types/mod.rs`, `crates/cyril-core/src/commands/builtin.rs`, `crates/cyril-core/src/commands/mod.rs`, `crates/cyril-core/src/commands/subagent.rs`, `crates/cyril-core/src/commands/workflow.rs`, `crates/cyril/src/app.rs`, `crates/cyril/tests/event_routing.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/traits.rs`, `crates/cyril-ui/src/memory_format.rs`, `crates/cyril-ui/src/widgets/toolbar.rs`, `crates/cyril-ui/src/chrome_theme_tests.rs`, affected snapshots/fixtures, and architecture documentation describing `CommandContext`/the fifth crate.

**Estimate:** 1 implementation day.

**Diff estimate:** 1,450 changed lines including tests/fixtures/docs.

**PR increment:** cyril-memory-status-integration

**Commands and expected results:**
- `cargo test -p cyril-core memory` → all public methods and five states are covered; `/memory status` registers, validates args, returns typed state, and sends nothing to a closed bridge.
- `cargo test -p cyril-ui memory` → five-state formatter/projection/no-color rows match the independent table; SessionCreated preserves state; default Disabled full-frame output is unchanged.
- `cargo test -p cyril --test event_routing` → App passes current typed status through the local command result into UiState without ACP notification routing.
- `cargo metadata --format-version 1` plus the public-contract fence → core/UI dependency direction remains valid and the public request surface contains health/shutdown only.
- At checkpoint, C9/C10/C11 mutations each turn their named fence red; restore returns all focused commands green.

## Tracker taxonomy

- Permanent first-release non-goal: backend trait/selector/remote/no-op adapters — one real implementation creates no real seam.
- Permanent M0 non-goal: content, retrieval, vector, job, queue, model-worker, prompt-injection, MCP, and tool interfaces — no M0 production consumer.
- Intended future work remains in verified issues: turn capture/lexical recall `cyril-n3j7`; scoped MCP capability `cyril-3dqf`; teach/forget/promotion `cyril-s7gn`; consolidation jobs `cyril-nxq5`; embeddings `cyril-y91y`.

## Self-review

- [x] Every design row C1-C13 (including C5a/C5b) is assigned exactly once; every PENDING falsifier is carried by its implementing slice.
- [x] Every slice has all thirteen mandatory fields and no unclassified conditional blank.
- [x] Every claim's permanent fence and mechanical named mutation lands in the same slice.
- [x] Every loop states asymptotic/production cost and an explicit accepted bound; every always-on phase has a wall budget.
- [x] Diff sum 7,600 + 25% margin 1,900 = 9,500; three independently mergeable increments satisfy the >4,000 rule.
- [x] Every deferral phrase is classified as a permanent non-goal or cites a verified tracker ID.
- [x] No slice is declared complete; checkpointed-build owns completion.

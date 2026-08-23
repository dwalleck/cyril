# Design: cyril-j7um

## Route and inputs

- Route: **Empirical**, from [`route.md`](route.md).
- Behavior source: `spec.md` is `N/A — behavior fully explicit`; the complete given/when/then contract is `route.md` T4. In summary: absent/configured-off memory leaves current ACP/TUI startup usable; valid enabled memory starts one private runtime and reaches Ready only after locking, storage, IPC, and authenticated health; every startup/config/protocol failure is typed and fail-open toward ACP; shutdown is bounded and restart preserves stores; M0 exposes health/shutdown only.
- Empirical source: [`evidence.md`](evidence.md) and `probe.*`. P1-P3 validate Unix locking, WAL reopen, and owner-only socket modes. P5 validates a safe current-user-SID Windows pipe interface. P6 validates authenticated bounded framing. P7 validates bounded descendant reaping plus the Windows Job Object compile path. P4 and P8 are non-premises because executable/path resolution are behavior to implement and fence.
- Edge decisions extracted from the issue: memory validation runs even when disabled; malformed whole-file TOML is not absence; separate memory and knowledge SQLite files contain schema metadata only; the credential is 256 random bits in child environment only; IPC v1 has a 1 MiB cap and only health/shutdown; startup readiness is 10 seconds by default; request timeout is 2 seconds by default; graceful shutdown gets two seconds before process-tree termination.
- Delivery increments: `N/A — route.md carries no requester-approved increment split`; `plan.md` will apply the review-size gate after approval.

## Input shapes

| Input | Production-reachable shapes | Status |
|---|---|---|
| Config file | missing; unreadable; malformed TOML; well-formed without `[memory]`; well-formed valid `[memory]`; valid memory with unrelated ordinary-config error | Covered by C1 |
| `[memory]` fields | enabled absent/false/true; data root absent/absolute/relative/empty/Unicode/spaces; timeout absent/zero/positive/u64 overflow; unknown field; wrong scalar/table/type | Covered by C1-C2 |
| Data root | absent path; existing directory; existing file; symlink; inaccessible; wrong owner/mode/ACL; default Linux/macOS/Windows root; override outside cwd | Covered by C2 |
| Runtime endpoint | Unix runtime dir present/absent/relative/too long; stale socket path; Windows current-user SID available/unavailable; duplicate pipe name | Covered by C2, C5a-C6 |
| Store set | neither exists; both valid; one valid/one missing; both reopen; corrupt schema row; unsupported version; lock held by another process; killed writer/WAL recovery | Covered by C2-C3, C13 |
| Request frame length | fewer than 4 header bytes; zero; valid below cap; exactly 1 MiB; 1 MiB + 1; announced/actual mismatch | Covered by C4 |
| Request JSON | malformed; missing fields; unknown fields; request ID zero/new/duplicate; protocol v1/unsupported; empty/nonempty payload | Covered by C4 |
| Authentication | missing; wrong type; wrong length; correct length/wrong bytes; exact 32-byte credential | Covered by C4-C5b |
| Operation | health; shutdown; unknown string; empty string | Covered by C4 |
| Connections | no connection before deadline; one orchestration connection; invalid offender then valid client; multiple sequential offenders; offender disconnect mid-frame | Covered by C4, C6 |
| Runtime lifecycle | spawn error; bind error; lock failure; migration failure; authenticated Ready; pre-ready exit; startup timeout; post-ready exit; normal shutdown; ignored shutdown; forced kill | Covered by C6-C8, C13 |
| Executable | absolute existing runnable; relative; missing; directory; path with spaces/Unicode; test override; platform `.exe` suffix | Covered by C12 |
| Status projection | Disabled/Starting/Ready/Degraded/Failed; detail absent/present; versions absent/present; repeated identical/new status | Covered by C9-C10 |
| Platform | Linux; macOS; Windows; Unix permission bits; Windows protected DACL; process group; Job Object | Covered by C2, C5a-C5b, C8, C12 |
| Memory/session capability | scoped credential, binding, MCP operations | N/A — intended future work is `cyril-3dqf`; M0 must not create an unused handle or operation |
| Memory content/retrieval | empty stores only; turns, facts, embeddings, jobs, queues, agent tools | N/A — intended future work is `cyril-n3j7`, `cyril-s7gn`, `cyril-nxq5`, and `cyril-y91y` |
| Backend selection | local enabled or disabled only | N/A — permanent non-goal for first release: one production implementation does not justify a backend seam |

## Removed invariants

This change is additive. It does not remove bridge serialization, notification routing, session identity, UI modal precedence, or existing config fallback. `Config::load_from_path` remains unchanged for compatibility; main adopts an additive report that independently classifies memory.

## Placement

### Memory domain, config report, storage, and runtime

- **Owner:** new `cyril-memory` crate. It owns strict `[memory]` parsing/validation, data/runtime path resolution, permissions, exclusive ownership, migrations, IPC envelopes, stable memory errors, `MemoryRequest → MemoryResponse`, `AdminClient`, and runtime execution. This wins over `cyril-core` because persistence and IPC dependencies must not enter core, and over `cyril` because the runtime binary and real-process tests need the same deep module.
- **New seam A (chosen):** a versioned `MemoryRequest → MemoryResponse` domain interface wrapped by `AdminClient::{health, shutdown}`; `cyril` owns the child and calls the client. High leverage: callers learn two consumed methods while framing, auth, transport, correlation, and error mapping stay private.
- **Competing seam B (rejected):** `MemoryBackend` trait in core with disabled/local/no-op adapters. It creates a hypothetical seam with one real adapter, makes disabled configuration look like a backend, and invites dead future operations.
- **Config seam A (chosen):** `cyril_memory::load_config_report(path) -> ConfigLoadReport { ordinary: cyril_core::Config, memory: MemoryConfigState }`, preserving `Config::load_from_path` and strict-parsing only the memory subsection.
- **Config seam B (rejected):** add `memory: Option<MemoryConfig>` directly to core `Config`. A single serde failure would again collapse invalid-present memory into defaults and place runtime validation in the wrong crate.
- **Forbidden:** no ACP/MCP/ratatui types; no backend trait/selector; no session capability; no memory/retrieval/job table; no public raw SQLite connection, path-based repository, or parallel storage interface; no `.ok()`/default that collapses missing and corrupt.

### Runtime process lifecycle

- **Owner:** `cyril` orchestrator in a dedicated `memory_runtime` module. It resolves/canonicalizes an absolute companion executable, creates a fresh 32-byte credential, starts the child with endpoint/data-root/credential in environment, owns `process-wrap` containment, forwards typed status, sends authenticated shutdown, waits two seconds, then kills and waits for the whole group/job.
- **New seam A (chosen):** `MemoryRuntimeHandle` exposes current typed status, a bounded status receiver, and one `shutdown()` lifecycle method. The internal task owns child/admin-client state.
- **Competing seam B (rejected):** let `cyril-memory` spawn itself and hide the child. That violates the issue's ownership rule and makes terminal-restoration bounds invisible to App.
- **Forbidden:** credential in argv/config/log/status; parsing human process text in UI; blocking bridge creation/session startup on readiness; direct `tokio::process::Child::kill` on Windows without Job Object containment.

### Core status and local command

- **Owner:** `cyril-core` carries only `MemoryStatus`, immutable `MemoryStatusView`, typed safe diagnostics, and optional `MemoryStoreVersions`. `/memory status` is an always-registered local command returning a typed `CommandResultKind::MemoryStatus` from `CommandContext`.
- **New seam:** slots behind existing command/result and core-type interfaces; no ACP `Notification`, `RoutedNotification`, or `SessionController` change.
- **Forbidden:** storage/IPC/process types in core; session-scoped routing; formatting labels/colors in core.

### UI projection and rendering

- **Owner:** `cyril-ui` owns `UiState::set_memory_status`, the `TuiState` getter, pure `/memory status` formatting, and a compact status-bar projection. App owns status-event routing and redraw.
- **New seam:** slots behind `TuiState`; no new modal or key/mouse layer.
- **Forbidden:** opening stores, resolving paths, starting processes, parsing IPC/process output, mutating `Activity` or `SessionStatus`.
- **Default-disabled rule:** `UiState` explicitly stores Disabled and `/memory status` renders it, but the chrome omits the Disabled chip. This preserves the existing disabled startup surface; Starting/Ready/Degraded/Failed render distinct text/icons independent of color.

### Packaging

- **Owner:** the `cyril` package adds a `cyril-memory-runtime` binary target backed by `cyril-memory`; release workflows build/stage both binaries. Real-process integration tests live in `crates/cyril/tests` and use Cargo's binary path, explicit temporary data/runtime roots, and the same `AdminClient`.
- **Forbidden:** PATH lookup for the runtime, cwd-relative stores/endpoints, test reads of HOME/XDG/LOCALAPPDATA, or a test-only public protocol.

## Claims

- **C1:** Presence-aware loading preserves ordinary fallback while classifying memory as Absent, Valid, Invalid, Unreadable, or ConfigUnparseable, and validates present fields even when disabled.
- **C2:** Runtime paths are canonical OS-appropriate user locations, and the data root plus descendants are owner-private before stores open.
- **C3:** The two stores independently migrate under the root lock to WAL/foreign-keys-on databases containing only one explicit schema-version table/row and reopen without destructive initialization.
- **C4:** IPC v1 enforces a 1 MiB length-prefix cap, request correlation, unique nonzero IDs per connection, constant-time admin authentication, exact health/shutdown dispatch, and stable safe errors without killing the listener.
- **C5a:** Rust 1.94 can compile safe current-user SID retrieval, protected-DACL Tokio named-pipe creation, and Job Object/process-group containment without unsafe Cyril code.
- **C5b:** Unix sockets are mode 0600 under a 0700 runtime directory; Windows pipes use a protected DACL naming the current process user SID and reject remote clients.
- **C6:** Memory reaches Ready only after root ownership, permissions, both migrations, listener bind, and authenticated health; every pre-ready failure becomes Failed with typed actionable detail.
- **C7:** Memory-disabled, invalid, timeout, spawn, migration, and IPC failures never prevent bridge startup, SessionCreated, or ordinary prompting.
- **C8:** Quit sends authenticated shutdown, allows two seconds, then process-group/Job-Object kills and waits for descendants within a hard outer bound before terminal restoration.
- **C9:** App projects process-global status atomically into UiState; Disabled/Starting/Ready/Degraded/Failed remain independent of ACP session/activity state and are text-distinguishable without color.
- **C10:** `/memory status` is local, always available, performs no bridge/runtime request, and formats the current typed state, safe detail, protocol, and both schema versions.
- **C11:** `MemoryRequest → MemoryResponse` plus AdminClient is the only external memory seam; domain types leak no ACP, MCP, ratatui, SQLite, native-model, or process types and expose no unconsumed future operation.
- **C12:** Cyril starts exactly one runtime through a canonical absolute executable; the random credential appears only in child environment and in-memory client state, never argv, persistent config, logs, status, or health.
- **C13:** A second runtime for the same canonical root returns `already_running`; graceful or forced exit releases ownership and the next runtime reopens both version-1 stores.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Presence-aware config is lossless and strict for memory. | Full config/presence/type/path/timeout matrix | Table-driven loader test; any classification or ordinary fallback differing from expected falsifies. | Python `tomllib` presence/type table plus legacy `Config::load_from_path` assertions. | In `config.rs`, deserialize `Option<MemoryConfig>` through the broad loader; `disabled_invalid_memory_is_rejected` turns green→red. | `cyril-memory::config` unit matrix + existing `nd4h_legacy_config_compat`. | 1 min | PENDING — checkpointed-build, config slice |
| C2 | Paths and permissions are canonical/private before open. | OS/root/path matrix | Temporary-root tests inspect canonical placement/modes/DACL before DB open; relative override acceptance falsifies. | `stat(1)` on Unix; Windows ACL enumeration in native CI; expected path table computed without production resolver. | Remove Unix chmod or Windows protected-DACL application in `permissions.rs`; owner-private assertion turns red. | `paths_and_permissions_are_private` on Unix/Windows. | 2 min + Windows CI | PENDING — checkpointed-build, runtime-foundation slice |
| C3 | Stores are minimal, versioned, WAL, and reopenable. | Missing/partial/existing/corrupt/killed-writer stores | Real runtime initialization/restart and forced-kill test; extra table, lost version, non-WAL, or destructive reopen falsifies. | `sqlite3` CLI queries `sqlite_master`, schema row, PRAGMAs, and integrity check. | Add a speculative `jobs` table in migration; exact table-set assertion turns red. | `real_runtime_initializes_and_reopens_minimal_stores`. | 3 min | PENDING — checkpointed-build, storage slice |
| C4 | Framing/auth/version/operations are bounded and typed. | Frame/auth/version/ID/operation/connection matrix | Independent client sends every case to real runtime; wrong code, accepted unauthorized input, listener death, or request-ID mismatch falsifies. | Python socket/named-pipe client builds frames independently and checks stable code table. | Replace constant-time compare with unconditional true or remove max length in `wire.rs`; auth/cap tests turn red. | `real_runtime_rejects_invalid_ipc_without_listener_loss`. | 3 min + Windows CI | PENDING — checkpointed-build, IPC slice |
| C5a | Safe platform mechanisms exist without project unsafe code. | Windows current-user SID/DACL/Job Object and Unix process group | Compile the isolated mechanism probe for host and `x86_64-pc-windows-msvc`; any unavailable API, unsafe call requirement, or MSRV failure falsifies. | Library source traces SID to `TokenUser`, descriptor to `CreateNamedPipeW`, and wrapper to Job Object/process group; Microsoft documents the OS contracts. | Replace `interprocess` descriptor creation with Tokio's unsafe raw-attributes call; `#![forbid(unsafe_code)]`/compile fence turns red. | `platform_mechanisms_compile_without_project_unsafe` in the platform modules and Windows CI. | <1 min | PASS — exact Rust 1.94 host and Windows-target probe compiled |
| C5b | IPC endpoint is user-private on both families. | Unix socket and Windows current-user SID | Native endpoint ACL/mode test; any broad ACE/remote acceptance, missing descriptor, or other-user connection falsifies. | `stat(1)` and Microsoft/Windows ACL access check from a distinct test user/token. | Use Tokio `ServerOptions::create` with null security attributes on Windows; ACL assertion/other-user denial turns red. | `ipc_endpoint_is_current_user_only` on Unix/Windows. | 2 min native | PENDING — checkpointed-build, IPC slice |
| C6 | Readiness is complete and failures typed. | Every startup state/failure | Fault-injected real child delays/exits/corrupts store/contends lock; Ready before all gates or generic/incorrect failure falsifies. | Filesystem/SQLite/child-exit facts inspected independently after each run. | Emit Ready before health in supervisor; `ready_requires_authenticated_health` turns red. | `startup_state_matrix_is_typed_and_complete`. | 4 min | PENDING — checkpointed-build, orchestration slice |
| C7 | ACP remains available under memory failure. | disabled/invalid/spawn/timeout/runtime failures | Start Cyril harness with broken memory and fake/real ACP fixture; missing SessionCreated or prompt response falsifies. | Same ACP fixture without memory, comparing ordered bridge notifications and prompt response. | Propagate `start_memory(...)?` from main; failure scenario exits before SessionCreated and fence turns red. | `memory_failure_does_not_block_bridge_or_prompt`. | 5 min | PENDING — checkpointed-build, integration slice |
| C8 | Shutdown is graceful then hard-bounded/tree-complete. | normal, ignored request, descendant, already-exited | Real runtime/fixture tests measure outer deadline and descendant liveness; survivor or bound breach falsifies. | `/proc`/`tasklist` liveness and independent elapsed clock; process-platform probe. | Replace wrapped-child kill with direct child kill; grandchild liveness fence turns red. | `shutdown_reaps_tree_within_bound` on Unix/Windows. | 3 min + Windows CI | PENDING — checkpointed-build, orchestration slice |
| C9 | Status projection is global, atomic, and visible without color. | Five states/details/repeated updates/session reset | UiState transition + TestBackend/no-color matrix; reset, stale detail, or indistinguishable labels falsifies. | Explicit five-row expected text/status table independent of renderer mapping. | Reset memory status in `SessionCreated`; persistence-across-session test turns red. | `memory_status_projection_and_render_matrix`. | 2 min | PENDING — checkpointed-build, UI slice |
| C10 | `/memory status` is local and typed. | Five states; no bridge sender; invalid args | Execute command with closed bridge receiver and each state; send or incorrect output/usage falsifies. | UI formatter expected table from typed view. | Implement as `BridgeCommand::ExecuteAgentCommand`; no-send assertion turns red. | `memory_status_command_is_local_for_all_states`. | 1 min | PENDING — checkpointed-build, command slice |
| C11 | One deep seam has no dependency/type leaks or future stubs. | Public interface and manifests | `cargo public-api`/source audit plus dependency graph; a forbidden type/dependency or extra request variant falsifies. | `cargo metadata` dependency edges and an allowlisted public-symbol table. | Add `rusqlite::Connection` to a public request or `Recall` variant; allowlist fence turns red. | `memory_public_contract_is_m0_only` plus crate dependency checks. | 2 min | PENDING — checkpointed-build, runtime-foundation slice |
| C12 | Launch is absolute, singular, and secret-safe. | executable path matrix; argv/env/log/health | Spawn-recording fixture and real launch inspect argv, environment handoff, process count, health/status/logs; relative path or credential occurrence falsifies. | OS process argv inspection plus independent captured config/log/status scan using a sentinel secret. | Append credential as command arg in `memory_runtime.rs`; sentinel scan turns red. | `runtime_launch_is_absolute_single_and_secret_safe`. | 3 min | PENDING — checkpointed-build, orchestration slice |
| C13 | Ownership and reopen survive both exit modes. | second owner; graceful restart; forced restart | Start two real runtimes then restart after graceful/forced exit; second Ready or lost/change versions falsifies. | lock acquisition with independent process and `sqlite3` schema/version reads. | Delete a PID sentinel instead of retaining OS lock; concurrent-owner fence turns red. | `exclusive_owner_and_restart_matrix`. | 4 min | PENDING — checkpointed-build, integration slice |

## Non-goals and future work

- Permanent non-goal for the first release: no `MemoryBackend` trait, backend selector, remote/no-op adapter, or fake-success local mode. Rationale: one real adapter creates no real seam; the versioned domain interface is the replacement seam when a second production implementation exists.
- Permanent M0 non-goal: no memory/knowledge/vector/job/queue schema beyond version metadata, no retrieval, embeddings, transcript capture, prompt injection, MCP registration, agent tools, or model worker. Rationale: none has an M0 production consumer.
- Intended future work: authoritative turn capture and lexical recall — `cyril-n3j7`.
- Intended future work: session capability and scoped MCP recall — `cyril-3dqf`.
- Intended future work: teach/forget/promotion governance — `cyril-s7gn`.
- Intended future work: durable consolidation jobs — `cyril-nxq5`.
- Intended future work: local embeddings and retrieval-quality gates — `cyril-y91y`.

## Falsifier run log

- 2026-08-22 — C5a/P5 cheapest mechanism falsifier:
  - `cargo run --quiet --manifest-path .cyril-j7um/probe-platform/Cargo.toml` — PASS: Unix process-group descendant reaped within two seconds; all seven authenticated framing outcomes matched.
  - `./.cyril-j7um/probe-platform-oracle.py` — PASS: independent Python framing/process oracle agreed.
  - `cargo check --manifest-path .cyril-j7um/probe-platform/Cargo.toml --target x86_64-pc-windows-msvc` — PASS: Rust 1.94 compiled safe current-user SID retrieval, protected DACL, local-only Tokio named pipe, and Job Object paths without project unsafe code.

## Approval

- Status: APPROVED.
- Requester words: “Approve design as written”
- Date: 2026-08-22.
- Risk acceptances: None — every claim has a deterministic regression fence; no `N/A — approved risk` row exists.

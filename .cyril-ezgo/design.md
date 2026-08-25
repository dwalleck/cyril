# Design: cyril-ezgo

## Route and inputs

- Route: **Empirical**, from [`route.md`](route.md). T1 is discharged; T2 remains structurally load-bearing.
- Behavior source: `spec.md` is **N/A — behavior fully explicit**. The complete given/when/then set is `route.md` § Route tests, T4: teach; list/inspect; restart persistence; worktree-stable project identity; bounded first-prompt injection; transcript separation; exactly-once behavior; fail-open fallback; trust separation; fake-agent regression proof.
- Empirical premise: [`evidence.md`](evidence.md) P1. The standalone [`probe.py`](probe.py) and independent Git/`realpath` oracle agree that the primary checkout and linked worktree share `/home/dwalleck/repos/cyril/.git` while retaining distinct canonical display paths; `/tmp` resolves to itself as a non-Git workspace. P1 is `PASS`.
- Specification edge table: N/A — no `spec.md`; the decisions below sharpen the issue's existing Design field without changing its observable scope.
- Delivery increments: N/A — owned by `budgeted-plan` after approval.

## Input shapes

| Input | Production-reachable shapes | Status |
|---|---|---|
| Session workspace | Canonical primary Git checkout; canonical linked worktree; nested directory below either; canonical non-Git directory; relative CLI path; Unicode/spaces; missing or unreadable path | C1, C11; missing/unreadable resolves to typed memory-unavailable while ordinary ACP startup retains its own existing error path |
| Runtime state | Disabled-absent; disabled-configured-off; starting; ready; degraded; failed; runtime disappears after ready | C4, C8 |
| Lesson command | `/memory status`; `teach` with absent/whitespace, ASCII, Unicode, CRLF, control characters, at-bound and over-bound text, supported secret shapes; exact duplicate; `teach --replace` with active, invalidated, malformed, missing, or other-project ID; `list` empty/single/many; `inspect` active/invalidated/missing/malformed | C2, C3, C5 |
| Lesson collections | Empty; one; many distinct; duplicate normalized content; active plus invalidated history; more than the list limit | C2, C5, C9 |
| Prompt blocks | Empty vector; one original text block; multiple distinct blocks including file attachments; duplicate blocks; Unicode content | C6, C7, C8 |
| Session prompt ordinal | First accepted prompt; second and later prompts; session ID reused by a new-session response; prompt send rejected before ACP dispatch | C6, C8 |
| Context budget | Zero; smaller than fixed framing; exact whole-lesson boundary; one lesson too large; several fitting lessons; overflow with omitted count; 4,000-character production maximum | C9 |
| Lesson trust/status | `user_explicit` + `instruction` + active; invalidated explicit history; any future derived/document provenance or non-instruction trust | C2, C9; future rows are rejected from the instruction query by positive selection, not a catch-all exclusion |
| Store lifecycle | Fresh v1 memory store; migrate to v2; reopen v2; partial initialization; interrupted write; second owner; corrupt/unsupported schema | C2, C12 |
| Wire request | Existing v1 health/shutdown null payloads; each new typed payload; unknown operation/field; malformed/oversized frame; wrong auth; non-monotonic ID | C10 |
| Secret detector | Key/value assignments (`password`, `passwd`, `token`, `secret`, `api_key`, `apikey`, `access_key`, `private_key`); GitHub token prefixes; AWS access-key IDs; OpenAI-style `sk-` tokens; PEM private-key blocks; ordinary lookalikes below detector thresholds | C3 |
| Time/order | Same-millisecond writes; restart; multiple active lessons; replacement transaction | C2, C9 |

Numeric negative values are **N/A — unreachable**: CLI text has no numeric lesson argument; internal budgets and limits use `usize`. Image/resource ACP content is **N/A — current `BridgeCommand::SendPrompt` carries text `Vec<String>` only**. Loaded-session injection is **N/A — cyril-ezgo requires fresh sessions; `BridgeCommand::LoadSession` has no trusted workspace and adding load binding belongs to capability-bound recall in `cyril-3dqf`**.

## Removed invariants

The move is additive. It does not remove the bridge's one-active-turn guard, original `BridgeCommand::SendPrompt` ownership, runtime single-owner lock, frame cap, authentication, or UI message commit ordering. It does add a prefix transformation at the bridge seam; C6–C8 fence the pre-existing invariant that every original prompt block remains byte-equivalent and in order on success and failure.

## Placement

### Project identity and lesson domain

- **Owner:** `cyril-memory`. New `project` and `lesson` modules own `ProjectScope`, opaque `ProjectId`, `LessonId`, normalization, validation, redaction, provenance/trust/status, audit metadata, list/inspect views, and context rendering. This owner wins because the rules must be identical for CLI teaching and bridge injection and must not be recreated in UI or ACP code.
- **New seam:** `MemoryClient::bind_project(&Path) -> Result<ProjectMemory, ClientError>`. `ProjectMemory` is a deep module: `teach`, `replace`, `list`, `inspect`, and `prepare_first_prompt` hide canonical identity, protocol payloads, scope enforcement, ordering, budgeting, and storage.
- **Forbidden:** no ACP, MCP, ratatui, bridge command, UI state, process-cwd lookup, SQL, or raw credential leaves this module. Callers cannot supply a `ProjectId` after binding.

Identity resolution canonicalizes the supplied workspace once. It walks ancestors for `.git`; a `.git` directory is the common directory, while a `.git` file resolves `gitdir:` and its optional `commondir`. The opaque `ProjectId` is SHA-256 over a platform-tagged canonical common-directory path (Unix raw bytes; Windows UTF-16 code units). Non-Git workspaces hash the platform-tagged canonical workspace path. The canonical workspace remains a separate display path. No operation re-reads process cwd.

Lesson text normalization converts CRLF/CR to LF, trims surrounding Unicode whitespace, preserves internal whitespace, rejects NUL and non-tab/non-newline control characters, and accepts 1–2,000 Unicode scalar values after normalization. Redaction then replaces the explicitly enumerated secret shapes from the Input shapes table with `[REDACTED]`; raw input is dropped before hashing, protocol serialization, SQL, audit, or logging. `content_hash` is SHA-256 of normalized redacted UTF-8.

`/memory teach <text>` creates a random 128-bit hex `LessonId`. `/memory teach --replace <lesson-id> <text>` atomically invalidates that active same-project row and inserts a new row linked by `supersedes_id`; invalidated/missing/foreign IDs return the same safe not-found result. Exact normalized redacted content already active in the project is idempotent and returns the existing ID. Every create, replace, and duplicate teaching attempt appends an audit event containing IDs, action, timestamp, and content hash but no lesson text.

### Store and protocol

- **Owner:** `cyril-memory::store`, `protocol`, `wire`, and `runtime` retain their M0 responsibilities.
- **New seam:** none — M1 deepens the existing versioned runtime interface with typed project-lesson operations. Protocol v1 remains v1 because existing health/shutdown frames remain valid byte-for-byte and the new operations are additive; operation-specific payload decoders replace the current global `payload == null` rule while health/shutdown still require null.
- **Forbidden:** no generic storage backend trait/selector, unbounded row load, SQL in client/UI/binary, trust expressed as free-form caller strings, or error response containing paths/content/credentials.

The memory store migrates from schema v1 to v2 in one transaction. Tables: `projects` (opaque ID, latest display path, timestamps), `lessons` (monotonic sequence, opaque ID, project FK, redacted normalized content, hash, fixed provenance/trust/status values, supersedes link, timestamps), and `memory_audit` (project/lesson/action/hash/timestamp). A partial unique index enforces one active row per `(project_id, content_hash)`. Newest ordering is `sequence DESC`, independent of clock resolution. The knowledge store remains schema v1.

`list` returns at most 100 active rows plus `omitted_count`; each summary preview is at most 160 characters. `inspect` returns one same-project active or invalidated row. `prepare_first_prompt` performs `COUNT(*)` plus a newest-first cursor and stops reading once the budget cannot admit another whole lesson.

### Runtime access

- **Owner:** `cyril-memory::client` owns cloneable `MemoryClient`/`ProjectMemory`; `cyril/src/memory_runtime.rs` owns process lifecycle and publishes a credential-hiding access descriptor only while the runtime is Ready.
- **New seam:** `MemoryRuntimeHandle::bind_project` returns a binary-held `ProjectMemory` whose operations obtain the current ready client; it can represent starting/disabled/degraded/failed without exposing endpoint or credential.
- **Forbidden:** `AdminCredential` never enters `App`, `CommandContext`, bridge commands, argv, config, logs, Debug output, or UI types. Lesson methods are not added to `AdminClient`; shutdown privilege stays separate from project-scoped methods.

### `/memory` command and UI projection

- **Owner:** `cyril-core::commands::builtin::MemoryCommand` owns syntax only; `App` owns the concrete cross-crate call; `cyril-ui::memory_format` owns rendering typed read-only projections.
- **Competing seam A (selected):** new `CommandResultKind::MemoryAction` carries `Teach`, `Replace`, `List`, or `Inspect` intent. `App::submit_input` executes it against its concrete `ProjectMemory`, maps the memory-domain response into core-neutral `MemoryLessonView` projection types, and calls UI formatters. This preserves a persistence-free core/UI and avoids a hypothetical backend selector.
- **Competing seam B (rejected):** put a memory backend trait/client in `CommandContext`. It enlarges every command test fixture, creates a one-adapter backend abstraction explicitly rejected by M0, and makes core commands know runtime errors.
- **Competing seam C (rejected):** move `/memory` out of the core registry into the binary. It creates a second command convention beside the existing registry and weakens `/help`/parse locality.
- **Forbidden:** command/UI code does not validate lesson contents, redact, hash, order, budget, query SQL, or format stored protocol rows. `cyril-ui` does not depend on `cyril-memory`.

### First-prompt adapter

- **Owner:** `App::send_prompt` owns the once-per-session attempt state (`first_prompt_lessons_pending: Option<SessionId>`, armed on `SessionCreated`) and wire prefixing; `ProjectMemory::first_prompt_context(&self)` owns context selection/rendering and the 250 ms deadline; `App` also owns the visible original-user-message projection.
- **Competing seam A (rejected in review; originally selected):** a core-owned `FirstPromptContext` trait consulted by the bridge mediator. Every `SendPrompt` producer already lives in `app.rs` (typed submit, the startup `--prompt`, `/code` follow-ups), so the bridge placement bought no coverage while adding a core trait, `SpawnConfig`/`BridgeLoopConfig` plumbing, and a 250 ms inline await on the mediator that queued cancel/steer behind it.
- **Competing seam B (rejected):** make `cyril-core` depend on `cyril-memory`. This creates a dependency cycle and violates persistence-free core.
- **Competing seam C (selected):** prepend in `App` at the single `send_prompt` seam that all three producers call. The lookup runs on a spawned task and the prompt is dispatched from the `memory_task_rx` result handler, so the event loop never blocks on the companion.
- **Forbidden:** the bridge never opens storage, resolves project paths, selects rows, parses lesson responses, or reconstructs framing. The memory module never imports ACP types.

For each `SessionCreated`, `App` arms `first_prompt_lessons_pending = Some(session_id)` (a resumed session counts as fresh for this process). On that session's first non-empty prompt, `send_prompt` clears the flag **before** the lookup, marks the session Busy, and calls `ProjectMemory::first_prompt_context()` — no query text is passed; the block is selected newest-first within the fixed 4,000-character `MAX_CONTEXT_CHARS` budget under a 250 ms deadline. A returned block becomes a new first ACP text block; every original block follows unchanged. `None`, disabled/degraded/failed state, channel closure, error, or deadline expiry sends the original vector unchanged with the concrete cause logged at `warn!`. Only a companion that is still *starting* re-arms the session, so its next prompt is augmented. Empty original vectors are sent unchanged and do not consume the attempt. Later prompts never call memory.

The returned block has fixed framing:

```text
<CYRIL_LESSONS trust="user_explicit_instruction">
These are explicit project instructions taught by the user. Follow them unless the current user request supersedes them.
- <redacted lesson text>
...
[<N> additional lesson(s) omitted]
</CYRIL_LESSONS>
```

Only active `user_explicit` + `instruction` rows are eligible. The total Unicode scalar count never exceeds the caller's budget. Header, footer, bullets, and omitted marker count against the budget; lessons are admitted whole or omitted.

### Bound workspace

- **Owner:** `main` resolves the initial CLI/default workspace to one canonical absolute path; `App` stores it; `NewCommand` receives it at registry construction just like hooks/workflow sources.
- **New seam:** none — this repairs the current `/new` process-cwd re-read by reusing the startup-bound value.
- **Forbidden:** no session creation, memory command, or prompt adapter calls `std::env::current_dir()` after startup binding.

## Claims

- **C1:** One canonical project identity is stable across a primary Git checkout and its linked worktrees, while non-Git workspaces use their canonical workspace identity and every workspace retains a separate display path.
- **C2:** Schema v2 preserves active and superseded explicit lessons, project isolation, deterministic newest-first order, idempotent duplicates, and an audit event for every teaching attempt across reopen.
- **C3:** Validation and supported secret detection prevent raw rejected or secret-bearing lesson input from reaching protocol payloads, hashes, SQL rows, audit rows, logs, or Debug output.
- **C4:** Project-scoped lesson behavior remains behind `cyril-memory`'s concrete deep interface; core and UI stay persistence-free and no credential-capable client escapes runtime lifecycle ownership.
- **C5:** `/memory status|teach|list|inspect` and `teach --replace` produce typed, bounded, project-scoped user output without duplicating memory-domain policy in command or UI code.
- **C6:** The bridge attempts context exactly once per accepted fresh session and, on success, sends `[complete memory block, original block 0, ... original block N]` with every original block byte-equivalent and ordered.
- **C7:** Successful wire injection never changes the TUI's stored/rendered original user message and injected text never becomes source input.
- **C8:** Disabled, starting, degraded, failed, missing, erroring, closed, or over-deadline memory sends the exact original block vector and leaves the ACP turn usable; failure also consumes the session's one automatic attempt.
- **C9:** Context rendering selects only active `user_explicit`/`instruction` lessons, orders by monotonic sequence newest-first, remains within 4,000 characters, truncates only between lessons, and reports the exact omitted count.
- **C10:** Additive protocol-v1 operations preserve existing authenticated health/shutdown behavior, strict operation-specific payload validation, monotonic IDs, safe errors, and the 1 MiB pre-allocation frame cap.
- **C11:** Initial and `/new` sessions use the canonical startup-bound workspace; changing process cwd or constructing commands elsewhere cannot retarget project scope.
- **C12:** A real runtime restart preserves project, lesson, supersession, and audit state and reopens with memory schema v2/knowledge schema v1 without weakening exclusive ownership or private permissions.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Stable canonical project identity | Primary, linked, nested, non-Git, distinct displays | Resolve the real primary/linked/non-Git paths; differing Git IDs or a non-self non-Git ID falsifies C1. | `git rev-parse --path-format=absolute --git-common-dir` plus `realpath`, independent of the probe and Rust resolver. | `crates/cyril-memory/src/project.rs`: return canonical workspace for linked `.git` files; `project_identity_matches_git_common_dir_and_keeps_display_paths` becomes red. | `project::tests::project_identity_matches_git_common_dir_and_keeps_display_paths` plus retained `probe.py` comparison. | <1 minute | PASS |
| C2 | Durable scoped history/idempotency/audit | Fresh/migrated/reopened; duplicate; replacement; cross-project; same timestamp | Execute create, duplicate, replace, list/inspect in two projects, reopen, and compare rows/invariants; any duplicate active row, cross-project result, mutated history, nondeterministic order, or missing audit falsifies C2. | A separate read-only rusqlite connection queries tables/indexes directly and hand-counts expected state. | `store.rs`: remove the partial unique index or update old content in place; `lesson_lifecycle_is_scoped_idempotent_and_append_audited` becomes red. | `store::tests::lesson_lifecycle_is_scoped_idempotent_and_append_audited`; `memory_v1_migrates_atomically_to_v2`. | 2 minutes | PENDING — checkpointed-build, storage slice gate |
| C3 | Raw secrets never persist or leak | Validation bounds/control chars plus every detector and ordinary lookalike | Teach a table of known secrets/lookalikes, then inspect responses, captured wire JSON, SQL text/audit, logs, and Debug; any raw supported secret outside the input fixture or redacted ordinary lookalike falsifies C3. | A fixed detector fixture table with exact expected redacted strings plus direct database queries/byte scan, independent of production matcher branches. | `lesson.rs`: bypass `redact` before hashing/storage; `supported_secrets_are_redacted_before_every_boundary` exposes fixture token and becomes red. | `lesson::tests::supported_secrets_are_redacted_before_every_boundary`; runtime integration `raw_secret_never_crosses_or_persists`. | 2 minutes | PENDING — checkpointed-build, lesson-domain slice gate |
| C4 | Deep concrete memory interface and dependency discipline | Every runtime state and caller crate | Compile public callers and inspect dependency graph; any core/UI dependency on cyril-memory, public SQL/credential accessor, or policy in App/UI falsifies C4. | `cargo tree -p cyril-core`/`-p cyril-ui` and Cargo manifest parsing, independent of Rust type implementation. | `crates/cyril-core/Cargo.toml`: add `cyril-memory`; `architecture_tests::core_and_ui_remain_persistence_free` becomes red. | `crates/cyril/tests/architecture_tests.rs::core_and_ui_remain_persistence_free` plus compiler visibility checks. | 1 minute | PENDING — checkpointed-build, client/wiring slice gate |
| C5 | Complete typed `/memory` UX | Command syntax matrix; empty/single/many; active/invalidated/missing/foreign | Execute each command against a fake typed App memory projection; wrong parse, policy in UI, unbounded list, unsafe not-found distinction, or missing provenance/trust falsifies C5. | Hand-authored command/result table and exact UI strings, independent of store and formatter implementation. | `commands/builtin.rs`: treat `teach` as a system string instead of `MemoryAction`; `memory_commands_emit_typed_actions` becomes red. | `commands::tests::memory_commands_emit_typed_actions`; `memory_format::tests::lesson_command_matrix_is_bounded_and_typed`; App command integration test. | 2 minutes | PENDING — checkpointed-build, command/UI slice gate |
| C6 | Exactly-once ordered wire prefix | Empty/one/multi/duplicate/Unicode blocks; first/later/reused session | Capture FakeAgent prompt contents for two prompts and a reused session; any second injection, missing first prefix, changed/reordered original, or retry after failed attempt falsifies C6. | The test retains a cloned original `Vec<String>` and constructs expected ACP block vectors directly, without calling production preparation code. | `bridge.rs`: move `memory_attempted = true` after await or replace `insert(0, block)` with overwrite; `first_prompt_memory_is_ordered_and_exactly_once` becomes red. | `bridge::tests::first_prompt_memory_is_ordered_and_exactly_once`; `empty_prompt_consumes_attempt_without_mutation`. | 2 minutes | PENDING — checkpointed-build, bridge slice gate |
| C7 | Visible transcript/source separation | Interactive prompt, startup `--prompt`, file attachments, injected block | Drive App prompt submission with an injecting bridge harness; any `CYRIL_LESSONS` text in `UserText`, source capture, or attachment display falsifies C7. | Compare UI messages to the original input fixture and FakeAgent wire capture to a separately constructed prefixed vector. | `app.rs`: call `add_user_message` with prepared wire text; `injected_context_is_wire_only` becomes red. | App/integration `injected_context_is_wire_only_for_interactive_and_startup_prompts`. | 2 minutes | PENDING — checkpointed-build, end-to-end slice gate |
| C8 | Deadline-bound exact fail-open | Every non-ready/error/closed state and pending adapter; first/later prompt | With paused Tokio time, return `None`, typed error, closed future, and forever-pending future; any changed original vector, stuck turn, second attempt, or missing completion falsifies C8. | Original-vector equality plus FakeAgent turn completion; paused virtual clock independently proves the 250 ms bound. | `bridge.rs`: use `?` on context error or omit `tokio::time::timeout`; `memory_failure_and_timeout_are_exact_fail_open` becomes red/hangs under virtual-time bound. | `bridge::tests::memory_failure_and_timeout_are_exact_fail_open` with `start_paused = true`. | 2 minutes | PENDING — checkpointed-build, bridge slice gate |
| C9 | Trusted bounded deterministic rendering | Empty; one too large; exact fit; multiple overflow; invalidated/derived/non-instruction | Render fixtures at budgets 0 through 4,000 and compare chars/order/omitted count; any ineligible row, partial lesson, over-budget block, or wrong omitted count falsifies C9. | A small test-only greedy reference over immutable fixture lengths and eligibility flags, not the SQL query or production renderer. | `lesson.rs`: truncate the final string at budget or query without positive trust predicate; `context_block_respects_trust_order_and_whole_lesson_budget` becomes red. | `lesson::tests::context_block_respects_trust_order_and_whole_lesson_budget`. | 1 minute | PENDING — checkpointed-build, lesson-domain slice gate |
| C10 | Compatible strict authenticated protocol v1 | Old null operations; new typed operations; malformed/unknown/oversized/auth/id matrix | Replay legacy health/shutdown frames and each new valid/invalid frame through real codec/runtime; changed legacy response or accepted invalid frame falsifies C10. | Hand-authored raw JSON frames and response-code table, independent of typed serializer/deserializer. | `wire.rs`: deserialize new payloads with a permissive `Value`/ignore unknown fields; `v1_operation_payload_matrix_is_strict_and_backward_compatible` becomes red. | `wire::tests::v1_operation_payload_matrix_is_strict_and_backward_compatible`; existing cap/auth/id tests. | 2 minutes | PENDING — checkpointed-build, protocol slice gate |
| C11 | No post-bind process-cwd retargeting | Initial/default/relative workspace and `/new` constructed away from fixture path | Construct registry/App with a canonical fixture workspace distinct from process cwd, issue `/new`, and inspect command cwd; any process-cwd value falsifies C11. | Direct equality to the constructor fixture path, independent of command implementation. | `builtin.rs`: restore `std::env::current_dir()` in `NewCommand::execute`; `new_command_uses_bound_workspace` becomes red. | `commands::tests::new_command_uses_bound_workspace`; event-routing integration expectation. | <1 minute | PENDING — checkpointed-build, workspace slice gate |
| C12 | Real restart persistence/private ownership | Runtime start/teach/replace/shutdown/restart; lock and permissions | Use the real runtime binary and temp data root across restart; missing rows/audits, wrong versions, lock overlap, or weakened permissions falsifies C12. | Direct read-only SQLite queries plus filesystem metadata and second-process attempt, independent of client response. | `store.rs`: omit v2 migration on reopen or drop the lock before serving; `real_runtime_restart_preserves_lessons_and_audit` becomes red. | `crates/cyril/tests/memory_runtime.rs::real_runtime_restart_preserves_lessons_and_audit` plus existing lock/privacy fences. | 3 minutes | PENDING — checkpointed-build, end-to-end slice gate |

## Non-goals and future work

- No generic memory backend trait or selector in this change. Permanent non-goal for cyril-ezgo: local is the only approved implementation, and the versioned domain protocol is already the future seam.
- No configurable context budget, deadline, detector set, list limit, or lesson-length tuning. Permanent non-goal for M1: fixed bounded constants keep the interface small until production evidence requires configuration.
- No delete, restore, global promotion, or agent-authored mutation command. Permanent non-goal for this explicit-teaching slice: replacement preserves history; broader policy is not needed for the accepted behavior.
- Transcript-derived episodes, turn capture, and derived-memory injection are intended future work in verified issue `cyril-n3j7`; derived/document rows remain positively excluded from C9.
- Agent-facing capability-bound MCP recall and loaded-session project binding are intended future work in verified issue `cyril-3dqf`.
- Cited document knowledge and embedding/retrieval expansion are intended future work under verified epic `cyril-ct0y`; no content/job/knowledge schema lands in cyril-ezgo.
- The later proxy-stage replacement of the interim bridge adapter is intended future work under verified epic `cyril-ct0y` and ADR-0003; this change preserves an adapter-neutral behavior contract but does not build the proxy.

## Falsifier run log

- 2026-08-24 — C1 cheapest falsifier: `python .cyril-ezgo/probe.py main /home/dwalleck/repos/cyril linked /home/dwalleck/repos/cyril-wt-feat-cyril-ezgo nongit /tmp` compared with `git -C <workspace> rev-parse --path-format=absolute --git-common-dir` and `realpath`. **PASS**: both Git workspaces resolved to `/home/dwalleck/repos/cyril/.git`, display paths remained distinct, and `/tmp` resolved to itself. Full outputs: `evidence.md` § Comparisons.

## Approval

- Requester approval: "I approve this design"
- Date: 2026-08-24
- Approved risk acceptances: None; every claim has a deterministic regression fence.

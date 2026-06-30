# KAS-5a — fs host-callback responders + the host-io seam (cyril-7bdu)

Falsifiable design. Extends the prove-it (`.cyril-7bdu/PROVE-IT.md`, capture
`host_callbacks_2.10.0.json`) which confirmed the 2.10.0 host-callback wire is
unchanged from baseline. **Scope: the host-io mediation seam + `fs/read_text_file`
+ `fs/write_text_file`.** Terminal (`terminal/*`) is KAS-5b — see Negative space.

## Purpose

KAS delegates file I/O to the host via server→client ACP *requests* when the
client advertises `fs` capabilities. Today cyril advertises empty
`ClientCapabilities` (`V2Engine`), so KAS runs file I/O in-process and cyril never
sees it. KAS-5a makes `KasEngine` advertise `fs` caps and implements the
responders, so cyril becomes the executor — the audit/gate/transform point
(ADR-0003). This is the first `cyril-stages` interception seam.

## Architecture (grounded in the code, not aspiration)

- **Mechanism (verified):** `fs/read_text_file` / `fs/write_text_file` arrive as
  **typed `acp::Client` trait methods** (`read_text_file(ReadTextFileRequest)`,
  `write_text_file(WriteTextFileRequest)`), default body `Err(method_not_found())`
  in acp 0.10.2 (`client.rs:54,64`). `KiroClient` (`protocol/client.rs`) already
  overrides `request_permission`/`session_notification`/`ext_notification`/
  `ext_method`; KAS-5a adds the two fs overrides. NOT ext-routing (`_kiro/fs/*`
  never fired — prove-it).
- **Capability gate:** `Engine::client_capabilities()` (engine.rs:30) — `KasEngine`
  returns `ClientCapabilities::new().fs(FileSystemCapability{read_text_file:true,
  write_text_file:true})`; `V2Engine` stays `ClientCapabilities::new()` (empty).
  This is the on/off switch: v2 never receives fs requests, so v2 is byte-unchanged.
- **Mediation (ADR-0004):** each fs request routes **through the bridge loop's
  host-io arm** so a future stages gate can observe/transform it; the loop
  **forwards and never awaits resolution**. Resolution spawns **off-loop** and
  replies via the request's embedded `responder` oneshot (the `request_permission`
  pattern, client.rs:52). The acp connection already spawns each inbound request
  as its own task (`rpc.rs:272`), so concurrency exists — but the runtime is
  **single-threaded** (`current_thread`+`LocalSet`), so resolvers must use **async
  I/O** (`tokio::fs`), never thread-pinning `std::fs`.
- **Path boundary:** request paths/cwd are absolute (prove-it). They cross
  `platform::path` translation at the responder boundary (Linux no-op;
  Windows `C:\`↔`/mnt/c`, `platform/path.rs`).

## Input shapes (step 2)

`ReadTextFileRequest { session_id, path, line: Option<u32>, limit: Option<u32> }`,
`WriteTextFileRequest { session_id, path, content }`. Distinct production-reachable shapes:

- **path**: absolute (observed), relative (spec-reachable → resolve vs session cwd),
  nonexistent, unreadable/permission-denied, points-at-a-directory, Unicode/spaces.
- **line/limit**: both `None` (observed `line:0`), `line=Some(n>0)`, `limit=Some(m)`
  (spec-reachable; not yet observed — honor per ACP or it's a latent bug).
- **content** (write): empty string, normal text, multi-line/Unicode.
- **write target parent dir**: exists, missing (prove-it responder created it).
- **session_id**: always present; main session vs a subagent session (routing).
- **platform**: Linux (no-op), Windows/WSL (`C:\`↔`/mnt/c`) — the untested-on-dev shape.

Out of scope (one-line each): symlink-following policy (mirror OS default); huge-file
streaming (read returns whole content per ACP `ReadTextFileResponse{content:String}`);
non-UTF-8 files (ACP read is text — surfaces as an error, covered by C7).

## Removed-invariant sweep (step 2b)

**Core move is subtractive underneath.** "+fs capability" removes the invariant
**"cyril performs no mid-turn server→client work; the single bridge thread only ever
does fast forwarding."** Chain it guaranteed for free:

1. bridge thread never blocks mid-turn → 2. the rpc read loop (`rpc.rs:267`) and the
prompt task are never starved → 3. a user **cancel** issued mid-turn, and streamed
notifications, are always processed promptly.

After KAS-5a, fs resolvers do mid-turn I/O **on that single thread**. If a resolver
blocks it (sync `std::fs`, or — the real hazard inherited by KAS-5b — a sync
`std::process` wait) or the loop *awaits* the resolution inline (the ADR-0004
regression), links 2–3 break: a cancel can't be processed while a resolver runs. →
**Claim C4** (the property that must still hold). Invariants judged still-safe:
v2 path (advertises no fs caps → receives no fs requests → nothing changed; one-line
reason). Permission-overlay reentrancy is safe — fs write reuses the existing
`request_permission` path unchanged (→ C9).

## Claims

1. `KasEngine::client_capabilities()` advertises `fs.read_text_file` + `fs.write_text_file` = true; `V2Engine::client_capabilities()` stays empty.
2. **[cheapest]** KAS fs ops arrive as typed `acp::Client::{read_text_file, write_text_file}` calls (wire `fs/read_text_file`, `fs/write_text_file`); cyril overrides those trait methods, not `ext_method`/`_kiro/*` routing.
3. ~~Every fs request is forwarded through the bridge loop's host-io arm (observable seam) and the loop never awaits its resolution.~~ **DEFERRED to cyril-g9vt** (checkpointed-build): the loop seam is consumer-less (no stages gate yet) and forces an un-`#[cfg]`-able `run_loop` param; KAS-5a resolves fs directly in a `#[cfg(feature="kas")]` `KiroClient` override. C4 (non-blocking) still holds via the acp connection's per-request `spawn_local` (`rpc.rs:272`) — the loop arm was never load-bearing for C4.
4. A slow fs resolver does not starve the single-threaded bridge runtime: a concurrently-arriving cancel / notification / second fs request still makes progress (resolvers use async `tokio::fs`, never `std::fs` on the loop thread).
5. A request `path` crosses `platform::path` translation at the responder boundary before any filesystem access (Linux no-op; Windows `C:\`↔`/mnt/c`).
6. `read_text_file` returns the translated-path file's UTF-8 content; when the request carries `line`/`limit`, the returned content honors them (not the whole file).
7. `read_text_file` on a missing / unreadable / directory path returns `Err` (ACP error) — never a panic, never empty-content-as-success.
8. `write_text_file` writes the exact `content` (including empty string) to the translated path, creating missing parent directories, and returns success.
9. fs READ resolves with no permission prompt; fs WRITE flows through cyril's existing approval overlay unchanged (the KAS `_meta.kiro.consent` block is tolerated/ignored, not required to parse).
10. fs paths are absolute per ACP (`ReadTextFileRequest` doc: "Absolute path"; capture confirms); a non-absolute path is **rejected with an error**, never silently read against cyril's process cwd. (Refined from "resolve against session cwd" during budgeted-plan: ACP guarantees absolute, so session-cwd threading is unjustified state for an input that never occurs; fail-loud is safer than silently-wrong-file.)

## Falsification

| # | Claim | Falsifier | Oracle (independent) | Cost | Status | Regression fence |
|---|-------|-----------|----------------------|------|--------|------------------|
| 1 | KasEngine advertises fs caps; V2 empty | Call each engine's `client_capabilities()`; if KAS lacks fs or V2 non-empty → false | direct struct assertion | 5m | **passed** (design: builder methods exist, schema 0.11.2 `client.rs:1611-1639`) | unit `engine::kas_advertises_fs_v2_empty` |
| 2 | fs via typed Client methods, not ext | Inspect `host_callbacks_2.10.0.json` method strings + acp `Client` trait; if fs arrived as `_kiro/fs/*`/ext OR crate lacks `read_text_file`/`write_text_file` trait fns → false | the empirical capture + the pinned crate's trait def (both outside cyril SUT) | 2m | **passed** (capture shows `fs/read_text_file`/`fs/write_text_file`; acp 0.10.2 `client.rs:54,64` exposes both) | integration `kas_read_reaches_kiroclient_override` (=C6 fence) |
| 3 | fs request visits loop host-io arm | Install a counting observer at the loop seam; issue N fs requests; if observer count ≠ N → false | test-double observer at the seam, independent of the resolver | 1h | pending | integration `host_io_routes_through_loop` |
| 4 | slow resolver doesn't starve loop | (architectural under direct-resolve — see note) | acp per-request `spawn_local` (`rpc.rs:272`) + `tokio::fs` blocking-pool offload (both upstream-tested) | n/a | **satisfied (architectural)** | **none — noted absence.** A faithful deterministic fence needs a no-writer-FIFO + `current_thread` harness that tests *tokio's* offload, not cyril's logic. cyril's only obligation — "use `tokio::fs`, never `std::fs`/`std::process`" — is documented in the `host_io` module doc and a review item. |
| 5 | path translated at boundary | Feed a Windows `C:\x` path through the responder's translation step on a win-target build; if it hits the FS untranslated → false. Linux: a `/mnt/c/...` path is a no-op | `platform::path` unit oracle (pure fn, separate from fs) | 20m | pending | unit `host_io_translates_path_both_platforms` |
| 6 | read returns content, honors line/limit | Write a 5-line file; `read_text_file{line:2,limit:1}`; if result ≠ the single expected line (returns whole file) → false | fixture file written by the test; expected slice computed in-test | 30m | pending | unit `read_text_file_returns_content_and_honors_line_limit` |
| 7 | read errors, no panic/empty-as-ok | `read_text_file` on a path that doesn't exist; if it returns `Ok("")` or panics the bridge → false | the test asserts `Err`; bridge stays alive | 20m | pending | unit `read_text_file_missing_path_errors` |
| 8 | write exact content + mkdir -p | `write_text_file{path:"sub/dir/f.txt", content:""}` into a fresh tmp; if `sub/dir` not created, file absent, or content ≠ `""` → false | read the file back with `std::fs` in-test (not the SUT path) | 30m | pending | unit `write_text_file_creates_parents_exact_content` |
| 9 | read no-prompt; write via approval | Run a KAS turn that reads then writes; if read raises a permission request, or write bypasses the approval overlay → false | the permission channel's observed events (the prove-it showed write→`request_permission`, read→none) | 1.5h (live) | pending (measurement) | integration `kas_fs_write_requires_approval_read_does_not` |
| 10 | non-absolute path rejected, not process-cwd-read | Place `rel.txt` under cyril's process cwd; `read_text_file{path:"rel.txt"}`; if it returns that file's content (resolved against process cwd) instead of `Err` → false | a file placed only under the process cwd; the test asserts `Err` | 25m | pending | unit `fs_relative_path_rejected_not_process_cwd` |

Cheapest (C2) is **passed** — design may proceed to planning per the gate.

### Non-vacuity (named buggy impls)
- C1: a `KasEngine` copy-pasting `V2Engine`'s empty caps → fails.
- C2: a design routing fs through `ext_method` (string-match `_kiro/fs/*`) → falsified by the capture's bare `fs/*` strings.
- C3: a `KiroClient::read_text_file` that resolves inline without sending to the loop → observer counts 0.
- C4: the ADR-0004 regression — loop `select!` arm `.await`s the resolver, or a resolver using `std::fs::read_to_string`/`std::process` → B serialized behind A.
- C5: responder calling `tokio::fs::read(req.path)` with the raw wire path on Windows → reads `C:\…` literally, no `/mnt/c` translation.
- C6: responder ignoring `line`/`limit` → returns whole file; line/limit test fails.
- C7: `read.unwrap_or_default()` / `.ok().unwrap_or_default()` → returns `Ok("")` for a missing file.
- C8: responder using `write` without `create_dir_all` → missing-parent write errors; or a `if !content.is_empty()` guard → empty content no-ops.
- C9: advertising fs caps but routing write around the existing approval path → write executes unapproved.
- C10: `tokio::fs::read(req.path)` with no `is_absolute()` guard → a relative path resolves against the bridge process cwd and reads the wrong file instead of erroring.

### Per-claim distinctness
Each claim has its own oracle output (distinct unit/integration test names above); a
failure localizes to exactly one claim. C2 and C6 share a fence test only as a
*regression* sentinel (C2 is design-time empirical; C6 is the runtime override proof).

## Negative space (what KAS-5a deliberately does NOT do)

1. **Terminal host-callbacks** (`terminal/{create,output,wait_for_exit,release,kill}`, `_kiro/terminal/shell_type`) — KAS-5b, tracked at **cyril-ufie**. The seam (C1/C3/C4/C5) is built here and reused there.
2. **`_kiro/fs/*` Kiro-extras** (`read_directory`/`stat`/`delete`) — not implemented. Prove-it 2.10.0 shows KAS routes directory-list and delete through the *shell*, never these methods; the existing unknown-method `debug!` arm surfaces them if a future capture ever shows them firing.
3. **Stages gating/transform of host-io** — the loop seam only *forwards*; no policy/audit/transform consumer exists yet (ADR-0003 `cyril-stages`, later phase). The seam exists; the gate is pass-through.
4. **No cyril-imposed fs policy** — no path allowlisting, sandboxing, or read-permission policy of cyril's own; cyril mirrors KAS's behavior (read auto-allowed, write prompts).
5. **No live Windows/WSL validation in 5a** — C5 is unit-tested both directions, but a live Windows KAS run is not part of this slice (Linux dev box).

## Tracker references
- **cyril-7bdu** (this issue) — scoped to KAS-5a (fs) for the first PR.
- **cyril-ufie** — KAS-5b terminal host-callbacks (filed from this design; `blocks` edge: cyril-7bdu's seam must land first).

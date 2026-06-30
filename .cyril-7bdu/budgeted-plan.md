# KAS-5a — budgeted plan (cyril-7bdu)

From `.cyril-7bdu/falsifiable-design.md` (gate-passed). 8 slices, each ≤2 files /
≤~50 lines / ≤30 min. Claim coverage: C1→S1, C5read+C6+C7→S2, C5write+C8→S3,
C10→S4, C3→S5, C4→S5+S7, C2→S6, C9→S8.

Grounding (verified): acp 0.10.2 `Client` trait has default-`method_not_found`
`read_text_file`/`write_text_file` (override these); `ReadTextFileRequest{session_id,
path:PathBuf, line:Option<u32> 1-based, limit:Option<u32>}` → `ReadTextFileResponse
{content:String}`; `WriteTextFileRequest{session_id, path, content}` → `{}`.
`ClientCapabilities::new().fs(FileSystemCapability{read_text_file,write_text_file})`
(schema `client.rs:1611-1639`). Bridge: `KiroClient::new(inbound_tx, req_tx, engine)`
(`bridge.rs:380`); `run_loop` `select!` already has a permission `req_rx` arm
(`bridge.rs:430,478`) — host-io mirrors it. Path: `platform::path::to_native(&Path)`
(agent→native; Linux no-op, Windows `/mnt/c`→`C:\`).

---

## Slice 1: KasEngine advertises fs caps; V2 stays empty

**Claim:** C1.
**Oracle:** direct assertion on the returned `ClientCapabilities` struct (independent of any wire).
**Stress fixture:** assert `V2Engine.client_capabilities()` is byte-equal to `ClientCapabilities::new()` (empty) — designed to fail if a copy-paste adds fs caps to V2 (the parity-break bug). Plus assert KAS's `fs.read_text_file && fs.write_text_file` are true.
**Loop budget:** none (no loop; constant struct build).
**Files:** `crates/cyril-core/src/protocol/engine.rs`.

**Code (advisory):**
```rust
// impl Engine for KasEngine
fn client_capabilities(&self) -> acp::ClientCapabilities {
    acp::ClientCapabilities {
        fs: acp::FileSystemCapability { read_text_file: true, write_text_file: true, meta: None },
        terminal: false, // KAS-5b (cyril-ufie)
        meta: None,
    }
}
```

**Verification:**
- [ ] Unit `kas_advertises_fs_v2_empty` passes (KAS fs=true,true; V2 == empty).
- [ ] `cargo test -p cyril-core` + `clippy -D warnings` green.
- [ ] Oracle: V2 path unchanged (existing `engine.rs:136` parity test still passes).

---

## Slice 2: fs read resolver — content + line/limit + error

**Claim:** C5(read), C6, C7.
**Oracle:** the test writes a known file with `std::fs` and computes the expected slice itself (independent of the resolver's own reading path).
**Stress fixture:**
  (a) 5-line file `"l1\nl2\nl3\nl4\nl5\n"`, request `line=Some(2), limit=Some(1)` → expect exactly `"l2\n"` (1-based line 2). Fails if the resolver ignores line/limit and returns the whole file.
  (b) request on a path that does not exist → expect `Err`. Fails under `read_to_string(..).unwrap_or_default()` returning `Ok("")`.
**Loop budget:** line/limit slicing iterates lines: `O(L)`, L = lines in file. Production scale L ≲ 10^5 (a large source file) → well under 10^6; one `read` syscall. In budget.
**Files:** `crates/cyril-core/src/protocol/kas/host_io.rs` (new), `crates/cyril-core/src/protocol/kas/mod.rs` (add `mod host_io;`).

**Doc-comment-as-contract:** the "missing/unreadable path → Err" contract is **load-bearing for correctness** (silent `Ok("")` would be wrong output) → a runtime `match` on the `io::Error` returning `acp::Error`, surviving release builds. NOT `debug_assert!`.

**Code (advisory):**
```rust
pub(crate) async fn read_text_file(req: &acp::ReadTextFileRequest)
    -> acp::Result<acp::ReadTextFileResponse> {
    let path = crate::platform::path::to_native(&req.path);              // C5
    let text = tokio::fs::read_to_string(&path).await
        .map_err(|e| acp::Error::into_internal_error(/* io */ e))?;       // C7 runtime check
    let content = slice_lines(&text, req.line, req.limit);                // C6 (1-based)
    Ok(acp::ReadTextFileResponse { content, meta: None })
}
// slice_lines: if line/limit None → whole text; else lines().skip(line-1).take(limit).
```

**Verification:**
- [ ] Unit (a) line/limit slice == `"l2\n"`; (b) missing path → `Err`.
- [ ] Loop budget holds at L=10^5 fixture (optional large-file timing check).
- [ ] prove-it oracle: capture's `fs/read_text_file{line:0}` → whole-file path still returns full content.

---

## Slice 3: fs write resolver — exact content + mkdir -p

**Claim:** C5(write), C8.
**Oracle:** the test reads the written file back with `std::fs` (not via the resolver).
**Stress fixture:** `write_text_file{path: <tmp>/a/b/c.txt, content: ""}` into a fresh tempdir where `a/b` does NOT exist → expect `a/b/c.txt` created and its content exactly `""`. Fails under (i) missing `create_dir_all` (write errors on absent parent), (ii) an `if !content.is_empty()` guard (empty content no-ops, file absent).
**Loop budget:** `create_dir_all` is `O(path depth)`, depth ≲ 30; one `write` syscall. In budget (no unbounded loop).
**Files:** `crates/cyril-core/src/protocol/kas/host_io.rs`.

**Code (advisory):**
```rust
pub(crate) async fn write_text_file(req: &acp::WriteTextFileRequest)
    -> acp::Result<acp::WriteTextFileResponse> {
    let path = crate::platform::path::to_native(&req.path);              // C5
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(to_acp_err)?;     // C8 mkdir -p
    }
    tokio::fs::write(&path, &req.content).await.map_err(to_acp_err)?;     // exact content incl ""
    Ok(acp::WriteTextFileResponse { meta: None })
}
```

**Verification:**
- [ ] Unit: empty-content + missing-parent fixture → file exists, content `""`.
- [ ] Non-empty + Unicode content round-trips byte-exact.
- [ ] `clippy -D warnings` (no `unwrap`, every `Result` handled).

---

## Slice 4: non-absolute path rejected (not process-cwd read)

**Claim:** C10.
**Oracle:** the test asserts `Err` for a relative path; places a same-named file under the process cwd and asserts it is NOT returned.
**Stress fixture:** create `rel.txt` in the bridge process cwd; call read/write with `path = "rel.txt"` (relative) → expect `Err`, and `rel.txt`'s content must NOT be returned. Fails under `tokio::fs::read(req.path)` with no absolute-guard (resolves against process cwd → reads the wrong file).
**Loop budget:** none (one `is_absolute()` check).
**Files:** `crates/cyril-core/src/protocol/kas/host_io.rs`.

**Doc-comment-as-contract:** "path must be absolute" is **load-bearing for correctness** (a relative path silently reads/writes the wrong file) → runtime guard `if !req.path.is_absolute() { return Err(...) }` in BOTH resolvers, surviving release. NOT `debug_assert!`.

**Code (advisory):**
```rust
fn require_absolute(p: &std::path::Path) -> acp::Result<()> {
    if p.is_absolute() { Ok(()) }
    else { Err(acp::Error::invalid_params() /* "path must be absolute" */) }
}
// call require_absolute(&req.path)? at the top of both read_text_file and write_text_file,
// BEFORE to_native (a /mnt/c path is absolute; a bare "rel.txt" is not).
```

**Verification:**
- [ ] Unit: relative path → `Err` for both read and write; the process-cwd file is untouched/unreturned.
- [ ] Absolute path still works (regression: slices 2/3 fixtures still pass).

---

## Slice 5: host-io channel + loop seam (forward, spawn off-loop, never await)

**Claim:** C3 (seam); enables C4 (non-blocking).
**Oracle:** a counting observer installed at the loop's host-io arm (independent of the resolver); counts requests that traversed the seam.
**Stress fixture:** push N=3 `HostIoRequest`s into the channel; assert the loop arm observed all 3 and each resolver's response reached its oneshot. Fails if a request bypasses the loop (observer < 3) or if the arm `.await`s the resolver inline (would serialize; checked harder in S7).
**Loop budget:** the `select!` arm is `O(1)` per request (build + `spawn_local`); no iteration. The loop itself is the existing event loop (unchanged asymptotics). In budget.
**Files:** `crates/cyril-core/src/protocol/kas/host_io.rs` (`HostIoRequest` enum), `crates/cyril-core/src/protocol/bridge.rs` (channel + `select!` arm).
**Boundary note:** `HostIoRequest` lives in `protocol/` (not `types/event.rs`) and carries acp types directly. Unlike `PermissionRequest` — which is in the deliberately acp-free `types/event.rs` because it crosses to the App/UI — `HostIoRequest` is resolved entirely inside `protocol/` (the resolver is `kas::host_io`), so it never reaches the App and keeps `types/event.rs` acp-free.

**Doc-comment-as-contract:** ADR-0004 "the loop forwards and never awaits resolution" is **load-bearing** — capture it at the arm with a comment AND structurally (the arm only `spawn_local`s, no `.await` on the resolver). The non-await is enforced by S7's concurrency test, not an assert.

**Code (advisory):**
```rust
// protocol/kas/host_io.rs (protocol-internal; carries acp types)
pub(crate) enum HostIoRequest {
    ReadText { req: acp::ReadTextFileRequest, responder: oneshot::Sender<acp::Result<acp::ReadTextFileResponse>> },
    WriteText { req: acp::WriteTextFileRequest, responder: oneshot::Sender<acp::Result<acp::WriteTextFileResponse>> },
}
// bridge.rs run_loop select! arm:
host_io = host_io_rx.recv() => match host_io {
    Some(HostIoRequest::ReadText { req, responder }) => {
        tokio::task::spawn_local(async move { let _ = responder.send(kas::host_io::read_text_file(&req).await); });
    }
    Some(HostIoRequest::WriteText { req, responder }) => {
        tokio::task::spawn_local(async move { let _ = responder.send(kas::host_io::write_text_file(&req).await); });
    }
    None => { /* channel closed */ }
}
```

**Verification:**
- [ ] Unit/integration: 3 requests → observer sees 3, each responder resolves.
- [ ] The arm body contains no `.await` on the resolver (only `spawn_local`).
- [ ] v2 path unaffected (host_io_rx simply never receives under V2 — empty caps).

---

## Slice 6: KiroClient fs overrides route through the seam

**Claim:** C2.
**Oracle:** an end-to-end test driving acp `Client::read_text_file` on a `KiroClient` wired to a real host-io channel + resolver; asserts content returned (independent of the resolver internals — it checks the override is reached and the round-trip closes).
**Stress fixture:** call `client.read_text_file(req)` for an existing file → expect its content. Fails if the override isn't implemented (default `Err(method_not_found())`) or the oneshot round-trip deadlocks.
**Loop budget:** none (one channel send + one oneshot await per call).
**Files:** `crates/cyril-core/src/protocol/client.rs` (two overrides + `host_io_tx` field), `crates/cyril-core/src/protocol/bridge.rs` (channel creation + pass `host_io_tx` to `KiroClient::new`).

**Doc-comment-as-contract:** none new (the overrides await a oneshot; a dropped sender → `Err`, handled via `recv().await.map_err`).

**Code (advisory):**
```rust
// client.rs — impl acp::Client for KiroClient
async fn read_text_file(&self, args: acp::ReadTextFileRequest) -> acp::Result<acp::ReadTextFileResponse> {
    let (tx, rx) = oneshot::channel();
    self.host_io_tx.send(HostIoRequest::ReadText { req: args, responder: tx }).await
        .map_err(|_| acp::Error::internal_error())?;
    rx.await.map_err(|_| acp::Error::internal_error())?
}
// write_text_file symmetric. Add `host_io_tx: mpsc::Sender<HostIoRequest>` to KiroClient + new().
```

**Verification:**
- [ ] Integration `kas_read_reaches_kiroclient_override`: read returns content end-to-end.
- [ ] write override returns success end-to-end (file on disk).
- [ ] prove-it oracle still agrees: a live KAS turn that reads a file renders identically.

---

## Slice 7: slow resolver does not starve the loop (non-blocking)

**Claim:** C4 (the removed-invariant claim).
**Oracle:** ordering of two oneshot resolutions observed by the test — independent of the resolvers.
**Stress fixture:** request A reads a **FIFO/named pipe with no writer** (blocks deterministically); request B reads a normal small file. Send A then B. Expected: B's response resolves while A is still pending. Fails if the loop arm awaits A inline (B serialized behind A) or a resolver uses thread-pinning `std::fs` (single-thread starves). Cleanup: unblock/drop A's FIFO at test end.
**Loop budget:** the test itself; no production loop added.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (test module only — `#[cfg(test)]`).

**Verification:**
- [ ] Integration `host_io_slow_request_does_not_block_loop`: B resolves before A.
- [ ] Same test confirms a concurrently-sent `BridgeCommand::Cancel` is still processed while A blocks (cancel target reachable mid-host-io).
- [ ] No `std::fs`/`std::process` in resolvers (grep gate in review).

---

## Slice 8: permission interaction (read no-prompt, write via approval)

**Claim:** C9.
**Oracle:** the permission channel's observed events during a live KAS turn (the prove-it already showed write→`session/request_permission` with `_meta.kiro.consent`, read→none).
**Stress fixture:** a KAS turn that reads a file then writes one. Expected: zero permission requests attributable to the read; the write's KAS-sent `session/request_permission` flows through the existing approval overlay and the write proceeds on approve. Fails if read raises a prompt, or the write executes without traversing the approval path. (No new production code — KAS sends the consent request separately; `KiroClient::request_permission` already handles it. This slice is verification + tolerating the `_meta.kiro.consent` block, which cyril ignores.)
**Loop budget:** none.
**Files:** `crates/cyril/tests/` (integration test only) — no production change expected; if `_meta.kiro.consent` parsing breaks the existing path, fix in `convert/` (≤1 file).

**Verification:**
- [ ] Integration: read → 0 permission events; write → 1 permission event handled by the existing overlay path.
- [ ] The KAS `_meta.kiro.consent` block does not break `request_permission` conversion (tolerated/ignored).

---

## Plan Self-Review

**1. Every loop — complexity + budget:**
- S2 `slice_lines` line iteration: `O(L)`, L ≲ 10^5 lines, < 10^6. ✓
- S3 `create_dir_all`: `O(depth)`, depth ≲ 30. ✓
- S5 loop arm: `O(1)`/request (spawn, no iteration). ✓
- No other new loops. No always-on phase → no wall budget needed (all per-request, event-driven). ✓

**2. Every fixture — bug class it fails under (not happy-path):**
- S1: copy-paste adds caps to V2 (parity break). ✓
- S2: ignores line/limit (returns whole file); `unwrap_or_default` masks missing file as `Ok("")`. ✓
- S3: missing `create_dir_all`; `if !is_empty` empty-content no-op. ✓
- S4: no `is_absolute` guard → silent process-cwd read of wrong file. ✓
- S5: request bypasses loop seam (observer < N). ✓
- S6: override missing (default `method_not_found`) / oneshot deadlock. ✓
- S7: loop awaits inline / `std::fs` thread-pin → serialization. ✓
- S8: read raises a prompt / write bypasses approval. ✓

**3. Every doc-comment precondition — classification + enforcement:**
- S2 "missing path → Err": load-bearing-correctness → runtime `match`/`map_err` (not debug_assert). ✓
- S4 "path must be absolute": load-bearing-correctness → runtime `is_absolute()` guard returning `Err` (not debug_assert). ✓
- S5 "loop never awaits resolution": load-bearing → structural (arm only spawns) + S7 test. ✓
- No sanity-hint-only preconditions in this slice set.

**4. Every write target — data vs diagnostic:**
- Resolvers return values (no stdout/stderr writes). ✓
- Error paths use `tracing` (diagnostic → stderr), consistent with the codebase. ✓
- No `println!` added. ✓

**5. Every tracker reference — resolves to existing issue covering the work:**
- `cyril-7bdu` (this issue, KAS-5a) — exists, in_progress. ✓
- `cyril-ufie` (KAS-5b terminal, Negative space) — filed + verified, `blocks` cyril-7bdu. ✓
- Design C10 refinement (reject relative) — settled in S4, not a deferral; no tracker needed (the `Err` surfaces an off-spec relative path if one ever arrives). ✓
- `_kiro/fs/*` not-built — settled rationale (Negative space #2); the existing unknown-method `debug!` arm surfaces it; no tracker. ✓

No gaps.

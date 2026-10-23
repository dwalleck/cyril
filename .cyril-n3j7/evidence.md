# Evidence: cyril-n3j7

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | The workspace's exact `rusqlite 0.39` configuration (`default-features = false`, `features = ["bundled"]`) provides operational FTS5. | Can a binary built with that exact dependency report `ENABLE_FTS5`, create an FTS5 virtual table, and execute a MATCH query? | PASS |
| P2 | FTS5 can provide the required literal-token, project-filtered, deterministic lexical floor. | Does a parameterized quoted-token MATCH over production-shaped rows return only the requested project and order equal-ranked rows by completion recency then stable ID? | PASS |
| P3 | The bridge exposes normalized lifecycle and authoritative turn ownership before App/UI projection. | N/A — current repository evidence covers this existing-system behavior: `TurnMediator` owner-stamps accepted terminal events and absorbs duplicate/stale companions; `RoutedNotification` preserves session/turn identity; bridge fake-agent tests fence mediation. | N/A — covered by current applicable repository evidence |
| P4 | Result counts, episode character budgets, framing, and completion eligibility. | N/A — these are behavior already decided by the issue Design and acceptance criteria, not claims about an existing system. | N/A — specification behavior |

## Data

- Source: production-shaped generated fixture.
- Shape: four completed source-turn search rows across two canonical project IDs, including two equal-content/equal-rank same-project rows with different completion times, one higher-recency foreign-project row, and one same-project non-match. Indexed fields match the planned prompt/assistant/tool lexical surface; unindexed fields match project, stable turn identity, and completion-time filters/tie-breakers.
- Safety: both probe and oracle use independent in-memory SQLite databases. They read the repository's declared dependency configuration but do not open, write, mutate, or delete repository/runtime databases or operator state.

## Probe

- File: `probe.rs`; supporting exact dependency manifest: `Cargo.toml` and generated `Cargo.lock` in this directory.
- Mechanism: a standalone Rust binary builds against `rusqlite 0.39` with the workspace's exact bundled feature configuration, asks SQLite for `ENABLE_FTS5`, creates/populates the production-shaped FTS table, and runs the parameterized project-filtered quoted-token query.
- Run: `CARGO_TARGET_DIR=/tmp/cyril-n3j7-probe-target cargo run --quiet --manifest-path .cyril-n3j7/Cargo.toml --bin fts5-probe`
- Output:

  ```text
  fts5_enabled=1
  rows=turn-new,turn-old
  ```

## Oracle

- P1 mechanism: inspect the independently generated debug binary's static symbol table. Presence of the linked local `sqlite3Fts5Init` initializer has a different failure mechanism from executing SQL through rusqlite and from the future production query implementation.
- P1 run: `nm --defined-only /tmp/cyril-n3j7-probe-target/debug/fts5-probe`; symbol-table row observed: `00000000001c0298 t sqlite3Fts5Init`.
- P2 mechanism: `oracle.py` uses Python's separately built standard-library SQLite binding and an independently written fixture/query, then compares the returned IDs to the hand-derived expectation: the foreign project and non-match are absent; identical same-project matches tie on BM25 and therefore order `turn-new` before `turn-old` by completion time. This differs from the Rust/rusqlite probe and future Rust storage implementation.
- P2 run: `python .cyril-n3j7/oracle.py`
- P2 output:

  ```text
  rows=turn-new,turn-old
  ```

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | `fts5_enabled=1`; FTS5 table creation and MATCH query succeeded. | Linked binary defines local `sqlite3Fts5Init`. | PASS |
| P2 | `rows=turn-new,turn-old` | Python SQLite and hand expectation both produce `rows=turn-new,turn-old`. | PASS |

## Validated / learned

- P1: Validated prior understanding — the exact workspace bundled-rusqlite configuration compiles, links, and executes FTS5 rather than merely declaring a feature flag.
- P2: Validated prior understanding — parameterized quoted-token MATCH supports strict project filtering, and explicit completion-time/stable-ID tie-breakers make equal-rank results deterministic.

## Related issues

- Consulted through one bounded native-tracker listing/search: `cyril-ct0y` (parent contract; FTS5 lexical floor), `cyril-n3j7` (this capture/episode slice), `cyril-3dqf` and `cyril-s7gn` (later scoped MCP/fact recall consumers), `cyril-39xn` (later knowledge FTS5 consumer), `cyril-nxq5` (later completed-turn consolidation), `cyril-y91y` (later vector/FTS fusion), and closed predecessor `cyril-ezgo` (lesson context and project binding).
- Filed: none — both premises passed; no substrate defect or uncovered future work was found.

## C9 forwarder measurement-boundary repair (2026-10-22)

### Failure and governing obligation

- **Original CI result: FAIL, not classified as flaky.** PR #190 at `af78b161`, [Default Features (ubuntu-latest), attempt 1](https://github.com/dwalleck/cyril/actions/runs/39806350722/job/119920492208), failed `capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown` with `C9 forwarder batch exceeded budget: 153.901529ms`. The retained job excerpts report 417 passed, one failed, one skipped, and exit code 100.
- The old stopwatch began before constructing/enqueueing the fixture and ended after the entire forwarder drain. It therefore measured producer-through-drain wall time, without identifying any actual batch or its event count. That failure does not establish the duration of an individual batch. It also does not prove successful persistence or graceful runtime shutdown: the failing assertion preceded both checks.
- The unchanged obligation is `plan.md`, Slice 2: asynchronous forwarder batch **≤100 ms**, with the IPC-inclusive scope recorded in `checkpoints.md`, Slice 2. The separate shutdown drain remains hard-capped at **two seconds**. `design.md` retains the dedicated bounded, sequential forwarder, a fresh client per batch, and bridge completion → capture drain → memory shutdown. No approved risk acceptance applies.
- Historical checkpoint results retain their original source-state meaning; they do not establish a PASS for this repaired measurement.

### Delivered repair and source provenance

- The permanent source repair changes only `crates/cyril/src/capture_forwarder.rs`; this evidence record and the byte-exact text receipts below document it. Before editing, `cmp` confirmed that the worker file matched the reviewed EJ worktree byte-for-byte. Both files had SHA-256 `b80f4de46766295ddd06940cae80abed68eef0e12494a969ba1ac4f70dccaebf`.
- `BatchObservation { event_count, elapsed }`, the optional observation sender, its ordinary-spawn `None` argument, the timer, event count, and observation send are all gated with separate `#[cfg(test)]` and `#[cfg(unix)]` attributes. Non-test builds acquire no observation channel, allocation, timing, or counting work; Windows tests acquire no Unix-only observation support.
- The worker starts the clock immediately after obtaining its first event, before identity lookup, sizing, batch gathering, conversion, or validation. It stops immediately after `ProjectMemory::capture_batch(...).await` returns, including runtime checks, fresh connection creation, request transmission, real persistence, and awaited response. Observation delivery happens after stopping the clock. Scheduling delays inside that asynchronous interval are not subtracted.
- The existing real-runtime fixture now prequeues and closes all three contiguous source events before spawning the worker. It still drains, inspects the exact persisted prompt and `Finished(Completed)` status, then gracefully shuts the runtime down. Only afterward does it require one observation containing three events, a disconnected observation channel with no second batch, and elapsed time ≤100 ms. A failed capture attempt is observed too; conversion rejection and production capture-failure handling are unchanged.
- Queue/event/byte limits, connection lifetime, sequential batching, conversion behavior, two-second drain policy, App shutdown ordering, and all other regression tests are intentionally unchanged. No runtime API, backend substitute, timeout relaxation, retry, averaging, or synthetic pass was added.

### Scoped qualification — PASS

Main's authorized sole-HOST checkpoint completed on 2026-10-22 at worktree HEAD `03838a501dad8be140acd78dc3fff4cae35fafef` with the uncommitted repair. Its [canonical receipt](evidence/capture-batch-oracle-20261022/qualification.json) has SHA-256 `b635a74e268d40c5ad81de350ed0306783cc33c6c08c35bc4c6807601e4b4f98`; external original paths and source identities are preserved byte-for-byte.

| Initial gate | Actual command | Result |
|---|---|---|
| Formatting check only | `cargo fmt --all -- --check` | PASS |
| Exact C9, defaults | `cargo test -p cyril --bin cyril capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown -- --exact --nocapture` | PASS |
| Exact C9, all features | `cargo test -p cyril --bin cyril --all-features capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown -- --exact --nocapture` | PASS |
| Exact C9, no defaults | `cargo test -p cyril --bin cyril --no-default-features capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown -- --exact --nocapture` | PASS |
| Shutdown ordering | `cargo test -p cyril --bin cyril c10_shutdown_waits_bridge_then_drains_capture_then_stops_memory -- --nocapture` | PASS |
| Strict App lint, all targets/features | `cargo clippy -p cyril --all-targets --all-features -- -D warnings` | PASS |

The ten original command logs, named `01-initial-fmtCHECK.log` through `10-inside-IPC150ms-exact-restoration-GREEN.log`, are retained beside the canonical receipt. There were **ten commands: nine exit 0, one intended assertion-specific exit 101, zero unexpected failures**. The eight libtest invocations overlap feature variants, controls, and restorations: seven passed case invocations and one intended failed case invocation, covering two unique case names with no ignored case invocations. These are not ten distinct tests.

The [independent source review](evidence/capture-batch-oracle-20261022/source-review.md), SHA-256 `4ab74573168afd45ce9ae3e378a6508db3263d68f24326fd0455b5751dbe18c4`, reported no findings and confidence 0.96. It reviewed the repaired source and then-pending evidence statically; it did not run or claim the subsequent qualification.

Source-bound identity and applicability:

- The [actual 498-input manifest](evidence/capture-batch-oracle-20261022/actual-source-inputs.json), SHA-256 `fbeb14cd8cbcde4d4daa79bfa500711bd4dbfc4b75c7b00354a90467d1a58f02`, records the actual Cargo/config/toolchain/crate inputs, not a Git tree or final commit. Its descriptor is `1ddc92faa9079f4f59cf597d301c9646fd4bec9c14c25659b18d10f1462c1e14`.
- Repaired worker SHA-256: `4fb824a80afdbf4cba3174e5b3fd55d625562d84a1d2357455c39cd9ee73fefc`. Unchanged memory runtime SHA-256: `f26507aaee55f6e9c7351c5e577d0285a0e3ab60b699d30bd1ba6858ed6188de`.
- Both throwaway scenarios below were restored from immediate byte backups, followed by fresh default-feature C9 GREEN. All 498 input bytes/modes returned to their initial state; the initial formatting and strict Clippy results remain applicable. No formatter application or permanent source repair was performed by the qualifier.
- The receipt's evidence-document hash `72913c86625c0f2b1f19a9ff3e03f13dffbf52d5716006cb50593d56f3f0bd07` identifies the earlier pending document, not this results amendment. This document and the published text receipts are outside the manifest's Cargo input scope.
- Unmutated C9 runs did not print numeric batch elapsed time: their evidence is the executed ≤100 ms assertion. Libtest total-wall summaries are not batch measurements. The controlled figures below are not a speedup comparison with the original 153.901529 ms producer-through-drain failure; no injected delay is subtracted to invent a baseline.
- This is scoped Linux qualification only. It does not establish full-workspace or assembled X/EJ/NK qualification, a production App binary or native/live-auth consumer run, Windows behavior, or final CI. Original CI remains **FAIL, not flaky**; Main owns publication/porting and any further required qualification. No Git/PR/tracker/auth operations are implied.

### Discrimination and exact restoration — PASS

| Controlled scenario | Observed result | Restoration |
|---|---|---|
| Test-local 150 ms sleep before worker entry; prequeued/closed events and unchanged permanent assertions | [Command 07](evidence/capture-batch-oracle-20261022/07-external150ms-before-worker-excluded-PASS.log): PASS, outer elapsed **155.679395 ms**, actual three-event batch **4.427638 ms**. Persistence, completed status, runtime shutdown, single-batch and ≤100 ms checks passed. | Worker restored byte-exactly; [command 08](evidence/capture-batch-oracle-20261022/08-external150ms-exact-restoration-GREEN.log) fresh C9 GREEN. |
| Real IPC: 150 ms sleep after `execute_request(...)`, before sending only `MemoryResponse::Captured`; no dependency `cfg(test)` and no startup/inspection/shutdown response delay | [Command 09](evidence/capture-batch-oracle-20261022/09-inside-real-IPC150ms-specific100ms-fence-RED.log): intended exit 101 at the unchanged actual-batch 100 ms assertion, **155.291782 ms**, event count three. Exact persisted prompt, `Finished(Completed)`, runtime shutdown, and one-batch/disconnected-channel assertions passed before this timing panic. It was not a compile/setup failure or two-second drain timeout. | Runtime restored byte-exactly; [command 10](evidence/capture-batch-oracle-20261022/10-inside-IPC150ms-exact-restoration-GREEN.log) fresh C9 GREEN. |

Both controlled edits were temporary and are absent from the delivered source. Their intended outcomes establish exclusion of pre-worker delay and inclusion of awaited real IPC delay without changing the 100 ms budget or two-second drain policy.

## C9 ordinary/performance split — cyril-xy4x (2026-10-23)

### Current approved verification policy

The requester-approved separation is recorded once in [cyril-xy4x's Local route](../.cyril-xy4x/route.md#selected-route-and-approval). Ordinary C9 keeps the shared real-runtime fixture's contiguous three-event batching, exact persisted prompt, `Finished(Completed)`, completed drain, graceful shutdown, and one-batch/disconnected-channel assertions without a tight elapsed ceiling. Existing C10 lifecycle ordering remains covered separately.

The requester subsequently clarified: **timing budgets alone must not hold up merging fixes unless they reveal true, lengthy delays**. A 163 ms batch is not such a delay. The inherited **100 ms IPC-inclusive reference is advisory, not a required pre-merge checkpoint**; this supersedes the earlier mandatory-checkpoint policy retained in historical receipts. No replacement numeric merge threshold is introduced. Functional failures, genuine stalls and failed cleanup remain blockers; existing correctness watchdogs are unchanged.

The diagnostic clock still starts immediately after the worker obtains its first event, before gathering/conversion/validation, and stops immediately after awaited capture IPC returns. In-interval scheduling delays remain included. No production timeout, completion SLA or 100 ms flush cadence was introduced.

Optional diagnostic invocation from the worktree root, alone without competing build/test workloads; a reference-ceiling failure is not by itself a merge blocker:

```sh
cargo test -p cyril --bin cyril capture_forwarder::tests::c9_forwarder_batch_stays_within_100ms_including_awaited_ipc -- --ignored --exact --nocapture --test-threads=1
```

A default-suite ignored/skipped result is **not** performance evidence. The isolated receipt must show the selected case actually executed, zero ignored cases, its actual batch observation, and the checked source/profile/features/environment.

The preceding measurement-repair results remain historical evidence for their named source state, not fresh proof for this changed test registration/fixture. The original aggregate **153.901529 ms** CI failure remains FAIL. The later [PR #190 actual awaited-IPC failure](https://github.com/dwalleck/cyril/actions/runs/39872725799/job/120150806626), event count three and **163.159373 ms**, also remains a genuine FAIL, not flaky, old aggregate timing, or a waived result. The observations below are not a speedup comparison or attribution of either failure.

### Scoped qualification and independent review — PASS; CI pending

The sole-HOST coordinator executed the checkpoint on 2026-10-23 at HEAD `4241739c51a5f1996c483b11e6b63538882cdcd3` with the source change uncommitted. The byte-exact [canonical qualification receipt](evidence/capture-timing-policy-20261023/qualification.json), SHA-256 `f595da25a56b762b6bf73cded08e55070dc45c13bd8d9de7055d8d8816785f44`, owns command/source/environment provenance. The [portable index](evidence/capture-timing-policy-20261023/portable-index.json) maps its original paths to retained raw logs, both temporary mutation diffs, the approved-policy receipt, and retained toolchain/host metadata; no binaries or nested historical archive are included.

| Check / raw receipt | Executed command or scenario | Result |
|---|---|---|
| [01](evidence/capture-timing-policy-20261023/01-fmt-CHECK.log) | `cargo fmt --all -- --check` | PASS; no formatter application |
| [02](evidence/capture-timing-policy-20261023/02-ordinary-capture-forwarder-module.log) | `cargo test -p cyril --bin cyril capture_forwarder::tests -- --nocapture` | Ordinary C9 PASS; one ignored checkpoint, not performance PASS |
| [03](evidence/capture-timing-policy-20261023/03-exact-C10-lifecycle.log) | `cargo test -p cyril --bin cyril app::tests::current_runtime_contract::shutdown::c10_shutdown_waits_bridge_then_drains_capture_then_stops_memory -- --exact --nocapture` | C10 lifecycle PASS |
| [04](evidence/capture-timing-policy-20261023/04-isolated-actual-runtime-checkpoint-baseline.log) | Explicit isolated command above | PASS; actual three-event batch **3.351936 ms** |
| [05](evidence/capture-timing-policy-20261023/05-mutation1-150ms-pre-worker-expected-PASS.log) | Temporary 150 ms await before `super::run`; observed pre-worker delay **151.228487 ms** | PASS; actual three-event batch **4.480846 ms**, delay excluded |
| [06](evidence/capture-timing-policy-20261023/06-mutation1-restored-checkpoint-GREEN.log) | Exact worker restoration; explicit isolated command | GREEN; actual three-event batch **9.059132 ms** |
| [07](evidence/capture-timing-policy-20261023/07-mutation2-150ms-real-Captured-IPC-expected-100ms-RED.log) | Temporary 150 ms delay after real `execute_request`, before sending only `MemoryResponse::Captured`; dependency edit not `cfg(test)`-gated | Intended exit 101 at the unchanged 100 ms assertion; actual three-event batch **155.692403 ms**, after shared persistence/drain/shutdown assertions; not compile/setup/watchdog failure |
| [08](evidence/capture-timing-policy-20261023/08-mutation2-restored-final-checkpoint-GREEN.log) | Exact IPC runtime restoration; explicit isolated command | GREEN; actual three-event batch **3.739313 ms** |
| [09](evidence/capture-timing-policy-20261023/09-App-alltarget-allfeature-strict-Clippy.log) | `cargo clippy -p cyril --all-targets --all-features -- -D warnings` | PASS |

There were **nine commands: eight exit 0 and one specific expected assertion RED**, with no unexpected failure. All five isolated checkpoint invocations executed exactly one case with zero ignored cases: four PASS and the intended inside-IPC RED. Cargo/libtest total wall time is not the batch metric.

Source and applicability:

- Checked and byte-exactly restored forwarder SHA-256: `a8297e3c13458a93945cff4aabf2b275e7db00e0d8b4496d9cb4fef723b53d94`; IPC runtime: `f26507aaee55f6e9c7351c5e577d0285a0e3ab60b699d30bd1ba6858ed6188de`. Both temporary edits are absent. The canonical receipt's route/evidence/plan hashes name the pre-amendment documents, not this documentation update; no Cargo input changed during this amendment.
- Tests used default App features (`kas`) and the declared optimized test profile (`opt-level = 1`) on Linux/Unix; strict Clippy covered all App targets/features. The retained toolchain/host metadata comes from the coordinator's 2026-10-22 observations, not a fresh metadata probe. Input pins are explicit/bounded, not an exhaustive transitive workspace manifest.
- The changed checkpoint actually runs `InProcessRuntime`/`run_runtime`, real Unix IPC and SQLite, persistence inspection, and graceful runtime shutdown. This supplies real-runtime smoke for the changed checkpoint, not a separate standalone App launch or native/provider/auth/model proof.
- [Final ordinary independent review](../.cyril-xy4x/review.md) found no actionable source, proof, or documentation defects (confidence 0.97). It covers the completed qualification and authoritative evidence/plan/route amendments. Main still owns separate PR publication, the required CI feature/platform matrix, and merge gating; local qualification and review are not CI or merge authorization.


# Evidence: cyril-nanu

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | The panel refresh path fires many times within one turn, so coalescing (spec D4) is load-bearing rather than decoration. | In a real captured session, how many refresh triggers does one turn produce? | PASS |
| P2 | A second connection can hold one consistent read (spec D5) while the writer commits on the event loop, without either side erroring. | With two connections on one WAL database and a 250 ms busy timeout, does a deferred read transaction see one point in time across a concurrent commit, and does the writer stay unblocked? | PASS |
| P3 | `UsageSnapshot` can be produced off the event loop and moved back to it. | Is `UsageSnapshot: Send + 'static`? | PASS |
| P4 | The App's existing async-result channel is a reusable carrier for that hop, not a same-thread convenience. | Is the type that channel already carries (`UsageEnrichmentResult`) also `Send + 'static`? | PASS |
| P5 | Which of `snapshot()`'s nine rollups dominate its cost. | `N/A — not an empirical premise: spec D3 decided cost reduction out of scope, so a per-query breakdown cannot change the design. The route recorded this as a premise before the spec was signed; the signed spec supersedes it.` | `N/A — non-premise` |
| P6 | The event-loop stall is observable to the operator as dropped input. | `N/A — not an empirical premise: the design moves the work off the loop regardless of how the stall manifests, and the spec's criteria assert the loop does not wait rather than asserting a perceptual outcome. Performance observation is checkpoint territory.` | `N/A — non-premise` |

## Data

- **Source (P1)**: production-shaped — two committed live ACP captures of real `kiro-cli` 2.11.0 sessions, `experiments/conductor-spike/{v2,kas}-live-session-trace-2.11.0.jsonl`. Read-only; the probe opens them for reading and writes nothing.
- **Source (P2)**: production-shaped — 1,000 `usage_turns` rows generated to the real column shape, in a `tempfile.TemporaryDirectory` / `mktemp -d`. Never touches the operator's `usage.sqlite3`; each run creates and destroys its own database.
- **Source (P3/P4)**: the real `cyril-core` crate, compiled as-is. The probe was copied to `crates/cyril-core/tests/probe_nanu_send.rs`, run, and removed; no production file was modified and none remains changed (`git status` clean apart from `.cyril-nanu/`).
- **Safety**: no production state read or written; no approval needed, so none recorded.

## Probe

| ID | File | Mechanism | Run |
|----|------|-----------|-----|
| P1 | `probe_cadence.py` | Structural JSON parse of every trace record, with turns segmented two ways — prompt→its own response, and prompt→next prompt — and context samples matched against the exact converter dispatch conditions (`convert/kiro.rs:340-347`, `convert/kas.rs:314-326`). | `python3 .cyril-nanu/probe_cadence.py` |
| P2 | `probe_wal.py` | Two connections in ONE process via Python's `sqlite3`, with the pragmas `UsageLog` sets (`usage.rs:716-726`): WAL and a 250 ms busy timeout. Reader opens `BEGIN DEFERRED` and reads; writer commits mid-transaction; reader re-reads, commits, reads again. Includes a `journal_mode=DELETE` control. | `python3 .cyril-nanu/probe_wal.py` |
| P3, P4 | `probe_send.rs` | The compiler: `fn assert_send_static<T: Send + 'static>()` instantiated for both types, plus a real `std::thread::spawn` round trip so the bound is exercised and not merely declared. | `cp .cyril-nanu/probe_send.rs crates/cyril-core/tests/probe_nanu_send.rs && cargo test -p cyril-core --test probe_nanu_send && rm crates/cyril-core/tests/probe_nanu_send.rs` |

## Oracle

| ID | File | Mechanism — and why it differs | Run |
|----|------|-------------------------------|-----|
| P1 | `oracle_cadence.sh` | `awk` over raw text. Never parses JSON and segments turns by a different rule (prompt→next prompt only). A wrong JSON path, a wrong nesting assumption, or a mis-correlated request id in the probe surfaces as a count mismatch. | `.cyril-nanu/oracle_cadence.sh` |
| P2 | `oracle_wal.sh` | Two separate OS **processes** driving the `sqlite3` CLI, coordinated through a FIFO. Different binary, different binding, different process model: an in-process special case or a Python-binding quirk in the probe cannot reproduce here. | `.cyril-nanu/oracle_wal.sh` |
| P3, P4 | `oracle_send.sh` | Source inspection, never compiling: scans the reachable type definitions for the only constructs that can make a plain std-built struct non-`Send` (`Rc`, `RefCell`, `Cell`, raw pointers, bare `dyn`), for `unsafe impl` that could fake the bound, and for borrowed lifetimes that would break `'static`. Trait resolution vs. reading the source are independent routes to the same answer. | `.cyril-nanu/oracle_send.sh` |

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 (v2) | turns 14; inter-prompt window per turn `[1×14]`; total 14; before first turn 1; max triggers in one turn **2** | turns 14; per turn `[1×14]`; total 14; before first turn 1; max triggers in one turn **2** | PASS — identical |
| P1 (KAS) | turns 2; inter-prompt window per turn `[3, 17]`; total 20; before first turn 1; max triggers in one turn **18** | turns 2; per turn `[3, 17]`; total 20; before first turn 1; max triggers in one turn **18** | PASS — identical |
| P2 (WAL) | reads 1000 / 1000 / 1001; writer error none; CONSISTENT True; WRITER UNBLOCKED True; READER ADVANCES True | reads 1000 / 1000 / 1001; writer error none; CONSISTENT True; WRITER UNBLOCKED True; READER ADVANCES True | PASS — identical |
| P2 (DELETE control) | reads 1000 / 1000 / 1000; writer `database is locked`; WRITER UNBLOCKED **False** | reads 1000 / 1000 / 1000; writer `database is locked`; WRITER UNBLOCKED **False** | PASS — identical, and the control fires |
| P3 | `assert_send_static::<UsageSnapshot>()` compiles; a snapshot moves into a spawned thread and back | no `Rc`/`RefCell`/`Cell`/raw pointer/bare `dyn` in the reachable definitions; no `unsafe impl`; no borrowed lifetime | PASS — same conclusion by different routes |
| P4 | `assert_send_static::<UsageEnrichmentResult>()` compiles | same scan, same result over `usage.rs` | PASS — same conclusion by different routes |

## Validated / learned

- **P1 — learning, and it reshapes the design's emphasis.** cyril-nanu asserts context samples fire "MULTIPLE TIMES DURING a turn". That is true, but **only on KAS**: v2 fires exactly one context sample per turn in all 14 captured turns (2 refresh triggers per turn counting the turn-end write), while KAS reached **17 context samples in a single turn — 18 triggers**. Coalescing (D4) is therefore load-bearing, not insurance: without it one KAS turn would queue 18 recomputes, ~12 s of work at the measured ~700 ms per snapshot, while the operator waits. The engine asymmetry was not previously recorded anywhere.
- **P1 — probe correction, recorded rather than hidden.** The first probe run reported zero KAS context samples. Cause 3 (the probe was wrong): it looked for `_meta` under `params` when the KAS frame nests it under `params.update`, and it did not require the `kind`/`usagePercentage` pair the converter's dispatch arm actually demands. Corrected against `convert/kas.rs:314-326` and re-run. A second, smaller disagreement then remained — probe `[2,16]` vs oracle `[3,17]` — traced to the two using different turn windows (prompt→response vs prompt→next prompt); the probe now reports both, and the operationally relevant one, inter-prompt, matches the oracle exactly. Totals agreed at 21 throughout, which is what localised the cause.
- **P2 — validated prior understanding, with the control doing real work.** A deferred read transaction on a second connection sees one point in time across a concurrent commit, and the writer is never blocked: probe and oracle agree on 1000 / 1000 / 1001 with no writer error. The `journal_mode=DELETE` control shows the writer failing with `database is locked` in both probe and oracle, which is what makes the WAL result meaningful rather than a property that would have held regardless. D5 is implementable exactly as specified.
- **P3/P4 — validated prior understanding.** `UsageSnapshot` and `UsageEnrichmentResult` are both `Send + 'static`; the snapshot survives a real `spawn`/`join` round trip. The App's existing `mpsc` + `select!` path (`app.rs:76`, `:748`) already carries an owned value across a thread boundary, so the delivery mechanism this design needs exists and is reusable rather than novel. The probe's negative control confirms the assertion is not vacuous: substituting `Rc<()>` fails to compile with "`Rc<()>` cannot be sent between threads safely".

## Related issues

- Consulted (copied from `spec.md`, not re-searched): **cyril-9kyk** (closed — the p90/max work that measured the cost and created this ticket; its 700 ms and 2 s fences bound this change), **cyril-b163** (open — unbounded `usage_turns` growth, the axis that makes the cost scale), **cyril-kryv** (closed — shipped the panel; source of the adopted async-with-explicit-status precedent), **cyril-c6la** (open — same disease on the render path), **cyril-gfkm** / **cyril-4h6i** (closed — built the observer producing these writes).
- Filed: none. No premise revealed an underlying-system defect, and no deferral was created by this stage.

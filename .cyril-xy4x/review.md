# Independent review: cyril-xy4x

## Final verdict

- **Overall correctness: correct.**
- **Findings: none.** No patch-introduced actionable defect or unresolved source/proof/documentation blocker was found.
- **Confidence: 0.97.**
- The final ordinary independent review covers the test-only change, completed scoped qualification, portable raw proof, and authoritative evidence/plan/route amendments. This closes the previously provisional review; it does **not** claim green final CI or authorize merge. Main still owns separate PR publication, the required CI feature/platform matrix, tracker/Git operations, and merge gating.

## Reviewed scope and provenance

- Worktree: `/home/dwalleck/repos/cyril-wt-fix-cyril-xy4x`.
- Initial source review read the actual `git diff 1ec6fc2f035d2f807f66c434d02a8e351fe7b125 -- crates/cyril/src/capture_forwarder.rs .cyril-xy4x/route.md`, the full current forwarder, and the full new route. The untracked route was read directly and is included in the supplied complete patch.
- Source pin retained from the handoff and coordinator restoration receipt: `capture_forwarder.rs` SHA-256 `a8297e3c13458a93945cff4aabf2b275e7db00e0d8b4496d9cb4fef723b53d94`. IPC runtime pin: `f26507aaee55f6e9c7351c5e577d0285a0e3ab60b699d30bd1ba6858ed6188de`. The final documentation handoff records no source edits after qualification. Source analysis below is retained rather than reconstructed or rerun.
- Final proof: `local://c9-xy4x-timing-policy-qualified-handoff.json`, canonical SHA-256 `f595da25a56b762b6bf73cded08e55070dc45c13bd8d9de7055d8d8816785f44`, also published as `.cyril-n3j7/evidence/capture-timing-policy-20261023/qualification.json`.
- Final documentation handoff: `local://c9-xy4x-timing-policy-docs-ready.json`; documentation amendment: `local://c9-xy4x-timing-policy-doc-amendment.diff`; complete qualified patch: `local://c9-xy4x-timing-policy-complete-qualified.diff` (supplied SHA-256 `b284c25f3d3658d827c18600345148cc16ca1fe0bc006d7374f48df74448bbb1`).
- Read the actual final evidence/plan/route, portable index and proof-directory inventory, raw command logs 01–09, both complete temporary mutation diffs, and retained rustc/Cargo/kernel/CPU metadata. The reported SHA identities are the supplied coordinator/transport receipts; the reviewer did not run checksum or verification commands.
- Governing inputs read: `local://cyril-xy4x-user-approved-policy.json`, `local://c9-original-contract-findings.md`, `local://c9-awaited-budget-source-findings.md`, source-ready JSON/diff, gilfoyle/CONTRACT, Rust review rules, and fence-vacuity/layer-attribution rules.

## Findings

None meeting the patch-introduced, actionable, evidence-backed defect criteria.

## Retained source conclusions

1. **The metric is preserved exactly.** `capture_forwarder.rs:75-83` still starts the monotonic clock immediately after obtaining the first event; `:117-136` still converts/validates the batch, awaits `memory.capture_batch(batch)`, then records elapsed time before sending the observation. These lines are outside the source diff. `:265-268` retains the exact `observation.elapsed <= Duration::from_millis(100)` predicate and failure message. No scheduler subtraction, timer relocation, retry, warm-up exemption, timeout change, or threshold relaxation was introduced.

2. **Ordinary C9 retains the original functional assertions without the 100 ms assertion.** The existing test at `:256-259` awaits the private shared fixture. That fixture at `:272-349` retains prequeued sequences 0/1/2, sender closure, the real worker spawn, forwarder drain, exact persisted prompt and `Finished(Completed)` checks, graceful runtime shutdown, exactly three observed events, and a disconnected observer channel with no second batch (`:314-347`). The only elapsed-time comparison is inside the ignored performance test at `:266`; the helper returns its actual observation. The existing two-second drain watchdog (`:19,51-60`) and existing runtime readiness/shutdown bounds are unchanged, not new ordinary-suite machine-speed requirements.

3. **The ignored checkpoint has a correct operational entrypoint, now supported by execution receipts.** `main.rs:2` registers `capture_forwarder`; the Unix test module at `capture_forwarder.rs:244-247` contains `c9_forwarder_batch_stays_within_100ms_including_awaited_ipc` at `:261-270`. The exact command documented in the route, evidence, and Slice 2 plan is `cargo test -p cyril --bin cyril capture_forwarder::tests::c9_forwarder_batch_stays_within_100ms_including_awaited_ipc -- --ignored --exact --nocapture --test-threads=1`. It explicitly opts into the ignored case and selects it exactly. There is no early-return/environment skip in the test or shared fixture. Raw baseline/restored outputs show the named case executing, zero ignored, and the actual printed observation; these are not default-suite skip receipts.

4. **The new function boundary has correct consumers.** The worker still sends `BatchObservation` through the existing optional channel (`:126-136`); the fixture receives it, validates count and worker/channel completion, and returns that same value (`:332-347`). The performance test consumes `elapsed` (`:264-266`). The ordinary test intentionally discards only the returned timing value after all shared assertions run (`:258`). There is no unmatched consumer dispatch, replacement expected-value computation, duplicated fixture, or tautological assertion.

5. **The checkpoint remains real Unix IPC plus SQLite, not mocked timing.** The helper calls `InProcessRuntime::start` and binds private workspace scope (`capture_forwarder.rs:273-275`). `memory_runtime.rs:820-884` prepares real private data/socket roots and credentials, spawns `cyril_memory::run_runtime`, and waits for readiness; `:912-924` requires graceful shutdown and a clean runtime join. `ProjectMemory::capture_batch` (`:197-205`) awaits a fresh client's capture request. Server `runtime.rs:168-202` executes the request before sending its response, and `:292-297` dispatches CaptureBatch to `stores.capture_batch`, yielding Captured on success. `client.rs:202-217` awaits the response and explicitly accepts Captured. The later `inspect_turn` check remains an actual persisted-record observation, independent of event construction.

6. **No production behavior changes.** The source diff consists only of the Unix test-module comment and registration/fixture extraction below `#[cfg(test)]`/`#[cfg(unix)]`. Event/byte/queue bounds, batch selection, conversion, IPC, production spawn, failure handling, and shutdown code are unchanged. C10 is not modified. No dependency, public API, production timeout, or parallel proof implementation was added.

## Completed proof review

The canonical receipt binds the run to worktree HEAD `4241739c51a5f1996c483b11e6b63538882cdcd3` with the source change uncommitted, explicit source/config/manifests/lock/entrypoint pins, default App features (`kas`), and declared test `opt-level = 1`. Command timestamps are sequential. Source/config/ref guards surround commands and final restoration. The bounded source pins are explicitly not presented as an exhaustive transitive workspace manifest.

Raw files below are under `.cyril-n3j7/evidence/capture-timing-policy-20261023/`:

| Receipt | Directly observed result | Review conclusion |
|---|---|---|
| `01-fmt-CHECK.log` and canonical command 01 | Empty raw output; recorded `cargo fmt --all -- --check` exit 0 | Formatting check PASS; no formatter application claimed |
| `02-ordinary-capture-forwarder-module.log` | Ordinary C9 `ok`; performance case explicitly ignored; summary 1 passed / 0 failed / 1 ignored | Functional default coverage executed; ignored result is correctly excluded from performance proof |
| `03-exact-C10-lifecycle.log` | Exact shutdown-order case `ok`; 1 passed / 0 ignored | Existing C10 lifecycle remains exercised |
| `04-isolated-actual-runtime-checkpoint-baseline.log` | Exact performance case `ok`; `BatchObservation { event_count: 3, elapsed: 3.351936ms }`; 1 passed / 0 ignored | New operational entrypoint actually executes its real-runtime assertion |
| `05-mutation1-150ms-pre-worker-expected-PASS.log` | Logged pre-worker wait 151.228487 ms; actual three-event batch 4.480846 ms; 1 passed / 0 ignored | Outside-worker wait is excluded, without subtracting it from the measured interval |
| `06-mutation1-restored-checkpoint-GREEN.log` | Actual three-event batch 9.059132 ms; 1 passed / 0 ignored | Fresh GREEN after recorded exact worker restoration |
| `07-mutation2-150ms-real-Captured-IPC-expected-100ms-RED.log` | Named test panics at `capture_forwarder.rs:265:9` with the unchanged 100 ms failure message; actual three-event batch 155.692403 ms; 1 failed / 0 ignored; canonical exit 101 | Specific timing fence RED, not compile/setup/skip/watchdog failure |
| `08-mutation2-restored-final-checkpoint-GREEN.log` | Actual three-event batch 3.739313 ms; 1 passed / 0 ignored | Fresh final GREEN after recorded byte-exact runtime restoration |
| `09-App-alltarget-allfeature-strict-Clippy.log` | All App target/feature dependency checks finish successfully; canonical exit 0 | Strict all-target/all-feature Clippy PASS, not an all-feature behavioral test claim |

The count is accurate: **nine commands, eight exit 0, one intended assertion-specific exit 101**. Five isolated invocations each execute exactly one case with zero ignored: four PASS and one intended inside-IPC RED. Six positive test-case passes across all invocations do not mean six unique test names. Cargo/libtest total wall time is never substituted for actual batch elapsed time.

### Control placement and restoration

- `mutation1-preworker.complete.diff` wraps only the shared fixture's task entry, awaits 150 ms, logs that wait, then calls unchanged `super::run`. It does not move the timer or weaken the predicate. This matches the outside-delay receipt and intended exclusion.
- `mutation2-captured-IPC.complete.diff` adds 150 ms only after real `execute_request(...)` and before sending a response matching `MemoryResponse::Captured`, without dependency `cfg(test)` gating. Ready, inspection, and shutdown replies are not delayed. The actual named timing assertion fails with event count three. Since `forward_batch_and_assert_lifecycle().await` returns only after persistence/status/drain/shutdown/observation checks, reaching the subsequent panic establishes those checks completed first.
- Canonical mutation records bind immediate backups, mutated snapshots/diffs, restored bytes/modes, command source hashes, and subsequent GREENs. Final source pins return to forwarder `a8297e3c...` and runtime `f26507aa...`; both temporary edits are absent according to the exact-restoration guards. The final documentation worker changed only Markdown and portable evidence, preserving the scoped code-proof applicability.
- These are deliberate falsification/restoration steps, not retries to hide a genuine unmodified failure. The original CI failures remain failures.

## Documentation and evidence-claim review

- `.cyril-n3j7/evidence.md:111-153` records the approved split, exact explicit `--ignored --exact --test-threads=1` invocation, actual execution counts/observations, current scoped controls and restorations, and source/environment limits. Its figures match the raw logs and canonical command records read above.
- `.cyril-n3j7/plan.md:79,95` keeps the existing ≤100 ms IPC-inclusive obligation, makes its isolated pre-merge execution explicit, and states skipped default results do not discharge it. Existing numerical obligations and the two-second drain bound remain intact.
- `.cyril-xy4x/route.md:17-19,30-44` retains the requester-selected scheduling policy, refers to the evidence owner, accurately records completed scoped qualification, and reserves final CI/merge gating. Its final-review-pending language describes the pre-review handoff state; Main can now publish this final review receipt and update that status without changing the reviewed behavior or rerunning proof.
- Original aggregate 153.901529 ms and actual awaited-IPC 163.159373 ms CI results remain genuine FAIL, not flaky, waived, exonerated, or misidentified as each other. Prior measurement-boundary controls retain historical source-state meaning; fresh changed-registration controls are used for the current claim.
- The portable directory inventory has 18 text proof files matching the documented allowlist; no binaries, source-preimage tree, credential material, or nested historical archive is present. `portable-index.json` preserves original paths and maps current evidence/policy owners and raw receipts. Canonical pre-amendment route/evidence/plan hashes are explicitly distinguished from the later documentation hashes.
- Retained rustc/Cargo/kernel/CPU metadata is explicitly dated to the coordinator's 2026-10-22 observations, not misrepresented as a fresh probe. The documented scope is Linux/Unix default-feature tests plus all-feature lint, not full feature/platform behavioral CI.
- The checkpoint itself supplies real runtime/Unix IPC/SQLite/persistence/shutdown smoke for its changed entrypoint. No separate standalone App, native terminal, provider, live-auth, or model proof is claimed or demanded.

## Remaining Main-owned publication gates

Required final CI feature/platform coverage and separate PR/tracker/merge processing remain pending. Local qualification and this final ordinary review must not be relabeled as final CI PASS or merge authorization. There are no remaining reviewer-requested source or proof repairs.

## Reviewer actions and limits

Read-only inspection only; local report writing is the sole reviewer artifact mutation. No tests, builds, formatters, linters, runtime launches, source edits, Git mutations, tracker operations, PR changes, or merges were performed. The initial LSP attempts reported ready rust-analyzer but returned no linked-worktree references/definitions; tightly scoped reads/searches established the relevant helper consumers and runtime dispatch. No full reconstruction or rerun was performed after qualification.

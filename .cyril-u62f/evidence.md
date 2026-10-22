# cyril-u62f — prepared phase observation

## Premise and current conclusion

**U62F-P1 — UNATTRIBUTED; PENDING, Main / separately authorized hosted phase capture.** Which named phase accounts for the approximately 7–9 seconds outside the historical readiness-message measurement, under the unchanged normal Windows Default Features package command? Relevant boundaries remain parent setup/spawn, child root protection/bind/store initialization/accept/health, error/backtrace capture or reporting, root cleanup, and executor teardown. These are observation boundaries, **not asserted causes**. No further fault injection or guessed phase budget is selected.

The actual issue acceptance was read from `.rivets/issues.jsonl`, record `cyril-u62f`: phase attribution first, then a fix or documented budget. A green isolated test, unchanged relevant source, or a green retry is not attribution. No production repair has been selected or implemented.

### Current representative-observation preparation (2026-10-22)

The approved proposal is the latest `agent://DiagnoseTqrWindowsReady` handoff. This increment prepares a **temporary draft PR on its normal Actions checkout**, not another exact-d6 VM export and not a production repair. Main owns review/gates, Git/commit/publication, the draft PR, CI observation and complete job-log retention. No execution is authorized by this preparation.

The existing local receipts remain immutable and have narrower meanings:

- `local://u62f-native-attribution-run02.json`: no controlled fault; all seven memory-runtime tests passed, but only 200 package results completed before the observer interrupted the driver. This is **not a completed no-fault package PASS** and did not exercise the failed readiness/reporting path.
- `local://u62f-controlled-native-attribution-run03.json`: the deliberately mismatched credential exercised Unauthorized/readiness-error/reporting boundaries; it did not reproduce or explain the historical startup pattern.
- `local://u62f-postbind-native-observation-run01.json`: the separately authorized 500ms post-bind input naturally recovered; 282 passed and one skipped, with Ready on attempt 4 at 602,356us after two connect failures and one health timeout. The first Ready response encountered BrokenPipe 232, then later attempts recovered. This is a **controlled local diagnostic**, not no-fault hosted evidence. Its native preparation is `local://u62f-postbind-native-preparation-attempt2.json`. The earlier host-import failure and its separately diagnosed isolation correction remain preserved in the run receipt.

These observations do not attribute the original 7–9 seconds. Local passing readiness spans, or the recovery after a controlled timeout/BrokenPipe path, do not authorize a fix, timeout increase, automatic rerun or issue closure.

#### Prepared hosted instrument and fidelity

- `runtime.rs` now activates the existing bounded connection ordinal/observation with `cfg!(windows) && phase_probe::enabled()`, independently of the 500ms input. The Windows/test-name/delay-flag/auth-absence fault guards and the 500ms delay itself are unchanged; only the now-unused non-Windows `post_bind_probe=false` binding is removed. The 16-connection cap, 256-record cap, detail cap, serial handler/bool contract, first-health boundaries and outcome-before-drop/after-await-return distinction remain unchanged.
- Only the existing Windows **Default Features** matrix lane prepares a fresh, non-hidden `$RUNNER_TEMP/cyril-u62f-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT/phases` directory. Both fault inputs and inherited profile/thread overrides must be **absent**, including empty values; `RUST_BACKTRACE=1` is required. Inherited probe/directory/export-source inputs are also rejected, so the helper's historical edit base is never relabeled as the hosted checkout.
- The existing installer pins nextest **0.9.148 on Windows in this job only** through its `tool` input. Linux/macOS retain the installer's existing `nextest` input. No other installer or job changes.
- The original `cargo build -p cyril --bins` and **`cargo nextest run -p cyril`** remain unchanged. Only the package test step gets the helper's existing `CYRIL_MEMORY_PHASE_PROBE` / `CYRIL_MEMORY_PHASE_DIRECTORY` inputs (disabled/empty on other platforms). No output flags, wrapper, `continue-on-error`, profile/thread/fail-fast/feature overrides, timeout changes, assertion changes, process supervisor or VM driver are added.
- Windows finalization and separate `actions/upload-artifact@v4` upload are immediate following steps, each with `always() && runner.os == 'Windows'`, before later Clippy/workspace/no-default steps. Outcome metadata records the original Actions step outcome, **not an invented native exit code**. Executable SHA256s are taken only afterward, from exact `current_exe` paths in retained helper identity records, linked to phase file/line/PID/component/role; cached fixture globs are not execution evidence.
- The only upload path is the prepared fresh root. Artifact name includes run ID/attempt; missing files are an error, retention is 14 days, overwrite and hidden-file inclusion are false. There is no manual ZIP or broad hidden-directory upload. Metadata failure cannot prevent attempting the independent upload, but runner loss/forced cancellation can still leave missing evidence.

The first ordinary frozen-state review found F7: the finalizer initially omitted the helper's existing library `child_configuration` identity and would have failed before saving binary pins. The two-line repair admits that role alongside `child_entry` and validates both as `cyril-memory-runtime.exe`; fixture validation stays distinct. `review-decisions.md` owns the direct-source finding/repair record, and `local://u62f-hosted-capture-preparation-review.json` records the independent read-only review. That review supplies source/serialization reasoning, not executed Windows finalization; the separate current syntax-only proof is recorded below.

#### Main-owned source qualification and bounded lint repair

Under a separate Main GO, `QualifyPerformanceTrees` ran exactly one pinned `cargo fmt --all` (exit 0), then `cargo check -p cyril --all-targets` (exit 0). `cargo clippy -p cyril --all-targets -- -D warnings` stopped with exit 101 on five `clippy::expect_used` sites: companion runtime creation and the fixture's existing root/child invariant accessors. The planned Linux `memory_runtime` smoke was **not invoked**. Immutable command/source/format/pin/log evidence: `local://u62f-hosted-capture-Linux-qualification-attempt1.json`. The pre-format preparation packet/diff and this receipt's post-format identities are separate source states, not one current-byte attestation.

Main authorized only explicit matches for those five sites. Some/Ok returns the same path/root/reference/runtime; None preserves the same invariant panic messages, and companion Err preserves `create runtime: ` plus Debug error formatting instead of introducing a fallback or normal exit. Ownership, take/borrow operations, drop order, runtime builder/phase/result boundaries, test assertions and every timing/fault policy are unchanged. `review-decisions.md` owns F8; `local://u62f-hosted-capture-expect-lint-repair-source-frozen.json` preserves the exact reviewed source handoff, and the existing reviewer found no defect (source-only confidence 0.99). This is instrument qualification, not the original Windows readiness cause.

**Repaired Linux qualification — scoped PASS, partial trace.** `local://u62f-hosted-capture-repaired-Linux-qualified-handoff.json` (SHA256 `78e5e4c3d230aaf0f5258b87986249de468b4749af9250b350ea4fdeefbf662a`) binds the actual 503-input repaired frame `486f0dc40cfc33ea55b5122b88592c446ec43caf6ef6aaaa6d8beaea0202464d`, verified 11 source-pin entries and unchanged manifest SHA256 `258e393848dab0ff78cd068e858a50816e16e312bf1b73c3e18e54590237a452`. Main's sole qualifier ran format **check only**, strict App all-target Clippy and the full Linux `memory_runtime` target, all exit 0; libtest reported **8 passed, 0 failed, 0 ignored, 0 filtered out**. The prior App all-target check is retained under the receipt's applicability statement. No second formatter, source/pin mutation or Cargo rerun occurred. Publication-document refreshes after this frame are metadata-only and are not falsely assigned that full-frame identity.

The standalone case-line matcher initially counted six because two case-result lines interleaved with child stderr. Its administrative STOP remains immutable (`immutable-whole-target-case-line-matching-stop.json`, SHA256 `92864ab5005a4a8e056b68f5cd6da91dea9b402f5d26bd0713c22406da001bb8`); Main explicitly accepted the **original** exit-0 whole-target summary and eight expected unique case starts. No result rerun or checker redesign supplied the PASS.

The retained Linux capture has **28 files / 866 records / 32 identities / 9 owned-child records**, with **one parent fixture helper reaching the global 256-record cap**. Later parent cleanup/drop trace coverage is incomplete. This single libtest process is not nextest's per-test process model; do not promote these counts to complete nextest/parent teardown evidence. Zero observed truncation/file-error markers applies only to retained material. Windows connection observation is cfg-disabled on Linux, and successful Linux ACL-related phases do not prove Windows ACLs. Both actually observed Linux executables were retained privately by the qualifier; no binaries belong in the publication allowlist.

**Workflow syntax only:** `local://u62f-hosted-workflow-parser-proof.json` records successful YAML loading and actual local `pwsh` `Parser.ParseFile` for the prepare/finalize bodies, each exit 0 with zero parser errors, against workflow SHA256 `078af904cf6b4c41acc67cf37a6c37572b6171d37c876bbd4ea7c0817f05e130`. Neither body was executed; this is not hosted Windows behavior or upload proof.

#### Provenance and remaining interpretation limits

Whitelisted preparation records retain the **actual** checkout and GitHub SHA, PR head/base, repository/ref/run/attempt/job, runner name/OS/architecture, `ImageOS` and **actual `ImageVersion`**, OS/build/CPU/RAM, Rust/Cargo/nextest/PowerShell versions, selected existing build/backtrace/target inputs, and source/configuration/lock SHA256s. There is no environment dump, credential, wire payload or binary-content upload. The historical image `windows-2025-vs2026` / `20261018.275.1` is not presumed to be the newly resolved `windows-latest` patch.

`source-pins.json` binds the temporary instrument while retaining historical exact-d6 VM fields as historical. The current worktree lock SHA256 is `9d8af317e2be6201724d88bc5f0137f60d73bd673b244a269eb4c3842a94ecb8`, **not** d6's `67f80b06648c4f534e91435fc49c374f71908874faf0b5b9df5a63831f114600` (Git blob `6c3314a4cc49c5e4b899a1170a8624eb576ad4fc`). Hosted CI uses/hashes its actual normal checkout lock; it does not substitute the historical one. **No 4ef=d6 or published-head baseline-equivalence claim is made.** Main will commit the qualified diagnostic and merge Main038 normally; **after that merge**, compare the actual published head against d6 across all `crates/cyril-memory/**`, fixture/companion, relevant manifests/lock, `rust-toolchain.toml`, CI and Cargo/nextest configuration, classifying diagnostic versus ambient changes. The future Actions merge checkout SHA is unknown until CI exists and must be evaluated from its captured identity separately.

Use only complete phase boundaries from the actual run. Nested spans must not be summed; elapsed origins from separate component files must not be subtracted. Handler outcome→return envelopes include intervening work/scheduling and do not isolate Drop, physical pipe closure, native wait or limbo flush. **Parent client-drop duration remains unobserved.** Observer I/O can perturb timing. A cap/missing phase/interruption/upload failure leaves the relevant mechanism unknown.

An actual slow hosted recurrence with complete phases may attribute a named interval before a repair/budget is chosen. A green hosted capture is retained as **non-reproduction**, without closure or automatic rerun. Complete original job logs must be retained separately to preserve the actual command, compiler diagnostics, nextest summary and reported process exit. `local://u62f-hosted-capture-preparation.json` is the source-only handoff; its planned gates/publication commands are not executed evidence.

## Immutable failure and source provenance

- Run 39536153773, attempt 1, Windows Default Features job 119013159742: <https://github.com/dwalleck/cyril/actions/runs/39536153773/job/119013159742>.
- Raw source: `/tmp/cyril-pr193-run39536153773-awwr_b6s/job-119013159742.raw.log`, 1,516 lines / 131,495 bytes; scout SHA256 `d1487ee7e27ab43ecd0e22351222b3402596f40720b698f7661e093b9f6ef585`.
- Actual checkout `d6f7937af9200a93b9d8997bf856b8fd80c7204e`, parents `c139dbef7a9970ceb8312153962d32d85128152b` and `b75010427c16b1dac13b7cf00c595ce2492f391f` (raw log lines 84, 135).
- Failed command: **`cargo nextest run -p cyril`**, package default features. Companion preparation: `cargo build -p cyril --bins`. Do not replace this recurrence with the older workspace/all-features command cited in historical tracker notes.
- C3 failed during **initial** startup, not replay/restart (lines 1168–1202); forced-kill test failed during **initial** startup, not reopen (1277–1309); schema test failed during **initial** startup, not schema inspection (1383–1417).
- Whole tests ~12.07s, readiness messages ~5.09–5.15s; each retained one health error and a final connect timeout. First typed error, child initialization phase, cleanup phase, and executor completion were not retained. Therefore the remainder is **unattributed**.
- Full scouts read: `agent://DiagnoseTqrWindowsReady` and `agent://DiagnoseTqrReviewerRetain`. The latter is the separately owned `cyril-peaq`, not evidence of this Windows cause.

### Explicit 4ef versus d6 comparison

The instrumentation was edited in the worktree based at `4ef93cad80a47a5fb49a08794e80de57b13054cb`, **not** the failing merge. Main fetched immutable d6 into the shared object database and reported a direct 4ef→d6 comparison of all `cyril-memory`, this fixture/companion entry, Cargo manifests/lock, and `.github/workflows`: **only Cargo.lock differed**. This is a scoped source-equivalence fact, not whole-workspace equivalence or exoneration.

Main's immutable d6 Git blobs (equal to untouched 4ef for the five source owners):

| Owner | d6 Git blob |
| --- | --- |
| `crates/cyril/tests/memory_runtime.rs` | `0c17f4340011eee2cf5def82ada0e6c279b9d6b1` |
| `crates/cyril-memory/src/runtime.rs` | `f912d0d401f20f9df1be60177e216f5a84c3f6ba` |
| `crates/cyril-memory/src/paths.rs` | `092b54a313f6cbda33df20410bb73f019bc315c3` |
| `crates/cyril-memory/src/store.rs` | `302e06ad77cf6cf3654a32ccddf2e52b4d2f4145` |
| `crates/cyril/src/bin/cyril-memory-runtime.rs` | `14a0e3ab7e94ef8f5f6edb8df94ccfe26a107279` |
| root `Cargo.toml` | `dd8a193aca234f3a3c8b96d6f322d2cf2038bece` |
| cyril `Cargo.toml` | `e5d41c57b0b0294b65c763b497713b0857a3b4c5` |
| d6 `Cargo.lock` | `6c3314a4cc49c5e4b899a1170a8624eb576ad4fc` |

The historical VM export contract required **exact d6 plus only the authorized instrumentation delta**, while leaving the different 4ef worktree lock untouched. `probe.ps1` still refuses a lock other than d6; it is not the hosted capture mechanism. Its identity distinguishes `instrumentation_edit_base=4ef…` from `exported_baseline=d6…`, a validated 40-hex driver input supported by source/archive pins, not self-authenticating proof. The current hosted preparation above instead records its normal actual checkout and lock without claiming baseline equivalence.

## Probe mechanism and deliberate choices

### Parent fixture

- Temporary-root creation, memory path preparation/ACL, endpoint identity, credential generation, spawn, owned child PID/executable, readiness entry/exit.
- First **and** last typed connect/health `ClientError` observations, preserving the original human-readable last observation and counters. The poller retains at most two owned errors per operation type and adds no diagnostic formatting, file writes or stderr writes to error handling inside the active readiness loop. Its scope guard emits first/last errors only after polling stops, on success, error or unwind; a sole error is borrowed for both roles without cloning. Raw credentials, environment dumps, command environments and wire payloads are never recorded.
- Deadline observation **before** error construction; separate `parent_readiness_error_construct` covers anyhow backtrace capture.
- Explicit existing shutdown/kill waits log child exit status. `parent_kill_await_includes_wait` deliberately labels the existing `kill().await` behavior; its misplaced following timeout is **not repaired**.
- Observed root wrappers log the result of the existing removal operation, including failure, without adding assertions or reordering root/child destruction. Root removal still occurs before the field's kill-on-drop guard on fixture failure. This preserves, rather than silently repairs, the separate cleanup hazard.
- The child guard logs `try_wait()` status and guard-drop duration. A dropped guard is explicitly **not** an exit/reap acknowledgement. Native observation supplies a separate OS exit boundary where available.
- Existing Windows-relevant test names are synchronous wrappers around the same current-thread Tokio builder/`enable_all` behavior as the original `#[tokio::test]`; assertions and async bodies remain substantive. Panic payloads are resumed after executor teardown. `ObservedTestResult::report` delegates to the original `Result` Termination reporter, measuring **error formatting plus stderr reporting after executor drop**, without premature anyhow Debug formatting, duplicated symbolization, or a changed pass/fail decision.

### Child boundary

- Entry/executable/source identity and multi-thread Tokio executor creation (same flavor as original `#[tokio::main]`), runtime body, executor destruction/end.
- Configuration/root preparation, ACL protection and canonicalization, pipe bind, random instance identity, lock open/acquire, each store precreate and initialization, first accept/request and first actual health receive/send.
- Typed initialization failures are retained. The first successful request is reduced to unit for logging: no request body or credential is Debug-printed.
- Original `process::exit(1)` failure behavior remains; its marker explicitly says executor-drop observation is skipped. No new graceful-shutdown semantics are invented.

### Bounded, live evidence

`probe.rs` is a disposable instrument included at the three owning compilation boundaries. It is disabled unless `CYRIL_MEMORY_PHASE_PROBE=1`. Each linked component permits 256 records and truncates detail to 2,048 UTF-8 bytes before line escaping. No per-attempt stream is added to the readiness loop. Binary and library components use separate per-PID/component/start-stamp files to avoid competing for one file or confusing recycled PIDs.

With `CYRIL_MEMORY_PHASE_DIRECTORY` set to a fresh owned directory, records are appended directly to bounded process files as well as stderr. Thus a worker stuck during executor shutdown does not hide the last phase behind nextest's output buffering. File-open/write failures carry an explicit `CYRIL_U62F_PHASE_FILE_ERROR` marker; a missing marker or truncated trace is not a completed phase.

Each record includes PID, component, sequence, UTC microseconds and per-component monotonic elapsed time. Individual span durations use `Instant`; spans are inclusive/nested and must not be blindly summed. A span starts after its own begin record is emitted; nested logging and observer I/O can still perturb scheduling. The probe measures that instrumented execution, not the unobserved CI run retroactively.

`probe.ps1` preserves package defaults, default profile/concurrency and fail-fast. It adds only `--success-output immediate --failure-output immediate`; it does **not** use `--no-capture`, `--test-threads`, `--no-fail-fast`, a feature override, or increased fixture budgets. These output flags are verified against the exact nextest tag: <https://github.com/nextest-rs/nextest/blob/cargo-nextest-0.9.148/cargo-nextest/src/dispatch/core/run.rs#L485-L503>. Default scheduling evidence: <https://github.com/nextest-rs/nextest/blob/cargo-nextest-0.9.148/nextest-runner/default-config.toml#L23-L25>.

The driver samples only memory-runtime/test-host executables under the owned target directory at 50ms intervals and retains native process handles. After run02's infrastructure interruption, optional sampled-child ExitTime/ExitCode reads were retired: an exit record means **HasExited was observed true at observedUTC**; actual exit time/code are explicitly null and `unavailable-not-sampled`. No actual exit timestamp or status is inferred. The driver's optional exit timestamp is likewise not sampled, but its actual ExitCode remains required, cached after HasExited, and fail-closed if unavailable. Existing fixture shutdown/kill/status observations retain their own actual outcomes independently. Very short processes can be missed; absence from the observer is not inferred exit. The separate 120-second **outer diagnostic** bound is not a fixture-budget change. Cleanup is labelled **observer cleanup**, never test teardown success; no new supervisor/discovery model was added. A primary observer ErrorRecord projection, including InvocationInfo and ScriptStackTrace, is written to `observer-primary-error.json` before cleanup; a cleanup exception has a separate `observer-cleanup-error.json`. Those files, missing required driver status, or a missing probe receipt mean infrastructure failure, not a package result.

## Native prerequisites actually exercised

Read-only preflight command used the existing helper and a credentials **path**, not credential contents:

```text
env PROBE_VM_CREDENTIALS=/home/dwalleck/.config/resourcefs/windows-vm/credentials.env uv run /home/dwalleck/repos/tethys/docs/2026-09-06-legacy-probe-sidecar/vmps.py /tmp/cyril-u62f-vm-preflight.ps1 120
```

WinRM `http://127.0.0.1:55985/wsman`, guest `resourcefs-win11`:

- Windows 11 Pro, `10.0.26200`, build 26200, 64-bit.
- AMD Ryzen AI MAX+ PRO 395; **8 logical processors**.
- RAM 17,109,004,288 bytes; free RAM 12,006,704 KiB at observation.
- C: size 170,392,530,944 bytes; free 64,468,590,592 bytes at observation.
- `rustc 1.94.0 (4a4ef493e 2026-03-02)`, host `x86_64-pc-windows-msvc`, LLVM 21.1.8; cargo `1.94.0 (85eff7c80 2026-01-15)`.
- MSVC `C:\BuildTools\VC\Tools\MSVC\14.44.35207\bin\HostX64\x64\link.exe` and `cl.exe`; imported by `C:\BuildTools\VC\Auxiliary\Build\vcvars64.bat` into the preflight process only.
- No `cyril-memory-runtime` process at preflight.

**Host difference:** failing hosted runner was **Windows Server 2025 Datacenter `10.0.26100`**, image `windows-2025-vs2026`, version `20261018.275.1` (raw lines 11–20). The local Windows 11 guest is not that runner/image, and its CPU/RAM, MSVC and disk state are not claimed equivalent. A non-reproducing local run does not exonerate the defect.

### Authorized nextest prerequisite provisioning

A dedicated follow-up preflight confirmed `cargo-nextest.exe` was not on guest PATH. Main then explicitly authorized only an official exact-version prebuilt install in a **new owned user-local directory**, published-hash verification, and `--version` smoke. This was executed with `/tmp/cyril-u62f-install-nextest.ps1` via the same helper, status 0.

- Archive: <https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.148/cargo-nextest-0.9.148-x86_64-pc-windows-msvc.zip>.
- Published checksum: <https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.148/cargo-nextest-0.9.148-x86_64-pc-windows-msvc.sha256>.
- Corroborating official asset metadata: <https://api.github.com/repos/nextest-rs/nextest/releases/assets/620604881>.
- Verified archive bytes: **7,720,933**; SHA256 **`7a931b4a9123fe5e641dd0f2b73d897f64bcf77bb06f080db17cb434b7eac782`**. Both the downloaded published checksum line and the archive digest/size were checked **before extraction/execution**.
- New owned tools: `C:\Users\resourcefs\AppData\Local\cyril-u62f-prerequisite-3224cc2fadf14af3a236ae0f844f775e\tools`.
- Executable: `cargo-nextest.exe`; SHA256 **`d9dd12a3d5dd67d5b8ca9315fa9f3c2384d1d47f4e324039ba634af36e2862d8`**.
- Only smoke: absolute executable, `nextest --version`, exit 0: **cargo-nextest 0.9.148 (cd1d6d546 2026-10-07)**, full commit `cd1d6d5467322dbc0d39a80cb671d000ac302798`, native MSVC host.
- No global/system PATH change, admin action, Rust/toolchain change, VM restart, repository staging, repository build/test, or runtime probe occurred. This result is prerequisite provisioning, **not Windows qualification**.

## Native preparation and driver process-API observation

Main separately authorized preparation only. The immutable export `0a4ed594858d288bfa00430bcc30f173b01b0f15a8da4fa37af795e4e25122ed` (38,614,174 bytes) is exact d6 plus the authorized instrumentation/workflow overlay. Archive, instrument/manifests/CI pins and d6 Cargo.lock were verified in an owned guest source directory before preparation. The original driver pin was `9bb8d04c75ef819137e8b7fc451ebe88142e45c269d8ad7015457c6429057f4c`; its PowerShell parser reported zero errors without invocation.

- Attempt 1 failed in WinRM command transport before script execution: the encoded command exceeded the command-line limit. Receipt: `local://u62f-native-preparation-transport-failure.json`.
- Attempt 2 staged and verified the original archive, then stopped before compilation when its preparation command runner could not obtain a non-null native exit status. Receipt: `local://u62f-native-preparation-attempt2-failure.json`. Neither failure is a compiler/test result or a product/CI cause.
- After explicit transport/status-repair authorization, attempt 3 reused that exactly verified source, used ordinary synchronous PowerShell native invocation with immediate `$LASTEXITCODE` capture, and completed all seven preparation commands with observed exit 0. These included the default `cargo build -p cyril --bins` and `cargo-nextest.exe nextest run -p cyril --no-run`. Twelve compiled executable pins were retained. **No test, application/runtime scenario or `probe.ps1` invocation occurred.**
- Full successful native command logs: `artifact://2200`. Complete commands, versions, source/binary pins, original/repair script pins, native paths and preserved failure receipts: `local://u62f-native-preparation-handoff.json`. Its VM receipt SHA256 is `a966dc5b66037156089b546d19555ad774eedd55634ea9cbad25d766296c7388`. All three owned allowlisted HTTP services were stopped.

Main also authorized one throwaway `cmd.exe /d /c exit 7` comparison per process-access pattern, not an app/test run. With the original driver's access order, PID 12012 had `HasExited=true`, but `ExitTime` was null and `.ToUniversalTime()` threw after 99.0856ms. **ExitCode was unknown/unread because that earlier access threw**; it was not inferred from stdout or measured as a null ExitCode API return. Eager `[void]$process.Handle` before waiting produced observed ExitCode **7**, a valid exit timestamp and no exception for PID 9468 after 69.9197ms. These timings describe the host-API comparison only, not the unobserved CI delay.

Main authorized the one-line F5 driver correction: `[void]$driver.Handle` immediately after `Start-Process`, before any other driver access/wait. Its reviewed PS SHA256 was **`4c76aa1cc234f89d77f66d6d7b94809f598654c13600b3e73df3f69599134aa6`**; Main's bounded review reported no findings (`local://u62f-driver-repair-review.json`, confidence 0.98). That two-file PS-plus-manifest overlay was subsequently authorized and exercised in run01/run02. The immutable original export and failed-attempt receipts remain unchanged. The later observer-only F6 correction has PS SHA256 **`1500fc08ad6d74e978a7b1f8e98fef3470e37e59fb734835e008aa9314095c2c`** and has **not** been parsed, overlaid or executed on the VM. It requires independent bounded review and separate overlay/runtime GO; no Rust rebuild is needed solely for this PS-only change.

## Historical authorized native attempts — snapshot records

### Run01 and exact missing-Git prerequisite

Run01 retained 13 completed results (12 PASS, one FAIL) before default fail-fast: the real `crtool_cli` test called `Command::new("git")` and received NotFound. No memory-runtime test ran and no phase file existed: **NOT_EXERCISED/noPhaseProof** for U62F-P1, not a startup reproduction. Complete immutable receipt: `local://u62f-native-attribution-run01.json`; raw output: `artifact://2280`.

The failed hosted job had Git `2.56.0.windows.2` and passed that same crtool test; missing Git on this guest was a separate VM prerequisite, **not the hosted failure's cause**. Main authorized official MinGit of that exact version in a fresh user-local prefix. Its published GitHub asset digest was verified before extraction; all 373 extracted files were pinned before the sole version smoke, which returned that exact version with exit 0. A process-local `tools\cmd` prefix survived the same vcvars import and resolved the pinned executable; no global PATH/registry/Git-config change occurred. Complete package/source/hash/native-path evidence and the preserved long-command transport failure are in `local://u62f-git-prerequisite-handoff.json`. MinGit is not the hosted full Git installation or a host-equivalence claim.

### Run02 — seven normal target passes, observer infrastructure interruption

Run02 was separately authorized after that specific prerequisite correction; it retained the same package/default/profile/fail-fast/concurrency policy and outer bound. Source/lock/archive/PS pins, 12 prepared binaries and all 373 Git files were checked before launch. The run reported **200 completed PASS results out of 282 planned**, with no final Summary; the other 82 lack completed results, not proof that none started. The crtool prerequisite passed in 1.727s. **All seven memory-runtime tests reported PASS**, producing 38 live files / 1,006 records from seven fixture and 12 child PIDs, with continuous per-file sequences and no cap/truncation/file-error marker.

Completed normal observations include readiness spans **24.406–132.534ms**, executor-drop spans **37–67us**, and success-report spans truncated to 0us. No readiness deadline, readiness-error construction, or failed anyhow/backtrace formatting occurred. Two shutdown-begin→shutdown-request marker gaps were 250.378/263.095ms; those include client acquisition/health plus scheduling/logging, not individually measured IPC durations. PID9832's data-root close returned Windows sharing violation 32 in **5.683ms** while its test still passed; the holder is unknown. PID6500 was explicitly killed by the forced-kill test, so its absent child executor-end marker is intentional. These local facts neither reproduce the original slow failure nor establish its cause.

The reviewed external observer then threw `InvokeMethodOnNull,probe.ps1`; the actual inner InvocationInfo/ScriptStackTrace was not retained. It recorded cleanup killing the nextest driver and two observed executable pins, but no driver-exit/probe receipt or final nextest status. The strongest source-only candidate is the optional sampled-child ExitTime conversion at **the run02 snapshot's line148**, with PID6500 the sole registration lacking an exit record; **neither exact access nor fatal PID is proven**. Wrapper invocation/check lines are not the inner failure site. Run02 is **infrastructure-interrupted**, not package/CI/product PASS. Full post-run source/all-12-binary/Git checks and remaining-process inventory were not reached.

Canonical facts, per-test/per-child timings, precise stream/source locators and scope limits: `local://u62f-native-attribution-run02.json`; original output: `artifact://2379`. Complete decoded records: `local://u62f-run02-phase-records.json`; 200 reported completions: `local://u62f-run02-reported-test-results.json`. All owned HTTP services were stopped. U62F-P1 remains unresolved.

### Historical controlled readiness preparation — source-only snapshot

Main separately approved implementation only of the **fixture-only** valid credential mismatch. The new branch is immediately after `config.apply_to_command` and requires all three exact guards:

- `CYRIL_MEMORY_PHASE_PROBE=1`, through the shared probe's existing cached `enabled()` check;
- `CYRIL_U62F_READY_AUTH_MISMATCH=1`;
- `NEXTEST_TEST_NAME=real_process_reports_ready_v3_and_exact_store_schema`.

The chosen test performs only one initial launch. Missing or different guard values leave the original command unchanged. The branch copies only the already-applied 64-byte encoded child credential into a fixed stack array, changes its first hex nibble (`0`→`1`, otherwise→`0`), and overrides only that command's `CYRIL_MEMORY_ADMIN_CREDENTIAL`. This avoids another credential serialization/heap buffer, another random credential or library/API changes. Parent credential, endpoint, roots, 5s/250ms/20ms budgets, assertions and scheduling remain unchanged. Impossible missing/length/encoding prerequisites return static diagnostic context without credential values. Only `controlled_readiness_auth_mismatch` with boolean `true` is emitted, after the child environment override and before spawn; it is an active-input marker, not proof of successful spawn.

**Expected, not observed:** the real child completes initialization, stays alive and rejects attempted health requests with typed `Protocol/Unauthorized`; the original readiness loop retries to its existing deadline and constructs its original error. Authentication precedes request decoding, so first-health receive/send markers should be absent while first-request/typed Unauthorized observations should appear. Retain `readiness_deadline`, `parent_readiness_error_construct`, `parent_readiness_wait` error completion, interrupted setup, root/child guard-drop outcomes, executor drop/end and failed `parent_result_report_error_format_and_stderr`. Failed startup does not hand a `RunningRuntime` to the caller, so explicit shutdown/kill/reap-helper phases are not expected; guard drop and observer cleanup are not reap acknowledgement or successful fixture teardown.

This characterizes the controlled error-construction/report/cleanup path, **not hosted startup causality or automatic U62F-P1 closure**. No sleep, second server, backend hook, timeout increase, assertion suppression or failure-to-success conversion was added. Current fixture SHA256: **`69e3b820a96839a5b0ac63546ce17a9491757934ff35d48d7af5ab9daa91b06f`**. PS remains the unqualified F6 `1500fc08ad6d74e978a7b1f8e98fef3470e37e59fb734835e008aa9314095c2c`. The earlier PS/run02 handoffs remain immutable snapshots; this source state supersedes their proposal-only status, not their measured evidence. Main must independently review the actual PS+fixture delta and separately authorize pinned overlay/build and one runtime diagnostic. No VM/build/check/runtime action occurred for this input. Full proposal/alternative analysis: `agent://PrepareU62fAttribution.MapU62fControlledFailure`; actual input handoff: `local://u62f-controlled-readiness-input-handoff.json`.


## Historical VM command contract — not hosted execution authority

1. Main exported exact d6 into an owned fresh source directory and overlaid only authorized instrumentation/workflow files, retaining d6's Cargo.lock. The original archive/source provenance is preserved above and in the preparation handoff. Do not replace it with the worktree's lock, target, environment files or credentials. Later driver-only overlay provenance must explicitly identify the original archive plus the reviewed delta; it must not relabel the original archive.
2. The pinned archive was staged to the owned guest source recorded in the preparation handoff, then the reviewed F5 PS-plus-source-pins overlay was exercised. Preserve original-archive-plus-explicit-delta provenance. Current F6 PS plus the guarded fixture input require independent review and a separate pinned source-overlay/build GO; the fixture change requires new compilation/binary pins, unlike PS-only F6. Do not recreate/overwrite the verified source, call its existing target fresh, silently reuse the old fixture binary, or apply/run these unqualified changes without authorization.
3. Preparation imported vcvars64, put user Cargo and owned nextest tools on process-only PATH, set CI env and used a target absent at the start of compilation. The commands below describe the completed preparation, not authorization to rerun it. The complete handoff preserves actual absolute executables/arguments and native statuses:

```powershell
$env:CARGO_TERM_COLOR = 'always'
$env:RUST_BACKTRACE = '1'
$env:CARGO_INCREMENTAL = '0'
$env:CACHE_ON_FAILURE = 'false'
$env:CARGO_TARGET_DIR = Join-Path $source 'target-u62f-probe'
Set-Location -LiteralPath $source
& cargo.exe +1.94.0 build -p cyril --bins
if ($LASTEXITCODE -ne 0) { throw 'Pinned companion build failed' }
& $nextest nextest run -p cyril --no-run
if ($LASTEXITCODE -ne 0) { throw 'Pinned test preparation failed' }
```

4. Preserve the default nextest profile/scheduling (no inherited `NEXTEST_TEST_THREADS` or `NEXTEST_PROFILE` override). The driver validates source/lock/executable pins, imports the already preflighted `C:\BuildTools\VC\Auxiliary\Build\vcvars64.bat` into its **own fresh process**, and verifies/resolves `link.exe` plus `cl.exe` before running. It records those tool paths without logging environment contents. A prior WinRM/preparation shell is not assumed to leave MSVC initialized. The driver requires the explicit authorization switch:

```powershell
$records = Join-Path $env:LOCALAPPDATA ('cyril-u62f-observation-' + [guid]::NewGuid().ToString('N'))
& (Join-Path $source '.cyril-u62f\probe.ps1') `
  -SourceRoot $source -Nextest $nextest -RecordDirectory $records `
  -QualifiedProbeGo
```

This executes the exact selected package/default lane through the verified nextest executable, with only output-retention flags. A focused `--test memory_runtime` can be a deliberately labelled diagnostic, **not a substitute** for this recurrence's package lane or eventual required qualification.

5. Retain `phases/*`, both nextest streams, `process-observations.jsonl`, any `observer-primary-error.json` / `observer-cleanup-error.json`, source/archive/executable pins and every reported result. Preserve the actual required driver exit code; unknown status, an observer error or missing receipt is infrastructure failure, never inferred exit 0 or package failure/success. Record effective concurrency if directly available; current nextest output does not emit it, so CPU-derived expected capacity is labelled [INFERENCE], not measured runner capacity/overlap. Correlate PID/source identity with phase intervals and typed errors. Missing child exit time/code stay explicitly unavailable. Do not attribute intervals from aggregate wall time, a delayed sample, or unrelated cleanup errors.
6. Main judges U62F-P1, records the attributed phase or continuing uncertainty, then advances the Empirical workflow to a justified fix/budget and the existing full acceptance/qualification. No guessed finite-connect change, timeout increase, cleanup repair, unchanged-source retry or gate waiver closes this premise.

## Separate hazards, not CI causes

Source supports an unbounded native Windows blocking connect beneath the async client timeout, and fixture root/drop plus ineffective termination-bound hazards. They remain **separate candidates requiring their own discriminating native proof**, not established explanations of this CI occurrence. The instrumentation deliberately leaves them intact. Their possible fixes cannot substitute for U62F-P1 attribution.

## Verification and handoff status

**Historical preparation status:** prior preparation packets, native source/attempt receipts and failed Clippy/administrative-matcher records remain immutable with their original identities. They are not relabeled as the repaired-source PASS; the current qualified frame and its partial phase coverage are recorded above.

**Current publication handoff:** ordinary source review, Main-owned Linux qualification and YAML/PowerShell syntax parsing are complete at their stated scopes. Existing route/evidence/review-status refreshes and `local://u62f-hosted-capture-publication-handoff.json` prepare only a temporary **draft PR**, with an exact 12-path stage allowlist/pins; no whole workspace, binaries, retained runtime captures or private authentication material is staged. Main owns the qualified diagnostic commit, normal Main038 merge, post-merge relevant baseline comparison and publication. **Actual hosted Windows phase capture is next**, followed by attribution only if complete observed intervals support it. No publication/Git/check/runtime action was performed by this metadata handoff, no permanent fix or closure is claimed, and **U62F-P1 remains UNATTRIBUTED**.

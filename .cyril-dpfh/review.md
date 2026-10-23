# Independent review — cyril-dpfh Linux reviewer proc-stat observation

## Final verdict

**CORRECT — no actionable findings. Independent ordinary source and evidence review is complete.** The Linux-only classifier and owned-child regression implement the demonstrated proc-stat disappearance contract while preserving unrelated errors and existing lifecycle assertions. Coordinator GREEN receipts and the post-proof narrow route/portable evidence have now been reviewed; this is not PR publication, current-head CI, merge authorization, or tracker closure.

Confidence: **0.96**.

## Reviewed identity and scope

- Worktree: `/home/dwalleck/repos/cyril-wt-fix-reviewer-proc-exit-race`.
- Source base: `03838a501dad8be140acd78dc3fff4cae35fafef`.
- Source defect-review scope: only `crates/cyril-workbench/tests/reviewer.rs`. Read the full initial file context, actual scoped read-only Git diff, supplied complete patch, and both real lifecycle consumers.
- Initially reviewed source SHA-256: `474cc80f047a22b581caf9b2a8e4c8cf84a9e63adc327d9dad68e1ac209185e5`.
- Qualified formatted source SHA-256: `0002ca5f51eed0e020530006a3ae44e2c733d779e76943022f2b07827d574829`. Read the exact formatting-only amendment: one classifier-guard reflow and one live-child-assertion reflow, without semantic changes. The final portable original-to-green patch was also read. Coordinator guards preserve this source throughout qualification.
- Main supplied issue `cyril-dpfh`; qualification HEAD is `c64b6da113444023b84372f3f3818e5496f7d4ce`, the Main-authorized tracker-only commit. Source references below retain initial 474cc80f line numbering; final formatting shifts nearby anchors without changing logic.
- Post-proof documents reviewed: `.cyril-dpfh/route.md` and the complete substantive fields of `.cyril-dpfh/proof/receipt.json`, including its corrected actual one-shot probe launcher metadata. The receipt external self-hash remains packaging/publication metadata owned by Main/coordinator; this reviewer did not calculate checksums or execute verification.

## Findings

None meeting all introduced/actionable/provable-impact criteria. No source blocker or evidence blocker remains within this review assignment.

## Source assessment

### OS identity and classifier

At lines 695–712, `process_has_exited` still reads the actual `/proc/{id}/stat`; a private Linux-only function classifies that read's Result. The only new success case is Linux raw errno 3/ESRCH. Existing NotFound, successful zombie-state classification, and propagation of every unrelated error remain. This does not suppress errors from kill, wait, killpg, arbitrary filesystem operations, or production lifecycle code.

Linux procfs documents that open process descriptors retain the original identity and never switch to a later process reusing the PID. Its documented dead-process ESRCH behavior is supported by inspected Linux v6.8 `fs/proc/base.c`: `stat` is registered through `ONE(..., proc_tgid_stat)`, and `proc_single_show` returns `-ESRCH` when `get_pid_task` fails. These primary sources justify the observation contract, not a historical CI kernel or throwing-call attribution:

- https://www.kernel.org/doc/html/v5.9/filesystems/proc.html#process-specific-subdirectories
- https://github.com/torvalds/linux/blob/v6.8/fs/proc/base.c#L763-L790
- https://github.com/torvalds/linux/blob/v6.8/fs/proc/base.c#L3270

The pre-existing stat parser and numeric-PID lookup architecture remain outside this narrow repair.

### Real regression and controls

Lines 757–797 spawn an owned stdin-gated Python child, classify it live, and open a separate unread proc-stat descriptor. Only after releasing stdin and successfully waiting/reaping that exact child does the test perform the retained descriptor's first read. Raw errno 3 is asserted from that actual read before passing the same real Result to the classifier. No mock manufactures the failure.

The original partial-root, removed-root/live-current-process, and no-process controls remain (727–743). The added unrelated-error control obtains a real directory-read error, requires classifier failure, downcasts to I/O error, and compares kind and errno (745–755). Blanket suppression fails this control; always-exited classification fails the live controls. NotFound and zombie branches are source-identical apart from extraction.

### Ownership and cleanup

After spawn, fallible live observation is saved instead of returning before cleanup. Stdin is closed, exact-Child wait is bounded at five seconds, and wait error/timeout triggers a separately bounded `child.kill().await`. Inspected Tokio 1.53.1 implementation: kill performs start_kill followed by wait; wait is cancellation-safe. The original unsuccessful wait remains a failure after cleanup instead of becoming success. Cleanup failure/timeout also propagates; kill_on_drop is a best-effort backstop rather than an infallible-reap claim.

All live/classifier/read assertions after spawn occur after successful exact-child reap. Stdout/stderr are null, not test-output pipes. No proc-observed numeric PID is subsequently signalled. The corrected comment claims use of the same owned Child, not Tokio pidfd semantics. Child and retained File are RAII-owned.

### Boundary consumers and preserved behavior

The classifier result flows directly through `process_has_exited` into `runtime_drop_cleanup_complete` (715–721), which still samples every PID and requires both owning-root removal AND termination. Read both real consumers: `dropping_owner_terminates_process_tree_before_removing_evidence` (439–462) and `runtime_drop_retains_evidence_until_core_completion` (803–848). They preserve both-PID observations, original 15-second bounds, entire-root absence, empty-parent check, runtime-first destruction, full EOF-window observation, and both Core-completion/evidence-retention assertions. Fixture and late-write behavior are unchanged. No event, command, queue variant, or production router is introduced.

All new native code is Linux-gated in the existing Unix-only integration-test file. Python is already the file's fixture dependency. The private Result-classifier seam serves actual observation and the regression without another process abstraction. No retries, sleeps, budget adjustments, production lifecycle edits, broad suppression, or killpg fixes were introduced.

## Executed evidence reviewed, not rerun

### Independent premise and original-classifier RED

Read the owned-process probe source and result: live R, exact-child poll still live, normal exit 0/reap, first retained-descriptor read ESRCH; emergency signalling is pidfd-only and descriptors are closed. The portable receipt now explicitly records its actual one-shot launcher argv as `python3 /tmp/cyril-final201-owned-proc-stat-probe-20261022-repair.py`, the original cwd and 20-second tool deadline, while honestly stating that no probe execution timestamp was retained.

Read `local://reviewer-proc-race-red-handoff.json`, its retained old-classifier regression source, and the actual authorized raw log. The named exact test executed and failed exit 101: 0 passed, 1 failed, 37 filtered, with raw `Os { code: 3, kind: Uncategorized, message: "No such process" }`. Original match arms were retained and the ESRCH acceptance fix absent. Source-control-flow attribution to final classifier propagation is an inference; no dynamic callsite trace is invented. This RED remains applicable across the intended classifier fix and subsequent formatting-only reflows.

### Corrected GREEN

Read `local://reviewer-proc-race-green-handoff.json` (coordinator-supplied SHA-256 `38010ebfd226b2ae5e586e2fe9eab9fb8ee2dee10760a65c50d8f1c453d89414`) and all three original actual raw logs.

- `cargo fmt --all -- --check`: PASS, exit 0, empty raw output as recorded.
- `cargo test -p cyril-workbench --test reviewer`: PASS, exit 0; **38 passed, 0 failed, 0 ignored, 0 filtered**, test runtime 5.15s. Actual output explicitly names the native regression and both affected lifecycle consumers as `ok`; this is not skip-vacuous evidence.
- `cargo clippy -p cyril-workbench --test reviewer -- -D warnings`: PASS, exit 0, actual check finishes without warnings/errors.

The receipt pins qualified source 0002ca5f and its retained snapshot, records unchanged source and bounded inputs before/after each command and at final release, and preserves original RED/probe plus the separate first formatting-only failure. The coalesced suite exercises the regression and both consumers without duplicate focused invocations. Results are scoped Linux/default-feature reviewer-target proof, not whole-workspace/all-feature/all-platform or published-head CI acceptance.

## Post-proof route and portable packet

Read the full Local route. It accurately states that the route/packet were requested and authored after proof rather than fabricating earlier approval or artifacts. It separates the demonstrated helper defect from closed peaq/EOF repairs, preserves ownership/negative space, and distinguishes local qualification from Main's remaining PR/CI/publication obligations.

Read the packet's source pins, all payload identities, command chronology, original/GREEN applicability, retained environment/configuration provenance, and attribution limits. Inspected the complete directory inventory: the 26-file proof allowlist consists of the receipt plus 25 payloads, with no binaries/archive trees. Payload references are relative to proof; original absolute paths are provenance only. The receipt explicitly limits toolchain/config attestations instead of presenting current copied toolchain configuration as historically hash-pinned.

Also read the portable final source patch, portable green suite log, retained initial fmt-stop log, probe result, and nonsecret home Cargo build settings. These are consistent with the previously reviewed raw evidence and source transitions. Initial fmt failure remains an actual separate failure with no tests/Clippy in that attempt. No new execution is attributed to packaging. This review does not claim independent byte hashing of every copied payload; payload hashes and source guards are the named packaging/coordinator receipts.

Historical PR201 run39873211761's throwing operation remains **UNPROVEN** throughout route and receipt. The demonstrated helper defect and GREEN do not identify its syscall/PID or establish a production lifecycle fix.

## Handoff

The independent review obligation can now be recorded complete by Main. The route's pending independent-review row was correctly pending when authored; Main may amend that row with this result. Remaining issue delivery is Main-owned: separate PR, applicable current-head CI/review gates, tracker/publication disposition, and merge. None is asserted complete by this report.

## Reviewer execution limits

No tests, builds, probes, lints, formatters, source/tracker edits, or Git mutations were executed. Only scoped read-only Git diff, source/evidence reads, navigation/search, and this requested local report. Rust-analyzer listed symbols but returned no references/definitions for known same-file calls in the linked worktree; navigation was attempted before direct anchored consumer reads, and the tool inconsistency was reported via `xd://report_issue`.

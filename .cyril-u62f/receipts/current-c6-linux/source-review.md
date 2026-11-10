# Current C6 workspace/all-features diagnostic — combined source/static review

## Verdict

**Actionable findings: none.**  
**Overall correctness: correct within the reviewed source/static scope.**  
**Confidence: 0.95.**

The review found no evidence-backed, actionable defect introduced by the diagnostic overlay. This verdict is not compilation/runtime qualification, proof of hosted evidence completeness, attribution of the original Windows readiness failure, a permanent-fix claim, or a waiver of failed C6 CI.

## Reviewed state and governing inputs

- Worktree: `/home/dwalleck/repos/cyril-wt-diag-cyril-u62f-all-features`.
- Branch identified by the implementation handoff: `diag/cyril-u62f-all-features`.
- Current C6 baseline: `96fae2712888489dccd4cdfbf60e290879e8f6a5`; baseline tree recorded by the handoff/manifest: `63f8ac3052856074c8af39475aa58fc6d8986ba5`.
- Reused PR195 diagnostic source: `dc3fd535c3a78a9332abbdd6cb2cb267d6caa5ee`; source tree recorded by the handoff/manifest: `892eebf1e6da96253a40e42b37cb7b1fd3b33e62`.
- Settled contract: `agent://PlanWindowsAllFeatureCapture`.
- Implementation handoff: `local://u62f-c6-workspace-all-features-implementation-handoff.json`.
- Supplied eleven-file patch: `local://u62f-c6-workspace-all-features-diagnostic.patch`.
- Patch identity recorded by that handoff: 164,994 bytes; SHA256 `6462f9c5e94b92a5d18f3dfce15f722cbfbd2e92fcb482c838e403216788bf9c`. This review consumed the patch; it did not independently recompute its checksum.

The review used the baseline working diff, relevant full source context, the new local files and supplied patch, PR195 source comparison, and the settled plan. Read-only Git diffs showed six modified tracked paths against C6. The supplied patch additionally contains the five new `.cyril-u62f` paths below; the diagnostic directory listing contained those five files and no transplanted `probe.ps1`. No diagnostic commit/tree identity is asserted for this uncommitted working delta.

## Exact eleven-file diagnostic scope

1. `.cyril-u62f/probe.rs`
2. `crates/cyril-memory/src/paths.rs`
3. `crates/cyril-memory/src/runtime.rs`
4. `crates/cyril-memory/src/store.rs`
5. `crates/cyril/src/bin/cyril-memory-runtime.rs`
6. `crates/cyril/tests/memory_runtime.rs`
7. `.github/workflows/ci.yml`
8. `.cyril-u62f/source-pins.json`
9. `.cyril-u62f/route.md`
10. `.cyril-u62f/evidence.md`
11. `.cyril-u62f/review-decisions.md`

The scope is the diagnostic overlay against exact C6, not the full C6-to-main diff. Cargo manifests/lock/toolchain/configuration, C6 production work, tracker changes, general instruction/docs changes, and the historical native/VM `probe.ps1` driver are not part of this overlay.

## Source findings and conformance observations

### Reused observation machinery

- The Git comparison against PR195 showed no changes to `paths.rs`, `runtime.rs`, `store.rs`, or the companion entrypoint. The current `probe.rs` was read and compared with the pinned PR195 Git-object contents; no changed helper content was identified.
- The helper retains its existing historical edit-base identity, `cyril-u62f-phase-v1` identifier, identity tuple, 256-record cap, 2,048-byte detail cap, file-error reporting, and cap marker (`probe.rs:10-13,64-131`). The runtime retains the 16-connection observation cap (`runtime.rs:226-263`). No new tracing framework, supervisor, retry mechanism, IPC implementation, or production lifecycle repair was introduced.
- Existing five-second startup, 250ms connect/request, and 20ms readiness polling sleep budgets remain in place (`memory_runtime.rs:341-342,456-550`). Existing assertions and readiness outcomes remain intact. Manual executor construction preserves the previous fixture current-thread and companion multi-thread flavors; the fixture still delegates its result to the original `Termination::report` (`memory_runtime.rs:297-337`; companion entrypoint `:7-44`). No source-visible error-to-success conversion or changed failure-path wait was identified.
- Root and child wrappers retain the existing ownership/drop sequencing. Readiness typed-error emission remains deferred outside polling; the new linkage does not add polling-loop writes.

### Fixture test and launch linkage

- `memory_runtime.rs:23-39` contains one shared `observe_owned_child` helper. Its early enablement guard precedes both `NEXTEST_TEST_NAME` lookup and atomic ordinal allocation.
- Linkage detail is `(actual test name or explicit <unavailable>, 1-based parent-local launch ordinal, child PID, expected companion executable)`; the unchanged phase header supplies parent PID.
- The primary successful spawn calls the helper at `:434`; the direct AlreadyRunning second-companion spawn calls it at `:913`. Both therefore use the same process-local ordinal sequence. The primary record is enriched/replaced rather than duplicated.
- `test_identity` is emitted before executor construction and only when enabled (`:297-305`). All Windows fixture entrypoints use the observed executor. Existing companion `child_entry` and library `child_configuration` identities provide the child-PID join; there are no added child linkage environment inputs.
- Consumer-side handling was read: hosted finalization parses executable identity records, while the complete phase logs are uploaded unchanged. The separate test/launch events are not silently discarded by an identity-only upload projection.

### Workflow invocation, scope, and outcome preservation

- `.github/workflows/ci.yml:201-206` retains exactly `cargo nextest run --workspace --all-features`. No retry, output/filter, profile/thread, timeout, wrapper, or `continue-on-error` behavior was added.
- Runner labels, matrix `fail-fast: false`, and workflow concurrency/cancellation remain unchanged (`:20-22,93-99`). The Windows-only installer pin is nextest `0.9.148` (`:108-111`), checked during preparation (`:142-144`); other installer behavior remains baseline behavior.
- Preparation is Windows-only; the active probe inputs are scoped to the existing test step. Non-Windows receives probe value `0`, which does not enable the helper. Package/default is restored to its uninstrumented form (`:350-380`); there is no duplicate capture envelope.
- Neither controlled-fault input is defined. Preparation rejects inherited `CYRIL_U62F_POST_BIND_DELAY_500MS`, `CYRIL_U62F_READY_AUTH_MISMATCH`, profile/thread overrides, and probe/source overrides (`:126-133`). Existing guarded fault code is retained inactive rather than rewritten or invoked.

### Failure evidence and identity consumer

- Finalization and upload are separate steps, each with Windows-qualified `always()`, immediately after the original tests and before doc tests (`ci.yml:208-274`). Upload does not depend on finalization succeeding.
- Finalization saves preparation/test-step outcomes before parsing executable identities (`:217-225`). The original test-step outcome remains authoritative, and no native exit code is invented.
- The finalizer's consuming dispatch admits `fixture_executor`, `child_entry`, and `child_configuration` and validates the expected fixture/companion basename (`:229-260`). It is compatible with the unchanged helper serialization. Preparation requires `CYRIL_MEMORY_PROBE_SOURCE` absent, preserving the expected `None` tuple field.
- The producing callsites were traced: fixture executor (`memory_runtime.rs:298`), companion entry (`cyril-memory-runtime.rs:8`), and configuration (`cyril-memory/src/runtime.rs:54`). The configuration producer is called by the companion entrypoint. The other located in-process `run_runtime` test consumer is Unix-only and introduces no additional Windows identity role.
- Executable hashes are exact-path, post-execution hashes from retained identities, not cached fixture glob matches promoted to execution evidence.
- Actual checkout/tree, PR head/base, run/attempt/job, toolchain, OS/machine and actual image provenance are retained. C6/PR195 pins and failed image `windows-2025-vs2026 / 20261018.275.1` are distinguished from the new run's actual allocation (`:134-199`).

### Narrow evidence and sensitive-data boundary

- Source enumeration retains hashes rather than copying source/configuration contents (`ci.yml:145-155`). The artifact is restricted to the fresh evidence root: phase files, selected provenance, source-pin manifest, outcomes, and executable hashes. It does not upload runtime roots, databases, binaries, credentials, or an environment dump.
- Upload retains missing-files error, 14-day retention, no overwrite, and hidden-file exclusion (`:263-274`).
- Probe payloads and relevant error types were read. Readiness records retain typed categories/codes rather than private error payloads. First-request success is mapped to unit rather than logging decoded requests. No credential/token or wire-payload capture was identified in the reviewed delta.

### Current versus historical records

- `source-pins.json:3-61,149-218` separates current C6 composition from nested historical PR195/d6 provenance; current qualification is explicitly pending. Recorded per-file SHA256 values were treated as manifest/handoff identity records, not newly computed qualification evidence.
- `route.md:3-56` establishes the active eleven-file overlay, excludes `probe.ps1`, and rejects historical qualification reuse or a failed-CI waiver.
- `evidence.md:3-43` distinguishes the current workspace/all-features failure from PR195's older-image package/default non-reproduction. Historical material has an explicit boundary; current attribution remains **UNATTRIBUTED**.
- `review-decisions.md:3-23` limits historical review applicability and requires fresh current qualification. No contradictory active permanent-fix, current qualification, waiver, or issue-completion claim was identified.

## Review coverage and qualification limits

This is the combined review already completed by the main source reviewer plus read-only workflow and records scouts. The subsequent workflow confirmation did not create another review round or change the verdict.

Performed: source/file/patch reading, read-only Git diff/show comparison, source tracing and independent read-only workflow/record review.

Not performed: source edits, Git mutations, builds, tests, compiler/Clippy gates, linters, formatter commands, YAML/PowerShell parser execution, smoke runs, VM/native/hosted runtime execution, authentication changes, commits/pushes or PR publication.

Consequently:

- Static scheduling establishes that finalization/upload remain eligible after ordinary step failure; it does not prove execution or survival after runner loss, cancellation/forced termination, or upload failure.
- Finalizer success alone does not establish complete phase evidence. Caps, file errors, truncation, missing identities/linkage/terminal phases, or unexecuted fail-fast cases still limit interpretation as documented.
- Historical PR195 Linux/parser/hosted PASS results were not used as current C6 qualification. Its older package/default green capture is non-reproduction, not a fix or CI waiver.
- Main-owned current syntax/build/format/lint/test qualification and any separately authorized hosted capture remain outside this review. The original Windows readiness cause remains **UNATTRIBUTED**.

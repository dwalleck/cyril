# S2HB assembled production reconstruction

## Scope and source pin

This is a production-code reconstruction only. I did not use a design/spec/plan/checkpoint/conformance document or tracker/skill artifact.

* Worktree inspected: `/home/dwalleck/repos/cyril-wt-feat-cyril-s2hb-gather`
* Discovered branch: `feat/cyril-s2hb-gather` (the worktree `HEAD` file points at that ref).
* Pinned upstream: `d6955ab73090d0ae80b2aa08c8cb32679acb430a` (also the local `origin/main` ref).
* Current committed HEAD: `86038a92102ee69c5b183374ff92977b8b9eac8c`.
* The worktree reflog records `d6955ab7 -> e4870f73 -> 01e436f0 -> 86038a92`; the target commit metadata says the final amend retained the production/test/dependency tree from `e4870f73`. Thus the committed production delta from the pin is the native review leaf and hidden CLI wiring introduced by `e4870f73`; the current assembled state additionally includes the dirty diagnostics work described below.
* No build, test, formatter, linter, package-manager, or mutating Git command was run. The current dirty/untracked files were read directly and compared to the committed `e4870f73` source snapshots where available.
* Dirty-file content hashes were not exposed by the permitted read-only source tools. The assembled state is pinned by the current HEAD/upstream hashes above plus the explicit dirty/untracked path list and line citations below.

The base checkout at `d6955ab7` has no `cyril-review` workspace member or lockfile package, no `cyril-review` dependency in `cyril`, and no `crtool` dispatch in `cyril/src/main.rs` (base `Cargo.toml`, base `crates/cyril/Cargo.toml`, and base `crates/cyril/src/main.rs` contain no such entries). The assembled tree adds those seams. The target index contains the committed review leaf files but not the `src/diagnostics/` subtree or `tests/diagnostics.rs`; those diagnostics files are therefore treated as current untracked work, while the clock/lib/manifest/example/evidence differences from `e4870f73` are tracked worktree differences.

## Assembled changed-set census

### Committed in `d6955ab7..86038a92`

| Path | State | Observed role |
|---|---|---|
| `Cargo.toml` | committed | Adds `crates/cyril-review` to the workspace member list (`Cargo.toml:1-8`). |
| `Cargo.lock` | committed | Records the new `cyril-review` package and adds it to `cyril`'s resolved dependencies (`Cargo.lock:678-771`). |
| `crates/cyril/Cargo.toml` | committed | Adds the binary crate's path dependency on `cyril-review` (`crates/cyril/Cargo.toml:18-30`). |
| `crates/cyril/src/main.rs` | committed | Adds an optional Clap subcommand and exits into the mechanical CLI before ordinary TUI startup (`crates/cyril/src/main.rs:1-39`). |
| `crates/cyril/src/crtool.rs` | committed/new | Adapts `crtool gather` and `crtool facts` arguments to the review leaf, then maps output/errors to process stdout/stderr and exit codes (`crates/cyril/src/crtool.rs:7-63`). |
| `crates/cyril-review/Cargo.toml` | committed, then dirty description edit | Declares the standalone review package and its serde/regex/thiserror/test dependencies (`crates/cyril-review/Cargo.toml:1-19`). The current description includes diagnostics; the committed `e4870f73` description named gather and facts only. |
| `crates/cyril-review/src/lib.rs` | committed, then dirty diagnostics extension | Owns the public façade, `StepOutput`, common `Result`, and review error boundary (`crates/cyril-review/src/lib.rs:1-15`, `:22-45`). |
| `crates/cyril-review/src/clock.rs` | committed, then dirty diagnostics extension | Owns the time adapter; the current trait includes elapsed-time measurement in addition to gather timestamps (`crates/cyril-review/src/clock.rs:4-27`). |
| `crates/cyril-review/src/run.rs` | committed | Owns absolute run anchors, manifest/file records, layout, durable JSON/text/binary I/O, and manifest validation (`crates/cyril-review/src/run.rs:10-60`, `:82-121`, `:137-221`). |
| `crates/cyril-review/src/git.rs` | committed | Owns the explicit-argv Git subprocess adapter, status policy, raw path conversion, target resolution, and argument rendering (`crates/cyril-review/src/git.rs:9-76`, `:115-135`). |
| `crates/cyril-review/src/gather.rs` | committed | Orchestrates a scoped Git diff into durable patches/manifest artifacts and invokes facts construction (`crates/cyril-review/src/gather.rs:13-145`). |
| `crates/cyril-review/src/facts.rs` | committed | Extracts changed symbols, textual usages, changed documents, and bounded facts pages from a gathered run (`crates/cyril-review/src/facts.rs:49-128`, `:135-183`, `:279-378`, `:394-485`). |
| `crates/cyril-review/examples/parity_driver.rs` | committed, then dirty diagnostics/fixture extension | Verification-only public consumer; the current file adds direct diagnostics driving and native child-lifecycle fixtures (`crates/cyril-review/examples/parity_driver.rs:13-16`, `:167-176`, `:372-498`). |
| `crates/cyril-review/tests/evidence.rs` | committed, then dirty clock/platform adaptation | Public gather/facts consumer using temporary real Git repositories and artifact assertions (`crates/cyril-review/tests/evidence.rs:94-126`, `:182-330`). |

### Current dirty/untracked diagnostics assembly

| Path | State | Observed role |
|---|---|---|
| `crates/cyril-review/Cargo.toml` | tracked dirty | Description-only expansion from gather/facts to gather/facts/diagnostics; no new dependency is present (`crates/cyril-review/Cargo.toml:1-19`). |
| `crates/cyril-review/src/clock.rs` | tracked dirty | Extends `ReviewClock` with `diagnostics_elapsed(Instant)` and implements it with monotonic `Instant::elapsed()` (`crates/cyril-review/src/clock.rs:4-27`). |
| `crates/cyril-review/src/lib.rs` | tracked dirty | Adds the diagnostics module/re-exports and a typed `DiagnosticsError` wrapped by `ReviewError::Diagnostics` (`crates/cyril-review/src/lib.rs:1-15`, `:80-115`). |
| `crates/cyril-review/src/diagnostics/mod.rs` | untracked production Rust | Public diagnostics operation: validate a stamped run, prepare/cancel/execute a command, summarize output, and persist raw/report facts (`crates/cyril-review/src/diagnostics/mod.rs:1-15`, `:17-83`, `:84-167`). |
| `crates/cyril-review/src/diagnostics/command.rs` | untracked production Rust | Private native command-construction adapter with NUL/quote validation, POSIX tokenization, Windows executable selection/raw-tail handling, working-directory binding, and environment cleanup (`crates/cyril-review/src/diagnostics/command.rs:6-46`, `:58-108`, `:111-216`). |
| `crates/cyril-review/src/diagnostics/process.rs` | untracked production Rust | Private child-process/capture adapter with unique temporary files, poll/cancel/timeout ordering, kill/reap, exit classification, bounded stdout/stderr snapshots, and cleanup (`crates/cyril-review/src/diagnostics/process.rs:11-43`, `:46-112`, `:148-209`, `:210-276`). |
| `crates/cyril-review/examples/parity_driver.rs` | tracked dirty | Adds the verification caller for diagnostics, fixed/system elapsed clocks, pre/live/after-exit cancellation modes, and native fixture handshakes (`crates/cyril-review/examples/parity_driver.rs:372-498`). |
| `crates/cyril-review/tests/evidence.rs` | tracked dirty | Adds the elapsed-clock method required by the expanded trait and narrows the raw-byte collision fixture to Linux (`crates/cyril-review/tests/evidence.rs:7-27`, `:520-525`). |
| `crates/cyril-review/tests/diagnostics.rs` | untracked test Rust | Public-operation integration matrix for successful/failed/empty output, filtering limits, manifest/command refusal, precancel, timeout, live cancellation, raw bytes, and metadata preservation (`crates/cyril-review/tests/diagnostics.rs:1-81`, `:243-300`, `:351-434`, `:480-599`, `:645-703`). |

There is no current diagnostics subcommand in the production hidden CLI: `Step` still contains only `Gather` and `Facts` (`crates/cyril/src/crtool.rs:18-28`). The current diagnostics API is instead reached by the example driver and integration tests. Whether that lack of CLI wiring is intentional or pending is undetermined from production code.

## Module interfaces, invariants, and responsibilities

### Packaging and hidden CLI

* **Workspace/package manifests.** The root manifest owns package membership; `cyril` owns the binary-to-leaf path dependency; `cyril-review` owns its narrow library dependencies. These are wiring/configuration owners, not behavior owners (`Cargo.toml:1-8`; `crates/cyril/Cargo.toml:18-30`; `crates/cyril-review/Cargo.toml:1-19`).
* **`cyril/src/main.rs`.** `Cli` exposes an optional `crtool::Command`. If present, `main` calls `command.run()` and exits before logging, config loading, runtime creation, bridge startup, or terminal setup (`crates/cyril/src/main.rs:15-39`). This is protected-parent wiring only: it adds a dispatch seam but does not put review behavior in the TUI startup parent.
* **`cyril/src/crtool.rs`.** The interface is the hidden Clap tree: `crtool gather <rundir> <target> [scope]` and `crtool facts <rundir>` (`crates/cyril/src/crtool.rs:7-28`). `run` binds the workspace to `"."`, constructs `ReviewRun`, calls the public leaf operation with `SystemReviewClock`, writes owned stdout bytes, and maps errors through `ReviewError::exit_code` (`crates/cyril/src/crtool.rs:30-50`). It is a concrete process/CLI adapter, not a review algorithm.
* **Unchanged upstream shell seam.** `cyril-core` already defines safe `CrtoolPrefix` shell spelling for the hidden command, appending `" crtool` after path/dialect validation (`crates/cyril-core/src/review/mod.rs:1-3`, `:40-90`). It is an adapter from host-shell invocation to the new binary command, but it does not depend on `cyril-review` Rust APIs.

### Shared leaf façade and durable run contract

* **`cyril-review/src/lib.rs`.** The public interface is deliberately small: `ReviewRun`, `ReviewClock`/`SystemReviewClock`, `gather`, `facts`, `StepOutput`, the shared result/error types, and now diagnostics types/functions (`crates/cyril-review/src/lib.rs:1-15`). `StepOutput` owns stdout bytes and exposes only a byte slice (`crates/cyril-review/src/lib.rs:24-42`). `ReviewError::exit_code` reserves 3 for `EmptyDiff` and maps all other failures to 2 (`crates/cyril-review/src/lib.rs:88-98`). The current dirty extension adds `DiagnosticsError::{InvalidCommand,CannotStart,Lifecycle}` and wraps it as one common review error (`crates/cyril-review/src/lib.rs:80-115`).
* **`run.rs`.** `ReviewRun::new` accepts workspace and run-directory paths and stores lexically absolute, normalized anchors; `workspace()` and `directory()` are the only public path accessors (`crates/cyril-review/src/run.rs:59-105`). The private `Manifest`/`ManifestFile` schema is the durable contract shared by gather, facts, and diagnostics. A file can retain `_raw_path_bytes`, and `path_bytes()` uses those bytes when present (`crates/cyril-review/src/run.rs:28-57`). Run layout creates fixed artifact directories (`crates/cyril-review/src/run.rs:119-135`). JSON writes go through a sibling temporary path and rename (`crates/cyril-review/src/run.rs:158-172`). Reads validate an exact nonempty `crtool_version` and reject incoherent raw path bytes before operations proceed (`crates/cyril-review/src/run.rs:179-221`).
* **`clock.rs`.** `ReviewClock` is the current two-method seam: wall-clock UTC for gathered manifests and monotonic elapsed time for diagnostics (`crates/cyril-review/src/clock.rs:4-8`). `SystemReviewClock` supplies the host implementation and keeps the two time domains separate (`crates/cyril-review/src/clock.rs:10-27`). Fixed clocks in evidence/diagnostics/parity callers are additional concrete adapters, so this is a real test seam rather than a production trait with multiple production backends (`crates/cyril-review/tests/evidence.rs:15-27`; `crates/cyril-review/tests/diagnostics.rs:17-27`; `crates/cyril-review/examples/parity_driver.rs:372-394`).

### Git capture and gather

* **`git.rs`.** This is the only Git process adapter. It uses `Command::new("git")`, explicit arguments, and `ReviewRun.workspace()` as `current_dir` (`crates/cyril-review/src/git.rs:27-35`). Required operations fail with `GitFailure`; `grep` intentionally accepts status 0 and no-match status 1 (`crates/cyril-review/src/git.rs:38-59`). Unix raw filename bytes become `OsStr` without lossy conversion; non-Unix rejects non-UTF-8 path bytes (`crates/cyril-review/src/git.rs:9-25`). `resolve_target("auto")` probes upstream/main/master, checks tracked dirtiness, and chooses a range, merge-base, HEAD, or HEAD~1 with a warning as applicable (`crates/cyril-review/src/git.rs:64-113`). There is no `Git` trait or injected fake backend in production; tests use real Git repositories (`crates/cyril-review/tests/evidence.rs:29-61`).
* **`gather.rs`.** `gather(run,target,scope,clock)` is the operation owner (`crates/cyril-review/src/gather.rs:13-21`). On a fresh run it resolves the target, warns when the diff head differs from current HEAD, obtains full/status/numstat/path patches, and writes `diff.patch`, per-file patches, `changed-files.txt`, and a stamped manifest (`crates/cyril-review/src/gather.rs:34-145`). It rejects whitespace-only diffs as `EmptyDiff`; a pre-existing manifest is reusable only when requested target and normalized scope match, otherwise it returns `ExistingRun` (`crates/cyril-review/src/gather.rs:22-53`). `split_scope` uses Rust whitespace splitting and defaults an empty scope to `.` (`crates/cyril-review/src/gather.rs:147-155`); `diff_args` centralizes `--no-renames`, target, `--`, and scope ordering (`crates/cyril-review/src/gather.rs:158-170`). NUL-framed status/numstat parsers preserve Git path identity and distinguish binary counts (`crates/cyril-review/src/gather.rs:173-230`).

### Facts analysis

* **`facts.rs`.** `facts` is the public rebuild operation; `build_facts` is the private shared builder used by both public facts and fresh gather (`crates/cyril-review/src/facts.rs:49-64`). It reads the stamped manifest, detects added/changed definitions from patches for Rust/Python/JS/TS/Go-style patterns, maps symbols to language-specific `git grep` globs, and borrows manifest file identity (`crates/cyril-review/src/facts.rs:135-183`; `:164-277`). Usages are NUL-framed Git grep results, skip `.code-review` and the symbol's own definition, and retain replacement-decoded display text (`crates/cyril-review/src/facts.rs:279-350`). Changed documentation is selected by byte suffix and stored with path/size metadata (`crates/cyril-review/src/facts.rs:352-391`).
* Its output policy is also local: common names are marked ambiguous, unused symbols are grouped, normal symbols include bounded usages, and pages are packed by Unicode scalar count under a 20,000-character budget (`crates/cyril-review/src/facts.rs:13-19`; `:394-453`). `write_pages` removes stale numbered pages and writes `facts/usages-N.txt` with a header and page numbering (`crates/cyril-review/src/facts.rs:455-510`). The manifest's flattened metadata is updated in place, preserving unknown root/file/facts metadata through serde flattening (`crates/cyril-review/src/facts.rs:101-128`; `crates/cyril-review/src/run.rs:10-47`).

### Diagnostics assembly (current dirty work)

* **`diagnostics/mod.rs`.** The public input is a `ReviewRun`, command text, timeout options, a cloneable atomic `Cancellation`, and a `ReviewClock`; the result exposes outcome, whether a child started, and `StepOutput` (`crates/cyril-review/src/diagnostics/mod.rs:17-83`). Pre-cancel returns `Cancelled` without reading/writing the run; after manifest validation and command preparation, cancellation is checked again before launch (`crates/cyril-review/src/diagnostics/mod.rs:84-111`). On launch it delegates child lifecycle to `process::capture`, uses the clock for rounded elapsed seconds, matches changed paths after slash normalization, keeps the first 200 matching lines and last 15 nonempty lines, then writes `diagnostics-raw.txt`, `diagnostics.txt`, and two diagnostics fields under the existing `facts` object (`crates/cyril-review/src/diagnostics/mod.rs:112-167`, `:169-207`). It therefore owns orchestration/report formatting/durable integration, not command parsing or kill/reap mechanics.
* **`diagnostics/command.rs`.** `prepare` is the internal command adapter. It rejects NUL, creates a native `Command`, sets the run workspace as current directory, and removes empty `CARGO_`/`RUST` environment variables (`crates/cyril-review/src/diagnostics/command.rs:6-35`). POSIX tokenization handles quotes/backslashes but leaves shell metacharacters as literal argv data (`crates/cyril-review/src/diagnostics/command.rs:46-108`). The Windows path selects an executable without introducing a shell wrapper and passes the remaining text as a raw command tail (`crates/cyril-review/src/diagnostics/command.rs:111-159`). The cfg-specific implementations are concrete platform adapters, not runtime-swappable trait implementations.
* **`diagnostics/process.rs`.** `capture` owns temporary stdout/stderr files, attaches them to the child, polls with a 10 ms interval, and always cleans the capture paths (`crates/cyril-review/src/diagnostics/process.rs:11-86`). `terminal_decision` gives an observed exit precedence over live cancellation and timeout; cancellation/timeout terminate and reap the child, with a one-second reap deadline (`crates/cyril-review/src/diagnostics/process.rs:88-163`). Exit classification preserves Unix signal numbers as negative values and Windows unsigned exit bits (`crates/cyril-review/src/diagnostics/process.rs:210-236`). The snapshot samples both file lengths before reading, appends a newline between stdout and stderr, and bounds reads to the sampled lengths (`crates/cyril-review/src/diagnostics/process.rs:238-276`).

## Responsibility clusters and separating rule

1. **Process/package wiring:** root and crate manifests, `cyril/src/main.rs`, and `crtool.rs`. They choose whether to enter the leaf, parse process arguments, write process streams, and map exit status; they do not interpret diffs or child diagnostics (`Cargo.toml:1-8`; `crates/cyril/src/main.rs:34-39`; `crates/cyril/src/crtool.rs:30-63`).
2. **Run/artifact contract:** `lib.rs` error/output façade and `run.rs` path, manifest, stamp, layout, and file I/O. This cluster decides what a valid durable run is and how errors cross the public seam (`crates/cyril-review/src/lib.rs:22-138`; `crates/cyril-review/src/run.rs:59-221`).
3. **Time adapters:** `clock.rs`. The separating rule is “obtain time through the injected clock,” with wall UTC and monotonic elapsed values kept distinct (`crates/cyril-review/src/clock.rs:4-27`).
4. **Git capture and diff materialization:** `git.rs` owns subprocess/status/raw-path semantics; `gather.rs` owns the multi-command diff-to-artifact workflow (`crates/cyril-review/src/git.rs:27-113`; `crates/cyril-review/src/gather.rs:13-230`).
5. **Static source facts:** `facts.rs` owns changed-definition detection, textual usages, document lookup, rendering, and page limits. It consumes the run/Git contracts and is not responsible for launching arbitrary diagnostics (`crates/cyril-review/src/facts.rs:55-128`; `:279-510`).
6. **Dynamic diagnostics:** `diagnostics/mod.rs` owns operation/report/persistence; `diagnostics/command.rs` owns native argv construction; `diagnostics/process.rs` owns child lifecycle and lossless capture. The separating rule is orchestration versus command construction versus process lifecycle (`crates/cyril-review/src/diagnostics/mod.rs:74-167`; `command.rs:6-216`; `process.rs:15-276`).
7. **Verification-only consumers:** `tests/evidence.rs`, untracked `tests/diagnostics.rs`, and the expanded `examples/parity_driver.rs` construct real temporary repositories/processes and exercise the public seams. They are not production responsibility owners (`crates/cyril-review/tests/evidence.rs:182-330`; `crates/cyril-review/tests/diagnostics.rs:243-300`; `crates/cyril-review/examples/parity_driver.rs:167-176`).

## Dependency directions and concrete adapters

* Workspace -> `cyril-review` package -> its std/serde/serde_json/regex/thiserror dependencies (`Cargo.toml:1-8`; `crates/cyril-review/Cargo.toml:10-19`).
* `cyril` binary -> `cyril_review::{ReviewRun,gather,facts,SystemReviewClock,StepOutput}` through `crtool.rs`; `main.rs` only selects that path before ordinary startup (`crates/cyril/src/crtool.rs:4-50`; `crates/cyril/src/main.rs:15-39`).
* `cyril-review::lib` -> `clock`, `run`, `gather`, `facts`, and current `diagnostics`; the façade re-exports public types/functions, while private helpers centralize errors (`crates/cyril-review/src/lib.rs:1-15`, `:118-138`).
* `gather` -> `git`, `run`, `facts`, and `ReviewClock`; `facts` -> `git` and `run`; diagnostics -> `command`, `process`, `run`, and `ReviewClock` (`gather.rs:1-8`; `facts.rs:1-8`; `diagnostics/mod.rs:1-8`). The direction is leaf operation -> shared contract/adapters, never the reverse.
* `git.rs` -> the host `git` executable via explicit argv and run workspace; it does not call the shell (`git.rs:27-35`). `command.rs` -> native `std::process::Command`; `process.rs` -> `std::process::Child` plus private temporary files (`command.rs:46-159`; `process.rs:15-43`).
* Concrete adapters are `SystemReviewClock` (`clock.rs:12-27`), fixed test/parity clocks (`tests/evidence.rs:15-27`; `tests/diagnostics.rs:17-27`; `examples/parity_driver.rs:372-394`), the `crtool` Clap/process adapter (`crtool.rs:7-63`), `git` subprocess functions (`git.rs:27-76`), cfg-specific native command builders (`command.rs:45-216`), and the child/capture adapter (`process.rs:15-276`).
* The unchanged `CrtoolPrefix` is a separate shell adapter: host shell -> executable path + hidden `crtool` token; it has no Rust dependency edge into the leaf (`crates/cyril-core/src/review/mod.rs:40-90`).

## Pass-through modules and hypothetical seams

* **Thin by role, still useful:** `crtool.rs` mostly forwards parsed arguments and output, but earns its seam by preventing review code from entering `main` and by owning process-level I/O/exit classification (`crates/cyril/src/crtool.rs:30-63`). The root and package manifests are pure wiring, not behavior modules.
* **Façade:** `cyril-review/src/lib.rs` re-exports operations and centralizes error/output contracts. Its re-export lines are pass-through; its `StepOutput`, `ReviewError::exit_code`, diagnostics error wrapping, and constructors are not mere copies (`lib.rs:22-138`).
* **No production Git trait:** `git.rs` is a concrete adapter with private functions. A fake/in-memory Git seam is hypothetical only; current tests deliberately use real Git (`git.rs:27-76`; `tests/evidence.rs:29-61`).
* **No production child-process trait:** `diagnostics/process.rs` directly owns `Command`/`Child`/capture files. The command and process modules are internal seams for the diagnostics implementation, not swappable adapters exposed to callers (`process.rs:15-43`; `command.rs:6-35`).
* **Real clock seam:** `ReviewClock` has the production system adapter and several fixed/controlled verification adapters. The elapsed extension is a real interface expansion because diagnostics consumes it; it is not just a forwarding alias (`clock.rs:4-27`; `examples/parity_driver.rs:379-394`).
* **Observed seam asymmetry:** diagnostics is a public library operation and has a direct verification driver, but `crtool.rs` still exposes only gather/facts. The production code does not establish whether diagnostics is intended to be CLI-reachable yet (`crtool.rs:18-28`; `examples/parity_driver.rs:167-176`).

## Protected-parent growth

* `cyril/src/main.rs` grows only by adding the `crtool` module, optional subcommand field, and early dispatch. The new behavior is wiring; the protected TUI startup body remains after the early return (`main.rs:1-39`).
* `cyril-review/src/lib.rs` grows by adding a module/re-export and a `ReviewError` variant, plus the typed diagnostics error family. The module/re-export portion is wiring; the common error family is a necessary new public responsibility for classifying command-start/lifecycle failures (`lib.rs:1-15`, `:80-115`).
* `ReviewClock` grows from a gather timestamp method in committed e487 source to a two-method interface. This is not parent-only wiring: the new elapsed contract is consumed by diagnostics and implemented by system/fixed adapters (`clock.rs:4-27`; `diagnostics/mod.rs:112-116`).
* `crtool.rs` is a new adapter rather than protected-parent growth; its current `Step` enum demonstrates that only Gather/Facts are wired (`crtool.rs:18-28`).
* The workspace/package manifest changes are membership/dependency wiring only (`Cargo.toml:1-8`; `crates/cyril/Cargo.toml:18-30`). The current `cyril-review` description edit is metadata-only (`crates/cyril-review/Cargo.toml:1-7`).
* The parity driver and evidence test additions are verification wiring/fixtures, not production-parent responsibility (`examples/parity_driver.rs:167-176`; `tests/evidence.rs:15-27`).

## Tests that reach past an interface

These were inspected only after deriving the production model.

### Direct private-helper tests

The in-module unit tests bypass the public leaf interface to pin implementation details: UTC arithmetic (`clock.rs:63-75`), Unicode page packing and page-file writes (`facts.rs:516-537`), line filtering (`diagnostics/mod.rs:214-231`), POSIX tokenization and environment-name classification (`diagnostics/command.rs:218-277`), and capture-file permissions, bounded reads, terminal precedence, reap deadlines, cleanup, and exit classification (`diagnostics/process.rs:278-423`). These are implementation-level fences, not caller contract tests.

### Public API tests that cross the durable artifact protocol

* `tests/evidence.rs` drives `ReviewRun`, `gather`, and `facts` through public functions but reads `manifest.json`, patch files, changed-files text, symbol JSON, and facts pages directly (`tests/evidence.rs:94-126`; `:182-330`). It tests idempotent reuse/identity, auto target selection, Unicode/raw path identity, stamp refusal, malformed schema refusal, unknown metadata preservation, and fatal-versus-no-match Git grep behavior (`tests/evidence.rs:334-525`; `:699-835`; `:869-919`). This reaches past the Rust interface into the on-disk manifest/artifact protocol, which is the actual seam between gather and later steps.
* `tests/diagnostics.rs` constructs a valid stamped manifest and sentinel facts files directly, invokes only the public `diagnostics` operation, then reads raw/report files and rewritten manifest metadata (`tests/diagnostics.rs:35-81`; `:243-300`). It covers clean/nonzero/empty output, invalid UTF-8 and NUL bytes, path matching caps and tail retention, malformed/missing/stale manifests, malformed commands, precancel, timeout, and live cancellation (`tests/diagnostics.rs:283-434`; `:480-599`; `:645-703`). It is a durable-artifact consumer test rather than a private-helper test.
* `examples/parity_driver.rs` is a public seam consumer. Its ordinary mode drives gather/facts with a fixed clock; its current diagnostics mode drives the library with fixed/system elapsed clocks and controlled cancellation/fixture child processes (`examples/parity_driver.rs:116-140`; `:167-176`; `:372-498`).
* The unchanged `cyril-core` prefix example tests the process-level shell-to-hidden-command seam, including actual execution of the `crtool` token (`crates/cyril-core/src/review/mod.rs:197-220`, `:370-380`).

## Candidate structural tradeoffs (observations, not judgments)

* **Diagnostics CLI asymmetry:** the library and verification driver expose diagnostics, while the hidden production CLI has no diagnostics step. The production seam and intended reachability are undetermined from source (`crtool.rs:18-28`; `examples/parity_driver.rs:167-176`).
* **Clock interface breadth:** one `ReviewClock` now serves both wall-clock gather stamps and monotonic diagnostics elapsed time. This is exercised by independent fixed adapters, but the interface has two time responsibilities (`clock.rs:4-27`; `tests/diagnostics.rs:17-27`).
* **Durable schema versus typed public API:** `Manifest`/`ManifestFile` remain private while tests and sibling operations directly consume their JSON/artifact representation; serde flattening deliberately preserves later-step metadata (`run.rs:10-57`; `facts.rs:101-128`). That makes the files a broad internal protocol even though the Rust façade is small.
* **Textual path matching:** diagnostics normalizes slashes and uses substring matching against changed paths, then caps retained matches while counting all matches (`diagnostics/mod.rs:169-207`). The integration test pins the resulting false-positive/retention surface rather than a structured diagnostic parser (`tests/diagnostics.rs:369-432`).
* **Temp-file capture:** diagnostics captures stdout/stderr to private temporary files, samples lengths before reading, then joins the streams with one newline (`process.rs:15-43`, `:238-276`). This is a concrete lifecycle/capture choice; no pipe/async-process abstraction is present.
* **Verification-only side effects:** `DiagnosticsClock` can cancel from inside elapsed sampling to probe exit-versus-cancel ordering, and the parity driver owns child fixture handshakes. Those are test adapters, not production clock/process responsibilities (`examples/parity_driver.rs:372-394`, `:396-498`).

## Reconstructed ledger: `path | responsibility | interface owner`

| Path | Responsibility | Interface owner |
|---|---|---|
| `Cargo.toml` | Workspace membership for review leaf | Cargo workspace manifest |
| `Cargo.lock` | Resolved package/dependency graph for review leaf | Cargo lockfile |
| `crates/cyril/Cargo.toml` | Binary-to-review package dependency | `cyril` package manifest |
| `crates/cyril/src/main.rs` | Early hidden-subcommand dispatch, preserving ordinary TUI startup | `Cli` + `main` |
| `crates/cyril/src/crtool.rs` | Hidden process CLI adapter for gather/facts, stdout/stderr and exit mapping | `Command`, `Step`, `Command::run` |
| `crates/cyril-review/Cargo.toml` | Review package metadata/dependencies; current dirty description mentions diagnostics | `cyril-review` package manifest |
| `crates/cyril-review/src/lib.rs` | Public leaf façade, output/error boundary; current diagnostics exports/error wrapping | `cyril_review` crate root |
| `crates/cyril-review/src/clock.rs` | Wall UTC and monotonic elapsed time seam | `ReviewClock`, `SystemReviewClock` |
| `crates/cyril-review/src/run.rs` | Absolute run identity, manifest schema, stamp validation, artifact layout/I/O | `ReviewRun` plus private manifest contract |
| `crates/cyril-review/src/git.rs` | Explicit Git process/raw-path/status adapter | Private `git::*` functions consumed by gather/facts |
| `crates/cyril-review/src/gather.rs` | Diff target/scope resolution, patch materialization, manifest creation, gather reuse | `gather` |
| `crates/cyril-review/src/facts.rs` | Changed symbol/document/usage extraction and bounded facts pages | `facts`; private `build_facts` |
| `crates/cyril-review/src/diagnostics/mod.rs` | Current dirty diagnostics orchestration, filtering, reporting, facts metadata persistence | `DiagnosticsOptions`, `Cancellation`, `diagnostics`, `DiagnosticsResult` |
| `crates/cyril-review/src/diagnostics/command.rs` | Current dirty native command parsing/validation and cwd/env preparation | Private `command::prepare` |
| `crates/cyril-review/src/diagnostics/process.rs` | Current dirty child lifecycle, capture, termination, classification, cleanup | Private `process::capture` |
| `crates/cyril-review/examples/parity_driver.rs` | Verification caller/fixture adapter for public gather/facts/diagnostics seams | Example `main`, `diagnostics_driver` |
| `crates/cyril-review/tests/evidence.rs` | Public gather/facts artifact consumer and real-Git boundary | `gather`, `facts`, `ReviewRun` |
| `crates/cyril-review/tests/diagnostics.rs` | Public diagnostics artifact/lifecycle consumer with native fixture commands | `diagnostics`, `DiagnosticsResult`, `Cancellation` |
| `crates/cyril-core/src/review/mod.rs` (unchanged caller) | Safe host-shell spelling of the hidden `cyril crtool` command | `CrtoolPrefix`, `ShellDialect` |

## Uncertainty boundary

The production source establishes the module and dependency map above. It does not establish whether diagnostics is intentionally library-only, whether a later CLI step is expected, or whether the private command/process functions are intended future replacement seams. Those points are recorded as observations only; no design comparison or refactoring judgment is made here.

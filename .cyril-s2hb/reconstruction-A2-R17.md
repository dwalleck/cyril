# R17 isolated production reconstruction

Reviewer: `R17ProductionReconstruction` (read-only scout). Source: uncommitted production over `d6955ab73090d0ae80b2aa08c8cb32679acb430a`; no design/spec/plan/checkpoint access. This is the frozen reconstruction, not an approved-design comparison or acceptance verdict.

## Scope and method

The allowed read-only inventory contains the tracked wiring changes in `Cargo.toml`, `Cargo.lock`, `crates/cyril/Cargo.toml`, and `crates/cyril/src/main.rs`, plus the untracked `crates/cyril-review` leaf and `crates/cyril/src/crtool.rs`. I used bounded source reads, symbol searches, and a read-only working-tree diff. No LSP device was exposed, so the bounded fallback was direct symbol/import/call-site tracing. No builds, tests, formatters, or state-changing commands were run, per the isolation contract.

## 1. Changed module interfaces and responsibilities

### Binary parent and CLI adapter

- **`crates/cyril/src/main.rs` — protected entrypoint/wiring interface.** It declares `crtool` (`main.rs:1-3`), adds an optional Clap subcommand field to `Cli` (`main.rs:17-19`), and, after parsing, dispatches `command.run()` followed by process exit (`main.rs:44-48`). Normal logging, cwd resolution, config loading, runtime construction, bridge startup, and terminal setup remain after that branch (`main.rs:50-58`). The parent therefore presents the binary-level interface and owns ordering, not review behavior.
- **`crates/cyril/src/crtool.rs` — hidden process adapter.** `Command` exposes one hidden `Crtool` subcommand (`crtool.rs:7-15`); `Step` exposes only `Gather { rundir, target, scope }` and `Facts { rundir }` (`crtool.rs:17-28`). `Command::run(self) -> i32` constructs `ReviewRun`, selects `SystemReviewClock` for gather, and calls only public `cyril_review` functions (`crtool.rs:30-47`). `write_output` and `report_error` translate `StepOutput`/`ReviewError` into stdout/stderr and process classifications (`crtool.rs:52-65`).

### Leaf public surface

- **`crates/cyril-review/src/lib.rs` — crate facade and shared contract.** It keeps `facts`, `gather`, `ReviewRun`, `ReviewClock`, and `SystemReviewClock` as the public exports (`lib.rs:1-11`). `StepOutput` hides its bytes and exposes only `stdout(&self) -> &[u8]` (`lib.rs:20-38`). `ReviewError` is the shared typed error vocabulary, including I/O, JSON, Git, empty-diff, run identity, manifest-stamp, regex, and clock failures (`lib.rs:40-80`); `exit_code` maps only `EmptyDiff` to 3 and all other errors to 2 (`lib.rs:82-91`).
- **`crates/cyril-review/src/run.rs` — run/artifact interface.** `ReviewRun::new(workspace, directory)` creates absolute lexical anchors; `workspace()` and `directory()` expose those anchors (`run.rs:59-79`). The manifest types are `pub(crate)`, so callers receive path anchors rather than the schema as Rust types (`run.rs:11-57`). The module owns the fixed child directories (`run.rs:111-136`), binary/text/JSON persistence (`run.rs:137-177`), manifest loading plus raw-path validation (`run.rs:179-209`), and `crtool_version` validation (`run.rs:211-238`).
- **`crates/cyril-review/src/clock.rs` — timestamp interface.** `ReviewClock::gathered_at_utc(&self) -> Result<String>` is the public injection seam (`clock.rs:4-7`); `SystemReviewClock` is the production wall-clock adapter (`clock.rs:10-23`), with manual UTC/civil-date formatting behind it (`clock.rs:25-55`).
- **`crates/cyril-review/src/gather.rs` — gather operation.** `gather(run, target, scope, clock) -> Result<StepOutput>` is the public operation (`gather.rs:13-20`). It reuses only a matching stamped run and rebuilds missing facts (`gather.rs:21-39`), otherwise resolves the target, captures the full diff, rejects whitespace-only diffs, captures status/numstat/per-file patches, and preserves display/raw path identity (`gather.rs:41-112`). It stamps and writes the manifest, invokes facts generation, and returns facts output followed by a gathered summary (`gather.rs:114-145`). Its private `split_scope`, `diff_args`, status parser, and numstat parser are operation-specific helpers (`gather.rs:147-205`).
- **`crates/cyril-review/src/facts.rs` — evidence analysis operation.** `facts(&ReviewRun) -> Result<StepOutput>` validates the manifest, builds facts, and returns the rendered summary (`facts.rs:49-52`). `build_facts` finds changed documents and changed definitions, writes `facts/symbols.json`, greedily chooses usage caps, writes pages, and updates manifest metadata (`facts.rs:55-127`). The implementation owns language definition patterns (`facts.rs:130-162`), hunk-based symbol discovery (`facts.rs:164-277`), same-language Git-grep usage extraction (`facts.rs:279-350`), changed-document metadata (`facts.rs:352-393`), and page rendering/cleanup (`facts.rs:394-514`).
- **`crates/cyril-review/src/git.rs` — concrete Git adapter.** It converts raw path bytes to `OsStr` (`git.rs:9-24`), invokes `git` with explicit argv in `ReviewRun::workspace()` (`git.rs:26-35`), normalizes required-command failures and accepts Git-grep status 1 as “no matches” (`git.rs:37-62`), and owns `auto` target selection, dirty/empty-range fallback, head-side extraction, and diagnostic argv rendering (`git.rs:64-134`).

## 2. Responsibility clusters and the rule separating them

The reconstructed clusters are:

1. **Package/graph wiring:** workspace membership, binary dependency, and resolved lockfile package graph (`Cargo.toml:1-10`; `crates/cyril/Cargo.toml:26-30`; `Cargo.lock:669-767`). This cluster changes compilation/package composition only.
2. **Process-facing dispatch:** `main.rs` and `crtool.rs` parse/select the hidden operation, enforce startup ordering, and translate library output/errors into process streams and exit codes (`crates/cyril/src/main.rs:44-58`; `crates/cyril/src/crtool.rs:30-65`).
3. **Review operation semantics:** the leaf facade, gather coordinator, and facts analyzer own what gather/facts mean (`crates/cyril-review/src/lib.rs:1-11`; `crates/cyril-review/src/gather.rs:13-145`; `crates/cyril-review/src/facts.rs:49-127`).
4. **Durable run state:** `run.rs` owns the path anchors, manifest schema, layout, serialization, raw-path preservation, and stamp gate (`run.rs:11-26,59-79,111-238`).
5. **Volatile external inputs:** `git.rs` owns the Git subprocess and raw-byte argv policy (`git.rs:9-62`); `clock.rs` owns wall time and its injectable seam (`clock.rs:4-23`).

The separating rule visible in imports and visibility is: **the outer binary owns process/package wiring; the leaf owns review semantics; each leaf internal owns one durable or volatile concern; no inner operation reaches back into the binary.** The dependency layout supports that rule: `gather` imports clock/facts/Git/run (`gather.rs:1-8`), facts imports Git/run (`facts.rs:1-6`), Git imports only `ReviewRun` and shared errors (`git.rs:1-4`), and run/clock depend on the shared crate contract (`run.rs:1-6`; `clock.rs:1-2`).

## 3. Dependency direction and concrete adapters

The compile-time direction is:

```text
workspace manifests/lockfile
        -> cyril binary
        -> crtool CLI adapter
        -> cyril_review::{ReviewRun, gather, facts, SystemReviewClock}
        -> gather -> {clock, facts, git, run}
        -> facts -> {git, run}
        -> git -> {ReviewRun, ReviewError}
        -> run/clock -> {Result, ReviewError}
```

Evidence for the binary-to-leaf edge is the binary manifest dependency (`crates/cyril/Cargo.toml:26-30`) and the `crtool` imports/calls (`crates/cyril/src/crtool.rs:4-5,35-43`). Evidence for the leaf's internal direction is its module registration/reexports (`crates/cyril-review/src/lib.rs:1-11`) and the imports listed above. The leaf manifest contains only serde, serde_json, regex, and thiserror as runtime dependencies, with tempfile only under dev-dependencies (`crates/cyril-review/Cargo.toml:13-20`); the lockfile entry confirms the same package dependency set (`Cargo.lock:758-767`). There is no compile-time cyril-core, ACP, Tokio, or Git-library edge in this leaf.

Concrete adapters/seams are:

- **System wall clock:** `SystemReviewClock` implements `ReviewClock` over `SystemTime` (`clock.rs:5-23`). A second adapter exists outside production in the integration test (`tests/evidence.rs:18-22`) and parity example (`examples/parity_driver.rs:13-20`), making the clock seam real rather than hypothetical.
- **Git process:** `git::run_allow_failure` is the concrete `std::process::Command::new("git")` adapter, rooted at the supplied workspace (`git.rs:27-35`). No public `Git` trait or alternate production Git implementation is present in the inspected leaf.
- **Filesystem:** `run.rs` directly uses `std::fs` for layout and artifacts (`run.rs:119-176`); facts directly uses filesystem metadata and page cleanup (`facts.rs:352-382,455-506`). No filesystem trait/adapter is present.
- **CLI/process streams:** `crtool.rs` is the Clap plus stdout/stderr adapter (`crtool.rs:7-15,30-65`). The external parity driver is a second consumer that parses arguments and explicitly flushes streams (`examples/parity_driver.rs:96-154`), but not a second production adapter.
- **Cross-process invocation contract:** the adjacent existing core prefix builder emits the literal `"..." crtool`/PowerShell `& "..." crtool` suffix (`crates/cyril-core/src/review/mod.rs:74-90`); the new binary consumes that argv shape through its hidden Clap command (`crates/cyril/src/crtool.rs:7-28`). This is a process contract, not a Rust dependency from `cyril-review` to core.

## 4. Pass-through modules and hypothetical seams

- `lib.rs` is a thin facade in terms of reexports (`lib.rs:1-11`), but it is not a pure pass-through: it owns the public output container, shared error taxonomy, and process exit classification (`lib.rs:20-38,40-91`).
- `facts()` is a short wrapper (`facts.rs:49-52`), but it performs manifest validation and facts construction rather than merely forwarding arguments.
- `crtool.rs` forwards operation selection to the leaf, but earns its seam by owning the hidden command grammar, system-clock choice, stream writes, and exit mapping (`crtool.rs:30-65`).
- `run.rs` has simple `workspace()`/`directory()` accessors (`run.rs:73-79`), yet the module is deep around them: lexical normalization, layout creation, serialization, manifest decoding, raw-path checks, and stamp refusal are all localized there (`run.rs:82-238`).
- `git.rs` wraps process calls, but is more than a pass-through because it centralizes Git failure normalization, the special grep status-1 rule, raw filename handling, auto target policy, and range-head diagnostics (`git.rs:9-134`).
- **Real seam:** `ReviewClock` has one production adapter and fixed-clock consumers in tests/examples (`clock.rs:5-23`; `tests/evidence.rs:18-22`; `examples/parity_driver.rs:13-20`).
- **Hypothetical seams not evidenced by two production adapters:** a `Git` trait, a filesystem trait, and injectable stdout/stderr writers. The integration test uses real Git and direct temporary-file inspection rather than substituting those adapters (`tests/evidence.rs:24-54,177-216`), so introducing those seams would be speculative from this tree.
- The private helper boundaries in facts are internal seams for its own tests, not public leaf contracts: the page test calls private `write_pages` and `body_characters` directly (`facts.rs:516-537`).

## 5. Protected-parent growth: wiring versus business logic

- **`crates/cyril/src/main.rs` is the protected parent.** Its additions are exactly the module declaration (`main.rs:1-3`), optional subcommand field (`main.rs:17-19`), and early dispatch/exit (`main.rs:44-48`). The dispatch occurs before `setup_logging` and all normal startup (`main.rs:50-58`), so the parent performs routing/order control only.
- **Business logic is in new children.** `crtool.rs` owns command-to-public-leaf conversion and process I/O (`crtool.rs:30-65`); gather/facts/run/Git/clock own review behavior (`gather.rs:13-145`; `facts.rs:49-514`; `run.rs:59-238`; `git.rs:27-134`; `clock.rs:5-55`). No review algorithm, manifest parsing, Git invocation, or symbol analysis is added to `main.rs`.
- **Manifest/lock changes are graph wiring.** The workspace member is one manifest entry (`Cargo.toml:5-7`), the binary dependency is one path dependency (`crates/cyril/Cargo.toml:28-30`), and the lockfile records the resulting package edge and leaf dependencies (`Cargo.lock:685-699,758-767`).
- **The leaf manifest is the ownership boundary**, not a parent-growth shim: it declares the standalone package and its minimal runtime dependency set (`crates/cyril-review/Cargo.toml:1-20`).

## 6. Tests reaching past interfaces

### White-box tests that cross private seams

- `crates/cyril/src/main.rs` tests `Cli::try_parse_from` plus private `startup_cwd` and `normalize_lexically`, including path/error details (`main.rs:312-381`). These test the protected parent's private helpers, not the hidden command subprocess.
- `crates/cyril-review/src/clock.rs` tests private `format_utc` directly across leap/century boundaries (`clock.rs:57-75`).
- `crates/cyril-review/src/facts.rs` tests private `body_characters`/`write_pages` directly and checks page content (`facts.rs:516-537`).

### External tests that cross the public interface into durable artifacts

- `tests/evidence.rs` implements the public `ReviewClock`, constructs `ReviewRun`, calls public `gather`/`facts`, and independently compares raw Git output and serialized artifacts (`tests/evidence.rs:18-22,88-120,177-216`).
- The main evidence scenario reaches beyond `StepOutput` to `manifest.json`, `diff.patch`, per-file patches, `facts/symbols.json`, and page files, then verifies idempotence, run identity refusal, and auto-target behavior (`tests/evidence.rs:216-325`).
- Unicode and raw-invalid filename scenarios verify the durable manifest/path contract and facts reuse (`tests/evidence.rs:327-555`).
- Stamp refusal, malformed-manifest refusal without artifact replacement, unknown metadata preservation, fatal Git-grep handling, and Git-grep status-1 handling are exercised through public `facts`/`gather` calls while inspecting files (`tests/evidence.rs:560-770`).
- `examples/parity_driver.rs` is another genuine external consumer of the public leaf, using only `ReviewRun`, `ReviewClock`, `gather`, `facts`, `StepOutput`, and `ReviewError` (`parity_driver.rs:11-20,96-154`).

### Missing/undetermined interface coverage

Within the inspected production/test paths, the actual `cyril crtool` binary adapter has no dedicated subprocess test in `crtool.rs` and the `cyril` test references found are the private `main.rs` tests; the leaf integration tests bypass `Command::run` and call the leaf directly (`crtool.rs:1-65`; `main.rs:312-381`; `tests/evidence.rs:177-216`). Therefore hidden-command help, actual early-dispatch side effects, stream behavior, and CLI exit mapping are not exercised by those leaf tests. Whether another prohibited/out-of-scope test location covers them is undetermined from the allowed production/test tree.

## 7. Reconstructed ledger

| path | responsibility | interface owner |
|---|---|---|
| `Cargo.toml:1-10` | Register the standalone review package in the workspace | Workspace manifest |
| `Cargo.lock:669-767` | Record `cyril`'s leaf edge and the leaf's resolved dependencies | Lockfile/package graph |
| `crates/cyril/Cargo.toml:26-30` | Make the binary depend on the review leaf | `cyril` package manifest |
| `crates/cyril/src/main.rs:1-3,17-19,44-58` | Parse and early-dispatch the hidden operation while preserving ordinary startup | `Cli` and `main` |
| `crates/cyril/src/crtool.rs:7-28,30-65` | Define hidden `crtool gather`/`facts`, invoke the leaf, emit streams, classify exits | `Command`, `Step`, `Command::run` |
| `crates/cyril-review/Cargo.toml:1-20` | Define the standalone leaf package and its dependency boundary | Package boundary |
| `crates/cyril-review/src/lib.rs:1-11,18-38,40-91` | Expose operations/seams and own output/error/exit contracts | `cyril_review` facade; `StepOutput`; `ReviewError` |
| `crates/cyril-review/src/run.rs:11-26,59-79,111-238` | Own run identity, manifest schema, layout, persistence, raw paths, and version stamps | `ReviewRun`; private `Manifest`/`ManifestFile` |
| `crates/cyril-review/src/clock.rs:5-23` | Supply the timestamp seam and host-clock implementation | `ReviewClock`; `SystemReviewClock` |
| `crates/cyril-review/src/git.rs:9-134` | Execute Git explicitly, preserve path bytes, resolve targets, and normalize failures | Internal `git::*` adapter functions |
| `crates/cyril-review/src/gather.rs:13-205` | Orchestrate target/scope diff capture, manifest creation, and facts handoff | Public `gather` |
| `crates/cyril-review/src/facts.rs:49-514` | Analyze changed symbols/documents, find usages, render pages, and update fact metadata | Public `facts`; private `build_facts` and analyzers |
| `crates/cyril-review/tests/evidence.rs:177-770` | Verify public leaf behavior through real Git and persisted artifacts | External `cyril_review` consumer |
| `crates/cyril-review/examples/parity_driver.rs:96-154` | Provide a second public-API consumer with deterministic clock/error/stream handling | Example `main`/driver |

## Concrete consumer-visible defects / risks found from source

These are separate from the structural reconstruction. They are source-supported observations; their runtime effects were not executed under the read-only isolation contract.

1. **`--cwd` is ignored for hidden commands — [INFERENCE].** `Cli` parses a top-level `cwd` option (`crates/cyril/src/main.rs:21-23`), but `main` exits through `command.run()` before calling `startup_cwd(cli.cwd)` (`main.rs:44-52`). Both hidden steps construct `ReviewRun::new(".", rundir)` (`crates/cyril/src/crtool.rs:35-43`). Thus an invocation with the top-level option before the subcommand, such as `cyril --cwd /some/repo crtool gather ...`, can parse the option but the review operation still uses the process current directory rather than the supplied cwd.
2. **CLI output and diagnostics are not explicitly flushed before `process::exit` — [INFERENCE].** The parent terminates with `std::process::exit(command.run())` (`crates/cyril/src/main.rs:46-48`). `write_output` calls `write_all` but not `flush` (`crtool.rs:52-56`), and `report_error` calls `writeln!` but not `flush` (`crtool.rs:59-64`). The independent parity driver explicitly flushes both stdout and stderr (`examples/parity_driver.rs:132-154`), so the production adapter has a source-visible buffering/loss risk when its streams are pipes.
3. **Space-bearing scope path is split into multiple pathspecs — conditional source defect.** The CLI captures scope as one `String` (`crates/cyril/src/crtool.rs:20-24`), but `gather` applies `.split_whitespace()` (`crates/cyril-review/src/gather.rs:147-154`). A caller passing one quoted path containing spaces therefore supplies multiple Git pathspecs rather than one path. Whether space-bearing paths are intended inputs is undetermined from the allowed production code; the transformation itself is concrete.

## 1. Changed module interfaces and responsibilities

**Public leaf surface — `crates/cyril-review/src/lib.rs`.** The module graph is private and only `ReviewClock`, `SystemReviewClock`, `facts`, `gather`, and `ReviewRun` are re-exported (`lib.rs:1-11`). `StepOutput` owns operation stdout bytes and exposes only `stdout()` (`lib.rs:20-38`); `ReviewError` owns contextual I/O/JSON/Git/run-stamp/regex/clock failures and maps `EmptyDiff` to exit 3, all other errors to exit 2 (`lib.rs:40-89`).

**Run/artifact model — `run.rs`.** `ReviewRun::new` creates absolute, lexically normalized workspace and output-directory anchors; callers can read those anchors through `workspace()` and `directory()` (`run.rs:45-90`). The module privately owns the manifest schema (`Manifest`, `ManifestFile`), package-version stamp, fixed run subdirectories, binary/text/JSON writes, manifest reads, and `crtool_version` validation (`run.rs:11-43,93-198`).

**Gather orchestration — `gather.rs`.** `gather(run, target, scope, clock)` is the main leaf operation (`gather.rs:12-18`). It normalizes scope and rejects a stamped directory whose target/scope identity differs; otherwise it reuses the snapshot, optionally rebuilding missing facts (`gather.rs:19-37`). A fresh run resolves `auto`, records a range-head mismatch warning, rejects whitespace-only diffs, writes the full diff and per-file patches, parses name/status and numstat records, writes changed-files, stamps the manifest with the injected clock, then invokes fact generation (`gather.rs:40-139`). The byte-oriented argument helpers and `--no-renames`/NUL parsing are internal to this module (`gather.rs:142-231`).

**Facts derivation — `facts.rs`.** `facts(run)` reads and validates the manifest, delegates to `build_facts`, and returns a `StepOutput` (`facts.rs:49-55`). The implementation detects definitions in Rust, Python, JavaScript/TypeScript, and Go, reads changed hunks, finds same-language textual usages through Git, captures changed documents, serializes `facts/symbols.json`, renders bounded usage pages, and updates `change_docs`/`facts` manifest metadata (`facts.rs:130-354,356-475`).

**Git adapter — `git.rs`.** This crate-private module owns raw path conversion, `git` process execution in `ReviewRun.workspace()`, required-output/error normalization, the special Git-grep status-1-as-no-match rule, text/byte wrappers, `auto` target resolution, range-head extraction, and argument rendering (`git.rs:9-134`).

**Clock — `clock.rs`.** `ReviewClock` has one operation, `gathered_at_utc`; `SystemReviewClock` obtains `SystemTime`, converts elapsed Unix seconds to a UTC civil timestamp, and reports clock failures through `ReviewError::Clock` (`clock.rs:4-43`).

**CLI adapter and parent dispatch.** `crtool.rs` defines the hidden `crtool` command and `gather`/`facts` steps, constructs `ReviewRun`, supplies `SystemReviewClock` for gather, writes stdout, and maps leaf errors to stderr/exit codes (`crtool.rs:7-65`). The existing binary parent adds only the command field and an early `command.run()` exit before logging, runtime, agent, and terminal setup (`main.rs:17-19,41-50`).

## 2. Responsibility clusters and separating rule

The production call graph separates into five clusters:

1. **Shell/build wiring:** workspace membership and the binary dependency (`Cargo.toml:3-7`, `crates/cyril/Cargo.toml:28-30`), plus the parent parse/dispatch (`main.rs:1-19,41-50`).
2. **Process-facing CLI:** `crtool.rs` owns command shape, conversion of arguments into the leaf call, stdout/stderr, and exit behavior (`crtool.rs:7-65`).
3. **Review orchestration and public contract:** `lib.rs` exposes the leaf surface; `gather.rs` sequences the run (`lib.rs:1-11`, `gather.rs:12-139`).
4. **Evidence production:** `facts.rs` analyzes patches/source and renders facts (`facts.rs:49-126,164-475`); `run.rs` owns the durable run representation and file/manifest operations (`run.rs:11-43,93-198`).
5. **External volatile inputs:** `git.rs` owns the Git subprocess (`git.rs:27-61`); `clock.rs` owns wall time (`clock.rs:5-21`).

The separating rule evidenced by the imports is **one primary responsibility/effect per module, with orchestration above adapters and persistence**: `gather` imports clock/facts/git/run (`gather.rs:1-8`), `facts` imports Git plus run storage (`facts.rs:1-6`), and Git depends only on the run workspace (`git.rs:1-4`). The exact intended architectural rule is otherwise undetermined from production code.

## 3. Dependency direction and concrete adapters

The direction is outer shell → public leaf → orchestration → internal adapters/storage:

- `main` parses and dispatches to `crtool::Command::run` before ordinary startup (`main.rs:17-50`).
- `crtool` calls only the public `cyril_review` items: `ReviewRun`, `gather`, `facts`, and `SystemReviewClock` (`crtool.rs:4-5,30-43`).
- `gather` coordinates `ReviewClock`, `facts::build_facts`, Git helpers, and run persistence (`gather.rs:1-8,40-139`).
- `facts` uses Git for usage/document queries and `run` for patches, manifest, and output pages (`facts.rs:1-6,164-354,417-475`).
- `git` uses the `ReviewRun` workspace and returns normalized crate errors; `run` and `clock` depend only on the crate error/result definitions (`git.rs:1-4,27-61`, `run.rs:1-6`, `clock.rs:1-2`).

Concrete adapters are `SystemReviewClock` over the host clock (`clock.rs:10-21`), the direct `Command::new("git")` adapter (`git.rs:27-35`), and `crtool.rs` over Clap plus process streams (`crtool.rs:7-65`). The integration test and parity example provide a second, non-production `ReviewClock` adapter (`tests/evidence.rs:1,18-22`; `examples/parity_driver.rs:11-20`). There is no production Git trait or alternate Git/filesystem adapter in the inspected code; whether that absence is intentional is undetermined from production code.

## 4. Pass-through modules and hypothetical seams

- `lib.rs` is a façade/re-export layer, but not merely pass-through: it owns the public output container, the shared error taxonomy, and exit classification (`lib.rs:8-11,20-113`).
- `facts()` is a thin public wrapper, but it performs manifest validation/read, fact building, and output construction, so the behavior is not duplicated in callers (`facts.rs:49-52`).
- `crtool.rs` forwards operation selection to the leaf, but adds the CLI contract, stream writes, and failure classification (`crtool.rs:30-65`).
- `git.rs` is a shallow concrete adapter around Git process calls, yet it earns its module through target auto-resolution, Git-grep exit handling, raw path support, and error normalization (`git.rs:9-134`).
- `run.rs` exposes simple path accessors, but also owns manifest schema, atomic-ish temporary JSON replacement, run layout, and stamp validation (`run.rs:45-65,105-198`); it is not only a pass-through path object.
- `ReviewClock` is the explicit seam. Production has one adapter (`SystemReviewClock`), while tests/examples inject fixed clocks, so it is a deterministic-test seam rather than evidence of multiple production implementations (`clock.rs:5-21`; `tests/evidence.rs:18-22`; `examples/parity_driver.rs:17-20`). Git and filesystem operations have no corresponding public trait seam in the production code. The future meaning of the pre-created `candidates`, `deduped`, `queues`, and `verdicts` directories is undetermined; current gather/facts behavior visibly writes patches, facts, changed-files, and manifests (`run.rs:105-119`; `gather.rs:71-107`; `facts.rs:77-126,417-475`).

## 5. Protected-parent growth: wiring versus new responsibility

The protected existing parent is `crates/cyril/src/main.rs`. Its additions are a module declaration, an optional Clap subcommand field, and an early dispatch/exit (`main.rs:1-19,45-50`). The dispatch is deliberately before `setup_logging`, workspace resolution, runtime construction, bridge startup, or terminal setup (`main.rs:45-58`), so the parent only routes the new path and leaves review behavior in `crtool.rs`/`cyril-review`.

Classification: **wiring, not new review responsibility** in `main.rs`; the new responsibility is owned by the new `crtool.rs` CLI adapter and the new `cyril-review` leaf. Root workspace registration and the `cyril` dependency are also wiring (`Cargo.toml:3-7`; `crates/cyril/Cargo.toml:28-30`). The lockfile change is generated package-graph wiring (`Cargo.lock:685-686,759-766`).

## 6. Tests reaching past interfaces

- `clock.rs` has a unit test that imports and calls private `format_utc` directly, rather than exercising only `ReviewClock` (`clock.rs:57-75`).
- `crates/cyril-review/tests/evidence.rs` crosses the public seam by implementing `ReviewClock`, constructing `ReviewRun`, and calling `gather` (`tests/evidence.rs:1-22,88-94`). It then reaches into the durable artifact representation via `run.directory()`, directly reading `manifest.json`, patches, `facts/symbols.json`, and usage pages (`tests/evidence.rs:99-113,200-257,289-324`). The same integration file directly exercises stamp refusal and artifact-preservation behavior for both public operations (`tests/evidence.rs:541-623`) and Git-grep error/no-match behavior (`tests/evidence.rs:661-700`).
- `examples/parity_driver.rs` is not a test, but is a real external consumer: it implements a fixed clock, calls `ReviewRun::new`, `gather`, and `facts`, and writes `StepOutput.stdout()` (`examples/parity_driver.rs:11-20,106-124,144-153`).

These consumers establish that the observable interface includes not only return text/errors but also the `ReviewRun` path accessors and serialized run artifacts. No production-code evidence was found for a `crtool` integration test; whether one exists outside the inspected production/test paths is undetermined.

## 7. Reconstructed ledger

| path | responsibility | interface owner |
|---|---|---|
| `Cargo.toml:3-7` | Register the new workspace package | Workspace build graph; no runtime interface |
| `Cargo.lock:685-686,759-766` | Resolve the new package/dependency graph | Cargo manifests; no runtime interface |
| `crates/cyril/Cargo.toml:28-30` | Make the leaf available to the binary | `cyril` package build graph |
| `crates/cyril/src/main.rs:1-19,45-50` | Parse and early-dispatch hidden mechanical commands while preserving ordinary TUI startup | `cyril` binary entrypoint / `Cli` |
| `crates/cyril/src/crtool.rs:7-65` | Define hidden `crtool gather`/`facts`, invoke the leaf, emit streams, map exits | `Command::run` and Clap command tree |
| `crates/cyril-review/Cargo.toml:1-20` | Declare the standalone review leaf and serialization/regex/error dependencies | `cyril-review` package boundary |
| `crates/cyril-review/src/lib.rs:1-113` | Own public reexports, `StepOutput`, shared errors, and exit classes | `cyril_review` crate surface |
| `crates/cyril-review/src/run.rs:11-198` | Own run identity, manifest schema, layout, persistence, and version-stamp validation | `ReviewRun`; internal manifest contract |
| `crates/cyril-review/src/gather.rs:12-231` | Own scoped diff acquisition, per-file patch capture, identity/idempotence, and gather orchestration | `cyril_review::gather` |
| `crates/cyril-review/src/facts.rs:49-475` | Own changed-symbol/document analysis, textual usages, JSON facts, and bounded pages | `cyril_review::facts`; internal `build_facts` called by gather |
| `crates/cyril-review/src/git.rs:9-134` | Own concrete Git process/error/target adapter | Crate-private callers in `gather` and `facts` |
| `crates/cyril-review/src/clock.rs:4-76` | Own wall-clock timestamp contract and system UTC implementation | `ReviewClock` / `SystemReviewClock` |

Uncertainty is limited to intent not encoded in production: the future consumers of unused run directories, whether Git/filesystem seams are expected to become swappable, and any CLI compatibility/versioning policy beyond the current Clap definitions are undetermined from production code.

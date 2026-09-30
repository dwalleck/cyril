# Spec: Native crtool gather, facts, and diagnostics contract

## Historical request (verbatim; superseded by the functional-equivalence amendment)
> Complete assignment thoroughly:
>
> # Target
> Complete the interrogated-spec artifact for cyril-s2hb, adopting pinned issue and cyril-m8qv/cyril-queu prior art and resolving only genuinely unspecified behavior using user's delegated gilfoyle approvals.
> # Change
> Write full spec.md with observable triples, role names, edge checklist, related issues, decision table and truthful approval delegation. Key open issues: cancellation typed result/partial output and child vs descendant handling, POSIX shlex vs Windows raw command-string behavior, missing stamp refusal, explicit extra crtool_version oracle stamping, byte parity for timestamps/duration (do not introduce production test-only CLI/env overrides merely to force a pass), JSON ordering/newlines, default timeout=Python, and exact exit3 empty diff vs exit2 die. Keep std/serde/serde_json/regex/thiserror-only leaf, no tokio/core, no diagnostics CLI. Build a narrow lawful contract: don't silently widen trusted-check cancellation to arbitrary processes or change target/scope grammar. Record unresolved impossibilities honestly instead of fabricating evidence. Provide concise recommended probe cases after completed spec; do NOT load/run next gilfoyle stage.
> # Acceptance
> One complete spec artifact satisfies stage pinned bar under delegated approval and explains every source contradiction. Return adopted decisions and any actual blockers. All edits use explicit worktree paths; no other writes.

## Stage trigger

The canonical artifact directory is `.cyril-s2hb/`. The parent route selected the Empirical route and recorded unresolved T4 behavior, so this interrogation runs before prototype, design, or implementation. This file is the only artifact written by this stage.
Delegated handoff scope: the 2026-09-30 functional-equivalence correction is a synchronized normative update across `spec.md`, `design.md`, `plan.md`, and `route.md`. The original interrogation's single-artifact statement applies to that initial stage. This A2-C increment writes no production or implementation evidence; A2's future implementation checkpoint is not part of this publication.

## What this is

This slice makes the first native, hidden `cyril crtool` path observable: a Rust leaf gathers a checked-out Git diff, computes facts, and exposes a cancellable diagnostics library operation used by the later review driver. It preserves the existing Python oracle's run layout, target and scope grammar, shell-prefix contract, output semantics, and failure distinctions while adding a version fence. The operation remains local to one workspace and does not start a workflow or execute an arbitrary diagnostics command through the crtool shell surface.

## Roles

- **Requesting maintainer**: delegates Gilfoyle specification decisions, receives the signed artifact, and requires functional-equivalence claims to name their host and clock proof layer.
- **Review operator**: invokes `/review` later and may indirectly cause the App to call the diagnostics library; this role sees run status and one-line failures, not the hidden crtool command in ordinary help.
- **Cyril binary dispatcher**: parses the hidden `crtool` subcommand and dispatches `gather` or `facts` before logging, configuration, workspace startup, agent startup, or terminal setup.
- **`cyril-review` leaf**: owns run-directory files, explicit-argv Git calls, gather, facts, typed diagnostics outcomes/errors, and deterministic serialization; it has no `cyril-core`, ACP, Tokio, or shell-config dependency.
- **Review App/check owner**: starts diagnostics off the event loop, owns the cancellation signal, and discards a late result after cancellation rather than creating a workflow.
- **Resolved host-shell provider**: resolves the configured shell once and exposes only a typed POSIX-like versus PowerShell dialect to the prefix builder.
- **Python functional reference**: remains an experiment-only independent reference for complex gather/facts transformations and selected lifecycle cases; it is not a one-to-one implementation or a shipped Cyril dependency.
- **Git repository/worktree**: supplies the target revision, scope-filtered diff, changed paths, and source files from the invocation working directory.
- **Review clock provider**: supplies real UTC/monotonic samples to native CLI calls or explicit fixed samples to fixture library calls; it is the only source for serialized gather timestamps and diagnostics durations.
- **CI functional-equivalence verifier**: runs deterministic fixtures and real-clock integration checks on Linux, native Windows, and the existing macOS tooling lane, comparing semantic records, meaningful text/content, explicit statuses/errors, patch content, raw-capture integrity, and host-independent native text rules.

## Behavior

### Clock seam and proof layers

- **Given** a `cyril-review` library operation that needs `gathered_at` or diagnostics `took`.
- **When** the operation receives a `ReviewClock` provider.
- **Then** it reads those fields only from that provider. The native CLI dispatcher supplies `SystemReviewClock`, which uses the real UTC wall clock for `gathered_at` and a real monotonic interval for diagnostics duration. A fixture driver supplies `FixedReviewClock` with `ClockInputs { gathered_at_utc, diagnostics_took_seconds }`, where both values are explicit; the independent functional reference replays those values. No production CLI flag or environment variable overrides either field.

- **Given** equivalent repository inputs, argv, manifest fields, package-version fixture, command output, shell dialect, and `ClockInputs` supplied to the native library and the independent reference.
- **When** the deterministic differential fixture runs both implementations.
- **Then** it compares parsed JSON while preserving every real field, type, and array order; meaningful text/content and records with line endings treated as presentation; explicit status/error/context; and patch content using existing Git tooling. Deterministic timestamps compare parsed instants and durations compare their numeric meaning. Raw diagnostic captures remain lossless. The reference is not required to reproduce incidental serialization, wording, or wrapper behavior.

- **Given** actual native entry points using `SystemReviewClock`.
- **When** gather runs through the CLI or diagnostics runs through its library-only consumer outside the deterministic fixture.
- **Then** separate real-clock integration proof checks UTC timestamp shape, monotonic nonnegative duration shape, CLI-to-library wiring for gather, and ordinary diagnostics process behavior. These checks complement, rather than replace, functional-equivalence assertions.

### Native text and raw capture behavior

- **Given** a generated native JSON/text artifact or a line written to native stdout/stderr.
- **When** the native implementation encodes that output.
- **Then** generated native text uses UTF-8. JSON comparisons are semantic: all real fields and types are required, array ordering is required, and key order/indentation/final formatting are not requirements. Line endings are presentation, including native output; no LF-only gate or Python newline adapter is required. Captured diagnostic raw bytes remain lossless; replacement decoding is allowed only for rendered text and matching. Git patches must represent the selected changes without losing meaningful source or binary content; their serialization need not match Python's output.

### Hidden dispatch and startup boundary
- **Given** a Cyril binary whose normal CLI help is generated by clap and whose ordinary startup would otherwise configure logging, read config, resolve the workspace, construct the runtime, spawn an agent, or initialize ratatui.
- **When** the invocation is `cyril crtool gather <rundir> <target> [scope]` or `cyril crtool facts <rundir>`.
- **Then** clap recognizes `crtool`, the dispatcher selects only `gather` or `facts`, and the selected operation starts before all listed ordinary-startup side effects. `crtool` is absent from ordinary `cyril --help`; `cyril crtool --help` may describe the hidden surface. No `diagnostics` crtool subcommand is recognized or exposed.

### Leaf dependency and ownership boundary

- **Given** the workspace dependency graph containing the new `cyril-review` crate.
- **When** the graph and compiled leaf are inspected with the workspace's dependency and all-target checks.
- **Then** the leaf's non-standard dependencies are exactly `serde`, `serde_json`, `regex`, and `thiserror`; its remaining dependencies are `std`. It does not depend on Tokio, `cyril-core`, ACP, a Git library, or shell-configuration parsing. Git remains an explicit `std::process` subprocess, and the binary dispatcher remains thin.

### Executable prefix

- **Given** a canonical absolute `current_exe()` path and the already-resolved host-shell dialect.
- **When** the review prefix is built.
- **Then** a POSIX-like dialect (`posix`, `bash`, or `fish`) yields `"/absolute/cyril" crtool`, while a PowerShell dialect (`pwsh` or Windows PowerShell) yields `& "C:/absolute/cyril.exe" crtool`. The path uses forward slashes. An ASCII double quote, dollar sign, backtick, control character, or surviving backslash causes a typed prefix error and no unsafe fallback. PowerShell also refuses U+201C/U+201D/U+201E, which its native parser treats as closing double quotes inside this form. Apostrophes and typographic characters without that syntax effect remain valid. The dialect comes from the resolved shell value, never from `cfg!(windows)` or a second config parse.

### Canonical run handoff and workspace root

- **Given** the later `/review` caller is creating a local review run.
- **When** it allocates a run directory and prepares to call crtool.
- **Then** the later App lifecycle owns `.code-review/` run naming and self-ignore policy. This slice receives the supplied run path, makes it absolute, and creates the required run children and any missing ancestors needed for that path, matching Python's `makedirs`. It does not choose a run name, create an ignore file, or otherwise own parent-directory policy.

- **Given** a later `/review` request whose invocation working directory is not exactly `git rev-parse --show-toplevel`.
- **When** the App performs its workspace precondition before invoking crtool.
- **Then** `/review` refuses in preflight before invoking crtool instead of changing directory, rebasing paths, or silently supporting a subdirectory launch. This is a future chat-command refusal, not a CLI exit status. Direct leaf operations do not add a second cwd normalization rule; they use the caller-supplied workspace.

### Fresh gather

- **Given** a fresh run directory, a Git repository reachable from the invocation working directory, a target argument, and an optional scope argument.
- **When** `cyril crtool gather <rundir> <target> [scope]` runs.
- **Then** the run path is made absolute; the directories `patches`, `candidates`, `deduped`, `queues`, `verdicts`, and `facts` are created. Git is called only with explicit argv, including `diff --no-renames`, `--name-status -z`, `--numstat -z`, and one per-file patch call. The operation writes `diff.patch`, `changed-files.txt`, `patches/NNN.patch`, `manifest.json`, `facts/symbols.json`, `facts/usages-N.txt`, and the facts metadata in the manifest. It prints the facts line before the gathered line, both on stdout, and exits 0.

### Gather target and scope grammar

- **Given** a target argument and a space-separated scope string.
- **When** gather resolves the target and builds Git argv.
- **Then** scope is split and trimmed with Rust standard Unicode whitespace semantics (`split_whitespace()`/`trim()`), with `.` substituted only when the result is empty; no quote-aware path parser is added. Paths containing spaces therefore remain unrepresentable by this input grammar. A non-`auto` target is passed verbatim. For `auto`, the resolver checks `@{upstream}`, `main`, and `master` in that order; with no base it selects `HEAD` when the tracked worktree is dirty and `HEAD~1` otherwise, recording the fallback warning. With a base, a dirty worktree or empty base range selects the merge base and records its warning; otherwise it uses `<base>...HEAD`. A range's head side is compared with `HEAD`, and a mismatch is recorded as a manifest warning rather than silently corrected.

### Gather files and manifest

- **Given** a nonempty target diff.
- **When** gather parses Git's NUL-delimited name-status and numstat output.
- **Then** file order is the numstat order and each file has `index`, `path`, `status`, `insertions`, `deletions`, `binary`, `patch`, and `patch_bytes`. Binary insertions and deletions are JSON `null`; binary is true when numstat uses `-`. `total_patch_bytes` is the byte length of the native `diff.patch`, not a sum reconstructed from numstat or a size copied from Python. `changed-files.txt` is generated UTF-8 text with each decoded path on its own line. Each patch represents the selected Git changes for that path. Git path bytes that are not UTF-8 are decoded with replacement for text and JSON fields.

**Then (continued)** The manifest contains `requested_target`, `target`, `scope`, `head`, `worktree_matches_diff_head`, `gathered_at`, `crtool_version`, `total_files`, `total_patch_bytes`, `warnings`, and `files`. A later facts write adds `change_docs` and then `facts` when those fields do not already exist. JSON object key order and indentation are not requirements, while every field, type, and array order is semantic. The current Cyril package version (currently `0.2.0-alpha.1`) is the `crtool_version` value; production code obtains the running package version, not a Git hash.

### Empty diff and die exit

- **Given** Git returns success for `git diff --no-renames <target> -- <scope...>` but the returned bytes contain no non-whitespace content.
- **When** gather evaluates the full diff.
- **Then** it writes no manifest or diff output, keeps only any directories already created for the run, reports an actionable empty-diff error containing the target and scope context, and exits exactly 3. This is the only normal empty-diff exit; the exact quoting and wording are not requirements.

- **Given** a Git invocation fails, a precondition is missing, a version fence fails, an output write fails, or another implemented command calls `die`.
- **When** the command reports the failure.
- **Then** it reports the failure through a `crtool: error:` prefix on stderr with the relevant category and actionable context. Version-fence and empty-diff failures remain distinguishable; Git failure text includes the explicit argv and decoded Git stderr context. The command exits exactly 2. Exit 3 is never used as a generic failure.

### Gather idempotence and version fence

- **Given** an existing manifest whose requested target, scope, and `crtool_version` match the invocation and current package version.
- **When** gather is repeated.
- **Then** it does not recapture the diff, emits an already-gathered result with the file count and target context, and rebuilds facts only when `facts/symbols.json` is absent. A different requested target or scope exits 2 with a fresh-run-directory error; the exact prose is not a requirement.

- **Given** a manifest whose `crtool_version` is absent, empty, non-string, or different from the current package version.
- **When** gather takes the existing-run path or facts is invoked on that run.
- **Then** the command refuses before recomputing facts or diagnostics. An absent/empty stamp is reported as an unstamped-run refusal; a different or invalid stamp is reported as a mismatch with the run/current values in actionable context. Both exit 2 and write no replacement facts. This fence applies to facts in this slice and to every later crtool step; exact quoting and capitalization are not requirements.

The invalid-stamp diagnostic must preserve enough structured value/context to distinguish missing, empty, non-string, and mismatched stamps without requiring Python's repr or JSON serialization spelling. The missing-field and empty-string cases remain the distinct unstamped refusal.

### Facts extraction

- **Given** a stamped gathered manifest and its per-file patches.
- **When** `cyril crtool facts <rundir>` runs.
- **Then** it scans definition patterns for Rust (`fn`, `struct`, `enum`, `trait`, `type`, `const`, `static`, `mod`, `union`, with the Python-compatible optional visibility/qualifier prefixes), Python (`def`, `class`), JavaScript/JSX/TypeScript/TSX/MJS (`function`, `class`, `interface`, `type`, `enum`, `const`, with optional export/default/async prefixes), and Go (`func`, `type`, including receiver syntax). Added definitions are `added`; a changed hunk whose header names an enclosing definition is `modified`; binary files are skipped; an added definition immediately following `#[...test]`, `@pytest`, `it(`, or `test(` is skipped. Modified `mod` symbols in the Python common-name set are filtered. The common-name set is `{new, default, fmt, from, into, main, run, get, set, render, update, init, len, is_empty, clone, drop, build, name, id, parse, apply, handle, tests, test, value}`.

**Then (continued)** For every retained symbol, same-language usages are obtained through explicit `git grep -n -w -F -I`, excluding the definition line and `.code-review/` paths. Each usage records decoded file, line, and text truncated to 140 characters. `ambiguous` is true for a common name or a name shorter than four characters; `usage_count` counts all retained textual hits; `usages` retains those hits. The source's `MAX_USAGES = 40` constant is not applied by the Python oracle, so this slice does not invent a 40-hit storage cap. Display pages, rather than stored usage records, are bounded below.

**Then (continued)** The changed-document scan uses the full target diff, not only the requested review scope; it excludes `.code-review/`, keeps existing files with extensions `.md`, `.rst`, `.adoc`, or `.txt`, and records at most the first 40 as `{path, bytes}` in `change_docs`. Facts writes `facts/symbols.json`, removes stale `facts/usages-N.txt` pages before rewriting them, and always emits at least one page. Each page packs lines so accumulated body characters do not exceed 20,000 before another line is added; a single unsplit line longer than that is an explicit source-compatible exception. It tries per-symbol display caps 25, 12, 6, then 3 until the complete usage map fits within the four-page target; cap 3 remains the final choice if the map still needs more pages. Common and unused sections retain their defined meaning and ordering, but incidental Python wording is not required. It emits a facts success result carrying the symbol count, display cap, document count, and page paths, using native UTF-8 LF text.

### Diagnostics launch and typed outcomes

- **Given** a stamped manifest, a command string, and the later App check owner calling the leaf library operation `diagnostics(run_dir, command, options, cancel)`, where the default timeout is 1,800 seconds.
- **When** the operation starts the check.
- **Then** the App invokes this blocking library operation through its worker mechanism (`spawn_blocking` in the current runtime), while the leaf remains synchronous and `std`-only. POSIX hosts parse the command with Python `shlex.split`-equivalent POSIX parsing and launch the resulting argv without a shell. Windows passes the raw command string to the host's `CreateProcess` command-line parsing semantics, without applying POSIX or non-POSIX `shlex` splitting. The operation inherits the caller's current working directory (the App supplies the reviewed workspace) and removes only empty environment entries whose names start with `CARGO_` or `RUST`; all other inherited entries and nonempty matching entries remain.

**Then (continued)** The typed result is data for every started process: `Clean` with exit code 0, `Failed` with the actual exit code, `TimedOut` with partial output, or `Cancelled` with partial output. A process that cannot be started is `DiagnosticsError::CannotStart` and is not converted into a clean or failed result. Missing/invalid manifest or command parse errors are typed pre-launch errors. The library writes no diagnostics files for a pre-launch error.

- **Given** the child starts and emits stdout and/or stderr, including more than a pipe buffer or invalid UTF-8.
- **When** the process exits, fails, times out, or is cancelled.
- **Then** the combined captured bytes are `stdout bytes + b"\n" + stderr bytes`; those raw bytes remain lossless in `facts/diagnostics-raw.txt`. Rendered text decodes invalid UTF-8 with replacement only where needed for text and matching. The operation writes a filtered `facts/diagnostics.txt` and the manifest's `facts.diagnostics = "facts/diagnostics.txt"` and `facts.diagnostics_status`. The filtered file retains the command, status/duration/HEAD context, changed-file match count and lines, and trailing nonempty output with the defined 200/15 limits; exact labels, capitalization, and wrapper prose are not requirements. Nonzero exit and timeout are data, not errors. The stdout result identifies the status, duration, and changed-file count.

### Diagnostics cancellation ownership

- **Given** a running diagnostics child and a caller-owned cancellation signal.
- **When** the App sets that signal before the child exits.
- **Then** the leaf requests termination and reaps only the directly spawned child, as the closed cyril-queu resolution requires. The termination/reap request has a one-second deadline. Stdout and stderr are redirected to distinct owned temporary files rather than pipes; after observing the terminal child state, the leaf samples each file's length and reads no more than that snapshot. It writes the same raw and filtered files with status `CANCELLED` and returns a typed `Cancelled` result. There are no blocking pipe-reader threads to strand. The App treats the result as terminal and cannot create a workflow from it or from a late completion. No process-name scan, global kill, shell command, `pkill`, `killall`, `taskkill /T`, or other operation targeting unrelated processes is permitted. Descendants are not searched or terminated.

**Then (continued)** Cancellation observed before launch returns a typed cancelled result with `started = false`, writes no diagnostics files, and does not launch a process. Cancellation is not retried. Timeout uses the same one-second direct-child termination/reap deadline. Failed termination/reaping or capture-file cleanup is a typed lifecycle error, not permission to launch a workflow. Snapshot reading and serialization remain proportional to captured output; the one-second bound governs child termination, not arbitrary filesystem I/O or output volume.

### Inherited capture-handle cleanup

- **Given** a diagnostics child that exits, times out, or is cancelled while a descendant or helper still holds an inherited stdout/stderr write handle.
- **When** the direct child reaches its terminal state.
- **Then** the leaf snapshots bounded file lengths, closes its owned handles, and unlinks its temporary capture files without waiting for inherited holders. Bytes appended after those snapshots are not retained. It neither scans for nor terminates descendants; cleanup failure is a typed lifecycle error. Checks observe seekable regular files on stdout/stderr rather than Python's pipes: this explicit architectural tradeoff preserves recorded byte output for ordinary writing commands, not pipe-sensitive `fstat`/seek/backpressure behavior.

### JSON, text, and newline serialization

- **Given** any generated JSON artifact in this slice.
- **When** it is written or rewritten.
- **Then** it is atomically replaced through a sibling temporary file and encoded as UTF-8 text. JSON comparisons preserve every real field and type and every array's order; object key order, indentation, escaping choices for ordinary Unicode, line endings, and final formatting are not requirements. Text pages, `changed-files.txt`, rendered diagnostics, report-facing facts files, and CLI stdout/stderr preserve meaningful content without a host-specific newline wrapper. Captured diagnostic raw bytes and binary diff/patch files preserve their source bytes. Replacement decoding is used only where external bytes must become text, never as permission to normalize valid Unicode.

### Differential parity harness

- **Given** an isolated fixture Git repository, canned finder/model JSON, a controlled package-version fixture, and native Python and Rust implementations.
- **When** the differential harness runs gather, facts, diagnostics fixtures, prefix cases, and relevant self-test vectors.
- **Then** it checks functional equivalence: parsed JSON with all real fields/types/array order, meaningful text/content and records, explicit statuses/errors/context, and patch content through existing Git tooling. The Python implementation remains an independent functional reference for complex transformations; it is not a byte or text-wrapper oracle. Obsolete byte goldens are removed; earlier capture receipts remain historical evidence, not current green evidence.

**Then (continued)** The deterministic fixture supplies equivalent repository inputs, argv, manifest fields, package-version fixture, command output, shell dialect, and `ClockInputs` to both library seams. The real native CLI separately uses `SystemReviewClock`; its real-clock integration proves UTC timestamp shape, monotonic nonnegative duration shape, CLI-to-library wiring, and process behavior. Raw diagnostic captures are compared losslessly; generated text is checked for meaningful content rather than serialized bytes.

**Then (continued)** The reference clock adapter is experiment-only and replays the same captured clock inputs; no production CLI flag or environment variable controls time. Deterministic timestamps compare parsed instants and duration meaning, not serialized spellings. Status/error conditions and actionable context remain required even when exact stdout/stderr wording does not.

## Success criteria

- **Binary / structural / security**: `cyril --help` contains no `crtool`, while hidden gather/facts dispatch occurs before ordinary startup side effects; checked by a binary smoke fixture that records startup markers and help output.
- **Binary / structural / security**: the leaf dependency set is exactly `std` plus `serde`, `serde_json`, `regex`, and `thiserror`, with no Tokio, `cyril-core`, ACP, Git library, or diagnostics CLI; checked by dependency-tree and CLI-surface checks.
- **Binary / structural / security**: prefix output matches the four dialect cases and rejects every listed unsafe path character; checked by a dialect matrix using canonical fake executable paths.
- **Binary / structural**: a fresh fixture gather creates every named file, preserves explicit Git argv behavior, stamps the package version, and produces all required manifest fields and semantic array ordering; checked by the functional-equivalence harness.
- **Binary**: an empty diff exits 3 with one stderr error line, while Git failure, missing stamp, mismatched stamp, missing manifest, and write/precondition failures exit 2; checked by isolated fixture invocations and stderr/exit capture.
- **Binary / structural**: matching gather is idempotent, while changed target/scope or stale/missing version refuses without recomputation; checked by two-run and manifest-mutation fixtures.
- **Binary / structural**: the NOTES.md/new-helper/caller fixture records the change document, added symbol, usage, page, and manifest facts metadata; checked by parsed output assertions.
- **Binary / structural**: diagnostics distinguishes clean, nonzero, timeout, cancellation, and cannot-start; captures invalid UTF-8, noisy output, slash-normalized changed-file matches, 200 matching-line cap, and 15 trailing nonempty lines; checked by native process fixtures on each supported host.
- **Binary / security**: cancellation reaps only the directly launched child, snapshots partial output without waiting for inherited handles, removes owned captures, and never authorizes workflow launch; native owned-process fixtures verify leaf lifecycle here, while the App result-order fence is owned by cyril-305w/cyril-4o1u.
- **Binary / structural**: deterministic outputs are functionally equivalent to the independent Python reference for identical `ClockInputs`, preserving semantic fields/types/array ordering, meaningful text/content, statuses/errors/context, and patch content; separate real-clock CLI integration proves actual clock wiring and formats.
- **Binary / integrity**: child diagnostic evidence is checked as lossless captured bytes. Generated text encoding is UTF-8; no host-specific wrapper newline, LF-only rule, or incidental wording is a gate.
- **Quantitative**: each facts page packs no more than 20,000 accumulated body characters before adding the next line, except that one unsplit line may exceed the budget; the renderer tries caps 25, 12, 6, then 3 against a four-page target, with cap 3 retained when the target remains exceeded; measured by a long-symbol fixture.
- **Binary / structural**: no production test-only clock, duration, CLI, or environment override is present; checked by a production dependency/source-surface audit and by running the binary without test environment variables.

## Out of scope

This change does NOT include the full `/review` form or lifecycle, repository configuration loading, parent `.code-review/` run naming or self-ignore creation, run-directory creation policy outside the leaf's required child layout, recipe or agent materialization, workflow New/Invoke/Load/Retry operations, permission authorization, findings summary UI, resume/recovery, merge, shard, ballots, collate, finalize, comments, publication, vendor-neutral engines, non-HEAD target support, subdirectory launch, model-launch gates, automatic recovery, or tracker changes. It does NOT add a diagnostics shell subcommand, a general shell executor, a second target/scope grammar, a process-name or process-tree killer, descendant ownership semantics, a Git library, Tokio, `cyril-core` imports in the leaf, or production test-only clock/duration CLI/environment overrides.

## Related issues

- **cyril-s2hb**: pinned implementation ticket; owns hidden dispatch, gather, facts, diagnostics, version stamp, prefix contract, and differential harness for this slice.
- **cyril-5gb3**: W3 parent specification; supplies stories 55, 56, 57, and 60 and the no-Python/native crtool boundary.
- **cyril-m8qv**: closed prior-art decision; adopted the `cyril-review` leaf, hidden first-dispatch surface, shell-dialect prefix, explicit-argv Git, Python differential oracle, version fence, and library-only diagnostics boundary.
- **cyril-queu**: closed prior-art decision; adopted the canonical workspace run layout assumptions, preflight diagnostics as an off-loop library call, failure-as-data for checks, and cancellation before workflow creation.
- **cyril-r3t6**: closed KAS pipe-drain finding; its create-time reader evidence reinforces concurrent draining and bounded cleanup for chatty commands, but it does not add a Tokio dependency to this leaf.
- **cyril-zj2c**: closed input cited by cyril-m8qv/cyril-queu; supplies the exact-prefix policy input and run-authorization boundary, while authorization itself remains outside this slice.
- **cyril-p0xt**: closed input cited by cyril-m8qv/cyril-queu; establishes absolute cyril-owned recipe/agent assets, while asset installation remains outside this slice.
- **cyril-305w**: downstream W3 preflight consumer; requires a cancellable check before workflow creation and therefore consumes the typed diagnostics/cancellation contract here.
- **cyril-4o1u**: downstream cancellation issue; broad workflow cancellation is outside this slice, but its preflight phase must not receive an arbitrary-process cancellation primitive from this slice.
- **cyril-7vrl**: downstream native crtool step; relies on the same run layout, version fence, and parity discipline after this gather/facts slice.
- **cyril-ild0**: native Linux/Windows qualification issue; its cross-platform proof consumes the prefix, dispatch, diagnostics, and parity probes defined here.

## Decisions

`DG` means “adopted under the delegated Gilfoyle approval quoted verbatim in Approval.” It records authorization accurately; it is not presented as a direct technical quotation from the requester. `PA:<issue>` means the behavior is adopted from the cited closed prior-art decision.

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| Should the native port live in a leaf crate or under the binary? | **PA:cyril-m8qv — new `cyril-review` leaf.** | The closed port-shape decision owns run files and deterministic steps in the leaf and keeps the binary a dispatcher. | Leaf dependencies and ownership are pinned; core/App only consume narrow seams. |
| Which hidden commands exist in this slice? | **DG — only `gather` and `facts`; diagnostics is library-only.** | The pinned issue names those commands and explicitly forbids a diagnostics shell surface. | Any `diagnostics` crtool command is rejected and cannot enter the shell allowlist. |
| When does hidden dispatch happen? | **PA:cyril-m8qv — before logging/config/workspace/terminal/agent startup.** | The prior-art decision rejected a sibling binary and requires one short-lived executable. | Ordinary help/startup remains unchanged except for hidden clap recognition. |
| How is the prefix shell selected? | **DG — use one narrow resolved dialect value; do not infer from OS or duplicate config parsing.** | The route evidence shows `wire_name()` collapses PowerShell kinds and is not the needed typed seam. | POSIX-like and PowerShell forms are testable independently of the build target. |
| What characters make a canonical executable path unsafe? | **PA:cyril-m8qv, pinned issue — reject double quote, dollar, backtick, control, or surviving backslash; never fall back. DG native safety extension: reject U+201C/U+201D/U+201E for PowerShell.** | Native parser evidence shows these three typographic double quotes terminate the required PowerShell string; ASCII apostrophe and U+2018–201B/U+201F do not and must not be refused merely as a broad “quote” class. | Prefix construction returns a typed error only for the pinned hazards and demonstrated PowerShell syntax hazards. |
| Does scope parsing become quote-aware in Rust? | **DG — no; use Rust standard Unicode whitespace splitting/trimming and `.` for an empty result.** | The correction changes incidental Python splitting while preserving the established unquoted path grammar; paths with spaces remain unrepresentable today. | The resulting scope components reach Git as pathspec argv without shell quoting or a second parser. |
| What does `auto` resolve to? | **PA:Python reference and cyril-s2hb — upstream, main, master; dirty/empty merge-base; no-base HEAD/HEAD~1 fallback.** | Existing behavior is observable and the ticket does not authorize a target redesign. | Warnings and resolved target retain their meaning and actionable context; a head mismatch remains a warning. |
| Which process invokes Git? | **PA:cyril-m8qv — explicit `git` argv through `std`; never a shell, gix, or git2.** | Explicit argv preserves target/scope safety and meaningful Git content without requiring serialized-output parity. | Git failures remain exit 2 and Git output bytes remain the source for patch/content checks. |
| What distinguishes an empty diff from a die? | **DG — whitespace-empty full diff is exit 3; every Git/precondition/write/version failure is exit 2.** | The pinned acceptance names this distinction and Python's `die(code=3)` is the reference behavior. | Exit code plus error category/context are stable functional obligations; exact prose is not. |
| What happens to an existing run with a missing stamp? | **DG — refuse as an unstamped run, exit 2, before idempotence or recomputation.** | A missing field cannot prove which implementation wrote the run; treating it as valid would violate the version invariant. | Missing and mismatched stamps remain distinct refusals with actionable context, not fixed one-line spellings. |
| What version is stamped? | **DG — the running Cyril package semver from `env!("CARGO_PKG_VERSION")`; the current value is `0.2.0-alpha.1`, not a Git hash.** | The route report identified the package-version source and rejected a hash-based identity. | Every later step compares the same string; a package upgrade requires a fresh run. |
| How does the Python reference handle the new stamp? | **DG — amend the experiment reference/generator to emit the semantic `crtool_version` field and compare its value/type.** | The pinned acceptance adds a required field while current Python lacks it; silently ignoring the extra field would falsify functional equivalence. | The fixture supplies an equivalent version value to both implementations without adding a production CLI/env override; object-key position is incidental. |
| How are timestamp and duration equivalence resolved? | **DG — pass a `ReviewClock` library seam; deterministic fixtures provide shared `ClockInputs`, while the CLI always supplies `SystemReviewClock`.** | Separate real clocks cannot produce shared values, but a production library seam can accept explicit fixture inputs without a production CLI/environment override. | Compare parsed timestamp instants and duration meaning; separate real-clock CLI integration verifies actual wiring and formats. |
| What timestamp and duration formats remain required? | **DG — UTC `YYYY-MM-DDTHH:MM:SS+00:00` for `gathered_at`; integer seconds in the diagnostics result.** | Fixed clock inputs and real clocks both preserve the meaningful UTC/time value. | Time-zone/DST cannot alter the parsed UTC instant; duration uses a monotonic elapsed sample, while incidental serialization is not pinned. |
| How are JSON ordering and newlines handled? | **DG — semantic JSON comparison preserves every real field/type and array order; generated text uses UTF-8; atomic sibling-temp replacement remains required.** | Object-key order, indentation, line endings, host wrapper translation, and exact wording are incidental; meaningful source, binary, and raw-capture bytes are not. | No preserve-order metadata, host-specific text golden, or native LF-only gate is required. |
| Does the final manifest include `change_docs` as well as `facts`? | **DG — yes; both fields are present with their required values and types.** | The route summary listed the later `facts` key but the Python source also inserts `change_docs`; field presence and semantic content remain required while object-key position is incidental. | Differential checks compare parsed fields and array order, not insertion position. |
| Is there a 40-usage storage cap? | **DG — no storage cap in this slice; the Python `MAX_USAGES` constant is dead. Display pages use the 25/12/6/3 caps and 20,000-character budget.** | A source read shows `hits` is stored without slicing; adding a cap would silently narrow the oracle. | The implementation must preserve all stored hits unless the oracle is changed in a separately approved decision. |
| What is the cancellation result? | **DG, constrained by PA:cyril-queu — a started child cancellation is a typed `Cancelled` result with `started=true` and observed partial output; the directly spawned child is killed/reaped, and descendants are neither searched nor targeted.** | The closed resolution literally states “Esc / `/review cancel` kills the child and abandons the run”; it does not define descendants, so this slice does not infer a broader process search. | Raw/filtered files use `CANCELLED`; no retry or workflow creation follows. |
| What is the default diagnostics timeout? | **DG — 1,800 seconds.** | The crtool Python `diagnostics --timeout` default is 1800; the driver's separate `--timeout-min` default of 180 minutes is a different layer and must not be confused with it. | Library options default to 1800 seconds; configuration may pass an explicit later value. |
| How are POSIX command strings parsed? | **DG — Python POSIX `shlex.split` equivalence, no shell execution.** | The oracle explicitly uses `shlex.split` on non-Windows hosts. | Quotes and backslash escapes are removed as Python does; malformed input is a typed pre-launch error. |
| How are Windows command strings parsed? | **DG — pass the raw string to Windows CreateProcess command-line semantics; do not use `shlex` or a shell wrapper.** | Python intentionally passes the raw string on Windows, and non-POSIX `shlex` would retain quote characters. | Native Windows quoting is an empirical parity premise for the recommended probe. |
| What happens on a nonzero check or timeout? | **PA:cyril-queu — preserve it as diagnostics data and continue at the later workflow layer; the leaf returns typed data.** | A failing check is evidence for reviewers, not a preflight configuration error. | `FAILED (exit N)` and `TIMED OUT` are written to raw/filtered files and manifest status. |
| What happens when the check cannot start? | **PA:cyril-queu plus DG — return typed `CannotStart`, write no diagnostics files, and abort before workflow creation.** | Inability to execute the configured check is distinct from a check that executed and failed. | The App presents one line and leaves the run directory for inspection. |
| How are noisy streams and inherited holders handled? | **DG revised 2026-09-30 — use distinct file-backed captures and length-bounded snapshots; close/unlink owned files without waiting for inherited holders.** | Safe std-only Rust cannot forcibly close a pipe blocked in another owning thread; terminating descendants would violate the approved ownership boundary. The native spool probe proves unlinking with live inherited handles on Linux and Windows. | Accept the explicit regular-file versus pipe observability tradeoff; preserve byte capture, direct-child ownership, bounded termination, and no stranded reader threads. |
| What bytes form diagnostics output? | **DG — in-memory stdout bytes, one LF separator, and stderr bytes; raw captures remain lossless; decode only for rendered text and matching.** | The raw child evidence is meaningful and must not be normalized; wrapper newline conversion and exact labels are incidental. | Changed-file matching normalizes backslashes only for comparison; rendered output is checked for status/context/content rather than Python's serialized bytes. |
| What does a cancellation signal do before launch? | **DG — return `Cancelled { started=false }`, write nothing, and do not spawn.** | A pre-cancelled check has no output evidence and must not fabricate an empty successful run. | The App abandons the preflight attempt without launching a process. |
| What if cancellation races timeout or normal exit? | **DG — whichever terminal observation is first wins; a cancellation observed while the child is live yields `Cancelled`, otherwise the already-reaped exit/timeout result stands.** | This makes the result one typed terminal state instead of a contradictory pair. | The App's cancellation generation fence rejects a result received after cancellation. |
| Are concurrent writers supported? | **N/A — one run directory has one owner; separate run directories are the supported concurrency unit.** | The Python oracle has no lock or merge protocol, and adding one would widen this slice. | Callers must use a fresh run directory after a failed partial write; no cross-process consistency claim is made. |
| What is the partial-write policy? | **DG — JSON replacement is atomic, but gather/facts have no whole-run rollback; a failed run is invalid and must not be reused as a successful run.** | This matches the existing temp-and-replace JSON writer without fabricating transactional guarantees for binary patches and pages. | A later caller receives an explicit precondition error or uses a fresh run directory. |
| How are missing/null fields handled? | **DG — missing manifest, malformed stamp, missing required files, and invalid command are typed exit-2/pre-launch errors; binary numstat values are the intentional JSON null exception.** | Missing data must not be interpreted as an empty review, while binary `null` is an established manifest value. | Facts and diagnostics never silently synthesize defaults for required fields. |
| Do soft deletion, tenancy, replication, or cache invalidation affect this slice? | **N/A — run artifacts and Git refs are local files with no soft-delete records, tenant boundary, replica, or cache layer.** | These checklist dimensions cannot alter the defined behavior. | No storage or multi-tenant policy is introduced. |
| Does local time or DST affect output? | **DG — gathered timestamps are supplied as UTC by `ReviewClock`; diagnostics duration is a supplied monotonic elapsed value displayed as integer seconds.** | The Python manifest uses timezone-aware UTC, while fixtures and native CLI use the same seam with different providers. | DST transitions do not change the parsed UTC instant; real-clock and fixed-clock proofs remain distinct.
| Does the leaf expose diagnostics as a shell-allowlisted operation? | **PA:cyril-m8qv/cyril-queu — never.** | The command runs arbitrary configured text, unlike the narrow crtool allowlist. | Policy recognizes gather/facts and future mechanical steps, but never diagnostics. |

## Edge checklist

| Edge dimension | Behaviors covered | Decision |
|---|---|---|
| Empty set | Gather, facts, diagnostics | Empty gather diff is exit 3; facts writes a `(no symbols detected)` page; empty diagnostics writes the defined separator/raw evidence and zero matching lines. |
| Maximum scale | Gather, facts, diagnostics | No new whole-diff, symbol, usage, or raw-output cap is imposed; page packing uses 20,000 accumulated body characters, changed-file matches are capped at 200, and trailing lines at 15. The 25/12/6/3 renderer targets four reads but cap 3 may still produce more pages for an unbounded map; a single unsplit line may exceed the page budget. |
| Null / missing field | Manifest, files, diagnostics | Missing/malformed required values refuse with exit 2 or typed pre-launch error; binary insertions/deletions intentionally serialize as null; missing version is a distinct refusal. |
| Concurrent writes | All run artifacts | N/A — one owner per run directory; separate run directories are the only supported concurrent unit, with no lock protocol. |
| Permission denied / unauthenticated | Git and filesystem writes | N/A for network authentication; OS Git/read/write denial is a die/typed I/O error with no success fallback and no claim of rollback. |
| Partial failure (one of N succeeded) | Per-file patch collection, diagnostics output | Gather exits 2 and leaves an invalid partial directory when a later Git/read/write operation fails; diagnostics retains partial stdout/stderr for timeout/cancel and returns a typed lifecycle error if bounded child reaping or capture cleanup fails. |
| Retries / idempotency | Gather, facts, diagnostics | Matching gather is idempotent; facts may rebuild deterministically; diagnostics has no automatic retry and a later call overwrites its own named diagnostics files. |
| Soft-deleted records | Run files and manifest | N/A — this slice has no record tombstones or soft-delete state. |
| Multi-tenancy boundaries | Local workspace and run path | N/A — the contract is one local workspace with no tenant identity or cross-workspace access. |
| Time-zone / DST | Manifest timestamp and diagnostics duration | `ReviewClock` supplies UTC timestamp and monotonic elapsed value; local zone and DST cannot alter the serialized values. |
| Replication lag | Git refs and run files | N/A — no replica or remote read is used; each invocation observes its local Git/FS state. |
| Cache invalidation | Facts and manifest | N/A — no cache is introduced; facts reads the current manifest/patches and rewrites its named pages. |

## Recommended probe cases

These are concise probes for the Empirical stage after this artifact; they were not run while writing the specification.

1. **Fixture gather/equivalence**: make a temporary Git repository with a modified Rust helper, its caller, and `NOTES.md`; run native and Python library seams on one host with equivalent `ClockInputs`, version, argv, and fixture inputs; compare parsed records, meaningful content, statuses/errors, patch content, lossless raw captures, and idempotent repeat.
2. **Exit split**: run an empty target and a deliberately failing Git target; assert exit 3 versus exit 2, one stderr line each, and no false success output.
3. **Stamp fence**: exercise matching, different, missing, empty, non-string, and old-version `crtool_version` manifests for gather/facts; assert refusal category/context, exit status, and no recomputation.
4. **Prefix matrix**: feed canonical POSIX/PowerShell dialects and paths containing each rejected character; assert exact prefix strings and typed refusal.
5. **Command grammar**: on POSIX use quoted spaces, escaped quotes, backslashes, and malformed quotes; on native Windows use raw CreateProcess quoting with both PowerShell forms; compare parsed argv and meaningful command behavior to the independent functional reference.
6. **Diagnostics outcomes**: run clean, nonzero, invalid UTF-8, >64 KiB stdout/stderr, timeout, unstartable, pre-cancelled, and running-child cancellation fixtures; compare lossless raw captures, rendered meaningful content, statuses, and partial-output behavior.
7. **Cancellation cleanup**: make a temporary check child write partial output and spawn a descendant holding inherited capture handles; cancel it and assert direct-child reaping, exact bounded snapshots, successful owned-file cleanup, and no descendant targeting. A positive-control holder writes after unlinking and exits cooperatively; downstream App late-result rejection is separately owned by cyril-305w/cyril-4o1u.
8. **Clock and native-output proof**: feed fixed UTC/duration inputs and compare parsed timestamp/duration meaning and all semantic fields on Linux and native Windows separately; invoke the actual native CLI with its real clock and validate UTC/duration format and wiring without any CLI/env clock override. Presentation-only line endings are not a separate gate.
9. **Startup/help smoke**: invoke ordinary help and hidden gather/facts with side-effect markers enabled in the fixture binary; verify hidden dispatch ordering and absence of diagnostics from the CLI.

## Approval

Requester approval (verbatim):
> “For each issue based on the order of dependencies, create a worktree, run the gilfoyle skill with the rivets id as the argument, approve all signoffs and recommended decisions until the rivets issue is complete, open a PR for the issue and run a code review on the PR. Loop code reviews until no issues remain”
>
> “don't stop for scope expansion. Stop on unavailable hosts or destructive operations. Only auto-approve decisions regarding the gilfoyle skill”

Date: 2026-09-30

Approval record: The technical choices marked `DG` are adopted under this scoped Gilfoyle delegation. They are not fabricated direct quotations about implementation details. No host-unavailable or destructive-operation exception was invoked for this specification stage.

Historical pre-amendment revision 2026-09-30, specification owner: file-backed capture replaced the unimplementable safe-std blocked-reader-close contract. Its ordinary-output byte-parity wording and host-wrapper assumptions are retained as historical evidence only, not current acceptance; raw snapshots `AB`/`C`, unlink behavior, and direct-child ownership remain meaningful lifecycle evidence. `.cyril-s2hb/probe_spool.rs` SHA256 `5c8a20f5c207ab8654fef6e9d27f6dc6f8bf1136d1c53946716fd02a1b359a60` passed on Linux and native Windows. Diagnostics smoke wording is corrected to library-only; App ordering assertions are assigned to their actual downstream owners rather than claimed executable in this leaf slice.

Historical specification clarification, 2026-09-30, under the prior delegated approval: invalid-stamp display was pinned for the pre-amendment experiment. The current contract retains distinct missing/empty/mismatched refusal categories and actionable values/context, but not Python's repr/JSON spelling.

Prefix clarification, 2026-09-30: the issue explicitly says **double quote**, correcting this artifact's earlier ambiguous “quote”. Native Windows PowerShell `Language.Parser.ParseInput` on `& "C:/probe/a<character>b.exe" crtool` returned one parser error for each U+201C/D/E and zero errors with a single `StringExpandable` token for U+2018/19/1A/1B/1F. This supports the narrow PowerShell safety extension under delegated Gilfoyle approval, not blanket rejection of smart quotes or apostrophes.

The same parser matrix was also executed through native `C:\probe\cyril-w3-tools\pwsh\pwsh.exe` 7.6.6 with `-NoProfile -Command`; its eight error counts/token kinds matched Windows PowerShell 5.1 exactly. Both production PowerShell dialects therefore have observed evidence for the three-character branch.

Technical source-alignment correction, 2026-09-30: supplied-path recursive directory creation remains Python `makedirs` behavior. The earlier ban on all parent-directory creation accidentally contradicted fresh gather; the intended boundary is App-owned naming/self-ignore policy. The future `/review` cwd refusal is described as a preflight refusal rather than inventing an exit status for a chat command.

## Functional-equivalence amendment — 2026-09-30

Requester correction (verbatim):
> “It didn't need to be a 1 to 1 copy of the python script, just functionally the same”

Further requester clarification (verbatim):
> “The results and output don't have to be byte equivalent”

These latest requester corrections supersede the delegated byte-exact/one-to-one choices above. Active acceptance is functional equivalence: preserve run layout and filenames, JSON fields/types and array ordering, file/symbol/usage sequence, target and meaningful scope selection, facts/snippets/document and usage caps, meaningful page contents and20,000-character/unsplit-line rule, stamps and refusal/no-recompute behavior, exit0/2/3 distinctions, error category and actionable context, early dispatch and safety/lifecycle/native gates, Git work ceiling, and direct-child diagnostics ownership/deadlines. Patches must retain correct meaningful source/binary content; captured diagnostic raw bytes remain lossless. Generated text uses UTF-8. Under the standing Gilfoyle delegation, the specification owner also removes the intermediate native LF-only rule: Python repr/quote/boolean spellings, JSON object key order/indentation, line endings, host wrapper translation, CRCRLF formatting, and exact stdout/stderr prose are not requirements.

The Python implementation remains an independent functional reference for complex gather/facts transformations, not a serialization oracle. Verification uses parsed JSON semantic comparison, meaningful text/record comparison, explicit status/error/context checks, Git-tool patch-content comparison, and actual CLI smoke. Old byte-comparison results and host-specific formatting checks are historical. A2 acceptance, including C1–C4, is PENDING in this documentation-only increment; its future implementation checkpoint will own results and outstanding qualification. Publication counter history and the no-reset rule are recorded in `plan.md`, Review partition.

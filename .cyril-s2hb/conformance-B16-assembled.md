# S2HB assembled conformance report

## Final verdict

**PASS — every approved production ledger row matches the assembled source, every changed production owner maps to exactly one approved row, and there are no active `MISMATCH`, `UNCOVERED`, or `MISSING` items.** The frozen reconstruction hash and requested HEAD were verified, and current dirty/untracked diagnostics sources were read directly. This is a read-only structural result; it makes no claim about current-head tests, CI, review, merge, or publication state.

`local://s2hb-assembled-conformance.md` was not writable in this review mode, so this is the complete report for Main to freeze.

## Reconstruction verification

The reconstruction is materially accurate about the changed-set, ownership clusters, interfaces, dependency direction, and test seams. Direct source inspection resolves three reconstruction uncertainties without requiring a code change:

| Reconstruction item | Source/design resolution | Disposition |
|---|---|---|
| Diagnostics is public in the leaf but absent from the hidden CLI (`reconstruction-B16-assembled.md:52,109`). | The design explicitly forbids a diagnostics CLI (`design.md:47`); `crtool::Step` intentionally has only `Gather` and `Facts` (`crates/cyril/src/crtool.rs:18-28`). | **Resolved evidence clarification.** This is required asymmetry, not missing wiring. |
| Bridge and HostShell citations use shortened/noncanonical paths. | The actual owners are `crates/cyril-core/src/protocol/bridge.rs:36-43,214-228` and `crates/cyril-core/src/protocol/kas/host_shell.rs:155-164`. | **Resolved citation correction.** Treat the design’s `bridge.rs` / `kas/host_shell.rs` labels as module shorthand; no duplicate or missing module exists. |
| Production App use of the shell getter is not visible. | The contract says the **later** App captures it (`design.md:45`), while the approved A1 proof-layer explicitly selects the verification-only public consumer (`design.md:181-186`). That consumer reads `review_shell()` before `split()` and executes `CrtoolPrefix` (`crates/cyril-core/examples/review_prefix.rs:80-93`). App is not an owner in this increment’s ledger. | **Approved deferred-use tradeoff.** No current code/design mismatch; do not remove the getter or move prefix rendering into Bridge/HostShell. |

## Approved ledger comparison

| Approved ledger row | Verdict | Design evidence | Actual-source evidence | Disposition |
|---|---|---|---|---|
| `crates/cyril-review/src/lib.rs` | **MATCH** | Facade declarations/shared error owner; no Git/facts/process algorithms (`design.md:67`). | Module and operation exports are at `lib.rs:1-15`; `StepOutput` and borrowed `stdout()` are at `:22-42`; shared typed errors and exit classification are at `:45-115`. No operation algorithm is implemented here. | None. |
| `crates/cyril-review/src/run.rs` | **MATCH** | `ReviewRun` plus private validated I/O; owns anchors, stamp, manifest, atomic JSON and text, not target/regex/process policy (`design.md:68`; contract at `:38-44`). | Private manifest/raw-path schema is at `run.rs:10-57`; public anchors/accessors at `:59-79`; layout and shared I/O at `:107-176`; stamp, serde decode, and raw-path coherence checks at `:179-237`. | None. |
| `crates/cyril-review/src/clock.rs` | **MATCH** | Sole clock seam with system and fixture adapters; final contract has wall UTC plus diagnostics elapsed time (`design.md:38-42,69`). | `ReviewClock` has exactly the two approved methods and `SystemReviewClock` implements both at `clock.rs:4-27`; diagnostics consumes elapsed time at `diagnostics/mod.rs:113-116`. | None. Dual time domains are expressly approved in this one seam. |
| `crates/cyril-review/src/git.rs` | **MATCH** | Private explicit-argv Git owner; no shell/run policy (`design.md:70`). | Native raw-path conversion is at `git.rs:9-25`; the sole concrete Git subprocess adapter and status handling are at `:27-62`; target resolution and error rendering are at `:64-134`. | None. |
| `crates/cyril-review/src/gather.rs` | **MATCH** | Diff/status/numstat/patch orchestration and idempotence; no facts implementation or diagnostics (`design.md:71`). | Public orchestration, reuse, target/head checks, patch materialization, manifest creation and facts handoff are at `gather.rs:14-145`; scope/argv and NUL record parsing remain private at `:147-236`. Facts policy is called, not copied. | None. Cohesive-owner retention is the approved choice. |
| `crates/cyril-review/src/facts.rs` | **MATCH** | Symbols, usages, docs and page rendering; no UI/workflow/process cancellation (`design.md:72`). | Public/private operation boundary is at `facts.rs:49-128`; patterns/extraction at `:135-278`; NUL-framed usages and documents at `:279-392`; caps/rendering/Unicode page packing at `:394-510`. | None. Cohesive-owner retention is the approved choice. |
| `crates/cyril-review/src/diagnostics/mod.rs` | **MATCH** | Public diagnostics/options/cancellation/result; owns report/status/prelaunch transitions, not command parsing or manifest encoding (`design.md:73`). | Public values and accessors are at `diagnostics/mod.rs:17-72`; exact operation boundary and two prelaunch checks at `:74-111`; command/process delegation, status/filter/report persistence at `:113-207`. It reuses run-owned I/O (`:4-5,148-159`). | None. |
| `crates/cyril-review/src/diagnostics/process.rs` | **MATCH** | Private direct-child lifecycle, captures, snapshots and cleanup; no descendant targeting/shell policy (`design.md:74`). | Private capture/file setup and cleanup are at `process.rs:15-86`; exit/cancel/timeout decisions and direct-child termination/reaping at `:88-209`; exit classification and sampled-length bounded snapshots at `:210-276`. | None. |
| `crates/cyril-review/src/diagnostics/command.rs` | **MATCH** | POSIX shlex; Windows native executable/tail and explicit batch dispatch; environment filtering; no arbitrary-text shell wrapper (`design.md:75`, input decision at `:23`, prohibition at `:47`). | NUL/cwd/environment policy is at `command.rs:6-42`; POSIX parsing at `:46-108`; Windows explicitly preserves a supplied `.bat`/`.cmd` extension during resolution and passes the untouched tail through `Command::new(program).raw_arg(tail)` at `:111-159`, relying on Rust’s concrete standard adapter to detect `.bat`/`.cmd` and perform native `cmd.exe` batch dispatch. Arbitrary text is never shell-wrapped. | None. A hand-written second batch dispatcher would duplicate the selected concrete native adapter. |
| `crates/cyril-core/src/review/mod.rs` | **MATCH** (inherited) | `ShellDialect`/`CrtoolPrefix` own path validation and exact prefix only (`design.md:76`). | Four dialects are at `review/mod.rs:10-16`; canonicalization, absolute/UTF-8 checks and exact spelling at `:40-91`; hazard validation at `:99-115`; Windows namespace spelling at `:124-143`. No resolver/config/run logic is present. | None. Applicability verified; not redescribed as new A2/B work. |
| `crates/cyril/src/crtool.rs` | **MATCH** | Clap/process adapter owns parsing, stdout/stderr and exit mapping, not business logic/runtime startup (`design.md:77`; placement at `:47`). | Hidden command tree is at `crtool.rs:7-28`; public leaf dispatch at `:30-48`; borrowed byte output and error/exit handling at `:52-66`. | None. Its thinness is purposeful process-boundary adaptation. |
| Existing `main.rs`, core `lib.rs`, `bridge.rs`, `kas/host_shell.rs` | **MATCH** | Retain existing responsibility and add only narrow wiring (`design.md:78`); protected-parent rules are `:83-88`. | Main adds only module/subcommand/early dispatch before logging at `crates/cyril/src/main.rs:1-47`; core exports `review` at `crates/cyril-core/src/lib.rs:1-7`; Bridge stores/exposes/populates the dialect without rendering at `protocol/bridge.rs:36-43,149-174,214-228`; HostShell exhaustively projects its already-resolved kind at `protocol/kas/host_shell.rs:155-164`. | None. Core owners are inherited and remain applicable. |
| Root/leaf/binary Cargo manifests | **MATCH** | Register leaf and needed serde/regex features; no relaxed lints or extra leaf runtime dependencies (`design.md:79`). | Workspace registration is `Cargo.toml:1-9`; binary-to-leaf edge is `crates/cyril/Cargo.toml:26-31`; leaf runtime dependencies are only serde, serde_json, regex/unicode and thiserror at `crates/cyril-review/Cargo.toml:1-19`. | None. `Cargo.lock` is generated dependency resolution under this row, not a separate production owner. |

### Coverage accounting

- **UNCOVERED (code-not-ledger): none.** Every changed production owner maps once to a row above. Examples, integration tests, CI/oracles, README/AGENTS, and `Cargo.lock` do not introduce production responsibility owners.
- **MISSING (ledger-not-code): none.** Every approved owner exists with its intended interface.
- The dirty/untracked production owners are not hidden by the committed diff: `diagnostics/mod.rs`, `diagnostics/process.rs`, and `diagnostics/command.rs` were inspected directly and map to their three explicit rows.

## Responsibility mismatches

**None.** The actual clusters preserve the approved separation:

- façade/errors: `lib.rs:1-15,22-115`;
- durable run contract: `run.rs:10-79,107-237`;
- concrete Git: `git.rs:9-134`;
- gather orchestration: `gather.rs:14-236`;
- facts analysis/rendering: `facts.rs:49-510`;
- diagnostics orchestration versus command construction versus child lifecycle: `diagnostics/mod.rs:74-207`, `diagnostics/command.rs:6-216`, `diagnostics/process.rs:15-276`.

No second Git owner, manifest encoder, shell resolver, process-tree killer, UI/workflow owner, or facts implementation was found.

## Interface mismatches

**None.** The approved operation signatures and boundary values are present (`design.md:38-45`): `ReviewRun` exposes only path references (`run.rs:59-79`); gather/facts use the public run and clock boundaries (`gather.rs:14-19`; `facts.rs:49-52`); diagnostics returns typed outcome/started/output (`diagnostics/mod.rs:45-81`); `StepOutput` exposes borrowed bytes (`lib.rs:24-42`); errors remain typed (`lib.rs:45-115`).

Cross-boundary dispatch is complete:

- `StepOutput` from gather/facts is consumed by `crtool` at `crtool.rs:46-55`.
- Every `DiagnosticsOutcome` variant produced by `process.rs:132-140,210-235` is explicitly handled by diagnostics status routing at `diagnostics/mod.rs:116-120` and by the verification consumer at `examples/parity_driver.rs:507-524`.
- `Cancellation` is checked before validation, again before launch, and in the child loop (`diagnostics/mod.rs:82-111`; `process.rs:112-142`).
- `ShellDialect` is produced by HostShell, stored/routed by Bridge, and consumed before split by the approved native public consumer (`host_shell.rs:155-164`; `bridge.rs:36-43,214-228`; `examples/review_prefix.rs:80-93`).

## Dependency/adapter mismatches

**None.** Dependency direction is binary → leaf and operation → concrete private adapters. The leaf has no core, ACP, Tokio, Git-library, or process-framework dependency (`crates/cyril-review/Cargo.toml:13-19`). `ReviewClock` is the only production trait (`clock.rs:4-8`). Git, native command construction, and child capture remain concrete std adapters (`git.rs:27-35`; `diagnostics/command.rs:46-159`; `diagnostics/process.rs:15-43`).

## Seam mismatches and candidate tradeoffs

| Candidate from reconstruction | Governing approved row/decision | Ruling |
|---|---|---|
| Diagnostics library API but no CLI verb | Placement prohibition (`design.md:47`) and CLI row (`:77`). | **Permitted/required.** Do not add diagnostics to `crtool`. |
| One clock trait owns wall UTC and monotonic elapsed time | Final clock contract (`design.md:38-42,69`). | **Permitted.** Both methods have real consumers/adapters; no unused clock hook. |
| Private manifest with tests constructing/reading JSON artifacts | Run row and no-public-manifest rule (`design.md:43-44,68`). | **Permitted.** Tests exercise the durable file protocol while Rust manifest types remain private (`tests/diagnostics.rs:35-81,243-255`). |
| Direct private-helper tests | Locality decision (`design.md:63`) and process test-seam row (`:74`). | **Permitted.** Unicode page packing, bounded reads and terminal decisions are the expressly named deterministic fences (`facts.rs:516-538`; `process.rs:277-355`). Other same-module parser/filter/cleanup unit checks create no production seam. |
| Verification-only controlled clocks/child handshakes | Clock and diagnostics rows (`design.md:69,73-75`). | **Permitted.** They are concrete test adapters, not runtime flags or copied production backends (`examples/parity_driver.rs:372-498`). |
| Temporary-file capture instead of pipes | Selected capture alternative and explicit descriptor tradeoff (`design.md:61-64`). | **Permitted accepted risk.** Actual files request Unix mode `0600`, bound reads to sampled lengths, and clean up (`process.rs:46-86,238-276`). |
| Textual changed-path filtering | Diagnostics report owner (`design.md:73`) and fixed diagnostics behavior. | **Permitted owner-local policy.** It does not create a structured-parser seam (`diagnostics/mod.rs:169-207`). |
| Raw/display path duplication | Run/gather/facts raw-identity decision (`design.md:249-256`). | **Permitted and necessary.** Raw bytes are stored only when replacement decoding would lose identity (`gather.rs:88-96`); display labels remain strings. |
| Copying/cloning | Public/result/cancellation contracts (`design.md:36-45`). | **No unapproved copying.** `StepOutput` is borrowed by consumers (`lib.rs:35-40`; `crtool.rs:52-55`), `Cancellation::clone` shares one `Arc<AtomicBool>` (`diagnostics/mod.rs:29-43`), and raw capture buffers are moved through the result rather than cloned (`process.rs:238-254`; `diagnostics/mod.rs:113-148`). |
| Concrete Git/process modules without backend traits | Selected concrete-run alternative (`design.md:52-63`). | **Permitted/required.** No hypothetical adapter or mock-echo seam exists. |
| Prefix/getter currently consumed only by verification code | Prefix row plus approved proof-layer revision (`design.md:76,181-186`). | **Permitted deferred seam.** The verification consumer proves the current carry; the design explicitly labels App consumption as later. Preserve the getter for that handoff. |
| Large cohesive gather/facts/process/command owners | A2 and B retain-owner decisions (`design.md:90-99,213-225`). | **Permitted.** Production test boundaries occur below the approved triggers: clock before `clock.rs:62`, facts before `facts.rs:516`, diagnostics mod before `mod.rs:214`, process before `process.rs:277`, command before `command.rs:218`, and prefix before `review/mod.rs:146`. This is supporting shape evidence only; the MATCH decision rests on preserved ownership and readable boundaries, not line-count fitting. |

## Protected-parent mismatches

**None.** Main remains startup wiring only and dispatches before logging/config/runtime (`main.rs:39-47`); core lib remains export-only (`cyril-core/src/lib.rs:1-7`); Bridge only carries the optional resolved dialect and leaves sender/split contracts intact (`protocol/bridge.rs:36-43,57-93,214-228`); HostShell only performs exhaustive kind projection (`protocol/kas/host_shell.rs:155-164`). No review algorithm, JSON parsing, prefix rendering, or duplicate resolution entered a protected parent.

## Non-MATCH disposition summary

There are **no active non-MATCH ledger items**. The reconstruction-only citation/uncertainty corrections above are fully disposed by corrected source/design evidence and require no code change or design amendment.

## Exact PASS reason

**PASS because all 13 approved ledger rows are implemented by their designated owners, all changed production owners are covered exactly once, inherited owners remain applicable without acquiring new responsibility, every new boundary value reaches an approved consumer, and every candidate structural tradeoff is explicitly permitted by its governing approved row.**
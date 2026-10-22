# cyril-xavt shell approval review decisions

## PR191 F4 — Windows DEFAULT ReplayFrame layout

**Scoped Linux repair qualification: PASS; fresh current-head Windows CI remains blocking.** This bounded repair supersedes the historical no-net-replay-boxing conclusion below for the current source only. Earlier October20 and held-af78 receipts keep their original source scope.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F4 | The current replay enum fails Windows DEFAULT strict Clippy; compact both large replay payloads without reshaping production types. | CI39847823115, relayed by Main | Verified | PR191 pre-repair head `75b6d148dd1936210399e725b17a6be75dce9b69`: Windows `Clippy (default features)` fails at pre-repair `crates/cyril-core/src/test_support.rs:438`, `clippy::large_enum_variant`, Permission at least584 bytes versus Notification at least376. Exact retained hosted diagnostic and skipped-step metadata are portable in the F4 archive below. | Accept | Box both `ReplayFrame` payloads, both creation paths, and move out of each box at all four existing handler calls in the two App replay helpers. | blocking until fresh repair-head Windows CI; ordinary review and the three scoped Linux repair commands passed, without rerunning the reported old-head hosted failure. |

### Delivered behavior and authorization

DG — Main authorizes the minimal compact test-support representation in this delegated repair assignment. This is an approved-contract qualification repair under existing Slice1/design C5 ownership and permission-semantics obligations, not a production-domain redesign or relaxed lint requirement.

- `crates/cyril-core/src/test_support.rs:435-509`: `ReplayFrame::Notification(Box<RoutedNotification>)` and `ReplayFrame::Permission(Box<PermissionRequest>)`; `kas_trace_replay` boxes the complete existing permission literal and each already-mediated routed notification. Each replay vector slot carries a pointer-sized payload rather than the entire domain object. Boxing only Permission would leave the other large inline alternative against a pointer and can invert the lint.
- `crates/cyril/src/app.rs:10444-10509,10926-10952`: `lki9_replay_trace` and `q9104_feed` move `*routed`/`*request` into the unchanged production handlers. No payload reconstruction, cloning, sender replacement, sorting, batching or new wiring is introduced. Permission session/title observations, frame labels, turn/busy checks, transcript/order assertions and all existing test assertions remain unchanged.
- The permission oneshot sender remains inside its original non-Clone request until the existing App handler takes ownership. Existing replay receiver disposal is unchanged. Notification iteration still follows trace order and `mediated.forward` order.
- No production domain type, method, dependency, feature, threshold, platform gate or lint configuration changes. This adds one owned allocation per replay frame only; it does not add allocation to the live bridge/App path. The earlier static at-most200-byte gap prediction is not current Windows evidence and is superseded by the observed584/376 diagnostic. No exact post-repair layout or successful Clippy result is claimed from source reasoning.

### API inventory and production reachability

LSP was used first for `ReplayFrame` references at declaration/constructor positions, `kas_trace_replay` references, hover, and payload definitions. Although rust-analyzer reported ready, these returned empty results despite visible source callsites; the tool inconsistency was reported. Complete Rust-source fallback found one enum, its sole creator with two construction paths, and exactly two consuming helpers, both migrated.

`crates/cyril-core/src/lib.rs:17-21` gates `test_support` with `any(test, feature = "test-support")`; the enum/helper additionally require `kas`. Both App consumers are inside the `#[cfg(test)]` module at `app.rs:3239-3240` and independently `#[cfg(feature = "kas")]`. `RoutedNotification` and the non-Clone, oneshot-owning `PermissionRequest` remain unchanged at `types/event.rs:492,614`; production handlers remain unchanged at `app.rs:1177,1185`.

Workspace resolver2 and the package manifests keep normal `cyril`/UI core dependencies free of `test-support`; their dev-dependencies opt in (`crates/cyril/Cargo.toml:46-51`, `crates/cyril-ui/Cargo.toml:28-29`). App defaults still enable `kas`, while Core `kas` remains opt-in. The standalone historical `.cyril-vhfz/probe` explicitly enables test support and is not live binary wiring. All-target test compilation can therefore expose this enum without making its allocations part of normal production execution.

### Source handoff pins

These exact file-byte identities are bound to the scoped qualification below, not a newly published commit. Source ancestry is PR191 head `75b6d148dd1936210399e725b17a6be75dce9b69`, with no held merge. Main must use the actual published repair HEAD, not that pre-repair ancestry pin or a guessed next commit, for final propagation.

| File | Before repair SHA-256 | Source handoff SHA-256 |
|---|---|---|
| `crates/cyril-core/src/test_support.rs` | `503161a2a8d23bb166a3c05f5faeda1f6ba31269c53fda7136203323201d5cee` | `390f3a3bd277495926384f9ea6e9aae280786f5e3fc62f5165040349965ac207` |
| `crates/cyril/src/app.rs` | `75241006e287566870b67dfae9234cbb04c1edde72ded0f260890aa2c3eaf26d` | `f19d9d4fb29e32cb4c78651082e4d27584e52cc11ebb6b2a65fe33f0dd3e8ac0` |

### Performed checks, repair review, and unresolved qualification

The implementation worker performed source/reference/feature inspection and SHA-256 identity capture only; no Cargo, formatter, application runtime or final-integration-worktree edit. The sole HOST coordinator subsequently performed the three scoped commands recorded below exactly once. This metadata finalization consumes those retained receipts, not another qualification run.

Independent ordinary repair review: **NONE / correct / confidence0.97 — ReviewReplayRepair, 2026-10-22**, covering both exact source handoff pins above, both creator paths, all four consuming moves and affected assertions/ownership/order/feature interactions. The reviewer independently found no payload loss/cloning, replay reordering, responder-ownership change, live-path allocation or lint/configuration bypass. Box field inspections followed by whole-payload moves were judged plausible by source reasoning, explicitly uncompiled. No isolated reconstruction, new proof machinery or repository checks were performed. This is source-only review, not successful qualification or Windows CI.

**HOST coordinator scoped qualification: PASS**, canonical `xavt75-two-box-replay-repair-scoped-qualified-attempt1-handoff.json`, SHA-256 `5cbecc649c234205e0ff3341d19c402d3ebd2afb11cb2f021e19ea14cc81a727`, completed2026-10-22T20:58:01Z on Linux. Canonical commands and full logs are retained byte-for-byte in [F4 portable evidence](evidence-replay-repair532-e8728c85-index.json).

| Recorded command | Actual result | Portable full log |
|---|---|---|
| `cargo fmt --all -- --check` | PASS, exit0; no formatter applied | `gates/01-initial-fmt-CHECK.log` (empty successful log; exit result is in the canonical) |
| Exact replay-dependent App nextest selection below | PASS, exit0: **13 passed, 456 skipped/filtered**; skipped cases are not executed coverage | `gates/02-exact-thirteen-replay-dependent-App-nextest.log` |
| `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | PASS, exit0 | `gates/03-strict-App-Core-KAS-alltargets-Clippy.log` |

The current full532 manifest has SHA-256 `49365a2068906cf0847ea77751057449b13641020b964fd221af54b4efa115c2`, descriptor `e8728c859208f056dc17fbfd44db5611ee2ed5a01d9fe9caf1ae2589de08db9e`. Exactly the two source rows above differ from the prior modal532 inventory; the other530 rows and Cargo.lock (`52a1751579bce849fe5d349c3b6994f70455a6321378318b237f7e2882830035`) are unchanged. Before/after command guards retain that descriptor. This is not a claim that old held-bc6/af78 metadata equals published75, a full historical-matrix replay, a new private App/consumer run, or final integration acceptance.

The minimal portable archive `evidence-replay-repair532-e8728c85.tar.gz` is55,420 bytes, SHA-256 `faa150a7ffa25831e454c8a6abddb783a76a0dc9e56ac0185c7c809f3bd8722b`. Its index inventories nine members: eight bounded payloads plus their manifest—current canonical, full532 manifest, three full command logs, the original independent reviewer output, retained hosted failure/six skipped product steps, and the exact two-file diff. No binaries, full source tree, nested archives or unrelated hosted logs are included. Prior evidence archives/indexes remain unchanged; the original evidence-index.json is not amended.

Main additionally authorized only read-only `git ls-files --stage --` for the exact five existing delta paths and the raw-index SHA-256. Their actual stage0 mode/blob preimages are carried by publication-files.json; index identity is `2234c0ac507ccb7479dc8ad67e33d2c8b1876c12ee4bbef5332570cdd4cefeb3`. No index mutation, commit, push or tracker operation was performed by this worker.

The thirteen existing replay-dependent App tests cover both consuming helpers: the five `lki9_replay_` cases plus `repeat_cap_wake_is_labelled_in_live_capture`, `parked_step_question_shows_in_live_capture`, `parent_answer_renders_inside_its_labelled_wake`, `answer_command_prompts_the_parked_step_and_the_run_resumes`, `failed_answer_reopens_the_parked_step`, `wake_credits_and_run_totals_in_live_capture`, `run_completion_line_renders`, and `stale_repeat_cap_pause_never_labels_a_later_wake`. These are the recorded HOST-coordinator qualification commands, **not instructions to rerun them**:

```sh
cargo fmt --all -- --check
cargo nextest run -p cyril --bin cyril --features kas -E 'test(lki9_replay_) | test(repeat_cap_wake_is_labelled_in_live_capture) | test(parked_step_question_shows_in_live_capture) | test(parent_answer_renders_inside_its_labelled_wake) | test(answer_command_prompts_the_parked_step_and_the_run_resumes) | test(failed_answer_reopens_the_parked_step) | test(wake_credits_and_run_totals_in_live_capture) | test(run_completion_line_renders) | test(stale_repeat_cap_pause_never_labels_a_later_wake)'
cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings
```

Fresh actual Windows DEFAULT `cargo clippy --workspace --all-targets -- -D warnings` and the six subsequent product steps remain required on Main's actual published repair HEAD. In CI39847823115 those steps did not run: workspace-default tests; binary no-KAS build, Clippy and tests; Core no-KAS Clippy and tests (`.github/workflows/ci.yml:219-237`). The retained terminal run/job metadata and exact diagnostic establish the original failure and skips; Linux success does not turn them into PASS. Main owns publication, fresh current-head CI and final integration/propagation. No whole-issue or merge acceptance is claimed here.

## Scope and authorization

This delivery copy retains the original combined-source and October20 reconstruction/repair history below, including F1/F2/F3 and the superseded representation repair. Its EJ `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` reconstruction, gates860, private338 and review0.98 are historical, not the current held HEADbc6 + MERGE_HEADaf78 qualification. delivery-handoff.md preserves reconstruction identities; [current verification](verification.md#current-held-af78-checkpoint) and the [portable index](evidence-current-modal532-b6a846be-index.json) own current scope and immutable receipt mapping.

2026-10-19, ReviewShellApproval F1/F2. Initial regression additions were an explicitly authorized tests-only exception to the combined worktree freeze. Main subsequently observed behavioral RED in artifact454 and authorized minimal production repairs and inheritance of repaired un4p/EJ source into the combined tree. The approval-widget owner implements R1/R2 in combined source and the cumulative X snapshot; dependency/App/artifact propagation is separately owned. No test, build, lint, formatter, smoke, commit, push or tracker mutation was performed by this worker.

This record follows Gilfoyle's workflow contract and assessing-review-feedback stage. Governing obligations remain `.cyril-xavt/design.md` C2 (safe command/cwd display without loss) and C3 (actual drawn command and selected option/editor before answering), owned by plan Slice1. The authorized repair preserves behavior, ownership, allocation bounds and permission policy; it does not relax an inspection requirement. Parent owns observed RED, GREEN, gates and independent repair review.

## Findings

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Fully visible one-character shell commands cannot answer Allow or submit a rejection reason; permit a complete one-character command without treating whitespace, clipping or cwd as context. | ReviewShellApproval | Verified | Pre-fix ShellBody::command_shown required visible>1. Parent artifact454 observes all eight actual App response regressions failing despite visible command context, with82 existing positive cases retained. | Accept | Actual App regressions retained. Initial ShellRows pass now gathers meaningful-command-grapheme metadata capped at two; ShellBody requires positive min(2,total) visible unclipped meaningful graphemes. | Closed: repaired90 App cases pass in542; assembled688 and actual-delivery860 pass; fresh actual-source review correct/confidence0.98. Long commands still need two visible graphemes. |
| F2 | Scalar-width shell wrapping loses the closing quote of FE0F command/cwd boundary cases; count and wrap consistently with rendered graphemes so complete content is reachable. | ReviewShellApproval; ReviewShellApproval.ReviewCoreShellProjection | Verified | Pre-fix shell_rows summed scalar widths although ratatui rendered graphemes. Parent artifact454 observes all four FE0F command/cwd next/end cases seeing p or C instead of the closing quote. | Accept | Actual terminal-buffer/scroll/cwd-refusal regressions retained. Both command and cwd now use borrowed sanitized graphemes consistently for wrapping, counting and visibility. | Closed: correct explicit FE0F filter passes in575; assembled688 and actual-delivery860 pass; fresh actual-source review correct/confidence0.98. Cwd-only refusal retained. |
| F3 | Retained without-cwd fixture expands consent.triggeringResource to the full raw command; restore the exact captured `echo SLEPT` only. Mechanism: source-fixture fidelity. | Actual-X review relayed and repair authorized by Main | Verified | Direct reading of cited `kas-lead-interrupt-sig15-2.32.0.jsonl:56` shows triggeringResource=`echo SLEPT`, distinct from command/resource=`sleep 45 && echo SLEPT`; actual fixture line107 incorrectly repeated the latter. | Accept | Only `without_cwd.permission._meta.kiro.consent.triggeringResource` in shell-preview-2.32.0.json is restored to `echo SLEPT`; canonical eleven-file source manifest refreshed. | Closed before actual-delivery860 gates. Raw command/cwd, production and assertions unchanged; frozen combined source/patch untouched; prior behavioral review remains applicable. |

## Source evidence and fixture geometry

The initial source check used frozen pre-fix production, not a clean base or executed binary; inspected snapshot was approval.rs#709A. The source geometry below explains the subsequently observed parent RED and does not claim worker execution.

- Pre-fix `crates/cyril-ui/src/widgets/approval.rs:310-320`: the guard required at least two visible positive-width nonwhitespace command scalars. `:`, `w` could provide only one; neither path nor supplied cwd contributed command glyphs.
- The same file at `848-859`, `967-973` and `1138-1144`: both option confirmation and reason submission require the command guard. `crates/cyril-ui/src/state.rs:2523-2535` refuses a decision before popping its responder; `App::handle_approval_key` routes ordinary Enter and feedback Enter into that real state path.
- `approval.rs:96-105`: option desired width is 60, capped by `modal::popup_width`; shell body width subtracts the two borders and four-space `TEXT_INDENT`. At area 80x24 this leaves **54 body cells**.
- `crates/cyril-ui/src/widgets/modal.rs:14-27,43-62`: popup width 60 centers at x=10. `input_top=5` restricts placement to `[1,5)`, giving popup `(10,1,60,4)` and inner height 2. `OptionLayout::new` at `approval.rs:454-458`, called with `preview.has_context()`, reserves exactly one preview/body row and one selected-option row for a typed shell, even without a path. Thus body is y=2, text starts x=15, last body cell is x=68, selected action is y=3, and right border is x=69. The tests assert actual border/action cells and exactly one extracted body row; they do not call a production wrapper to derive expected content.
- Exact command: `format!("printf '{}#\u{fe0f}'", "x".repeat(44))`. Its eight-cell `printf '` prefix plus 44 `x` cells, `#` scalar width 1, FE0F scalar width 0 and final quote total 54 scalar-counted columns. The `#\u{fe0f}` grapheme occupies two rendered cells, making 55. In the first view it starts at `(67,2)` with continuation cell `(68,2)`; the final quote must be reachable at `(15,2)` after offset 1 or the end offset.
- Exact cwd: `format!("/{}#\u{fe0f}'", "x".repeat(46))`, rendered after the literal five-cell `Cwd: ` prefix. Prefix 5 + slash 1 + 46 `x` + scalar `#` 1 + FE0F 0 + final quote 1 again total 54 scalar columns but 55 rendered cells. `echo COMMAND_TOKEN` occupies row 0; offset 1 shows the cwd prefix/FE0F grapheme, and offset 2 or end must expose the cwd's closing quote. Both cwd-only views must remain unable to confirm.
- Pinned dependencies: `Cargo.lock` identifies ratatui 0.30.0, ratatui-core 0.1.0, ratatui-widgets 0.3.0, unicode-segmentation 1.12.0 and unicode-width 0.2.2. Local dependency source under `/home/dwalleck/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`: `ratatui-core-0.1.0/src/text/span.rs:306-315` segments graphemes; `ratatui-widgets-0.3.0/src/paragraph.rs:424-444,460-471` passes them through `LineTruncator` and advances by `symbol.width()`; `unicode-width-0.2.2/src/lib.rs:56-61` explicitly documents emoji-presentation string width differing from summed scalar width. `workflow_plan.rs:144-148` excludes FE0F from bidi-format sanitization, so the sequence is retained.

## R1 — one-character command response regressions

**Delivered:** retained tests and authorized minimal production repair, owned by xavt Slice1. App regressions:

- `xavt_app_whole_one_character_shell_command_can_allow`
- `xavt_app_whole_one_character_shell_command_can_submit_rejection_reason`

Each covers `:` and `w`, pathless and `VOJX_PATH` cases, reusing `xavt_request`, `approval_draw_at` (the actual `App::draw_frame` path), actual `handle_key` routing and the existing request's oneshot receiver. They require the exact whole command and supplied cwd in drawn body cells, visible path when present, actual Allow/Reject selection, and actual `Selected(allow)` or `RejectWithReason(reject,"why")` delivery. Reason entry goes through Down, `r`, and the actual paste event. The chat draft remains owned by chat; the answered request must release approval ownership. No production confirmation predicate is used to compute the expected answer, and the existing pipeline-specific two-character screen classifier is not reused for these whole-command cases.

**Performed checks/results:** parent-reported artifact454 executes90 App tests:82pass and8fail, exactly the colon/w Allow/reason cases with and without a path. These are genuine missing-response failures after visible context, not compile or setup failures. No worker check was run.

**Disposition:** closed by parent GREEN542, assembled gates688, actual-delivery gates860 and fresh actual-source ReviewShellApproval correct/confidence0.98; verification.md owns provenance. The original RED is retained without rerunning it.

## R2 — FE0F closing-quote scroll reachability regressions

**Delivered:** retained tests and authorized minimal production repair, owned by xavt Slice1. Widget regressions:

- `xavt_shell_fe0f_command_closing_quote_is_scroll_reachable` — next-row offset 1 and end offset `usize::MAX`.
- `xavt_shell_fe0f_cwd_closing_quote_is_scroll_reachable` — next-row offset 2 and end offset `usize::MAX`, preceded by the actual cwd-only first row at offset 1.

Both reuse `shell_approval(&command, cwd)` and `render_at(&state,80,24,5)`. Initial-view assertions read actual grapheme symbols and continuation/border cells, and verify one actual body row. Scrolled assertions require the literal closing quote at the actual first text cell `(15,2)`, as well as the selected Allow row. Existing `shell_body_rows`/`screen_rows` only extract already rendered cells; no replacement wrapper or independent width oracle was added. The cwd case preserves explicit negative `confirm_is_on_screen` assertions for both its first and final cwd-only views. Existing cwd-only App/widget regressions are untouched.

**Performed checks/results:** parent-reported artifact454 executes all four FE0F command/cwd next/end cases; all fail at the required quote cell with p or C instead. This is genuine unreachable-tail RED, not a compile failure. Earlier exact first-view grapheme checks retain FE0F and the reviewed geometry. No worker execution is claimed.

**Disposition:** closed by the correct explicit four-case FE0F GREEN575, assembled gates688, actual-delivery gates860 and fresh actual-source ReviewShellApproval correct/confidence0.98. The unrelated zero-match filter in542 is never counted as focused GREEN.

## Parent focused commands — not run by worker

Parent's pre-fix behavioral RED is recorded above; do not repeat it to reconfirm the reported defect. After the authorized repair is handed off ready, run the focused commands for GREEN:

```sh
env -u CARGO_TARGET_DIR cargo test --manifest-path /home/dwalleck/repos/cyril-wt-fix-cyril-xavt/Cargo.toml -p cyril xavt_app_whole_one_character_shell_command -- --nocapture
env -u CARGO_TARGET_DIR cargo test --manifest-path /home/dwalleck/repos/cyril-wt-fix-cyril-xavt/Cargo.toml -p cyril-ui xavt_shell_fe0f_ -- --nocapture
```

These are historical focused command references, not outstanding rerun instructions. verification.md owns the completed assembled and actual-delivery results and exact source boundaries; this finalization worker did not run repository checks.

## Review errors and repair review

The previous stack-handoff.md two-glyph requirement was too restrictive for a complete valid one-character command under design C3. Its text now matches the initial-pass threshold, without weakening response assertions. The older xavt_command_drawn helper remains a multi-character pipeline-screen classifier, not a whole-command oracle, so it is intentionally unchanged and not used by R1.

No other reviewer error was established by that source check. The October20 actual-source ReviewShellApproval reported correct/confidence0.98 with no actionable behavioral findings, specifically retaining the one-character and FE0F repairs and their regressions. F3 subsequently restored one reference field without changing the reviewed runtime behavior. That historical receipt and its applicability remain recorded in verification.md; the later modal source review is separately scoped below.

## Production authorization

Main after artifact454: “Authorize minimal R1wholeonecharacceptance + R2grapheme-consistent wrap/rowcount forcommand ANDcwd, sameborrowedsegmentationedge approved. You maynowpropagate repairedun4p/EJ snapshots into combined andfinishxavt production repairs; retainall82positive behaviors.” The shared primitive is the already-locked unicode-segmentation1.12.0 with explicit default-features=false, approved in the owning un4p repair record; combined manifests and UI lock edge now match that foundation. No new package/version or public API is introduced.

## Delivered production repair receipt

Both authorized approval.rs files received identical repair edits (tool snapshot3C97) after the cumulative repaired X widget was propagated into combined source. Private ShellRows returns row count and total meaningful command graphemes capped at two during the existing initial count pass; ShellBody reuses that count rather than rescanning the entire command for a threshold. Current-view guards count only positive-width nonwhitespace, fully fitting command graphemes. Zero meaningful content, clipping and cwd-only windows remain refused.

Private shell_graphemes preserves safe VS16/ZWJ clusters and borrows sanitized pieces. Controls/bidi become spaces, raw LF remains a newline, and CRLF preserves its prior space-then-newline display semantics. Both command and cwd use these same grapheme widths for row count, viewport rendering and visibility. Only requested viewport rows allocate strings; count/guard passes allocate none, range-end early stopping remains, and no complete sanitized copy is made. Raw typed command/cwd and grant semantics are unchanged.

Historical parent artifact542 executes all90 xavt App cases successfully and full workspace3117pass with13existing ignored tests. The attempted UI filter xavt_review_ matched zero cases: it is NOT four focused FE0F GREEN results, although the full suite executed UI tests. All-target Clippy then blocked on the replay helper's large enum variant; build did not run. The later correct FE0F filter575, approved owner-level representation repair, full combined gates688 and actual-delivery gates860 supersede that intermediate gate failure. No behavioral RED is hidden and no new check is claimed for the historical worker.

## Interim ReplayFrame gate repair — superseded

Parent artifact542 reported ReplayFrame::Permission616 versus routed Notification376 under large_enum_variant/-Dwarnings. Both helper payloads were temporarily boxed rather than shifting the imbalance from Permission to Notification. Parent575 then passed the correct four FE0F cases and workspace3117, but exposed ChatMessageKind::ToolCall296 versus SteerEcho73. Build still did not run. This established that the new inline optional shell context, not each containing enum, was the proper representation owner.

Main approved private Option<Box<ShellCommand>> at the existing ToolCall owner, preserving borrowed APIs and allocating only for an attached shell projection. The three source lines are applied in core types/tool_call.rs. Both interim ReplayFrame boxes, both construction wrappers and all four App consuming dereferences have been retired together. The helper/App edit receipts restore their pre-box snapshots3918/FD79; no net test-support API/heap delta remains, and every assertion is unchanged.

LSP references for the exported helper/variants returned inconsistent empty results before both interim change and retirement; full Rust-source fallback identified two constructors and two consuming helpers. Primary-worker references for ShellCommand/builder/getter also falsely returned empty; source fallback inventories seven builder, nine getter and eighteen domain request sites, all API-compatible. Tool defects were reported. Static source-derived layout bounds and Box-versus-Arc rationale are in design.md; actual final Clippy confirmation remains parent-owned. The obsolete03b-replay-support patch in frozen un4p outputs must not be applied; the updated X feature patch supersedes it.

## Historical October20 qualification boundary

At the October20 checkpoint, actual X source was reconstructed from the pinned latest feature delta with the approved private optional-context box, no per-ToolCall/chat heap layer and no ReplayFrame workaround. Combined gates688 remained historical; actual-delivery qualification was parent bg128/artifact860 after F3 and final formatting. Source was frozen after860; the later eleven-file freeze digest was not a contemporaneous gate-time tree hash. That binary's SHA-256 was33891699d8cfa71f5ce8e16fae3675b23c4e104d384a7f7d69416880ffebfb84. The then-fresh actual-source review and exact captured-wire actual-delivery RejectOnce proof complemented, but did not relabel, the one historical real-KAS run. These identities and results remain historical; the current held af78 checkpoint is recorded below and in verification.md.

## Historical actual-delivery fixture-fidelity repair

F3 belongs to Slice1's retained live-derived fixture under design C1 and C5. Main reports fresh actual-X independent review correct/confidence0.98 with no behavioral findings, then authorizes this exact one-field correction before gates. Reading the named capture line and fixture field establishes the mismatch directly; this is reference restoration, not an execution/permission behavior change or a new oracle.

Delivered: fixture triggeringResource matches `echo SLEPT`; actual full shell command remains `sleep 45 && echo SLEPT`, and cwd/production/assertions are unchanged. delivery-handoff.md retains the corrected pre-format reconstruction identity; verification.md owns the post-gate source freeze. ApprovalVisibility acknowledged retaining the correction in future authorized exports; frozen inputs remain untouched.

Evidence: direct source/fixture reading established reference restoration before final parent860 qualification. The prior source-only behavioral review is unchanged in scope by this one-field correction. Artifact860 then passes the actual-delivery full suite/all-target Clippy/build/format chain; the owned consumer proof passes with exact captured payloads, only one RejectOnce, no execution, no completed-frame replay, and a clean exit0. This closes the bounded repair and consumer documentation handoff; Main owns publication.

## Current modal repair, review and proof scope

This chronology adds Main's bounded modal DG without changing the original X requirements, F1/F2/F3 dispositions or representation-repair history. Canonical originals are mapped in the [portable index](evidence-current-modal532-b6a846be-index.json); [current verification](verification.md#current-held-af78-checkpoint) owns the detailed command/receipt record.

1. **Preserved original consumer failure.** On the pre-repair held af78 source, the captured892 case passed but the controlled case failed at reason wait with zero observations; all narrow controls were unreached. Original raw ANSI/result/error/cleanup remain retained. Frozen replay shows empty body/FF9E at `(8,12)`. The internal App Buffer wide-lead/diff-skip explanation is source-inferred, not observed internal instrumentation or an established timing cause.
2. **Added Main DG.** `local://approved-overlay-wide-boundary-repair.md`, SHA-256 `475c6c4ea2ff7f896b3f50397f4a1336cfbfa508d6b28f72d726248670bd5540`, explicitly approves existing modal ownership of whole intersecting painted spans at both edges, arbitrary actual released `CellWidth`, preserved disjoint cells/styles/rows, no production allocation/copy/normalization/backend/diff-flag workaround, and migration of twelve existing callers. [design.md's appended addendum](design.md#modal-ownership-repair--additional-main-dg) retains the original contract and approvals unchanged.
3. **Original RED and distinct oracle correction.** The retained original test-only run has exactly two decisive backend-emission failures at original `render.rs:1112`. Its third failure at1142 is a separate restoration-oracle error, not a third emission RED or proof that old controls completed. Corrected width-one ordinary-combining cases are disjoint: preserve outside cells/styles and require no close re-emission. Actual crossing-border, released diff/backend and whole-span restoration assertions remain; no historical mutation replay is claimed.
4. **Independent source review, then qualification.** The immutable source review reports NONE / correct / 0.97, source-only. Its pending-qualification wording records review-time chronology; Main later retains semantic applicability after authoritative rustfmt-only normalization. This is not a new review round. Current qualification preserves the initial fmt CHECK FAIL, followed by seven successful command records: authorized formatter, postformat CHECK, focused producer3, modal helper7, full default UI/App (1,574 passed, 3 ignored), strict Clippy and App bins build. The 532-input roster was pinned before current UI/App execution: descriptor `b6a846bee5f24ab0c2d0b1a48f066dd69ae575abb82bc7033c3f42a56ff48308`, private App SHA-256 `8e2b7c3a82b360d4ca4f1c1b6410084bf3a5a9e1673612d04bfd6c61739ee7f0`, held HEAD `bc6f48a21016320cc6491356f880f37463cf8c6e` plus MERGE_HEAD `af78b1615bd5b97b4c702542fd3ab193151bd4c1`. Original exact10 PASS and unchanged Core197/root manifests/lock/toolchain/config evidence are retained, not replayed; old530 execution pinning plus two posthoc fixtures is not relabeled old532.
5. **Accepted current owned-consumer scope.** Both captured-command and controlled cases passed exactly once: driver0/App0 each; Node/peer return codes **UNOBSERVED**. The corrected inventory records 22 observations (12+10), 24 scenario captures (13+11), plus two cleanup final screens; the count correction only included omitted03b/07b/07c metadata, without changing raw/results/assertions or rerunning. Each case sends one RejectOnce and executes nothing. Home/End refusal in both phases, path retention, the width12 raw/decoded pair and restored80 full command/path/selected Deny are covered by the current receipts, not retroactively by the original FAIL.

No native glyph/reflow, fresh live-KAS, final-integration MCP thirteenth-caller, CI, final assembly or whole-issue/global PASS is claimed. The source review and owned-consumer records retain their separate scopes. This metadata refresh runs no checks, formatter, runtime, Git or publication command.

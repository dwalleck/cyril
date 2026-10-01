# Plan: cyril-s2hb

## Inputs and design verification

Consumes route/spec/evidence and approved design dated 2026-09-30. F0 has observed native Linux/Windows proof; C1–C10 have complete falsifiers, independent oracles, named mutations and named checkpoint owners. File-backed capture's descriptor tradeoff is explicitly approved in spec/design. At initial approval no failed or waived fence was carried; the later explicit A1-only F19/F20 exceptions are owned by design.md and review-decisions.md. This plan declares no implementation complete.

Three executable slices remain: A1 complete resolved-shell prefix construction, A2 complete hidden gather/facts, and B complete cancellable diagnostics. A separate documentation-only contract amendment now precedes A2 publication after the assembled size forecast crossed the review tripwire. No executable seam, acceptance criterion or diagnostics staging changes.

## Review partition

Revised projections include artifacts and fixtures:
- A1 prefix: measured3,426 changed lines before the attempt5 cap handoff, plus raw-identifier correction, qualification cases and final records → **3,600**. Remaining churn margin10%360 → **3,960**. This supersedes2,800+20% after observing the completed lexical/scoped proof helper and replay. The smaller remaining percentage applies to largely implemented work, still reserves360 lines, and does not change the4,000-line partition rule. C10 helper cohesion review remains in design.md; no production interface or partition changes.
- A2-C contract amendment: four existing normative documents, including the affected plan update, forecast **400** lines; 20% margin80 → **480**.
- A2 native evidence after A2-C: the observed combined uncommitted draft is approximately3,974 lines; moving approximately255 contract-document lines to A2-C leaves approximately3,719. R16's raw-path repair/tests and final receipts project the implementation increment to **3,900**; remaining churn100 → **4,000**. Most source/fixtures already exist in that uncommitted draft; no completion or landing is claimed. No source/fixture compression or dropped guard is authorized. Recount after R16 and review; an overrun requires repartition, not a waiver.
- B diagnostics: production650 + tests/fixture330 + oracle220 + documentation/receipts170 = **1,370**; 20% margin274 → **1,644**.
- Sum **9,270**; churn margin **814**; total **10,084 > 4,000**. The four increments below are independently mergeable and individually at/below4,000 with their stated margins. Prior A2 estimates are superseded by this observed repair-driven partition.

Four independently mergeable PR increments under delegated Gilfoyle partition approval:
1. **S2HB-A1 native prefix**: prefix constructor, existing resolved-shell projection/carry, native shell proof and shared design/evidence. It builds and verifies against the discovered upstream without the uncommitted leaf/CLI draft.
2. **S2HB-A2-C functional contract amendment**: only normative spec/design/plan/route changes, based on merged A1. User quotations, unchanged safety obligations and staged ownership can be reviewed independently of the uncommitted implementation. No runtime code, checker/CI change, implementation-success receipt, README feature announcement or tracker closure lands here.
3. **S2HB-A2 native evidence**: the complete gather/facts leaf and hidden CLI, all permanent functional/error fences, runtime documentation, implementation receipts and CI. Based on merged A2-C; independently useful without diagnostics.
4. **S2HB-B native diagnostics**: complete synchronous diagnostics operation, based on merged A2 and executable through its library consumer without /review.

The issue remains in_progress until all executable increments and the contract amendment satisfy their review/CI/merge gates. At A2-C PR opening, R15 had completed implementation cycle13/15 and R16 was allocated cycle14/15; cycle12 is historical. Three full PR reviews preceded A2-C; PR150's initial full review is round4/5. These are opening-history pins, not a live remaining-budget claim: subsequent review consumption must be recorded in the PR review thread. A documentation PR consumes a review normally, never resets or evades the issue-wide cap. Every other issue retains five/five caps. No dependent issue starts before closure; no unrelated primary-checkout work is staged.

## Module growth ledger

Production-region convention and tripwire review procedure are in design.md. Ranges are forecasts, not targets to reach by compressing code. Baselines count retained existing cfg-test helper methods outside the terminal test module.

| Module | Baseline | Projected final | Responsibility/interface delta | Protected-parent rule |
|---|---:|---:|---|---|
| review lib.rs | 0 | 90–150 | exports and structured errors | no algorithms |
| review run.rs | 0 | 180–230 | run anchors, stamp, semantic manifest I/O, atomic replacement and UTF-8 text | A2 run-owner tripwire300 |
| review clock.rs | 0 | 70–120 | clock seam and UTC conversion | two actual adapters |
| review git.rs | 0 | 140–190 | explicit argv/target/error behavior | no shell |
| review gather.rs | 0 | 300–320 | gather orchestration | delegate facts/I/O; A2 Length review tripwire350 |
| review facts.rs | 0 | 533–560 | symbols/usages/docs/pages | no diagnostics/workflow; A2 Length review tripwire620 |
| review diagnostics/mod.rs | 0 | 170–220 | typed operation/report | reuse run I/O |
| review diagnostics/process.rs | 0 | 160–200 | direct child/captures/snapshots | no descendants/read threads |
| review diagnostics/command.rs | 0 | 130–185 | host command plan/environment | no implicit shell |
| core review/mod.rs | 0 | 140–160 | dialect/prefix; C8 Length review retained one owner | no resolver; tripwire190 |
| binary crtool.rs | 0 | 65–100 | clap and output/exit mapping | no run implementation |
| binary main.rs | 304 | 312–320 | early dispatch only | +20 maximum |
| core lib.rs | 19 | 20 | export only | +2 maximum |
| bridge.rs | 417 | 433–442 | resolved dialect carry/getter | +30 maximum |
| host_shell.rs | 478 | 486–492 | exhaustive dialect projection | +15 maximum |
| root/leaf/binary Cargo manifests | existing | +20 combined plus leaf metadata | leaf registration, semantic JSON and Unicode regex features | no weakened lints/extra runtime dependencies |

The sum of range upper bounds is not the diff estimate: it includes whole files and independent uncertainty. Design tripwires bound growth review, while actual cumulative changed lines govern PR partition. Any overrun triggers the recorded Length review before changing its threshold. No shared gate is replaced.

## Slice A1: construct safe native executable prefixes

**Claim IDs:** C8; introduces the C10 standalone ownership fence, whose scope expands with subsequent slices.
**Expected behavior:** canonical current executable becomes a dialect-correct, forward-slash, double-quoted prefix; listed hazards and native PowerShell delimiter characters refuse; the dialect is projected from the single existing resolved HostShell.
**Oracle:** literal prefix strings, actual native shell argv/output, Windows PowerShell5.1/pwsh7.6.6 parser observations, and the approved module ledger.
**Stress fixture:** spaced/Unicode/native drive/UNC paths, each hazard in otherwise-valid absolute paths, valid apostrophes and non-active typographic quotes, PowerShell-only U+201C/D/E, invalid UTF-8, relative/device forms, and long native path spelling. The valid control succeeds; each unsafe case returns its own typed refusal.
**Regression fence:** core review behavioral tests and `.cyril-s2hb/oracles/check_shape.py --phase prefix` for source ownership. Attempt9 replaces the unsound token-flow predicate with `cargo run -p cyril-core --features kas --profile test --example review_prefix -- <shell> <dialect>` in the existing native CI matrix; this consumer executes the generated prefix and checks exact child argv and bridge shutdown. Remove the obsolete issue-local resolved probe after promotion. Its separate path-hazard probe remains applicable.
**Named mutation:** omit dollar guard and force POSIX opening for PowerShell (C8); add forbidden `review_gather_body` to a protected parent (C10). Their independently observed red/restored-green receipts remain required. The replay additionally executes native CI against decoy/discard/early-return/shadow variants and valid macro/raw/closure controls, but its runtime verdicts are diagnostic only under design.md's explicit F19 exception: **N/A — approved risk: F19 bypassed for A1 only**. Child-role rejection retains its direct red/green proof. Source and runtime verdicts stay separate; no operational failure is claimed as semantic detection. Mutation builds use an owned target and refreshed archived crate-root timestamps.
**Complexity/production scale:** validation and spelling are O(N) path bytes, with one output allocation sized for normalized path plus fixed opening/suffix; no subprocess per character and no new persistent loop. The long-path case verifies exact output and actual execution, while source census verifies one reserved String and append-only construction. Accepted storage/work are linear, not a constant input cap.
**Wall budget/phase:** one-off prefix creation; N/A — no latency SLA. Existing bridge resolution remains once per startup.
**Module shape:** core review owns spelling; HostShell only projects its kind; BridgeHandle only carries the optional dialect. Core lib <=2, bridge <=30, HostShell <=15 production-line growth; main unchanged. Prefix retain-and-raise disposition is in design.md's C8 Length review.
**Files:** new core `src/review/mod.rs`; existing core lib.rs/protocol/bridge.rs/protocol/kas/host_shell.rs; shared issue-local route/spec/evidence/design/plan/probes, prefix checkpoint and shape oracle; existing CI KAS lane gains the standalone prefix ownership gate with full checkout history. Attempt9 promotes the resolved probe to core `examples/review_prefix.rs` and adds its native runtime step to CI's existing OS matrix. PR146's bounded qualification repairs also change cfg(unix) version.rs fixtures, add two executable fixture scripts, and add the source/runtime replay. All are owned by review-decisions.md; leaf/CLI/parity draft files remain excluded.
**Estimate:** one implementation/verification cycle; estimate is not a gate.
**Diff estimate:**3,600 plus10% remaining churn margin. Attempt9's net replacement/native replay is charged to that remaining margin; measure the assembled cumulative delta before committing, never waive the4,000-line partition. Production growth, independent mergeability and downstream estimates remain unchanged.
**PR increment:** S2HB-A1 native prefix.
**Commands and expected results:**
- `cargo test -p cyril-core --features kas review::` → exact valid prefixes and typed hazard/namespace refusals; actual POSIX fixture emits `crtool`.
- Native `review_prefix` through bash/fish and installed Windows PowerShell5.1/pwsh7.6.6 → resolved dialect, generated command, exact child argv and shutdown. CI also executes bash on macOS. Retained separate native path-hazard/drive/UNC proof remains applicable to unchanged production bytes.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase prefix` → approved path/count/protected-parent checks; C10 mutation red/restored green. Prefix responsibility remains directly reviewed; its missing automated enforcement is **N/A — approved risk: F20 bypassed for A1 only**, not proved by this command.
- C8 focused tests under each named mutation → localized red; exact restoration → green.
- `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, `cargo check -p cyril --features kas` → independently green prefix-only tree, with no leaf/CLI draft dependency.
- Native smoke, isolated production reconstruction/ledger comparison, fresh PR review and required current-head CI precede merge.

## Slice A2-C: publish the approved functional contract

**Claim IDs:** N/A — requester-authorized specification/verification correction; executable C1–C10 remain assigned to A1/A2/B.
**Expected behavior:** the published normative artifacts match the requester's functional-equivalence correction without claiming an unlanded implementation complete.
**Oracle:** requester quotations and the approved responsibility/safety contract, inspected directly; N/A — no executable implementation comparison in this documentation-only slice.
**Stress fixture:** N/A — documentation only.
**Regression fence:** N/A — no executable gate, validator or policy implementation changes.
**Named mutation:** N/A — no executable fence changes; A2 retains its assigned behavioral/mutation obligations.
**Complexity/production scale:** N/A — no runtime loop or allocation changes.
**Wall budget/phase:** N/A — documentation only.
**Module shape:** N/A — no production modules/parents change; the approved ledger remains normative for subsequent code.
**Files:** `.cyril-s2hb/{spec,design,plan,route}.md`, normative amendment hunks only; implementation receipts stay with A2.
**Estimate:** one documentation review/verification change; not a completion gate.
**Diff estimate:**400 plus20% churn.
**PR increment:** S2HB-A2-C functional contract amendment.
**Commands and expected results:** inspect the exact four-file diff against requester quotations and staged source ownership; `git diff --check` passes; production tree hashes equal the reviewed A1 baseline, preserving its runtime evidence. Run the unchanged CLI help smoke, repository quality gates and required current-head CI; obtain a fresh independent PR review. Do not infer code acceptance from this contract publication.

**A2-C pre-commit checkpoint:** base `422d16467aa48d8956d11dcc3b752fa91d53bd6f`; only the four normative Markdown files differ, so production and executable policy remain identical to reviewed A1. `git diff --check` passed. With the reused A1 target cache and four build jobs, `cargo fmt --check`, `cargo test` (2,096 passed), all-target Clippy with `-D warnings`, and actual `cargo run -p cyril --bin cyril -- --help` passed (`artifact://1069`); help reports the unchanged ordinary options. This is documentation qualification, not A2 implementation acceptance. Fresh PR review/current-head CI remain mandatory before merge.

| Checkpoint item | State for A2-C only |
|---|---|
| 1 affected tests | PASS — unchanged workspace suite and quality commands above. |
| 2 falsifiers | N/A — no executable claim implemented; all product falsifiers remain with A1/A2/B. |
| 3 stress fixture | N/A — documentation only. |
| 4 implementation/oracle | N/A — no executable implementation change; normative text follows the quoted requester correction. |
| 5 module shape | N/A — no production path/interface/dependency/parent changes. |
| 6 production/wall budgets | N/A — neither runtime loops nor latency behavior change. |
| 7 regression fence | N/A — no executable fence changes. |
| 8 named mutation | N/A — no executable fence changes. |
| 9 restored fence | N/A — item8 requires no mutation. |
| 10 parity/reuse | PASS — shared contract wording is updated in its existing owners; formatting-only distinctions change while semantic fields/order/content/status/safety remain required. |
| 11 preserved enforcement | PASS — requester quotations authorize the normative byte-to-functional amendment; no code/checker/CI rule is removed, and A1's two explicitly scoped exceptions are unchanged. |

**A2-C review repair:** F27 Verified/Accept — pinned head `8b748154` has no A2 checkpoint, so this normative increment now records C1–C4 as PENDING and assigns evidence to the future implementation checkpoint, not an absent current artifact. F28 Verified/Modify — stale cycle12/current-count claims are removed; Review partition owns explicit historical opening pins (13 completed,14 allocated; initial PR review4/5), while later review consumption belongs to the PR thread. No cap, acceptance requirement or executable behavior changes.

## Slice A2: gather and inspect native review evidence

**Claim IDs:** C1, C2, C3, C4, C9; extends C10 to the new leaf/binary owners; retains valid A1 C8 proof.

**Expected behavior:** actual `cyril crtool gather`/`facts` produce all stamped run artifacts and functional status/error results; safe prefix matches the already-resolved dialect; ordinary startup/help remain unaffected. A default build works without Python or KAS. The deterministic fixture example supplies clock values through the same public leaf operations, never production flags.

**Oracle:** design C1/C2/C4 independent Python functional reference with explicit shared timestamp/version, plus semantic helper/caller/doc expectations and direct Git content; C3's new native stamp refusal uses independently specified category/context and unchanged-file observations; C8 literal prefix/native shell argv; C9 actual process/filesystem observations; C10 compiler/dependency checks, direct ownership inspection and final isolated conformance, with existing narrow accidental-placement tripwires. No semantic field, type, array order, or meaningful content is removed or normalized.

**Stress fixture:** multi-language definitions and Unicode names; POSIX non-UTF8 Git name plus its distinct replacement-character look-alike; >40 usages; 41 changed docs outside scope; zero/binary/deleted diff; duplicate scopes; stale manifest and missing facts; page caps/unsplit lines; conflicting main/master refs; invalid config for early dispatch; prefix hazards with valid controls. Preserve semantic content/order and declared refusals; raw filename identity must not be replaced by its display label.

**Regression fence:** leaf public-operation integration cases; `.cyril-s2hb/oracles/parity.py --phase gather`; `.cyril-s2hb/oracles/check_shape.py --phase gather`; binary `crtool` subprocess smoke. The differential run is permanent functional-equivalence tooling, not a production source-string test. Fixture driver `crates/cyril-review/examples/parity_driver.rs` is verification-only and is not installed as a Cyril command.

**Named mutation:** C1 corrupts/drops a manifest field, value, or type (replacing the formatting-only manifest-key-order mutant; no newline/quoting/format-only mutation); C2 master-before-main and empty-exit2; C3 bypass stamp validation; C4 truncate stored usages to40; C9 dispatch after config; C10 add forbidden review_gather_body to main. Each new/changed fence must fail on its named observable, then pass after exact restoration. C8 retains A1's still-applicable mutation proof rather than repeating it solely because another slice landed. Parent owns safe in-place mutation and preserves other edits.

**Complexity/production scale:** Git calls at most F + S + 20 for F changed files/S retained symbols; no Git call per usage hit. Facts may scan repository text once per symbol: O(S×R + H), R searchable bytes/H returned hits. Page retries are at most four complete rendering passes; semantic JSON/text conversion O(B) bytes; retained diff, patches, usages/output use O(B+H) storage. Scale fixture: 100 files, 200 definitions, >40 usages per definition, 2 MiB patch/text input; accepted work ceiling is F+S+20 Git calls and four page passes, not an invented wall-time target. No fixed input cap is introduced; corresponding formulas, rather than constant memory, are the accepted bound for larger inputs. Prefix is one O(path characters) scan; UTC conversion constant arithmetic. The functional reference records fixture counts and source loop census to exclude hidden per-hit subprocess work.

**Wall budget/phase:** one-off CLI/operation/prefix phases; N/A — no one-off latency obligation. No always-on loop/background task is introduced. Complexity is checked structurally and through the scale fixture, not by a flaky timing assertion.

**Module shape:** leaf owns run/Git/gather/facts/clock; core owns prefix; binary owns dispatch. Protected deltas: main <=20, core lib <=2, bridge <=30, HostShell <=15 production lines. The design's A2 Length review retains gather/facts at350/620 tripwires with unchanged ownership; no source compression. `check_shape.py --phase gather` covers its declared paths/dependencies/protected-parent/growth cases, not arbitrary Rust semantics. Parent directly inspects every ledger boundary; final isolated conformance remains authoritative.

**Files:** create `crates/cyril-review/Cargo.toml`, `src/{lib,run,clock,git,gather,facts}.rs`, `examples/parity_driver.rs`, `tests/evidence.rs`, and `crates/cyril/src/crtool.rs`; modify root Cargo.toml/Cargo.lock, binary Cargo.toml/main.rs; add the functional-equivalence oracle and canonical fixture builders; extend the already-committed shape oracle/CI phase to gather. Update only the cyril-s2hb record in `.rivets/issues.jsonl` with the requester's revised acceptance; parent cyril-5gb3 and unrelated records remain byte-identical. Existing core prefix/projection files are inherited from merged A1, not reintroduced. Update `README.md` and `AGENTS.md` after smoke; no unrelated skills or primary-checkout changes.

**Estimate:** one implementation/verification cycle, potentially several hours of native qualification; not a completion gate.

**Diff estimate:**3,900 after the separate normative amendment, plus100 remaining churn, including R16 and all existing fences/receipts.

**PR increment:** S2HB-A2 native evidence.

**Commands and expected results:**
- `cargo build -p cyril -p cyril-review --examples` and `cargo build -p cyril` → build actual default Cyril and fixture driver; no missing diagnostics module/stub.
- `cargo test -p cyril-review --test evidence` → helper definition/caller/doc, stamp/no-recompute, target precedence and page-boundary expected values.
- `python3 .cyril-s2hb/oracles/parity.py --phase gather --cyril target/debug/cyril --driver target/debug/examples/parity_driver` → semantic JSON/record/content/status equivalence for all enumerated same-host cases; actual CLI timestamp falls within independent invocation bounds; hidden help/startup assertions execute.
- Same command with native `.exe` paths/Python on resourcefs-win11 → binary/Unicode meaningful-content checks and native PowerShell prefix execution; Linux shell smoke executes POSIX prefix. CI's existing macOS tooling lane runs its own functional fixture.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase gather` → approved paths/dependencies/protected parents and module growth; named C10 mutation red/restored green.
- Focused commands above under each named mutation → localized expected failure; exact source restoration returns green.
- `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, plus `cargo test -p cyril-core --features kas` and `cargo check -p cyril --features kas` → formatting/workspace/current shell-projection gates pass; the KAS feature branch is actually compiled.
- Actual hidden CLI smoke is mandatory beyond unit tests. Then isolated production reconstruction + independent ledger comparison, PR review, current-head required CI, merge. Existing docs updated only after smoke proves the behavior.

## Slice B: execute cancellable native preflight diagnostics

**Claim IDs:** C5, C6, C7; C1/C3/C10 retained/extended and reverified for the new consumer. F0 retained mechanism proof applies because the same std file-handle strategy is selected.

**Expected behavior:** public diagnostics executes the explicit native command, preserves lossless raw captures and meaningful filtered evidence, and returns typed clean/nonzero/timeout/cancelled outcomes; cannot-start/parse/stamp failures write no reports; pre-cancel does not launch; live cancellation kills/reaps only its owned direct child. No diagnostics CLI verb exists.

**Oracle:** C5 uses functional command/argv/environment behavior plus literal child receipts; C6 uses meaningful evidence/status/content and report limits; C7 uses OS process status and independently controlled handshakes/holder; deterministic deadline/first-observation cases are not fake real-time success.

**Stress fixture:** 131,073 bytes on each stream including invalid UTF-8; 201 changed-path matches and 16 tail lines; child-emitted CRLF, if present, remains raw capture data; hostile quoted argument shapes; failed exit and signal; zero/default timeout; cancel before launch/after ready/after exit; descendant still writing after direct child exits; spawn failure and cleanup errors. Expected partial snapshots contain only bytes before sampled lengths, and holder survives until fixture-owned release.

**Regression fence:** `crates/cyril-review/tests/diagnostics.rs` with a native fixture child mode in the verification driver; `.cyril-s2hb/oracles/parity.py --phase diagnostics`; permanent native owned-process cancellation qualification. Checks assert functional statuses/content and lossless raw captures. No test kills processes by name or relies on a shared current directory/environment mutation.

**Named mutation:** C5 splits Windows raw tail/removes nonempty CARGO_ value; C6 reverses stream order or uses a199 match cap; C7 omits direct-child kill/reads past snapshot length. Deterministic terminal-decision tests also invert exit/cancel observation order. Native control proves holder/unrelated sentinel are live, so absence assertions cannot pass vacuously.

**Complexity/production scale:** command parsing O(C) characters; Windows executable candidates bounded by whitespace positions in C, with no successful-launch retry. Capture is O(B) disk/output bytes, two distinct files, zero reader threads, one direct child. Polling uses a fixed 10 ms sleep while live; terminal deadline decisions use monotonic samples, no busy-spin. Snapshot reads are capped by two sampled lengths even if holders keep appending. Filtering O(L×F) for L output lines/F changed paths (matching functional path semantics); reports retain at most200 matches/15 nonempty tail lines; no raw-output cap. Scale fixture 32 MiB dual-stream output and100 changed paths; accepted bound is two capture files, no reader thread, one spawn, at most sampled stdout+stderr+separator bytes and source report limits. It does not promise bounded disk cost for arbitrary input volume.

**Wall budget/phase:** one-off diagnostics operation; user/default timeout is the running deadline and direct-child termination/reap has the approved one-second deadline. Deterministic virtual observation tests enforce deadline/precedence decisions; real native process fixtures establish actual reaping/cleanup, with outer timeout solely a hang guard. N/A — no separate latency SLA for filesystem output, no always-on background task.

**Module shape:** create diagnostics/{mod,process,command}.rs; lib gains only exports/error variants; run owns unchanged stamp/semantic JSON/native text implementation. No protected-parent production changes are required. `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics` verifies final ledger/dependencies and growth; re-run affected C10 mutations if fence changes. Return to design before changing ownership or thresholds.

**Files:** create `crates/cyril-review/src/diagnostics/{mod,process,command}.rs` and `tests/diagnostics.rs`; extend leaf lib.rs and verification example; extend parity/shape oracle and native CI phase; update existing docs after actual diagnostics smoke. No crtool clap diagnostics variant.

**Estimate:** one substantive implementation/verification cycle, potentially several hours of native qualification; not a completion gate.

**Diff estimate:** 1,370 changed lines plus20% margin.

**PR increment:** S2HB-B native diagnostics, after A2 merges.

**Commands and expected results:**
- `cargo test -p cyril-review --test diagnostics` → typed statuses, lossless raw captures, meaningful rendered content, no report on prelaunch errors, deadline/terminal precedence, owned lifecycle assertions.
- `cargo build -p cyril-review --examples` then `python3 .cyril-s2hb/oracles/parity.py --phase diagnostics --cyril target/debug/cyril --driver target/debug/examples/parity_driver` → functional status/content/record equivalence for identical clock/command/manifest inputs, lossless raw-capture checks, and separate actual-system-clock library smoke.
- Native Windows equivalent → raw command-tail argv semantics, Windows exit-code treatment, lossless child evidence, meaningful rendered content, cancellation and holder-cleanup observations. CI macOS executes its own host fixture.
- Each mutation's focused case → named red output; restore → green. Record source hash/environment and observed terminal values.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics`, `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings` → final assembled gate passes; no source fence runs from production tests.
- Actual library consumer smoke, independent production reconstruction/ledger comparison, fresh PR review and current-head CI must pass before merge. Verify every planned increment's merge revision (A1, A2-C, A2 and B) and all acceptance; close cyril-s2hb without changing parent cyril-5gb3.

## Self-review and handoff

All design claims remain assigned to their executable slices; A2-C changes only their already-approved normative record. C10 grows only with actual owners. The new documentation slice records all fourteen fields; inherited executable fields and same-slice fences remain intact. Four independently mergeable increments preserve acceptance, readable source and existing guards. No slice/checkpoint is declared PASS here.

## Planning amendments, 2026-09-30

C8 Length review in design.md retains the cohesive prefix owner and raises its tripwire to190, with updated forecast140–160 (baseline0) and30-line uncertainty. No ownership/interface or protected-parent delta changes. EvidenceParity reports the full readable oracle/golden tooling exceeds600 lines; retain required coverage and recompute the review partition from actual assembled diff before checkpoint/commit rather than compressing fixtures. Any increment exceeding4,000 remains blocked until repartitioned into independently mergeable slices.

## Functional-equivalence amendment — 2026-09-30

Requester correction (verbatim):
> “It didn't need to be a 1 to 1 copy of the python script, just functionally the same”

Further requester clarification (verbatim):
> “The results and output don't have to be byte equivalent”

This supersedes active byte/text-wrapper parity requirements. A2 checks preserve semantic JSON fields/types and array order, file/symbol/usage sequence, meaningful content, stamps/refusals, exit/status/error/context, page budgets/caps, patch content, and native safety/lifecycle gates. Scope parsing/trimming uses Rust standard Unicode whitespace; generated text uses UTF-8 without a host-specific adapter or native LF-only gate. B uses functional command/status/content checks with lossless raw captures, not Python wrapper-byte parity. The C1 formatting-only manifest-key-order/newline mutant is replaced by manifest data corruption; no format-only mutation is required.

Pre-amendment byte comparisons, host-specific CRLF/CRCRLF checks, and exact stdout/stderr wording are historical evidence only. A2 acceptance, including C1–C4, is PENDING in this documentation-only increment; its future implementation checkpoint will own results and outstanding qualification. The run-owner projection is now 180–230 lines with a 300-line tripwire, without changing ownership. Publication counter history and the no-reset rule are recorded in Review partition above.

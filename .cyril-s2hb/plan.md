# Plan: cyril-s2hb

## Inputs and design verification

Consumes route/spec/evidence and approved design dated 2026-09-30. F0 has observed native Linux/Windows proof; C1–C10 have complete falsifiers, independent oracles, named mutations and named checkpoint owners. File-backed capture's descriptor tradeoff is explicitly approved in spec/design. No FAIL or waived fence is carried. This plan declares no implementation complete.

Three atomic slices: A1 delivers complete resolved-shell prefix construction; A2 delivers complete hidden gather/facts; B delivers complete cancellable diagnostics. This repartition replaces the original two-slice forecast after observing the complete source/harness sizes. No diagnostics declaration/stub appears before B. C10's standalone fence is introduced by A1, then extended and reverified for A2/B ownership; C8 is discharged by A1, C1–C4/C9 by A2, and C5–C7 by B. F0 remains retained mechanism evidence for B.

## Review partition

Revised projections include artifacts and fixtures:
- A1 prefix: measured1,948 changed lines at1dae375 plus an850-line qualification-repair forecast, rounded to **2,800**; 20% margin560 → **3,360**. This supersedes the original1,800 forecast. The increase covers immutable native fixtures, current-wiring/CI guard repair, rerunnable A/B evidence and bounded repair records; no production interface or partition changes.
- A2 gather/facts: leaf/binary production and wiring 1,550 + tests/driver 370 + parity harness 800 + compact canonical goldens/CI/docs/receipts 380 = **3,100**; 20% margin620 → **3,720**. The broad live differential matrix remains complete; retained goldens use a complete small helper/caller/NOTES scenario rather than redundantly snapshotting the large >40-usage stress fixture.
- B diagnostics: production650 + tests/fixture330 + oracle220 + documentation/receipts170 = **1,370**; 20% margin274 → **1,644**.
- Sum **7,270**; churn margin **1,454**; total **8,724 > 4,000**. Actual assembled source already measures1,412 leaf lines,184 integration-test lines,184 fixture-driver lines and775 parity-harness lines, replacing the underestimated forecasts. Final staged diff still decides each increment's size gate.

Three independently mergeable PR increments under the delegated Gilfoyle review partition:
1. **S2HB-A1 native prefix**: prefix constructor, existing resolved-shell projection/carry, native shell proof and shared design/evidence. It builds and verifies against the discovered upstream without the uncommitted leaf/CLI draft.
2. **S2HB-A2 native evidence**: gather/facts leaf and hidden CLI, with same-host byte parity and compact complete canonical goldens. Based on merged A1; independently useful without diagnostics.
3. **S2HB-B native diagnostics**: complete synchronous diagnostics operation, based on merged A2 and executable through its library consumer without /review.

The issue remains in_progress until all three increments satisfy joint acceptance, clean reviews, CI and merge. The issue-wide five implementation/five PR-review-round caps do not reset per increment. No dependent issue starts before closure. Isolated increment worktrees preserve the uncommitted downstream draft; no unrelated primary-checkout work is staged.

## Module growth ledger

Production-region convention and tripwire review procedure are in design.md. Ranges are forecasts, not targets to reach by compressing code. Baselines count retained existing cfg-test helper methods outside the terminal test module.

| Module | Baseline | Projected final | Responsibility/interface delta | Protected-parent rule |
|---|---:|---:|---|---|
| review lib.rs | 0 | 90–150 | exports and structured errors | no algorithms |
| review run.rs | 0 | 190–250 | run anchors, stamp and ordered atomic/text I/O | N/A — new deep helper |
| review clock.rs | 0 | 70–120 | clock seam and UTC conversion | two actual adapters |
| review git.rs | 0 | 140–190 | explicit argv/target/error behavior | no shell |
| review gather.rs | 0 | 140–210 | gather orchestration | delegate facts/I/O |
| review facts.rs | 0 | 280–340 | symbols/usages/docs/pages | no diagnostics/workflow |
| review diagnostics/mod.rs | 0 | 170–220 | typed operation/report | reuse run I/O |
| review diagnostics/process.rs | 0 | 160–200 | direct child/captures/snapshots | no descendants/read threads |
| review diagnostics/command.rs | 0 | 130–185 | host command plan/environment | no implicit shell |
| core review/mod.rs | 0 | 140–160 | dialect/prefix; C8 Length review retained one owner | no resolver; tripwire190 |
| binary crtool.rs | 0 | 65–100 | clap and output/exit mapping | no run implementation |
| binary main.rs | 304 | 312–320 | early dispatch only | +20 maximum |
| core lib.rs | 19 | 20 | export only | +2 maximum |
| bridge.rs | 417 | 433–442 | resolved dialect carry/getter | +30 maximum |
| host_shell.rs | 478 | 486–492 | exhaustive dialect projection | +15 maximum |
| root/leaf/binary Cargo manifests | existing | +20 combined plus leaf metadata | leaf registration, ordered JSON and Unicode regex features | no weakened lints/extra runtime dependencies |

The sum of range upper bounds is not the diff estimate: it includes whole files and independent uncertainty. Design tripwires bound growth review, while actual cumulative changed lines govern PR partition. Any overrun triggers the recorded Length review before changing its threshold. No shared gate is replaced.

## Slice A1: construct safe native executable prefixes

**Claim IDs:** C8; introduces the C10 standalone ownership fence, whose scope expands with subsequent slices.
**Expected behavior:** canonical current executable becomes a dialect-correct, forward-slash, double-quoted prefix; listed hazards and native PowerShell delimiter characters refuse; the dialect is projected from the single existing resolved HostShell.
**Oracle:** literal prefix strings, actual native shell argv/output, Windows PowerShell5.1/pwsh7.6.6 parser observations, and the approved module ledger.
**Stress fixture:** spaced/Unicode/native drive/UNC paths, each hazard in otherwise-valid absolute paths, valid apostrophes and non-active typographic quotes, PowerShell-only U+201C/D/E, invalid UTF-8, relative/device forms, and long native path spelling. The valid control succeeds; each unsafe case returns its own typed refusal.
**Regression fence:** core review module behavioral tests and `.cyril-s2hb/oracles/check_shape.py --phase prefix`; native host qualification executes prefix construction and generated shell commands rather than merely compiling cfg branches.
**Named mutation:** omit dollar guard and force POSIX opening for PowerShell (C8); add forbidden `review_gather_body` to a protected parent (C10). Observe the corresponding refusal/prefix/ownership failures and restored green.
**Complexity/production scale:** validation and spelling are O(N) path bytes, with one output allocation sized for normalized path plus fixed opening/suffix; no subprocess per character and no new persistent loop. The long-path case verifies exact output and actual execution, while source census verifies one reserved String and append-only construction. Accepted storage/work are linear, not a constant input cap.
**Wall budget/phase:** one-off prefix creation; N/A — no latency SLA. Existing bridge resolution remains once per startup.
**Module shape:** core review owns spelling; HostShell only projects its kind; BridgeHandle only carries the optional dialect. Core lib <=2, bridge <=30, HostShell <=15 production-line growth; main unchanged. Prefix retain-and-raise disposition is in design.md's C8 Length review.
**Files:** new core `src/review/mod.rs`; existing core lib.rs/protocol/bridge.rs/protocol/kas/host_shell.rs; shared issue-local route/spec/evidence/design/plan/probes, prefix checkpoint and shape oracle; existing CI KAS lane gains the standalone prefix ownership gate with full checkout history. PR146's bounded qualification repairs additionally change the cfg(unix) version.rs fixtures and add two executable fixture scripts plus the A/B replay harness, as owned by review-decisions.md. Leaf/CLI/parity draft files are excluded from this increment.
**Estimate:** one implementation/verification cycle; estimate is not a gate.
**Diff estimate:**2,800 plus20% margin; affected-only budget update for PR146 qualification repairs under the existing approved partition.
**PR increment:** S2HB-A1 native prefix.
**Commands and expected results:**
- `cargo test -p cyril-core --features kas review::` → exact valid prefixes and typed hazard/namespace refusals; actual POSIX fixture emits `crtool`.
- Native Windows same test selection plus a temporary real library consumer through installed PowerShell5.1/pwsh7.6.6 → drive/UNC/dialect behaviors execute on Windows, with actual shell output.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase prefix` → only approved prefix/projection ownership and protected-parent deltas; C10 mutation red/restored green.
- C8 focused tests under each named mutation → localized red; exact restoration → green.
- `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, `cargo check -p cyril --features kas` → independently green prefix-only tree, with no leaf/CLI draft dependency.
- Native smoke, isolated production reconstruction/ledger comparison, fresh PR review and required current-head CI precede merge.

## Slice A2: gather and inspect native review evidence

**Claim IDs:** C1, C2, C3, C4, C9; extends C10 to the new leaf/binary owners; retains valid A1 C8 proof.

**Expected behavior:** actual `cyril crtool gather`/`facts` produce all stamped run artifacts and exact stdout/error/exit behavior; safe prefix matches the already-resolved dialect; ordinary startup/help remain unaffected. A default build works without Python or KAS. The deterministic fixture example supplies clock values through the same public leaf operations, never production flags.

**Oracle:** design C1–C4 Python reference with explicit shared timestamp/version, plus literal expected helper/caller/doc and direct Git bytes; C8 literal prefix/native shell argv; C9 actual process/filesystem observations; C10 approved ledger versus independent census. No fields are removed or normalized from byte equality.

**Stress fixture:** multi-language definitions and Unicode names; >40 usages; 41 changed docs outside requested scope; zero/binary/deleted diff; duplicate scopes; stale manifest and missing facts; page caps on >20,000 characters and long unsplit line; conflicting main/master refs; invalid config for early dispatch; every unsafe prefix character with a valid spaced-path positive control. Expected results are the exact source-compatible fields/bytes and declared exit 2/3 refusals.

**Regression fence:** leaf public-operation integration cases; `.cyril-s2hb/oracles/parity.py --phase gather`; `.cyril-s2hb/oracles/check_shape.py --phase gather`; binary `crtool` subprocess smoke. Cross-host oracle run is permanent qualification tooling, not a production source-string test. Fixture driver `crates/cyril-review/examples/parity_driver.rs` is verification-only and is not installed as a Cyril command.

**Named mutation:** C1 reorder manifest keys (and Windows newline mutation on native host); C2 master-before-main and empty-exit2; C3 bypass stamp validation; C4 truncate stored usages to40; C9 dispatch after config; C10 add forbidden review_gather_body to main. Each new/changed fence must fail on its named observable, then pass after exact restoration. C8 retains A1's still-applicable mutation proof rather than repeating it solely because another slice landed. Parent owns safe in-place mutation and preserves other edits.

**Complexity/production scale:** Git calls at most F + S + 20 for F changed files/S retained symbols; no Git call per usage hit. Source-compatible facts may scan repository text once per symbol: O(S×R + H), R searchable bytes/H returned hits. Page retries are at most four complete rendering passes; JSON/text conversion O(B) bytes; retained diff, patches, usages/output use O(B+H) storage. Scale fixture: 100 files, 200 definitions, >40 usages per definition, 2 MiB patch/text input; accepted work ceiling is F+S+20 Git calls and four page passes, not an invented wall-time target. No fixed input cap is introduced; corresponding formulas, rather than constant memory, are the accepted bound for larger inputs. Prefix is one O(path characters) scan; UTC conversion constant arithmetic. The oracle records fixture counts and source loop census to exclude hidden per-hit subprocess work.

**Wall budget/phase:** one-off CLI/operation/prefix phases; N/A — no one-off latency obligation. No always-on loop/background task is introduced. Complexity is checked structurally and through the scale fixture, not by a flaky timing assertion.

**Module shape:** leaf owns run/Git/gather/facts/clock; core owns prefix; binary owns dispatch. Protected deltas: main <=20, core lib <=2, bridge <=30, HostShell <=15 production lines. `python3 .cyril-s2hb/oracles/check_shape.py --phase gather` must print C10 PASS and localize injected forbidden body/import/growth failures. Length-review decisions/tripwires are those in design.md.

**Files:** create `crates/cyril-review/Cargo.toml`, `src/{lib,run,clock,git,gather,facts}.rs`, `examples/parity_driver.rs`, `tests/evidence.rs`, and `crates/cyril/src/crtool.rs`; modify root Cargo.toml/Cargo.lock, binary Cargo.toml/main.rs; add the parity oracle and compact canonical goldens; extend the already-committed shape oracle/CI phase to gather. Existing core prefix/projection files are inherited from merged A1, not reintroduced. Update existing architecture/user documentation after smoke; no unrelated skills or primary files.

**Estimate:** one implementation/verification cycle, potentially several hours of native qualification; not a completion gate.

**Diff estimate:**3,100 changed lines plus20% margin.

**PR increment:** S2HB-A2 native evidence.

**Commands and expected results:**
- `cargo build -p cyril -p cyril-review --examples` and `cargo build -p cyril` → build actual default Cyril and fixture driver; no missing diagnostics module/stub.
- `cargo test -p cyril-review --test evidence` → helper definition/caller/doc, stamp/no-recompute, target precedence and page-boundary expected values.
- `python3 .cyril-s2hb/oracles/parity.py --phase gather --cyril target/debug/cyril --driver target/debug/examples/parity_driver` → every output-file/stdout/stderr/exit byte agrees on all enumerated same-host cases; actual CLI timestamp falls within independent invocation bounds; hidden help/startup assertions execute.
- Same command with native `.exe` paths/Python on resourcefs-win11 → Windows CRLF/binary/Unicode parity and native PowerShell prefix execution; Linux shell smoke executes POSIX prefix. CI's existing macOS tooling lane runs its own same-host fixture.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase gather` → approved paths/dependencies/protected parents and module growth; named C10 mutation red/restored green.
- Focused commands above under each named mutation → localized expected failure; exact source restoration returns green.
- `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, plus `cargo test -p cyril-core --features kas` and `cargo check -p cyril --features kas` → formatting/workspace/current shell-projection gates pass; the KAS feature branch is actually compiled.
- Actual hidden CLI smoke is mandatory beyond unit tests. Then isolated production reconstruction + independent ledger comparison, PR review, current-head required CI, merge. Existing docs updated only after smoke proves the behavior.

## Slice B: execute cancellable native preflight diagnostics

**Claim IDs:** C5, C6, C7; C1/C3/C10 retained/extended and reverified for the new consumer. F0 retained mechanism proof applies because the same std file-handle strategy is selected.

**Expected behavior:** public diagnostics executes the explicit native command, records exact raw/filtered evidence and typed clean/nonzero/timeout/cancelled outcomes; cannot-start/parse/stamp failures write no reports; pre-cancel does not launch; live cancellation kills/reaps only its owned direct child. No diagnostics CLI verb exists.

**Oracle:** design C5/C6 Python diagnostics and literal child argv/environment/bytes; C7 OS process status and independently controlled handshakes/holder; deterministic deadline/first-observation cases, not fake real-time success.

**Stress fixture:** 131,073 bytes on each stream including invalid UTF-8; 201 changed-path matches and 16 tail lines; native Windows CRLF producing retained CRCRLF; hostile quoted argument shapes; failed exit and signal; zero/default timeout; cancel before launch/after ready/after exit; descendant still writing after direct child exits; spawn failure and cleanup errors. Expected partial snapshots contain only bytes before sampled lengths, and holder survives until fixture-owned release.

**Regression fence:** `crates/cyril-review/tests/diagnostics.rs` with a native fixture child mode in the verification driver; `.cyril-s2hb/oracles/parity.py --phase diagnostics`; permanent native owned-process cancellation qualification. No test kills processes by name or relies on a shared current directory/environment mutation.

**Named mutation:** C5 split Windows raw tail/remove nonempty CARGO_ value; C6 reverse stream order/use199 match cap; C7 omit direct-child kill/read past snapshot length. Deterministic terminal-decision tests also invert exit/cancel observation order. Native control proves holder/unrelated sentinel are live, so absence assertions cannot pass vacuously.

**Complexity/production scale:** command parsing O(C) characters; Windows executable candidates bounded by whitespace positions in C, with no successful-launch retry. Capture is O(B) disk/output bytes, two distinct files, zero reader threads, one direct child. Polling uses a fixed 10 ms sleep while live; terminal deadline decisions use monotonic samples, no busy-spin. Snapshot reads are capped by two sampled lengths even if holders keep appending. Filtering O(L×F) for L output lines/F changed paths (matching Python); reports retain at most200 matches/15 nonempty tail lines; no raw-output cap. Scale fixture 32 MiB dual-stream output and100 changed paths; accepted bound is two capture files, no reader thread, one spawn, at most sampled stdout+stderr+separator bytes and source report limits. It does not promise bounded disk cost for arbitrary input volume.

**Wall budget/phase:** one-off diagnostics operation; user/default timeout is the running deadline and direct-child termination/reap has the approved one-second deadline. Deterministic virtual observation tests enforce deadline/precedence decisions; real native process fixtures establish actual reaping/cleanup, with outer timeout solely a hang guard. N/A — no separate latency SLA for filesystem output, no always-on background task.

**Module shape:** create diagnostics/{mod,process,command}.rs; lib gains only exports/error variants; run owns unchanged stamp/encoding implementation. No protected-parent production changes are required. `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics` verifies final ledger/dependencies and growth; re-run affected C10 mutations if fence changes. Return to design before changing ownership or thresholds.

**Files:** create `crates/cyril-review/src/diagnostics/{mod,process,command}.rs` and `tests/diagnostics.rs`; extend leaf lib.rs and verification example; extend parity/shape oracle and native CI phase; update existing docs after actual diagnostics smoke. No crtool clap diagnostics variant.

**Estimate:** one substantive implementation/verification cycle, potentially several hours of native qualification; not a completion gate.

**Diff estimate:** 1,370 changed lines plus20% margin.

**PR increment:** S2HB-B native diagnostics, after A2 merges.

**Commands and expected results:**
- `cargo test -p cyril-review --test diagnostics` → exact typed statuses/bytes, no report on prelaunch errors, deadline/terminal precedence, owned lifecycle assertions.
- `cargo build -p cyril-review --examples` then `python3 .cyril-s2hb/oracles/parity.py --phase diagnostics --cyril target/debug/cyril --driver target/debug/examples/parity_driver` → byte equality for identical clock/command/manifest inputs and separate actual-system-clock library smoke.
- Native Windows equivalent → raw command-tail argv equality, Windows exit-code treatment, CRCRLF evidence, cancellation and holder-cleanup observations. CI macOS executes its own host fixture.
- Each mutation's focused case → named red output; restore → green. Record source hash/environment and observed terminal values.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics`, `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings` → final assembled gate passes; no source fence runs from production tests.
- Actual library consumer smoke, independent production reconstruction/ledger comparison, fresh PR review and current-head CI must pass before merge. Verify all three merge revisions and all acceptance; close cyril-s2hb without changing parent cyril-5gb3.

## Self-review and handoff

All design claims are assigned to their implementing slice; C10's production-tree coverage grows only as new owners actually arrive. Every slice has all fourteen fields, independent checks and same-slice fences/mutations. New loops retain explicit work bounds; no filesystem SLA is invented. The three-increment partition is based on observed source growth, not reduced acceptance or source compression. No slice or checkpoint is declared PASS here.

## Planning amendments, 2026-09-30

C8 Length review in design.md retains the cohesive prefix owner and raises its tripwire to190, with updated forecast140–160 (baseline0) and30-line uncertainty. No ownership/interface or protected-parent delta changes. EvidenceParity reports the full readable oracle/golden tooling exceeds600 lines; retain required coverage and recompute the review partition from actual assembled diff before checkpoint/commit rather than compressing fixtures. Any increment exceeding4,000 remains blocked until repartitioned into independently mergeable slices.

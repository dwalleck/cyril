# PR146 review decisions

Reviewed head: `1dae375f2b0e3131de8dee2689f2e3fd414e43c4`. Reviewer: fresh isolated PrefixPrReview1. Issue-wide PR review round1/5. Attempt2 produced the first repair; attempt3 refines its argv fence and corrects the observed executable-fixture race. No counter resets for A2/B.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Unix native smoke assumes `/bin/printf`; use an owned portable fixture. | PrefixPrReview1 | Verified | macOS job109788629393 in run36684976118 fails the native-prefix test's exit-status assertion; source uses `/bin/printf`; all other eight prefix tests passed. | Accept | R1: `review::tests::generated_posix_prefix_runs_as_a_native_shell_command` executes a per-test alias of `tests/fixtures/native-prefix.sh` through the generated prefix and checks exact argv/output. | blocking — preserve native execution rather than skip macOS or weaken assertions. |
| F2 | CI skips bridge/lib/HostShell-only changes; watch the integration surface without freezing unrelated parent edits. | PrefixPrReview1 | Verified | Committed CI selector lists only `.cyril-s2hb` and core review directory. Bridge/HostShell-only mutations cannot select the checker, and the checker has no separate current-wiring mode. | Accept | R2: current-wiring validation plus `--integration-only` in `oracles/check_shape.py`; CI selects full increment checks for owned changes, wiring-only checks for the three integration files. | blocking — close the actual trigger gap while preserving unrelated shared-parent work. |
| F3 | R1's interpreter operand does not establish ordinary executable argv; use an executable fixture that rejects extra arguments. | PortableFixtureReview | Verified | Native replay of both `"/bin/sh" crtool` and `"/bin/sh" -e crtool` exited0 with exact `crtool` output under the first R1 script. | Accept | R1 refinement: a private executable alias whose immutable script requires argc1 and argv1=`crtool`. | blocking — the first replacement fixture was portable but its exact-argv claim was too broad. |

## R1 — portable native execution fixture

Ownership: F1/F3, C8, plan slice A1; technical proof repair under the repository's cross-platform fixture obligation. Production behavior/interface/architecture are unchanged. The test symlinks immutable `tests/fixtures/native-prefix.sh` at a private spaced executable path, passes that path to the real prefix constructor, executes through `/bin/sh`, and requires exit0, exact `crtool` stdout and empty stderr. The fixture requires argc1 and argv1=`crtool`; shell options can no longer disappear before its script operand. No external printf location, global cwd/environment mutation, feature skip or dependency is added.

Observed before: macOS CI907/2087 tests run,906 passed,1 failed,13 skipped;1180 canceled. The failure log did not include child stderr, so the exact shell diagnostic is not claimed. The source's external binary-layout assumption is removed regardless; `/bin/sh` successfully launched in the failing run. Static next-wave audit: the only real spawn in the new module remains cfg(unix); all eight other new tests already passed on macOS.

Historical attempt2 receipt (superseded fixture): module SHA256 `d6c561698468ce0de148b2f12ab1ad6c95a25a16581211b2149b203b08250570`; omitting the suffix failed exact stdout, restoring passed1. F3 invalidated that fixture's broader exact-argv claim.

Current R1 receipt: module SHA256 `005a182472409d082aec14b04c05b1534721c955d71b98daa2339f9f788721b4`; immutable fixture SHA256 `e9c1e34dcbc24d2d90bd296cf3dfd9edde95ec0ef6ecfcc5ecc817449c0e893c`, mode0755. `cargo test -p cyril-core --features kas review::tests::generated_posix_prefix_runs_as_a_native_shell_command -- --exact` passed1. Replace only the suffix with `" -e crtool`: the same compiled fence failed, child exit64 (raw Unix status16384), Cargo exit101. Restore the exact suffix: passed1/exit0; module restored byte-for-byte. Production-region SHA256 remains `a74477f1ecbdadf2fc093f3b7f0d12f22af29cc176eeb1b2e31ef4dfcafc0b53`, equal to1dae375.

Historical attempt2 assembled check: formatting passed; workspace tests stopped at the existing home-shorthand wrapper case with a missing journal (core1078passed/1failed/5ignored). Chained Clippy did not run; a separate Clippy invocation passed. Q1 below records the exposed process error and correction. Baseline CI run36684976118 completed with Windows/Linux test legs and other independent jobs passing; macOS Test and aggregate CI Success failed. Required new-head CI remains outstanding.

Independent first repair review: PortableFixtureReview found F3; parent reproduced its false pass and replaced the interpreter-operand fixture. Fresh ImmutableFixtureReview then inspected the refined R1/Q1 diff, both fixture files and this record read-only. Verdict: correct, zero findings; alias identity/journal placement, exact argv, immutable executable bytes, Unix gating and isolation were preserved. It performed no execution; the parent owns the receipts above.

## Q1 — executable-fixture qualification failure

Ownership: S2HB-A1 assembled quality; N/A — technical correction governed by the repository's isolated, full-suite-safe process-fixture requirement. No product interface/behavior changes. The existing version tests write/chmod/execute temporary scripts. Temporary result logging in the failing test exposed `Text file busy (os error 26)` before child startup on iteration19 of a bounded full KAS-core replay; the missing journal was secondary. The first18 instrumented runs passed, so a single retry was not treated as a fix. The earlier no-feature core run excluded KAS and is not proof for this path.

Applied correction: `fake_wsl_launcher` now symlinks immutable `tests/fixtures/wsl-launcher.sh` as each test's private `wsl`; the script derives its journal from that alias. The helper creates no writable executable during parallel execution. The busy executable is observed; the specific fork retaining its writer was not traced ([INFERENCE], consistent with the repository's documented hazard). Removing runtime writer handles eliminates that precondition without retries, sleeps or suite serialization. Build errors are inspected before journal assertions; temporary diagnostic logging is removed. All prior wrapper argv/version assertions remain.

Affected paths: `crates/cyril-core/src/protocol/kas/version.rs` test-only helper/callers; `crates/cyril-core/tests/fixtures/wsl-launcher.sh`. The three helper callers are `wrapper_probe_asks_kiro_cli_through_the_wsl_launcher`, `wrapper_probe_passes_the_launcher_home_shorthand_through`, and `wrapper_version_mismatch_names_the_probed_command`. The source/disposition reference is existing `tests/spawn_isolation.rs`'s documented inherited-writer hazard; its local lock cannot protect this different test binary against every unrelated core child spawn. Static fixture plus symlink avoids that coupling.

Current Q1 source SHA256: version.rs `fc90d802277afa92d8677453f574b8a68d37e4669e8686d9bbe3c98213c42217`; fixture `d1fb062bc3a25418c9836b4bf4b5596d8f26d30215caf37aae61f2d4ea6edfa5`, mode0755. `cargo test -p cyril-core --features kas protocol::kas::version::tests::wrapper_` passed4, including all three process cases. Removing only the production `if arg == "~" { continue; }` branch made the exact home-shorthand fence fail with `command ... is ~, not kiro-cli`, exit101. Restoring passed1/exit0. Subsequent rustfmt changed only the fixture-path macro layout; production bytes equal1dae375.

Assembled repair verification on the pinned R1/Q1 source: `cargo fmt --check && cargo test && cargo clippy -- -D warnings` passed; workspace aggregate2,096 passed/13 ignored. Then40 consecutive `cargo test -p cyril-core --features kas --lib` invocations all exited0. The loop stopped on any failure; it did not retry failures into a green result. All effective Cargo commands unset the inherited empty CARGO_TARGET_DIR, as in checkpoint-A1.md. The initial formatting gate requested multiline layout for the fixture-path macro; `cargo fmt` applied that layout before the successful full chain.

Native smoke outside the Rust test harness executed both actual fixture files through private aliases in a spaced/apostrophe/Unicode directory. Prefix fixture: `crtool` → exit0/exact `crtool`; no args or inserted `-e` → exit64; wrong sole arg → exit65; all stderr empty. WSL fixture: own `--version` → WSL2.6.1; `~ -d Ubuntu kiro-cli --version` → kiro-cli2.21.1; wrong routed executable → exit3/exact diagnostic. Each journal exactly matched the supplied argv. Temporary directories were automatically removed.

### Native-fixture checkpoint (R1/Q1)

These repairs share the native-fixture qualification commit: R1 alone leaves the observed Linux Q1 failure; Q1 alone leaves the observed macOS F1 failure. No independently verified green intermediate is claimed. The owning compact records remain separate above.

Reuse/symmetry: targeted helper searches covered the two Rust modules, existing spawn_isolation fixture discipline and the proof-tool directory. R1 reuses `test_support::must_succeed`, tempfile and the standard Unix symlink API; Q1 retains `fake_wsl_launcher`'s signature and all three callers. No new Rust helper/API exists. The immutable scripts retain separate behavioral responsibilities; sharing their unrelated argv protocols would couple oracles. Error behavior is stricter reporting, not fallback; no new production logging, resource lifecycle, guard or environment semantics. Unix gating remains symmetric, and Windows-compiled production is unchanged.

| Checkpointed-build obligation | Judgment / evidence |
|---|---|
| 1. Affected tests | PASS — exact native-prefix case, four wrapper-filter cases, full2,096-test workspace and forty full KAS-core runs. |
| 2. Falsifiers | PASS — exact argv and retained wrapper/version behavior; C8 native production conclusions retained from checkpoint-A1 because production bytes are unchanged. |
| 3. Stress fixture | PASS — native spaced/apostrophe/Unicode aliases and argv boundary smoke; retained C8 long-path/hazard/dialect matrix. |
| 4. Independent oracle | PASS — literal expected argv, journal, version, output and exit statuses agree with actual native execution. |
| 5. Module shape | PASS —145/190 prefix census; no production ownership/interface change; frozen production reconstruction/conformance remains applicable. Q1 only relocates fixture data, not production logic. |
| 6. Budget | PASS — retained C8 O(path bytes) production proof. New production loop/latency budget: N/A — these repairs change test fixtures only; no SLA or always-on phase is introduced. |
| 7. Regression fences | PASS — native argv and wrapper/version assertions remain green; no test is skipped or weakened. |
| 8. Named mutations | PASS — inserted prefix option and removed production tilde skip both compiled and failed their exact fences. |
| 9. Restored green | PASS — both focused fences passed after restoration; final production bytes equal1dae375; formatting-only macro layout does not change applicability. |
| 10. Parity/reuse | PASS — helper search, three-caller census and all error/logging/fallback/observability/resource/guard/acceptance dispositions above. |
| 11. Preserved enforcement | PASS — prefix, wrapper argv and required-version assertions retained; stronger exact-argv fixture and unchanged tilde-skip defect detection demonstrated. No policy is relaxed. |

`cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` also passed on the pinned repaired source. Hosted current-head CI and final PR review remain publication gates; this local checkpoint does not claim either.

Independent staging proof: the original checker from1dae375 was also executed against the native-repair tree using its original CLI and the discovered base; it passed145/190 and all protected-parent checks. The native-fixture commit therefore does not depend on the separate R2 checker changes for its green source gate. Upstream fetch found no movement from76bfb1e. Sweep found no temporary DEBUG marker, TODO, obsolete `/bin/printf` executable use or generated handoff left in source; remaining `/bin/printf` prose is explicitly historical evidence.

## R2 — integration-trigger and current-wiring guard

Ownership: F2, C10, plan slice A1. Keep the full increment census as the default. Add `--integration-only` for the already-established integration surface: require the core export, typed private bridge field, exact getter, one approved resolved-shell assignment and exhaustive HostShell projection. This mode must not compare unrelated parent bodies or discover unrelated new modules. It reports only the wiring conclusion, never a full-ledger PASS. Full mode also runs these presence/value checks, closing deletion/vacuity holes.

CI watches the three integration files in addition to existing owned paths. Owned-path changes select full mode; integration-only changes select wiring-only mode; unrelated changes skip. All pre-existing CI gates remain. This technical correction preserves the approved source ownership and consumer behavior; it changes which proof is run, not product behavior or accepted risk.

Executed proof: `python3 .cyril-s2hb/oracles/mutation_f2.py`, old checker/CI from1dae375 versus checker SHA256 `21b896cef900722525976ee1dd358347ce1ab8483ad8640e30041abbe6a143c5`, CI `ba7c2b707b6b0a11cf3d27f96de10a311786a5f056cd95c4320a4df819bb9e8b`, harness `e2b8c4ac93022a0a2c4a5f1daca980205e387a0617d9545f30ff51d9a44921c8`. Fresh Git fixtures use the same reviewed production source and exactly one mutation; actual old/new CI shell bodies are extracted, not retyped. Printed results:

| Single mutation | Old CI | Repaired CI | Old/repaired full checker |
|---|---|---|---|
| Bridge getter returns None | SKIP0 | FAIL1: private-field getter | FAIL1 / FAIL1 |
| Resolved assignment becomes None | SKIP0 | FAIL1: approved KAS/non-KAS assignment missing | FAIL1 / FAIL1 |
| Posix/Fish projection swapped | SKIP0 | FAIL1: exhaustive mapping | FAIL1 / FAIL1 |
| Unrelated command-capacity constant changes | SKIP0 | PASS0: integration wiring only | FAIL1 / FAIL1: forbidden parent delta |
| Original `review_gather_body` added to main | SKIP0 | SKIP0 (unchanged scope) | FAIL1 / FAIL1: startup and parent-body violations |

The real unchanged production tree passed both `check_shape.py --phase prefix --base 76bfb1efb45e96f5efe41f0a8be434768da0ddc3` (145/190 census) and `--integration-only`. No full-ledger conclusion is attributed to wiring-only mode. Main-only CI selection is intentionally unchanged; the original named mutation still fails the full increment fence.

Independent repair review: fresh GateRepairReview inspected the applied checker/CI/harness diff and record read-only, with no execution. Verdict: correct, zero findings; full detection retained, correct mode precedence, isolated frozen-source replay. Native/runtime behavior is not claimed by this source-gate review.

### Source-gate checkpoint (R2)

Reuse receipt: `LIB_PATH`, `BRIDGE_PATH`, `HOST_SHELL_PATH`, `BRIDGE_FIELD` and the two approved-body constants are shared by current checks and existing stripping; `balanced_body` extracts the old balanced-brace mechanism, reused by `remove_function`/`extract_block`. `current_wiring_errors` reuses `production`/`normalized`. The targeted helper search found the old checker Git runner is bound to its ROOT; the independent A/B script's `run_process`/`git`/`git_show` instead accept per-case roots and must not import the candidate checker into its own oracle. `Case`/CASES, frozen archive/setup, real-step extraction, execution and reporting functions are qualification-only; their repeated owner paths are independent oracle inputs, not a second production convention. All imports remain Python stdlib.

Symmetry: both checker modes use the same current-wiring predicate, localized FAIL format and fail-closed errors. Wiring-only deliberately omits whole-parent delta, new-owner and growth comparisons under F2's established obligation to allow unrelated shared-parent work; its PASS explicitly names that narrower result. Full mode preserves those checks. The replay covers accepted unrelated input and refused wiring defects through both modes, plus the old named ownership violation.

| Checkpointed-build obligation | Judgment / evidence |
|---|---|
| 1. Affected execution | PASS — both checker modes ran successfully on the actual repaired tree; no Rust behavior changes in R2. |
| 2. Falsifiers | PASS — five-case same-source A/B receipt above; CI now selects the guard for each changed integration owner. |
| 3. Stress fixture | PASS — distinct missing-value/mapping defects, unrelated parent control and original forbidden-owner mutation, each in a fresh Git tree. |
| 4. Independent oracle | PASS — approved wiring/ownership expectations agree with localized real checker/CI verdicts; no runtime proof is inferred. |
| 5. Module shape | PASS — full145/190 census, existing protected-parent deltas and retained frozen production conformance. |
| 6. Production budget | N/A — proof tooling only; R2 introduces no production loop, latency SLA or always-on phase. |
| 7. Regression fence | PASS — both modes green on actual source, correct CI selection demonstrated. |
| 8. Named mutations | PASS — all three previously skipped wiring defects now fail CI; both full checkers still reject the original main-owner mutation. |
| 9. Restored green | PASS — fresh isolated cases never altered the real production tree; both modes were rerun afterward and passed. |
| 10. Parity/reuse | PASS — helper/constant census and seven symmetry dispositions above; predicate shared, verdict scope explicit. |
| 11. Preserved enforcement | PASS — full source gate's original mutation remains red; unrelated-parent acceptance belongs only to the new mode. Existing CI jobs/dependencies are unchanged. |

GateRepairReview's independent zero-findings pass covers this repair. Plan.md records the affected-only2,800+20% A1 forecast; no production placement or partition decision changed.

## Evidence disposition and remaining gates

Initial checkpoint-A1.md remains historical evidence. Native Windows and production prefix/bridge smoke conclusions remain applicable because both Rust production regions are unchanged and the fixture changes are Unix-gated. The current local repair checkpoints and both independent repair reviews pass. Required current-head hosted CI and a fresh full PR review remain publication gates; no merge or cyril-s2hb completion is claimed here. The measured assembled delta before final review-receipt prose was2,596 lines, below the revised3,360 allowance and4,000 partition limit; final committed size is checked at publication. No tracker delta is included.

## Review errors

No additional review error is claimed. The diagnostic log demonstrates the failed test, not the exact missing-program stderr; the root-cause explanation above distinguishes those facts.

## Round2 — event and live-source qualification

Reviewed head `b4e79c17bacb98b850235c28195957bdcfeddac3`; fresh isolated PrefixPrReview2; issue-wide review round2/5 and implementation attempt4/5. CI run36693820938 passed all16 jobs. macOS job109816947226 explicitly ran the native-prefix case and all three wrapper process cases; aggregate2087passed/13skipped. The product and native fixture repairs remain valid; this round changes proof tooling only.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F4 | Push CI compares against the already-pushed default-branch HEAD; select the event's pre-push revision. | PrefixPrReview2 | Verified | Disposable b4 source fixture with getter=None, origin/fixture moved to pushed HEAD and GITHUB_EVENT_NAME=push printed SKIP/exit0. | Accept | R3: CI event-aware comparison and event-specific replay cases. | blocking — PR merge-base selection and push before/after comparison have different semantics. |
| F5 | Raw source matches accept commented/disabled or misowned wiring; inspect live owned items. | PrefixPrReview2 | Verified | Disposable b4 source fixture wrapping the exact getter in a block comment printed integration-only PASS/exit0. | Accept | R4: lexical/scoped current-wiring validation in check_shape.py and its private source helper. | blocking — a source receipt cannot establish a callable seam from comment text. |
| F6 | Replay returns success when verdicts disagree with required outcomes; enforce the outcome matrix. | PrefixPrReview2 | Verified | Executed b4 replay main with only the candidate CI body changed to unconditional SKIP; all five candidate CI rows skipped and main returned normally (CLI exit0). | Modify | R5: retain complete A/B diagnostics, then fail on any unexpected verdict or unknown/error result. | blocking — promote the diagnostic survey into an automated qualification command without fail-fast loss of evidence. |

### Bounded repair scope — R3/R4/R5

Ownership: C10 and plan A1; unchanged product behavior, architecture, source ownership and risk. R3 owns CI event/base dispatch; R4 owns live-item validation; R5 owns replay outcome enforcement. Their shared replay proves the combined guard, so the repairs form one qualified commit with all three finding IDs. Unchanged approved A1 fields are inherited.

Expected proof: PR and push scenarios reject guarded defects; unrelated shared-parent changes retain narrow acceptance; comment/conditional/wrong-owner text cannot substitute for required items; original five R2 cases and forbidden-main full-mode refusal remain covered. An unconditional-SKIP candidate must make the qualification command nonzero while preserving every printed comparison. The earlier print-only A/B guidance governed a diagnostic survey; this approved technical correction explicitly makes the command an automated verdict gate under assessing-review-feedback step5.

Affected evidence: R2 source-gate/replay conclusions are replaced below. Production/native C8, R1/Q1 and production-only conformance remain applicable: no Rust or native fixture bytes changed. These source guards are proof tooling, not a Rust compiler or a new runtime dependency.

### R3 — event-correct CI selection

F4/C10/A1. `.github/workflows/ci.yml` uses PR merge-base diffs, but push before→HEAD diffs; verifies the comparison commit before selection. `mutation_f2.py` preserves a separate before ref and advances the simulated default-branch ref on pushes. Real extracted CI bodies now reject the pushed getter defect, accept the unrelated constant only in wiring mode, and select full mode for owned-path changes. Missing-base rejection is fail-closed Git resolution, not a fallback to HEAD.

### R4 — live owned-item qualification

F5/C10/A1. `check_shape.py::current_wiring_errors` reads complete files through private `RustSource` queries, requiring unconditional owned exports/fields/methods/statements. Literal tokens and nested delimiter scopes cannot impersonate those items. `rust_source.py` hides lexical scanning and direct-scope discovery; policy remains in the existing checker. The source helper is not a compiler, macro expander or cfg evaluator. Its cohesion/size review is in design.md.

Parent handoff inspection corrected multiline normal-string scanning, field visibility boundaries and incomplete constructor cleanup; unused mask/span APIs were removed rather than retained speculatively. The new multiline-literal and public-field cases cover those concerns. The commented-getter mutant and six additional getter/assignment/lexical-control variants compiled with `cargo check -p cyril-core --features kas`; compilation is not claimed as runtime or Clippy proof. The two truncated raw-transfer probes were invalid tool-input runs, not policy evidence; only complete-source runs below count.

### R5 — qualification outcomes, not successful execution alone

F6/C10/A1. `mutation_f2.py::run_case/main` prints all comparisons and accumulates mismatches before returning nonzero. `verdict` distinguishes wiring/full passes and requires an actual C10 failure diagnostic; unclassified execution results become UNKNOWN, not an expected rejection. The original five cases are retained; the reviewed baseline advances to b4 for this round. Historical R2 evidence remains pinned to its original script/hash.

Discriminating F6 replay held source, checker and candidate CI at b4 and used the same original five cases on both sides. Only candidate CI's shell body was replaced by unconditional SKIP. b4's replay exited0; repaired replay exited1 with four unexpected matrices and all five cases printed. Restoring candidate CI gave exit0/five correct matrices. Temporary files were removed; no real CI/Rust source was mutated. This establishes the command's changed exit semantics independently of R4.

### Attempt4 source pins and historical checkpoint

SHA256: checker `0df8035ee9cf6f4e4c7deb748ea933645e36fe45c1241d929d01e5588990f4d4`; helper `83a59858ad5a23fd3900d191e991b39e94f2d03427fc316084cb3f4187ec90b4`; replay `594b6d98b6bd3c61d724ddc462c1eb42601a731cf5bbb6eb43aeeaf7337cde39`; CI `60963fb8a00e5825ec3c67946e080fbf403fbeb9d67adf982b397d125042e450`.

Commands: `python3 .cyril-s2hb/oracles/check_shape.py --integration-only`; full mode with `--phase prefix --base 76bfb1efb45e96f5efe41f0a8be434768da0ddc3`; `python3 .cyril-s2hb/oracles/mutation_f2.py`; then `env -u CARGO_TARGET_DIR cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, and KAS all-target Clippy (the same env correction applies to every Cargo command). All passed; tests2096passed/13ignored. Full census145/190. The19-case replay produced zero unexpected matrices:

| Inputs | Old b4 CI → repaired CI | Full checker result |
|---|---|---|
| Three original value/mapping defects; public field | FAIL → FAIL | Both FAIL |
| Eight non-live/misowned getter or assignment variants | PASS_WIRING → FAIL | Both FAIL |
| Unrelated nested comments/raw-string/char/lifetime control | FAIL → PASS_WIRING | Both FAIL: strict parent delta |
| Unrelated bridge constant | PASS_WIRING → PASS_WIRING | Both FAIL: strict parent delta |
| Original forbidden main owner | SKIP → SKIP | Both FAIL: original ownership fence retained |
| Pushed getter defect / unrelated constant / owned proof comment | SKIP → FAIL / PASS_WIRING / PASS_FULL | Both FAIL / FAIL / PASS_FULL |
| PR owned proof comment | PASS_FULL → PASS_FULL | Both PASS_FULL |

Reuse/callers: pyright references identify only local `main→run_case→fixture_tree/run_ci` callers; grep finds only the qualification command's record reference outside its script. Existing process/Git/step-extraction/report helpers and original cases are reused. No existing repository Rust lexical/scoped helper was found. The lexer shares scanners across tokenization; dead mask/span interfaces were removed. Approved token tuples deliberately remain independent policy inputs; old regex stripping stays solely for strict parent deltas. No new product fallback, logging, resource ownership or dependency exists. Both modes share live validation/error reporting; only full mode retains whole-parent/growth checks.

| Checkpoint obligation | Judgment |
|---|---|
| 1. Affected execution | PASS — both real checker modes,19-case CLI replay, fmt/tests/both Clippy selections. |
| 2. Falsifiers | PASS — push skip and non-live-source false passes flip; strict outcome enforcement flips exit0→1 on identical wrong input. |
| 3. Stress/control inputs | PASS — conditional ownership, two string forms, nested comments, chars/lifetimes, wrong owner, private-field policy and both event types. |
| 4. Independent oracle | PASS — explicit approved outcome matrix agrees with real Git/CI/checker execution; no Rust runtime inference. |
| 5. Module shape | PASS — full145/190 census; proof-helper cohesion review; production-only conformance retained. |
| 6. Production budget | N/A — proof tooling only; retained C8 runtime complexity evidence is unchanged. |
| 7. Regression fences | PASS — current modes and strict replay are green. |
| 8. Named mutations | PASS — original main-owner refusal retained; F4/F5 same-source A/B and F6 wrong-candidate exit proof above. |
| 9. Restored green | PASS — isolated fixtures removed; actual full/narrow modes and19-case replay rerun successfully afterward. |
| 10. Parity/reuse | PASS — caller/helper census and symmetry dispositions above; no duplicated production convention. |
| 11. Preserved enforcement | PASS — original five cases retained; public-field refusal retained; full parent/directory/dependency limits unchanged. |

Independent Round2GuardRepairReview returned two findings below. Its inspection and the parent counterexamples invalidate the broad conditional-ownership and exact-push-base conclusions of this19-case checkpoint. These results remain historical, not current commit authorization.

### Final authorized attempt5 — inner scopes and exact push endpoints

Issue-wide implementation attempt5/5; PR review round remains2/5. No counter resets for the later A2/B increments. The fifth cycle repairs the remaining qualified A1 defects; remaining issue scope must be escalated at the cap.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F7 | Inner cfg attributes can remove required owners while lookup calls them live; propagate scope conditions. | Round2GuardRepairReview | Verified | Getter-only impl with inner `#![cfg(any())]` produced current PASS_WIRING/exit0 and compiled with KAS cargo check. | Accept | R6: recognize leading inner cfg/cfg_attr on file/item scopes and propagate into required ownership decisions; replay conditional and nonconditional controls. | blocking — attempt4 covered outer attributes only. |
| F8 | Full qualification replaces the push endpoint with a merge base; preserve exact event.before. | Round2GuardRepairReview | Verified | Disposable divergent history had identical bridge constant33 at both endpoints and only an owned proof-comment delta; CI still rejected ancestor-only bridge drift. Captured stderr supplied the diagnostic. | Accept | R7: mutually exclusive exact-base CLI mode, selected for pushes; divergent-history replay. | blocking — no real remote force-push is performed or authorized. |

R6 inherits C10/A1's explicit unconditional-owner obligation and changes only rust_source.py plus replay inputs. R7 inherits C10/A1's event-correct comparison obligation and changes checker CLI/base resolution, workflow argument selection and replay history. Root causes remain separate; any eventual combined repair commit must include F4–F8. Attempt5 observed inner conditional owners refused, nonconditional inner metadata accepted, and a divergent push with unchanged endpoint parent work passing full qualification. This does not authorize publication: Q2 below remains unresolved.

Attempt5 receipts: both full comparison modes and integration-only checker passed (full census145/190); the actual CLI replay reported24 cases and0 unexpected matrices. `env -u CARGO_TARGET_DIR cargo fmt --check`, `cargo test` (2096 passed,13 ignored,35 suites), `cargo clippy -- -D warnings`, and `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` all succeeded. Production/native fixture bytes remain unchanged; these checks do not replace the failed boundary proof.

Attempt5 SHA256 pins: check_shape.py `9ad0b2ee273c8b015511202536271816a55e74b6b6afca4f70bc14fb04334b9d`; rust_source.py `d7dc57d37042dce011e8f8044c70eb7601b2d28f6c8eb1bd57776edee629eda1`; mutation_f2.py `64ac5315f398de348a32ff23b1d075dd728bd2d8618ebf38427f936ff5d870ff`; ci.yml `763a278b3b1ad65b594c925a45ceda02518291eec22a24e7634005c1c5433f05`.

### Q2 — raw-identifier qualification failure; cap reached

The parent injected `#[r#cfg(any())]` immediately before the real `BridgeHandle::review_shell` getter in a disposable same-source fixture. Current actual CI integration-only qualification returned PASS_WIRING/exit0; `cargo check -p cyril-core --features kas` also exited0. The lexer splits `r#cfg` into separate tokens, so conditional-attribute recognition misses Rust's raw identifier.

A separate consumer compile makes the observable loss explicit: appending a temporary function that returns `handle.review_shell()` compiled with the live getter (exit0), but the same consumer against the raw-cfg mutant failed with E0599, “no method named `review_shell` found for reference `&BridgeHandle`” (exit101). Disposable fixtures were removed. Q2 has no applied fix and is not covered by the24-case replay.

Current checkpoint: FAIL / NO SHIPPING. The unconditional-live-owner claim and named-mutation qualification are not discharged. Before this cap record, the working cumulative delta was3426 changed lines across28 files, exceeding A1's3360-line forecast allowance; scope reconciliation remains pending, not waived. The helper is now489 lines, checker400, and replay498.

Implementation attempts5/5 and full PR review rounds2/5. No sixth implementation attempt is authorized. PR146 remains at `b4e79c17bacb98b850235c28195957bdcfeddac3`, whose CI run36693820938 passed all16 jobs; it was converted to draft because that published head and the uncommitted repairs are not merge-qualified. No repair commit/push, merge, or issue closure follows these receipts. F4–F8 working changes are preserved; final independent repair/full-PR review and new-head CI remain pending. A2 is an unverified draft; B is unimplemented. Remaining work requires explicit user authorization to extend the issue-wide implementation cap.

### Attempt6 — authorized Q2 correction

User selected “Extend this issue to 10 attempts”: s2hb implementation cap10, five already used; review cap5 and every other issue cap remain unchanged. Attempt6 now starts. Q2 inherits C10/A1's unconditional-live-owner requirement. Normalize raw identifiers in the existing tokenizer after literal recognition, retaining original offsets; reuse its identifier scan rather than special-casing cfg. LSP identifies tokenize's callers as token_texts and RustSource.__init__; no caller signature changes. Add actual-CI raw cfg/cfg_attr refusals and a live raw-identifier getter control to the existing replay. Expected: the previous lexer fails the new replay; repaired lexer passes it, including existing raw-string controls. A1 forecast amendment belongs to plan.md; production/native receipts remain applicable because only proof tooling changes. Checkpoint and independent repair review: PENDING.

Attempt6 Q2 results: the same27-case replay failed before normalization with3 unexpected matrices (exit1), then passed all27 (exit0). Both full modes and integration-only mode passed. Actual Rust consumers confirmed raw cfg/cfg_attr remove the getter (E0599/exit101), while the live raw-identifier getter compiles (exit0). Q2 is corrected, but PrefixGuardRepairReview6 returned the three verified findings below; attempt6 is not commit-qualified.

### Attempt7 — owner identity, source preamble and operational failures

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F9 | A generic specialization is accepted as the returned handle's owner; require approved self-type headers. | PrefixGuardRepairReview6 | Verified | BridgeHandle<const N: usize = 0> with getter only on BridgeHandle<1> passed actual CI and core compilation; default-handle consumer failed E0599. | Accept | R8: check_shape.py checks the exact BridgeHandle declaration; RustSource.top_level_impls accepts only the approved bare self type; replay keeps both changed declaration and impl in one fixture. | blocking — preserve b4's nongeneric owner policy. |
| F10 | A leading Rust shebang hides root cfg; recognize the source preamble. | PrefixGuardRepairReview6 | Verified | Shebang then root cfg(any()) passed actual CI/core compilation; actual public review-module consumer failed E0433. | Accept | R9: skip optional BOM and genuine shebang using shared lexical trivia handling in rust_source.py; replay conditional and live controls. | blocking — not a cfg evaluator or general parser. |
| F11 | Operational census failure counts as policy refusal; classify it UNKNOWN. | PrefixGuardRepairReview6 | Verified | Candidate helper RuntimeError on getter-none produced two operational diagnostics yet run_case returned true against its all-FAIL matrix. | Accept | R10: recognize the operational diagnostic before the policy branch in mutation_f2.py, with actual-CLI crash control and same-case fault injection. | blocking — no error-status waiver. |

These separate root repairs inherit C10/A1 and the replay's existing R5 error/refusal obligation. LSP callers remain local: verdict→report/run_case, fixture_tree→run_case; tokenizer callers are unchanged. Reuse identifier, comment and literal handling, item_header policy comparisons, and existing fixture/report helpers. Expected: wrong self-type/root-cfg cases refuse; live preambles pass; operational failures never satisfy a policy-refusal expectation. All earlier replay cases remain. Changed proof paths only; production/native/conformance and attempt5 Rust quality evidence remain applicable. Combined checkpoint and independent repair pass: PENDING; implementation count7/10, PR review count2/5.

Attempt7 execution: integration-only and both full modes passed (145/190); the same33-case replay changed from4 unexpected matrices/exit1 before R8–R10 to0/exit0 afterward. The same getter-none fixture with a candidate RuntimeError now produces UNKNOWN/UNKNOWN and run_case=false; restoring the helper produces FAIL/FAIL and run_case=true. The intentional operational-error control checks UNKNOWN explicitly; it is not a policy-refusal case. Rust consumers independently confirmed BOM/root cfg and spaced/commented inner cfg remove the export (E0433/exit101), while BOM+shebang+nonconditional metadata retains it (exit0).

Source pins (SHA256): checker `bf3ca4252feb57f4d2a9cf587638de771a9979aeeb8ffda62afaa8ca7508bcbd`; helper `e99a051f366cf2082a26723e88e1b1d51a6940d48817ec72c0aad94da68c2dc3`; replay `3f5569398e9a2db1740b977bd1499760e0ea463e5f5ecfb39a882e03cf95183f`; CI retains attempt5's pin. Measured delta before this receipt:3517 lines/28 files, within the amended3960 allowance and4000 partition threshold. Helper488/checker403/replay557 physical lines. Generic impl support was removed because neither actual consumer permits it, not to hide growth; ordinary token formatting is unchanged.

Reuse/symmetry: pyright missed cross-file impl-query callers; grep confirmed only the BridgeHandle and HostShell lookups in check_shape.py. Both now use the same exact nongeneric lookup. _skip_trivia reuses nested-comment scanning and serves preamble and normal tokenization; only byte-order mark/shebang handling is file-initial. Existing SourceError refusal and RuntimeError operational reporting remain distinct; no product logging, fallback, allocation or resource path changes. Additional fixture edits reuse the exact-one-needle check and remain one changed file/one conceptual mutation. Existing policy cases remain unchanged; the new error control cannot make an ordinary policy case accept UNKNOWN.

Independent attempt7 repair review: GuardReplayRepair7 judged its replay/CI scope correct with no findings. GuardOwnerRepair7 found the three claims below; attempt7 remains unqualified until the verified owner gaps are corrected.

### Attempt8 — returned value and default module owner

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F12 | The exact store can target a decoy instead of the returned bridge. | GuardOwnerRepair7 | Verified | Actual CI passed the decoy fixture; the then-retained probe_resolved_prefix consumer panicked with None instead of Some(Posix) for bash/KAS. | Modify | R13 supersedes failed R11: examples/review_prefix.rs executes the actual returned dialect/prefix; CI and mutation_f2.py exercise this consumer. | blocking — carry is proved by C8 runtime, not assignment text; the attempt8 token predicate is removed. |
| F13 | A path attribute redirects the review export outside its approved source owner. | GuardOwnerRepair7 | Verified | Actual CI passed a path redirect to an alternate copy of review/mod.rs; KAS core compilation succeeded with that different owner. | Accept | R12: reuse contiguous-attribute lookup to refuse path overrides on the required export; replay valid redirected and documentation-only owners. | blocking — default module resolution is part of C10 ownership. |
| F14 | A harmless attribute on the assignment is wrongly rejected. | GuardOwnerRepair7 | Refuted | The exact proposed #[allow(unused_assignments)] form fails pinned Rust1.94 with E0658 “attributes on expressions are experimental”; the nonsuppressing #[warn] variant fails identically. | Reject | N/A — proposed input is not valid on the supported toolchain. | non-blocking — no checker relaxation for an uncompilable expression; allow also conflicts with repository rules. |

R11/R12 inherit C10/A1 and do not introduce general Rust data-flow analysis or macro expansion. The bounded rule pins the protected handle's construction, sole binding and return; unrelated code without a new use of that protected value remains permitted. Path attributes on this export require ownership requalification rather than an alternate-source fallback. Attribute matching will be shared with existing conditional inspection; fixtures reuse the same frozen old/current trees. Initial decoy smoke argument sh was unsupported and supplied no carry evidence; the corrected recorded bash choice produced the actual failure above. Implementation8/10, PR review2/5; affected execution and independent repair review PENDING.

The R11 boundary probe also found an early Ok(create_channel_pair().0) return could bypass the final projected handle. It passed the first flow predicate and failed the actual bash/KAS consumer with None vs Some(Posix). The same atomic repair now refuses explicit return statements as well as alternate handle uses; additional return control flow requires requalification. This is a bounded source-shape rule, not a promise to analyze arbitrary control flow or expand macros. Its focused replay and final assembled run remain required before the attempt8 checkpoint.

Attempt8 executed39 source cases with0 unexpected matrices, both full modes and integration-only; fmt,2096 tests/13 ignored and both Clippy selections also passed. Actual-checkout bash/fish consumers returned Some(Posix)/Some(Fish) for KAS and None for V2, with all bridges completing; the temporary example was removed. A restored archived probe initially reused the compiled decoy due old archive mtimes/shared target; byte identity plus metadata-only crate-root refresh forced recompilation and restored the expected values. That stale run is not product evidence.

### Attempt9 — replace token data-flow claims with native consumer qualification

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F15 | A shadowed Ok call can replace the correctly projected handle. | PrefixOwnerRepair8 | Verified | A snake-case helper imported locally as Ok (no lint suppression), with standard earlier Ok patterns qualified, passed source CI and compiled; actual bash/KAS consumer returned None. | Modify | R13: compiled native prefix execution in the CI platform matrix, replacing the token-only flow rule. | blocking — source spelling is not Rust name resolution. |
| F16 | Counting every handle token rejects unrelated macro field names. | PrefixOwnerRepair8 | Verified | tracing::debug!(handle = \"bridge constructed\") was refused by source CI but compiled unchanged KAS behavior. | Modify | R13: remove the token-count semantic predicate; retain structural checks and execute the actual consumer. | blocking — no special-case logging-macro interpreter. |
| F17 | Normalized raw identifiers and nested returns are mistaken for function return control. | PrefixOwnerRepair8 | Verified | A local r#return used in logging compiled but failed the source gate; it does not reference the protected handle or return from the function. | Modify | R13: remove the token-return semantic predicate; Rust compilation/runtime supplies scope semantics. | blocking — the valid raw-identifier counterexample is sufficient; no unrun nested-closure result is claimed. |

R13 changes the proof layer under design.md's explicit C8/C10 revision, approved through the existing Gilfoyle delegation. R11's token-flow implementation is superseded, not retained as redundant or partial protection; R12's default module ownership remains. Parent owns source/runtime replay and integration; an isolated writer prepares the verification-only consumer and CI step. Required outcome: source ownership and native runtime together refuse every retained negative, while unrelated valid code passes. Source/runtime classification must remain separate. Implementation9/10, full PR review2/5; no publication authorized until new native/Linux/Windows proof, mutation restoration and independent review pass.

R13 qualification, current uncommitted attempt9 sources:
- Source pins (SHA256): check_shape.py `30e890c50fbbd62215d6dab51b40670124fa5381a97bfd3f30ce4a6666e41c0b`; rust_source.py `7617fdd5f279f1805b8f9cbf40898852df35d73a5154fd143719cfbe4857f7f1`; mutation_f2.py `a7b2a420a7b0998b5ff33cfd2745dd98e1b6cd20b292b0f239b623b1746e4edf`; CI `265addb398c74f8954b68d5bd2ae8364b11d18f972336da3b750531cfe017157`; example `d2f3952104b35b879f598f436d6549fb11ea569a11b1f5e7e5fa76ef57e84ea8`.
- `python3 .cyril-s2hb/oracles/mutation_f2.py --runtime`:43 source matrices,0 unexpected;9 native executions,0 unexpected. The pinned b4 source-only CI accepts all four semantic defects and has no runtime step. Current compiled decoy refuses None versus Some(Posix); discarded/early/Ok-shadow handles refuse the closed completion channel. All three unrelated macro/raw/closure controls and fresh healthy restoration execute exact child argv and complete both bridges. These are runtime failures after successful compilation, not source-census detections.
- The actual malformed Rust compiler control includes RUN/FAIL strings in its source but emits E0425 and is classified UNKNOWN. Missing current runtime-step extraction raises ValueError, not historical SKIP_RUNTIME. The actual default-feature consumer emits RUN/FAIL and exit1; restored `--features kas` emits RUN/PASS and exit0. No permanent compiler/source-text unit test was added.
- `env -u CARGO_TARGET_DIR cargo fmt --check`, `cargo test`, default Clippy and core KAS all-target Clippy passed;2,096 tests passed/13 ignored. Actual `review_prefix` runs through bash and fish passed. Both full ownership comparison modes against76bfb and integration-only passed, including145/190 production census.
- Native Windows archive `4ef0488d2b166da991c920d19e07bb9204a226810be56bc0961861bf9b27ef45` was SHA256-checked before extraction. Rust1.94 rebuilt the example after crate-root timestamp refresh. Exact CI Cargo commands through Windows PowerShell5.1 and pwsh7.6.6 each emitted RUN/PASS, exact child argv and both bridges completed. The VM has MinGit without Bash: these native consumer results are not a claim that the literal Git Bash CI wrapper ran locally. That wrapper and macOS remain current-head GitHub CI obligations. The first script-file invocation was policy-blocked before qualification; inline execution changed no execution policy. An unchanged host_io.rs unused-mut warning is not a Windows Clippy PASS.

R13 impact/reuse: LSP found extract_review_step's definition and sole run_ci caller and renamed both to extract_step; the extractor now serves both actual CI bodies. Existing Case/fixture_tree/run_process/combined_output infrastructure is reused rather than copying fixture setup. RUNTIME_EXPECTED and runtime_verdict are new private replay policy, with no predecessor callers; native receipts deliberately differ from C10 source-policy diagnostics so compilation is never semantic detection. run_runtime_cases owns only disposable trees/one dedicated target and invokes that shared machinery. New example main/qualify/execute_prefix and marker constants replace the obsolete probe entrypoint; bridge setup/shutdown reuses its existing public consumer, prefix construction stays in CrtoolPrefix, command/file/timer behavior uses std/Tokio/tempfile already available. The example's private Result alias preserves heterogeneous consumer errors without a new domain interface. Shell argument spelling is intentionally independent of HostShell's private renderer: this is a consumer falsifier, not another product resolver.

Symmetry: (1) operational errors remain nonzero, not defaults; (2) both native success/failure have explicit claim receipts, unlike source-only C10 policy output; (3) no fallback, absent feature refuses; (4) caller can distinguish source acceptance, runtime refusal, compilation/operational UNKNOWN and historical absent-step SKIP; (5) bridge drops/timeout reuse the original consumer, shell execution is one synchronous native proof call, fixtures/targets are owned and removed; (6) C10 structural guards remain, semantic carry moves to executed C8 without dropping any recorded negative; (7) decoy/discard/early/shadow inputs now refuse in composed qualification while valid macro/raw/closure inputs pass. No production-reachable parallel implementation was introduced; differential product-path parity is N/A for this verification-only cutover.

Evidence disposition: all four production/wiring files are byte-identical to published b4, so its applicable native path/hazard, complexity and isolated production-conformance evidence is retained by reference. Source/fence/CI conclusions from earlier attempts are historical and replaced by the current43-case plus9-execution results. Current cumulative delta before this receipt was3,821 including the untracked126-line example, within3,960. Final size is still measured before commit. All eleven checkpoint obligations inherit their still-valid production evidence or the fresh results above; final judgment and independent repair re-review remain PENDING, owned by this checkpoint. No commit or full-PR-review completion is claimed here.

### Attempt10 — bind child role independently of argv

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F18 | Parent-shaped arguments in a child invocation start another qualifier instead of refusing. | Main boundary probe | Verified | Current compiled example with CYRIL_REVIEW_PREFIX_MARKER set and bash posix args exits0 with RUN/PASS, leaving the requested child marker absent. A malformed self-prefix using parent args can therefore recurse. | Accept | R14: examples/review_prefix.rs selects child role by marker presence before dispatch; mutation_f2.py rejects parent-shaped child argv in each healthy native fixture. | blocking — enforces the approved exact child-argv contract, not a new product mode. |

R14 inherits C8/C10 and plan A1/attempt9 without changed responsibility, oracle meaning or risk. Add the role control first and observe it refuse the existing example; then correct main and requalify native/default/quality plus affected replay. No product code changes. NativeRepairReview9 judged its bounded attempt9 scope correct; F18 invalidates that consumer conclusion and requires a fresh independent pass. SourceRepairReview9 is being given the replay addition before its final verdict. Counter10/10; no further substantive implementation cycle is authorized. Prior source-only matrices and unchanged production conformance remain applicable; changed consumer/runtime proof remains PENDING until rerun.

Review error: NativeRepairReview9 initially mistook Rust `[0]` for an empty buffer, then explicitly retracted the finding. It is a one-byte array and already checks EOF; no code change or defect is attributed to it.

R14 results: `child_role_refused` was red against the compiled attempt9 source `d2f39521` (exit0/RUN/PASS), then green on corrected source (exit1, no RUN, no marker). Final example SHA256 `648d6e86964734d7010a36598f634ac9bc9a1c9d783cf20eb0a54b1a76240e08`; replay `4109f59f0de6457ac8ced38b2713929fde5ab8a9374692152d4330a2df273de2`. Other R13 source pins are unchanged. The complete `--runtime` run again reported43 source matrices/9 runtime executions with0 unexpected, and both healthy role controls refused. Native Windows archive `10a8a2aee1914a989d6ea323659f81cbf2dd4a3337db4655b681cc95abfffd9b` was hash-checked; PowerShell5.1/pwsh7.6.6 each passed and the same permanent Python role control refused on Windows. Linux formatting,2,096 tests/13 ignored, both Clippy selections, bash/fish, default-feature refusal and restored KAS passed. Owned temporary qualification scripts were removed.

ConsumerRoleReview10 independently returned correct with zero findings for R14. The existing native marker protocol is reused by the new private child_role_refused helper; its cross-language environment-key literal is the deliberate executable/driver interface, not a second product configuration source. It reuses run_process and the already-successful current binary, owns its marker directory, and cannot turn a missing executable into refusal because process launch errors propagate. Parent/child divergence is deliberate: a marked child may only write the marker for sole crtool argv and never start qualification; unmarked parent behavior is unchanged.

### Cap10 stop — unresolved operational-classification defect

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F19 | Any post-start operational error can count as semantic mutation detection. | SourceRepairReview9 | Verified | With correct production and current consumer, private bash exits74; runtime_verdict reports FAIL rather than UNKNOWN. | Reject (tracked at cyril-mrvc) | No classifier repair in A1; explicit requester exception in design.md, diagnostic warning in mutation_f2.py, and verified follow-up cyril-mrvc. | non-blocking under the explicit A1-only risk acceptance below; the technical defect remains, and runtime verdicts are not semantic proof. |

SourceRepairReview9 found no remaining source-owner/dispatch/isolation/child-role defect, but returned incorrect for this runtime classification. Its proposed distinct semantic receipt is consistent with the existing C8/C10 proof-layer contract. The parent reproduced F19 without changing production or committed sources; the temporary shell was removed. This is not a shell/host availability blocker and not a waived fence.

Final bounded gate judgment at attempt10: (1) affected tests PASS; (2) falsifiers FAIL on operational attribution; (3) retained production stress PASS; (4) independent oracle agreement FAIL for correct-product/shell-exit74 classification; (5) module shape PASS; (6) production-scale budget PASS retained, wall/always-on budgets N/A per plan; (7) qualification regression fence FAIL; (8) required operational-class discrimination proof FAIL; (9) corresponding repair/restoration FAIL—not implemented; (10) reuse/symmetry inventory PASS but its identified classification defect remains F19; (11) preserved proof attribution FAIL. R14's isolated role correction passed, but the assembled repair does not. No commit/push/merge is authorized by these results.

Issue-wide implementation cap is exhausted at10/10; full PR rounds remain2/5. PR146 remains draft at published b4e79c17bacb98b850235c28195957bdcfeddac3. All uncommitted repairs and evidence are preserved; A2 is still unverified and B unimplemented. No dependent issue or parent tracker record is advanced. Resume only after an explicit cap decision.

Preserved cumulative working delta measured3,879 changed lines including the untracked128-line example before this handoff paragraph; below plan allowance3,960 and hard partition4,000. The blocker is F19 plus exhausted authorization, not review size. No temporary mutation or qualification script remains in the repository.

### Requester-authorized A1 oracle bypass

Requester: “If we have proven the correctness of our work, lets bypass the oracle”. Applied narrowly through design.md's explicit exception and plan.md's affected mutation field: **N/A — approved risk: F19 bypassed for A1 only**. This changes the F19 disposition above; it neither fixes nor denies the defect. The earlier cap-stop judgment remains historical. No product acceptance, native execution, source ownership, other finding, fresh full-PR review or current-head CI gate is bypassed.

The condition is supported independently: published-b4 identity of all four production/wiring files; retained literal prefix/hazard/native-path and isolated conformance receipts; final actual bash/fish/Windows PowerShell5.1/pwsh7.6.6 consumer results and exact argv; fresh Linux quality and role-boundary red/green. F19 concerns attribution of a nonzero mutant result, not acceptance of a failing ordinary product run: every nonzero native consumer still fails CI. Runtime mutation matrices are diagnostic observations only and no longer cited as semantic proof.

Tracker lookup covered46 registered worktree copies,364 unique IDs, no IDs absent from primary, and full-text searches for PR146/F19/mutation_f2/runtime_verdict/post-start operational/semantic mutation. The scout initially searched only the active copy; parent completed the missing cross-worktree search rather than treating that as exhaustive. No reusable record existed. Created and verified open bug `cyril-mrvc`; installed Rivets accepted the tracker and its write added exactly one record, preserving every previous line byte-for-byte, including parent cyril-5gb3.

Only the replay's disclosure changed after attempt10: help and runtime output warn about F19; classification, fixtures, CI and all Rust bytes are unchanged. `python3 .cyril-s2hb/oracles/mutation_f2.py --help` executed and displayed the limitation. Existing execution/source receipts retain applicability; the warning is not a classifier repair or another implementation attempt. Upstream moved76bfb→cff5def9 with only unrelated .cyril-lki9 artifacts, no product, tracker, CI or owned-file overlap.

Reconciled precommit checkpoint:

| Obligation | Judgment |
|---|---|
| 1. Affected tests | PASS — attempt10 formatting,2,096 tests/13 ignored, both Clippy selections; warning-only delta does not alter these paths. |
| 2. Falsifiers | PASS — direct C8/C10 receipts retained; automated runtime attribution N/A — approved risk: F19 bypassed for A1 only. |
| 3. Stress | PASS — unchanged production's native path/hazard/namespace matrix retained. |
| 4. Independent oracle | PASS — literal prefixes/expected dialects/exact native child argv; automated attribution N/A — approved risk: F19 bypassed for A1 only. |
| 5. Shape | PASS —145/190 census, full/endpoint/integration modes and source-owner review; approved production ownership unchanged. |
| 6. Budget | PASS — retained linear production work/storage; wall and always-on budgets N/A per plan. |
| 7. Regression fences | PASS — product/source/native/child-role checks remain; automated attribution N/A — approved risk: F19 bypassed for A1 only. |
| 8. Mutation | PASS — independent dollar/wrong-opening/source-owner and role red receipts; F19 attribution N/A — approved risk: F19 bypassed for A1 only. |
| 9. Restoration | PASS — retained restored product and fresh native/role controls; F19 repair/restoration N/A — approved risk: F19 bypassed for A1 only. |
| 10. Reuse/symmetry | PASS — R13/R14 inventory retained; new warning reuses existing diagnostic output, no new algorithm or parallel product path. |
| 11. Enforcement | PASS — no ordinary CI/source/product gate removed; automated attribution alone N/A — approved risk: F19 bypassed for A1 only, tracked at cyril-mrvc. |

Bounded reviews cover source/dispatch/isolation (SourceRepairReview9, whose sole remaining F19 is now explicitly accepted risk) and final consumer/role (ConsumerRoleReview10, correct). The disclosure-only amendment requires no new behavioral repair review. Publication may proceed to fresh full PR round3 and current-head CI; neither is claimed complete. Implementation count remains10/10. A2/B still need separate authorization for further implementation; no issue or source parent is closed by this exception.

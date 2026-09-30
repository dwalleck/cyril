# PR146 native-fixture review decisions

Atomic receipt for F1/F3 and Q1. F2 lands separately; this is not a full-round completion receipt.

Reviewed head: `1dae375f2b0e3131de8dee2689f2e3fd414e43c4`. Reviewer: fresh isolated PrefixPrReview1. Issue-wide PR review round1/5. Attempt2 produced the first repair; attempt3 refines its argv fence and corrects the observed executable-fixture race. No counter resets for A2/B.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Unix native smoke assumes `/bin/printf`; use an owned portable fixture. | PrefixPrReview1 | Verified | macOS job109788629393 in run36684976118 fails the native-prefix test's exit-status assertion; source uses `/bin/printf`; all other eight prefix tests passed. | Accept | R1: `review::tests::generated_posix_prefix_runs_as_a_native_shell_command` executes a per-test alias of `tests/fixtures/native-prefix.sh` through the generated prefix and checks exact argv/output. | blocking — preserve native execution rather than skip macOS or weaken assertions. |
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

## Evidence disposition and remaining gates

The R1/Q1 local checkpoint and fresh independent repair review pass. Retained production/native Windows conclusions remain applicable because production bytes are unchanged and fixture changes are Unix-gated. F2 is a separate pending commit. Required current-head hosted CI and a fresh full PR review remain publication gates; no merge or issue completion is claimed. No tracker delta is included.

## Review errors

No additional review error is claimed. The diagnostic log demonstrates the failed test, not the exact missing-program stderr; the root-cause explanation above distinguishes those facts.

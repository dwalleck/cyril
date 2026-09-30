# S2HB-A1 prefix checkpoint

Governing claims: C8 and A1's C10 scope in design.md. Plan: S2HB-A1. Issue-wide implementation attempt1/5; PR review rounds0/5 at this checkpoint. All eleven A1 obligations below pass; this is not completion of cyril-s2hb or its later increments.

## Entry and source state

The original A draft exceeded its projected review size. The plan owner split prefix from gather/facts without reducing coverage. This worktree contains only prefix production and core wiring; the downstream leaf/CLI draft remains in its original worktree. Discovered upstream baseline: `76bfb1efb45e96f5efe41f0a8be434768da0ddc3`. The existing core files had byte-identical base contents to original design baseline `cc5eba260572e08fd20a58c8b0cdd1aaec782484` before integration. No changed resolver, process or formatting premise invalidates the retained empirical probes.

Entry critique: C8 is linear in path bytes, not filesystem size; the actual long-path smoke is stronger than string-only construction. Native dialect execution is separate from the cross-platform literal matrix. A1 makes no hidden CLI or /review availability claim. HostShell still resolves once; no configuration fallback or OS-guessed dialect is introduced. The prefix Length review is recorded in design.md; source is145 production-region lines against190.

Production SHA256 at qualification:
- core lib.rs: `aa2e3d5cc660a54cc58a674c1d7296a4a57062295b2dbfac7c89a95adcacdf2b`
- protocol/bridge.rs: `1cd20d40adf5abd5082f6c478d71be6d874601c0b19159bab8599888ac427b83`
- protocol/kas/host_shell.rs: `52c7cad1a12831b1b0860f99759ee8ef8022e2f604601607fd0a8189e481d78c`
- review/mod.rs: `4036e6ff47100322ea68104dbb2d39f0f42703a19ebdc785a71648a433a2b64f`

## Impact and ownership

Initial linked-worktree LSP references failed even at known bridge callers; reload did not repair resolution. The original impact census used targeted grep rather than treating empty semantic results as absence. BridgeHandle constructors at bridge.rs30/69/150 receive the new field; sender/split signatures are unchanged. HostShell::resolve remains at its existing bridge call. spawn_bridge callers include binary main, examples/test_bridge, workbench reviewer, bridge tests, and KAS/spawn/session/platform integration tests; none requires signature migration. New prefix constructors and review_shell getter have no predecessor consumer. cyril-iowg owns the later App capture before splitting.

Existing HostShell owns command rendering and resolution. C8 deliberately rejects executable hazards under a fixed quoted-prefix grammar instead of reusing its arbitrary-command escaping. Core lib only exports; bridge only carries/projects an optional dialect. No leaf, main, Cargo or existing command semantics change in A1. CI extends the existing KAS lane with the standalone ownership gate; existing builds, tests, lint and required CI Success dependencies remain in place.

## Linux execution receipts

Environment: native Linux x86_64, repository-pinned Rust1.94.0; `/usr/bin/sh` and `/usr/bin/fish`. The shell environment supplied an empty CARGO_TARGET_DIR, so the initial Cargo invocation refused before compilation. All effective commands use `env -u CARGO_TARGET_DIR`; this is a qualification-environment correction, not a source/config change.

- `cargo test -p cyril-core --features kas review::`: **9 passed**, including actual `/bin/sh` invocation, dialect matrix, Unicode/spaces, safe quotes, PowerShell-specific quote refusals, relative/non-UTF8 paths and surviving backslash refusal.
- `cargo fmt --check && cargo test && cargo clippy -- -D warnings && cargo check -p cyril --features kas`: exit0. Workspace test aggregate: **2,096 passed,13 ignored**. No ignored test is claimed as executed acceptance.
- A temporary core example called the actual public `CrtoolPrefix::current`, printed the prefix, and printed `native-prefix-child` only when invoked with the resulting `crtool` argument. Built with `cargo build -p cyril-core --example prefix_smoke`; copied into owned temporary directories. Each prefix was compared with the independently canonicalized literal quoted path before executing it through sh and fish. Both shells returned exactly `native-prefix-child\n`, empty stderr and exit0, for an84-byte path containing spaces/Unicode/apostrophe and a2,891-byte nested executable path. Observed elapsed values: POSIX0.0058/0.0057s; Fish0.0998/0.0618s. These are observations, not latency guarantees. The temporary example was removed after smoke.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase prefix`: `C10 census crates/cyril-core/src/review/mod.rs: 145/190`; `PASS C10: prefix ledger, dependencies, protected parents; base=76bfb1efb45e96f5efe41f0a8be434768da0ddc3`.

## Named mutation receipts

Exact uncommitted prefix/main bytes were preserved outside the worktree before mutation; no checkout/reset restoration was used. No build ran concurrently against the same mutated source.

1. **C8 dollar guard:** remove only `$` from the forbidden-character match. `cargo test -p cyril-core --features kas review::tests::every_shell_hazard_is_refused -- --exact` compiled and failed1 test, exit101: `unsafe path unexpectedly produced "/tmp/cyril$bin" crtool`. Restoring the match produced1 passed, exit0.
2. **C10 ownership:** add `fn review_gather_body() {}` to binary main. The standalone prefix gate exited1 with both `prefix increment must not change binary startup` and `protected-parent body changed outside approved wiring`. Remove exactly that insertion: exit0 with the original PASS census.
3. **C8 wrong dialect:** replace the PowerShell opening `& "` with the POSIX opening `"`. The exact dialect test compiled and failed1 test, exit101: actual `"/opt/Cyril tools/cyril" crtool`, expected `& "/opt/Cyril tools/cyril" crtool`. Restore the opening: complete prefix suite9 passed and formatting exit0.

The C8 Cargo fences are identified by their exact qualified test names above; their assertions expose the failing observed prefix/character. C10 reports its claim ID directly.

## Native Windows qualification

Required host resourcefs-win11 reported native Git2.56.0.windows.1 and Rust1.94.0. Qualification archive SHA256: `4041a468ce21004c86e8659ec2f7c3ef866b955f82cc23b2601119087e1700a0`; it contains the stated baseline plus exact changed core files and temporary consumer. Host download checked this hash before extraction. `cargo test -p cyril-core --features kas review::`: **9 passed**, including native verbatim-drive/UNC and unsupported namespace branches. The actual prefix consumer compiled and executed from `C:/probe/cyril-s2hb-prefix/native 雪 O'Brien/prefix smoke.exe` in both Windows PowerShell5.1 and pwsh7.6.6, returning exactly `native-prefix-child` and exit0. The prefix also equaled the independently constructed literal path.

Qualification corrections, not Rust fixes: Windows PowerShell5.1 initially decoded native Rust stdout using code page437. Numeric inspection found `[920,162,172]` where the literal path contained `[38634]` (雪); the file existed. Setting `[Console]::OutputEncoding` to UTF-8 made the generated prefix equal the literal and execute successfully. The outer PowerShell5.1 `pwsh -Command $prefix` transport still exited1; passing the identical command text through pwsh's UTF-16LE `-EncodedCommand` interface returned the required marker. No quoting or validation production rule changed. The initial shell-quoted vcvars command and HTTP script byte/string conversion also failed before their intended stages; corrected commands executed the actual native build, not substitutes.

Native builds reported existing Windows-only unused imports/mutability/helpers in host_io/host_shell/sdk_runtime/terminal_io. Those unchanged code paths were not suppressed or modified. The exercised native Cargo tests and consumers exited0; no Windows Clippy PASS is claimed.

## Resolved-dialect runtime proof

The retained `.cyril-s2hb/probe_resolved_prefix.rs` was temporarily compiled as the core example `resolved_prefix_smoke`. It calls actual `spawn_bridge`, observes the getter before split, then drops all channel owners and awaits bridge completion. A private empty temporary workspace and intentionally invalid Free/replacement-environment combination prevent launching an agent; this proves shell resolution/carry and shutdown, not ACP connectivity or authentication.

- Linux KAS build: configured `bash` → `Some(Posix)`, `fish` → `Some(Fish)`; V2 → `None` for both. All four bridges completed.
- Linux non-KAS build: both Kas and V2 inputs → `None`; both bridges completed.
- Native Windows KAS build: configured `powershell` → `Some(WindowsPowerShell)`, portable pwsh directory on the process-local PATH and configured `pwsh` → `Some(Pwsh)`; V2 → `None` for both. All four bridges completed.
- Initial smoke arguments incorrectly supplied executable paths where the existing resolver accepts choice names. Both hosts returned the existing typed Unsupported error. The corrected arguments above exercised the real resolver unchanged.

`.cyril-s2hb/probe_prefix.rs` retains the exact public-current-executable consumer compiled on both hosts. To reproduce, copy it temporarily to the absent `crates/cyril-core/examples/prefix_smoke.rs`, and copy `probe_resolved_prefix.rs` to the absent `crates/cyril-core/examples/resolved_prefix_smoke.rs`; build/run the named examples with the recorded native shell inputs, then remove only those owned copies. Do not overwrite an existing example. No probe is an installed command or production runtime override.

## Reuse and symmetry dispositions

`reuse-A1.md` freezes the independent static per-symbol/helper/dependency census and all seven symmetry answers. Parent verified its named `test_support::must_succeed` helper actually accepts a context argument, so the suggested loss-of-context rationale was incorrect: the prefix test helper now delegates to it, retaining the local context instead of duplicating a Result match. No production owner or acceptance changed.

The census's runtime gaps are discharged by the real `current()` executions and resolved-dialect smokes above. No permanent getter/forwarding test was added; behavioral prefix tests and the standalone projection ownership gate remain the regression fences. Injecting OS lookup failures would require a new backend seam, which the approved design excludes; source-bearing error propagation remains explicit and no such injected test is claimed.

Final `review/mod.rs` SHA256: `e19460c96944edd6910a61e4b3d801cd99335da96615f778eca1cf6bfafa83a8`. Only the test helper differs from the initial qualification snapshot; the entire production region was compared byte-for-byte unchanged. Prefix suites were rerun on this final source: Linux9 passed, Windows9 passed. Linux formatting and `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` also exited0. The native current/bridge smoke and mutation conclusions remain applicable: no predicate, expected value, execution path or mutation mechanism changed.

Preserved enforcement: the only modified pre-existing policy file is `.github/workflows/ci.yml`, adding full checkout history and a prefix shape check in the existing KAS lane when review-owned paths change. No prior gate, validator, oracle or policy is repointed, relaxed, deleted or orphaned. The Python review oracle and all existing CI lanes remain intact. The ticket-specific delta check is scoped to `.cyril-s2hb`/core review changes so it does not freeze unrelated future edits to shared parents. Its base comes from GitHub's PR/default-branch metadata, not an assumed remote HEAD symbolic ref. An isolated real-Git fixture proved the selector skips an unrelated README commit (exit0) and selects a prefix-owner commit (exit1); actual source-census behavior was separately proved on this worktree.

## Independent conformance

`reconstruction-A1.md` freezes PrefixReconstruction's blank-context production-only reconstruction, obtained before exposing the approved design. A separate fresh reviewer, PrefixConformance, verified it against the approved design and current source; `conformance-A1.md` records **PASS**, all four applicable owners MATCH, no MISMATCH/UNCOVERED/MISSING row. The duplicate private/public dialect enums are an approved lossless adapter, not a second resolver; rejection versus arbitrary-command escaping is deliberate; tests enter the public prefix boundary. Existing-parent growth is export1, bridge19 net lines and HostShell10, all within their approved bounds.

## Eleven-item checkpoint judgment

| Obligation | State | Evidence / applicability |
|---|---|---|
| 1. Affected unit tests | PASS | Final-source prefix suites9 Linux/9 native Windows; Linux workspace tests and KAS all-target Clippy above. |
| 2. Assigned falsifiers | PASS | C8 literal/native shell and actual resolved-dialect observations; A1 C10 source census and independent conformance. Other claim owners remain A2/B. |
| 3. Stress fixture | PASS | Hazard/dialect/platform matrix plus84-byte Unicode/spaced/apostrophe and2,891-byte native paths; typed invalid-path refusals. |
| 4. Independent oracle agreement | PASS | Literal path/prefix and native child marker agree; independent configured-shell expectations match actual bridge values. |
| 5. Approved module shape | PASS | Standalone145/190 census, protected-parent checks, isolated reconstruction and fresh four-row MATCH comparison. |
| 6. Production-scale budget | PASS | One reserved output String, one character-validation traversal and bounded native spelling traversals, O(path bytes) work/storage; long-path execution observed. Wall latency: N/A — plan declares no SLA. Always-on phase: N/A — none added. |
| 7. Regression fence | PASS | Prefix behavioral matrix and standalone C10 ownership gate green on final state. |
| 8. Named mutation | PASS | Dollar omission, wrong PowerShell opening and forbidden parent body each produced the recorded localized failure. |
| 9. Restoration | PASS | Original mutation snapshots restored byte-exactly; subsequent helper-only deduplication changed no predicate/assertion/mutation applicability, and both native suites passed again. |
| 10. Parity and reuse | PASS | Frozen per-symbol census, seven symmetry answers, verified helper reuse correction, and runtime-gap dispositions above. |
| 11. Preserved enforcement | PASS | Existing CI checks retained; only an additional scope-qualified source gate. No replaced detection set or policy waiver. |

Stale/dead-path sweep: no compatibility alias, diagnostics/CLI stub, new resolver, or production test backend was introduced. Temporary compiled examples were removed; retained probe sources are qualification evidence. Source and docs describe current A1 behavior, not completed `/review`. Parent tracker cyril-5gb3 is untouched; no tracker file is staged.

Before final comparison receipt, the staged increment measured1,893 changed lines including artifacts. Even with the recorded20% churn margin and the comparison/checkpoint additions, it remains below4,000; the next increments stay separately mergeable. Commit/publication and required current-head PR review/CI remain later obligations, not claims of this checkpoint.

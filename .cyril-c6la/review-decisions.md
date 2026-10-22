# cyril-c6la review decisions

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Actual cfg(test) operation counters before/in renderer functions make the existing source-fence helper stop before covered runtime code; preserve scanning through the real test-module boundary. | SharedMarkdown integration handoff; direct source investigation | Verified | theme.rs production_source split at the first literal cfg(test), while the completed Markdown counter declarations precede its renderer and inline increments occur inside it. All covered widget test modules use the existing separate cfg(test)/mod tests form. | Accept | Change only the helper split to the exact conventional test-module marker; add production_source_keeps_runtime_after_test_instrumentation against that real helper. | blocking — required source-coverage repair applied; parent execution, sensitivity and scoped independent review remain PENDING. |
| F2 | Completed Read/Execute qualification payloads supply Text content but omit the authoritative raw_output required by the preserved completed-output policy; fix fixtures, not production. | IndexedChatRenderer source finding; RendererQualification confirmation | Verified | output_text_view starts from raw_output; unit tool/edit_tool/adoption and benchmark completed_tool/old-tool updates lacked it while asserting output markers. | Accept | Add JSON-string raw_output alongside unchanged content in those exact fixture constructors/updates. | blocking — fixture correction applied and source re-frozen; parent consumer/benchmark execution and scoped review remain PENDING. |
| F3 | Newly nested approved widget modules are absent from the existing immediate-file theme source fence and byte accounting. | Direct source investigation at final handoff | Verified | Original theme guard includes/censuses only 17 immediate files; five new nested sources escape it. Main relayed explicit user approval for a fully recursive total cap22, cap23 only with MCP, same byte cap/prohibitions. | Accept | One 22-source path/content inventory, exact recursive path-set census, duplicate detection, total <=22/300000 and the same five checks over every production source. | blocking — approved correction applied; parent actual source gate, A/B mutation proof and independent review PENDING. Split subgroup proposal was not adopted. |
| F4 | Prefix boundary detection copies a potentially huge noncrossing first grapheme; suffix detection allocates one probe String per Prepend scalar. | Direct source inspection against Main's only-necessary-crossing-bytes approval | Verified | Original affixed uses format on the first non-ASCII symbol before checking a crossing and format for each two-scalar suffix probe. | Accept | Borrowed GraphemeCursor/PreContext before materializing a true crossing; eight-byte UTF-8 stack suffix probe; exact approved existing1.12.0 dependency edge and source-range/fidelity controls. | blocking — scoped correction applied and source frozen; parent focused tests and independent review PENDING. No worker checks or expected-output narrowing. |
| F5 | Newly covered source getters/paint callback use the broad existing forbidden resolve( spelling, so F3 would reject legitimate source access. | Source-confirmed integration finding | Verified | presentation.rs recursive/preparation getters, retained.rs paint and chat_layout/mod.rs callback contain the exact fifth-prohibition token. | Accept | Rename only the new private seam/callback to source_text and migrate fourteen scoped declaration/caller sites across three files; all five prohibitions remain unchanged. | blocking — naming correction applied; actual gate/fidelity verification and independent review PENDING. No source-policy exemption. |
| F6 | Visible Diff rows repeat linear first-Diff selection through preceding hidden content blocks on every unchanged draw. | ReviewConversationLayout P2; independently source-confirmed | Verified | Parent artifact605 shows actual RED; bg116 raw781 shows all three frozen hidden-content workloads GREEN, including zero warm selector visits. | Accept | APPROVED/APPLIED: one indexed diff_view selection initializes/reads the lazy preview; prepared DiffRow stores content and row ordinals; paint directly indexes content and cached preview. First-Diff and equivalent-source Arc reuse remain; actual visit hook stays in the selector. Supplemental one-pass/absence and shifted-ordinal cache tests added. | blocking — targeted RED/GREEN PASS; independent F6 repair review and integrated/full qualification PENDING Main. |
| F7 | Qualification receipt ID older-receipt is rejected because it lacks the required steer- prefix. | ReviewConversationRoot P1; ReviewConversationHistory corroboration | Verified | Three original test binding/marker literals and two benchmark literals are invalid for the unchanged parser. | Accept | Authorized fixture-only correction to steer-older-receipt at all five sites; preserve every original assertion/status/note and parser. | blocking qualification setup corrected in separate test-only delta; not performance RED evidence. Parent execution remains PENDING. |
| F8 | Raw receipt-only and still-withheld delta revisions rebuild unchanged nonempty live Markdown. | ReviewConversationRoot P2; ReviewConversationHistory corroboration | Verified | Parent artifact605 shows actual RED; bg116 raw781 shows complete/opening/body work GREEN and both nested semantic positives still passing. | Accept | APPROVED/APPLIED: UiState owns cached displayed cutoff; direct-slice getter; append/harvest decides revision from displayed length or any deletion touching old visible prefix. HarvestedReceipts carries ordered receipts and minimum start across ALL removals; all nine old callers migrate. Raw flush resets cutoff and bumps for every nonempty raw commit; child/raw policy preserved. | blocking — frozen five-review GREEN PASS; retained Root source review found no further F7/F8 defect. Parser/state/consumer/full qualification PENDING Main. No length-only invalidation or body equality scan. |
| F9 | The separate color-source checker truncates actual production at cfgtest counters and omits all five nested widget files; repair its own splitter and recursive census without changing policies. | ReviewConversationRoot two P2 findings on tree49640ed; independently source-confirmed | Verified | Parent bg116 artifacts781/782 execute all seven expected A/B pairs with eight BEFORE/nine AFTER unfiltered tests; fresh Root review of treedc200a8 has no findings. | Accept | APPROVED/APPLIED: explicit22recursive widgets+highlight23 inventory, exact recursive path census/duplicate rejection, full-path diagnostics and LF-normalized ordinary-test-module-only cutoff; new LF/CRLF counter regression. audit_source and all signed/RGB/Reset/palette exemptions unchanged. | blocking — scoped actual-checker sensitivity/standing and independent F9 review PASS; final original/merged-tree qualification PENDING Main. No coverage substitution or new exemption. |
| F10 | First full-workspace Clippy rejects production expect sites and one equivalent cold-control comparison; static next-wave audit finds three modulo spelling candidates. | Parent QualifyPerformanceTrees actual gate; bounded scout/source inspection | Verified | Original519b1445 fails nine production expect_used plus int_plus_one; subsequent parent Clippy GREEN/F10 independent review correct.99 applies to prior formatted830b4673, whose next tests fail F11. | Accept | Frozen seven-file8ad00e53 after519b1445, preserving borrowed invariants and equivalent comparisons without policy/API/default/allocation/scan changes. | non-blocking — repaired and scoped reviewed; original receipts are not new43db6bd5 qualification. No warning suppression or worker checks. |
| F11 | Pinned WordWrapper/private copy append a positive glyph after a full carried prefix; correct Cyril's carry under the explicitly approved narrow parity exception. | Parent actual test RED; independent source attribution/upstream2679; ReviewCarryParitySource | Verified | Prior830b4673 UI882 PASS/3 FAIL/1 ignored; source derives width2 a界a unsafe start2. Fresh source review of applied wrap/presentation reports correct/no findings.93. No runtime caller stack fabricated. | Modify | APPLIED in frozen14945de1/tree43db6bd5: cached-width prefix flush, original-mode unsafe-start classifier, retained safe full-reference comparisons and normative row/cell/conservation/style/bounds coverage. No dependency/vendor/painter changes; positive overhang unchanged. | blocking — source repair/review complete, actual new-fence sensitivity and post-repair qualification pending Main. |
| F12 | Full-area scrollbar overwrites rightmost URL bytes; reserve fixed gutter without scroll-dependent rewrap. | Parent controlled Tqr native RED; C6 source inspection; ReviewStableGutterSource | Verified | Prior C6 full-width preparation/paint and overlay shares the actual Tqr mechanism. Fresh source review of retained/root tests/bench reports correct/no findings.93. | Modify | APPLIED in frozen14945de1/tree43db6bd5: width>1 content-1 everywhere and separate thumb gutter; width0/1 fullcontent/noindicator; complete140wide URL/style/warm-scroll/overflow/tiny-width controls and preserved80k giant fixtures. | blocking — C6 actual sensitivity/full qualification pending Main; source review and Tqr receipt are not C6 test PASS. |

## F1 bounded coverage-preserving repair

- **Governing obligation / owning slice:** the existing source fences must continue scanning the production portions of their included widget files. Design C1 requires counters at actual hash/parser/wrap/paint operations, not a substitute probe; those legitimate cfg(test) counters cannot turn the required fence vacuous. This preserves the declared obligation, not a new generic source-policy gate or an expanded theme policy.
- **Delivered change:** `theme::tests::production_source` now splits only at `\n#[cfg(test)]\nmod tests {`. Its ordinary regression input contains both top-level and inline instrumentation, followed by a forbidden resolver token and then test-only fixture text. The actual helper must retain the complete runtime input and forbidden token while excluding the test module. F1 itself changed only this helper/test, preserving the five assertions and then-current source/file/byte accounting; the separately requester-approved F3 below now owns the complete recursive22 inventory. This repair belongs to c6la, not the frozen x5xi patch.
- **Performed checks and results:** direct source inspection identified the real early counter placement and the old truncation mechanism. No build/test/lint/formatter/mutation run occurred in this worker. This ordinary regression uses the existing checker helper directly, not a second scanner/oracle. The pinned x5xi/zj0m independent sensitivity receipt does not qualify c6la or its source fences.
- **Parent RED/GREEN:** transplant the new `production_source_keeps_runtime_after_test_instrumentation` test unchanged to the existing pre-repair helper (or use the assembled tree with only `.split_once("#[cfg(test)]")` restored temporarily in an owned proof copy). Run `env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib theme::tests::production_source_keeps_runtime_after_test_instrumentation -- --exact --nocapture` on both helper versions over the same input. Expected old helper assertion RED, corrected helper GREEN; require one executed test. Compile errors/zero tests do not qualify. Then run the unchanged `widgets_only_use_the_explicit_theme` and the applicable full source/UI gates on the actual c6la tree with no temporary proof modules.
- **Unresolved gates / owner:** Main retains actual before/after logs and checked source identity, executes final assembled source/UI/workspace gates and obtains scoped independent repair review. These remain PENDING; no source-gate PASS or c6la completion is claimed.

## F2 bounded qualification-input repair

- **Governing obligation / owning slice:** design C1/C3/C4 and Slice 4 require real visible output controls through the preserved production decoder. Completed output remains raw-output-backed; only failed errors have fiv1's content fallback. Missing fixture data must never authorize a new production fallback.
- **Delivered changes:** in `conversation_performance_tests.rs`, non-Write `tool`, `edit_tool`, and the occupied-adoption ToolCallUpdated payload now include JSON-string raw_output matching their unchanged Text content. In `benches/conversation_render.rs`, non-Write `completed_tool` and the older-tool transition update include the same raw-output shape. Write Diff branches, IDs, statuses, marker text, content, all expected cells and all assertions are unchanged. No production file or behavior changed for F2.
- **Performed checks and results:** both implementation owners independently source-confirmed the mismatch and the qualification owner reported the exact completed correction and re-freeze. No worker executed tests/benchmark or claims a runtime result. The final c6la patch was generated after the renderer reported fixture correction and re-freeze.
- **Required parent proof:** run the declared full `conversation_performance_tests::` module, especially old-tool/receipt, prune A/B, tool-boundary, focus/adoption and both portable baseline tests, then the production-shaped `long-history` benchmark. The earlier failure would be a positive-control fixture failure, NOT a qualifying performance RED; the portable RED protocol still requires successful visible controls before unchanged-work assertions. Run `completed-write` unchanged in final qualification.
- **Unresolved gates / owner:** Main executes named consumers/benchmarks and final assembled gates on the frozen source, retains actual results/source identity and obtains independent review. All are PENDING. This is a bounded fixture repair, not acceptance narrowing, a new decoder, or permission to alter captured/frozen expectations.

## F3 approved complete recursive inventory repair

- **Evidence:** the existing `widgets_only_use_the_explicit_theme` includes the seventeen immediate sources and compares their count with nonrecursive `read_dir("src/widgets")`. c6la's five approved nested production modules therefore are outside the five prohibition assertions and scanned-byte sum, even after F1 preserves the correct production/test boundary.
- **Delivered correction:** one explicit path/include inventory contains all 22 Rust widget files. A single recursive filesystem walk compares the exact relative-path set, with a separate duplicate-entry assertion. The same production_source splitter feeds all sources into the same <=22 file cap, unchanged <=300000 aggregate byte cap and unchanged five prohibitions. Assertion diagnostics retain the source path. No counted subgroup or nested exemption exists.
- **Approval state:** Main explicitly relayed the requester's approval of a fully recursive cap22 for this renderer tree and cap23 after MCP inclusion. The original split-group/17-cap suggestion is superseded and NOT implemented. Main owns the combined23-source assembly, not an automatic reserved slot here.
- **Required parent proof:** run `python .cyril-c6la/gate-review-f3.py --target-dir /nonempty/owned/target` on this frozen22-source tree. It reconstructs the actual pre-F3 test function from reviewed fiv1 plus the unchanged pinned x5xi/zj0m patches, retains the same F1 splitter/current source, and runs the real gate on fresh identical source copies. Expected unchanged both PASS; forbidden nested production comment and unlisted nested .rs BEFORE false-PASS/AFTER FAIL; forbidden existing production comment both FAIL. It prints source/checker hashes and complete transcripts, never asserts or declares aggregate success. Compile errors/zero tests do not count. Harness and all actual gate/mutation results remain unexecuted/PENDING with Main; no proof files mutate production.

## F4 bounded allocation-free boundary detection

- **Approved obligation:** Main allows only genuinely derived crossing-cluster ownership, with unbounded pathological bytes counted and no per-frame assembly. Borrowing ordinary noncrossing source is still required.
- **Delivered source:** preparation preserves the existing filtered-first-symbol/pointer/non-ASCII guards. GraphemeCursor checks the virtual prefix/body boundary using the borrowed first symbol and requested borrowed prefix PreContext; only a false boundary creates the crossing label String. Suffix classification encodes its two scalars in an eight-byte stack buffer and uses the same pinned span segmentation; actual crossing suffix insertion and body-range accounting remain unchanged. No alternate renderer/oracle is introduced.
- **Dependency/API evidence:** source-verified registry unicode-segmentation1.12.0 grapheme.rs394-434/565-654 documents partial chunks and mandatory requested PreContext retry. Main approved the same workspace version/default-features=false and UI workspace declaration as the approval owner. Only UI lock membership is added; package/version unchanged.
- **Authored controls:** existing exact Paragraph/cell/style fixtures remain, including zero/narrow widths and controls; add large noncrossing ZWJ and 4096 combining/Prepend source controls for UserText/SteerEcho. `noncrossing_first_grapheme_keeps_its_original_source_range` checks nonzero source offset, zero owned bytes and authoritative pointer. No expected cells are regenerated/narrowed.
- **Parent proof / owner:** `env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib presentation::tests:: -- --test-threads=1 --nocapture`, then all declared renderer/UI/repository gates and independent review. No worker checks ran. This addresses one actual probe-allocation mechanism, not a new abstract source provider or general telemetry/budget.

## F5 bounded source-policy naming correction

- **Evidence/obligation:** enabling complete F3 coverage exposes legitimate source access named resolve to the intentionally broad fifth assertion `!contains("resolve(")`. The policy must not be weakened or made vacuous to accommodate a new helper.
- **Delivered change:** PreparedBlock::source_text and the painter's source_text callback replace only the new resolve identifiers; production recursion, preparation, painting and existing test calls migrate with unchanged inputs/outputs. AST proposal/application covered fourteen sites in presentation.rs, retained.rs and chat_layout/mod.rs.
- **Code-intelligence prerequisite:** actual linked-worktree LSP references and both rename previews returned no references / -32602 despite concrete local/cross-file callers; this failure was reported. Scoped structural discovery and AST rename were used only after that failed semantic route, with no primary-worktree edit.
- **Required parent proof:** the existing actual all-source theme gate and F3 A/B harness must execute as declared, alongside source/Paragraph/fidelity consumers and independent review. This worker ran no tests, source-gate mirror, formatter or Cargo command. Formatting remains an explicit parent-owned unqualified gate; no observed-pass claim is made.

## Independent source review and test-first repair gate

Layout review found F6 only; root review found F7/F8; history review independently confirmed those root findings and reported no additional history/state defect. All reviews are source-only and verdict incorrect; none supplies runtime RED or approval of a production repair.

The review delta changes only traits.rs test instrumentation, conversation_performance_tests.rs and benchmark fixture IDs. The cfg(test) cumulative diff_selection_steps getter counts each item inside the actual selected_diff predicate; no reset/mocked selector or production lookup change exists. Four new intended-RED tests drive unchanged real-root work: hidden first-Diff selection at 0/256/4096 blocks, complete receipt-only large live Markdown, ten chunked opening states and five chunked body states. All workload/control draws execute before final target work assertions and emit C6LA_REVIEW_CONTROLS_EXECUTED. A fifth positive test preserves equal-length changed visible content and earlier-prefix exposure through nested brackets; this standing must pass, not count as RED.

Exact all-new filter: `env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib conversation_performance_tests::review_ -- --nocapture --test-threads=1`. Require five executed tests, all positive-control markers, four intended work failures and the nested semantic positive; compile errors, zero tests, invalid protocol, missing cells or failed cold controls are not RED. Individual test names and candidate owner repairs are recorded in plan.md. No worker check ran and no production correction is authorized before Main retains actual qualifying RED.

## Actual review RED / production authorization

Main's bg94 [artifact605](artifact://605) executes all five frozen review tests against b84+9b7: four intended work failures, one nested-prefix semantic PASS and all positive controls. Warm hidden selection visits are 200/51400/819400 for 0/256/4096 blocks across100draws. Unchanged 413773–413777-byte live Markdown performs live1/hash1/wrap2 per withheld chunk; complete receipt wraps3 including its legitimate chip. No source/cell/status/cold failure substituted for RED.

Main explicitly DG-approved both bounded F6/F8 production owner repairs after actual RED, including ALL-removal earliest-offset metadata and unchanged parser/raw-flush/child contracts. Owners remain complete/frozen. Root independently reviewed tree49640ed and found no further F7/F8 production defect, while identifying F9 below. Parent bg116 has now supplied targeted GREEN; F6 repair review, original-fixture/benchmark qualification and full C6 proof remain pending Main. No worker check was run.

### Parent bg116: frozen five-review GREEN

**PASS — actual targeted regression sensitivity**, checked source `dc200a8276d7ed7aafbeced4bd7c4824e20451dc`, full source patch c69099f25f98fa6635abeeb0adc8caa9b2856211333b98eb147a2d3f553e4318. [Raw artifact781](artifact://781) executes the unchanged five-review filter:5passed,0failed,0ignored,881filtered,0.09s. Pair with pre-repair RED artifact605; these are not zero-test, compile-only or fixture failures.

All0/256/4096 hidden-block workloads execute their positive controls: cold selector visits1/257/4097, then exactly0 warm visits across100draws each, with zero warm preparation/hash/parse/wrap/Myers and unchanged visible/style controls. Every incomplete opening/body receipt chunk has zero live/hash/parse/wrap derivation on413773–413776-byte live source. Complete receipts on413773–413777-byte source do only the legitimate one-message/one-wrap chip update, with live/hash/parse/Myers0. Both nested changed-content positives still execute and PASS; changed source genuinely invalidates, unchanged warm source reuses.

Scope is these five regressions only—not original/full C6 tests, timing benchmark, native consumer or final repository qualification. Parent owns remaining proof; source/harness/snapshots were not changed to record this receipt.

## F9 bounded technical-proof repair

**Delivered:** governing obligation is each existing source policy's full widget coverage, not a new generalized parser or policy. Main explicitly approved this repair within the existing fully recursive22-widget inventory plus retained highlight (23entries here; combined MCP assembly will have23widgets+highlight24). Changed only `crates/cyril-ui/tests/widget_theme_sources.rs`: `production_source_text` normalizes CRLF then stops at the ordinary test-module boundary; `collect_widget_sources` recursively compares exact manifest-relative paths with the explicit inventory and rejects duplicate entries. Existing `audit_source`, signed functions, Reset comparisons, palette rules and CRLF drift coverage remain unchanged. Theme.rs's 300000-byte/five-prohibition gate remains independent and unchanged.

**Verification disposition:** F1/F3 repaired the different theme.rs checker; they cannot discharge this distinct palette/Color obligation. Retain both mandated checkers rather than merging/removing policies or building a new checker. This is the first bounded repair of this integration checker. The named gaps are post-counter production and nested-source/inventory omissions; use the real before/after target over the same source, not a duplicate audit implementation. Source inspection found no production Color/palette hits in the five newly included nested files; their Color test fixtures remain beyond ordinary test-module boundaries. This is not executed gate evidence.

**Frozen inputs:** `widget-theme-before-f9.rs`9961bytes SHA256 `661d8badc938a073ebcc2fa916b7b35907454d50f1b91be63ac662d54544cc57`; `widget-theme-after-f9.rs`12752bytes SHA256 `e3d7b75288869fae0a885c8318ea3600a4dd853c5655dcc6ec8702c7bb191403`. Complete actual checkers, preserved unchanged. `gate-review-f9.py` SHA256 `ff12c0466374d85f0fc3b2135bb0d90b41f8a80f35cfd31106dc5ae19e6a1055` was parent-executed in bg116, source original perf worktree, shared target `/home/dwalleck/repos/cyril/target`. Reproduce with `python .cyril-c6la/gate-review-f9.py --target-dir /home/dwalleck/repos/cyril/target`; no rerun was performed for this record.

**PASS — actual checker sensitivity and retained standing.** [Artifact782](artifact://782) and [raw781](artifact://781) retain all14 runs: each BEFORE executes8tests, each AFTER9tests, with0ignored/measured/filtered and both named cleanliness/census tests executed. Every mutated path has identical byte count/SHA across sides. All expected pairs are observed; no compiler/zero-test failure substitutes for a verdict. Copies retained required captures and rebuilt both Core/UI package roots at each shared-target switch.

| Identical input | BEFORE observed | AFTER observed |
|---|---|---|
| unchanged | PASS | PASS |
| nested Color::White | PASS (coverage hole) | FAIL, presentation.rs named |
| post-counter Color, LF | PASS (coverage hole) | FAIL, markdown.rs named |
| post-counter Color, CRLF | PASS (coverage hole) | FAIL, markdown.rs named |
| unlisted nested Rust file | PASS (coverage hole) | FAIL, exact extra path named |
| allowed signed conversion | PASS | PASS |
| existing top-level Color violation | FAIL | FAIL |

**PASS — independent F9 repair review:** fresh ReviewConversationRoot receipt checks treedc200a8/full patchc69099f2, actual checker, both snapshots, F9 delta and harness; no findings, overall correct, confidence0.99. It confirms all22widgets+highlight, exact recursive/duplicate census, normalize-before-cutoff, unchanged policies/theme cap, valid snapshots, fresh identical inputs and executed-test verdict requirements. This is source review; the parent execution above separately establishes actual sensitivity.

The earlier F3 harness received the same fixture-copy/touch correction only; checker/source mutation meaning is unchanged. Current `gate-review-f3.py` SHA256 `b134d6d51f6ba527276615bc3988c4feb06158a28a6d73d4cfa9af0334f907ea` is still UNRUN. No temporary probe is inserted in production. All frozen five F6/F8 tests, source production and theme.rs remain unchanged by F9.

**Unresolved:** Main still owns final original-tree/full C6 and later merged24-entry source-policy qualification, original behavior/consumer tests, remaining F1/F3 checker proof, benchmarks/native smoke, final review coverage and issue publication. F9's unchanged AFTER fixture executes the actual full integration target with9PASS; no separate original-root full-suite run or platform-green is inferred. No worker build/test/lint/format/harness/smoke execution occurred, and no source or proof input changed for this evidence-only update.

Run these frozen F9 snapshots in the original22-widget worktree, not a combined tree containing the later MCP widget: BEFORE's original flat census would otherwise reject that unrelated extra top-level module and invalidate baseline standing. Main separately owns the approved combined23-widget+highlight24 inventory migration and final merged-target qualification. The source-only F9 delta/pins and latest34-file source tree are owned by route.md.

## Qualification prerequisite applicability

Main has separately merged the common auth-fixture commit identified in route.md's qualification identity. Main reports bg118's six deterministic RED/GREEN sensitivity cases and independent review for that test-only immutable-interpreter repair; this worker did not execute or independently inspect that proof and does not relabel it as C6 feature evidence. No production auth change, feature-source edit, proof-input edit or feature-patch regeneration is made here.

**Retain F9 PASS:** covered widgets, actual checker, frozen snapshots and harness remain byte-identical across the prerequisite merge, so the existing bg116 A/B and independent F9 review still establish their original claims. The original receipt/source pin is preserved rather than relabeled as a post-merge full-workspace run. Main still owes current-checkout full C6/merged qualification, benchmarks/native consumer and publication. PR183 alias propagation remains a separate parent-owned later stack refresh.

## F10 bounded first-Clippy repair

**Observed qualification failure, not rerun:** parent `/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/c6la/01-clippy.log` records `cargo clippy --workspace --all-targets -- -D warnings`, exit101. Nine production expect_used findings are presentation124/128/253, retained183/195/258/265/314, chat_layout/mod37; test int_plus_one is conversation_performance_tests1464. Parent formatter00-initial-fmt.log completed `cargo fmt --all` with exit0 BEFORE this failure. No full tests/build/bench receipt exists from that halted gate sequence.

**Provenance preserved:** qualifier `pre-format.source.json` identifies actual pre-format tree53386832198e5c430e9a48a5e6567aee53a4be76, tracked-vs-HEAD patch SHAce30e78787f3eb6e722bf9c0231dc3f25d7ca81bd7121403703532a14bb4f808. `qualified-source.source.json` separately identifies post-format failed-Clippy tree519b1445d0a3f13f24a01b05dc3ca93556111964, patch SHAa3cf50f02751ba7971146dd6fd1f4c90389c12aa07b23d4e5786c043034979fc. Both capture HEADffb9a2be5fdd41291f6b379cf135629cdae973da, common auth prerequisite ancestry, and unchanged real-index SHA66b08705437ddfdf35ca1468347918fefd163123d66fffe88cb6cf0fa260dac3. Original manifests/private indexes/patches remain in that directory; historical c690 feature/e3d checker proof is not relabeled post-format.

**Delivered:** presentation's external selector explicitly handles impossible absence, then indexes by validated borrowed source range; the UTF8 probe still uses the same stack8/two encode_utf8 calls and validation, with a loud impossible-Err branch. Physical-row paint indexes the validated borrowed run range. Retained live text is bound once only when changed, reused after the original label preparation order; missing live/range/history remains an explicit invariant panic, never a default/skip. No new public contract, allocation, body copy/hash/full scan, unsafe code or source-policy exemption. This is within Main's bounded repair authorization and unchanged valid-input behavior.

The test cold control changes `cold_selection >= hidden_blocks +1` to equivalent `> hidden_blocks` for the unchanged0/256/4096 workloads. Dense benchmark cadence and supplemental wrapping/index fixtures use equivalent usize is_multiple_of predicates (benchmark4, wrapping2, row-index4). No cases, timing method, values or target assertions are weakened. Scout statically inspected affected bench/layout modules and found no other certain next-wave candidates; this is not a Clippy PASS.

**New frozen boundary:** route.md/clippy-repair.source.json own the new seven-file patch identity after the exact parent post-format tree. All historical artifacts, auth fixture, harnesses and frozen snapshots are unchanged; no feature patch absorbs the auth delta. Two edit-tool saves warned of client normalization; returned source windows were read and the tool behavior reported to QA. No worker formatter or Cargo command was invoked. The exported delta records actual saved bytes.

**Subsequent parent evidence:** Main reports current-source `cargo clippy --workspace --all-targets -- -D warnings` GREEN and independent bounded-repair review correct .99. Main formatted again before the next test run; that tested source is tree `830b4673e7e3d49ed3f1e272af420f8fba75059e`, not frozen after-F10 tree0aa5eec7. Full workspace tests then fail F11 below. Retained bg116 targeted GREEN/F9 proof keeps its original meaning; current full C6/native/timing/PR obligations remain unfulfilled. No worker gate was run.

## F11 reference wrapping defect — requester-approved narrow exception

**Observed pre-repair:** Main's `c6la-repair1/02-test.log` under the same qualification directory records UI882 PASS/3 FAIL/1 ignored, exit101. The three failures are generated_affixes_preserve_original_single_span_graphemes_and_controls, deterministic_mixed_grapheme_sequences_match_real_paragraph and exact_paragraph_cells_for_whitespace_unicode_controls_and_windows. All report ratatui-core0.1.0 Buffer250 rejecting x5 for x3/width2; y/height are4/3,6/5 and3/2. No backtrace is present. These ground-truth RED receipts stay pinned to830b4673; the worker never reran them. Subsequent applied source is separately identified below.

**Source attribution:** both differential helpers render real Paragraph before the candidate. Pinned widgets0.3.0 reflow107–164 can emit the existing line while retaining a full-width pending word, then append another positive-width glyph to that word. Its Paragraph460–471 and our layout painter both write that glyph at the out-of-range start. Width2 `a界a` manually derives rows `a`, `界a`, with the final `a` starting at2; the existing whitespace fixture derives the exact third failure's geometry. The two other first-case derivations are source inferences, not captured runtime stack frames. Independent source scouts AttributeFidelityPanic and InspectRowPaintingCause agree; no run-coalescing, suffix-probe or affix-source change is justified by this evidence.

**Verified upstream:** [issue2679](https://github.com/ratatui/ratatui/issues/2679) reports the exact `a🌞b` width2 panic. [PR2680](https://github.com/ratatui/ratatui/pull/2680) is OPEN/unmerged at exact head `6e25eca2ecda0d2dcec13cfb6f754208a315b5b7`; its [immutable diff](https://github.com/ratatui/ratatui/commit/6e25eca2ecda0d2dcec13cfb6f754208a315b5b7.diff) changes the root reflow, not the buffer bound. No released fix was established. Its incoming-inclusive overflow rule would also change our positive `界界` width3 control from one row of width4 to two rows. Therefore blindly upgrading/backporting it would violate the current approved fidelity obligation.

**Approved/applied decision:** design.md F11 records the requester-selected narrow parity exception. wrap_lines uses the true compile-time carry repair; the original false mode is only a test classifier, with real Paragraph row-count cross-check and actual positive-start inspection. New repair uses cached widths and no extra source pass; painters/dependencies remain unchanged. Safe comparisons stay exact; unsafe cases still execute source/order/effective-style/alignment/UTF8-width/bounds and real wrapped-Paragraph per-row cell/window checks, plus independently specified literal wrapping goldens. U+200B retains original whitespace-discard policy; non-whitespace zero-width is conserved. Positive width3界界 one-row/width4 assertions are unchanged.

### F11/F12 current delivered boundary and independent source review

**Frozen source:** route.md/carry-gutter-repair.source.json own exact five-file14945de1 delta after830b4673, reconstructed tree43db6bd5. Original worktree contains it already. No previous feature/source-policy/sensitivity artifact was regenerated. Actual saved bytes include tool-reported benchmark import/ensure normalization; no worker formatter ran and the tool behavior was reported.

**F11 authored tests:** `legacy_classifier_detects_only_actual_unsafe_positive_starts`, `carried_prefix_repair_has_literal_rows_cells_styles_and_windows`, `generated_affix_carry_repair_has_literal_rows_cells_and_styles`. Keep the original three failing workloads, all widths/alignment/span/control/4096-grapheme/deep-window matrices and positive overhang. Literal a界a/a🌞b/U+3000/combining/spacing/affix rows/cells are normative, not captured from repaired output. Per-row Paragraph is a cell/style oracle only, not an independent row-boundary oracle.

**F12 delivered:** retained::render computes contentRect once and uses it for history/live/activity synchronization, paint and viewport rows; scrollbar uses a separate1-column Rect. New root tests `c2_c5_stable_gutter_preserves_complete_auth_url_at_bottom_and_page_up` and `c1_c5_gutter_overflow_threshold_keeps_committed_content_width_and_work_stable`, plus retained `zero_and_one_column_use_full_content_and_one_to_two_keeps_preparation`. Actual140×40 opaque URL lengths278/280/281 require every byte/style at bottom/PageUp/PageDown/follow transitions; warm transitions require zero derivation and stable metrics. Height-only overflow crossings require zero derivation; real append permits exactly messages1/live0/hash0/parse0/wrap1/Myers0. Tiny offset rectangles cover0→1→2→1→0 and stable source backing/preparation at1↔2. Giant test/bench retain80,000×159-byte newline-free segments, all markers/multiMB/>u16 checks and unchanged outer160×48/timing. Captured TSV/snapshots/provenance remain unchanged after scoped fixture census.

**Independent pre-qualification source reviews:** ReviewCarryParitySource (wrap/presentation) and ReviewStableGutterSource (retained/root tests/bench), each correct/no findings, confidence.93. Both compare actual saved source with prior830b4673 and approved design; neither executes checks. Carry reviewer confirms original-mode classification, safe comparison retention, scoped replacement coverage, no hot work and wrapped per-row oracle preserving valid overhang inside affected inputs. Gutter reviewer confirms consistent geometry, full URL/style controls and giant row arithmetic. Tool diff anomalies falsely depicting extant worktree files as deletions were reported; reviews use absolute saved source plus baseline git-show, not those false deletions. These source reviews precede qualification and do not substitute post-check repair review.

**Required actual sensitivity — Main:** F11 change ONLY production entry `wrap_lines_with_carry::<true>` to false; keep original-mode classification and all assertions enabled. Literal row and safe-start fences must fail for actual legacy owned output, then restore/GREEN. F12 restore old production content.width=area.width AND scrollbar-on-area (not the separate gutter) at the140-column consumer; complete URL/gutter assertions must detect the original overlay, then restore/GREEN. Neither source reasoning nor the old reference-first panic is presented as an exercised new-fence RED.

**Checks/remaining proof:** no worker Cargo/test/build/Clippy/fmt/bench/native or mutation execution. Main must format/pin current source, run affected wrap/presentation/retained/conversation-performance tests, actual sensitivity, uninstrumented theme/source-policy gates, full workspace/build, benchmark and remaining consumer/platform smoke, then final independent repair review and issue PR. Native Kiro checks are separately requester-paused, not waived. Existing accepted Windows baseline limit remains exact; new failures are not covered. Post-smoke permanent docs/changelog remain Main-owned.

## Current qualified-source review and delivery adjudication (2026-10-21)

The [current publication manifest](current-width-e3f2a7c7/publication-manifest.json)
owns current source/receipt bindings, raw observations and archived consumer
evidence. Route.md records the505-input descriptor and separate canonical/private
binary pins. All historical sections above retain their original source,
confidence, receipt scope and then-pending checkpoints; the following disposition
does not rewrite them as freshly executed PASS.

### Current independent source review

The current conformance review reports **correct, no findings, confidence0.96**
for the34introduced source paths versus incoming93bc, bound to the505-row
descriptor `e3f2a7c73e2de8dfb1895be825305a572dd203a94743633490f8de8f91165f94`.
It covers the assembled ownership/interfaces, approved F11 exception, F12 sole
gutter owner, released physical-width interactions, incoming output/error
selection and retained F6/F8/F9/F10 contracts. This is independent **source review**,
not consumer/native/staging/CI approval. It is distinct from the earlier six-path
pre-format review at0.96, the disjoint F11/F12 pre-qualification reviews at0.93,
and the historical F9/F10 reviews at0.99; none is relabeled as another receipt.

### Exact current qualification accounting

| Evidence | Executed accounting and scope |
|---|---|
| Command total | 35:29 exit0 successes +3 intended compiled assertion REDs at exit101 +3 expected benchmark-CLI negatives at exit1; 0 unexpected failures |
| Workspace | 2953 passed /0 failed /13 ignored; ignored coverage did not execute |
| Current ordinary carry/gutter fences | wrap11 PASS and root performance22 PASS, with current workspace standing; these are not fresh historical mutation reruns |
| Source policies | Unchanged raw300000-byte renderer budget and five prohibitions; exact recursive22-widget inventory, plus highlight for23entries in the separate color checker. No automatic future inventory-cap increase |
| Benchmarks | 79 ordered case executions /2370 raw samples:3 completed-write executions /90 samples and76 long-history executions /2280 samples. Repeated160×48 transitions remain ordered executions, not deduplicated workload names |

The benchmark layer is TestBackend, not a native terminal, baseline-speedup or RSS
claim. Actual current command/restore receipts and source-policy results are in
the packet; no worker check is supplied by this documentation update.

### Retained F11/F12 applicability versus fresh released-width sensitivity

- **F11 retained applicability:** the historical compiled RED/restored-GREEN pair
  still discriminates the same true-to-false production carry entry and first
  literal `a界a` row assertion, with unchanged a1/界2 physical geometry. Its RED
  reaches that first row assertion, **not later windows**. Current ordinary wrap
  and workspace fences pass; the old receipt retains its historical source pin.
- **F12 retained applicability:** retained-renderer geometry, both historical
  mutation lines and the278/280/281-byte opaque-URL byte/style and warm-scroll
  controls remain applicable. Its historical RED reaches the **first URL-byte
  assertion**, not later PageUp/windows or native-terminal assertions. Current
  root-performance/workspace fences pass; the old pair is not renamed current.
- **Fresh historical F11/F12 mutation reruns:0.** Their retained receipts establish
  only the unchanged discriminators above. They do not prove the new physical-width
  layer.
- **Three genuine current RED/restored-GREEN pairs:** production cached wrapper
  width, production painter physical advance, and a **test-only unsafe-reference
  classifier**. The classifier pair is not production rendering proof. The packet
  preserves the initial false matcher classification and its reconciliation
  against Rust `Debug` spelling, rather than replacing that raw observation.
- **Auth prerequisite6c:** `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69` remains a
  separately owned test-only fixture repair. Its six deterministic historical
  sensitivity cases ran in copied source. They are not a C6 production-auth
  repair, current full-tree proof or current native-auth acceptance.

The narrow unsafe-positive-start exception and independent literal/conservation
coverage remain; valid Paragraph parity, including width3`界界` positive overhang,
is preserved. No new C6 vendor/dependency fix or generic harness hardening is
implied by these receipts.

### Actual consumer adjudication and honest limits

Route.md's three-outcome table preserves the original current-C6 burst **FAIL**,
the qualified pre-C6 Zj original-burst **ASSERTFAIL at114.94s** (not timeout), and
the reviewed current cadence **PASS for both independent startup color modes**.
The latter records per mode67/67 checks,49 capture pairs and1200selected intended
page actions:30ordinary selected writes /4800bytes requested and written, within
97total writes /6085bytes requested and written. These are PTY submission/write
counts, **not exact App decoded/dispatched/consumed key counts**.

All three400-action calls remain, each using10×40 with ordinary0.25s draining.
Chunk size and cumulative draining/pacing changed together, so the pre-C6
comparison establishes only an inherited observable burst limitation—not an
upstream root cause, determinism, global C6 exoneration or product repair. Both
burst failures remain failures without a waiver. The third call was unreached
in those failures; applying the approved cadence consistently to it is not proof
it previously failed. Actual cadence receipts reach genuine content-backed
failed-tool chronology c21a and c22; a general counterfactual harness-finish
concern does not invalidate those existing receipts or require a repair.

The PTY consumer does not measure internal work/cache counts, RSS, allocations,
latency or baseline speedup; root TestBackend tests/benchmarks retain their own
layer. Crew lifecycle/main routing is not native child-focus entry/back.
Under the [supported-controls user approval](design.md#supported-controls-user-approval),
existing state/render/root qualification owns main↔child focus/back, and the
original `truecolor`/`none` pair uses separate native launches. The all-four-mode
state/render matrix and deterministic same-process theme/mode cache-invalidation
tests/benchmark retain their separate current qualification. Neither independent
PTY nor native startups prove same-process mode switching. These replaced native
instruments are not remaining blockers and add no UI controls.

**Required supported native outcomes — PASS, by Main's technical combined-layer
judgment**, owned by [design.md](design.md#main-technical-native-gate-judgment) and
the [native evidence record](current-width-e3f2a7c7/native-supported-controls-evidence.json).
The [native archive](current-width-e3f2a7c7/native-supported-controls-evidence.tar.gz)
retains the actual owned Ghostty/Xvfb pair and supporting captures. Existing
qualified exact root/layout cells, styles, normative rows and conservation
establish exact fidelity; source-bound native exports/scene receipts correlate
the native execution; independent actual full-window PNG reviews establish
positive native appearance/usability with no observed defect. Every named
appearance subcheck passes per mode. The reviewers' **26 literal claims per mode
remain NOT_PROVEN by visuals alone**; exports are not style-cell captures, and
no new pixel oracle or waiver is introduced.

Counts, hashes, natural App exits, completed cleanup and unchanged-source
receipts belong to the native evidence record. They are distinct from the
earlier PTY wrapper exit0, which was not separately observed graceful App exit0.
The raw capture handoff and mode receipts retain their historical capture-time
pending-visual statuses unchanged; later reviews and Main's judgment discharge
the current gate rather than rewrite those raw observations.

The original deterministic/root and TestBackend benchmark workloads still own
50,000-history, giant-message/line and >u16 proof. Earlier blanket native50000/
giant pending language was an overbroad nonclaim, not a controlling requirement
for a new native fixture or an obligation being waived. No native50,000-history,
native work-counter/performance, or real Kiro/account/model-turn result is claimed.
Retained workbench/cross-feature limits keep their existing bounded proof and
remaining Main-owned integration requirements; user-native Kiro turns remain
separately paused, not waived.

Future Tqr/NK/Caar and combined-feature/workbench qualification,
final integrated-tree qualification and new C6 PR CI/platform evidence remain
Main-owned and unqualified; the scoped supported-native PASS is not final delivery.
The existing PR183 Windows exception stays exactly
23passed /3 initial-readiness failures /2670cancelled /13skipped; cancelled
coverage is not green and no new failure is waived. Late raw-index drift leaves
full4158logical-index identity and its cause unproved despite all505source inputs
remaining exact; Main's not-product-blocker adjudication does not convert it to
tested staging. Main owns publication; PR187 remains untouched/unmerged.

### Native publication wording clarification

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| PUB-NATIVE-1 | The owning record's “does not remove an independently required native-volume check” could imply an unmet native-volume obligation; name the retained deterministic-volume check instead. | Late aggregated static review | Verified | Direct reading of `native-supported-controls-evidence.json`'s `volume_scope_authority.correction_not_waiver`; original design133–138/142/271 already distinguishes deterministic/benchmark volume proof from native usability. | Accept | Replace only “native-volume” with “deterministic-volume”; refresh the dependent metadata pins. | non-blocking — resolved docs-only ambiguity, not a changed obligation, proof instrument, gate result or risk acceptance. Existing native/source evidence and the prior substantive static review remain applicable; no runtime, mutation or source qualification is invalidated. |

### Main's adopted ordinary-scroll instrument disposition (2026-10-21)

The later owning [scroll-obligation adjudication](current-width-e3f2a7c7/scroll-obligation-adjudication.json)
records the complete original DG-approved quotes and frozen-source pins:
`design.md:142` (“scroll oldest/middle/newest and back to follow”), C2 at
`design.md:117` (“Preserve first/middle/last windows, follow/rows-back/overlarge-back”),
and original `route.md:57` (“oldest/middle/newest/back-to-follow scrolling”).
The approved positive obligation is observed ordinary browsing/return-to-follow,
rows-back and overlarge-back with App-owned lifetime and terminal evidence;
those sources do not mandate400 keys in one write or an input-throughput target.

**Actor: Main; decision: adopt the current cadence instrument.** Under CONTRACT
Approval semantics/verification complexity, Main explicitly adopts10×40 ordinary
sends for all three existing400-action calls. The same1200 intended actions per
mode, all other cases/actions/assertions, geometry/timeouts and terminal evidence
are retained. This is a technical verification decision, neither a user quote
nor a user-risk waiver, and not an inference from preparation/runtime GO.
The current full-cadence PASS covers the approved ordinary-scroll obligation;
that instrument decision is resolved, not left pending or blocked by the original
burst check alone.

The three outcomes retain their original attribution: current-C6 original-burst
**FAIL**, qualified pre-C6 Zj original-burst **ASSERTFAIL**, and current
adopted-cadence **PASS**. The inherited observable burst limitation remains
preserved—not PASS/N/A, a product fix or global C6 exoneration—and submission
counts remain distinct from App consumption. A separately mandated burst or
throughput obligation, if discovered, blocks merge until repair or explicit
user-risk approval; none is waived by this decision.

This ordinary-scroll addition does not rewrite earlier review records or
source/proof pins. The later [supported-controls user approval](design.md#supported-controls-user-approval)
replaces only native same-process mode switching and child-focus/back with the
separately attributed instruments above. **Required supported native outcomes
are PASS** under [Main's technical judgment](design.md#main-technical-native-gate-judgment);
exact deterministic, correlated native-export and positive actual-PNG appearance
layers remain distinct, as do raw historical pending-visual statuses and the
literal visual-only NOT_PROVEN claims. Current-main/held93 integration, retained
workbench/cross-feature, final integrated-tree and new-PR CI/platform limits
remain Main-owned. Neither instrument decision nor this scoped native gate is
blanket C6/merge acceptance, and PR187 remains untouched/unmerged.

## Current Main038 qualification and runtime chronology (2026-10-22)

The [new integration manifest](current-main038-b5d45fe2/publication-manifest.json)
owns exact current506/source/canonical/private-binary pins and the portable
capsule. The earlier 505-source reviews and native/benchmark/source-sensitivity
receipts above stay historical; none is relabeled a new execution.

Current local qualification passed all17 assigned commands, with workspace
3113/0/13 and 217 required current cases. Independent merge preflight found no
static defect (confidence0.94); the bounded consumer binding review found none
(confidence0.97). These are scoped source/preparation reviews, not evidence that
an interpreter supports every runtime API, product execution or CI approval.

| Stage | Actual disposition |
|---|---|
| First Main-authorized invocation, Python3.13.9/harness88 | Exit1 at the unchanged pidfd capability guard, before App dispatch. No consumer output, captures, result or cleanup receipt was created. At least one required Python pidfd API was absent; the guard did not distinguish which. |
| Main's runtime-only correction and bounded host-self probe | Restore the actual original C6 `/usr/bin/python` plus existing `smoke-dfb` pyte0.8.2/wcwidth0.9.2 environment. Self pidfd/open/signal-zero and imports passed without launching App. Current Python3.14.8 was observed; the historical interpreter version was not queried and remains unclaimed. |
| Separate fresh Main GO, exact original-C6 env shape | Actual current App `truecolor` PASS: all original67 predicates and49 capture pairs, c21a genuine failed-tool chronology and c22 final turn reached, deny selected, no unexpected callbacks, independent cleanup COMPLETE. No automatic retry, extra mode or source/guard repair. |

The correction changes only the wrapper's fresh output and recorded wcwidth
version binding; the consuming actor/scenario/peer/0ce adapter/proof/failure-input
bytes and every guard implementation remain unchanged. The actual launch drops
the borrowed outer isolation flags to match the original C6 invocation shape.
The failed attempt, consumed authorizations, capability probe and successful
App run are all separately preserved in the
[consumer record](current-main038-b5d45fe2/c6la-main038-consumer-result.json).
The second raw GO retains a stale `NOT AUTHORIZATION` preparation note; its
`go=true`, Main actor/source fields and explicit Main instruction authorized the
run. Preserve that counterevidence rather than silently correcting consumed bytes.

All97 writes submitted6085 requested/written bytes. The selected three C6 legs
are30 writes/1200 intended keys/4800 bytes; inherited and interior controls are
separate. These remain PTY submissions, not App-consumed counts. Wrapper exit0
and cleanup are not an independently recorded graceful App exit0.

Main retains the original native192-PNG truecolor/none component with its
original505/e3f/3ef binding. Fresh current217 ordinary controls and actual
pending/resize/deny corroborate the bounded changed-approval applicability;
they do not create current native frames or promote the26 visual-only literal
claims. Original78 applicable benchmark observations remain source-bound and
pending-preview timing remains historical. Original burst FAIL/ASSERTFAIL and
Main's ordinary-scroll instrument decision remain distinct and unchanged.

README's post-smoke bytes are a documented publication overlay; the qualified
506-input snapshot is not a later staged/committed tree. Main owns held038
completion/new C6 PR/CI/platform and remaining delivery decisions. PR187 stays
untouched/unmerged; no new waiver or whole-issue acceptance is recorded.

## Restored layout-owner findings and current evidence (2026-10-22)

The [restored-owner manifest](current-restored-layout-owner-7d88498e/publication-manifest.json)
owns current source `7d88498e…`, its 506-input manifest and default App
`e31ef685…`, qualification/runtime receipts and exact review bindings. The
preceding b5/e3 source reviews, one-mode consumer, native/benchmark carry-forward,
failed prelaunch and authorization chronology remain historical as recorded;
none is relabeled fresh D1 evidence.

### D1/D2/D3 dispositions

The original [c084 conformance report](conformance-final-c0846f99.md) remains
**FAIL**, and its [isolated reconstruction](reconstruction-final-c0846f99.md)
remains the pre-restoration source record. Main selected the restoration in the
[repair plan](local://c6la-final-layout-owner-repair-plan.md); the following
dispositions do not rewrite either report.

| Finding | Applied disposition | Remaining boundary |
|---|---|---|
| D1 — physical-layout ownership | Restore the approved boundary: layout owns physical preparation, aggregate/index mutation, seek, traversal and painting; chat retains presentation/source resolution and transcript/live policy. This is a production responsibility repair, not a clerical correction or waiver. | Fresh isolated reconstruction and independent comparison PASS for working production: approved layout owner restored, no remaining ownership/interface discrepancy. |
| D2 — Core ledger | Existing design row108 now names approved `HarvestedReceipts`, ordered receipts/`into_parts` and conservative earliest-removal metadata; forbidden renderer dependencies stay unchanged. | Documentation-only reconciliation confirmed MATCH by the fresh comparison; no residual ledger gap or new Core permission. |
| D3 — `traits.rs` ledger | Existing design row109 names approved read-only history/live and authoritative borrowed/derived/Cow/direct-source/indexed-diff projections, forbidding physical-layout ownership and event routing. | Documentation-only reconciliation confirmed MATCH by the fresh comparison; no uncovered module, new interface or dependency-direction claim. |

### Fresh isolated source conformance and frozen report quartet

`ReconstructC6RestoredFinal` reconstructed production without reading design,
plans, checkpoints or prior reviews. Its frozen report preceded the separate
fresh-context `CompareC6RestoredFinal` comparison, which read that reconstruction
and the approved design, then verified load-bearing claims against actual
working production. Both reports explicitly include the three uncommitted
restoration files; the c084 HEAD tree was not substituted. Main connects the
restored target to `7d88498e…`. Neither report independently attests the 506-input
manifest or executed qualification, benchmark, native or CI outcomes.

The four Main-owned reports are immutable publication evidence:

| Source target / report | Bytes | SHA256 | Disposition |
|---|---:|---|---|
| [Original c084 reconstruction](reconstruction-final-c0846f99.md) | 32,469 | `4e9affba97fea12e9069a7d8aecd1ae5c21fd0f7fc5e587922c88d4113e55997` | Historical pre-restoration reconstruction, not a verdict. |
| [Original c084 comparison](conformance-final-c0846f99.md) | 19,200 | `51629a36040a118ee7cb6637a969c906b44fb32d83418e2e2bf35a9a759b3fd3` | **FAIL remains FAIL** for that source/design target. |
| [Restored working-production reconstruction](reconstruction-restored-7d88498e.md) | 44,955 | `97c8f8c7baf33453eaeb0a38eec052c2227925e276682e956ea89f0cc4881fad` | Fresh blind reconstruction, not qualification or conformance by itself. |
| [Restored working-production comparison](conformance-restored-7d88498e.md) | 22,607 | `08d9f1bfbea3e44b5386728479ba80bc69cc3049e0b3766083fe64446fcdde4e` | **PASS**, source-level design conformance only; a separate target, not reversal of the old FAIL. |

Approved `design.md` SHA256
`219f959520dd99b75d14a6b93ee2850e42bab80c62c3f1cd99118ac3e993a8e9`
is retained byte-exact as reviewed input. Its prior pending-conformance snapshot
at lines402–404 is historical and superseded for the current source gate by
the fresh reports, not a reason to rewrite the design, its ledger or quotes.

The reconstructed 16-file map below is grouped by the comparison's nine approved
rows; `UI/` means `crates/cyril-ui/src/`. The frozen reconstruction's
lines228–245 supply its production map, and the fresh comparison's lines34–46
supply per-row approved-design evidence and verdicts.

| Approved row | Changed production paths assigned exactly once | Reconstructed responsibility / final verdict |
|---|---|---|
| L1 | `UI/message_history.rs`; `UI/lib.rs` | Concrete committed-history owner and export wiring — MATCH. |
| L2 | `UI/state.rs`; `UI/subagent_ui.rs` | Main/child notifications, live revisions, lifecycle and history policy — MATCH. |
| L3 | `UI/widgets/chat_layout/mod.rs`; `UI/widgets/chat_layout/index.rs`; `UI/widgets/chat_layout/wrap.rs`; `UI/widgets/mod.rs` | Physical preparation, block/index ownership, seek/traversal/painting and private registration — MATCH. |
| L4 | `UI/widgets/chat.rs`; `UI/widgets/chat/presentation.rs`; `UI/widgets/chat/retained.rs` | Chat facade, semantic presentation/source resolution, transcript invalidation and window/gutter policy — MATCH. |
| L5 | `UI/widgets/markdown.rs` | Markdown derivation and immutable shared cache results — MATCH. |
| L6 | `UI/render.rs` | Retained root interface, frame geometry and panic invalidation — MATCH. |
| L7 | `crates/cyril/src/app.rs` | Application render-state lifetime and draw wiring — MATCH. |
| L8 | `crates/cyril-core/src/types/steer_receipt.rs` | Receipt-transform metadata without renderer ownership — MATCH. |
| L9 | `UI/traits.rs` | Read-only render contract and authoritative tracked-tool projections — MATCH. |

**Final source-conformance result: PASS — 16 changed production files, nine
approved rows, 9 MATCH, 0 MISMATCH, 0 UNCOVERED, 0 MISSING and no unresolved
candidates.** The comparison separately disposes private opaque `Layout<P>`
payload ownership, the history/traits type dependency, current-reference versus
adaptation provenance and the retained wrapping/storage/test-coupling tradeoffs
(lines98–159). No new responsibility/interface approval or residual discrepancy
is inferred. This closes D1/D2/D3 for source conformance, not CI or whole-issue
acceptance.

### Current source-bound evidence and limits

All 17 assigned local qualification commands PASS (workspace 3118/0/13).
Fresh timing covers the complete 79 ordered executions / 2370 samples, including
pending preview: three completed-Write cases plus 76 long-history cases. This
supersedes only the historical retained78/pending-timing restriction; local
qualification and TestBackend measurements do not imply CI or native performance.

The [actual consumer aggregate](current-restored-layout-owner-7d88498e/c6la-restored-layout-owner-consumer-result.json)
records both authorized modes PASS on their first attempts, each with 67/67
checks, 49 capture pairs and COMPLETE independent owned cleanup. Pending resize
and deny-only response passed without a grant. Actual PTY submissions are not
App-consumed-key/work or retained-row counts, and wrapper exit0 is not separate
graceful-App-exit0 evidence.

Main's fresh [supported-native judgment](design.md#current-supported-native-judgment-and-limits)
is **PASS with explicit limits**, based on current captures, both complete
independent visual reviews and separately attributed exact/content/action proof.
All 192 PNGs / 80 obligations were reviewed, not all80 unqualified visual PASS.
The manifest preserves both raw reviews, the baseline/no-op plus four actual
resize changes, toast occlusion and omitted initial unattached marks in native
exports. Capture-time pending-visual statuses stay immutable; Main's later
qualified disposition is separate.

Original burst FAIL/pre-C6 ASSERTFAIL remain unchanged; ordinary 10×40 cadence
adoption is not a throughput repair, user waiver or global exoneration. Earlier
0.94/0.97 source/preparation reviews are not restored-source final conformance.
Restored-head CI/platform, applicable all-feature/release/doc gates and final
combined acceptance remain **PENDING**. Main owns final Git/publication/index/
commit/tracker work and existing draft **PR197**, not a new PR187. Five-doc
publication postimages are outside compiled506 and do not establish final-tree
identity. Remaining Structural, retained workbench/cross-feature and separately
paused user-native Kiro obligations remain. The fresh source-conformance PASS
above grants no merge readiness, issue closure or global PASS.


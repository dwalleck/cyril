# cyril-c6la route

Date: 2026-10-19. Source state: assigned worktree based on `8e5c325e`; no production edits or verification runs performed by this worker. Full `rivets show cyril-c6la cyril-zj0m cyril-x5xi --json` records were read, including empty notes/design/resources/relationship fields.

## Requested behavior

`cyril-c6la`: Scale conversation Markdown rendering with the visible viewport. The issue requires a production-shape long-history benchmark, steady-state rendering bounded by the visible window plus documented overscan, and correct scrolling. Parent additionally requires preservation of streaming, width/theme invalidation, and no avoidable deep clone/all-history work. Deliver this issue in its own reviewed PR, after the two cache issues below; do not close or publish it before parent integration/verification/review.

## Route tests

| Test | Verdict and evidence |
|---|---|
| T1 Empirical premise | **No.** The consequential behavior is source-answerable. `widgets/chat.rs:19-77,108-148` builds every selected-history line, measures the whole wrapped Paragraph, then wraps/skips its prefix again. Locked Ratatui 0.30.0 / widgets 0.3.0 source confirms `Paragraph::line_count` exhausts `WordWrapper` and wrapped drawing scans the scroll prefix (`paragraph.rs:331-365,418-445`); reflow is private (`lib.rs:131`). No uncertain agent/backend behavior determines this design. Benchmark measurements qualify implementation, not an unresolved external premise. |
| T2 Structural module shape | **Yes.** There is no persistent renderer owner: `render::draw` and `chat::render` are stateless, called by `App::draw_frame` (`render.rs:8-16,153-182`; `app.rs:946-974`). A concrete long-lived UI rendering value, a tracked-history read interface, and private row-layout ownership are required. `UiState` and shared `SubagentStream` own mutations; `TuiState` must remain read-only. |
| T3 Production-scale risk | **Yes.** Retaining geometry/output can trade latency for history-sized memory; stale in-place edits or incorrect physical-row indexing can hide tools/receipts, break scroll, or mix streams. Rebuilding on every live delta, retaining cell grids, or merely caching Paragraphs would leave material costs. Scope-specific deterministic work checks plus production-shaped benchmark/smoke are required. |
| T4 Explicit behavior | **Yes.** Preserve existing cells/styles/chronology and row-back-from-bottom semantics, including follow mode, overlarge scroll-back, three-row floor, input/crew/voice geometry, focused-subagent selection, and scrollbar overlay width. Main: committed history plus live Markdown/thought/activity; child: header plus committed/live agent text, always bottom-follow. Workflow streams remain storage-only. Correct the existing `u16` scroll ceiling by selecting indexed physical rows before drawing. No history truncation or new accepted input limit is authorized. |

**Selected route: Structural.** No scope interrogation is required; placement and performance design require approval before dependent production changes.

## Source-backed invalidation findings

- Main currently has only a global `messages_version`; tool updates do not increment it (`state.rs:574-579`). A parallel Local repair by MissingToolResults will correct this, but a global counter alone still cannot locate edits without scanning.
- There are 17 logical main-history mutation sites and six shared child/workflow sites. Of those, 22 can change displayed content/order/geometry. Main sites: `state.rs:551-558,574-579,1313-1375,1455-1700,1740-1744,1862-1865,3545-3549`. Shared stream sites: `subagent_ui.rs:77-152`.
- Receipts can alter an arbitrary earlier steer during a live stream while visible live text stays unchanged (`state.rs:1507-1585`). Prefix pruning makes Vec position unsuitable as lifetime identity. Child/workflow adoption can prepend history (`subagent_ui.rs:71-88`; `workflow_ui.rs:61-73`).
- Markdown cache hits hash the complete source and deep-clone all output lines (`widgets/markdown.rs:19-94`). Its 256-entry global cache cannot supply a history layout identity.

## Artifact ownership and gates

- `route.md`: change-workflow (this artifact).
- `design.md`: falsifiable-design; **DG-approved by parent Main, 2026-10-19**, including strict indexed visible-row behavior and exact pinned Ratatui fidelity. Its Approval section owns the delegation. zj0m's explicit first-Diff behavior decision is now approved in that issue's design, so this dependency no longer blocks implementation. Concrete interfaces, row-layout tradeoff, workloads and consumer checks remain governed here.
- `plan.md`: budgeted-plan, only after design approval. Each issue remains an independently verifiable PR; detailed atomic slices consume the approved design.
- `spec.md`: not required; requested outcomes and compatibility are explicit above.
- `evidence.md` / probe: not required; no unresolved factual premise. Source facts and future run receipts belong to their existing design/completion records, not a second oracle.

Sequence: approval → budgeted-plan → checkpointed-build → parent-run regression/benchmark/smoke/repository gates → independent code review → issue-specific PR. Terminal criterion is Structural completion in the Gilfoyle contract, including all current valid proof and no blocking finding. **Routing/design preparation does not complete the issue.**

No requester feature budget or applicable hard line cap was supplied. None is invented. Tests, benchmark fixtures, and production implementation remain separately identified by purpose, not a universal line target.

## Frozen implementation checkpoint

Reviewed1v25 `a5d2c4b5` is integrated by authorized cherry-pick as `1ee1d92f`; reviewed fiv1 `7eefe275` follows as `75aaa6f1` (PR184, based on PR183). All assigned source owners have frozen their edits: concrete revision-bearing history and caller migration; stable tool positions/live revisions; shared Markdown; borrowed tool text; retained root/App state; exact indexed source-backed chat/painter; fidelity/lifecycle/workload tests and both benchmark modes. Main approved necessary crossing-grapheme derivation; the requester-approved fully recursive22 widget fence and exact shared existing1.12.0 dependency edge are implemented. This is authored source, not executed acceptance.

`wrap.rs` provides the agreed borrowed SourceSpan/SourceLine → compact WrappedRow/RowRun interface. It ports pinned widgets 0.3.0 trim=false wrapping with exact upstream MIT text, Florian Dehau / Ratatui Developers copyright notices, and source/LICENSE links pinned to published VCS revision `0a2a7c0363a4806b0cf05c1915bf7cdd438f756c`. Temporary source-subslice addresses determine byte offsets safely, including filtered control bytes; no pointers survive preparation. Input-span identity constrains run coalescing to preserve Unicode segmentation. Seven original authored tests compare real Paragraph row counts and exact Buffer cells/styles across whitespace, controls, Unicode, widths/alignment/style boundaries, deterministic mixed sequences and >65,535-row deep selection. `tests::paint_window` now calls the actual production painter; real Paragraph remains the independent oracle.

`index.rs` provides the agreed concrete RowHeights API: logarithmic push/set/locate, constant-time total, zero-height support, retained-relative indices, prefix tombstones and amortized linear metadata-only compaction. Nine authored tests cover known windows, power-of-two append boundaries, update bounds, prune/compaction, all-discard reuse, mixed operations, usize row counts and logarithmic seek steps. These are authored coverage, not executed results.

**Worker-run checks: none.** No worker build/test/lint/formatter/benchmark/smoke was invoked. Source assembly is frozen; compiler/fidelity/lifecycle/workload proof, production-shaped benchmark, native smoke, post-smoke permanent docs/changelog, independent review and the c6la PR remain required. `review-decisions.md` records F1 production-scan preservation, F2 raw-output fixtures, F3 complete recursive source coverage and F4 borrowed/stack boundary detection, all with pending parent proof. The edit tool reported client formatting normalization during the F3 write; the returned source snapshot, not an assumed pre-normalization buffer, is included in the frozen patch.

**PENDING — parent Main:** c6la assembled compiler/fidelity/lifecycle/workload/source gates and own pre-change RED/post-change GREEN; benchmark/native smoke; post-smoke permanent docs/changelog; independent review and issue PR. Both DG questions are resolved, not pending. Reviewed prerequisites are integrated. PR183's existing cyril-u62f Windows readiness failure/cancelled coverage is a requester-accepted qualification limit, NOT platform-green; all other required proof and new failures remain blocking. Intentional owned traits recovery stash `c8e8e1ff89f3dca380e846b833daf9890acdca81` was reapplied successfully; no user stash was altered. x5xi/zj0m saved patches and pinned sensitivity boundaries remain unchanged and never qualify c6la automatically.

## Exact issue boundary

`.cyril-c6la/production.patch` is a **c6la-only source delta**, 461,128 bytes across 32 files, SHA256 `b84fb6d259ad446bba113de49b08a103b3aa8d0f9b1ae06fbcc63e9c7331f8fa`. This supersedes both the pre-F3/F4 448022-byte checkpoint and the pre-F5 461072-byte checkpoint, not the frozen predecessor patches. Its predecessor tree remains `84b4681afd30af1f537a56ca6ec4fe3b6951cbb4`: reviewed fiv1 parent plus unchanged frozen x5xi/zj0m, assembled only in an isolated temporary Git index. That index was removed; patch generation never staged or rewrote the actual worktree. No implementation commit or push was made.

The source delta has eight new files: `message_history.rs`, `conversation_performance_tests.rs`, `tool_text_tests.rs`, `widgets/chat/{presentation,retained}.rs`, and `widgets/chat_layout/{mod,wrap,index}.rs`. Existing changes cover state/child/workflow/traits, root/App/callers, Markdown, source-fence coverage, batch1 integration fixtures and benchmark. The three additional manifest/lock paths add only the approved shared unicode-segmentation direct edge and UI membership, preserving locked package1.12.0. x5xi highlighting/theme-identity and zj0m diff-core/benchmark/approval changes are not duplicated.

Parent runs exact commands/protocols in plan.md, with `env -u CARGO_TARGET_DIR` or a deliberate nonempty target. Native smoke must reuse the App-owned RenderState across actual streamed text/thought/tool/failure updates, oldest/middle/newest/back-to-follow scrolling, live resize/palette/no-color and focused-child/back interactions in an owned fixture. Retain actual screen/source/environment receipts, not TestBackend output as terminal proof. Existing architecture/interface documentation (including `.agents/summary/interfaces.md` and components.md) and shipped changelog/README must be updated after smoke, not replaced by these workflow artifacts.

**Later user replacement:** only the same-process color-mode and native focused-child/back components of this earlier protocol are replaced by the [supported-controls user approval](design.md#supported-controls-user-approval); all other components retain their original meaning. The current [Main technical native-gate judgment](design.md#main-technical-native-gate-judgment) records PASS for the required supported outcomes with exact deterministic, native-export/source-correlation and actual-PNG appearance layers kept separate; it does not rewrite this historical protocol or qualify a later integrated tree.

## Approved source-coverage and boundary corrections

F3 implements one fully recursive22-source path/content inventory, exact filesystem-set/duplicate checks, the same five prohibitions and unchanged aggregate300000-byte cap. The requester, via Main, explicitly approved cap22 here and cap23 only after separate MCP inclusion; no split17 subgroup or nested exemption exists. `.cyril-c6la/gate-review-f3.py` is the unrun parent-owned actual-checker A/B harness with clean and still-forbidden controls. F4 uses source-verified borrowed GraphemeCursor/PreContext and stack UTF-8 probes before deriving only truly crossing bytes; ordinary raw source remains selector-backed. All relevant approvals are recorded in design.md, not inferred from tests.

F5 preserves the broad existing `resolve(` prohibition rather than weakening it for new source accessors. The private PreparedBlock getter and painter callback are now `source_text`, with all fourteen scoped declaration/caller identifiers migrated in three files. Linked-worktree LSP references/rename returned no references; the source-backed AST fallback staged and applied exactly those fourteen sites. This is a naming-only integration correction; parent source-gate/fidelity proof remains unexecuted.

## Accepted Windows baseline qualification limit

Main relayed the requester's explicit baseline exception for the existing cyril-u62f memory-runtime readiness failures on PR183: three initial-ready failures, 23 passes, 2670 cancelled later tests and 13 skipped. Other substantive jobs passed; unchanged memory identities were proven, but the mechanism is unresolved and cancelled tests did not pass. This does not authorize retries, timeout increases, skips, memory-runtime changes, or acceptance of new/differently attributed failures. Keep the [public record](https://github.com/dwalleck/cyril/pull/183#issuecomment-6284441811) and [receipt](local://windows-baseline-exception.md); no obsolete diagnosis-pending or platform-green claim applies.

## Retained pre-RED tests-only checkpoint

Independent source review identified Layout P2 hidden first-Diff reselection, Root P1 invalid receipt fixture IDs and P2 unchanged live-Markdown invalidation; History corroborated root findings with no additional defect. `review-decisions.md` owns F6–F8. At this checkpoint Main authorized only test/instrumentation and valid-protocol fixture corrections until genuine RED, not production repairs; the later explicit approval and fixed-source handoff follow below.

`.cyril-c6la/review-test-only.patch` is 22,116 bytes across exactly three files (traits.rs, conversation_performance_tests.rs, benches/conversation_render.rs), SHA256 `9b7aeba7f91b651247bb18659517bab082e1766a4f7e8e8705b05af19a9dec1a`. Apply AFTER the unchanged production.patch SHA256 `b84fb6d259ad446bba113de49b08a103b3aa8d0f9b1ae06fbcc63e9c7331f8fa`; exact reconstructed baseline tree is `51dd940ecd4e033b9eeb4e967ea2199596a1d378`. Patch generation used/removed only an isolated temporary index; actual staging/worktree were not rewritten. The product baseline is deliberately NOT replaced before RED.

At that checkpoint the delta added actual predicate-visit cfg(test) counting, five real-root regressions and five valid steer-prefix literal corrections, without a runtime source/state/parser/cache repair. Fixture correction was not RED evidence. All hidden-content workloads and every withheld chunk execute positive controls before target work assertions; hidden-Diff scenes are Idle and receipt cases use the fixed-clock helper.

Parent filter: `env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib conversation_performance_tests::review_ -- --nocapture --test-threads=1`. It subsequently executed four intended work failures after C6LA_REVIEW_CONTROLS_EXECUTED plus one passing nested-prefix semantic standing test in artifact605. Exact workloads and context tradeoffs are in plan.md. No worker verification is claimed.

## Current post-review handoff and historical feature boundaries (F6–F10)

F6/F8/F9 historical implementation/proof boundaries remain intact. Parent bg116 supplied targeted five-review GREEN and seven F9 A/B pairs at the retained pre-format source; subsequent actual formatting/Clippy/F10 and F11 RED/approved F11/F12 source repair are separately recorded below. None of those earlier receipts is relabeled as the new full assembly. The assembly owner now receives a complete34-source-path C6 export rather than stacking partial repair patches; exact identities/exclusions follow at the end. Historical artifacts/harnesses/snapshots remain unchanged.

**Original execution worktree already contains feature work, common prerequisite, parent formatting and F10 repair. Do not reapply patches there.** Reconstructed FEATURE-ONLY treedc200a8276d7ed7aafbeced4bd7c4824e20451dc is the retained pre-format F9 checkpoint, not the current full qualification tree; current post-format/F10 identity is below.

- `review-production-fix.patch`: four files, 25203 bytes, SHA256 `f51b2d57d82d92a09b903759715cdb56db04a1537cf84f86c611e88318a71238`; applies only after exact b84+9b7 source tree `4b277b4c9153d625a9ae802b13ecc4eb4da8cab2`.
- `review-f9-gate.patch`: one checker file,7444bytes, SHA256 `5448abe06ff6d2185e79fa4ebc22f5d0991a6bd90b1dafb6c188d530a13ff13b`; applies after F6/F8 tree `49640ed0460fe3bcd887ae5b9b8684b230e7e346`.
- Historical `production-after-f9.patch`: feature-only delta after predecessor84b4681afd30af1f537a56ca6ec4fe3b6951cbb4;34files,505338bytes, SHA256c69099f25f98fa6635abeeb0adc8caa9b2856211333b98eb147a2d3f553e4318. It deliberately excludes common auth, parent final formatting and F10. Earlier production-after-review.patch095 remains the33-file F6/F8 checkpoint.
- Keep original b84 production.patch and9b7 review-test-only.patch for actual RED reconstruction. Isolated artifact index removed; no actual staging/working-tree rewrite, commits or tracker changes.

**Next action — Main:** F11/F12 source repair is complete in the original worktree and both disjoint pre-qualification source reviews report correct/no findings (.93). Use the exact new five-file handoff below; do not reapply it here. Parent owns formatting, actual new-fence RED/restored-GREEN, affected/full/source-policy/build/benchmark/remaining smoke, post-check review and issue PR. Prior Clippy/F10/three-test RED retain830b4673, not the new43db6bd5 source. No dependency/vendor change, worker Cargo/formatter, commit or tracker edit. PR183 alias propagation/actual zj0m stay separately owned; accepted Windows limit is not platform-green.

## Common qualification prerequisite identity

Main reports the standalone common auth-fixture repair `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69` is non-fast-forward merged into this original C6 worktree and the actual zj0m delivery tree. It is test-only19+/18-, uses an immutable interpreter instead of a mutable executable fixture and changes no production auth behavior. Parent bg118 supplies six deterministic sensitivity cases (old-held-writer RED/new GREEN, gate-removed RED/restored GREEN, constructor-key-cached RED/restored GREEN) and independent review. This is a separately owned qualification prerequisite for the known ETXTBSY failure, not feature or full-workspace proof.

The common prerequisite is retained in the actual qualification HEAD captured below; feature-only artifact pins remain historical and unchanged. This worker does not check or overwrite the auth fixture or absorb its delta into feature patches. F9 proof keeps its original checked provenance; subsequent formatting/F10 qualification is separately pending. Full current-checkout C6 and actual zj0m qualification and later PR183 alias propagation remain Main-owned.

## F10 source-ready boundary after first Clippy failure

Parent pre-format tree53386832198e5c430e9a48a5e6567aee53a4be76 and post-format failed-Clippy tree519b1445d0a3f13f24a01b05dc3ca93556111964 are retained separately in the qualifier manifests referenced by review-decisions.md. HEAD at that capture is ffb9a2be5fdd41291f6b379cf135629cdae973da with common prerequisite ancestry; neither is historical feature treedc200a8.

`review-clippy-repair.patch` applies AFTER exact post-format tree519b1445d0a3f13f24a01b05dc3ca93556111964:7files,11028bytes, SHA256 `8ad00e53daa307c6bfbc52b90b5237641d514aef632418d4080dbff3c2759b2d`. Reconstructed after-repair tree `0aa5eec7ce7d476d133cf2eb19f4d4afc51929e6`. Exact metadata/file list is `.cyril-c6la/clippy-repair.source.json`. Original worktree already has the delta; no application/staging needed. Artifact generation used a removed initialized private index; no actual index rewrite.

Scope: presentation.rs, retained.rs, chat_layout/{mod,wrap,index}.rs, conversation_performance_tests.rs and conversation_render.rs. No public API change, silent fallback, warning suppression, new allocation/hash/history scan, harness/snapshot/auth edit or actual-zj0m edit. Parent subsequently reports current-source Clippy GREEN and independent F10 review correct .99; see the distinct formatted/full-test source below. Keep all earlier c690/e3d source-proof artifacts immutable.

## F11 prior qualification failure and approved repair ownership

Prior parent-formatted tested tree: `830b4673e7e3d49ed3f1e272af420f8fba75059e`. Parent workspace/all-target Clippy PASS; full workspace tests exit101 with UI882 PASS/3 FAIL/1 ignored. Exact log: `/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/c6la-repair1/02-test.log`, failures3040–3068. Frozen0aa5/8ad remains the earlier F10 boundary; neither prior receipt nor frozen artifact is relabeled as the new43db6bd5 repair.

Both real-reference-first helpers expose pinned WordWrapper emitting a positive-width glyph at viewport x==width after carrying a full-width word. Minimal source-derived witness is width2 `a界a`; our copied routine inherits it. The log lacks a backtrace, so exact runtime caller attribution is not claimed. Upstream2679/OPEN2680 confirm the reference defect, but the broader proposed fix changes an existing positive overhang assertion.

**Approved/applied disposition:** design.md F11 records the explicit requester exception: repair Cyril's carried-prefix start, retain defined Paragraph parity and positive overhang, use independent normative rows/cells/conservation/style checks only for actual unsafe reference starts. NO vendor/dependency change or padded oracle. F12's stable gutter is applied identically to Tqr's contract. Five reviewed source files are frozen below; post-repair checks remain Main-owned, not claimed PASS.

## Frozen F11/F12 source-reviewed repair handoff

- `.cyril-c6la/review-carry-gutter-repair.patch`: **5 files,46711 bytes**, SHA256 `14945de13e4a94f4210b9f5d65995e9ea70f3d06b45df0d863d67f758eb25502`.
- Applies only after prior tested tree `830b4673e7e3d49ed3f1e272af420f8fba75059e`; reconstructed after-tree **`43db6bd5ef8d3b7919a501094e8e6bca10570a9a`**.
- Metadata/file/blob identities and required sensitivity: `.cyril-c6la/carry-gutter-repair.source.json`. Original worktree ALREADY contains all five edits. Initialized private index constructed the artifact and was removed; no actual staging/commit/push.
- Files: chat_layout/wrap.rs, chat/presentation.rs, chat/retained.rs, conversation_performance_tests.rs and benches/conversation_render.rs. No Cargo/vendor/auth/fixture/actual-zj0m changes. Historical artifacts remain byte-unmodified.

ReviewCarryParitySource and ReviewStableGutterSource independently report correct/no findings, confidence.93 each, on their frozen disjoint source. These are pre-qualification source reviews, NOT check results or replacement final repair review. Review/evidence details and exact new test names are owned by review-decisions.md below. Real Paragraph supplies safe full comparisons and corrected-row cell/style checks; independent literals supply the changed row-boundary expectations.

Main next runs repository formatting/Clippy and the affected wrap/presentation/retained/root-consumer tests, actual required sensitivity, uninstrumented source-policy gates, full workspace/build and production-shaped benchmark/remaining smoke. New source-save normalization (benchmark imports/ensure formatting) is recorded in metadata and tool QA; no worker formatter ran. Final formatted source needs its own pin. User-native Kiro checks remain separately paused, not waived. Full issue completion/docs-after-smoke/PR remain Main-owned.

## Complete C6-only assembly export from frozen candidate43db

Assembly owner AssembleConversationDelivery requests a complete feature delta for actual branch7972f884fa35de6a0cffa8cb7e78349af4b7d3f3, not only F11/F12. `.cyril-c6la/production-complete-source43db.patch` contains the exact34 C6 source paths owned by historical c690/F9 scope, taking their complete current blobs from frozen candidate43db6bd5ef8d3b7919a501094e8e6bca10570a9a.

- Export base is pre-C6 **tree snapshot**84b4681afd30af1f537a56ca6ec4fe3b6951cbb4, not a commit or current delivery head.
- Complete patch:34files,555698bytes, SHA256 `b7d78998aa8ca5fbdb2db8682a30f18eac73a782156860a1bfc41aab63c87c4c`.
- Reconstructed **feature-only** after-tree `f80aea98ef9a5b42df7deffbcf3f26deb6848319` equals candidate43db on all34 feature paths but deliberately retains the old base outside that scope. It is NOT a claim that the complete candidate tree equals this export.
- `.cyril-c6la/complete-c6-source43db.source.json` owns every changed path/base/candidate/target blob, artifact lineage, owned workflow inventory and portable mutation recipes. Actual target7972 tree is858bfff965633bb3b876a13a701457936af17ec9; this is a source ledger, not target merge/qualification proof.
- Exclude the separately owned common auth fixture at crates/cyril-core/src/protocol/kas/auth.rs (commit6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69), unrelated highlight.rs formatting, and all embedded workflow snapshots from the production patch. Source inspection shows the highlight difference is only test formatting. No common-auth path overlaps the34 C6 paths.
- Six feature paths differ between actual target and both old base/candidate: conversation_render.rs, theme.rs, traits.rs, widgets/chat.rs, widgets/markdown.rs and tests/missing_tool_updates.rs. Reconcile both sides, retaining actual Zj alias-LAST and other landed fixes; do not overwrite target wholesale. Other relations are20 equal-old-base and8 absent/new files.

Copy current owned `.cyril-c6la` workflow/proof inputs verbatim according to the manifest, separately from this production patch; their working-file origin is NOT the old workflow blobs embedded in candidate43db. Do not overwrite actual x5xi/zj0m workflow with this original worktree's historical copies. F3/F9 frozen recipes retain their22-widget/source pins; combined MCP's approved23-widget/current-policy qualification is assembly-owned, not a relabeled old PASS. Original source remains untouched; export uses a removed private index only. Main/assembly own merging, formatting/new pin, all actual sensitivity/qualification/review and PR.

## Current delivery qualification and publication disposition (2026-10-21)

The [current publication manifest](current-width-e3f2a7c7/publication-manifest.json)
owns the current evidence packet, source bindings, executed receipts, consumer
archives and limits. This section appends the observed disposition; every earlier
source/proof/review pin and authored-but-unrun checkpoint above remains historical.
Neither the old candidate43db nor the pre-format current-width handoff is
silently repinned to the qualified source.

### Qualified source and artifact boundary

| Identity | Current binding |
|---|---|
| Bounded workspace-input descriptor | `e3f2a7c73e2de8dfb1895be825305a572dd203a94743633490f8de8f91165f94`, covering 505 source-input rows |
| Input-manifest SHA256 | `19bbbc77ad4ddd548512d780dc6a662a3101f3096c49a57c5394bbed4f9b4098` |
| Canonical qualification-receipt SHA256 | `1716bd3091f8c41835afb8cdda2d6214c77a73d99a0ebcbe138be502f02ccb96` |
| Qualified private-binary SHA256 | `3efb0b67f240a55236d8fbabdaa45a0cbd8acfd2ad59f5071a7f936f81c1fd72` |

The descriptor is **not a Git tree, commit, `source_revision` alias or final staged
tree**; the packet-directory suffix is a descriptor prefix, not a commit prefix.
The canonical receipt and private binary have distinct identities and purposes;
neither proves a later integrated source state. Product executables and full
Rust/workspace/production-source copies are excluded from the publication packet.
Existing executed harness/decoder/proof inputs and small named mutation/rebinding
patches are retained only with explicit allowlists, hashes and source bindings.
Reproduction inputs are not execution authorization.

The qualification records 35 commands: 29 exit-zero successes, three intended
compiled assertion REDs (exit101), three expected benchmark-CLI negatives (exit1),
and zero unexpected failures. Workspace results are 2953 passed, 0 failed and
13 ignored. Both TestBackend benchmark modes produced **79 ordered case
executions**, not 79 unique workload names, and 2370 raw samples. Source review is
correct/no findings, confidence0.96, with its scope and retained sensitivity
applicability recorded in review-decisions.md and the packet. These are bounded
source/test/benchmark results, not native performance or final publication approval.

### Three distinct observed PTY consumer outcomes

| Run | Observed outcome |
|---|---|
| Original current-C6 burst, truecolor | **FAIL**, 89.93s; 53 checks true and 36 capture pairs. At postc09, `C6LA_HISTORY_LIVE_TAIL` and `C6LA_COMMIT_269` were missing while oldest rows and `SCROLL` remained. Separate `none` startup was NOT RUN. |
| Qualified pre-C6 Zj, original burst | **ASSERTFAIL**, 114.94s, not an outer timeout; the same postc09 observable, 53 checks true and 36 capture pairs. |
| Current C6, reviewed chunked cadence | **PASS in both independent startups**, `truecolor` and `none`; each has 67/67 checks, 49 capture pairs and 1200 selected intended page actions, reaching content-backed failed-tool chronology c21a and final turn c22. |

Cleanup is COMPLETE for all three runs. The current cadence preserves the three
original calls (400 PageUp, 400 PageDown, 400 PageDown), each delivered as10×40
with the unchanged ordinary0.25s drain. The third call was unreached in both burst
failures; its cadence change is approved class consistency, not an observed
third-call defect. Original archives remain byte-identical.

The baseline establishes an inherited **observable burst limitation only**.
Chunk size and cumulative draining/pacing changed together. No upstream root
cause, determinism, exact decoded/dispatched/consumed key count, global C6
exoneration, product fix or waiver is established; neither FAIL becomes PASS.
The packet and review-decisions.md retain the actual PTY consumer adjudication.
The later supported-native result below does not relabel any of these runs.

### Required supported-native outcomes — PASS

The owning [native supported-controls evidence record](current-width-e3f2a7c7/native-supported-controls-evidence.json) and [native evidence archive](current-width-e3f2a7c7/native-supported-controls-evidence.tar.gz) retain the actual owned Ghostty/Xvfb `truecolor`/`none` pair, complete independent PNG appearance reviews, content assertions, natural exits/cleanup and unchanged-source receipts. [Design's Main technical judgment](design.md#main-technical-native-gate-judgment) owns the combined gate: qualified exact deterministic root/layout cells, styles and conservation; correlated native exports/source/scene receipts; and positive actual-PNG native appearance with no observed defect. Counts, hashes and process identities belong to the evidence record.

Every named appearance subcheck passes in each mode; the reviewers' 26 literal claims per mode remain NOT_PROVEN by visuals alone. The historical capture handoff/mode receipts keep their original capture-time pending-visual statuses, discharged only for the current gate by later reviews and Main's judgment. No new pixel oracle, waiver, source change or UI control is introduced.

This is the original two-launch native pair, not the distinct all-four-mode state/render matrix or a same-process switch. Existing state/render/root qualification owns approved main↔child focus/back; same-process invalidation and benchmark evidence keep their separate current qualification. The original 50,000-history and giant/>u16 deterministic/benchmark proof is not native50,000 coverage or an additional native fixture requirement; prior blanket native-pending wording was an overbroad nonclaim, not a controlling obligation being waived. Retained workbench/cross-feature, final integration and publication limits below remain Main-owned.

### Main-only publication boundary

Late raw-index drift occurred after qualification while all505source inputs
remained exact. Full4158logical-index identity and a stat-refresh explanation
were not proved. Main adjudicated this as not a product blocker and owns expected
staging/integration; this is not a claim that the final index/tree was tested.
These docs/evidence are postqualification publication additions.

**Main alone owns** the held93 merge/current-main integration, staging, commit,
tracker updates and a **NEW C6 PR**. **PR187 remains untouched and unmerged.**
New-PR CI/platform evidence, final integrated-tree qualification and the retained
workbench/combined-feature obligations named in plan.md remain unqualified. The
required supported-native outcome gate above is PASS, not a claim of native
performance or later platform approval. The exact prior PR183 Windows exception
is not platform-green or a waiver for new failures. This publication package
does not declare Structural terminal completion.

### Owning ordinary-scroll verification disposition (2026-10-21)

The [scroll-obligation adjudication](current-width-e3f2a7c7/scroll-obligation-adjudication.json)
owns Main's technical instrument-adoption decision and the complete original
quotes/frozen-source pins. The original DG-approved authorities are
`design.md:142` (“scroll oldest/middle/newest and back to follow”), C2 at
`design.md:117` (“Preserve first/middle/last windows, follow/rows-back/overlarge-back”),
and original `route.md:57` (“oldest/middle/newest/back-to-follow scrolling”).
Their positive ordinary-scroll obligation is observed browsing and return to
follow, including rows-back and overlarge-back, under the stated App-owned
RenderState lifetime and terminal-evidence controls—not single-write throughput.

**Main explicitly adopts** the current10×40 cadence for each of the same three
400-action calls as a coverage-preserving consumer instrument under CONTRACT
Approval semantics/verification complexity. All1200 intended actions per mode,
other cases/actions/assertions, geometry/timeouts and terminal evidence remain.
This is Main's technical verification decision, not a user quote, user-risk
waiver or adoption inferred merely from execution GO. The current full-cadence
PASS covers that approved ordinary-scroll obligation; instrument adoption is
resolved, and the original burst failure does not by itself block that obligation.

The original current-C6 burst **FAIL**, qualified pre-C6 Zj original-burst
**ASSERTFAIL**, and current adopted-cadence **PASS** remain distinct. The inherited
observable burst limitation is preserved, not relabeled PASS/N/A, fixed or
globally exonerated; submitted actions are not proved App-consumed counts.
Any separately mandated burst/throughput obligation discovered would block merge
until repaired or explicitly user-risk-approved; this decision supplies no waiver.

This ordinary-scroll disposition preserves earlier checkpoints and all
source/proof/review pins. The later [supported-controls user approval](design.md#supported-controls-user-approval)
assigns separate native `truecolor`/`none` launches and existing state/render/root
focus/back to distinct instruments; the all-four-mode state/render matrix and
same-process invalidation/benchmark evidence retain their own qualification.
**Required supported native outcomes: PASS**, under the [Main technical combined-layer
judgment](design.md#main-technical-native-gate-judgment) and owning native evidence
record above. The raw capture-time pending-visual statuses remain historical;
the literal visual-only limits are not promoted to exact proof. Current-main/
held93 integration, retained workbench/cross-feature, final integrated-tree and
new-PR CI/platform limits remain Main-owned. No remaining gate is waived, no
whole-issue or blanket merge approval follows, and PR187 stays untouched/unmerged.

## Held Main038 integration and current App consumer (2026-10-22)

The [current integration manifest](current-main038-b5d45fe2/publication-manifest.json)
owns the new qualification/consumer capsule and publication overlay. The
2026-10-21 sections and original packet above retain their historical bindings.

| Qualified snapshot identity | Binding |
|---|---|
| Bounded inputs | 506 rows; `b5d45fe2b8c78d23b5e5e26f5f8ff963822822830d2b5d54cba39f85f6c2c157` |
| Manifest SHA256 | `f14e529a5bcec86983edac49be4b83958c86ea53b504eb9ee7299b22e4106f77` |
| Final canonical SHA256 | `018f7b4b70de5933d3bf1b59b89729638b55ce0a8ca94cf003c29266adbda484` |
| Default private App SHA256 | `993a2fd5f32353b7d0701ca37281f5b7ad5c03e5d58b3e9e0c2e60da6de57388` |
| HEAD / held incoming | `31ff479539d6c69b6a61c571cbc999bafacdb856` / `03838a501dad8be140acd78dc3fff4cae35fafef` |

All 17 assigned current local commands passed: outer workspace 3113 passed,
0 failed and 13 ignored, with 217 source-derived required current cases observed.
The isolated package/feature lanes and current source fences retain their exact
scope in the canonical receipt; this is not all-feature/release/doc/platform CI.

The fresh actual-App **truecolor consumer PASS** used the original C6
`/usr/bin/python` plus existing `smoke-dfb` environment, pyte0.8.2/wcwidth0.9.2.
It completed in 87.7335463759955 scenario seconds (88.16s tool wall time), reached
all original 67 predicates and 49 capture pairs, and completed independent owned
cleanup with no survivors or unproven marker matches. The same three 400-key
legs remain 10×40 ordinary sends: 400 PageUp plus 800 PageDown, 1200 selected
actions. Pending comparison/resize and the actual deny-only response passed.
The [consumer record](current-main038-b5d45fe2/c6la-main038-consumer-result.json)
preserves the earlier Python3.13.9 prelaunch pidfd-guard failure, the separate
host-self capability probe and the newly authorized successful App invocation.

Main retains the original 192 native PNGs/two-startup-mode proof as a historical
505/e3f2a7c7/3efb0b67 component. Changed pending-approval applicability has bounded
static equivalence plus fresh ordinary and actual-App corroboration; no new
current506 native frames or `none` launch are claimed. The original 78 applicable
benchmark observations retain their source pins; the pending-preview timing is
historical only. Original burst failures and visual-only literal limits remain.

The 506 descriptor identifies the qualification/execution snapshot, not a Git
tree or the later documentation/staged tree. README's post-smoke documentation
change is an explicit publication overlay, not silently repinned qualification.
Main still owns the held038 merge, staging/commit, tracker, NEW C6 PR and remaining
CI/platform/delivery decisions. PR187 stays untouched/unmerged; user-native Kiro
turns remain separately paused, not waived. No Structural terminal completion or
blanket merge approval follows.

## Restored layout-owner qualification and runtime (2026-10-22)

This section owns current evidence status. The
[restored-owner manifest](current-restored-layout-owner-7d88498e/publication-manifest.json)
binds the 506-input descriptor `7d88498e…`, manifest `2401dcc7…` and default App
`e31ef685…`; full hashes, inventories and receipts belong there. The preceding
b5 Main038 and e3 native/benchmark checkpoints remain dated source history,
not current D1 producer proof or substitutes for the fresh results below.

Main chose **D1 restoration of the approved owner**, not an ownership waiver:
layout owns physical preparation/aggregation/index/seek/traversal/painting;
chat retains presentation/source resolution and transcript policy. D2's Core
receipt metadata and D3's `traits.rs` projections were already approved and are
now recorded in the existing ledger without new permission. The
[finding dispositions](review-decisions.md#restored-layout-owner-findings-and-current-evidence-2026-10-22)
preserve the original c084 reconstruction and conformance FAIL.

Fresh source-level conformance is now **PASS**. `ReconstructC6RestoredFinal`
derived the working-production map blind to design/change artifacts, then
`CompareC6RestoredFinal` independently checked that frozen reconstruction
against the unchanged approved design and load-bearing working source. They
inspected the committed census plus the three working repairs, not merely
the c084 HEAD tree. Main binds this restored target to the qualification
descriptor `7d88498e…`; the reviewers did not independently qualify the 506-input
manifest, executable, benchmark, native or CI results.

- [Frozen reconstruction](reconstruction-restored-7d88498e.md), 44,955 bytes,
  SHA256 `97c8f8c7baf33453eaeb0a38eec052c2227925e276682e956ea89f0cc4881fad`.
- [Fresh comparison](conformance-restored-7d88498e.md), 22,607 bytes,
  SHA256 `08d9f1bfbea3e44b5386728479ba80bc69cc3049e0b3766083fe64446fcdde4e`.

All 16 changed production files map exactly once to nine approved ledger rows:
9 MATCH, 0 MISMATCH, 0 UNCOVERED, 0 MISSING and no unresolved conformance
candidates. D1's restored owner and the already-approved D2/D3 ledger
reconciliations leave no residual discrepancy. The
[report quartet and reconstructed map](review-decisions.md#fresh-isolated-source-conformance-and-frozen-report-quartet)
preserve the old c084 FAIL separately. Design SHA256
`219f959520dd99b75d14a6b93ee2850e42bab80c62c3f1cd99118ac3e993a8e9`
remains byte-exact reviewed input: its prior pending snapshot is history,
superseded for this current source-conformance gate by the new reports, not
rewritten or promoted to final acceptance.

- **Local qualification PASS:** all 17 assigned commands; workspace 3118 passed,
  0 failed, 13 ignored. The manifest separates inherited case groups from the
  current owner-case roster; these are local package/feature scopes, not CI.
- **Fresh complete benchmark PASS:** all 79 ordered executions / 2370 samples:
  three completed-Write cases including pending preview, plus 76 long-history
  cases. This supersedes the b5 retained78/pending-timing restriction only for
  restored-source timing; it is TestBackend measurement, not native performance
  or an unmeasured-baseline speedup.
- **Both-mode PTY PASS:** the [actual consumer aggregate](current-restored-layout-owner-7d88498e/c6la-restored-layout-owner-consumer-result.json)
  records separate first-attempt `truecolor` and `none` App lifetimes, each with
  all 67 checks, 49 capture pairs and COMPLETE independent owned cleanup.
  Pending comparison/resize/deny and chronology/viewport/lifecycle controls
  passed. PTY submissions are not App-consumed-key, retained-row or work counts;
  wrapper exit0 is not an independently recorded graceful App exit0.
- **Supported native PASS with explicit limits:** fresh Ghostty/Xvfb launches
  each recorded 79/0 content assertions, 96 PNGs and 40 obligations, natural exit
  and complete owned cleanup. All 192 images / 80 obligations were reviewed;
  [Main's qualified judgment and exact limits](design.md#current-supported-native-judgment-and-limits)
  preserve the distinct review dispositions, four actual resize changes,
  toast occlusion and unattached-mark export omissions. Raw capture-time
  pending-review statuses remain unchanged, not relabeled PASS.

The descriptor names compiled/executed inputs, not a Git commit or the final
documentation/staged tree. These five-doc changes are a publication overlay.
Main owns publication/index/commit/tracker work on **existing draft PR197**;
the older NEW-PR/held-merge wording above is historical, and PR187 is untouched.
Restored-head CI/platform, applicable all-feature/release/doc gates and final
combined acceptance remain **PENDING**, as do Main's final Git/publication
decisions. The source-level conformance PASS above is not Structural completion,
merge readiness or issue closure. Retained workbench/cross-feature obligations
and separately paused user-native Kiro turns are not waived. Original burst
FAIL/pre-C6 ASSERTFAIL remain failures; ordinary 10×40 cadence adoption is not
a throughput fix or global exoneration.

## Approval-parent refresh: bounded current-source follow-through (2026-10-23)

The normal held merge of C6 `96fae2712888489dccd4cdfbf60e290879e8f6a5`
and published X `fa5b31bc270aca021223b2951d0dc92b3ac29789` had no text
conflicts. The sole integration adapter adds a shared mutable `RenderState`
to the three producer tests' helper and reuses one state across all five
compositions; its helper and complete scenario match qualified final201
exactly. C6's persistent history/layout owners and X's Shell/current-viewport
guards, twelve non-MCP modal callers, Replay moves and C9 source are retained.

The [refresh publication manifest](approval-parent-refresh-41295b82/publication-manifest.json)
and [portable evidence index](approval-parent-refresh-41295b82/integration-archive-index.json)
bind the actual **540-input** descriptor
`41295b82c541775250b92911b02471483597f2162e8676fc46e977a40a143fab`.
The five authorized gates PASS; their test totals are overlapping observations,
not unique tests. The fresh private default App/runtime pair also passed the
two existing owned smoke cases, `captured` and `retained-halfwidth`, with actual
driver/App exit0, complete owned cleanup and unchanged source/ref/config/doc/pair
pins through final postguard. This is bounded owned PTY evidence, not new native,
real-provider/auth, all-feature/platform or benchmark qualification.

Main accepted the original C6, X and final201 intersection reviews and the
narrow adapter applicability; no new full review round was substituted.
Original C6 compiled506/native192/79-benchmark evidence and configured final571
native13 evidence remain separately source-bound history, not current540
execution. Approved design and CI/release/current-width recipes are unchanged.
The executed README pin is retained; this section, its status cross-references
and the README terminal-forwarding caveat are a postqualification docs-only
overlay after the final runtime/source guard, not a repinned execution.

Current-head C6 hosted CI and final combined acceptance remain **PENDING**.
Independent PR190/dbb CI39872725799 is terminal FAIL: actual repaired C9 batch3
took 163.159373ms against 100ms. Final201 CI39873211761 is terminal FAIL:
Ubuntu's `runtime_drop_retains_evidence_until_core_completion` returned Os3,
`No such process`. Passing X/fa5 CI39874490149 and Windows/macOS legs do not
exonerate either failure. The user approved retaining `<=100ms` as a separate
isolated IPC-inclusive checkpoint instead of a tight ordinary-suite assertion;
`xy4x` repair proceeds in a separate worktree from main
`afedbdcd44bcc9182aa296e803b7a55a880a7716`. The reviewer proc-fd
`open -> reap -> read` mechanism was independently demonstrated; its separate
deterministic regression RED is pending. Neither existing CI failure is erased
or waived, and no blind retry or green-sibling exoneration is claimed.
Main may commit this local packet but **HOLDS PUSH until the shared CI repairs
integrate**. Historical Windows96 failure/u62 attribution remain unresolved.
Main owns staging, the held merge, exact publication and merge decisions;
no merge readiness, issue closure or global acceptance follows from this PASS.


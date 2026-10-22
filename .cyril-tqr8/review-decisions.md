# cyril-tqr8 review decisions

**Current held-main503 checkpoint (2026-10-22): fresh Cargo qualification PASS and one fresh actual owned-consumer PASS, separate from scoped static integration review correct0.96/no actionable source findings.** The [owning held-main evidence](evidence.md#held-main-integration-2026-10-22) owns current source/execution figures and archive/member pins. The [standalone PR193 repair package](current-publication-evidence/pr193-repair/index.json) retains ordinary repair review no findings/correct0.97 and its original actual qualification/consumer results; it is not promoted into whole-merge or isolated-reconstruction review. Prior a09 qualification and all original RED/FAIL receipts remain historical with exact source/reach. Main038 source integration is held and qualified; Main still owns completed merge/ancestry, commit/push, current PR-head CI/review, separate u62f/native/final-integration obligations and publication.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | The subagent-written loopback fixture has not executed; run py_compile and manifest/SIGTERM smoke before handing it off. | Delayed aggregated model advisory | Verified | The source handoff and `evidence.md` explicitly mark fixture execution pending. No py_compile/start-stop or integrated consumer run receipt exists. The delegated assignment explicitly prohibits worker build/test/lint/formatter/smoke checks and assigns verification to Main. | Reject | N/A — worker does not override the explicit verification-ownership contract. | non-blocking for approval-only handoff: parent must perform fixture qualification before relying on it for any live acceptance. This is not risk acceptance or permission to ship unverified code. |

Parent subsequently qualified the current fixture and ran the earlier native evidence. The 0.98 feature repair review and separate 0.97 common auth-fixture repair review have different scopes; neither is an empirical acceptance substitute. Their exact provenance and the later native-auth stop are in the current evidence record.

## PR193 four-comment inventory and bounded repair

Review5594500686 judged `b75010427c16b1dac13b7cf00c595ce2492f391f`; all four comments were unresolved/non-outdated in the supplied review. The source-verification report is `agent://AdjudicateTqrScopedViews`, backed by the paths below and the actual [PR193 review comments](https://github.com/dwalleck/cyril/pull/193). This record distinguishes **SourceVerified mechanisms** from behavioral tests, which this worker has not executed.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F-WORKFLOW / [4332636476](https://github.com/dwalleck/cyril/pull/193#discussion_r4332636476) | Workflow-owned MCP errors lack a stream-history renderer. | chatgpt-codex-connector | Verified | Original workflow/pending routes actually RED; fresh App workflow/pending/adoption cases PASS, plus actual visible workflow presentation/dedup in the owned consumer. | Modify | Existing workflow seam keeps canonical storage and adds one safely attributed main copy through MCP notice-and-reveal. | Resolved in scoped repair review; no workflow view, authority migration or native workflow-origin claim. |
| F-URL / [4332636456](https://github.com/dwalleck/cyril/pull/193#discussion_r4332636456) | Global sign-in notice is hidden by supported child focus or prior main browse. | chatgpt-codex-connector | Verified | Original UI/App URL cases actually RED; corrected fresh App14 suite reaches exact URL/count/preservation assertions; actual owned consumer proves browse-to-main reveal/full URL/draft/caret. | Modify | One chronological global main notice, leave child focus and reset main follow; no banner, invented source or child-owned global copy. | Resolved in scoped repair review; focus entry is existing public UI/API support, not an ordinary-binary binding. |
| F-GUTTER / [4332636462](https://github.com/dwalleck/cyril/pull/193#discussion_r4332636462) | Focused-child full-width paint lets scrollbar overwrite content. | chatgpt-codex-connector | Verified | Original opaque underscore/live415/tiny geometry failures retained; corrected focused tests actually PASS and run in fresh suites. | Accept | One private allocation-free content/gutter split used consistently by both existing render branches, no scrollbar at widths0/1. | Resolved in scoped repair review; original RED/oracle FAILs remain immutable and C6 remains separate. |
| F-CLI / [4332636469](https://github.com/dwalleck/cyril/pull/193#discussion_r4332636469) | Example whole-string escaping changes printable error text. | chatgpt-codex-connector | Verified | Separate owner actually captured one pre-fix RED and one GREEN with three exact error rows; unchanged probe/example source retains applicability. | Accept | Actual example error arm uses existing terminal-control/bidi-only formatter. | Resolved in scoped repair review; finite initial-drain proof, not full-example/native/network/CI acceptance; no replay. |

### Review errors and source-reachability qualifications

- F-URL/F-GUTTER are supported-state/public-UI defects. No production caller currently enters child focus: the setter and renderer exist, test callers enter it, and App only binds exit via Esc. `docs/omp-feature-comparison.md:242-247` independently records this gap. Do not add a focus binding to manufacture reachability.
- F-URL's “acknowledged immediately” is imprecise: `protocol/kas/callbacks.rs:429-439` acknowledges only after bounded event acceptance. The defect is downstream presentation; callback ACK semantics are not the repair.
- F-WORKFLOW does not establish that all workflow UI is missing: workflow commands, status snapshots, approvals and parked questions already render. None consumes arbitrary workflow-stream MCP system notices; “permanently invisible” means no current presentation consumer, not a claim about all future versions.
- Earlier main-only scrollbar/source/consumer proof does not establish focused-path correctness. F-GUTTER is a pre-existing geometry mechanism newly exposed by this feature, not evidence that the main-gutter repair introduced it. C6 integration remains separate.
- The first read-only separator diagnosis cited installed widgets0.3.0 rather than the actual locked version. **Corrected evidence:** current `Cargo.lock:2464-2467` resolves `ratatui-widgets0.3.2`; the actual0.3.2 `reflow.rs:126-149` and `paragraph.rs:431-454` directly establish the same relevant wrap-boundary whitespace behavior. The old version citation is not current dependency evidence.
- The second stopped repaired attempt's `global manual URL hidden` assertion overstates the displayed result: its full frame contains the complete URL, with only a phantom continuation-cell blank after `日` in the test's all-cell string projection. This is a second **test consumer/extraction** defect, not newly observed product visibility or sanitization loss. Main approved replacing that helper's inner loop with the existing physical-width traversal, not changing the fixture or normalizing spaces.

### Scoped UI/App repair — local checks, review and owned consumer PASS

**Governing claims and decision:** C2/C3 and `design.md#dg-supported-view-presentation-and-gutter-decision`; Main's DG selects the MCP-only chronological notice-and-reveal policy within existing responsibilities. Main notice once for global URL/main error; canonical scoped error plus one `[error] [session {safe actual SessionId}] {safe message}` main presentation for child/workflow. Existing pending classification/replay and late adoption stay authoritative. General `add_system_message`, workflow-view/focus-entry scope and all protocol/auth semantics are unchanged. This class-wide decision supersedes the earlier main-only restriction only for the existing focused branch; it does not absorb C6.

**Delivered behavior:** after actual pre-fix RED and explicit Main production GO, only `state.rs` and `widgets/chat.rs` production prefixes changed in this UI repair. `add_mcp_notice` uses unchanged chronological `add_system_message`, leaves child focus and resets main follow; scoped application inserts the canonical error first, then one safely attributed main copy. Global URL/main error use only the MCP operation. Adoption, App classification/pending logic, canonical SubagentStream, general system-message policy, draft/cursor/activity/authority and protocol semantics remain untouched. One private stable content/gutter split serves committed/live preparation, line count, scrolling/paint and guarded scrollbar in both existing branches; widths0/1 retain content. The original production handoff preserved all frozen tests. Subsequent explicitly approved oracle/helper corrections and four test-only formatter reflows are distinct frames recorded below; historical raw-byte pins are not current postformat identity.

**Frozen selectors and pre-fix predictions (actual results below):**

| Selector (fully qualified suffix) | Declared cases | Expected pre-fix result and distinguishing assertion |
|---|---:|---|
| `state::tests::mcp_notices_reveal_main_without_changing_draft_or_activity` | 1 test, URL/error loop | RED: focused child remains selected; after repair both notice kinds must preserve all non-presentation state and commit preceding text/thought. |
| `state::tests::ordinary_system_message_keeps_child_focus_and_main_browse_position` | 1 | GREEN control: ordinary system messages must not inherit MCP reveal policy. |
| `subagent_ui::tests::mcp_error_retains_canonical_text_chronology_and_active_tool` | 1 | GREEN control: canonical error/activity/chronology and tool index already work; preserve them. |
| `widgets::chat::tests::focused_mcp_error_bytes_survive_scrollbar` | 1 | RED: complete safe error bytes are lost at the content/gutter edge under actual overflow. |
| `widgets::chat::tests::focused_gutter_width_is_stable_without_overflow` | 1 | RED: last full-width content cell occupies the intended stable gutter and the expected continuation row is absent. |
| `widgets::chat::tests::focused_live_text_uses_reserved_content_width` | 1 | RED: fewer than415 live `L` cells survive in139content columns. |
| `widgets::chat::tests::zero_and_one_column_focused_chat_keep_content_without_scrollbar` | 1 test, widths0/1 | RED at width1: scrollbar chrome overwrites expected content/blank cells in the four-row inner viewport; width0 remains blank and all outside cells are checked. |
| `app::tests::mcp_oauth::global_mcp_url_reveals_main_after_child_focus_and_prior_browse` | 1 | RED: actual App frame hides the complete global URL;40prior main messages/browse prevent clearing focus alone from passing. |
| `app::tests::mcp_oauth::mcp_error_keeps_canonical_owner_and_reveals_main_presentation` | 5 generated cases: main, main-claimed, tracked, optimistic, workflow | RED in all5: actual frame hides the notice/presentation. Then fences exact safely escaped actual-session attribution, canonical owner, chronology, once-only main copy, live state/authority, later text/status durability and zero outbound commands. |
| `app::tests::mcp_oauth::premain_mcp_error_waits_for_authority_then_presents_once` | 3 generated cases: main, child, workflow | Main GREEN control; child/workflow RED for missing main presentation. All3 retain the original three-frame pending sequence until main authority, then assert canonical order and one visible notice. |
| `app::tests::mcp_oauth::late_workflow_adoption_moves_canonical_mcp_error_without_representing_it` | 1 | RED for missing initial presentation. After repair adoption must preserve exact canonical history, add no main message and not reset a subsequently selected browse position. |

The frozen suite declares **17 new/replacement runnable cases**:7UI plus10App; the parent actually observed **14 intended RED and3 GREEN controls**, plus4retained App GREEN,21runnables total,0ignored/0unexpected failures. Source declares19input scenarios, but only18iterations entered: the second UI-state MCP-error iteration was not reached after the first URL assertion failed. App's complete module retains four prior tests (one `kas`-gated):14declared cases with KAS,13without. Net test count grows by16. Existing main URL/live/stable/tiny-gutter and v2 manual-URL tests remain unchanged. Exact commands, flags, source pins and per-case reach are in the owning RED receipt, not re-executed by this worker.

**Production authorization:** Main explicitly supplied GO after consuming canonical actual RED receipt `/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/tqr8-scoped-notices-test-only-red-attempt1/canonical-handoff.json`, SHA256 `b7035ad7ab352b290981d9558959881d7a9330101a3e1018694e962945b17256`. The unchanged pre-fix498-row bounded source descriptor is `0556e444909cab450c0326eb2f7c25aabafa2f014fdc77b11ae20267ac263382`. Main authorized only the approved MCP notice/stable-gutter production correction in state/chat, preservation of frozen tests/other production, and existing workflow-doc/handoff amendment. This is not Green, publication or scope/risk relaxation.

**Performed checks and results:** qualifier's eight authorized commands ran once each: seven exact UI selectors yielded2PASS/5intendedRED; complete App module yielded5PASS/9intendedRED. Live focused text showed413characters vs415required; width1painted▲instead ofX at(1,1). Ordinary system-message policy, canonical-stream/live-tool control and pending-main control completed GREEN, as did four prior App tests. All six file pins, production-prefix/test-body pins and the bounded source manifest were stable. The canonical receipt owns these results; this worker inspected it but ran no Cargo/build/test/lint/formatter/smoke or Git operation.

**Reach limits, not acceptance claims:** UI state's second error iteration and first URL test's preservation/chronology assertions were unreached. App URL and all five error-route preservation/owner/order/attribution/later-status assertions after the failed visible-frame check were unreached. Pending child/workflow canonical selection/order assertions after missing-main-message failure were unreached. ALL late-adoption actions followed its failing initial visibility assertion and were unreached. Focused error/live/short-history later chrome/header assertions and remaining width1cells were unreached; width0's18cells completed. Post-repair GREEN must execute these remaining obligations; actual pre-fix RED is not proof they pass.

**Attempt chronology, preserved:** attempt1 passed the first three UI selectors then stopped at the focused-error separator assertion; attempt2 passed the corrected focused error and remaining three UI cases, then App13PASS/1wide-cell-projectionFAIL. These FAILs and original assertion reach remain immutable. Attempt3 freshly passed all14App cases and broader source gates before the final formatter check failed; Main then approved one four-file test-only formatter correction, explicit token/production identity retention and actual final fmt/default-build PASS. Final details below, not the old stopped-attempt statuses, own the current scoped result. No CI/native/integration failure is waived.

### Focused-error wrap-separator oracle correction

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| Oracle/focused-error-separator | Full reconstructed viewport string requires a separator legitimately consumed at a wrap boundary. | Main and RepairTqrScopedNotices diagnosis | Verified | Immutable old-gutter capture misses one opaque underscore; repaired capture has the complete287-byte tail but omits only the prefix separator. Actual locked widgets0.3.2 source explains the boundary. | Modify | Exact306-byte canonical System equality plus ordered rendered prefix and complete contiguous unchanged opaque/escaped tail/final marker; all original input/geometry/helper/production unchanged. | Coverage preserved; corrected Rust case actually PASS in attempt2 and fresh suites. Original FAIL remains FAIL, not retroactive execution. |

**Actual first repaired attempt, retained unchanged:** `/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/tqr8-scoped-notices-repaired-green-attempt1/canonical-handoff.json`, SHA256 `bc29bc7153e388d31d3e4e0452692fc16caf5a74f223b0d39470b19d4800857d`. The first three exact UI tests PASS; focused-error test then fails at `chat.rs:2398` with `full focused error lost`. Source498descriptor `adb7bd9768f157e54f250c74303bef21c34ec1f854b2536aabfcf21f7c8d8c1b`; failure log SHA256 `80c8276bdbcb6385e968044853371cf9c2b48191a0aa69c7d0fbc7eae04a7f9b`. The qualifier stopped; later focused tests, App module and repository/feature/format gates did not run. This remains an actual FAIL on its original test frame, never relabelled GREEN.

**Root cause and resolved dependency evidence:** current Tqr `Cargo.lock:2464-2467` pins widgets0.3.2, registry checksum `66e3d19bcc9130ca376277d93b60767ff121ace3be06f5f95f81dd68956407d1`. Direct installed source `/home/dwalleck/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-widgets-0.3.2/src/reflow.rs:126-149` emits a wrapped prefix line and pops pending separator whitespace fitting that line's remainder, without a trim guard. Its `paragraph.rs:431-454` uses that WordWrapper and renders successive returned vectors on successive rows. Existing `viewport_content_text` trims row-end padding and concatenates rows. Therefore `[error] MCP failed` and the long opaque word have no reconstructed separator, even with `Wrap{trim:false}`. This is independent of scrollbar overlap; production wrapping policy and reconstruction helper are not changed.

**Coverage-preserving technical decision — Main approved:** only `focused_mcp_error_bytes_survive_scrollbar` assertions change. The existing full expected canonical System message, including its separator and every safe byte, is now asserted directly against the authoritative child stream. The actual rendered prefix must appear before the complete unchanged287-byte whitespace-free opaque payload plus escaped suffix and `#FULL_ERROR_END`, with that entire payload compared contiguously and exactly. Actual scrollbar, raw-control absence and focused-session assertions remain. Input notification, opaque literal/repetition, Unicode/quotes/backslash, terminal geometry, helper and all production prefixes are unchanged. No whitespace normalization, masking, reinsertion, emulator or alternate wrapper is introduced. Exactly one of the17new/replacement cases has a new assertion frame; the original frozen frame and both prior receipts remain immutable. Current before/after file/test-body pins are in `local://tqr8-focused-error-oracle-repair.json`.

**Retained sensitivity and limits:** offline comparison used only existing captured diagnostic strings, decoded without normalization. Original old-gutter RED contains286payload bytes rather than287 and lacks exactly one underscore; the corrected full-payload assertion still rejects it. First repaired FAIL contains all287payload bytes exactly; its305-byte reconstructed error differs from306-byte canonical text only by the wrap-boundary separator. This is decisive retained old-gutter sensitivity for the unchanged production line, **not** an executed corrected-test GREEN or proof of its newly added canonical/later chrome assertions. Main explicitly requires no mutation rerun for the unchanged gutter line.

**Worker / subsequent execution:** the repair worker diagnosed source/lockfile/retained captures and made only authorized edits, without Cargo/tests/fmt/Git/runtime. The qualifier subsequently passed the corrected focused case and fresh suites; actual attempt3 completion, controlled formatting applicability, independent review and current owned consumer are recorded in the final checkpoint below.


### App wide-cell projection correction

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| Oracle/App-wide-continuation | Per-Cell string joining adds phantom padding after wide `日`. | Main and RepairTqrScopedNotices diagnosis; read-only convention survey | Verified | Attempt2 complete frame contains the full notice with one extra continuation blank; existing approval screen_rows and actual locked CellWidth/paint establish the physical-cell traversal convention. | Modify | Replace only helper inner iteration with width advancement via existing CellWidth; preserve every real space and row newline. At correction all14case bodies/23444bytes were unchanged; later formatting preserves tokens, not those raw bytes. | Fresh attempt3 App14/14 actually PASS, reaching later canonical/state/authority/no-outbound-action assertions. No fixture normalization or product change. |

**Actual attempt2, retained unchanged:** `/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/tqr8-scoped-notices-repaired-green-attempt2/canonical-handoff.json`, SHA256 `e2d07ddfebad7f1097ea7c524b76c9bee89db4ca2913688b24b89e3d57d28dd4`; source498descriptor `4ffcbee23ac0c22ad6117290a881f2abb972eea3c74be8dc087c26fb19abb4e4`. Corrected focused-error plus remaining three fresh UI cases PASS; first three earlier passes were retained only as byte-applicable, not rerun. Whole App module14cases actually yielded13PASS/1FAIL: URL notice test at original line251. Log `05-fresh-08-whole-App-mcp_oauth-module.log`, SHA256 `fe0c7e676f80f6506885397cc348f12f2988c5ef2f96d17183771559f51c3f82`, contains the complete32-row frame at lines32-63. Later URL canonical/state/activity/authority/outbound-action assertions were unreached in this attempt; all other App cases completed, including the five error routes, three pending cases, late adoption and KAS conversion. Broader gates stopped.

**Root cause / existing convention:** old App helper `mcp_oauth.rs:5-18` appended all160physical Cell symbols independently. A width2glyph occupies its own cell and covers the next blank cell; the test therefore projected `日` plus an artificial space where the terminal displays `日` alone. Existing `widgets/approval.rs:1852-1870::tests::screen_rows` explicitly advances by display width without trimming rows; `widgets/chat.rs:2222-2235::tests::viewport_content_text` uses the same traversal principle, although its row trimming/concatenation is deliberately not copied here. These private UI test helpers are inaccessible across crate test modules; no suitable width-aware helper exists through App `super::*`. Existing production `cyril-ui/src/text.rs:6-15,45-46` already uses `ratatui::buffer::CellWidth`; the App depends on Ratatui directly, so no Cargo/dependency change is needed.

**Resolved local dependency evidence:** `Cargo.lock:2396-2413` resolves ratatui0.30.0/core0.1.2; `ratatui-0.30.0/src/lib.rs:431-432` re-exports core buffer; core `src/buffer.rs:11-12` exports `CellWidth`. Core `buffer/cell_width.rs:8-44` provides canonical physical width; `buffer/cell.rs:309-317` returns a cell's forced width or symbol width. Actual widgets0.3.2 `paragraph.rs:461-472` paints each symbol and advances by its physical width. Core `buffer/buffer.rs:350-366` likewise resets covered continuation cells; its own debug projection at588-598 skips them. Source inspection concerns these actual locked versions, not remembered APIs.

**Exact bounded correction:** add only the existing CellWidth import and replace only the helper's inner iteration with `while x < width`: append the entire current Cell symbol unchanged, advance by `cell.cell_width().max(1)`, then retain existing untrimmed row collection and `join(\"\\n\")`. Real leading/internal/repeated/trailing spaces remain as width1symbols; row boundaries remain. There is no `replace`, filtering, trimming, normalization, changed URL/opaque bytes, fixture special case, new helper/parser/renderer or product/sanitization change. All14case bodies and complete canonical message/count/chronology/state assertions remain byte-identical; helper correction applies class-wide. Before/after helper and unchanged-case-region pins are in `local://tqr8-wide-cell-oracle-repair.json`.

**Original RED still discriminates, without a generic RED rerun:** the retained original App RED log has a still-focused child frame for URL at lines200-231, containing neither manual notice nor URL; five route frames likewise contain only `Other focused child`, with no `[error]` main presentation. Skipping covered padding cannot synthesize those absent prefixes/messages or reveal an unrendered main stream. Pending child/workflow cases fail canonical main-message equality before extraction, entirely unaffected; late-adoption initially fails its absent presentation. The original nine App REDs remain actual results on their immutable source/helper frame; no retroactive corrected-helper execution is claimed. Main selected this evidence-backed compatibility decision, not a blanket mutation waiver. The helper correction does affect current extraction semantics, so all14App cases require fresh module execution; retained13PASS is historical only, not current corrected-helper GREEN.

**Helper handoff / subsequent execution:** source/capture diagnosis, existing-convention survey and approved helper/review edits were completed without worker checks or runtime. The then-pending fresh whole-App module subsequently actually passed14/14 in attempt3; formatter-only retention and remaining scope limits are below. All prior receipts/frames remain immutable.

### CLI owner's separately authorized repair

RepairTqrCliEscapes reports the actual pre-fix probe ran once on `b750`: `/tmp/cyril-tqr8-cli-error-red-20261020/result.json`, SHA256 `7ace3887377db2068246a64ee41490175c970416ba2a611eec1477f69d695e63`, verdict `EXPECTED_RED_OBSERVED`, probe exit1/example exit0,3.30seconds. The actual example received three typed `session/update` display-error notifications during its initial drain, emitted the exact predicted over-escaped rows and exited through the existing no-SessionCreated path. Client wire operations were initialize/new only. Pre-fix private example SHA256 `bedb91ba30249f0856ca7962bef27cc774634cfa8dfbb19329566e47f246599a`; stdout.bin SHA256 `abad19c60f96326285b8b53046a5edb77eb99f0213282ce34b4bf104193fd324`. The CLI owner reports owned roots and exact example/Node/peer PIDs independently absent after the run, with no cleanup signals; this is not UI/App or native-KAS qualification.

Under separate Main GO, the CLI owner applied only shared formatter reuse in the actual example error arm plus normal multiline wrapping. Pre-source SHA256 `042b4623699df8e923b6e6790972538b02888e1fbd197e3513575e8a2671c288`; post-source SHA256 `42d991b5008f86c583bf65503310afd0c381c8eb2736511efb242def13f43c12`. No helper, allocation, interface or permanent wiring test was added. Unchanged probe `/tmp/cyril-tqr8-cli-error-prepared-RepairTqrCliEscapes-20261020.py`, SHA256 `41bac1cf68e50f401eb8953c687d5c72fec2c869677efaaabab8b7ddfbfc0e41`, then produced the owner's one Main-authorized actual GREEN:3.24seconds, probe exit0/example exit0, exact three MCP rows preserving printable quotes/apostrophe/backslash/Unicode/combining acute/ZWJ and literal escape-looking text while visibly escaping supplied controls/bidi. Result `/tmp/cyril-tqr8-cli-error-green-20261020/result.json` SHA256 `2216b12630aa3743881403aa2047629417c2b522129b8fc99564c4d91cced3ea`; actual example binary `80eace069898be6828aa884713b67018a13bd6dbee4cf1d525d8d42820fab4f0`; stdout.bin `64d433612d91270e343909fbba08fca7e99381093394cbac3778584198c623f8`. The retained build receipt `/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/tqr8-cli-error-repaired-example-build-attempt1/canonical-handoff.json`, SHA256 `8ca1392b249a98e1fba6d2db88af4d2a34ae0c5c0c1f3aa67ba9ff1f5e06bd48`, also lists App mcp_oauth/UI subagent/chat test edits since the original RED build. The runtime revision argument is a label, not a commit or only-one-edit claim. Client operations remain initialize/new only; CLI owner reports no cleanup signals and independent owned-root/exact-PID absence. Applicable final checks/review were pending at that original CLI handoff and are now complete within the final checkpoint below; this finite CLI proof is still not UI/App or whole-feature qualification.

**Standalone repair re-review (2026-10-21, retained09/d265 scope):** COMPLETE, correct with no findings/confidence0.97, covering MCP reveal/state/authority, scoped canonical/presentation/adoption interactions, both render geometries, both oracle corrections and actual CLI fidelity. The reviewer directly inspected every formatter hunk and withdrew an earlier mistaken comma-removal claim after reading the preserved fixture: exact tokens including punctuation stand. This is an ordinary independent scoped repair review, not isolated reconstruction or whole-feature/native/platform/CI acceptance. The [portable review](current-publication-evidence/pr193-repair/repair-review.json) retains that correction and source-bound applicability.

### Held-main integration applicability — 2026-10-22

The [held-main evidence owner](evidence.md#held-main-integration-2026-10-22)
supersedes the active standalone source pointer and not-yet-integrated
chronology. It owns the actual503 descriptor/held parent pins, exact fresh
Cargo commands/counts, subsequent one-shot consumer, archive/member hashes
and bounded cleanup facts. Three distinct layers must not be conflated:

- `agent://IntegrateTqrPrerequisite.ReviewHeldMainTqrIntegration`, selected by
  canonical `both_parent_static_review_scope_only`, found **no actionable
  source-integration findings; correct for scoped static integration review,
  confidence0.96**. Both-parent routing/reveal/canonical/dedup, gutters,
  incoming theme/cache/diff/selected-fit, CLI formatter and Peaq fixture
  behavior were preserved. That review itself makes no qualification PASS,
  fresh whole-merge-review or isolated-reconstruction claim.
- The qualifier's **fresh held-source Cargo qualification PASS** is empirical
  evidence separate from that static review. Changed UI production, App test
  composition, manifests/lockfile, Core test support and five incoming inputs
  invalidate automatic inheritance of standalone App/UI/workspace/Core gates.
- The **subsequent fresh503-bound owned-consumer PASS** is separate empirical
  evidence, not a rewrite of pre-consumer canonical/ready/HOLD fields or the
  historical standalone consumer. Main separately reviewed its binding-only
  diff before GO; retained recipe review applies only to unchanged case
  bodies, actors, fixtures, adapter and cleanup.

The ordinary standalone repair review0.97 and actual standalone results below
retain their original source/time/reach. Unchanged formatter/example mechanism
does not establish whole-production byte identity or fresh merged CLI
execution. Main-only cache/performance/C4/benchmark proof is not automatically
held-composition proof. Held source integration is qualified, but completed
merge/ancestry, commit/push, current PR-head CI/review, u62f Windows attribution,
native/auth/host/platform, downstream NK and final NK/C4/caar/C6 acceptance
remain unresolved/unclaimed. No source-review label or local PASS waives them.

### Final repaired checkpoint — qualification, review and owned consumer PASS

**Historical standalone checkpoint notice:** the “current” identities,
qualification, ordinary review, consumer and cleanup figures in the following
record describe2026-10-21 standalone09/d265, not held-main503. Its original
receipts and49-member archive remain unchanged; use the held-main evidence
owner above for the active checkpoint.

**Identity / portability:** current source is the498-row actual-input SHA256 descriptor `d265f295d35ba2eabe692d6000c3127771ded51f30865361b55cb86406bb7117`, not a Git tree/commit or whole-repo identity. Published base is still b750 plus uncommitted repairs. The final qualifier canonical is `/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/tqr8-scoped-notices-repaired-green-attempt3/canonical-handoff.json`, SHA256 `2ba47cce19d4f09deb62759d4d298b91b656eaa579496311000847207c9cbad0`,798044bytes. Portable selected qualification/history/review/consumer receipts, manifest/format proof and unchanged49-member consumer archive are allowlisted by [index](current-publication-evidence/pr193-repair/index.json). No product source snapshot or binary is added by this package. The nested archive retains the full canonical as evidence metadata, not a fresh qualification.

**Actual current code gates:** eleven successful commands ran on preformat descriptor64af: App MCP14/0/0; workspace2885/0/13; workspace all-targets Clippy; default all-bins build; isolated no-default App287/0/1, Clippy and all-bins build; isolated no-default UI862/0/2 and Clippy; explicit-KAS isolated App303/0/1 and Clippy. Overlapping test executions sum4351PASS/0FAIL/17ignored, not4351unique tests; ignored tests did not execute. The isolated no-default App selects13MCP cases and explicit-KAS App14; UI's dev Core KAS does not prove an isolated no-KAS Core lane. Unchanged199Core inputs retain separately pinned historical Core obligations. Seven earlier exact UI passes are retained, not individually replayed; all seven also actually PASS within fresh workspace/isolated UI suites.

**Fmt FAIL and actual correction, not erased:** original gate12 `cargo fmt --check` exit1/log SHA256 `b867a4f81ee3efba565d4c1897a2412ada32ca1379fce79610ba1fc5b5be0c4f` is historical ground truth. Main authorized one formatter invocation13 for four test-only reflows; no semantic fix or assertion/input weakening. Exact complete Rust tokens **including punctuation**, literals/assertions/cases and all three UI production prefixes remain identical; applicability record SHA256 `72af3b7be781cdc957a407713a10ca47ef85130f115bb3486b868d579e7c5a2a` retains the eleven actual preformat gates without full/variant reruns. Actual postformat gate14 fmt PASS/log `eb94d149ec801334b243662b080aacd78438b36c2ec6ae22616b253715f5dbd0`; gate15 incremental default all-bins PASS/log `b9cbfe3cf8299ba934af57e70b776efc255c209d6f2939a9009abe1b344275ee`. There were15Cargo commands,14exit0 and the one preserved fmt FAIL—not “all15passed.”

**Current independent review:** no findings, correct/confidence0.97; all four PR193 repairs resolved in approved scope. The original optional-field-comma-removal concern was retracted against actual preserved before/after bytes. Exact punctuation equality remains valid; no new fence/rerun or relaxed risk criterion followed. Review preceded current consumer execution; its then-unexecuted runtime fields remain historical, superseded for execution only by the distinct result below.

**Actual one-shot consumer:** final new private path `/tmp/cyril-tqr8-current-postfmt-tested-20261021`, binary SHA256 `691519208093d6b682fea20ce0cb366e01e15d39f739218c7b98d61e979a81f6`, actually executed once/no retry under explicit Main GO. Result SHA256 `4a6f0d2b89cb5c20eb9002b526fe607d2606cea1ff5cf03516d6901ac5dd17c2`; eleven140×40captures PASS, wire5client/67peer frames and native Kiro sessions0. Visible proof includes full retained/current URLs and escaped controls, ACK-not-authorization, durable main/child/workflow presentations with entire opaque tails, actual browse/follow restoration, chronology/dedup and exact draft/independent caret. Hidden canonical/pending/adoption/authority and supported focused renderer remain source/API-test evidence, not screen proof; no focus-entry binding or native gate-off workflow origin is invented.

**Cleanup precision:** owned root directly observed absent; zero marker matches is restricted to readable cwd/exe links. Global census had475permission/other errors per link; sameUID supplement had5EACCES+29ENOENT per link. Those opaque entries are not declared inspected/absent or attributed; no captured complete PGID/ancestry history or universal process-absence claim. The unchanged archive SHA256 `56aa494a80e05a5abf414447a5ff51847d82cdf0214e7c22472cd05496067231` preserves49explicitly selected members/270484bytes without product binary or private Kiro roots.

**Retained proof and current limits:** original14intendedRED/3newGREEN controls/4priorAppGREEN remain actual original results with their unreached postconditions; the two technical oracle corrections do not rewrite their reach. Separate CLI initial-drain RED/GREEN three-row proof remains applicable to unchanged example/probe and is not rerun or expanded to full-example/native execution. Prior a09/six-capture/AFAIL/partialB/C/native/auth-stop results retain their original identity and scope.

**Standalone chronology and current supersession:** at this2026-10-21 standalone checkpoint, PeaqPR194 was already **merged** at main `03838a501dad8be140acd78dc3fff4cae35fafef`, following refreshedhead991b16f9/baseUn4main7a CI39690453316GREEN/current review; its reviewer-CI prerequisite was resolved, not pending. Tqr b750-plus-repairs had not yet integrated main038. The subsequent [held-main addendum](evidence.md#held-main-integration-2026-10-22) now records source integration held and qualified; it does not claim Main's merge/ancestry completion, commit/push or current PR-head CI/review. PR193 CI remains old RED and u62f Windows attribution remains open despite green Windows. Qualified local NK checkpoint `ef46c43813d12b63f45b264e427abda84d299f46` is not proof of downstream propagation or final NK/native/C4 acceptance. Native KAS/auth/host/platform, Main-only cache/performance/C4/benchmark applicability and final NK/C4/caar/C6 acceptance remain unclaimed. Standalone raw index-only drift has unknown cause; no original logical-index identity or restoration is asserted. This docs/evidence amendment performs no source/Git/check/runtime/tracker/publication action and authorizes no merge, issue closure or shipping.


## Prior qualification decisions — 2026-10-20

| ID | Evidence state | Decision / disposition |
|---|---|---|
| URL/F1 — whole-string escaping changed printable apostrophes | Verified; original RED artifact444 and actual-source repair review artifact447 | Resolved by the streaming terminal-control/bidi-only formatter; retained unaffected substantive review plus fresh scoped repair review, correctness correct for reviewed repair, confidence0.98. No new whole-tree review or independently computed whole-feature source SHA is claimed. |
| Common/auth-retarget — generated executable/writable descriptor hazard and lock regression proof | Verified; parent bg118 deterministic six-case proof, portable `auth-retarget-proof-index.json` and receipts | Test-only common-base repair, independently reviewed confidence0.97; not a production auth change or identification of the historical descriptor holder. |
| Consumer/full-URL — right-edge characters covered by conversation scrollbar when long content is browsed | Verified original failure on pre-repair6e99; historical five RED/restored-GREEN pairs; fresh current four-test GREEN and owned B PASS | **Resolved within current local/owned-consumer scope.** The stable gutter/content rectangle and full-byte assertions are unchanged. All12 current Cargo lanes pass on a09; ReviewCurrentTqr finds no source defects, confidence0.96. Historical mutation receipts are not claimed rerun under the released pair. Native/CI evidence remains separately scoped. |

The failure was repaired by layout geometry, not URL shortening, a changed
reconstruction oracle, a larger terminal, relaxed byte comparison or relabelled
old FAIL. Current local qualification and the one current owned consumer PASS;
native retries/login/account/configuration changes remain prohibited.
Main owns staging, issue closure and PR/CI actions; none is claimed complete.

## Prior assembled-source review and publication boundary

ReviewCurrentTqr reports **no findings**, correct within the assigned source
scope, **confidence0.96**, for exactly the19 feature paths from prerequisite
tree699 to qualified tree `a09f1dfb63b17d380f4661823de431bfb5d72160`.
Its copied receipt is `current-publication-evidence/review/current-source-review.txt`.
The reviewer inspected current callback/capability gates, bounded ACK meaning,
opaque escaping, notification consumers, callback cleanup, gutter/four tests,
and both-sides integration. It did not run tests/consumer/native checks,
recompute all hashes, re-audit common mechanisms or review the later README
status-only documentation delta.

The separate canonical Cargo handoff records all12 requested lanes PASS; the
separate current owned B receipt records one invocation PASS on binary1d79,
all six captures and unchanged product assertions. Their exact source/binary/
command hashes and copied originals are in the owning evidence/index, not
manufactured as findings from the source reviewer.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| Current/preflight-method | Initial tree-versus-index source pin reported deletion of five known untracked files; use exact blob/mode comparisons before any run | VerifyCurrentTqrConsumer / Main adjudication | Verified | Original1041-line observation retained under `current-publication-evidence/method-failure/`; zero product invocations before correction; corrected preflight records exact bytes and100644 modes for precisely five files | Modify | Correct the evidence method only; no source/product/oracle change and no repetition of the failed command | non-blocking; method failure, not a failed consumer or product retry; no index-equality claim |
| Current/source-postimages | Old publication README/state hashes no longer identify the integrated source; preserve historical hashes and record the new source/docs identities | Integration receipt / delayed advisory | Verified | Integration records17 unchanged feature files and byte-exact README24/state18+77 additions retaining main; exact40-path qualified inventory and old manifest/index snapshots are portable | Modify | Current manifest separates historical snapshots, qualifieda09 postimages and the later README status-only before/after hash plus patch | non-blocking; no old source hash is relabelled current |

**Cleanup evidence limit:** current independent observation establishes actual
owned-root absence and zero post-exit exact owned-cwd/tested-binary markers.
Live observation captured only uv, not a complete descendant PID/PGID history.
No complete-process-group claim or observer cleanup signal is asserted.

The immutable raw consumer receipt's `fully_qualified:false` and `pr_ready:false`
limit its own consumer-only scope; the combined local qualification/owned
consumer/source review is ready for Main's publication actions, not proof of
native KAS/host, current NK/Caar, CI-all-features or shipping authorization.


## Delayed publication advisories

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| Advisory/archive-order | Index edit requires manifest refresh, archive rebuild and final sidecar; proposed strict order | Aggregated delayed advisory | Refuted | The final packaging operation added the publication-review receipt to the allowlist, recomputed every manifest entry including index, rewrote the manifest, rebuilt the archive and then generated its sidecar in that order. Final SHA is `162c5f4eb41096a58ce9421a3ccce978da734fec9c567e915b3bc3f5e9beb863` for that snapshot; a later documentation amendment produces a new archive/sidecar. | Reject | N/A — already satisfied before feedback | non-blocking; packaging construction is observed, but no additional archive audit/check is claimed or authorized. |
| Advisory/harness-identity | Edited synthetic-ID harness must not be misrepresented as the executed original | Aggregated delayed advisory | Refuted | The publication allowlist takes `controlled-peer-a-harness.py` from the executed output directory's immutable copy, not the subsequently edited `/tmp/cyril-tqr8-manual-owned-20261020.py`. The index explicitly describes the original run's identity-redacted projection and says the future peer must use synthetic IDs. The unexecuted edited variant is not packaged or claimed executed. | Reject | N/A — executed copy already preserved and distinguished | non-blocking; no rerun is needed to repair a misattribution that the delivered records do not make. |
| Advisory/gutter-coupling | Stable gutter must feed pre-wrap, count, paragraph and scrollbar geometry; short prefix tests cannot fence byte loss | Aggregated delayed advisory | Verified | Source diagnosis and approved repair below; historical sensitivity, current four-test GREEN/owned consumer PASS and current source review establish the correction | Modify | Consumer/full-URL fixed-gutter repair, `widgets/chat.rs`; no focused-subagent expansion | non-blocking now resolved within current local/owned scope; same product mechanism and acceptance, not a waiver |
| Advisory/native-identity | Original native session UUIDs must not enter the publication package | Aggregated delayed advisory | Refuted | Published decoded text/JSON captures replace original native session UUIDs with an explicit redaction marker; raw controlled ANSI was excluded to avoid partial redraw disclosure. Original private receipts remain unchanged outside the allowlist. The independent artifact reviewer found no actionable privacy-boundary issue. | Reject | N/A — privacy projection already applied | non-blocking; original private artifacts are retention evidence, not committed/publication inputs. |
| Advisory/scrollbar-mask | Treat overwritten URL characters as a capture artifact and mask the scrollbar in the oracle | Aggregated delayed advisory, later retracted by its source | Refuted | Original screen/source show actual covered characters, not a capture artifact; repaired layout retains exact bytes under the unchanged oracle | Reject | N/A — masking would manufacture a false PASS | non-blocking now the underlying layout defect is repaired and current owned PASS observed; preserve original FAIL and exact comparisons |

No extra product change or check was made for these delayed advisories.
Their originally blocking gutter defect is now resolved by the approved repair
and separately sourced current evidence; this log does not close the issue.

## Consumer/full-URL bounded repair

**Approval:** Main DG, 2026-10-20: “outerwidth>1ALWAYSreserve1rightcol
(notdependentoverflow/scroll/follow); usecontentRectwidthforallrender/stream/count/wrap/paint;
scrollbaronlygutter. width0/1fullcontent/noindicator.” Main explicitly authorized
source-only edits to the actual Tqr chat viewport owner, behavioral regressions,
README and issue evidence, with no URL-byte/protocol/auth changes. No new
consumer run is authorized until Main supplies a newly qualified binary and GO.

**Delivered behavior:** Only main-conversation `widgets/chat.rs::render`
changes: a stable one-column gutter for widths>1; the same content width reaches
committed-message rendering, streaming Markdown preparation, paragraph line
count and paint. Scrollbar paints into its separate one-column rectangle.
Widths0/1 retain full content width and omit the indicator. No allocation,
normalization, URL truncation, callback/ACK/auth change, new abstraction or
focused-subagent expansion was introduced. C6 owner applies the same contract
to its separate retained-layout worktree.

**Regression seam:** Four source-only tests exercise the real chat renderer:
`manual_url_bytes_survive_scrollbar_in_follow_and_browse` passes actual
`ExternalUrlRequested` through `UiState`, then requires the complete opaque URL
with printable quotes/backslash, café/wide Unicode and visible control/bidi
escapes under vertical overflow; `chat_gutter_width_is_stable_without_overflow`
fences unconditional geometry; `live_text_uses_the_same_reserved_content_width`
fences complete streaming text; and
`zero_and_one_column_chat_keep_available_content_without_scrollbar` covers
tiny geometry in follow and browse, with a four-row viewport and complete
content/outside-cell assertions. No prefix-only or masked-scrollbar oracle
replaces the complete URL comparison.

**Performed checks and results:** Tests were written before production changes;
the retained tests-only, layout-only and assembled patches record the
pre-format proof inputs. Parent attempt2 subsequently observed all four
unchanged tests behaviorally RED on old layout and GREEN after exact
restoration, then exact indicator-only width1 mutation RED/restored GREEN.
Bare HEAD without feature APIs and the earlier compiler-setup failure are not
behavioral RED. Raw logs/source pins are portable in
`publication-evidence/gutter-attempt2-canonical-handoff.json`.

Parent ran authorized formatting and workspace all-target Clippy successfully.
Full workspace tests then failed in
`runtime_drop_retains_evidence_until_core_completion` with `"missing EOF window"`;
2,834passed,1failed,13ignored/not executed are completed top-level counts
before stop, not whole-workspace/doc-test completion. Main expressly authorized
seven continued changed-path gates; all passed without repeating the failed
workspace or sensitivity pairs. `gutter-continued1-canonical-handoff.json`
records post-format source tree851c and partial candidate2954.

**Current resolution:** the historical missing-EOF workspace failure remains
immutable, not waived; after auth/EOF/main/rendering prerequisites, all12 fresh
assembled lanes PASS on a09. Historical B/C candidate2954 receipts remain
partial only; the one fresh owned B consumer now passes all six stages on
binary1d79. Original A FAIL and old five sensitivity pairs retain their source
pins. Main owns staging, closure, PR and any separately authorized native work.

## Fresh gutter-repair review — P3 tiny-test vacuity

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| Gutter/P3 | A width1 viewport of height2 leaves no scrollbar track after default arrowheads, so removing the width guard need not fail; use a taller overflowing viewport and assert all content/no-indicator cells | ReviewMcpUrl, fresh gutter review relayed by Main | Verified; parent empirical sensitivity subsequently observed | Static ratatui-widgets0.3.0 `scrollbar.rs:508–510` returns before painting when track length excluding heads is0; parent attempt2 later executed the exact indicator-only mutation RED/restored GREEN with unchanged content geometry | Accept | Test-only geometry correction in `zero_and_one_column_chat_keep_available_content_without_scrollbar`:3×6 backend, four-row inner viewport, six content rows, exact assertions over all18 buffer cells for widths0/1 and follow/browse. Production is unchanged for P3. | P3 discrimination is now observed; overall full-workspace failure remains independently blocking and unwaived. |

ReviewMcpUrl's fresh follow-on source review now finds P3 resolved by inspection,
no new actionable findings, correctness **correct for amended geometry+test
repair**, confidence **0.97**. Independently observed reviewed chat Git blob:
`7535e86096b0a32c8204c55667bd981396772e10` (`artifact://1156:2`).
The preparer-computed SHA256 is
`e750b9765c6843b9d649b9897f407992e4385cdfee81188e0c03d12465896b1c`;
the reviewer did not recompute it. Receipt:
`publication-evidence/fresh-gutter-ReviewMcpUrl.log`. This separate gutter
review does not replace earlier feature-fidelity0.98 or common auth-fixture0.97.
The ordinary URL/live/no-overflow tests remained unchanged. This source
review itself executed no checks; later parent RED/GREEN belongs to its
separate source-pinned attempt2 receipt, not to reviewer inspection.

Named parent-only sensitivity input:
`scrollbar-width-one-guard-mutation.patch` changes only the indicator guard
after content paint from width>1 to width>0, leaving the separate content-area
rule and all content width/row geometry intact. Width0 stays safe; width1's
arrows/track must replace expected existing cells. Parent attempt2 observed
that exact executed all-cell RED, then unchanged-test restored GREEN and exact
source restoration. Source review alone did not establish either result.
The original actual full-URL FAIL remains intact.

## Parent-applied canonical URL callback fixture cleanup

Main applied only the canonical `review-callback-cleanup-final.patch`
(SHA256 `f44db50c711041216ab6edc31669479dc4b179ebf49f912a484c5edc62e814c2`)
to actual Tqr `external_url_wire.rs`; no chat or production change.
Source-owner rationale and inspected canonical patch establish that the
`JoinHandle<()>` can finish normally after shutdown when KAS is absent.
Accept normal unit completion or cancellation within the existing whole-cleanup
five-second fixture allowance; panic/other errors still fail. Initialization
and exact callback outcome assertions are untouched.

Pre-format application SHA256:
`2b4e4eb80bff700ac0da74fbf5493b18a7756699c5a90d412490c2411a6a3599`;
owner-supplied pre-format Git blob `2ab281893c88998154d3fe5e8bfad021f3e65110`.
After parent's authorized formatter, publication SHA256 is
`01fccc081c3f9343b02e9b789b03de3f291a7ce130853989c7ab41100c1cf674`.
See `publication-evidence/parent-url-fixture-cleanup.log`. NK-only
`mcp_wire.rs` cleanup is explicitly excluded from Tqr. Original no-default
Caar cleanup FAIL remains parent-owned evidence, not a waiver or worker rerun.

No additional source application/Cargo/formatter/native run by this worker.
Parent's no-default exact callback case passed1/1; core full lane passed1,031
with4ignored/not executed and binary-package lane278 with1ignored/not executed.
These overlapping lane totals are not unique aggregate counts or full
workspace acceptance. Historical818/6e99 evidence remains unchanged.

## Historical owned consumer / partial publication checkpoint

Main explicitly authorized the exact WORKSPACEBLOCKED default-KAS partial
candidate2954, source Git tree851c, for controlled peers only. At the same
140×40 geometry, Main approved exact whole-URL comparison from the actual
139content columns while retaining140-column raw captures and separately
asserting scrollbar chrome. No padding, reinsertion, URI-byte deletion,
viewport enlargement or clipped-URL oracle was used.
The old captured continuation omitted `l`; the repaired capture actually
contains it, independently consistent with parent renderer sensitivity.

Both B and C consumers PASS. B's own `sess_tqr_owned_synthetic` was already
valid under the admitted KAS sess_ fingerprint shape. C's UUID-shaped own ID
is an additional fixture variant only: it finished before Main corrected the
preparer's mistaken interpretation of a UUID-suffix requirement. No criterion
was added, B is not invalidated, and no later run followed the clarification.
Original B/C executed harnesses/results remain unchanged; final C raw ANSI,
screens/content-cell records, wires and inputs are explicitly allowlisted.

Independent read-only owned-proof review: no findings, correct within scope,
confidence0.94. B wires/screens and the five expressly authorized C delta
files were inspected; C's other wire outcomes are receipt-backed. The reviewer
withdrew “requested UUID format” wording. C now observes owned-root removal
after temporary-context exit, resolving B's preassigned-flag limitation only;
no descendant/handle census, binary-hash recomputation, production-source
review, native/host/full-gate qualification or PR readiness is claimed.
Receipt: `publication-evidence/owned-partial-consumer-review.log`.

Original A navigation FAIL, historical workspace missing-EOF FAIL and earlier
native active31/32 FAIL remain immutable and separately scoped. Native raw ACP
frame/sourceURL were unavailable; peers use reconstructed envelopes and retained
safe fixture URL/error values. No native/auth retry, account/config change,
browser, clipboard, model turn or grant occurred. The current fresh local gates
and owned B PASS supersede the old current-blocker statement, not those receipts.


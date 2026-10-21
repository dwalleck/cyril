# cyril-tqr8 review decisions

Historical decisions and original failures below remain retained evidence, not the current gate. Current assembled-source qualification, independent review and the one owned B consumer PASS are recorded in `evidence.md`, delivery checkpoint, and `current-publication-evidence/index.json`. Publication actions remain Main-owned.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | The subagent-written loopback fixture has not executed; run py_compile and manifest/SIGTERM smoke before handing it off. | Delayed aggregated model advisory | Verified | The source handoff and `evidence.md` explicitly mark fixture execution pending. No py_compile/start-stop or integrated consumer run receipt exists. The delegated assignment explicitly prohibits worker build/test/lint/formatter/smoke checks and assigns verification to Main. | Reject | N/A — worker does not override the explicit verification-ownership contract. | non-blocking for approval-only handoff: parent must perform fixture qualification before relying on it for any live acceptance. This is not risk acceptance or permission to ship unverified code. |

Parent subsequently qualified the current fixture and ran the earlier native evidence. The 0.98 feature repair review and separate 0.97 common auth-fixture repair review have different scopes; neither is an empirical acceptance substitute. Their exact provenance and the later native-auth stop are in the current evidence record.

## Current decisions — 2026-10-20

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

## Current assembled-source review and publication boundary

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


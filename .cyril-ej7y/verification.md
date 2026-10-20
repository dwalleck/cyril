# cyril-ej7y verification index

Updated 2026-10-20. This is the single owning record for performed qualification, consumer evidence, sensitivity, review and portable publication inputs. Route/design/plan/review decisions reference it rather than keeping separate changing run totals.

## Delivered state and scope

Actual isolated delivery: `fix/cyril-ej7y`, parent `2fd94879db1b4281dc85cf7dcf528649c1da3087` ([PR #188](https://github.com/dwalleck/cyril/pull/188)). The nine-file EJ delta was reconstructed from the historical `02-cyril-ej7y.patch` onto that published parent; overlapping App/widget hunks were reconstructed by their owners, not applied unchanged successfully. The parent retained a source-frozen handoff before qualification. This finalization changes README and EJ workflow/evidence records only; no production, tests, Cargo, published-parent, tracker or later Shell/X paths are edited.

“Full comparison” means the complete **cleaned retained workflow `comparisonText`**: controls/bidi formatting become spaces, whitespace collapses, ends are trimmed, and the entire value including prefix is capped at 600 Unicode code points, with `...` appended only when cut. It does not mean full file Diff/Myers output, expanded Diff cache, or recovery beyond that reference cap. The approved 12-visible-body-row bound includes navigation status. Tree/fallback omissions remain explicit. Ordinary Text/raw-text five-plus-marker and ordinary Diff twenty-plus-marker limits are distinct and unchanged.

The archive's `source/freeze-manifest.json` and publication manifest identify all nine frozen source files by SHA-256. These are **documentation-freeze hashes, not contemporaneous gate-time hashes**. No gate-time nine-file hash manifest or full environment/toolchain-version dump was retained. Provenance instead includes the original gate invocation/completion, actual worktree compilation paths, reconstruction handoffs, historical patch SHA-256 `443bab1e7af1a80712bb3922536cc90491c8e4561a4053c2fe34c7709b68877e`, and preserved tested executable SHA-256 `0e6cf27b8b32557fd3247b2379f873d5e7f2322a2051b3d4c6be7f4d6b2b066a`. No executable is packaged.

## G1 — actual isolated repository qualification: PASS

Parent-run `bg_101`, original `artifact://647`. Invocation at `2026-10-20T07:16:04.525Z`; completion message `dc1d2765` at `2026-10-20T07:20:07.540Z`. Portable raw output: `gates/artifact647.log` (3,320 lines); exact arguments, job linkage and explicit environment settings: `gates/provenance.json`.

Original command, run from the actual EJ worktree:

```sh
env -u CARGO_TARGET_DIR cargo fmt --all &&
touch crates/cyril-core/src/lib.rs crates/cyril-ui/src/lib.rs crates/cyril/src/main.rs &&
env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo test --workspace &&
env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo clippy --workspace --all-targets -- -D warnings &&
env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo build -p cyril &&
env -u CARGO_TARGET_DIR cargo fmt --all --check
```

| Obligation | Retained result |
| --- | --- |
| Workspace tests, including EJ renderer/App/state/fallback/cleaning/snapshot and existing approval/layout regressions | PASS; parent completion summary: 3,008 passed, 13 ignored. Raw suite output is retained. Ignored cases are not relabeled as executed. |
| Workspace clippy, all targets, warnings denied | PASS; chained command reached the later default build and final fmt check. |
| Default `cyril` build | PASS; preserved executable digest matches S1. No V2-only/no-default-features or native-platform claim is inferred. |
| Final `cargo fmt --all --check` | PASS as reported for the completed chained gate; the initial formatting operation is included in the original command above. |

The completion record has no separately retained numeric exit-status field for `bg_101`; no numeric gate exit code is invented. This is retained parent qualification, not a fresh run by the documentation worker. Applicability: Main confirms no production changes since the exact G1-qualified source; docs/receipts do not change the exercised implementation or fixtures.

## S1 — actual owned-process smoke: PASS

Canonical original summary: `local://ej7y-owned-permission-smoke.json`; full receipt: `/tmp/cyril-ej7y-owned-smoke-20261020-a/result.json`. Portable copies are `smoke/summary.json`, `smoke/result.json`, and the complete smoke directory. Actual Cyril launched with its normal KAS production conversion path and an owned fake ACP peer, not helper-injected domain approvals.

**First actual runtime attempt:** 24/24 permission requests/responses (`8000`–`8023`), exactly one offered manual `RejectOnce`/`optionId: reject` response each, exit0. Every captured navigation checkpoint asserted the originating permission remained unanswered. Final reasons were exactly `first\nsecond`, `queue-only`, and `why` for their respective cases; the chat draft `D` remained intact.

| Design claims / cases | Measured coverage |
| --- | --- |
| C1: retained comparison, not four-row cap | Long comparison reconstructed from status-indexed drawn rows: four option pages/39 projected rows at 32×24 and three reason pages/28 projected rows at 40×40; `chg11` retained, `chg13` excluded; 600 retained code points plus truncation suffix. |
| C1: 599/600/601 boundaries | ASCII and wide fixtures all reached their retained tails: three ASCII pages and five wide pages each. 599/600 retain `TAIL_ID`; 601 retains `TAIL_I...`, not full `TAIL_ID`. |
| C2: body budget/context | Every plan body capture is at most 12 rows including status; pinned path/choice/editor and input remain outside the scrolling body. At 40×14, one content row remains while the hint moves to the border. |
| C2/C3: navigation, modifiers and resize | Unmodified PageUp/PageDown, exact Shift+Up/Down/Home/End; ineligible modifier probes; first Shift-Up/PageUp normalize stale offset after resize. At 80×40 the endpoint is `4–14/14`: 11 content rows plus one status row. Ordinary selection and multiline reason movement remain separate. |
| C3: reason/drafts/request locality | Position retained through reason entry; reason text/cursor and chat draft survive preview keys. Next already-queued request starts at row1 with an empty reason editor. |
| C2/C4: tree and fallback honesty | Explicit first tree omission row; typed tree at 12 rows; details-omitted annotation. Plan Text and raw-text input lengths 11/12/13/25 produce 11/12/12/12 total rows; 13 becomes 11 content plus `...2`, 25 becomes 11 plus `...14`. Malformed typed plan retains the plan fallback budget. |
| C4: independent ordinary controls | Ordinary Text/raw-text: five content rows plus marker (six total). Ordinary Diff: 20 diff lines plus marker (21 total), not comparison/file-diff expansion. |
| C1–C3: narrow geometry | Widths4–10 cannot retain hidden navigation offsets in option/reason phases. Usable width11 ASCII and width12 wide fixtures reach `TAIL_ID` and the retained fill glyph without exceeding the body budget. |

Portable inputs and observations: exact `harness.py` (SHA-256 `3319d9faffcf5665078702b5e9d74933763691ece880a03d1f0ffc46ab77ebbb`), `specimens.json`, both wire journals, `observations.jsonl` with geometry and raw ANSI byte offsets, all **222 numbered decoded-screen captures**, `terminal.ansi`, `final-screen.txt`, `cyril.log`, control input, safe fake-auth helper and its captured argv. Capture names are indexed in `observations.jsonl`; representative images are text captures, not native screenshots.

Before the first launch, independent static review corrected two stale reason-phase observation predicates to require reason-only `Rejecting: Deny` rather than an option-page hint. Scoped static rereview was correct/confidence0.99. Preparation/rereview receipts are archived. That checker preparation is not product RED, and no runtime failure was discarded or reclassified.

## S2 — actual old/new retained-content sensitivity: PASS

Canonical receipt: `local://ej7y-bounded-sensitivity.json`, portable `sensitivity/summary.json`. This bounded run was obtained **after** G1/S1, not invented as an earlier test-first RED. The earlier `artifact442` E0308 was a boxed workflow-plan fixture compilation error before behavioral execution; the pre-launch static smoke-predicate repair was also not product RED. No baseline-failure waiver or risk acceptance is inferred.

Both binaries consumed the exact first three retained S1 wire frames (start, pending update, permission8000), including the unchanged typed workflowPlan/44 steps and Text comparison. Identical `fixture.json` SHA-256: `560a83159304ca9c28e8ae644e58bf48dfa4cd4607a76bc2088bdba8a89e0226`. Both ran at32×24 with chat draft `D` and the same three PageDown keys followed by Shift+End. A final ordinary Down/Up visibly selected Deny then restored Allow, proving input consumption while keeping the request unanswered.

| Executable | Observed product behavior | Probe / cleanup |
| --- | --- | --- |
| Preserved old-parent binary SHA-256 `e2a8fb92972b144e3101bcc571dbbf817277527bd289769215c96cdb7013b2a2` | BEHAVIORAL_RED: retained late ID `chg11` (zero-based position592, inside the600-point reference) absent from all six body captures after navigation; body remained tree plus initial comparison text. | Intended probe exit1; actual Cyril cleanup exit0. |
| G1-qualified EJ binary SHA-256 `0e6cf27b8b32557fd3247b2379f873d5e7f2322a2051b3d4c6be7f4d6b2b066a` | GREEN: actual drawn `chg11, c...` reached after the third PageDown and retained after Shift+End and the input-consumption fence. | Probe exit0; actual Cyril cleanup exit0. |

Both preserved target path/Allow/chat draft and emitted zero permission responses before cleanup. This establishes actual late-content reachability, **not merely presence of navigation status**. Exact `probe.py` SHA-256 `040c1c5a870c802ad3962bcf808b879c46e9fcb90db69b24f44b102c42faa0c1`, fixture, result, wires, six screen captures per binary, geometry/ANSI byte-offset observations, ANSI and logs are retained under `sensitivity/old/` and `sensitivity/new/`. Neither probe was discarded or rerun; S1's24-case harness/evidence stayed unchanged. Coverage is one real retained-comparison defect, not historical RED proof for every fallback/Unicode regression and not a native glyph/live-KAS claim.

## Independent source review

Historical R1: `ReviewApprovalDiff` returned no findings/correct/confidence0.91, but inspected scoped EJ behavior in the original combined tree against `8e5c325e`, before reconstruction onto PR188. Portable `reviews/ReviewApprovalDiff.txt`, assigned-scope provenance and navigation subreview preserve that actual identity; they are not a new review of G1's reconstructed source.

**R2 — fresh exact reconstructed-source review: PASS.** Canonical receipt `local://ej7y-final-source-review.md`, portable `reviews/ExactEjSourceReview.txt`; exact full-index nine-file diff `artifact://711`, portable `reviews/artifact711-exact-nine-file.diff` (nine files,1094 insertions/47 deletions). ReviewApprovalDiff found no actionable findings: correct, confidence0.94. Its independent consuming-dispatch subreview also found none (`reviews/ExactEjDispatchReview.txt`). The receipt identifies each resulting source Git blob above published D base `2fd94879`; that base is **not an EJ commit**. It reviewed the actual uncommitted nine-file delta, full retained comparison/row budget/omissions, exact key/geometry/offset/confirmation chain, inherited D path metadata/grapheme10×11 acceptance and migrated constructors. No core/Cargo/Shell/X/file-Diff scope expansion was found. No code checks or runtime runs were performed by the reviewers; their then-pending sensitivity reference is discharged separately by S2, not claimed as reviewer-exercised proof.

## Safety and measured limits

- Actual process, production request conversion, permission wires, decoded text and ASCII-box geometry were exercised. PTY/pyte does **not** prove native Unicode glyph fidelity, live upstream KAS behavior, or native macOS/Windows operation. Exhaustive scalar/width/modifier/later-phase regressions belong to G1, not narrower smoke claims.
- Only owned fake peer/auth helper and isolated HOME/config/data/workspace were used. No real account, workflow execution, file/terminal callback, ShellCommand scenario or durable permission grant occurred; the process was stopped after the run.
- The portable archive contains text scripts, inputs, output and provenance only: no Cyril binary, build directory, symlink, private HOME/config/data/workspace directory or real credential store. Original absolute path strings remain in raw logs as provenance; referenced directories are not packaged. `AT-fake` and profile `FAKE` are deliberate synthetic fixture values.
- Reading the archive needs no original absolute paths/account. Repeating a run needs a separately supplied matching executable, repository fixture, Node and Linux/POSIX PTY plus uv/pyte; the archive includes the exact script and an explicit example, not a claim of cross-host reproducibility already exercised.

## Portable archive and publication boundary

Frozen archive: `.cyril-ej7y/evidence-20261020.tar.gz`, rooted at `cyril-ej7y-evidence-20261020/`. `ARCHIVE-NOTES.txt` explains provenance, reading/rerun prerequisites and exclusions. `MANIFEST.sha256` hashes every other regular archive member. `.cyril-ej7y/publication-manifest.json` owns exact publication membership, byte lengths and SHA-256 values, including the final archive digest; its own SHA-256 is returned separately to avoid self-hashing recursion.

Archive SHA-256: `11b1e929a6d7a9a28508518d177b66e2e8917e9832d0d6e544834f2c423b5cbf` (215,138 bytes;283 regular files,282 payload hashes plus the checksum manifest). `packaging.json` records regular-file-only assembly and byte-identical copying of265 actual gate/review/smoke/sensitivity files. Static inventory found no unrelated payload or apparent live credential; packaging metadata checks address that scout's unavailable lstat/whole-file comparison, not an additional product-code gate.

G1, S1, S2 and R2 discharge this issue's local implementation/regression, actual consumer, retained-comparison sensitivity and independent source-review obligations without an unresolved product finding or evidence waiver. Source/docs/receipts are frozen for Main's publication. Parent still owns final assembled approval, commit, own issue PR, remote CI and tracker updates; this documentation task neither performs nor claims those actions. No source edits or code-check reruns are authorized by this record.

# cyril-fiv1 — v2 failed-validation tool rows

Date: 2026-10-19. Base for this stacked issue: cyril-1v25 shared repair, starting from main 8e5c325e. Owner: MissingToolResults; integration/verifier/review/publication owner: Main.

## Request and outcomes

Authoritative request: full `rivets show cyril-fiv1 --json`. A v2 `tool_call_chunk` followed by a failed `tool_call_update`, with no start frame, must render one failed call with its validation error rather than disappear. Reuse the shared unknown-update insertion from cyril-1v25; do not invent a second chunk-synthesis state machine. Chunk-only traffic retains existing activity behavior. Qualify both committed captures, including the unrelated interleaved chunk in 2.4.1 and the surrounding streamed agent text in 2.11.0.

## Route tests

| Test | Verdict and evidence |
| --- | --- |
| T1 — empirical premise | No unresolved consequential premise. Captures show the complete rejected-read payload: 2.4.1 physical lines 5244 (target chunk), 5245 (another ID's chunk), 5246 (target failure), and 2.11.0 lines 52–53 (chunk/failure). Both failures carry validation text in `content`, no `rawOutput`, and no start. Existing production v2 engine converts the extension chunk and standard failure. The issue's line 5245 points between the actual target frames, resolved from repository evidence. |
| T2 — structural module shape | No. Shared row insertion stays owned by UiState in the prerequisite issue. This issue adds v2 production-conversion test replay within the existing core test-support module, downstream capture/render qualification, and the residual UI presentation fix: failed tools must also expose text content when raw output has no usable error. No production signature, schema, responsibility ownership, or dependency direction changes. |
| T3 — production-scale risk | No. Two bounded capture fixtures, ordinary test execution, and existing error text extraction/rendering. No production concurrency, queue, or volume change. |
| T4 — explicit behavior | Yes. The failed call and exact validation message must be visible once; chunk activity and ordinary known-ID merging stay unchanged. The issue explicitly permits synthesis from the failed update, selected by the shared repair. |

Selected route: **Local** (T1=no, T2=no, T3=no, T4=yes).

## Distinct PR boundary

The cyril-1v25 PR owns shared state production code and its KAS fences. This stacked PR owns genuine Rust/v2 converter replay (not the misleadingly named KAS `kas-v3c-v2` capture), two capture-derived failed-validation fixtures/tests, and content-backed failed-tool error presentation. It remains a substantive consumer qualification/residual fix, not an empty PR or duplicated shared patch. Parent owns commits, reviews, tracker updates, and publication.

Only workflow artifact required: this route. Spec/design/prototype/plan are unnecessary for Local changes determined by the existing contract.

## Verification contract

- Replay the original `_kiro.dev/session/update` chunks and `session/update` failures through **V2Engine**, not KAS conversion or hand-built domain notifications. Preserve source line provenance in fixture comments/tests.
- Assert one matching failed row, ID attribution, original raw input, exact error text in state and the actual chat widget, and surrounding text order; chunk alone must not fabricate a completed/failed row.
- Pre-fix sensitivity: with both fixes absent the row assertions fail; with only shared cyril-1v25 insertion present, the error rendering assertions fail because the captures have text content but no raw output. Run the unchanged test at both boundaries where practical.
- Real TUI smoke: owned ACP peer feeds original v2 chunk/failure payloads to the actual built Cyril process; inspect the rendered failure text and chronology. This is fixture-backed end-to-end wiring proof, not a claim of new live upstream behavior.
- Parent runs focused regressions, inherited known-ID/cancellation merge tests, applicable repository gates, real TUI smoke, independent issue code review, and post-smoke permanent docs/changelog update before publishing the stacked PR.

## Completed edit slice

- `crates/cyril-ui/src/traits.rs`: `TrackedToolCall::error_message` preserves raw-output error/output precedence and falls back to the first nonblank text content only for failed calls. This repairs the genuine residual after shared row insertion.
- `crates/cyril-core/src/test_support.rs`: `v2_trace_to_routed` replays inbound `dir: "in"` captures through the actual V2Engine standard and extension converters. ACP stays confined to core.
- `crates/cyril-ui/tests/v2_failed_validation.rs`: both rejected-read captures, interleaved chunk identity, chunk-only activity, exact validation error rendering, failed status, raw input preservation, chronology, raw-output precedence, and completed-status exclusion.
- `crates/cyril-ui/tests/fixtures/v2_failed_validation_2_4_1.jsonl` and `v2_failed_validation_2_11_0.jsonl`: selected original frames, no fabricated starts. The shared smoke harness belongs to the preceding PR.

Parent commands from this worktree (`BIN` is the absolute freshly built binary; use fresh output directories):

```sh
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --test v2_failed_validation
uv run --script .cyril-1v25/smoke.py --binary "$BIN" --engine v2 --capture crates/cyril-ui/tests/fixtures/v2_failed_validation_2_4_1.jsonl --expect "The tool arguments failed validation:" --output /tmp/cyril-fiv1-241-smoke
uv run --script .cyril-1v25/smoke.py --binary "$BIN" --engine v2 --capture crates/cyril-ui/tests/fixtures/v2_failed_validation_2_11_0.jsonl --expect "The tool arguments failed validation:" --output /tmp/cyril-fiv1-2110-smoke
```

Main exercised residual sensitivity with the shared `state.rs` repair, new v2 replay helper/tests/fixtures, and original `traits.rs`: both v2 tests failed with missing error text (`None` instead of the captured validation error), artifact 87. This isolates this issue's content-error repair from the prerequisite row insertion.


## Recorded parent evidence

Checked checkpoint state: base `8e5c325e` plus the uncommitted shared state repair, this issue's `traits.rs` error fallback, `crates/cyril-core/src/test_support.rs` v2 helper, the two integration test files, and three capture fixtures; before the later F1 session-boundary repair. Common assembled test/clippy/build proof is owned by `.cyril-1v25/route.md` **Recorded parent evidence** (artifact 92), not a new commit hash or duplicate run claim.

| Obligation | Result and owning receipt |
| --- | --- |
| Residual regression sensitivity | PASS — expected RED with original error extractor, both tests failed; artifact 87, as recorded above. |
| Repaired v2 consumer tests | PASS — both v2 tests passed in Main's artifact 92; exact captured errors and chronology were asserted. |
| Real-process v2 TUI, 2.4.1 | PASS — original outputs `/tmp/cyril-fiv1-241-smoke-shell-20261020/{result.json,screen.txt,terminal.ansi,agent-wire.jsonl}`. Portable result/screen excerpt: `receipts/smoke-v2-2-4-1.json` and `receipts/smoke-v2-2-4-1-screen.txt`. |
| Real-process v2 TUI, 2.11.0 | PASS — original outputs `/tmp/cyril-fiv1-2110-smoke-shell-20261020/{result.json,screen.txt,terminal.ansi,agent-wire.jsonl}`. Portable result/screen excerpt: `receipts/smoke-v2-2-11-0.json` and `receipts/smoke-v2-2-11-0-screen.txt`. |
| Permanent user documentation | Delivered after smoke — the separate README paragraph beginning **On v2, a tool rejected during argument validation** documents failed rows/error content and chunk-only activity. Stage this hunk with fiv1, not the common Tool call history section in 1v25. |
| Issue review | Main reports ReviewMissingV2 found no blocking findings for this checkpoint. The shared prerequisite has blocking F1; its repair/review must finish before this stacked PR publishes. |

Both TUI smokes used the actual built binary and controlled ACP peer with session scopes normalized to its advertised v2 main session. Main supplied `SHELL=/bin/bash` after diagnosing the harness-only private-PATH startup failure. These are fixture-backed UI/wiring receipts, not new live-upstream or Windows/macOS claims.

## Historical isolated-stack verification

COMPLETE — The prerequisite is reviewed commit `a5d2c4b5` / PR #183, including the cross-session identity repair and passing independent re-review. This issue's error fallback, v2 replay helper, fixtures and assertions are unchanged from the independently reviewed slice: ReviewMissingV2 found no blocking findings.

Main verified the isolated stacked `fix/cyril-fiv1` tree. Artifact 368 records successful `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build -p cyril`, and `cargo fmt --all --check`. Compilation paths identify this delivery checkout; local crate entrypoint mtimes were touched before switching the shared target to prevent stale sibling-artifact reuse. Build commands set `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`; formatting unsets the ambient empty variable.

Both actual-process v2 smokes then passed on that freshly built binary, using the commands above with output directories `/tmp/cyril-fiv1-final-241-20261020` and `/tmp/cyril-fiv1-final-2110-20261020`. Each capture rendered its exact failed-validation message once, between the chronology markers. Portable evidence: [2.4.1 result](receipts/final-v2-2-4-1.json), [2.4.1 terminal](receipts/final-v2-2-4-1-screen.txt), [2.11.0 result](receipts/final-v2-2-11-0.json), [2.11.0 terminal](receipts/final-v2-2-11-0-screen.txt).

The original residual RED remains valid: the same tests observed missing error text with the shared insertion fix but original error extractor (artifact 87). The README's v2 validation-error paragraph describes this separately mergeable residual fix. No live-upstream or native Windows/macOS qualification is claimed; no blocking local check or review finding remains.

## Refreshed published-stack verification — 2026-10-20

**COMPLETE for refreshed source HEAD `5e523ebf10978cc990746934732e4256596244ff`, tree `7ba80340f2e37516df31d964454126c12a22df36`.** This refresh inherits the approved successful-output alias from PR #183; it does not duplicate that production repair or change this issue's Failed-only content fallback. These are the qualified source pins before the documentation-only evidence packaging below. Earlier REDs and verification files remain historical and are not relabeled as refreshed-head proof.

The separate [refresh verification record](refresh-verification.json) binds the source, binary, full gates, independent review, and controlled-peer consumers. The private binary SHA256 is `e3f072bcd89cc8e1bf935fb52912035ffb273ac913cd1c31f40c225405534db1`; no binary or private HOME/config/data directory is published.

| Refreshed obligation | Evidence and result |
| --- | --- |
| Full repository gates | [Qualifier receipt](receipts/refresh-20261020/full-gates.json) and its five adjacent command logs: initial `cargo fmt --all`, workspace/all-targets clippy with `-D warnings`, workspace tests, `cargo build -p cyril`, and final fmt check all exited 0. Workspace totals are **2826 passed, 0 failed, 13 ignored**, excluding nested child summaries from double counting. |
| Approved KAS completion consumer | Exactly one actual-process replay of the approved sanitized `0d613a49…` capture: completed header and `| XAVT_BEGIN XAVT_END` body each rendered once, in order between the markers. |
| Both v2 validation captures | Original selected 2.4.1 and 2.11.0 messages rendered one exact failed Read/error each. The unrelated 2.4.1 chunk did not create another row; 2.11.0 retained its surrounding streamed-text chronology. |
| Raw-error/content/status controls | Eleven explicitly synthetic owned cases in three runs covered first-nonblank content, unknown raw-object fallback, raw string/error/message precedence, stderr and direct-output fallback, and Completed/Pending/InProgress exclusions. Completed Read retained its six-character summary rather than exposing error/content/raw text. All **18 negative screen predicates** held. |
| Consumer identity and deduplication | [Six-run receipt](receipts/refresh-20261020/consumer.json): all six first-attempt runs passed. Full original parsed messages and captured session/new identities were preserved. Each tool update was deliberately replayed twice, other original messages once; visible rows remained unique. Synthetic controls used owned IDs, never private MCP IDs. |
| Independent code review | [Verbatim original review](receipts/refresh-20261020/review.txt): no findings, correctness **correct**, confidence **0.97**. Its joint fiv1/x5xi pins are retained; this issue relies on the fiv1 delta from `7eefe275fbec97f610677b2ee6433af996806599` to the refreshed HEAD above, not the later separate Zj verdict. The reviewer did not independently verify the tree hash or run qualification. |

Full-gate source-tree provenance comes from the qualifier, separately from the static review. The qualifier's historical “Main GO required” fields predate Main's subsequent bounded controlled-peer GO and completed consumer runs. Qualification is default-feature Linux only: no all-features, benchmark, live-upstream account/model, native-terminal glyph, Windows, or macOS claim is made.

The consumer proof used the actual pinned Cyril process under PTY with owned ACP peers and pyte-decoded text. Captured raw commands remained inert JSON; no permission or terminal requests, payload execution, or grants occurred. Owned-process cleanup completed. Packaging reran no source checks or smokes and changed no application code.

### Portable refresh evidence

The [consumer archive](receipts/refresh-20261020/consumer-evidence.tar.gz) has SHA256 `e2d554c5910bb54af53bf2f57d34d4f08224d070500b27a73b07ad04a727e07d`. It contains all six result/screen/wire/ANSI/log sets, exact allowed capture copies, owned synthetic inputs, a byte-identical frozen harness/fake-CLI layout, and `qualification.json` with per-case replay argv, input hashes, positive counts, and negative predicates.

Extract it into a fresh owned directory and enter the included `cyril-fiv1-refreshed-consumer-20261020` directory. Each `case_results[].replay_argv_from_packet_root` uses packet-relative input/harness paths, an external binary, and a fresh output directory. The recorded binary/output paths identify the qualification host; substitute them explicitly when reproducing elsewhere. The copied harness was not separately launched: the recorded runs used its byte-identical pinned original. Negative predicates must also be inspected in the resulting screen; the harness itself checks positive expectations and frame preservation.

Only issue-scoped documentation/evidence under `.cyril-fiv1/` is packaged. The exact publication allowlist and SHA256 hashes are supplied separately in `local://fiv1-refresh-publication-handoff.json`; earlier receipt files and tracker records are unchanged.

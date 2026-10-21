# cyril-tqr8 implementation plan

Approved design: `design.md` Approval (DG, Main, 2026-10-19). Source premises: `evidence.md`. Existing responsibility boundaries and manual URL behavior are unchanged by this plan. No feature budget or new dependency is proposed.

## Slice 1: Supported URL callback and visible MCP failure

**Outcome:** A bound KAS process can request actual URL surfacing through its advertised real callback; Cyril displays the manual URL and acknowledges accepted delivery. MCP connection display-errors reach the visible consumer without falsely ending a turn. v2/no-KAS callback refusal and existing v2 MCP behavior remain intact. Design C1–C5.

**Changes:** One atomic, caller-complete slice across core and UI. Core owns new typed `Notification::ExternalUrlRequested { url: String }` (no invented server/session fields) and `Notification::McpConnectionError { message: String }` (variant supplies the error classification), concrete external-URL adapter presence, host request parsing/dispatch/delivery acknowledgement, KAS display-error conversion, and core/exhaustive caller migration. App/UI own only manual-link and durable error presentation through the typed events. Core and consumer edits can be prepared independently under this exact notification contract but form one assembled slice. Generic HostMediator remains capability-agnostic. No browser/clipboard/secret backend/dark completion path is introduced.

LSP references were attempted before extending exported Notification: rust-analyzer reports ready but references and hover for the linked-worktree enum both returned none despite known usages. Reported through `xd://report_issue`; code owner must investigate relevant exhaustive consumers with bounded textual fallback rather than interpreting the empty result as no callers. Dynamic registration and wire names also need text search.

**Verification:**
- Parent observes actual pre-fix red then repaired green for SDK callback/adapter refusal-reachability and KAS display-error-to-consumer regressions → real dropped-callback/error sensitivity, not only helper assertions.
- Parent runs default and KAS core suites plus consumer/UI tests, workspace formatting/type/lint/build gates → caller completeness and existing behavior preserved. Use `env -u CARGO_TARGET_DIR cargo ...` because the ambient variable is empty.
- Parent qualifies `.cyril-tqr8/fixture.py` before using it, then runs the real consumer against clean-root loopback MCP OAuth/failure endpoints → actual URL/error visible, no browser/clipboard/secret writes, no callback/reset acknowledgement mistaken for authorization, owned descendants shut down.
- Integrated passive/reset/status completion C4 waits for the substantive nk4o PR, whose own consumer work must remain distinct. Standalone tqr8 must still exercise its real callback/error entry points without requiring that panel.
- Parent obtains ordinary independent code review for this issue and opens its own substantive PR; blockers are resolved before publication.

**Dependencies:** No unresolved external premise for this slice. Worker explicitly skips tests/build/lint/formatters/smoke; Main owns all checkpoint judgments, fixture qualification, docs/changelog after successful smoke and publication. That ownership transfer is not a PASS or shipping waiver.

## Delivery increment

One tqr8 issue PR containing the entire slice. Approved subsequent stack is nk4o then caar, whose absent-inventory acceptance decision is now explicit in its owning spec. No commits/pushes by worker.

## Checkpoint record

**Delivered edits:** Concrete adapter-derived real URL callback parsing/dispatch, bounded accepted-delivery acknowledgement, malformed/closed-path and non-KAS refusal, flat-authoritative MCP display-error conversion, typed global URL/scoped error consumption through durable system messages (including existing peer stream and binary example). The UI's tiny private streaming text formatter preserves printable URL characters/Unicode exactly and visibly escapes only controls/bidi without an intermediate allocation; its own regression tests cover that distinction. No browser/clipboard/secret backend/dark completion or turn-completion effect. Core/consumer workers added wire/domain/App/render behavior tests but ran no checks.

**Publication boundary:** The older per-file patch series below this directory records the original `8e5c325e` assembly, not the current delivery source; do not apply it as a final-base publication recipe. Current local qualification is pinned to assembled source tree `a09f1dfb63b17d380f4661823de431bfb5d72160` over prerequisite tree `699256c2b0dadf4000f72f6beb178b15ae8d1b90`, with 19 feature paths and the approved gutter regressions/callback cleanup unchanged. The [owning current evidence](evidence.md#delivery-checkpoint-2026-10-20) links the exact source allowlist, integration receipt, [qualification handoff](current-publication-evidence/qualification/canonical-handoff.json), [source review](current-publication-evidence/review/current-source-review.txt) and [consumer artifact index](current-publication-evidence/consumer/artifact-index.json). The old `tqr8-publication-evidence.tar.gz` remains immutable and separately scoped. Current publication index/archive packaging is parent-owned and not asserted complete here. Publication documentation updates do not assert a rerun or a newly qualified whole-tree hash. Parent owns final staging, commits, tracker disposition and the actual PR; none is claimed performed here.

**Historical Main sensitivity recipe (retained, not a current rerun):** The commands below record the earlier callback/error checkpoint recipe: transplant only `external_url_wire.rs`, `mcp_error.rs` and their test-module registrations onto pristine production (they do not reference new enum variants), observe red, then run the repaired stack green. Actual outcomes and source pins remain in the owning evidence, not this recipe. The separate five historical gutter sensitivity pairs were not rerun for the current ratatui pair.

```sh
env -u CARGO_TARGET_DIR cargo test -p cyril-core --features kas --lib protocol::domain_mediator::tests::external_url_wire::external_url_callback_wire_result -- --exact
env -u CARGO_TARGET_DIR cargo test -p cyril-core --features kas --lib protocol::domain_mediator::tests::mcp_error::kas_mcp_display_error_preserves_conversion_result_and_message -- --exact
env -u CARGO_TARGET_DIR cargo test -p cyril-core
env -u CARGO_TARGET_DIR cargo test -p cyril-core --features kas
env -u CARGO_TARGET_DIR cargo test -p cyril-ui external_url_request_is_manual_and_nonterminal
env -u CARGO_TARGET_DIR cargo test -p cyril-ui mcp_connection_error_is_durable_and_nonterminal
env -u CARGO_TARGET_DIR cargo test -p cyril-ui mcp_notice
env -u CARGO_TARGET_DIR cargo test -p cyril --bin cyril app::tests::mcp_oauth
env -u CARGO_TARGET_DIR cargo test -p cyril --no-default-features --bin cyril app::tests::mcp_oauth
env -u CARGO_TARGET_DIR cargo check -p cyril --all-targets
```

**Performed checks and results:** The [owning current evidence](evidence.md#delivery-checkpoint-2026-10-20) and retained [qualification receipt](current-publication-evidence/qualification/canonical-handoff.json) are authoritative. On assembled source tree `a09f1dfb63b17d380f4661823de431bfb5d72160`, all 12 local Cargo lanes PASS: default full-workspace tests (2,869 passed/0 failed/13 ignored), isolated no-default binary tests (278/0/1), isolated no-default core tests (1,035/0/4), explicit isolated KAS core tests (1,816/0/10), four all-target Clippy lanes, default/no-default binary builds, initial formatting with no changes and final format check. Ignored tests were not executed; overlapping lanes are not a unique-test aggregate. Current source review of the exact 19-path feature delta has no findings (confidence 0.96), a read-only judgment rather than empirical acceptance. One current owned B actual-consumer run PASS covers all six retained captures at 140×40 outer/139 content geometry: manual URL and delivery ACK-not-authorization, durable error, escaped controls/bidi with printable text exact, draft preservation and PageUp/PageDown. This is synthetic ACP peer/PTy evidence, not real native KAS/host or CI acceptance.

Historical bg121/artifact818 remains pre-repair PASS (2,839 passed,13 ignored/not executed); the original A full-URL PageUp FAIL, source tree851c workspace EOF FAIL and partial candidate2954 B/C PASS remain unchanged historical receipts. The four old-layout RED/restored GREEN pairs plus exact indicator-only width1 mutation RED/restored GREEN are five historical sensitivity pairs on the unchanged source body, not reruns for the current ratatui pair. Current four-gutter-test GREEN coverage and current owned B PASS are separate evidence. No Cargo/check/formatter/consumer/native operation was run for this documentation refresh.

**Unresolved acceptance:** Current local qualification and one current owned B actual-consumer PASS replace the former current-workspace-BLOCKED framing; they do not waive integrated `nk4o`/C4, native KAS/host or CI evidence. Earlier native NK passive17/17 PASS and active31/32 rawFAIL with separate normal-reset adjudication remain historical and distinct; no fresh native-Tqr frame/sourceURL/run is invented. The owned synthetic session ID is not the historical public-redacted native session ID. Native retry/login/account/config changes remain unauthorized after the before-initialize auth failure. NK previews/inventory and `caar` deadlines remain unqualified and are not Tqr features. Parent owns final staging, commit/tracker disposition, this issue's PR and the approved subsequent stack. The retained current receipts are a source-pinned local-evidence handoff, not an all-claims PASS, issue-close claim, completed publication or CI/native shipping authorization.

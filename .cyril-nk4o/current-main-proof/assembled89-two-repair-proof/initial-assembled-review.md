# Ordinary assembled NK design/source review

## Boundary and verdict

Reviewed /home/dwalleck/repos/cyril-wt-fix-cyril-nk4o at HEAD 89ef8075c9972d5c2489485041adcdd962ed5f54 against immediate Tqr parent d0cc60d7662ced69a4a32c904e9800bea6990a22, including affected inherited callbacks/notices and receiving consumers outside the diff.

Overall correctness: incorrect. Confidence: 0.96. Two actionable patch-introduced P2 defects remain. Otherwise the reviewed owner placement, typed dispatch, root lifecycle and inherited Tqr callback/notice interactions conform to the approved contract. This is ordinary assembled source review, not isolated reconstruction or acceptance.

## Finding 1 — P2: Anchor unavailable catalog tabs before allowing reset or auth

File: /home/dwalleck/repos/cyril-wt-fix-cyril-nk4o/crates/cyril-ui/src/mcp_panel/presentation.rs
Diff-overlapping lines: 476–478.
Confidence: 0.99.

At supported compact 80x6 geometry, select a failed/authentication-required server, switch from Servers to Tools with Tab, then press plain r or a. An empty connected catalog tab produces the same defect. The document initializes focus to 0 at presentation.rs226, the Session row. The selected server row supplies focus only when panel.tab == Tab::Servers at476–478; the unavailable-catalog branch611 and empty-catalog branches never replace it. key(Tab) rebuilds with follow_focus=true (mcp_panel/mod.rs443–450,481–503), reanchoring the one-row viewport to that unrelated Session row. Actual compact placement has popup height2/body1 and no header: widgets/mcp_panel.rs19–39; corresponding retained source regression geometry assertions are mcp_panel/tests.rs1564–1570. target_visible() in mod.rs307–311 now sees the unrelated focus0 inside the viewport and returns true. The r/a arm457–464 constructs the retained selected server's reset request; App1896–1902 sends BridgeCommand::Mcp. Core types/mcp.rs404–415 permits reset of failed/connected servers, and domain_mediator/commands/mcp.rs25–34 sends kiro/mcp/resetServer with that server and startOAuth:true only for a. Therefore a reconnect/OAuth operation is actually admitted without the actionable server identity being visible; there is no downstream refusal making this harmless.

Approved contract violated: design.md C3/C4 and Assembly/UI-F1 require hidden-target activation to reanchor the original target without dispatch, with only a later visible explicit activation permitted to dispatch. The retained UI-F1 tests stay on Tab::Servers and do not cover unavailable/empty other tabs.

Narrow fix: retain the selected server row as the fallback action focus when the current tab has no selectable catalog entry, or otherwise make reset/auth visibility use an actual server-target anchor and reanchor before admission. Do not add App business logic, retarget to the painted row, or add the explicitly rejected draw-ack latch. Add compact failed/empty-tab reset and auth regressions through existing UI/App typed-command seams; reviewer did not execute them.

## Finding 2 — P2: Reject wrong-typed recognized provenance fields

File: /home/dwalleck/repos/cyril-wt-fix-cyril-nk4o/crates/cyril-core/src/protocol/convert/kas/mcp.rs
Diff-overlapping lines: 161–170.
Confidence: 0.93 (scout0.91; independently validated converter/reducer/receiver path).

A matching-session status frame with a resource descriptor containing present wrong-typed known fields, e.g. _meta.kiro.resource.krn:[] or source.origin:17, passes provenance(). The text closure161 and source.origin chain164–168 use and_then(Value::as_str), turning invalid recognized values into absent optional data rather than Err. Only the outer descriptor object's shape is validated. snapshot() invokes provenance(&value)? for server descriptors270, and provenance(value)? for top-level descriptors297, so these invalid fields do not stop successful conversion. convert()119–135 emits McpUpdate::Snapshot rather than Diagnostic. inbound.rs203–206 and38–44 route the typed update through McpOwner::ingest into the matching-session reducer; SessionController301–307 uses the same reducer. types/mcp.rs326–334 replaces the previous inventory, clears its diagnostic, and removes retained passive auth URLs for removed servers. App refreshes the already-open panel1620–1624; presentation.rs244–248 has no STATUS ERROR banner because diagnostic was cleared. Pretty raw descriptor details remain displayable but are not a malformed-field diagnosis and do not preserve the last valid snapshot.

Approved contract violated: design.md converter-owner row explicitly requires diagnosing malformed recognized fields and retaining the last valid snapshot as stale/error rather than accepting a healthy replacement; C1 names malformed-known data. This is the same strictness already used by deserialized optional known status fields in this converter, not a new blanket-validation requirement. It is a static malformed-input counterexample, not a claim malformed data was observed live.

Narrow fix: distinguish absent optional recognized provenance fields from present wrong-typed fields and return a descriptive Err for the latter, including source/origin shape where present. Continue preserving unknown/future descriptor fields in details. Existing convert() Diagnostic path/reducer already supplies retention and visible stale-state behavior; no new state owner is needed. Add malformed descriptor conversion plus prior-snapshot retention regression at existing seams; reviewer did not execute it.

## Scope and unaffected source conclusions

Reviewed typed inventory/governance conversion and domain types, authoritative replacement/removal/passive URLs, narrow retained-policy contradiction; management/reset/read payloads, canonical promptName/resource URI and required string arguments; off-loop budgets, command/result receivers and stale/cancelled correlation; sole new/load admission, true-session staging, same-ID live authority, activation ordering and failure release; panel/request planning, projected grapheme/CellWidth/caret paint, modal/key/paste priority, theme registration/cap and v2 /mcp preservation; bound-engine form/URL decline, delivery failure propagation, global contextual publication and root state scope; shared App/Engine wiring and inherited Tqr global/scoped URL/error durable notice behavior and consumers.

Existing Integration/F1 and Integration/F2 repair conclusions are retained only in scope. Actual hashes matched all four reviewed pins: test_bridge.rs8f51bc683b8587e244a3f5d2a997903ba7c5d183b81e4c2273868ad4ea36d3c5; state.rs650741c521b836512b75293e843fb5cc7a4a18921b92cc3e3b15594a9695d025; mcp_panel/integration.rsa79911bf6ef3ad4899e9addbc2df9f785f37a719e03aaf11fccf4af6f936884a; App tests/mcp_panel.rs7f39942bfb2fda3ea89bb61fdf03a8d2e1720cf95e9ec004dbcd4268d752cb53. Formatter/decline producer and receiving/render paths were reread; no whole-feature or historical runtime PASS was promoted. New finding1 is an additional UI visibility gap, not invalidation of those separately scoped repairs.

## Actual approved-design SHA256 pins

NK /home/dwalleck/repos/cyril-wt-fix-cyril-nk4o/.cyril-nk4o/design.md: c966bf4be3755007901eb3c03009c5695fcae8421825d514f6d446b262162749.

Current TqrWT /home/dwalleck/repos/cyril-wt-fix-cyril-tqr8/.cyril-tqr8/design.md, including approved public terminal_safe DG amendment: dbf41172474809b07f7afbd01a45882700a758f50712044818485db8f352c5a5.

These are actual read-time file SHA256 hashes, not historic document hashes or Git blob IDs. Tqr's old private-interface discrepancy and stale universal isolated-review claims are not findings or gates here.

## Limits

No repository edits, Git mutations, builds, tests, linters, formatters, launches, authentication probes, native or platform acceptance runs. Test code and retained receipts were reviewed, not executed. No current CI, runtime, native glyph, auth, enterprise enforcement or source qualification/acceptance result is established. Empty LSP reference/definition results were reported as tool limitations and were not absence evidence; receiving dispatch was traced through exact source. No independent whole-repo/security certification. Main owns repairs, qualification and publication.

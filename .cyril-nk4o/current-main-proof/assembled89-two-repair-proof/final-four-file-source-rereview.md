# NK two-finding repair — final ordinary source-only rereview

## Final verdict and boundary

Scoped source rereview: PASS. Overall correctness: correct. New/unresolved findings: none. Confidence: 0.98.

Boundary: the exact four-file repair in /home/dwalleck/repos/cyril-wt-fix-cyril-nk4o against original HEAD89ef8075c9972d5c2489485041adcdd962ed5f54, INCLUDING the Main-authorized nested-source-array follow-up. This is a source-level verdict, not an executed post-fix GREEN or acceptance result. Original pre-fix FAIL/RED records remain unchanged.

Original full assembled review: local://nk4o-assembled-review-89ef8075.md. Its two P2 findings are now resolved in the final candidate by source analysis; unaffected conclusions are retained under the dependency-specific applicability below.

## Independently computed final file SHA256 pins

- crates/cyril-ui/src/mcp_panel/presentation.rs: cca5728d3913e3355136a87368dfa3da7ec928ce278dd3523a9287d28e43eb75
- crates/cyril-ui/src/mcp_panel/tests.rs: bfe027fc5eab6aaa86aafffba71c628ad575783f596189378babf0ebe1ffc7d7
- crates/cyril-core/src/protocol/convert/kas/mcp.rs: 16b2330d61729860e45b2322c2d7c6a3307ccf00ac739f53209afe9999d2c1f0
- crates/cyril-core/src/protocol/convert/kas/mcp_tests.rs: dc56ff8f0d9862e8e8dcbf9772f655bf25943e1c1b8407e4ecd77174cbeb3d33

These were actually computed by this reviewer, not merely copied from worker/Main assertions. Initial converter9e4795bbed190ae641e248cf1618d1fea7c8f66c9378a9d62ef303ac848e3cbc and tests14eb67049c4f6c66bc4d3a6bc1b95eb9a085051057f21f6a4e0c5bfb7278539b were reviewed first and are not the final converter/test boundary.

## Per-item closed source dispositions

### Original UI P2: RESOLVED

Deciding production source: crates/cyril-ui/src/mcp_panel/presentation.rs476–478 now assigns the selected server row to focus independently of the active catalog tab. It supplies the missing fallback for failed/unavailable or empty catalogs. Later selected-entry branches516–611 still override that fallback on nonempty Tools/Prompts/Resources/Templates, preserving existing entry navigation.

[INFERENCE — manual source derivation] After free PageUp hides the selected server, unchanged target_visible() in mcp_panel/mod.rs307–311 compares the real target row with the viewport rather than comparing Session row0. The first unmodified r/a activation consequently constructs no request, then the existing rebuild(true)/clamp_scroll path481–505 reanchors the retained original server. A later explicit visible activation uses the unchanged exact session/server/OAuth request. Page::Arguments and Page::Read are separate branches and are untouched; no App body, target retargeting, key classifier, draw-ack latch, renderer allocation or geometry owner was added.

Regression source: mcp_panel/tests.rs1648–1794 adds four harnesses covering reset/auth, empty connected catalogs and failed/unavailable catalogs; loops specify 24 scenarios across four tabs and the two failedAuthorization values where applicable. They use actual UiState/full UI TestBackend rendering and key handling, force three PageUp keys, require a Session-only painted row with neither server visible, then check first no-request, original-target repaint, exact second McpRequest, preserved draft and closure. This is test source inspection, not execution of the 24 scenarios.

Approved contract: design C3/C4 and Assembly/UI-F1 hidden-target/reanchor ownership. Source confidence0.99.

### Original provenance P2: RESOLVED AFTER FOLLOW-UP

Deciding final source: convert/kas/mcp.rs104–116 introduces private typed ResourceProvenance/ResourceSource DTOs;175–182 explicitly rejects every present non-null non-object source BEFORE DTO deserialization183. Recognized optional resourceType/krn/layer/origin use Option<String> type validation. Missing/null optionals remain accepted. Unknown descriptor and source-object fields remain accepted and preserved in details by serializing the original descriptor, not the reduced DTO.

[INFERENCE — manual source derivation] Scalar/array/object values in recognized string fields cause DTO error. Present non-null source strings/numbers/booleans/arrays cause explicit shape error. Missing source, null source, source:{}, source:{origin:null}, and source:{origin:string} preserve their existing accepted meanings. This same helper serves server and top-level descriptors. An error propagates through snapshot() to the existing Diagnostic branch in convert(); the existing McpState reducer updates diagnostic without replacing the last valid inventory or modifying retained authorization URLs. No new event/receiver/state owner is introduced.

Important repair chronology: the initial DTO-only candidate9e4795 was PARTIAL, not PASS. Source rereview established that source:["workspace"] and source:[null] could deserialize positionally into ResourceSource and still produce a healthy replacement snapshot. Exact locked dependency proof was read independently: Cargo.lock uses serde/derive1.0.228 and serde_json1.0.149; serde_json/value/de.rs1009–1022 accepts arrays for borrowed-Value deserialize_struct,730–746 delegates to visit_seq, and serde_derive/struct_.rs70–95,140–146 plus de.rs451–552 generates positional field consumption. The final175–182 object-shape guard closes that precise bypass before Serde can admit it. This is resolved AFTER the follow-up, not a retroactive claim the initial DTO candidate was correct.

Regression source: convert/kas/mcp_tests.rs124–174 now specifies nine malformed inputs at both descriptor locations,18 planned scenarios, including one-element source string/null arrays. Each requires Diagnostic, applies it to a real converted prior valid inventory, checks Arc::ptr_eq for retained snapshot and equality of stored diagnostic. Missing/null optional and future-field/arbitrary-string controls remain present177–247. Authorization URL retention is established by unchanged reducer source, not by an auth-seeded assertion in this fixture. None of the expanded18 scenarios is claimed executed by this reviewer.

Approved contract: converter-owner malformed-recognized-field obligation and C1. Source confidence0.99.

## Independent repair adversary and unaffected applicability

A separate read-only repair-diff adversary found no remaining actionable patch-introduced defect in the final four-file candidate, confidence0.98. It reviewed private optional decoding, map/sequence representation boundaries, focus fallback versus later nonempty entry focus, pinned request identity, argument/read/caret paths, and test changes. Existing-test changes in the exact diff are import ordering and assertion/signature formatting/reflow; no old predicate, literal or behavior assertion was weakened.

The original full assembled review's unaffected conclusions remain applicable within their scopes because the production repair changes only (a) the catalog document's fallback focus and (b) private provenance decoding before the same Snapshot/Diagnostic interface. Root admission/staging, live replay authority, request planning, wire payloads, timeouts/correlation, callback adapters, ElicitationDeclined publication, App routing, inherited Tqr URL/error ownership and the shared terminal-safe formatter are unchanged by this four-file repair. Integration/F1 and Integration/F2 keep their independently justified original applicability; no old runtime result is promoted to repaired-source execution.

## Preserved original RED/FAIL and actual reach

Owning receipt: local://nk4o-89-hidden-target-and-provenance-test-only-RED-handoff.json. Its two raw logs were read; commands were NOT rerun.

- Original UI command exited101: four failing harnesses,0 passed,0 ignored. Each stopped at the first hidden Tools reset/auth no-dispatch assertion. Four started loop scenarios—not all24—and no reanchor repaint, second exact request, later tab/status or closure verdict was reached. This is UiState/TestBackend/key-return proof, not App bridge/native dispatch proof.
- Original core command exited101: malformed-case test failed at the first server-level krn:[] because conversion returned Snapshot; the two independent missing/null and future-field control harnesses passed. Only one malformed scenario started; the remaining13 original malformed scenarios were unreached. The failing test did not apply the malformed snapshot, so runtime reducer clobber was not observed there.
- The subsequent one-element-array counterexamples and final18-case test matrix are source-derived/source-reviewed only. The original RED is UNEXTENDED: no new array RED, postrepair GREEN, expanded-case execution, native or acceptance result is invented.

The original FAILs remain valid historical sensitivity evidence at their original production/test/source-descriptor identities. They are not relabeled repaired results.

## Governing approved-design read-time SHA256 pins

- NK .cyril-nk4o/design.md: c966bf4be3755007901eb3c03009c5695fcae8421825d514f6d446b262162749
- Current-TqrWT context .cyril-tqr8/design.md, including approved public terminal_safe DG amendment: dbf41172474809b07f7afbd01a45882700a758f50712044818485db8f352c5a5

These are the actual approved documents consumed for the original assembled review and retained same-contract repair context; no obsolete private-formatter requirement or universal isolated-review gate was imposed.

## Limits and remaining owner

No repository edits, Git mutation, builds, tests, formatter, linter, smoke, native/auth/platform activity or executable mutation probe. Read-only diff/source/dependency/log/receipt inspection and local report only. Both source item verifiers and the repair adversary operated read-only. No post-fix GREEN, full-source descriptor match, CI, platform, native glyph/auth/enterprise, publication or integration GO is claimed. Main owns applicable postrepair execution/qualification and publication. Scoped source review is complete with no remaining finding.

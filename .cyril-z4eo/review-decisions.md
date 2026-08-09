# Design-review decisions

| # | Finding | Reviewer | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|---|
| 1 | Preserve approval origin through trust confirmation | DesignFalsifier | Bug/design | Yes. `UiState::approval_confirm` returns only `TrustOption` (`state.rs:1781-1846`), while `App::persist_trust_grant` selects the main session's current mode (`app.rs:698-732`). | Modify | Enrich the confirmation result with `SessionId`; preserve main persistence, block and report foreign-to-main writes. Durable foreign-config persistence is tracked by verified issue `cyril-ufld`. |
| 2 | Generic workspace compilation is not a distinct placement fence | DesignFalsifier | Design | Yes. The workspace can compile after deliberately exposing or relocating queue state, so the original falsifier could pass despite the claimed boundary regressing. | Modify | Claim 8 now uses explicit negative compile operations: external queue reordering must fail privacy, and responder consumption through immutable `TuiState::approval()` must fail ownership. No source-text test is introduced. |

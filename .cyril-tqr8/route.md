# cyril-tqr8 route

Date: 2026-10-19; current PR193 repair checkpoint updated 2026-10-21. The approved design inspected base `8e5c325e`; the current scoped repair is bound to the **498-input SHA-256 descriptor**, not Git tree or commit, `d265f295d35ba2eabe692d6000c3127771ded51f30865361b55cb86406bb7117`. Published base HEAD `b75010427c16b1dac13b7cf00c595ce2492f391f` alone does not identify the repaired working source. The [owning current evidence](evidence.md#current-pr193-repair-checkpoint-2026-10-21) and [portable repair package](current-publication-evidence/pr193-repair/index.json) distinguish the current scoped PASS from the retained historical a09 tree/12-lane/six-capture package. Tracker scope and approvals are unchanged.

## Route tests

| Test | Verdict and evidence |
|---|---|
| T1 — external premise | No unresolved design-changing premise. Current KAS 0.66.43 executable source establishes the callback, its independent capability gate, passive-versus-explicit OAuth behavior, and memory-only credential fallback. Current first-party TUI displays/copies the URL and acknowledges handling without opening a browser. See `evidence.md`. Integrated live acceptance remains required, not a substitute for an unknown protocol contract. |
| T2 — module shape | Yes. Add a real external-URL host-callback adapter and typed domain notification at the existing Engine/host-callback seam; derive advertisement from installed adapter presence. Extend KAS display-error conversion. Public notification consumers and their exhaustive matches must migrate together. No new generic host-effect framework or browser process owner. |
| T3 — production-scale risk | No material new throughput or memory risk is proposed. Existing bounded host/domain channels and off-loop callback dispatch remain authoritative; displaying a URL must not block initialize or session creation. |
| T4 — explicit behavior | Yes, grounded in the ticket and current first-party callback semantics: on bound KAS, advertise and actually answer `_kiro/openExternalUrl {url}`; visibly surface its URL; report handling, never OAuth completion. Surface `session_info_update` `display_error` with `errorType:mcp_connection_error`. Keep v2 notification UX working. Do not advertise secret storage, persist MCP credentials, invent server/session fields in the callback, or depend on dark `_kiro/mcp/oauthCompleted`. `nk4o` supplies the passive status/panel and explicit reset action; the callback implementation itself must not depend on that PR. |

**Selected route: Structural.** Capability ownership and typed core-to-UI interfaces change. Existing primary source settles protocol questions, so a new empirical OAuth probe is not a prerequisite to design approval.

## Required artifacts and sequence

- This `route.md`: change-workflow owner.
- `evidence.md`: primary-source factual record, produced using prove-it-prototype's evidence discipline; no new probe necessary to establish semantics.
- `design.md`: falsifiable-design owner; DG approval recorded from Main on 2026-10-19.
- `plan.md`: budgeted-plan owner; approved issue slice and parent-owned verification/PR boundary recorded.
- An isolated loopback MCP/OAuth acceptance fixture is test infrastructure, not a replacement implementation and not claimed as executed evidence.
- checkpointed-build: parent-owned red-before/green-after, consumer smoke, repository gates and independent review; then this issue's own PR. Parent owns commits/push/publication.

No separate spec is needed for the source-established URL-surfacing behavior. Automatic browser launch/clipboard ownership is not requested and is not introduced. Any later choice to introduce it returns to design.

## Terminal criterion / current gate

Complete only when the callback and capability are jointly reachable on KAS and jointly absent on v2/no-KAS builds, MCP errors and URLs visibly reach the real consumer, passive/explicit OAuth integration works with `nk4o`, all applicable parent-owned checks and independent review pass, and the issue has its own PR. **Current checkpoint 2026-10-21: all four approved PR193 findings are resolved within scope; qualification PASS, independent bounded repair review has no findings (correct, confidence 0.97), and one authorized current owned-consumer invocation PASS with 11 captures / client5 / peer67 / zero retries.**

The original 21-case run recorded 14 actual intended RED, three new GREEN controls and four prior App GREEN controls, with first-failure reach limits retained. Eleven successful preformat gates remain applicable through four authorized test-only reflows preserving exact Rust tokens **including punctuation** and production bytes; the original fmt FAIL is immutable, and actual postformat fmt/default-all-bins build PASS. This is not a claim that all gates reran postformat. The unchanged finite-peer CLI RED/GREEN remains separate. The [owning current evidence](evidence.md#current-pr193-repair-checkpoint-2026-10-21), [qualification](current-publication-evidence/pr193-repair/qualification.json), [repair review](current-publication-evidence/pr193-repair/repair-review.json), [owned consumer](current-publication-evidence/pr193-repair/owned-consumer.json) and [history/CLI](current-publication-evidence/pr193-repair/history-and-cli.json) own the exact results and boundaries.

The current visible-main proof establishes complete opaque URLs and scoped child/workflow payloads, browse-to-follow reveal, chronology, no duplicate old presentation after late claim, and exact draft plus independent caret preservation. Hidden canonical ownership/adoption/pending/internal authority and focused-renderer/reveal behavior remain source/API tests, not screen-proven production focus entry: no new production focus-entry caller, binding or view was added. Actual owned-root absence and zero **readable** owned-marker links retain explicit EACCES/ENOENT visibility limits, not universal process absence.

PR193 CI is still the old RED result, not current-head platform CI PASS. Peaq PR194's refresh/review/CI blocker is **resolved and landed** at main `03838a501dad8be140acd78dc3fff4cae35fafef` after refreshed head `991b16f9f902ff225c18ec528df7ee999f914e36` / base `7a0716dc7f28035244dd7ba451a4e2293e887a0c` and GREEN CI run39690453316. The current Tqr b750+repairs source has **not** yet integrated that main; Main owns ancestry reconciliation and current CI after this package. Separate `u62f` Windows attribution remains open, not waived by green Windows CI.

Current native KAS/host/platform acceptance and final `nk4o`/C4 reset→callback→authoritative-connected, `caar` and C6 retained-history/live/gutter integration are not claimed. NK local checkpoint `ef46c43813d12b63f45b264e427abda84d299f46` does not establish final NK/native/C4 acceptance or propagation into Tqr. Delivery ACK is not authorization. The native-auth stop still forbids retry/login/account/config changes without fresh Main authorization.

The original A full-URL PageUp FAIL, old workspace EOF FAIL, partial candidate2954 B/C, historical818/nativeNK and a09/12-lane/0.96-review/six-capture results retain their original scopes. Five old-layout/indicator sensitivity pairs were not rerun for this repair. The [historical checkpoint](evidence.md#delivery-checkpoint-2026-10-20) is not the current source identity or acceptance ledger. Parent owns final staging, commit/tracker disposition and PR/stack actions; no issue closure, shipping, merge or completed publication follows from this record.

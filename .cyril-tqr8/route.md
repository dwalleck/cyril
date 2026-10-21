# cyril-tqr8 route

Date: 2026-10-19; delivery checkpoint updated 2026-10-20. The approved design inspected base `8e5c325e`; current local qualification is pinned to assembled source tree `a09f1dfb63b17d380f4661823de431bfb5d72160` over prerequisite tree `699256c2b0dadf4000f72f6beb178b15ae8d1b90`, including the 19-path feature delta. The [owning current evidence](evidence.md#delivery-checkpoint-2026-10-20) and retained [qualification receipt](current-publication-evidence/qualification/canonical-handoff.json) record exact source, receipts and scope; the root publication index/manifest also retain the separate historical evidence. Tracker scope and approvals are unchanged.

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

Complete only when the callback and capability are jointly reachable on KAS and jointly absent on v2/no-KAS builds, MCP errors and URLs visibly reach the real consumer, passive/explicit OAuth integration works with `nk4o`, all applicable parent-owned checks and independent review pass, and the issue has its own PR. **Current local evidence: all 12 Cargo lanes PASS, current source review has no findings (confidence 0.96), and one current owned B actual-consumer run PASS.** These are source-pinned local results, not an all-claims PASS, native KAS/host or CI acceptance, or completed publication. The [owning current evidence](evidence.md#delivery-checkpoint-2026-10-20) owns the detailed results and remaining limits rather than this route maintaining a second gate ledger.

The original A full-URL PageUp FAIL, old workspace EOF FAIL, partial candidate2954 B/C results, historical818 and nativeNK results remain historical receipts, not current failures or newly relabeled successes. The four old-layout RED/restored GREEN pairs and indicator-only width1 RED/restored GREEN are five historical sensitivity pairs on the unchanged source body; they were not rerun for the current ratatui pair. Current gutter GREEN coverage and the current owned B consumer PASS are separate evidence. The owned peer/PTy run does not establish current `nk4o`/C4, `caar`, real native KAS/host or CI acceptance; the native-auth stop still forbids retry/login/account/config changes. Parent owns final staging, commit/tracker disposition, this issue's PR and the subsequent stack; none is claimed complete here.

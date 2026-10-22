# cyril-caar — full startup diagnostic acceptance gate

## Current draft publication — 2026-10-22

**Native acceptance BLOCKED by authentication; draft only.** The approved acceptance below is unchanged, including the separate >=90s New budget and real consumer requirement. Draft publication is not an acceptance reduction or waiver.

The [current proof index](current-main-proof/index.json) owns source/binary identities, the sealed local qualification, independent-review applicability and pending gates. See [local qualification](current-main-proof/qualification.json), [review applicability](current-main-proof/review-applicability.json), [native attempts and blocker](current-main-proof/native-consumer.json), and the [draft PR body](pr-body.md). No native/CI/final-conformance/merge-ready or issue-closure claim follows from this publication. Logical delivery remains Tqr #193 → NK #196 → Caar; the draft targets main for existing CI and explicitly depends on #196.

## Retained approved contract and canonical history

Imported from the owning canonical Caar record. Approved requirements remain governing; historical source pins, patch recipes and check statuses below remain attributed to their named earlier frames, not to the current qualified source. Archival patches/private receipts referenced below are not implicitly shipped or authorized for reapplication. Current status is the proof index above.

## Request (verbatim)

> caar session/new >=90s separate budget and full diagnostics/early events acceptance, not just constant tweak.

Authoritative ticket acceptance: `session/new has its own budget of at least 90s, separate from the generic request timeout. Pre-response _kiro.dev/mcp/* frames are not dropped, and a timeout names which servers reported, which are silent, and which await OAuth.`

## Settled behavior

- Dedicated new-session budget is at least 90 seconds and cannot be shortened by changing ordinary control-call or session/load constants. Do not quietly inflate unrelated control/load deadlines; prompt remains unbounded by response wait.
- Keep processing and exposing early ready/failure/OAuth events during the wait, including the post-30s OAuth scenario reported in the ticket. Preserve true wire session/attempt attribution and events needed to explain failure across session-created resets.
- On actual timeout, present elapsed budget plus named reported outcomes, named awaiting-OAuth servers and named silent servers when supported by an authoritative expected inventory. Never infer OAuth completion, silence or success from absence of frames.
- Preserve existing fatal startup-failure notification, bridge teardown and late-response isolation. Do not create an orphan slot by treating a timed-out new as successfully active or by accepting its late response into a replacement attempt.

## Approved absent-inventory behavior

**D1 — resolved by the requester through Main.** User selected **“Report known names and unknown inventory”**:

> Name reported and OAuth-waiting servers; name silent servers when an authoritative inventory exists. Otherwise explicitly say the agent supplied no complete inventory. Keep the separate ≥90s startup budget.

This governs the original ticket's absent-data case; it does not permit inferred server names, a partial filesystem/config scan presented as complete, or relabeling all unresolved servers never-reported. The live v2 query's timely availability remains unproven (evidence P1b), but the approved unknown-inventory branch no longer depends on that guarantee.

Expected timeout presentation:

> Session creation timed out after 90s.
> Reported: ready-server (ready), broken-server (failed).
> Awaiting OAuth: auth-server.
> Silent: names from authoritative expected inventory, if one arrived.
> Other configured servers: unknown — the agent supplied no complete startup inventory.

The unknown-inventory explanation is required when no complete inventory exists, not a false `silent:none`. Do not duplicate Kiro's profile/power/registry/policy resolver.

“Announced but not settled” is distinct from “silent”: a connecting-status row has already reported. Awaiting-OAuth is explicit engine evidence, never guessed from a 30-second gap. Where a provider calls `pending` its unresolved set, preserve that label/meaning rather than relabel every pending server never-reported.

## Success / evidence

Parent's behavioral checks must cover >=90s separate wait, response after 30s success, timeout with ready/failed/OAuth/silent inventory cases, no-inventory truthfulness, pre-response event retention, SessionCreated reset ordering, exact session matching, late timeout-response isolation and owned subprocess teardown. Real consumer fixture is required in addition to private helper tests. P1b remains an observed limitation, not an unapproved acceptance reduction.

## Prior art / scope

Full records: `cyril-caar`, `cyril-nk4o`, `cyril-tqr8`; closed `cyril-0dv4` covers serialized mcpServers presence. Broader MCP panel/reset is nk4o, real URL callback/error surfacing is tqr8. No new config resolver, retry/recovery system, automatic browser/clipboard behavior, production account change or generic diagnostic framework.

## Approval

**D1 APPROVED** as recorded in the owning decision above. Parent design-placement approval, implementation, red-before/green-after and real consumer/repository proof, independent review and a distinct PR remain required. No deadline-only patch is presented as completion.

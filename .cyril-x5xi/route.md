# cyril-x5xi route

Date: 2026-10-19. Assigned worktree based on `8e5c325e`; full `rivets show cyril-c6la cyril-zj0m cyril-x5xi --json` records read, including empty notes/design/relationship/resource fields. Source facts below describe routing; current approval and implementation gates are recorded under Artifact ownership.

## Requested behavior

Theme-dependent cache identity must be structurally complete: a newly used rendering field cannot silently escape identity; highlight identity includes only rendering dependencies; cross-theme cache isolation tests pass. Existing palettes/modes and rendered output remain unchanged. This issue must end in its own independent review and PR.

## Route tests

| Test | Verdict and evidence |
|---|---|
| T1 Empirical premise | **No.** Dependency closure is visible in current source. Highlighting consumes syntax selection and fallback text color (`highlight.rs:47-50,100-123,164-194`); Markdown consumes those plus code/subdued/emphasis/accent_tertiary/accent_quinary (`widgets/markdown.rs:119-262,315-335,383-433`). No runtime premise changes the appropriate identity. |
| T2 Structural module shape | **Yes.** Replace Theme-dependent renderer implementations with concrete, hash-derived rendering-input values. Public Theme adapters remain, but actual implementations and their keys accept the same restricted input type. Full Theme identity uses structural derivation; no second handwritten role list is introduced. |
| T3 Production-scale risk | **No new material scale risk.** These are Copy-sized inputs replacing existing broad 29-role hashing. Tests must prove cache reuse as well as isolation; no arbitrary latency gate is warranted for this issue alone. |
| T4 Explicit behavior | **Yes.** Every actual output dependency participates, irrelevant role changes reuse cached output, syntax/mode/fallback distinctions remain correct, and existing thread-safe cache/poison fallback/eviction behavior is preserved. Entire resolved Theme remains structurally identifiable where a full-theme consumer needs it; narrow rendering-input identities are separate. |

**Selected route: Structural** because the concrete rendering-input seam changes, although no new empirical premise or latency budget is needed.

## Evidence

`Theme` contains 31 Color fields and `syntax`, while both current key functions enumerate the same 29 colors (`theme.rs:386-421`; `highlight.rs:59-98`; `widgets/markdown.rs:53-94`). The omitted roles, `text_secondary` and `accent_violet`, are not currently consumed by either renderer. This is evidence of fragile manual maintenance, **not proof of an existing stale-output bug for those two roles**. Most included roles also do not affect the cached outputs.

Existing tests manually repeat the old role list and require over-broad invalidation; migrate those expectations to actual dependency input values rather than freezing the defect (`highlight.rs:407-518`; `widgets/markdown.rs:867-937,1083-1164`). Preserve bundled-palette/mode isolation, no-color sharing, both cache population orders, poison fallback, eviction and concurrency coverage. Marker-theme fixture already names all 31 fields (`traits.rs:1111-1145`).

## Artifact ownership and terminal criterion

`route.md`: change-workflow. `design.md`: falsifiable-design, **DG-approved by parent Main, 2026-10-19**; its Approval section owns the delegation and approved decisions. Budgeted-plan's `plan.md` defines the one atomic implementation slice. [verification.md](verification.md) owns the checked source identities, portable parent-run regression/consumer/review receipts and qualification limits. No separate spec or empirical prototype is needed; explicit outcomes and source facts suffice.

Recommended first PR of the performance stack, followed by `cyril-zj0m` and then `cyril-c6la`. It must pass the cross-theme/reuse tests, existing rendering regression/consumer checks, applicable repository gates and independent code review before parent opens its issue-specific PR. No production restructuring before approval. No requester feature budget or applicable hard line cap was supplied or manufactured.

## Delivered edit slice / parent handoff

Production edits are present, uncommitted, in `crates/cyril-ui/src/theme.rs`, `crates/cyril-ui/src/highlight.rs` and `crates/cyril-ui/src/widgets/markdown.rs`. Theme/SyntaxTheme structurally derive Hash; concrete HighlightStyle/MarkdownStyle values own both real rendering inputs and identity; public Theme adapters project and delegate. Manual cache role-hash lists are removed. Cache policy, Vec output backing and rendering semantics are intentionally unchanged; Arc output is c6la's separate approved work.

Coverage includes structural Theme identity for previously omitted roles/syntax, exact style projections, relevant/transitive output isolation, irrelevant-role reuse, content/language distinctions, fallback/error behavior and both cache population orders. Existing lifecycle, palette/mode, no-color, poisoning, concurrency and eviction coverage is retained. Main's artifact 555 executed the assembled workspace: **2,836 passed, 0 failed, 13 existing ignored**; clippy workspace/all-targets, executable build and formatting passed. The portable raw log and source/command receipt are indexed in [verification.md](verification.md).

**Documentation-worker verification: none.** Main owns every executed gate. The checked delivery is published fiv1 base `7eefe275fbec97f610677b2ee6433af996806599` / [PR #184](https://github.com/dwalleck/cyril/pull/184) plus exactly the three production paths above. The index binds their SHA-256 values and the actual consumer executable; earlier original-base sensitivity remains separately labeled.

**Actual-App consumer PASS:** three separately authorized truecolor/none/truecolor-return startups rendered committed/live code, palette preview/cancel/commit, resize, oldest/middle/follow browse and a later live append. This is actual CLI-process/PTY/decoded-cell evidence, not a live Kiro/model or native graphical-terminal glyph claim; configured history is 1,000, not 50,000. README now documents the projected used-style identity after that smoke. **Local verification handoff complete; PENDING — Main:** issue-specific publication, CI adjudication and tracker closure. The index owns the qualification boundary.

## Reviewed repair checkpoint

`review-decisions.md` owns ReviewThemeIdentity F1: the exact MarkdownStyle allowed-import correction preserves all implicit-resolution prohibitions and the 17-file/300,000-byte limits. Main's corrected bg100 actual-Cargo A/B executed one test per case: identical approved widget BEFORE FAIL / AFTER PASS; forbidden ThemeId widget BEFORE and AFTER FAIL. Current workspace qualification passed, and final independent source-only review returned **correct, confidence 0.99**. The earlier fixture-copy compile failure had **NO TEST VERDICT**, not product RED/GREEN. Main's corrected `gate-review-f1.py` is preserved unchanged; full transcript, input/checker hashes and script bytes are portable.

`production.patch` is the **earlier original-base sensitivity snapshot**, not a current assembled-tree patch: 61,267 bytes; SHA256 `9ac057ccb15ee7e809330f41bb3abf39259018ea61a8066950635eb02bd93585`. Current production-file identities, base and executable hash live in [verification.md](verification.md).

## Retained independent sensitivity

The portable archive's `sensitivity/receipt.md` retains the executed RED/GREEN receipt for the original patch SHA256 above, on baseline `8e5c325ebb5dac3a4ad89f71838fd198cc177543`. The x5xi irrelevant-role cache-eviction regression failed before and passed after; relevant syntax/fallback visible controls passed on both sides. Six probes ran overall (baseline 2 PASS / 4 intended FAIL; patched 6 PASS), but the other failures/construction counts belong to the separately scoped zj0m issue. This earlier proof is not relabeled as current-source execution, whole-UI qualification or native terminal proof. Temporary probe source files are not ported into production; current uninstrumented qualification is separately indexed.

## Shared requester-accepted Windows qualification limit

The requester's explicit existing cyril-u62f Windows memory-readiness baseline exception remains applicable: PR183 had three initial-ready failures, 23 passes and 2,670 cancelled later tests (13 skipped); cancelled coverage is not green. Other substantive jobs passed with unchanged memory identities proven. This is an accepted qualification limit, not a repaired runtime/platform-green claim or permission for retries, skips, timeout/runtime changes or new failures. The [public record](https://github.com/dwalleck/cyril/pull/183#issuecomment-6284441811) and portable archive's `limits/windows-baseline-exception.md` own this exception; all other issue obligations still apply.

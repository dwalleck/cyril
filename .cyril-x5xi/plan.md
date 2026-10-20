# cyril-x5xi implementation plan

Approved design: [design.md](design.md), Approval (DG, parent Main, 2026-10-19). One substantive issue PR, first in the x5xi → zj0m → c6la stack. No commit/push by this worker; parent owns integration checks, independent review and PR publication.

## Slice 1: structurally complete rendering inputs

**Outcome:** C1–C4: cache identity follows actual concrete rendering inputs; irrelevant role changes reuse output; current palette/mode rendering and cache lifecycle remain unchanged.

**Changes:** `theme.rs` adds Hash derivation for Theme/SyntaxTheme plus crate-private Copy/Debug/Eq/Hash `HighlightStyle { syntax, text }` and `MarkdownStyle { highlight, code, subdued, emphasis, accent_tertiary, accent_quinary }`, projected from `&Theme`. `highlight.rs` keeps its public Theme adapter and adds the crate-private style entry used by Markdown; actual cached key/render/fallback bodies consume HighlightStyle only. `widgets/markdown.rs` keeps its public Theme adapter; key/parser/helpers consume MarkdownStyle and pass nested HighlightStyle directly. Both old manual role-hash lists disappear. Update internal test callers and obsolete all-role-invalidates assertions atomically. No new runtime dependency, cache-policy rewrite, lints change or public signature change.

**Verification:**
- Parent observes new irrelevant-role reuse assertion RED against old key functions and GREEN against this slice; relevant-role output/isolation positive controls remain discriminating.
- Parent runs focused highlight/Markdown tests, then full UI and applicable workspace compiler/lint/format gates. Preserve fallback, concurrency, poisoning, eviction and existing conversation fixture coverage.
- Parent drives real root/App render smoke with committed/live code, palette/mode switch and theme preview commit/cancel; update existing architecture/cache documentation after successful smoke.
- Parent obtains independent code review of this issue's implementation and ownership before opening its PR.

**Dependencies:** No unresolved production decision. LSP references for Theme/do_render returned no results in this worktree despite listed symbols; prior tool issue is recorded, and explicit source caller mapping is the fallback. Workers ran no checks. Main's current assembled workspace, clippy/build/fmt, F1 source-gate A/B, actual-App smoke and independent review receipts are indexed in verification.md. Issue-specific publication remains parent-owned. Dependent zj0m behavior does not expand or block this slice.

## Delivery and budget

One atomic implementation slice; theme/highlight and Markdown edits had disjoint ownership and integrated before Main's checks. No independently enforced line limit or requester feature budget applies. README's permanent used-style cache and theme-picker guidance was delivered after actual-App consumer PASS. Source state, test results and qualification limits are owned once by verification.md, not inferred from this plan.

## Portable baseline defect-sensitivity case

The permanent tests use new style-only helper signatures; copying them verbatim to old production would cause a compile error, not valid RED. The executed probe instead kept the same behavior at each source's real helper seam: a fresh local `HashCache` of capacity 2 retains source A and probe B, then renders A after varying only `Theme.code`. Old broad identity inserts a third entry and evicts A; the new HighlightStyle path hits without eviction. Both syntax-enabled and no-syntax fallback controls ran. Original checked source identities, actual assertions and results are retained in verification.md rather than an unexecuted snippet.

Permanent regressions include `irrelevant_theme_roles_reuse_highlight_cache_entries`, `highlight_cache_isolates_syntax_and_fallback_text_inputs`, `cached_error_fallback_isolates_text_but_reuses_irrelevant_roles`, `full_theme_hash_includes_previously_omitted_roles_and_syntax`, and `markdown_cache_isolates_direct_and_fallback_colors_in_both_orders`. Other updated Markdown tests exercise composite direct/transitive styles and irrelevant roles in both orders. Main's current workspace executed these permanent tests; the portable archive separately retains the original-base sensitivity receipt.

### Executed pre-change sensitivity

The earlier illustrative test snippet is superseded by the actual independently
executed probe/log/source-manifest receipt in verification.md's **Earlier defect
sensitivity** section. Its original base and patch hashes are preserved; it is
not a current-source run or native terminal claim. The x5xi irrelevant-role
eviction assertion failed before and passed after, with syntax/fallback output
controls passing on both sides. The same six-probe run also contains separate
zj0m observations, which do not become x5xi acceptance requirements.

## Executed actual-App consumer

The owned-peer consumer replaced the illustrative live-agent procedure as the
instrument for the same Markdown/palette/picker outcomes without credentials,
model turns or real file operations. Main authorized only the exact tested
x5xi executable. Three independent truecolor/none/truecolor-return startups
recorded committed/live syntax code, preview/cancel/commit, narrow/wide resize,
oldest/middle/follow browse and a later live append; verification.md links the
actual ANSI/cell/wire/results and byte-identical executed harness.

This exercises the real CLI executable and App lifetime, not just TestBackend,
but PTY/pyte does not qualify native graphical font/glyph rendering. Startup-only
mode selection is not a same-process mode switch; max_messages=1000 is not a
50,000-message scene. No benchmark, latency budget or later zj0m/c6la runtime
claim is introduced. README was updated after the successful consumer. Main's
corrected F1 A/B passed its intended sensitivity; issue publication is parent-owned.

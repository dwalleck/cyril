## Findings

None meeting the required patch-introduced, actionable-defect criteria.

## Inspected state and scope

Reviewed the **actual assembled working source** against held main `03838a501dad8be140acd78dc3fff4cae35fafef`, including staged and unstaged changes—not HEAD alone. The supplied current input identity is the 497-input descriptor `cfeb275f8ace1ec73f13858d41797cfe02bb4f82956f662ae53da742ddc3ec3c`; this review does not independently regenerate that descriptor or equate it with a Git tree.

The main-relative source delta covers the substantive approval changes in:

- `crates/cyril-ui/src/widgets/approval.rs`
- `crates/cyril-ui/src/widgets/workflow_plan.rs`
- `crates/cyril-ui/src/state.rs`
- `crates/cyril-ui/src/traits.rs`
- `crates/cyril/src/app.rs`

Also inspected affected fixture initializers, added/retained regressions, the retired initial-placement snapshot, and README behavior documentation. The unstaged source change is the incoming chat fixture’s `preview_scroll_offset: 0` initializer; it was included.

Interaction review additionally covered `tool_diff.rs`, chat rendering, shared physical-width helpers, modal placement, frame/input geometry, feedback editing, tool-call merge/update ownership, subagent wrappers, and the permission/workflow producer-to-App route. Concurrent publication records were not treated as approval authority; the supplied frozen design inputs governed this review.

## Code and contract evidence

| Area | Current source evidence and conclusion |
|---|---|
| **Independent tree and comparison budgets** | `widgets/workflow_plan.rs:63–94` wraps and caps the tree independently at 12 rows, including its omission marker, then appends the entire cleaned comparison. `:100–149` preserves zero-budget behavior and the 600-code-point cleaning/capping rule. No four-row comparison cap remains. This matches the explicit separate-tree-budget amendment. |
| **Visible body and retained path geometry** | `widgets/approval.rs:139–146,199–272` bounds scrollable body demand/painting to 12 rows including status. Body capacity subtracts the **retained fitted path’s actual row count**, without refitting it. Painting, navigation bounds and border hints use that same body-window computation. |
| **Pagination, resize and narrow widths** | `widgets/approval.rs:698–726` derives navigation bounds from current frame/page geometry and rejects zero usable text cells or zero visible body rows. `state.rs:2340–2371` clamps the stored offset before movement and pages by actual visible content rows. Comparison continuations use the approved adaptive indent while tree continuations retain four columns. |
| **Fallback ordering and omission accounting** | `widgets/approval.rs:635–688` preserves usable typed-plan precedence, then Diff, Text-content, raw text, and degraded preview handling. `:550–584` counts the displaced row in a plan fallback’s marker: overflowing 13-row text retains 11 content rows plus `...2 more lines`; ordinary Text remains five content rows plus marker. Ordinary Diff retains its separate 20-row contract. |
| **Keys, phases and queue ownership** | `app.rs:1843–1989` routes through the topmost modal and delegates preview keys before editor handling. The exact NONE/SHIFT modifier matches do not steal ordinary selection/editor movement or Ctrl/Alt combinations. Every explicit approval constructor initializes offset zero. Reason transitions retain the same request; response/cancellation exposes the next independently initialized queued request. Confirmation remains guarded before responder ownership is consumed. |
| **Workflow delivery** | The permission mediator carries the typed workflow plan through `PermissionRequest`; App’s existing review-policy route explicitly handles owned review requests and forwards ordinary requests to `show_approval`. The plan reaches `ApprovalState` without a newly introduced silent discard. |
| **Borrowed-source/cache integration** | `approval.rs:635` now ties returned lines to `&'a ApprovalState`. `traits.rs:366–437` and `tool_diff.rs:8–120` show that the cache owns ranges/tags/counts—not borrowed strings or styled lines. `chat.rs:400–432` obtains descriptors and source from the same tracked wrapper and applies the current theme. Nonempty effective-source replacement invalidates the appropriate cache slot; initialized clones share immutable descriptors, not mutable source ownership. Approval snapshots remain separate from later transcript/active-call updates. |

## Retained versus new review applicability

- **Retained narrowly:** Compared with historical qualified parent `21496f1b…`, current `workflow_plan.rs` and `app.rs` are unchanged, and the approval-file change is the borrowed-source lifetime annotation. The independent-tree projection, retained-path/body-window mechanism, navigation regressions and physical-cell controls therefore retain their earlier source-review relevance. Their affected current implementations were nevertheless inspected again.
- **Freshly broadened:** Incoming `TrackedToolCall` caching, source invalidation, clone behavior, range slicing, chat lifetimes and their approval adapter received new static review. The historical owned-string adapter review alone cannot establish these interactions.
- **Behavioral distinction preserved:** Incoming chat headers now follow the same **first Diff** as the body, rather than potentially skipping an unchanged first Diff for a later changed one. That was not mislabeled as unchanged historical behavior.
- **Evidence boundaries preserved:** Historical 488-input reviews/qualification are not automatically current497 approval. No unsupported mutation count was used as behavioral proof. The supplied successful current full24 actual-App consumer is separate evidence, not an execution or result produced by this review.

Rust-analyzer reference lookup timed out and then returned no references despite visible callers; explicit scoped constructor/caller inventories were used instead. No successful semantic-reference coverage is claimed.

## Verdict

**Overall correctness:** correct  
**Explanation:** No concrete patch-introduced defect was found in the current assembled approval behavior or its affected ownership, cache, workflow and App-dispatch interactions. The inspected source conforms to the frozen approved design; this judgment is static source review, not runtime, native-platform, CI or publication acceptance.  
**Confidence:** 0.94

No files were changed, and no builds, tests, linters, formatters or applications were run.
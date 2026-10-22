# cyril-zj0m: one retained diff view per tracked source

## Route and inputs

Structural; [route.md](route.md) owns requested behavior and sequencing. Existing `TrackedToolCall` owns private UI display data with centralized `new`/`update`; main, child and approval all already use it. The core terminal ledger is not a candidate: it evicts completed calls, while UI history retains them. Implementation and executed qualification are indexed in [verification.md](verification.md), with historical sensitivity kept distinct from actual-tree gates.

## Selected design and observable decision

Use the **first `ToolCallContent::Diff` for both summary and body**. Preserve the body's existing first-Diff selection. Previously summary could instead describe a later changed Diff while the first unchanged Diff rendered no body. The approved correction removes those misleading counts rather than broadening display into a multi-file diff UI. First-Diff no-change yields no summary/no changed body, even if a later Diff differs. The Approval section records the explicit user decision; permanent regression tests and the actual-App consumer exercise it.

Add a concrete private `tool_diff` module owning one theme-independent `DiffPreview` derivation:

- Optional exact `(added, removed)` summary for the selected first Diff.
- At most 20 retained body row descriptors, one context row under existing `similar` grouping, plus truncation state.
- Each descriptor identifies old/new source side, byte range, change class and displayed line number. It does not own a second copy of the source line. The retained source remains the private wrapped ToolCall. Rendering borrows source slices and applies current semantic colors.
- An explicit no-Diff/no-change result is cached too. No sentinel count means absence.

`TrackedToolCall` gains a lazy `OnceLock<Arc<DiffPreview>>` and an internal borrowed accessor. The initializer constructs one `TextDiff::from_lines`, obtains both summary and bounded body from it, and retains the result. The wrapper's existing Clone shares an already-initialized immutable derivation in O(1); updates detach only their own lazy slot, preserving snapshots that still own the old source. Empty slots clone empty: starting both active and historical snapshots does not eagerly derive twice. Only consumers that actually display a diff initialize it.

Before merging a nonempty content replacement, `update` compares the effective selected first-Diff old/new inputs against the incoming selected first Diff. Empty update content preserves source and cache. Equivalent effective inputs retain it; actual input/selection changes clear the lazy slot. Path/title/locations/raw-output/kind/status alone do not invalidate the text-only result. `None` old text and `Some("")` are equivalent derivation inputs. The source's private ownership prevents mutation that bypasses this seam.

`compute_diff_summary` is removed. `render_tool_call` reads summary and body from the same borrowed result. `render_diff_lines` remains the real style adapter used by chat and pending approvals, now consuming the retained result instead of deriving again. It must not acquire a Completed/Write restriction: approval preview displays Diff content for pending operations. Source text is borrowed through explicit lifetimes rather than cloned into each row; the later conversation-layout PR can retain prepared styled blocks without repeating this work.

## Placement and module shape

| Module | Owner / disposition | Interface and forbidden responsibilities | Check |
|---|---|---|---|
| `tool_diff.rs` (new, private) | Create cohesive diff derivation owner. | Effective selected source → immutable summary + compact bounded row descriptors. Own context/row-limit policy; no Theme, ratatui, protocol, IO or global cache registry. | Direct coherence tests plus consumer rendering. |
| `traits.rs::TrackedToolCall` | Retain tracked-source lifetime owner; deepen with lazy derived value. | Constructor/update/accessor and coherence invalidation only. Treat file as protected parent for the derivation algorithm: no Myers traversal/row assembly bodies here. | Compiler ownership and independent review. |
| `widgets/chat.rs` | Retain presentation. | Style the retained rows/current header; no `TextDiff` construction. Preserve existing status visibility and output behavior. | Existing cell/role fixtures plus exact summary tests. |
| `widgets/approval.rs` | Retain preview behavior. | Continue using the shared real diff style adapter for pending approval content. No second derivation path. | Pending-approval preview test. |
| `cyril-core::ToolCall` | Unchanged domain source and merge semantics. | Must not gain derived UI context, row policy, cache, Theme or ratatui dependency. | Dependency/compiler checks and review. |

A global `(tool ID, theme, content hash)` cache is rejected: it hashes large source each frame, adds cross-session identity/eviction state, and duplicates lifecycle information already owned by the tracked wrapper. Eager derivation is rejected because active/history copies and off-screen incoming Writes would both pay immediately. Copying complete old/new strings into the derivation is rejected; compact descriptors reuse the existing source. A per-frame single derivation alone is insufficient because historical redraw still repeats Myers.

## Claims / falsification

| ID | Claim and cases | Falsifier / verification | Regression coverage | Status |
|---|---|---|---|---|
| C1 | One selected source and one Myers construction supply exact summary and body. First-Diff selection is coherent, including unchanged-first/changed-second. | Actual-constructor counters plus exact visible header/body/gutter assertions; unchanged-first and changed-second remain discriminating. | Derivation/coherence and permanent chat tests; original-base independent RED/GREEN; actual-App completed/unchanged/replacement scenes. | PASS — [verification.md](verification.md), obligation ledger and historical sensitivity boundary. |
| C2 | Historical reuse survives status-only/metadata-only/equivalent-content updates, width/theme switches and wrapper clone; real source changes invalidate coherently. | Initialized/cloned snapshots and update categories; zero new derivation for unchanged input, one for changed input, old clone remains correct. Current styling is applied without deriving again. | Tracked-wrapper and root consumer tests in the qualified workspace; historical actual-constructor counts; actual-App width/palette/offscreen/restored-source observations. | PASS — [verification.md](verification.md); the PTY consumer alone does not claim counter measurements. |
| C3 | Existing presentation limits and pending preview remain correct. | Exact bounded rows/ellipsis, context/line numbers and colors; pending approval preview without a Completed restriction. | Qualified workspace tests, including the repaired pending regression; actual pending ledger join and RejectOnly response. | PASS — [verification.md](verification.md). No permission grant or real file operation was exercised. |
| C4 | Repeated production-shaped rendering does not repeat unchanged diff derivation. | Real typed-notification/root/TestBackend consumer counters; report-only cold/warm benchmark with fixture construction excluded. | Qualified root regression and sparse-2k/dense-20k/pending benchmark, plus separately recorded unknown-mode CLI rejection. | PASS — [verification.md](verification.md). No unmeasured baseline comparison, time threshold or native-terminal timing claim. |

Counter instrumentation is test-only at the actual expensive operation, not production telemetry or a test oracle that reimplements diff. The retained original-base RED/GREEN receipt proves duplicate/repeated Myers sensitivity for its pinned patch; current uninstrumented full gates independently execute the permanent regressions. Exact visible summary/body checks prevent a no-op renderer from satisfying the work assertion. Existing `similar` output/reference fixtures suffice; no separate diff algorithm or arbitrary mutation suite is introduced.

## Repeated-render benchmark and smoke

Add a `harness = false` UI benchmark target using `std::time::Instant`/`black_box`, not a new production dependency. The first issue owns a `completed-write` case; c6la extends the same target with long-history cases in its own PR. Setup uses public `UiState`, typed `ToolCallStarted`/`ToolCallUpdated` and real `render::draw`, never direct calls to only the derivation helper.

Workloads: a completed 2,000-line Write with sparse insert/delete groups and a second dense 20,000-line Write that exercises truncation; one pending preview control; 160×48 TestBackend. Build fixtures and initialize syntax/cache before timed steady samples. Report cold-first-draw separately, then 100 warmup draws and 30 samples of 100 stable draws; report median and p95 per frame with raw samples. Run single-process on the parent's pinned reference runner, record Rust/compiler/profile/target/features/CPU and relevant environment, and exclude construction from warm samples. No uncalibrated time assertion or invented universal 16 ms budget: deterministic no-rederivation is the acceptance fence; timings expose practical regression and are reviewed before publication.

Executed benchmark command: `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo bench -p cyril-ui --bench conversation_render -- completed-write`. The owning [verification index](verification.md) links the actual source/environment/commands and all 90 raw sample rows. The separately authorized direct existing-executable unknown-mode check is not backdated into that run.

Executed production smoke uses the real hash-pinned App/CLI with an owned ACP peer through a PTY: Write start → incremental update → completed update, unchanged-first/changed-second, replacement, width/palette changes and offscreen/back. A pending Diff snapshot joins a matching-ID partial permission request and survives resize before the sole RejectOnly option is sent. Final palette restoration and turn completion execute. [verification.md](verification.md) owns those observations and distinguishes PTY/pyte evidence from native graphical glyph proof. No external account, model, real file operation or grant is used. README documentation follows consumer PASS.

## Delivery and non-goals

Second issue-specific PR after cyril-x5xi; it is independently useful and verifiable without viewport virtualization. It contains the retained derivation, all consumer migrations, coherence/lifecycle tests, repeated-render benchmark and post-smoke documentation. The later cyril-c6la PR still owns real visible-window behavior and long-history qualification, not an empty duplicate PR.

Do not change the 20-row policy, introduce syntax-tinted diffs, display all Diff blocks, change ToolCall merge semantics, move UI logic into core, or redesign active/history snapshot storage. No feature budget/hard line target was supplied or invented. No issue acceptance is deferred.

## Approval

**Approved — 2026-10-19.** Explicit user behavior decision, relayed by parent Main: “Match summary to first diff” — “Recommended correction: counts and body describe the same file. If the first diff is unchanged, no change counts are shown even when a later diff changed.” This authorizes the first-Diff-for-both behavior in this design.

**DG — 2026-10-19, lazy tracked-source derivation ownership approved by parent Main under delegated routine sign-off.** User delegation reported verbatim by Main: “Delegate routine sign-offs” — “I select and record evidence-backed designs. I still ask before scope changes, relaxed acceptance, architecture changes outside the existing boundaries, or explicit risk acceptance.” No accepted risk or fidelity deviation is authorized. The design and scope remain unchanged; [verification.md](verification.md) owns delivered evidence and remaining publication actions.

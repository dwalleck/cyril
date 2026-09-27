## Fresh conformance review
Scope: current production source plus the actual diff, compared only with `.cyril-y628/reconstruction.md` and approved `.cyril-y628/design.md`.
The frozen reconstruction remains accurate for production ownership; two now-stale review observations are dispositioned below.

| # | Approved ledger row | Verdict | Current-source evidence and disposition |
|---|---|---|---|
| 1 | `types/event.rs` | MATCH | Capability and reason-bearing response remain domain data; exact option identity is retained (`crates/cyril-core/src/types/event.rs:511-522,570-590`). |
| 2 | `convert/mod.rs` | MATCH | Generic conversion owns ACP outcome/exact-ID handling and delegates optional metadata (`crates/cyril-core/src/protocol/convert/mod.rs:342-420`). |
| 3 | `convert/kas.rs` | MATCH | Engine, nonblank reason, offered exact ID, `RejectOnce`, and the sole production key literal are KAS-owned (`crates/cyril-core/src/protocol/convert/kas.rs:21-52`). |
| 4 | `domain_mediator/mod.rs` | MATCH | The mediator only snapshots engine capability, builds the request, and passes the same engine into conversion; the spawned oneshot flow is retained (`crates/cyril-core/src/protocol/domain_mediator/mod.rs:674-713`). |
| 5 | `feedback_editor.rs` | MATCH | The private leaf owns normalization, filtering, UTF-8 cursor movement, scalar accounting, atomic insertion, and notice state (`crates/cyril-ui/src/feedback_editor.rs:4-29,31-292`). |
| 6 | `traits.rs` | MATCH | `ApprovalPhase::EnterReason`, opaque editor exposure, request capability, and the shared eligibility predicate are the approved public UI value (`crates/cyril-ui/src/traits.rs:484-553`). |
| 7 | `state.rs` | MATCH | `UiState` copies capability, owns FIFO/phase/responder transitions, delegates editing, and emits blank/plain versus nonblank/reasoned responses (`crates/cyril-ui/src/state.rs:1595-1606,2086-2312`). |
| 8 | `widgets/input.rs` | MATCH | One concrete crate-private helper now supplies unchanged wrapping/cursor-follow rendering to chat and feedback (`crates/cyril-ui/src/widgets/input.rs:74-151`). |
| 9 | `widgets/approval.rs` | MATCH | Existing render dispatch now covers the reason phase and reads only approval/editor state for hint, draft, cursor, limit, and notice (`crates/cyril-ui/src/widgets/approval.rs:11-30,154-361`). |
| 10 | `lib.rs` | MATCH | The editor is registered as a private module only (`crates/cyril-ui/src/lib.rs:1-6`). |
| 11 | `app.rs` | MATCH | App contains only batch-boundary and key/paste routing delegation; it owns no text, eligibility, or wire policy (`crates/cyril/src/app.rs:1584-1601,1639-1660,1757-1791`). |

No UNCOVERED production responsibility was found: changes in `render.rs`, `floor_tests.rs`, `widgets/toolbar.rs`, and `tests/modal_theme.rs` are fixture-field migration only.
No MISSING ledger row was found; all eleven approved production paths are present and carry exactly their assigned responsibility.
The bridge rejection-feedback file is test-only layered coverage, not a twelfth production owner (`crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/rejection_feedback.rs:5-94`).

### Protected parents, dependencies, and seams
`app.rs` remains routing-only; feedback paste/key handling delegates directly to `UiState` (`crates/cyril/src/app.rs:1639-1660,1757-1791`).
`convert/mod.rs` remains generic; `rejectionReason` production encoding exists only in the KAS adapter (`crates/cyril-core/src/protocol/convert/mod.rs:342-420`; `convert/kas.rs:21-52`).
`domain_mediator/mod.rs` adds only immutable engine/capability wiring, while `state.rs` adds only approval transitions and editor delegation (`domain_mediator/mod.rs:674-713`; `state.rs:2086-2312`).
The actual diff adds no Cargo dependency; UI source contains no ACP response or KAS JSON dependency.
The approved concrete seam is intact: opaque `RejectionFeedback`, crate-private mutations, and one shared concrete render helper; no adapter trait, parallel editor state, compatibility renderer, or second wrapping algorithm appears.

### Independently checked facts and dispositions
Exactly one public `max_scalars` accessor exists, backed by the private 4,096-scalar constant (`crates/cyril-ui/src/feedback_editor.rs:3-6,75-78`); the widget consumes that accessor rather than duplicating the bound (`widgets/approval.rs:346-352`).
The actual diff shows legacy `approval_with`/`approval_for_tool_call` direct-`ApprovalState` fixtures were pre-existing; they remain at `widgets/approval.rs:491-509,853-867` and are not new feedback fixtures.
New feedback render fixtures create a `PermissionRequest`, call `UiState::show_approval`, enter via `approval_begin_feedback`, and edit via `approval_feedback_paste` (`widgets/approval.rs:512-529,599-668`).
Disposition: the frozen reconstruction’s statement that feedback widget tests directly construct `EnterReason`/`RejectionFeedback` is stale after repair; current source is stronger and matches the approved interface test, so no code or design change is needed.
Disposition: direct editor unit tests remain intentional owner-local tests (`feedback_editor.rs:300-460`); the design expressly assigns text invariants to that concrete editor.
Disposition: the concrete public editor view is an approved tradeoff forced by the public phase variant; construction/mutation remain crate-private (`feedback_editor.rs:31-45,76-134`; `traits.rs:484-495`).
Disposition: typed `Tab` is consumed while pasted tab is preserved; typed Tab was not an approved input cell, so this is neither drift nor an obligation to broaden behavior (`feedback_editor.rs:87-134,260-292`).

### Remaining correctness review
The frozen AltGr concern is resolved: current character admission accepts Ctrl+Alt with optional Shift while still excluding plain Ctrl/Alt shortcuts (`feedback_editor.rs:100-112`), with a focused behavioral fence at `feedback_editor.rs:318-341`.
New boundary values are consumed explicitly: `RejectWithReason` reaches generic/KAS conversion, capability reaches `ApprovalState`, and `EnterReason` reaches state and render dispatch; none is silently dropped.
No remaining production correctness defect meeting the review criteria was found.
This is a source/ledger conformance conclusion only; layered unit and bridge fixtures are not represented as proof of full UI integration.

## Final: PASS
All eleven ledger rows MATCH, protected-parent/dependency/seam constraints hold, and every observed reconstruction mismatch or tradeoff has a concrete non-blocking disposition.

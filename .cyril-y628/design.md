# Design: cyril-y628 rejection feedback

## Route and inputs

Structural route: route.md. Behavior source: every Given/When/Then under the approved spec.md, signed “Approve specification” on 2026-09-26. This includes KAS-only opt-in `r`, unchanged plain rejection, multiline editing/paste, atomic 4,096-Unicode-scalar insertion limit, whitespace-only plain rejection, Esc-back/discard, request identity/FIFO, modal isolation, and exact response-level metadata. Source baseline: claim commit 83b1ee65 (production inherited from cede8f5f).

Empirical artifacts: N/A — route T1 retains the paired KAS 0.66.8/0.66.0 evidence in docs/kiro-2.24.0-wire-audit.md §7.4 and its committed captures/probe. No premise that v2/other agents consume feedback or older KAS does. Independent wire expectations come from captured response dictionaries and failed tool content, not a second invocation of Cyril's converter.

## Input shapes and decision cells

| Input population / representation | Cells and required verdict | Coverage |
|---|---|---|
| Permission response | Selected with trust Some/None; Cancel; new RejectWithReason carrying exact option ID and owned String | C1, C2, C5 |
| Engine selection | Kas: feedback eligibility; V2: no UI action or metadata; kas feature disabled: no metadata path | C2, C3 |
| Offered options | empty/single/multiple; repeated kinds with distinct IDs; each AllowOnce/AllowAlways/RejectOnce/RejectAlways; selected index absent; foreign ID; duplicate IDs retain existing membership semantics (no new malformed-provider repair) | C1, C2, C3, C5 |
| Approval phase | SelectOption, SelectTrust, new EnterReason; only SelectOption + eligible RejectOnce enters; trust navigation and plain decisions unchanged | C3, C5 |
| Reason presence/content | absent via ordinary Selected; empty; whitespace only; ASCII, quotes, leading/trailing spaces, Unicode, combining/wide chars, newline/tab, literal cursor-block character | C2, C4, C6 |
| Input insertion | typed scalar, Ctrl+J/raw LF, explicitly modifier-reported Shift+Enter, pasted block, CRLF/CR normalization, disallowed control characters; text at 0/4095/4096 scalars and insertion crossing bound | C4 |
| Cursor/action | byte cursor at start/interior/end; UTF-8 multibyte boundary; Char/Backspace/Delete/Left/Right/Home/End; Enter/CR submit, Ctrl+J/raw LF newline, explicitly reported Shift+Enter newline, Esc back; irrelevant keys consumed | C4, C5, C6 real-terminal proof |
| Queue/transport | one request, multiple sessions/requests including equal tool IDs, later arrival during editing, back/re-enter, send receiver dropped | C5 |
| Modal delivery | feedback-active paste accepts; other approval/trust/modal paste rejects; voice remains blocked; normal chat draft unchanged; buffered key batch after terminal decision | C6 |
| Render geometry | ordinary, narrow/short, zero interior; long and multiline Unicode text; cursor at either end; overflow notice | C6 |
| Fixed ceiling | 4,096 Unicode scalar values in normalized/filtered draft, not bytes or wire overhead; every typed/newline/paste insertion enforces the same limit | C4; N/A — approved risk requested: no configuration knob raises this bound |
| Persistence/time/replication | N/A — ephemeral request editor; no new storage, clocks, caches, replication, or path processing | Permanent non-goal |

The ceiling counts only the explanation, after CR normalization/control filtering. It is not an ACP frame size limit; UTF-8/JSON encoding overhead is outside this count, and existing transport bounds remain unchanged.

## Removed-invariant sweep

The wire extension is additive. Input routing is selectively subtractive: paste was previously blocked under every modal. Its replacement invariant is “paste goes only to the active approval feedback editor, otherwise the existing modal guard remains”; C6 tests both accepted and refused paths with a positive normal-input control. Approval selection previously always resolved or entered trust; the new nonterminal editor must preserve FIFO, responder identity, and batch barriers (C5). Rendering helper extraction preserves existing chat-input output/cursor semantics (C6). No permission wait becomes inline async work.

## Placement

- Domain permission data: `types/event.rs`. Add `PermissionResponse::RejectWithReason { option_id: PermissionOptionId, reason: String }`, rather than a field that permits combining trust and rejection feedback. Existing Selected and Cancel retain their shapes. Add `PermissionRequest.can_reject_with_reason: bool`, derived from the configured KAS engine and feature availability at request creation, not payload sniffing.
- Generic ACP response selection: `protocol/convert/mod.rs`. Extend the concrete converter with `AgentEngine`; share exact-ID preservation and existing foreign-ID diagnostics. Dispatch optional KAS encoding to `convert/kas.rs`; no Kiro JSON key literals in generic conversion.
- KAS metadata: `protocol/convert/kas.rs`. Own the reason metadata builder and exact offered-ID/RejectOnce/nonblank eligibility. Attach metadata to RequestPermissionResponse, not SelectedPermissionOutcome. No parsing/rendering knowledge outside core.
- Request orchestration: `domain_mediator/mod.rs` only captures the immutable engine kind, sets the capability, and passes the engine into conversion. Existing oneshot and spawned permission task stay unchanged.
- Feedback text implementation: new private `cyril-ui/src/feedback_editor.rs`, concrete `RejectionFeedback` with private String/byte cursor/scalar count/notice; Debug/Clone/PartialEq/Eq are derivable. It owns insertion normalization, control filtering, bound, cursor editing, and read-only getters; no ACP, responder, approval queue, or persistence.
- Public UI value: re-export `RejectionFeedback` through `traits.rs` so `ApprovalPhase::EnterReason { chosen_option_id, editor: RejectionFeedback }` has a valid public interface. The module remains private; editing methods are crate-private. A phase-owned editor makes stale parallel editor state unrepresentable. `ApprovalState` gains the per-request capability; its derived eligibility method is shared by rendering and UiState.
- Existing approval state machine: `state.rs` owns entry/back/submit/FIFO. New narrow editing methods delegate text actions to the editor. Nonblank submit constructs RejectWithReason; blank submit constructs ordinary Selected with no trust. Esc restores the original selected option and discards the editor. No editor draft in ordinary chat input.
- Existing widgets: `widgets/input.rs` deepens with one concrete crate-private text/cursor rendering helper extracted from the current renderer; chat and feedback use it. `widgets/approval.rs` renders EnterReason and eligibility hint using existing modal geometry and preview/attribution. No compatibility render wrapper or second wrapping algorithm.
- App: key/paste delegation only. It never inspects option kinds, sanitizes/counts text, constructs metadata, or owns editor state. Voice guard unchanged.

## Module shape

Inventory counts are physical prefixes before test sections (including blanks/comments), not semantic LOC or architectural proof. For convert/mod.rs, exclude leading test-only probe declarations from interpretation; its main production section ends at line 491. For types/event.rs and traits.rs, use first test-only module rather than the later `mod tests`. Counts below are tied to baseline 83b1ee65.

| Module/path | Baseline lines | Interface / invariants | Owns / hides | Must not own | Adapters / callers / tests | Change |
|---|---:|---|---|---|---|---|
| crates/cyril-core/src/types/event.rs | 705 | PermissionRequest, PermissionResponse; owned responder, exact option ID | Domain capability and decision data | SDK/UI imports, editing | Mediator/UI/test fixtures; compile plus C1/C2/C5 | deepen |
| crates/cyril-core/src/protocol/convert/mod.rs | 491 | from_permission_response(response,args,engine); exact ID and legacy warning | Generic ACP decision selection; dispatch | KAS metadata literals, UI | Mediator and converter tests | deepen |
| crates/cyril-core/src/protocol/convert/kas.rs | 582 | crate-private metadata helper, only eligible rejection | KAS JSON format and offered-option guard | UI/editor/stateful policy | Generic converter; C2 wire tests | deepen |
| crates/cyril-core/src/protocol/domain_mediator/mod.rs | 760 | handle_permission; spawned response, immutable configured engine | Existing request orchestration | Reason parsing/editing | SDK/domain queue; bridge integration tests | retain |
| crates/cyril-ui/src/feedback_editor.rs | 0 | RejectionFeedback private state; crate-private edit actions, read-only getters | Bounded Unicode editing and notice | ACP, oneshot, queue, JSON | UiState edits, approval widget reads, editor tests | create |
| crates/cyril-ui/src/traits.rs | 781 | ApprovalPhase/ApprovalState; read-only TuiState remains unchanged | Phase/option/editor ownership, shared eligibility | Text-editing implementation, wire metadata | UiState/widgets/external fixtures | deepen |
| crates/cyril-ui/src/state.rs | 2853 | Existing approval methods plus narrow begin/edit/paste; responder consumed once | Approval transitions, FIFO, request-local phase | Editor algorithms, new protocol parsing | App and state tests | deepen |
| crates/cyril-ui/src/widgets/input.rs | 138 | Concrete text/cursor renderer; preserve current wrapping | Shared wrapping/cursor-follow drawing | Input mutation, approval policy | Existing chat renderer and approval widget | deepen |
| crates/cyril-ui/src/widgets/approval.rs | 325 | Existing render signature, reads ApprovalState | Approval presentation, hint/editor/notice | State mutation, bridge/SDK | Main renderer and modal tests | deepen |
| crates/cyril-ui/src/lib.rs | 22 | Private module declaration | Crate declarations | Feature implementation | Rust module resolution | retain |
| crates/cyril/src/app.rs | 2901 | Existing key/event handlers | Routing and existing trust effects | Text/count/metadata policy or editor state | Terminal loop and App behavior tests | retain |

Public construction sites and exhaustive matches migrate atomically, including test fixtures and examples if affected. Existing exported paths remain only where still the canonical owner; no deprecated aliases or compatibility wrappers are introduced. No new dependency or generic interface.

### Alternatives and seam tests

1. **Selected: phase-owned concrete editor.** `ApprovalPhase::EnterReason { chosen_option_id, editor }`; caller `ui.approval_begin_feedback()` / `ui.approval_feedback_key(key)`; renderer reads the phase. Hides UTF-8 editing and bounds in one module, keeps queue transitions in UiState, and binds draft to a request. Public read-only editor type is a deliberate interface change, not exposed mutable fields.
2. **Private parallel editor session in UiState.** `Option<(PermissionOptionId, Editor)>` beside approvals; renderer needs new TuiState view methods. Minimizes existing enum/fixture churn but creates invalid combinations (editor with no front request, trust phase plus editor) and distributed reset invariants. Rejected for weaker state modeling and a wider rendering seam.
3. **Separate editor overlay.** An independent modal emits typed submit/cancel events linked to a request token. Allows reuse but creates another overlay/queue owner and ordering protocol for one real consumer. Rejected as hypothetical flexibility and responsibility duplication.

Deletion test: removing the editor would spread Unicode/control filtering/count/limit behavior across key and paste callers. Interface test: App and tests use UiState operations; text tests use the same concrete edit operations. Adapter test: no new generic seam; one concrete editor is appropriate. Locality test: core owns wire, editor owns text, UiState owns decisions, widgets own rendering. Sharing the existing text renderer avoids two cursor/wrapping implementations.

### Protected parents

| Parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| App app.rs | Event routing, cross-module effects | Existing approval-key/paste dispatch branches | New feedback-policy/editor/serialization body | No new feedback state or helper algorithms; production growth projection ≤35 lines |
| Generic convert/mod.rs | ACP conversion | Variant/engine dispatch and legacy exact-ID handling | Literal rejectionReason encoding | Key literal exists only in KAS owner; growth projection ≤40 lines |
| DomainMediator mod.rs | Serial domain/request lifetime | Capture engine, populate capability, pass converter arg | Feedback logic or synchronous human wait | Existing task/oneshot structure retained; growth projection ≤12 lines |
| UiState state.rs | State transitions and input owners | Existing approval phase transitions and editor delegation | Unicode editing/count/filter algorithm copied into state | New editing implementation stays in feedback_editor.rs; growth projection ≤130 lines |

Module-shape fence: issue-local `.cyril-y628/oracles/shape.py`, non-production. It checks changed production path allowlist, required private editor/phase path, forbidden SDK dependencies in UI, KAS key ownership, editor algorithm ownership, absence of duplicate editor state/compatibility render wrapper, and protected-parent growth against baseline discovered through default/upstream refs plus the pinned base receipt. Reports C7 and offending path/symbol/delta. Numeric growth is a tripwire: inspect and revise plan for same-owner growth; ownership changes require design reapproval. No shared shape gate for this cluster was identified; extend a discovered applicable shared gate instead if implementation finds one.

## Claims and falsification

Each row is the numbered claim list. All claims are discharged by the single owning final checkpoint in plan.md; baseline evidence remains historical below. Behavioral fences are consumer-observable; the shape oracle is separate from production tests.

| # / Claim | Input shape | Falsifier and distinguishing control | Independent oracle | Named mutation (expected red) | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|
| C1 Legacy decisions preserve exact ID, cancellation and trust shape. | Selected trust Some/None, Cancel, same-kind options, foreign ID | Run existing converter fences; assert exact JSON, warning and ID, not mere success. Distinct option IDs distinguish accidental first-option selection. | Existing captured qo13 replies and literal expected JSON independent of converter | convert/mod.rs: replace selected ID with first offered ID; exact-ID fence must report different optionId | Existing from_permission_response_* tests plus qo13 behavioral replay | Focused core test | PASS — plan.md final checkpoint |
| C2 Only KAS + offered reject_once + nonblank reason emits response-level metadata. | Every engine/option kind; foreign ID; absent/empty/whitespace/nonblank Unicode reason | Serialize eligibility matrix and drive SDK request/reply. Assert exact top-level reason, exact outcome, nested trust unchanged. Positive eligible row proves absence cases are not a dead code path. | Captured KAS response dictionary plus raw JSON assertions on SDK response | convert/kas.rs: omit response-level reason metadata; positive wire fence must show missing _meta. Separately replace RejectOnce guard with unconditional acceptance; negative RejectAlways row must fail | rejection_feedback_wire_matrix and bridge permission-feedback round trip | Focused core tests, kas feature | PASS — plan.md final checkpoint |
| C3 Feedback entry is optional and restricted to the selected KAS reject_once. | Request capability true/false; all option kinds; empty/multi options; SelectOption/SelectTrust/EnterReason | UiState entry matrix: eligible case enters without response; others stay unchanged; ordinary Enter still sends Selected immediately. Same displayed predicate checked by render hint. | Spec decision table and explicit request fixtures | traits.rs: remove capability gate from eligibility predicate; non-KAS entry row must enter incorrectly and fail | UiState/ApprovalState rejection_feedback entry and eligibility tests | Focused UI tests | PASS — plan.md final checkpoint |
| C4 Text editing preserves valid Unicode and atomically enforces the approved scalar bound. | Every key/insertion/cursor and string/boundary cell above | Drive edit/paste interface; compare exact text/cursor/notice to hand-authored expectations; the independent test-only fixture keeps the approved maximum and checks the runtime advertised bound. Under/over bounds are distinguished from control stripping by printable Unicode input. | Hand-authored scalar-edit expectations and approved 4096-scalar fixture, not the editor's counter | feedback_editor.rs: remove insertion refusal; overflow test must observe forbidden text change. Separately lower/raise advertised constant by one; spec-bound case must fail. C4_drop_altgr preserves printable modifier coverage | feedback_editor editing/Unicode/control/bound tests | Focused UI tests | PASS — plan.md final checkpoint |
| C5 Feedback submission/back preserves exact request identity and FIFO. | Blank/nonblank; two requests/sessions with equal tool IDs; Esc/reentry; dropped receiver | Drive UiState with two responders; only first gets exact chosen ID/reason, second pending until next decision; Esc sends nothing and returns selected index, reentry empty. Check plain/trust and failed-send paths separately. | Explicit ordered request/response transcript, distinct option/session IDs | state.rs: submit using approvals.back_mut/pop_back instead of front for feedback path; queue transcript must identify wrong responder/ID. Esc mutation to terminal Cancel must fail no-response control | rejection_feedback_queue_is_fifo_across_sessions_with_equal_tool_ids and approval/trust lifecycle tests | Focused UI tests | PASS — plan.md final checkpoint |
| C6 Real input/render routes feedback without leaking into chat or another modal. | Key/paste/voice/batch; normal/cramped viewport and Unicode cursor | App input checks and real PTY controlled-agent smoke; exact feedback received, normal draft unchanged, cursor/hint/notice in overlay. Other modal paste refusal and normal-input paste success are controls. Preserve existing input-wrap oracle regressions after renderer extraction. | Raw ACP bytes from controlled agent; terminal cell capture; existing independent input-wrap fixture | app.rs: route feedback paste to normal chat input; paste-isolation check must show draft/response mismatch. widgets/approval.rs: bypass editor rendering; buffer check must lose expected reason/cursor. C6_wrong_phase_entry additionally checks the UiState fixture seam | App feedback isolation and approval editor render tests through UiState, existing input-wrap tests | Focused tests + terminal smoke | PASS — plan.md final checkpoint |
| C7 Production shape matches the approved module ledger. | Every touched production module/parent above | Run standalone shape census; deliberate disallowed function/key placement must produce C7 path/symbol diagnostic; baseline clean control | Ledger manifest and git source/diff census, independent of production code | Add a new feedback editor-policy function to app.rs; shape oracle must reject function ownership (even under line limit) | .cyril-y628/oracles/shape.py | Standalone Python | PASS — plan.md final checkpoint |

## Non-goals and future work

Permanent non-goals for this feature: generic feedback-editor framework, separate modal controller, v2/third-party feedback promises, local explanation history, voice dictation, logging reason contents, or changing trust/consent semantics. They are excluded because the approved feature is one optional KAS permission response, not a new policy/storage abstraction. Existing independent future work: cyril-gn07 (consent-scope research), cyril-levc (stall-clock behavior), cyril-p7kp (unknown-kind watch); records verified during specification search. No acceptance obligation is deferred to those records.

## Falsifier run log

2026-09-26, production at 83b1ee65 (claim-only delta from main): `env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo test -p cyril-core from_permission_response_ -- --nocapture` in the feature worktree: 4 passed, 0 failed; artifact://26. C1 PASS for the legacy path before changes. An earlier invocation did not reach Cargo tests because inherited CARGO_TARGET_DIR was empty; corrected only the command environment, not repository config. Applicable restored-green C1 evidence must be produced after implementation.

LSP prerequisite attempted: PermissionResponse references and hover returned no information despite configured rust-analyzer; fallback source-reference search enumerated constructors/matches. Tool issue reported. No public change will rely on the empty reference result as completeness evidence.

## Approval

Requester design approval (verbatim): "Approve design"
Date: 2026-09-26
Approved risk acceptances: N/A — approved risk: fixed 4,096-Unicode-scalar explanation ceiling with no configuration knob. No regression fence waivers.

2026-09-26 amendment approval (verbatim): "Use Ctrl+J reliably". Crossterm 0.29 raw-mode parsing maps CR to unmodified Enter and LF to Ctrl+J; Cyril does not negotiate enhanced keyboard reporting. C4/C6 now require a real raw-LF newline path; the editor accepts Ctrl+J and still accepts explicitly reported Shift+Enter. No new startup/protocol responsibility or accepted risk is introduced; all other design approvals remain applicable.

Closure clarification: actual fence names and the test-local approved-bound fixture are now named above; “scalar-edit expectations” denotes the same hand-authored independent output oracle, not a production-derived model. R1/R2 repaired implementation/proof drift without changing approved behavior, ownership, interfaces or risk. Their additional modifier/phase mutations strengthen the existing C4/C6 obligations. Final isolated conformance is retained in conformance.md; review-decisions.md records all dispositions.

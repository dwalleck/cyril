Frozen reconstruction from the assembled production diff, including untracked `crates/cyril-ui/src/feedback_editor.rs` and `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/rejection_feedback.rs`.

## 1. Changed interfaces and responsibilities

- Core domain transport adds `PermissionRequest.can_reject_with_reason` and `PermissionResponse::RejectWithReason { option_id, reason }` at `crates/cyril-core/src/types/event.rs:511-522` and `crates/cyril-core/src/types/event.rs:570-590`. The exact `PermissionOptionId` remains the response identity.
- `DomainMediator::handle_permission` derives capability from the immutable bound engine plus compile-time KAS availability, carries it to the UI, and captures the same engine for return conversion at `crates/cyril-core/src/protocol/domain_mediator/mod.rs:653-713`.
- Generic `from_permission_response` now accepts `AgentEngine`, maps both plain and reasoned rejection to the standard ACP selected outcome, preserves ordinary trust metadata, and delegates only optional KAS metadata at `crates/cyril-core/src/protocol/convert/mod.rs:342-402`. The extracted foreign-ID warning policy is at `crates/cyril-core/src/protocol/convert/mod.rs:405-420`.
- KAS-specific encoding is isolated in `attach_rejection_metadata`, which rechecks KAS engine, nonblank reason, exact offered ID, and `reject_once` kind before adding response-level `_meta.kiro.rejectionReason` at `crates/cyril-core/src/protocol/convert/kas.rs:21-52`.
- UI contracts add `ApprovalPhase::EnterReason`, re-export the opaque `RejectionFeedback` view, and centralize eligibility in `ApprovalState::can_reject_with_reason` at `crates/cyril-ui/src/traits.rs:484-553`.
- New private `feedback_editor.rs` owns UTF-8 byte-cursor movement, scalar counting, CR/CRLF normalization, control filtering, multiline key interpretation, and the 4,096-scalar atomic ceiling (`crates/cyril-ui/src/feedback_editor.rs:4-29`, `crates/cyril-ui/src/feedback_editor.rs:31-134`, `crates/cyril-ui/src/feedback_editor.rs:136-292`). Mutators and `FeedbackAction` are crate-private; public methods are observational.
- `UiState` remains owner of the FIFO approval queue, phase transitions, and oneshot responder. It copies capability on enqueue (`crates/cyril-ui/src/state.rs:1595-1606`), routes feedback entry/key/paste/back operations (`crates/cyril-ui/src/state.rs:2086-2202`), and sends plain `Selected` for blank feedback or `RejectWithReason` for nonblank feedback (`crates/cyril-ui/src/state.rs:2205-2312`).
- The approval widget keeps its entry-point signature but adds new-phase dispatch, an eligibility hint, and reason-editor rendering (`crates/cyril-ui/src/widgets/approval.rs:11-30`, `crates/cyril-ui/src/widgets/approval.rs:154-239`, `crates/cyril-ui/src/widgets/approval.rs:244-361`).
- The existing input renderer is deepened rather than duplicated: `text_cursor_lines` extracts the existing wrapping/cursor-window projection and is reused by both chat and feedback (`crates/cyril-ui/src/widgets/input.rs:34-130`).
- `App` adds terminal routing only: paste is routed to feedback rather than chat, and approval keys delegate to feedback while that phase is active (`crates/cyril/src/app.rs:1639-1660`, `crates/cyril/src/app.rs:1757-1791`).

## 2. Clusters and separation rule

1. **Wire/domain:** `types/event.rs`, `domain_mediator/mod.rs`, `convert/mod.rs`, `convert/kas.rs`. The mediator owns trusted engine-derived capability, generic conversion owns ACP outcome identity, and only the KAS adapter owns KAS metadata names and eligibility.
2. **Editor:** `feedback_editor.rs`. Text/cursor/filter/limit invariants are request-local and independent of queue, responder, rendering, and ACP.
3. **Approval orchestration:** `traits.rs` and `state.rs`. Phase, FIFO order, exact selected option identity, and responder ownership remain in `UiState`; the editor never sends responses.
4. **Rendering:** `widgets/approval.rs` and `widgets/input.rs`. Renderers receive read-only state and share one wrapping/cursor implementation; they do not mutate phase or encode wire data.
5. **Runtime routing:** `app.rs`. App decides which surface owns an event, then delegates editing and approval transitions.

## 3. Dependency direction and concrete adapters

Forward: ACP request → `DomainMediator` → core `PermissionRequest` → bridge channel → `App` → `UiState`/`ApprovalState` → read-only widget. Return: `RejectionFeedback` → `UiState` response construction → oneshot `PermissionResponse` → mediator → generic ACP converter → KAS metadata adapter → ACP response. The channel and return handoff are at `crates/cyril-core/src/protocol/domain_mediator/mod.rs:681-713`; the renderer sees only the queue front through `TuiState::approval` at `crates/cyril-ui/src/state.rs:312-314` and `crates/cyril-ui/src/render.rs:213-245`.

Concrete adapters are existing `Engine::kind` (`crates/cyril-core/src/protocol/engine.rs:144-151`, KAS implementation `:270-273`), mediator ACP→domain assembly, generic domain→ACP conversion, KAS response-metadata attachment, App event→UiState routing, and ratatui state→cells rendering. No new one-implementation adapter trait was introduced.

## 4. Pass-through and hypothetical seams

- `can_reject_with_reason` is intentional pass-through data, but the UI consumes it jointly with selected option kind (`crates/cyril-ui/src/traits.rs:543-553`).
- `RejectWithReason` passes through the oneshot unchanged until generic conversion; the cfg-disabled adapter explicitly returns the response unchanged (`crates/cyril-core/src/protocol/convert/mod.rs:342-363`).
- `traits.rs` re-exports the concrete editor because it appears in public `ApprovalPhase`, while construction/mutation remain private (`crates/cyril-ui/src/traits.rs:7`, `crates/cyril-ui/src/feedback_editor.rs:38-45`, `:76-134`). This is concrete exposure, not a second owner.
- A possible future editable-buffer seam is not present. Feedback repeats the low-level UTF-8 backspace/delete/left/right mechanics in chat input (`crates/cyril-ui/src/state.rs:1691-1757`), but feedback has distinct filtering, atomic ceiling, line movement, and no autocomplete. The actual shared seam is rendering via `text_cursor_lines`; extracting a generic editor now would be hypothetical.

## 5. Parent-file production growth

- `state.rs` grows within its existing approval state-machine section; this is phase/queue/responder responsibility, not a new unrelated responsibility (`crates/cyril-ui/src/state.rs:2086-2343`).
- `widgets/approval.rs` grows a third phase within the existing permission-modal renderer (`crates/cyril-ui/src/widgets/approval.rs:11-30`, `:244-361`).
- `app.rs` growth is routing only and reuses the existing input-batch boundary that prevents a buffered second decision from landing on the next queued request (`crates/cyril/src/app.rs:1584-1601`, `:1639-1660`, `:1757-1791`).
- `convert/mod.rs` remains the generic permission converter; `convert/kas.rs` is the existing concrete KAS wire owner; `widgets/input.rs` extracts an existing renderer algorithm.
- `render.rs`, `floor_tests.rs`, `widgets/toolbar.rs`, and `tests/modal_theme.rs` contain fixture-only field updates. `probe_qo13.rs` is test-only (`crates/cyril-core/src/protocol/convert/mod.rs:10-11`).
- The substantive new editing responsibility is isolated in the private leaf registered at `crates/cyril-ui/src/lib.rs:5`.

## 6. Changed-production symbol inventory

- **Cross-crate/public:** `PermissionRequest::can_reject_with_reason`; `PermissionResponse::RejectWithReason`; re-exported `RejectionFeedback` observation API; `ApprovalPhase::EnterReason`; `ApprovalState::can_reject_with_reason` field/predicate; public `UiState` feedback routing methods.
- **Crate-private additions:** `FeedbackAction`; editor construction/mutation; cfg-paired generic `attach_rejection_metadata`; KAS `attach_rejection_metadata`; `input::text_cursor_lines`.
- **Private additions:** ceiling/notice/normalization and cursor helpers; `UiState::approval_submit_feedback`; `approval::render_reason_phase`.
- **Changed existing symbols:** `from_permission_response`; `DomainMediator::handle_permission`; `UiState::{show_approval, approval_select_prev, approval_select_next, approval_confirm, approval_cancel}`; `approval::render`; `App::{handle_terminal_event, handle_approval_key}`.
- **Existing siblings reused:** typed `PermissionOptionId`/`PermissionOptionKind`; bound `Engine::kind`; private `VecDeque` and oneshot responder; overlay priority; terminal input-batch boundary; `modal::place`; approval preview; `wrapped_rows` and cursor-follow rendering.

## 7. Tests reaching past interfaces

- Editor unit tests call private mutation methods directly (`crates/cyril-ui/src/feedback_editor.rs:300-433`).
- Approval widget tests directly construct `ApprovalState`, `ApprovalPhase::EnterReason`, and `RejectionFeedback` (`crates/cyril-ui/src/widgets/approval.rs:477-500`, `:560-695`).
- UiState tests manually set `can_reject_with_reason = true`, bypassing engine derivation (`crates/cyril-ui/src/state.rs:7486-7527`, `:7599-7690`).
- App tests inject local requests and invoke private event handlers, bypassing bridge/converter (`crates/cyril/src/app.rs:3864-4001`).
- Converter matrix tests directly construct impossible UI combinations to fence generic/KAS encoding (`crates/cyril-core/src/protocol/convert/mod.rs:1659-1800`).
- The untracked bridge contract uses the real mediator and serialized wire but manually sends `RejectWithReason`, bypassing the UI/editor (`crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/rejection_feedback.rs:5-94`). Thus coverage is layered rather than one complete UI-to-agent test.

## 8. Reconstructed ledger

| Path | Responsibility | Interface owner |
|---|---|---|
| `crates/cyril-core/src/types/event.rs` | Domain capability and reason-bearing response vocabulary | Core permission domain types |
| `crates/cyril-core/src/protocol/domain_mediator/mod.rs` | Derive trusted capability; bridge request/response | `DomainMediator` |
| `crates/cyril-core/src/protocol/convert/mod.rs` | Generic ACP permission outcome and feature dispatch | Generic converter |
| `crates/cyril-core/src/protocol/convert/kas.rs` | Validate/encode KAS rejection metadata | KAS wire adapter |
| `crates/cyril-ui/src/feedback_editor.rs` | Bounded request-local multiline editor invariants | `RejectionFeedback` |
| `crates/cyril-ui/src/lib.rs` | Private module registration | UI crate root |
| `crates/cyril-ui/src/traits.rs` | Approval phase/read-model eligibility contract | `ApprovalPhase` / `ApprovalState` |
| `crates/cyril-ui/src/state.rs` | FIFO approval queue, transitions, responder | `UiState` |
| `crates/cyril-ui/src/widgets/input.rs` | Shared text/cursor rendering | Input widget |
| `crates/cyril-ui/src/widgets/approval.rs` | Option/trust/reason modal rendering | Approval widget |
| `crates/cyril/src/app.rs` | Terminal key/paste ownership and delegation | `App` |

## Correctness/security observations

Reason identity, FIFO promotion, blank/nonblank semantics, engine authority, and generic-vs-KAS encoding are correctly separated and revalidated on the inspected path. One concrete keyboard defect remains: printable AltGr characters are silently consumed by the new modifier filter (finding below). A separate comparison point, whose intended behavior is undetermined from production code, is typed Tab: paste normalization deliberately preserves `\t`, but `KeyCode::Tab` falls through as consumed (`crates/cyril-ui/src/feedback_editor.rs:97-134`, `:268-281`). The fresh design reviewer should adjudicate whether typed Tab belongs to the editor contract rather than assuming either behavior.

## Structured findings

[
  {
    "title": "Accept AltGr characters in the feedback editor",
    "body": "On Windows, crossterm reports printable characters produced with AltGr as `KeyCode::Char` carrying `CONTROL | ALT` (and sometimes `SHIFT`), but this branch accepts only empty modifiers or exactly `SHIFT`; those events therefore fall through to `Consumed` without inserting anything. Users of keyboard layouts that require AltGr cannot type common characters such as `@`, `\u20ac`, or braces in rejection reasons, even though the existing chat editor accepts every `KeyCode::Char`; allow the AltGr modifier combinations for printable character insertion.",
    "priority": 2,
    "confidence": 0.98,
    "file_path": "crates/cyril-ui/src/feedback_editor.rs",
    "line_start": 97,
    "line_end": 100
  }
]

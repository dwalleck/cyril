# Protocol Parity — Deferred Followup Items

Issues identified during code review of tasks #6–#8 that were not addressed in the initial implementation. None are blocking, but all improve robustness.

## Design Improvements

### Typed picker dispatch instead of string comparison

**Location:** `crates/cyril/src/app.rs` — `handle_picker_key` and `SessionsListed` handler

The picker's title string `"Resume session"` is used as the discriminant for routing picker confirmation to `BridgeCommand::LoadSession` vs `BridgeCommand::ExecuteCommand`. A typo or rename on either side silently routes to the wrong branch. The `else if let Some(session_id) = self.session.id()` fallback also silently drops the selection when no session is active.

**Recommendation:** Introduce a `PickerKind` enum (`ResumeSession` vs `CommandOption(String)`) so the dispatch is exhaustive at compile time. This would require changes to `show_picker()`, `PickerState`, and `picker_confirm()`.

### `Timestamp` newtype for `SessionEntry.updated_at`

**Location:** `crates/cyril-core/src/types/session_entry.rs`

`updated_at: Option<String>` carries an implicit ISO 8601 format invariant that the type does not express. A thin `Timestamp` newtype would communicate intent and provide a hook for future validation without breaking the API.

## Test Gaps

### App-level picker wiring untested

The two new behaviors in `app.rs` have no test coverage:
- `SessionsListed { sessions: non-empty }` → `show_picker("Resume session", ...)` with formatted labels
- `picker_confirm()` returning `"Resume session"` → `BridgeCommand::LoadSession` (not `ExecuteCommand`)
- `SessionsListed { sessions: [] }` → "No previous sessions found." system message

These are testable through the `event_routing.rs` pattern.

### `parse_session_list` edge case: JSON null title

No test covers `{"sessionId": "x", "title": null}`. The current code handles it correctly (`Value::Null.as_str()` returns `None`), but an explicit test prevents regressions.

### Modes reset on new session

No test verifies that `ctrl.modes()` is reset to empty when `SessionCreated` arrives with `available_modes: Vec::new()` — i.e., that modes from a previous session don't carry over.

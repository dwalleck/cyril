# Falsifiable design — cyril-08eh: Render `_kiro/system/notify {level, message}`

## Purpose

KAS 0.17.2 (kiro-cli 2.12.3) emits `_kiro/system/notify {level, message}` — a
connection-scoped notification wired to the agent's `onDelayMessage` hook. Fires
on model-request backoff during ordinary local KAS turns. cyril drops it today;
surface it as a system message with level-aware formatting.

Same family as cyril-3zy4 (`_kiro/error/rate_limit`, merged in PR #59) — the
pattern is identical: converter arm + Notification variant + system message.

## What the probe established

- `kiro/system/notify` → `Ok(None)` (unknown-method drop, kiro.rs:840)
- No existing variant, no converter arm, no UI handler — broader scope than
  cyril-3zy4 (which reused an existing `RateLimited` variant)
- `_kiro/system/` is a PREFIX namespace; unknown sub-methods must still drop

## Architecture

Three new pieces, mirroring the cyril-3zy4 pattern exactly:

1. **`SystemNotifyLevel` enum** (event.rs) — `Info`, `Warning`, `Unknown(String)`.
   Already added in the probe commit (produces `#[derive(Debug, Clone, PartialEq)]`).
2. **`Notification::SystemNotify { level, message }` variant** (event.rs).
   Already added in the probe commit.
3. **Converter arm** (kiro.rs) — `"kiro/system/notify" =>` parsing `{level, message}`
   from params.
4. **UI handler** (state.rs) — `apply_notification` arm calling `add_system_message`
   with level-prefixed text like `"[warning] model is taking longer..."`.

## Classification

**Additive.** New match arm, new variant, new UI handler — removes no constraint,
guard, ordering, or uniqueness property. No removed-invariant sweep applies.

## Key design decision: level styling

The ticket says "render with level-appropriate styling." Options:

A. Add a new `ChatMessageKind::SystemNotify { level, text }` with dedicated chat
   rendering per level (different colors/styles for info vs warning).
B. Prefix the system message text with `[info]` / `[warning]` and use the existing
   `add_system_message` path (italic, theme.system color).

**Decision: (B) for this PR.** The existing system message rendering is uniform;
adding a new `ChatMessageKind` variant touches the chat widget renderer and the
theme system, which is scope creep. The `[level]` prefix makes the severity
visible without new rendering infrastructure. Real level styling (color, icon,
transient banner) is deferred to a follow-up: **filed as cyril-08eh-styling**
(verify-or-file after merge).

## Input shapes

1. `{level: "info", message: "retry delay"}` — **in scope** → `SystemNotify { level: Info }`
2. `{level: "warning", message: "long wait"}` — **in scope** → `SystemNotify { level: Warning }`
3. `{level: "error", message: "..."}` — **in scope** → `SystemNotify { level: Unknown("error") }` (future-proof)
4. `{message: "..."}` (level absent) — **in scope** → default level `Info` (least surprising)
5. `{level: "info"}` (message absent) — **in scope** → default message `"system notification"`
6. `{level: 5}` (non-string) — **in scope** → `as_str() == None` → default level `Info`
7. `{}` (both absent) — **edge case** → default both → `SystemNotify { level: Info, message: "system notification" }`
8. Unknown `_kiro/system/*` (e.g. `kiro/system/future_method`) — **must still drop** → `Ok(None)`

## Claims

1. `"kiro/system/notify"` with `{level: "info", message}` → `Ok(Some(SystemNotify { level: Info, message }))`.
2. `"kiro/system/notify"` with `{level: "warning", message}` → `Ok(Some(SystemNotify { level: Warning, message }))`.
3. `"kiro/system/notify"` with `{level: "error", message}` → `Ok(Some(SystemNotify { level: Unknown("error"), message }))`.
4. `"kiro/system/notify"` with `{message}` only (no level) → `SystemNotify` with default `Info` level.
5. `"kiro/system/notify"` with `{level}` only (no message) → `SystemNotify` with non-empty default message.
6. Both absent → `SystemNotify` with `Info` level and non-empty default message.
7. Unknown `"kiro/system/other"` → `Ok(None)` (namespace prefix, not catch-all).
8. Applying `SystemNotify` to `UiState` adds a system message formatted as `"[{level}] {message}"` and does NOT clear the busy guard (same as `RateLimited` — non-terminal notification).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | info level converts | call with `{level:"info",msg}`; if ≠ `SystemNotify{Info,msg}`, false | runtime assert variant + level + msg | 1m | passed | `probe_kas_system_notify_info_converts` |
| 2 | warning level converts | call with `{level:"warning",msg}`; if ≠ `SystemNotify{Warning,msg}`, false | runtime assert | 1m | passed | `probe_kas_system_notify_warning_converts` |
| 3 | unknown level → Unknown arm | call with `{level:"error",msg}`; if ≠ `Unknown("error")`, false | runtime assert | 1m | passed | `probe_kas_system_notify_unknown_level_converts` |
| 4 | missing level defaults Info | call with `{msg}` only; if level ≠ Info, false | runtime assert | 1m | passed | `probe_kas_system_notify_missing_level_defaults` |
| 5 | missing message defaults | call with `{level:"info"}` only; if message empty, false | runtime assert | 1m | passed | `probe_kas_system_notify_missing_message_defaults` |
| 6 | both absent → both defaulted | call with `{}`; if level ≠ Info or message empty, false | runtime assert | 1m | passed | `system_notify_empty_payload_defaults` (new) |
| 7 | unknown namespace still drops | call with `"kiro/system/other"`; if ≠ `Ok(None)`, false | runtime assert | 1m | passed | `probe_unknown_kiro_system_still_dropped` |
| 8 | UI adds level-prefixed system msg, busy preserved | apply SystemNotify to UiState in Streaming; assert message contains level prefix, activity unchanged | runtime assert | 2m | pending | `system_notify_adds_level_prefixed_message` (new, state.rs) |

Non-vacuity (buggy implementation each fence catches):
- C1-2: maps level to wrong variant, drops message → fails 1/2.
- C3: rejects unknown level instead of Unknown → fails 3.
- C4-5: panics/returns Err on missing field instead of defaulting → fails 4/5.
- C6: returns `Ok(None)` on empty payload → fails 6.
- C7: prefix-match `kiro/system/` over-catches → fails 7.
- C8: uses `set_activity` instead of just `add_system_message` → fails 8.

## Negative space

1. **No `ChatMessageKind` variant or chat rendering change.** Level styling is
   done via text prefix `[info]` / `[warning]`, not a new message kind with
   per-level colors. Deferred to follow-up (to be filed).
2. **No transient banner / toast.** It is a persistent system message, matching
   the existing `RateLimited` / `CompactionStatus` behavior.
3. **No `_kiro/system/*` catch-all.** Only `system/notify` is handled; unknown
   sub-methods still drop cleanly — each gets its own handling when needed.
4. **No busy-guard change.** `SystemNotify` does not clear `turn_in_flight` or
   `Activity` — it is non-terminal, same as `RateLimited` (cyril-3zy4 claim 5
   pattern).
5. **No `_kiro/safety/*` or `_kiro/mcp/*` handling.** Those are sibling KAS-8
   gaps (cyril-3ald, cyril-nk4o — verified open).

## Tracker references

- cyril-3zy4 (closed, PR #59) — pattern source; RateLimited → system message.
- cyril-3ald (P2, open) — sibling KAS-8 gap, `_kiro/safety/*`.
- cyril-nk4o (P3, open) — sibling KAS-8 gap, `_kiro/mcp/*`.
- Level-styling follow-up → to be filed after merge.

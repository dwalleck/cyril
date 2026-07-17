# Related issues — cyril-08eh prior art

Tracker search (`.rivets/issues.jsonl`) for `system/notify`, `08eh`.

- **cyril-08eh** (this issue): render `_kiro/system/notify {level, message}`.
- **cyril-3zy4** (closed, PR #59): KAS-8 `_kiro/error/rate_limit` — same family,
  identical fix pattern (converter arm + Notification variant + system message).
  Landed 2026-07-16. The pattern is fresh and reusable.
- **cyril-3ald** (P2, open): `_kiro/safety/*` enforcement gate — sibling KAS-8 gap.
- **docs/kiro-2.12.3-wire-audit.md:22**: confirms trigger (model-request backoff on
  local KAS turns), payload `{level, message}`, levels `info`/`warning`.

## Existing converter gap

Grep for `system/notify` or `SystemNotify` in `kiro.rs` → **zero matches**. The
ACP `_`-strip means `_kiro/system/notify` → `kiro/system/notify`, which falls to
the `other => Ok(None)` unknown-method drop (kiro.rs:840).

## Existing UI gap

No `Notification::SystemNotify` variant exists. `add_system_message` renders all
system messages identically (italic, `theme.system` color — chat.rs:221-224). No
per-message-level styling today. The ticket says "level-appropriate styling" —
this needs either a new `ChatMessage` variant or a level-tagged `System` message.
Decision for the design: embed level in the system text for now (simplest, matches
existing pattern), or add a new `ChatMessageKind` variant with level-aware rendering.

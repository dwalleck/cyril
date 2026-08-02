# cyril-h8zb — tracker prior art

Searched: `rivets list | grep -iE "refus|content.?filter|metadata|CONTENT_FILTERED"` + `rivets show` on hits.

- **cyril-fh06** (closed) — `_kiro.dev/metadata` routing by `params.sessionId`. Already shipped; the
  `session_id` tag on `MetadataUpdated` and App-side scoped routing exist. A refusal frame from a
  subagent session will be diverted before `UiState::apply_notification` sees it — main-toolbar/chat
  refusal handling only ever sees main-session frames. Design must not re-solve routing.
- **cyril-1gim** (closed 2026-08-01) — modeled the full metadata surface; added the known-key
  allowlist in `convert/kiro.rs` where `refusal` + `stopReason` sit recognized-but-ignored with a
  comment pointing at this issue, plus the `to_ext_notification_metadata_refusal_and_stop_reason_not_flagged`
  fence. This issue replaces that ignore with real parsing; the fence must be updated, not deleted.
- **cyril-9akh** (open, P3) — streamed agent text can render AFTER TurnCompleted (notification-vs-response
  ordering race). Relevant negative space: the refusal system message must not assume TurnCompleted
  arrives last; ordering probe shows metadata immediately precedes the prompt response, but the race
  issue means "commit at metadata arrival", not "commit at turn end", is the robust placement.
- **cyril-1gim's related edge → this issue** is the only tracker link touching `refusal` itself;
  no duplicate of this feature exists.

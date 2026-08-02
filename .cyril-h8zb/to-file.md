# Issues to file at close-out (jsonl stays out of this branch)

1. **KAS refusal surface** (P3/P4, kas): tui.js 2.15.0 KAS adapter normalizes `i?.refusal` from a
   session update and emits `model_refusal` with hardcoded `stopReason:"CONTENT_FILTERED"`
   (probe site 1, `.cyril-h8zb/findings.md`). Cyril's KAS path (`convert/kas.rs` /
   `session_info_update` handling) has no refusal parsing — a KAS-side refusal is dropped the
   same way the v2 one was pre-h8zb. Needs: locate the KAS wire shape (covenant first), then
   mirror the v2 rendering. Discovered-from: cyril-h8zb.

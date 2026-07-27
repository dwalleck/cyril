# KAS `turn_end` fixture provenance

These fixtures are sanitized copies of genuine inbound frames in
`experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl`:

- `turn-end-cancelled.json`: source line 84, timestamp `1783041453801`.
- `turn-end-end-turn.json`: source line 525, timestamp `1783042242922`.

Sanitization changes only `params.sessionId` from the captured session UUID to
`sess_<sanitized>`. No field was added, removed, renamed, or reordered within
the captured `msg` object. In particular, neither capture contains a native
turn identifier; the fixtures do not invent one.

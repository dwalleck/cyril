# Related issues — cyril-a71q

Bounded search completed 2026-07-12 against the repository-local Rivets tracker (`.rivets/issues.jsonl`). Search terms: `stale`, `turn-seq`, `turn id`, `turn-id`, `TurnCompleted`, `turn_in_flight`, `busy guard`, `stop_reason`, `session_info_update`, `turn_end`, `prompt-response`, `rate_limit`, `shutdown`, `disconnect`, and `reconnect`. The search was limited to the local tracker and the directly named source/roadmap records; no production probe or sibling-design retry was run.

## Directly related

- **cyril-a71q** (open, P3) — target issue. Requires per-turn ownership trustworthy across sessions; a stale same-session or cross-session terminal event must not clear or be forwarded as the active turn's completion. Its notes identify the unowned KAS `turn_end` seam and require joint consideration with cyril-pnwb.
- **cyril-j16p** (closed, P2) — origin of the current KAS dual-completion dedup and busy-clear seam. It established that KAS `session_info_update.kind == "turn_end"` is primary for liveness, while the prompt response is secondary and may be late; v1/v2 behavior remains supported.
- **cyril-pnwb** (open, P3) — shares the observer seam. It asks which of KAS's two same-turn terminal sources supplies authoritative `stop_reason`, especially on cancellation. The requester kept precedence out of cyril-a71q; the identity work must preserve both source/reason inputs so that later decision remains possible.
- **cyril-3zy4** (open, P2) — blocked consumer. Its rate-limit path must release the busy guard, making the stale-completion race reachable. The durable failed-falsifier finding is that identical session-only projected traces require opposite decisions for late completion A and real completion B.

## Boundary and interaction issues

- **cyril-l7tw** (closed, P2) — fixes prompt/transport failure ordering: `BridgeError` → one `TurnCompleted` → `BridgeDisconnected` for mid-turn engine death. Per-turn ownership must preserve that owned terminal marker and must not let a stale marker trigger deferred disconnect completion.
- **cyril-9akh** (open, P3) — streamed agent notifications can theoretically trail `TurnCompleted`. This is a separate notification-ordering problem; cyril-a71q owns terminal identity/dedup, not stream reordering.
- **cyril-gua0** (open, P4) — reconnect/respawn after disconnect is explicitly separate. Current bridge spawn/run-loop channels are one-shot, so a fresh bridge process has no queued terminal events from the prior bridge; reconnect UX and history preservation are out of scope here.
- **cyril-3lh8** (closed, P3) — cancel reaps live KAS terminals with kill semantics. Its cancel targeting uses the in-flight session; this issue must preserve cancellation/error terminal ownership but does not change child-process reap policy.
- **cyril-2vcc** (closed, P2) — documents the visible damage of an incorrectly cleared busy guard (later prompts are accepted/rejected at the wrong time). Its input/steering UX is already resolved and is not reopened here.

## Search conclusion

No second feature was folded into cyril-a71q. Rate-limit rendering/retry UX remains cyril-3zy4; the requester explicitly kept stop-reason authority in cyril-pnwb; stream ordering remains cyril-9akh; reconnect remains cyril-gua0; terminal reap remains cyril-3lh8.

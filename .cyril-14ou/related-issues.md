# cyril-14ou — related tracker prior art (probe step 0)

- **cyril-bh7g** (closed) — the motivating research: wire-captured 16-min backend stall,
  zero client-visible traffic, turn completed late. Evidence:
  `experiments/conductor-spike/kas-turn-stall-2.16.2.{md,jsonl}`.
- **cyril-w0vy** (P1, open) — v2 sibling: security-filter marker chunk then no response,
  `is_busy` wedges forever. Different fix (marker detection + synthesized completion);
  a stalled-turn indicator would also make ITS symptom visible while unfixed.
- **cyril-740a** (P3, open) — "wire a host-callback family onto the mediator's
  cancel/shutdown seam (currently no production opt-in)" — a cancel/shutdown seam
  already exists on the mediator; items 3/4 should land on it, not beside it.
- **cyril-lvok / cyril-el3x / cyril-mbio** (open) — ProcessTree drop semantics
  (killpg/process_group coupling) for host-shell terminals: existing in-repo kill
  machinery; item 4 (KAS child reaping) should mirror its idioms (and its Windows gap
  el3x is a caution).
- **cyril-3lh8** (closed) — reap a session's live terminals on cancel/turn-end —
  precedent for teardown-owns-cleanup.
- **cyril-pnwb** (P3, open, needs-info) — turn_end vs response stop_reason fidelity on
  KAS cancel; the Q3 cancel probe below may produce its missing live evidence as a
  side effect.
- **cyril-3zy4** (closed) — rate-limited turn must release the busy guard; liveness
  signal must not re-introduce "busy forever" semantics.
- **cyril-l7tw** (closed) — engine death visibility; BridgeDisconnected paths already
  hardened — the stall case is "engine alive but silent", deliberately distinct.
- **cyril-2vcc / cyril-8ej2** (closed) — Enter-while-busy message-drop fixes: the
  stalled-turn UI state must not create a new variant of silently-dropped input.

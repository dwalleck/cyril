# cyril-b4y4 — prove-it-prototype findings

Date: 2026-08-02. Branch: `feat/cyril-b4y4-turn-mediator`.

## Smallest question

For each terminal frame arriving in `run_loop`'s notification arm, which of
the six dispositions does the inline policy (bridge.rs:2124-2281 at HEAD
`981276c`) produce, as a function of `(active_turn, companion)` state and the
frame's `(turn-stamp, session-scope)`?

## Probe

`probes/probe_mediation_model.py` — a ~75-line standalone Python model of the
policy as read from the source. Imports nothing from cyril. Scenario: two KAS
turns on one session covering all six dispositions, terminal orders taken
from the live-confirmed KAS wire behavior (turn 1: `turn_end` first, response
second — the captured order; turn 2: response first — the order the dedup
comment also claims to handle). Output: `probes/probe-output.txt`.

## Oracle

The REAL `run_loop` driven through the committed test harness (KAS engine,
fake agent with `block_prompt`, frames injected via the cyril-upjh inbound
seam, `SystemNotify` markers proving FIFO position of invisible outcomes,
stderr tracing subscriber capturing the loop's own debug lines as disposition
labels). Different mechanism entirely: the probe is my model; the oracle is
the shipped code executing. Source: `probes/oracle_scenario.rs` (temporary
test, removed from bridge.rs after capture — NOT a fence). Output:
`probes/oracle-output.txt`.

## Agreement — item by item, 10/10

| Step | Probe (model)                        | Oracle (real run_loop)                                            |
| ---- | ------------------------------------ | ----------------------------------------------------------------- |
| p1   | PROMPT-ACCEPTED turn#0               | PROMPT-ACCEPTED turn#0                                             |
| f1   | RELEASE-BY-SCOPE turn#0, forward     | `turn completed (wire turn_end) owner=turn#0` + forwarded          |
| f2   | ABSORB synthesized turn#0            | `absorbed ... first=(Wire, EndTurn) second=(Synthesized, EndTurn)` |
| p2   | PROMPT-ACCEPTED turn#1               | PROMPT-ACCEPTED turn#1                                             |
| f3   | DROP-STALE turn#0                    | `dropping stale completion stale_owner=turn#0 active=Some(1)`      |
| f4   | FORWARD-FOREIGN, main untouched      | `forwarding foreign terminal; main turn untouched` + forwarded     |
| f5   | RELEASE-BY-OWNER turn#1, forward     | `turn completed owner=turn#1` + forwarded                          |
| f6   | ABSORB wire sess_fake-0              | `absorbed ... first=(Synthesized, EndTurn) second=(Wire, EndTurn)` |
| f7   | DROP-UNOWNED                         | no debug line at all + not forwarded                               |
| p3   | PROMPT-ACCEPTED turn#2               | PROMPT-ACCEPTED turn#2                                             |

## What I learned (not obvious before the probe ran)

1. **The unowned drop is silent.** Five of the six dispositions emit a
   tracing line; the sixth (`None => continue` at bridge.rs:2260 — no active
   turn, no companion match) emits nothing. The oracle had to prove it by
   marker position alone. Under CLAUDE.md's silent-failure rules the
   extraction should make `Disposition::DropUnowned` a logged, first-class
   outcome — and this is precisely cyril-ri8q's complaint arriving from the
   other direction.
2. **The two absorb log lines are truly symmetric evidence records** — the
   oracle shows `first`/`second` swap cleanly between the two orders, i.e.
   the pnwb `{source, reason}` evidence trail works in both arrival orders
   today and must survive the extraction intact.
3. **The unstamped-release arm registers its Synthesized companion
   unconditionally** (no `owes_wire_companion` gate — only the stamped arm
   gates). Model captured it, oracle confirmed: correct today because only
   KAS emits unstamped terminals, but it is an *implicit* engine assumption
   the mediator type must make explicit (the source-authority constraint in
   the issue note).

## Hard-gate checklist

- [x] Probe written, runs against the real repo state
- [x] Oracle defined (real run_loop via harness) and produces output
- [x] Probe and oracle agree on a non-trivial slice (10/10 lines, all six
      dispositions, both terminal orders)
- [x] Learned something new: the silent unowned drop (item 1 above)

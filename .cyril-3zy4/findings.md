# Findings — cyril-3zy4 prove-it-prototype

## Smallest question

Does cyril's converter route the KAS-dialect `_kiro/error/rate_limit` to
`Notification::RateLimited`, or drop it? (And does the engine path drop it too?)

## Probe

In-crate `#[cfg(test)]` unit tests (the `convert`/`engine` modules are
`pub(crate)`, so an external `tests/` probe can't reach them — the probe lives
next to the converter it measures). Probe source kept in-repo as the permanent
fences; see `crates/cyril-core/src/protocol/convert/kiro.rs`
(`probe_kas_dialect_rate_limit_*`, `probe_v2_dialect_rate_limit_still_converts`)
and `crates/cyril-core/src/protocol/engine.rs` (`probe_kas_engine_routes_rate_limit`).

Result (BEFORE fix — the probe's job is to fail):

```
probe_kas_dialect_rate_limit_converts              FAILED  Ok(None)
probe_kas_dialect_rate_limit_missing_message_defaults FAILED  Ok(None)
probe_v2_dialect_rate_limit_still_converts         ok      (control)
probe_kas_engine_routes_rate_limit                 FAILED  Ok(None)
```

## Oracle (independent mechanism: grep, not the converter)

`grep 'error/rate_limit" =>' crates/cyril-core/src/protocol/convert/kiro.rs`
→ **exactly one match arm**: `"kiro.dev/error/rate_limit"` at line 644. No
`"kiro/error/rate_limit"` arm exists, so `match method` falls to the
`other =>` unknown-method drop (`Ok(None)`).

## Agreement

Probe (runtime) and oracle (static grep) agree on every slice: the KAS-dialect
method is dropped; the v2-dialect method converts. **Agree.**

## Wire-naming resolution (the fact the whole design stands on)

Source-verified in three places (`kas/auth.rs:20-24`, `kas/terminal_io.rs:411-414`,
`convert/kas.rs:483-484`): the `agent-client-protocol` crate strips the **single
leading `_`** inbound. Therefore:

| wire method                | arrives at converter as     | handled? |
|----------------------------|-----------------------------|----------|
| `_kiro.dev/error/rate_limit` (v2)  | `kiro.dev/error/rate_limit` | ✓ yes (line 644) |
| `_kiro/error/rate_limit` (KAS)     | `kiro/error/rate_limit`     | ✗ dropped |

The gap is **only** the `.dev` infix. The payload `{message: string}` is
identical across dialects (docs/kiro-acp-protocol.md:2699 confirms
`{sessionId, message}`), so the existing `RateLimited { message }` variant, the
UI arm (`state.rs:529`), and the message-default logic are all reusable verbatim.

## What I learned (that wasn't obvious before probing)

The gap is **not** "the whole rate_limit pipeline is missing" — it is a
**one-string method-name dialect gap**. Every downstream piece (domain variant,
UI system-message arm, missing-message default, test shape) already exists and
is tested for the v2 dialect. The fix is a single additional match arm
(`"kiro/error/rate_limit"`) that shares the v2 arm's body — plus the
turn-completion interaction check the ticket calls out (see below).

## Busy-guard interaction (ticket bullet 3) — RESOLVED from source

The ticket's third bullet ("a rate-limited turn must release the busy guard")
is satisfied by the **existing KAS-2a mechanism**, no new work:

- `turn_in_flight` (bridge.rs:711) clears **only** when the loop observes
  `TurnCompleted` — never on any other notification (bridge.rs:1684-1693).
- `RateLimited` (state.rs:529) only calls `add_system_message`; it touches
  neither the bridge `turn_in_flight` nor the UI `Activity` busy guard.
- This is **correct and must be preserved**: a rate-limit the backend retries
  is non-terminal (docs/kiro-acp-protocol.md:2700 shows `retry_warning` firing
  on the stream when a retry kicks in), so the busy guard must NOT clear on
  `rate_limit` alone. Clearing there would prematurely free the input while the
  turn is still retrying.
- If the rate-limited turn ultimately ends, KAS emits `turn_end` →
  `TurnCompleted` → the loop clears the guard (fence
  `kas_turn_end_completes_without_prompt_response`, bridge.rs:3234).

**Design consequence:** the `RateLimited` converter arm must NOT synthesize a
`TurnCompleted` or clear any busy state. It is a pure informational system
message. The falsifiable claim is the *negative* one: rate_limit does not
clear busy / does not emit TurnCompleted.

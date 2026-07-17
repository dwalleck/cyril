# Related issues — cyril-3zy4 prior art

Tracker search (`.rivets/issues.jsonl`) for `rate_limit`, plus converter audit.

- **cyril-3zy4** (this issue): KAS-8 — surface `_kiro/error/rate_limit`.
- **cyril-08eh** (P3, open): render `_kiro/system/notify {level,message}` — same
  `_kiro/*` dialect family, same unknown-variant drop. Not this ticket.
- **cyril-3ald** (P2, open): `_kiro/safety/*` enforcement gate — sibling KAS-8 gap,
  no probe exists yet. Not this ticket.
- **cyril-l7tw** (closed): bridge engine-death visibility; its design.md lists
  "No KAS rate-limit surfacing" as a deliberate deferral → this issue.

## Key existing-code finding (the crux)

cyril **already fully handles** the legacy dialect `kiro.dev/error/rate_limit`
(no leading underscore):

- Converter arm: `crates/cyril-core/src/protocol/convert/kiro.rs:644`
  → `Ok(Some(Notification::RateLimited { message }))`, defaults message to
  "Rate limit exceeded" when absent.
- Domain variant: `crates/cyril-core/src/types/event.rs:116` `RateLimited { message }`.
- UI arm: `crates/cyril-ui/src/state.rs:529` → `add_system_message("Rate limited: …")`.
- Tests: `convert/mod.rs:1429` `parse_rate_limit_error` (+ missing-message case),
  `state.rs:3375` `rate_limited_adds_system_message`.

## Wire-dialect evidence

`docs/kiro-2.7.0-wire-audit.md:61`: as of kiro-cli 2.7.0, **every** extension
notification moved from `kiro.dev/*` to `_kiro/*` on the live wire. So the
handled arm is dead for KAS — the live notification arrives as
`_kiro/error/rate_limit` and falls into the KAS unknown-variant drop.

**Hypothesis to probe:** the only change needed is the method-name match
(`kiro.dev/…` → `_kiro/…`); payload shape `{message}` is unchanged, so the
existing `RateLimited` variant + UI arm should be reusable verbatim.

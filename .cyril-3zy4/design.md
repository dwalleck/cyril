# Falsifiable design — cyril-3zy4: Surface `_kiro/error/rate_limit`

## Purpose

KAS (kiro-cli ≥2.7.0) emits rate-limit notices as `_kiro/error/rate_limit`.
cyril handles only the legacy v2 dialect `_kiro.dev/error/rate_limit`; the KAS
form falls into the converter's unknown-method drop, so the user sees a stalled
turn with no explanation. Close the one-string dialect gap so the KAS form
produces the existing `Notification::RateLimited` and its system message —
without disturbing the busy guard or the legacy path.

## What the probe established (ground truth)

- The ACP crate strips the single leading `_` inbound. Wire `_kiro/error/rate_limit`
  arrives at the converter as `kiro/error/rate_limit`; wire `_kiro.dev/error/rate_limit`
  arrives as `kiro.dev/error/rate_limit`.
- Only one match arm exists (`kiro.dev/error/rate_limit`, kiro.rs:644). The KAS
  form routes to `Ok(None)` — confirmed at runtime by the probe AND statically
  by grep (oracle). Probe and oracle agree.
- Payload shape `{message: string}` is identical across dialects
  (docs/kiro-acp-protocol.md:2699). The `RateLimited { message }` variant, the
  UI arm (state.rs:529), and the missing-message default are reusable verbatim.
- `RateLimited` touches neither the bridge `turn_in_flight` nor UI `Activity`;
  `turn_in_flight` clears only on `TurnCompleted` (bridge.rs:1684-1693).

## Architecture

One new match arm in `to_ext_notification` (kiro.rs) matching
`"kiro/error/rate_limit"`, sharing the v2 arm's conversion body. No new type,
no new UI path, no bridge change. `KasEngine.convert_ext_notification` already
delegates to this function (engine.rs:143), so the engine path is fixed by the
same arm — no engine change required.

## Classification

**Additive.** Adds a new reachable match arm; removes no constraint, guard,
ordering, or uniqueness property. No removed-invariant sweep applies (step 2b
skipped with justification: the change relaxes nothing — it only stops one
previously-dropped method from being dropped).

## Input shapes

The converter arm's input is `(method: &str, params: &Value)`:

1. `method = "kiro/error/rate_limit"`, `params.message` = non-empty string → **in scope**.
2. `method = "kiro/error/rate_limit"`, `params.message` absent → **in scope** (default message).
3. `method = "kiro/error/rate_limit"`, `params.message` present but empty `""` → **in scope** (treated as absent → default; matches v2 arm's `unwrap_or` semantics — the v2 arm does NOT filter empty, it uses `unwrap_or`, so an empty string passes through. **Decision: match v2 exactly — `unwrap_or` only, no empty filter — to avoid a second convention.**).
4. `method = "kiro.dev/error/rate_limit"` (legacy), any message → **in scope** (regression control; must keep working).
5. `method` = any other `_kiro/*` unknown → **out of scope** (still dropped by the unknown arm; this ticket is rate_limit only).
6. `params.message` non-string (e.g. number) → **in scope** → `as_str()` yields `None` → default message (same as absent).

Shape 3 note: the v2 arm uses `.and_then(|v| v.as_str()).unwrap_or("Rate limit
exceeded")` — an empty string `""` passes through as `""`. We replicate this
verbatim rather than "improving" it, to keep one conversion body. (Filtering
empty would be a behavior change to the v2 path too — out of scope here.)

## Claims

1. A converter call with `method = "kiro/error/rate_limit"` and a non-empty
   `message` produces `Ok(Some(Notification::RateLimited { message }))` carrying
   that exact message.
2. The same call with `message` absent produces
   `Ok(Some(Notification::RateLimited { message }))` with a non-empty default.
3. The legacy `method = "kiro.dev/error/rate_limit"` still produces
   `RateLimited` (no regression from adding the arm).
4. `KasEngine.convert_ext_notification("kiro/error/rate_limit", …)` routes to
   `RateLimited`, not `Ok(None)` — the engine delegation path picks up the arm.
5. Applying `Notification::RateLimited` to `UiState` produces a system message
   and does NOT change `Activity` away from a busy state (busy guard untouched)
   and does NOT synthesize `TurnCompleted`.
6. The unknown-method drop still drops a genuinely-unknown `_kiro/*` method
   (e.g. `kiro/does/not/exist`) to `Ok(None)` — the new arm does not over-match.

## Cheapest falsifier (run before approval)

Claims 1-4 and 6 are falsified by the probe unit tests already in the tree
(`cargo test -p cyril-core --lib --features kas probe_` and the existing
`parse_rate_limit_error` / `kas_engine_drops_unknown_ext_frame`). Claim 5 is
falsified by the existing `state.rs` test `rate_limited_adds_system_message`
plus a busy-preservation assertion. Cheapest = claims 1 & 6 (pure converter, no
features). See the Falsification table for status.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | KAS method + non-empty msg → `RateLimited{msg}` | call `to_ext_notification("kiro/error/rate_limit", {message})`; if result ≠ `Ok(Some(RateLimited))` with same msg, claim false | grep: exactly one arm per dialect; runtime assert msg equality | 1m | **passed** (post-fix) | unit test `probe_kas_dialect_rate_limit_converts` |
| 2 | KAS method + absent msg → non-empty default | call with `{}`; if `RateLimited.message` empty or variant wrong, claim false | runtime assert `!message.is_empty()` | 1m | **passed** (post-fix) | unit test `probe_kas_dialect_rate_limit_missing_message_defaults` |
| 3 | legacy `kiro.dev/…` still converts | call `to_ext_notification("kiro.dev/error/rate_limit", {message})`; if ≠ `RateLimited`, claim false | runtime assert (existing test `parse_rate_limit_error`) | 1m | **passed** | unit test `probe_v2_dialect_rate_limit_still_converts` + existing `parse_rate_limit_error` |
| 4 | KasEngine routes rate_limit | `KasEngine.convert_ext_notification("kiro/error/rate_limit",…)`; if `Ok(None)`, claim false | runtime assert variant | 2m | **passed** (post-fix) | unit test `probe_kas_engine_routes_rate_limit` (cfg kas) |
| 5 | `RateLimited` leaves busy guard + emits no TurnCompleted | apply to `UiState` in `Activity::Streaming`; if activity changes or a TurnCompleted is produced, claim false | runtime assert activity unchanged + system message added | 2m | **passed** (post-fix) | unit test `rate_limited_preserves_busy_activity` (state.rs) |
| 6 | unknown `_kiro/*` still dropped | call `to_ext_notification("kiro/does/not/exist", {})`; if ≠ `Ok(None)`, claim false | runtime assert (existing `kas_engine_drops_unknown_ext_frame` + `to_ext_notification_unknown_method_returns_none`) | 1m | **passed** | existing `kas_engine_drops_unknown_ext_frame`, `to_ext_notification_unknown_method_returns_none` |

Non-vacuity (buggy implementation each fence catches):
- C1/C2: an arm that matches but maps to the wrong variant, drops the message,
  or forgets the default → fails 1/2.
- C3: an edit that replaces (not adds) the arm, breaking legacy → fails 3.
- C4: an arm added only to the v2-only path (not reachable via KasEngine
  delegation) → fails 4.
- C5: an implementation that "helpfully" clears busy on rate_limit → fails 5.
- C6: a catch-all `_kiro/` prefix match that swallows unknowns → fails 6.

## Negative space

1. **No new Notification variant.** Reuses `RateLimited`; does not model
   retry-after or rate-limit-quota fields (none observed in the payload).
2. **No busy-guard / turn-end change.** Does not clear `turn_in_flight` or
   `Activity` on rate_limit; the turn is non-terminal under retry.
3. **No `_kiro/system/notify` or `_kiro/safety/*` handling.** Those are sibling
   KAS-8 gaps — cyril-08eh and cyril-3ald respectively (verified open).
4. **No empty-message filter.** Matches the v2 arm's `unwrap_or` semantics
   exactly; tightening the empty-string case would be a v2 behavior change, out
   of scope.
5. **No rate-limit UI affordance** (banner, countdown, auto-retry button). It is
   a plain system message, same as the v2 path.

## Tracker references

- cyril-08eh (`_kiro/system/notify`) — verified open.
- cyril-3ald (`_kiro/safety/*`) — verified open.
- cyril-j16p (KAS-2a busy-clear mechanism) — verified closed; this design relies on it.
- No new deferrals introduced.

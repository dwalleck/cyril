# Raw request (verbatim, from rivets cyril-f2g8)

> **K1a — Queue-steering wire + state plumbing**
>
> Wire + state plumbing for Kiro 2.7.0 queue steering, no UX change (ROADMAP
> K1a). Bridge sends _session/steer and _session/steer/clear as awaited ext-
> requests (ids, not notifications), each emitting a notification on success
> AND error. Add steering queued/consumed/cleared notification variants; handle
> the three sessionUpdate variants in the Kiro converter (today they hit the
> unknown-variant error arm). Gate by optimistic send with a clean -32601
> fallback ('steering requires kiro-cli 2.7.0+'), remembered per session.
> Defensive unknown-field tolerance.
>
> Acceptance Criteria:
> - Bridge sends steer/clear as awaited ext-requests; notifies on both success and error paths
> - Converter handles steering_queued/consumed/cleared incl. unknown-field tolerance
> - -32601 surfaces one system message and marks steering unsupported for the session
> - convert-layer tests for all three variants + SessionController/UiState state tests + bridge error-path test

## Drift captured during interrogation

The phrase "today they hit the unknown-variant error arm" is **factually wrong**, now confirmed against captured wire (`experiments/conductor-spike/logs/probe-steer-goal-2.7.0.log`):

1. The variants arrive on `_kiro.dev/session/update` (underscore-dot prefix), **not** the unprefixed `kiro.dev/session/update` the issue/ROADMAP assume.
2. cyril's converter has no arm for that method, so they hit the **outer `other =>` arm (kiro.rs:674) and are silently dropped (`Ok(None)`)** — they never reach the inner `Some(other) => Err` arm the issue refers to.

The fix is therefore a **new outer match arm**, not new cases under the existing arm. The wire-audit's "returns a Protocol Err" line (29) shares the same error. **Action: correct rivets cyril-f2g8, ROADMAP K1a, and docs/kiro-2.7.0-wire-audit.md.** (Decision 1.)

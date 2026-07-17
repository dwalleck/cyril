# Budgeted plan — cyril-3zy4: Surface `_kiro/error/rate_limit`

Source design: `.cyril-3zy4/design.md` (6 claims, cheapest falsifiers already green).
The core change is a single match arm. Slices below are deliberately small —
the design's claims 1-4 & 6 are exercised by converter/engine unit tests, claim
5 by a UI state test. No loops are introduced anywhere (a `match` arm and one
`add_system_message`), so the loop budget is trivially satisfied; stated per
slice for completeness.

## Slice 1: KAS-dialect converter arm

**Claim:** Design claims 1, 2, 3, 6 — `kiro/error/rate_limit` converts to
`RateLimited` (with default on absent message), the legacy `kiro.dev/...` arm is
unaffected, and unknown `_kiro/*` methods still drop to `Ok(None)`.
**Oracle:** the prove-it-prototype oracle — grep for exactly one arm per dialect
plus runtime variant assertions.
**Stress fixture:** absent-message payload `{}` (forces the `unwrap_or` default
branch — the bug class "arm matches but drops/empty-string the message"), AND an
unknown sibling `kiro/does/not/exist` (forces "did the new arm over-match into a
catch-all?"), AND the legacy `kiro.dev/error/rate_limit` (forces "did the edit
replace instead of add?").
**Loop budget:** none (a `match` on `&str`, O(1) arms, no iteration).
**Files:** `crates/cyril-core/src/protocol/convert/kiro.rs`

**Code (advisory):** merge the two dialect strings into one arm:
```rust
"kiro.dev/error/rate_limit" | "kiro/error/rate_limit" => { …RateLimited… }
```

**Verification:**
- [x] `probe_kas_dialect_rate_limit_converts` passes (claim 1)
- [x] `probe_kas_dialect_rate_limit_missing_message_defaults` passes (claim 2)
- [x] `probe_v2_dialect_rate_limit_still_converts` + `parse_rate_limit_error` pass (claim 3)
- [x] `to_ext_notification_unknown_method_returns_none` passes (claim 6)

## Slice 2: KAS engine routing fence

**Claim:** Design claim 4 — `KasEngine.convert_ext_notification` routes the KAS
method to `RateLimited` (the delegation at engine.rs:143 picks up the arm).
**Oracle:** runtime variant assertion on the engine path (distinct from the
direct-converter oracle — catches an arm added only to a v2-only branch).
**Stress fixture:** run the KAS method through `KasEngine` (not the bare
`to_ext_notification`) — the bug class is "arm reachable via the free function
but not via the engine delegation."
**Loop budget:** none.
**Files:** `crates/cyril-core/src/protocol/engine.rs` (test only)

**Verification:**
- [x] `probe_kas_engine_routes_rate_limit` passes (cfg kas) (claim 4)

## Slice 3: busy-guard preservation fence

**Claim:** Design claim 5 — `RateLimited` does not clear UI `Activity` (busy
guard) and emits no `TurnCompleted`.
**Oracle:** runtime assertion that `Activity::Streaming` is unchanged after
applying the notification (distinct from claim 1's converter oracle).
**Stress fixture:** apply `RateLimited` while in a **busy** state
(`Activity::Streaming`) — the bug class is "implementation helpfully clears busy
on rate_limit, prematurely freeing input mid-retry." A happy-path fixture
(apply in `Ready`) would be vacuous.
**Loop budget:** none.
**Files:** `crates/cyril-ui/src/state.rs` (test only)

**Verification:**
- [x] `rate_limited_preserves_busy_activity` passes (claim 5)
- [x] `rate_limited_adds_system_message` passes (system message still added)

## Plan Self-Review

1. **Loops:** none introduced in any slice (match arms + one system-message
   push). No complexity budget exceeded.
2. **Fixtures:** each names a bug class — Slice 1: message-drop / over-match /
   replace-instead-of-add; Slice 2: engine-path-not-wired; Slice 3:
   busy-cleared-on-nonterminal. All more than happy-path.
3. **Doc-comment preconditions:** none new (no new public API; the arm is
   internal to `to_ext_notification`).
4. **Write targets:** no new stdout/stderr writes (converter returns a typed
   `Notification`; the UI renders via existing system-message path).
5. **Tracker references:** cyril-08eh, cyril-3ald (verified open), cyril-j16p
   (verified closed). No new deferrals.

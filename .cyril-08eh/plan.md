# Budgeted plan — cyril-08eh: Render `_kiro/system/notify`

Source design: `.cyril-08eh/design.md` (8 claims, all cheapest falsifiers green).
Implementation already complete (design phase required the fix to run falsifiers).
No loops introduced.

## Slice 1: SystemNotify variant + converter arm

**Claim:** Design claims 1-7 — converter routes `kiro/system/notify` to
`SystemNotify` with correct level parsing, defaults, and unknown-namespace drop.
**Oracle:** runtime assert on variant, level, message equality.
**Stress fixture:** all 5 converter probe tests covering info/warning/unknown
level, missing level, missing message, empty payload, and unknown namespace.
**Loop budget:** none (match arm, O(1)).
**Files:** `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/protocol/convert/kiro.rs`

**Verification:**
- [x] 7 converter probe tests pass

## Slice 2: UI handler + test_bridge arm

**Claim:** Design claim 8 — `apply_notification` adds level-prefixed system
message, preserves busy guard. `test_bridge.rs` print_notification exhaustive.
**Oracle:** runtime assert message contains `[level]` prefix, activity unchanged.
**Stress fixture:** info + warning messages, busy-state preservation.
**Loop budget:** none.
**Files:** `crates/cyril-ui/src/state.rs`, `crates/cyril/examples/test_bridge.rs`

**Verification:**
- [x] `system_notify_adds_level_prefixed_message` passes
- [x] `system_notify_preserves_busy_activity` passes
- [x] test_bridge compiles (exhaustive match)

## Slice 3: Full gate

**Claim:** No regressions — all existing tests, clippy, fmt pass.
**Oracle:** `cargo test --workspace --lib` 0 failures, `cargo clippy -D warnings` clean.
**Stress fixture:** full workspace test suite (915 tests).
**Files:** none (gate only).

**Verification:**
- [x] 915 tests pass
- [x] clippy `-D warnings` clean
- [x] `cargo fmt --check` clean

## Plan Self-Review

1. **Loops:** none introduced. Budget satisfied.
2. **Fixtures:** 7 converter probes + 2 UI probes + full workspace suite — all adversarial.
3. **Doc-comment preconditions:** none new (no new public API).
4. **Write targets:** no new stdout/stderr.
5. **Tracker references:** cyril-3ald, cyril-nk4o (verified open); level-styling follow-up to be filed.

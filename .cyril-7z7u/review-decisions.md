# cyril-7z7u — Review-feedback decisions

Source: `/code-review xhigh --fix PR 28` (Claude code-reviewer bot output, 9 findings).
Assessed per `gilfoyle:assessing-review-feedback` — each finding treated as a
hypothesis (bug claim + fix claim), verified before applying. The review ran in
recall mode (over-surfaces), so a reject-heavy outcome is expected and healthy.

## Decisions

| # | Finding (one line) | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| 1 | Chip sticks >0 when a turn-tail steer never drains (no next turn / non-EndTurn / backend drop) | Bug | Yes — TurnCompleted arm drains only via consume/clear/session; no backstop after the removed reset | **Reject (defer)** | Real; a fix risks regressing the deferred-steer feature and needs the probe to confirm consume-is-eventual → filed **cyril-nvmh** (P2) |
| 2 | `dispatch_steer` leaks chip+echo if `bridge.send` fails after `add_steer_echo` | Bug | Yes — echo committed before the fallible `.await?` (app.rs:966) | **Reject (defer)** | Real but trigger is a dead bridge; clean fix needs a rollback method outside the reviewed file → filed **cyril-7n1l** (P3) |
| 3 | Eviction "desync" breaks the `chip == #Queued echoes` invariant permanently | Bug | **REFUTED as a bug** — chip tracks *pending* count, not visible echoes; `SteeringConsumed` decrements unconditionally so it drains correctly after an echo is evicted | **Reject** | Behaviorally benign; plan doc already notes "modulo message-cap eviction". Decrement-on-eviction would *introduce* an under-count of genuinely-pending steers. No code change. Comment slightly overclaims but is acceptable. |
| 4 | `SteeringUnsupported` discards `flip_queued_steer_echoes` return → fragile/stale redraw | Bug | **REFUTED** — `messages_version` has **zero** production consumers (redraw is driven by the `apply_notification` bool, always `true` here). The stale render cannot occur. | **Reject (reverted)** | Applied in the first pass, then reverted under gilfoyle re-verification: shipped on a false premise and replaced fine, documented code with an inert redundant write. |
| 5 | Stale test header "TurnCompleted resets the steer chip counter" (state.rs:1863) | Comment-rot | Yes — header asserted the opposite of the test body | **Accept** | Corrected — the wrong header invited reintroducing the removed reset |
| 6 | Stale test header "queue mirror queued->1, consumed->0" (state.rs:1972) | Comment-rot | Yes — header contradicts the body's "wire is a no-op" assertion | **Accept** | Corrected |
| 7 | No test pins bare wire `SteeringQueued` from zero (chip stays 0, no echo) | Coverage | Yes — grep confirmed only "no re-count from N" was tested | **Accept** | Added `bare_wire_steering_queued_is_a_noop` |
| 8 | `SessionController.steering_depth` is a dead, divergent parallel counter | Design | Yes — zero non-test readers | **Reject (defer)** | Duplicate of **cyril-85py**; PR deliberately defers removal |
| 9 | Multi-client: unconditional `SteeringConsumed` decrement under-counts cyril's own steers | Bug | Plausible (single-client model today) | **Reject (defer)** | Duplicate of **cyril-8lfs** |

Tally: **3 Accept, 6 Reject** (2 defer→filed, 2 defer→duplicate, 2 outright — one of which was reverted after being initially applied).

## Applied changes (Accept — findings 5, 6, 7)

All in `crates/cyril-ui/src/state.rs`, test/comment scope only:
- Rewrote two stale test header comments (1863, 1972) to describe the new optimistic-chip behavior.
- Added `bare_wire_steering_queued_is_a_noop` pinning the single-client invariant from zero.

Verification: `cargo fmt -p cyril-ui --check` clean · `cargo clippy -p cyril-ui --all-targets -- -D warnings` clean · `cargo test -p cyril-ui` → 315 passed.

## Key verification note

The reviewer's headline finding (eviction creates a permanent phantom chip, #3)
and the one behavioral fix initially applied (#4) were **both refuted on
verification** — exactly the failure mode this discipline guards against. The
chip's contract is "# pending steers," and `SteeringConsumed` decrements it
regardless of whether the echo still exists, so it drains correctly even after
eviction; and `messages_version` drives no render, so the "fragile redraw" could
not occur. Auto-applying the reviewer's proposed eviction fix would have shipped
a real under-count bug.

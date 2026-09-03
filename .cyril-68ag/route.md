# Route: cyril-68ag

Change: Preserve KAS startup notifications that race `SessionCreated`.
Date: 2026-09-02

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The current repository contains the observed 2.21.0 launcher capture at `experiments/conductor-spike/kas-baseline-live-2.21.0.jsonl`: its lines 8, 17, and 20 show the scoped `fetch_cloud_config` tool start, `session/new` response, and matching tool update in that order. `crates/cyril/src/app.rs` currently maps an unidentified pre-main scope to `NotificationRoute::Drop`. No external premise remains unverified. | no |
| 2 | Structural module shape | No public interface, schema, dependency direction, module ownership, or responsibility owner changes. `App::handle_notification` already owns notification routing and session-start projection; a bounded pending-route queue stays inside that existing responsibility. `SessionController`, `UiState`, protocol conversion, and the protected bridge/domain mediator interfaces remain unchanged. | no |
| 3 | Production-scale risk | Startup buffering is bounded to the bridge notification-channel capacity and exists only until the main session is identified. It adds no unbounded memory, throughput, concurrency, or persistent-data path. | no |
| 4 | Explicit behavior | Given a session-scoped notification arrives before Cyril knows the main session, when `SessionCreated` establishes the main id, then Cyril buffers the frame without warning and replays it exactly once through normal routing in arrival order. Given the buffered scope equals the created main id, replay reaches the main `SessionController` and `UiState`; given it differs, replay follows the existing foreign-session route and never mutates main state. Given pending startup frames exceed the fixed bound, the oldest frame is dropped with a warning rather than allowing unbounded growth. The 2.21.0 `fetch_cloud_config` start → `SessionCreated` → update fixture completes without the stale Drop-arm warning. | yes |

Unknown tests: none

## Selected route

Local — this is a bounded correction inside App's existing notification-routing responsibility, with current repository wire evidence and no interface or ownership change.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict) |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD

## Terminal criterion

Local — the focused behavioral verification named here records PASS: `cargo test -p cyril --features kas startup_cloud_config_frame_is_buffered_and_replayed_without_warning`; after the hand-off append `Result: <YYYY-MM-DD> | <command> | <PASS or FAIL>`

Result: 2026-09-02 | `cargo test -p cyril --features kas startup_cloud_config_frame_is_buffered_and_replayed_without_warning` | PASS

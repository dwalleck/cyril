# Related issues

- **cyril-6beh** — closed. Introduced the typed nine-event workflow protocol, `WorkflowTracker`, and the committed 2.16.0/2.16.2 capture replay fences this issue must extend.
- **cyril-0qe6** — closed and merged in PR #95. Its `/workflow` controls consume command outcomes and snapshots and have no event-driven pause affordance; there is no pause consumer to migrate in this compatibility fix.
- **cyril-zd8u** — open. Future workflow renderer; pause chips must derive immediacy from `node_paused`.
- **cyril-2ibk** — open. Deferred mutating workflow verbs, including explicit pause; adjacent control-plane scope, not part of this compatibility fix.
- **cyril-sinu** — open. Improves converter error paths for invalid node identifiers; unrelated to valid pause-frame ordering.
- **cyril-2rgp** — open. Pre-existing intermittent tracing-capture test failure in workflow warning assertions; do not conflate it with deterministic pause state behavior.
- **cyril-7sjs** — open. Strengthens workflow fixture/manifest tripwires; related test infrastructure but separate scope.

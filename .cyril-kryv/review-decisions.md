# Review decisions: cyril-kryv

Source: merge-readiness review of stacked draft PRs #97–#99. Findings continue with stable IDs when later increments add rows.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F19 | The v1→v2 migration classifies every legacy row without an error string as successful, including cancelled and other non-success stop reasons. | merge-readiness review | Verified | Direct SQL/code comparison plus the red/green legacy outcome matrix: the error-only mutation produced five successes instead of one. | Modify | Derive migrated outcome with error precedence, then cancelled, then clean end-turn success, with every other terminal reason classified as error. | Gate passed: `v1_migration_is_lossless_idempotent_and_enrichment_atomic`; default/KAS workspace tests, strict Clippy, and rustfmt passed. |
| F20 | `TurnMeteringUpdated.used_tools` must be copied into portable durable tool rows. | merge-readiness review | Refuted | Direct domain/routing check: the summary contains vendor-specific names but no call IDs or portable kinds; durable portable calls come from typed ACP `ToolCallStarted`/`ToolCallUpdated`, while C1 already fences exact preservation in the metering notification. | Reject | N/A — no sound name-to-kind or call-ID mapping exists at this layer. | Permanent no-op: inventing IDs/kinds would fabricate per-call attribution and can double-count the authoritative ACP lifecycle. |

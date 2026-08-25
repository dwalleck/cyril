# Checkpoints: cyril-n3j7

Date: 2026-08-25
Route: Empirical

## Slice 1 — durable source-turn storage and query-aware prompt context

Implementation: `ddc7343a` plus terminal budget hardening in `babd588`.

1. **Affected unit tests — PASS.** `cargo test -p cyril-memory` and the affected Cyril runtime/architecture tests pass.
2. **PENDING falsifiers — PASS.** C3, C4, C8, C11, and C13 deterministic falsifiers agree with their independent SQL/hash, Python FTS, raw-wire, migration-manifest, and dependency-boundary oracles.
3. **Stress fixture — PASS.** Partial/restart/identical/conflicting replay, two-project eligibility, bounded literal queries, protocol limits, v1/v2/v3 migration, 16-event/256-KiB capture, and 100,001-row FTS/list fixtures produced the planned outcomes.
4. **Implementation vs independent oracle — PASS.** The retained bundled-SQLite probe and `oracle.py` agree with production FTS selection (`rows=turn-new,turn-old`); read-only SQL counts/manifests and independently recomputed hashes agree with stored rows.
5. **Production-scale budget — PASS.** Ignored benchmark fence `source_turn_operations_meet_production_scale_budgets` on 100,001 rows measured capture `4.187 ms` (≤100 ms), FTS recall `9.645 ms` (≤50 ms), list `4.580 ms` (≤50 ms), and inspect `0.097 ms` (≤50 ms). Migration wall budget: **N/A — one-off startup phase**, as recorded in `plan.md`.
6. **Regression fences — PASS.** C3/C4/C8/C11/C13 fences are green.
7. **Named mutations — PASS (red).** Immutable replacement, scope/status removal, skipped sequential migration, removed query bound, and leaked App policy each made its named fence red.
8. **Fence restoration — PASS (green).** All five mutations were removed and their fences returned green.

## Slice 2 — authoritative bridge capture and bounded forwarder

Implementation: `db64d3cd` plus terminal budget hardening in `babd588`.

1. **Affected unit tests — PASS.** Core bridge/source-observer tests, Cyril App/runtime tests, and capture-forwarder tests pass.
2. **PENDING falsifiers — PASS.** C1, C2, C5, C6, C9, C10, and C12 match the hand source/wire vectors, terminal truth table, reducer/secret scanner, queue/shutdown model, dependency manifest, and durable-identity oracle.
3. **Stress fixture — PASS.** Context-enriched Unicode source, streaming/tool tails, thoughts, every terminal disposition, replay history, numeric turn-ID reuse, 33 events against a one-slot test receiver, 32 simultaneous ingress guards, bridge completion, and runtime drain produced the planned outcomes.
4. **Implementation vs independent oracle — PASS.** Captured source equals original blocks rather than enriched wire blocks; terminal states match the fixed truth table; durable random IDs remain distinct across numeric reuse; thoughts and injected context are absent.
5. **Production-scale budget — PASS.** The 64-KiB UTF-8 observer fence asserts ≤10 ms; the 32-guard ingress fence asserts ≤50 ms; the in-process forwarder fence asserts ≤100 ms and completed in `0.02 s` including IPC. Queue capacity is 32 and the one-off shutdown drain is hard-capped at two seconds (**N/A — one-off process phase** for an always-on wall budget).
6. **Regression fences — PASS.** C1/C2/C5/C6/C9/C10/C12 fences are green.
7. **Named mutations — PASS (red).** Context-prefixed source, false completion, context folded into originals, thought persistence, overflow completion, forbidden dependency classification, and zero durable IDs each made the named fence red.
8. **Fence restoration — PASS (green).** All seven mutations were removed and their fences returned green.

## Slice 3 — storage-backed typed `/memory` inspection

Implementation: inspection landed with the concrete adapter boundary in `db64d3cd`; bounds and scale fences landed in `babd588`.

1. **Affected unit tests — PASS.** Core memory command parsing, UI turn formatting, StoreSet list/detail, and App storage-backed action tests pass.
2. **PENDING falsifier — PASS.** C7 list/inspect remains correct after UiState retention is cleared and rejects foreign/missing identities through the safe not-found path.
3. **Stress fixture — PASS.** Empty, one, 100, and >100 row cases; bounded Unicode-safe 16-KiB detail; 100-row render; omitted counts; malformed/foreign IDs; and exhaustive typed state mapping produce the planned outcomes.
4. **Implementation vs independent oracle — PASS.** Direct runtime records and hand-authored list/detail strings agree before and after UI eviction; storage is the only source of inspection data.
5. **Production-scale budget — PASS.** On the 100,001-row fixture, list measured `4.580 ms` and 16-KiB inspect `0.097 ms`; the 100-row list plus 16-KiB detail formatter asserts ≤50 ms and completed in `0.00 s`.
6. **Regression fence — PASS.** Core parser, UI formatter, StoreSet bounds, and App C7 fences are green.
7. **Named mutation — PASS (red).** Removing durable storage rows from the App-backed result made C7 red.
8. **Fence restoration — PASS (green).** Durable storage mapping was restored and C7 returned green.

## Final integration

- Evidence premises P1/P2 — **PASS**.
- Claims C1–C13 implementation/oracle comparisons — **PASS**.
- Claims C1–C13 falsifiers — **PASS**.
- Claims C1–C13 permanent fences — **PASS**.
- `cargo test` — **PASS**, 1,556 tests across 24 suites; 6 ignored production/manual gates.
- `cargo fmt --check` — **PASS**.
- `cargo clippy --all-targets -- -D warnings` — **PASS**.
- Workspace all-feature warning-denied all-target Clippy, including the KAS feature — **PASS**.
- `cargo nextest run -p cyril -p cyril-core --features kas` — **PASS**, 1,185 tests; 8 skipped.
- Production-scale ignored gate — **PASS** with measurements recorded above.
- Failures — none.

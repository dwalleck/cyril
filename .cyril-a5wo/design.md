# Design: cyril-a5wo

## Route and inputs

- Route: **Empirical**, from `.cyril-a5wo/route.md`. The unverified premise was current KAS wire representability of absent or partial `rawInput`.
- Behavior source: `route.md` T4; `spec.md` is `N/A —` it records the superseded live-partial-capture request. Complete behavior:
  1. Given current extracted `@kiro/agent`, audit every production `tool_call` / `tool_call_update` emission path and cite whether absent or partial `rawInput` is representable.
  2. Given each of six committed cancel captures, replay its one-identifier `pending` → `in_progress` → `failed` sequence through Cyril conversion and UI merge/commit, then require one committed failed call, preserved fields, and no active call after cancelled turn end.
  3. Because absent and partial input are source-proven, derive a pinned fixture and require conversion plus `TrackedToolCall::primary_path` / `command_text` to return source-correct values without panic.
- Empirical input: `.cyril-a5wo/evidence.md` P1 is `PASS`. The Python lexical probe and independent JavaScript AST oracle agree on four initial and fifteen update emission sites. Current source emits an absent initial input and status-only updates, and explicitly emits partial path-only append/write/replace actions.
- Independent evidence oracle: parsed-AST site enumeration plus hand-counted pinned source lines, as recorded in `evidence.md`.
- Existing captures: `.cyril-a5wo/captures/attempt-{1,2,3}.jsonl` (2.16.2) and `.cyril-a5wo/captures-2.18.1/attempt-{1,2,3}.jsonl` (2.18.1). Each has one tool-call start, two same-ID updates, and a cancelled `turn_end`.

## Input shapes

| ID | Production-reachable shape | Status |
|---|---|---|
| S1 | Pinned `@kiro/agent` 0.38.7 source with SHA-256 `965ae084…70a`. | Covered by C1. |
| S2 | Source identity or emission-site lines drift before fixture regeneration. | Covered by C1 — the probe fails closed instead of silently carrying stale provenance. |
| S3 | Initial `user_input` `tool_call` with absent `rawInput`, followed by a status-only update. | Covered by C2 and C3. |
| S4 | Streaming replace `tool_call` with path-only partial `rawInput`, followed by an update with path plus partial `newStr`. | Covered by C2 and C3. |
| S5 | Display path absent because both locations and `rawInput` path keys are absent. | Covered by C3. |
| S6 | Display path present as one absolute Unicode-safe string; `command` absent. | Covered by C3. Embedded spaces and Unicode are represented in the fixture path. |
| S7 | Six complete subagent input objects (`name`, `prompt`, `explanation`, `contextFiles`) across two engine versions and three attempts each. | Covered by C4 and C5. |
| S8 | Initial `pending` call with title/kind/status, middle `in_progress` update without title/kind, terminal `failed` update with title and empty-string `rawOutput`. | Covered by C4 and C5. |
| S9 | `turn_completion` metering frame before cancelled `turn_end`; only `turn_end` is terminal. | Covered by C4. |
| S10 | Distinct session and tool-call IDs in each capture; exactly one start and two updates per tested identifier. | Covered by C4; the fence asserts six named inputs and one committed entry per input. |
| S11 | Empty content/location collections in the six captures and one location in the source-derived partial fixture. | Covered by C3 and C5; existing non-empty merge-guard tests remain part of C5's fence. |
| S12 | Multiple locations, diff-content path precedence, or a present shell `command`. | N/A — existing helper precedence is unchanged; the revised request asks only for source-derived absent/partial shapes. Permanent non-goal for this change. |
| S13 | Malformed or non-object `rawInput`. | N/A — not produced by the cited source paths and no longer part of the revised acceptance. Permanent non-goal for this change. |
| S14 | A duplicate second `tool_call` start for the same identifier. | N/A — none of the six accepted lifecycle captures contains this shape; changing duplicate-start semantics would exceed evidence. Permanent non-goal for this change. |
| S15 | A complete input followed by a semantically partial object whose missing keys should be recursively retained. | N/A — KAS exposes no completeness marker and the source-proven streaming order expands partial input rather than shrinking a complete object. Recursive JSON merge is a permanent non-goal. |
| S16 | Terminal update arriving after `turn_end`. | N/A — all six accepted captures place `failed` before `turn_end`; reordered recovery is not evidenced by the revised request. Permanent non-goal. |

## Removed-invariant sweep

Purely additive verification change. It adds evidence, fixtures, dev-only feature wiring, and tests; it removes no production constraint, guard, ordering guarantee, uniqueness rule, or serialization point.

## Placement

### Source-derived wire fixture

- **Owner:** `cyril-core`'s existing `tests/fixtures/kas/` module, because the bytes are ACP/KAS protocol inputs shared through the core conversion adapter. Provenance is appended to its existing README; `.cyril-a5wo/derive-raw-input-fixture.py` owns deterministic derivation from the pinned source.
- **New seam:** none. The fixture is raw JSON-RPC; tests use the existing `cyril_core::test_support::kas_capture_to_routed` interface.
- **Forbidden:** no fixture parser or KAS wire type enters production UI code; no second fixture convention or duplicate copy of the six live captures is created.

### Capture conversion

- **Owner:** `cyril-core::test_support::kas_capture_to_routed`, the existing deep test adapter that hides ACP deserialization and KAS conversion behind domain `Notification` values. A private UI-test normalizer unwraps each recorder row's `parsed` frame before crossing that seam.
- **New seam:** none. `cyril-ui` enables core's existing `kas` + `test-support` dev features only.
- **Forbidden:** `cyril-ui` must not add `agent-client-protocol`, import `acp::` types, or reimplement KAS conversion.

### Lifecycle and display assertions

- **Owner:** colocated `UiState` tests in `crates/cyril-ui/src/state.rs`, because chronological commit, in-place `TrackedToolCall` update, active-call cleanup, activity, and display helpers are UI state/presentation responsibilities.
- **New seam:** none. Tests exercise the same public `UiState::apply_notification` and `TuiState` read-only interface used by the application and renderer.
- **Forbidden:** no production branch special-cases these captures, tool names, versions, or IDs; no renderer-only suppression substitutes for clearing state.

## Claims

- **C1.** The pinned current KAS source proves both absent and partial `rawInput` are production-reachable, and the audit fails closed on source drift.
- **C2.** One deterministic source-derived JSONL fixture represents the cited absent user-input lifecycle and partial streaming-replace lifecycle byte-for-byte, with version, source SHA, fixture SHA, and derivation command recorded.
- **C3.** Replaying that fixture through the production KAS conversion adapter commits two calls whose display helpers return `None`/`None` for absent input and the exact path/`None` for partial path-only input, without panic.
- **C4.** Replaying every one of the six named live captures through production KAS conversion and `UiState` leaves exactly one committed call for its captured identifier, status `Failed`, zero active calls, and `Activity::Ready` after cancelled `turn_end`.
- **C5.** At each captured update checkpoint, the committed call preserves its initial title, raw input, content, and locations when the update omits those fields; the existing non-empty merge-guard fence remains green.
- **C6.** ACP/KAS parsing stays inside `cyril-core`; UI tests consume only domain notifications through the existing test-support seam and add no production interface.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Pinned source proves absent and partial input and fails closed on drift. | S1–S2 | Run the pinned source probe; any site-count/hash mismatch or `absent=false` / `partial=false` falsifies C1. | Independent AST queries and hand-counted source trace in `evidence.md`. | Change `EXPECTED_SHA256` in `.cyril-a5wo/probe-wire-raw-input.py` to one zero digit; the probe exits nonzero with `unexpected acp-server.js sha256`. | `.cyril-a5wo/probe-wire-raw-input.py` retained as the deterministic source gate. | <1 minute, local read-only | PASS |
| C2 | Fixture exactly represents the two cited source lifecycles with pinned provenance. | S3–S4 | Regenerate to a temporary file and compare bytes; any diff, missing frame, or provenance hash mismatch falsifies C2. | The pinned source lines cited in `evidence.md`, read independently of the generator. | Delete `rawInput` from the partial initial frame in the generated fixture; byte comparison and the fixture-shape assertions fail. | `source_derived_raw_input_fixture_is_exact` plus regeneration byte comparison in the checkpoint gate. | <1 minute | PASS |
| C3 | Conversion and display helpers are total and source-correct for absent/partial input. | S3–S6, S11 | Replay the source-derived fixture; panic, non-`None` fallback for absent input, wrong path, or non-`None` command falsifies C3. | Direct expected-value table derived from source: absent user-input has no path/command; streaming replace has one location/path and no command. | Replace `raw_input().and_then(...)` in `TrackedToolCall::primary_path` with `raw_input().expect("input")`; the absent case panics. | `source_derived_absent_and_partial_raw_input_display_safely` | <1 minute | PASS |
| C4 | All six cancel captures converge to one failed committed call with no active state. | S7–S10 | Replay each named capture into a fresh `UiState`; any count other than one, non-Failed final status, nonempty active list, or non-Ready activity falsifies C4. | Independent Python capture oracle's per-ID ordered `recovery_frames` plus raw `turn_end` fields recorded in `findings.md`; it does not call Cyril. | Change `UiState::ToolCallUpdated` to append a new message instead of updating `tool_call_index`; committed-call count becomes three. | `cancellation_captures_converge_to_one_failed_committed_call`, covering all six captures | <1 minute | PASS |
| C5 | Captured partial updates do not clobber guarded fields. | S7–S8, S11 | Compare the committed call against its initial converted snapshot after every update; any title/raw-input/content/location drift falsifies C5. Run the existing non-empty guard tests for collection coverage absent from captures. | Direct JSON frame comparison: the middle update omits title/kind while carrying the same complete input; initial capture frame is the independent expected snapshot. | Replace the guarded assignments in `ToolCall::merge_update` with unconditional title/raw-input/content/location assignments; the checkpoint assertion and existing guard fences fail. | Lifecycle assertions plus `merge_update_preserves_content_when_update_has_none`, `merge_update_preserves_title_when_update_is_empty`, `merge_update_preserves_raw_input_when_update_has_none`, and `ledger_partial_update_preserves_non_empty_fields` | <1 minute | PASS |
| C6 | KAS parsing remains behind core's existing test seam. | Placement | Compile UI tests after enabling core's dev-only `kas` + `test-support` features; any direct ACP import or production feature change falsifies C6. | `cyril-ui/Cargo.toml` dependency direction and `test_support`'s domain-only return type. | Add `use agent_client_protocol as acp;` to the UI test without adding a forbidden direct dependency; `cargo test -p cyril-ui --no-run` fails unresolved import. | Rust compiler on `cargo test -p cyril-ui --no-run`; review gate confirms no normal dependency was added. | <2 minutes | PASS |

## Non-goals and future work

- Permanent non-goal: infer or recursively merge semantic completeness inside arbitrary JSON `rawInput`; KAS supplies no completeness bit, and current partial producers expand in arrival order.
- Permanent non-goal: change duplicate-start handling or late-after-turn ordering without a production capture carrying that shape.
- Permanent non-goal: redesign display precedence for multiple locations, diff content, or command-bearing calls; existing behavior remains untouched.
- Permanent non-goal: add malformed/non-object cases from the superseded spec when the revised source-derived criterion does not produce them.
- Intended future work: KAS agent-subtask rendering/grouping remains `cyril-ebqu`; this change verifies state convergence only and does not render crews.
- Intended future work: trusted tool identity and raw-input camelCase documentation remain `cyril-2yn8`; this change adds no identity fields.

## Falsifier run log

- 2026-08-18 — C1 cheapest falsifier:

  ```sh
  ./.cyril-a5wo/probe-wire-raw-input.py \
    ~/.local/share/kiro-cli/kas/2.18.1-b23bc10526e6c0197a381b39bc956c5edf18c5a041a765d7597a68c97e191d90/node_modules/@kiro/agent/dist/server/acp-server.js \
    ~/.local/share/kiro-cli/kas/2.18.1-b23bc10526e6c0197a381b39bc956c5edf18c5a041a765d7597a68c97e191d90/node_modules/@kiro/agent/package.json
  ```

  Result: **PASS** — package 0.38.7, pinned SHA matched, 4 initial / 15 update sites, `REPRESENTABLE absent=true partial=true`.

- 2026-08-18 — C2: deleting `rawInput` from the partial initial fixture made both the generator byte check and `source_derived_raw_input_fixture_is_exact` fail; regeneration restored both to green. **PASS**.
- 2026-08-18 — C3: replacing the optional `primary_path` lookup with `expect("input")` made the absent-input fixture panic; restoration returned the focused display test to green. **PASS**.
- 2026-08-18 — C4: changing `ToolCallUpdated` to append committed messages made the six-capture fence report three calls instead of one; restoration returned all six to green. **PASS**.
- 2026-08-18 — C5: making title/raw-input/content/location assignments unconditional made three `merge_update_preserves_` tests and `ledger_partial_update_preserves_non_empty_fields` fail; restoration returned all five guards to green. **PASS**.
- 2026-08-18 — C6: importing `agent_client_protocol` directly in the UI test failed with unresolved import; restoration returned `cargo test -p cyril-ui --no-run` to green. **PASS**.

## Approval

- Requester words: “We're picking up where a previous agent left off. I approve to move forward”
- Date: 2026-08-18
- Approved risk acceptances: None.

# Budgeted plan: KAS workflow lifecycle state

Approved design: `.cyril-6beh/design.md` at `e4b93ab`.
Cheapest falsifier: `.cyril-6beh/falsify-node-paths.py` passed with 3 wire paths, 2 preserved wrapper records, and 8 discriminator controls.

## Materialized independent oracles

These artifacts exist and were validated before production implementation:

- `.cyril-6beh/oracle-manifest.json` — JSON contract for the nine methods, required/optional fields, descriptors, enum/scalar/opaque boundaries, C6 merge/index rules, C9 progress permutations, warning schemas, transition/ownership/reset rules, replay checkpoints, and 64×256×10 scale expectations.
- `.cyril-6beh/oracle.sh` — jq projection of committed 2.16.2 captures; exact JSON output is `{"failed":2,"aborted":2}` modulo jq whitespace.
- `.cyril-6beh/oracle-snapshot.py CAPTURE...` — independent final-snapshot canonicalizer. Output is one compact, key-sorted JSON array; each item is `{run, nodes}`, and `nodes` is path-sorted with each entry `{path,data}`.
- `.cyril-6beh/oracle-replay.py JSONL...` plus `oracle-replay-events.jsonl` — standalone nine-event lifecycle folder. It emits one compact sorted array of `{source,expected,oneEqualsTwo}`; `expected` contains named opening/node-opening/event-only/terminal/after-retry checkpoints plus final state. It folds the synthetic all-method stream and three live captures once/twice without Rust.
- `.cyril-6beh/falsify-node-paths.py` — independent repeat-wrapper discriminator and capture/path oracle.
- `.cyril-6beh/compare-oracles.sh manifest|terminal|snapshot|replay` — creates an independent expected JSON file, passes its path in `CYRIL_WORKFLOW_ORACLE_EXPECTED`, and runs the named compiled Rust test that must compare exact parsed JSON.
- `.cyril-6beh/check-structure.py` — persistent C12 privacy/import/ownership fence with a valid fixture and eight forbidden-mutation self-tests; run against the repo after App wiring.

`compare-oracles.sh terminal` becomes runnable in Slice 4, `manifest` in Slice 9, and `snapshot`/`replay` in Slice 20. Before each mode exists, the slice-specific Rust test loads the relevant manifest section directly and the independent scripts are rerun to ensure their outputs remain stable.

## Gate after every slice

1. Run the slice's named unit test and adversarial fixture.
2. Run every currently available mode of `.cyril-6beh/compare-oracles.sh`; exact JSON equality occurs inside the compiled test binary. Run `.cyril-6beh/falsify-node-paths.py` after snapshot-identity changes.
3. Measure each named production loop and always-on wall budget.
4. Run `cargo test`, `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
5. Before changing an existing exported symbol, use LSP references and record callers. Before adding a helper, search source and existing dependencies. New symbols explicitly record “no prior callers.”
6. Run stale-reference and structured-file drift checks; commit only after all gates pass. One commit per slice.

The compiled Rust test binary is the executable observation surface: this issue intentionally adds no user-visible command or renderer.

Complexity notation is parametric because the signed spec accepts large strings/JSON and imposes no transport/domain cap: `J` is input JSON bytes visited, `H` is the total bytes of canonical paths/ids hashed or materialized, `W`/`Q`/`I` are the bytes in one workflow id/node path/node id, `N` is node count, `D` is descriptor depth, `P` is queued descriptors, `B` is one node-id bucket, and `R` is run count. Values such as 1 MiB, 64 KiB ids, N=256, D=10, P=256, B=256, and R=64 are measured adversarial fixtures, never acceptance limits.

## Evidence Slice A: Freeze independent manifest and snapshot oracle

**Claim:** C1–C15 expected inputs, outputs, warning schemas, merge/progress tables, replay checkpoints, and snapshot projection are independent of production Rust.
**Oracle:** Kiro 2.16.2 source/capture audit, signed design tables, and committed failed/aborted/repeat captures.
**Stress fixture:** Validate every manifest family/control/warning/merge/progress/replay set; run the snapshot oracle over both terminal captures. Expected: exact nonempty sets, terminal statuses include failed/aborted, and repeated snapshot output is byte-identical.
**Loop budget:** Python evidence traversal O(F+J), where F is capture frames; no production loop.
**Wall budget:** <=2 s for both captures.
**Files:** `.cyril-6beh/oracle-manifest.json`, `.cyril-6beh/oracle-snapshot.py`.

**Verification:** `jq -e` validates exact family/control/table counts; Python compiles and emits deterministic compact JSON twice; commit these two files before production code.

## Evidence Slice B: Freeze independent nine-event lifecycle folder

**Claim:** C6/C9/C13 event folding, event-only snapshot preservation, and retry clearing are independently executable before Rust.
**Oracle:** Literal all-nine-method `oracle-replay-events.jsonl`, signed transition tables, and three committed live captures.
**Stress fixture:** Fold the synthetic stream and failed/aborted/repeat captures once/twice. Expected: all nine methods occur; opening run and node snapshots omit status; `event_only` contains pending+ack/run pause/node pause/false loop/first watch; `terminal` preserves current event-only state with true loop/second watch; `after_retry` contains only root+new node and none of six forbidden stale fields; one/two complete sorted projections are byte-equal for every source.
**Loop budget:** Offline Python O(F×(J+H)); no production loop.
**Wall budget:** <=2 s for all four sources and both pass counts.
**Files:** `.cyril-6beh/oracle-replay.py`, `.cyril-6beh/oracle-replay-events.jsonl`.

**Verification:** Python compiles; jq asserts nine methods, five checkpoints, absent opening statuses, event-only values, retry absence, and four `oneEqualsTwo=true` results; commit both files before production code.

## Evidence Slice C: Freeze compiled-binary comparison wrapper

**Claim:** Independent JSON reaches the compiled Rust tests through a fixed external path rather than being regenerated by production code.
**Oracle:** Literal mode→test→expected-file table in `.cyril-6beh/compare-oracles.sh`.
**Stress fixture:** `bash -n`; missing/unknown mode; manifest/terminal/snapshot/replay mode inspection. Expected: usage failure for bad mode and one exact cargo test invocation/expected-file form per valid mode.
**Loop budget:** O(1) shell dispatch; snapshot/replay modes invoke their evidence-oracle parametric costs.
**Wall budget:** <=2 s excluding cargo compilation/test execution.
**Files:** `.cyril-6beh/compare-oracles.sh`.

**Verification:** shell syntax and four-mode table pass; commit the wrapper alone before production code. Each mode is rerun as soon as its named binary test exists.

## Evidence Slice D: Freeze structural ownership/privacy fence

**Claim:** C12 structural invariants are executable and fail for private-field, wire-visibility, dependency, or non-App ownership regressions.
**Oracle:** A synthetic valid crate seam plus eight independently injected forbidden mutations.
**Stress fixture:** `python3 .cyril-6beh/check-structure.py --self-test`. Expected: valid fixture passes; ACP alias, protocol import, async runtime, public domain field, public wire type, public App field, Session owner, and UI owner each fail.
**Loop budget:** O(S) source bytes in an offline script; no production loop.
**Wall budget:** <=1 s self-test.
**Files:** `.cyril-6beh/check-structure.py`.

**Verification:** self-test reports all eight mutations caught; normal repo mode is intentionally red until the planned modules/App owner exist; commit the fence alone before production code.

## Slice 1: Workflow identifiers, counters, and closed vocabularies

**Claim:** C3/C12 — exact scalar/status domains with no sentinels in `cyril-core`.
**Oracle:** `oracle-manifest.json` sections `enum_domains` and `identifier_cases`; compiled `workflow_identifier_string_matrix` and `workflow_enum_domain_matrix` compare exact JSON projections.
**Stress fixture:** Empty, ASCII, Unicode, spaces, `#`, `/`, backslash, and 64 KiB workflow/node ids; `u32::{MIN,MAX}`; every enum variant plus one unknown. Expected: only empty ids/unknown enums reject; all other ids round-trip byte-for-byte.
**Loop budget:** No production loops; constructors/accessors O(1). Large-string allocation is the caller-owned newtype value, with no second copy.
**Wall budget:** N/A — no always-on phase.
**Files:** `crates/cyril-core/src/types/workflow.rs` (new), `crates/cyril-core/src/types/mod.rs`.

**Verification:** unit fences pass; manifest projection matches; `oracle.sh` remains `{failed:2,aborted:2}`; no loop budget applies.

## Slice 2: Node descriptors and full snapshot shapes

**Claim:** C3/C12 — all five recursive descriptors and complete wire-neutral snapshot shapes.
**Oracle:** `oracle-manifest.json` `descriptor_fields`, enum/counter domains, snapshot-owned fields, and literal source shapes from the audit.
**Stress fixture:** A 256-node/depth-10 forest with every descriptor, empty/one/many children, duplicate paths, shared ids, `u32::MAX`, every optional mask, and all snapshot run/node fields. Expected projection is the manifest-defined field set with no invented absent value.
**Loop budget:** Data construction/accessors add no implicit traversal; explicit clone is O(N+J) only when called. Production conversion/state will consume owned values.
**Wall budget:** N/A — pure data definitions.
**Files:** `crates/cyril-core/src/types/workflow.rs`.

**Verification:** `workflow_node_descriptor_shape_matrix` and snapshot-shape tests match the manifest; 256-node fixture preserves every field; currently available oracle gates stay green.

## Slice 3: Lifecycle events and immutable read models

**Claim:** C3/C12 — all nine lifecycle event variants and immutable accessor-only read models exist in core.
**Oracle:** `oracle-manifest.json` method/field inventory plus exhaustive event-construction projection.
**Stress fixture:** Construct one field-rich event per method and every accessor/optional mask. Expected: exact event/accessor data, private fields, and no protocol/UI dependency.
**Loop budget:** No traversal loops or always-on work.
**Wall budget:** N/A — pure data definitions.
**Files:** `crates/cyril-core/src/types/workflow.rs`.

**Verification:** event/read-model and privacy fences pass; manifest sections match; external oracle outputs remain stable.

## Slice 3A: Box Notification and update exhaustive neutral consumer

**Claim:** C3/C12 — the boxed core boundary compiles atomically while the existing exhaustive `UiState` consumer gains only the required explicit no-op arm and the exhaustive smoke harness labels the workflow method.
**Oracle:** Box-size equality, pre-change unrelated notification outputs, a populated observable UI-state fingerprint, and the source-level no-op arm (which has no mutation expression).
**Stress fixture:** Box the maximal event as `Notification::Workflow`, require `size_of::<Box<WorkflowEvent>>() == size_of::<usize>()`, call `UiState::apply_notification` directly with it, and construct 10,000 unrelated notifications. Expected: workflow returns false with its populated UI-state fingerprint unchanged; unrelated outputs/discriminants stay unchanged. The compile-only arm destructures no workflow payload, imports no workflow type, and stores no workflow state; `SessionController`'s existing wildcard remains unchanged. App is still the only production receiver.
**Loop budget:** One allocation only when constructing a workflow notification; UI no-op O(1).
**Wall budget:** <=100 ms for 10,001 direct consumer calls.
**Files:** `crates/cyril-core/src/types/event.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril/examples/test_bridge.rs`.

**Verification:** boxed-shape and `workflow_notification_is_ui_noop` tests pass in default/KAS builds; every exhaustive match compiles; the smoke harness reports the event method; structural fence proves no UI workflow state/type dependency.

## Slice 4: Run opening/completion adapter

**Claim:** Valid-input C3 subset — `run_start` and `run_complete` convert losslessly under `protocol::convert::kas::workflow`.
**Oracle:** Raw committed captures, `oracle-manifest.json` run method/descriptor fields, and `.cyril-6beh/oracle.sh`.
**Stress fixture:** Field-rich opening; failed/aborted captures; 256-node/depth-10 final snapshot; one missing field, wrong type, and outer/final mismatch followed by valid input. Expected valid variants preserve all data; invalid rows warn/drop; successor converts; terminal counts equal 2/2.
**Loop budget:** O(J+H), one pass over the provided `Value` plus owned output construction; no whole-value clone or syscall. The 1 MiB/256-node/depth-10 case is measured, not enforced.
**Wall budget:** <=50 ms for the representative 1 MiB/256-node/depth-10 frame in opt-level-1 tests; accepted larger inputs remain linear in `J+H`.
**Files:** `crates/cyril-core/src/protocol/convert/kas.rs`, `crates/cyril-core/src/protocol/convert/kas/workflow.rs` (new).

**Verification:** run adapter fences pass; `.cyril-6beh/compare-oracles.sh terminal` passes; largest-frame measurement meets budget.

## Slice 5: Node lifecycle adapter

**Claim:** Valid-input C3 subset — `node_start`, `node_complete`, and `node_paused` convert every documented field and contextual status.
**Oracle:** Manifest node method fields, node-status domain, and literal expected variants independent of serde structs.
**Stress fixture:** Double node start without/with session; all start optional fields; completion with each node status and completion optional; paused reason; Unicode/large strings and path segments. Expected exact typed fields; no absent option becomes a sentinel.
**Loop budget:** O(J+H) per frame; one path allocation and average one hash-table bucket probe, no state-map scan or syscall.
**Wall budget:** <=100 ms for 10,000 minimal frames and <=50 ms for a frame containing 64 KiB ids/path segments.
**Files:** `crates/cyril-core/src/protocol/convert/kas/workflow.rs`.

**Verification:** node adapter valid-shape tests match manifest; terminal oracle still passes; 10,000-frame measurement meets budget.

## Slice 6: Progress, pause, and queue adapter

**Claim:** Valid-input C3 subset — `loop_iteration`, `watch_poll`, `paused`, and `steps_queued` convert exact boolean/outcome/reason/descriptor forms.
**Oracle:** Manifest method fields, queue/outcome domains, and literal expected variants.
**Stress fixture:** False/true stop condition; all watch outcomes; empty/Unicode pause reasons; queue empty/nonempty crossed with absent/present resolution, all outcomes, and reason absence/presence. Expected exact typed fields and independent option presence.
**Loop budget:** Fixed-field events O(J); queue descriptor conversion O(J+H), with no hidden cap on `P` or depth.
**Wall budget:** <=50 ms for the representative 256-step/depth-10/1 MiB queue frame; <=100 ms for 100,000 minimal fixed-field frames.
**Files:** `crates/cyril-core/src/protocol/convert/kas/workflow.rs`.

**Verification:** progress/queue adapter tests match manifest; terminal oracle stays green; measurements meet budgets.

## Slice 7: Exact nine-method engine dispatch

**Claim:** C1 — only normalized exact workflow names dispatch for `KasEngine`; `V2Engine`, raw underscore names, near/unrelated inputs, the superseded metadata facade, and default-feature behavior remain unchanged.
**Oracle:** Manifest `method_controls` plus committed pre-change expectations for unrelated extensions and ACP user-message conversion.
**Stress fixture:** In one `kas` build feed both engines all nine normalized names and all nine raw `_kiro/...` names; also feed `run_started`, an unrelated KAS method, and a `session/update user_message_chunk` carrying `_meta.kiro.notification.kind=workflow-progress`/`wf-progress-` id. Repeat default build. Expected: only normalized+Kas emits nine workflow variants; `KasEngine` exact-routes them to `kas::workflow` before falling back to the unchanged `kiro` converter; V2/raw/near do not; unrelated/facade/default retain prior results.
**Loop budget:** Fixed nine-arm match O(Lm) in method-name bytes; rejected names perform no payload traversal.
**Wall budget:** <=50 ms for 100,000 short near misses plus one 64 KiB method-name control.
**Files:** `crates/cyril-core/src/protocol/engine.rs`, `crates/cyril-core/src/protocol/convert/kas/workflow.rs`.

**Verification:** `workflow_method_dispatch_is_engine_exact`, `workflow_superseded_metadata_facade_remains_user_message`, and all existing engine/converter suites pass default/KAS; terminal comparison passes; dispatch budget holds.

## Slice 8: Exhaustive malformed-field isolation

**Claim:** Converter half of C2 — every required omission and known-field wrong type/invalid enum/disallowed null emits the manifest's exact `converter_rejection` warning, drops only that frame, and preserves the next valid conversion.
**Oracle:** Manifest `fields`/`descriptor_fields` schema drives exact case ids and `warning_schemas.converter_rejection` level/message/field/error-kind sets, plus independent expected `drop_then_success` outcomes.
**Stress fixture:** Every required field omitted once; every known field receives representative wrong types and disallowed null; every enum unknown; valid successor after each row. Expected per case id: one WARN `malformed workflow notification` carrying exactly `method,field_path,error_kind,error`, no notification for bad row, one notification for successor, no panic.
**Loop budget:** Production cost remains one O(J+H) parse per frame. Matrix generation is test-only.
**Wall budget:** <=5 s for the exhaustive test matrix; representative 1 MiB/depth-32 opaque JSON and 64 KiB key/id frames each remain <=50 ms; larger accepted inputs remain linear.
**Files:** `crates/cyril-core/src/protocol/convert/kas/workflow.rs`.

**Verification:** `malformed_workflow_field_matrix_isolated` passes exact row-set/warning/successor assertions; current oracle modes pass; budgets hold.

## Slice 9: Exhaustive converter-owned shape matrix

**Claim:** Converter-owned C3 outcomes are exact; duplicate canonical paths alone remain structurally valid through conversion and are rejected atomically by state in Slice 11.
**Oracle:** Entire `oracle-manifest.json`, especially `shape_boundary_outcomes`; raw duplicate-key parse projection; jq capture controls.
**Stress fixture:** All 13 family rows cross every enum/descriptor, opaque JSON and exact number/string/key boundary, optional mask, collection cardinality, raw duplicate key, id/scalar/path/workspace boundary, typed extra, duplicate canonical path, and completion mismatch. Expected family/case sets match exactly: malformed rows warn/drop, lossless rows preserve, and duplicate canonical paths emit one exact typed event for later state rejection.
**Loop budget:** Production conversion O(J+H); fixture generation/sorting test-only O(C log C).
**Wall budget:** <=5 s complete matrix; representative 1 MiB/depth-32 nested JSON, 64 KiB key/id, and 256-node frames each <=50 ms; these are not input caps.
**Files:** `crates/cyril-core/src/protocol/convert/kas/workflow.rs`.

**Verification:** `workflow_oracle_manifest_matches_binary` and the 12 converter-owned C3 tests pass; duplicate-path conversion-preservation control passes; `.cyril-6beh/compare-oracles.sh manifest` and `terminal` pass; budgets hold.

## Slice 10: Tracker storage and public read interface

**Claim:** C7/C11/C13 interface subset — run-scoped storage, exact `get`, allocation-free exact-size `iter`.
**Oracle:** Manifest scale counts plus literal empty/known/unknown lookup and empty/multi cardinality tables.
**Stress fixture:** Privately seed 64 run shells with colliding node ids; query first/middle/last/unknown; iterate empty/multi. Expected exact membership/cardinality; iterator order is unspecified and canonical projectors sort after collection.
**Loop budget:** `get` expected O(W) to hash the workflow-id bytes plus one average bucket probe; explicit `iter` O(R), allocation-free and unsorted; no event-path full-map scan.
**Wall budget:** <=100 ms for 100,000 short-id `get` calls plus <=50 ms for one 64 KiB workflow-id lookup.
**Files:** `crates/cyril-core/src/workflow.rs` (new), `crates/cyril-core/src/lib.rs`.

**Verification:** `workflow_tracker_get_known_and_unknown` and `workflow_tracker_iter_empty_and_exact_size` pass; lookup budget holds; converter oracle modes remain green.

## Slice 11: Atomic snapshot canonicalizer and repeat identity

**Claim:** C4/C5 plus the state-owned C3 duplicate-path outcome — validate/flatten before mutation, reject duplicate canonical paths atomically, and translate only exact repeat wrappers.
**Oracle:** `.cyril-6beh/oracle-snapshot.py`, `.cyril-6beh/falsify-node-paths.py`, manifest `shape_boundary_outcomes`/repeat controls, and duplicate-path expectation.
**Stress fixture:** 256-node/depth-10 snapshot; two valid wrappers; all eight near misses; malformed deepest child; converter-preserved duplicate canonical path. Expected compact canonical projection equals Python; malformed/duplicate inputs return structured error and zero mutation.
**Loop budget:** O(J+H): each descriptor/input byte is visited a constant number of times and every materialized canonical-path byte is hashed/copied a constant number of times; temporary run then one swap. No N/D cap is imposed.
**Wall budget:** <=50 ms for the representative 1 MiB/256-node/depth-10/64 KiB-segment snapshot; <=2 s matrix; larger accepted snapshots remain linear in `J+H`.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `workflow_duplicate_canonical_path_rejected`, repeat/path/atomic tests, and C05 script pass; constructed snapshot projection equals Python fixture projection; budgets hold.

## Slice 12: Snapshot entrypoint/status gates

**Claim:** C4 transition subset — direct snapshots and completion events share canonicalization behind distinct prior/incoming gates.
**Oracle:** Manifest `snapshot_transition_rules`, literal entrypoint×prior×incoming table, exact warning/error manifest.
**Stress fixture:** Direct/completion crossed with missing/active/paused/each-terminal prior and every valid incoming status; exact/non-exact terminal repeats; malformed child on every accepting path. Expected actions exactly `seed/reconcile/unchanged/warn_unchanged/TerminalSnapshotConflict` with atomic hashes.
**Loop budget:** Gate O(1); accepting paths reuse Slice 11 O(J+H); rejecting paths perform no traversal where status suffices.
**Wall budget:** <=2 s full matrix; <=50 ms per representative 1 MiB/256-node input.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `snapshot_entrypoint_status_matrix`, `snapshot_entry_paths_are_equivalent`, `active_snapshot_can_become_terminal`, `invalid_snapshot_is_atomic`, and `terminal_snapshot_conflict_is_atomic` pass; budgets/oracles hold.

## Slice 13: Snapshot field ownership

**Claim:** C4 ownership subset — omitted snapshot-owned fields clear; event-only fields survive reconciliation.
**Oracle:** Manifest `snapshot_owned_run_fields`, `snapshot_owned_node_fields`, and `event_only_fields` exact sets.
**Stress fixture:** Seed every optional ownership class and every event-only field, reconcile snapshots omitting one field at a time and all at once. Expected owned fields clear exactly; event-only fields remain byte-identical; malformed snapshot changes nothing.
**Loop budget:** No additional traversal beyond Slice 11 O(J+H); per-field ownership decisions O(1) during merge.
**Wall budget:** <=2 s presence matrix; <=50 ms each representative 1 MiB/256-node snapshot.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `snapshot_field_ownership_matrix` passes exact field-set/hash assertions; snapshot/terminal/manifest oracles remain green.

## Slice 14: Active opening conflict semantics

**Claim:** C15 — exact active opening is idempotent; each independently conflicting opening field emits the manifest's exact state-rejection warning and changes nothing.
**Oracle:** Manifest `active_opening_rules`, `warning_schemas.state_rejection`, and literal per-field/presence transition table.
**Stress fixture:** Opening, exact duplicate, then independent name/input/tree/parent-session conflicts including None→Some, Some→None, changed Some. Expected unchanged state/cardinality and one WARN `workflow event ignored` carrying `workflow_id,event_kind,reason=active_run_start_conflict`.
**Loop budget:** Opening comparison/canonicalization O(J+H); no runtime-map scan or hard N/D cap.
**Wall budget:** <=50 ms for representative 1 MiB/256 descriptors with 64 KiB string control; <=1 s conflict matrix.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `active_run_start_conflict_presence_matrix` passes; all current oracle modes pass; budget holds.

## Slice 15: Node partial merges and index buckets

**Claim:** C6 — guarded node start/complete merges and bucket-safe id movement.
**Oracle:** Manifest `node_merge_matrix` and `node_index_rules`; tests consume those exact precommitted rows rather than generating expected values from tracker code.
**Stress fixture:** Observed double start; every start/completion optional mask with preseeded prompt/session/progress; id move from shared old bucket into occupied new bucket. Expected values, preserved fields, and old/new bucket membership are exactly the manifest rows.
**Loop budget:** Path event expected O(W+Q+I) to hash workflow/path/node-id bytes plus a constant number of average bucket probes; no bucket or state scan for path-bearing changes.
**Wall budget:** <=1 s full merge/index matrix including 64 KiB workflow/path/node-id controls.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `node_start_merge_presence_matrix`, `node_complete_merge_presence_matrix`, and `node_index_bucket_cardinality_matrix` pass exact values/membership; oracles stay green.

## Slice 16: Queue and independent progress state

**Claim:** C8/C9 — acknowledgement-only resolution and latest-only independent pause/loop/watch fields.
**Oracle:** Manifest `queue_rules`, enum domains, and `progress_permutation_table`; tests consume both explicit event orders and named expected outputs.
**Stress fixture:** No-resolution empty/nonempty replacement; resolution empty/nonempty crossed with all outcomes/reason presence; both manifest orders of run/node pause, false/true loop, and first/second watch. Expected queue rules and the manifest's four final progress values exactly; unrelated fields unchanged; raw history count zero.
**Loop budget:** Queue replacement O(J+H) to validate/materialize supplied descriptors; fixed progress updates O(W+Q+I) hashing plus average constant bucket probes; no history append or hard `P` cap.
**Wall budget:** <=50 ms representative 1 MiB/256-step queue; <=1 s matrices including 64 KiB id/path controls.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `queue_resolution_and_pending_matrix` and `workflow_progress_fields_are_independent_and_current` pass; budgets/oracles hold.

## Slice 17: Unknown identity and typed repeat lookup

**Claim:** C7 — no placeholders for every forbidden identity; repeat lookup filters node type, rejects ambiguity, and emits the manifest warning schema.
**Oracle:** Manifest unknown-event lists, `node_index_rules` typed-resolution outcomes, and `warning_schemas.state_rejection`.
**Stress fixture:** All eight non-opening variants against unknown run; four node updates against unknown node; same-id step+repeat; same-id two repeats; 64 KiB ids/paths. Expected zero mutation/cardinality for rejects, one WARN `workflow event ignored` with exact required fields/reason and applicable optional identity fields; unique repeat alone updates.
**Loop budget:** Path events expected O(W+Q+I) hashing plus average bucket probes. Pathless loop is O(W+I+B×Q̄) within one id bucket, never all nodes/runs; no hard `B` cap.
**Wall budget:** <=50 ms for representative 256-entry bucket including one 64 KiB identity; <=1 s full matrix.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** all four C7 named tests pass; lookup counters and hashes match manifest; oracles stay green.

## Slice 18: Status authority and absorbing terminal matrix

**Claim:** C10 subset — metadata never determines status/liveness; every non-opening post-terminal event is absorbing except exact completion duplicate; rejections use the manifest warning schema.
**Oracle:** Manifest `completion_metadata_matrix`, `post_terminal_events`, `warning_schemas.state_rejection`, terminal status rules, and raw terminal controls.
**Stress fixture:** 4 completion statuses × 7 node statuses × 4 signal states × 3 source states; all eight post-terminal variants; exact/non-exact completion repeats. Expected statuses remain input statuses, liveness derives only from run status; forbidden frames emit exact WARN fields/reasons and remain unchanged; exact duplicate unchanged without warning.
**Loop budget:** Metadata/gates O(W+Q) for key hashing; accepted completion reuses snapshot O(J+H). No stale-state traversal.
**Wall budget:** <=2 s Cartesian/absorbing matrices; <=50 ms representative 1 MiB/256-node snapshot.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `workflow_completion_metadata_never_controls_status`, `post_terminal_event_matrix_is_absorbing`, and terminal truth-table controls pass; terminal oracle remains exact 2/2.

## Slice 19: Full retry-incarnation reset

**Claim:** C10/C13 retry subset — post-terminal same-id opening replaces every prior incarnation field/index.
**Oracle:** Manifest `retry_reset_fields`, independent old/new state manifest, and committed 2.16.2 retry capture.
**Stress fixture:** Fully populate terminal run with nodes/shared buckets/queue/ack/pauses/loop/watch/completion/all optionals, then same-id open with disjoint data. Expected all old manifest fields absent and only new opening data installed; capture's two openings both apply.
**Loop budget:** Destruction of the old incarnation plus new descriptor canonicalization is O(J_old+H_old+J_new+H_new); no prior-history walk beyond releasing current owned state and no hard node/depth cap.
**Wall budget:** <=50 ms representative fully populated 256-node/1 MiB reopening; <=1 s reset matrix.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `retry_opening_clears_full_prior_incarnation`, `post_terminal_run_start_replaces_incarnation`, and capture control pass; no stale field/index remains.

## Slice 20: Assembled conversion/state replay and malformed atomicity

**Claim:** State half of C2 plus C13 — raw malformed frames cannot mutate state/lose successors; complete one/two replays equal the independent folder.
**Oracle:** `oracle-manifest.json`, `.cyril-6beh/oracle.sh`, `.cyril-6beh/oracle-snapshot.py`, `.cyril-6beh/oracle-replay.py`, its all-method fixture, and the three live captures.
**Stress fixture:** Run every malformed row from Slice 8 through converter→tracker with before/after hashes and valid successor. Then replay synthetic/failed/aborted/repeat sources once/twice and project exclusively through public read APIs. Expected bad rows leave byte-identical state and successors mutate; both production pass counts exactly equal every independent folder checkpoint/final projection, including event-only preservation and retry stale-field absence.
**Loop budget:** Per-event parametric budgets from prior slices. Projection sort is test-only O(M log M); production `iter()` remains allocation-free/unsorted.
**Wall budget:** <=5 s malformed assembled matrix and <=2 s representative four-source replays; accepted larger frames retain prior linear bounds.
**Files:** `crates/cyril-core/src/protocol/convert/kas/workflow.rs`, `crates/cyril-core/src/workflow.rs`.

**Verification:** `malformed_workflow_pipeline_is_atomic`, `workflow_capture_replay_matches_independent_folder`, `workflow_capture_replay_is_state_idempotent`, and iterator fences pass; all four `compare-oracles.sh` modes pass exactly.

## Slice 21: Run isolation and 163,840-event budget

**Claim:** C11 — run isolation, no duplicates/history, bounded ordinary lookup at design scale.
**Oracle:** Manifest `scale` exact inputs/counts/lookup bounds and independent deterministic event generator parameters.
**Stress fixture:** 64 workflows × 256 nodes × 10 interleaved short-key events with repeated paths/cross-run id collisions, plus isolated 64 KiB workflow/node/path key lookups. Expected 163,840 events, 64 runs, 16,384 nodes, zero raw history, exact per-event bucket-probe counts from manifest, stable canonical digest, and byte-linear large-key handling.
**Loop budget:** Ordinary event performs one run and node lookup at expected O(W+Q+I) hashing cost; pathless event performs one id-bucket lookup and O(B×Q̄) typed filtering. Fixture totals 163,840 transitions; no full-map scan and no claim that key hashing is O(1).
**Wall budget:** <=5 s scale fixture in opt-level-1 tests; <=50 ms isolated 64 KiB lookup.
**Files:** `crates/cyril-core/src/workflow.rs`.

**Verification:** `workflow_tracker_scale_and_isolation` passes exact manifest counts/digest/counters; all oracle modes pass; measured time <=5 s.

## Slice 22: Exact-once App ownership and error isolation

**Claim:** C12/C14 — App owns tracker, branches on workflow before every existing SessionController/UiState call, consumes it once, and isolates state errors with the manifest warning schema.
**Oracle:** Counters placed at the actual tracker/session/UI call sites, component hashes, `warning_schemas.app_state_error`, unique ids, existing non-workflow App tests, and structural fence.
**Stress fixture:** Inject a valid opening; then a manually constructed domain-level `Notification::Workflow` with duplicate canonical paths (not a converter output) followed by a valid event; interleave 10,000 unrelated notifications. Expected workflow counters tracker=1/frame, session=0, UI=0; the tracker-error frame leaves state atomic and emits one WARN `workflow state application failed` carrying exactly `workflow_id,event_kind,error_kind=duplicate_canonical_path,error`; successor applies; unrelated behavior unchanged.
**Loop budget:** O(1) enum branch plus the tracker's parametric event cost; no new loop/clone/await/channel. Counters test-only.
**Wall budget:** <=100 ms for 10,002 minimal App dispatches excluding rendering; large workflow payload cost remains owned by tracker budgets.
**Files:** `crates/cyril/src/app.rs`.

**Verification:** `workflow_notification_branches_before_other_consumers`, `workflow_notification_is_consumed_exactly_once`, `workflow_state_error_isolated_by_app`, existing App tests, and `.cyril-6beh/check-structure.py` pass; all oracle modes stay green.

## Claim coverage

| Claim | Slices |
|---|---|
| C1 exact conversion | 7 |
| C2 malformed isolation | 8, 20 |
| C3 shape coverage | 1–11, including 3A |
| C4 snapshot path/status/ownership | 11–13 |
| C5 repeat identity | 11 |
| C6 guarded merges/index | 15 |
| C7 unknown identity | 10, 17 |
| C8 queue | 16 |
| C9 progress | 16 |
| C10 status/terminal/retry | 18–19 |
| C11 scale/isolation | 10, 21 |
| C12 ownership | Evidence D, 1–4 including 3A, 10, 22 |
| C13 replay/iteration | 10, 19–20 |
| C14 App dispatch | 22 |
| C15 opening conflicts | 14 |

## Final integration command matrix

Every distinct oracle, falsifier, and permanent fence runs after Slice 22. “Same” means the deterministic falsifier is also its permanent regression fence.

In this table, every bare core test name expands to `cargo test -p cyril-core --features kas NAME`; every bare App test name expands to `cargo test -p cyril NAME`. Tests read the exact independent JSON path supplied by the comparison script or the named manifest section; no expected transition output is generated by production code.

| Claim | Independent oracle/falsifier | Permanent compiled fence |
|---|---|---|
| C1 | `oracle-manifest.json` normalized/raw/V2/KAS/default/near/unrelated/facade controls | `cargo test -p cyril-core --features kas workflow_method_dispatch_is_engine_exact` plus `workflow_superseded_metadata_facade_remains_user_message` |
| C2 | manifest field rows, exact converter warning schema, and before/after hashes | tests `malformed_workflow_field_matrix_isolated` and `malformed_workflow_pipeline_is_atomic` |
| C3 | `compare-oracles.sh manifest`; raw duplicate-key jq | 13 tests: `workflow_node_descriptor_shape_matrix`, `workflow_enum_domain_matrix`, `workflow_arbitrary_json_shape_matrix`, `workflow_scalar_string_matrix`, `workflow_identifier_string_matrix`, `workflow_optional_field_presence_matrix`, `workflow_collection_shape_matrix`, `workflow_duplicate_raw_json_key_last_wins`, `workflow_numeric_and_path_boundaries`, `workflow_workspace_path_is_opaque`, `workflow_typed_unknown_extra_is_ignored`, `workflow_duplicate_canonical_path_rejected`, `workflow_run_complete_status_mismatch_rejected` |
| C4 | `oracle-snapshot.py` + manifest transition/ownership tables | `snapshot_entrypoint_status_matrix`, `snapshot_entry_paths_are_equivalent`, `snapshot_field_ownership_matrix`, `active_snapshot_can_become_terminal`, `invalid_snapshot_is_atomic`, `terminal_snapshot_conflict_is_atomic` |
| C5 | `.cyril-6beh/falsify-node-paths.py` | `repeat_snapshot_paths_match_wire_paths` |
| C6 | manifest `node_merge_matrix`/`node_index_rules` | `node_start_merge_presence_matrix`, `node_complete_merge_presence_matrix`, `node_index_bucket_cardinality_matrix` |
| C7 | manifest variant/type tables | `unknown_workflow_event_matrix_no_placeholders`, `unknown_node_update_matrix_no_placeholders`, `loop_lookup_filters_type_and_rejects_ambiguity`, `workflow_tracker_get_known_and_unknown` |
| C8 | manifest queue table | `queue_resolution_and_pending_matrix` (same) |
| C9 | manifest `progress_permutation_table` | `workflow_progress_fields_are_independent_and_current` (same) |
| C10 | manifest metadata/reset/absorbing tables + `compare-oracles.sh terminal` | `workflow_completion_metadata_never_controls_status`, `retry_opening_clears_full_prior_incarnation`, `post_terminal_event_matrix_is_absorbing`, `workflow_terminal_capture_controls` |
| C11 | manifest scale generator/counts/bounds | `workflow_tracker_scale_and_isolation` (same) |
| C12 | `.cyril-6beh/check-structure.py --self-test`; repo structural run; cargo dependency/privacy compile | default/all-feature builds plus both structural script modes (persistent fence) |
| C13 | `compare-oracles.sh replay` and snapshot/terminal controls; standalone nine-event folder's one/two projections | `workflow_capture_replay_matches_independent_folder`, `workflow_capture_replay_is_state_idempotent`, `workflow_tracker_iter_empty_and_exact_size` |
| C14 | App call-site counters/component hashes + exact App warning schema | `workflow_notification_branches_before_other_consumers`, `workflow_notification_is_consumed_exactly_once`, `workflow_state_error_isolated_by_app` |
| C15 | manifest opening conflict table | `active_run_start_conflict_presence_matrix` (same) |

Then run `.cyril-6beh/compare-oracles.sh manifest`, `terminal`, `snapshot`, and `replay`; `.cyril-6beh/falsify-node-paths.py`; `.cyril-6beh/check-structure.py --self-test` and repo mode; every exact test above; the full default/all-feature test and clippy commands; and every wall measurement. Any mismatch stops the build.

## Plan self-review

### Every loop

- JSON conversion O(J+H); representative 1 MiB/depth-32/256-node and 64 KiB key/id controls <=50 ms; no accepted-size cap.
- Snapshot/opening/retry O(J+H), including hashing/materializing canonical paths and releasing the current incarnation; temporary state then one swap; representative largest fixture <=50 ms.
- Path event expected O(W+Q+I) for key bytes plus constant average bucket probes; never scan all runs/nodes.
- Pathless repeat O(W+I+B×Q̄) within one id bucket; representative B=256/64 KiB control <=50 ms; no hard bucket cap.
- Queue O(J+H); representative P=256/1 MiB <=50 ms; no hard queue cap.
- Explicit `iter` O(R), allocation-free/unsorted; representative R=64; deterministic projectors alone sort output.
- Test-only sort O(M log M), representative M=16,384. Scale fixture 163,840 transitions <=5 s.
- No production path adds syscalls. Hashing/copying is explicitly byte-linear; no lookup is called O(1) with respect to unbounded key bytes.

### Every fixture

Each slice names an expected adversarial outcome and kills a distinct plausible bug: sentinel/overvalidation, missing shape, enum inflation, run/node/progress conversion loss, prefix dispatch, defaulting/null collapse, omitted matrix families, cross-run lookup, repeat-id theft, partial snapshot mutation, transition conflation, stale/cleared ownership, opening overwrite, merge/index corruption, queue clearing, history growth, placeholder creation, metadata-derived liveness, terminal mutation, incomplete retry reset, malformed pipeline mutation, replay duplication, scale scan, or App forwarding/drop.

### Every doc-comment precondition

Non-empty ids, required fields, enum domains, completion context, status agreement, canonical paths, duplicate paths, and transition gates are load-bearing and receive release runtime checks. No correctness rule relies only on `debug_assert!`; no sanity-hint-only precondition is planned.

### Every write target

Production warnings use `tracing` diagnostics. The feature writes no stdout/file/persistence/channel data. Test oracle projections are data in temporary files/stdout; comparison scripts send usage/errors to stderr.

### Every tracker reference

Control/list/load/reattachment remains in verified `cyril-0qe6`; peer-session routing remains in verified `cyril-jxfu`. UI, gating changes, persistence implementation, and full history are excluded by signed `cyril-6beh`; no anonymous deferral exists.

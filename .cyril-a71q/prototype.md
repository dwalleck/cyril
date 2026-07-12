# Prove-it prototype — cyril-a71q

Date: 2026-07-12

## Verdict

**PASSED.** The durable probes cover the revised signed-spec boundary through the public `cyril-core` seam and the real KAS free-path subprocess transport. A Node ACP mock launched via `KIRO_KAS_SERVER_PATH` reproduces the existing same/cross defects and the newly predicted prompt-response-only defect: current production code forwards two global completions and accepts subsequent prompts despite receiving no scoped `turn_end`. The independent artifact-only oracle agrees with 3/9 desired dispositions and identifies exactly the prior three plus the three revised-spec defect observations. This is honest agreement about the current broken substrate, not a claim that the desired sole-`turn_end` behavior is implemented.

**Post-prototype contract correction:** requester choice A fixes normal signed KAS input at `turn_end: Option<one scoped notification>` plus `prompt_response: Option<one RPC result>`. An authoritative `turn_end` aborts that turn's prompt task and discards any response not already observed; response evidence is preserved only when observed before `turn_end`. Repeated scoped `turn_end` is unsupported live-wire drift. The production substrate did not change, so the runtime defect probes remain valid and were not rerun for this design-only correction.

## Smallest questions

1. What fields does a genuine KAS 2.11.0 `turn_end` frame expose, and is any native turn identifier available?
2. Does the existing bridge already preserve KAS liveness when the prompt response is absent and deduplicate the two terminal sources?
3. At which real source boundaries are completion scope, Busy release, routing, shutdown, and disconnect currently decided?
4. Can current session-only state distinguish global v2, same-session stale, cross-session, and KAS dual-source traces from a history that knows the hidden turn owner?
5. Does a KAS prompt response alone currently forward a completion and release the guard so a later prompt is accepted without any scoped `turn_end`?

These are the smallest questions needed before design: wire identity, existing compatibility, ownership/routing seams, prompt-response authority, and independently computed counterexamples.

## Probe size and method

| Probe | Lines | Mechanism | Independent check |
| --- | ---: | --- | --- |
| `.cyril-a71q/probes/extract_turn_end.py` | 51 | Reads the real KAS 2.11.0 JSONL trace, enumerates leaf fields, sanitizes only `params.sessionId`, and compares with pinned fixtures. | Exact structural equality against separately stored fixtures plus an independent search for ID-like leaf paths other than `sessionId`. |
| `.cyril-a71q/probes/ownership_projection.py` | 58 | Projects four fixed traces through a minimal model of current session-only completion behavior. | `hidden_history` computes dispositions from hidden owner identity and completed-owner history rather than the current model's session/active-state rule. |
| `.cyril-a71q/probes/source_inventory.py` | 28 | Lexically inventories named source seams in the real repository. | Runtime KAS tests and the trace/oracle probes cover the behaviorally important seams by different mechanisms; lexical results are not presented as runtime proof. |
| `.cyril-a71q/probes/runtime/src/main.rs` | 54 | Calls public `spawn_bridge(AgentCommand, AgentEngine::Kas, KasSpawn::Free, cwd)` and records routed notifications. | The Node trace independently records requests accepted by the real subprocess connection. |
| `.cyril-a71q/probes/mock_kas_server.js` | 51 | ACP-over-stdio mock launched by production KAS discovery through `KIRO_KAS_SERVER_PATH`; its `response_only` branch responds to prompts 1 and 2 and emits no `turn_end`. | Rust output records what crossed the bridge boundary, independently of mock labels. |
| `.cyril-a71q/probes/run_runtime_probes.py` | 57 | Creates the fixture-schema sqlite auth store in an isolated fake home, captures all three scenarios, then removes DB/home/target. | Durable run output records successful exits and absent cleanup paths. |
| `.cyril-a71q/probes/runtime_oracle.py` | 40 | Reads only durable stdout and mock traces; hidden owner labels and the signed sole-`turn_end` rule determine expected dispositions. | It shares no bridge state or projection logic and compares nine observations item by item. |

All probe source files are **<=100 lines**.

## Genuine wire evidence

`PROVENANCE.md` pins two sanitized copies of genuine inbound frames from `experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl`:

- `turn-end-cancelled.json`: source line 84, timestamp `1783041453801`.
- `turn-end-end-turn.json`: source line 525, timestamp `1783042242922`.

The only sanitization is replacement of `params.sessionId` with `sess_<sanitized>`; no field was added, removed, renamed, or reordered in the captured `msg`. Extraction reports `capture_count=2`, `fixture_count=2`, and `pinned_exact=True` for both frames.

Both frames expose exactly these observed leaves:

- `jsonrpc`
- `method`
- `params.sessionId`
- `params.update.sessionUpdate`
- `params.update._meta.kiro.kind`
- `params.update._meta.kiro.stopReason`
- `params.update._meta.kiro.turnEnd.stopReason`

The values distinguish `cancelled` and `end_turn`, while both use `jsonrpc="2.0"`, `method="session/update"`, `sessionUpdate="session_info_update"`, and `kind="turn_end"`. Both report `native_turn_id_candidates=[]`. No native KAS turn-id candidate is evidenced.

## Complete material-boundary inventory

| Material boundary | Durable observation | Coverage disposition |
| --- | --- | --- |
| Live KAS terminal wire shape | Two exact KAS 2.11.0 frames; seven observed leaf paths; no native turn-id candidate. | Covered by real trace extraction and exact pinned fixtures. |
| KAS terminal conversion/routing entry | Current source contains scoped KAS routing at the client. | Covered by source inventory; lexical evidence only. |
| KAS fixed-pair liveness and current dedup | `kas_turn_end_completes_without_prompt_response` and `kas_turn_end_and_prompt_response_dedupe_to_one` pass 2/2. They exercise one `turn_end` plus one prompt result, not repeated scoped frames. | Covered against the existing bridge runtime; requester choice A adds the not-yet-implemented abort/discard requirement. |
| Notification backpressure substrate | `NOTIFICATION_CAPACITY` is 256. | Covered as current-source fact; 257-event behavior is explicitly excluded below. |
| Active-turn guard | `turn_in_flight` is `Option<acp::SessionId>`. | Covered as current-source fact: ownership is session-only. |
| Completion release guard | Completion checks only whether the guard `is_some`/`is_none`; there is no owner comparison. | Covered as current-source fact and exercised by projection. |
| Global v1/v2 terminal path | Synthesized v1/v2 `TurnCompleted` is global (`session_id: None`). | Covered by source inventory and hidden-owner/current agreement on `global_v2`. |
| Scoped KAS terminal path | KAS notification is scoped to `session_id`. | Covered by source inventory and the KAS/cross-session projection traces. |
| App foreign-session boundary | App applies a foreign notification to subagent state and returns early. | Covered as current-source fact and represented by the independent foreign-forward oracle disposition. |
| Same-session stale ownership | Real bridge output forwards A's scoped terminal, then forwards A's late global prompt response as `terminal-2`; the mock trace proves prompt C reached the server before B's own terminal. | Covered through production KAS discovery, subprocess transport, ACP conversion, mediator, and public notification channels; defect reproduced. |
| Cross-session ownership | Real bridge output forwards foreign X's scoped completion; the mock trace proves prompt C reached the server before B's own terminal, demonstrating X cleared the main guard. | Covered through the same production runtime seam; defect reproduced while preserving X's routed scope. |
| KAS distinct-source ownership | Existing runtime deduplicates one scoped `turn_end` plus one global prompt result while no newer turn intervenes. Under requester choice A, response-before-turn_end is evidence and turn_end aborts/discards the task; repeated scoped frames are unsupported drift. | Covered for the current distinct-source substrate; abort/discard is a design obligation, not prototype-proven behavior. |
| KAS prompt-response-only authority | With zero scoped `turn_end` frames, real bridge output contains two global `TurnCompleted` notifications; the mock trace proves R2 and then R3 reached the subprocess. | Covered through public `spawn_bridge` and production KAS transport. This reproduces the current defect against the revised contract: a response should be secondary/nonterminal and leave the guard Busy. |
| Shutdown/process lifetime | Current shutdown aborts the prompt task. | Covered as current-source fact; no new shutdown semantics are claimed. |
| Prompt error/process death | Current source contains deferred disconnect state. Related durable evidence pins existing `BridgeError` -> `TurnCompleted` -> `BridgeDisconnected` behavior. | Inventoried as a design/test preservation boundary; this prototype does not claim a new owned lifecycle implementation. |
| Cancellation | Existing cancellation targets the in-flight session; cancellation reason authority remains cyril-pnwb. | Inventoried and explicitly left for ownership implementation tests; no precedence claim. |
| Rate-limit consumer | cyril-3zy4 will consume ownership but its payload converter, text, retries, and terminal representation are separate. | Explicitly excluded from this prototype and feature implementation scope. |
| Ownership identity mechanism | Current inventory finds no `TurnId`, `turn_seq`, `turn_owner`, or `ownership_counter` match (`ownership_counter_absent=False` means the absence-check regex did not match). | Explicitly not yet implemented; design choice excluded. |
| Identity exhaustion | No allocator exists yet. | Explicitly excluded from prototype behavior; later implementation must inject the `u64` boundary and fail closed after the last unused value. |
| 257-notification backlog | Only the 256 capacity is observed. | Explicitly excluded from prototype behavior; later validation must pause the consumer, enqueue/reconcile 256 notifications plus the blocked terminal event. |

## Independent oracle rationale and results

The two mechanisms do not share the decisive state:

- `current_projection` stores only the active session and clears it on any completion while active, mirroring the observed session-only guard and `is_some` release substrate.
- `hidden_history` stores `(session, owner)`, a completed-owner set, and independently classifies foreign, owned, and stale events. The owner values exist only in the probe trace; they are not inferred from current code or KAS fields.

Therefore the oracle can compute the contract disposition independently rather than restating the implementation. Results:

| Trace | Hidden-owner oracle | Current projection | Interpretation |
| --- | --- | --- | --- |
| `global_v2` | `complete-active` | `complete-active` | Agree: global synthesized completion compatibility must be preserved. |
| `same_session_stale` | complete A, drop stale A, complete B | complete A, complete B on late A, then drop B | Expected mismatch exposes the session-only defect. |
| `cross_session` | forward foreign X, complete B | complete B on X, then drop B | Expected mismatch exposes missing scope-sensitive ownership. |
| `kas_dual` | complete on first source, classify second as stale | complete on first source, then generic drop | Expected mismatch exposes the absence of owner/history classification even though the no-new-turn runtime case deduplicates. |

The global-v2 agreement validates a compatibility constraint. The projection's other three mismatches are agreeing evidence about the known substrate defect because current code lacks hidden owner identity.

The runtime oracle then reads only the durable bridge stdout and mock traces. Its nine comparisons preserve the original six unchanged and add three signed-contract checks: R1 response forwards 0 completions, R2 is rejected while Busy, and R2 response forwards 0 completions. Result: `item_agreement=3/9`. The prior exact defect set remains `same/A late response`, `same/C before B terminal`, and `cross/C before B terminal`; the new exact defects are `response/R1 prompt response`, `response/R2 prompt accepted`, and `response/R2 prompt response`. Output records `existing_defect_set_preserved=True`, `revised_spec_defect_reproduced=True`, and `model_defect_reproduced=True`. Thus runtime behavior matches the predicted current defect without claiming the desired behavior exists; no oracle rule was rewritten to fit the bridge.

## Item-by-item evidence matrix

| Required fact | Evidence | Result |
| --- | --- | --- |
| Two genuine KAS 2.11.0 frames pinned exactly after sessionId-only sanitization | `PROVENANCE.md`, both fixtures, `turn-end-extraction.txt`: 2 captures, 2 fixtures, both `pinned_exact=True`. | Proven. |
| Observed fields and no native turn ID | Extraction lists only `jsonrpc`, `method`, `sessionId`, `sessionUpdate`, `kind`, both stop-reason paths; both ID candidate lists are empty. | Proven for the two captures. |
| Existing dual-source dedup | KAS bridge test output: dedup test `ok`. | Passed. |
| Existing absent-response liveness | KAS bridge test output: absent prompt-response test `ok`. | Passed. |
| Existing KAS bridge test total | `2 passed; 0 failed`. | 2/2 passed. |
| Channel capacity | Source inventory: `notification_capacity_256=True`. | Proven lexically. |
| Session-only guard | `turn_guard_session_only=True`. | Proven lexically. |
| Completion `is_some` guard | `completion_checks_only_some=True` (pattern checks the inverse early guard). | Proven lexically. |
| Global synthesized v1/v2 | `synthesized_global=True`, `global_has_none=True`. | Proven lexically; compatible oracle trace agrees. |
| Scoped KAS | `kas_scoped_at_client=True`. | Proven lexically. |
| App foreign early return | `app_foreign_early_return=True`. | Proven lexically. |
| Shutdown abort | `shutdown_aborts_task=True`. | Proven lexically. |
| Deferred disconnect | `deferred_disconnect=True`. | Proven lexically. |
| No ownership counter | The counter-pattern result is false. | Proven lexically for named current-source spellings; see blindness. |
| Runtime same-session trace | Scoped A completion forwarded; late A response forwarded globally; C accepted before B terminal. | Production bridge defect reproduced. |
| Runtime cross-session trace | Scoped X completion forwarded; C accepted before B terminal; B terminal later forwarded. | Production bridge defect reproduced. |
| Runtime response-only trace | Two global prompt-response completions forwarded; R2 and R3 accepted; zero `turn_end` frames emitted. | Revised sole-`turn_end` contract defect reproduced through the public production seam. |
| Runtime artifact oracle | 3/9 desired dispositions; prior exact three defects preserved and new exact three response-only defects found; both named defect checks are true. | Honest item-by-item reconciliation. |
| Oracle comparison | Global-v2 projection agreement plus defect-revealing projection/runtime differences. | Sufficient to falsify session-only ownership and current KAS prompt-response authority without rejecting the probes. |
| Probe size | 51, 58, 28, 54, 51, 57, and 40 lines. | 7/7 <=100. |
| Cleanup | Fake sqlite DB/home and probe `target/` absent after capture. | No generated fake credentials or build tree retained. |

## Exact recorded commands

These are the exact recorded probe/test invocations. The first four were pre-existing durable evidence; the focused runtime runner and artifact oracle were run for this boundary capture. No broad suite was rerun:

```text
python .cyril-a71q/probes/extract_turn_end.py
python .cyril-a71q/probes/ownership_projection.py
python .cyril-a71q/probes/source_inventory.py
cargo test -p cyril-core kas_turn_end
python .cyril-a71q/probes/run_runtime_probes.py
python .cyril-a71q/probes/runtime_oracle.py
```

Recorded validation summaries:

- Extraction: 2/2 genuine captures match pinned sanitized fixtures exactly; 0 native turn-id candidates.
- Ownership projection: global v2 agrees; same-session stale, cross-session, and KAS dual traces expose the known substrate defect.
- Source inventory: all nine positive seam checks are true; the ownership-counter search does not match.
- KAS bridge tests: 2 passed, 0 failed, 520 filtered out.
- Runtime probes: `same: exit=0`, `cross: exit=0`, `response_only: exit=0`; all real bridge traces reproduced the predicted current defects.
- Response-only runtime: two global `TurnCompleted` notifications plus `guard-probe-sent`; mock trace contains accepted R1, R2, and R3 prompts and zero `turn_end` frames.
- Runtime oracle: 3/9 desired dispositions; prior three defects preserved; exact three response-only defects matched; `existing_defect_set_preserved=True` and `revised_spec_defect_reproduced=True`.
- Cleanup: `fake_home_exists=False target_exists=False`; the sqlite DB was deleted after capture.

## New learning and named blindness

**Genuinely new learning:** the public production seam confirms that current KAS prompt responses are independently terminal: with no scoped `turn_end`, each of the first two responses forwards a global completion and releases the guard, allowing R2 and R3 onto the real ACP subprocess. This is precisely the behavior the revised signed spec must change. The earlier routing/guard decoupling and absence of a native turn identifier remain established.

**Named blindness — scripted-runtime and captured-wire blindness:** the three deterministic subprocess schedules do not cover other scheduler interleavings, protocol-violating repeated scoped `turn_end`, live-agent wire drift, or future consumers that bypass `RoutedNotification`; two genuine frames cannot prove every KAS version lacks an identifier. The response-only trace proves current release behavior but cannot prove response-before-turn_end evidence retention or turn_end-driven task abortion/discard. The runtime probe also does not observe the not-yet-implemented allocator, exhaustion failure, rate-limit consumer, or 257-event paused-consumer boundary. Those remain explicit implementation obligations. The pre-existing `unused_mut` warning at `crates/cyril-core/src/protocol/kas/host_io.rs:155` remains residual and was not fixed.

## Exact six-item hard gate

1. **PASS — smallest falsifiable questions:** five bounded questions isolate live wire identity, existing KAS compatibility, real ownership seams, trace disposition, and response-only KAS release authority.
2. **PASS — real material boundaries:** genuine KAS captures, real repository source, recorded compatibility evidence, and all three runtime traces run through public `spawn_bridge`, production free-path discovery, a real Node subprocess, ACP serialization, conversion, and bridge mediation.
3. **PASS — complete boundary accounting:** every material prototype boundary, including response-only KAS authority, is covered or explicitly excluded; later implementation obligations for identity exhaustion and the 257 backlog remain named.
4. **PASS — independent oracle:** hidden owner labels, artifact ordering, and the signed sole-`turn_end` rule compute dispositions without importing bridge state or sharing projection logic.
5. **PASS — honest reconciliation:** the runtime oracle reports 3/9 desired dispositions, preserves the prior exact defect set, and identifies the exact three predicted response-only defects; both named checks are true, with no oracle rewrite.
6. **PASS — bounded evidence with learning and blindness:** all seven probe sources are <=100 lines, generated target/fake-home are removed, the prompt-response authority learning is recorded, and blindness is named.

**Hard-gate result: 6/6 PASS.** Prototype may advance to design. This does not assert that ownership, exhaustion, or maximum-backlog behavior is implemented.

## Structured acceptance report

The current task's structured report is returned with the implementation handoff; durable evidence is under `.cyril-a71q/probes/output/runtime/`. No production source or tests changed.

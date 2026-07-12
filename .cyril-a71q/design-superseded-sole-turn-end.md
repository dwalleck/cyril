# Falsifiable design — cyril-a71q

Date: 2026-07-12
Status: **DESIGN GATE PASSED; review approval is not claimed**

## Purpose

Associate every accepted prompt with a process-local owner so only that turn's authoritative observation can release it. Preserve synthesized global v1/v2 completion, scoped foreign routing, fail-stop visibility, App/session/UI exactly-once effects, checked `u64` identity, and bounded-channel delivery.

Requester choice A fixes the KAS model:

- normal signed input per accepted KAS prompt is `turn_end: Option<one scoped notification>` plus `prompt_response: Option<one RPC result>`;
- a response observed before `turn_end` is owner-stamped secondary source/reason evidence and releases 0 turns;
- authoritative scoped `turn_end` releases once, aborts that turn's prompt RPC task, and discards any result not already observed;
- repeated scoped `turn_end` frames are unsupported live-wire drift, not a production input this design claims to protect.

This removes both superseded blockers. No old KAS task survives into B, and no normal old KAS source remains capable of producing an uncorrelated scoped frame during B.

## Input shapes

Every production-reachable ownership shape touched by the feature is enumerated. Unsupported wire drift is listed separately so it is not mistaken for protected behavior.

| Input family | Production shapes | Required disposition | Claims |
| --- | --- | --- | --- |
| Engine | `V2`; `Kas` | v1/v2 prompt result is terminal; KAS result is secondary evidence and scoped `turn_end` is ordinary authority. | C1, C2 |
| `SendPrompt` guard | no active owner; active same-session owner; active distinct-session owner | Accept only the no-active case after owner allocation; active cases emit Busy and start zero RPCs. | C1, C3 |
| Prompt session ID | empty opaque string; ASCII; Unicode; same as or distinct from current session | Treat as opaque routing/cancel data, never turn identity. Existing fingerprint refusal is unchanged. | C3, C5 |
| `content_blocks: Vec<String>` | empty; one; multiple distinct; duplicates | Every accepted collection gets one dispatch owner; content never correlates a terminal. | C2, C3 |
| Content string | empty; ASCII; Unicode; whitespace/path-like | Ownership is invariant under content; App validation remains unchanged. | C2 |
| Owner allocator | initial `0`; interior value; final unused `u64::MAX`; exhausted | Allocate each value once; exhaustion starts zero turns and fails closed visibly. | C8 |
| v1/v2 prompt `Ok` | `EndTurn`, `MaxTokens`, `MaxTurnRequests`, `Refusal`, `Cancelled` | Dispatch-owner-stamped global terminal forwards once only for its owner. | C2 |
| v1/v2 prompt `Err` | owner active; stale owner after a fail-stop boundary | Preserve owner-stamped `BridgeError` then completion; never release another owner. Connection death still adds disconnect in order. | C5 |
| KAS fixed source pair | each cell of `{turn_end: None/Some(one)} × {prompt_result: None/Some(one)}` | `turn_end` only: complete/abort; response only: evidence/Busy; response then turn_end: evidence then complete/abort; turn_end then would-be response: complete/abort and discard result; neither: Busy. | C1, C2, C6 |
| KAS response reason | every valid domain reason | Preserve exact reason only if the response is observed before `turn_end`; release 0. | C6 |
| KAS prompt result `Err` | observed before `turn_end`; would be observed after `turn_end`; concurrent connection death | Before: fail-stop sequence and close before B. After: discarded with the aborted owner. Death: the same ordered fail-stop closes the bridge. | C4, C5 |
| KAS scoped `turn_end` scope | `Some(active owner's session)`; `Some(foreign)`; `Some(active session)` with no active owner | Owned scope completes/aborts; foreign routes once with zero main mutation; no-active same scope drops. `None` is unreachable because `session_notification` always wraps the envelope ID. | C1, C3 |
| KAS stop reason field | every valid reason; missing; malformed | Preserve valid reason; missing/malformed defaults to `EndTurn` and still completes. | C2, C6 |
| Repeated KAS scoped `turn_end` | two or more frames for one accepted prompt | Unsupported protocol drift outside normal signed input. No stale-isolation guarantee is claimed for this identity-free shape. | C1, C6 |
| Global result scope | v1/v2 global terminal; KAS global evidence; unowned global terminal | v1/v2 uses dispatch owner; KAS never completes; unowned terminal drops. | C2, C3 |
| Routed scope | `None`; `Some(main)`; `Some(foreign known)`; `Some(foreign unknown-to-tracker)` | Only owned applicable main completion enters main consumers; both foreign forms follow App's early return. | C3, C7 |
| Nonterminal notification | global; main scoped; foreign scoped | Ownership mediation does not suppress or reinterpret it. | C3, C9 |
| Cancellation | no active owner; active T; active T after current-session retarget; response-before-turn_end; turn_end after cancel | No-active logs only; active cancel targets T's immutable session. KAS remains Busy until its one `turn_end` or fail-stop; response is evidence only. | C4, C5 |
| Process death | idle; active T; response already observed; turn_end/result race | Idle disconnects without fabricated completion. Active emits ordered error/completion/disconnect and accepts no B. If `turn_end` wins observation, its abort/discard rule owns the result. | C4, C5 |
| Pending disconnect | `None`; `Some(owner, reason)` | Only its owner's marker consumes it; sequence closes the bridge before another prompt. | C5 |
| Shutdown | no task; one active task; response already resolved but turn still Busy | Abort/take the active task if present, exit with zero required completions, isolate a fresh bridge. No prior KAS task exists after `turn_end`. | C4 |
| Fresh bridge lifetime | initial; fresh process after shutdown/death/exhaustion | New allocator and channels; old events cannot cross. | C4, C8 |
| Rate-limit observation | active/no active; global/scoped representation; before turn_end; stale during U; one/repeated | Forward zero KAS App completions and release zero turns from observation alone. | C10 |
| Backpressure collection | empty; one; 2–255 distinct; duplicates; 256 full; terminal blocked as 257th; receiver closed; receiver live then resumed | Live resumed receiver receives all 257 once in order and correct final ownership; closed receiver exits. | C9 |

Numeric negatives do not exist for `u64` owners or collection lengths. Wall clocks, paths, time zones, persistence, soft deletion, caches, and replication do not participate because ownership is process-local.

## Removed invariants

### Classification

The change is **subtractive**. It removes generic “any `TurnCompleted` while a session is in flight may clear it” semantics and removes prompt-response terminal authority under KAS. The old single-task constraint remains safe only because choice A aborts/takes KAS task ownership at `turn_end` before the next prompt is accepted.

| Sweep target | Removed/exposed assumption | Still-holds replacement | Claim |
| --- | --- | --- | --- |
| Prompt-task replacement | A KAS task could outlive release and be overwritten by B. | `turn_end` aborts/takes A before clearing A; B is accepted only afterward. Shutdown owns at most the active task. | C1, C4 |
| Active guard | `Option<SessionId>` was treated as terminal epoch. | Active record is immutable `{owner, engine, session, task, evidence}`; Busy/release/cancel use it. | C1, C3 |
| Late global response | Generic task completion could clear B. | Owner is captured at dispatch; v1/v2 remains terminal; KAS response before turn_end is evidence, and post-abort result is discarded. | C1, C2 |
| Scoped KAS `turn_end` | No native ID exists on the frame. | Normal input supplies at most one such frame; response cannot release; therefore active T remains the unique eligible owner until its frame. Repeated frames are unsupported drift. | C1 |
| Scoped foreign `turn_end` | Bridge guard could clear before App routes foreign, creating split-brain. | Foreign routes once and mutates zero main ownership or Busy state. | C3, C7 |
| Pending disconnect | Any completion could trigger an unkeyed disconnect. | Pending disconnect is owner-keyed; fail-stop sequence closes before another command is accepted. | C5 |
| Cancellation | Current session can change while T runs. | Cancel reads T's immutable owner session; KAS release still waits for authoritative turn_end/fail-stop. | C4, C5 |
| App/session/UI effects | Every forwarded main completion was assumed applicable. | Only an owned applicable completion reaches main consumers; evidence/rate-limit/stale/foreign inputs cause zero main completion effects. | C7 |
| Source/reason evidence | First-source dedup erased provenance. | Active record holds at most one response observation plus the one turn_end observation; unobserved post-turn_end response is intentionally discarded. | C6 |
| Identity uniqueness | Counter wrap/reuse would recreate an owner. | Checked allocation uses every `u64` once and then fails closed. | C8 |
| 256+1 delivery | Correct classification could be lost at capacity. | Awaited forwarding remains lossless while receivers live; release commits with terminal delivery. | C9 |
| Rate-limit side path | A terminal-looking consumer could bypass source authority. | Rate-limit has no release operation. | C10 |

Still safe: the single mediator serializes observations and commands; there is at most one active accepted turn and at most one owned prompt task; nonterminal conversion remains engine-selected; App foreign routing remains an early return.

## Architecture

### Owner allocation and active state

- `TurnId(u64)` is allocated with checked, never-reused semantics when `SendPrompt` is accepted, before RPC dispatch.
- Active state contains `{ owner, engine, session, prompt_task, response_evidence }`.
- The task captures owner/engine/session at dispatch. Receive-time state never relabels a task result.
- Busy rejection and cancel targeting read the active record, not task completion or mutable current session.

### Engine-specific result mediation

- **v1/v2 `Ok`:** synthesize a global owner-stamped terminal. It forwards/releases only if owner equals active owner.
- **v1/v2 `Err`:** emit owner-stamped `BridgeError` then owned completion; connection death adds `BridgeDisconnected` in the established order.
- **KAS `Ok` observed before turn_end:** store one owner-stamped response source/reason observation, remove the finished task handle, emit zero App completions, and remain Busy.
- **KAS result after turn_end:** cannot originate from a live owned task because `turn_end` aborts/takes it. If a result was already queued, its dispatch owner is stale and the mediator discards it.
- **KAS `Err` observed before turn_end:** atomically enter fail-stop, emit `BridgeError → owned TurnCompleted → BridgeDisconnected`, close command processing, and exit. No B can be accepted, so T's optional unobserved turn_end cannot meet a newer turn.

### Scoped KAS `turn_end`

Normal input contains at most one scoped `turn_end` for T. Since KAS response never releases, T is still active when that frame arrives. The mediator:

1. distinguishes foreign scope first and routes foreign completion without main mutation;
2. for scope equal to active KAS owner session, stamps T, aborts/takes T's prompt task immediately, and marks any queued T result stale;
3. awaits forwarding of exactly one applicable completion;
4. clears T only after successful delivery, then permits B;
5. retains only evidence already observed before the frame.

No timeout or FIFO history is required. A repeated same-turn scoped frame violates the signed fixed-pair input and is recorded only as live-wire blindness.

### Cancellation, death, and shutdown

- Cancel targets active owner's immutable session and preserves existing terminal-child reap policy.
- Cancel does not itself grant KAS response authority; normal KAS release remains turn_end.
- IO death and pre-turn_end KAS prompt failure are fail-stop. The mediator does not return to command selection between the ordered lifecycle events and exit.
- Shutdown aborts/takes the sole active task and exits with zero required completions.
- A fresh bridge owns new channels and a new owner domain.

### App/session/UI and evidence seam

- Foreign scoped notification retains its `RoutedNotification` scope and follows App's known/unknown subagent early return.
- Only owned applicable main completion reaches `SessionController::apply_notification` and `UiState::apply_notification`, producing one summary/cost/status/activity transition.
- The observer seam represents `{owner, source, scope, reason}`. For KAS it can contain prompt-response evidence only if observed before turn_end. This preserves the cyril-pnwb seam without choosing reason precedence or retaining unobserved history.

## Claims

1. **C1 — fixed-pair KAS reachability:** A response observed before turn_end releases 0, turn_end releases A exactly once and aborts/takes A's task, B is accepted only afterward, and no A result enters B's lifetime.
2. **C2 — engine compatibility:** Dispatch-owner-stamped v1/v2 global response remains terminal, while KAS prompt result is never an ordinary completion and owned scoped turn_end remains sole ordinary KAS authority.
3. **C3 — guard and scope isolation:** Busy rejection, stale drop, owned release, and foreign forwarding use immutable owner/scope classification; foreign completion routes once and changes zero main state.
4. **C4 — task/cancel/shutdown ownership:** The bridge owns at most the active prompt task, cancel targets its immutable session, and turn_end or shutdown aborts/takes that task without orphaned work.
5. **C5 — lifecycle fail-stop:** KAS prompt error observed before turn_end and active connection death preserve `BridgeError → owned TurnCompleted → BridgeDisconnected`, accept zero subsequent prompts, and cannot expose T's optional turn_end to U.
6. **C6 — bounded source/reason seam:** At most one pre-turn_end KAS response and one turn_end reason are preserved per active owner; missing/malformed turn_end reason falls back to `EndTurn`; unobserved post-turn_end response is discarded without a precedence decision.
7. **C7 — consumer effects:** Exactly one owned applicable completion causes one SessionController and UiState completion transition; stale, foreign, evidence-only, and rate-limit observations cause zero main completion transitions.
8. **C8 — exhaustion:** The final unused `u64` owner is allocated once and the next request starts zero turns, emits one visible fail-closed lifecycle signal, and requires a fresh domain.
9. **C9 — backpressure:** With a live receiver that resumes, 256 queued notifications plus the blocked terminal 257th are delivered once in order, and ownership changes only with delivered terminal authority.
10. **C10 — rate-limit nonauthority:** Any one or repeated rate-limit observation, before or after a turn boundary, forwards zero KAS App completions and releases zero turns by itself.

## Falsification

| # | Claim | Falsifier and falsifying result | Independent oracle | Concrete buggy implementation | Cost | Status | Deterministic fence / distinct output | Blindness |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C1 | Fixed-pair reachability | Trace response A → request B → turn_end A → request B, plus turn_end-first then attempted response A. Any early B, response release, completion count ≠1, live A task, or A evidence during B falsifies C1. | Fixed-pair transition oracle and hidden owner labels, independent of bridge state. | Generic KAS response completion or clearing before abort/take. | <1s | **passed** | `.cyril-a71q/probes/design_revised_falsifier.py`; `REVISED-CHEAPEST-PASSED` | B1, B11 |
| C2 | Engine compatibility | Run v1/v2 response and four KAS pair cells. Counts other than v1/v2=1; KAS=`1,1,0,0` by turn_end presence, or wrong owner, falsify C2. | Fake ACP request IDs and external source-authority table. | Suppress global v1/v2 or treat KAS response as terminal. | 3m | pending implementation | `engine_terminal_source_matrix`; `C2-ENGINE` | B1, B8 |
| C3 | Scope/guard | During main B inject unowned main, foreign-known, foreign-unknown, and no-active global/main/foreign events. Main release, foreign loss, or early prompt acceptance falsifies C3. | Per-receiver count ledger and fake-agent prompt count. | Clear on variant before scope routing. | 3m | pending implementation | `terminal_scope_owner_matrix`; `C3-SCOPE` | B2, B9 |
| C4 | Task/cancel/shutdown | Park KAS task, retarget current session, cancel, then inject turn_end or shutdown. Wrong cancel session, task alive after boundary, more than one owned task, or required shutdown completion falsifies C4. | Wire cancel transcript and task-drop sentinel. | Single task handle overwritten before abort, or cancel current session. | 4m | pending implementation | `active_prompt_task_boundary`; `C4-TASK` | B4, B11 |
| C5 | Lifecycle fail-stop | Inject KAS prompt Err before optional turn_end and active IO death; queue B and the optional turn_end. Wrong event order/count, accepted B, or post-disconnect frame delivery falsifies C5. | Ordered channel recorder, process-kill lever, server accepted-prompt transcript. | Emit error+completion then resume command loop. | 4m | **passed at model seam** | `design_revised_falsifier.py` model output plus bridge fence `kas_error_is_failstop`; `CHOICE-A failure_order` | B4, B17 |
| C6 | Evidence seam | Exercise response-first disagreement, turn_end-first, response-only, turn_end-only, and missing/malformed reason. Lost observed tuple, retained unobserved response, wrong fallback, or asserted precedence falsifies C6. | Captured raw fields and separately encoded expected tuples. | Keep task after turn_end or collapse sources to bare completion. | 3m | pending implementation | `kas_evidence_cutoff_matrix`; `C6-EVIDENCE` | B3, B10, B11 |
| C7 | Consumer effects | Snapshot public SessionController/UI observables around owned, stale, foreign, KAS evidence, and rate-limit events. Any non-owned main delta or owned transition count ≠1 falsifies C7. | Expected public-state delta table and routed receiver identity. | Forward stale completion and rely on consumers to dedupe. | 4m | pending implementation | `only_owned_completion_mutates_main`; `C7-EFFECTS` | B2, B13 |
| C8 | Exhaustion | Inject final-unused and exhausted allocator states. Skip/reuse max, another server prompt, or visible signal count ≠1 falsifies C8. | Arithmetic boundary fixture and server prompt count. | `wrapping_add`, `saturating_add`, or same-process reset. | 1m | pending implementation | `turn_owner_exhaustion_fails_closed`; `C8-EXHAUSTION` | B6, B12 |
| C9 | Backpressure | Pause receiver, fill IDs 0..255, block terminal 256, resume, reconcile order and guard. Missing/duplicate/reordered ID, early B, or wrong final owner falsifies C9. | Independently generated range and receiver-order ledger. | `try_send` drop or clear before failed delivery. | 3m | pending implementation | `owned_terminal_survives_256_backlog`; `C9-BACKPRESSURE` | B7, B12 |
| C10 | Rate-limit nonauthority | Inject one/repeated rate limits while T active and while U runs without turn_end. Any App completion, release, or accepted next prompt falsifies C10. | App completion count and fake-server prompt transcript. | Map `RateLimited` directly to release. | 2m | pending implementation | `rate_limit_never_releases_kas`; `C10-RATE-LIMIT` | B5, B12 |

All pending fences are deterministic CI obligations for implementation, not manual measurements or a build plan.

## Cheapest falsifier result

Persisted artifacts:

- `.cyril-a71q/probes/design_revised_falsifier.py`
- `.cyril-a71q/probes/output/design-revised-cheapest-falsifier.txt`

Command:

```text
python .cyril-a71q/probes/design_revised_falsifier.py > .cyril-a71q/probes/output/design-revised-cheapest-falsifier.txt
```

Output:

```text
CHOICE-A response_releases_A=False
CHOICE-A response_evidence=[('A', 'cancelled')]
CHOICE-A B_rejected_before_turn_end=True
CHOICE-A turn_end_completions=['A']
CHOICE-A active_task_after_turn_end=None
CHOICE-A B_active_after_turn_end=True
CHOICE-A abort_record=['abort:A']
CHOICE-A late_A_entered_B_lifetime=False
CHOICE-A discarded_after_abort=[('A', 'cancelled')]
CHOICE-A failure_order=['BridgeError', 'TurnCompleted', 'BridgeDisconnected']
CHOICE-A B_rejected_after_failstop=True
REVISED-CHEAPEST-PASSED
REVISED-DESIGN-GATE-PASSED
```

The result is non-vacuous: making `response()` clear active admits B early; omitting turn_end's task abort lets A evidence enter B; returning to command selection after prompt error accepts B. Each mutation fails a distinct assertion/output.

## Material-boundary accounting

Every material boundary in `prototype.md` is carried here.

| Prototype boundary | Design disposition |
| --- | --- |
| Live KAS terminal wire shape | C1/C6 use one scoped no-turn-ID frame per normal pair; repeated frames remain B10 unsupported drift. |
| KAS terminal conversion/routing entry | C1/C3/C6 cross converter → scoped route → mediator. |
| KAS fixed-pair liveness/current dedup | C1/C2 replace first-source dedup with response evidence and turn_end abort authority. |
| Notification backpressure substrate | C9 covers 256+1. |
| Active-turn guard | Sweep plus C1/C3 replace session-presence semantics. |
| Completion release guard | C1/C2/C3 require owner/source classification. |
| Global v1/v2 path | C2 preserves synthesized global completion. |
| Scoped KAS path | C1/C2/C3 cover owned, foreign, and no-active scope. |
| App foreign-session boundary | C3/C7 preserve early-return routing. |
| Same-session stale ownership | Dispatch-stamped v1/v2/lifecycle events compare owner; normal KAS leaves no post-turn task/source. |
| Cross-session ownership | C3/C7. |
| KAS distinct-source ownership | C1/C6 cover one turn_end plus one optional result; repeated scoped frames are B10. |
| KAS prompt-response-only authority | C1/C2 require zero completion and Busy persistence. |
| Shutdown/process lifetime | C4 requires at most active task and fresh-domain isolation. |
| Prompt error/process death | C5 makes KAS failure fail-stop before B. |
| Cancellation | C4/C5 preserve target; C6 preserves pre-turn_end evidence without precedence. |
| Rate-limit consumer | C10 defines zero release; verified issue cyril-3zy4 owns payload/render/retry semantics. |
| Ownership identity mechanism | C1–C5 define dispatch owner; C8 covers allocator. |
| Identity exhaustion | C8. |
| 257-notification backlog | C9. |

## Oracle blindness ledger

Every signed `This method cannot see:` sentence and prototype stand-in/normalization boundary is carried below.

| ID | Erased/unseen difference | Disposition |
| --- | --- | --- |
| B1 | Scripted same-session tests cannot see unrepresented scheduler interleavings or undocumented live frames. | Named risk; C1/C2/C5 fence deterministic race orders. |
| B2 | Routing tests cannot see consumers bypassing `RoutedNotification` or mutating Busy outside the tested pipeline. | Named risk; C7 pairs runtime deltas with current-consumer source review. |
| B3 | Evidence tests cannot decide KAS reason precedence when both sources were observed before turn_end. | Verified issue cyril-pnwb owns authority; C6 preserves observed tuples only. |
| B4 | Lifecycle fixtures cannot see OS-specific death timing or an uncaptured live cancellation reason. | Named risk; C5 uses deterministic EOF/error schedules and claims ordering, not timing coverage. |
| B5 | Rate-limit tests cannot see live payload, rendering, retry timing, or cyril-3zy4's fail-stop mapping. | Verified issue cyril-3zy4; C10 asserts observation-alone release count only. |
| B6 | Counter injection cannot see a natural `2^64` lifetime or memory corruption. | Named risk; C8 covers exact injected boundaries. |
| B7 | The 257 fixture cannot see an indefinitely wedged consumer, unbounded producer memory, or scheduling beyond bounded channels. | Named risk; C9 conditions delivery on a live resumed receiver. |
| B8 | Workspace tests cannot see live wire drift, production scheduling, or behavior excluded from tests. | Named risk; captured fixtures supplement but do not erase B10. |
| B9 | Sanitizing session IDs erases original opaque values. | Same/foreign equality fixtures cover routing; undocumented encoding meaning is accepted risk. |
| B10 | Captures and mocks cannot prove every KAS version obeys the signed one-turn_end pair; repeated scoped frames carry no identity. | Named unsupported live-wire drift. This design explicitly claims no protection for it. |
| B11 | Response-only runtime output cannot prove response-before-turn_end evidence retention, task abortion, or queued-result discard. | C1/C4/C6 use separate task/evidence sentinels. |
| B12 | Runtime prototype does not observe allocator exhaustion, rate-limit consumer, or 257 backlog. | C8–C10 are separate falsifiers. |
| B13 | Lexical inventory observes spellings, not normalization, ordering, forwarding, task isolation, or state effects. | It proves no runtime claim; executable fences are required. |
| B14 | Named-counter regex can miss differently spelled allocator state. | C8 tests behavior, not tokens. |
| B15 | Hidden owner labels are unavailable to production. | Deliberate independent oracle; production creates owner at dispatch and does not infer one from result content. |
| B16 | Deterministic fixtures erase unscripted timing and wire drift. | B1/B8/B10 name the erased classes; no scheduler-completeness claim is made. |
| B17 | Node ACP mock is transport-compatible but not the live KAS service. | Real captures pin known shape; C5 fail-stop is a design contract pending implementation fence. |
| B18 | Source inventory establishes declarations but not runtime normalization, ordering, forwarding, cache/task isolation, or effects. | Same disposition as B13. |
| B19 | Prototype's pre-existing `unused_mut` warning is unrelated and was not corrected. | Unrelated baseline; no production code changes here. |

## Negative space

1. Repeated scoped KAS `turn_end` frames are unsupported live-wire drift; no identity or safety claim is invented for them.
2. A KAS prompt result not observed before authoritative turn_end is discarded, not retained for cyril-pnwb.
3. No FIFO guess, timeout, unbounded task registry, or completed-turn history supplies ownership.
4. No production code or build plan is included in this design correction.
5. Stop-reason precedence is not selected; verified issue **cyril-pnwb** owns it.
6. Rate-limit payload/render/retry behavior is excluded under verified issue **cyril-3zy4**.
7. Stream/completion ordering is excluded under verified issue **cyril-9akh**.
8. Reconnect/respawn UX is excluded under verified issue **cyril-gua0**.
9. Enter-while-Busy steering and terminal-child reap policy are unchanged; closed issues **cyril-2vcc** and **cyril-3lh8** record them.

## Tracker audit

Repository-local `.rivets/issues.jsonl` references were verified for existence and coverage:

- **cyril-a71q** — open target; turn ownership, stale/cross-session hazards, global v1/v2 compatibility, and KAS source seam.
- **cyril-j16p** — closed; one KAS turn_end plus one prompt-result liveness/current dedup substrate.
- **cyril-pnwb** — open; reason disagreement and authority when both sources were observed.
- **cyril-3zy4** — open; live rate-limit payload/conversion/render/retry and lifecycle interaction.
- **cyril-l7tw** — closed; failure visibility and ordered disconnect delivery.
- **cyril-9akh** — open; streamed notifications trailing completion.
- **cyril-gua0** — open; respawn/reconnect and fresh-session UX.
- **cyril-3lh8** — closed; cancellation terminal-child reaping.
- **cyril-2vcc** — closed; Busy/input steering behavior.

No phantom reference or uncited technical-debt promise is present.

## Self-review

- **Claim count:** 10, within 3–15.
- **Input coverage:** both engines; all option cells; collection/string/reason/scope states; cancellation/error/death/shutdown; evidence cutoff; exhaustion; rate limit; and 256+1 map to claims.
- **Fixed-pair boundary:** normal KAS input is exactly two distinct optional sources, each with maximum cardinality one. Repeated scoped frames are blindness/negative space, not production coverage.
- **Removed-invariant coverage:** task replacement, active guard, late global response, scoped owned/foreign turn_end, pending disconnect, cancellation, App/session/UI effects, evidence, exhaustion, rate-limit, and backpressure are swept.
- **Prompt-error counterexample:** pre-turn_end KAS prompt failure is fail-stop and exits before B; post-turn_end result is discarded under choice A. No signed path releases A yet leaves the bridge able to deliver A's normal optional turn_end during B.
- **Independence/non-vacuity:** every row names an external oracle and a concrete mutation; C1 and C5 mutations were exercised by the persisted model.
- **Distinctness:** every claim has a unique `C#-*` output.
- **Cost:** all proposed fences are deterministic and at most four minutes.
- **Material boundaries:** all 20 prototype rows are accounted for.
- **Blindness:** all eight signed `cannot see` statements and prototype stand-in/normalization limits are named.
- **Negative space:** nine explicit exclusions with verified issue IDs where another issue owns behavior.
- **Cheapest falsifier:** executed and passed the exact requester-choice-A sequence; superseded duplicate/lower-bound outputs are absent.
- **Gate result:** passed and ready for required review. No approval, production change, or plan is claimed.

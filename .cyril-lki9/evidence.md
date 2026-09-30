# Evidence: cyril-lki9

## Premise checklist
| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | Wake-turn frames are identifiable on the wire | Which `session/update` kinds in an agent-initiated turn carry `_meta.kiro.agentInitiated` / `agentInitiatedReason`, and do `turn_start` / `turn_end` carry it? | PASS (learning) |
| P2 | Cyril's pinned ACP crate delivers that `_meta` to the converter | Does `agent-client-protocol =2.0.0` (schema 1.5.0, `schema::v1`) deserialize and retain `_meta` on `AgentMessageChunk`, `ToolCall`, `ToolCallUpdate`? | PASS |
| P3 | Current bridge drops the wake turn's terminal | With no cyril-owned active turn, does the current bridge forward a `TurnCompleted` for a wake turn's `turn_end`, or drop it (`DropUnowned`)? | PASS (dropped) |
| P4 | Cancel works on a wake turn | Does `session/cancel` during a wake turn end it with `turn_end`, and with what stopReason? | PASS |
| P5 | Operator input during a wake turn | During a wake turn, what does KAS do with `_session/steer`, and with `session/prompt`? | PASS (learning) |
| P6 | Busy-parent wake is detectable | When a run ends while the parent is mid-turn (steering injection), does any wire frame mark it? | PASS (learning: yes, and it carries the wake text) |
| P7 | Correlation order for naming | Does the triggering `run_complete` arrive before the wake turn's first tagged frame? | PASS |
| P8 | Wake label claim timing (added 2026-09-30 for the approved claim-at-turn-start rule) | Does a wake's triggering `run_complete` arrive before the wake's `turn_start` on its session? | PASS |
| — | Header text, parity, FIFO naming | Behavior decided in `spec.md` | N/A — spec decision, not a system claim |
| — | Stall threshold behavior for wake turns | Watchdog design | N/A — design/checkpoint territory |

## Data
- Source: committed live captures (production-shaped: real kiro-cli 2.26.0 / KAS 0.66.15 wire traffic, temp HOME + workspace, auth redacted).
  - `experiments/conductor-spike/kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl` (cyril-style gate-off run + wake; commit `bfc498b1`)
  - `experiments/conductor-spike/kas-workflow-new-06615-gateon-2.26.0.jsonl` (gate-on authoring, wake chain; commit `bfc498b1`)
- Shape: raw JSON-RPC frames both directions, `{ts, dir, msg}` JSONL.
- Safety: read-only analysis of committed files; live legs (P4–P6) use temp HOME + temp workspace, real XDG_DATA_HOME, token read-only from the auth DB (no renewal).

## Probe
- File: `probe_p1_p7.py`
- Mechanism: Python parse, per-session turn segmentation by `turn_start`/`turn_end`, client/server origin by preceding `session/prompt`, counts of tagged/untagged update kinds per turn, and `run_complete` timestamp vs first tagged frame.
- Run: `cd experiments/conductor-spike && python3 <worktree>/.cyril-lki9/probe_p1_p7.py kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl kas-workflow-new-06615-gateon-2.26.0.jsonl`

## Probe (P2)
- File: `probe_p2/` (standalone cargo crate; `Cargo.lock` copied from the worktree to pin `agent-client-protocol 2.0.0` / `agent-client-protocol-schema 1.5.0`).
- Mechanism: deserialize every raw-tagged `session/update` `params` from both captures into `agent_client_protocol::schema::v1::SessionNotification` (the exact path `convert/mod.rs:1` imports as `acp`) and check `meta.kiro.agentInitiated == true` and `agentInitiatedReason` present on the typed value.
- Run: `cp <worktree>/Cargo.lock .cyril-lki9/probe_p2/ && cd .cyril-lki9/probe_p2 && env -u CARGO_TARGET_DIR cargo run -q --offline -- <capture.jsonl>` (run from a scratch copy; the committed probe omits the lockfile copy).

## Oracle
- Mechanism (P1, P7): line-oriented `grep`/`uniq -c` over the raw JSONL. No JSON parsing, no turn or session segmentation; counts every line containing the tag by `sessionUpdate`, the tagged `turn_(start|end)` lines, reason strings, and the line numbers of the first `run_complete` vs the first tagged line. Different failure mechanism: it cannot mis-segment turns or mis-scope sessions (the bug the probe actually had).
- Run: the `grep` pipeline recorded in the session transcript, per file:
  `grep '"agentInitiated": true' F | grep -oE '"sessionUpdate": "[a-z_]+"' | sort | uniq -c`;
  `grep -E '"kind": "turn_(start|end)"' F | grep -c agentInitiated`;
  `grep -oE '"agentInitiatedReason": "[^"]+"' F | sort | uniq -c`;
  `grep -n '_kiro/workflow/run_complete' F | head -1` vs `grep -n '"agentInitiated": true' F | head -1`.

## Probe (P3)
- File: `probe_p3.rs` (throwaway `#[tokio::test]`; copied into `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/probe_lki9.rs` with a `mod probe_lki9;` line for the run only, then both removed; `git status` afterwards showed only `.cyril-lki9/` untracked).
- Mechanism: real bridge `run_loop` + `TurnMediator` via the FakeAgent harness; `start_session` then, with NO `SendPrompt`, inject through `InboundProbe` (the post-conversion path live frames take): scoped(main, streaming `AgentMessage("WAKE-CHUNK")`), scoped(main, unstamped `TurnCompleted{EndTurn}` = the wake's wire `turn_end`), scoped(main, `AgentMessage("SENTINEL")`); record every notification the App receiver gets until SENTINEL.
- Run: `env -u CARGO_TARGET_DIR cargo test -q -p cyril-core --lib probe_lki9 -- --nocapture` at worktree HEAD `bfc498b1` (+ the temporary probe file), 2026-09-29.

## Probe (P4–P6)
- File: `probe_live.py` (standalone Python raw JSON-RPC; harness infra copied from `experiments/conductor-spike/probe-kas-workflow-channels-2.26.0.py`). Gate OFF, cyril's launch shape (`new{parentSessionId: main}` → `invoke`), one-step recipe `lki9-quick` (`wf-coder`: "Reply with exactly the single word OK").
- Legs: `cancel` (at first tagged wake frame: `session/cancel`), `steer` (`_session/steer {sessionId, message}` = cyril's `session/steer` ext path, `subagents.rs:115`), `prompt` (`session/prompt`, what cyril sends today while it believes it is idle), `busy` (long no-tool operator turn first, then `new`+`invoke`).
- Run: `cd .cyril-lki9 && LEG=<leg> python3 probe_live.py` — kiro-cli 2.26.0 / KAS 0.66.15, 2026-09-29, legs serial. Traces: `lki9-live-<leg>-06615-2.26.0.{jsonl,-verdict.json,-stderr.log}`.
- Probe bug found and fixed: the first `busy` run used a `sleep 30` shell turn; the harness answers `terminal/wait_for_exit` synchronously, which blocked the probe loop, so the run launched only after the operator turn ended (never busy). Kept as `lki9-live-busy-sleepblocked-*`; re-run with a no-tool essay turn.

## Oracle (P4–P6)
- Mechanism: line-oriented `grep` over each raw trace (no JSON parsing or turn assembly): `turn_end` stopReason strings, `turn_start` count, `steering_*` kind counts, `notify-*` messageId prefixes, `agentInitiated` line count, `"queued": true` count.

## Oracle (P2)
- Mechanism: read the struct definitions in the registry source `agent-client-protocol-schema-1.5.0/src/v1/{client.rs,tool_call.rs}`: `ContentChunk`, `ToolCall`, `ToolCallUpdate` each declare `#[serde(rename = "_meta")] pub meta: Option<Meta>` with `Meta = serde_json::Map<String, Value>` (`v1/ext.rs:15`); `SessionUpdate::{AgentMessageChunk(ContentChunk), ToolCall(ToolCall), ToolCallUpdate(ToolCallUpdate)}` (`v1/client.rs:103-109`). Static source reading vs the probe's runtime deserialization. Initial oracle read the **v2** `ContentChunk` by mistake (the crate ships both); corrected to v1 after checking cyril's import. The v1 and v2 definitions agree on `_meta`.

## Oracle (P3)
- Mechanism: static reading of `crates/cyril-core/src/protocol/turn_mediator.rs:317-325` (at `bfc498b1`): a terminal with no active turn and nothing owed takes `None => { debug!("dropping unowned terminal — no active turn, nothing owed"); Disposition::DropUnowned }`; combined with the raw wire (P1 probe/oracle): every captured wake turn ends with a `turn_end` on a session with no client prompt in flight. Code reading + wire facts vs the probe's execution of the compiled loop.

## Comparisons
| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 (tail) | tagged: chunk 57, tool_call 2, update 4; turn_start/end tagged: no; reasons {workflow-complete-wake} | chunk 57, tool_call 2, update 4; tagged turn_(start\|end) lines 0; reason ×63 | PASS |
| P1 (gate-on, first run) | main-only: chunk 187, tool_call 6, update 8; reasons {workflow-complete-wake} | chunk **292**, tool_call 6, update 8; reasons workflow-complete-wake ×201, **send-message-wake ×105** | disagreement → investigated (below) |
| P1 (gate-on, fixed probe) | main 187/6/8 (workflow-complete-wake) + step `f8edf23e` 105 chunks (send-message-wake) = 292/6/8 | 292/6/8; 201 + 105 | PASS |
| P2 | tail 57/57, 2/2, 4/4; gate-on 292/292, 6/6, 8/8 (raw-tagged / retained on the typed value) | v1 structs declare `_meta: Option<Map>` on all three; nothing strips unknown keys inside a Map | PASS |
| P3 | App received `[AgentMessage(WAKE-CHUNK), AgentMessage(SENTINEL)]`: no `TurnCompleted` | mediator returns `DropUnowned` for an unowned terminal; the wire always carries the wake's `turn_end` | PASS (premise holds: the terminal is dropped) |
| P4 | wake `turn_start` 3.29 s → 1 tagged chunk → cancel sent 4.61 s → `turn_end{stopReason: cancelled}` 4.62 s + `steering_cleared`; no further main turn in 20 s | stopReasons: step `end_turn`, wake `cancelled`; turn_start ×2; tagged ×1 | PASS |
| P5 steer | `_session/steer` → `{queued: true, messageId: "steer-…"}`; `steering_queued` → `steering_injected` → wake `turn_end{end_turn}`; reply contains BANANA + KAS `[STEERING steer-…: …]` ack | `queued` ×1; steering_queued/injected/cleared ×1 each; stopReasons all `end_turn` | PASS |
| P5 prompt | wake `turn_start` 1.62 s, then 60 s with no chunk/tool/context frames; `session/prompt` at 61.79 s → wake `turn_end{cancelled}` 61.80 s → new `turn_start` 61.82 s → "PINEAPPLE" → `end_turn`; prompt response `{stopReason: end_turn}` | turn_start ×3 (step, wake, prompt); stopReasons `end_turn`, `cancelled`, `end_turn`; tagged 0 | PASS |
| P6 | run completed at ~35.9 s inside a 38 s operator turn; no separate wake turn; `steering_injected{content: "A workflow you launched (\"lki9-quick\") completed. Review its results…", messageId: "notify-wf-<uuid>", notificationSeverity: "info"}` then `steering_cleared` naming it; 0 tagged frames | turn_start ×2 (step, operator); `notify-wf-` ×1; steering_injected/cleared ×1; tagged 0 | PASS |
| P7 | tail: true; gate-on main wake: true | first run_complete line < first tagged line: 156 < 165; 318 < 324 | PASS |

Disagreement cause (P1 gate-on): **the probe was wrong**. It scoped turns to the main session only. The fixed probe segments every session. No oracle or recorded output was changed.

## Validated / learned
- P1: **learning**. Believed: tagging identifies "the wake turn on the main session". Observed:
  1. Every `agent_message_chunk`, `tool_call` and `tool_call_update` of an agent-initiated turn carries `_meta.kiro.agentInitiated: true` + `agentInitiatedReason`; `turn_start`/`turn_end` never do. The first tagged frame, not `turn_start`, is where origin becomes known.
  2. Agent-initiated turns occur on **step sessions too**. In the gate-on capture the parent's `send_message` to an already-finished creator step woke that step (`agentInitiatedReason: "send-message-wake"`, `rewakeWithInfo`) for an 8 s billed turn. Wakes can chain (main wake → step wake).
  3. **Server-started ≠ agent-initiated.** Ordinary workflow step turns are started by the engine with no client prompt but carry **no** tag. Origin must be keyed on the tag, never on the absence of a client prompt.
- P2: validated prior understanding. The pinned crate retains the full `_meta.kiro` map on all three update kinds through cyril's exact type path; the converter can read `agentInitiated`/`agentInitiatedReason` from the typed notification with no raw-JSON side channel.
- P3: validated prior understanding, and it names a **live defect in shipped cyril**: after any cyril-launched run, the wake turn's streaming chunks reach `UiState` (activity → `Streaming`) but its `turn_end` is dropped at the mediator, so no `TurnCompleted` ever flushes the streamed text or returns activity to idle for that turn. In scope for this change (spec B2: exactly one `TurnCompleted` per wake turn).
- P4: validated prior understanding. `session/cancel` ends a wake turn in ~10 ms with `turn_end{cancelled}`; KAS does not re-wake afterwards (20 s tail).
- P5: **learning**. (a) `_session/steer` during a wake is queued and honored exactly as in an operator turn. (b) A plain `session/prompt` during any server-started turn **pre-empts** it: the wake ends `cancelled` and the prompt's turn starts 20 ms later. Cyril today believes it is idle during a wake (P3), so an operator prompt typed during a wake silently cancels it. (c) A wake turn can stay **silent ≥ 60 s** after `turn_start` (no chunks, no tagged frames); the tag cannot be the busy signal. On the main session a `turn_start` with no cyril prompt in flight must count as busy by itself.
- P6: **learning** (the spec assumed "not detectable" and "wake text not on the wire"; both false). A run that ends while the parent is mid-turn produces **no wake turn**: the wake text is injected into the running turn as `steering_injected{content: <full wake text>, messageId: "notify-wf-<uuid>", notificationSeverity}`. Separately, step `send_message` verdicts reach the parent's model context the same way (`messageId: "notify-<uuid>"`, `content: "[notification/<severity>] <message>"`), in idle wakes (cancel leg, tail capture) and presumably in operator turns. Cyril converts these to `Notification::SteeringConsumed{content, message_id}` (`convert/kas.rs:416`), and `UiState::flip_consumed_steer_echo` (`state.rs`) falls back to flipping the OLDEST queued steer chip for an unknown id: a `notify-*` injection while an operator steer is queued would wrongly mark that steer Applied (static read; see Filed).
- Observation (not a premise): in a fresh session the model answers the wake with "I don't have any record of a workflow I launched in this session…" (steer leg, busy leg follow-up). The KAS wake text assumes the model launched the run; under ADR-0011 it did not.
- P8 (2026-09-30, after the isolated conformance review found stale FIFO labels): validated. In all five captured wakes (tail, gate-on, cancel, prompt, steer) the triggering `run_complete` precedes the wake's `turn_start` (probe: per-trace event order of `run_complete` / main `turn_start` / client prompts; oracle: `grep -n` line of the first `run_complete` < the next agent->client `turn_start` line). The busy trace shows the other branch: `run_complete` arrives INSIDE an already-open operator turn (unclaimed → the `notify-wf` injection names it). Static: KAS's `autoWakeParentOnComplete` runs after the event emit.
- P7: validated prior understanding. The triggering `run_complete` precedes the wake's first tagged frame in both captured wakes, so FIFO correlation at first-tagged-frame time has its data available.

## Related issues
- Consulted (from `spec.md`): cyril-lki9, ADR-0011, ADR-0004, cyril-5gb3, cyril-queu, cyril-0asq, cyril-fb1m, cyril-otpi, cyril-zd8u, cyril-99ds.
- Filed: cyril-5n75 (P6: notify-* injections flip the operator's oldest queued steer chip via the FIFO fallback).

# Review decisions: cyril-9104

Independent review (rust-code-reviewer subagent) of `d7158994`'s predecessor state,
2026-10-18. Every finding has one evidence state and decision.

| # | Finding | Evidence | Decision |
|---|---|---|---|
| H1 | `cargo clippy -p cyril-core --no-default-features` dead code: kas-only capture-replay helpers | gate lane `clippy-core-nodef-FAIL` (same finding, found independently) | Fixed in `d7158994` (helpers `#[cfg(feature = "kas")]`) |
| M1 | A second `/answer`, or a user answer after the parent's, pre-empts the step's resumed turn: the step stays listed until KAS re-emits `node_start` | Real: wfplan 165 (parent answer) → 167 (resume) gap; same for the user's own answer | Fixed: `WorkflowTracker::mark_answered` closes the park episode; `/answer` returns `CommandResultKind::StepAnswered` and the App marks it; the parent-answer branch of `record_message` marks it. Tests: `answered_episode_leaves_the_list_until_the_step_parks_again`, wfplan replay asserts the list is empty after line 166, App e2e sends a second `/answer` and asserts nothing is sent, `a_sent_answer_reports_its_step_session`. |
| M2 | A snapshot without `attention` wipes the run's list | Not a defect on 0.66.43: KAS deletes `attention` from run state when the list is empty (`Sce`: `t.length>0?e.attention=t:delete e.attention`); lifecycle events always carry the array (`lq`: `attention:HJ(state)`). Gate-off line 449 is a repeat-cap pause with nothing waiting. | Kept; documented at the parse site (`convert/kas/workflow.rs`, WireSnapshot conversion). |
| L1 | One node id parked on two paths of one run is unanswerable | Real (parallel branches / included recipes) | Fixed: selectors fall back node id → `wf/node` → full node path; `/answer` accepts any of the forms and reports "several match" instead of "not waiting". Test `same_node_on_two_paths_is_answerable_by_path`. |
| L2 | Blank `send_message` note hides the stream fallback | Real | Fixed: each source trimmed/filtered before falling back; case `blank_note_falls_back_to_the_reply` (red before the fix). |
| L3 | A resume `node_start` without `sessionId` leaves a stale note | Real (audit hazard: resume path may skip the re-emit) | Fixed: the note is cleared through the node's own session after the event merges. Test `sessionless_restart_clears_the_question_note`. |
| L4 | The answer task is detached | Real | Fixed: tracked in `command_tasks` (aborted on shutdown). |
| L5 | `not_waiting` state lookup takes any run in map order, misses qualified selectors | Real | Fixed: matches node id / `wf/node` / path; prefers a non-terminal run. |
| L6 | Top-level `run_complete.attention` converted then dropped | Real but harmless (equal to `finalState.attention` in every capture) | Kept the model; `apply_event` now warns if the two differ. |

## Review round 2 (coordinator's cross-model review of PR #182: APPROVE-WITH-NITS)

| # | Finding | Evidence | Decision |
|---|---|---|---|
| R2-1 (Medium) | A failed answer hides a step that is still parked: `StepAnswered` marks the episode answered on dispatch, and a failed `session/prompt` (step session gone, -32603, timeout) never unmarks it | Real | Fixed. The bridge reports a failed step answer as `Notification::WorkflowStepAnswerFailed { session_id, message }` (instead of a bare `BridgeError`); the App calls `WorkflowTracker::reopen_answered(session)` and the failure renders as before (`answer workflow step failed: …`). The immediate mark on dispatch stays, so a second `/answer` still cannot pre-empt the turn the first started. Red-first: `receipts/red-review-round2.txt`. Tests: tracker `failed_answer_reopens_the_episode`, bridge `failed_step_answer_names_its_step_session`, App `failed_answer_reopens_the_parked_step` (dispatch → failure → listed again → `/answer` sent). |
| R2-2 (Low) | The L6 warn fires on every ordinary completion: KAS sends top-level `attention: []` but omits an empty `finalState.attention` (11 of 12 gate-off `run_complete`s) | Real | Fixed: absent is treated as empty on both sides before comparing. Test `completion_attention_disagreement_warns_only_for_real` (empty-vs-omitted red before the fix; a real disagreement still warns). |
| R2-3 (Low) | The `<wf>/<node>` selector tier was untested (a mutation that never picks it survived) | Real | Test `nested_shared_node_ids_get_the_run_qualified_tier` (nested node ids shared across runs → `wf_a/ask`; shared within one run → full path). Mutation receipt in `receipts/red-review-round2.txt`. |
| R2-nit | `/answer @alice …` — an answer text that starts with `@` is read as a selector | Noted, not changed (coordinator: leave it). The answer is refused with "No workflow step "alice" is waiting", nothing is sent, and the user can re-send without the leading `@`. |

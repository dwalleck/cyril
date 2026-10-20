# Route: cyril-9104

Change: when a KAS workflow step asks a question, show it naming the run and
step, let the user answer it inline (`session/prompt` to the step session), and
make the parent model's answer visible inside its labelled agent-initiated turn
(ADR-0013 decision 3 and consequences).
Date: 2026-10-18. Stacked on cyril-2lh8 (PR #178, which contains cyril-6lql,
PR #177); `origin/main` merged in at the start.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| T1 | Empirical premise | Every wire fact the design uses is observed or read from the shipped engine. **Parked question:** `experiments/conductor-spike/audit-2.32.0/live/kas-workflow-gateoff-2.32.0.jsonl` (conn `main`) lines 986–988: `node_paused` / `paused` / `run_complete{status:"paused"}` each carry run-level `attention:[{kind:"question", since, nodeId, nodePath, sessionId}]`; the step's question is its own last agent text (`WAITING`, 978–979), with no `send_message`. **Inline answer:** line 994 sends `session/prompt` to the step session; KAS re-emits `run_start`/`node_start` with `attention:[]`, the step resumes and the run completes (995–1019). **Question via `send_message`:** `kas-lead-wfplan-2.32.0.jsonl` (conn `wfplan`, gate on): the step's `send_message{severity:"warning"}` → `_kiro/session/notify{sessionId:<parent>, callerSessionId:<step>, sender:"step", severity:"warning", workflowId, nodeId, agentName, message}` (110) → parent `turn_start` 5 ms later (111) tagged `send-message-wake` (113) → `node_paused{reason:"Step requested user input via send_message.", attention:[question]}` (157) → the parent's `Send Message` tool call (162) → `_kiro/session/notify{sessionId:<step>, callerSessionId:<parent>, sender:"parent", severity:"info", message}` (165), inside the wake, before its `turn_end` (201). **Shapes (KAS 0.66.43 bundle, `~/.local/share/kiro-cli/kas/2.32.0-…/acp-server.js`):** run attention entry = `$ir.extend({nodeId, nodePath, sessionId})` with `$ir = {kind: "permission"\|"question", since, toolCallId?, consentRound?}` (`Uir`); every lifecycle event and `run_complete` carries `attention: HJ(state)` (`lq`); a `question` without `toolCallId` is a parked step (`parkAwaitingUser` → `wSo`), with `toolCallId` it is a pending interaction (user_input) that arrives as `session/request_permission`. `_kiro/session/notify` = `vPt`: `{sessionId, callerSessionId?, message, severity, sender, workflowId?, nodeId?, agentName?}`; `sender` ∈ `Xir = ["parent","step"]`; `severity` ∈ `["info","success","warning","error"]`. KAS wakes an idle parent for a step's `warning`/`error` message (`_Su`) with the prompt "The X step is paused waiting for an answer to the question below…" (`mIo`). **Prompting a running session:** `.cyril-lki9/design.md` P5: `session/prompt` during a server-started turn pre-empts it (`turn_end{cancelled}`). The covenant doc (`docs/kiro-kas-acp-covenant.md`) predates both surfaces and does not list them; the shipped bundle is the reference (memory: KAS bundle = the reference impl). | no (resolved) |
| T2 | Structural module shape | New public surface: `Notification::WorkflowMessage`, `BridgeCommand::AnswerWorkflowStep`, domain types `WorkflowAttention` / `WorkflowMessage`, tracker API (`step_questions`, `take_raised_questions`, `record_message`), a new `/answer` command. No owner moves; the tracker stays the only consumer of `Notification::Workflow`/`WorkflowSnapshot`. Protected parents (lki9 C21): `app.rs` and `state.rs` production deltas stay within the fence. | yes |
| T3 | Production-scale risk | Per lifecycle frame: one attention list replaced on one run; per raise check a scan of current attention lists (a handful of entries). No new I/O beyond the user-issued prompt. | no |
| T4 | Explicit behavior | Outcomes come from the issue's acceptance criteria; the inline-answer UX choices were made here (design.md "Decisions", delegated) and are reported as open questions. | yes (with recorded delegated decisions) |

## Selected route

Structural (T2). No unresolved empirical premise; behavior is explicit, so the
next stage is `falsifiable-design` → `budgeted-plan` → `checkpointed-build`.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| design.md | falsifiable-design | `design.md` |
| plan.md | budgeted-plan | `plan.md` |
| spec.md | — | not needed: T4 behavior is the issue's acceptance plus delegated decisions recorded in design.md |
| evidence.md | — | not needed: T1 resolved from captures and the shipped bundle (cited above) |

## Terminal criterion

Structural: design claims all PASS with evidence, every ci.yml lane passes
locally and in CI on all platforms, the lki9 C21 shape oracle passes, one live
check of a parked question answered inline, and independent review.

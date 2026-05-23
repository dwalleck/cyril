# Workflow Engine Design

A programmatic workflow capability for cyril: define multi-stage state machines that drive an ACP agent through a sequence of prompts (e.g., write → test → review → fix → verify), observing turn outcomes, tool calls, and subagent results to decide transitions.

This document captures findings from an investigation prompted by [Pi's context-workflow extension](https://raw.githubusercontent.com/owainlewis/pi-extensions/refs/heads/main/extensions/context-workflow/context-workflow.ts) and the broader [Pi extension system](https://pi.dev/docs/latest/extensions). The conclusion: cyril can implement Pi's *programmatic workflow* pattern natively, and in some places (clean-context review via orchestrated subagent crews) the ACP wire model gives a strictly better primitive than Pi's in-process hooks.

This is design-only; no implementation exists yet. Should land as a numbered phase in [`ROADMAP.md`](ROADMAP.md) before any code work begins.

All wire-level claims in this document are verified against Kiro CLI 2.4.1 with explicit references to [`kiro-acp-protocol.md`](kiro-acp-protocol.md) sections.

## Motivation

Pi's `context-workflow.ts` is a 5-stage state machine that drives the agent through write → test → review → fix → verify → done. It works because Pi extensions run *inside* the Pi agent process and have access to:

- A hook bus (`pi.on("turn_end", ...)`) to observe each agent turn
- Custom LLM-callable tools (`pi.registerTool("workflow_test_result", ...)`) for structured signals from the agent
- `ctx.compact()` with custom instructions to clear context between stages
- `pi.sendUserMessage(..., {deliverAs: "followUp"})` to auto-progress

Cyril sits on the *client* side of ACP and cannot reach into the agent's internals. But it can observe and drive everything that crosses the wire — which, in Kiro 2.4.1, turns out to be most of what a workflow engine actually needs, plus capabilities Pi doesn't have direct equivalents for.

## Capability mapping (Kiro 2.4.1)

| Pi capability used by context-workflow | ACP/Kiro equivalent | Status |
|---|---|---|
| Hook on each turn end | `session/prompt` response with `stopReason`; `_kiro.dev/metadata` post-turn carries `meteringUsage`, `turnDurationMs`, `effort` ([§ 6, § 11.4](kiro-acp-protocol.md)) | ✅ verified |
| Send follow-up user message | `session/prompt` to main session | ✅ verified |
| `ctx.compact()` | `_kiro.dev/commands/execute` with `{"command":"compact","args":{}}`; `_kiro.dev/compaction/status` reports `started`/`completed`/`failed` ([§ 11.11](kiro-acp-protocol.md#11-empirical-wire-type-verification-241-captures)) | ✅ verified |
| Custom LLM-callable tools (`pi.registerTool`) | In-process MCP server over HTTP (`mcpCapabilities.http: true`); requires MCP server config in `~/.kiro/mcp.json` | ⚠ deferred — see Phase 5 |
| Block dangerous tool calls | `session/request_permission` with 4-option set + `_meta.trustOptions[]` for persistent allowlist patterns ([§ 11.4](kiro-acp-protocol.md)) | ✅ verified |
| Modify tool results | (none — agent owns execution) | ❌ structural limit |
| `pi.registerTool` system-prompt influence | (none — system prompt is agent-owned; cyril can only frame the *user* prompt) | ❌ structural limit |
| State persistence across reload | Plugin-owned file store (cyril writes to `~/.cyril/workflows/<id>.json` or session sidecar) | ✅ feasible |
| Fresh-context review stage | Two distinct subagent mechanisms — see next section | ✅ verified, **richer than Pi** |

## Two subagent mechanisms (the architectural decision)

Kiro 2.4.1 has **two distinct subagent spawning paths** with completely different semantics. The workflow engine needs to use each for what it's good at. See [§ 11.11 "Client-spawned vs agent-crew subagents"](kiro-acp-protocol.md#11-empirical-wire-type-verification-241-captures) for the full asymmetry table; the headline differences:

| Aspect | Agent-crew (`subagent` tool) | Client `/spawn` (`_session/spawn`) |
|---|---|---|
| Spawn primitive | Parent agent invokes its `subagent` tool with `stages[]` | Client sends `_session/spawn {task, name?}` |
| Role specialization | ✅ `stages[].role` → `.kiro/agents/<role>.json` | ❌ inherits parent mode (`kiro_default`) |
| Lifecycle | `working → terminated` (single-task, gone) | `working → terminated → awaitingInstruction` (persists for follow-ups) |
| Result delivery | `Summarizing` tool_call on parent's stream + inbox notification | **None** — output stays on subagent's own stream |
| Dependency-ordered stages | ✅ `stages[].depends_on` + `pendingStages[]` | ❌ no ordering primitive |
| Multi-turn dialogue | ❌ subagent dies after task | ✅ `_message/send` to the persisted subagent |

**For the workflow engine, this maps to two distinct `StageAction` variants**:

- **`DelegateToCrew`** — invoke an orchestrator agent that has the `subagent` tool, get role-specialized subagents with structured result delivery. The natural primitive for "spawn a clean-context reviewer, read its summary."
- **`SpawnSubagent`** — fire-and-monitor background worker. No result handoff; the runner has to consume the subagent's own session stream if it wants to know what the subagent said. Best for long-running investigations.

The previous design treated subagents as one mechanism with a `role` parameter; that mechanism doesn't exist on the wire. The split here reflects the empirically-verified reality.

### `DelegateToCrew` orchestration overview

```
Workflow runner            Cyril App                  Kiro CLI agent (orchestrator mode)
─────────────────          ─────────────              ──────────────────────────────────
StageAction::DelegateToCrew
{ orchestrator_agent, crew_prompt, on_complete }
        │
        ▼
1. commands/execute {agent, args: {value: "<orchestrator_agent>"}}
        │                                              ─→ switch mode; emit agent/switched
        ▼
2. session/prompt {crew_prompt}
        │                                              ─→ LLM decides to invoke `subagent` tool
        │                                                 with stages: [{name, role, prompt_template}…]
        │                                              ─→ Kiro spawns N subagents
        │                                              ←─ subagent/list_update (subagents working)
        │                                              ←─ session/update on each subagent's stream
        │                                              ─→ when each stage completes:
        │                                                 parent emits Summarizing tool_call on
        │                                                 main session with rawInput.taskResult
        ▼
3. Runner watches main session/update stream for tool_call { title: "Summarizing" }
        │
        ▼
4. on_subagent_complete(stage, SubagentResult) called per Summarizing
   OR on_crew_complete(stage, CrewResult) batched at join
```

Step 2's "LLM decides to invoke" is the one piece outside cyril's control — the orchestrator agent's prompt must be reliably triggering. Cyril ships orchestrator templates (see Phase 3) so users don't have to author this themselves.

## Design

### `Workflow` trait

```rust
// crates/cyril-core/src/workflow/mod.rs

pub trait Workflow: Send {
    fn name(&self) -> &str;
    fn initial_stage(&self) -> StageId;

    /// Called when entering a stage. Returns the action to take.
    fn enter_stage(&mut self, stage: StageId, ctx: &WorkflowCtx) -> StageAction;

    /// Called after each agent turn completes on the main session.
    fn on_turn_complete(
        &mut self,
        stage: StageId,
        outcome: &TurnOutcome,
    ) -> Transition;

    /// Called once per subagent completion (one Summarizing tool_call).
    /// Only relevant for stages using DelegateToCrew or SpawnSubagent.
    fn on_subagent_complete(
        &mut self,
        stage: StageId,
        result: &SubagentResult,
    ) -> Transition {
        Transition::Stay
    }

    /// Called when all subagents in a crew have terminated.
    /// Default: aggregates all per-subagent results and stays in stage.
    /// Override for join semantics (collect all, then transition).
    fn on_crew_complete(
        &mut self,
        stage: StageId,
        crew: &CrewResult,
    ) -> Transition {
        Transition::Stay
    }

    /// Called if a spawned subagent or crew fails to start, errors out,
    /// or terminates unexpectedly.
    fn on_subagent_error(
        &mut self,
        stage: StageId,
        error: &SubagentError,
    ) -> Transition {
        Transition::Failed(format!("subagent error: {error}"))
    }

    /// Optional: intercept permission requests during this workflow's
    /// session. Return Some to override; None falls through to the user.
    fn on_permission_request(
        &mut self,
        stage: StageId,
        req: &PermissionRequest,
    ) -> Option<PermissionDecision> {
        None
    }
}
```

### `StageAction` — what a stage does

```rust
pub enum StageAction {
    /// Send a user prompt to the main session.
    SendPrompt(String),

    /// Trigger /compact on the main session, then enter `then` once
    /// `_kiro.dev/compaction/status` reports `type: "completed"`.
    Compact { then: StageId, on_failure: Option<StageId> },

    /// Switch the main session to an orchestrator mode (an agent defined
    /// at `.kiro/agents/<orchestrator_agent>.json` that has the `subagent`
    /// tool in its allowed-tools), then prompt it with `crew_prompt`. The
    /// orchestrator's LLM invokes the `subagent` tool with stages[]; results
    /// arrive as `Summarizing` tool_calls on the main session, triggering
    /// `on_subagent_complete` per stage and eventually `on_crew_complete`.
    ///
    /// Provides: role specialization (via stages[].role), structured result
    /// delivery, dependency-ordered stages, parallel execution.
    DelegateToCrew {
        orchestrator_agent: String,
        crew_prompt: String,
        on_complete: StageId,
    },

    /// Spawn a single long-lived subagent via `_session/spawn`. The subagent
    /// inherits the parent's mode (no role specialization), persists in
    /// `awaitingInstruction` after task completion, and produces NO automatic
    /// result delivery — the runner subscribes to its `session/update` stream
    /// if it wants to know what the subagent said. Best for "fire and monitor"
    /// background workers. The workflow must explicitly TerminateSubagent on
    /// stage exit to avoid leaking persistent sessions.
    SpawnSubagent {
        task: String,
        name: Option<String>,        // omit to let Kiro auto-generate (e.g., "Lancelot")
        on_complete: StageId,
    },

    /// Send a follow-up message to a persistent subagent (one spawned via
    /// SpawnSubagent, in `awaitingInstruction` state). Wire: `_message/send`.
    MessageSubagent {
        session_id: SessionId,
        content: String,
    },

    /// Terminate a subagent (typically one previously spawned via
    /// SpawnSubagent). Wire: `_kiro.dev/session/terminate`. Best-effort.
    TerminateSubagent {
        session_id: SessionId,
    },

    /// Workflow finished successfully.
    Done,

    /// Workflow failed; reason surfaces as a system message in the chat.
    Failed(String),
}

pub enum Transition {
    /// Stay in the current stage (e.g., waiting for more turns or
    /// subagent completion).
    Stay,
    /// Move to another stage.
    Goto(StageId),
    /// Workflow finished.
    Done,
    /// Workflow failed.
    Failed(String),
}
```

### Result types

```rust
pub struct TurnOutcome<'a> {
    pub messages: &'a [Message],              // committed AgentText + ToolCalls
    pub tool_calls: &'a [TrackedToolCall],    // every tool call this turn
    pub stop_reason: StopReason,              // EndTurn | MaxTokens | Cancelled
    pub turn_duration_ms: Option<u64>,        // from kiro.dev/metadata
    pub turn_credits: Option<f64>,            // sum of meteringUsage[].value
    pub context_usage_pct: Option<f64>,
    pub effort: Option<String>,               // from metadata when on thinking model
}

impl<'a> TurnOutcome<'a> {
    pub fn last_bash(&self) -> Option<&TrackedToolCall> { /* … */ }
    pub fn last_tool(&self, name: &str) -> Option<&TrackedToolCall> { /* … */ }
    /// Reads bash tool's `rawOutput.items[0].Json.exit_status`. Replaces
    /// Pi's `workflow_test_result({exitCode})` LLM-callable tool with a
    /// deterministic wire-side read.
    pub fn bash_exit_code(&self) -> Option<i32> { /* … */ }
    pub fn agent_text(&self) -> String { /* concatenated AgentText */ }
}

pub struct SubagentResult {
    /// The subagent's sessionId.
    pub session_id: SessionId,
    /// The role from stages[].role (None for SpawnSubagent / client-spawned).
    pub role: Option<String>,
    /// The original task description (taskDescription from Summarizing rawInput,
    /// or the task argument from SpawnSubagent).
    pub task_description: String,
    /// The subagent's final response text.
    /// For DelegateToCrew: rawInput.taskResult from the Summarizing tool_call.
    /// For SpawnSubagent: reconstructed from the subagent's agent_message_chunk
    /// stream (less structured, possibly partial).
    pub final_message: String,
}

pub struct CrewResult {
    /// All per-subagent results in arrival order.
    pub subagents: Vec<SubagentResult>,
    /// The crew's orchestrator session id (the main session at delegation time).
    pub orchestrator_session: SessionId,
}

pub enum SubagentError {
    /// Subagent's session terminated before delivering a result.
    Terminated { session_id: SessionId, reason: Option<String> },
    /// Rate-limit hit per `_kiro.dev/error/rate_limit` on the subagent's
    /// session.
    RateLimited { session_id: SessionId, message: String },
    /// Spawn request itself failed.
    SpawnFailed { reason: String },
}
```

### `WorkflowRunner`

Owned by `App` alongside `SessionController` and `UiState`. Acts as a third notification reducer — consuming `RoutedNotification`s and producing `Vec<BridgeCommand>` for the App's event loop to dispatch (matches the pattern established by cyril's existing `handle_notification` after the `/rewind` work).

```rust
pub struct WorkflowRunner {
    active: Option<ActiveWorkflow>,
    registry: HashMap<String, Box<dyn WorkflowFactory>>,
}

struct ActiveWorkflow {
    workflow: Box<dyn Workflow>,
    current_stage: StageId,
    turn_accumulator: TurnAccumulator,         // builds TurnOutcome from notifications
    pending_compact: Option<CompactPending>,   // waiting on compaction/status: completed
    pending_crew: Option<CrewPending>,         // waiting on Summarizing tool_calls
    pending_spawn: HashMap<SessionId, SpawnPending>, // persistent subagents we own
}

impl WorkflowRunner {
    /// Called from App's notification fan-out, AFTER SessionController and
    /// UiState (the workflow runner observes but doesn't mutate UI state).
    pub fn apply_notification(&mut self, n: &RoutedNotification) -> Vec<BridgeCommand>;
}
```

State transitions inside the runner:

| Notification observed | Runner action |
|---|---|
| `session/update` (`agent_message_chunk`, `tool_call`, `tool_call_update`) on main session | accumulate into `TurnOutcome` draft |
| `TurnCompleted` on main session | finalize outcome, call `workflow.on_turn_complete()`, apply transition |
| `_kiro.dev/compaction/status` with `type: "completed"` | if `pending_compact.is_some()`, enter the `then` stage |
| `_kiro.dev/compaction/status` with `type: "failed"` | enter `on_failure` stage if specified, else `Transition::Failed` |
| `session/update` with `tool_call.title == "Summarizing"` on main session | parse `rawInput.{taskDescription, taskResult, __tool_use_purpose}`, build `SubagentResult`, call `on_subagent_complete` |
| `_kiro.dev/subagent/list_update` — crew member transitions to `terminated` | track in `pending_crew`; when all stages terminated, call `on_crew_complete` |
| `_kiro.dev/subagent/list_update` — spawn-subagent transitions to `awaitingInstruction` | mark `SpawnPending` as ready; runner can now `MessageSubagent` or `TerminateSubagent` |
| `_kiro.dev/error/rate_limit` on a subagent session | call `on_subagent_error(RateLimited{…})` |
| `session/update` (`retry_warning`) on a subagent session | optionally surface to UI; not a stage transition |

The runner returns the deferred `Vec<BridgeCommand>` that App fires through `bridge_sender.send().await` in the event loop.

### Slash commands

Three commands, registered alongside the existing `CommandRegistry`:

```
/workflow list                   # show registered workflows
/workflow run <name> [args...]   # start a workflow
/workflow status                 # show current stage and history
/workflow cancel                 # stop active workflow, terminate persistent subagents
/workflow init                   # copy orchestrator templates into .kiro/agents/
```

`/workflow run` parses free-form args after the workflow name — the workflow defines how to interpret them (typically as `spec: String`). `/workflow init` solves the orchestrator-template bootstrap problem (see Phase 3).

## Worked example: ContextWorkflow port

The 5-stage write → test → review → fix → verify workflow from Pi, expressed against cyril's primitives:

```rust
pub struct ContextWorkflow {
    spec: String,
    iteration: u32,
    max_iterations: u32,
    review_issues: Vec<String>,
    changed_files: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage { Write, Test, Review, Fix, Verify }

impl Workflow for ContextWorkflow {
    fn name(&self) -> &str { "context-workflow" }
    fn initial_stage(&self) -> StageId { Stage::Write.into() }

    fn enter_stage(&mut self, stage: StageId, _: &WorkflowCtx) -> StageAction {
        match stage.into() {
            Stage::Write => StageAction::SendPrompt(format!(
                "Implement the spec. When done, list the files you changed:\n\n{}",
                self.spec
            )),
            Stage::Test => StageAction::SendPrompt(
                "Run the test suite. Report exit code.".into()
            ),
            // Uses DelegateToCrew, not SpawnSubagent: clean-context review
            // requires role-specialized subagents + structured result delivery,
            // which only the agent-crew path provides.
            Stage::Review => StageAction::DelegateToCrew {
                orchestrator_agent: "review-orchestrator".into(),
                crew_prompt: format!(
                    "Coordinate a code review crew. Spawn a code-reviewer \
                     subagent. Have it review this implementation against \
                     the spec.\n\nSpec:\n{}\n\nFiles changed:\n{}",
                    self.spec, self.changed_files.join("\n")
                ),
                on_complete: Stage::Fix.into(),
            },
            Stage::Fix => StageAction::SendPrompt(format!(
                "Address these review issues:\n{}",
                self.review_issues.join("\n")
            )),
            Stage::Verify => StageAction::SendPrompt(
                "Re-run the test suite to verify everything still passes.".into()
            ),
        }
    }

    fn on_turn_complete(&mut self, stage: StageId, outcome: &TurnOutcome) -> Transition {
        match stage.into() {
            Stage::Write => Transition::Goto(Stage::Test.into()),
            Stage::Test => match outcome.bash_exit_code() {
                Some(0) => Transition::Goto(Stage::Review.into()),
                Some(_) => Transition::Goto(Stage::Fix.into()),
                None => Transition::Failed("test stage produced no bash exit code".into()),
            },
            Stage::Fix => Transition::Goto(Stage::Test.into()),
            Stage::Verify => match outcome.bash_exit_code() {
                Some(0) => Transition::Done,
                _ => Transition::Failed("verification failed".into()),
            },
            Stage::Review => Transition::Stay,  // wait on on_crew_complete
        }
    }

    fn on_crew_complete(&mut self, stage: StageId, crew: &CrewResult) -> Transition {
        if stage == Stage::Review.into() {
            // Aggregate findings from all reviewer subagents
            self.review_issues = crew.subagents
                .iter()
                .flat_map(|r| parse_issues(&r.final_message))
                .collect();
            if self.review_issues.is_empty() {
                Transition::Goto(Stage::Verify.into())
            } else {
                Transition::Goto(Stage::Fix.into())
            }
        } else {
            Transition::Stay
        }
    }
}
```

This requires `.kiro/agents/review-orchestrator.json` + a `.kiro/agents/code-reviewer.json` on disk. Cyril ships these as templates (see Phase 3).

## Implementation phases

Each phase is independently testable and produces observable behavior.

### Phase 1 — Trait + types (≈ 1 day)

Define `Workflow`, `StageAction`, `Transition`, `TurnOutcome`, `SubagentResult`, `CrewResult`, `SubagentError`, `PermissionDecision` in `crates/cyril-core/src/workflow/mod.rs`. Compile-only, no runtime. Lets us pressure-test the trait shape against the `ContextWorkflow` example without wiring anything.

Verification: ContextWorkflow compiles as a `Box<dyn Workflow>`; trait methods have all the inputs they need.

### Phase 2 — Runner without crew or spawn support (≈ 1 day)

Implement `WorkflowRunner` handling `SendPrompt`, `Compact { then, on_failure }`, and the turn-outcome accumulator. Wire into `App` as a third notification reducer. Returns `Vec<BridgeCommand>` matching the pattern established in `/rewind` work.

Ship one trivial built-in workflow ("explain then summarize" — two `SendPrompt` stages) so the path is exercised end-to-end. Slash command `/workflow run trivial` triggers it.

Verification: the trivial workflow runs to completion against a real Kiro session.

### Phase 3 — Slash commands + orchestrator templates (≈ 0.5 day)

Add `/workflow {list, run, status, cancel, init}`. Implement `/workflow init` to copy bundled orchestrator templates from cyril's repo into the user's `.kiro/agents/`. Initial templates:

- `review-orchestrator.json` — has `subagent` tool, prompt instructs it to spawn reviewer stages.
- `code-reviewer.json` — single reviewer role, used by review-orchestrator.

Verification: a fresh repo, `/workflow init` populates `.kiro/agents/`, the templates parse correctly and the orchestrator can be entered via `/agent`.

### Phase 4 — `DelegateToCrew` + crew-stage handling (≈ 2 days)

The substantive phase. Add `WorkflowRunner` handling for:

- Switching mode via `_kiro.dev/commands/execute {agent}` and waiting for `agent/switched` confirmation
- Sending the crew prompt and watching for `Summarizing` tool_calls
- Per-stage `on_subagent_complete` callbacks
- Joining via `on_crew_complete` when all subagents in the crew have terminated
- `_kiro.dev/error/rate_limit` and `retry_warning` propagation to `on_subagent_error`

Port `ContextWorkflow` as the first real built-in workflow. End-to-end run against a real spec.

Verification: ContextWorkflow drives a full write → test → review → fix → verify cycle against a small spec, with the review stage actually using a reviewer subagent.

### Phase 5 — `SpawnSubagent` + `MessageSubagent` + `TerminateSubagent` (≈ 1 day)

Add support for long-lived background workers via `_session/spawn`. The runner subscribes to the spawn-subagent's `session/update` stream, accumulates output, and exposes follow-up messaging via `MessageSubagent`.

Cleanup discipline: workflows that spawn must terminate; the runner enforces termination on workflow exit even if the workflow itself forgets.

Verification: a workflow that spawns a subagent, sends it a follow-up message, gets the new response, then terminates the subagent — observable in `subagent/list_update` transitions.

### Phase 6 — MCP-bridge for plugin-defined tools (deferred)

If we want plugin-defined LLM-callable tools later (Pi's `pi.registerTool` analog), embed an HTTP MCP server in cyril and document how Kiro config picks it up via `~/.kiro/mcp.json`. Most workflows won't need this once `DelegateToCrew` and `SpawnSubagent` are available.

### Phase 7 — External workflows (open)

Whether plugin-defined workflows ship as Rust dylibs, Wasm components, or an embedded scripting layer. Punt until Phases 1–5 confirm the trait API is right.

## What's still off-limits

Honest framing of what cyril CAN'T do at the ACP wire, even with the workflow engine:

- **System-prompt modification** — no ACP method exists. Pi's `before_agent_start` can prepend to the system prompt; cyril cannot. Workaround: prepend framing to user prompts per stage.
- **Mid-turn tool result modification** — `tool_call_update` arrives after the agent has executed. A future *proxy stage* between cyril and the agent could rewrite results, but the workflow engine running inside cyril cannot.
- **Custom compaction instructions** — `/compact` is a black box; we can trigger it and observe `compaction/status` but not steer the summary content. `DelegateToCrew` largely obviates this (clean context comes from a fresh subagent, not from compacting the main session), but if a workflow specifically needs "compact the main session with rules X," it can't.
- **Pre-execution blocking of non-permission-gated tools** — file reads aren't gated by `request_permission`. The workflow only votes on tools that go through that mechanism (currently shell + grep + web_fetch + workspace-boundary file reads).
- **Directly creating role-specialized subagents** — `_session/spawn` doesn't accept `role`. Role specialization only happens via the agent-crew path, which requires the orchestrator agent's LLM to invoke its `subagent` tool. Non-deterministic if the prompt isn't strong.

The residual gap versus Pi is much smaller than a naive client-vs-host comparison would suggest. Two specific places cyril is *better* than Pi: clean-context review (via `DelegateToCrew`) doesn't require compaction-workaround, and persistent background workers (via `SpawnSubagent`) have no in-process equivalent in Pi.

## Open questions

- **Per-stage vs batched crew callbacks.** Default: both `on_subagent_complete` (per Summarizing) AND `on_crew_complete` (after all stages terminate). Workflows can override whichever suits their join semantics. Sane default seems to be `on_crew_complete` overriding; `on_subagent_complete` for progressive aggregation. Need real workflows to validate.
- **Workflow state persistence.** Pi persists via `pi.appendEntry()`. Cyril should write to `~/.cyril/workflows/<workflow-id>-<session-id>.json` from Phase 4 — without persistence, a cyril restart kills any in-progress workflow. Format: serialize the workflow's `serde::Serialize` state + current stage + accumulated `SubagentResult`s.
- **Concurrency: one workflow at a time, or many?** Phase 1 trait assumes one active workflow. Multi-workflow execution adds bookkeeping for which `RoutedNotification` belongs to which workflow's tracker. Defer to Phase 4+.
- **Orchestrator template authoring.** Phase 3 ships `review-orchestrator.json` + `code-reviewer.json`. What other orchestrators ship out of the box? "PR-test-analysis," "doc-update," "refactor-impact-analysis"? Driven by user demand once Phase 4 lands.
- **User interaction during active workflow.** What if the user sends a normal chat message while a workflow is running? Interrupt? Queue? Cancel? The slash command `/workflow cancel` is the explicit cancel path; the implicit "user typed a message" case needs a decision.
- **Multi-iteration limits.** Pi has `maxIterations: 10` to prevent infinite loops. The trait should expose `Workflow::max_turns_per_stage()` or similar, with the runner enforcing a hard ceiling regardless. Buggy workflows shouldn't be able to burn unbounded credits.

## References

- [`docs/kiro-acp-protocol.md`](kiro-acp-protocol.md) — protocol reference; all wire claims in this doc cite specific sections
- [`docs/cyril-acp-coverage-vs-2.4.1.md`](cyril-acp-coverage-vs-2.4.1.md) — what's already implemented in cyril vs what this design depends on
- [`docs/ROADMAP.md`](ROADMAP.md) — phased direction for cyril; this work should land as a numbered phase
- `experiments/conductor-spike/trace-2.4.1-multi-subagent.jsonl` — canonical multi-subagent wire capture used to verify the spawn/crew asymmetries
- Pi context-workflow source: https://raw.githubusercontent.com/owainlewis/pi-extensions/refs/heads/main/extensions/context-workflow/context-workflow.ts
- Pi extension docs: https://pi.dev/docs/latest/extensions

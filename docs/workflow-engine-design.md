# Workflow Engine Design

A programmatic workflow capability for cyril: define multi-stage state machines that drive an ACP agent through a sequence of prompts (e.g., write → test → review → fix → verify), observing tool calls and turn outcomes to decide transitions.

This document captures findings from an investigation prompted by [Pi's context-workflow extension](https://raw.githubusercontent.com/owainlewis/pi-extensions/refs/heads/main/extensions/context-workflow/context-workflow.ts) and the broader [Pi extension system](https://pi.dev/docs/latest/extensions). The conclusion: cyril can implement Pi's *programmatic workflow* pattern natively, and in one important case (clean-context review stages) the ACP wire model gives a strictly better primitive than Pi's in-process model.

This is design-only; no implementation exists yet. Should land as a numbered phase in [`ROADMAP.md`](ROADMAP.md) before any code work begins.

## Motivation

Pi's `context-workflow.ts` is a 5-stage state machine that drives the agent through write → test → review → fix → verify → done. It works because Pi extensions run *inside* the Pi agent process and have access to:

- A hook bus (`pi.on("turn_end", ...)`) to observe each agent turn
- Custom LLM-callable tools (`pi.registerTool("workflow_test_result", ...)`) for structured signals from the agent
- `ctx.compact()` with custom instructions to clear context between stages
- `pi.sendUserMessage(..., {deliverAs: "followUp"})` to auto-progress

Cyril sits on the *client* side of ACP and cannot reach into the agent's internals. But it can observe and drive everything that crosses the wire — which, in Kiro v1.29.0, turns out to be most of what a workflow engine actually needs.

## Capability mapping (Kiro v1.29.0)

Verified against [`kiro-acp-protocol.md`](kiro-acp-protocol.md). Line references are to that document.

| Pi capability used by context-workflow | ACP/Kiro equivalent | Verdict |
|---|---|---|
| Hook on each turn end | `session/prompt` response carries `stopReason`; `kiro.dev/metadata` post-turn carries `meteringUsage` and `turnDurationMs` (lines 814–820) | ✅ |
| Send follow-up user message | `session/prompt` | ✅ |
| `ctx.compact()` | `kiro.dev/commands/execute` with `{"command":"compact","args":{}}` (lines 765–785); `kiro.dev/compaction/status` reports started/completed/failed (lines 891–908) | ✅ |
| Custom LLM-callable tools (`pi.registerTool`) | In-process MCP server over HTTP — `mcpCapabilities.http: true` in v1.29.0 (line 71) | ✅ (deferred — see Phase 5) |
| Block dangerous tool calls | `session/request_permission` (lines 425–465) for any permission-gated tool (currently shell) | ⚠ partial |
| Modify tool results | (none — agent owns execution) | ❌ |
| `pi.registerTool` system-prompt influence | (none — system prompt is agent-owned) | ❌ |
| State persistence across reload | Plugin-owned file store | ✅ |
| Fresh-context review stage | `session/spawn` (lines 231–253) — spawns a subagent with isolated context; lifecycle via `kiro.dev/subagent/list_update` (lines 963–1015) and result delivery via `kiro.dev/session/inbox_notification` (lines 1017–1031) | ✅ — *better than Pi* |

### The `session/spawn` insight

The single most important capability is one Pi doesn't have a direct equivalent for. Pi's `ctx.compact("strip implementation details, keep spec and file list")` is a *workaround* for "I want a model with clean context to do the review step." Cyril doesn't need that workaround — `session/spawn` creates a fresh subagent whose context is exactly what you put in the `task` field. The reviewer has never seen the implementation discussion. The result lands in `inbox_notification` as a structured event. The subagent has its own session ID for `session/update` routing.

This means the review stage of a context-workflow port is not "compact and pray" but "spawn `code-reviewer` mode with `{task: "Review this against the spec: ..."}` and read the inbox."

### Parallelism is free

`kiro.dev/subagent/list_update` is built around multiple concurrent subagents (lines 925–1015 describe the multi-stage subagent model). A workflow stage can fan out to several subagents — e.g., a code-reviewer and a pr-test-analyzer in parallel — and define a join condition. Pi's hook model is single-threaded by design.

## Design

### `Workflow` trait

```rust
// crates/cyril-core/src/workflow/mod.rs

pub trait Workflow: Send {
    fn name(&self) -> &str;
    fn initial_stage(&self) -> StageId;

    /// Called when entering a stage. Returns the action to take.
    fn enter_stage(&mut self, stage: StageId, ctx: &WorkflowCtx) -> StageAction;

    /// Called after each agent turn completes. Returns the transition to apply.
    fn on_turn_complete(
        &mut self,
        stage: StageId,
        outcome: &TurnOutcome,
    ) -> Transition;

    /// Optional: called when a spawned subagent posts results.
    /// Only relevant for stages that used StageAction::SpawnSubagent.
    fn on_subagent_complete(
        &mut self,
        stage: StageId,
        result: &SubagentResult,
    ) -> Transition {
        Transition::Stay
    }

    /// Optional: called on permission requests during this workflow's session.
    /// Returns Some to override; None to fall through to the user.
    fn on_permission_request(
        &mut self,
        stage: StageId,
        req: &PermissionRequest,
    ) -> Option<PermissionDecision> {
        None
    }
}
```

### Action and transition types

```rust
pub enum StageAction {
    /// Send a user prompt to the main session.
    SendPrompt(String),

    /// Trigger /compact on the main session, then enter `then` once
    /// kiro.dev/compaction/status reports completed.
    Compact { then: StageId },

    /// Spawn a subagent with a clean context. The workflow runner
    /// captures the subagent's message stream and inbox notification,
    /// then calls on_subagent_complete with the result.
    SpawnSubagent {
        agent: String,          // mode ID (e.g., "code-reviewer")
        task: String,           // initial query
        on_complete: StageId,   // default transition; on_subagent_complete may override
    },

    /// Workflow finished successfully.
    Done,

    /// Workflow failed; reason surfaces as a system message in the chat.
    Failed(String),
}

pub enum Transition {
    /// Stay in the current stage (e.g., waiting for more turns).
    Stay,
    /// Move to another stage.
    Goto(StageId),
    /// Workflow finished.
    Done,
    /// Workflow failed.
    Failed(String),
}
```

### `TurnOutcome` — what the workflow observes

The load-bearing helper is `bash_exit_code()` — this replaces Pi's `workflow_test_result(exitCode)` tool by reading the same number out of the bash tool's `ToolCallUpdate`.

```rust
pub struct TurnOutcome<'a> {
    pub messages: &'a [Message],              // committed AgentText + ToolCalls
    pub tool_calls: &'a [TrackedToolCall],    // every tool call this turn
    pub stop_reason: StopReason,              // EndTurn | MaxTokens | Cancelled
    pub turn_duration_ms: Option<u64>,        // from kiro.dev/metadata
    pub turn_credits: Option<f64>,            // from meteringUsage
    pub context_usage_pct: Option<f64>,
}

impl<'a> TurnOutcome<'a> {
    pub fn last_bash(&self) -> Option<&TrackedToolCall> { /* … */ }
    pub fn last_tool(&self, name: &str) -> Option<&TrackedToolCall> { /* … */ }
    pub fn bash_exit_code(&self) -> Option<i32> { /* … */ }
    pub fn agent_text(&self) -> String { /* concatenated AgentText */ }
}
```

### `WorkflowRunner`

Owned by `App` alongside `SessionController` and `UiState`. Acts as a third notification reducer:

```rust
pub struct WorkflowRunner {
    active: Option<ActiveWorkflow>,
    registry: HashMap<String, Box<dyn WorkflowFactory>>,
    bridge: BridgeSender,
}

struct ActiveWorkflow {
    workflow: Box<dyn Workflow>,
    current_stage: StageId,
    turn_accumulator: TurnAccumulator,    // builds TurnOutcome from notifications
    pending_subagent: Option<SessionId>,  // if waiting on a SpawnSubagent stage
    pending_compact: bool,                // if waiting on compaction/status
}

impl WorkflowRunner {
    /// Called from App's notification fan-out, after SessionController and UiState.
    pub fn apply_notification(&mut self, n: &RoutedNotification) -> bool;
}
```

State transitions inside the runner:

- `SessionUpdate(AgentMessageChunk | ToolCall | ToolCallUpdate)` for the main session → accumulate into `TurnOutcome` draft
- `TurnCompleted` for the main session → finalize outcome, call `workflow.on_turn_complete()`, apply transition
- `kiro.dev/compaction/status` with `type: "completed"` → if `pending_compact`, enter the `then` stage
- `kiro.dev/subagent/list_update` shows our pending subagent as terminated → fetch its captured stream, call `on_subagent_complete()`
- `kiro.dev/session/inbox_notification` for our pending subagent → also feeds `on_subagent_complete()`

The runner needs a `BridgeSender` clone to dispatch `BridgeCommand::SendPrompt`, `BridgeCommand::SpawnSession`, and `BridgeCommand::ExecuteCommand("compact", {})`.

### Slash commands

Three commands, registered alongside the existing `CommandRegistry`:

```
/workflow list                   # show registered workflows
/workflow run <name> [args...]   # start a workflow
/workflow status                 # show current stage and history
/workflow cancel                 # stop active workflow, clean up subagents
```

`/workflow run` parses free-form args after the workflow name — the workflow defines how to interpret them (typically as `spec: String`).

### Example: porting context-workflow.ts

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
                "Implement the following spec. When done, list the files you changed:\n\n{}",
                self.spec
            )),
            Stage::Test => StageAction::SendPrompt(
                "Run the test suite. Report exit code.".into()
            ),
            Stage::Review => StageAction::SpawnSubagent {
                agent: "code-reviewer".into(),
                task: format!(
                    "Review this implementation against the spec.\n\n\
                     Spec:\n{}\n\nFiles changed:\n{}",
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
            Stage::Review => Transition::Stay,  // wait on on_subagent_complete
        }
    }

    fn on_subagent_complete(
        &mut self,
        stage: StageId,
        result: &SubagentResult,
    ) -> Transition {
        if stage == Stage::Review.into() {
            self.review_issues = parse_issues(&result.final_message);
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

## Implementation phases

1. **Phase 1 — Types only.** Define `Workflow`, `StageAction`, `Transition`, `TurnOutcome`, `SubagentResult`, `PermissionDecision` in `crates/cyril-core/src/workflow/mod.rs`. Compile-only, no runtime. Lets us pressure-test the trait shape against the `ContextWorkflow` skeleton above without wiring anything.

2. **Phase 2 — Runner without subagent support.** Implement `WorkflowRunner` handling `SendPrompt`, `Compact`, and the turn-outcome accumulator. Wire into `App` as a third notification reducer. At this point a workflow that doesn't use `SpawnSubagent` works end-to-end.

3. **Phase 3 — Slash commands.** Add `/workflow run|list|status|cancel`. Register one trivial built-in workflow (e.g., "write code then run tests then summarize") to exercise the path.

4. **Phase 4 — Subagent stages.** Add `SpawnSubagent` handling: dispatch `BridgeCommand::SpawnSession`, track the returned session ID, capture its message stream from `RoutedNotification`s with that session ID, fire `on_subagent_complete` on terminal `subagent/list_update` or `inbox_notification`. Port `ContextWorkflow` as the first real built-in.

5. **Phase 5 (deferred) — MCP-bridge for plugin tools.** If we want plugin-defined LLM-callable tools later (Pi's `pi.registerTool` analog), embed an HTTP MCP server in cyril and document how Kiro config picks it up. Most workflows won't need this once `SpawnSubagent` is available.

6. **Phase 6 (open) — External workflows.** Whether plugin-defined workflows ship as Rust dylibs, Wasm components, or an embedded scripting layer. Punt until Phases 1–4 confirm the trait API is right.

Each phase is independently testable. Phases 1–3 are roughly a day each; Phase 4 is larger because of multi-session bookkeeping.

## What's still off-limits

To stay honest about the boundary:

- **System-prompt modification** — no ACP method exists for this. Pi's `before_agent_start` hook can prepend to the system prompt; cyril cannot. Workaround: prepend framing to the user prompt for each stage.
- **Mid-turn tool result modification** — `ToolCallUpdate` arrives after the agent has already executed. A future *proxy stage* between cyril and the agent could rewrite results, but the workflow engine running inside cyril cannot.
- **Custom compaction instructions** — `/compact` is a black box; we can trigger it and observe completion but not steer it. `session/spawn` mostly obviates this, but if a workflow specifically needs "compact the main session with rules X," it can't.
- **Pre-execution blocking of non-permission-gated tools** — file reads, for instance, are not gated by `request_permission`. The workflow only gets a vote on tools that go through that mechanism (currently shell).

The residual gap versus Pi is much smaller than a naive client-vs-host comparison would suggest.

## Open questions

- **MCP server transport.** v1.29.0 advertises `mcpCapabilities.http: true`. Does Kiro discover MCP servers from `.kiro/mcp.json`, runtime config, or both? Needed before Phase 5 can be scoped.
- **Subagent message capture vs. drill-in UI.** Cyril already has `SubagentUiState` capturing per-subagent streams (see `cyril-ui/src/subagent_ui.rs`). The workflow runner needs read access to that for `SubagentResult.final_message`. Decide whether to share state or duplicate capture.
- **Workflow state persistence.** Pi persists via `pi.appendEntry()` so workflows survive reload. Should cyril write to `~/.cyril/workflows/<workflow-id>.json` or attach to a session store?
- **Concurrency: one workflow at a time, or many?** Phase 1 trait assumes one active workflow. Multi-workflow execution is plausible but adds bookkeeping. Defer to Phase 4+.
- **Failure recovery.** If `session/spawn` fails or a subagent crashes mid-stage, what's the transition? Current design has no `on_subagent_error` hook. Likely need one.
- **`/compact` failure handling.** `kiro.dev/compaction/status` can report `type: "failed"`. Should `StageAction::Compact` carry an `on_failure: StageId` field, or always fall through to the same `then`?

## References

- [`docs/kiro-acp-protocol.md`](kiro-acp-protocol.md) — protocol reference, primary source for capability claims above
- [`docs/ROADMAP.md`](ROADMAP.md) — phased direction for cyril; this work should land as a numbered phase
- Pi context-workflow source: https://raw.githubusercontent.com/owainlewis/pi-extensions/refs/heads/main/extensions/context-workflow/context-workflow.ts
- Pi extension docs: https://pi.dev/docs/latest/extensions

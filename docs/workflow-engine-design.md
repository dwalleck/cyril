# Workflow Engine Design

> **⚠ Empirical corrections 2026-05-23.** Three load-bearing assumptions in this doc were checked against Kiro 2.4.1 and need adjustment before any code lands. See [§ Empirical corrections (2026-05-23)](#empirical-corrections-2026-05-23) at the bottom for the verified facts and how they reshape the design. The high-level direction (state-machine workflows driving an ACP agent through stages) is sound; the specific `SpawnSubagent` action signature needs to drop the `agent` field, and `SubagentResult.final_message` needs to be sourced from a different wire surface.

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

---

## Empirical corrections (2026-05-23)

Three findings from a wire-level probe against Kiro 2.4.1 (artifacts: `/tmp/conductor-spike/logs-241/20260523-*.log`, `experiments/conductor-spike/trace-2.4.1-tui-recorder.jsonl`). Each updates an assumption made in the original design above.

### 1. `SpawnSubagent.agent` cannot be passed per-spawn

**Original assumption** (in `StageAction::SpawnSubagent` and the `ContextWorkflow` example):

```rust
StageAction::SpawnSubagent {
    agent: "code-reviewer".into(),    // ← assumed mode/role selector
    task: "...",
    on_complete: ...,
}
```

**Verified reality:** `_session/spawn` accepts `{sessionId, task, name?}` per Kiro 2.4.1 user docs and empirical probe. `name` is a **UI label for the crew monitor**, NOT a mode selector — the Kiro `/spawn` documentation is explicit about this. Spawning with `name: "kiro_planner"` produces a subagent whose mode is `kiro_default` (inherited from the parent), not `kiro_planner`. See [`docs/kiro-acp-protocol.md` § 7](kiro-acp-protocol.md#_sessionspawn--request) for the corrected wire shape and provenance.

**Implication:** the `StageAction::SpawnSubagent` action should drop the `agent` field. The clean-context advantage holds (subagent starts with empty conversation history) but role specialization is not available via `/spawn`. Three workarounds, ranked by ugliness:

1. Rely on task framing alone — same agent, fresh context, role-shaped prompt.
2. Switch main session's mode before spawning (via `commands/execute model` or `agent` slash command), spawn, switch back. Racy; pollutes the user's interactive mode.
3. Trigger the agent's `subagent` tool via a prompt — non-deterministic.

The Kiro docs draw a sharper distinction the design missed: `/spawn` (user-initiated, parallel long-running session, no role selection) versus agent-initiated subagents (created via the agent's `subagent` tool, support role specialization through the tool's stages array). At the wire level both surface in `_kiro.dev/subagent/list_update`, but they are semantically different mechanisms — only the agent-initiated path supports role selection, and clients cannot directly invoke it.

### 2. `SubagentResult.final_message` source — `Summarizing` tool_call, not `inbox_notification`

**Original assumption:** the runner reads the subagent's result message from `_kiro.dev/session/inbox_notification`.

**Verified reality:** `inbox_notification` carries only metadata — `{sessionId, sessionName, messageCount, escalationCount, senders}`. The actual result content is delivered via a different mechanism. When the subagent completes its turn, the **parent agent** emits a `session/update` of variant `tool_call` on the **main session**, with `title: "Summarizing"`, `kind: "other"`, and `rawInput.taskResult` containing the subagent's final message:

```json
"rawInput": {
  "__tool_use_purpose": "Task is complete, reporting back.",
  "taskDescription": "<the original task>",
  "taskResult": "<the subagent's final message>"
}
```

**Implication:** the workflow runner detects subagent completion by watching the main session's `session/update` stream for `tool_call`s with `title="Summarizing"` and `kind="other"`. `SubagentResult.final_message` is `rawInput.taskResult`; the original `task` argument can be correlated via `rawInput.taskDescription`. See [§ 11.5](kiro-acp-protocol.md#115-subagent-result-delivery-the-summarizing-tool_call) of the protocol doc.

This is structurally cleaner than the inbox-based design: the workflow runner only needs to listen on the main session's stream (which it's already listening on for `on_turn_complete`), not on a per-subagent stream. The subagent's own `session/update` stream (under its own sessionId) carries the full per-message history if needed; `taskResult` is the agent-summarized version.

### 3. `Compact { then }` confirmed; no design change

`/compact` works as the design assumed. Wire flow verified on 2.4.1:

```
→ _kiro.dev/commands/execute {command: "compact", args: {}}
← {success: true, message: "Compacting conversation...", data: null}   (immediate ack)
← _kiro.dev/compaction/status {status: {type: "started"}}
← _kiro.dev/compaction/status {status: {type: "completed"}, summary: "<markdown>"}
← _kiro.dev/metadata {contextUsagePercentage: <reduced> ...}
```

`StageAction::Compact { then }` is implementable as designed: send the execute, watch for `compaction/status` with `type: "completed"`, then enter `then`. The `summary` field is at the top of `params` (not nested under `status`) — matches cyril's existing parser.

### Updated implementation phase notes

These corrections do not invalidate the phased plan above (Phases 1–6). Concrete adjustments:

- **Phase 1 (Types only):** drop `agent` from `StageAction::SpawnSubagent`. Add `taskDescription: String` to the action so the runner can correlate `Summarizing` tool_calls back to their spawning stage (multiple subagents in flight need to match results to spawns).
- **Phase 4 (Subagent stages):** the runner watches the main session's `session/update` stream for `tool_call { title: "Summarizing" }`. Detection is therefore cheaper than the original design (no separate subagent-stream wiring required for completion detection — only for per-message history capture if a workflow wants it).
- **Open question on role specialization** (formerly resolved as "spawn with `agent` field"): now genuinely open. Workflows that need fresh-context AND role-specialized reviewers should either rely on prompt framing (simplest) or test the mode-switching dance as a Phase 4 sub-spike before locking the trait API.

---

## Empirical corrections, round 2 (2026-05-23 evening)

A second verification pass (multi-subagent code-review capture + manual `/spawn` test) yielded findings that **invalidate round-1 Correction 2 in its current form** and require a substantive reframe of the entire subagent stage strategy. Round-1 Correction 1 is sharpened; Correction 3 (compact) is unchanged.

The new artifact: `experiments/conductor-spike/trace-2.4.1-multi-subagent.jsonl` (5484 lines, 1.4 MB). Covers a 4-stage agent-initiated review crew plus a separate `/spawn` invocation by the user.

### 4. There are TWO distinct subagent mechanisms with different semantics

The protocol doc now documents this comprehensively in [§ 11.11 (Client-spawned vs agent-crew subagents asymmetry)](kiro-acp-protocol.md#11-empirical-wire-type-verification-241-captures), but the workflow design needs to internalize it:

| Aspect | Agent-crew subagents | Client `/spawn` subagents |
|---|---|---|
| Spawn primitive | Agent invokes its `subagent` tool with `stages[]` | Client sends `_session/spawn {task, name?}` |
| Role specialization | ✅ `stages[].role` references `.kiro/agents/<role>.json` | ❌ inherits parent mode |
| Lifecycle | `working → terminated` (single-task; gone) | `working → terminated → awaitingInstruction` (long-lived; persists) |
| Persistence | Single-shot worker | Stays alive for `_message/send` follow-ups |
| Result delivery | `Summarizing` tool_call on parent stream + inbox notification | **Neither fires.** Result stays on subagent's own `session/update` stream |

These are not configuration variants of one mechanism — they're architecturally distinct primitives for distinct use cases.

### 5. Round-1 Correction 2 was incomplete: Summarizing fires ONLY for agent-crew

Round 1 said `SubagentResult.final_message` comes from a `Summarizing` tool_call. **That's only true for agent-crew subagents.** The Lancelot `/spawn` test in this trace ran a task (`"Review the @file:AGENTS.md file"`), completed in 22 seconds, and emitted **zero** `Summarizing` tool_calls and **zero** inbox notifications. Lancelot's review output exists only on its own `session/update` stream.

**Implication:** if the workflow runner uses `_session/spawn` to create reviewer subagents, it has NO wire-level result-delivery mechanism. It would have to:

1. Track the spawned subagent's `sessionId`
2. Subscribe to its `session/update` stream (separately from the main session)
3. Detect "task complete" by watching for `status → awaitingInstruction` in `subagent/list_update`
4. Reconstruct the final response from accumulated `agent_message_chunk`s

That's substantially more bookkeeping than the agent-crew path, which delivers a structured `taskResult` via `Summarizing` on the *main* stream the runner already consumes for `on_turn_complete`.

### 6. The right `StageAction` for clean-context review is NOT `SpawnSubagent`

Given the asymmetries above, the workflow design's `StageAction::SpawnSubagent` should be reconsidered. Two viable primitives:

**Option A: `StageAction::SpawnSubagent` uses `_session/spawn` directly (round-1 design)**
- Pros: client-driven, no LLM compliance dependency, can spawn even from non-orchestrator modes.
- Cons: no result delivery, no role specialization, requires the runner to track + parse the subagent's stream.
- Best for: "background investigator" workflows where the subagent does long-running work and the workflow runner periodically polls its state, not for "spawn-and-collect-result" patterns.

**Option B: `StageAction::DelegateToCrew` invokes an orchestrator agent (NEW recommendation)**

```rust
pub enum StageAction {
    /// Switch the main session to an orchestrator mode that has the
    /// `subagent` tool configured, then prompt it to spawn a crew with
    /// the given stage specifications. Results arrive via `Summarizing`
    /// tool_calls on the main session stream.
    DelegateToCrew {
        orchestrator_agent: String,    // .kiro/agents/<name>.json (e.g., "review-orchestrator")
        crew_prompt: String,            // instruction to the orchestrator
        on_complete: StageId,
    },
    // ... existing variants
}
```

- Pros: gets the full role-specialization machinery (stages[].role from `.kiro/agents/`), structured result delivery via Summarizing, inbox notifications, dependency-ordered stages, parallel execution — all the things the agent-crew mechanism provides natively.
- Cons: requires the user to have an orchestrator agent definition on disk (cyril could ship default templates), and the LLM has to actually invoke the `subagent` tool (a prompt-engineering concern, not protocol).

**Recommended split:** keep `SpawnSubagent` for "background worker" use cases and add `DelegateToCrew` for "orchestrated review" use cases. The original `ContextWorkflow` example (write → test → review → fix → verify) should use `DelegateToCrew` for the review stage, not `SpawnSubagent`.

### 7. Concrete primitives the workflow runner can use

Now empirically verified:

| Action | Wire mechanism | Notes |
|---|---|---|
| `SendPrompt` | `session/prompt` | Standard ACP; unchanged |
| `Compact { then }` | `commands/execute compact` + watch for `compaction/status: completed` | Confirmed wire flow (round 1, Correction 3) |
| `SpawnSubagent` (background worker) | `_session/spawn` | No result delivery; runner must consume subagent's session stream. Use only for fire-and-monitor workflows. |
| `MessageSubagent { session_id, content }` | `_message/send` | Sends a follow-up to a `/spawn`'d subagent that's in `awaitingInstruction`. Enables multi-turn dialogues with a parallel worker. Wire confirmed in this trace's `_session/spawn` capture. |
| `TerminateSubagent { session_id }` | `_kiro.dev/session/terminate` | Best-effort; response discarded. |
| `DelegateToCrew` (NEW) | `commands/execute agent {value: "<orch>"}` then `session/prompt {<crew-instruction>}` then await `Summarizing` tool_calls | The orchestrator agent invokes its `subagent` tool with stages[]. Results arrive via Summarizing on the main session. |

### 8. Updated phase plan

- **Phase 1 (Types only):** define `StageAction::SpawnSubagent` (single-shot worker, no result delivery), `StageAction::DelegateToCrew` (orchestrated multi-stage with role specialization + structured results), `StageAction::MessageSubagent`, `StageAction::TerminateSubagent`. Drop the `agent` field from `SpawnSubagent`. Add `taskDescription` to `SpawnSubagent` for stream-routing correlation.
- **Phase 4 (Crew stages):** runner watches main session for `Summarizing` tool_calls (via `_session/update.tool_call` with `title == "Summarizing"`). Reads `rawInput.taskResult` as the subagent's final response, `rawInput.taskDescription` as the correlator. For `SpawnSubagent`-style background workers, the runner separately subscribes to the subagent's own session/update stream — that's a different subscription model than crew stages, and the trait should reflect the distinction.
- **Phase 4 dependency**: requires `.kiro/agents/<orchestrator-name>.json` exist on disk with the `subagent` tool in its allowed-tools and a prompt that knows how to invoke it. Cyril should ship one or more default orchestrator templates in a `.kiro/agents-templates/` directory + a `/workflow init` command that copies them into the user's `.kiro/agents/`.
- **New cross-cutting concern**: the workflow engine needs to handle the `awaitingInstruction` lifecycle for `_session/spawn`'d subagents — they don't auto-terminate, so the runner needs `TerminateSubagent` calls on stage exit to avoid leaking long-lived sessions.

### 9. ContextWorkflow port — revised

The 5-stage write → test → review → fix → verify workflow re-expressed against the new primitives:

```rust
impl Workflow for ContextWorkflow {
    fn enter_stage(&mut self, stage: StageId, _: &WorkflowCtx) -> StageAction {
        match stage.into() {
            Stage::Write => StageAction::SendPrompt(format!(
                "Implement the spec:\n\n{}", self.spec
            )),
            Stage::Test => StageAction::SendPrompt(
                "Run the test suite. Report exit code.".into()
            ),
            // CHANGED: Review uses DelegateToCrew, not SpawnSubagent
            // Requires the user to have a code-review orchestrator agent
            // defined at .kiro/agents/review-orchestrator.json
            Stage::Review => StageAction::DelegateToCrew {
                orchestrator_agent: "review-orchestrator".into(),
                crew_prompt: format!(
                    "Review this implementation against the spec.\n\n\
                     Spec:\n{}\n\nFiles changed:\n{}",
                    self.spec, self.changed_files.join("\n")
                ),
                on_complete: Stage::Fix.into(),
            },
            Stage::Fix => StageAction::SendPrompt(format!(
                "Address these review issues:\n{}", self.review_issues.join("\n")
            )),
            Stage::Verify => StageAction::SendPrompt(
                "Re-run the test suite to verify.".into()
            ),
        }
    }
    
    fn on_subagent_complete(&mut self, stage: StageId, result: &SubagentResult) -> Transition {
        // Multiple Summarizing tool_calls arrive — one per crew stage
        // (e.g., code-reviewer, silent-failure-hunter, type-design-analyzer).
        // The runner aggregates them and calls on_subagent_complete with
        // each, OR (better) batches them into a single CrewComplete call.
        // Open design question — see § 10 below.
        ...
    }
}
```

This requires the user to have written `.kiro/agents/review-orchestrator.json` + a `code-reviewer.json` (etc.) ahead of time. Cyril could ship templates.

### 10. New open questions surfaced by round-2 corrections

- **Multi-stage crew result aggregation.** Agent-crew workflows produce *multiple* Summarizing tool_calls (one per stage), all arriving on the main session over time. Does the runner call `on_subagent_complete` once per Summarizing (and the workflow handles aggregation), or batch them into `on_crew_complete` (and the runner handles the join)? The former is simpler trait-wise; the latter is more honest about the workflow's intent.
- **Orchestrator agent author guidance.** What does a good `.kiro/agents/review-orchestrator.json` prompt look like? It needs to reliably invoke the `subagent` tool with the right stages. This is prompt-engineering, not protocol, but cyril probably needs to provide working templates or the feature is unusable for new users.
- **Persistent worker management UX.** `_session/spawn`'d subagents stay alive in `awaitingInstruction`. Should cyril surface them as long-running background tasks in the UI, with explicit "terminate" actions? Without this, users will accumulate orphan subagents over time.

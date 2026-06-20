# Kiro Subagent Tool Schemas

Authoritative JSON input schemas for Kiro's subagent / orchestration tools, extracted
from the **`kiro-cli-chat` 2.5.0** backend binary.

| field | value |
|---|---|
| Binary | `kiro-cli-chat` 2.5.0 |
| Binary snapshot | 2026-05-29T03:12:20Z |
| Backend axis | 2026-05-28 |
| `kiro-cli-chat` sha256 | `b2142a355add88b1d234ef405a226781aea5719e841c77c0b15ee1abcb2804fc` |
| Source | binary string + byte-region extraction (`grep -ab -o` → `dd`) |

Related: [`kiro-2.5.0-wire-audit.md`](kiro-2.5.0-wire-audit.md), [`kiro-acp-protocol.md`](kiro-acp-protocol.md).
Public docs (v2 only): <https://kiro.dev/docs/cli/chat/subagents/>.

## Tool-name aliasing (read this first)

The **public docs name the tool `subagent`**; the **v2 Rust tool registers as `agent_crew`**;
tui.js's crew-panel renders a *set* of names. So the wire `ToolCall.name` cyril observes
may be any of these — **match the set, not a single literal**:

```js
// tui.js crew-panel grouping (kiro-tui-2.5.0.js)
cp = new Set(["session_management", "subagent", "agent_crew"])
```

`use_subagent` / `delegate` are NOT in this set and are NOT mentioned in the public docs —
further confirmation they are the legacy **v1** tools.

## Why two systems exist — and which one is the default

Kiro ships **three selectable agent engines** in the same binary, chosen via
`kiro-cli acp --agent-engine <v1|v2|kas>`. **`--help` reports `[default: v2]`.** cyril
spawns `kiro-cli acp` with **no engine flag** (`crates/cyril/src/main.rs` defaults to
`["kiro-cli", "acp"]`), so **cyril gets v2** — meaning **`agent_crew` is the live subagent
tool cyril actually drives**, and `use_subagent` is the *legacy* v1 path.

| Engine flag | Crate(s) | Subagent tool(s) | Status |
|---|---|---|---|
| `v1` (legacy) | `crates/chat-cli/src/cli/chat/tools/` | `use_subagent`, `delegate` | Reachable only via explicit `--agent-engine v1` |
| **`v2` (DEFAULT)** | `crates/chat-cli-v2/` (ACP frontend) + `crates/agent/src/agent/tools/` (engine) | **`agent_crew`** (= "subagent"), `session` / `session_management`, `summary` | **Default `kiro-cli acp` path — what cyril drives today** |
| `kas` | TypeScript engine (assets may not be embedded) | (TS) | Opt-in; errors if KAS assets absent |

Engines are distinguishable by the source-path provenance baked into panic strings
(`crates/chat-cli/...` vs `crates/chat-cli-v2/...` + `crates/agent/...`).

> **Corroboration cyril can observe directly:** it already receives
> `_kiro.dev/subagent/list_update` carrying `hasLoop` / `loopMaxIterations` /
> `loopIteration`. Those are `agent_crew`'s `loop_to` block projected to the wire — a v1
> `use_subagent` session cannot produce them. So cyril's own traffic confirms it is on v2.

> **Earlier-revision correction:** a prior draft of this doc labelled the chat-cli /
> `use_subagent` engine "default" and `agent_crew` "embedded, not default-active." That was
> backwards — it was inferred from the `kiro_default` *agent-config* string (which lists
> `use_subagent`) without checking `acp --help`. The `kiro_default` config is the v1 engine's
> default *agent*, a different axis from the *engine* default.

> **`tui.js` does NOT contain these schemas.** The bundle only carries a *display
> grouping* — `cp = new Set(["session_management","subagent","agent_crew"])` — telling
> the crew-panel widget to render those tool names specially (it reads `action` / `task`
> / `name` / `target` from the tool content). The schema lives entirely in the Rust
> backend as embedded JSON strings.

> **Extraction note:** the embedded JSON contains literal newlines, so `strings` shatters
> each schema into fragments. To read one whole, find its byte offset with
> `grep -ab -o '<marker>' kiro-cli-chat` and `dd if=kiro-cli-chat bs=1 skip=<off> count=N | tr -d '\000'`.

---

## Engine v1 — `chat-cli` (legacy; only via `--agent-engine v1`)

### `use_subagent` — synchronous, blocking, ≤4 parallel

Source: `crates/chat-cli/src/cli/chat/tools/use_subagent.rs`.
Rust types: `UseSubagent`, `InvokeSubagent`, `AgentIdentifier`.

Description (verbatim, abridged): *"⚠️ CRITICAL DELEGATION TOOL ⚠️ … If you DON'T have
the necessary tools → delegate to a subagent that does … Up to 4 subagents can work in
parallel … subagents spawned together cannot communicate with each other; spawn dependent
tasks in a different tool call."* `@prompt-name` references in `query` are resolved inline
before the subagent starts.

```json
{
  "name": "use_subagent",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "enum": ["ListAgents", "InvokeSubagents"],
        "description": "ListAgents to query available agents, or InvokeSubagents to invoke one or more subagents"
      },
      "content": {
        "type": "object",
        "description": "Required for InvokeSubagents. Contains subagents array and optional conversation ID.",
        "properties": {
          "subagents": {
            "type": "array",
            "description": "Array of subagent invocations to execute in parallel.",
            "items": {
              "type": "object",
              "properties": {
                "query":            { "type": "string", "description": "The query or task to be handled by the subagent" },
                "agent_name":       { "type": "string", "description": "Optional specific agent; defaults to the default agent" },
                "relevant_context": { "type": "string", "description": "Optional additional context for the subagent" }
              },
              "required": ["query"]
            }
          }
        },
        "required": ["subagents"]
      }
    },
    "required": ["command"]
  }
}
```

### `delegate` — legacy, async/background (deprecated)

Description (verbatim, abridged): *"IMPORTANT: This tool is being replaced by
'use_subagent'. … The delegate tool runs tasks asynchronously in the background
(non-blocking), while use_subagent runs synchronously (blocking). Only use 'delegate' if
the user explicitly requests background/async execution … Files are stored in
`.kiro/.subagents/`."*

```json
{
  "name": "delegate",
  "input_schema": {
    "type": "object",
    "properties": {
      "operation": { "description": "launch, status, or list", "$ref": "#/$defs/Operation" },
      "agent":     { "description": "Agent name (optional; uses \"q_cli_default\")", "type": ["string", "null"], "default": null },
      "task":      { "description": "Task description (required for launch). Async — do NOT query immediately after launching.", "type": ["string", "null"], "default": null }
    },
    "required": ["operation"],
    "$defs": {
      "Operation": {
        "oneOf": [
          { "description": "Launch a new agent with a specified task", "type": "string", "const": "launch" },
          { "description": "Check status of a specific agent (or all if None)", "type": "object",
            "properties": { "status": { "type": ["string", "null"] } }, "required": ["status"], "additionalProperties": false },
          { "description": "List all available agents", "type": "string", "const": "list" }
        ]
      }
    }
  }
}
```

---

## Engine v2 — `chat-cli-v2` + `agent` crate (DEFAULT — what cyril drives)

The introspect doc-index confirms `subagent` / `agent_crew` / `crew` / `pipeline` / `DAG`
are the same tool: *"Spawn and coordinate multiple AI agents in a pipeline (DAG) with
dependency management."* Three tools cooperate: `agent_crew` (build the pipeline),
`summary` (report results / trigger loops), `session_management` (persistent peers).

### `agent_crew` — DAG pipeline with loops

Source: `crates/agent/src/agent/tools/agent_crew.rs`. Rust type: `struct AgentCrew`
(3 elements); supporting `CrewMode::Sequence`, `LoopConfig`, `LoopTriggerData`.

Description (verbatim): *"Spawn and coordinate multiple AI agents in a pipeline (DAG). Each
stage runs as a persistent session. Stages with no `depends_on` start immediately in
parallel. MODES: background (not yet implemented) — fire-and-forget, results arrive in
inbox; blocking (default) — waits for all stages, returns consolidated results. LOOPS: add
`loop_to` on a stage to create iterative cycles (e.g. reviewer loops back to implementer);
`trigger` = text in output that triggers the loop (e.g. 'NEEDS_CHANGES'); `max_iterations`
= safety cap; the target stage re-runs with the triggering stage's feedback as context.
Each stage becomes a session you can monitor via ctrl+g in the TUI."*

```json
{
  "type": "object",
  "required": ["task", "stages"],
  "properties": {
    "task": { "type": "string", "description": "Overall task description" },
    "mode": { "type": "string", "enum": ["blocking"], "description": "Execution mode: 'blocking' (wait for completion)" },
    "stages": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "role", "prompt_template"],
        "properties": {
          "name":            { "type": "string" },
          "role":            { "type": "string" },
          "prompt_template": { "type": "string", "description": "Task for this stage. Use {task} to reference the overall task." },
          "depends_on":      { "type": "array", "items": { "type": "string" } },
          "model":           { "type": "string" },
          "loop_to": {
            "type": "object",
            "description": "Loop back to a target stage when this stage's output contains the trigger text. Useful for review→implement cycles.",
            "properties": {
              "target":         { "type": "string", "description": "Name of the stage to loop back to" },
              "max_iterations": { "type": "integer", "description": "Maximum loop iterations (safety cap)" },
              "trigger":        { "type": "string", "description": "Text in output that triggers the loop (e.g. 'NEEDS_CHANGES')" }
            },
            "required": ["target", "max_iterations", "trigger"]
            // Validation bounds (from public docs, not in the embedded JSON):
            //   trigger >= 4 chars; max_iterations in 1..=10;
            //   no self-loops; no mutual loops; loops are planned upfront.
          }
        }
      }
    }
  }
}
```

### `summary` — subagent → main-agent result reporting

Description (verbatim, abridged): *"A tool for conveying task summary and results from
subagent to main agent. … If you are in a crew pipeline with a loop configured and you
want the target stage to re-run with your feedback, set `resultType` to 'changes_needed'.
Otherwise leave it unset or set to 'terminal'."*

```json
{
  "type": "object",
  "properties": {
    "taskDescription": { "type": "string", "description": "Description of the task assigned to the subagent" },
    "contextSummary":  { "type": "string", "description": "Relevant context gathered during task execution" },
    "taskResult":      { "type": "string", "description": "The final result or outcome of the completed task" },
    "resultType":      { "type": "string", "enum": ["terminal", "changes_needed"],
                         "description": "Use 'changes_needed' in crew pipelines to re-run the target stage with feedback. Defaults to 'terminal'." }
  },
  "required": ["taskDescription", "taskResult"]
}
```

### `session_management` — persistent peer sessions with inbox messaging

Description (verbatim, abridged): *"Manage persistent agent sessions for orchestration.
Sessions are long-lived agents that communicate via inbox messaging, unlike subagents
which are ephemeral. … Use `use_subagent` for one-off tasks; use sessions for ongoing
collaboration."*

```json
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "enum": ["spawn_session", "send_message", "read_messages", "list_sessions",
               "get_session_status", "interrupt", "inject_context", "manage_group", "revive_session"],
      "description": "The session management operation to perform"
    },
    "agent_name": { "type": "string", "description": "Agent config name for spawn_session" },
    "task":       { "type": "string", "description": "Initial task/prompt for spawn_session" },
    "name":       { "type": "string", "description": "Optional friendly name for spawn_session" },
    "role":       { "type": "string", "description": "Optional role description for spawn_session or manage_group add" },
    "target":     { "type": "string", "description": "Target session ID/name. Omit for escalation auto-route to parent." },
    "message":    { "type": "string", "description": "Message content for send_message, interrupt, or manage_group broadcast" },
    "priority":   { "type": "string", "enum": ["normal", "escalation"],
                    "description": "'escalation' auto-routes to parent if no target specified." },
    "limit":      { "type": "integer", "description": "Max messages for read_messages (default 5)" },
    "filter":     { "type": "string", "enum": ["active", "idle", "busy", "terminated", "all"],
                    "description": "Optional filter for list_sessions" },
    "verbose":    { "type": "boolean", "description": "Full details for get_session_status incl. live activity (default false)" },
    "context":    { "type": "string", "description": "Context content for inject_context" },
    "action":     { "type": "string", "enum": ["create", "add", "remove", "list", "broadcast"],
                    "description": "Action for manage_group" },
    "group":      { "type": "string", "description": "Group name for spawn_session or manage_group" },
    "persistent": { "type": "boolean", "description": "If true, session stays alive after its task (persistent helper). Default false (ephemeral worker)." }
  },
  "required": ["command"]
}
```

---

## Agent-config schema (orchestrator side, from public docs)

Distinct from the *tool-input* schemas above: this is how an agent config grants and
restricts subagent spawning. Lives under `toolsSettings.subagent` in the agent config.

```json
{
  "toolsSettings": {
    "subagent": {
      "availableAgents": ["reviewer", "tester", "docs-*"],
      "trustedAgents": ["reviewer", "tester"]
    }
  }
}
```

- `availableAgents` — restricts which agents can be spawned (glob patterns supported)
- `trustedAgents` — agents that run without permission prompts (glob patterns supported)
- A spawned subagent inherits `tools`, `toolsSettings`, and `allowedTools` from its own agent config.
- The orchestrator must have `subagent` in its `tools` array (or via the `@builtin` sigil) to spawn at all.

**Session / autonomy fields** (public docs):
- `is_interactive: false` → non-interactive subagent fails fast if a tool needs approval.
- `dangerously_trust_all_tools` → bypasses all prompts (use cautiously).
- File reads inside CWD auto-approve; reads outside CWD prompt.
- Each subagent session records its parent session ID for traceability.

## Wire-format correlation (why this matters for cyril)

The 2.5.0 `_kiro.dev/subagent/list_update` per-entry loop fields are the wire projection
of the **default v2 engine's** `agent_crew` `loop_to` block:

| Wire field (`list_update`) | Source in `agent_crew` / `summary` |
|---|---|
| `hasLoop` | stage has a `loop_to` object |
| `loopMaxIterations` | `loop_to.max_iterations` |
| `loopIteration` | runtime counter against `max_iterations` |
| `name` | stage `name` |
| `createdAtMs` | stage session creation timestamp |
| (loop fires) | a stage's `summary.resultType == "changes_needed"` matching `loop_to.trigger` |

**This is the live path, not scaffolding.** Because `kiro-cli acp` defaults to
`--agent-engine v2` and cyril passes no engine flag, the `agent_crew` / `summary` /
`session` tools are what cyril's sessions actually exercise. `use_subagent` / `delegate`
are reached only by explicitly spawning `kiro-cli acp --agent-engine v1`. The tool-call
*names* (`agent_crew` vs `use_subagent`) cyril sees in `session/update` therefore depend
on the engine flag — default is `agent_crew`.

**Verification method (for future re-checks):** `kiro-cli acp --help` reports the default
engine; cross-check the spawn command in `crates/cyril/src/main.rs`. Don't infer the active
engine from the `kiro_default` *agent-config* tool list — that's the v1 default agent, a
different axis from the engine default.

## Two control surfaces: prompt-steered crew vs. client-driven sessions

There is no ACP method for a client to hand Kiro a pre-built DAG. `agent_crew` and
`session_management` are **model-invoked tools** — the model decides, inside a tool call,
what the stages/dependencies/loops are. That leaves cyril with two distinct ways to get
multi-agent work done, with very different control properties:

| | **`agent_crew` (the DAG tool)** | **`session/spawn` + `_message/send`** |
|---|---|---|
| Who invokes | Kiro's model only | **cyril, directly** (client ext methods) |
| cyril's lever | The **prompt** (a bias, not a contract) | Full programmatic control |
| Determinism | Model may restructure/skip stages or not call the tool | Deterministic — cyril owns the graph |
| Shape | Native Kiro DAG: `depends_on`, `loop_to`, Ctrl+G monitor, Kiro's loop engine | Flat peer sessions; cyril implements its own stage/loop logic |
| Portability | Kiro-internal (v2 engine) | Vendor-portable across the ACP registry |
| cyril wiring | influenced via `session/prompt` text | `/spawn`→`SpawnSession`→`session/spawn` (`bridge.rs`); `/msg`→`_message/send`; `/kill`→`session/terminate` |

**Prompt-steering `agent_crew` precisely.** Because the schema is known, prompt text can
name the exact fields, e.g.: *"run this as a pipeline — a `research` stage, an `implement`
stage that `depends_on` research, and a `reviewer` that `loop_to`s `implement` with trigger
`NEEDS_CHANGES`, max_iterations 3."* This maps 1:1 onto the schema but remains a **bias**:
the model may deviate, so probe the resulting `list_update` rather than assuming the shape.

**Which to use.** Want Kiro's native crew UX and built-in loop engine → steer `agent_crew`
via prompt, accept non-determinism. Want cyril to *own* orchestration deterministically →
compose `session/spawn` peers in client code, bypassing `agent_crew` entirely. The latter is
the path chosen for cyril's session-level workflow engine (decision 2026-05-23), precisely
because it is client-drivable and vendor-neutral.

## cyril integration status & potential enhancement

**Current state (audited 2026-06-02): no v1 assumption to fix.** cyril's subagent support is
entirely notification-driven and engine-agnostic — it never matches the orchestrator's
`ToolCall.name`:

- Detection keys off the `kiro.dev/subagent/list_update` *method*, parsing engine-neutral
  metadata (`sessionId`, `sessionName`, `agentName`, `status`, `hasLoop` /
  `loopMaxIterations` / `loopIteration`) in `parse_subagent_entry` (`convert/kiro.rs`).
- The DAG is consumed via `pendingStages[]` + `depends_on` (`crew_panel.rs`,
  `SubagentTracker`) — i.e. cyril already models the v2 `agent_crew` pipeline shape.
- Orchestrator tool calls flow through the generic `ToolCall` path (`convert/mod.rs`) with
  `raw_input` passed through opaquely; nothing branches on the subagent tool's input schema.

So the v1→v2 tool rename (`use_subagent` → `agent_crew`) is invisible to cyril today.

**Potential enhancement — earlier stage-graph rendering (low priority).** cyril currently
learns the stage graph only when the first `subagent/list_update` arrives. The `agent_crew`
tool call's **`raw_input` carries the full planned DAG at spawn time** (`stages[]` with
`prompt_template` / `depends_on` / `loop_to`), a beat earlier than the first `list_update`.
Parsing it would let the crew panel show planned stages sooner.

> **Guard if implemented:** this is the one change that would reintroduce the name-matching
> trap. Any code that special-cases the orchestrator's subagent tool call MUST match the
> full set `{"session_management", "subagent", "agent_crew"}` (see *Tool-name aliasing*
> above), not a single literal, and tolerate `raw_input` being absent (it's `Option`).

## KAS subagent observability — and how it relates to the `_kiro/sessions/changed` roster

Verified against the embedded `@kiro/agent` 0.3.257 source maps (2026-06-18; see
[kiro-2.8.1-wire-audit.md](kiro-2.8.1-wire-audit.md)). On the **KAS** engine subagent activity
is surfaced through a different channel than the v2 `subagent/list_update`:

- **Subagents are real persisted sessions, but they do NOT appear in the live roster.** The
  covenant `CreatedReasonSchema = z.enum(['human','rewind','subagent','thread'])` and the
  `SessionMetadata` / `SessionSummary` schemas carry `parentSessionId` + `parentExecutionId`,
  so a subagent session is persisted with its parent linkage and shows up in **`session/list`**
  (full history). But the new live **`_kiro/sessions/changed`** roster excludes it:
  `SessionRosterManager.track()` is reached only via `trackSessionInRoster()`, which the
  `agent.ts` source calls from exactly two sites — the ACP **`session/new`** and
  **`session/load`** handlers. Subagents are spawned internally by the subagent tool
  (`createSubagentInvocationTools` → tools named `subagent/<agentId>`), never via `session/new`,
  and the `SessionRosterEntry` has no `parentSessionId` field.
- **Live subagent work streams through the PARENT session.** `execution-message-adapter.ts`
  tags sub-agent tool calls/results with `subExecutionId` (*"Add subExecutionId if this tool
  call belongs to a sub-agent"*), which becomes **`agentSubtaskId`** on the wire — the same
  grouping CLAUDE.md documents for the KAS engine. So a KAS crew panel is built from the
  focused session's `session/update` stream grouped by `agentSubtaskId`, **not** from the
  session roster.
- **cyril implication:** keep `SessionTracker` (top-level/peer, roster-fed) and
  `SubagentTracker` / crew rendering (parent-stream-fed) as separate concerns — KAS draws the
  same line at the protocol level. The `_kiro/sessions/changed` roster feeds the *session-level*
  (peer-session) workflow path, not subagent rendering (which stays KAS-3's `agentSubtaskId`
  job).

> **Docs-vs-bundle discrepancy — built-in semantic reviewer.** The public docs
> (https://kiro.dev/docs/cli/chat/subagents/) state there are no pre-built reviewer agents
> (review agents are user-configured). The 0.3.257 bundle disagrees: there is a **per-session
> `semanticReviewEnabled` toggle** gating a built-in *"semantic-reviewer subagent"* — a
> covenant `SessionMetadata` field ("Whether the semantic-reviewer subagent participates in
> this session", persisted across reload) plus a `prompt-template.ts` Mustache section
> `{{#semanticReviewEnabled}}…{{/semanticReviewEnabled}}` wrapping reviewer content in the
> planner profiles. This confirms the bundled `semantic_reviewer` / `functional_task_alignment`
> verification agents the CLAUDE.md KAS note describes; the public docs understate it.

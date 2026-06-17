# Kiro CLI 2.7.1 — ACP Wire Audit

**Analyzed:** 2026-06-16 (build date 2026-06-15T23:57Z, hash `45f3dc59`, target `x86_64-unknown-linux-gnu`) · **Method:** fresh download + sha256-verified against `prod.download.cli.kiro.dev` manifest, `strings`/symbol inspection of `kiro-cli-chat`, live raw-JSON-RPC probes against both `acp` (default v2) and `acp --agent-engine kas`, and direct reading of the self-extracted `@kiro/agent` TypeScript `.d.ts` definitions.

**Verdict for cyril:** **SAFE TO UPGRADE for the existing path — and a strategic inflection point.** Nothing on cyril's current `kiro-cli acp` (v2 engine) path changed shape. The headline is *additive and opt-in*: 2.7.1 is **the KAS landing** we have tracked since 2.3.0. KAS assets are now embedded in the binary and self-extract, and the KAS agent engine is **reachable and functional over ACP today** via `--agent-engine kas`. The interactive `chat` TUI gates KAS behind a staged-rollout switch, but the ACP surface — the one cyril uses — has no such gate.

---

## Headline: KAS is embedded and works over ACP

Through 2.7.0, `--agent-engine kas` errored with "KAS assets not embedded." That error path is gone. 2.7.1 embeds the KAS server bundle inside `kiro-cli-chat` and self-extracts it on first KAS launch:

- Binary strings: `extracting KAS bundle`, `KAS asset extraction complete`, sha-gated extract-on-first-run (`path does not exist, extracting` / `existing hash is different from embedded hash, extracting` / `asset does not need to be extracted` / `asset extracted successfully`).
- Extraction target: **`~/.local/share/kiro-cli/kas/`** (verified: extracted on my first `acp --agent-engine kas` run). **801 MB** extracted — this is why the headless tarball jumped to 527 MB xz (from ~250–300 MB pre-KAS).
- Launch env vars `KIRO_KAS_NODE_PATH` / `KIRO_KAS_SERVER_PATH`; server entry `node_modules/@kiro/agent/dist/server/acp-server.js`, run under node with `--experimental-wasm-modules` (cedar-wasm). bun is extracted separately and only for the v2 TUI.

**Extracted bundle = `@kiro/agent` v0.3.224** (the IDE-extension reverse-engineering had 0.3.17 / 0.3.323). Stack:

- **LangGraph-based agent**: `@langchain/langgraph` ^1.3.0, `@langchain/aws` ^1.3.7, `@langchain/core` ^1.1.45.
- **Official ACP SDK**: `@agentclientprotocol/sdk` ^0.19.0 — NOT the `sacp` crate the Rust v1/v2 engines use. (Confirms the loose-on-disk IDE dist finding.)
- **Cedar** policy engine: `@cedar-policy/cedar-wasm` ^4.9.1 (→ the `policyNotifications` capability and 2.6.1's `evaluate_url_permission`).
- `@huggingface/transformers` ^3.4.1 (local embeddings — knowledge / codeIntelligence), `@kiro/sandbox-proxy` 0.3.224 (bubblewrap), `@modelcontextprotocol/sdk`, hono, grpc, opentelemetry, AWS codewhisperer-streaming + control/runtime-plane clients.

---

## The `--v3` flag and the "V3 not supported" gate

`--v3` is a **hidden, `chat`-only convenience alias** for `--agent-engine=kas`:

```
--v3    Launch chat with the KAS agent engine (shorthand for --agent-engine=kas)
```

It only appears under `kiro-cli --help-all` (not plain help), is **rejected by `acp`** ("unexpected argument"), and is conditionally registered on `chat`.

**The gate is frontend-only.** Verified asymmetry on x86_64-linux (glibc, CachyOS):

| Invocation | Result |
|---|---|
| `chat --v3` / `chat --agent-engine kas` | **BLOCKED**: `V3 is currently not supported for your system` at `crates/chat-cli/src/cli/chat/mod.rs:5847` |
| `acp --agent-engine kas` | **WORKS**: full `initialize` + `session/new` succeed |

The word "currently" plus the working ACP path means this is a deliberate staged-rollout switch on the interactive v2 TUI, **not** a platform or missing-asset limitation. Cyril, which drives the ACP path, is unaffected by the gate.

---

## KAS ACP capability surface (vs v2)

`initialize` against the KAS engine advertises (new/changed vs the v2 baseline documented in `docs/kiro-2.7.0-wire-audit.md`):

- **Namespace = `_kiro/*`** (as predicted), not `_kiro.dev/*`. `extensionMethods`: `_kiro/knowledge`, `_kiro/codeIntelligence`, `_kiro/session/context`, `_kiro/session/compact`, `_kiro/session/export`, `_kiro/session/history`.
- **`sessionCapabilities: { list: {}, fork: { _meta.kiro.messageId: true } }`** — non-empty. On v2 these were empty / unstable; KAS makes `session/list` and `session/fork` real.
- `_meta.kiro`: `checkpoints: true`, `sessionList: true`, `policyNotifications: true`.
- New per-run log dir: `~/.kiro/logs/<timestamp>/{kiro,mcp,powers}.log`.
- `protocolVersion: 1`, prompt capabilities (image, embeddedContext), MCP http+sse — unchanged.

### `session/new` is far richer than v2

KAS returns `sess_<uuid>` session IDs (v2 used a bare uuid). Two big deltas:

**1. `configOptions` is now POPULATED, and `session/set_config_option` is a working SET** (verified — on v1.28→2.x v2 it was *always `null`* + "Method not found"). The request is `{sessionId, configId, value}` (value = the option's value-id string; or `{type:"boolean", value}` for boolean options). It **returns the rebuilt `configOptions`** with the updated `currentValue`, and that response is the source of truth — `autopilot on→off`, `mode vibe→spec` both took effect and persisted cumulatively across calls. Two caveats: (a) **no `config_option_update` notification is emitted on an explicit set** (that variant fires during prompt turns, not as a set echo), so a host must read the *response*, not wait for a notification; (b) **invalid values are silently coerced, not rejected** — `autopilot="bogus"` returned no error and resolved to `"off"`. KAS returns three live `select` options:

| id | currentValue | options |
|---|---|---|
| `mode` | `vibe` | the 7 modes below |
| `autopilot` | `on` | `on` (Autopilot — execute tools without confirmation) / `off` (Supervised — ask before file changes) |
| `contentCollection` | `disabled` | `enabled` / `disabled` (service-improvement opt-out) |

**2. Seven bundled modes** (vs v2's vibe/plan-ish):

| id | name | description |
|---|---|---|
| `vibe` | Default | General coding assistance |
| `spec` | Spec | Structured feature development |
| `quick-spec` | Quick Spec | Fast spec workflow: clarify, then auto-generate requirements, design, and tasks |
| `bug-fix` | Bug Fix | Structured bug-fix workflow: investigate, diagnose, resolve |
| `plan` | Plan | Plan-only mode, no changes (welcomeMessage) |
| `autonomous` | Autonomous | Autonomous execution; asks for a goal, auto-approves all tools except MCP, `/sandbox enable` (welcomeMessage) |
| `semantic_reviewer` | semantic_reviewer | Behavioral-level code review, narrative organized by concern, local or PR diffs |

`session/new` `_meta`: `schemaVersion: "1.0.0"`, `agentMode`, `workspacePaths`, `createdAt`/`lastModifiedAt`, `semanticReviewEnabled: true`, `ftaEnabled: false`.

---

## Subagent flows ARE changing under KAS

v2's single Rust `agent_crew` tool (DAG-pipeline + summary + session — see `docs/kiro-subagent-tool-schemas.md`) is replaced by a LangGraph orchestration with a richer tool set. Authoritative from the bundle's `.d.ts`:

**`InvokeSubAgent`** (`dist/tools/invoke-subagent.d.ts`) — invoke a single sub-agent execution nested in the parent's workspace context.
- Input schema: `{ name, prompt, explanation, preset?: string|null, contextFiles?: [{path, startLine?, endLine?}] }` — note the ability to pass **file ranges** into the child (converted to synthetic `read_file` args).
- **`MAX_CONCURRENT_SUBAGENTS = 5`** — per-parent concurrency semaphore, abort-aware (queued children don't linger after interruption).

**`OrchestrateSubAgent`** (`dist/tools/orchestrate-subagent/`) — the direct successor to `agent_crew`'s DAG pipeline.
- Input: `{ task, stages: [{ name, role, prompt_template, depends_on?: string[] }] }`.
- "Executes them in parallel waves using InvokeSubAgent." Validates unique stage names + that all `depends_on` exist + **no cycles (Kahn's algorithm, `validateStages`/`hasCycles`)**.

**`subagent_response`** (`dist/tools/subagent-response.d.ts`) — the tool a subagent calls as its **final action** to return `{ response, files?: [{path, startLine?, endLine?}] }` to the orchestrating parent; the files are pulled into the parent's context.

**`createSubagentInvocationTools`** (`dist/tools/subagent-tool.d.ts`) — generates one tool per registered agent, named **`subagent/<agentId>`**, from a `CustomAgentRegistry`. Autonomous-mode planner/coder sub-agents use a structured input schema `{ user_instruction_verbatim, environmental_context?, explanation }` wrapped in XML framing blocks, with **framing-tag stripping as a prompt-injection defense**.

**Bundled agents** (`dist/bundled-agents/`, `getBundledAgentDefinitions(semanticReviewEnabled, ftaEnabled)`) — three built-in agents, each authored as a **markdown doc + YAML frontmatter** and parsed by `custom-agent-parser` (gray-matter):

| agent | purpose | tools | gating |
|---|---|---|---|
| **`Explore`** | read-only codebase understanding — architecture, dependency/data-flow tracing ("explain, don't change"). Primary consumer of the `c2s_*` CodeToSpec tools. | `read_file`, `list_directory`, `grep_search`, `file_search`, `fs_write`, + full `c2s_*` suite (`code_to_spec`, `c2s_list_packages/modules/functions`, `c2s_describe_function/module`, `c2s_traverse_functions/modules`) | always available |
| **`semantic_reviewer`** | behavioral code review (by concern, not file); diff-driven, layered output to `./semantic-review/…md` with confidence qualifiers + APPROVED/NEEDS_CHANGES verdict across iterative passes | `read_file`, `fs_write`, `grep_search`, `file_search`, `execute_bash`, `@sandbox/github_*` (PR/issue/CI); `includePowers`+`includeMcpJson` | `semanticReview` setting |
| **`functional_task_alignment`** (fta) | claim-based output validation / devil's-advocate — decomposes the user's request and verifies each claim against the **diff/on-disk state** ("the diff wins; the agent's summary is unverified claims") | `read_file`, `fs_write`, `str_replace`, `grep_search`, `file_search`, `execute_bash` | `fta` setting |

**Enable path** — both gated agents are KAS feature settings on the same `_meta.kiro.settings` channel as `subagentOrchestration` (set at `initialize`, default applies otherwise):

- `resolveSemanticReview = parsed.data.semanticReview?.enabled ?? persistedMetadata ?? **true**` → semantic_reviewer is **on by default**.
- `resolveFta = parsed.data.fta?.enabled ?? persistedMetadata ?? **false**` → fta is **off by default**; enable with `initialize` `clientCapabilities._meta.kiro.settings.fta = {enabled: true}` (verified mechanism — same as `subagentOrchestration`).

Note the cluster: `semantic_reviewer` and `fta` are the **verification agents**, and `fta` in particular is KAS's built-in answer to a "validator" stage (diff-grounded, claim-based) — the role a custom `crew-dag-loop` validator played, shipped as a one-shot bundled agent rather than a declarative loop.

### How a crew executes — one-shot, fail-fast DAG (no native loop)

Mental model: KAS exposes subagents through **two tools over one registry** — `InvokeSubAgent` delegates *one* task to one registered agent; `OrchestrateSubAgent` runs a whole *crew* (DAG of registered agents) in a single call. Registered agents (your custom-agent definitions) are the stage `role`s and are also individually callable as `subagent/<agentId>`.

**Each stage runs as its `role` agent, with that agent's own model, tools, and permissions.** KAS resolves `role` → the registered `CustomAgentDefinition` and applies, per stage: its **`model`** override (`definition.model` → `executionModel`/`modelOverride` — stages can run on different models); its **`tools`/`excludedTools`** allowlist (`buildToolPolicy` → `filterTools`, plus `includeMcpJson`/`includePowers`); and its **`permissions.rules`** as a `SubagentPolicyEngine` built via `policySession.createSubagentEngine(definition.permissions)` that **combines restrictively with the parent** (`combineResults` takes the more-restrictive effect). So per-agent model choices and write/exec scoping *are* honored across a crew — a stage can only be scoped **down** from the parent session, never escalate above it. (`CustomAgentDefinition` carries `model?`, `tools`/`excludedTools`, and `permissions: {rules:[{capability, match?, exclude?, effect: allow|deny|ask}]}`.)

*Live-verified (2026-06-16, `experiments/conductor-spike/probe-kas-agent-scope-2.7.1.py`):* a 2-stage `OrchestrateSubAgent` crew using two workspace-local `.kiro/agents/*.json` roles — `probe-ro` with `permissions: {rules:[{capability:"fs_write", match:["**"], effect:"deny"}]}` and `probe-rw` unrestricted — ran with the denied stage **unable to write** (no `fs/write_text_file` callback, file absent, subagent reported "fs_write is denied by policy") while the unrestricted stage wrote successfully. Confirms per-agent permission scoping is enforced on each stage. (The `model` override rides the same definition-application code path and the definition demonstrably loaded, but the model used per stage was not independently observable on the wire.)

`OrchestrateSubAgent.handle` (from `dist/orchestrate-subagent-*.js`) runs the pipeline exactly once:

1. **Validate** — unique stage names, every `depends_on` exists, and **cycles are rejected** (`hasCycles`/Kahn → "Dependency cycle detected in stages").
2. **Parallel waves** — every stage whose dependencies are satisfied runs together (`Promise.all`), each as an `InvokeSubAgent` to its `role`; then the next wave. Upstream outputs thread into the stage prompt (see *inter-stage context* below).
3. **Fail-fast** — the first stage returning `success: false` halts the pipeline (`Pipeline stopped: <stage> failed: <response>`), emits an `Error`, and returns partial results. No stage is retried; no later wave runs.
4. Throughout, emits `_meta.kiro.pipeline.stages[]` (per-stage `status`/`dependsOn`/`agentSubtaskId`) for rendering.

**Inter-stage context (how outputs pass between stages, from `executeStage`).** Each stage's prompt is built as: take the stage's **`prompt_template`** and substitute **`{task}`** with the crew's overall `task` string; then, for **each `depends_on` stage that succeeded**, append that upstream stage's **`response` text** under a header:

```
<prompt_template, {task} substituted>

---
## Context from previous stages

## Results from <upstreamStageName>

<upstream stage's response text>
```

So context flows as **prompt-injected text along the DAG edges only** — `depends_on` stages (not all prior stages), success-only, and only the upstream's text **`response`** (what it returned via `subagent_response`). There is no structured data channel and no placeholder other than `{task}`. Two consequences: (a) an upstream stage's returned **`files`** are *not* auto-attached into a downstream stage's prompt — only its text response is; (b) **file-based handoff between stages** (e.g. a stage writing `.testagent/research.md` for a later stage to read) works via the **filesystem**, not this threading — each stage reads the prior stage's files itself with its fs tools (they persist in the workspace across stages). The threading is a bonus summary in-prompt; the filesystem is the authoritative channel for anything larger than a text blurb.

**Gotcha — `contextFiles` is unavailable in a crew stage.** `InvokeSubAgent` accepts `contextFiles: [{path, startLine?, endLine?}]` to pre-seed a child with file ranges, but **`OrchestrateSubAgent` stages cannot use it**: the stage schema has no `contextFiles` field, and `executeStage` hardcodes `contextFiles: void 0` when it calls invoke. So inside a crew, files reach a stage only via the filesystem (stage reads them itself) or the text-`response` threading above — never via `contextFiles`. Parent→child file seeding with `contextFiles` works only on a *direct* `InvokeSubAgent` call. (For code review specifically, neither `contextFiles` nor the threading is the right input anyway — the bundled `semantic_reviewer` agent fetches the **diff** via `git diff` and reads files on demand; see below.)

**There is no loop or retry inside the orchestrator** — cycles rejected, each stage runs once, failure stops rather than re-runs. This is the one thing v2's `agent_crew` had that KAS dropped: `loop_to {target, max_iterations, trigger}` is gone. A loop-on-failure (e.g. validator → re-run implement, max N) therefore **cannot live in the crew payload**; it must be driven one level up:

- the **orchestrating model** re-invoking `OrchestrateSubAgent`/`InvokeSubAgent` after seeing the `Error` result (model-driven — essentially what a "manual" RPI crew already does), or
- a **blocking command hook** (`PreToolUse`/`PreTaskExec`, non-zero exit → `block` + stderr fed back), with the iteration cap in the hook's own logic.
- (KAS's agent-loop graph has its own failure-intervention/restart path — `shouldRestartGraph`, `agentIterationLimit`/`agentIterationNumber` — but that retries the *engine's own turn*, not your crew stages, and isn't user-authorable.)

**The no-loop is a design choice, not a LangGraph limitation.** Worth being precise, because it's tempting to blame the framework: `OrchestrateSubAgent` is **not a LangGraph graph** — it has zero `StateGraph`/`addNode`/`addConditionalEdges`; it's a hand-rolled `Promise.all` wave scheduler (`executeStage`) that *explicitly* rejects cycles in its own validation (`hasCycles`/Kahn). Meanwhile the agent loop in the same engine (`graph-D30A2gnX.js` / `chat-agent-graph`) **does loop back**, via multiple `addConditionalEdges(...)` routing to `END` or back to a node (the model→tools→model cycle bounded by `agentIterationLimit`). LangGraph is cycle-native; loop-back is already running in KAS one layer over. So restoring a bounded crew loop is feasible without re-architecting — either lift the cycle-rejection in the scheduler and re-add a `loop_to`/`max_iterations` construct (v2 parity), or model the crew as a LangGraph subgraph with conditional edges (native bounded loop). The absence reflects a deliberate "keep the crew a predictable acyclic fan-out scheduler, push iteration up to the agent-loop/autonomous layer" choice (and likely a not-yet-reimplemented `loop_to` in the 0.3.x rewrite) — *not* a constraint of the foundation. This strengthens both the upstream-feedback path ("you already cycle in the agent graph — expose a bounded loop on the crew tool, as v2 had") and a cyril-side review-loop stage (which would be doing exactly what LangGraph is built for).

So: **a KAS crew is a one-shot DAG of subagents that fans out in dependency waves and stops on first failure; looping is now an orchestration concern in the driving agent or a hook, not the crew definition.** (Plugin impact: a manual RPI `agent_crew` ports to `OrchestrateSubAgent` directly; a `loop_to`-based validator loop must be reshaped into model-re-invocation or a hook gate.)

**Concurrency cap — `MAX_CONCURRENT_SUBAGENTS = 5`.** This is a per-parent-execution semaphore (`getExecutionSemaphore(execution)` + `acquireWithAbort`, abort-aware), **shared** across *everything* a parent spawns — both direct `InvokeSubAgent` calls and `OrchestrateSubAgent` stages acquire the same semaphore. So a wave with more than 5 dependency-ready stages runs 5 at a time and **queues the rest**; the effective fan-out width is 5 regardless of graph shape. Recursive crews are blocked (the `subagentOrchestration` gate prevents a subagent from spawning its own crew), so the cap can't be multiplied by nesting — it's 5 concurrent subagents per turn, full stop.

**What the DAG model is *for* (its core benefit).** The win is **declarative, dependency-aware parallel fan-out in a single tool call**, with three things the model would otherwise hand-manage across its own turns: (1) **engine-scheduled waves** — you hand over the whole graph and the engine computes the topological order (parallel where independent, serial where dependent) instead of the model reasoning turn-by-turn about what can run together; (2) **automatic context threading** — upstream `response` text is injected into downstream prompts; (3) **heterogeneous specialist composition** — each stage runs as its own registered agent with its own model/tools/permissions, so a pipeline of differently-scoped specialists is expressed declaratively and rendered upfront (`_meta.kiro.pipeline.stages[]`). It also conserves the parent's turns/tokens (one deterministic run vs multi-turn conducting). The design center is therefore **breadth, not depth** — decompose-and-fan-out-with-dependencies, run once — which is exactly why it's a clean fit for "run N specialists in parallel and synthesize" and a poor fit for "iterate until a quality bar is met" (the dropped `loop_to` case).

### The agent loop is a LangGraph StateGraph

`dist/graphs/`: `chat-agent-graph` (main loop), `custom-agent-graph` (sub-agent loop), `consume-queued-steering`, `context-overflow-handler`, `inject-todo-context`.
`dist/nodes/`: `context-reset`, `failure-detection`, `intent-detection`, `invoke-spec-agent`, `populate-steering`, `post-tool-steering`, `summarization-detection`, `summarization`, `user-hook`, `user-intervention`.

This is a literal `@langchain/langgraph` (^1.3.0) `StateGraph`, not a metaphor. `chat-agent-graph.d.ts` imports `END` from `@langchain/langgraph` and defines its state as a LangGraph `AnnotationRoot` of channels (`LastValue`/`BaseChannel`/`OverwriteValue`), including the loop counter `agentIterationLimit`/`agentIterationNumber`. The graph is wired with `.addEdge(...)` (×12) and **`.addConditionalEdges(...)` (×5)** — those conditional edges are the routing and the loops: the agent's model→tools→model cycle is a conditional-edge cycle that loops back until a stop condition (or the `agentIterationLimit` runaway guard) routes it to `END`. So **iteration in KAS lives in the graph's conditional edges, authored by Kiro — not as a user-facing parameter.** This is the architectural root of the crew-loop gap: v2's `loop_to` was a declarative knob on the `agent_crew` tool; KAS moved iteration into `addConditionalEdges` inside the engine, where it isn't authorable from the orchestrate payload. (Model calls run through `@langchain/aws` + `@langchain/core`; v2 was a hand-written Rust agent loop.)

Notably, **2.7.0's queue-steering wire feature is realized here as graph nodes** (`populate-steering` / `post-tool-steering` / `consume-queued-steering`), draining at tool boundaries — consistent with the 2.7.0 finding that steering drains at tool boundaries.

---

## KAS auth contract: the host must supply the token (verified live)

Unlike v2 (which reads its own auth store), **KAS makes the ACP host provide the bearer token.** Verified by running a full authenticated turn:

- KAS uses `AcpCallbackAuthProvider` and issues a server→client request **`_kiro/auth/getAccessToken`** (params `{}`).
- The host must reply **`{ accessToken, expiresAt, profileArn, provider? }`**:
  - `accessToken` required — empty/missing → turn dies with `agent_message_chunk` "[TokenInvalidError] … Host refresh callback returned no access token".
  - `expiresAt` parsed by `new Date(t).valueOf()`; must be > `now + ~3min` or it throws `malformed expiresAt` / `token already expired`.
  - **`profileArn` required in practice** — without it the backend 400s mid-turn: "profileArn is required for this request." (KAS logs "Hosts SHOULD include profileArn so KAS can route region.")
- `kiro-cli-chat acp --agent-engine kas` does **not** self-answer this — it forwards to the topmost ACP client. There is no `--token-path` / fallback on the acp path in 2.7.1.
- The token lives in kiro's own store: `~/.local/share/kiro-cli/data.sqlite3`, table `auth_kv`, key `kirocli:social:token` → `{access_token, expires_at, refresh_token, profile_arn, provider}`. It refreshes on use (an idle token expires; any authenticated `kiro-cli` op re-mints it).

**Cyril impact:** to drive KAS, cyril must implement an `_kiro/auth/getAccessToken` responder and source a live kiro token (reading kiro's credential store, refreshing as needed). This is a real integration dependency, not a passive one — it activates the dormant `_kiro/auth/getAccessToken` first seen in 2.6.1.

---

## KAS subagent wire format (verified live)

**KAS does not use the v2 `kiro.dev/subagent/list_update` model at all** (zero occurrences in a turn that spawned two subagents). Instead, **subagents are ordinary ACP `tool_call`s tagged `_meta.kiro.kind: "agent-subtask"`**, linked by `agentSubtaskId`. Lifecycle for one subagent (model chose parallel `InvokeSubAgent`, not `OrchestrateSubAgent`):

1. **Spawn** — `tool_call`:
   ```json
   { "sessionUpdate":"tool_call", "toolCallId":"invoke_subagent_tooluse_…",
     "title":"Sub-agent: general-task-execution", "kind":"other", "status":"pending",
     "rawInput":{"name":"general-task-execution","prompt":"You are \"poet\"…","explanation":"…","contextFiles":[]},
     "_meta":{"kiro":{"kind":"agent-subtask","agentSubtaskId":"invoke_subagent_tooluse_…"}} }
   ```
   `rawInput.name` selects the registered agent; the persona/role goes in `prompt`.
2. **In progress** — `tool_call_update` `status:"in_progress"`; **`_meta.kiro.agentSubtaskId` rotates to the real child-execution UUID** (the join key for everything below).
3. **Child returns** — a *separate* `tool_call_update`: `{ toolCallId:"tooluse_…", title:"Subagent Response", status:"completed", rawInput:{response, files:[]}, _meta.kiro:{agentSubtaskId:<childUUID>, toolOrigin:"acp"} }` (the child's `subagent_response` tool).
4. **Parent completes** — `tool_call_update` on the `invoke_subagent_*` id, `status:"completed"`, `rawOutput:{ response, subExecutionId:<childUUID> }`.

**Permission:** each spawn fires a standard `session/request_permission`: `{ toolCall:{toolCallId, title:"Invoke Agent"}, options:[accept|always-accept|reject|always-reject], _meta.kiro:{ toolId:"invoke_sub_agent", consent:{ capability:"subagent", resource:"<agentId>" } } }`.

**New `session/update` variant `config_option_update`** echoes the full `configOptions` array mid-turn.

**Cyril impact:** the current `SubagentTracker` + `crew_panel` (built on `list_update`) will see *nothing* under KAS. To render KAS crews, cyril groups `tool_call`s by `_meta.kiro.agentSubtaskId` and recognizes `kind:"agent-subtask"` + the `title:"Subagent Response"` child returns. They already render as opaque tool calls today; nested-crew UI is the only gap.

### The DAG orchestrator (`OrchestrateSubAgent`) is gated off by default

In vibe mode, the model has only **`invoke_sub_agent`** (parallel/sequential single invokes) — it does *not* get the DAG tool. The bundle gates it:

```js
if (customAgentRegistry) {
  if (isSettingEnabled(settings, "subagentOrchestration"))   // off by default
    chatTools.push(createAcpOrchestrateSubAgentTool(...));
}
```

The bundle comment states the purpose: prevent "crews that recursively spawn crews." The setting is a KAS feature toggle (`settings[key].enabled === true`) parsed from `_meta.kiro.settings` via `parseSettings` against `BaseAgentSettingsSchema` (siblings: `knowledge`, `codeIntelligence`, `toolSearch`).

**Activation (verified):** the toggle must be supplied by the host in **`initialize` → `clientCapabilities._meta.kiro.settings.subagentOrchestration = {enabled: true}`** (the server caches this as `clientMeta`). Sending it on `session/new` `_meta` does **not** work. In KAS's own settings-builder (`Hme`, used by the IDE/first-party client) `subagentOrchestration` **defaults to `true`** (sourced from the `chat.*` settings store) — but a bare ACP client that omits it gets `parseSettings(undefined) → {}` → the gate reads `false`. So it is **not** a per-agent JSON field (absent from `agent_config.json.example`); it is a client/host-supplied capability setting at initialize, normally derived by the host from the kiro settings store.

With it enabled, the DAG tool fires. Captured `tool_call` (the orchestrator's own wire shape):

```json
{ "sessionUpdate":"tool_call", "toolCallId":"tooluse_…", "title":"Orchestrate Sub-agent",
  "kind":"other", "status":"in_progress",
  "rawInput": { "task":"…",
    "stages":[ {"name":"pick","role":"general-task-execution","prompt_template":"…"},
               {"name":"double","role":"general-task-execution","prompt_template":"…","depends_on":["pick"]},
               {"name":"report","role":"general-task-execution","prompt_template":"…","depends_on":["double"]} ] },
  "_meta":{"kiro":{"pipeline":{ "groupId":"pipeline-…",
    "stages":[ {"name":"pick","role":"general-task-execution","status":"pending","dependsOn":[],"agentSubtaskId":"<uuid>"},
               {"name":"double","role":"general-task-execution","status":"pending","dependsOn":["pick"],"agentSubtaskId":"<uuid>"},
               {"name":"report","role":"general-task-execution","status":"pending","dependsOn":["double"],"agentSubtaskId":"<uuid>"} ] }}} }
```

Two things matter for rendering: (1) `rawInput.stages[].role` is the **registered agent id** (here the bundled `general-task-execution`); the v2 `agent_crew` `role` was a freeform label. (2) **`_meta.kiro.pipeline.stages[]` projects the whole DAG upfront** — each stage carries `dependsOn`, a `status` (advances pending→…), and a pre-assigned `agentSubtaskId` that links to that stage's child `agent-subtask` `tool_call`. This is the KAS analog of v2's `agent_crew` `pendingStages` that cyril's `crew_panel` already consumes — a host can render the full pipeline graph from this one notification.

**`OrchestrateSubAgent` vs v2 `agent_crew` — the schema changed (from `.d.ts`):**

| field | v2 `agent_crew` | KAS `OrchestrateSubAgent` |
|---|---|---|
| `task` | ✓ | ✓ |
| `stages[].{name, role, prompt_template, depends_on}` | ✓ | ✓ |
| `stages[].loop_to {target, max_iterations, trigger}` | ✓ | **removed** |
| `stages[].model` (per-stage model) | ✓ | **removed** |
| `mode` | ✓ | **removed** |
| cycle handling | loops allowed via `loop_to` | **cycles rejected** (Kahn's algorithm in `validateStages`) |

So KAS orchestration is a **pure acyclic DAG** executed in parallel waves; the v2 review-loop (`loop_to`/`max_iterations`) has no orchestrate-tool equivalent. Iteration moves into the graph layer (`nodes/failure-detection`, the bundled `semantic_reviewer`) or the orchestrator agent re-invoking. When the orchestrator runs, its per-stage children are the same `agent-subtask` `tool_call`s captured above (it calls `InvokeSubAgent` internally), capped at `MAX_CONCURRENT_SUBAGENTS = 5`.

---

## `/goal`: a v2 command with no KAS equivalent — autonomous mode instead

The v2 (Rust) engine added `/goal` + a `goal` tool (`{command:"complete", summary}`) in 2.7.0. **KAS does not implement it.** Grepping `@kiro/agent` finds no `goal` tool, no `command:"complete"`, no `SwitchToExecution`, and `/goal` is absent from KAS's `commands/available` (KAS serves its own command set — modes/steering/skills — not the v2 list). KAS's goal-driven execution is instead the **`autonomous` mode**, one of the 7 bundled session modes, defined in `dist/autonomous/`:

- `getAutonomousBrainDefinition(...)` → the "brain" (orchestrator) `CustomAgentDefinition` for `/autonomous`.
- `getAutonomousSubagentDefinitions(...)` → the subagent definitions the brain drives.

So a goal in KAS is pursued by a **brain orchestrator agent delegating to bundled subagents** (via `InvokeSubAgent`/`OrchestrateSubAgent`), not by a discrete tool call — reusing the per-agent model/permissions and crew machinery documented above. The mode's `session/new` welcome states the contract: *"Autonomous agent will ask for a goal and work towards it. All tools except MCP will be automatically approved. Enable a local sandbox using `/sandbox enable`."* This also explains why v2's `/goal` "loop" never manifested on bare ACP — the actual goal-pursuit loop lives in the autonomous brain+subagents, which only exists in KAS. (Read from the bundle's `dist/autonomous/` exports + the mode definition; not yet probed live.)

---

## KAS filesystem callbacks (verified live) — capability-negotiated

KAS is the **first Kiro engine to call ACP `fs/*` client callbacks** — but only when the host opts in. Two runs of a write-then-read-back task settled it:

| Client `initialize` advertises | KAS behavior |
|---|---|
| `clientCapabilities.fs = {readTextFile, writeTextFile}` | Routes **all** file I/O through server→client `fs/*` callbacks (4 in the run: read-before-write, the write, two verifying reads). File created by the *host*. |
| no `fs` capability (`{}`) | File I/O happens **in-process** (file created, **zero** callbacks) — identical to v1/v2. |

The methods are the **public ACP names, not `_kiro/fs/*`** (the IDE dist had hinted at the private namespace):
- `fs/read_text_file` — params `{sessionId, path, line?}` → reply `{content}`. (Reads do **not** require permission.)
- `fs/write_text_file` — params `{sessionId, path, content}` → reply `{}`. (Writes fire `session/request_permission` with `_meta.kiro.consent: {capability:"fs_write", resource, askType:"implicit", workspaceRoot, consentRound}`.)

The agent's own tool surface (`tool_call` "Write File" kind `edit`, "Read File" kind `read`) is unchanged; the `fs/*` callbacks are how those tools reach disk when the host owns the filesystem.

**Cyril impact — opt-in, NOT a hard requirement.** This refines `reference_kiro_no_fs_callbacks` (true only while the client advertises nothing). Cyril can:
- Keep advertising no fs capability → KAS does in-process I/O, **no new code needed** (KAS behaves like v2 here).
- Opt in (advertise fs + implement `fs/read_text_file`/`fs/write_text_file` responders) → gain a real proxy-stage hook over every file op KAS performs (audit, org policy, WSL path translation). This is the first time a Kiro engine makes that interception possible.

---

## KAS host-responsibility callback map (verified live)

A single turn (write a file + run a shell command + delete + open a URL) with the client advertising `clientCapabilities { fs: {readTextFile, writeTextFile}, terminal: true }` produced the definitive set of **server→client callbacks a host must service to drive KAS**. This is tighter than the ~45-method bundle surface — most of those are client→server methods or situational.

**Core contract for a coding turn:**

| Callback | Direction / shape | Required? |
|---|---|---|
| `_kiro/auth/getAccessToken` | `{}` → `{accessToken, expiresAt, profileArn}` | **always** — turn dies without (see auth-contract section) |
| `_kiro/terminal/shell_type` | `{sessionId}` → `{shellType}` (`bash`/`zsh`/`fish`/`powershell`/`sh`) | fired at session setup; feeds the system prompt's `Shell:` line — an empty reply yields `Shell: undefined` |
| `session/request_permission` | standard ACP | yes (cyril already implements it) — fires for writes/commands/deletes |
| `fs/read_text_file` / `fs/write_text_file` | `{sessionId, path[, content]}` → `{content}` / `{}` | only if `fs` capability advertised (else in-process) |
| `terminal/create` → `terminal/wait_for_exit` → `terminal/output` → `terminal/release` | create: `{sessionId, command, args[], cwd}` → `{terminalId}`; the rest key off `{terminalId}` | only if `terminal: true` advertised (else in-process) |

**Shell execution is host-delegated via ACP `terminal/*`, exactly parallel to fs** — capability-gated on `terminal: true`. Advertise it and every command the agent runs flows through the `create → wait_for_exit → output → release` lifecycle on the host; omit it and KAS runs the shell in-process. `_kiro/terminal/shell_type` is its companion: KAS asks the host once, at session setup, which shell to assume.

**Did NOT fire this turn (situational, despite having bundle handlers):** `_kiro/fs/{delete,stat}` (the delete tool resolved in-process), `_kiro/openExternalUrl` (the agent "fetched" the URL with an in-process web tool rather than asking the host to open it), `_kiro/system/notify`, `_kiro/userInput`. So a host can drive KAS end-to-end with just the five rows above; the rest are opt-in/edge surfaces.

**Cyril impact:** cyril implements **none** of the `terminal/*` callbacks or `_kiro/terminal/shell_type` today, so it cannot host KAS shell unless it adds them (or deliberately omits the `terminal` capability and lets KAS run shell in-process). This is the same opt-in proxy-stage opportunity as fs, one layer up: owning `terminal/*` would let cyril audit/gate/translate every command KAS runs. Reproduced by `experiments/conductor-spike/probe-kas-callbacks-2.7.1.py`.

---

## KAS session-management + account methods (verified live)

The advertised `_kiro/session/*` methods all resolve, and the bundle handles several more than `initialize` advertises. Probed against a 1-turn KAS session:

| Method | Params | Result |
|---|---|---|
| `_kiro/session/history` | `{sessionId}` | `{updates:[], hasMore:false}` — paginated replay stream. **Empty even after a completed turn** (the live turn's updates weren't in the history store; likely needs a reload/cursor — flagged). |
| `_kiro/session/context` | `{sessionId}` | `null` when no context items attached (nullable). |
| `_kiro/session/export` | `{sessionId}` | `{success:true, filePath:"…/kiro-exports/kiro-session-<sid>.zip"}` — **writes a real `.zip` archive** of the session to disk and returns the path. |
| `session/list` / `_kiro/session/list` | `{}` | `{sessions:[{sessionId, cwd, title, updatedAt, _meta.kiro:{agentMode, createdAt}}]}` — **global** across all cwds (returned my earlier probe sessions); `title` derived from the first prompt. Both method names hit the same handler. |
| `session/fork` | `{sessionId, cwd}` | requires `cwd` — `{sessionId}` alone fails `-32602 Invalid params` (`cwd: expected string`). |
| `_kiro/session/compact` | `{sessionId}` | `{success:true}` — compacts (summarizes) the conversation. |

**Bonus methods handled by the bundle but not in `extensionMethods`** (worked when called directly):

- **`_kiro/account/getUsage`** `{}` → `{success, message, data:{planName, billingCycleReset, overagesEnabled, isEnterprise, usageBreakdowns:[{resourceType:"CREDIT", displayName, used, limit, percentage, currentOverages, overageRate, overageCharges, currency}], bonusCredits}}`. **This is credit/usage data on the wire** — the thing cyril has never had over ACP (credits were previously only readable from the on-disk session sidecar). A KAS-mode cyril can show a live credit gauge by calling this.
- **`_kiro/permissions/list`** `{sessionId}` → `{rules:[{capability, match:[…], exclude:[…], effect:"ask"|"deny", scope:"kiro", source:"kiro-scope"}]}` — the **Cedar permission ruleset** on the wire (filesystem ask with `~/.kiro/**` excluded, `fs_write` deny on `~/.kiro/settings`/sandbox-state, ask on `.git`/`.vscode`/`.kiro/agents`/`mcp.json`, etc.). Directly relevant to the "organizational permission policies" stage.
- Also handler-registered, unprobed: `_kiro/session/{delete,rename}` (destructive — not exercised), `_kiro/permissions/explain`, `_kiro/policy/check`, `_kiro/spec/getTaskStatuses`, `_kiro/knowledge`, `_kiro/codeIntelligence`.

---

## KAS hooks — the "kas-unified-hooks" engine (from the bundle)

KAS ships a full hooks engine (`dist/hooks/`: registry, matcher, executor, schema, triggers, actions, loaders, telemetry, `v2-platform-adapter`) whose source cites `.kiro/specs/kas-unified-hooks/design.md`. Its explicit purpose is to **unify three previously-separate hook dialects** onto one `HookTrigger` enum via `normalizeTriggerName()` — which is why IDE-only event names now appear on the CLI/KAS side. *(Read from the shipped `@kiro/agent` 0.3.224 type defs + JS; the ACP methods are statically present — live hook firing over the wire is unprobed.)*

**11 canonical triggers:** `SessionStart`, `Stop`, `PreToolUse`, `PostToolUse`, `PreTaskExec`, `PostTaskExec`, `UserPromptSubmit`, `PostFileCreate`, `PostFileSave`, `PostFileDelete`, `Manual`.

**Alias table (the §9 unification — authored name → canonical):**

| Authored | → Canonical | Dialect |
|---|---|---|
| `sessionStart` | SessionStart | IDE camelCase |
| `agentSpawn` | SessionStart | CLI alias (v1's old `AgentSpawn` folds in here) |
| `stop` / `agentStop` / `SessionEnd` | Stop | CLI / IDE / OpenPlugin |
| `userPromptSubmit` / `promptSubmit` | UserPromptSubmit | CLI / IDE |
| `preToolUse` / `postToolUse` | PreToolUse / PostToolUse | camelCase |
| `preTaskExecution` / `postTaskExecution` | PreTaskExec / PostTaskExec | IDE |
| `fileEdited` / `AfterFileEdit` | PostFileSave | IDE / OpenPlugin |
| `fileCreated` / `fileDeleted` | PostFileCreate / PostFileDelete | IDE |
| `userTriggered` | Manual | IDE |

There is **no** `Notification` / `PermissionRequest` / `WaitingForApproval` trigger — you can gate a decision in a `PreToolUse` hook but cannot be notified by a hook when the agent pauses for approval (that's a protocol/client concern; see roadmap CN1).

**Hook document** (standalone `.kiro/hooks/*.json` as `{version, hooks[]}`, or inline under an agent profile's `hooks` key; v2 reads `.kiro/hooks/*.json` only — `.kiro.hook` stays IDE/v1):

```jsonc
{
  "name": "string",            // required
  "description": "string?",    // optional
  "trigger": "string",         // any dialect spelling; normalized at load
  "matcher": "regex?",         // optional; empty/absent = always match
  "action": { ... },           // discriminated union (below)
  "timeout": 60,               // optional seconds; command action only; default 60
  "enabled": true              // optional; default true
}
```

**Action is a discriminated union on `type` — `command` is fully retained, `agent` is additive (the IDE "Ask Kiro" behavior), pick one per hook:**

```jsonc
// shell hook (classic CLI + IDE): spawns a subprocess, pipes HookInput JSON to stdin,
// honors `timeout` (default 60s)
{ "type": "command", "command": "string" }

// agent hook (from the IDE "Ask Kiro" action): no subprocess; splices `prompt` + the
// trigger-metadata JSON into the model, wrapped in <HOOK_INSTRUCTION> tags; `timeout` ignored
{ "type": "agent", "prompt": "string" }
```

**`HookInput`** (piped to a command hook's stdin / appended to an agent hook's prompt) is Claude-Code-shaped: `{ hook_event_name, tool_name, tool_input?, session_id, cwd, trigger, prompt }`.

**ACP surface (split):** the v2 Rust binary exposes only `_kiro/hooks/list` + `_kiro/hooks/didChange` (since ≥2.6.1; likely TUI↔backend, not confirmed on the cyril ACP wire). KAS implements the **full** set: `_kiro/hooks/{list, executeHook, triggerHook, sessionStart, cancel, didChange}`. `_kiro/powers/*` is KAS-only (absent from the v2 binary).

---

## Steering inclusion under KAS — `fileMatch` now works (against `openFiles`)

A real CLI↔IDE gap-closure. The v1/v2 Rust engine parsed steering `inclusion` frontmatter but ignored `fileMatch` entirely (the string isn't even in the binary) — all steering loaded unconditionally. **KAS implements it:**

- Frontmatter schema: `inclusion: enum["always", "fileMatch", "manual", "auto"]` + `fileMatchPattern: string | string[]`, validated ("fileMatchPattern required when inclusion is fileMatch").
- `matchDocsForFiles` glob-matches each fileMatch doc via **`minimatch(filePath, pattern, {dot})`** — workspace-relative, single-or-array patterns. So `fileMatchPattern: "components/**/*.tsx"` is honored exactly like the IDE.

**But it matches against `openFiles`.** The populate-steering node calls `getSteeringDocuments({ files: openFiles.length > 0 ? openFiles : undefined })`, and the fileMatch lookup runs only `hasFiles ? getMatchedDocuments(filePaths) : []`. So:

- **IDE + KAS:** open editor tabs supply `openFiles`, so fileMatch steering triggers — the gap is closed in practice.
- **Bare ACP CLI + KAS:** no open files → `hasFiles` is false → the fileMatch lookup is skipped → only `inclusion: always` docs load (effectively the v1/v2 behavior). The feature is *implemented but dormant* for lack of input, not unimplemented.

(Same `openFiles`/`activeFile` session state also drives spec mode's `activeFile` logic via `minimatch`.)

**Cyril impact / TODO:** cyril is a chat TUI with no editor "open files," so against KAS today fileMatch steering never fires. To light it up, cyril must **synthesize an `openFiles`/`activeFile` set** (from `@`-attached/referenced files, recently-touched files, or cwd) and feed it to KAS via the `_meta.kiro`/document channel. This is the smallest change that turns on a class of IDE-parity behavior (conditional steering, spec `activeFile`) without cyril reimplementing those features — the engine already does them; it just needs the input. Tracked as **ROADMAP KAS-6**.

---

## Cyril impact

- **Passive compatibility:** unchanged. Cyril's default `kiro-cli acp` (v2 engine) wire shape is unaffected; the entire KAS surface is opt-in behind `--agent-engine kas`.
- **KAS is adoptable now, not "when it lands."** Cyril can spawn `kiro-cli acp --agent-engine kas` today and bypass the TUI's "V3 not supported" gate entirely. Doing so unlocks: the `_kiro/*` extension namespace, real `session/fork` + `session/list`, populated `configOptions` (including `mode` and `autopilot`, which cyril could surface in its toolbar/pickers), and 7 modes.
- **Two dialects to support.** KAS uses `_kiro/*`; the v1/v2 engines use `_kiro.dev/*` (now `_kiro/*` in tui.js per 2.7.0, but the backend ACP methods cyril handles are still `kiro.dev/*` / `_kiro.dev/*`). A KAS integration needs a parallel converter arm in `convert/kiro.rs` keyed on the engine.
- **Subagent display changes** (now verified live — see the KAS subagent wire-format section). Under KAS, subagents are `tool_call`s with `_meta.kiro.kind:"agent-subtask"` grouped by `agentSubtaskId`, **not** `kiro.dev/subagent/list_update`. Cyril's `SubagentTracker`/crew panel see nothing on this path until they group by `agentSubtaskId`.
- **`configOptions` populated** and `session/set_config_option` is a working SET (verified) — cyril can drive `mode`/`autopilot` from its toolbar/pickers, reading the set *response* (no `config_option_update` echo fires) and constraining `value` to advertised option ids (invalid values silently coerce).
- **Hooks become a real ACP surface (reverses prior guidance).** The standing "don't implement hooks; they're backend-only" stance holds for v1/v2 but not KAS — KAS exposes `_kiro/hooks/{list,executeHook,triggerHook,…}`, so cyril can observe/list/trigger hooks (and the unified command+agent action model spans IDE+CLI authoring). A prime composable-stage opportunity; direction (client-manages vs server-fires) still to probe.

---

## Not verified this session (follow-ups)

- `_kiro/session/history` returning empty after a live turn — confirm whether a reloaded/persisted session or a pagination cursor surfaces the updates.
- The destructive/unprobed bonus methods: `_kiro/session/{delete,rename}`, `_kiro/permissions/{explain}`, `_kiro/policy/check`, `_kiro/spec/getTaskStatuses`.
- A **clean v2 baseline re-capture on 2.7.1** — the default engine did not respond to the piped init+session/new probe in this session (KAS did); v2 baseline here is taken from `docs/kiro-2.7.0-wire-audit.md`, not freshly re-captured.

---

## Reproduce

```sh
# Download + verify (manifest at prod.download.cli.kiro.dev/stable/latest/manifest.json)
curl -o kirocli-2.7.1.tar.xz \
  https://desktop-release.q.us-east-1.amazonaws.com/2.7.1/kirocli-x86_64-linux.tar.xz
sha256sum kirocli-2.7.1.tar.xz   # f8d22bf104a74f50875503fd6425b10952155c1e7a09b8c1a4f4f3cdc0746ec6
tar xf kirocli-2.7.1.tar.xz      # binaries under kirocli/bin/

# The gate (interactive TUI) vs the working ACP path
kirocli/bin/kiro-cli chat --v3                       # -> "V3 is currently not supported for your system" (mod.rs:5847)
printf '%s\n%s\n' \
 '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{}}}' \
 '{"jsonrpc":"2.0","id":1,"method":"session/new","params":{"cwd":"/tmp","mcpServers":[]}}' \
 | kirocli/bin/kiro-cli-chat acp --agent-engine kas   # -> init + session/new succeed

# KAS bundle self-extracts here on first run; read the .d.ts for authoritative schemas
ls ~/.local/share/kiro-cli/kas/node_modules/@kiro/agent/dist/{graphs,nodes,tools,bundled-agents}
cat ~/.local/share/kiro-cli/kas/node_modules/@kiro/agent/dist/tools/orchestrate-subagent/types.d.ts

# Full authenticated turn that forces subagent orchestration and captures the wire.
# Self-sources the bearer token from kiro's own auth store and answers
# _kiro/auth/getAccessToken with {accessToken, expiresAt, profileArn}; the secret
# never leaves the subprocess. Refresh first if idle: `kiro-cli whoami`.
python3 experiments/conductor-spike/probe-kas-subagent-2.7.1.py
#   -> logs/probe-kas-subagent-2.7.1.log : subagents arrive as tool_call with
#      _meta.kiro.kind="agent-subtask", grouped by agentSubtaskId (NOT list_update)
```

Binary archived at `~/.local/share/kiro-research/binaries/2.7.1/`.

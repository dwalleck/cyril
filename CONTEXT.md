# Cyril

Cyril is a polished terminal interface for the Agent Client Protocol (ACP) ecosystem: one TUI that drives any registered ACP agent, with composable proxy stages adding behaviors no agent ships natively. This file is the project's glossary — the canonical name for each domain concept. It is not a spec; implementation lives in code, direction lives in [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Language

### Interface status

**Context usage**:
The percentage of an agent session's available context window that has been consumed. Higher values mean less context remains.
_Avoid_: context remaining, context left

**Usage snapshot**:
One point-in-time aggregate of the whole usage log — overview, provider/model/folder/agent rollups, tools, context, recent and errors — computed by a single read and rendered as a unit. Every rollup in one snapshot describes the same instant, so figures on different pages always reconcile; a snapshot may be older than the newest recorded turn, but is never a mixture of two instants.
_Avoid_: usage stats, usage data, the usage numbers

**Refresh trigger**:
An event that requests a new usage snapshot — a recorded turn, a context sample, a sidecar enrichment, or opening the panel. A trigger requests; it does not itself compute, and several arriving together yield at most one further snapshot.
_Avoid_: refresh event, update tick, reload

### Presentation

**Semantic theme**:
A named mapping from semantic color roles to concrete colors (e.g. `cyril-dark`) that widgets consume; widgets never choose colors directly.
_Avoid_: palette, color scheme, skin

**Color mode**:
A terminal's color capability tier — truecolor, ansi256, ansi16, or none. The selected theme is projected into the color mode; theme and capability are separate axes.
_Avoid_: color depth, theme (when you mean capability)

### Agent orchestration

**Subagent**:
A child agent session that performs delegated work and has its own activity stream.
_Avoid_: worker, child process

**Optimistic subagent stream**:
A message stream created on first contact for a scoped session nothing has yet named, so no frame is lost while the session's identity (a subagent list update, or a workflow claim) is still in flight.
_Avoid_: phantom stream, unknown-session stream

**Crew**:
A named orchestration group containing subagents and pending stages.
_Avoid_: subagent list, team

**Crew member**:
A subagent assigned to a crew. A pending stage is not yet a crew member.
_Avoid_: pending stage

**Pending stage**:
Planned crew work that has not yet started a subagent session.
_Avoid_: waiting member, queued subagent, stage (unqualified)

**Pipeline stage**:
One node of a KAS agent-subtask DAG: named, role-tagged, dependency-ordered work the agent orchestrates within a turn.
_Avoid_: stage (unqualified), workflow step (that runs as a peer session, not under a DAG tool call)

### Workflows

**Recipe**:
The definition of a workflow — a named plan of nodes with declared inputs, before any execution of it exists.
_Avoid_: workflow (unqualified), template, workflow file

**Workflow run**:
A workspace-scoped, persisted execution object identified by a workflow id. It outlives the session and process that watched it, can be listed/resumed/re-attached, and may contain successive retry incarnations under the same id.
_Avoid_: workflow (unqualified), job, pipeline, execution attempt

**Run incarnation**:
One execution attempt within a workflow run, from `run_start` through a `run_complete` whose status is terminal (`completed`/`failed`/`aborted`); a `run_complete` with status `paused` is non-terminal and the run stays resumable. Kiro retry starts a fresh incarnation under the existing workflow id; Cyril's canonical current state retains only the latest incarnation.
_Avoid_: workflow run, retry run, attempt (unqualified)

**Workflow step**:
A node of a run that executes as a peer session rather than as delegated work under a parent.
_Avoid_: subagent, stage, pipeline stage (that is the DAG-tool-call model)

**Node pause**:
The immediate, node-scoped suspension of one workflow node, with its node reason. It does not imply that the workflow run has reached its resumable paused summary.
_Avoid_: run pause, workflow pause (ambiguous), termination

**Run pause**:
The resumable, non-terminal state of a workflow run after the current execution settles. It summarizes the run and may follow a node pause or arise without one, such as repeat exhaustion.
_Avoid_: node pause, completion, termination

**Claim**:
A workflow event (`node_start` or snapshot-borne node state) naming a step's session id, binding that session to a workflow node. Only claims make a session workflow-owned — per-frame metadata never does.
_Avoid_: registration, announcement, session binding

**Late claim**:
A claim arriving after the claimed session's frames have already begun streaming. The dominant observed ordering on 2.16.0, not an edge case: routing must re-parent already-received history, not merely tag future frames.
_Avoid_: out-of-order claim, race (unqualified)

**Workflow-owned**:
The property of a session id currently claimed by any workflow node's state. A routing input, computed from tracker state at classification time; it persists through run termination so straggler frames stay attributed.
_Avoid_: workflow session (ambiguous with the parent), step-tagged

**Re-parent**:
Moving an optimistic subagent stream — messages, order, and activity intact — into the workflow store when a late claim lands, leaving no subagent stream keyed by a workflow-owned id.
_Avoid_: migrate, transfer, drop-and-recreate

**Attach**:
Reading a persisted run's current state (`inspect`) into the tracker so status renders locally — read-only, acquiring nothing: the engine's ownership of the run is untouched and no execution starts. Resuming is the separate, ownership-taking act.
_Avoid_: subscribe (no such wire verb), follow, reattach-and-resume (conflates two verbs)

**Run ownership**:
The engine-side exclusive claim on a run's execution, held by one process via a pid-stamped heartbeat (`run.beat`; beat interval × 4.5 = stale, dead pid = stale immediately). A live foreign owner refuses `resume` naming its pid; an abandoned run lists as `paused` after the engine's sweep. Cyril never bypasses this — it surfaces the refusal verbatim.
_Avoid_: lock (it expires), lease (the client renews nothing), busy (says nothing about who owns it)

### Sessions & turns

**Session**:
One conversation context with an agent, identified by a `SessionId`. A single agent subprocess can host several at once.
_Avoid_: chat, conversation, thread

**Peer session**:
A session running alongside the main session as an equal rather than as delegated work under it; KAS workflow steps run as peer sessions.
_Avoid_: sibling session, secondary session, subagent (delegated work with a parent)

**Permission approval**:
An operator decision requested by one session before the agent may perform a gated action. Concurrent approvals retain their originating session and are presented in arrival order; only the current approval accepts input.
_Avoid_: confirmation (unqualified), approval slot, permission prompt

**Session mode**:
The agent-side operating mode of a session — Kiro's vibe/spec axis, KAS's plan / bug-fix / quick-spec family. An axis of the session, not of the engine.
_Avoid_: mode (unqualified)

**Turn**:
One round of agent activity: from a submitted prompt until the agent stops and control returns to the user.
_Avoid_: exchange, round-trip, response

**Turn-end**:
The signal that a turn has completed. Which wire event carries it differs per engine, so only the bound engine may declare a turn over.
_Avoid_: prompt response (v2's carrier, not the concept), end of stream

**Turn owner**:
The per-turn identity (`TurnId`) allocated when the bridge accepts a prompt, never reused for the life of the agent subprocess. Terminals the bridge synthesizes itself are stamped with the owner; wire terminals are identity-free and resolved by session.
_Avoid_: turn id (when you mean the concept rather than the type), sequence number, session (not per-turn unique)

**Companion terminal**:
Under KAS every turn ends with two terminal signals — the wire turn_end and the synthesized prompt response. Whichever arrives second is the companion: an expected signal to absorb, recording its `{source, reason}` evidence, not a duplicate to drop. At most one companion expectation is outstanding at any time.
_Avoid_: duplicate completion (it is expected, not anomalous), echo

**Turn mediation**:
The bridge policy deciding what each inbound notification does to the active turn — forward it, release the turn, absorb a companion, or drop it (stale or unowned). Owned by the `TurnMediator` state machine inside the bridge; distinct from the bridge's host-callback mediation (ADR-0004), which gates the opposite direction.
_Avoid_: mediator (unqualified — collides with the host-callback mediator), dedup (absorption records evidence; only the stale/unowned cases drop)

**Turn liveness**:
Whether the active turn is showing signs of progress: the time since the last inbound frame scoped to the turn's session (or global), with the clock parked while cyril owes the agent a host-callback reply. Owned by the `TurnLiveness` state machine inside the bridge; time is an input, never read internally.
_Avoid_: heartbeat (Kiro sends none; context_usage merely behaves like one), timeout (nothing expires — the turn stays open), health check (no probe is sent)

**Stalled turn**:
An active turn whose liveness clock has passed the stall threshold (default 30s) with nothing outstanding — reported to the UI as a session-scoped `TurnStalled` notification, at most once per quiet period. A stalled turn is still a live turn: it can complete minutes later (cyril-bh7g captured one finishing after 16), so a stall is information, never a terminal.
_Avoid_: dead turn, timed-out turn, hung (the engine process is alive), failed (no failure has been observed)

**Quiet period**:
A maximal span of an active turn with no qualifying inbound traffic. One `TurnStalled` fires per quiet period that exceeds the threshold; resumed traffic ends the period and re-arms the signal.
_Avoid_: gap (reserved for the inter-frame measurements in the bh7g analysis), silence window

### Agents & engines

**Vendor**:
An agent provider selectable from the ACP registry — Kiro, Claude, Codex, Goose, and others. The unit the agent picker and registry reason about.
_Avoid_: provider, backend (when you mean the vendor), agent (reserve "agent" for the running process)

**Agent**:
The running agent process cyril drives over ACP — for Kiro, one `kiro-cli acp` subprocess. A vendor ships it; an engine is the implementation inside it.
_Avoid_: assistant, bot, model (the LLM an agent calls)

**Engine**:
A Kiro-internal agent implementation — **v2** (the Rust engine, `kiro.dev/*` / `_kiro.dev/*` wire dialect) or **KAS** (the TypeScript/LangGraph engine, `_kiro/*` dialect). Engine is an axis *within* the Kiro vendor: both engines share the `kiro-cli` binary, the `~/.kiro` auth store and session storage, and Kiro's slash-command/mode heritage, differing mainly in wire dialect and lifecycle. Cyril binds one engine at agent-subprocess spawn (startup): the bridge runs one `kiro-cli acp [--agent-engine kas]` process and holds one engine for its life, so every session on that process shares it. Switching engines means a new subprocess.
_Avoid_: mode (that's a session mode — vibe/spec), version (v2/v3 are engines, not release versions), variant

**v2**:
The Kiro engine cyril drives today (`kiro-cli acp`, default). Rust, `sacp`-based, `kiro.dev/*` dialect.
_Avoid_: rust engine, classic, legacy

**KAS** (Kiro Agent Server):
The Kiro TypeScript/LangGraph engine, embedded as of kiro-cli 2.7.1, reached over `kiro-cli acp --agent-engine kas`. `_kiro/*` dialect; host supplies auth; can call fs/terminal callbacks; uses the `agent-subtask` subagent model.
_Avoid_: v3 (it's the user-facing TUI alias `--v3`, but the engine is KAS), TypeScript engine

**Wire dialect**:
The extension-method family an engine speaks — `kiro.dev/*` for v2, `_kiro/*` for KAS.
_Avoid_: protocol (that's ACP itself), extension namespace

**Persona**:
The client identity cyril presents to the agent at the handshake; agent-side behavior (system-prompt voice, feature briefings) keys off it.
_Avoid_: impersonation, client name, identity (unqualified)

### Host integration

**Bridge**:
The single mediation point between cyril and the agent subprocess. It owns the ACP connection for the subprocess's life; every message in either direction crosses it.
_Avoid_: proxy (that's a proxy stage), protocol thread

**Host callback**:
A server-to-client ACP request or control notification through which the running agent asks Cyril, acting as the host, to provide a decision or capability such as permission, authentication, file I/O, terminal control, or hooks.
_Avoid_: client callback, host request (excludes control notifications), tool call

**Host-callback mediation**:
The bridge seam every handled host callback crosses (ADR-0004 amendment): KiroClient parses the ACP payload into a typed callback, the `HostMediator` state machine **accepts** it — registering its lifecycle in channel order, before any work — and its **resolution** runs concurrently off the loop against the capability adapter set, with a failing callback's user-visible notification enqueued before the agent sees the error. Acceptance is ordered; resolution is not. A family with no adapter is refused at parse time and never crosses.
_Avoid_: mediator (unqualified — collides with the turn mediator), interception (that's a proxy-stage concern layered on this seam), dispatch (reserve for the adapter-side resolve step)

**Capability adapter (adapter set)**:
The bound engine's declaration, as data, of which host-callback families it installs — Auth, Host I/O (file + terminal), Hooks (`Engine::adapters()`, ADR-0001 amendment). Inbound capability advertisement is derived from the set, and a family with no adapter is refused with JSON-RPC method-not-found — never answered with the protocol-default null. One set is bound per agent-subprocess lifetime.
_Avoid_: capability sub-trait (the withdrawn `as_*` accessor design), responder (that's the code answering a callback, not its availability), feature gate (cargo features gate what links; adapters gate what answers)

**Fs dialect**:
The file-operation callback family in force for one operation — Kiro's extended `_kiro/fs/*` or bare ACP `fs/*`. Selected per operation, not per session.
_Avoid_: fs protocol, fs mode

**Agent location**:
Which side of the Windows/WSL filesystem boundary the spawned agent process lives on — **native** (same filesystem as cyril; path translation off) or **wsl** (inside a WSL distro; paths translate at the boundary). Resolved once per spawn from the *resolved* spawn command (the argv actually spawned, after engine resolution — not the CLI `--agent-command`), overridable via `CYRIL_AGENT_LOCATION`. Moot on non-Windows hosts, where translation is always a no-op.
_Avoid_: agent platform, agent OS (the boundary is a filesystem fact, not an OS version), translation mode (names the effect, not the fact), WSL mode

**Hook generation**:
One of KAS's two hook execution models — hooks run by cyril as host callbacks, or by the agent's own registry. A session gets exactly one; they do not compose. Cyril-side, the Hooks capability adapter names its side of the bidirectional `_kiro/hooks/*` surface a **direction**: **Inbound** (cyril serves list/execute/sessionStart host-side), **Outbound** (agent runs its own registry; cyril only advertises `{enabled, v2}`), or none.
_Avoid_: hooks mode (cyril's config knob), hooks v2 (a wire flag, not a name), bidirectional adapter (say which direction)

**Approval preview**:
The stable, request-time view of the tracked tool call a permission request refers to, joined only by exact session and tool-call identity. A missing, malformed, cross-session, or out-of-order join is shown as unavailable while the approval choices remain actionable.
_Avoid_: tool-call cache, live preview, permission content

### Proxy platform

**Proxy stage**:
A composable layer on the path between cyril and an agent that observes or transforms ACP traffic to add behavior the agent lacks — skills, audit, policy, memory.
_Avoid_: stage (unqualified), middleware, plugin, interceptor

# Cyril

Cyril is a polished terminal interface for the Agent Client Protocol (ACP) ecosystem: one TUI that drives any registered ACP agent, with composable proxy stages adding behaviors no agent ships natively. This file is the project's glossary — the canonical name for each domain concept. It is not a spec; implementation lives in code, direction lives in [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Language

### Interface status

**Context usage**:
The percentage of an agent session's available context window that has been consumed. Higher values mean less context remains.
_Avoid_: context remaining, context left

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
One execution of a recipe, identified by a workflow id. A run is a workspace-scoped, persisted object: it outlives the session that started it and the cyril process that watched it, and can be listed, resumed, and re-attached to later.
_Avoid_: workflow (unqualified), job, pipeline

**Workflow step**:
A node of a run that executes as a peer session rather than as delegated work under a parent.
_Avoid_: subagent, stage, pipeline stage (that is the DAG-tool-call model)

### Sessions & turns

**Session**:
One conversation context with an agent, identified by a `SessionId`. A single agent subprocess can host several at once.
_Avoid_: chat, conversation, thread

**Peer session**:
A session running alongside the main session as an equal rather than as delegated work under it; KAS workflow steps run as peer sessions.
_Avoid_: sibling session, secondary session, subagent (delegated work with a parent)

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

### Proxy platform

**Proxy stage**:
A composable layer on the path between cyril and an agent that observes or transforms ACP traffic to add behavior the agent lacks — skills, audit, policy, memory.
_Avoid_: stage (unqualified), middleware, plugin, interceptor

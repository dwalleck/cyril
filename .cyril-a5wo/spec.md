# Feature: Verify and recover interrupted KAS tool calls

## What this is

Cyril will capture one real KAS subagent tool-call interruption from the archived kiro-cli 2.16.2 binary and use that frame sequence to verify its protocol-to-UI state transitions. When recovery frames reuse a tool-call identifier, Cyril will maintain one committed call, preserve fuller fields, and finish the visible call rather than leaving an in-progress spinner.

## Users

- **Cyril protocol maintainer**: runs the bounded live probe, inspects the redacted ACP sequence, and needs evidence tied to the exact 2.16.2 binary.
- **Cyril UI maintainer**: reads the state-level regression fence and needs repeated or partial frames to produce deterministic, non-duplicated state.
- **Cyril operator**: watches an interrupted subagent turn and needs the transcript to show one call with a terminal state and no permanently active spinner.

## Behavior

### Live 2.16.2 subagent capture
- **Given**: the archived KAS-enabled kiro-cli 2.16.2 binary is available and authenticated.
- **When**: the probe launches a subagent, injects `session/cancel` while that subagent tool call is still receiving arguments, and records the ACP traffic.
- **Then**: a committed, credential-scrubbed JSONL capture contains the interrupted subagent tool-call sequence, including the reused tool-call identity and the terminal/recovery frames needed by the state fence.

### Bounded live failure
- **Given**: a fresh authenticated probe attempt does not produce the required subagent mid-arguments sequence.
- **When**: three fresh attempts have completed.
- **Then**: the run records the negative result and remains blocked; no synthetic or top-level-only trace is presented as satisfying the live capture criterion.

### Same-identifier recovery
- **Given**: a committed tool call exists for `(session_id, tool_call_id)` and a later start or update uses the same identifier.
- **When**: Cyril applies the recovery frame.
- **Then**: the existing committed entry is updated in place, the committed entry count for that identifier remains one, and no earlier entry is orphaned.

### Partial-field preservation
- **Given**: the existing call has a non-empty title, content, locations, or complete raw input and a recovery update omits or supplies only a partial value for one of those fields.
- **When**: Cyril merges the update.
- **Then**: fields absent from the update and fuller existing raw input remain unchanged; explicitly supplied terminal status and non-empty replacement fields still apply.

### Late terminal update
- **Given**: an interruption causes turn completion before a terminal tool-call update arrives.
- **When**: Cyril applies the late update for the existing identifier.
- **Then**: the committed entry reflects the terminal status, the active-call index no longer reports it as in progress, and the UI has no stuck spinner for that call.

### Partial display input
- **Given**: a tracked call has missing, partial, malformed, or non-string raw input.
- **When**: the display helpers render its primary path or command text.
- **Then**: the helpers return a safe absence/fallback result without panicking or treating an incomplete value as a guaranteed complete command/path.

## Success criteria

- **Live evidence**: 1 committed credential-scrubbed JSONL capture from kiro-cli 2.16.2 containing a subagent mid-arguments interruption, measured by frame inspection; at most 3 fresh attempts are used.
- **Recovery identity**: 1 deterministic state test proves the repeated identifier produces exactly 1 committed tool-call entry, measured by committed-message count.
- **Field preservation**: 1 deterministic merge test proves a partial recovery update retains the pre-existing complete raw input and other omitted fields, measured by exact field equality.
- **Terminal recovery**: 1 deterministic UI-state test proves a late terminal update leaves 0 active entries and 0 in-progress indicators for the interrupted identifier, measured after the sequence is applied.
- **Display safety**: 1 deterministic shape test covers missing, partial, malformed, and non-string raw input for both display helpers with 0 panics and explicit fallback/absence results.
- **Existing behavior**: all workspace tests, formatting checks, and clippy checks pass with 0 failures after the change, measured by the repository's documented Cargo commands.

## Edge cases and decisions

| Edge | Decision | Rationale |
|---|---|---|
| No live binary or authentication | Record the missing prerequisite and remain blocked; do not claim AC1. | Live evidence is the acceptance boundary. |
| Required partial-argument sequence absent | Stop after three fresh attempts and preserve the negative result. | The requester selected an honest blocked outcome over a synthetic substitute. |
| Top-level cancellation only | Treat as supplemental evidence; it does not satisfy the subagent capture criterion. | The 2.16.2 release note names subagent recovery. |
| Empty raw input | Display safe absence or title fallback; never panic. | Empty arguments are a valid interruption shape. |
| Missing raw input field | Preserve existing fields during merge and use display fallback. | Recovery updates may be status-only. |
| Partial raw input | Preserve the fuller existing value unless the update is an explicit complete replacement. | Prevents an interruption frame from clobbering usable display data. |
| Malformed or non-string raw input | Return safe absence/fallback and log only where the existing API represents an unexpected failure. | Wire data is untrusted and display must remain total. |
| Duplicate start for an existing identifier | Merge into the existing entry in place. | A recovery frame reopens the same logical tool call. |
| Update after turn completion | Apply it to the committed entry and clear its active state. | The terminal frame can arrive after the turn boundary. |
| Unknown identifier update | Preserve current behavior unless the live sequence proves a required routing case; do not create an orphan entry from an update alone. | An update without a start has no stable committed destination. |
| Multiple sessions with the same tool-call identifier | Key state by originating session and tool-call identifier. | Identifiers are only unique within a session. |
| Concurrent subagent updates | Apply each frame in arrival order on the owning stream. | The UI state machine receives ordered notifications per stream. |
| Credential-bearing capture fields | Scrub secrets before commit and retain only protocol evidence. | The capture is repository data and must not expose credentials. |

## Out of scope

This change does NOT include:

- Fixing the separate v2 security-filter bridge hang in `cyril-w0vy`.
- Adding support for a new ACP `ToolCallStatus` enum variant that the current schema rejects before conversion.
- Reproducing the same interruption on a top-level call as a second required live scenario.
- Broad redesign of tool-call rendering beyond partial-input safety and the recovery sequence.
- Changing the Kiro binary or upstream KAS behavior.

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| Live probe attempts | ≤ 3 fresh attempts | Probe log and committed findings |
| Capture provenance | Exactly one 2.16.2 subagent capture required for AC1 | Binary path/version and frame inspection |
| State identity | 1 committed entry per `(session_id, tool_call_id)` | Deterministic state test |
| Terminal state | 0 active entries after terminal recovery | Deterministic state test |
| Secret exposure | 0 unredacted credentials in committed capture | Scrubber audit and pattern scan |
| Compatibility | Existing tests and rendering behavior remain green | Cargo test, fmt, and clippy commands |

## Decisions log

| # | Question | Decision | Why |
|---|---|---|---|
| 1 | What evidence satisfies AC1? | A live KAS run from archived kiro-cli 2.16.2 with actual mid-turn `session/cancel`; synthetic/replayed frames do not satisfy it. | The issue asks for behavior against the fixed engine, not a model-only fixture. |
| 2 | Which live scenario is required? | A subagent tool call interrupted while arguments are incomplete. | 2.16.2's release note specifically names subagent recovery mid-arguments. |
| 3 | What happens if live reproduction fails? | Stop blocked after three fresh attempts and leave AC1 unsatisfied. | The requester chose honest negative evidence over a substitute. |
| 4 | How does a repeated tool-call identifier behave? | Merge into the existing committed entry in place; preserve fuller fields and do not append. | The recovery frame represents one logical call. |
| 5 | Who relies on the result? | Protocol maintainer, UI maintainer, and Cyril operator are the named roles. | Each role has a distinct observable need. |

## Sign-off

Agent summary: this change will use at most three fresh, authenticated live probes against archived kiro-cli 2.16.2, requiring a subagent tool call interrupted while arguments are incomplete; if that exact sequence does not appear, the run remains blocked. The runtime behavior will treat repeated `(session_id, tool_call_id)` frames as one call, preserve fuller fields through partial updates, apply late terminal updates, clear active state, and keep display helpers safe for incomplete raw input.

The requester agreed: "Live 2.16.2 only; merge same ID in place; subagent mid-arguments; stop blocked if the required live sequence cannot be produced; three attempts."

Date: 2026-08-09

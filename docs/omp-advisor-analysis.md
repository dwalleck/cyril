# oh-my-pi Advisor ("watchdog") — implementation analysis and cyril feasibility

*Analyzed 2026-07-28 against the oh-my-pi checkout at `~/repos/oh-my-pi` (advisor module last touched 2026-07-28). Companion doc: [`omp-tui-takeaways.md`](omp-tui-takeaways.md).*

oh-my-pi (omp) ships an **advisor**: an optional second model attached to a session that
reviews the primary agent's transcript after each turn, investigates the workspace with
its own read-only tools, and injects concise advice back into the primary session. This
doc records how it works and what a cyril equivalent would take.

**Verdict up front: very feasible in cyril.** Every load-bearing primitive exists on the
ACP wire today; the interrupting-advice channel is exactly `_session/steer`, and cyril's
K1 steering support has already shipped (`BridgeCommand::SteerSession`, `/steer`,
Enter-while-busy, `SteeringQueued`/`Consumed` conversion for both engines). The advisor is arguably the flagship use case for the proxy-stage
vision in CLAUDE.md ("transcript audit", "multi-client observers"), and cyril's
vendor-neutral position adds something omp structurally cannot: an advisor that is a
*different vendor's agent* than the primary.

## Source map (omp)

Everything lives in `packages/coding-agent/src/advisor/` (~2,300 lines) plus session
integration. Design doc: `docs/advisor-watchdog.md` (thorough; read it first).

| File | Role |
|---|---|
| `advisor/runtime.ts` (1,232 ln) | Delta cursor into primary transcript, backlog queue, background drain loop, failure policy, output quarantine |
| `advisor/advise-tool.ts` | The `advise` tool; severity model; delivery-channel resolution (`resolveAdvisorDeliveryChannel`) |
| `advisor/emission-guard.ts` | Code-level noise/dedupe/rate gate on advice emission |
| `advisor/watchdog.ts` + `advisor/config.ts` | `WATCHDOG.md` / `WATCHDOG.yml` discovery and roster parsing |
| `advisor/transcript-recorder.ts` | `__advisor.jsonl` persistence (cost attribution, observability) |
| `session/session-advisors.ts` | Multi-advisor lifecycle; constructs each advisor `Agent` |
| `session/agent-session.ts` | Turn-end hook (`#advisors.onPrimaryTurnEnd`, ~line 1045); steer/aside/preserve plumbing |
| `prompts/advisor/system.md` | The reviewer system prompt (worth reading verbatim) |

## Architecture

### Observation loop

1. **Turn-end hook.** `AgentSession` calls `onPrimaryTurnEnd(messages, willContinue, signal)`
   after every primary turn. In-process method call — omp owns the agent loop.
2. **Delta rendering.** `AdvisorRuntime` keeps a cursor into the primary transcript
   (message count + content fingerprints so compaction/clones don't desync it) and renders
   *only the new messages* as markdown — **including thinking, tool intent, and tool
   results** — so the advisor reviews reasoning, not just visible text. Re-injected
   boilerplate (plan-mode rules) dedupes to a `(unchanged — still in effect)` marker.
   Secrets are redacted before bytes reach the advisor model.
3. **Async drain.** Deltas queue into a backlog; a background drain loop prompts the
   advisor agent. The primary **never blocks on the advisor**, except an optional
   `advisor.syncBacklog` bounded catch-up wait (≤30 s when backlog ≥ threshold).
4. **Failure policy** (defensive throughout): retry a failed advisor prompt; after 3
   consecutive failures drop the backlog and continue; hard-halt on permanent errors
   ("model not found"); a failing advisor never parks the primary.
5. **Reset semantics.** Any primary-transcript rewrite (compaction, session switch,
   branch/fork, re-prime) resets the advisor: clears its private context, rewinds the
   cursor, clears the emission guard so it may legitimately re-raise old issues.
   Mid-session enable seeds the cursor at the current transcript end (no full replay).

### The advisor agent

A full `Agent` instance with its own model (`modelRoles.advisor`, per-advisor override),
its own tool session (id suffixed `-advisor` — shares no file snapshots or edit state
with the primary), default tools `read`/`grep`/`glob` plus `advise`. `WATCHDOG.yml` can
declare a roster of named advisors (each with own model, tool grant, specialization
prompt); `WATCHDOG.md` files (user + project levels, `@` imports) append review
priorities to the system prompt.

The system prompt's stance (load-bearing for quality):

- "Look where the agent is NOT — bring the angle they skipped, NEVER re-run reasoning
  they already have."
- Prefer silence; at most one `advise` per update; never repeat advice.
- Lane = correctness, edge cases, design, process. Never advise on user intent, never
  police scope/ambition, never raise backwards compatibility unsolicited.
- Cite only transcript evidence or tool output personally inspected; hidden arguments
  are UNKNOWN — never assert concrete values for them.
- Low-confidence bar applies only to concrete technical risk; vague unease → silence.

### Advice channel: three gates, then severity routing

`advise(note, severity?)` with `severity ∈ {nit, concern, blocker}` (omitted = nit):

1. **Tool-level dedupe** — severity-rank aware: escalation (nit→concern→blocker) passes,
   verbatim repeat at equal/lower rank is dropped.
2. **`AdvisorEmissionGuard`** — born from production failure (omp #3520: one session
   logged 309 advise calls — 114× "Stop.", 52× "No issue; continue." — flooding the
   primary transcript). Normalize (lowercase, NFKC, punctuation-fold) → suppress a
   noise-phrase allowlist ("lgtm", "done", "on track", …) → session-scoped dedupe
   (4096-entry FIFO ring) → **one accepted note per advisor update**. Suppression is
   invisible to the advisor (tool still returns "Recorded.") so the model can't learn to
   rephrase around the filter. Guard resets with the advisor.
3. **Delivery-channel resolution** (`resolveAdvisorDeliveryChannel`, pure function):

   | Situation | Channel |
   |---|---|
   | `nit` | non-interrupting aside, batched at next step boundary |
   | `concern`/`blocker`, primary streaming | steer into the live turn (may abort in-flight tools at next steering boundary) |
   | `concern`/`blocker`, idle mid-work (no terminal answer) | trigger a fresh primary turn |
   | late `concern` after a terminal answer, no queued work | **preserve** as visible card — don't wake the agent to restate "done" (omp #4840) |
   | late `blocker` after a terminal answer | still steers a triggered turn — broken work was handed off (omp #5628) |
   | after a **deliberate user interrupt** | preserve; never auto-resume a run the user stopped |
   | post-interrupt cooldown (`immuneTurns`, default 3) | further interrupts downgrade to asides |
   | plan mode / ACP client that can't represent agent-initiated turns | preserve |

Advice lands in the primary transcript as:

```xml
<advisory severity="concern" guidance="weigh, don't blindly obey">
XML-escaped note text
</advisory>
```

The primary's system prompt **never mentions advisories** — the `guidance` attribute is
the entire behavioral contract. Advice is data, not authority.

### Safety and observability

- **Output quarantine** (`quarantineAdvisorUnsafeOutput`): advisor turns that call
  ungranted tools or generate output-only destructive directives ("ignore previous
  instructions", `rm -rf` patterns) are rewritten to a sanitized error *before dispatch*
  — with a provenance check so quoting a dangerous command from the transcript isn't
  punished, only originating one.
- **Never a peer**: excluded from the hub roster, unmessageable, unkillable from collab,
  regardless of granted tools.
- **Own transcript file** `<session>/__advisor.jsonl`: cost attribution (`omp stats`)
  and read-only inspection (Agent Hub), independent of the advisor's in-memory context.
- Advisor cost is separate model usage, surfaced via `/advisor status`.

## Cyril feasibility

The one structural difference: **omp is the agent runtime; cyril is an ACP client.**
omp's hooks are in-process calls; cyril must assemble the same loop from wire
primitives. The mapping is surprisingly complete:

| omp capability | cyril equivalent | status |
|---|---|---|
| Turn-end hook | `TurnCompleted` (prompt response, `EndTurn`) | exists |
| Transcript delta incl. thinking | cyril already receives every `AgentMessageChunk`, `AgentThoughtChunk`, tool call/update | exists (thought chunks Anthropic-only under Kiro) |
| Advisor agent + model | a **second ACP agent session** — separate spawned `kiro-cli acp`, or any other registered ACP agent | primitives exist |
| Read-only tool enforcement | cyril answers `session/request_permission` — auto-deny mutating tools on the advisor connection | clean, client-enforced |
| `advise` tool | can't inject a tool into an arbitrary ACP agent; either an MCP server cyril exposes, or structured-output parsing of the advisor's final message | needs design; MCP route is robust |
| Non-interrupting aside | prepend `<advisory>` block when composing the next `session/prompt` | trivial |
| Interrupting steer | `_session/steer` (Kiro 2.7.0+, both engines, wire-verified) | **shipped** — K1 landed (`SteerSession`/`ClearSteering` bridge commands, `/steer`, Enter-while-busy) |
| Trigger a turn while idle | cyril sends `session/prompt` itself | exists |
| Abort in-flight tool | only `session/cancel` (whole turn) | degraded but acceptable |
| Emission guard | pure Rust state machine, same pattern as `SessionController` | trivial port, very testable |
| Cost attribution | advisor session's own `metadata` / sidecar metering | exists per session |

Notes:

- **The steering dependency is already satisfied.** K1 shipped: an advisor `concern`
  *is* a `BridgeCommand::SteerSession`. K1's finding that Kiro's model treats steers as
  advisory and can decline matches omp's "weigh, don't blindly obey" framing exactly —
  no new wire surface is needed for interrupting advice.
- **Separate advisor process sidesteps a known footgun.** One v2 connection can run
  parallel turns but cyril's busy-guard is global
  (`reference_kiro_v2_per_session_agent_switch`). A dedicated `kiro-cli acp` process for
  the advisor avoids touching that and isolates context/metering — the moral equivalent
  of omp's `-advisor` tool session.
- **Vendor neutrality falls out for free and is a differentiator.** The advisor can be
  any ACP agent: Kiro primary with a Claude Code advisor, or vice versa. A genuinely
  independent second opinion is something no single-vendor agent ships natively — the
  CLAUDE.md proxy-stage thesis verbatim.
- **New build surface:** a transcript recorder + markdown delta renderer in `cyril-core`
  (today the committed transcript lives in `UiState`, unreachable from core — the
  recorder wants to be core-side, fed by the same notification stream), the advisor
  runner (spawn, prime, feed deltas, parse advice), the emission-guard port, and TUI
  advice cards.

### Phasing sketch

1. **MVP:** separate advisor ACP process; turn-end delta feed; advice parsed from a
   structured block in the advisor's final message; aside-only delivery (visible TUI
   card + folded into next prompt); emission guard from day one. Steering is available
   but deferring it keeps the MVP's blast radius small.
2. **Steering delivery:** `concern`/`blocker` route through the existing
   `SteerSession` path; adopt omp's delivery matrix (terminal-answer preservation,
   user-interrupt suppression, immune turns).
3. **Polish:** MCP-based `advise` tool; `WATCHDOG.md`-style advisor guidance files;
   multiple advisors; cost display.
4. **Proxy-stage version:** when the stage layer exists, the advisor becomes a stage
   usable by any client, not just cyril's TUI.

MVP scope is on the order of 1,000–1,500 lines across the crates; omp's 2,300-line
module includes polish cyril wouldn't need initially (advisor context
promotion/compaction, coalescing rounds, headless drain deadlines, output quarantine).

### Copy wholesale, don't re-derive

Four pieces encode production failure modes omp already paid for:

1. The **emission guard** design (noise filter + invisible suppression + one-per-update).
2. The **`<advisory>` framing** — severity attribute, "weigh, don't blindly obey",
   XML-escaped body, no system-prompt coupling.
3. The **delivery matrix's interrupt semantics** — especially "never auto-resume a run
   the user deliberately stopped" and the #4840/#5628 terminal-answer pair.
4. The **system prompt stance** — prefer silence, look where the agent is not, lane
   discipline, evidence-only claims.

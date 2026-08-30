# KiroCrew ACP seam — findings for cyril

**Investigated:** 2026-08-29/30 · **Target:** `github.com/kirodotdev/KiroCrew` @ `53fd2257f` (main)
**Method:** 12 parallel subagents over non-overlapping scopes — ACP client core, dispatch/wire types,
runtime/spawn/auth, session handle, liveness, KAS modules, provider abstraction, the TypeScript
adapter, the `test_acp_*` suite (two agents), git history, and GitHub issues.

Companion to [`kirocrew-deep-dive.md`](kirocrew-deep-dive.md) (2026-08-04), which listed the ACP seam
as a blind spot: *"claude-agent-acp launch/auth contract (the first artifact the vendor-neutral track
needs)."* This is that targeted read.

## Why this source is worth trusting

KiroCrew is built by Kiro team members and occupies **cyril's exact architectural position**: a client
driving `kiro-cli acp` over JSON-RPC stdio. Its ACP seam is ~20,000 lines of Python (`client.py` alone
is 6,059) plus 33 `test_acp_*.py` files — roughly 4× cyril's protocol layer. Crucially, its code
comments cite kiro-cli's own Rust internals by symbol (`kiro_tool_identity_meta`,
`acp_agent.rs → SessionManagerRequestData::TerminateSession`, `auth/mod.rs UnifiedBearerResolver`),
which cyril's team reverse-engineered from binaries.

**Confidence tags used below:**

- `VERIFIED` — I checked cyril's source directly this session; citation given.
- `FIRST-PARTY` — asserted in KiroCrew code/spec with a file:line citation.
- `MEASURED` — KiroCrew ran a live probe and recorded numbers.
- `INFERRED` — reasoning, not assertion.

---

## 1. Executive summary

**Four things change cyril's plans:**

1. **The KAS turn stall has a first-party answer, and it is not "wait forever".** Kiro's own team
   synthesizes turn terminals on silence — in production, in multiple places, with `client.py:4292`
   literally logging `"Treating as complete."` The transferable part is the five constraints that make
   it survivable (§3.3), plus the `session/cancel` **liveness probe** (§3.2) that converts silence into
   evidence.
2. **KAS is reachable with a two-flag argv change and no auth work at all** —
   `kiro-cli acp --agent-engine v3 --auth-method cli`. AWS abandoned the `node acp-server.js` route
   cyril researched (§6.1).
3. **`kiro-cli chat --list-models --format json` is a complete model catalog with real context windows
   and credit multipliers, available with zero ACP round-trips.** This closes ROADMAP KAS-4 by the side
   door (§5.1).
4. **The harness-parity invariant set (H1–H15) is a ready-made ruleset for cyril's vendor-neutral
   refactor**, written by the team that owns the protocol, and mechanically enforced in CI (§7).

**Cyril is *ahead* of the first-party consumer on the watchdog axis.** KiroCrew's history contains zero
references to `stream_stall_notice`, `StreamIdleTimeoutError`, `-32000` as a stream-idle code,
`turn_end`, or `capturedOutput`. Their newest ACP work is 2026-08-29. Cyril's 2.19.0–2.20.1 research
has no first-party counterpart, and `cyril-srp6` (workflow `{{id.output}}` corruption) is **genuinely
novel** — no first-party consumer exercises the KAS workflow surface at all.

---

## 2. Verified against cyril's source

Several agent claims did not survive checking. Recorded here so they are not re-litigated.

### 2.1 Real gaps confirmed in cyril

| Finding | Evidence in cyril | Severity |
|---|---|---|
| `session/load` omits `_meta._kiro.dev/session_file` — kiro-cli **silently ignores** the request without it | `bridge.rs:1965` sends bare `LoadSessionRequest::new(...)`; `_kiro.dev/session_file` absent repo-wide | **High** — resume appears to succeed and does nothing |
| Inbound `_meta.kiro.toolName` / `mcpServerName` never read on the v2 tool-call path | `toolName` only in `usage/kiro_sidecar.rs:921` (on-disk JSONL) and a KAS `_kiro/permissions/explain` fixture; `mcpServerName` **zero** hits in `crates/` | **High** — the only non-forgeable tool identity |
| `todo_list` never consumed; `SessionUpdate::Plan` decoded but never sent by kiro-cli | `convert/mod.rs:434` handles `Plan`; `todo_list` appears only in `docs/` | **Medium** — a shipped feature silently missing |
| Approval-wait time is charged to the backend as agent silence | `turn_liveness.rs:85` counts only `HostMediator::in_flight()` (fs/terminal callbacks, `cfg(kas)`); a pending `session/request_permission` is not in that table | **Medium** — false `TurnStalled` on every approval |
| `--list-models` never used as a data source | appears only in archived changelog JSON under `docs/` | **Medium** — free model catalog unused |
| Agent stderr captured but not fed to liveness | `bridge.rs:642` `append_stderr_reason` is error-enrichment only | **Medium** — an available evidence channel |

### 2.2 Claims that did NOT survive verification

Cyril's implementation is repeatedly ahead of what its own docs imply.

- **`--agent-engine` spelling.** Agents flagged `v3` as new. Cyril already resolves it from the
  installed version: `protocol/kas/version.rs:32` — *"kiro-cli 2.8.0 renamed `--agent-engine kas` → `v3`;
  2.7.1 accepted `kas`."* Cyril is **ahead** here; KiroCrew hardcodes `v3` and would break on 2.7.1.
- **`session/terminate`.** Already sent — `bridge.rs:2359`; KAS's `_kiro/session/delete` is in a test
  fixture.
- **`_meta.kiro.settings` placement.** Cyril already gets it right:
  `protocol/kas/settings.rs:3` documents `clientCapabilities._meta.kiro.settings` at `initialize`.
  Only cyril's *memory note* was abbreviated.
- **Consumer-park accounting and independent stall timer.** Both already solved:
  `turn_liveness.rs:1-13` counts host-side work as activity and deliberately polls at tick rather than
  from an event feed (*"the drain task is not loop-visible"*). Cyril independently arrived at two of
  the four hazards KiroCrew documents.
- **Dropped permission requests (KiroCrew's worst incident, a 2-hour wedge).** Structurally unreachable
  in cyril: `client.rs:212-266` forwards **every** permission request to `permission_tx` with no
  session-ownership filter, and failure paths return a JSON-RPC error rather than hanging.
- **Concurrent approvals.** `state.rs:1487` uses `approvals.push_back(...)` on a `VecDeque`, so
  kiro-cli's batch gate (several simultaneous requests) queues correctly.
- **Crate boundary.** Only `cyril-core` declares `agent-client-protocol`; four files outside
  `protocol/` reference `acp::`, all inside `cyril-core`. KiroCrew's equivalent seam has **68 direct
  import edges across 42 files**, some reaching into private functions (`acp.liveness.socket_inodes`).
  Their own RFC verdict: *"switching agent backends is not a driver swap; it is an edit across the
  whole tree."*

---

## 3. The stall problem

Cyril's #1 open issue. This is the section that matters most.

### 3.1 "The stall" is at least five different bugs

Cyril models it as one phenomenon. It is not:

| # | Mechanism | Signature | Detectable? |
|---|---|---|---|
| 1 | **Security-filter tool interrupt** | agent emits exactly `Tool uses were interrupted, waiting for the next user prompt`, then goes idle forever with no prompt response | **Deterministic** — exact stripped string match |
| 2 | **Compaction-failure abandonment** | `_kiro.dev/compaction/status` = `failed`, then no response and no `end_turn`, ever (their issue #3583) | **Deterministic** — key off the failed status |
| 3 | Tool dispatched, never resolves | tool in flight, no frames | evidence-based |
| 4 | Model-wait wedge | text streamed, then silence | evidence-based |
| 5 | Cancel never acked | `session/cancel` sent, no `cancelled` stopReason | 10s grace |

Two of these are cheaply pattern-matchable *today*. Cyril's `TurnStalled` chip treats all five as one
probabilistic condition.

Note on #1: kiro-cli's built-in, non-overridable security filter cancels every tool use in a turn
(triggered by e.g. shell commands containing "credentials"). KiroCrew matches the marker by **exact
stripped equality**, deliberately, so it does not fire when the model merely quotes the string in prose.

### 3.2 `session/cancel` as a non-lethal liveness probe — the key idea

Cyril's rule *"never synthesize a turn end on silence"* is correct but leaves the user stuck forever.
The missing third option is: **don't guess, ask.**

```
idle past threshold + evidence says not-working
  → send session/cancel (bounded 5s) as a PROBE, not a termination
      ├─ ack arrives (stopReason: cancelled)
      │     → the turn was really done; the response frame was lost
      │     → RECLASSIFY to `stale_recover`, never surface as "cancelled by user"
      └─ no ack within 10s
            → CONFIRMED WEDGE (a done-but-missing-frame turn would have acked)
            → synthesize a labelled terminal
```

The reclassification is load-bearing (`session_handle.py:2284-2300`):

> *"kiro-cli acks `session/cancel` on a LIVE mid-generation turn too, so a probe-induced 'cancelled'
> must NOT surface as a user cancellation (the turn would die silently — the original session-killer).
> … **An oracle mistake therefore costs a regeneration, never a session.**"*

This works on **both engines**, needs nothing from 2.20.1, and cyril already sends `session/cancel` on
Esc — the transport exists.

### 3.3 They DO synthesize — and the five constraints are the deliverable

`FIRST-PARTY`, unambiguous. `client.py:4275-4300` ends a turn on 90s of silence with a bare `return`,
logging `"Treating as complete."` The spec document uses the same phrase. Note the polarity: **only a
`WORKING` verdict defers** — `DEAD`, `UNKNOWN`, probe failure, probe timeout, and no-`/proc`-at-all
(macOS/Windows) *all* end the turn. Their words: *"fail toward reaping, never toward hanging."*

The constraints that make it survivable:

1. **Never `end_turn`.** Every synthetic terminal carries a distinct reason —
   `stale_recover`, `error: tool stall`, `error: compaction failed`, `error: cancel unacked`, `timeout`.
2. **An empty stop reason is NOT neutral.** A synthesized terminal with `""` was misread one layer up as
   a timeout and escalated to a **hard kill of the shared runtime**, killing co-tenants. Fall back to a
   benign non-empty `end_turn`.
3. **Synthesis implies a mandatory session reset.** The backend still counts the turn as in progress;
   the next prompt collides with *"prompt already in progress."* The pre-fix bug: *"the original bare
   `return` abandoned the turn but left the kiro-cli child ALIVE mid-prompt, so the slot wedged and
   every later prompt hit 'Prompt already in progress' until the whole backend was killed by hand."*
4. **Drain leftover frames at the next turn's start** — the abandoned turn keeps emitting into an
   unbounded queue and contaminates the next transcript. Exception: permission **requests** are
   answered, never dropped.
5. **Bounded retries with a visible terminal state** — 3 attempts, separate budgets per mechanism, not
   reset on exhaustion, then `"Session stuck — please start a new chat."`

**Verdict for cyril:** the first-party position is not *"never synthesize."* It is *"synthesize, but
label it, reset the session, drain the queue, bound the retries, and make a wrong call cost one
regeneration."*

### 3.4 The liveness oracle — evidence instead of timeouts

`liveness.py` (931 lines, pure `/proc`, no dependencies) returns
`WORKING | DEAD | STUCK_INPUT | UNKNOWN` plus a free-text evidence tag. Policy contract: `WORKING`
never acts *at any elapsed time*; `DEAD`/`STUCK_INPUT` act immediately; **`UNKNOWN` is the only
timeout-governed class**, and its actions are non-lethal.

The model-wait branch is exactly cyril's KAS stall:

| Subtree state | Established TCP socket? | Verdict |
|---|---|---|
| CPU/IO moving | — | `WORKING` — defer unboundedly |
| Flat counters | **yes** | `UNKNOWN` tagged `established_flat` — probably a non-streamed think; **extend** window to 900s |
| Flat counters | **no** | **`DEAD`** — *"the done-but-lost-frame wedge signature"* |

Silence-with-a-live-connection and silence-with-no-connection are opposite states. Cyril's 30s chip
conflates them.

Evidence tags shift windows in **both** directions: `established_flat` *narrows* a tool's window
3600→900s but *extends* a model-wait's 300→900s; `shell_child_absent` narrows 3600→300s.

### 3.5 Threshold table

| Knob | Default | Measures |
|---|---|---|
| `check_after_secs` | 60s | idle before the oracle is consulted at all |
| `stale_window_secs` | 300s | UNKNOWN model-wait → safe-probe via `session/cancel` |
| `tool_stall_suspect_secs` | 3600s | UNKNOWN in-flight tool → cancel + nudge |
| `tool_stall_hard_cap_secs` | 3600s | absolute UNKNOWN ceiling; never applies to WORKING |
| `model_silent_probe_secs` | 900s | extended window for `established_flat` |
| `_CANCEL_GRACE_SECS` | 10s | cancel-ack budget |
| `_STALE_TURN_TIMEOUT` | 90s | `AcpClient` reap-on-silence (dedicated-process path) |
| `_TOOL_STALL_TIMEOUT` | 600s | `AcpClient` tool stall → kill process |
| `_COMPACTION_FAILED_TURN_BUDGET` | 60s | silence after compaction `failed` |
| `_DEFAULT_PROMPT_TIMEOUT` | 7200s | outer turn ceiling |
| dispatch tick | 5s | watchdog evaluation cadence |
| `_TURN_CEILING_WINDOW_FRACTION` | 0.9 | **every window clamped to 0.9 × prompt timeout** |

The 1-hour tool default is calibrated for **macOS, where the oracle degrades (no `/proc`)** — i.e. it is
the sanctioned no-evidence fallback. That is the regime cyril is in today.

### 3.6 Rules cyril should adopt before shipping any stall window

- **Clamp every window to 0.9 × the turn timeout.** Their test comment: *"A default above it silently
  disables the branch it governs on every install, which is exactly how the UNKNOWN class became dead
  code."*
- **Three nested layers, not one:** verdict-driven watchdog (60s–1h) → turn ceiling (2h) →
  unattended-caller bound (30m). The third exists because *"if the agent calls a non-allowlisted tool,
  the interactive-approval callback would block on the human-approval wait with no human present —
  wedging the whole subsystem."*
- **Two clocks, not one.** Tool clock = session-attributable frames only. Stale clock folds in a broader
  activity clock. A merged clock means either a reasoning burst masks a stall or a quiet tool trips one.
- **Ownerless frames must not feed the stall clock.** `_kiro.dev/subagent/list_update` carries **no
  `sessionId`** and is broadcast to every session; treating it as per-session activity silently
  refreshed every co-tenant's idle clock and *poisoned their entire stall investigation*. Gate on
  **provenance** (was this frame routed or fanned out?), not event kind.
- **TOCTOU guard the probe.** Snapshot delivery signals before an `await` that can take 10s, and recheck
  after; a frame can arrive by the queue *or* by a concurrent responder buffering it.
- **`_kiro/system/notify` must be handled unconditionally** — and must NOT count as activity, or the
  soft warn extends the very window it is warning about.

### 3.7 Their own architectural warning

KiroCrew has **two** ACP clients with opposite stall doctrines, and the divergence is the lesson:

- `client.py` owns a dedicated process it can kill → at 90s it synthesizes.
- `session_handle.py` shares a runtime across co-tenants → killing it kills other users' work → it
  **never** synthesizes blindly; it probes, reclassifies, labels.

The newer, safer design is the one that cannot use force. **Cyril is architecturally in
`session_handle`'s position** — which confirms cyril's instinct was right, and identifies which of the
two doctrines to copy.

Also: their watchdog lives in an async generator's timeout arm, so a consumer parked at the `yield`
freezes it for a whole turn — an incident where 611 journal lines contained **zero** stall warnings.
Their principle: *"A detector must not be downstream of the failure it detects."* They considered and
**rejected** a separate pump task, because a detector reachable during a human approval forms verdicts
about a state it does not model. Cyril's poll-at-tick design is already immune.

Naming trap: `docs/system-specs/modules/heartbeat.md` is a background **chore scheduler**, not a
liveness signal. There is no heartbeat/ping protocol with the agent; `session/cancel` is the only active
liveness signal on the wire.

---

## 4. New wire facts

### 4.1 Silent-failure footguns (highest diagnostic cost)

| Fact | Failure mode |
|---|---|
| `session/new` **must** carry `mcpServers`, even as `[]` | kiro-cli treats the missing field as malformed and **exits rc=0 with no stderr** |
| `session/load` **must** carry `_meta._kiro.dev/session_file` (abs path to `~/.kiro/sessions/cli/<sid>.json`) | silently ignored |
| `session/load` must use the **transcript's own** sessionId, not a fresh one | kiro-cli replays the old transcript onto a primed session and dies/refuses |
| `session/load` **re-initializes MCP servers** | an empty `mcpServers` is *applied*, un-pooling the session for life |
| `_kiro.dev/commands/execute` string form | rc=0, **no response** — per-command (`/compact`, `/help`); object form works for `/effort` |
| A slash command sent as **prompt text** | the model **summarizes** kiro-cli's output instead of returning it — a silent wrong answer |
| `session/set_model` with an unentitled id | kiro-cli **accepts** it; the service rejects mid-prompt with `-32603 ... model is not available`, every turn |
| Namespaced agent id at `set_mode` | `_kiro.dev/agent/not_found`, `Mode 'ns/name' not found`, **runtime killed at spawn**. Bare names only |
| `clientInfo.name` must be **nested** | a flat `clientName` is ignored; sessions bucket as `(none)` in telemetry |

### 4.2 Tool identity and content

- **`_meta.kiro.toolName` is the only stable tool identity.** The visible `title` is LLM-authored prose;
  for shell tools it is deliberately the model's own description. Any security gate keyed on `title` is
  forgeable.
- **`_meta.kiro.mcpServerName` is set ONLY for MCP-served tool calls** — the trusted "this came from an
  MCP server" discriminator. Emitter named as `kiro_tool_identity_meta` in kiro-cli's engine.
- **`plan` is never emitted.** The TODO list arrives as the `todo_list` **tool's** `rawOutput`, keyed on
  `_meta.kiro.toolName` (the title is prose like *"Creating task list: …"*). Every command echoes the
  **whole** list — always a full snapshot. `completed` is a plain bool; there is no in-progress state.
- **`__tool_use_purpose` and `__toolUsePurpose` are BOTH emitted, ~50/50** — measured 160 vs 184 in a
  single real session across the same tools. Models also paraphrase it (`__purpose`,
  `__thinking_purpose`, `__woohoo_purpose` all seen in real transcripts), so match by *shape*: dunder
  prefix, normalized suffix `purpose`.
- **kiro-cli tool titles are gerunds** (`Reading hi.md:1`, `Searching for '…'`), which defeats naive
  prefix matching. And **ACP's `search` kind is not `read`** — searches never auto-approve on kind.
- `rawOutput.items[]` is an externally-tagged Rust enum: `{"Text": …}` / `{"Json": …}`, with
  `Json.stdout` for shell. An MCP result is forwarded verbatim as a `Json` item, and re-serializing it
  with `json.dumps` **silently corrupts embedded markers while still looking correct to a human reader.**
- Chunk text nests under `content.text` since **2.10.0**; flat `text` is back-compat only. An
  `agent_message_chunk` whose `content.type` is `thinking`/`reasoning` is **reasoning, not visible text**.
- **2.16.0 announces a tool call early** (`tool_call_chunk` with `args:{}`) **but delivers arguments
  whole** — no argument deltas exist.

### 4.3 Turn, history, and error envelope

- **kiro-cli replays the full history every turn from a fixed index.** Any backend-rejected content is
  re-sent **forever** — their log shows one image failing 19 consecutive times. The only escape is
  discarding the native conversation and cold-starting. This generalizes past images to any poisoned
  conversation.
- Every mid-stream provider error is wrapped in **`"Encountered an error in the response stream: <real
  cause>"`**. The envelope is a transport wrapper, not a signal — classify what is *inside* it, or you
  discard the real cause.
- **≥2.16 reworded the capacity rejection to a nameless form** — *"The model you've selected is
  temporarily unavailable"* — which broke name-based classification and made transient blips read as
  terminal.
- `session/new`'s `models.currentModelId` is **best-effort**; the backend may advertise `availableModels`
  and omit it. On a default-model session it is the *only* source of the served model.
- `modes.availableModes` is three-valued: **omitted** = unknown, try anyway; **present but empty** = the
  backend offers none, fail closed.
- kiro-cli **advertises no reject option** on `session/request_permission` (per code; their newer spec
  disagrees — read the advertised options rather than assuming). Options arrive in two shapes: ACP-spec
  `{optionId, name, kind}` and kiro-cli's historical `{id, label}` with **no `kind`**.
- Reject outcome semantics are **not cosmetic**: `selected` + reject-id → `status:"failed"` /
  `"User denied tool execution"` / turn continues (`end_turn`). `cancelled` → turn ends **immediately**
  with `stopReason:"refusal"`, no text, and queued steers dropped. Therefore
  **`stopReason:"refusal"` is not by itself evidence of a model content refusal.**
- **Independent request-id namespaces.** An inbound `session/request_permission` id can collide with an
  in-flight `session/prompt` id; matching must also require `method is None`, or the permission is
  misread as the prompt's completion → early turn end + unanswered permission → stuck turn.
- **Answer every server→client request**, including unrecognized ones (`-32601`). A dropped *request*
  strands the backend's response oneshot. A session-less request must be answered **once at connection
  level** — broadcasting yields N responses for one id, a JSON-RPC violation.
- `session/request_permission` awaits with **no timeout**, and kiro-cli's batch gate holds every sibling
  tool in the same assistant turn. kiro-cli's own TUI answers even for unowned sessions —
  *"the client answering is the expected contract."*

### 4.4 Sessions, MCP, and startup

- `session/new` and `session/load` **block on MCP server initialization**. A pending-OAuth remote server
  holds the response for its **full 30s** wait; a 71-server agent takes ~14s clean. Their timeout is
  **90s** (`[90,900]` configurable) with a do-not-tidy warning. **A 30s client timeout is a race the
  client usually loses**, and the session is created server-side then orphaned.
- `_kiro.dev/mcp/server_initialized` / `server_init_failure` / `oauth_request` arrive **before** the
  `session/new` response (each carrying `params.serverName`; failures carry `error`). A client that
  registers its session route only after the response drops them.
- Agent-level `mcpServers` are **not** ignored in ACP mode, and `"disabled": true` is **not** honored.
- kiro-cli holds a **native per-session lock** at `~/.kiro/sessions/cli/<uuid>.json`. An unclean death
  strands it and the next `session/load` returns *"active in another process"* → empty completions.
  Their answer: drain (`session/cancel` + wait for turn-done ack **before** SIGTERM), then retry
  `session/load` 4× at 1/2/4s, then fall back to `session/new` + history replay.
- Transcripts at `~/.kiro/sessions/cli/{sid}.json` + `.jsonl` — **nothing else deletes them.**
- `_kiro.dev/session/terminate` is the **only** RSS reclaim on a multiplexed process; without it RSS
  grows unbounded (multi-GB after ~24h). KAS's `_kiro/session/delete` is **destructive** — it also
  removes the persisted record.
- **kiro-cli 2.15+ is a multi-call binary.** It dispatches subcommands by exec'ing a *sibling*
  executable (`kiro-cli-chat`) located relative to its own path. Never copy it, and never realpath it
  (a multiplexer dispatches on `argv[0]`) — either strands the sibling and ACP dies at the handshake.
- **Auth failures must be latched as stderr arrives, not re-scanned.** On 2.19.1 an expired IdC token
  produces `GetProfile failed: AccessDeniedException: "Invalid token" (HTTP 400)` and
  `Access denied: The bearer token included in the request is invalid.` — never the `not logged in`
  banner. A 20-line ring buffer loses the auth line on a chatty startup.
- **kiro-cli does not encrypt its tokens** — plaintext in a SQLite `auth_kv` table, protected only by
  `0600`. KiroCrew built an encrypted vault because *"it faces a threat kiro-cli does not: its own AI
  agent reads files on the same machine."* That threat model applies to cyril identically.
- **Robustness at the frame boundary:** response-frame `id` is agent-controlled and may be a **string**;
  an oversize frame does **not** corrupt the stream (the "must tear down" premise is provably false, and
  a prefix-only drain splits UTF-8 into a decode error that escapes the JSON guard); `usage_update` can
  carry JSON `NaN`/`Infinity` and arbitrary-precision ints.

### 4.5 Error classification, metering, and update-merge pins

From the `test_acp_*` suite — an executable spec written with kiro-cli source access.

- **The retryable 5xx set is exactly `500 / 502 / 503 / 504 / 529`.** `501 Not Implemented` is
  **terminal**. And a naive substring match on a bare number misfires:
  `"max_tokens 500 exceeds the model limit of 200000"` is a **terminal** error whose text contains
  `500`. Their matcher requires an `HTTP`/`status` anchor for exactly this reason.
- **`AcpError` carries a tri-state transient flag** — `None` (unknown) / `True` / `False`. Cyril's
  error type should model *unknown* distinctly from *terminal*, or an unclassifiable failure silently
  becomes non-retryable.
- **`meteringUsage[].unit` is an open enum.** Their parser tolerates non-credit rows (a
  `{"value": 5, "unit": "token"}` entry contributes nothing to the credit sum), and they log an
  unrecognized unit's **literal label** because *"`unit=cacheRead` is the discovery; `unit:str` conveys
  nothing."* If a `token` row ever appears it would be the first billing token count on the ACP surface.
  *(The fixture is hand-constructed, so this is what AWS's parser tolerates, not proof kiro emits it.)*
- **Compaction grace-drain ordering.** A credits-only `_kiro.dev/metadata` frame (no
  `contextUsagePercentage`) can arrive **between** the `completed` status and the real post-compaction
  usage frame, and must not terminate the drain — otherwise the real usage frame is stranded and the
  meter falls back to its reset state. So the ~1s post-compaction metadata is not necessarily the *next*
  frame; it may be the second.
- **Partial-update merge rules.** `status: "completed"` on a `tool_call_update` is what sets the
  final-result flag. A refinement may repeat `title` **without** resending `rawInput`, so the command
  must come from the cached initial params. A `tool_call_update` may omit `kind` entirely — a kind-less
  refinement must not clobber a cached `is_shell=true`. An `insert` edit with no `insertLine` derives no
  diff at all, deliberately: *"the hunk position would be a guess."*
- **Framing details.** Line-delimited `json.dumps(...) + "\n"` followed by `drain()`; a broken pipe on
  *either* the write or the drain is the process-death signal. `params: null` is a tolerated inbound
  shape. Repeated oversize-frame overruns must never accumulate into a process kill.
- **Caps:** `TODO_TASKS_MAX = 200`, `TODO_TEXT_MAX = 500`, tool-result truncation 4000 chars/part and
  8000 total, stdout frame limit 10 MB.
- **`STOP_REASON_REFUSAL = "refusal"`** is a first-class non-retryable stop reason. Cyril currently
  detects refusal via `kiro.dev/metadata` `stopReason == "CONTENT_FILTERED"` — a different channel.


---

## 5. Model and context accounting

### 5.1 `--list-models` is a free, complete catalog

`kiro-cli chat --list-models --format json` returns, with **zero ACP round-trips**:

```json
{"models":[{"model_name":"auto","model_id":"auto",
            "description":"Models chosen by task for optimal usage and consistent quality",
            "context_window_tokens":1000000,"rate_multiplier":1.0}],
 "default_model":"auto"}
```

Row shape: `{model_name, model_id?, display_name?, description?, context_window_tokens?, rate_multiplier?}`.

This closes **ROADMAP KAS-4**: the picker never has to wait for a late `configOptionUpdate`, and the
2.17.0 "transiently absent configOption" case stops mattering.

### 5.2 Context-window precedence (`FIRST-PARTY`, from `model_registry.py:21-29`)

```
usage_update.size  >  --list-models cache  >  static registry  >  [1m] heuristic  >  None
```

> *"There is no silent 200k default: a genuinely-unknown window returns None and callers substitute
> REFERENCE_WINDOW_TOKENS (1M), so an unknown model is never wrongly shrunk."*

Their TypeScript adapter contradicts this with a 200,000 default — **the Python side is right**.
Understating a window silently truncates the user's context; overstating it merely mis-scales a meter.
When you must guess, guess in the direction whose failure is cosmetic.

Note: **KiroCrew gets no context-bucket breakdown over ACP at all.** They reconstruct it by measuring
their own injected characters and inferring the rest at 4.0 chars/token, labelled "Not measured (est.)".
This partly contradicts cyril's note that KAS pushes 5 buckets via `session_info_update`.

### 5.3 Model-selection rules worth copying

- **Empty/unknown advertised set means ALLOW** — never "nothing is allowed".
- **Never compare model ids across vendor namespaces.** kiro advertises bare ids; the claude backend
  uses prefixed provider ids. One shared membership test calls every legitimate model unusable.
- **Substitute picks inherit; explicit picks raise.** A caller-chosen model that is unavailable must
  raise a dedicated non-retryable error — never silently swap.
- **An absent configOption is *unknown*, never *unsupported*.** `supports_config_option` returns `True`
  when no options have been reported yet, *"so that a backend which advertises options lazily (after the
  first turn) is not permanently treated as unsupported."* Three lines; directly fixes cyril's KAS-4.
- **`session/new`'s `availableModels` can be a transient degraded snapshot** — 2 free-tier models for a
  fully-entitled account, because the backend raced entitlement lookup against token/profile resolution
  at startup. The same binary + account advertised all 20 models 50 minutes later. Re-probe before
  refusing a pick.
- **Dotted vs dashed ids are different models with different windows** — `claude-opus-4.8` is 1M,
  `claude-opus-4-8` is 200K. A naive `.`→`-` fold conflated them (their #5339).
- **Never default `rate_multiplier` to 1.0** — the real served spread is 0.01x–4.4x, and kiro re-prices
  mid-life (GPT-5.6 Luna moved 0.6x → 0.1x in 2026-07). *"A guess can be wrong by 6x."*
- **Reset context state on model switch, `/new`, and compaction.** Three separate staleness bugs, one
  of which let a meter understate usage ~4× and **never** re-derive the new window.
- **`~/.kiro/agents/` is a shared directory** and other tools write non-string `model` fields into it —
  observed: `{"id": "anthropic:claude-opus-4-8"}`. It crashed their whole Agents tab. Cyril's config
  deserialization needs a tolerant `model` field, not a bare `String`.

### 5.4 `session/set_model` — cyril's docs are probably stale

`MEASURED` on kiro-cli **2.15.1**, raw JSON-RPC probe with no client code in the path:

- acked synchronously in **12.5 ms**
- conversation carried across the switch, **including across vendors**
- sticks over subsequent turns
- fired **mid-turn**, the in-flight turn is undisturbed and completes with `end_turn`; the new model
  takes effect from the next turn

And on 2.15.2: `auto` is advertised in `availableModels` and `set_model("auto")` returns `OK {}`.

Cyril's CLAUDE.md says `session/set_model` is *"behind unstable feature flag, not advertised in
capabilities"* and routes model changes through `commands/execute`. **Re-probe against 2.20.1 and
correct the note either way.**

---

## 6. KAS specifics

### 6.1 Launch: the relay route supersedes cyril's research

```
kiro-cli acp --agent-engine v3 --auth-method cli
```

> *"`cli` keeps token resolution inside the kiro-cli process, which already holds the OIDC refresh
> token. Without it the engine would expect its host to answer `_kiro/auth/getAccessToken`."*

kiro-cli's relay spawns KAS itself and **consumes the auth callback internally** — the frame never
reaches the ACP client. AWS explicitly abandoned the direct `node acp-server.js` route (which cyril
researched) because it *"made Crew depend on kiro-cli's internal on-disk layout."* Measured parity: the
relay forwards complete NDJSON frames byte-for-byte and advertises **two extension methods the direct
route did not** (`_kiro/sourceProviders/list`, `_kiro/sourceProviders/listResources`) — a superset.

They also state it explicitly rather than relying on the default, *"because the default is kiro-cli's to
change and a silent fall back to v2 would look like KAS working while serving a different agent
entirely."*

### 6.2 Handshake differences (hard blockers)

| | kiro-cli | KAS |
|---|---|---|
| `protocolVersion` | `"2025-08-22"` (string) | **`1` (integer)** |
| `clientCapabilities` | `ACP_CLIENT_CAPABILITIES` | + `_meta.kiro.settings: {}` |

> *"KAS validates this field against a numeric schema and rejects the kiro-cli date string with
> `expected number, received string`."*

One protocol version for both engines fails at `initialize` and **presents as a spawn or auth bug**.

KAS reads only `fs.readTextFile`, `fs.writeTextFile`, and `terminal` from the top level of
`clientCapabilities`; everything else it honours lives under `_meta.kiro`, and those are **callback**
capabilities. `settings` is specifically the feature-flag channel.

### 6.3 KAS semantics

- **KAS does NOT sandbox tool execution.** It implements seatbelt/bubblewrap, but kiro-cli's relay
  spawns it **without `--sandbox`**, and KAS's sandbox factory resolves an absent config to its no-op
  backend. Trap: kiro-cli self-sandboxes based on `argv[0]` basename being literally `kiro-cli` — a
  predicate that **matches the KAS spawn argv but is wrong for it**. Their membership set excludes KAS
  and notes *"this is the one membership test that fails OPEN."*
- **`_meta.kiro.customAgents` on BOTH `session/new` and `session/load`** is the client agent-injection
  path — a resumed session needs the same definitions. Capped at **50** (they read KAS's Zod schema).
  Registered agents surface as *modes*, activated by ordinary `session/set_mode`.
- **`tools` absent means NO tool access, not all tools** — KAS resolves it as `agent.tools ?? []`.
- **`permissions` absent means everything resolves to `ask`** — not "no policy". `match` absent defaults
  to `['**']`. Capability vocabulary: `mcp`, `web_fetch`, `web_search`, `subagent`, `skill`; MCP
  resources addressed as `<server>/<tool>`. *"Omitting the field is not the neutral choice it looks
  like."*
- `prompt` must be **non-empty** — KAS crashes session creation where kiro-cli tolerates `""`.
- **KAS folds into `session_info_update._meta.kiro` what v2 splits across top-level `_kiro.dev/*`
  methods:** `context_usage`, `turn_completion`, `summarization_*`, `steering_*`. On KAS those v2 frames
  never arrive.
- **`promptTurnSummaries` carries the whole turn's total** — ASSIGN, never accumulate, or a replayed
  frame double-counts credits. Only `unit == "credit"` entries contribute.
- KAS subagents are `tool_call`s tagged `_meta.kiro.agentSubtaskId` / `_meta.kiro.pipeline`, with titles
  literally prefixed **`"Sub-agent: "`**. There is no aggregated `list_update`.
- **The repeat-iteration ambiguity cyril identified is unfixed in first-party code.** They mitigate with
  a per-turn roster reset (symptom otherwise: *"duplicate spawn/done card"* and an unbounded roster),
  but a `repeat` loop runs *within* one turn, so iterations still accumulate.
- Model changes on KAS go through `session/set_config_option{configId:"model"}` — there is no
  `session/set_model`.
- `_kiro/auth/getAccessToken` response: `{accessToken, expiresAt}` required, `expiresAt` **must be
  > now + 3min**; optional `profileArn` (mandatory for enterprise/IdC; **its 4th ARN segment is the
  region source**), `authMethod`, `provider`. `provider` is a **governance discriminator** —
  `BuilderId | Google | Github | Enterprise | ExternalIdp | Internal` — and an empty value
  misclassifies. A single failed callback **fails the entire prompt.**

---

## 7. The vendor-neutral seam — harness parity

KiroCrew's `docs/system-specs/modules/harness-parity.md` is 15 numbered invariants, each closed by a
named test, enforced by a diff-scoped CI gate. This is the most transferable artifact in the repo for
cyril's ROADMAP.

The framing:

> *"**an added harness may only adapt itself to the seams the Kiro harness already runs through. It may
> not move, widen, generalize, or add a branch to those seams.** … The failure mode this file exists to
> prevent is not a broken adapter — that fails loudly on its own first session. It is the *silent
> capture* of the Kiro path."*

| Id | Invariant |
|---|---|
| **H1** | The default backend is selectable unconditionally; an operator who configures nothing and one whose config is unusable both get it |
| **H2** | The harness is chosen at one field; `provider` is never the harness selector |
| **H3** | An unknown/unselectable persisted value **degrades with a logged reason** — never raises, never survives |
| **H4** | Selectability has exactly ONE gate, and it logs |
| **H5** | **Identity is a positive comparison** against a named constant or set. Negations forbidden |
| **H6** | Capabilities are granted by **opt-in membership**, never by negation |
| **H7** | The sandbox-delegation predicate is a positive test — the one Group-B row that is also a security invariant |
| **H8** | New identifiers live in a **leaf** vocabulary module; capability sets are subsets of the known set |
| **H9** | The first-class harness keeps its own spawn-argv branch (no dict-of-builders refactor) |
| **H10** | **Protocol version and client capabilities stay per-harness literals** — pinned by a test asserting they are *unequal* |
| **H11** | The provider label is a closed mapping; an absent label means the default harness |
| **H12** | Model pre-flight keeps "empty/unknown advertised set means allow", and never compares ids across namespaces |
| **H13** | Harness support is additive at the registry seam; the default path gains no conditional |
| **H14** | Every capability the app layer reads is declared on the interface **with a safe default** — never a `hasattr` probe |
| **H15** | A capability that can be observed elsewhere must ratchet its **observability stamp** together with itself |

The rationale for H5 is the one to internalize: a negative test *"reads correctly with two harnesses and
then silently hands the third a capability, a sandbox waiver, or a session label nobody granted it —
**and it fails toward the permissive answer**, so nothing goes red until an operator who never opted into
that harness pays for it."* Two such sites shipped before the rule existed.

Their CI gate self-tests **first**, because *"a gate that has silently stopped matching reads as a green
signal, which is worse than no gate."* It is diff-scoped (added lines only) because a whole-tree gate
would charge a pre-existing backlog to whoever pushed next.

And the governance rule:

> *"Never relax a check to make a red invariant green. … If a harness genuinely cannot be adapted within
> these invariants, the correct outcome is that the harness does not land yet."*

### 7.1 What leaked in their seam (the cautionary half)

Their own RFC audits it as failing. The sharpest failure is one line —
`providers/base.py:30` aliases the ACP event type as the "provider-agnostic" one:

```python
from kiro_crew.acp.types import AcpEvent as LLMEvent  # noqa: F401
```

Consequence traced all the way out: **a raw JSON-RPC request id reaches the browser**
(`chat_runner.py:7487` ships `{"id": str(event.request_id)}`). A wire artifact became a UI contract.

Cyril is structurally immune to *that* bug — `cyril-ui` cannot name an `acp::` type — but the four
`acp::` sites in `cyril-core/types/` are the same shape in miniature, and are exactly where a wire field
would slip into `TrackedToolCall` and out to the renderer.

Their measured cost of adding **one** foreign host without a written host contract: 2 undefined `getattr`
seams, 6 override-only stubs, 11 dead isinstance guards, 19 prose-only contracts, 13 live branches, 146
symbol lines. And the failure modes were silent — *"a missing MCP override yielding a session with zero
tools, and a missing settings seed silently collapsing the context window from 1M to 200K."*

### 7.2 Write the host contract separately from the wire contract

> *"ACP defines the wire; this document defines the **host** — the filesystem layout, agent-definition
> format, session store, credential store, sandbox posture, MCP delivery channel, billing surface, and
> permission engine that sit around the wire and differ per backend."*

Eight buckets, and **"not supported" is a valid declaration** that degrades a surface rather than
assuming. *"Silence is not an answer."*

---

## 8. Version-pinned kiro-cli behavior

| Version | Fact |
|---|---|
| 2.10.0 | Chunk text nests under `content` (`{type,text}`); flat `text` is back-compat |
| 2.13 | Ships an internal agent sandbox (mutually exclusive with an outer macOS seatbelt); GPT effort uses the `reasoning` key, not `output_config` |
| 2.14.0 | `commands/execute` **string form → rc=0, no response** (`/compact`, `/help`); object form works |
| 2.14.0 | **Never emits ACP `plan`**; TODO rides the `todo_list` tool. `completed` is a plain bool |
| 2.14.0 | Compiles the `elicitation/create` schema (form + url) and capability-gates it, but does **not** route it over ACP — a stub MCP server gets `-32601` |
| 2.14.0 | `AskUserQuestion` does not exist — the string is absent from the binary |
| 2.15+ | **Multi-call binary** — execs a sibling `kiro-cli-chat` relative to its own path |
| 2.15.1 | `session/set_model` works live: 12.5 ms, carries the conversation across vendors, mid-turn safe |
| 2.15.2 | `auto` advertised in `availableModels`; `set_model("auto")` → `OK {}` |
| 2.16.0 | `tool_call_chunk` announces early with `args:{}` but delivers arguments **whole** — no deltas |
| ≥2.16 | Capacity rejection reworded to a **nameless** form, breaking name-based classification |
| 2.16.2/2.17.0 | Steering `inclusion:` front matter ignored on the ACP path — every `.md` under `.kiro/steering/` loaded (~865K tokens measured) |
| 2.17.0 | Namespaced agent id → `_kiro.dev/agent/not_found`, **runtime killed at spawn** |
| 2.17.1-nightly.7 | A pending-OAuth MCP server blocks `session/new` for exactly **30s** |
| **2.19.0** | **FIX:** stopped injecting manual and fileMatch steering by glob (closes the overflow class) |
| 2.19.1 | Expired-token stderr is `AccessDeniedException: "Invalid token"` — **not** `not logged in` |
| 2.19.1 (KAS) | A single 20s `_kiro/auth/getAccessToken` timeout **fails the entire prompt** |
| 2.20.0 | Re-measured: only `always` steering documents load |
| 2.20.1 | `spawn_run` can return a run id with **no child process created** — silent, no failure event |

---

## 9. Corrections to cyril's own docs

1. **`session/set_model` is "behind an unstable feature flag"** — contradicted by a live probe on 2.15.1
   and by KiroCrew calling it unconditionally in production. Re-probe.
2. **`configOptions` is "always `null` on the v1/v2 engine"** — KiroCrew parses `configOptions` from the
   `session/new` response on both backends and derives `/effort` levels from it. Probe.
3. **`SessionUpdate::Plan` is handled** — but kiro-cli never sends it. Dead code; the real channel is
   `todo_list`.
4. **`_meta.kiro.settings` memory note** should read `clientCapabilities._meta.kiro.settings` (the code
   is already correct).
5. **Probe isolation.** Cyril's rule (`HOME=<tmp>` + real `XDG_DATA_HOME`) is coarser than KiroCrew's,
   which pins `KIRO_HOME` as a **separate axis** because *"that directory is kiro-cli's own home, shared
   with the real installed agent."* A blanket `HOME=<tmp>` relocates `~/.kiro`, so the spawned kiro-cli
   finds an empty `~/.kiro/agents/` — meaning **past cyril probes of `--agent`, `set_mode`, or mode
   advertisement may have measured an artificially empty world**, and a `Mode 'x' not found` result may
   be an artifact of the isolation rather than a finding. Note the binding is captured at process start.
6. **Context buckets.** Cyril's note that KAS pushes 5 buckets via `session_info_update` is worth
   re-checking: the vendor's own client gets no breakdown and estimates it.

---

## 10. Ranked action list

**Correctness bugs (fix):**

1. Send `_meta._kiro.dev/session_file` on `session/load`, use the transcript's own sessionId, and
   re-declare `mcpServers`. *(silent no-op today)*
2. Raise the `session/new` timeout to ≥90s. *(intermittent orphaned sessions on MCP-heavy configs)*
3. Read `_meta.kiro.toolName` / `mcpServerName` in `convert/kiro.rs`; stop deriving tool identity from
   `title`.
4. Read both `__tool_use_purpose` and `__toolUsePurpose` (match by shape). *(~half of tool purposes are
   blank today)*
5. Exclude approval-wait time from the `TurnStalled` clock. *(false stalls on every approval)*
6. Guard `/model` picks against `session/new`'s `availableModels` — empty means allow. *(otherwise every
   turn fails with `-32603` after a bad pick)*
7. Make `session/new` assert `mcpServers` is always present, even empty.
8. Per-agent `protocolVersion` + `clientCapabilities` literals before any non-Kiro agent work.

**Features / capability (build):**

9. Render the `todo_list` tool's `rawOutput` as the plan view.
10. Ingest `kiro-cli chat --list-models --format json` at startup for the model picker and context
    windows; adopt the `usage_update.size`-first precedence chain and the 1M-not-200K unknown default.
11. Make an absent configOption mean *unknown*, not *unsupported* (closes KAS-4).
12. Reach KAS via `--agent-engine v3 --auth-method cli`; drop the `getAccessToken` work item.
13. Serialize `commands/execute` result `data`, not the lossy `message` stub.

**Stall work (design, then build):**

14. Feed agent stderr into `TurnLiveness` as an evidence channel.
15. Add the `session/cancel` liveness probe with `stale_recover` reclassification.
16. Add deterministic detectors for the security-filter marker and compaction-failure abandonment.
17. If synthesis lands: distinct non-empty stop reasons, mandatory session reset, pre-turn frame drain,
    bounded 3-attempt budget, 0.9× window clamp.

**Hygiene:**

18. A reflection test over `ToolCall` → `TrackedToolCall` and `Notification` → `UiState` that fails on
    any field neither forwarded nor explicitly listed as dropped.
19. Consider `#[serde(other)]` or a raw-value fallback on `SessionUpdate` — their dominant crash class
    was one malformed frame killing every consumer, and cyril's hard-failing enum is the same shape.

---

## 11. Filed issues

Everything actionable from this investigation is tracked. Pre-existing issues that already covered a
finding are listed so they are not re-filed.

**Filed 2026-08-30:**

| Issue | P | Finding |
|---|---|---|
| `cyril-levc` | 1 | Approval-wait charged to the backend → false `TurnStalled` (§2.1) |
| `cyril-fj6j` | 1 | `/model` picker has no entitlement guard → `-32603` every turn (§4.1, §5.3) |
| `cyril-caar` | 1 | `session/new` budget must be ≥90s (§4.4) |
| `cyril-hg2x` | 1 | Per-target `protocolVersion` / `clientCapabilities`; KAS needs integer `1` (§6.2, §7) |
| `cyril-qd8o` | 1 | `session/cancel` liveness probe + `stale_recover` reclassification (§3.2, §3.3) |
| `cyril-ybv8` | 2 | Both `__tool_use_purpose` spellings (§4.2) |
| `cyril-5onw` | 2 | Render `todo_list` as the plan view (§4.2) |
| `cyril-qy6v` | 2 | Ingest `--list-models` as the model catalog (§5.1–5.3) |
| `cyril-zbu7` | 2 | Reach KAS via the relay; retires the auth-callback item (§6.1) |
| `cyril-xdll` | 2 | Re-probe `session/set_model` + v1/v2 `configOptions` (§9) |
| `cyril-ss5i` | 2 | Pin `KIRO_HOME` as its own probe axis (§9) |
| `cyril-b976` | 2 | Render `commands/execute` `data`, not the `message` stub (§4.1, §4.3) |
| `cyril-3ck6` | 2 | Adopt harness-parity invariants for the vendor-neutral seam (§7) |
| `cyril-rsyq` | 2 | Poisoned-conversation escape hatch (§4.3) |
| `cyril-0dv4` | 3 | Assert `mcpServers` always present on `session/new` (§4.1) |

**Already tracked — not re-filed:**

| Issue | Covers |
|---|---|
| `cyril-rtrh` | `_kiro.dev/session_file` meta on `session/load` |
| `cyril-2yn8` | `_meta.kiro.toolName` / `mcpServerName` as trusted tool identity |
| `cyril-ai1y` | Unknown `sessionUpdate` variant hard-fail |
| `cyril-w0vy` | Security-filter interrupt marker → wedged `is_busy` |
| `cyril-14ou` / `cyril-0w3u` / `cyril-fvht` | Stall signalling and the session-less notify problem |
| `cyril-cxwb` | Lifting `config_options` from `session/new` (KAS half) |
| `cyril-0gke` | Agent stderr drain |

---

## 12. Provenance and gaps

**Covered:** `acp/client.py`, `_dispatch.py`, `types.py`, `runtime.py`, `session_handle.py`,
`liveness.py`, `worker_pool.py`, `session_provider.py`, `kas_*.py`, `prompt_blocks.py`,
`providers/acp.py`, `acp_backends.py`, `providers/base.py`, `website/src/providers/adapters/acp.ts` and
its tests, `docs/system-specs/modules/{acp-client,harness-parity,providers,kas-auth,kas-backend,heartbeat}.md`,
`docs/system-specs/features/agent-host-contract.md`, `docs/ci/harness-parity-gate.md`, `AGENTS.md`,
`CHANGELOG.md`, 165 ACP-touching commits, and ~2,266 GitHub issues (432 in `area: agents`).

**Complete.** All twelve scopes reported; the final test-suite agent added §4.5 and changed none of
the conclusions or the ranked list.

**Caveat on issue-tracker sourcing:** many "maintainer" comments in KiroCrew issues are posted by an
automated triage bot running under individual accounts. Statements about *KiroCrew's* internals are
code-verified; statements about *kiro-cli's* internals are observational unless the reporter ran a probe.
The deepest analyses (#2854, #3785, #1355, #2932, #3026, #4237) come from human reporters doing
first-party-quality forensics and are quoted preferentially above.

**Strategic note.** KiroCrew is moving onto cyril's turf: issue #6615 ("Pluggable agent backends —
Claude Code, Codex, opencode, Ollama via the ACP seam"), a Codex CLI harness (#6665), and an unmerged
26,250-line `feat/pluggable-acp-backends` branch. But it has **no TUI** — it is a gateway plus an
Electron dashboard, and its CLI chat surface has repeatedly shipped broken (#1666: `kirocrew chat` never
answered permission requests at all). Cyril's polished-TUI niche holds; "vendor-neutral ACP" alone no
longer differentiates it.

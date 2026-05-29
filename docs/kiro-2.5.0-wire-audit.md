# Kiro CLI 2.5.0 — ACP Wire Audit

**Analyzed:** 2026-05-28 · **Method:** same-day binary isolation vs archived 2.4.1 (backend held constant), plus live `kiro-tui` reference trace (`/tmp/trace.jsonl`).

**Verdict for cyril:** 2.5.0 adds **two real ACP-wire changes** (thinking chunks, subagent review-loop fields) and one client-originated telemetry method. Cyril already has partial thinking plumbing but **cannot currently enable thinking** (no `/effort` command) and has a **thought-rendering bug**.

---

## Methodology note (read first)

A subtle probe bug nearly produced the wrong conclusion. `_kiro.dev/commands/execute` for `model`/`effort` **must be sent as a JSON-RPC request (with `id`)**. Sent as a notification it is silently dropped — the session stays on `auto`, where effort options are empty and thinking never engages. My first-round probes sent model-execute as a notification and wrongly concluded "thinking is TUI-only / zero wire impact." The corrected probes (model + effort as requests) reproduce thinking against the public `acp` endpoint from any client. **cyril already sends these as `ExtRequest` (requests), so cyril is unaffected by the bug** — only the probe was wrong.

---

## 1. Thinking display → `agent_thought_chunk` (NEW, binary-introduced)

**Changelog:** *"Thinking display — see the agent's reasoning process in real time. Enabled by default, toggle via /settings > Display > Show thinking."*

**Wire fact:** thinking surfaces as the **standard ACP `agent_thought_chunk`** session update, streamed token-by-token:

```json
{"method":"session/update","params":{"sessionId":"…","update":{
  "sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":" I'll check"}}}}
```

**Binary isolation (same day, same backend, Opus 4.8 + effort=high, model/effort set as requests):**

| binary | `agent_thought_chunk` | `agent_message_chunk` |
|---|---|---|
| **2.5.0** | **35–71** | 51–53 |
| 2.4.1 | **0** | 58 |

2.4.1 emits **zero** thought chunks under identical Opus+effort config ⇒ the `acp` frontend forwarding of thinking is a **2.5.0 binary change**, not a backend rollout. Backed by new `agent::agent::agent_loop::types::ThinkingBlock` (stream parser) and `chat_cli::cli::chat::parser::ResponseParser::take_thinking` (v1 TUI).

**Preconditions to receive thinking (all required):**
1. Active model is a thinking model (Opus 4.6/4.7/4.8) — switched via a **request** (`commands/execute model`, await `"Model changed to …"`).
2. `effort` set to one of `low|medium|high|xhigh|max` via a **request** (`commands/execute effort`). Under `auto`, effort options are empty and thinking never streams.
3. `clientInfo.name` is **irrelevant** — `cyril` and `kiro-tui` both receive thought chunks. No vendor gate.

**Effort defaults (current backend):** Opus 4.7 → `xhigh [active]`; Opus 4.6 → `high [active]`; Opus 4.8 → no default marker. `_kiro.dev/metadata` carries `"effort":"high"` once engaged (absent under `auto`/non-thinking models).

## 2. Subagent review-loops → new `subagent/list_update` fields (NEW)

**Changelog:** *"Subagent pipelines now support review loops — a reviewer stage can send work back to an implementer for revisions automatically."*

**Tool schema (advertised via `_kiro.dev/commands/available` → `tools[].subagent`):** 2.5.0 adds a `LOOPS:` section absent in 2.4.1 — `loop_to` (target stage), `trigger` (text in output that fires the loop, e.g. `NEEDS_CHANGES`), `max_iterations` (cap).

**Notification schema — `_kiro.dev/subagent/list_update` per-entry keys:**

| | keys |
|---|---|
| 2.4.1 | agentName, dependsOn, group, initialQuery, role, sessionId, sessionName, status |
| **2.5.0** | …same… **+ `name`, `createdAtMs`, `hasLoop`, `loopIteration`, `loopMaxIterations`** |

Live loop run: a `checker` stage with `loop_to=writer, trigger=NEEDS_CHANGES, max_iterations=2` reported `hasLoop:true`, `loopIteration` incrementing `0→1`, `loopMaxIterations:2`. Backed by new Rust types `orchestration::types::{LoopConfig, LoopTriggerData}`.

`status` is unchanged — already an object `{type, message?}` in 2.4.1 (`working`/`terminated`/`awaitingInstruction`), so no break.

## 3. `_kiro.dev/telemetry/processHealth` (NEW method, TUI-only)

Client→server notification; the v2 TUI reports its own process stats (`rssMb`, `heapUsedMb`, `cpuUserPct`, `lastRenderMs`, `rendersPerMin`, `yoga…`). New in 2.5.0 binary (absent from 2.4.1). **Cyril N/A** — it's client-originated TUI telemetry; cyril neither sends nor receives it.

## 3b. Tool-trust batch auto-approval is CLIENT-side, not wire (verified)

Changelog: *"Trusting a tool now automatically approves all other pending invocations of the same tool in the current batch."*

Probed with `probe-trust-batch-2.5.0.py`: the agent fired **3 parallel `shell` permission requests** (echo alpha/bravo/charlie); the client answered only the first with `allow_always`. Result: the turn **did not complete** and the backend sent **no retraction** for the other two — it kept waiting on per-request responses. So the backend does **not** auto-resolve siblings at the ACP layer; the "trust auto-approves the batch" behavior is implemented **in the Kiro TUI client**, not on the wire.

**Cyril impact:** no correctness gap — there is no orphaned/stale state, because the backend never resolves a `request_permission` behind the client's back. It's at most an optional **UX-parity** feature: when the user picks "Always" for a tool, cyril *could* auto-answer other pending `request_permission` overlays for the same tool instead of making the user click each. Not required for correctness.

## 4. Unchanged

- Slash command set (identical 23 commands; `/reply`, `/rewind`, `/effort`, `/stats` all pre-existed in 2.4.1).
- Standard ACP method set; core `session/update` variants.
- `_kiro.dev/metadata` shape (the `effort` field already existed under thinking models).
- tui.js grew 12.10→12.16 MB (streaming-render rewrite + thinking UI; no wire impact).

## 5. Backend-attributed (not binary)

- Effort-option availability depends on the **active model being applied** (empty under `auto`); the levels themselves are stable (5 under Opus). My earlier "effort empty" reading was the model-switch-as-notification bug, not a backend change.

---

## Cyril action items

1. **`/effort` already works** — cyril does NOT hardcode it; `CommandRegistry::register_agent_commands` (commands/mod.rs:168, called at app.rs:271) dynamically registers every command Kiro advertises in `commands/available`, including `/effort` (non-local selection command → picker → `ExecuteCommand` request). So `/model` (Opus) + `/effort` enables thinking today. (My first pass wrongly claimed this was missing — only `model`/`compact` are *built-ins*; the rest are dynamic.)
2. **Fix `streaming_thought` accumulation (real gap)** — `cyril-ui/src/state.rs:273` does `self.streaming_thought = Some(thought.text.clone())`, overwriting on each token-delta, while `AgentMessage` (state.rs:263/266) uses `push_str`. `convert/mod.rs:355` passes the raw per-chunk delta (" I", "'m", " working"), so cyril currently renders only the **last token** of a thought. Must `push_str`; decide commit-vs-transient semantics (cleared at state.rs:587 without committing to scrollback).
3. **Surface new `subagent/list_update` fields** — `name`, `createdAtMs`, `hasLoop`, `loopIteration`, `loopMaxIterations` to display review-loop progress (e.g. "reviewer ↻ 1/2"). Verify `SubagentTracker` (subagent.rs) deserialization captures them.
4. Confirm `_kiro.dev/metadata` parser tolerates/uses the `effort` field (already flagged in 2.4.1 audit).

## Reproduction

`experiments/conductor-spike/probe-thinking-replicate.py` (model+effort as requests → counts `agent_thought_chunk`); `probe-subagent-loop-2.5.0.py` (loop pipeline → `list_update` field diff). Set `KIRO_BIN` to isolate binary axis.

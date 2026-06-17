# Kiro CLI 2.7.0 — ACP Wire Audit

**Analyzed:** 2026-06-12 (build date 2026-06-12, hash `6f3383d`) · **Method:** same-day binary isolation vs archived 2.6.1 with the backend held constant, `nm`+rustfilt symbol diff, method-string diff on self-validated `tui.js` carves, and five live raw-JSON-RPC probes against the cyril path (`kiro-cli-chat acp`, clientInfo `cyril`).

**Verdict for cyril:** **SAFE TO UPGRADE — no code changes required for passive compatibility.** 2.7.0 is the largest *additive* wire delta since 2.5.0, but every new surface is opt-in: nothing on cyril's existing path changed shape, and the new notifications only fire if the client invokes the new methods. Two genuinely attractive features (queue steering, `/goal`) are now available for cyril to adopt.

---

## Headline: queue steering is a real wire-level feature

The changelog's "queue steering" is not TUI-side buffering — it's two new ACP extension methods served by the backend, **absent in 2.6.1** (`Method not found`, verified same-day):

| Method | Params | Response (2.7.0) | 2.6.1 |
|---|---|---|---|
| `_session/steer` | `{sessionId, message: string}` | `{"queued": true}` | -32601 |
| `_session/steer/clear` | `{sessionId}` | `{"cleared": true}` | -32601 |

Both are JSON-RPC **requests** (id + await), same family as `_message/send`. `message` is a plain string.

**Live-verified semantics** (probe `probe-steer-goal-2.7.0.py`):

- Steer mid-turn → immediate `{queued:true}` + a `_kiro.dev/session/update` notification with **new sessionUpdate variant `steering_queued`** `{message}`.
- At the next tool boundary the backend injects the message: variant **`steering_consumed`** `{content}`. Backend symbol: `agent::agent::Agent::drain_steering_or_end_turn` — drain happens at tool boundaries and turn end.
- `_session/steer/clear` before pickup → variant **`steering_cleared`** `{}` and the queued message is dropped (verified: cleared steer never influenced the turn).
- Steering an **idle** session also returns `{queued:true}` — held for the next turn.
- **The model treats steering as advisory, not as a command.** In the live test, haiku read the injected "stop and say PINEAPPLE" steer, *explicitly weighed it against the original task*, and chose to finish the original three tool calls first ("Following the user's primary request takes precedence over mid-task steering that would prevent task completion"). Steering = mid-turn context injection with model arbitration, same philosophy as `_message/send` subagent injection.

**Cyril impact:**
- *Passive:* none. The three `steering_*` variants ride **`_kiro.dev/session/update`** (matching line 22, with the underscore-dot prefix — *not* the unprefixed `kiro.dev/session/update` the original draft of this line claimed). cyril's converter (`convert/kiro.rs` `to_ext_notification`) has **no arm for `_kiro.dev/session/update`**, so these fall to the outer `other =>` arm (`kiro.rs:674`) and are **silently dropped (`Ok(None)` + debug log)** — they do *not* reach the inner `Some(other) => Err` arm (that arm lives under the unprefixed `kiro.dev/session/update` and is unreachable for steering). Either way it's passive: the variants are only emitted as echoes of a client's own steer, which today's cyril never sends.
- *Adoption (worth a roadmap phase):* this is exactly the "redirect without cancel" UX cyril wants. Requires: a `BridgeCommand::SteerSession`, ExtRequest wiring for the two methods, the three new variants handled in `convert/kiro.rs` (Notification variants, not errors, once cyril can trigger them), and a keybind/input mode in the TUI. Gate on the method existing (2.7.0+) — a steer to ≤2.6.1 errors cleanly with -32601 (no hang, unlike the 2.4.1 `/effort` case).
- New TUI settings `chat.defaultInterruptBehavior` + `chat.keybindings.toggleInterruptBehavior` are the TUI-side preference for steer-vs-queue mode; not wire-relevant.

---

## `/goal` — new dynamic command + new `goal` tool

### Wire surface

- `commands/available` now lists **24 commands** (2.6.1: 23): the addition is `/goal` — `{"inputType":"panel","subcommands":["clear"]}`. Cyril picks it up automatically via dynamic command registration (like `/effort`, `/rewind`).
- The advertised **tool list gains `goal`** (now: code, glob, grep, goal, introspect, knowledge, read, shell, subagent, todo_list, use_aws, web_fetch, web_search, write).
- New ext notification method registered in tui.js: **`kiro.dev/goal/status`** with payload `{state, iteration, maxIterations, message, elapsedSecs}`, states observed in tui.js: `active | paused | completed | exhausted | cleared`. **Not observed live in any of four probe configurations** (see below) — cyril's unknown-ext-method catch-all logs and drops it harmlessly.
- New backend modules: `chat_cli_v2::agent::acp::goal::GoalController`, `acp::commands::goal`, `agent::agent::goal`, `agent::agent::tools::goal`; serde types `GoalSnapshot`, `GoalIterationResult`, `GoalTool::Complete`.

### Live-verified contract (4 probes)

1. **Set:** `_kiro.dev/commands/execute` `{command:"goal", args:{value:"<description> --max N"}}` → `{success:true, data:{goal_action:"set", definition:{description, max_iterations}}}`. `--max N` is parsed out of the value string (default 5).
2. **Clear:** `args:{subcommand:"clear"}` → `{goal_action:"clear"}`.
3. **BUG:** `args:{subcommand:"status"}` is **misparsed as setting a goal described "status"** (max 5). Only `clear` is a real subcommand. The TUI sends the same shape for `/goal status`, so this appears to be an upstream 2.7.0 bug. Don't expose a status passthrough until fixed upstream.
4. **The goal definition is injected into agent context** once set — the agent knows the goal and the verification contract (binary embeds the instruction "Each criterion in the goal MUST be individually verified by tool output you can cite"; an agent asked to file a false completion refused, citing the verification contract).
5. **`goal` tool schema:** accepts **only `{command:"complete", summary}`** — `command:"status"` is rejected with `unknown variant 'status', expected 'complete'`. The agent calls it when it believes the goal is met (observed live: write file → `goal{complete, summary}` → completed).
6. **The iterative loop did NOT manifest on the bare ACP path.** Setting a goal and prompting normally produced ordinary single turns: no `goal/status` notifications, no auto-iterations, no verification turns — even when the prompt ignored the goal entirely (turn ended in 1s with "hello"). The loop machinery (`GoalIterationResult`, max-iteration enforcement) appears to engage either intra-turn via goal-tool results or through a driver path our probes didn't trigger. **For cyril this means: nothing about a session with a goal set changes the wire contract cyril already handles.**

### Cyril impact

Passive: `/goal` appears in cyril's command palette automatically; the `goal` tool call renders through the generic tool-call path (kind `other` — note: cyril filters `ToolKind::Other` "planning" steps from display, so goal-complete calls may be invisible; revisit if cyril adopts goal UX). No breakage. Adoption: trivially usable today via the dynamic command; dedicated goal-status UI only becomes worthwhile if/when `goal/status` actually fires on this path.

---

## Namespace migration `_kiro.dev/*` → `_kiro/*` accelerating (KAS convergence)

The 2.7.0 tui.js has **zero `_kiro.dev/*` strings left** — every underscore-prefixed extension is now `_kiro/*` (`_kiro/customAgent/config_error`, `_kiro/error/rate_limit`, `_kiro/mcp/governance_disabled`, new `_kiro/session/context`, `_kiro/steering/session_update`). The **un**prefixed `kiro.dev/*` family cyril consumes (`kiro.dev/commands/*`, `kiro.dev/metadata`, `kiro.dev/subagent/list_update`, …) is **fully intact on the wire** — verified live. tui.js registers handlers for both dialects.

- `_kiro/session/context` → **Method not found** on the v2 engine (KAS-dialect, dormant).
- KAS assets **still not embedded** (`kiro-cli acp --agent-engine kas` → "KAS assets not embedded"), but five new backend modules (`kas::{file_detection, persist, schema, session_id, v2_to_kas}`) are session-migration plumbing — the v2→KAS cutover is being actively built. The audit-time expectation stands: when KAS lands, cyril needs a `_kiro/*` dialect handler and fs-callback responders.

---

## Everything else

- **`/rewind` enrichment is client-side.** The turn picker summaries are built from the TUI's local message history; `/rewind` still has `inputType:"panel"`, no optionsMethod. The execute call gains a `turnIndex` arg alongside `messageId`. KAS path does rewind via `session/fork` + `_meta.kiro.{messageId, createdReason:"rewind"}`. No new wire data for cyril; a cyril rewind picker would summarize from its own message list.
- **Settings/theme/title overhaul** — tui.js-only (+42.8 KB bundle, sha-verified carve). New keys: `chat.terminalTitle`-family, `chat.defaultAgent`, `chat.defaultModel`, `chat.defaultInterruptBehavior`.
- **Custom agents inherit default resources** — backend behavior below ACP; no wire change.
- **Removed:** `agent::agent::tools::tool_search` module (tool list unchanged vs 2.6.1, so this was internal), legacy `chat_cli::os::fs`.
- **Launcher** `kiro-cli` +4.9 MB: dependency churn only (fig_proto mux, tungstenite). `kiro-cli-chat` +1.8 MB.
- **`__tool_use_purpose`** in tool `rawInput` — pre-existing (present in 2.6.1 captures), not a 2.7.0 change.
- **No embedded changelog entry** for 2.7.0 yet (`kiro-cli version --changelog=2.7.0` → none), same lag as 2.1.1.

## Regressions checked

| Surface | 2.6.1 | 2.7.0 |
|---|---|---|
| `initialize` agentInfo/capabilities | — | `{name:"Kiro CLI Agent", version:"2.7.0"}`, caps identical |
| `commands/available` | 23 cmds | 24 (= 23 + `/goal`) |
| `_kiro.dev/metadata` shape | ctx/metering/duration/effort | identical (live) |
| `tool_call_chunk` / subagent / inbox | unchanged | unchanged |
| Thinking parity (Opus+effort → `agent_thought_chunk`) | 29 chunks | **29 chunks — parity holds** |
| `/stats` `input_tokens`/`output_tokens` | null | **still null** |
| `cargo run --example test_bridge` (cyril path) | clean | **clean run, exit 0** |

### Thinking parity — replication matters

The first 2.7.0 thinking probe (easy "17 sheep" prompt) returned **0** `agent_thought_chunk` while the same-day 2.6.1 control returned 7 — looking exactly like a binary regression. Replication with a heavier reasoning prompt produced **29 vs 29**: parity. The zero was stochastic (Opus skipped thinking on a trivial question; `effort:"high"` was correctly echoed in metadata in both runs). Single-run thinking counts are not evidence — always replicate with a control. Symbol diff agrees: thinking/effort machinery is flat between binaries (one `ThinkingBlock` deserialize monomorphization difference, noise-level).

## Reproduce

```sh
# archive + carve (sha-trailer self-validating)
grep -abo --text -F '#!/usr/bin/env bun' kiro-cli-chat | tail -1   # bundle offset
# dd from offset, cut after last `();\n` following "Session ended.", sha256 == 64-hex trailer

# steering + goal probes
KIRO_BIN=~/.local/share/kiro-research/binaries/2.7.0/kiro-cli-chat PROBE_TAG=2.7.0 \
  python3 experiments/conductor-spike/probe-steer-goal-2.7.0.py
KIRO_BIN=... python3 experiments/conductor-spike/probe-goal-loop-2.7.0.py

# binary isolation control (expect -32601 on 2.6.1)
# … same probe with KIRO_BIN=binaries/2.6.1/kiro-cli-chat
```

Wire logs: `/tmp/cyril-probe-steer-goal-2.7.0.log`, `/tmp/cyril-probe-goal-{loop,tool,complete,verify}.log` (captured 2026-06-12).

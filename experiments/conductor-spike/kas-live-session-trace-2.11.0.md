# KAS live session trace — kiro-cli 2.11.0

Companion to [`kas-live-session-trace-2.11.0.jsonl`](kas-live-session-trace-2.11.0.jsonl) — a **real** KAS (v3) session captured 2026-07-02 from kiro-cli 2.11.0's own TUI client, not a probe. This is the richest KAS wire sample we have; use it as a reference corpus for KAS notification shapes, ordering, and interaction/subagent flows.

## Capture method

Kiro's built-in ACP recorder: `KIRO_ACP_RECORD_PATH=~/acp-trace.jsonl`. Each line is `{ts, dir, msg}` — `ts` epoch-ms, `dir` = `out` (client→agent) / `in` (agent→client), `msg` = the raw JSON-RPC frame. **TUI-only feature** (the v2 TUI's ACP client records its own traffic to KAS); the Rust `kiro-cli acp` path cyril spawns does *not* honor it — so this is a *reference* for how the same backend behaves, captured at near-zero gap (in-process, source-timestamped), not a capture of cyril's own stream.

**Redaction:** `accessToken` → `<redacted>` (1 occurrence); `profileArn` account-id masked to `<account-id>`. No other secret-keyed fields present.

## Shape

- **527 frames, ~880s**, dialect KAS (`_kiro/*`), workspace `~/repos/rivets`.
- **2 turns:** prompt id=5 (cancelled at 90.4s via `session/cancel`), prompt id=8 (completed at 879.5s after ~13 min of heavy `agent-subtask` + tool activity).
- 1 `_kiro/auth/getAccessToken` callback → reply `{accessToken, expiresAt, profileArn, provider}`, `provider: "Enterprise"` (note: prior audits assumed this user's *non*-enterprise token — this TUI session used an Enterprise profile).

**Methods** (excl. `session/update`): `session/request_permission` ×10, `_kiro/customAgent/config_error` ×8, `_kiro/sessions/changed` ×6, `session/set_config_option` ×5, `_kiro/mcp/status` ×4, `_kiro/tools/didChange` ×3, `_kiro/progressive_context/items_changed` ×2, `_kiro/policy/changed` ×2, `session/prompt` ×2, and singletons: `initialize`, `session/new`, `session/cancel`, `_kiro/auth/getAccessToken`, `_kiro/governance/state`, `_kiro/steering/documents_changed`, `_kiro/powers/items_changed`, `_kiro/hooks/cancel`.

**`session/update`** (bulk): `agent_message_chunk` ×166, `tool_call_update` ×112, `agent_thought_chunk` ×55, `tool_call` ×45; `session_info_update` kinds: `context_usage` ×21, `pending_interaction` ×10, `interaction_resolved` ×10, `steering_inclusion` ×9, `user_message_id_assigned` ×2, `turn_start` ×2, `turn_completion` ×2, `turn_end` ×2, `focus_update` ×1; plus `agent-subtask`-tagged `tool_call`/`tool_call_update` (KAS subagents — no `list_update`).

## Notable findings

1. **No new wire.** Every `_kiro/*` method and `session_info_update` kind here is already in the [covenant](../../docs/kiro-kas-acp-covenant.md) / audit catalog. A real 8-min KAS session on 2.11.0 stays entirely within the documented surface.
2. **Turn-boundary ordering (cyril-9akh evidence).** On BOTH turns, the only `session/update` trailing `turn_end` within 5s is a single `context_usage` (~10ms after the prompt response). **No `agent_message_chunk`/`tool_call` arrives after `turn_end`** on either turn. So in this KAS session the "streamed text after TurnCompleted" race did not manifest on the wire; only benign context telemetry trails. (Caveat: KAS/v3, 2 turns, one cancelled — one sample, not proof; cyril-9akh may also concern the v2 path.)
3. **`pending_interaction`/`interaction_resolved` directly observed** (10 each) — the 2.7.1 audit could only see these indirectly. Good corpus if the KAS interaction/elicitation path is ever built.
4. **Environment artifact (not a Kiro/cyril bug):** 8 `_kiro/customAgent/config_error` "No front matter found" for `~/repos/rivets/.kiro/agents/prompts/*.md` — frontmatter-less prompt files that KAS's recursive `.kiro/agents/**` scanner tries to load as agents. Env-specific; noted only because it's ~8 error notifications per session there.

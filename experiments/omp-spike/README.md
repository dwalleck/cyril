# omp-spike — oh-my-pi as an ACP agent under cyril

Live spike (2026-08-17, omp 17.3.4) answering two questions:

1. **Can kiro-cli slot into oh-my-pi via ACP?** No — omp's ACP faces
   agent-side only (`omp acp` serves stdio for clients like Zed/cyril; its
   `ClientSideConnection` is test-only). Its extension seams (60+ LLM
   providers, task agents, MCP-client) all sit below or inside its own agent
   loop.
2. **Can cyril drive `omp acp`?** Yes, unmodified — `cargo run --example
   test_bridge -- --agent-command omp acp` passes end-to-end: SessionCreated
   with modes, `available_commands_update` (92 commands), streamed turn,
   `UsageUpdated` (cyril's `unstable_session_usage` feature flag is
   load-bearing here), TurnCompleted, exit 0. All `kiro.dev/commands/*` steps
   degrade gracefully (`success=false`, bridge continues).

## Files

- `probe-omp-acp-surface.py` — raw JSON-RPC probe (conductor-spike template
  lineage): initialize → authenticate → session/new → session/list →
  `_omp/*` ext methods → one paid mini-turn → session/close. Runs with the
  real `$HOME` (omp's `agent` auth method reads `~/.omp`), throwaway cwd.
- `omp-acp-spike.jsonl` — full both-direction capture from that run,
  `{ts, dir, msg}` format (KIRO_ACP_RECORD_PATH-compatible; diffable with
  `../conductor-spike/diff-acp-wire.py`).
  The committed capture replaces session/account identifiers, session titles,
  and local working directories with stable redaction markers.

## Key wire facts

- `session/new` returns `configOptions` (`mode`, `model` ×41, `thinking` ×7)
  and `modes` (default/plan); `session/set_config_option` implemented —
  cyril's model picker on omp needs configOptions support, the same
  machinery ROADMAP KAS-5 needs.
- `sessionCapabilities {list, fork, resume, close}` all functional over
  standard ACP.
- Slash commands execute as **prompt text** (`session/prompt` with
  `/cmd …`), not `kiro.dev/commands/execute`.
- `usage_update`: `{size, used, cost: {amount, currency}}`.
- `session_info_update` carries `{updatedAt}`/title — no `kind` field
  (unlike KAS's kind union).
- Ext dialect: `_omp/sessions/listAll`, `_omp/projects/list`,
  `_omp/chats/byCwd`, `_omp/usage`, `_omp/extensions[/toggle]`, called as
  plain JSON-RPC method names.

Full analysis: memory `reference_omp_acp_agent.md` (cyril-memory repo).

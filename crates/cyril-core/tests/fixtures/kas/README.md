# Captured KAS `session/update` fixtures (KAS-0 Slice 7, cyril-atjw)

Real `session/update` notification **params** (= `acp::SessionNotification`)
captured from a live KAS run, used by the ACP-coverage verification spike
(`protocol::convert::tests::schema_deserializes_captured_kas_session_updates`,
D9, **non-gating**) to confirm schema 0.11.2 deserializes the variants KAS emits.

**Source:** extracted from `experiments/conductor-spike/logs/probe-kas-*.log`
by `experiments/conductor-spike/extract_kas_session_update_fixtures.py`.

**Coverage (no silent cap):** only `agent_message_chunk` + `session_info_update`
extracted cleanly — the probe logs **truncate** frames past ~400 chars, so the
longer ones (`tool_call`, `tool_call_update`, `available_commands_update`,
`config_option_update`) failed to parse. Those are **standard ACP variants** the
v2 engine also emits and that the v2 `convert` tests already exercise via the
same `acp::SessionNotification` deser path — so they are covered. The
KAS-distinctive variant is **`session_info_update`** — the envelope that, via
`_meta.kiro.kind`, carries KAS sub-kinds including `turn_end`.

**Captured sub-kinds (`_meta.kiro.kind`):**
- `user_message_id_assigned` — `session_info_update.json` (a non-terminal sub-kind).
- **`turn_end`** — `session_info_update_turn_end.json` — the **load-bearing**
  terminal lifecycle signal; `_meta.kiro.stopReason` (mirrored at
  `_meta.kiro.turnEnd.stopReason`). Captured live 2026-06-29 by
  `experiments/conductor-spike/probe-kas-turnend-capture.py` (KAS-2a / cyril-j16p
  cheapest-falsifier).
- `turn_completion` — `session_info_update_turn_completion.json` — **metering
  only** (`promptTurnSummaries`/`elapsedTime`/`status`), NOT the busy-clear
  signal. Fires BEFORE `turn_end`. Kept as a negative fixture so the converter
  can't confuse metering for completion.
- **`context_usage`** — `session_info_update_context_usage.json` — the
  proactively-pushed per-category breakdown (KAS-2b / cyril-5et2 →
  `ContextBreakdownUpdated`). `_meta.kiro` carries flat `usagePercentage`, a
  nested `contextUsage.usagePercentage` mirror, and `breakdown` (5 buckets;
  `items[]` only on contextFiles/sessionFiles). Captured live from **2.10.0** by
  `experiments/conductor-spike/probe-kas-context-breakdown-capture-2.10.0.py`.

**Observed order in one turn** (falsifier finding): `… turn_completion → turn_end
→ context_usage`. `turn_end` is the terminal *lifecycle* signal but is **not the
last frame** (a `context_usage` trails it), so the converter must key on
`kind == "turn_end"` specifically, never on "the last `session_info_update`".

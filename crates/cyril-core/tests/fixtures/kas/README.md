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
KAS-distinctive variant is **`session_info_update`** (the `turn_end` carrier),
which is captured here. A full multi-variant live capture is a KAS-2a task
(cyril-j16p), where live KAS work happens anyway.

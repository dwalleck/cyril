# cyril-sm36 live gate-on parity probe — PENDING (blocked on Kiro auth)

Binary: `cyril` built from `0c5080ea` (default features, KAS), strings-checked for `chat.enableWorkflows`.
Environment: private `HOME` with `.kiro/settings/cli.json` = `{"chat.enableWorkflows": true}`, `KIRO_HOME` unset,
`XDG_DATA_HOME=/home/dwalleck/.local/share`, private `XDG_CONFIG_HOME`, `KIRO_LOG_LEVEL=debug`; kiro-cli 2.32.0 / KAS 0.66.43.

## Attempt 1 — 2026-10-19 ~01:13Z: auth failure, stopped

cyril screen:

```
Disconnected: invalid configuration: KAS auth unavailable: `/home/dwalleck/.local/bin/kiro-cli chat _ get-kas-token` did not answer within
30 s; check the network connection to the Kiro auth service, or another kiro-cli holding its token-refresh lock ...
```

kiro-cli log (`$XDG_RUNTIME_DIR/kiro-log/`), same minute:

```
2026-10-19T01:13:27.855073Z DEBUG fig_auth::builder_id: 447: Refreshing access token
2026-10-19T01:13:28.220744Z ERROR fig_auth::builder_id: 488: Failed to refresh builder id access token: service error: InvalidGrantException
2026-10-19T01:13:28.232962Z  WARN q_cli::util: 302: called welcome_login_prompt while stdout is not a terminal
```

The stored refresh token was rejected (`InvalidGrantException`), so kiro-cli is logged out and waits on a login
prompt. Per the task's hard rule no kiro-cli auth command was run; the probe step stopped here. It needs the user
to log in again (`kiro-cli login`), then a re-run of this probe.

## Still to show once auth is back

- KAS `kiro.log` (debug) has no `configuration.intake.unknown-key` for `workflows` or `goal` (positive control:
  the already-stale keys cyril sends, e.g. `thinking`, do log it).
- `session/new` result `_meta.workflowsEnabled: true` (persisted session metadata under the private `HOME`).
- The model's tool list includes `run_workflow` / `inspect_workflow` / `update_workflow` / `send_message`, and
  `invoke_sub_agent` is absent; a gate-off control (`cli.json` = `{}`) shows the reverse.
- cyril's own `/workflow run .kiro/workflows/sm36-echo.workflow.json` still launches and completes with the gate on.

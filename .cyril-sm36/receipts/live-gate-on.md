# cyril-sm36 live gate-on parity probe — PENDING (attempt 2 found a defect; fix awaits re-probe)

Environment: private `HOME` with `.kiro/settings/cli.json` = `{"chat.enableWorkflows": true}`, `KIRO_HOME` unset,
`XDG_DATA_HOME=/home/dwalleck/.local/share`, private `XDG_CONFIG_HOME`, `KIRO_LOG_LEVEL=debug`; kiro-cli 2.32.0 / KAS 0.66.43.
Paths below are under the scratch dir `/home/dwalleck/.claude/tmp/claude-1000/-home-dwalleck-repos-cyril/0473cb2a-6b72-4739-a03f-aec2a7f97d90/scratchpad/cyril-sm36/sm36-live/on/`.

## Attempt 1 — 2026-10-19 ~01:13Z, binary `0c5080ea`: auth failure, stopped

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

The stored refresh token was rejected, so kiro-cli was logged out. No kiro-cli auth command was run.

## Attempt 2 — 2026-10-19 ~23:47Z, binary `cf88ab17`, run by the coordinator: gate stayed OFF (defect)

- **KAS accepted both keys.** `home/.kiro/logs/20261019T234754304/kiro.log` (debug) has exactly two
  `configuration.intake.unknown-key` lines, for `setting/subagentOrchestration` and `setting/thinking` (layer
  `client-connection`). There are none for `workflows` or `goal`.
- **The session ran gate-off.** Session `sess_aacab65f-9fde-4c3b-b382-2e977335ed22` persisted
  `home/.kiro/sessions/cbc868292fc9051f/sess_aacab65f-9fde-4c3b-b382-2e977335ed22/session.json` with
  `"workflowsEnabled": false`. KAS advertised only 3 slash commands (no `workflow-*`, no `goal`).
- **Cause.** In the 0.66.43 bundle, `workflows` is a session-scoped settings row:
  `{key:Xn.workflows, wire:AI("workflows"), messages:jR, record:nWe("workflowsEnabled")}` with
  `jR=["session/new","session/load"]`. `validation` (`validationSettingEnabled`) and `fta` (`ftaEnabled`) are the
  same. Those rows are read from `session/new`'s `_meta.kiro.settings`; cyril sent settings only on `initialize`.
  Kiro's TUI sends the same object in both places (committed capture
  `experiments/conductor-spike/audit-2.32.0/live/kas-workflow-gateon-2.32.0.jsonl`, fixture
  `crates/cyril-core/tests/fixtures/kas/session_new/initialize-and-session-new-gateon-2.32.0.json`) and none on
  `session/load`.
- **Fix.** `session/new` now carries `_meta.kiro.settings` = the engine's `initialize` settings (see `route.md`,
  Correction). It needs a fresh probe (attempt 3, run by the coordinator).

## Still to show (attempt 3)

- No `configuration.intake.unknown-key` for `workflows` or `goal`; `session.json` `workflowsEnabled: true`.
- KAS advertises `workflow-run|resume|status|cancel` and `goal` (cyril filters them from its own command list).
- The model's tool list includes `run_workflow` / `inspect_workflow` / `update_workflow` / `send_message`, and
  `invoke_sub_agent` is absent; a gate-off control (`cli.json` = `{}`) shows the reverse.
- cyril's own `/workflow run .kiro/workflows/sm36-echo.workflow.json` still launches and completes with the gate on.

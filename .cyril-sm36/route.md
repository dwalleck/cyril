# cyril-sm36 — KAS honours `chat.enableWorkflows` (ADR-0013 decision 1)

Date: 2026-10-18. Route owner: Gilfoyle change-workflow.

## Inputs and source state

Requester (orchestrator): implement rivets issue cyril-sm36 on `feat/cyril-sm36-enable-workflows` from
`origin/main` at `59f9db1a` (KAS default since #170). Authoritative behaviour: `rivets show cyril-sm36` and
[ADR-0013](../docs/adr/0013-workflows-follow-the-users-kiro-setting.md) decision 1.

## Routing evidence

| Test | Verdict | Evidence |
|---|---|---|
| T1: consequential unresolved external premise | No | Kiro's TUI resolution is read from the 2.32.0 bundle (`kiro-tui-2.32.0.js`): `ab(){let e=Rn("workflows");return{available:e,enabled:e&&ea(et.CHAT_ENABLE_WORKFLOWS,!1)}}`; `ZOn(e,t){if(t.available)e.workflows={enabled:t.enabled},e.goal={enabled:t.enabled}}`; `ea(k,d)` returns the value only when it is a boolean, else the default `false`; `va()` overlays the workspace `cli.json` only for keys in `nq`, and `chat.enableWorkflows` is **not** in `nq` → global `cli.json` only (`$r("settings","cli.json")`, KIRO_HOME-aware). The `workflows` rollout row in `kiro-cli-chat` 2.32.0 is `treatment_percent: 100`, `segment: "all"` ("Generally available to all users on the KAS engine"). KAS 0.66.43 intake: `workflows` and `goal` are both keys of the `AgentSettings` schema (`m6n`), `workflows` → `workflowsEnabled` via `AI("workflows")`, `goal` read by `TP(settings,"goal")` (advertises a `/goal` command of `_meta.kiro.type:"workflow"`). The live gate-on probe is an acceptance check, not a design premise. |
| T2: structural interface/ownership/security boundary | No | One row added inside the existing owner `protocol/kas/settings.rs::marshal_agent_settings` (pub(crate), unchanged signature). No new module, dependency or interface. |
| T3: production-scale risk | No | One map lookup at startup. |
| T4: explicit behaviour | Yes | Issue acceptance: flat global `chat.enableWorkflows: true` → `settings.workflows={enabled:true}` and `settings.goal={enabled:true}`; `false`, absent, or non-boolean (Kiro's `ea` default) → both `{enabled:false}`. "available" = engine is KAS (the marshal runs only for `KasEngine`, `bridge.rs::engine_for`). Old never-marshal test removed; every ADR-0011 citation in `crates/` updated to ADR-0013. Live gate-on probe on KAS 0.66.43. |

## Correction (2026-10-19): the gate rides `session/new`, not only `initialize`

The T1 row above assumed `initialize.clientCapabilities._meta.kiro.settings` was enough. It was not, and the
coordinator's live gate-on probe of `cf88ab17` showed it (`receipts/live-gate-on.md`, attempt 2). KAS accepted
both keys (no `unknown-key` for them), but the session ran with `workflowsEnabled: false`. In the 0.66.43 bundle,
`workflows` (and `validation`, `fta`) are **session-scoped** settings-table rows:
`{key:Xn.workflows, wire:AI("workflows"), messages:jR, record:nWe("workflowsEnabled")}` with
`jR=["session/new","session/load"]`. KAS reads them from those messages' `_meta.kiro.settings`. The committed 2.32.0
gate-on capture shows the TUI sending one settings object both on `initialize` and as `session/new`
`_meta.kiro.settings`, and none on `session/load`, where KAS keeps the session's recorded value.

Fix: `domain_mediator/commands/session.rs::new_session_request` takes the engine's `settings_extra()` (the same value
`initialize` advertises) and sends it as `_meta.kiro.settings`; v2 (`None`) stays bare; `session/load` is unchanged.
Fences: `bridge/tests/current_runtime_contract/session_settings.rs` (KAS carries the gate; the object equals
`initialize`'s with `_meta.kiro` = `{settings}` like the capture fixture; v2 carries no `_meta`). Red receipt:
`receipts/red-session-new-settings.txt`. The route stays Local (one builder in its existing owner).

## Selected route

**Local.** Explicit behaviour within the existing settings-marshal owner; no unresolved external premise.

## Decisions recorded

- **Rollout `available`:** cyril cannot evaluate Kiro's host rollout registry (`KIRO_ENABLED_FEATURES` is set by the
  Rust host, which cyril's direct `node acp-server.js` spawn bypasses). The `workflows` row is GA (100 %, segment
  `all`) in 2.32.0, so cyril treats it as available whenever the engine is KAS — the issue's own statement.
- **`workflowNotifications`:** Kiro's 2.32.0 TUI also sends `workflowNotifications={enabled, delivery}` derived from
  the same gate. KAS 0.66.43 contains zero references to it (it would be an `intake.unknown-key`), and the issue
  scopes only `workflows` + `goal`; cyril does not send it.
- **Gate-advertised commands (`convert/kas.rs`):** with the gate on, KAS 0.66.43 advertises `workflow-run|resume|
  status|cancel` (type `workflow`, `HJr`/`Q6u`) and `/goal` (type `workflow`, `WJr`). cyril's own `/workflow` family
  stays (ADR-0013 decision 4); the exact-name suppression is kept and re-cited to ADR-0013. All five already fall
  outside `PROMPT_COMMAND_TYPES`, so none reaches the registry.

## Required artifacts and sequence

`route.md` only (Local). Red-first unit tests in `settings.rs`, repository gate (every ci.yml lane), live gate-on
parity probe, independent review via the PR.

## Terminal criterion

Requested behaviour delivered; marshal unit tests red-first then green; every ci.yml lane passes locally and in
GitHub CI; live gate-on probe evidence in `receipts/`; no blocking defect unresolved.

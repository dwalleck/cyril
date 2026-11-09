# Compiled-in rollout registry 2.32.0 → 3.1.0 (STATIC; `treatment%[/internal%i] segment/channel`; `=` unchanged; `—` row absent)

| row | 2.32.0 | 2.33.0 | 2.33.1 | 2.34.0 | 2.34.1 | 2.34.2 | 2.35.0 | 2.35.1 | 2.35.2 | 2.36.0 | 2.37.0 | 2.37.1 | 3.0.0 | 3.1.0 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `background_execution` | 0 int/nig | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `c2s` | 100 int/nig | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `cloud_config` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `infra_safety` | 100 int/* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `kvim` | 100 int/* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `lite` | 100 int/* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `local_sandbox` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | — |
| `managed_settings` | 0 int/nig | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `memory` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `memory_cloud_sync` | — | = | = | = | = | = | = | = | = | = | = | = | = | 0 int/nig |
| `memory_controls` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `model_fallback_auto_refusal` | — | = | = | 100 int/* | = | = | 100 all/* | = | = | = | = | = | = | = |
| `remote_changelog` | 100 */sta | = | = | = | = | = | = | = | = | = | = | = | = | — |
| `remote_sandbox` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | — |
| `session_dashboard` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | — |
| `standalone_public` | — | = | = | 0 */* | = | = | 10 */* | = | 30 */* | 50 */* | 100 */* | = | = | — |
| `standalone_toolbox` | 0 all/* | = | = | 5 all/* | 20 all/* | = | = | 50 all/* | 100 all/* | = | 100 */* | = | = | — |
| `tangent` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | — |
| `test` | 100 */* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `test_internal_only` | 100 int/* | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `test_nightly_only` | 100 int/nig | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `trust_classifier_enforce` | — | = | = | 100 int/nig | 100 int/* | = | = | = | = | 100 all/* | = | = | = | — |
| `trust_classifier_shadow` | 100 int/nig | = | = | = | = | = | = | = | = | = | = | = | = | = |
| `tui` | 50 int/* | = | = | = | = | = | = | = | = | = | = | = | = | — |
| `v3_prompt` | 10/75i all/* | = | 25/100i all/* | 50/100i all/* | = | 100/100i all/* | = | = | = | = | = | = | = | — |
| `voice` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | — |
| `workflows` | 100 all/* | = | = | = | = | = | = | = | = | = | = | = | = | — |

row counts: {'2.32.0': 23, '2.33.0': 23, '2.33.1': 23, '2.34.0': 26, '2.34.1': 26, '2.34.2': 26, '2.35.0': 26, '2.35.1': 26, '2.35.2': 26, '2.36.0': 26, '2.37.0': 26, '2.37.1': 26, '3.0.0': 26, '3.1.0': 15}

3.1.0 descriptions (15 rows):
- `background_execution`: Background execution for V3 sessions. Whenever it is on, the TUI sends universalBackgroundTool: { enabled: true } on initialize, session/new and session/load, so the agent runs shell jobs (long commands that keep running after their tool call returns). The CLI display gated on this flag (shells chip, transcript rows for finished background processes, Ctrl+B, the Ctrl+G Shells monitor) reads the agent's older background layer, which this flag no longer turns on, so it has nothing to show and Ctrl+B reports the move as not enabled, until it moves onto jobs. KAS reads no KIRO_ENABLED_FEATURES; a KAS reached without the CLI is enabled by its own operator override, KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED=true. Off, CLI and agent behave exactly as before the feature existed. Dark-shipped at 0% until the telemetry layer ramps it for internal users on the nightly channel; treatment_percent is the dial and the kill switch. Opt in anywhere with KIRO_ENABLED_FEATURES.
- `c2s`: Code-to-Spec Explore agent and analysis pipeline. Internal nightly only; the rollout exposes Explore and the settings control, while chat.enableC2s enables the analysis tools.
- `cloud_config`: Cloud config: the consolidated /config panel, cloud/local source labels on config listings, and the launcher passing the stage-default cloud-config BFF endpoint to KAS for local V3 sessions. Turning this off hides the UI and stops the launcher supplying that stage default, so KAS makes no cloud-config calls unless the user explicitly sets a trusted-host CLOUD_CONFIG_ENDPOINT override (which applies regardless of the rollout). GA: ramped to 100% for ALL users on every channel; per-user activation stays governed by KAS's server-side AB_CLOUD_CONFIG entitlement, so non-entitled users see local-only sources. This flag is retained purely as the client-side kill switch: to darken the fleet, set treatment_percent to 0 in a follow-up release (config is compiled in); the immediate per-user lever is KIRO_FEATURE_CLOUD_CONFIG_ENABLED, an env override read by the KAS server (kiro-agent's feature-config provider), not by this crate.
- `infra_safety`: ICECAP infrastructure-safety gate (monitor/enforce). Internal-only initially; launcher exports KIRO_INFRA_SAFETY_ROLLOUT_ENABLED and the TUI advertises the capability + honors the infraSafety settings only when enabled. Ramp via treatment_percent.
- `kvim`: /kvim: Neovim in a pane above the fullscreen chat. The launcher includes kvim in KIRO_ENABLED_FEATURES and the TUI registers the /kvim command only when enabled. Internal users on every channel; dial treatment_percent to 0 to kill-switch. Opt in anywhere with KIRO_ENABLED_FEATURES.
- `lite`: Lite TUI mode - all channels, Amazon employees only
- `managed_settings`: Organization-managed settings display on V3: the /config managed category and subcommand, organization Source labels on /config, /mcp and /hooks rows, the organization-disabled/unavailable/hidden MCP row, the _kiro/governance/state subscription feeding them (off: the TUI reads only the web-tools toggle from it, as before), and the pre-session _kiro/config/template mode seed. Admin enforcement is not gated: KAS enforces governance itself. Dark-shipped at 0%; ramp internal nightly first via treatment_percent, which is also the kill switch. Opt in anywhere with KIRO_ENABLED_FEATURES='["managed_settings"]'.
- `memory`: Persistent agent memory and the /memories command. Generally available to all users on every channel; memory starts off for external users until they opt in, and KAS's memory experiment still decides per-user eligibility.
- `memory_cloud_sync`: KAS-only memory cloud sync in /memories: the account and this-machine switches in config (memorySync at user-synced and user-local, read and written through KAS's configuration methods), the last-sync line, and each memory's cloud state in the list. Off, the CLI neither reads nor writes the setting; KAS alone decides whether sync runs. Dark-shipped at 0%; ramp internal nightly first via treatment_percent, which is also the kill switch. Opt in anywhere with KIRO_ENABLED_FEATURES='["memory_cloud_sync"]'.
- `memory_controls`: KAS-only /memories controls: saved memory access and automatic-update defaults, plus record browsing and deletion (list, preview, detail, delete). Generally available to all users on every channel; set treatment_percent to 0 to darken it.
- `model_fallback_auto_refusal`: Re-send a turn refused on Auto, on Auto, with the refused message's reasoning envelope back in history so the backend routes the turn around the model that refused. Off, a refusal on Auto stands as the turn's answer. On for every user on every channel. The V2 engine and V1 subagents both follow the kiro-cli process's decision (KAS has its own model_fallback_auto_refusal client config). Dial treatment_percent to 0 to kill-switch.
- `test`: Test experiment for unit tests
- `test_internal_only`: Test experiment for unit tests (internal only)
- `test_nightly_only`: Test experiment for unit tests (nightly channel only)
- `trust_classifier_shadow`: Permission-classifier shadow mode: on every approval prompt the engine also asks the permission-classifier model for a verdict and records it beside the human's decision, never changing the prompt. Internal nightly only. Per-user opt-out is the trust.classifier.shadow setting; the launcher exports the combined decision as KIRO_TRUST_CLASSIFIER_SHADOW; today only the V2 engine reads it (V3 is planned). Dial treatment_percent to 0 to kill-switch.

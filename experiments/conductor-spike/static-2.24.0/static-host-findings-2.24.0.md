# Host-side static lane — kiro-cli 2.22.0 → 2.23.0 → 2.23.1 → 2.24.0

Static only; no kiro-cli/KAS process spawned except `--help`/`version --changelog`
on archived binaries under a temp `HOME` (no auth touched).

## Binaries are stripped from 2.23.0; the size drop is stripping + compression

| binary | 2.22.0 | 2.24.0 | cause |
|---|---:|---:|---|
| `kiro-cli` | 113.9 MB | 43.8 MB | `.debug_*` (~58 MB) + `.symtab`/`.strtab` (~12 MB) removed; `.text` 32.0 → 32.0 MB |
| `kiro-cli-term` | 87.0 MB | 34.0 MB | same (debug ~43 MB + symtab/strtab ~9 MB) |
| `kiro-cli-chat` | 462.1 MB | 213.0 MB | symtab/strtab (15 MB) removed; `.rodata` **368.4 → 132.3 MB** (payloads now zstd) |

`nm` lane is dead from 2.23.0. Substitute: the `crates/<crate>/src/*.rs`
source-path strings (tracing `event crates/…:LINE` + panic locations) survive
stripping — `static-host-srcpaths-delta.json`.

`kiro-cli-chat` 2.24.0 embedded payloads (all zstd frames; 2.22.0 had them raw,
except KAS which was gzip):

| offset | payload |
|---|---|
| 6.84 MB | KAS bundle tar (`@kiro/agent` 0.66.8) |
| 33.8 MB | **Node.js v22.22.2** ELF (124.7 MB decompressed) — runs KAS |
| 65.8 MB | **Bun v1.4.2** (744846f84) ELF (79.5 MB) — runs the TUI; 2.22.0 shipped Bun **v1.3.13** |
| 75.47 MB | Bun node-polyfill module frames |
| 95.7 MB | **TUI bundle** `#!/usr/bin/env bun` (13.27 MB) |

TUI bundles carved and archived: `~/.local/share/kiro-research/tui-bundles/kiro-tui-{2.22.0,2.24.0}.js` (+ .sha256).

## Rollout registry (18 → 19)

* 2.23.0: **+`local_sandbox`** (100 %, internal, nightly: "CLI LocalSandbox testing for internal nightly users on V3"); `v3_prompt` 5 → **10 %** internal.
* 2.23.1: `v2_non_interactive` 50 → **75 %**; `v3_prompt` 10 → **25 %** internal.
* 2.24.0: none.
* `model_fallback` still 0 %. FeatureOverride cohort: one entry `v3_prompt`, 135 treatment hashes, digest `3d29b950321fdd59` identical in all four builds.

## Host env tokens (chat binary ∪ decompressed TUI bundle)

144 → 149, **zero removed** (the raw-binary diff's ~75 "removals" were TUI-bundle
names now inside a zstd frame). Added: `KIRO_KAS_REGION` (launcher
`launch/kas_endpoint_overrides.rs` — endpoint overrides now validated: https or
loopback http; krs/cps region must agree), `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`
(TUI `Mn("local_sandbox")`), `KIRO_TURN_MARKER_DIR` (host `turn_marker.rs`),
`KIRO_TEST_HOMEBREW_PREFIX` (Homebrew updater fix). `KIRO_KAS_SERVER_PATH` 4=4,
`KIRO_KAS_NODE_PATH` 3=3, `KIRO_ACP_RECORD_PATH` 1=1,
`KIRO_ROLLOUT_FORCE_INTERNAL` 2=2 — all survive. `KIRO_SESSION_ID` 4→6: the TUI
now sets `process.env.KIRO_SESSION_ID` on session switch and the v2 agent
(`agent/util/sanitize.rs`) exports it to shell tools.

## New host source modules (chat 360 → 376 paths)

`chat-cli-v2/src/agent/acp/commands/reasoning.rs` (**new v2 ACP TuiCommand
`reasoning`**, `struct ReasoningArgs with 4 elements` incl. `setAsDefault`,
`targetModelId`; persists a "reasoning default"; v2 slash list gains `/reasoning`
between `/effort` and `/goal` — present from 2.23.0), `agent/util/image.rs`
(image downscale; `[image removed: rejected by the model provider]`),
`agent/util/sanitize.rs`, `amzn-kiro-controlplane-bearer-rust-client`
(`get_user_preference`, `set_user_preference`, `list_available_models`),
`chat-cli-v2/src/database/settings_file.rs`, `chat-cli/src/cli/chat/resume.rs`,
`chat-cli/src/cli/sandbox.rs`, `launch/kas_endpoint_overrides.rs`, `turn_marker.rs`.

## TUI bundle 2.22.0 → 2.24.0

* `onExtNotification` 14 → 15: **+`_kiro/sandbox/status`** (gated on `local_sandbox`);
  quoted literals +`_kiro/sandbox/status`, +`_kiro/sandbox/applyConfig`. Both
  methods already existed in KAS 0.66.0 (1 / 4 occurrences = 0.66.8).
* `/tools trust-all` = **TUI-local**. `H4e` parses `trust-all` → `requestTrustAllTools()`
  (confirm gate) → `trustAllToolsConfirmed`; the pre-existing permission handler
  (same code in 2.22.0 for `--trust-all-tools`) auto-selects `allow_always`
  (else `allow_once`) and, for KAS, attaches `_meta.kiro.consent{capability,
  scope:"session", resource?, workspaceRoot?}` (`kasWholeCapability`). No new
  method, no setting sent to KAS.
* `/effort [default]` = TUI display only (`Id({saved: chat.modelDefaults[model]})`).
  `chat.defaultModel` / `modelDefaults` / `initialModel` / `set_config_option`
  counts unchanged — no wire change visible.
* Queued steering: no `_session/steer` literal in either TUI; UI queue only.
* **First-party consumer of `rejectionReason`**: `ow()` now sends
  `_meta.kiro.rejectionReason` (from typed feedback) on `reject_once` (0 → 1).
  Matches 2.23.0 note "Feedback typed when denying a tool call now reaches the model".
* New agent-capability parse `replayMarking` (TUI 0 → 12 refs). KAS already
  advertised `replayMarking:true` and marks replayed history frames
  `_meta.kiro.replay:true` in 0.66.0 (1 = 1). The TUI uses it in the workflow
  child-session observer to separate replayed from live step frames after
  proactive `session/load`.
* Not consumed by the TUI: `_kiro/workflow/node_failed`, `_kiro/terminal/settings_changed`,
  `commandTimeoutMs`, `focus_update`/`turnActive`. `session/delete` 1 = 1.
* `initialize._meta.kiro` client payload (`y8e`/`zze`) unchanged.

## Doc manifests

Identical: `2026-09-02T18:38:18` / 103 docs and `2026-08-20T10:49:07` / 139
docs in both 2.22.0 and 2.24.0; 0 added / removed / changed.

## CLI help / changelog

`--help-all`, `acp --help`, `chat --help` identical (one option reordered).
**2.23.0's own binary carries a 33-line 2.23.0 changelog** (`static-changelog-2.23.0.txt`)
that 2.24.0's embedded history lacks ("No changelog information available").

## Project `.env` fix

2.24.0's launcher passes **`--no-env-file`** to the Bun TUI runtime (inlined as
`--no-env`+`env-file` 8-byte immediates beside `--coverage`/`--coverage-dir=`;
0 hits in 2.22.0 / 2.23.0 / 2.23.1, 1 in 2.24.0). Bun auto-loads `.env*` from
the cwd into `process.env`, which every TUI child (KAS node, MCP servers, tools)
inherited. Cyril spawns `node acp-server.js` from Rust and Node does not
auto-load `.env`, so cyril was never exposed; the `kiro-cli acp` launcher path
never ran under Bun either.

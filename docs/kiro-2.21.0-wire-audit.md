# kiro-cli 2.21.0 wire audit (delta from 2.20.1)

**Audited:** 2026-09-01 (2.21.0 released 09-01). Installed `kiro-cli 2.21.0`;
**KAS 0.54.3 → 0.54.8**. Binary axis isolated same-day against one backend:
the 2.20.1 binaries were re-fetched from the versioned origin (the 2.20.x
releases had never been archived) and both hops now live in
`~/.local/share/kiro-research/binaries/{2.20.1,2.21.0}/`.

**Verdict: SAFE for cyril** — nothing cyril sends or parses changed on either
engine. One new client-visible behaviour on KAS: a **session-start
`fetch_cloud_config` tool call that arrives BEFORE the `session/new` response**
(§ 5). Cyril's routing classifier discards that frame by design and logs a
warning; the tool call is `kind: other`, so nothing is lost on screen, but the
"no shipped engine produces this ordering" premise behind the Drop arm is now
false.

---

## 1. Static census — protocol surface unchanged, one packaging change

| lane | 2.20.1 / 0.54.3 | 2.21.0 / 0.54.8 |
|---|---|---|
| `_kiro/*` method literals in `acp-server.js` | 110 | **110** (zero added, zero removed) |
| advertised `extensionMethods` at `initialize` (live) | 23 | **23**, identical list |
| KAS feature-flag registry | 15 flags / 9 env-reachable | **identical** (§ 2a) |
| KAS env vocabulary (`KIRO_[A-Z0-9_]+` in bundle) | — | **identical** |
| bundle size | 17,564 lines / 11.35 MB | 17,577 lines / 11.34 MB |
| bundle string literals (`"[A-Za-z_][A-Za-z0-9_./:@ -]{3,80}"`) | 22,643 | 22,646 (**+6 / −3**) |

The six added / three removed literals are the whole readable delta of the
0.54.3 → 0.54.8 patch series:

```
+ "policy.shell.parse_error"                       # new count metric, Feature:shellPolicy
+ "invalidInput"                                   # Introspect tool: doc_path not found → recordToolError
+ "a refused or failed start reports success to the model"        # control_process withLegacySuccess note
+ "a timed-out command reports success to the model even though it failed"  # executeBash withLegacySuccess note
+ "node --test scripts/sync-workspace-versions.test.mjs" / "test:sync-versions"  # build scripts
- "tree-sitter-powershell"  - "No workspace folders are open"  - "Process control operation completed successfully"
```

**Packaging:** the PowerShell grammar used by the shell-policy parser moved from
the native `tree-sitter-powershell` package (node-gyp prebuilds for six
platforms) to a vendored **`@kiro/tree-sitter-powershell` 0.26.4-kiro.48 wasm**
(`policy.powershell.grammar.load.failed` on load error); `node-addon-api` and
`node-gyp-build` left the tree. The policy parser's shell set is
`pwsh, powershell, pwsh-preview, cmd, bash, gitbash, wsl, zsh, sh, fish, dash, ksh`
(anything else → `other`). Not wire-visible.

**Lesson (re-learned):** a broad literal regex (`"[^"\\]{2,120}"`) reports
4,265 added / 4,259 removed on this pair — all minifier churn where template
literals straddle quotes. Only the strict identifier-shaped regex above is a
valid A/B on a minified bundle.

---

## 2. Feature flags — three registries, one real change

### 2a. KAS `FeatureConfigRegistry` — unchanged

`extract-kas-feature-flags.py` on the carved 0.54.8 bundle reproduces the
2.20.1 table exactly: 15 flags, 9 env-reachable
(`KIRO_FEATURE_{STEERING_SUPERVISOR,CGS_DELEGATION_V2,SESSION_TITLE_LLM,USER_AGENT_REFACTORING,KIRO_INFRA_SAFETY_MONITOR,FTA_VIBE,STREAM_IDLE_WATCHDOG,AUTH_EXPIRY_RETRY,MEMORY_EXTERNAL}_ENABLED`),
6 experiment-only. **No `cloud_config` flag exists in KAS** — that matters for § 2b.

### 2b. Rust host rollout registry — NEW LANE, 4 entries changed

`kiro-cli-chat` compiles in a JSON experiment table (two identical copies, one
per engine crate) keyed by feature name with
`{treatment_percent, segment?, channel?, description}`. It was present in
2.20.1 but never audited. Extraction: brace-match the object enclosing the
literal `"cloud_config": {` in the raw binary and `json.loads` it (recipe in
the session scratch; the table below is the 2.21.0 content).

| feature | 2.20.1 | 2.21.0 | note |
|---|---|---|---|
| `cloud_config` | 100 % **internal** | 100 % **all** | **GA'd** — launcher now passes the stage-default cloud-config BFF endpoint to KAS for local V3 sessions; the `/config` panel from the changelog |
| `session_dashboard` | 100 % **internal** | 100 % **all** | **GA'd** — `/sessions`, `--sessions`, `--resume-picker` (V3-only) |
| `v2_non_interactive` | 100 % nightly | **1 % all channels** | "Default non-interactive and piped-stdin to V2 engine instead of V1" — a stable ramp has started (not ACP) |
| `workflows` | 100 % internal, nightly | 100 % internal, **all channels** | still internal-only |
| `v3_prompt` | 0 % all | 0 % all | unchanged, but the description is the strategic signal (below) |
| `tui` 50 % internal · `lite` internal · `memory` internal/nightly · `c2s` internal/nightly · `infra_safety` internal · `tangent` all · `remote_sandbox` all · `voice` all · `remote_changelog` stable · `test*` | — | unchanged | |

`v3_prompt` verbatim: *"Who is offered the V3 ease-in prompt. There is no
feature that flips the default engine to V3 during the ease-in — the prompt is
the only path — so a user is never defaulted onto V3 without being asked.
Dialing this to 0 stops asking; answers already given are honored via the
persisted `chat.agentEngine` setting."* — i.e. the engine switch is an opt-in
prompt (`launch/v3_ease_in.rs`, shipped in 2.20.x), currently offered to nobody.

**`KIRO_FEATURE_CLOUD_CONFIG_ENABLED` is the only new `KIRO_*` token on the
Rust side and it is a dead lever.** It appears exactly twice in the binary —
both inside the `cloud_config` *description*, which says the flag is *"an env
override read by the KAS server (kiro-agent's feature-config provider), not by
this crate"*. The shipped KAS 0.54.8 has **zero** occurrences of that string
and no `cloud_config` entry in its registry (§ 2a). Live confirmation: the
`flagoff` leg (`KIRO_FEATURE_CLOUD_CONFIG_ENABLED=false`, 2.21.0 host) still
emitted the session-start tool call with `rawOutput {kind:"notEnabled"}` —
identical to the no-flag leg. Schema-vs-runtime again: the description
documents a lever the runtime does not implement.

### 2c. Rust env vocabulary delta (2.20.1 → 2.21.0, `kiro-cli-chat` only)

```
+ KIRO_FEATURE_CLOUD_CONFIG_ENABLED   (description-only literal, see above)
+ KIRO_MIDWAY_EXPIRY                  (chat_cli::launch::midway::expiry_wire_value)
+ KIRO_SKIP_MIDWAY_CHECK              (chat_cli::launch::midway::ensure_midway)
```

`launch/midway.rs` is the one new source file (`run_mwinit`, `read_cookie`,
`mcscli_session`, `classify`) — Amazon-internal Midway/mwinit session
handling, paired with the one new embedded doc
`features/midway-session-indicator.md` ("internal users only"). `kiro-cli` and
`kiro-cli-term` env vocabularies are byte-identical across the hop.

---

## 3. Live wire — v2 engine: binary-identical, models unchanged

`probe-v2-ext-methods-ab-2.19.2.py <2.20.1> <2.21.0>` (same hour, same
backend, HOME-isolated): all 11 probed extension methods return the same class
and payload (`commands/options model`, `settings/{list,set}`,
`session/{list,terminate}`, `_session/steer/clear`, `_message/send`,
unknown-method control); `session/new` keys `['models','modes','sessionId']`
on both; `commands/available` 25 commands / 14 tools on both; `_kiro.dev/metadata`
carries `contextUsagePercentage`, `meteringUsage[]`, `turnDurationMs` on both.
`sweep-new-fields.py --diff new old`: **120 = 120 paths, identical**.

**Models on the wire (v2):** `session/new` → `models.availableModels` is the
same 19 ids in the same order with the same descriptions as the 2.19.0
baseline and the 2.20.1 leg; `commands/options model` credit groups are
identical. `kiro-cli chat --list-models --format json` (saved as
`v2-list-models-2.21.0.json`) gives the catalog with numbers — the only place
context windows and multipliers are structured rather than prose:

| model | ctx | ×credits | | model | ctx | ×credits |
|---|---|---|---|---|---|---|
| auto | 1M | 1.0 | | claude-sonnet-4.5 | 200k | 1.3 |
| claude-opus-5 | 1M | 2.2 | | claude-sonnet-4 | 200k | 1.3 |
| claude-sonnet-5 | 1M | 1.3 | | claude-haiku-4.5 | 200k | 0.4 |
| claude-opus-4.8 | 1M | 2.2 | | deepseek-3.2 | 164k | 0.25 |
| gpt-5.6-sol | 272k | 2.4 | | minimax-m2.5 | 196k | 0.25 |
| gpt-5.6-terra | 272k | 1.0 | | minimax-m2.1 | 196k | 0.15 |
| gpt-5.6-luna | 272k | 0.1 | | glm-5 | 200k | 0.5 |
| claude-opus-4.7 / 4.6 | 1M | 2.2 | | qwen3-coder-next | 256k | 0.05 |
| claude-sonnet-4.6 | 1M | 1.3 | | claude-opus-4.5 | 200k | 2.2 |

---

## 4. Live wire — KAS: bundle-identical, models unchanged

`probe-kas-baseline-2.21.0.py` (initialize with fs+terminal capabilities →
`session/new` → drain → `"Reply with exactly: OK"` → drain), four legs in one
hour against one backend:

| leg | host binary | KAS bundle | session-start `tool_call` | sweep paths |
|---|---|---|---|---|
| `live` | 2.21.0 | 0.54.8 (shipped) | **yes** | 253 |
| `pinned` | 2.21.0 | 0.54.3 (`KIRO_KAS_SERVER_PATH`) | **yes** | 253 — identical to live |
| `flagoff` | 2.21.0 + `KIRO_FEATURE_CLOUD_CONFIG_ENABLED=false` | 0.54.8 | **yes** | = live |
| `host2201` | **2.20.1** (`kiro-cli-chat acp --agent-engine kas`) | 0.54.3 (its own) | **no** | 246 |

`live` vs `pinned`: `sweep-new-fields.py --diff` → **253 = 253, identical
field sets**; same 23 `extensionMethods`, same `sessionCapabilities
{list, fork{_meta.kiro.messageId}}`, same `_meta.kiro {checkpoints, sessionList,
policyNotifications}`, same `configOptions` (`mode` 7 · `model` 19 · `autopilot`
2 · `contentCollection` 2), same `session/new._meta` echo keys
(`semanticReviewEnabled`, `disableAutoCompaction`, `ftaEnabled`,
`workflowsEnabled`, `specPlanEnabled`, `specSkipClarificationEnabled`,
`specWorkflow`, `source`, …), same pushes at session creation
(`_kiro/{governance/state, mcp/status, powers/items_changed,
steering/documents_changed, progressive_context/items_changed, tools/didChange,
sessions/changed}`), same `turn_completion` shape (`promptTurnSummaries`
credits, `elapsedTime`, `requestIds`). Zero `_kiro/system/notify` on any leg.
`available_commands_update` still advertises only the five bundled skills.

**Models on the wire (KAS):** the `model` configOption is the same 19 ids /
display names / descriptions as the 08-27 capture on both bundles.

---

## 5. `fetch_cloud_config` — a tool call BEFORE `session/new` returns

The changelog's "[V3] Session-start tool card no longer shows as cancelled" is
this. Frame timing from the `live` leg (seconds from `initialize`):

```
2.264  client  session/new
2.284  agent   _kiro/terminal/shell_type            (host callback)
2.679  agent   session/update  tool_call  "Fetching your cloud config"  kind:other  status:in_progress
                                            _meta.kiro.toolId = fetch_cloud_config
2.680  agent   _kiro/governance/state, _kiro/mcp/status, _kiro/powers/items_changed, …   (7 pushes)
2.713  agent   session/update  config_option_update
2.714  agent   ← session/new RESPONSE
2.715  agent   session/update  available_commands_update
2.730  agent   session/update  session_info_update:context_usage
2.950  agent   session/update  tool_call_update  status:completed  rawOutput {kind:"notEnabled", retracted:false}
```

**Attribution — launcher-gated, binary-new for external users.** The tool call
appears with the 2.21.0 host regardless of bundle (`live`, `pinned`,
`flagoff`) and is absent with the 2.20.1 host (`host2201`), so it is the
`cloud_config` rollout flip (§ 2b: internal → all) supplying the endpoint, not
KAS 0.54.8. The bundle side pre-exists in 0.54.3 (`fetch_cloud_config`,
`"Fetching your cloud config"`, `notEnabled` all 1/1/23 occurrences in both).

**Mechanism (0.54.8):** `startCloudConfigPull(session)` → `pullCloudConfig` →
`if (!this.cloudConfig.isEnabled()) return` → `deterministicToolCalls.run({name:
"fetch_cloud_config", title: "Fetching your cloud config"})` → `sync()`.
Outcomes (metric activity in parentheses): `notEnabled` (disabled) ·
`upToDate` (current) · `synced {downloaded, deleted}` (updated) ·
`incomplete {downloaded}` (partial) · `fellBackToCache {reason}` (stale).
`notEnabled` means the manifest endpoint answered "not entitled" — per the
Rust description, activation is *"governed by KAS's server-side
AB_CLOUD_CONFIG entitlement"*; `retracted:true` would mean previously-synced
cloud files were withdrawn. A `synced`/`retracted` result triggers
`rescanCloudConfigConsumers` (agents, MCP, powers, steering, skills, hooks —
the `/config` panel's six sources) and, before the first prompt,
`resnapshotActiveProfile`. Endpoint plumbing in the bundle:
`KE("cloud-config-endpoint") ?? process.env.CLOUD_CONFIG_ENDPOINT`.

**How the launcher enables it — an env export, not argv.** Read straight from
`/proc/<node-child>/{cmdline,environ}` while each host handled `initialize`
(strict parent-pid match, orphans excluded):

```
argv (both hosts, byte-equal):
  <data>/kiro-cli/node --experimental-wasm-modules <kas>/…/acp-server.js --transport=stdio --auth=acp-callback

env exported to the KAS child          2.20.1 host        2.21.0 host
  CLOUD_CONFIG_ENDPOINT                 (absent)           https://app.kiro.dev      ← the whole change
  KIRO_REMOTE_SESSIONS_ENDPOINT         https://app.kiro.dev   same
  KIRO_CONTENT_COLLECTION_ENABLED       true               true
  KIRO_CUSTOM_USER_AGENT                KiroCLI/2.20.1 KAS/0.54.3 …   KiroCLI/2.21.0 KAS/0.54.8 …
  KIRO_KEY                              ksk_… (launcher-injected secret, both; value not recorded)
```

So `cloudConfig.isEnabled()` is simply "an endpoint is configured", and the
2.21.0 rollout flip makes the launcher export the stage default. Anyone who
spawns `acp-server.js` directly inherits nothing and gets no pull.

**Bearing on cyril — two separate facts.**

1. **Cyril's KAS sessions do not pull cloud config at all.** Cyril's KAS path
   spawns `node acp-server.js --transport=stdio --auth=acp-callback` itself
   (`protocol/kas/discovery.rs`) and never sets `CLOUD_CONFIG_ENDPOINT`.
   Live check with the real bridge (`cargo run --features kas --example
   test_bridge -- --agent-engine kas`): no `ToolCallStarted`/`ToolCallUpdated`
   before or after `SessionCreated`, zero occurrences of `fetch_cloud_config`
   in the harness output or `cyril.log`. Consequence: an org that manages
   agents/MCP/steering/skills/hooks through Kiro cloud config gets **local-only
   sources under cyril** while `kiro-cli` users get the cloud set — a silent
   policy divergence. Exporting `CLOUD_CONFIG_ENDPOINT=https://app.kiro.dev`
   on the direct-node path would restore parity (the bundle then decides
   entitlement server-side via `AB_CLOUD_CONFIG`; non-entitled accounts get
   `notEnabled` as above). Filed cyril-0b8t.
2. **If cyril ever runs KAS through the launcher (or exports the endpoint),
   the ordering above hits the Drop arm.** `classify_notification_route`
   (`crates/cyril/src/app.rs`) maps *scoped, no main session yet, id unnamed*
   to `Drop` + `warn!`, and its rationale comment states that no shipped
   engine produces this ordering — the launcher path now does, on every
   session. Effects would be one `warn!` per session, a `tool_call_update`
   for an id cyril never committed, and nothing user-visible (`kind: other`
   renders as a bare title at most). The seven `_kiro/*` pushes in the same
   window are unmodeled extension notifications and were already ignored.
   Filed cyril-68ag.

Incidentally, `_kiro.dev/commands/options` on KAS answers `-32603
"[PersistenceClassification] Ext method … has no persistence classification.
Add it to KnownExtMethod in persistence-classification.ts."` — that text is
in 0.54.3 too (2 / 2 occurrences); the v2 slash layer is still absent on KAS,
just with a different unknown-method message than the `-32603 "Unknown ext
method"` seen for `_kiro/powers/refresh`.

---

## 6. Embedded doc manifest — 162 → 163

Union of the two manifests (`extract_doc_manifest.py`): 103 + 138 → 103 + 139.
One new node, `features/midway-session-indicator.md` ("Displays remaining
Midway session time in the status line (internal users only)", validated
2026-08-20); `features/session-management.md` revalidated 2026-04-24 →
2026-08-12 (the `/sessions` dashboard GA). Nothing else moved.

---

## 7. Rust-side items that belong to the 2.19.2 → 2.20.1 hop (not 2.21.0)

Same-day archives now exist for all three binaries, so the two hops separate
cleanly. These landed in **2.20.x** and were not covered by the KAS-focused
2.20.1 audit; recorded here for attribution, unprobed:

* `chat_cli::launch::v3_ease_in` (+ `v3_prompt` experiment at 0 %) — the opt-in
  engine switch prompt.
* `chat_cli::pinned_bin{,::materialize,::PinnedBinHeartbeat}` + sqlite tables
  `pinned_bin_versions` / `extracted_kas_versions` (migration 10, 2026-08-28) —
  per-version binary pinning with a last-used heartbeat ("pinned bin GC: reaped
  stale link").
* `chat_cli::cli::update::staging`, `chat_cli::cli::chat::tools::web_fetch` (v1
  crate), `chat_cli_v2::api_client::governance::GovernancePrefetch`,
  `kiro_telemetry::delivery`, `kiro_telemetry::metric::{OsType,OsDistribution}`,
  `kiro_telemetry_observer::observer::TurnState`.

2.20.1 → 2.21.0 symbol-set churn beyond `launch::midway` is LTO re-glue
(`agent_loop`, `line_tracker`, `RequestObservation` reappear; `McpTool`,
`OptOutInterceptor`, `json_streaming`, `sacp::schema::proxy_protocol`
disappear) — no source-path change backs any of it.

---

## 8. Residuals

* **`kiro-cli-chat` shrank 979.3 MB → 838.9 MB (−140 MB) and it is not the
  KAS payload**: the embedded `kas-bundle.tar` gzip stream is 150.3 MB →
  149.8 MB (541 → 543 MB unpacked), the `strings` output only −1.8 MB. The
  drop is in some other non-text blob (tui.js / runtime); unexplained.
* Cyril's KAS discovery (`protocol/kas/discovery.rs`) resolves
  `$HOME/.local/share/kiro-cli/kas` and never consults `XDG_DATA_HOME`,
  while kiro-cli itself honours it — HOME-isolated probes that keep a real
  `XDG_DATA_HOME` work for kiro-cli and fail for cyril (symlink workaround
  used for the § 5 `test_bridge` check). Filed cyril-brui.
* **Probe hygiene:** the python harnesses `terminate()` the kiro-cli host and
  leave its `node acp-server.js` child alive, reparented to PID 1 with the
  temp cwd and a token in memory. 23 orphans from the 2.20.1 and 2.21.0
  probes were found (and killed by cwd match) during this audit; one of them
  answered a `/proc` argv scan before the fresh child did. Filed cyril-fklu.
* `session/new._meta` no longer shows `steeringSupervisorEnabled` on either
  bundle today; the 2.20.1 note recorded it while the supervisor setting was
  being sent. Treat the echo as "keys present when set", not a fixed list.
* `0.54.3 → 0.54.8` semantic changes beyond the literal delta are unknowable
  statically (minified); the identical live inventories bound them to
  non-wire behaviour.

## Artifacts

* `experiments/conductor-spike/probe-kas-baseline-2.21.0.py` — legs `LEG=live|pinned|<any>`,
  `KAS_PIN=<acp-server.js>`, `KIRO_BIN=<host binary>`, `KIRO_PROFILE_ARN=` to
  skip the `whoami` lookup. Captures `kas-baseline-{live,pinned,flagoff,host2201}-2.21.0.jsonl`
  (+ `-verdict.json`), access tokens redacted.
* v2 A/B: `probe-v2-ext-methods-ab-2.19.2.py` reused unchanged →
  `v2-ab-2.21.0-{old,new}.jsonl`; catalog `v2-list-models-2.21.0.json`.
* Sweeps: `sweep-new-fields.py --diff` on `v2-ab new/old` (120 = 120),
  `kas-baseline live/pinned` (253 = 253), `kas-baseline live/host2201`
  (+7 paths, all the § 5 tool call).
* Static: `extract-kas-feature-flags.py` on the carved 0.54.8 bundle;
  `extract_doc_manifest.py` on both binaries; KAS bundle carved from the
  binary's `kas-bundle.tar` gzip stream (offset 7.34 MB) per the 2.13.0 recipe.

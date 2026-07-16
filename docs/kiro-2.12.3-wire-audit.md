# kiro-cli 2.12.2 + 2.12.3 wire/feature audit (2026-07-15)

Baseline: archived 2.12.1 binaries (`~/.local/share/kiro-research/binaries/2.12.1/`).
**2.12.2 (2026-07-13) was never installed locally** — this audit covers both releases in one
diff, 2.12.1 → 2.12.3. Installed 2.12.3 BUILD_DATE `2026-07-15T18:19:23Z` (released same day).

**Verdict: SAFE for cyril's current v2 path** — v2 ACP surface frozen (same-day A/B: zero
field-path drift, 24 commands / 15 tools unchanged). The story of this release pair is on
the KAS side: `@kiro/agent` jumps **0.8.0 → 0.17.2** after three byte-frozen releases, and
the drop is the client-side plumbing for **Kiro cloud/remote agents** — dormant, but
complete and visible in the handshake.

## Changelogs (embedded feed)

- **2.12.2** (fixes only): ACP `--agent` flag now applied to *every* new session (was
  first-session-only, later sessions silently fell back to the default agent + its MCP
  servers); re-loading an already-active session now shuts down the previous instance
  (previously **leaked MCP server processes** — confirms multiple live instances per
  connection were possible); `chat` no longer renders the TUI on piped stdin.
- **2.12.3**: sticky `/model` + `/effort` defaults (opt-outs `chat.disableAutoDefaultModel`,
  `chat.disableAutoDefaultEffort`); rotating startup tips; V3 `/chat save|load` quoted-path
  fix; diff-render fallback to foreground-only colors on light/unknown terminals; MCP OAuth
  discovery sends User-Agent (CloudFront WAF 403s); shell-escape stuck "Running..." fix.

## THE headline — KAS 0.17.2: cloud/remote agent client plumbing (dormant)

KAS tree delta 2.12.1 → 2.12.3 (`~/.local/share/kiro-cli/kas/<ver>-<sha>/`, trees retained
side-by-side; 24 changed / 3 added / 0 removed files; acp-server.js +676 KB):

### New `_kiro/*` methods (bundle string diff, 75 → 78)

- **`_kiro/sourceProviders/list`** — connection-scoped, session-less, no request fields
  ("the picker calls it before a session exists"). Lists an **account-scoped catalog** of
  source providers.
- **`_kiro/sourceProviders/listResources`** — `{providerType, cursor?, limit?}` paged
  listing of one provider's resources. Resources are **repositories**.
- `_kiro/system/` — new connection-scoped notification prefix; concrete:
  **`_kiro/system/notify` `{level, message}`** wired to an `onDelayMessage` hook (system/
  delay messages a client should render as system lines).

Both sourceProviders methods are advertised **only when a catalog is wired**
(`providersConfigured`) — "a client never sees a method the agent would refuse."

### Reachability probe — forced `providersConfigured` (2026-07-15)

`probe-source-providers-2.12.3.py` (log `logs/source-providers-probe-2.12.3-20260715.log`):

- **The dispatch is unconditional** — even unadvertised, calling either method hits the
  handler; the catalog guard returns a *typed* refusal, not method-not-found. Baseline
  (no endpoint): `_kiro/sourceProviders/list` → `-32000 "Source provider catalog error:
  no source provider catalog is configured"`.
- **The lever is `KIRO_REMOTE_SESSIONS_ENDPOINT`** (or `--remote-sessions-endpoint`).
  Setting it constructs `buildRemoteSessionAdapters()` → `BffSourceProviderCatalog` +
  `BffRemoteSessionSource` + `BffRemoteAgentLink` over a `KiroWebPortalServiceClient`, and
  **flips all four dormant caps at once**: `sourceProviders true`, `sessionSources
  ["local","remote"]`, `sessionListScopes ["workspace","user"]`, `executionTargets
  ["local","cloud-sandbox"]`. Both methods then advertise. So the whole cloud surface is
  gated behind one endpoint value — no per-cap flags.
- **But the call stops at KAS host auth, before any HTTP leaves.** With a bogus endpoint
  the catalog op fails at `AcpCallbackAuthProvider`: `-32000 "…listProviders:
  Authentication token is invalid: Host refresh callback returned no access token"`. The
  local mock captured **0 HTTP requests** — `auth.resolveRequestCredential()` (the
  `_kiro/auth/getAccessToken` host callback needing a `profileArn`-bearing token, per the
  launch contract) resolves *before* the `kiro-web-portal-service` request is built. No
  token reached the mock.
- **Upshot:** the sourceProviders methods are real and reachable, and the cloud surface can
  be switched on locally with one env var — but capturing the actual web-portal HTTP
  contract needs a satisfied `getAccessToken` (a valid bearer + profileArn), which the host
  path did not supply in this probe. The auth wall, not the endpoint, is what stands
  between a client and Kiro's cloud backend today.

### Provider types = GITHUB, GITLAB, MIDWAY

`recognizeRepository()`: `gitlab:` scheme → GITLAB, bare name → **MIDWAY** (Amazon-internal
SSO — internal-Amazon deployment), `owner/repo` → GITHUB. Providers report
`connectionStatus` (`CONNECTED` / `not_connected`) with an optional **`setupUrl`** OAuth
handoff. The wire client is an AWS-SDK-style **`kiro-web-portal-service`** TypeScript
client (`ListProviderResourcesCommand`, `bff.source_provider_catalog.*` log tags,
`src/session/bff-remote-agent-link.ts`) — a web-portal backend-for-frontend.

### Remote session store + cloud-sandbox execution

- `ExecutionTargetSchema` = `{kind:"local"} | {kind:"cloud-sandbox"}`, persisted per
  session (`executionTarget`, fixed at creation; absent ⇒ read as local). Placement
  semantics: local → `executedHere`, cloud-sandbox → **`relayed`**.
- "A session always runs locally; it runs on a cloud sandbox only when a **relay** can
  drive one." `cloud-sandbox` token: 0 → 16 occurrences. `_kiro/sandbox/applyConfig`
  (landed 2.11.0) was scaffolding for this.
- `_kiro/session/list` gains `_meta.kiro.listScope` (`workspace` | `user`) and a session
  source dimension (`local` | `remote`); unsupported scope ⇒ typed InvalidParams error.
  `_kiro/session/delete` resolves a target store (local vs remote).
- `resolveAgentContext`: client names now `kiro-web` | `kiro-ide` | `kiro-cli`;
  `executionEnvironment === "sandbox"` ⇒ treated as kiro-web. The **bubblewrap sandbox
  setup code ships in the same bundle** (bwrap args, `/tmp/user_role_creds_dir → /root/.aws`
  bind, Brazil build-cache mounts) — KAS runs on both ends of the relay.
- `multiplex-stream.d.ts` adds `sessionSubscriberCount(sessionId)` — "a relayed session
  reads it to learn whether any client is attached to answer a human-facing callback"
  (permission prompts on cloud sessions with nobody watching). New doc comments codify
  session-scoped routing invariants (cross-session isolation; no replay bleed).

### Handshake delta (live capture, `acp --agent-engine kas`, vs archived 2.12.0 capture)

Exactly **4 new `_meta.kiro` capability keys** on initialize, all reporting the dormant
local-only state on a standalone CLI:

```
executionTargets: ["local"]          # would include "cloud-sandbox" with a wired relay
sessionSources:   ["local"]          # ["local","remote"] with a remote store
sessionListScopes:["workspace"]      # ["workspace","user"] with a remote store
sourceProviders:  false              # true with a wired catalog
```

Advertised extensionMethods unchanged (6, incl. pre-existing `_kiro/session/export`);
sessionCapabilities (fork/list) unchanged. Capture:
`experiments/conductor-spike/logs/kas-host-init-2.12.3-20260715.json`.

### Auth refactor (KAS-1 relevant)

All four auth providers (`acp-callback`, `env`, `file`, `select`) now implement a new
**`RequestCredentialResolver`** interface — one ensure-then-read routine returns an atomic
`{token, profileArn, method, provider}` snapshot so "a refreshed access token can never
pair with a stale profile ARN" (the exact failure mode where a `getAccessToken` reply
missing `profileArn` kills the turn). FileAuthProvider can now source the profile ARN from
the VS Code extension's separate `profile.json` when the token file carries none.

### Other KAS tree changes

- **First native addons in the tree**: `node-addon-api` + `node-gyp-build` +
  **`tree-sitter-powershell`** (first tree-sitter grammar ever shipped in KAS). Likely the
  permission/trust detector getting AST-level PowerShell parsing (2.12.0 tightened bash
  combined-short-option parsing; same safety lane, Windows shell next).
- `tar` dependency bumped (CVE hygiene).
- Portal/control-plane endpoint strings (`kirocontrolplanebearerservice`,
  `runtime.us-east-1.kiro.dev`) — identical between versions, pre-existed 0.8.0.

## Sticky defaults — runtime probe (the changelog is misleading)

Probe: `experiments/conductor-spike/probe-sticky-defaults-2.12.3.py` (select model+effort
via `_kiro.dev/commands/execute` in a live ACP session, inspect settings store, fresh
process + session/new). Logs: `logs/sticky-probe-2.1{1.0,2.1,2.3}-20260715.log`.

| binary | saves `/model` to `chat.defaultModel` over ACP | honors `chat.disableAutoDefaultModel` |
|---|---|---|
| 2.11.0 | **yes** — "Model changed to X (saved as default)" | key didn't exist |
| 2.12.1 | **yes** | **no — ignores it, saves anyway** |
| 2.12.3 | **yes** | **yes — no save, message drops "(saved as default)"** |

- The sticky behavior itself is OLD on the ACP path (≥2.11.0, probably older): the ACP
  `model`/`effort` command handlers have always written the user-global settings. What
  2.12.3 actually adds is (a) the opt-out keys — honored on the ACP path too — and
  (b) persistence from the TUI picker paths.
- Storage: `chat.defaultModel` (string model id) and `chat.modelDefaults`
  (`{<modelId>: {output_config: {effort}}}` — the Anthropic-style effort schema; per-model).
- Applied at `session/new`: fresh sessions come up with `currentModelId` =
  `chat.defaultModel` on all three binaries tested.
- **Cyril implication (long-standing, newly understood):** when a cyril user runs `/model`
  or `/effort`, Kiro **silently rewrites the user's global defaults** — every later session
  (including the Kiro TUI) inherits the choice. Cyril does render the response message, and
  the "(saved as default)" suffix now signals it; a user who wants session-local selection
  must set `chat.disableAutoDefaultModel` / `chat.disableAutoDefaultEffort` (2.12.3+ only —
  on ≤2.12.1 the keys are ignored).
- Also re-confirmed in passing: unprefixed `kiro.dev/commands/execute` → **-32601**; only
  `_kiro.dev/commands/execute` exists (post-2.7.0 `_kiro` migration).

## v2 wire surface — frozen

Same-day A/B captures (`logs/v2-surface-2.12.{1,3}-ab-20260715.{jsonl,summary}`), probe
`probe-v2-surface-ab-2.11.0.py`: initialize + session/new + settle notifications. Zero
field-path drift across 6 message kinds; 24 commands / 15 tools identical; models list and
`currentModelId` semantics identical. The 2.12.2 `--agent` per-session fix and session
re-load shutdown are behavioral (same shapes) — no cyril parser impact.

## feed.json — embedded, not a push channel

`~/.local/share/kiro-cli/feed.json` (118 KB) is the machine-readable changelog feed
(113 entries back to the 2024 CodeWhisperer→Amazon Q announcement; schema supports
`release` + `announcement` types + a hidden 0.0.0 placeholder). It is **embedded in
kiro-cli-chat** (build-time assertion string "feed.json is valid json" trails the payload)
and registered in `chat_cli::util::paths` as a self-extracted asset like tui.js/kas. It
also existed in 2.12.1 binaries — the on-disk file is just newly conspicuous. **No remote
fetch: not an announcement push channel.** Presumably feeds the rotating startup tips.

## nm symbol diff (v2 Rust side)

nm + rustfilt module-path diff, hash-stripped (kiro-cli-chat 92,857 → 92,642 unique
symbols; churn dominated by LLVM/serde noise). **v2 ACP request-handler set byte-identical
— no new ACP method** (all 13 handlers from the `sacp::jsonrpc::Builder` chain: Initialize,
NewSession, LoadSession, Prompt, SetSessionMode, CommandExecute, CommandOptions,
ListSessions, TerminateSession, SettingsList, SettingsSet, SessionSteer,
SessionSteerClear). Confirmed by demangled symbol paths:

- **Sticky defaults machinery** (net-new): `chat_cli_v2::database::settings::{deep_merge,
  Settings::from_data, Settings::get_value, Settings::with_read}`; the opt-out key strings
  are 0 → 6 hits each.
- **2.12.2 `--agent` per-session fix** (net-new):
  `chat_cli_v2::agent::acp::session_manager::SessionManager::resolve_agent_name`;
  `chat_cli::cli::chat::agent_swap::AgentSwapState::{take_pending_swap,take_pending_prompt}`.
- **2.12.2 session-reload shutdown** (net-new): `SessionManager::shutdown_session`
  (0 → 10 syms); `chat_cli_v2::agent::session::SessionDb::close`.
- **MCP OAuth User-Agent**: not symbol-provable (`agent::agent::mcp::oauth_util`
  86 = 86 symbols — in-body header add), consistent with changelog.
- **AddOn billing shapes** (2.12.1 watch item): 13 = 13, identical set — **still dormant**.
- **feed/tips plumbing pre-existing**: `chat_cli::cli::feed::Feed`,
  `constants::tips::get_rotating_tips`, `Database::set_changelog_last_version` in BOTH
  versions — 2.12.3 only switched the welcome screen to use tips; no new Rust plumbing.
- **Watch items (net-new, off-wire today):**
  - Hooks **wire-docs refactor**: `WireHookDocument` (0 → 14), `WireHookAction` (0 → 6),
    `HooksField::to_wire_docs`, `normalize_hook_trigger` — an agent-config hook shape
    change; watch for HookInfo/agent-config compat in future releases.
  - **Experiments/rollout system expanded**: `chat_cli::rollout` 5 → 10 symbols,
    `ExperimentManager::get_experiments` net-new — likely the gate for the sticky-defaults
    rollout; a server-driven feature-flag lane to keep an eye on.
  - New env-detection utils: `util::env_var::{in_ci, in_codespaces, is_remote_fake}`,
    `system_info::which_mwinit`, `util::stdin_is_interactive` (the 2.12.2 piped-stdin
    TUI fix).

Artifacts: scratchpad `nm-diff/` (demangled symbol sets, added/removed lists, cluster
scripts).

## tui.js diff (+42,458 bytes, 12,613,855 → 12,656,313)

Carve verified: embedded expected sha at bundle end == on-disk tui.js sha
(`4f5ef26d…`) — **on-disk tui.js was FRESH this install** (unlike 2.12.1's stale-file
gotcha; still always verify via the embedded sha). Archived as
`~/.local/share/kiro-research/tui-bundles/kiro-tui-2.12.3.js`.

- **Sticky defaults (TUI side)**: persist to the JSON settings file; `/model` handler
  writes `chat.defaultModel` unless the disable key is set; effort persists per-model into
  `chat.modelDefaults[id]` under a net-new **model-catalog field `effortSchemaPath`**
  (`output_config` | `reasoning`, 0 → 7 hits) — Kiro's internal answer to the per-model
  effort-schema split. **`effortSchemaPath` does NOT cross the ACP wire** (session/new
  `availableModels[]` still only `{modelId, name, description}`) — the schema-selection
  problem remains invisible to ACP clients (context for cyril-838u / cyril-1gim). Applied
  at session bootstrap; a `--model` CLI flag wins over the saved default.
- **Rotating tips**: net-new weighted tip pool (`chance`/`requiresFlag`/`surface`/`when`)
  with per-surface arrays (base/tui/lite). The changelog feed is separate: welcome-screen
  changelog reads `KIRO_FEED_FILE` (pre-existing env) pointed at the extracted
  `feed.json`.
- **Diff-render fallback**: new `KIRO_TERMINAL_THEME=dark|light|safe` env + a
  confidence-scored background detector; low/unknown confidence → "safe" theme =
  foreground-only diff colors. Cosmetic.
- **Shell-escape fix**: render component gained `isRunning`; empty output + not-running →
  render nothing instead of a stuck spinner. Cosmetic.
- **`/chat save|load` quoting**: v2 handler has NO quote-stripping (only `~/` expansion) —
  confirms the fix is V3/KAS-only, per changelog.
- **Refusal handling grew** (8 → 13 tokens): `refusal` is now a first-class `stopReason`
  zod literal with two emit paths → `model_refusal {stopReason, category, explanation,
  recommendedModel}`. Cyril note for **cyril-h8zb**: tolerate stopReason `"refusal"` in
  addition to metadata `CONTENT_FILTERED`.
- **New slash commands**: **`/repo` (cloudOnly)** and **`/upgrade-agent`** (V2 →
  "universal V2+V3" agent-config migration) — `/repo` ties into the cloud-session
  substrate (`startedCloudSession` token, sourceProviders client in the TUI).
- **Dormant infra-safety flags**: `chat.enableInfraSafetyMonitor/Enforce`, gated behind
  `KIRO_INFRA_SAFETY_ROLLOUT_ENABLED=1` (family of `_kiro/safety/*`, cyril-3ald). New envs
  also: `KIRO_ENABLED_FEATURES`, `KIRO_TERMINAL_THEME`.
- Zero `kiro.dev/*` wire-method additions/removals. Size attribution: no single blob —
  largest coherent chunks are the KAS cloud-session/sourceProviders client + migration
  panels, then sticky-defaults, tip pool, refusal expansion (small each).

## Doc manifest + metric catalog

- **Doc manifests** (two embedded per binary): small 84 → 86, large 118 → **byte-identical**;
  merged union 136 → 138. The only additions are the two opt-out setting docs —
  `settings/disable-auto-default-{model,effort}.md` (`chat.disableAutoDefaultModel`,
  `chat.disableAutoDefaultEffort`; both `validated: 2026-06-26` — built late June, shipped
  now; absent from the public sitemap = the unannounced-feature signal for this release).
  Binary string cross-check: 0 hits for either key in 2.12.1, 6 each in 2.12.3 — matches
  the runtime probe (2.12.1 ignores the keys). No `category: feature` additions; 8
  validated-date-only touches, incl. `settings/default-model.md` + `slash-commands/{model,
  effort}.md` re-baselined to the same 06-26 date (coordinated defaults-doc rework).
- **Metric catalog** (embedded YAML `MetricsDoc`): set unchanged — 89 metric specs +
  5 critical alarms + 23 legacy `_other_mappings`, identical across versions. Sole delta:
  `kiro_cli.bedrock.request.errors` gains attribute **`failure_reason_code`** (finer
  Bedrock failure attribution). Counting correction for future audits: the catalog's
  structured content is 89 specs + 5 alarms; the "190 metrics" figure carried in earlier
  audit notes came from a looser regex count. The `codewhispererterminal_*` regex A/B
  (18 vs 16) is LTO string-pooling noise — trust the structured YAML diff.
- Reusable extractors + full YAML/JSON artifacts: scratchpad `manifest-diff/`
  (`extract_manifest.py`, `diff_manifest.py`, `extract_metrics.py`).

## Rivets issues filed

- **cyril-v2ol** (P3) — surface/document the sticky-default side effect: `/model` +
  `/effort` rewrite the user's global defaults (always have; opt-outs are 2.12.3+).
- **cyril-tikf** (P3, KAS-4) — model the 4 new initialize capability keys
  (`executionTargets`/`sessionSources`/`sessionListScopes`/`sourceProviders`).
- **cyril-08eh** (P3, KAS-8) — render `_kiro/system/notify {level, message}` instead of
  dropping it (family: cyril-3zy4, cyril-3ald).

## Artifacts

- Binaries: `~/.local/share/kiro-research/binaries/2.12.3/` (+ BUILD-INFO, checksums)
- tui.js: `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.12.3.js` (+ .sha256)
- Captures: `experiments/conductor-spike/logs/v2-surface-2.12.3-ab-20260715.*`,
  `kas-host-init-2.12.3-20260715.json`, `sticky-probe-2.1{1.0,2.1,2.3}-20260715.log`
- Probe: `experiments/conductor-spike/probe-sticky-defaults-2.12.3.py`
- KAS method sets: scratchpad `kiro-methods-2.12.{1,3}.txt` (75→78)

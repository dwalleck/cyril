# Kiro CLI 2.31.1 — client-side MCP governance (v2 Rust engine + classic) — host-static reconstruction

Date 2026-10-16 · release audited **2.31.1** (KAS 0.66.34) · A/B baseline **2.29.1**.
Lane: **host** (Rust). Bins: `kiro-cli-chat` (v2 engine `chat-cli-v2` + classic `chat-cli`), launcher `kiro-cli`,
TUI `kiro-cli-term`. All evidence STATIC (zstd-frame-masked strings) unless marked DOCUMENTED.

Evidence files:
- `experiments/conductor-spike/audit-2.31.1/host/raw/strings/{2.29.1,2.31.1}/{kiro-cli-chat,kiro-cli,kiro-cli-term}.txt`
  (produced by `host/raw/mkstrings.py`; `host/raw/methods-v2.txt`, `host/raw/env/host-env.json`, `host/raw/ctx.py`).
- Prior: `host/raw/../host-findings.md`, `docs/kiro-2.3.0-wire-audit.md` (l.47, l.252),
  `docs/cyril-acp-coverage-vs-2.4.1.md` (l.37, l.153), `docs/kiro-kas-acp-covenant.md` (§9, `_kiro/mcp/status`, `_kiro/governance/state`).

**Dump conventions (read first).** Records are `offset<TAB>printable-run` (`[\x20-\x7e]{4,}`; `mkstrings.py` masks
zstd frames first). rustc stores `tracing`/`format!` literal **pieces** concatenated with the `{}` slots elided,
and `&str` constants are not NUL-terminated, so one record glues many adjacent literals from unrelated code.
Consequences used below:
- `agent '' has  servers:` = source `agent '{}' has {} servers:`; `mcp_enabled=, registry_url=, web_tools_enabled=` =
  `mcp_enabled={}, registry_url={}, web_tools_enabled={}`; `( servers)` = `({} servers)`. `{}` characters *do* exist
  in the dump (90 hits) but only in non-format literals (JSON/serde strings) — for `format!`-family literals the
  slots are absent, so quoted dump text below is **piece text**, not the source format string. [INFERENCE]
- Line-number anchors (`file.rs:NNN`) are `tracing` callsite literals; the message strings near them in a glued run
  are usually (not always) the message of the nearest preceding callsite. Pairings marked ≈ are approximate.
- Confidence: **HIGH** = module path + multiple independent literals agree; **MED** = single literal / glued pairing;
  **LOW** = one soft clue or [INFERENCE] reconstruction.

Version counts quoted `[2.29.1, 2.31.1]` are substring counts in `kiro-cli-chat` unless noted.

---

## 0. TL;DR — what the mechanism is

1. The **v2 engine** (`chat-cli-v2`, what `kiro-cli acp` runs) fetches a **governance config from an API**
   (`api_client/governance.rs`) with three logged fields: `mcp_enabled`, `registry_url`, `web_tools_enabled`.
   Failure is **fail-closed**: `defaulting to disabled` → `MCP disabled, web tools disabled` → session-injected
   MCP servers dropped, user-configured + registry servers suppressed for the session.
2. A **registry** URL (from governance, or the `mcp_registry_url` setting, or a test override) is fetched,
   validated against a rich per-entry schema, cached, and used to **filter** MCP servers per agent/session.
   Without a registry (or for non-enterprise users) MCP is enabled **ungoverned** (`enabling MCP without registry`).
3. Classic `chat-cli` (V1) implements the older shape: registry mode filters agent configs
   (`Before/Applied registry filtering`), non-registry mode ignores registry-type servers
   (`Non-registry mode: ignoring registry-type server` — **not** `on-registry`; `accessMode` does not exist in the dump).
4. On the wire, v2 exposes exactly one governance signal: the `_kiro.dev/mcp/governance_disabled` notification
   (DOCUMENTED payload `{ apiFailure: boolean }`, from 2.3.0). `_kiro.dev/webTools/governance_disabled` appears in the
   dump only inside a process-health **telemetry** name run. **There is no `mcp/status` method on v2.**
5. Org-managed login controls are read from **`/etc/kiro/managed-settings.json`** by
   `chat-cli-v2/src/auth/login_controls.rs` (shared with classic via a relative include): `idc_region`,
   `signin_method` rules (`match`/`exclude`/`effect: deny`), `issuer_url`, `sso_region` validation.
6. Test hook: **`KIRO_TEST_GOVERNANCE_DISABLED`**; an "override registry" path exists but its trigger name is not
   recoverable from the dump (see Open questions).

---

## 1. Governance config fetch — v2 `crates/chat-cli-v2/src/api_client/governance.rs`

| # | exact literal (dump bytes) | source anchor | class | what it proves | conf |
|---|---|---|---|---|---|
| 1.1 | `crates/chat-cli-v2/src/api_client/governance.rs` (also build path `/__w/kiro-cli/kiro-cli/crates/...`) | offsets 4631192 / 4601040 | STATIC | module exists; compiled into `kiro-cli-chat` | HIGH |
| 1.2 | `governance prefetch task failed, refetching` + module `chat_cli_v2::api_client::governance` + field `join_error` | `event crates/chat-cli-v2/src/api_client/governance.rs:66` | STATIC | config is fetched by an **async prefetch task**; a task join error triggers **refetch** | HIGH |
| 1.3 | `Retrieved governance config from API: mcp_enabled=, registry_url=, web_tools_enabled=` | same region (offset 100217200, adjacent to `chat-cli/src/cli/chat/mod.rs:911` run) | STATIC | response fields: `mcp_enabled`, `registry_url`, `web_tools_enabled` (dump slots elided) | HIGH |
| 1.4 | `Failed to get governance config from API: , defaulting to disabled` [2,2] | governance region + session_manager region | STATIC | **fail-closed**: fetch/join/parse failure ⇒ config disabled | HIGH |
| 1.5 | `Non-enterprise user detected, enabling MCP without registry` [1,1] | same glued run as 1.3 | STATIC | non-enterprise users skip the registry gate; MCP enabled without registry | HIGH |
| 1.6 | `Initial MCP state: enabled=` [1,1] (dump: `enabled=/*Registry fetch failed, retrying in 1s`; `/*` is an unrelated adjacent constant — 75 `/*` hits in file) | same run as 1.3 | STATIC | session init logs one MCP-state line before/after registry resolution | MED |
| 1.7 | `BEFORE filtering - Agent '` / `AFTER filtering - Agent '` | same run | STATIC | v2 filters agent MCP server sets at init (capital `Agent`; classic uses `agent`) | MED |
| 1.8 | `MCP disabled, web tools disabled` | session_manager block @100535033 | STATIC | both feature toggles come from the same governance resolution | HIGH |
| 1.9 | `Dropping session-injected MCP servers: MCP disabled by governance` | `session_manager.rs` region (~:1441 glue) | STATIC | ACP `session/new` `mcpServers` are dropped when governance says disabled | HIGH |
| 1.10 | `user-configured and registry MCP servers are suppressed for this session` | @100535280 | STATIC | on failure the suppression covers user config too | HIGH |
| 1.11 | `MCP functionality has been disabled by governance ` / `MCP functionality has been disabled by your administrator.` | @100535161 / @98904506 | STATIC | two messages: API-failure path vs admin-policy path | HIGH |
| 1.12 | state/log fields `mcp_enabled web_tools_enabled mcp_api_failure is_enterprise is_api_key has_registry` (+ `first_err`) | `session_manager.rs:422` (:385 `first_err`) | STATIC | resolution inputs/outputs; `mcp_api_failure` ≈ the wire `{apiFailure}` flag | HIGH |
| 1.13 | `ConversationState` serde fields `mcp_enabled mcp_registry_cache mcp_server_versions mcp_registry_error_type mcp_disabled_due_to_api_failure` (struct 21 elements) | serde dump region | STATIC | persistence of governance/registry state per conversation; **field is `mcp_server_versions` (plural)** | HIGH |
| 1.14 | `mcp_registry_url` (record `mcp_registry_urlsage` @4771488) | 4771488 + neighborhood: `mcp.loadedBefore…` @4771936, `…mcp_registry_err…` @4772000, `chat.fullscreen`/`chat.verbosity`/`tools.args` in the same settings-name cluster | STATIC | persisted **settings/config field name** carrying the registry URL (see §2) | MED-HIGH |

Flow (reconstructed, [INFERENCE] where stated):
`session/new` → resolve governance (prefetch task at governance.rs:66; test hook 1.15/§6) → log `Retrieved governance
config…` or `Failed to get…defaulting to disabled` → if disabled: `MCP disabled, web tools disabled`, drop injected
servers, suppress config+registry servers → else fetch/refresh registry (§2) → filter agent config (`BEFORE filtering`
/`AFTER filtering`) → `MCP governance resolved`.

- **Trigger**: async prefetch during session/agent init (refetch on task failure). No pre-auth trigger visible.
- **Fail-closed**: yes (1.4 → 1.8/1.9/1.10/1.11).
- **Refetch cadence**: `governance prefetch task failed, refetching` (on join error) and registry retry
  `Registry fetch failed, retrying in 1s` (`1s` literal, session_manager block). No periodic interval recoverable.
- **Endpoint/host/verb/auth**: **not recoverable from strings** (no URL/path/operation literal in the governance run).
  [INFERENCE] it rides the v2 HTTP API client (same crate as `api_client/mod.rs`), not a KAS method.

---

## 2. Registry fetch, cache, validation — v2 `chat-cli-v2/src/mcp_registry.rs` + classic `chat-cli/src/mcp_registry.rs`

DOCUMENTED counterpart: the host doc manifest (all four versions) carries
`features/mcp-registry.md` — title **MCP Registry**, validated 2026-04-24, category `feature`,
description *"Enterprise MCP server governance allowing administrators to control which servers users can access"*,
keywords `mcp, registry, governance, enterprise, admin, pro` (`host/raw/docman/doc-manifests-{0..3}.json`).
This is the user-visible feature name; the mechanism below is its client implementation.

### 2a. Fetch/cache/error literals

| # | exact literal | anchor | class | proves | conf |
|---|---|---|---|---|---|
| 2.1 | `Fetching MCP registry from: ` | v2 `mcp_registry.rs` (run after :311/:332) **and** classic `:312` | STATIC | same phrase in both clients | HIGH |
| 2.2 | `Failed to fetch MCP registry: HTTP ` | v2 + classic `:333` | STATIC | HTTP error class | HIGH |
| 2.3 | `Fetched and validated  servers from registry` | v2 (near :311/:332), classic (:348/:357 region) | STATIC | count of validated servers (slot elided) | HIGH |
| 2.4 | `Failed to read response body: ` / `Failed to parse registry JSON: ` / `Registry validation failed: ` | both | STATIC | error taxonomy: transport-read / parse / validation | HIGH |
| 2.5 | `Using cached registry data` / `Cache is stale, refreshing` / `Registry URL changed from '' to '', invalidating cache` | classic `:351` region | STATIC | on-disk/process cache; stale refresh; URL-change invalidation | HIGH |
| 2.6 | `registry HTTP client configuration must be valid` | both | STATIC | client construction is validated up-front (config error class) | HIGH |
| 2.7 | `unreachable refused timed out 404 500 503` (glued run) | classic fetch region | STATIC | retryable/transport error sets incl. HTTP statuses | MED |
| 2.8 | `Registry fetch failed, retrying in 1s` / `Failed to fetch MCP registry` | session_manager block @100535033 | STATIC | retry with 1 s backoff | HIGH |
| 2.9 | `Fetched MCP registry from ` / `Refreshing MCP registry data ( servers)` / `Failed to queue registry refresh for session` | session_manager (`:1264`,`:1284` anchors) | STATIC | per-session refresh plumbing | HIGH |

### 2b. Registry URL source (precedence)

1. Governance `registry_url` from the API config (§1.3) — [INFERENCE] becomes the effective registry URL.
2. Settings key **`mcp_registry_url`** (1.14) — classic mirrors it as `Registry URL changed from '{}' to '{}'`. The
   dump record `mcp_registry_urlsage` (@4771488) sits inside a settings-key cluster (`mcp.loadedBefore` @4771936,
   `…mcp_registry_err…` @4772000, `chat.fullscreen`, `chat.verbosity`, `tools.args`), i.e. a persisted
   settings/config field rather than a log field.
3. Test **override registry**: `Fetched override registry from ` / `Failed to fetch override registry` (glued between
   `KIRO_TEST_GOVERNANCE_DISABLED` and `Failed to get governance config from API` in the session_manager region).
   The switch/env/file that feeds it is **not named anywhere in the dump** (no `*REGISTRY*` env in `env/host-env.json`)
   → Open question L3.

### 2c. Entry schema + per-entry validation (v2 serde tokens)

Types: `McpRegistryResponse{servers}`, `McpRegistryServerEntry`, `RemoteServerConfig`, `McpServerDefinition`,
`EnvironmentVariable`, `PackageConfig`, `serverHttpHeader`, `title`, `remotes`, `packages`, `registryType`,
`registryBaseUrl`, `identifier`, `transport`, `environmentVariables`, `@NPM_CONFIG_REGISTRY`, `docker`
(also `RemoteServerConfig EnvironmentVariable` near `McpServerDefinition`).

Validation error templates (dump, slots elided): `… has empty name`; `' has empty description'`; `' has empty version'`;
`' must have either 'remotes' or 'packages' defined'`; `' has empty remote URL'`; `' has empty package identifier'`;
`stdio`; `' has invalid package argument type '', mu…`; `' has invalid runtime argument type '', …`;
`' has invalid transport type '', must be 'stdio''`; `' has invalid registry type '', must be 'npm', 'pypi', or 'oci''`;
`' must have exactly one package configuration, found '`; `' has invalid remote type '', must be 'streamable-http' or 'sse''`;
`' must have exactly one remote configuration, found '`; `' cannot have both 'remotes' and 'packages' defined'`;
`Registry contains no servers`; `Duplicate server name found: `; `Registry server ''`; `Unknown registry type '' for server '': `.

Error classes → user-facing message (classic): `MCP is disabled because the configured registry at  is not reachable.
Check your network connection` (registry mode, transport failure) and `… contains invalid data. Contact your administrator.`
(validation failure).

### 2d. v2-only `mcp.json` contribution logic

| literal | anchor | proves | conf |
|---|---|---|---|
| `skipping mcp.json contributions from this file` | v2 `mcp_registry.rs:311/:332` run | per-file `mcp.json` contributions can be skipped | HIGH |
| `mcp.json entry dropped: name not in registry` (+ field `server_name`) | v2 `:813` (parse error at `:826` `Failed to parse mcp.json `) | an `mcp.json` server survives only if its name is in the registry | HIGH |
| `Resolved  registry servers for agent config: ` / `Removing unresolved registry servers: ` / `' referenced in tools but not found in registry, skipping` | v2 `:1159`/`:1297`/`:1342`/`:1362` run | resolution of registry names → agent servers; unresolved ones removed; stale tool references skipped | HIGH |

---

## 3. Filtering algorithm — who survives

### 3a. Classic `crates/chat-cli/src/mcp_registry.rs` (V1)

| literal | anchor ≈ | proves | conf |
|---|---|---|---|
| `Before registry filtering - agent '' has  servers:` | :749/:778/:786 run | per-agent pre-filter snapshot | HIGH |
| `Applied registry filtering to agent ' servers validated,  ignored. Agent config preserved with minimal registry entries.` | same run | filter result; **if all servers are filtered out the agent keeps a minimal (registry) entry set** rather than nothing | HIGH |
| `Failed to apply registry filtering to agent '` | same run | filter failure is logged per agent | HIGH |
| `Loaded registry server: ` / `Loaded legacy registry server: ` / `Loaded user-defined server: ` | :450–:486 run | three server classes | HIGH |
| `Non-registry mode: ignoring registry-type server ''` | :814 run (exact dump text; **no `on-registry` string exists** — the substring only occurs inside `Non-registry`) | when no registry is configured, servers marked as registry-type are ignored | HIGH |
| `Registry server '' not found in registry, ignoring` / `' (not in registry)` | :450–:486 run | name-resolution misses are ignored, not fatal | HIGH |
| `Filtering out tool '' - server '' not in registry` | :814 | **tool-level** filtering: tools of non-registry servers are removed in registry mode | HIGH |
| `Duplicate server name found: ` | :450–:486 | name-collision handling exists (first-wins, [INFERENCE]) | HIGH |
| `@NPM_CONFIG_REGISTRY` | :814 run | npm-registry placeholder/env substitution appears in registry config handling | MED |
| `Unknown registry type '' for server '': ` | :814 run | per-server registry type dispatch | HIGH |

### 3b. v2 `chat-cli-v2` session/agent filtering

| literal | anchor | proves | conf |
|---|---|---|---|
| `MCP governance resolved` / ` registry servers disabled for this session` | session_manager @100535161 | resolution outcome logged; registry servers can be session-disabled | HIGH |
| `disabling registry servers until next refresh` | @100536325 | degraded mode: registry servers off until refresh succeeds | HIGH |
| `user-configured and registry MCP servers are suppressed for this session` | @100535280 | full suppression on governance-disabled | HIGH |
| `ACP MCP servers override existing servers in agent config` | session_manager (~:1441) | precedence: `session/new.mcpServers` > agent config servers (when allowed) | HIGH |
| `Dropping session-injected MCP servers: MCP disabled by governance` | session_manager | the drop counterpart of the previous rule | HIGH |
| `MCP functionality has been disabled by governance ` / `…by your administrator.` | @100535161 / @98904506 | two disable reasons (API failure vs admin policy) | HIGH |
| `enabling MCP without registry` (§1.5) | governance region | non-enterprise path = MCP on, no registry filtering | HIGH |

Effective algorithm (reconstructed; [INFERENCE] steps marked):
non-enterprise → MCP enabled, no registry. Enterprise → governance config; on failure disabled (fail-closed).
If enabled with registry URL: fetch→validate→cache → resolve registry names for the agent config → drop unresolved →
filter tools (§3a rules) → `AFTER filtering`. Registry fetch failure (degraded) → keep user servers, disable registry
servers until next refresh (`disabling registry servers until next refresh`).

### 3c. Anchor-name reconciliation (assignment wording)

- `accessMode`: **0 hits in all three 2.31.1 bins (and 0 in 2.29.1 chat)** — no such token; not part of the mechanism.
- `on-registry mode`: does not exist; the exact string is **`Non-registry mode: ignoring registry-type server`**.
- `mcp_server_version` (singular) in the task brief: dump shows **`mcp_server_versions`** (plural) in `ConversationState`.

---

## 4. What reaches an ACP client (cyril) — v2 wire

Method inventory `host/raw/methods-v2.txt` (counts `[2.29.1…2.31.1]`):

| method | counts | class | semantics | conf |
|---|---|---|---|---|
| `_kiro.dev/mcp/governance_disabled` | [1,1,1,1] | DOCUMENTED (2.3.0 wire audit l.47) + STATIC present | **ACP notification**, payload `{ apiFailure: boolean }`; v2 TUI handler `handleMcpGovernanceDisabled` since 2.2.2; 2.3.0 wired the agent to emit it. Cyril silently drops it (`docs/cyril-acp-coverage-vs-2.4.1.md` l.153) | HIGH |
| `_kiro.dev/mcp/startup_status`, `_kiro.dev/mcp/server_init_failure`, `_kiro.dev/mcp/server_initialized`, `_kiro.dev/mcp/oauth_request` | [1,1,1,1]/[2,2,2,2] | STATIC | other MCP lifecycle methods on v2 | HIGH |
| `_kiro.dev/webTools/governance_disabled` | 1 literal occurrence in chat, both versions | STATIC (telemetry context) | appears only in the glued run `Failed to send process health telemetry: ` `_kiro.dev/agent/not_found` `_kiro.dev/agent/config_error` `_kiro.dev/mcp/governance_disabled` `_kiro.dev/webTools/governance_disabled` (event `acp_agent.rs:5987`, module `chat_cli_v2::agent::acp::acp_agent`; adjacent `:5978 Failed to send V2 mode change telemetry: telemetry key must contain exactly 32 bytes`) → these literals double as **telemetry event names**. Not in the ACP method inventory → notification vs telemetry-only = Open question L4 | MED |
| `mcp/status` (`_kiro/mcp/status` or `_kiro.dev/mcp/status`) | **0 hits, all three bins, both versions** | STATIC | **no `mcp/status` on v2**; `_kiro/mcp/status` is a V3/KAS method (`docs/kiro-kas-acp-covenant.md`); v2's closest is `_kiro.dev/mcp/startup_status` | HIGH |
| `_kiro/governance/state`, `_kiro/config/template` | [0,0,0,2] | STATIC + DOCUMENTED | in the **host** strings these occur only inside the 2.31.1 dark-rollout description text (`managed_settings` row: org-managed `/config`,`/mcp`,`/hooks` labels + `_kiro/governance/state` subscription + pre-session `_kiro/config/template` mode seed); the host is **not a party** to them (matches `host-findings.md`). KAS side carries `{sessionId, isEnterprise, features:{mcpEnabled, webToolsEnabled, …}}` (covenant §9). Cyril tracks the drop as cyril-0o7e | HIGH |

Net for cyril (v2): the only live governance signal is `_kiro.dev/mcp/governance_disabled` (`{apiFailure}`).
Enforcement itself is client-side/host-side (servers simply never start / are dropped); there is no status query
method to poll.

---

## 5. `/etc/kiro/managed-settings.json` login controls

Code: `crates/chat-cli/src/auth/../../../chat-cli-v2/src/auth/login_controls.rs` — i.e. **one shared file**
(`chat-cli-v2/src/auth/login_controls.rs`, module `chat_cli::auth::login_controls` in the classic build).
File literal: `/etc/kiro/managed-settings.json` [1,1]. Event callsites observed: 406, 446, 462, 539, 552, 568, 591,
603, 625, 632, 652, 660, 676, 685, 702, 706, 712, 716, 732, 736, 743, 747, 753, 766, 770.

| exact literal (dump) | proves | conf |
|---|---|---|
| `could not read managed settings file` | unreadable file → controls skipped | HIGH |
| `managed settings file is not valid JSON, ignoring login controls` (≈:406) | parse failure is non-fatal | HIGH |
| ` no enterprise sign-in settings were applied.` | summary when no usable controls | HIGH |
| `managed setting is not a region, ignoring it` (≈:446) | `sso_region`/region validation (shape) | HIGH |
| `managed setting is not a region Kiro can sign in to, ignoring it` (≈:462) | region must be a Kiro-supported endpoint region | HIGH |
| `Kiro does not support sign-in in . Check the spelling, or use a region Kiro has endpoints in, such as us-east-1.` | same, surfaced to the login prompt | HIGH |
| `No region was given. The region is only read from the interactive prompt, so --region, the managed `idc_region` setting and a remembered region all seed that prompt …` | **`idc_region` is a managed key**; `--region` + remembered region also seed the prompt; `idc_region` count [1,2] (new occurrence in 2.31.1) | HIGH |
| `managed setting must be an https URL, ignoring it` (≈:660) | `issuer_url` must be https | HIGH |
| `managed setting is not a valid URL, ignoring it` | URL shape validation | HIGH |
| `managed setting contains control characters, ignoring it` / `managed setting is too lon…` | input hardening + length cap | HIGH |
| `the "signin_method" rules in  permit no sign-in method` | rules are keyed **`signin_method`** (list of rules) | HIGH |
| `a signin_method rule's "match" named nothing recognized, ignoring the restriction` | rule fields: `match` | HIGH |
| `unrecognized sign-in method in a rule's "match", dropping it and applying the rest` | per-entry drop semantics | HIGH |
| `unrecognized sign-in method in a rule's "exclude", ignoring the restriction` | rule fields: `exclude` | HIGH |
| `a signin_method rule names too many methods, ignoring the restriction` | list-size cap | HIGH |
| `a signin_method rule's effect is not "deny", ignoring the restriction` | rule field `effect`; only `deny` honored | HIGH |
| `managed settings denied every sign-in method, ignoring the restriction` / `managed settings denied no sign-in method, ignoring the restriction` / `the restriction was ignored and all methods are available.` | degenerate policies are ignored (guard against lockouts) | HIGH |
| ` a "signin_method" rule in  lists an unrecognized method in "exclude" (accepted: )` | accepted-method list is enumerated at runtime (values not in dump) | HIGH |
| `builderid` `awsidc` (glued `builderidawsidcthe restriction was ignored…`) | accepted sign-in method values include `builderid`, `awsidc` (dump bytes lowercase — serde value names; display labels not in dump) | MED |
| ` sign-in is not permitted by your administrator. ` / `Need help signing in? ` / `This sign-in method is not permitted by your administrator.` | user-facing denial copy (login + portal) | HIGH |
| `Missing issuer_url for  auth` / `Missing sso_region for ` / `Unusable sso_region for  auth: ` | auth portal (`chat-cli-v2/src/auth/portal.rs` :126/:140/:260/:437/:459/:686) consumes the validated values | HIGH |

Key list (recovered): **`idc_region`**, **`signin_method`** (rules with `match`/`exclude`/`effect:"deny"`),
**`issuer_url`**, **`sso_region`** — plus the file itself. Additional keys may exist (`client_id`, `scopes`,
`start_url` appear in the auth portal strings but not demonstrably as *managed* keys; `start_url` 11 hits are
AWS-config/`sso_start_url` code) → Open question L6.

---

## 6. Test/override environment variables

| env | counts | anchor | class | proves | conf |
|---|---|---|---|---|---|
| `KIRO_TEST_GOVERNANCE_DISABLED` | [1,1] | glued in session_manager region (`Failed to spawn IPC server: KIRO_TEST_GOVERNANCE_DISABLED` … `Fetched override registry from` … `Failed to get governance config from API`); present in `env/host-env.json` | STATIC | test-only kill switch read by the v2 session manager: [INFERENCE] forces governance-disabled path | HIGH name / MED semantics |
| override registry fetch | `Fetched override registry from ` / `Failed to fetch override registry` (each 1 hit) | same region | STATIC | an override registry can be fetched from somewhere before the governance fallback; **no env var or file name for it in the dump** (no `*REGISTRY*` name in the whole env table) | HIGH literal / LOW trigger |
| other KIRO_TEST_* seen in chat env table | — | `env/host-env.json` | STATIC | `KIRO_TEST_SESSIONS_DIR`, `KIRO_TEST_SETTINGS_PATH`, `KIRO_TEST_AGENTS_DIR`, `KIRO_TEST_DB_PATH`, `KIRO_TEST_MODE`, `KIRO_TEST_LIVE_API`, `KIRO_TEST_DROP_FIRST_END_TURN`, `KIRO_TEST_MOCK_KAS_SESSIONS`, `KIRO_TEST_KAS_TURN_LOG_PATH`, `KIRO_TEST_TUI_JS_PATH`, `KIRO_TEST_CHAT_IPC_SOCKET_PATH`, `KIRO_TEST_HOMEBREW_PREFIX` | HIGH |
| `KIRO_ENABLED_FEATURES` / `KIRO_ROLLOUT_FEATURES` / `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED` | [1,1] each | glued run `…trust_classifier_shadowKIRO_ENABLED_FEATURESKIRO_ROLLOUT_FEATURESKIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED…` | STATIC | feature/rollout overrides; `KIRO_ENABLED_FEATURES` opts into dark features incl. `managed_settings` (host-findings.md) | HIGH |

---

## 7. A/B 2.29.1 vs 2.31.1 (host strings)

| literal | [2.29.1, 2.31.1] |
|---|---|
| `governance prefetch task failed` / `Retrieved governance config from API` / `Non-enterprise user detected, enabling MCP without registry` / `Initial MCP state` / `MCP governance resolved` / `MCP functionality has been disabled by governance` / `registry servers disabled for this session` / `Fetched override registry` / `KIRO_TEST_GOVERNANCE_DISABLED` / `managed-settings.json` / `Non-registry mode` | [1,1] each |
| `Failed to get governance config from API` | [2,2] |
| `signin_method` / `issuer_url` | [6,6] / [5,5] |
| `idc_region` | **[1,2]** (new occurrence: interactive region prompt copy) |
| `accessMode` | [0,0] |
| `governance_disabled` occurrences | 2 in both (1× mcp + 1× webTools literal) |
| `_kiro.dev/mcp/governance_disabled` (methods-v2.txt) | [1,1,1,1] |
| `_kiro/governance/state`, `_kiro/config/template` (methods-v2.txt) | [0,0,0,2] (2.31.0/1 description text only) |
| `kiro-cli`, `kiro-cli-term` governance hits | 0 in both versions |

⇒ The **client-side governance mechanism is byte-stable 2.29.1→2.31.1**; 2.31.x only adds the dark
`managed_settings` rollout row (TUI/KAS side) and login-control surface (`idc_region` copy).

---

## Open questions (live-probe list)

- **L1 Governance API endpoint.** Host/path/verb, auth (SigV4/service client?), response schema beyond the three
  logged fields, and the `is_enterprise`/`is_api_key` inputs that pick the non-enterprise branch. Not present in strings.
- **L2 Refetch cadence.** Prefetch timing (startup vs `session/new`), retry/backoff beyond the observed `retrying in 1s`,
  and whether the registry cache survives across sessions (TTL, location; `mcp_registry_cache`/`mcp_server_versions`
  persistence format).
- **L3 Override registry trigger.** Name/env/file of the "override registry" (`Fetched override registry from `), its
  format, and precedence vs governance `registry_url` vs the `mcp_registry_url` setting. No candidate name in
  `host-env.json`.
- **L4 `_kiro.dev/webTools/governance_disabled`.** Independent ACP notification or telemetry-only event name? Probe:
  force governance API failure with an ACP client subscribed, diff received notifications; compare with the
  `{apiFailure}` payload of the mcp variant.
- **L5 `_kiro.dev/mcp/governance_disabled` trigger/payload in 2.31.1.** Exact `apiFailure` values, emitted per session
  or once per process, and ordering vs `session/new` response.
- **L6 managed-settings.json shape.** Full key set (is it `signin_method` or `signin_methods`? object vs array?),
  size cap value, accepted `signin_method` enum (builderId/awsIdc/externalIdp/google/github?), and whether
  `issuer_url`/`sso_region` are managed keys or only consumed from other sources.
- **L7 `mcp_registry_url` setting.** Exact settings file/scope (global `~/.kiro/settings/*`? workspace?), accepted
  URL schemes (https only? file?), and interaction with governance-provided `registry_url` (which wins when both exist).
- **L8 Non-enterprise detection.** What makes `is_enterprise=false` (token issuer/profile/region/api key) and whether
  it changes web-tools behaviour identically (`Non-enterprise user detected, enabling web tools without GetProfile check`).
- **L9 v2 `_kiro/governance/state`.** Confirm the v2 host never emits it (strings suggest TUI/KAS only).

---

## Re-verification recipes

```bash
cd experiments/conductor-spike/audit-2.31.1/host/raw/strings/2.31.1
# glued-window greps (LC_ALL=C; '.' stops at newline, so windows stay inside one record)
LC_ALL=C grep -o '.\{0,900\}Retrieved governance config from API.\{0,900\}' kiro-cli-chat.txt
LC_ALL=C grep -o '.\{0,900\}MCP governance resolved.\{0,900\}' kiro-cli-chat.txt
LC_ALL=C grep -o '.\{0,200\}Non-registry mode.\{0,300\}' kiro-cli-chat.txt
# offset-range dump (records are offset<TAB>run)
awk '$1>=100534500 && $1<=100536500' kiro-cli-chat.txt
# version counts
for v in 2.29.1 2.31.1; do grep -c 'governance' $v/kiro-cli-chat.txt; done
```

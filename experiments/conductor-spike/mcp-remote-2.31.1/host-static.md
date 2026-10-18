# Remote (HTTP/SSE) MCP servers in the v2 Rust engine — 2.31.1 host-static

**Verified:** 2026-10-16 · **Binary:** `kiro-cli-chat` 2.31.1 (BUILD 2.31.1; KAS 0.66.34) · **A/B:** 2.29.1
**Scope:** the *v2* (`chat-cli-v2` + `agent` crate) end-to-end path for a **remote** MCP server: config intake → filter → connect → auth → ready/failed → tool discovery → ACP notification. KAS (`--agent-engine v3`) is out of scope (sibling lane).
**Engine framing (STATIC):** the live v2 MCP implementation is `crates/agent/src/agent/mcp/{mod,actor,service,oauth_util}.rs` + the shared client `crates/chat-cli/src/mcp_client/{client,oauth_util}.rs`, driven from `crates/chat-cli-v2/src/agent/acp/{acp_agent,session_manager}.rs`; the v2 `/mcp` surface is `crates/chat-cli-v2/src/agent/acp/commands/mcp.rs`; registry is `crates/chat-cli{,-v2}/src/mcp_registry.rs`.

**Evidence classes**
- `STATIC` — literal in the 2.31.1 host strings; anchor = a numeric offset in `experiments/conductor-spike/audit-2.31.1/host/raw/strings/2.31.1/kiro-cli-chat.txt`.
  - **Offset caveat (2026-10-16, spot-checked).** The numeric anchors are **not one space**: some resolve to raw-binary offsets (e.g. `mcp_init_timeout` = 4,805,760 matches `…/binaries/2.31.1/extracted/kirocli/bin/kiro-cli-chat`), others to byte offsets inside the dump file (`RemoteMcpServerConfig` = 460,928), and a few to neither. Treat the **literal text** as the anchor: `grep -F '<literal>'` in the dump, or `grep -abo -F '<literal>'` in the binary when an offset matters.
- `A/B-stable` — same literal byte-present in `…/strings/2.29.1/kiro-cli-chat.txt` (differs only where noted).
- `LIVE-prior` — captured earlier; cited doc.
- `DOCUMENTED` — kiro.dev prose; cited doc.
- `INFERENCE` — placement/shape reasoned from rodata adjacency, not proven.
- `UNKNOWN` — not recoverable from strings.

> **Glue caveat.** The strings dump is *glued* — adjacent literals from the same rodata pool share a line. Message↔`event <file>:<line>` callsite pairing below is by adjacency; where several messages precede several `event` anchors the *set* is verified but the *pairing* is `INFERENCE`. Wire method names are quoted as emitted (`kiro.dev/mcp/...`); ACP serializes the leading underscore (`_kiro.dev/...`) — see §5.

---

## 0. Staged pipeline

```
                      ┌─────────────────────── config intake (§1) ────────────────────────┐
  ~/.kiro/agents/*.json mcpServers ─┐
  agent mcp.json (legacy)           ├─► McpServerConfig untagged enum ──┐
  <ws>/.kiro/settings/mcp.json      │   {Local(6 fields) | Remote(8)}   │
  session/new mcpServers (ACP)      │   RegistryMcpServerConfig (6)     │
  registry remotes[] (via mcp_registry) ──────────────────────────────► │
                      └──────────────────────────────────────────────────┘
                                            │  merge + precedence (§1.6)
                                            ▼
   FILTER (§2)  governance / registry allow-list / includeMcpJson / agent tool settings
                                            │
                                            ▼
   CONNECT (§3)  LocalMcpServerConfig ── TokioChildProcess (stdio)
                 RemoteMcpServerConfig ── StreamableHttpClientWorker<[AuthClient<]reqwest::Client[>]>
                                            │
                                            ▼
   AUTH (§4)  discovery(resource+AS metadata) → DCR | CIMD | pre-registered client
              → loopback redirect → code → token → (re)store
                                            │
                                            ▼
   READY / FAILED (§5)  server_initialized | server_init_failure | startup_status{allStarted|pending|determinable}
                        oauth_request{serverName,oauthUrl} when auth is required
                                            │
                                            ▼
   TOOL DISCOVERY (§6)  tools → agent tool set → re-advertise on tools/list_changed
```

---

## 1. Config intake

### 1.1 Shape inventory (STATIC)

| Literal / struct | Fields (verbatim, in pool order) | Anchor |
|---|---|---|
| `struct LocalMcpServerConfig` (6 elements) | `timeoutMs`, `disabledTools` adjacent in pool | `:460928`, `:1526527`, `:2737785` |
| `struct RemoteMcpServerConfig` (8 elements) | `forceAuth` adjacent in pool | `:460928`, `:1526527`, `:2737785` |
| `struct RegistryMcpServerConfig` (6 elements) | `toolSchema` adjacent in pool | `:460928`, `:1526527` |
| `enum McpServerConfig` (untagged) | variants `LocalMcpServerConfig` \| `RemoteMcpServerConfig` (+`RegistryMcpServerConfig`); error literal `data did not match any variant of untagged enum McpServerConfig` | `:477727`, `:1543533`, `:2744142` |
| `struct AgentConfigV2025_08_22` | `$schema welcomeMessage toolAliases toolsSettings toolSchema mcpServers allowedTools` | `:460928` |
| `struct Agent` (15 elements) | `$schema mcpServers toolAliases allowedTools toolsSettings includeMcpJson welcomeMessage` | `:1499733` |
| registry entry types | `RemoteServerConfig` (3 elements), `McpRegistryServerEntry`, `McpRegistryResponse` (1 element), `servers`, `remotes`, `McpServerDefinition`, `serverHttpHeader`, `PackageConfig`, `registryType`, `registryBaseUrl`, `identifier`, `transport`, `environmentVariables`, `title`, `EnvironmentVariable` (2 elements) | `:6791253`, `:1526527`, `:2654699` region |

`McpServerConfig` untagged ⇒ **discriminated structurally, not by a `type` tag** (INFERENCE, high confidence: the only untagged-enum error literal is present and no `"type"` discriminant string is emitted by the deserializer).

### 1.2 (a) Agent config `mcpServers`

- `mcpServers` is an `Agent` field; `includeMcpJson` gates legacy `mcp.json` inheritance (both STATIC, `:1499733`).
- Tool-selection doc copy: *"you can reference tools brought in by these servers as … just as you would with the servers you configure in the mcpServers field in this config"* (`:2435339`).
- Tool prefixing: *"Subject of the tool name change. For tools in mcp servers, you would need to prefix them with their server names"* (`:2432293`).
- Misconfiguration error: `No mcpServers field found in config` + ` uses the v3 format. Start  chat --agent-engine v3 and select it with --agent.` (`:2432293`) — v2 refuses a v3-shaped agent file.
- Load errors (agent crate): `failed to load global mcp configs` / `failed to load workspace mcp configs` (`crates/agent/src/agent/agent_config/mod.rs:363`), `Skipping unrecognized MCP server config entry` (`…:431`), `Failed to read MCP config from path ` (`agent_config/load.rs`) — `:478075`, `:1543533`, `:2744142`.

### 1.3 (b) `mcp.json` (global + workspace `settings/mcp.json`)

- Path literals: v2 `agents settings/mcp.json prompts …` (`crates/chat-cli-v2/src/util/paths.rs:205`, `:2604744`); classic `cli-agents settings/mcp.json …` (`crates/chat-cli/src/util/paths.rs:260`, `:2476824`). `KIRO_TEST_SETTINGS_PATH` and `KIRO_TEST_SESSIONS_DIR` are **test overrides for those paths** present in the v2 path module (STATIC, `:2604744`).
- Load errors: `Global MCP config not found: ` / `Error loading global mcp config from ` / `Error loading workspace mcp config from ` / `Failed to parse MCP config` (`crates/chat-cli/src/cli/agent/mod.rs:1730`, `:1765`; `:2436078`).
- Legacy handling: `use_legacy_mcp_json` (STATIC), ` is already configured in agent config. Skipping duplicate from legacy mcp.json.`, `[legacy] Server from legacy mcp.json`, ` Server '' is from legacy mcp.json and cannot be removed from agent config.` + `   To remove it, use:  mcp remove --name  --scope workspace`, `Malformed legacy global mcp config detected: . Skipping mcp migration.` (`chat-cli/src/cli/agent/legacy/mod.rs:215/218`, `:2450742`; `:1640361`, `:98826256`).
- `mcp add` flag docs (global vs agent scope): `AGENT Where to add the server to. If an agent name is not supplied, the changes shall be made to the global mcp.json` / `ENV Environment variables to use when launching the server` / `TIMEOUT Server launch timeout, in milliseconds` / `Whether the server should be disabled (not loaded)` / `Overwrite an existing server with the same name` (`:1571357`). Docs URL `https://kiro.dev/docs/cli/mcp` (`:1725132`).
- Registry interaction: `mcp.json entry dropped: name not in registry`, `Failed to parse mcp.json `, ` skipping mcp.json contributions from this file` (`crates/chat-cli-v2/src/mcp_registry.rs:311/332/813`), and `Failed to fetch registry data: { "mcpServers": {} }` / `All registry servers are already enabled in  config` (`:98827188`).
- **"type":"registry" local marker (DOCUMENTED, not re-derivable from v2 strings):** a `mcp.json` entry opts into registry identity with `"type": "registry"` (`docs/kiro-enterprise-mcp-governance.md` §1, from kiro.dev governance/mcp). The v2 Rust string set does **not** contain the literal `type":"registry` or `registryType":"registry`; the nearest v2 evidence is `Non-registry mode: ignoring registry-type server '…'` (governance doc §3). Exact v2 parse of that marker = `UNKNOWN` (§8 Q1).

### 1.4 (c) Powers `~/.kiro/powers/installed/<name>/mcp.json`

- **`powers/installed` has 0 hits in both 2.29.1 and 2.31.1 `kiro-cli-chat` (STATIC, negative).**
- Present: `crates/chat-cli/src/cli/powers.rs` module (`:2451862`: `spawning powers command`, `failed to spawn powers command: bun <…> (bundle: install|uninstall)`, events `powers.rs:56`) and `PowersArgs`/`Manage installed powers`/`UpdateArgs`/`--check` (`:1591302`). Path assembly is therefore **not a literal** (joined at runtime) ⇒ the `<name>/mcp.json` shape is `UNKNOWN` from this artifact; it is DOCUMENTED-adjacent at best. Probe recipe §8 P3.

### 1.5 (d) ACP session-injected `session/new.mcpServers`

- `session/new` is the ACP entry point (`:1578153`, `:6911843`).
- v2 message: `Dropping session-injected MCP servers: MCP disabled by governance` (`crates/chat-cli-v2/src/agent/acp/session_manager.rs` region, `:2608461`).
- Precedence literal: **`ACP MCP servers override existing servers in agent config`** (`:2608461`) — session-injected wins over agent-config entries of the same name (STATIC; direction explicit in the literal, no inference).
- Telemetry source value `acp_injected` (§1.7) confirms it is a first-class source in the init metric.

### 1.6 Merge / precedence (as far as the strings prove)

| Order (highest wins) | Evidence | Class |
|---|---|---|
| ACP `session/new.mcpServers` | `ACP MCP servers override existing servers in agent config` | STATIC |
| Agent config `mcpServers` | `' is already configured in agent config. Skipping duplicate from legacy mcp.json.` (legacy never overrides an agent entry) | STATIC |
| Legacy `mcp.json` (global + workspace) | same literal; gated by `includeMcpJson` | STATIC |
| Registry | allow-list **filter**, not a merge: `mcp.json entry dropped: name not in registry`, `Active server (loaded from registry)`, `Ignored server (not in registry)` | STATIC |
| Governance | outermost: `Dropping session-injected MCP servers: MCP disabled by governance`, `user-configured and registry MCP servers are suppressed for this session` | STATIC (governance doc §3) |

Relative precedence of **global vs workspace** `mcp.json` is not stated by any literal ⇒ `UNKNOWN` (§8 Q2). Existence of both is proven: `failed to load global mcp configs` / `failed to load workspace mcp configs` (`:478075`).

### 1.7 Source enum (STATIC, verbatim)

```
description: Configuration layer that supplied an MCP server.
allowed_values: [registry, global, workspace, agent, acp_injected, unknown]
  - name: mcp_server_name
    description: Raw configured MCP server identity retained only for EMF diagnostics.
```
Anchor `:3309840` (`:102476949` in the embedded metric catalogue). Emitted as the `mcp_server_source` dimension of `kiro_cli_mcp_server_init_total`.

### 1.8 Registry intake

- `crates/chat-cli-v2/src/mcp_registry.rs` events at `:311, :332, :813, :826, :1159, :1297, :1342, :1362`; classic twin at `crates/chat-cli/src/mcp_registry.rs:312,333,348,351,357,450,460,471,473,480,486,749,778,786,814`.
- Fetch: `Fetching MCP registry from: `, `Failed to fetch MCP registry: HTTP `, `Fetched and validated  servers from registry`, `Registry validation failed: ` (`:2592893` region).
- Validation: `Unknown registry type '' for server '': ` (`mcp_registry.rs:1297`), `Duplicate server name found: Registry server ''`, `Registry contains no servers`, ` cannot have both 'remotes' and 'packages' defined`, ` must have either … configuration, found '` (`:6791253`).
- `remotes[]` accepts `transport` ∈ `streamable-http` | `sse` and `serverHttpHeader` (DOCUMENTED: governance/mcp "at most one of `remotes` (`streamable-http`/`sse`) or `packages` (`registryType` npm/pypi/oci)"; also present as literals `serverHttpHeader`, `transport`, `remotes` `:6791253`). The literal `streamable-http` itself is **not** in the v2 Rust string pool — transport is deserialized into `transport` and validated at `mcp_registry.rs:1297` (INFERENCE).
- `/mcp` panel statuses: `Active server (loaded from registry)` / `Ignored server (not in registry)` / `[legacy] Server from legacy mcp.json` / `(empty)` (`:98829037`), `MCP registry is empty (fetch may have failed)` (`v2 commands/mcp.rs` region, `:100540688`), `No MCP registry configured` (same).

---

## 2. Filter

| Step | Literal | Anchor | Class |
|---|---|---|---|
| Governance prefetch fail-closed | `MCP disabled, web tools disabled`; `user-configured and registry MCP servers are suppressed for this session`; `Dropping session-injected MCP servers: MCP disabled by governance` | `:2608461`; governance doc §3 | STATIC |
| Registry allow-list (v2) | `mcp.json entry dropped: name not in registry`; `Removing unresolved registry servers: …`; `BEFORE filtering - Agent …` / `AFTER filtering - Agent …` | `:2592893`; governance doc §3 | STATIC |
| Non-registry mode | `Non-registry mode: ignoring registry-type server '…'` | governance doc §3 (exact literal) | STATIC |
| Agent tool settings | servers/tools per session (`requiredMcpServers` is the **KAS** mechanism) | governance doc §1/§4 | DOCUMENTED |
| Filtered tools (classic) | `Filtering out tool '…' - server '…' not in registry` | governance doc §3 | STATIC |

---

## 3. Transport

### 3.1 Classification

- `McpServerConfig` untagged enum (§1.1): `LocalMcpServerConfig` → legacy implied **stdio**; `RemoteMcpServerConfig` + `forceAuth` → **remote HTTP**. Forced-auth guard (v2 `/mcp auth`): `' is not a remote (HTTP) server; forced auth is only supported for remote servers` (`crates/agent/src/agent/mod.rs` region, `:518638`) — STATIC and unambiguous.
- Transport type names (STATIC, `:4554435`, `:4615263`, `:4700550`, `:450382`):
  - `rmcp::transport::streamable_http_client::StreamableHttpClientWorker<reqwest::async_impl::client::Client>`
  - `rmcp::transport::streamable_http_client::StreamableHttpClientWorker<rmcp::transport::auth::AuthClient<reqwest::async_impl::client::Client>>` ← **auth-wrapped variant = the OAuth path**
  - `rmcp::transport::child_process::TokioChildProcess` (stdio)
  - `rmcp::transport::worker::WorkerTransport<…>`
  - SSE client: `rmcp::transport::common::client_side_sse` (`:3556997` region)
- `rmcp 3.0.0` in **both** 2.29.1 and 2.31.1 (A/B-stable).
- **No `waitForReady` literal** in either version (0 hits). The nearest concepts are the init/test hooks (§5.4) and the timeout below ⇒ a literal `waitForReady` config key is `UNKNOWN`/likely absent (§8 Q3).

### 3.2 HTTP client setup

| Item | Literal | Anchor | Class |
|---|---|---|---|
| UA (MCP OAuth fetches) | `kiro-cli/2.31.1` | `:511500` | STATIC (2.12.3 added UA after CloudFront 403s — prior audit) |
| UA (ACP/API path) | `acp-client/` + `appVersion` `2.31.1`, `AWS_TOOLING_USER_AGENT`, `KIRO_CLI_CLIENT_APPLICATION` | `:2592319` | STATIC |
| Startup timeout env | `KIRO_REQUIRE_MCP_STARTUP_TIMEOUT_SECS` | `:1631327` | STATIC, A/B-stable |
| Strict startup flag | `--require-mcp-startup` / `REQUIRE_MCP_STARTUP` — *"Require all enabled MCP servers to start successfully; exit with code 3 if any fail"* | `:1577714` | STATIC |
| Per-server timeout | `TIMEOUT Server launch timeout, in milliseconds` (`mcp add`); `timeoutMs` field | `:1571357`, `:460928` | STATIC |
| Telemetry timeout marker | `mcp_init_timeout` | `:4805760` | STATIC |
| Session header | `Mcp-Session-Id`, `Last-Event-Id`, `mcp-protocol-version`, `text/event-stream` | `:470770` region | STATIC (rmcp-side) |
| x-mcp-header validation | `rejecting invalid x-mcp-header annotations: ` | `:4554435` region | STATIC (rmcp 3.0.0) |

### 3.3 Error surface (rmcp + kiro)

- `streamable HTTP response stream closed before its final response`
- `delete session success` / `server doesn't support delete session` / `fail to delete session: `
- `unexpected content type: ` / `: could not parse JSON body as a`
- `invalid URL, scheme is missing`, `: could not parse JSON body as a` (hyper/rmcp)
- `HTTP Fetched and validated  servers from registry` (registry leg)
- `failed to create http client` (`aws_common` interceptor, `:2592319`)
- `Connection closed` — **not found as a v2 MCP literal** in 2.31.1 (§8 Q4). The report's `Connection closed` is more likely a hyper/rmcp `Display` string than a kiro literal — the binary contains no such string, so any user-visible text with that wording comes from a dependency or the TUI.

---

## 4. OAuth lifecycle

Two parallel implementations with the same shape: the **agent-crate** one (`crates/agent/src/agent/mcp/oauth_util.rs`, events `:272 :293 :354 :362 :377 :384 :391 :400 :405 :409 :527 :533 :634 :647 :653 :663 :670 :677 :684 :689 :693 :697 :704 :713 :721 :809 :828 :837 :862 :882 :885 :896 :938 :965 :981 :1011 :1020 :1036 :1052 :1192 :1212 :1220 :1291 :1395 :1513 :1517`) and the **shared client** one (`crates/chat-cli/src/mcp_client/oauth_util.rs`, events `:245 :266 :343 :355 :361 :371 :378 :385 :392 :397 :401 :405 :412 :421 :429 :522 :531 :548 :568 :571 :582 :624 :650 :663 :693 :698 :714 :730 :861 :924 :1023 :1141 :1145`). The 2.29.1 audit recorded the agent crate's tracing call-sites at 47 and the `chat-cli` one at 34, and the 2.29.1 issuer fix as "engine-internal `oauth_util.rs` 47→46 / 34→33 tracing call sites, zero new literals" (`docs/kiro-2.29.1-wire-audit.md`).

### 4.1 Discovery

| Literal | Anchor | Class |
|---|---|---|
| `/.well-known/oauth-authorization-server`, `/.well-known/openid-configuration` | `:511118`, `:3556997` | STATIC |
| `/.well-known/oauth-authorization-server/`, `/.well-known/openid-configuration/` (path-inserted forms) | `:3556997` | STATIC (rmcp 3.0.0 `transport/auth.rs:1498`) |
| `oauth-protected-resource` | `:470105` | STATIC |
| `resource metadata discovery url: `, `failed to parse resource metadata as JSON: `, `resource metadata request returned non-200: `, `resource metadata probe returned unexpected status: `, `Failed to parse metadata for discovery … returned non-200` | `:470105` | STATIC |
| `## mcp: rejecting  : no issuer field: issuer `, `issuer mismatch`, `missing issuer`, `no authorization support`, `http error`, `metadata error`, `other error` | `:506843` (agent `oauth_util.rs:1011`) | STATIC |
| `## mcp: rediscovery found only legacy endpoints, trying …` | `:511566` (agent `oauth_util.rs:527`) | STATIC |
| `## mcp: root fallback request to  failed: ` | `:511500` (agent `oauth_util.rs:1395`) | STATIC |
| `Authorization server issuer mismatch: expected , received ` | `:3564195` | STATIC |
| `Authorization server response missing required issuer: expected ` | `:3564195` | STATIC |
| `authorization server metadata omits code_chall…` | `:3557069` (rmcp `auth.rs:1608`) | STATIC |

**Issuer handling.** The 2.29.1 changelog line *"MCP OAuth sign-in in classic and Rust V2 now works with servers that require an issuer"* was located in 2.29.1 to exactly those two `oauth_util.rs` files with **zero new literals**; the issuer literals above are then A/B-stable as *strings*, but the acceptance/verification logic around them is what changed (STATIC-located / `INFERENCE` on the code delta). Behavioural shape: a metadata document whose `issuer` is absent is rejected (`no issuer field: issuer …`) and a mismatch against the expected issuer is fatal (`Authorization server issuer mismatch: expected , received `) — for both the discovery document and the AS response (LIVE-prior: `docs/kiro-2.29.1-wire-audit.md` row 16 / 2.29.1 §"issuer fix").

### 4.2 Client registration (DCR / CIMD / pre-registered)

| Mechanism | Literal | Anchor | Class |
|---|---|---|---|
| DCR | `Dynamic registration failed: `; `client_name`, `client_type`, `redirect_uris`, `grant_types` (rmcp `RegisterClientInput` / `ClientMetadata`) | `:470770`, `:452451`, `:1467702` | STATIC |
| DCR skip on pre-registered | prior: *"Skip Dynamic Client Registration when a custom `client_id` is configured"* | `docs/kiro-2.12.0-wire-audit.md` | LIVE-prior/DOCUMENTED |
| `client_secret` | `with_client_secret`, `pair with`, `clientSecret` field | `:470770`, `:452451` | STATIC |
| CIMD (SEP-991) | `client_id_metadata_document_supported`, `clientMetadataUrl`, `client id metadata authorization failed, falling back` (agent `oauth_util.rs:533`; chat-cli `oauth_util.rs:1145`) | `:470770`, `:452451`, `:509778`, `:511566`, `:1618926` | STATIC |
| CIMD suppressed by secret | `ignoring clientMetadataUrl because clientSecret is set: a client id metadata document identifies a public client` (agent `oauth_util.rs:1291`; chat-cli `:924`) | `:509778`, `:1618926` | STATIC |
| CIMD doc copy | *"URL of a Client ID Metadata Document (SEP-991), used as the client identifier when the authorization server advertises `client_id_metadata_document_supported`. Ignored when `client_id` is set, and downgraded to Dynamic Client Registration when the server does not advertise support."* | `:2039037` (`:99904494`) | STATIC |
| DCR `client_name` fix | prior: 2.12.0 *"MCP OAuth DCR now sends correct `client_name`"* | `docs/kiro-2.12.0-wire-audit.md` | LIVE-prior |
| `OAuthConfig` shape | `clientId clientSecret clientMetadataUrl redirectUri oauthScopes` (5 elements) — also `oauth_scopes` alias/new location | `:452451`, `:2039444` | STATIC |

### 4.3 PKCE + redirect_uri loopback

| Literal | Anchor | Class |
|---|---|---|
| `code_challenge`, `code_challenge_method`, `code_challenge_methods_supported`, `response_type=code`, `client_id=`, `redirect_uri=`, `state=`, `scope=` | `:4978592`, `:2469497`, `:3351102` | STATIC |
| `host must be 127.0.0.1 or localhost, got ` (agent `mod.rs:484` region; chat-cli `oauth_util.rs:693`) | `:506720`, `:1616002` | STATIC |
| loopback bind prefix `://127.0.0.1`, `127.0.0.1:0/authorize?` | `:439879`, `:2469759` | STATIC |
| `Custom loopback redirect URI for the OAuth flow, e.g. `127.0.0.1:7778` or `http://localhost:7778/callback`. Only used to pin the loopback port (and path, when matching a pre-registered app); the host must be `127.0.0.1` or `localhost` and the scheme `http`. If omitted, the OS assigns a random port.` | `:2039272` (`:99904660`) | STATIC |
| `Oauth loopback server cancelled` | `:1619778` | STATIC |
| `.token.json`, `.registration.json`, `.metadata.json` (per-server state files) | `:506843` | STATIC |
| `secrets.json` | `:1629374` | STATIC |
| `client_secret` + redirect change | prior: 2.12.0 *"`redirect_uri` now accepts full URLs with custom paths and validates loopback-only hosts"* | `docs/kiro-2.12.0-wire-audit.md` | LIVE-prior/DOCUMENTED |

Token storage backends: `secrets.json` + the three per-server files above are STATIC; keychain/db backends are **not** proven by this artifact (`secret store` strings in this binary belong to the AWS token path — `loading device registration from secret store`, `Failed to store pkce registration to secret store` at `:2469759`, which is `chat-cli/src/auth/pkce.rs`, **not** MCP) ⇒ `UNKNOWN`/doubtful (§8 Q5).

### 4.4 Refresh / logout / cancel-auth

| Literal | Anchor | Class |
|---|---|---|
| `Authentication failed for MCP server ''. Token refresh failed. Please re-authenticate using /mcp.` | `:518638` (`agent/src/agent/mod.rs` region) | STATIC |
| `token refresh after server rejection failed: ` | `:470770` | STATIC |
| `__reauth__` (internal marker; `agent/src/agent/mod.rs:1581`) | `:518638` | STATIC |
| `/mcp` subcommand set | `Unknown subcommand: . Try /mcp, /mcp list, /mcp add, /mcp remove, /mcp auth, /mcp cancel-auth, or /mcp logout` | `:100540688` | STATIC |
| force auth | `Usage: /mcp auth <server>`, `Forcing authentication for '`, `Failed to start authentication for '` | `:100540902` | STATIC |
| cancel auth | `Aborted authentication for '`, `Failed to abort authentication for '` | `:100540688` region | STATIC |
| logout | `Usage: /mcp logout <server>`, `Failed to remove credentials for '` | `:100540688` region | STATIC |
| persist to disk | `/mcp: agent has no file path, skipping disk persistence`, `/mcp: failed to serialize agent config: `, `/mcp: failed to parse agent config: `, `/mcp: failed to read agent config from `, `/mcp: failed to write agent config to `, `/mcp: persisted MCP changes to ` | `:100540688` region | STATIC |
| auth tool | `authentication`; `Failed to get MCP server info: `; `/mcp: failed to get server info` | `:100540688` region | STATIC |
| `^A` force auth / `^X` abort auth / `^R` remove credentials | prior: 2.11.0 TUI shortcuts | `docs/kiro-2.11.0-wire-audit.md` | DOCUMENTED/LIVE-prior |

`/mcp auth` guard: `' is not a remote (HTTP) server; forced auth is only supported for remote servers` and `No MCP server named '` (`agent/src/agent/mod.rs:1445`, `:518638`).

---

## 5. Status + notifications

### 5.1 Emitted wire surface (v2)

| Method (as emitted) | Payload | When | Class |
|---|---|---|---|
| `_kiro.dev/mcp/startup_status` | `McpStartupStatus` — enum, **3 variants `allStarted` \| `pending` \| `determinable`** (STATIC enum literal; serde names verbatim). Request counterpart `McpStartupStatusRequest` with 1 element (unnamed in pool) | aggregate startup state; `Forwarding …`? no — this one is emitted via the ACP notification writer (`_kiro.dev/mcp/startup_status` adjacent to `_kiro.dev/session/terminate` in the notification-name pool) | STATIC (name + variants); payload field names `UNKNOWN` (§8 Q6) |
| `_kiro.dev/mcp/oauth_request` | `{serverName, oauthUrl}` — cyril decodes exactly these (`convert/kiro.rs:1123-1143`) | when a remote server needs interactive auth (2.29.0 added the URL so SSH/tmux users can open it) | STATIC-cyril + LIVE-prior |
| `_kiro.dev/mcp/server_initialized` | server name; `struct McpServers with 1 element` adjacent | after a successful initialize; also re-emitted after a hot-reload/tool-list change | STATIC |
| `_kiro.dev/mcp/server_init_failure` | `{serverName, error}` — `Forwarding MCP server init failure to client` (`acp_agent.rs:4119`) | auth failure **or** transport failure; also on hot-reload of a changed server (2.10.0 LIVE) | STATIC + LIVE-prior |
| `_kiro.dev/mcp/governance_disabled` | `{apiFailure: bool}` | governance off/fail-closed | STATIC (2.3.0 origin; governance doc §5.1) |
| `_kiro.dev/commands/available` | full `mcpServers` list (replace semantics) | re-advertised on hot-reload and after `MCP server tool list changed, re-advertising` | LIVE-prior (2.10.0 `probe-hotreload-2.10.0.py`) |

**`oauth_cleared` does not exist** — 0 hits in 2.31.1 *and* 2.29.1 (STATIC, negative). Nothing is emitted for logout/cancel-auth; the TUI/`/mcp` reads state directly. (Matches the advisory finding and cyril's matcher set.)

**Aggregate vs per-server.** `server_initialized` / `server_init_failure` are **per server** (both carry the server name — 2.10.0 LIVE showed one `server_init_failure` per changed server). `startup_status` is **aggregate** (`allStarted` vs `pending` vs `determinable`).

### 5.2 Forwarding sites (STATIC, `crates/chat-cli-v2/src/agent/acp/acp_agent.rs`)

```
event acp_agent.rs:4119   Forwarding MCP server init failure to client
_kiro.dev/mcp/server_init_failure
Forwarding OAuth request to client
_kiro.dev/mcp/oauth_request
Forwarding MCP server initialized to client
```
Anchor `:2654699`. The fourth adjacent message — `MCP server tool list changed, re-advertising` (with `acp_agent.rs:1733`) — is the **tool-discovery re-advertisement** trigger (§6).

### 5.3 Engine-internal lifecycle messages (`crates/agent/src/agent/mod.rs`)

Observed message set (anchor `:518638`; callsite lines `1333, 1350, 1352, 1361, 1368, 1422, 1429, 1436, 1440, 1445, 1581, 1702, 1793, 1865, 1880, 1983`):

- `failed to launch MCP config, skipping`
- `MCP server successfully initialized`
- `MCP server is awaiting authorization; continuing without it`
- `MCP server is awaiting authorization; holding startup for it`
- `timed out before all MCP servers could be initialized`
- `MCP server initialization finished`
- `received result from LaunchServer request`
- `failed to launch MCP server`
- `channel unexpectedly dropped during MCP initialization`
- `failed_servers` / `pending_auth_servers` (internal state, `mod.rs:1333`)
- `config.server_name` (field, `mod.rs:1429`)
- `__reauth__` (`mod.rs:1581`)
- `failed to get prompts from server` / `Ambiguous prompt name ''. Multiple servers have this prompt. Use server_name/` / `' not found in any server` / `Failed to get prompt: MCP server failed`
- `failed to send call tool request to the MCP server: ` / `Authentication failed for MCP server ''. Token refresh failed. Please re-authenticate using /mcp.`

`crates/agent/src/agent/mcp/service.rs` (events `216 219 243 273 288 305 323 395 398 401 404 407 683 727 734 737 744 771 775 778 802`; `:438405`): `Poison error encountered: `, `Failed to send auth code`, `Method not found: `.
`crates/agent/src/agent/mcp/actor.rs` (events `361 368 371 385 394 427 469 471 496 502 510`).
`crates/agent/src/agent/mcp/mod.rs` (events `478 484 487 503 582 596 604 618 633 648 660 671 680 698 706 710 719 725 736`): `Tool manager request channel has closed, exiting`; `host must be 127.0.0.1 or localhost, got ` at the `478/484` cluster.

**Failure taxonomy (INFERENCE from the literal set):** *auth-required* → `pending_auth_servers` + `oauth_request` (+ `startup_status: pending`); *auth-failed* → `server_init_failure` with `Authentication failed … Token refresh failed`; *transport* → `server_init_failure` with the rmcp error text; *global timeout* → `timed out before all MCP servers could be initialized` and (unless `--require-mcp-startup`) start proceeds.

### 5.4 Startup timeout / strictness

- `KIRO_REQUIRE_MCP_STARTUP_TIMEOUT_SECS` (`:1631327`) + `--require-mcp-startup` (`:1577714`, exit code **3** on any failure).
- `mcp_init_timeout` (`:4805760`) — telemetry marker emitted when the "all servers initialized" deadline expires.
- `mcp_wait_for_authorization` — telemetry dimension adjacent to `tool_search_enabled`/`mandatory_mcp_names` (`:3226122` region ⇒ attribute set of the turn/agent-init event, `INFERENCE`).

### 5.5 Hot-reload (LIVE-prior 2.10.0, unchanged literals)

`docs/kiro-2.10.0-wire-audit.md` (module `agent::agent::mcp::reconcile`; live probe `probe-hotreload-2.10.0.py`):
- rewriting `.kiro/settings/mcp.json` mid-session ⇒ **~3.5 s later, idle**: `_kiro.dev/mcp/server_init_failure` **and** re-emitted `_kiro.dev/commands/available` carrying the full updated `mcpServers` list (replace semantics, so removals propagate);
- only changed servers restart.
- Session-manager twin (STATIC, `crates/chat-cli-v2/src/agent/acp/session_manager.rs` events `1125 1140 1188 1258`): `Reloading agent configs from disk`, `Agent configs reloaded`, `Failed to reload agent configs`, `Failed to push agent configs to session` (`:2611771`).
- Classic twin (STATIC, `chat-cli/src/cli/chat/conversation.rs:550/565/574`, `:1658204`, `:1725132`): `Failed to reload servers: `, `' not found in active clients`, `Terminating MCP server '' and removing its tools too`; TUI copy `Reloading MCP servers...` / `  Error reloading servers: Changes are in memory only and will be lost on restart.` / `Changes to  agent only apply to current session.` / `  Warning: Could not save to disk: ` (`:98903726`).
- **A/B:** all of these literals are present in 2.29.1 too (checked: `reload servers`, `MCP_STARTUP`, `mcp_init_timeout`, `acp_injected`, `RemoteMcpServerConfig`, `issuer mismatch`, `client_id_metadata_document_supported`, `oauth-protected-resource`, `startup_status`, `rmcp-3.0.0` — identical sets).

---

## 6. Tool discovery

| Fact | Literal | Anchor | Class |
|---|---|---|---|
| Remote tools become agent tools; prefixing | `For tools in mcp servers, you would need to prefix them with their server names`; `SkillComplexpattern…`; `toolsSettings`/`allowedTools` | `:2432293`, `:2596145`, `:1499733` | STATIC |
| Tool-list change ⇒ re-advertise | `MCP server tool list changed, re-advertising` (with `acp_agent.rs:1733`) | `:2654699` | STATIC; on-wire effect = `commands/available` re-emit (LIVE-prior 2.10.0) |
| Tool count/footprint | `number_of_tools`, `all_tool_names`, `loaded_tool_names`, `all_tools_count`, `mcp_tools_token_count_estimate` — *"Estimated token footprint of raw tool specifications advertised by a successfully initialized MCP server"* | `:1572572`, `:3310051` | STATIC |
| Per-server info | `/mcp: failed to get server info`, `Failed to get MCP server info: ` | `:100540688` region | STATIC |
| Removal | `Terminating MCP server '' and removing its tools too`; `' not found in active clients` | `:1658204` | STATIC |
| Registry tool filter (classic) | `Filtering out tool '…' - server '…' not in registry` | governance doc §3 | STATIC |
| KAS analogue (out of scope) | `struct KasMcpStatus` (`servers`), `struct KasMcpServerStatus` (`errorMessage`) | `:1499621`, `:1501425` | STATIC |
| Extra agent-config knob | `mcpServerName` + `without tool specification to include all tools` (glued; single-key agent tool-settings path) | `:2596145` region | `INFERENCE` (§8 Q7) |

**Cyil-relevant:** on the ACP wire, remote-tool discovery is not a dedicated notification — the tool set reaches the client through `_kiro.dev/commands/available` (mcpServers field) and the model's `tool_call`s (`cyril-core` `convert/kiro.rs:1103-1157` covers the three MCP notifications; catch-all `:1284`).

---

## 7. Verified vs inference vs unknown — summary

**VERIFIED (STATIC 2.31.1 + A/B 2.29.1 unless noted)**
1. `_kiro.dev/mcp/{startup_status, server_init_failure, server_initialized, oauth_request, governance_disabled}` are the **only** MCP notifications; **no `oauth_cleared`**.
2. `McpStartupStatus` = `{allStarted | pending | determinable}`.
3. Source enum `[registry, global, workspace, agent, acp_injected, unknown]` (verbatim `allowed_values`).
4. `ACP MCP servers override existing servers in agent config` (session-injected precedence).
5. `/mcp` subcommands `list|add|remove|auth|cancel-auth|logout` + their exact error/usage strings; forced auth restricted to remote (HTTP) servers.
6. `KIRO_REQUIRE_MCP_STARTUP_TIMEOUT_SECS`, `--require-mcp-startup` (exit code 3), per-server `TIMEOUT` (ms).
7. rmcp **3.0.0** streamable-HTTP worker + `AuthClient` wrapper + `TokioChildProcess`; A/B identical.
8. OAuth: `.well-known/oauth-authorization-server|openid-configuration`, `oauth-protected-resource`, CIMD (`client_id_metadata_document_supported`, `clientMetadataUrl`, downgrade-on-unsupported), `client_name`/`client_secret`, loopback-only `redirect_uri` (`127.0.0.1`/`localhost`, scheme `http`), issuer rejection/mismatch literals, `Authentication failed … Token refresh failed. Please re-authenticate using /mcp.`
9. Registry: `remotes`+`packages` exclusivity, `transport`, `serverHttpHeader`, `Unknown registry type`, allow-list filter (`mcp.json entry dropped: name not in registry`).
10. Hot-reload (2.10.0 LIVE): `server_init_failure` + full `commands/available` re-emit ~3.5 s after an `mcp.json` write, while idle.

**INFERENCE**
- `McpServerConfig` untagged ⇒ discriminated structurally (no `type` tag) for agent/`mcp.json` entries.
- Message↔callsite-line pairing inside `agent/src/agent/mod.rs`, `mcp/{service,actor,mod}.rs`, `mcp/oauth_util.rs` (sets verified; pairing by rodata adjacency).
- Failure taxonomy mapping (auth vs transport vs timeout) from the literal set.
- `mcp_wait_for_authorization` as an init-phase telemetry attribute.

**UNKNOWN**
- `session/new.mcpServers` entry schema on the v2 side (does v2 accept `{type,url,headers}` with which validation?).
- `RemoteMcpServerConfig`'s 8 field names (pool shows only `forceAuth` adjacent).
- Exact `startup_status` payload field names.
- Powers path assembly (`powers/installed/<name>/mcp.json`) — 0 literal hits.
- Token storage backends beyond `secrets.json`/`.token.json`/`.registration.json`/`.metadata.json`.
- Whether `"type":"registry"` (DOCUMENTED) is parsed by v2 or only by classic/KAS.

---

## 8. Open questions + live-probe recipes

**Q1 — `"type":"registry"` in v2.** *Probe:* workspace `.kiro/settings/mcp.json` with `{"mcpServers":{"probe":{"type":"registry","url":"http://127.0.0.1:PORT/mcp"}}}` + `mcp_registry_url` set; run `kiro-cli acp`, `session/new {mcpServers:[]}`; assert `/mcp list` shows `Active server (loaded from registry)` vs `Ignored server (not in registry)`, and grep the log for `Non-registry mode: ignoring registry-type server`.
**Q2 — global vs workspace `mcp.json` precedence.** *Probe:* same server name, different `url` in `~/.kiro/settings/mcp.json` and `<ws>/.kiro/settings/mcp.json`; capture which URL the agent dials (local listener on both ports).
**Q3 — `waitForReady`.** 0 literal hits; *probe:* check whether a v3-format agent file (or `mcp.json`) with `waitForReady:true` is silently ignored (`Skipping unrecognized MCP server config entry`) — asserts the key is not v2.
**Q4 — `Connection closed`.** 0 literal hits in v2; *probe:* point at a server that dies mid-SSE and capture the `server_init_failure.error` payload verbatim to see which layer produces the text.
**Q5 — token store.** *Probe:* complete an OAuth flow against a local AS; watch `~/.kiro/` for `secrets.json` vs per-server `.token.json`; assert with `KIRO_HOME` relocation.
**Q6 — `startup_status` field names.** *Probe:* `KIRO_LOG_LEVEL=trace`, `session/new`, and capture the 3 variants (0 servers / 1 slow server / 1 waiting-for-auth) over ACP; diff frame JSON.
**Q7 — agent tool-settings `toolSchema`/server-name gating.** *Probe:* agent file with `toolsSettings` restricting one MCP server; assert `all_tool_names` vs `loaded_tool_names` in the `kiro_cli_mcp_server_init_total` log/telemetry output.
**Q8 — session-injected schema.** *Probe:* `session/new {mcpServers:[{name,type:"http",url:…}]}` vs SSE; record which shapes v2 accepts vs silently drops (log line `Skipping unrecognized MCP server config entry`).

---

## Sources / prior art (consolidated, not re-derived)

- `docs/kiro-2.10.0-wire-audit.md` §Hot-reload — live MCP hot-reload proof (notifications + `commands/available`).
- `docs/kiro-2.11.0-wire-audit.md` — `/mcp auth|cancel-auth|logout`, `chat_cli::mcp_client::oauth_util::start_authorization`, `^A/^X/^R`, stdio console-window fix.
- `docs/kiro-2.12.0-wire-audit.md` — `client_secret`, DCR-skip with custom `client_id`, loopback `redirect_uri` validation, `client_name` DCR fix.
- `docs/kiro-2.12.3-wire-audit.md` — MCP OAuth discovery User-Agent (CloudFront WAF 403).
- `docs/kiro-2.29.1-wire-audit.md` — issuer fix location (agent 47→46, `mcp_client` 34→33 call sites, zero new literals), `oauthUrl` surfacing (2.29.0).
- `docs/kiro-2.3.0-wire-audit.md` — `_kiro.dev/mcp/governance_disabled`.
- `docs/kiro-enterprise-mcp-governance.md` §3/§5.1 — registry filter literals, v2 wire surface.
- `docs/kiro-2.22.0-deserialization-audit.md` §6 — KAS status.
- cyril matchers: `crates/cyril-core/src/protocol/convert/kiro.rs:1103-1157`; catch-all `:1284`.
- Artifacts: `experiments/conductor-spike/audit-2.31.1/host/raw/strings/{2.29.1,2.31.1}/kiro-cli-chat.txt` (all offsets above); `…/audit-2.31.1/host/raw/prose/*`.

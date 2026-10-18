# KAS 0.66.34 — remote (HTTP/SSE) MCP servers: static trace (KAS side)

Scope: how the **KAS (TypeScript) agent** in Kiro CLI 2.31.1 ingests, resolves, connects, authenticates and reports
**remote MCP servers** (`url`-bearing declarations), and what that implies for an ACP client. Host/Rust-engine half is
covered by the sibling artifact `host-static.md` in this directory (v2 `rmcp` engine) — not re-derived here.

## 0. Evidence base and method

| Item | Value |
|---|---|
| Bundle | `~/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/dist/server/acp-server.js` |
| Size / hash | 12,702,158 bytes, sha256 `9a75422c1eed126f033da2cff574c95bc588f054f090583f7d58875c4780ccfb` |
| Version | `@kiro/agent` 0.66.34 (`dist/../package.json`), single `0.66.34` literal in bundle |
| MCP client | inlined `@modelcontextprotocol/client@^2.0.0` + `@modelcontextprotocol/core@^2.0.0`; `@modelcontextprotocol` string itself is stripped from the bundle (esbuild inlines), identified by its distinctive literals (`server/discover`, `SEP-2352`, `mcp.RegistrationRejectedError`, …) |
| Bundle shape | minified, 17,450 lines, `sourceMappingURL` absent; no embedded source paths (checked: no `<file>.js:<line>` matches) → anchors are **byte offsets** |
| Marker legend | `[V]` read directly in the bundle at the cited offset · `[I]` inference/attribution · `[U]` unknown |

Method: `grep -abo -F <literal>` for offsets, `tail -c +<off> | head -c <len> | fold -w 200` for readable slices.
All offsets below are **decimal byte offsets into that one file**; re-verify with
`grep -abo -F '<literal>' <file>`. Every offset in the final revision was re-checked by a scripted sweep that
re-resolved each cited literal and dumped the bundle bytes at the cited offset (412 anchors); drafts whose identifiers
were mis-attributed were corrected in place (e.g. the §4 OAuth helper set, the §2.2 schema constants, the §7 literal
table) — treat any offset that does not resolve to the named token as a stale citation.

Prior audits are **consolidated, not re-derived**: `docs/kiro-2.10.0-wire-audit.md` (hot-reload live proof),
`docs/kiro-2.11.0-wire-audit.md` (`/mcp auth|cancel-auth|logout` on the v2 side), `docs/kiro-2.12.0-wire-audit.md`
(client_secret / DCR skip / loopback redirect), `docs/kiro-2.12.3-wire-audit.md` (OAuth discovery UA),
`docs/kiro-2.7.1-wire-audit.md` (client-observable MCP surface: `_kiro/mcp/status`, `_kiro/tools/didChange` MCP tags,
`session/new.mcpServers`, `secret/*`+`openExternalUrl`+`mcp/elicitation` as gated host callbacks),
`docs/kiro-2.22.0-deserialization-audit.md` §6 (KAS status), `docs/ROADMAP.md:386` (prior `kiro.mcp.<hash>` claim),
`docs/kiro-kas-acp-covenant.md` (§9 wire shapes, §2 capability gating), cyril matchers
`crates/cyril-core/src/protocol/convert/kiro.rs:1103-1157`.

---

## 1. Pipeline at a glance (stage → anchor)

| # | Stage | Code anchor(s) |
|---|---|---|
| S0 | Client capability resolution (`secretStorage`, `openExternalUrl`) | 8,626,340 (parse `_meta.kiro`) / 12,431,106 (apply) / 12,431,363 (`_kiro/openExternalUrl`) |
| S0b | `initializeMcp()` — pool + config manager wiring | 12,419,718 |
| S1 | Intake: `mcp.json` files, powers/plugins, agent front matter, builtin modes, client injection | 11,445,3xx (`_8e`), 11,643,798 (plugin loader), 11,659,7xx (powers), 12,461,034 / 12,514,982 / 12,532,007 (client `declareClient`), 12,420,6xx (agent), 12,385,845 (builtin) |
| S2 | Parse/validate/normalize + env substitution | `Hlt` 3,250,669, `Hba` 3,248,801, `zne` 3,248,446, `Yhr` 3,249,121, `Xhr` 3,250,015, `Wlt` 3,250,880 |
| S3 | Registry: catalog fetch + per-server resolution + accessMode | `OHo` 12,285,809, `loadRegistryCatalog` 12,294,289, `catalogFor` 12,292,849, `getAccessMode` 12,301,812, `nen` 12,314,437, `ren`/`VWu`/`KWu` 12,313,602-12,314,262 |
| S4 | Runtime set + precedence (profile/builtin/client/dispatched/file) | `KOt` 11,884,022, `CQr` 12,244,527, `M8o` 12,243,887, `bI` 393,923 |
| S5 | Pool reconcile / diff | `reconcileMcpForAllSessions` 12,420,6xx, `beginUpdate` 11,426,232, `classifyConfigChange` 11,438,6xx |
| S6 | Connect: transport construction, era negotiation, OAuth | `connectServer` 11,434,x, `FPt` 11,411,403, `connectClient` 11,413,970, `_Le` 3,233,090, `Olt` 3,228,415, `Uii` 3,190,809 |
| S7 | Server-side capability sync (tools/prompts/resources/templates) | `syncResourcesAndTools` 11,421,248, `listToolConfigs` 11,419,908, name sanitizer `Ddu` 11,411,076 |
| S8 | Status + failure surface | `getStatus` 11,430,486, `emitMcpStatus` 12,583,454, `onConnectionFailed` 12,420,073, `disableMcp()` 12,424,197 |
| S9 | Tool-call auth/session recovery, elicitation | 11,378,854 (lazy auth), 11,380,827 (`attemptSessionRecovery`), 11,387,799 (inbound elicitation), 11,678,799 (ACP elicitation ext method) |

---

## 2. Config intake (Q1)

### 2.1 Sources

| Source | Literal / anchor | Notes |
|---|---|---|
| User file | `"mcp.json"` 11,445,4xx; base `<userConfigDir>/settings/mcp.json` 11,446,0xx (`QPt.join(this.userConfigBase,"settings",eWr)`) | [V] JSONC parse (`tWr.parse` 11,446,2xx), field `.mcpServers` (`readConfigFile` 11,446,803) |
| Workspace file | `<workspace>/.kiro/settings/mcp.json` (`QPt.join(n,".kiro","settings",eWr)` 11,445,9xx); **only read when `workspaceTrusted`** (`readWorkspaceFiles` 11,446,870 returns empty map otherwise) | [V] watcher: `watchPaths:[join(".kiro","settings","mcp.json")]` + `userConfigDir` 11,445,4xx |
| Powers (legacy) | same user file, section `powers.mcpServers` (`readPowersConfig` 11,446,834) | [V] power standing gates the server: `resolvePowerServer` 11,659,169 → `{kind:"inForce"|…}`; displaced powers' servers are dropped (`JPt.restate` 11,443,5xx `if(c.kind==="displaced")return[]`) |
| Agent plugins (powers) | `<power dir>/mcp.json` via AgentPluginLoader, wired with `setPluginServers` 11,446,100 / `onPluginServersChanged` 11,656,126 | [V] see §2.4 |
| Agent front matter / custom agent def | `mcpServers` field of the agent definition: schema `jhr` 3,252,004 (= `preprocess(record(unknown)).optional()`), consumed by `runningAgentServers` 12,420,788 (`fos` 12,385,875 `.find(id===modeId)?.mcpServers` for builtin modes; `profileAgentServers` 12,421,116 for registry profiles) and `eos` 12,386,728 → `{profile:{servers,origin:{kind:"profile",agentId,…}}}` | [V] each entry re-parsed with the **same declaration schema** as `mcp.json` |
| Builtin modes | `fos` 12,385,845; sandbox environments get a hardcoded local HTTP server `FEl = {sandbox:{url:"http://127.0.0.1:8080/mcp",waitForReady:!0}}` 12,385,791 | [V] `Khr` detects it as transport `http` |
| Client-injected | `session/new` param `mcpServers` → `KOt.declareClient` (def 11,884,101) 12,461,034; `session/load` param → 12,514,982; `session/set_config_option` `_meta.kiro.mcpServers` → 12,532,007 (this one also calls `reconcileMcpForAllSessions()`) | [V] conversion in `SUo`/`BNu` 11,883,7xx (§2.5) |
| Registry catalog | org/profile `mcpRegistryUrl` → `OHo` 12,285,809 | [V] §2.6 |

**Addendum (2026-10-16): the trust-gate value.** `readWorkspaceFiles` returns an empty map unless the manager's `workspaceTrusted` is set — but in this build the value is **hardcoded true**: the only two constructions of the agent pass `workspaceTrusted:!0` (`O0l` stdio entry 12,698,641; mux entry 12,699,342), and the manager receives it verbatim (`new _8e({…,workspaceTrusted:this.workspaceTrusted,…})` 12,420,543). So CLI-spawned KAS reads `<workspace>/.kiro/settings/mcp.json` unconditionally; the gate matters for other embeddings (an embedder constructing the agent with `false`). Session/hook consumers read the same value via `readWorkspaceTrust:()=>this.workspaceTrusted` (12,413,4xx, 12,563,4xx; hooks `projectHookForWire` 12,593,8xx). [V]

### 2.2 Declaration schema for a remote entry (authoritative)

`zlt` 3,251,960 is the one schema used for **every** non-client declaration (mcp.json, agent, power, plugin after conversion):

```js
Uba  = { env: record(string), timeout: number().min(1), waitForReady: boolean,           // 3,251,424
         disabledTools: string[], autoApprove: string[], versionNegotiation: Ghr }
Bba  = { command: string, args: string[], cwd: string }                                   // 3,251,661
qba  = { url: string, headers: record(string), oauth: Lba, oauthScopes: string[] }        // 3,251,759
zba  = { disabled: boolean, type: string }                                                // 3,251,895
zlt  = object({...Uba, ...Bba, ...qba, ...zba})                                           // 3,251,960
Lba  = { clientId: string?.optional(), redirectUri: string?.optional(), clientMetadataUrl: string?.optional() }  // 3,251,196
Ghr  = enum(["auto","legacy"])                                                            // 3,251,394
```
[V] (all offsets are `grep -abo` hits on the literal `NAME=`; body quoted at 3,251,424-3,252,0xx.)

Consequences:
- **Only `command` xor `url` selects transport** (`Khr` 3,249,004: `if(e.command)…stdio; if(e.url)…http`). There is **no `sse`
  type for the main declaration** — SSE is only reachable as a *fallback* (§3.3) or via **plugin** entries that carry
  `type:"sse"` (plugin conversion `vIu` keeps `{url,headers,type}`; `type` is otherwise ignored by `Khr`). [V]
- `type` is only special-cased for the exact literal `"registry"` (`Xhr` 3,250,015); anything else is carried but unused. [V]
- Registry-type declaration copies **only** `{env, headers, timeout, disabledTools, versionNegotiation}` into
  `disposition.declaration` (`Xhr` 3,250,015) — `url` is not part of a registry declaration. [V]
- No `oauth.clientSecret` field exists in the declaration; secrets come from DCR/pre-registration only. [V]

Remote validation (`Yhr` 3,249,121):
- `new URL(url)` must parse → else `{kind:"invalid-url",url}`; `Llt` 3,244,810 requires **https**, or host
  `localhost|127.0.0.1|::1|[::1]` → else `{kind:"insecure-url"}`.
- `oauth.clientMetadataUrl`: only checked when `oauth.clientId` is **not** set (`bLe` 3,248,377 = non-empty string clientId)
  and it is defined → must pass `Uhr` (3,178,681: `https:` **and** `pathname !== "/"`) else `{kind:"invalid-client-metadata-url"}`. [V]
- `oauth.redirectUri` → `Whr` 3,245,787: accepts `host:port`, `:port`, or a full URL; http only, loopback host only
  (`qii = Set(["localhost","127.0.0.1"])` 3,251,355), no query/fragment, port 1-65535, default path `/oauth/callback`
  (`Wge` 3,251,317). Else `{kind:"invalid-redirect-uri",url,detail}`. [V]
- Errors are later rendered by `Hii` 3,247,682: e.g. `MCP server "<name>" has an invalid oauth.clientMetadataUrl: <url> — it must be an https url with a path (e.g. https://example.com/client-metadata.json).` [V]

Disposition normalisation (`Xhr` 3,250,015, `qlt` 3,250,585, `Wlt` 3,250,880): `registry` → `{status:"registry",declaration}`;
`disabled:true` → `disabled`; valid → `{status:"enabled",config}`; else `{status:"invalid",reason}`;
`{disabled:true,type:"registry"}` → `{status:"registry",declaration,disabled:true}`.

### 2.3 Env / header substitution

- `zne(value, extra)` 3,248,446: replaces `${NAME}` **recursively over strings/arrays/objects** using
  `Object.hasOwn(extra,NAME) ? extra[NAME] : process.env[NAME]`; a **process-env value** whose `.trim()` **starts with `()` stays literal** (injection guard; map values are returned as-is); unknown names are left as `${NAME}`. [V]
- `Hba(declaration,{globalEnv})` 3,248,801: builds `env = substitute(globalEnv) ∪ substitute(declaration.env)`, then
  substitutes the whole declaration with that map; if `env` is empty it is dropped from the output. [V]
- Applied via `Hlt` 3,250,669 (= `zlt.safeParse` → `Hba(parsed,{globalEnv})`): file-family declarations (`Q5r` 11,444,482) receive the manager's map; profile/builtin agent declarations (`yYr` 11,885,788) call `Hlt` with no map → `process.env` only; client-injected servers use the separate `SUo`/`BNu` conversion (§2.5). [V]
- `globalEnvVars` come from the config manager (`setGlobalEnvVars(` 11,446,059); the `getExpandedGlobalEnvVars` getter (11,446,235) has **no call site** in this bundle — [U] which upstream source populates `globalEnvVars` in 2.31.1 (not traced).
- Header substitution for **registry** servers is different (§2.6): registry-supplied header *array* + declared header
  *record*, case-insensitive merge with declared winning (`jWu` 12,313,410).

### 2.4 Plugin (power) remote servers — plugin loader

`fIu` 11,643,798 … `hIu` 11,645,7xx (AgentPluginLoader):
- Requires a known `$schema` (must match `plugin.json`'s Agent Plugins version), exactly the fields `$schema` + `mcpServers`
  (any other field → **all** servers dropped, message `mcp.json field "<f>" is not part of the Agent Plugins MCP
  configuration, so no MCP servers were loaded`). [V]
- Entry types `stdio` / `streamable-http` / `sse`, each with a strict field allow-list
  (`{type,url,headers}` for the remote two; unknown field → whole entry skipped). [V]
- URL rules `r$o` 11,649,763: must parse, scheme `http|https`, **no user-info, no fragment**, and non-loopback must be
  https; headers `n$o` 11,650,516: values must be strings, duplicate case-insensitive names rejected. [V]
- Conversion `KVr` 11,651,048 → `vIu` 11,651,510 for remotes: `{url,headers,type}` (type preserved: `sse` keeps the SSE transport
  available here, unlike main declarations). [V]

### 2.5 Client-injected servers (session injection)

`SUo(r)` 11,883,3xx + `BNu(e)` 11,883,5xx:
- `{name, type:"acp"}` entries are **skipped with a warn**: `mcp.client.transport.unsupported` `{serverName,transport:"acp"}` 11,883,054. [V]
- Accepted fields: `command,args,env[{name,value}]` (stdio) **or** `url,headers[{name,value}]` (remote), plus
  `_meta.kiro.{disabledTools,waitForReady,versionNegotiation}`. `versionNegotiation` outside `auto|legacy` →
  `mcp.client.versionNegotiation.unrecognized` `{serverName,received}` 11,883,404 and the value is dropped. [V]
- Not accepted from clients: `timeout`, `autoApprove`, `oauth`, `oauthScopes`, `disabled`, `cwd`, any `type` other than `acp`. [V]
- Client servers are inserted by `KOt.declareClient` (definition 11,884,101; class `KOt` 11,884,022) as enabled `"on"`, item path
  `mcpServers/<name>` (`EUo` 11,884,6xx), and a read before restatement throws
  `Session <id> declared MCP servers that were not restated before being read` (`declared(t)` 11,884,2xx). [V]

### 2.6 Registry (org catalog)

- **Declaration** (`type:"registry"` in mcp.json/agent): parsed loosely by `zlt`; strict registry schema
  `Ksh = {type:"registry",timeout?,headers?:record,env?:record,disabled?:boolean=false,autoApprove?,disabledTools?}`
  exists at **12,285,024 but has no second reference in the bundle** — the reachable path is `Xhr`'s field copy. [V] (single occurrence each for `Ksh`/`Vsh`)
- **Catalog fetch** `OHo` 12,285,809: https only; retry wrapper `vWu` (12,285,5xx) `maxAttempts:2` with 1 s delay (`SWu=1e3`) and
  `[getMcpRegistry] First attempt failed, retrying`; **10 s** timeout (`gWu=1e4`), **2000** server cap (`MHo=2e3`),
  **10 MiB** body cap (`NHo=10*1024*1024`), `User-Agent: Kiro` (`yWu`), `Accept: application/json`; cache per URL with 300 s TTL
  (`mWu=300*1e3`) and 30 s negative TTL (`v$t=30*1e3`); constants at 12,285,46x-53x; failure → `null` (cached); logs `mcp.registry.invalid.shape` (12,286,276),
  `mcp.registry.entry.invalid` (trace, 12,286,514), `mcp.registry.entries.dropped` (warn, 12,286,597). [V]
- **Catalog entry schema** `hWu` 12,284,513: `{name(min3,max200,regex),title?,description,version(no ranges),packages[x1]?,remotes[x1]?}`
  with XOR refine; `remotes[i] = {type:"streamable-http"|"sse", url, headers:[{name,value}]?}` (`AHo` 12,283,594 / `IHo` 12,283,697). [V]
- **Resolution** `nen(declarations,{accessMode,catalog,envProvider})` 12,314,437:
  - `accessMode` default `"nonRegistry"`; registry declarations are **filtered out** in `nonRegistry`
    (`mcp.registry.resolve.accessModeFiltered` 12,314,649), non-registry declarations are **filtered out** in `registry`. [V]
  - Disabled registry entry → `disabled` (`mcp.registry.resolve.disabled` 12,314,828); missing/unresolvable entry → `unresolvedServers` + `mcp.registry.resolve.unresolved`
    `{catalogSize,inCatalog}` 12,314,998; resolved → `{status:"enabled",config}`. [V]
  - `ren` 12,314,262 → `PHo` 12,285,305 (remote vs local by packages/remotes) → `VWu` 12,313,602 for remotes:
    `{url: substitute(remotes[0].url), headers: merge(registry headers, declared headers), timeout, disabledTools,
    versionNegotiation}` — **no `oauth`/`oauthScopes` for registry servers**. [V] (`KWu` 12,313,903 = local/packages path)
  - `getAccessMode()` 12,301,812 = `!mcpDisabled && registryUrl!=="" ? "registry" : "nonRegistry"`; `registryUrl` is the
    org value `value(Cm.mcpRegistryUrl)` 12,301,7xx; catalog empty unless governed value `mcp === "on"` and URL non-empty
    (`catalogFor` 12,292,849). [V] ⇒ **governed registry mode disables all file/agent-declared servers** [I from the code] .
- `resolvedServers` from the last reconcile are cached in `lastRegistryResolution` (12,420,6xx) and reported in status (§5).

### 2.7 Merge / precedence

- Config-manager level (`JPt.merged` 11,443,9xx): items from `user` + `workspaces(root)` + `powers` + `plugins`, each
  item `{place,stated,server,enabled}`; then filtered by the **configuration standing** map `YPt` 11,441,628
  (`active` wins over non-active, `displaced` skipped) and `UPt` 11,442,341 re-dispositions disabled ones. [V]
- Session level (`CQr` 12,244,527) — **precedence, highest first**:
  `client (session-injected)` ≻ `builtin (mode)` ≻ `profile (agent front matter / custom agent)` ≻ `inherited file servers (mcp.json)`,
  with names shadowed by higher sources; returns `[...fileNotShadowed, ...profileNotInBuiltinOrClient, ...builtinNotInClient, ...client, ...dispatchedNotShadowed]`. [V]
- File servers reach a session only if allowed by the agent's MCP policy (`M8o` 12,243,887 / `B9u` 12,244,027):
  builtin modes inherit all; otherwise `excludedTools` (`*`, `@mcp`, `@<name>`, `@<name>/*`, `mcp_*` patterns via `q9u`/`H9u` 12,244,417)
  denies, `includeMcpJson===true` or `allowedTools==="*"` allows, else `allowedTools` patterns must match; names in
  `ASBX_KIRO_MANDATORY_MCPS` (`bI` 393,923, csv env) always inherit. [V]
- Steady-state re-evaluation is triggered by config changes (watcher → `onConfigChanged` → `reconcileMcpForAllSessions` 11,445,4xx / 12,420,6xx).

---

## 3. Transport (Q2)

### 3.1 Selection, construction, timeouts

`FPt` (connection object) 11,411,403; `connectClient` 11,413,970; pool class `u8e` 11,425,205:
- `constructSSETransport` 11,413,111 → `new Olt(new URL(url),{requestInit:{headers},authProvider})`;
  `constructStreamableHTTPTransport` 11,413,233 → `new _Le(new URL(url),{requestInit:{headers},authProvider})`. [V]
- Selection: `config.transport==="stdio" ? stdio : (fallbackToSSE ? SSE : streamable HTTP)` 11,414,3xx. [V]
- **Connect timeout**: `mcpClient.connect(transport,{signal, timeout: Gge(config.timeout)})` 11,414,526 where
  `Gge(e) = e===undefined ? 60_000 : min(e, 600_000)` (`function Gge` 3,245,733; `Hhr=6e5` 3,251,180 / `Hne=6e4` 3,251,188). [V]
- Pool-level outer bound: `Bdu(connect, c, \`MCP server '<name>' connection timed out after <c>ms\`, ()=>controller.abort())`
  11,437,571 — same clamped value. [V]
- Concurrency: pool semaphore of **4** (`Sema(Fdu` 11,425,312; `Fdu=4, qko=2e3` 11,424,668). [V]
- MCP client identity: `new Bii({name:"kiro",version:"0.0.0"}, …)` 11,412,932; capabilities `elicitation:{form:{},url:{}}` 11,412,5xx. [V]
- `waitForReady` is honoured at pool level: `readyServerNames` 11,429,018 filters `config.waitForReady===true`, and
  `awaitRequiredMcpServers` 12,423,152 waits with `budgetMs = max(server.timeoutMs)`; `waitForServers({signal,budgetMs})`
  11,426,394 / 11,434,839 races the connect promise; the deadline throws `qat` = `McpServersNotReadyError` (-32000,
  `Required MCP server(s) … did not become ready in time. Check the MCP Servers panel, then try again.` 2,610,833/2,610,944);
  the inner budget check logs `Readiness wait exceeded its budget` 11,435,366. [V]

### 3.2 Streamable HTTP (`_Le` 3,233,090)

Read from the class body 3,233,090-3,251,0xx [V]:
- Constructor opts: `{requestInit, authProvider, fetch, skipIssuerMetadataValidation, sessionId, protocolVersion,
  reconnectionOptions ?? Dba, reconnectionScheduler, onInsufficientScope ?? "reauthorize", maxStepUpRetries ?? Nba}` with
  `Nba=1` (3,232,869), `Dba={initialReconnectionDelay:1e3, maxReconnectionDelay:3e4, reconnectionDelayGrowFactor:1.5,
  maxRetries:2}` (3,232,875), header allowlist `Oba=new Set(["authorization","content-type","mcp-protocol-version",
  "mcp-method","mcp-name","mcp-session-id"])` (3,232,980); requests go through `(this._fetch ?? fetch)`. [V]
- request `init` from `requestInit` (headers only from KAS), `mcp-session-id` header captured from responses (`this._sessionId`), 401 → `www-authenticate` parse (`qne` 3,179,098: `resource_metadata`, `scope`, `error`, `error_description`)
  → `_authProvider.onUnauthorized({response, serverUrl, fetchFn})` (errors rethrown marked `pb`/`mcp.authSeamEscape`), then **one retry**
  (`Server returned 401 after re-authentication` on second failure), else `Jh` (`mcp.UnauthorizedError`).
- 403 + `error=insufficient_scope` → `_stepUpAuthorize` 3,234,191 (inner 3,234,290): `"throw"` mode or no OAuth provider →
  `fii` `Insufficient scope…`; else merge scope, re-authorize, retry (`Server returned 403 insufficient_scope after
  step-up re-authorization (retry limit N reached)`); `!=="AUTHORIZED"` → throw `Jh`; else resend.
- 400 + "modern enveloped" body → parse JSON-RPC message(s) and dispatch locally (no error).
- 202 → drain body and start/resume the SSE stream (`_startOrAuthSse` 3,236,075, only once `notifications/initialized`
  has been seen; a GET re-open is triggered after the initialized POST). [V]
- content-type dispatch: `text/event-stream` → `_handleSseStream`; `application/json` → parse & dispatch; other →
  `ClientHttpUnexpectedContent` **`Unexpected content type: <h>`**; non-ok others → `ClientHttpNotImplemented`
  `Error POSTing to endpoint: <body>`.
- Session teardown `terminateSession()` 3,23x: `DELETE` with the session header; **405 tolerated**; else
  `ClientHttpFailedToTerminateSession` `Failed to terminate session: <statusText>`.
- `resumeStream(resumptionToken)` 3,23x → `_startOrAuthSse`; resumption uses the `last-event-id` header (3,236,391) and
  SSE frame `id:` values as tokens; a GET answered with 405 ends the stream (`onRequestStreamEnd()`). [V]
- Reconnection backoff: `_getNextReconnectionDelay` (honours a server-sent SSE `retry:` value) + `_scheduleReconnection`
  with `maxRetries:2`, delays 1 s → ×1.5 → max 30 s (`Dba`). [V]
- Redirects: **no** `redirect:"manual"`, `Location` or `maxRedirects` literal exists anywhere in the `_Le` span — the
  transport relies on the platform `fetch` default redirect semantics. The SSE-side EventSource wrapper does set
  `redirect:"follow"` explicitly (3,169,184). [V for the absence/literal; `[I]` that undici's defaults therefore apply
  unchanged].

### 3.3 SSE (`Olt` 3,228,415) and the 4xx fallback

- `connectClient` catch (11,415,2xx): `l = !fallbackToSSE && u instanceof _x && u.status && u.status>=400 && u.status<500` → retry with
  `{fallbackToSSE:true}` 11,415,651. i.e. **an HTTP 4xx from streamable HTTP downgrades the connection to legacy SSE**. [V]
- SSE transport is also the only path for plugin `type:"sse"` entries (§2.4) — [I] via `constructSSETransport` reuse only when fallback is requested; plugin servers are normal `url` declarations so they *start* as streamable HTTP and only reach SSE via this fallback. [I]
- SSE transport internals (`Olt` 3,228,415, EventSource `Bne` 3,165,224) [V]: the EventSource is built on a custom
  `fetch` with `Accept: text/event-stream` and `_commonHeaders()` = `Authorization: Bearer …` + `mcp-protocol-version`
  only, and sets `redirect:"follow"` (3,169,184). The server's `endpoint` event is resolved as `new URL(data, url)`;
  a cross-origin endpoint is rejected: `Endpoint origin does not match connection origin: <origin>` (3,230,624).
  `_send` before the endpoint arrives → `hn.NotConnected` `Not connected` (3,231,827); a 401 is retried once, any other
  non-ok response throws a **plain** `Error POSTing to endpoint (HTTP <status>): <body>` (3,232,701) — not an `_x` code.
  No resumption support: `resumptionToken`, `last-event-id` and 202 handling are absent from the `Olt` span. [V]
- SSE failure path inside the version-negotiation probe: `Connection closed during the version negotiation probe` 3,186,843 (stdio → treated as legacy). [V]

### 3.4 Protocol-era / version negotiation (`versionNegotiation`)

- Config → client: `jge(e) = e.versionNegotiation ?? "auto"` (3,246,924); host schema `Ghr=k.enum(["auto","legacy"])`
  (3,251,394); SDK default mode constant `_ba="legacy"` (3,201,988). `FPt.pinnedToLegacyEra` = `jge(config)==="legacy"`
  (11,412,233). [V]
- Client construction (`createMcpClient(){` 11,412,502): legacy → `{mode:"legacy"}`; otherwise
  `{mode:"auto",probe:{timeoutMs:eraProbeTimeoutMs()}}` where `eraProbeTimeoutMs(){` 11,412,073 =
  `max(1, min(floor((config.timeout ?? 60000)/2), stdio || timeout set ? 30_000 : 5_000))`
  (`Adu=3e4` 11,410,605, `Idu=5e3` 11,410,613). [V]
- Probe: config parser `Eba` 3,189,372; builder `Tba` 3,190,086 = `{method:"server/discover", params:{_meta:<envelope>}}`
  with id `"server-discover-probe-N"` (3,202,719); connect wiring `Uii` 3,190,809 (`timeoutMs = probe.timeoutMs ?? Slt`
  with `Slt=6e4` 2,998,496, `maxRetries ?? 0`); outcome mapping `fba` 3,186,535. Outcomes:
  result ∩ client modern versions → modern; JSON-RPC error `-32022` (`dba`) with `data.supported` → **corrective** retry or
  error; rpc codes `{-32001,-32020,-32021}` (`pba`) → legacy; HTTP 401/403 → `ClientHttpAuthentication`/`ClientHttpForbidden`
  `Version negotiation failed: the server requires authorization (HTTP 401)` / `…denied access (HTTP 403)`; HTTP ≥500 →
  `EraNegotiationFailed` `Version negotiation failed: the server answered the probe with HTTP <status>`; 4xx/502 body error → rpc
  path else legacy; network error (non-browser or non-TypeError) → `EraNegotiationFailed` `Version negotiation probe failed: <msg>`;
  timeout → stdio legacy / else `RequestTimeout` `Version negotiation probe timed out after <ms>ms`. [V]
- KAS retry on unanswered probe (`function Pdu(` 11,410,621, called 11,415,492): `Pdu(u)` = `Ln && (RequestTimeout | EraNegotiationFailed)` and transport != stdio →
  debug `mcp.era.probe_unanswered` → new client → `connectClient({forceLegacyEra:true})` → `prior:{kind:"legacy"}`; success logs
  `mcp.era.downgraded_after_unanswered_probe`. Telemetry `kiro.io/mcp` metric `connect.negotiated` `{protocolEra,protocolVersion,transport,versionNegotiation}`. [V]
- Legacy-era literals for pin mode (`versionNegotiation:{pin:…}`): SDK-only, not configurable from mcp.json (`Ghr` enum has only `auto|legacy`). [V]

### 3.5 Error taxonomy seen by KAS

- SDK error classes (`mcpBrand` markers, class-name literals 3,195,085-3,197,313; `Lii=Symbol.for("mcp.authSeamEscape")` 3,201,904) [V]:
  `vLe`=`mcp.OAuthClientFlowError`; `Ohr`=`mcp.IssuerMismatchError` (`Issuer mismatch in authorization server metadata (RFC 8414 §3.3)` / `authorization response (RFC 9207): expected X, received Y`);
  `wLe`=`mcp.RegistrationRejectedError` (`Dynamic Client Registration rejected (HTTP <status>): <body>`, fields `status/body/submittedMetadata`);
  `xii`=`mcp.InsecureTokenEndpointError` (`Refusing to send credentials to non-https token endpoint '<url>'. OAuth token requests MUST use TLS (localhost / 127.0.0.1 / ::1 are exempt).`);
  `Rhr`=`mcp.AuthorizationServerMismatchError` (`Authorization server changed between redirect and callback (redirected to X, callback resolved Y); refusing to send authorization_code/code_verifier to a different token endpoint`);
  `fii`=`mcp.InsufficientScopeError`; `Jh`=`mcp.UnauthorizedError` (message `Unauthorized`).
- `pb`/`lba` + `Symbol.for("mcp.authSeamEscape")` (`Lii` 3,201,904) mark auth errors that must escape transport error mapping. [V]
- KAS failure classification in `connectServer` 11,437,9xx: `failedAuthorization = msg==="Unauthorized" || code===401`;
  `timeout_too_low` when `msg.includes("timed out") && config.timeout!==undefined && config.timeout<5_000`, with the hint
  `… The timeout unit is milliseconds — a value of <n> is likely too low. Update "timeout" in your mcp.json (e.g. 60000 for 60 seconds).` [V]
- DCR refusal user message (`Fko` 11,403,409): `The authorization server refused to register Kiro as an OAuth client (HTTP <status>): "<error_description|error≤300>". Dynamic client registration is controlled by whoever administers that server. Ask them to allow this application, or set "oauth.clientId" in mcp.json to a client they pre-registered.` [V]
- `[U]` → resolved: **transport error codes are structured, and network/TLS classification happens in KAS, not the transport** [V]:
  the inlined SDK has an error-code enum `hn` (2,970,833-2,971,707): `NOT_CONNECTED`, `ALREADY_CONNECTED`,
  `NOT_INITIALIZED`, `CAPABILITY_NOT_SUPPORTED`, `REQUEST_TIMEOUT`, `CONNECTION_CLOSED`, `SEND_FAILED`, `INVALID_RESULT`,
  `UNSUPPORTED_RESULT_TYPE`, `INPUT_REQUIRED_ROUNDS_EXCEEDED`, `LIST_PAGINATION_EXCEEDED`,
  `METHOD_NOT_SUPPORTED_BY_PROTOCOL_VERSION`, `ERA_NEGOTIATION_FAILED`, `CLIENT_HTTP_NOT_IMPLEMENTED`,
  `CLIENT_HTTP_AUTHENTICATION`, `CLIENT_HTTP_FORBIDDEN`, `CLIENT_HTTP_UNEXPECTED_CONTENT`,
  `CLIENT_HTTP_FAILED_TO_OPEN_STREAM`, `CLIENT_HTTP_FAILED_TO_TERMINATE_SESSION`; thrown via `_x(code,msg,data)`/`Ln(code,msg)`.
  The transports rethrow raw `fetch` errors unchanged, and the tool-call layer classifies them:
  `$Co=/session (?:not found|expired|has expired|is invalid|was terminated|has been terminated|was invalidated|terminated)|(?:unknown|invalid|expired|missing|no valid) session id|session id not found|mcp-session-id/i` (11,366,245),
  `$lu=/^Error POSTing to endpoint \(HTTP (\d{3})\)/` (11,366,459),
  `Ulu=/connection closed|socket hang up|ECONNRESET|EPIPE/i` (11,366,509), `rAf=hn.RequestTimeout` (11,366,566),
  `Blu=[hn.ConnectionClosed,hn.NotConnected,hn.SendFailed]` (11,366,588), status extractor `UCo(e)` (11,366,624),
  classifier `s5r(e)` (11,366,775). Connect failures map `msg==="Unauthorized" || err.code===401` → `failedAuthorization`
  (11,438,7xx-11,439,3xx). **TLS-class strings** (`CERT_*`, `UNABLE_TO_VERIFY*`, `SELF_SIGNED_CERT_IN_CHAIN`, `ERR_TLS_*`,
  `HOSTNAME_MISMATCH`) exist in the bundle **only** in the AWS-SDK smithy transient-error set `LLu` (12,095,9xx) and an
  OpenSSL reason table (4,220,6xx) — no MCP code path matches them, and the MCP transport slice
  (3,150,000-3,260,000) contains none of `ECONNRESET`/`EPIPE`/`ECONNREFUSED`/`ENOTFOUND`/`CERT_`. [V]

### 3.6 Session recovery (streamable HTTP)

- Transport-level: session id captured; `terminateSession` (DELETE) and `resumeStream` only. [V]
- Tool-call level (`isRecoverableSessionLoss` 11,377,430): a lost *streamable-HTTP* session is recoverable when the error
  classifies as `json-rpc` / `worded-status`, or `session-status` when a session id was carried; recovery =
  `handleSessionReset` 11,378,077 → `attemptSessionRecovery` 11,380,827 (pool `resetServer` + wait for reconnect with deadline
  `c5r`, `sessionResetUnresolved` 11,381,266, `sessionResetUnresolved/waitExpired` diagnostics). Outcomes reported via
  `recordRecoveryOutcome` 11,376,606 and model-visible error reports: `The "<server>" MCP session expired during this call, so it
  is unknown whether "<tool>" took effect. The connection has been re-established…`, `…configuration changed while this call was waiting…`,
  `…was not connectable when <tool> was approved…`. [V]

---

## 4. OAuth (Q3)

### 4.1 Discovery

- 401 with `WWW-Authenticate: Bearer resource_metadata=…` → `resourceMetadataUrl` + `scope` (`qne` 3,179,098 / `Plt` 3,179,463). [V]
- Protected-resource metadata (`Pii` 3,179,666): `GET <well-known "oauth-protected-resource">` with header
  `MCP-Protocol-Version`, path-aware well-known URL (`ZEa` 3,180,338: `/.well-known/<name><path>`), retry at origin root
  when pathname≠"/" and status 4xx/502 (`JEa` 3,180,553); 404 → `Resource server does not implement OAuth 2.0 Protected Resource Metadata.`;
  non-ok → `HTTP <status> trying to load well-known OAuth protected resource metadata.` [V]
- AS metadata (`Nii` 3,181,517): candidate order from `eba` 3,180,955 — `/.well-known/oauth-authorization-server<path>`,
  `/.well-known/openid-configuration<path>`, `<path>/.well-known/openid-configuration` (origin-root variants when path=="/");
  header `MCP-Protocol-Version` + `Accept: application/json`; 4xx/502 → try next; **issuer validation** unless skipped:
  `metadata.issuer` must equal the URL (`Ohr("metadata", url, issuer)`, trailing-slash tolerant). [V]
- Fallback default AS = `new URL("/", serverUrl)` origin (`Dii` 3,182,169). [V]

### 4.2 Issuer binding across legs (SEP-2352)

- Stored credentials carry an `issuer` stamp; missing stamp → console warning (`[mcp-sdk] SEP-2352: stored OAuth credential has no 'issuer' stamp…`);
  callback leg requires the stored AS issuer/URL to match the current one (`Rhr` = `mcp.AuthorizationServerMismatchError`), and
  requires `saveDiscoveryState` to exist or it throws `discoveryState was not available on the callback leg; ensure your provider persists discoveryState alongside codeVerifier`. [V]
- KAS's provider (`s8e`) **does** implement `saveDiscoveryState`/`discoveryState` (→ `.discovery` store key) so the binding is active. [V]

### 4.3 CIMD (client-id metadata document)

- AS metadata flag `client_id_metadata_document_supported===true` (3,177,237) + declaration `oauth.clientMetadataUrl` (validated `Uhr` 3,178,681: https with non-root path) →
  **no registration request**: `client_id = <clientMetadataUrl>` stored with the issuer stamp (`T={client_id:A,issuer:g}`). [V]
- Explicit failure: `clientMetadataUrl must be a valid HTTPS URL with a non-root pathname, got: <A>` (`Lne.InvalidClientMetadata`). [V]
- KAS passes `clientMetadataUrl` into the provider 11,397,6xx and folds it into the credential key (§4.6). [V]

### 4.4 DCR and the refresh-grant withhold retry

- `oba` 3,185,366: requires `registration_endpoint` (else `Incompatible auth server: does not support dynamic client registration`);
  POST JSON client metadata (+scope); non-ok → `wLe{status,body,submittedMetadata}`. [V]
- KAS predicate `Lko` 11,403,027 + retry 11,415,826-11,415,945 (`withholdRefreshTokenGrant` 11,399,865): when the DCR failure has `submittedMetadata.scope` containing
  `offline_access` and status 4xx and the body's `error !== "invalid_redirect_uri"` → `withholdRefreshTokenGrant()`
  (client metadata without `grant_types:["refresh_token"]`) and retry once, logging
  `mcp.oauth.registration_refused_retrying_without_refresh_grant {status}`. [V]
- `oauth.clientId` in mcp.json → pre-registered `client_id` (no DCR); static-client (non-DCR) mode also skips the withhold logic. [V]

### 4.5 PKCE, authorize URL, token exchange, refresh

- Authorize URL (`tba` 3,182,614): `response_type=code`, `client_id`, `code_challenge` + `code_challenge_method=S256`,
  `redirect_uri`, `state` (provider `state()` = `randomUUID()`), `scope`, `resource` (when protected-resource metadata exists),
  plus `prompt=consent` when scope includes `offline_access`; AS metadata must advertise `code` + `S256`. [V]
- Token exchange (`Oii` 3,183,730) `grant_type=authorization_code&code&code_verifier&redirect_uri`; auth per
  `client_secret_basic|client_secret_post|none` (`qEa` 3,172,410 / `zEa` 3,172,866); refresh `grant_type=refresh_token`. [V]
- Retry policy (`Dlt` 3,174,262): `invalid_client`/`unauthorized_client` → clear client+tokens and retry once; `invalid_grant` → clear tokens and retry once. [V]
- Refresh failure that is a non-`ServerError` OAuth error aborts; `ServerError` falls through to a fresh interactive authorization. [V]
- Refresh token grant is declared by default (`grant_types:["authorization_code","refresh_token"]`, `_refreshTokenGrant="declared"`) and persisted per connection. [V]
- Loopback redirect server (`a8e=class` 11,403,816): `node:http` server on `host:"localhost"`, `path:"/oauth/callback"`, **port 0 (ephemeral)**
  unless `oauth.redirectUri` pins it; success page (HTTP 200, HTML) / error page (400); resolves `{code,state,iss?}`;
  "This authorization request has expired or already completed. Please retry from Kiro." when no pending resolver; 5 s graceful-stop ceiling then force-close. [V]

### 4.6 `secretStorage` contract (what exactly is stored)

Storage class `o8e` 11,394,369 (`keyPrefix="kiro.mcp"` 11,394,408) [V]:

- `calculateConnectionHash(url, headers, oauth)` 11,394,552:
  `sha256( url.trim().toLowerCase() + "|" + Object.keys(headers).sort().map(k=>`${k}:${headers[k]}`).join("|") [+ `|cimd:${oauth.clientMetadataUrl}` when no clientId and clientMetadataUrl defined] )`. [V]
- Keys: `kiro.mcp.<hash>.client` / `.tokens` / `.verifier` / `.discovery` (getters 11,394,805). [V]
- Stored JSON per key [V]:
  - `.client` — `{client_id, client_secret?, client_id_issued_at?, client_secret_expires_at?, token_endpoint_auth_method?, refresh_token_grant:"declared"|"withheld", created_at}`
  - `.tokens` — `{access_token, refresh_token?, token_type, expires_in?, scope?, created_at, expires_at(created_at+expires_in*1000)?}`
  - `.verifier` — `{code_verifier, created_at}`
  - `.discovery` — `{discovery_state:{authorizationServerUrl, resourceMetadataUrl?, resourceMetadata?, authorizationServerMetadata?}, created_at}`
- Validation failures delete just that key (`clearCredentials("client"|"tokens"|"verifier"|"discovery"|"all")` 11,398,080). [V]
- ACP backend `s$t` 12,245,217: `get` → `_kiro/secret/get {key}` → `{value}` (warn `[AcpSecretStorage] Failed to retrieve secret`),
  `store` → `_kiro/secret/store {key,value}`, `delete` → `_kiro/secret/delete {key}` (warn + rethrow). [V]
- **Without the `secretStorage` capability there is no persistence backend at all in 0.66.34**: `this.secretStorage` is assigned
  only inside the capability branch (12,431,106-12,431,184) and `s8e` creates `credentialStorage` only when it is handed one
  (`t && (this.credentialStorage = new o8e(t,r))` 11,399,128). Credentials then live in provider memory for the process lifetime —
  **refines** the older `docs/ROADMAP.md:386` note ("KAS keeps its own backend") which does not hold for this bundle; there is
  also **no warn/error literal** for the absent-capability case (only the positive log `[KiroAgent] Client supports ACP secret storage`).
  [V] (statement about `s8e`/agent assignment verified; "no other backend" verified by absence of other `secretStorage` assignments in the Agent region.)

### 4.7 How a sign-in URL reaches the user

Provider `s8e` 11,398,8xx, `redirectToAuthorization` 11,401,035 [V]:
1. URL scheme guard `xdu`: https, or http with host `localhost|127.0.0.1|[::1]`; otherwise only a warn
   `[MCP OAuth] Refusing authorization URL with unsafe scheme` and **no URL surfaces**.
2. `lastAuthorizationUrl = url.href` is always recorded (11,401,179; a getter `FPt.lastAuthorizationUrl` 11,417,8xx exposes it).
3. Browser open happens **only when phase !== "passive"** and the client declared `openExternalUrl`:
   log `[MCP OAuth] Opening authorization URL in browser: <user-initiated re-auth | lazy-auth (mid-session 401)>`
   → `onOpenExternalUrl(url)` (11,401,426) → `_kiro/openExternalUrl {url}` (ext **method**, i.e. request/response; agent wrapper at
   12,431,363, failure warn `[KiroAgent] Failed to open external URL`). [V]
   Phases: `passive` (initial connect), `lazy` (after a successful connect), `explicit-reauth` (connect started with
   `startOAuth`), the latter two open the browser. [V]
4. Non-browser paths:
   - Pool `failed` state carries `authorizationUrl` (11,431,149) + `failedAuthorization:true` → pushed in `_kiro/mcp/status`
     as `servers[].authorizationUrl` (§5) — **the URL is retrievable with no `openExternalUrl` capability** (status only). [V]
   - `display_error` session update with `errorType:"mcp_connection_error"`:
     `MCP server "<name>" requires authorization. Check the MCP Servers panel to authenticate.` (12,420,162; when `failedAuthorization`)
     or `MCP server "<name>" failed to connect: <error>` (12,420,196). [V]
   - User-initiated auth = `_kiro/mcp/resetServer {serverName, startOAuth:true}` (§5) → phase `explicit-reauth` → browser open. [V]
5. Mid-session 401 during tool calls → lazy auth: `completeLazyAuth` (5 min `LAZY_AUTH_TIMEOUT_MS=300e3`) waits for the
   loopback callback then retries the tool call (`Lazy auth completed; retrying tool call` 11,378,854); failure →
   `Jh('MCP server "<name>" could not complete authentication')`. [V]
6. Repeated auth failure right after a completed exchange → `Auth failed again right after a completed OAuth exchange; dropping cached discovery state` + `invalidateCredentials("discovery")` 11,418,3xx. [V]

---

## 5. Status / tool surface (Q4)

### 5.1 `_kiro/mcp/status` (extNotification, per session)

Producer `emitMcpStatus(session)` 12,583,454 = `mcpPool.getStatus()` (pool method 11,430,486) mapped:
- `servers[]` entries (order = declaration `desiredOrder`, deduped by name) [V]:
  - `connecting`: `{name, authType?, status, origin, item}`
  - `connected`: `{name, status, authType?, tools:[{name,description,disabled,inputSchema}], prompts, resources, resourceTemplates, origin, item}`
  - `failed`: `{name, status, authType?, failedAuthorization, authorizationUrl?, errorMessage, errorCode?, origin, item}`
  - `disabled`: `{name, status, origin, item}`
  - `unavailable` (invalid declaration) is **reported as `failed`** with `failedAuthorization:false` and `errorMessage = Hii(name, reason)` (3,247,682).
- Top-level additions **only when `governed.registryUrl !== ""`**: `accessMode` (`"registry"|"nonRegistry"`),
  `accessModeFilteredServers`, `unresolvedRegistryServers` (from `lastRegistryResolution`), and — when the catalog is non-empty —
  `registryServers:[{name,version,description,enabled}]` where `enabled = catalog name is connected in this pool`. [V]
- Each server keeps `origin`/`item`, and `_meta.kiro.resource = {resourceType:"mcpServer", origin, item:Bko(item)}` is attached
  (12,583,5xx, `mcpServerResource` 12,583,3xx). [V]
- Pushes: on every pool `onStatusChanged` (connect success/failure, reset start/end, declaration diff, tools refresh) →
  `emitMcpStatus` for every local session + context-breakdown refresh + `availableToolsManager.refreshAll()`
  (`initializeMcp` 12,419,718; `emitMcpStatus(` call sites 12,419,898, 12,463,879 `session/new`, 12,477,381 governance apply,
  12,517,535 `session/load`, 12,520,921 catch-up/reattach burst, 12,583,454 definition). [V]
  Pool-side trigger `notifyStatusChanged` 11,434,442 is invoked from `finishUpdate` 11,429,274, `runReset` 11,433,640 /
  11,434,033, `connectServer` finally 11,439,632 and `applyRefreshedTools` 11,440,348. [V]
- `_kiro/mcp/toggle off` and `disableMcp()` push `{sessionId, servers:[]}` to all local sessions (12,424,433 `disableMcp()` 12,424,197; toggle path 12,529,7xx). [V]
- `_kiro/mcp/governance_disabled` `{sessionId, reason}` is emitted instead when org policy disables MCP (12,463,750; `governed.mcpDisabled`). [V]

### 5.2 Requests

| Method | Anchor | Semantics |
|---|---|---|
| `_kiro/mcp/resetServer` | 12,436,556 (dispatch map) / 12,589,645 (`handleResetMcpServer`) | Throws `MCP governance is disabled by your organization` when governed off. `mcpPool.resetServer(name,{startOAuth:!!startOAuth, coalesce:false, trigger:"user-retry"})`; awaits ≤ **60 s** (`Xis=Hne=6e4`, 12,382,545) then warns `[KiroAgent] mcp.resetServer.responseCeilingExpired` and returns `{success:true}` (reset continues in background). Pool `resetServer` 11,431,917→`enqueueReset` coalesces per server; `runReset` transitions to `connecting` (preserving last tools for UI), dials again, outcome ∈ `reconnected|failed|skipped|superseded` (`$du`), telemetry `kiro.io/mcp` `resetServerOutcome{outcome,trigger}`, error `MCP server "<name>" did not reconnect after reset (status: <s>)` (`BPt`) swallowed for the caller. [V] |
| `_kiro/mcp/getPrompt` | 12,436,612 (dispatch) / 12,590,562 (`handleGetMcpPrompt`) | `No connected MCP server: <name>` if not connected; `mcpClient.getPrompt({name,arguments},{timeout?})`; returns `{description?, messages}`. [V] |
| `_kiro/mcp/getResource` | 12,436,664 (dispatch) / 12,590,746 (`handleGetMcpResource`) | `mcpClient.readResource({uri})` → `{contents}`; governance check first. [V] |
| `_kiro/mcp/toggle` | 12,529,3xx (branch inside `extNotification` 12,528,376) | `enabled = params.enabled ?? true`; sets the `mcpEnabled` setting via `configuration.stateMcpSwitch(gw(enabled))` (layer item `mcp-switch` 11,871,034, `stateMcpSwitch` 11,888,919, `gw(e)` 8,524,831); if setting is `"on"` → `initializeMcp()` + re-add workspace paths for local sessions + `reconcileMcpForAllSessions()`, else `disableMcp()` (dispose config manager + pool, push empty status, refresh tools). Response: none (notification path). [V] |
| `_kiro/mcp/elicitation` (agent→client) | 11,678,799 (extMethod call, wired as `acpElicitationHandler` 11,679,262) / 11,387,799 (inbound `elicitation/create`), 11,388,300 (resolve), 11,387,515 (`notifications/elicitation/complete`) | `extMethod` with `{sessionId, toolCallId, elicitation}`; failure logs `[ACP Elicitation] extMethod _kiro/mcp/elicitation failed:` and resolves `{action:"cancel"}` to the server. URL-mode elicitation is refused unless the URL is https/loopback (`URL elicitation rejected: URL must be HTTPS or localhost`); accepted URL elicitation sends `notifications/elicitation/complete {elicitationId}` back to the MCP server. If no handler is configured: `Elicitation requested but no approval handler configured` → `{action:"decline"}`. [V] |

Session scoping [V]: `_kiro/mcp/{resetServer,getPrompt,getResource,toggle}` are classified `localOnlyUntilScoped` by the
ext-method scope classifier `_Lt` (11,999,120; case list ending 12,000,966) and are all present in the
persistence-classification switch `nDu` (11,916,185; `L9e(e) = nDu(e)!==undefined` 11,917,934; unknown methods throw
`[PersistenceClassification] Ext method "<m>" has no persistence classification` from `BUo` 11,917,973, invoked as the
first statement of `extMethod` 12,439,771). `extMethod`'s relay gate forwards only when `_Lt` ∈
{`sessionLive`,`sessionPersisted`} (`relayedTargetOf` 12,398,830 via `F4o` 11,999,055), so these MCP control methods are
**handled locally by the owning agent and are never forwarded to a relayed session**; the notification path applies the
same gate before the toggle branch (`extNotification` 12,528,376 checks `relayedTargetOf` first). The
`localOnlyUntilScoped` value itself is not branched on at dispatch: MCP requests fall through to the
`capabilityHandlers` map lookup (12,440,3xx; map 12,436,301) once relay/workflow/sessionLive-hydration routing declines them. [V]

### 5.3 Tools

- Discovery per server after connect: tools (`listTools`, paginated), prompts, resources, resourceTemplates; `MethodNotFound`
  tolerated with `Server does not support listing tools. Skipping...` 11,421,x; resource templates get parsed
  `arguments` from `{name}` placeholders. [V]
- Tool naming: `Ddu(server,tool,seen)` 11,411,076 → `mcp_<server>_<tool>` lowercased, `[^a-zA-Z0-9_]` stripped, whitespace/dash → `_`,
  **capped at 64 chars** (`b5r=64`), collision suffix `_1`,`_2`…; display name `@server/tool` (`Odu` 11,412,0xx). [V]
- `disabledTools` applies per server at pool level (`applyToolOptions(t,r)` definition 11,440,419; calls 11,427,871 / 11,437,981 / 11,440,205 set `tool.disabled`). [V]
- `tools/list_changed` → debounced 500 ms (`Rdu`) refresh (`handleToolListChanged(){` 11,420,487, invoked from the list-changed listener 11,412,898; `mcp.listTools.refreshAbsorbed` on failure). [V]
- **Advertisement and on-demand loading** [V]:
  - MCP tools become one tag per tool `@<server>/<tool>` (`tt.MCP="@mcp"` 388,573; per-tool tags with `source:"mcp"` per 2.7.1 audit). The deferred-tool *id* is the same tag with `/`→`::`: `@server/tool` → `server::tool` (11,676,8xx `me`).
  - Tool records carry `{serverName, toolName, description, inputSchema, invocationName: sanitizedUniqueName}` (11,676,8xx `W`), plus `anonymized` origin/approval attribution (`XVr` 11,669,257).
  - **Deferral + load**: tools selected for deferral are not directly callable — calling one yields
    `Tool "<id>" is deferred and cannot be called directly. Load it first: tool_load with { "tool_ids": ["<id>"] } — the load result explains how to run it.` (8,167,908) and, from the executor, `Tool "<id>" has not been loaded yet. Load it first — tool_load({ "tool_ids": ["<id>"] }) — and read the schema it returns before calling. If you requested the load and this call in the same response, the schema was not available yet: call again now that the load r…` (10,895,963).
  - System-prompt pairing text: `… tool_load { "tool_ids": ["namespace::tool", ...] } (exact ids from this list; "namespace::*" loads every tool in that namespace) before you can use them; the load result explains how to run a loaded tool:` (8,167,7xx); a telemetry/flag description at 389,307 calls this the `tool_load / tool_call pair that reaches deferred MCP tools`.
  - **Feature gate**: `toolSearch` is a hidden-by-default setting (`toolSearch: km("toolSearch","off")`, plus `toolSearchMinPct`, `toolSearchMinTokens`, `toolSearchNeverDefer` 8,529,168); `neverDefer` ids (`@server/tool` → `server::tool`, or raw id) exempt tools from deferral (11,676,8xx).
  - `tool_search` results are parsed as `{tools:[{tool_name, server_name, …}]}` (12,248,977 `e5u`) and `tool_load` results as `{toolId, entry}` maps (12,249,0xx `t5u`) purely to **account context chars under `mcpTools`/`toolIO`** (`IQr` 12,249,3xx); ids are `"<server>::<tool>"`.
  - Power-origin MCP tools are excluded from the deferred-tool pool by origin filtering (`Vhr` 3,246,976 returns false for `kind:"power"`; applied in `ie()` 11,676,9xx).

---

## 6. What a client must implement vs what KAS does itself (Q5)

| Client capability | KAS use | Failure mode if absent |
|---|---|---|
| `secretStorage` (`_kiro/secret/{get,store,delete}`) | OAuth credential persistence for remote MCP: `kiro.mcp.<hash>.{client,tokens,verifier,discovery}` (§4.6) | No backend is installed (12,431,106 gate); OAuth still completes but tokens/registration/verifier/discovery live in memory only → **re-auth after every KAS restart**; `[AcpSecretStorage] Failed to retrieve secret` warns on transient get failures. [V] |
| `openExternalUrl` (`_kiro/openExternalUrl`) | Opening the authorization URL for `explicit-reauth` and `lazy` phases, and `attachedClientChannelFor().openUrl` (12,583,x) | No browser open. The URL is still recorded in `lastAuthorizationUrl` and surfaced in `_kiro/mcp/status.servers[].authorizationUrl` + `display_error{mcp_connection_error}`, so a UI can render a link itself. [V] |
| `mcp/elicitation` (`_kiro/mcp/elicitation` ext method) | Form/URL elicitation from MCP servers; `notifications/elicitation/complete` acknowledgement | Elicitation is `decline`d with the warning `Elicitation requested but no approval handler configured`; URL-mode rejections are `decline` too; ext-method failures become `cancel`. [V] |
| Nothing (secret storage decline) | — | stdio MCP, non-OAuth remote MCP, and pre-registered `oauth.clientId` flows still work; only *token persistence* is lost. [I] |

KAS itself performs: discovery (protected resource → AS metadata), CIMD/DCR, PKCE, the loopback callback server,
token refresh, and all MCP wire traffic. The client's only obligations are the three callbacks above
(consistent with covenant §9 and 2.7.1's "host callbacks not triggered" list).

---

## 7. Exact literal table (selection; offsets verified with `grep -abo -F`)

| Literal | Offset(s) | Meaning [V unless noted] |
|---|---|---|
| `mcp.json` | 11,445,4xx | config file name |
| `legacy.mcp-json.not.found` | 11,668,005 | powers legacy scan |
| `mcp.client.transport.unsupported` | 11,883,054 | client-declared `type:"acp"` rejected |
| `mcp.client.versionNegotiation.unrecognized` | 11,883,404 | client `_meta` value invalid |
| `mcp.closeConnection.absorbed` | 11,417,392 | close error swallowed |
| `mcp.dispose.resetDrained` / `resetDrainExpired` | 11,436,368 / 11,436,279 | dispose drain (2 s ceiling `qko=2e3`) |
| `mcp.era.probe_unanswered` / `mcp.era.downgraded_after_unanswered_probe` | 11,415,997 / 11,415,190 | era fallback |
| `mcp.oauth.registration_refused_retrying_without_refresh_grant` | 11,415,690 | DCR withhold retry |
| `mcp.registry.resolve.{accessModeFiltered,disabled,unresolved}` | 12,314,649 / 12,314,828 / 12,314,998 | registry resolution diagnostics |
| `mcp.registry.{invalid.shape,entry.invalid,entries.dropped}` | 12,286,276 / 12,286,514 / 12,286,597 | catalog parse diagnostics |
| `getMcpRegistry timed out after` | 12,287,093 | 10 s registry fetch deadline |
| `[ProfileSource] organization read failed — fail-closing enterprise governance` (`failedClosed` path `mcpRegistry`) | 12,293,902 | governance fail-closed (no `mcp.registry.refused` literal exists) |
| `mcp.inherited.withheld` | 12,422,216 | file servers withheld by policy |
| `mcp.toolCall.sessionLost` | 11,381,003 | streamable session lost |
| `mcp.toolCall.sessionResetUnresolved` / `sessionReset.responseCeilingExpired` | 11,381,253 / 12,590,493 | reset budget |
| `mcp.toolCall.urlElicitationAccepted` / `NotAccepted` | 11,380,211 / 11,380,064 | URL elicitation |
| `mcp.toolResult.imageOmitted` | 11,390,083 | non-PNG/JPEG/GIF/WebP image dropped, model sees `[MCP] Image of type <mime> omitted: only PNG, JPEG, GIF, and WebP images can be sent to the model.` |
| `mcp.authSeamEscape` (`Symbol.for`) | 3,201,920 | auth-error escape marker |
| `mcp.AuthorizationServerMismatchError` (`mcpBrand`) | 3,196,534 | SEP-2352 class name |
| `mcp.RegistrationRejectedError` / `InvalidClientMetadata` | 3,195,8xx / 3,172,3xx | DCR/CIMD errors |
| `_kiro/mcp/{status,resetServer,getPrompt,getResource,toggle,elicitation,governance_disabled}` (first occurrences) | 12,424,534 / 12,436,556 / 12,436,612 / 12,436,664 / 12,529,387 / 11,678,798 / 12,463,749 | wire surface |
| `_kiro/secret/{get,store,delete}` | 12,245,321 / 12,245,480 / 12,245,634 | secretStorage contract |
| `_kiro/openExternalUrl` | 12,431,363 | browser open |
| `kiro.mcp` (`keyPrefix`) | 11,394,408 | secret key prefix |
| `ASBX_KIRO_MANDATORY_MCPS` | 393,923 | mandatory file-server names |
| `client_id_metadata_document_supported` | 2,824,926 / 3,177,237 | CIMD flag (also seen in a second inlined copy at 2.82 M — [U] which package owns that copy) |
| `mcp_connection_error` | 12,420,409 | `display_error` type |
| `timeout_too_low` | 11,439,287 | errorCode for sub-5 s timeouts |
| `[KiroAgent] MCP subsystem initialized` / `disabled` | 12,420,5xx / 12,424,x | lifecycle logs |

Absence / presence notes (all `grep -F`, re-verified): `resolveMcpTimeout` and `UND_ERR_` are **absent** from the whole
bundle. `ECONNRESET`, `EPIPE`, `socket hang up`, `CERT_`, `UNABLE_TO_VERIFY`, `connection closed` are **absent from the MCP
transport slice** (3,150,000-3,260,000) but **present elsewhere**: the KAS tool-call error classifier at 11,366,5xx
(`Ulu` regex + neighbours), an AWS-SDK smithy transient-error set `LLu` at 12,095,9xx (`ECONNREFUSED`, `ENOTFOUND`,
`ETIMEDOUT`, `UNABLE_TO_VERIFY_LEAF_SIGNATURE`, `SELF_SIGNED_CERT_IN_CHAIN`, …), an OpenSSL reason table at 4,220,6xx, and
misc. non-MCP subsystems (7,725,5xx; 10,316,3xx). An earlier revision of this doc wrongly reported these as globally
absent — do not repeat that. `redirect:"follow"` exists once (3,169,184, SSE EventSource); no `redirect:"manual"`,
`Location` or `maxRedirects` in the streamable-HTTP transport. The literal `_kiro/mcp/status` occurs exactly twice — the
`disableMcp()` empty-status push (12,424,534) and the `emitMcpStatus` producer (12,584,210).

---

## 8. Open questions / unknowns

1. `[I]` The exact *selection* rule for which MCP tools get deferred (percentage/token thresholds `toolSearchMinPct` /
   `toolSearchMinTokens`) is not anchored: the settings and never-defer list are verified (§5.3), the sizing predicate
   that consumes `minPct`/`minTokens` was not traced.
2. ~~Redirect behaviour~~ → **resolved [V/I]**: the streamable-HTTP transport has no `redirect:"manual"`/`Location`/
   `maxRedirects` literal (platform `fetch` defaults apply, `[I]` = undici `follow`); the SSE EventSource sets
   `redirect:"follow"` explicitly (§3.2/§3.3). Residual `[U]`: whether a redirected POST preserves the `mcp-session-id`
   header (fetch strips auth-ish headers on cross-origin redirects — unverified here).
3. ~~TLS/network taxonomy~~ → **resolved [V]**: structured SDK error codes (`hn` enum, §3.5) + KAS-side regex classification
   (`Ulu`/`$lu`/`$Co` 11,366,5xx); TLS reason strings live only in the AWS-SDK smithy set (12,095,9xx) and an OpenSSL
   table (4,220,6xx), not in MCP code. Residual `[U]`: no user-facing message maps TLS failure classes for MCP servers.
4. `[U]` Which upstream populates `globalEnvVars` for `${VAR}` substitution (config manager setter located, producer not traced).
5. `[U]` The second inlined copy of CIMD/`InvalidClientMetadata` code near 2.82 M / 2,968,9xx (a second `@modelcontextprotocol`
   copy bundled by another dependency?) — not attributed.
6. `[U]` `Ksh`/`Vsh` registry schemas are defined but unreferenced in this bundle (§2.6) — either dead code or reachable
   only from a code path removed by the bundler; do not cite them as the registry contract.
7. `[I]` `accessMode` semantics are read off `nen` + `getAccessMode`: registry mode excludes all non-registry servers.
   Not yet confirmed live.
8. `[I]` `syncResourcesAndTools` is the only place tools/prompts/resources are enumerated; a server whose `capabilities`
   omit a class will never contribute that class (no separate refresh path except `tools/list_changed`).
9. `[U]` Whether the *host* (kiro-cli) may also inject MCP servers through another ACP entry point besides the three
   session paths in §2.1 (see sibling `host-static.md` for the host half).
10. `[U]` `autoApprove` is parsed in every remote-server schema (`autoApprove: z.array(z.string()).optional()`
   3,251,583; client-agent `mcpServers` 12,226,277; registry server config 12,285,215; a reconnect class defaults
   `autoApprove:"none"` 11,393,850; web-portal constant 9,583,068) but **no consumer was found** in the ACP-server
   region: tool approval goes through `acpToolApproval` inside the tool wrapper `DPt` 11,367,422, and
   `applyToolOptions` (11,440,4xx) applies only `disabledTools`. Either dead in KAS or consumed host-side — unknown.

## 9. Cross-references

- Sibling: `experiments/conductor-spike/mcp-remote-2.31.1/host-static.md` (v2 Rust engine: rmcp transport, error surface,
  OAuth lifecycle, hot-reload).
- `docs/kiro-2.7.1-wire-audit.md` — client-observable surface this trace refines (§5): `_kiro/mcp/status` shape previously
  recorded as `{sessionId,servers:[{name,authType,status}]}`; now full union incl. `tools/prompts/resources/resourceTemplates`,
  `failedAuthorization`, `authorizationUrl`, `errorCode`, `registryServers`, `accessMode`.
- `docs/kiro-2.14.1-wire-audit.md` + `docs/ROADMAP.md:386` — `kiro.mcp.<hash>` key claim: **confirmed with exact hash
  recipe and key suffixes** (§4.6); the "KAS keeps its own backend" clause is **superseded** for 0.66.34.
- `docs/kiro-kas-acp-covenant.md` §9 (wire shapes) / §2 (capability gating) — matches; `secretStorage`/`openExternalUrl`
  are `KiroClientMeta` flags read from the initialize handshake.
- `crates/cyril-core/src/protocol/convert/kiro.rs:1103-1157` — cyril's `kiro.dev/mcp/*` matchers consume the *v2* surface;
  the KAS surface is `_kiro/mcp/*` (§5) and is not translated there.

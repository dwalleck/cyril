# KAS-side governance + MCP registry — KAS 0.66.34 (kiro-cli 2.31.1) — static bundle audit

Date 2026-10-16 · release **2.31.1** (KAS 0.66.34; KAS 2.31.0 == 2.31.1, per `audit-2.31.1/kas-findings.md`).
Lane: **KAS static** (read-only). Nothing was executed; every row is STATIC unless marked DOCUMENTED (repo docs) or
LIVE-prior (captured in an earlier audit's artifacts, cited).
Peer lanes: `host-static.md` (Rust v2 engine + classic), `public-docs.md` (kiro.dev). Cross-checks against both are in §8.

Bundle under audit (the existing carve — the "
carve is missing" advisory was checked and is false):

```
~/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/
  package.json                        -> "version": "0.66.34"
  dist/server/acp-server.js           -> 12,702,158 B, 17,450 lines (matches kas-findings 2.31.1 size)
```

All anchors are **byte offsets** into that `acp-server.js`, reproducible with `grep -obF '<literal>' <file>`.
Normalized-bundle equivalents (`audit-2.31.1/kas/bundle/new.raw`) are not used because that file is regenerable
only; the raw JS is the canonical artifact.
Recarve command (NOT run in this lane; the carve already exists and was re-statted above — 12,702,158 B,
17,450 lines, `package.json` version `0.66.34`, both mtime 2026-10-13 19:19 local): the repo method is
`python3 experiments/conductor-spike/carve-frames-2.29.1.py <kiro-cli-chat> --kas-dir <dir> --frames-out <json>`
— exact CLI per that script's argparse (`binary` positional, `--frames-out` required, `--kas-dir`, `--tui-out`).
`audit-2.31.1/carve-2.31.1.log` records only the output of that run
(`… /extracted/kirocli/bin/kiro-cli-chat: 216000680 bytes, 77 magic candidates`; `extracted KAS tar ->
…/kas-carves/2.31.1`; `wrote TUI bundle -> …/tui-bundles/kiro-tui-2.31.1.js`; `top-level frames: 4 nested: 62`),
not the argv, so the command line is reconstructed from the script and is **not** a verbatim log line.

---

## 0. TL;DR — the five questions

1. **Origin.** KAS builds governance from **two control-plane calls per identity**: `GetProfile` (carries
   `optInFeatures` → the primary feature values incl. `mcpRegistryUrl`) and
   `GetEffectiveEnterpriseManagementResourceWithToken` (carries the **Cedar policy document** + a small
   `configuration[]` override list). Both go through `KiroControlPlaneBearerClient` at the "management" endpoint.
   Reads happen at `initialize` (eager, maxAttempts 2), `session/new`, `session/load`, each `prompt` (identity check),
   on identity change, and on an armed **5-minute** enterprise re-read timer. `disabledReason` is emitted as exactly
   `"api_failure"` | `"admin_disabled"` | absent — **`no_endpoint` is not a KAS literal** (see §5, §7).
2. **Registry.** The URL comes from the **profile** (`optInFeatures.mcpConfiguration.mcpRegistryUrl`), overridable
   by the enterprise-management `mcp_registry_url` configuration entry (https-only, and only when the profile value
   is non-empty). KAS fetches the registry JSON **itself over HTTPS** (`node:https`, UA `Kiro`,
   `Accept: application/json`, 10 s timeout, 2 attempts, ≤2000 servers, ≤10 MiB), caches it in memory (5 min ok /
   30 s failure), and resolves per-server. Enforcement: with a registry URL set, `accessMode` is `"registry"` and
   **every server whose disposition is not `{status:"registry"}` is filtered out of the MCP pool** (not merely
   displayed-hidden); registry-declared servers not in the catalog are `unresolved` and also never reach the pool.
   When MCP governance is off, `resolveRegistry` returns `undefined` → the pool gets **zero** servers.
   `accessMode` ∈ {`"registry"`,`"nonRegistry"`}; `accessModeFilteredServers` and `unresolvedRegistryServers` are
   **name lists**.
3. **Cedar.** Org MCP governance is **both**: (a) the governance-flag path above (KAS-side filter, no policy engine
   involved) and (b) a **Cedar policy path** — the enterprise-management body's `policy` document becomes
   `scope:"organization"` rules, MCP tool calls carry Cedar capability `"mcp"` (resource `remote/<tool>` or
   `<power>/<server>/<tool>`), and `_meta.kiro.policyDenial` surfaces denials. Additionally the OS-protected
   `managed-settings.json` is loaded **by KAS** as `scope:"administration"` rules (deny/ask only). The three are
   independent: turning the registry on/off does not require any policy rule, and a policy rule can deny MCP even
   with the registry off.
4. **cyril.** cyril would receive `_kiro/governance/state` (unsolicited; global `sessionId:""` broadcast after
   initialize **and** per-session at session start), `_kiro/mcp/governance_disabled` (session start only),
   `_kiro/mcp/status` (with `accessMode`/`registryServers`/… only when a registry URL is configured),
   `_kiro/policy/changed`, `_kiro/policy/error`, and can call `_kiro/permissions/{list,explain}`,
   `_kiro/policy/check`, `_kiro/mcp/toggle`, `_kiro/mcp/{resetServer,getPrompt,getResource}`. Current cyril
   (`crates/`, grep 2026-10-16) references **none** of these literals → DROPPED-unlogged (§5.3). The
   *server-enforced* part (server suppression, tool removal, pool emptying) needs no client cooperation.
5. **Claim verdict — "Admin enforcement is not gated: KAS enforces governance itself"** (repo doc §7 row).
   **Confirmed in-bundle, with a correction of the evidence class**: the enforcement code paths are gated by
   *nothing* — no `managed_settings` / `KIRO_ENABLED_FEATURES` / AB flag exists in the KAS bundle (0 occurrences),
   and the filter/refusal call sites are unconditional. The original row's evidence was a host rollout description
   (`managed_settings`, 0%) + TUI `Bn("managed_settings")` counts — i.e. **inference from rollout notes**; it is now
   backed by literals. What *is* rollout/UI-gated is the **client surface** (`/config managed`, governance panel),
   not KAS enforcement. See §6.

---

## 1. Where governance state originates (Q1)

### 1.1 Pipeline (all STATIC, `acp-server.js`)

Three chained objects:

- `F5e` = the **governance snapshot wrapper** (`F5e=class` @12301332): `constructor(t=settings,r=profile)`,
  getters `isEnterprise`,`mcpDisabled`,`mcpReason`,`webToolsDisabled`,`usageAnalyticsEnabled`,
  `codeReferenceTrackerEnabled`,`registryUrl`,`registryCatalog`,`getAccessMode()`,
  `buildNotification(sessionId)`, `features()`, `disabledReason(features)`, `value()`, `unreadable()`.
  Instantiated as `this.governed=new F5e(this.configuration.startup,this.profile)` (@12405863).
- `L5e` = the **profile source** (`var L5e=class` @12296188): holds `reading` state machine
  (`unread`→`undecided`/`ungoverned`/`enterprise`), `identity()`, `policyState()`, `registryCatalog()` getter,
  the 5-min scheduler `b$t`, and `refreshIfChanged()`/`readEverything()`/`rereadEnterprise()`.
- `_$t` = the **read implementation** (`_$t=class` @12291207 — `authProvider`, `controlPlaneEndpointOverride`,
  `regionOverride`, `customUserAgent`, `clientSurface`, `enterprise=new y$t(…)` @12291555): `read()`, `askProfile()`,
  `governed()`, `failedClosed()`, `catalogFor()`, `loadRegistryCatalog()`, plus the CPS client `y$t`
  (`y$t=class` @12280864).

Read flow (verbatim skeleton, `async read(t,r,n)` @12291734):

```js
async read(t,r,n){let i=nme(t); if(i==="none") return /*ungoverned*/ zHo;
  let o=this.clientSurface(), [s,a]=await Promise.all([this.askProfile(t,n),this.enterprise.readDeferring(t,o)]);
  ...
  return this.governed(t,s,a,xWu(r,o))}
```

```js
async governed(t,r,n,i){ ... await n.commit(); let s=BHo(r.body), a=qQr(s,o.values);
  try{ let c=await this.catalogFor(a); return {kind:"enterprise",lastRead:"fresh",profile:HHo(r.body.arn,t.profileArn),
    name:r.body.profileName,profileValues:s,enterprise:o,values:a,catalog:c} }
  catch(c){ return this.failedClosed(t,c,"mcpRegistry","read",o) } }
```

### 1.2 Control-plane client and endpoint

| item | exact literal | anchor | proves |
|---|---|---|---|
| Management op | `GetEffectiveEnterpriseManagementResourceWithToken` | @2500694 | CPS operation name; sent as `d.send(new f$t.GetEffectiveEnterpriseManagementResourceWithTokenCommand({profileArn, clientSurface, clientVersion, manifestVersionId?}))` |
| Transport | `new f$t.KiroControlPlaneBearerClient({region:u,endpoint:l,token:{token:a.accessToken},maxAttempts:t,customUserAgent:o,requestHandler:BG(l)})` | @12271222 (`var j5u=`) | bearer token from authProvider; `maxAttempts` from caller; request handler = shared `Fat(e,{connectionTimeoutMs:15e3,requestTimeoutMs:15e3})` (`BG`), the 0.66.34 `throwOnRequestTimeout` fix |
| Endpoint | `function hx(e){return YGn("management",e)}`; regions `KGn=["us-east-1","eu-central-1","us-gov-east-1","us-gov-west-1"]` | @2596762; @2596983 | "management" plane; `endpointOverride` (config) or region default |
| Identity classification | `function nme(e){return e.provider==="Enterprise"||e.provider==="ExternalIdp"?"organization":e.authMethod==="api_key"||e.authMethod==="machine_token"?"ask-control-plane":"none"}` | @2518706 | **three classes**: org/IdP auth → governed directly; api_key/machine_token → ask control plane (eligibility gate); everything else (Builder ID etc.) → `ungoverned` |
| Profile op | `GetProfileCommand({profileArn:c||void 0})`, wrapped `BU("GetProfile",a,...)` | @12287944 (`function LHo(`) | profile carries `optInFeatures` (see 1.3) |

### 1.3 Which document carries what

- **Profile** (`GetProfile` response body) → feature toggles (`BHo` @12278761):
  `mcp=FHo(t?.mcpConfiguration?.toggle)`, `webTools=FHo(t?.webTools?.toggle)`,
  `autonomousAgents=GQr(t?.autonomousAgents?.toggle)`, `usageAnalytics=GQr(t?.byUserAnalytics?.toggle)`,
  `promptLogging=GQr(t?.promptLogging?.toggle)`,
  `codeReferenceTracker=gw(e.referenceTrackerConfiguration?.recommendationsWithReferences==="ALLOW")`,
  `mcpRegistryUrl = mcp==="on" ? t?.mcpConfiguration?.mcpRegistryUrl ?? "" : ""`.
  Toggle mapping: `FHo(e)=gw(e!=="OFF")` (absent ≠ OFF ⇒ on), `GQr(e)=gw(e==="ON")` (**absent ⇒ off**).
- **Enterprise management body** (`V5u` @12272797) = `{policy: string (required), configuration: [{key,value,source,composeBehavior}]}`.
  Recognised configuration keys (switch in `bHo` @12278761): `reference_tracker` (`Z5u`: boolean or `"ALLOW"`/`"BLOCK"`)
  and `mcp_registry_url` (`J5u`: must start `https://` and be URL-parsable). Other known-but-ignored keys are
  `B5u=["user_activity_report.enabled","default_model","fallback_model","overages"]` (@12267486) → metric
  `EnterpriseManagementIgnoredKey`. Invalid value or compose for a key → dropped + metric
  `EnterpriseManagementInvalidValueDropped` (`_Ho`).
- Merge (`qQr` @12279085): the enterprise document may override **only** `codeReferenceTracker` and
  `mcpRegistryUrl`, and the URL only when the profile already has a non-empty registry URL.
- Manifest integrity (`R5e` @12274511): body hash must equal `manifest.hash` (`sha256:<hex>`), else
  `ManifestHashMismatchError`; policy parse refusal → `EnterpriseManagementPolicyRefusedError`.
  Envelope statuses (`function $Qr(` @12272015, SDK enum `EnterpriseManagementStatus` @2506203):
  `NOT_ELIGIBLE|DISABLED|UNCHANGED|ENABLED` (+ hard throw on missing/unknown).

### 1.4 Prefetch / refresh cadence

| trigger | literal | anchor |
|---|---|---|
| initialize (eager) | `syncIdentity("initialize",{maxAttempts:2})` — warn text `Eager governance resolution on initialize failed` | @12430930 |
| session/new | `await this.syncIdentity("newSession",{maxAttempts:2})` | @12459891 |
| session/load | `await this.syncIdentity("loadSession",{maxAttempts:2})` | @12517150 |
| each prompt | `await this.syncIdentity("prompt")&&this.applyGovernanceToSession(r,"identityChange")` | @12492381 |
| identity change | `handleIdentityChange` → `profile.refreshIfChanged({maxAttempts:2,...})` → per session `applyGovernanceToSession(n,"identityChange")` | @12476431 (`async handleIdentityChange(`) |
| 5-minute timer | `CWu=5*6e4`; scheduler `b$t=class` (@12294768); `armTimedRefresh()` (@12299422); `tick()` → `readOnTimer` | @12295774 |
| forced | `async refreshGovernance(){await this.syncIdentity("refreshGovernance",{maxAttempts:2})}` — called at **websocket-mode startup** (`ACP Server: startup governa…` warn path; callback `onInitialized:u=>` @12700985 emitting the state literal @12701006) and on identity change | @12610452 |
| dedupe | `pushGovernanceState` (@12477430) compares `JSON.stringify` per session in a `WeakMap` (`governanceStateSent`), metrics `GovernanceStatePushed{Activity}` @12377766; notification literal @12477672 |

Read selection (`L5e.nextRead`/`timedRead`): identity unchanged + `enterprise` ⇒ `enterprise-management` re-read
shared within the interval (`{kind:"shared-within",withinMs:enterpriseRefreshIntervalMs}`), `lastRead==="failed"` ⇒
`everything`; `undecided` ⇒ `everything`; `ungoverned` ⇒ `nothing`. Enterprise reads run under
`M5e("GetEnterpriseManagement timed out", …)` — a **10 s** deadline helper (@12279381, `setTimeout(...,1e4)`);
errors degrade to `basis:"held-body"` (last verified cached body) or `basis:"none"`, with the log
`[ProfileSource] Enterprise Management read failed — the profile's decisions stand` (@12281714) — i.e. **enterprise
read failure is NOT fail-closed**; the profile's own values stand.

### 1.5 `disabledReason` sources (exact)

```js
get isEnterprise(){return this.profile.identity().kind==="enterprise"}
get mcpDisabled(){return this.value(Cm.mcp)==="off"}
get mcpReason(){return this.unreadable()?"api_failure":"admin_disabled"}
...
disabledReason(t){if(this.isEnterprise)return this.unreadable()?"api_failure":
   [t.mcpEnabled,t.webToolsEnabled,t.autonomousAgents,t.usageAnalytics,t.promptLogging,t.codeReferenceTracker].includes(!1)?"admin_disabled":void 0}
unreadable(){let t=this.profile.identity();return t.kind==="enterprise"&&t.lastRead==="failed"}
```

`@12301477`–`@12301874`.

- `api_failure` ⇔ `identity.kind==="enterprise" && lastRead==="failed"` — i.e. **any** fail-closed read
  (profile failure or registry-catalog failure, see §2.4).
- `admin_disabled` ⇔ enterprise and **any** of the six features is false. Note `features.contentCollection` is
  *not* in the list; it is `identity.kind==="ungoverned"` (@12302356) and is informational.
- absent ⇔ not enterprise (or all six true).
- KAS **never** emits `no_endpoint` (`grep -c` = 0). See §5.2/§7 for attribution.

---

## 2. Governance settings model (Q1/Q2 support)

### 2.1 The seven keys and their descriptors

```js
Cm={mcp:p1("mcp","on",{hidden:!1}), webTools:p1("webTools","on",{hidden:!1}), autonomousAgents:p1("autonomousAgents","on",{hidden:!1}),
    usageAnalytics:p1("usageAnalytics","off",{hidden:!1}), promptLogging:p1("promptLogging","off",{hidden:!1}),
    codeReferenceTracker:p1("codeReferenceTracker","off",{hidden:!1}), mcpRegistryUrl:R0("mcpRegistryUrl",k.string(),"",J2,{hidden:!1})}
```
`@8525612` (`Cm={mcp:p1(`). `p1(e,t,r)=$Te(e,wB,t,r)` with `wB=["off","on"]` (@8525113); scalar descriptor row =
`{shape:"scalar", defaultCompose:"replace", allowedComposeBehaviors:["replace","locked"]}` (`Bae`).
`R0` for the URL uses `J2` (same replace/locked row).

User-level switch (separate namespace!): `mwo={mcpEnabled:p1("mcpEnabled","on",{hidden:!1})}` (@8526255), written by
`_kiro/mcp/toggle` through `stateMcpSwitch(t){...this.shared.replace(qOt,lUo(this.kiroAgent,t),"fresh")}` (@11888919)
with layer key `qOt={name:"mcp-switch",listing:43}` (@11871034).

### 2.2 Compose semantics (org vs user)

```js
EWu={mcp:"ceiling",webTools:"ceiling",usageAnalytics:"ceiling",autonomousAgents:"locked",promptLogging:"locked",codeReferenceTracker:"locked",mcpRegistryUrl:"locked"}
```
`@12288982`; `$Ho` (@12289149) attaches `{value,compose:EWu[key]}`; `UHo=$Ho({mcp:"off",webTools:"off",…})` (@12289441)
is the **fail-closed all-off value set**. Ops table `dUo` (@11871154):

```js
dUo={locked:new Set, ceiling:new Set(["remove"]), floor:new Set(["add"]), append:new Set(["add"]), replace:new Set(["remove","add","assert"])}
```

So `mcp`/`webTools`/`usageAnalytics` are ceilings (lower layers may only *remove*, i.e. turn off); `autonomousAgents`,
`promptLogging`, `codeReferenceTracker`, the registry URL are locked. Values are placed into the **`organization`**
configuration layer via `I9e={name:"organization-profile",listing:8,layer:"organization"}` (@11882292); layer order
`VE=["kiro-agent-defaults","client-defaults","kiro-service","organization","machine","user-synced","user-local","kiro-agent","client-connection","workspace","agent","session"]` (@2027054).

**Consequence (STATIC):** a user can switch MCP off (or leave it off) but *cannot* switch it on against an
org `off`; they cannot change any locked key. `_kiro/mcp/toggle` re-reads the effective `mcpEnabled` after writing
its value and calls `disableMcp()` when it is not `"on"` — the org ceiling wins.

### 2.3 Deferred/failed reads and the "ungoverned" state

`identity()` maps `unread|ungoverned|undecided` as follows: `undecided` is externally normalized to
`{kind:"ungoverned"}` (`AWu={kind:"ungoverned"}` @12295899; `RWu={kind:"unread"}` @12295879), so `isEnterprise=false` and no `disabledReason`
is emitted. `ungoverned` arises when auth method class is `none` (`nme`) or the initial profile read shows no
enterprise eligibility (`TWu` mapping `{notEligible, governed, profileRefused, undecided, profileBlocked}`,
@12290248; `settledKey` @12292258). `policyState` in the notification is `E$t(reading)`:
`ungoverned→undefined`, `undecided→"undecided"`, `enterprise+basis==="none"→"unavailable"`, else `"in-force"`.

### 2.4 Fail-closed (the sharp edge)

```js
failedClosed(t,r,n,i,o){ ... S.warn("[ProfileSource] organization read failed — fail-closing enterprise governance",{endpoint:s,failedRead:n,...Og(r)}),
  ... {kind:"enterprise",lastRead:"failed",failedRead:n,profile:HHo(t.profileArn),values:UHo,catalog:new Map,enterprise:o} }
```
`@12292729`, log `fail-closing enterprise governance` @12293934.

Triggered by: profile read failure (`failedRead:"getProfile"`) **and registry-catalog failure**
(`failedRead:"mcpRegistry"`, @12293092). Result: `values=UHo` (everything `off`, registry URL `""`),
`catalog=empty`, `lastRead:"failed"` ⇒ `mcpDisabled=true`, all servers dropped, `disabledReason:"api_failure"`.
The host v2 lane reports the same failure mode from the other side (`defaulting to disabled`).

### 2.5 Client-side write paths — can a client move a governed item?

Configuration statements from the client/agent side enter through **table-gated intakes**: `_Ot`
(@11818254; `this.initializeIntake=new _Ot(b9e,i,this.connection.place,new qN)` @11888016) evaluates the
`startup` table against the client connection and the `kiroAgent` table against the kiro-agent config; `EOt`
(@11818672, source kind `Q0e`) is the sibling recorder through the same `tables`. The tables live in
`b9e=new nYr({startup:[],kiroAgent:LMu,session:[PMu,MMu,NMu,...FMu,DMu,OMu]})` (@11822944;
`session:[PMu,MMu,NMu,...FMu,DMu,OMu]` @11822982; `LMu` is the kiroAgent table @11821917). Any key outside a
table is dropped and reported: `this.report("unknown-key",t,this.messagePlace)` (@11818132) /
`"unread-key"` (@11818086), `this.drops.report(t,this.source,n,{kind:"setting",key:r})` (@11818194).

The **entire** `wire:` spec catalogue of these tables (grep over the whole bundle):

| constructor | rows |
|---|---|
| `w1(name)` | `workflows` (@11821495) `validation` `fta` `_providerPowers` `knowledge` `thinking` `tangentMode` `checkpoint` `_parallelTasks` `_requirementAnalyzer` `_quickSpec` `_subagent` `_delegate` |
| `Zce(ns,field)` | `knowledge.{includePatterns,excludePatterns,indexType,maxFiles}` (@11822141) `compaction.{excludePercent,excludeMessages}` (@11822894) |
| singleton | `sessionEviction` (`F3o()` @11821778), `knowledgeChunking` (`L3o` @11822196) |

**No `mcp*` setting exists in any table** — and none anywhere else in the 11.7–11.9 MB configuration region:
all 19 `mcpRegistryUrl` byte hits are the string-const table (@2479735), the `Cm` descriptor
(@8525917/@8525936), and governance/profile code (@12278986–12301858). (Other `wire:` hits in the bundle are
unrelated object fields; the two MCP ones are `Hba` @3250737 — `Hlt`'s parsed mcp.json declaration — and
`KVr` @11667104, the power-server writer.) The `_kiro/configuration/contribute` schema `UMu` (@11823097)
does accept `asserts?∈replace|locked|floor|ceiling|append` and `readFrom{krn,uri}`, and a manifest source
`eue={name:"contribute-source",listing:0}` (@11823040) exists — but a contribution still needs a spec row to
land, so `mcpRegistryUrl` is unreachable from the client side; and even a same-name statement could not beat
the organization layer's `compose:"locked"` (§2.2), on which lower layers perform **no ops**.

Client statements are attributed to the per-connection **`client-connection`** layer (`Pbo` scope map
`"client-connection":"connection"` @11081638; value codec `"client-connection":aIt` @11810586), and that layer
is *labelled with the contributing client's own name* (`initialize.clientInfo.title ?? name ?? "This Client"`
@11812875). The configuration-state payload lists settings whose descriptor is not `hidden:true`
(`shown(t){return this.catalog.setting(IHr(t.krn).name)?.hidden!==!0}` @11816116); `mcpRegistryUrl`'s descriptor is
`hidden:false` — read-visible to the client, not writable (see §9 Q7).

### 2.6 Tamper surface — the enforcement engine is a writable file

Everything in §2 runs as plain JS from a user-writable tree: `~/.local/share/kiro-cli/kas/2.31.1-5763e7b9…/node_modules/@kiro/agent/dist/server/acp-server.js` (sha256 `9a75422c…0ccfb`, byte-identical to this lane's carve; mode `rw-r--r--`, owner = the user). The host dedups extraction by *directory name* (version + sha256 of the embedded bundle), not by re-hashing the tree: literals `KAS bundle already extracted` (@34766360) and `KAS extraction lock timed out; extracting without dedup` (@34766388), per-version `.lock` files (`{"pid":…,"acquired_at_ms":…}`), and the sqlite table `extracted_kas_versions(version, last_used_at)` (@6887189) — so an edited file inside the current version dir survives relaunches and is dropped only by an upgrade (new dir name) [INFERENCE on the no-per-file-verify part]. A first-class redirect exists: `KIRO_KAS_SERVER_PATH` / `KIRO_KAS_NODE_PATH` (@34766077) are consulted before the embedded bundle — ``Cannot resolve the server for the v3 engine: KIRO_KAS_SERVER_PATH is not set and the embedded server is unavailable`` (@98824548) — and `docs/kiro-2.3.0-wire-audit.md` live-proved node loading an arbitrary file through it (`/etc/hostname` as the server). One-line edits change governance outcomes: `get registryUrl(){return this.value(Cm.mcpRegistryUrl)}` @12301819, `getAccessMode()` @12301932, `nen` @12314437, `ren` @12314262. The only in-bundle integrity check (EM manifest `sha256`, `R5e`) runs *inside* this same file. This is the public docs' "enforced client-side" caveat made concrete; it does not change §2.5's protocol answer.

---

## 3. Registry: learn → resolve → enforce (Q2)

### 3.1 Learning the registry (URL and catalog)

| step | literal/behaviour | anchor |
|---|---|---|
| URL source | profile `optInFeatures.mcpConfiguration.mcpRegistryUrl` (via `BHo`); enterprise override `mcp_registry_url` (https-only) | @12278761, @12278952 |
| Fetch gate | `catalogFor(t){let r=t.mcpRegistryUrl.value;return t.mcp.value!=="on"||r===""?new Map:M5e("getMcpRegistry timed out",n=>this.loadRegistryCatalog(r,n))}` | @12294165 |
| Doc parser | `async loadRegistryCatalog(t,r){let n=new Map,i=await Pe("getMcpRegistry","network",()=>OHo(t,{signal:r}));for(let{server:o}of i?.servers??[])n.set(o.name,o);return n}` | @12294289 |
| HTTP | `function OHo(e,t){if(!e.startsWith("https://"))return null; …}` — in-memory cache `N5e=new Map` (@12285548) keyed by URL with `expiresAt`; TTL select `l=a.length===0&&c>0?v$t:mWu` (@12286691) i.e. `mWu=300*1e3` ok / `v$t=30*1e3` failure+limit+all-dropped (@12285473/@12285485); server limit `MHo=2e3` (@12285523), byte cap `NHo=10*1024*1024` (@12285531); `ir.error` template `` `[getMcpRegistry] Registry exceeds server limit (${o.servers.length} > ${MHo})` `` (@12286114) and `ir.error("mcp.registry.invalid.shape")` (@12286276) — both cache `{data:null,expiresAt:r+v$t}` | @12285800 |
| Retry | `vWu` (@12285560): `i9(…,{maxAttempts:2,shouldRetry:(i,o)=>o!==void 0,delayMs:()=>SWu @12285515,onRetry:…})` → `ir.debug("[getMcpRegistry] First attempt failed, retrying",…)` (@12285692) | @12285560 |
| HTTP impl | `wWu`: `node:https` `get(e,{headers:{"User-Agent":"Kiro",Accept:"application/json"}})`, socket timeout `gWu=1e4` (@12285496; message template `` `getMcpRegistry timed out after ${t}ms` `` @12287093), abort support (`"getMcpRegistry aborted"` @12287017), byte cap error template `` `getMcpRegistry response exceeded ${NHo} bytes` `` (@12287475) — the *rendered* text is `…exceeded 10485760 bytes`; non-2xx → `` `getMcpRegistry failed: HTTP ${l.statusCode}` `` (@12287626); `JSON.parse` failure → `` `getMcpRegistry invalid JSON: ${…}` `` (@12287756); catch-all `ir.warn` `` `[getMcpRegistry] Fetch failed: ${…}` `` (@12286787) | @12286907 |
| Document schema | `Vsh=k.object({servers:k.array(WQr)})`, `WQr=k.object({server:hWu})`, `hWu=…{name (3..200, /^[a-zA-Z0-9._-]+$\|^[a-zA-Z0-9.-]+\/…$/), title?, description (1..100, required), version (no ranges), packages[≤1] xor remotes[≤1]}` | @12284513 |
| Package entry | `lWu={registryType:["npm","pypi","oci"], registryBaseUrl?, identifier, transport (stdio\|streamable-http\|sse), runtimeArguments?, packageArguments?, environmentVariables?}` | @12283911 |
| Runner mapping | `een`: npm→`npx -y …`, pypi→`uvx`, oci→`docker run` (+`NPM_CONFIG_REGISTRY`/`--default-index`) | @12312218 |
| Unknown entry | per-entry `safeParse` failure → skipped, `ir.trace("mcp.registry.entry.invalid")`, summary `ir.warn("mcp.registry.entries.dropped")` | @12285800 region |

The fetched catalog is keyed by server **name** (`Map<name, server>`); `registryCatalog` returns it only when
the reading is enterprise (`get registryCatalog(){return O5e(this.reading)?this.reading.catalog:new Map}` @12297650,
wrapped by F5e as `get registryCatalog(){return this.profile.registryCatalog}` @12301874).

### 3.2 User declarations of registry servers

`mcp.json` entries: loose union `zlt=k.object({...Uba,...Bba,...qba,...zba})` (@3251960) where
`Uba={env?,timeout?,waitForReady?,disabledTools?,autoApprove?,versionNegotiation?}`, `Bba={command?,args?,cwd?}`,
`qba={url?,headers?}`, `zba={disabled?,type?}` — `type` is a free string; the strict registry-entry schema
`Ksh=k.object({type:k.literal("registry"),timeout?,headers?,env?,disabled:false,autoApprove?,disabledTools?})`
(@12285024) is defined **once and referenced nowhere else in the bundle** (dead in 0.66.34; the loose `zlt` is what
parses).

Declaration → disposition (parse `Hlt` @3250669 → wrap `Wlt` @3250880 → classify `Xhr` @3250015; disabled wrapper `qlt` @3250585):

```js
if(t.type==="registry"){ let o={status:"registry",declaration:{env:t.env,headers:t.headers,timeout:t.timeout,
   disabledTools:t.disabledTools,versionNegotiation:t.versionNegotiation}};
   return {…,disposition: t.disabled===!0 ? qlt(o) : o} }      // qlt: {…,disabled:true}
if(t.disabled===!0) return {…,disposition:qlt({status:"disabled"})};   // qlt({status:"disabled"}) === {status:"disabled"}
let i=Yhr(t); return i.ok?{…,disposition:{status:"enabled",config:i.config}}:{…,disposition:{status:"invalid",reason:i.reason}}
```

`Yhr` (@3249121) builds stdio (command) or http (url) configs; `${VAR}` substitution happens earlier in `Hba`/`zne`
(from entry `env`, then process env; values whose env value starts with `()` are left literal).

### 3.3 Resolution and enforcement (the core)

```js
function nen(e,t){let r=t?.accessMode??"nonRegistry",n=t?.catalog,i=t?.envProvider,o=[],s=[],a=[];
  for(let c of e){ if(c.disposition.status==="registry"){
      if(r==="nonRegistry"){a.push(c.name); ir.debug("mcp.registry.resolve.accessModeFiltered",{serverName:c.name,extra:{accessMode:r,kind:"registry"}});continue}
      if(c.disposition.disabled){o.push({...c,disposition:{status:"disabled"}}); ir.debug("mcp.registry.resolve.disabled",{serverName:c.name});continue}
      let u=ren(n?.get(c.name),c.disposition.declaration,i?{envProvider:i}:void 0);
      if(!u){s.push(c.name); ir.debug("mcp.registry.resolve.unresolved",{serverName:c.name,extra:{catalogSize:n?.size??0,inCatalog:n?.has(c.name)??!1}});continue}
      o.push({...c,disposition:{status:"enabled",config:u}}); continue }
    r==="registry" ? (a.push(c.name), ir.debug("mcp.registry.resolve.accessModeFiltered",{serverName:c.name,extra:{accessMode:r,kind:"nonRegistry"}}))
                   : o.push({...c,disposition:c.disposition}) }
  return {resolvedServers:o,unresolvedServers:s,accessModeFilteredServers:a}}
```
`@12314437` (`nen`), merge helper `ren` @12314262, log ids `mcp.registry.resolve.*` @12314636.

```js
resolveRegistry(t){if(!this.governed.mcpDisabled)return nen(t,{accessMode:this.governed.getAccessMode(),catalog:this.governed.registryCatalog})}   // @12577602
resolveMcpServers(t){let r=this.resolveRegistry(t);return this.lastRegistryResolution=r,r?.resolvedServers??[]}                                      // @12577491
reconcileMcpForAllSessions(){let t=[...this.localSessions()],r=this.restateRuntimeServers(t); if(this.disposeOnce||!this.mcpPool)return;
  let n=this.resolveMcpServers(r); t.length===0&&(this.lastRegistryResolution=void 0);
  S.debug("mcp.reconcile.allSessions",{sessionCount:t.length,serverCount:n.length}); this.mcpPool.updateServers(n).catch(…)}                       // @12422492
```

`getAccessMode(){return !this.mcpDisabled&&this.registryUrl!==""?"registry":"nonRegistry"}` (@12301932).

So the enforcement semantics are:

- **Registry URL set** (`accessMode:"registry"`): non-registry dispositions (`enabled`/`invalid`/`disabled` from
  stdio/http/agent/bundled sources) are **removed from the resolved set**; only `type:"registry"` entries that
  resolve against the catalog survive. Missing catalog entry ⇒ `unresolvedServers`, not started.
- **No URL** (`accessMode:"nonRegistry"`): registry-declared entries are filtered out; ordinary servers pass through
  with their own disposition.
- **`mcpDisabled`**: `resolveRegistry` returns `undefined` ⇒ `resolvedServers=[]` ⇒ `mcpPool.updateServers([])` —
  the pool is emptied (all sources), regardless of accessMode.
- The pool input (`restateRuntimeServers`) is the union of file/config servers (`mcpConfigManager.getServers()`) and
  runtime-declared servers (agent profiles, powers, bundled, client); `nen` applies to the union. [INFERENCE, med]:
  bundled/agent servers carry plain `enabled` dispositions, so they are subject to the same registry-mode filter;
  the code path (single filter site) is STATIC, the per-source disposition values are inferred from the shared
  `Xhr`/`Wlt` parse path.

### 3.4 In-session enforcement sites (beyond the pool)

| what | literal | anchor |
|---|---|---|
| MCP tools withheld | `ie=()=>{let tn=u();if(!tn||l?.().mcpDisabled)return[]; …}` (`l` = `getGovernanceState`) | @11677187 |
| MCP manager withheld | `mcpManager:()=>this.governed.mcpDisabled?void 0:this.mcpPool` | @12568174 |
| Governance accessor | `getGovernanceState:()=>({mcpDisabled:this.governed.mcpDisabled,webToolsDisabled:this.governed.webToolsDisabled})` | @12568235 |
| Notification | `this.governed.mcpDisabled&&ne.extNotification("_kiro/mcp/governance_disabled",{sessionId:i,reason:this.governed.mcpReason})` | @12463750 |
| Refusals | `throw new Error("MCP governance is disabled by your organization")` ×3 — `handleResetMcpServer`, `fetchMcpPrompt`, `handleGetMcpResource` | @12589715/12590098/12590822 |
| Status accessor | `governedMcpStatus(){return this.governed.mcpDisabled?void 0:this.mcpPool?.getStatus()}` (feeds slash-command `/mcp`) | @12589936 |
| Pool teardown | `disableMcp(){…}` disposes pool + broadcasts `_kiro/mcp/status {sessionId, servers:[]}` per session | @12424197 |
| Startup gate | `this.configuration.startup.value($i.mcpEnabled)==="on"&&this.initializeMcp()` | @12419215 |

---

## 4. Cedar / policy relationship (Q3)

### 4.1 Two independent mechanisms (both exist; neither subsumes the other)

| aspect | governance flags (§2–3) | Cedar policy (§4.2–4.4) |
|---|---|---|
| Carrier | profile `optInFeatures` + enterprise `configuration[]` | enterprise body `policy` document + `managed-settings.json` |
| Effect | MCP on/off, registry allow-list, webTools on/off | per-tool-call `allow`/`ask`/`deny`, incl. capability `mcp` |
| Enforcement point | `resolveRegistry`/pool, tool list, method refusals | policy engine (`H$t`/`vB`), approval flow |
| Client visibility | `_kiro/governance/state`, `_kiro/mcp/governance_disabled`, `_kiro/mcp/status` | `_meta.kiro.policyDenial`, `_kiro/policy/{changed,error}`, `_kiro/permissions/*`, `_kiro/policy/check` |

### 4.2 Org policy document → rules

`R5e` (@12274511): hash-verify body, then `wHo(body.policy)` (@12273922):
`return {kind:"read",rules:k$o(e,"organization",K5u).rules}` — empty/whitespace ⇒ `[]`; parse error ⇒
`kind:"refused"` → the whole read is refused (`R5e` turns it into `{kind:"policy-refused"}` @12274647; the error
class is `I5e=class extends Error` @12274940, thrown from the enterprise read @12278013; the on-disk cache maps it
back at @12304557). Rules are pushed into the
policy session as `e.organization.rules` (`function jQr(` @12290783); `xWu` supplies `{values:{},rules:[],surface,
eligibility:"unknown",basis:"none"}` when no enterprise reading is held.

### 4.3 Policy scopes and `administration`

```js
Uqr={kiro:["deny","ask"], organization:["deny","ask","allow"], administration:["deny","ask"],
     user:["deny","ask","allow"], workspace:["deny","ask","allow"], agent:["deny","ask","allow"], session:["deny","ask","allow"]}
```
`@10822376`. Note `organization` ∈ allowed scopes with `allow` — the covenant doc's `PolicyScope` list
(`kiro|administration|user|workspace|agent|session`, §9 line 202) is **missing `organization`** and is stale on that
point. `administration` rules are loaded by KAS itself from the OS-protected file:

```js
ZIu={darwin:"/Library/Application Support/Kiro/managed-settings.json", linux:"/etc/kiro/managed-settings.json",
     win32:"C:\\ProgramData\\Kiro\\managed-settings.json"}
```
`@11698277`; loader `D$o(o,"administration")` @11699636 inside the policy-session `load()`; fingerprint helper
`M$o` (`sha256:…` / `absent` / `unreadable`) drives rebuild-on-change (`rebuildIfChanged`). Built-in defaults add
`QIu=["shell","mcp","power"]` ⇒ `{scope:"kiro",source:"kiro-scope:untrusted-execution",rule:{capability,effect:"ask"}}`
for untrusted workspaces (@11698871) — i.e. **MCP is `ask` by default in untrusted workspaces**.

### 4.4 MCP as a Cedar capability

- capability enum incl. `mcp`: `C5t={all:[…Jln,"mcp"], …}`, `Rms=new Set(["all","builtin","filesystem","fs_read","fs_write","shell","web_fetch","web_search","mcp","subagent","skill","power","context","diagnostics","sandbox_network"])`, validator `Nle(e){return Rms.has(e)}` (@391984 / @389867); tag map `kms=[{tag:tt.MCP,capability:"mcp"},…]` (@391807).
- Remote/MCP tool scope: `policyScope(){let t={capability:"mcp",resource:`remote/${this.toolSpec.name}`}, …}` (@10858074); power-hosted MCP tools use `restrictions:[{capability:"mcp",resource:`${power}/${server}/${tool}`}]`.
- Client methods: `_kiro/permissions/explain` (`handlePermissionsExplain` @12613941), `_kiro/permissions/list`
  (`handlePermissionsList` @12614765: returns `{rules:[{capability,match,exclude,effect,scope,source}]}` filtered by
  optional `scope`), `_kiro/policy/check` (`handlePolicyCheck` @12613524: validates `capability` via `Nle`, runs
  `r.acpToolApproval({…})`, maps through `gFr` → `{outcome:"allow"}` | `{outcome:"deny",reason}`).
- Denial envelope: `hXs` `_meta.kiro` schema contains
  `policyDenial:{capability,resource,triggeringResource?,effect:"deny",matchedRule:{capability,match?,exclude?,effect:…},scope,source}`
  (@2034434, inside `hXs` @2033441 / `kfe=k.object({kiro:hXs.optional()})` @2035302); attached by
  `withPolicyDenial(...)` when a tool call is rejected (`concludeToolRejection`, @7971967).
- Notifications: `_kiro/policy/changed` `{sessionId,status:"failed",errors:[{scope,source,message,fatal}]}`
  (@11675863) and `_kiro/policy/error` `{sessionId,errors:[{scope:"session",source:"consent",…}]}` (@11303735).

**Answer to Q3:** org MCP governance is *primarily* a client-side (KAS-side) filter driven by governance flags —
that filter does not consult the policy engine. In parallel, the org policy document and the admin
`managed-settings.json` feed the Cedar engine, where MCP is a first-class capability; a rule can `deny`/`ask` MCP
even when the registry is absent, and the registry filter operates regardless of policy. There is **no evidence** of
registry membership being expressed as policy rules (no literal connects `registryCatalog`/`nen` to the engine).

---

## 5. Wire surface for clients (Q4)

### 5.1 Notifications KAS emits

| method | payload (STATIC reconstruction) | when | anchor |
|---|---|---|---|
| `_kiro/governance/state` | `{sessionId, isEnterprise, features:{mcpEnabled,webToolsEnabled,usageAnalytics,contentCollection,promptLogging,codeReferenceTracker,autonomousAgents}, disabledReason?, policyState?}` (`disabledReason` omitted when undefined; `policyState` = `undefined\|"undecided"\|"unavailable"\|"in-force"`) | (a) after every profile/enterprise read resolves: broadcast to **all live clients** with `sessionId:""` (`onGovernanceResolved` → `broadcastNotificationToAllClients`); (b) per session at session start; (c) per session on timed refresh / identity change (deduped by JSON equality) | @12302020 (`buildNotification`), @12699837 (broadcast), @12477672 (per-session) |
| `_kiro/mcp/governance_disabled` | `{sessionId, reason}` where `reason = mcpReason` = `"api_failure"`\|`"admin_disabled"` | session start **only**, iff `mcpDisabled` | @12463750 |
| `_kiro/mcp/status` | `{sessionId, servers:MCPServerStatus[], _meta.kiro.resource?}` + **iff registryUrl≠""**: `accessMode:"registry"\|"nonRegistry"`, `accessModeFilteredServers:[names]`, `unresolvedRegistryServers:[names]` (both iff a resolution exists), `registryServers:[{name,version,description,enabled}]` (iff catalog non-empty; `enabled` = present in the pool) | session start, session load/new, config change, governance apply, pool status change, `disableMcp` (empty list) | `async emitMcpStatus(t){` @12583448 |
| `_kiro/policy/changed` | `{sessionId,status:"failed",errors:[{scope,source,message,fatal}]}` | policy rebuild failures (e.g. sandbox network rules) | @11675863 |
| `_kiro/policy/error` | `{sessionId,errors:[{scope:"session",source:"consent",message,fatal:false}]}` | consent-persistence warnings during approvals | @11303735 |
| `_kiro/policy/ignore_files_changed`, `_kiro/terminal/settings_changed` | client→agent | n/a | @11916786 |

`servers[]` item shape (pool `getStatus()` @11430486): `{name, status:"connecting"|"connected"|"failed"|"disabled"|"unavailable"→rendered failed, authType?, tools:[{name,description,disabled,inputSchema}], prompts, resources, resourceTemplates, origin, item:{krn,layer,standing,readFrom?}, errorMessage?, authorizationUrl?, errorCode?, _meta:{kiro:{resource:{resourceType:"mcpServer",source,krn,layer,standing,readFrom?}}}}` —
resource provenance (`ELe` @3245349, `Ya` @7904238, `mcpServerResource` @12583339).
`extNotification` is a passthrough with **no client capability gate** (`extNotification(t,r){return this.notify(t,r)}` @310965; no `notificationMethods`/`extNotifications` key exists in the bundle).

### 5.2 Methods relevant to governance

| method | semantics | availability class | anchor |
|---|---|---|---|
| `_kiro/mcp/toggle` | dispatch `case"_kiro/mcp/toggle"` (@11916763/12000857) → impl (@12529388): `let i=r.enabled??!0; …this.configuration.stateMcpSwitch(gw(i)) @12529502; if(this.configuration.startup.value($i.mcpEnabled)==="on"){this.initializeMcp();…;this.reconcileMcpForAllSessions()}else this.disableMcp() @12529756` | `localOnlyUntilScoped` | @11916763 |
| `_kiro/mcp/resetServer` | dispatch `case"_kiro/mcp/resetServer"` (@11916606/12000775) → map entry (@12436556) → `async handleResetMcpServer(t){if(this.governed.mcpDisabled)throw new Error("MCP governance is disabled by your organization"); …}` (@12589639) | `localOnlyUntilScoped` | @11916606 |
| `_kiro/mcp/getPrompt` / `_kiro/mcp/getResource` | same shape: dispatch (@11916634/@11916660, second list @12000803/@12000829) → map (@12436612/@12436664) → `handleGetMcpPrompt` (@12590568) / `handleGetMcpResource` (@12590752), both `if(this.governed.mcpDisabled)throw new Error("MCP governance is disabled by your organization")` | `localOnlyUntilScoped` | @11916634/11916660 |
| `_kiro/permissions/list` | `{scope?}` → `{rules:[…]}` | "metadata" in one classifier, "sessionLive" in another (`nDu` @11916185, `_Lt` @11999120; `BUo` @11917973 enforces classification) | @12614765 |
| `_kiro/permissions/explain` | `{capability?|toolId?, resource}` → explanation | same | @12613941 |
| `_kiro/policy/check` | `{capability, paths?, command?, toolId?}` → `{outcome, reason?}` | same; resolves `ask` by firing `session/request_permission` (covenant §9, LIVE-verified 2.7.1) | @12613524 |
| `_kiro/config/template`, `_kiro/configuration/{getState,contribute}` | pre-session config template / configuration-state read (layers list incl. `organization`); `getState` requires an initialized client | `transient`/`localOnly` | @11917820/11917848 (handler map; dispatch lists @12000154/12000182) |

### 5.3 What cyril does today

`grep` over `crates/` (2026-10-16) finds **no** reference to `_kiro/governance/state`,
`_kiro/mcp/governance_disabled`, `_kiro/mcp/status`, `policyDenial`, `_kiro/permissions/*`, `_kiro/policy/check`,
`accessMode`, or `registryServers`; `isEnterprise` appears only for the *account/usage* payload
(`protocol/convert/kas.rs:138`) and workspace-trust wording (`types/kas_hooks.rs:23`). So today cyril would DROP all
five governance notifications unlogged. `docs/kiro-kas-acp-covenant.md` already documents the shapes (§9 lines
67–75, 198, 202, 222–223) — the covenant, not the code, is the current reference; a §9 correction is needed for
`organization` policy scope (`Uqr` includes it; the covenant omits it).

---

## 6. Verdict on the repo claim (Q5)

Claim (`docs/kiro-2.31.1-wire-audit.md:197`): “**Enforcement is not gated: KAS enforces governance itself**,”
evidence column: “host rollout row text; TUI `Bn("managed_settings")` 0→5”.

| proposition | in-bundle evidence | verdict |
|---|---|---|
| KAS enforces governance without a host/feature gate | gate literals absent: `managed_settings` 0, `KIRO_ENABLED_FEATURES` 0, `KIRO_TEST_GOVERNANCE` 0, `AB_*ENTERPRISE*` 0 (all `grep -c` on the bundle); enforcement call sites unconditional (`resolveRegistry` @12577602, `mcpManager` @12568174, tool filter @11677187, refusals @12589715…) | **confirmed** |
| KAS enforces it *itself* (client-side filter, not only server-side refusal) | pool emptying (`updateServers`), MCP tool removal, notification, method refusals, fail-closed values | **confirmed** |
| The claim’s stated evidence was sufficient | it was a rollout description + TUI counts ⇒ inference; now upgraded to literals | **upgraded, was inference** |
| The *feature* is ungated everywhere | TUI carries `Bn("managed_settings")` gates and the host rollout is 0%/internal; `_kiro/configuration/*` and `/config managed` belong to that surface | **partially false**: the *client UI surface* is rollout-gated; KAS-side enforcement is not |
| Public docs agree enforcement is client-side | `public-docs.md`: “Both the toggle and the registry settings are enforced on the client side … can be circumvented by users” | **consistent** (KAS is that client) |

---

## 7. Host-side attribution of the task-listed “KAS session_manager” fields

The fields `mcp_enabled`, `web_tools_enabled`, `mcp_api_failure`, `is_enterprise`, `is_api_key`, `has_registry`
are **not KAS literals** (`acp-server.js`: `mcp_enabled` 0, `has_registry` 0; the names appear only in camelCase as
KAS JS properties). They are **host v2 (Rust) fields**: the strings dump
`audit-2.31.1/host/raw/strings/2.31.1/kiro-cli-chat.txt` contains
`…session_manager.rs:422` immediately followed by the glued field list
`mcp_enabledweb_tools_enabledmcp_api_failureis_enterpriseis_api_keyhas_registry`, plus
`crates/chat-cli-v2/src/agent/acp/session_manager.rs` and `ConversationState with 21 elements` listing
`mcp_enabled`, `mcp_registry_cache`, `mcp_server_versions`, `mcp_registry_error_type`,
`mcp_disabled_due_to_api_failure`, `tangent_state`, `user_turn_metadata`.
Log strings in the same run: `Non-enterprise user detected, enabling MCP without registry`;
`Retrieved governance config from API: mcp_enabled=, registry_url=, web_tools_enabled=`;
`Failed to get governance config from API: , defaulting to disabled`; `Initial MCP state: enabled=`;
`BEFORE filtering - Agent '` / `AFTER filtering - Agent '`; `Fetched override registry from ` /
`Failed to fetch override registry`; test hook `KIRO_TEST_GOVERNANCE_DISABLED`.
Attribution: `host-static.md` §1 (v2 `api_client/governance.rs`) and §2. **Correction for the record:** the task
brief’s “KAS session_manager fields” are the v2 host’s, and the host has its *own* registry fetch/filter code path
(“Registry fetch failed, retrying in 1s”, “BEFORE/AFTER filtering”) independent of KAS.

`no_endpoint`: 0 occurrences in KAS, 0 in the v2 host dump, 0 in the TUI bundle as a *producer* — the TUI only
*consumes* it (`drn=new Set(["admin_disabled","api_failure","no_endpoint"])`, copy “No organization control plane is
configured; disabled settings are conservative defaults.”). [INFERENCE, med] it is a v2-host enum variant serialized
via serde snake_case (invisible to `strings`), i.e. the host’s “no control-plane endpoint configured” reason; it is
**not** producible by KAS 0.66.34.

---

## 8. Cross-checks against the sibling lanes

- **24 h registry refresh (public docs) vs KAS**: KAS has **no 24 h timer**; catalog refresh is governance-read
  driven (startup/identity/5-min enterprise re-read) with a 5-minute in-memory cache (`mWu=300*1e3`) and 30 s
  negative cache. The “every 24 hours” claim is a client/doc-level cadence that 0.66.34’s KAS does not implement;
  the host v2 keeps its own `mcp_registry_cache` (host-static §2). Worth flagging in the doc.
- **“Registry authoritative: terminate stale servers, version-pin, block re-add” (public docs)**: KAS 0.66.34
  implements the *filter* half (never start = never connect; name lookup gates `type:"registry"` entries; catalog
  `version` is stored in `registryServers[]` for display). No literal in KAS shows a *terminate/relaunch* path for
  servers removed from the registry (a running server is dropped simply because the resolved set no longer includes
  it, then `updateServers` reconciles). [STATIC + inference]
- **“Hidden servers” UX (public docs)**: KAS reports hidden names in `_kiro/mcp/status.accessModeFilteredServers`
  and unresolved names in `unresolvedRegistryServers`; the TUI copy (“hidden: your organization allows only servers
  from its MCP registry.”) is TUI-side (`tui-findings.md` T19) — KAS itself sends only names, not per-server items,
  for hidden/unresolved servers.
- **Covenant `PolicyScope`** (line 202) omits `organization`; KAS `Uqr` includes it (deny/ask/allow). Covenant §9
  line 222 documents `_kiro/governance/state` without `policyState`; KAS 0.66.34 emits `policyState`. Both are doc
  updates for the covenant package.

---

## 9. Open questions / live probes

1. **`policyState` values on the wire**: STATIC shows `"undecided"|"unavailable"|"in-force"`; verify what a
   non-enterprise session actually gets (omitted vs `undefined` key) — live: `initialize`+`session/new` with an
   api-key token, capture the `_kiro/governance/state` frame.
2. **`sessionId:""` global frame**: confirm the client receives the broadcast *before* `session/new` and that a
   second frame arrives per session; record ordering vs `_kiro/mcp/status`.
3. **Mid-session fail-close**: the `_kiro/mcp/governance_disabled` notification is session-start-only. Verify that a
   mid-session governance flip (fail-closed) is visible **only** via `_kiro/governance/state` + empty
   `_kiro/mcp/status`, by forcing a registry fetch failure (e.g. an https registry URL that 404s / times out) and
   watching frames across the 5-minute timer.
4. **Registry cache TTLs on the wire**: with a valid registry, confirm that toggling the enterprise-management
   value (URL unchanged) does not refetch within 5 min (catalog TTL) even though `applyGovernanceToSession`
   re-runs; assert via server-side request counting (mitm) rather than frame differences.
5. **`type:"registry"` refusal UX**: check the exact client-visible outcome when a `type:"registry"` entry is
   absent from the catalog (expect: no `_kiro/mcp/status.servers` entry; name in `unresolvedRegistryServers`;
   no `session/request_permission`, no connection attempt).
6. **`accessModeFilteredServers` for bundled servers**: with a registry URL set, confirm bundled/agent servers are
   absent from `servers[]` and listed in `accessModeFilteredServers` (settles the §3.3 inference).
7. **`_kiro/configuration/getState` organization layer**: does it expose governance values (`value`+`compose`) so a
   client could render “managed by your organization” without parsing notifications? Read the response for an
   enterprise profile. STATIC partial (§2.5): the payload lists settings whose descriptor is not `hidden:true`
   (`shown` @11816116) and names the layers (`organization` → "Your Organization", `client-connection` → the
   client's own `initialize.clientInfo` name); `mcpRegistryUrl`'s descriptor is `hidden:false` — so the listing
   shape exists. What an enterprise profile actually returns (value+compose vs redaction) remains this probe.
   Cross-ref: the 2.28.0 live pull (`rivets cyril-iq2c`) already shows the shape carries
   `value`+`compose`+`contributions[].used` and that the pull is ungated; the only unknown left is the
   *enterprise* row for a governed `mcpRegistryUrl` (present with `compose:"locked"` + an organization
   contribution) — probe it there rather than re-deriving here.
8. **Host/v2 divergence**: whether the v2 engine’s `no_endpoint` reason can travel as
   `_kiro.dev/mcp/governance_disabled.apiFailure` (host lane) and how it should be rendered by a KAS-only client.

---

## 10. Evidence index (literal → byte offset)

All offsets are byte offsets in
`~/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/dist/server/acp-server.js`
(12,702,158 B, KAS 0.66.34). Each entry below was asserted on 2026-10-16 by byte-comparing
`data[offset:offset+len(literal)]` against that file (scripted, 0 mismatches before writing this
block); where two offsets are shown, both were asserted. `…`/`${..}` in a label means the bundle
literal is a template — the label quotes it up to the interpolation.

```text
extNotification(t,r){return this.notify ................ 310965
function Nle( (capability validator) ................... 389867
kms=[{tag:tt.MCP,capability:"mcp"} ..................... 391807
Rms=new Set(["all",...]) ............................... 391984
VE=[ (config layer order) .............................. 2027054
hXs=k.object({visibility ............................... 2033441
policyDenial:k.object .................................. 2034434
kfe=k.object({kiro:hXs.optional()}) .................... 2035302
function nme( (auth class) ............................. 2518706
function hx( (management endpoint) ..................... 2596762
KGn=[ (regions) ........................................ 2596983
function ELe( (origin mapper) .......................... 3245349
function Xhr( (disposition factory) .................... 3250015
function qlt( (disabled wrapper) ....................... 3250585
qlt(e){return e.status==="registry" .................... 3250594
function Hlt( (parse mcp.json declaration) ............. 3250669
function Wlt( (disposition wrap) ....................... 3250880
zlt=k.object (loose mcp.json union) .................... 3251960
withPolicyDenial( ...................................... 7971967
function p1( (scalar descriptor factory) ............... 8525113
Cm={mcp:p1( (setting keys) ............................. 8525612
mcpEnabled:p1 (user switch) ............................ 8526255
Uqr={ (policy scopes) .................................. 10822376
policyScope(){let t={capability:"mcp" .................. 10858074
_kiro/policy/error ..................................... 11303735
getStatus(){let t=[] (pool) ............................ 11430486
_kiro/policy/changed ................................... 11675863
ie=()=>{ ...mcpDisabled} (tool filter) ................. 11677187
ZIu={ (managed-settings paths) ......................... 11698277
QIu=["shell","mcp","power"] ............................ 11698871
D$o(o,"administration") ................................ 11699636
qOt={name:"mcp-switch" ................................. 11871034
dUo={ (compose ops) .................................... 11871154
I9e={name:"organization-profile" ....................... 11882292
stateMcpSwitch(t){ ..................................... 11888919
_kiro/mcp/resetServer (dispatch) ....................... 11916606/12000775
_kiro/mcp/getPrompt (dispatch) ......................... 11916634/12000803
_kiro/mcp/getResource (dispatch) ....................... 11916660/12000829
_kiro/mcp/toggle (handler branch) ...................... 11916763
_kiro/policy/ignore_files_changed ...................... 11916786
_kiro/config/template (handler map) .................... 11917820
_kiro/configuration/getState (handler map) ............. 11917848
_kiro/config/template (dispatch) ....................... 12000154
_kiro/configuration/getState (dispatch) ................ 12000182
_kiro/mcp/toggle (2nd dispatch / impl) ................. 12000857/12529388
async handleIdentityChange (1st) ....................... 12009021
d$t=class (model client) ............................... 12259662
B5u=[ (ignored config keys) ............................ 12267486
var j5u= (CPS transport) ............................... 12271222
function $Qr( (CPS envelope) ........................... 12272015
function V5u( (EM body schema) ......................... 12272797
function wHo( (policy parse) ........................... 12273922
function R5e( (manifest verify) ........................ 12274511
policy-refused (R5e result) ............................ 12274647
I5e=class (policy-refused error) ....................... 12274940
g$t=class (cache reader) ............................... 12275093
readShared(t,r){ ....................................... 12275798
function bHo( (config entries) ......................... 12278761
"reference_tracker" .................................... 12278833
"mcp_registry_url" ..................................... 12278952
function qQr( (merge) .................................. 12279085
function M5e( (10 s deadline) .......................... 12279381
zQr={ (ask-control-plane default) ...................... 12279587
function tWu( .......................................... 12280027
function THo( .......................................... 12280102
function xHo( .......................................... 12280158
y$t=class (CPS client) ................................. 12280864
readDeferring(t,r,n=zQr) ............................... 12281014
lWu=k.object (package entry) ........................... 12283911
hWu=k.object (server schema) ........................... 12284513
Vsh=k.object (registry document) ....................... 12284987
Ksh= (strict registry schema; unused) .................. 12285024
mWu=300*1e3 (ok TTL) ................................... 12285473
v$t=30*1e3 (failure TTL) ............................... 12285485
gWu=1e4 (socket timeout) ............................... 12285496
yWu="Kiro" (User-Agent) ................................ 12285504
SWu=1e3 (retry delay) .................................. 12285515
MHo=2e3 (server limit) ................................. 12285523
NHo=10*1024*1024 (byte cap) ............................ 12285531
N5e=new Map (catalog cache) ............................ 12285548
function vWu( (retry wrapper) .......................... 12285560
[getMcpRegistry] First attempt failed, retrying ........ 12285692
function OHo( (registry fetch) ......................... 12285800
Registry exceeds server limit (${..} > ${MHo}) ......... 12286114
mcp.registry.invalid.shape ............................. 12286276
l=a.length===0&&c>0?v$t:mWu (TTL select) ............... 12286691
[getMcpRegistry] Fetch failed: ${..} ................... 12286787
function wWu( (node:https get) ......................... 12286907
getMcpRegistry aborted ................................. 12287017
getMcpRegistry timed out after ${t}ms .................. 12287093
getMcpRegistry response exceeded ${NHo} bytes .......... 12287475
getMcpRegistry failed: HTTP ${status} .................. 12287626
getMcpRegistry invalid JSON: ${..} ..................... 12287756
function LHo( (GetProfile op) .......................... 12287944
EWu={ (compose table) .................................. 12288982
function $Ho( (org value build) ........................ 12289149
UHo=$Ho( (all-off fail-closed set) ..................... 12289441
function qHo( (layer statements) ....................... 12290092
zHo={kind:"ungoverned"} ................................ 12290201
function TWu( (eligibility) ............................ 12290248
function E$t( (policyState) ............................ 12290616
function jQr( (org rules push) ......................... 12290783
function xWu( (default enterprise shape) ............... 12290900
var _$t=class (read impl) .............................. 12291207
new y$t( (CPS client ctor) ............................. 12291555
async read(t,r,n){ ..................................... 12291734
settledKey( ............................................ 12292258
failedClosed( .......................................... 12292729
"mcpRegistry","read" (failedRead) ...................... 12293092
fail-closing enterprise governance ..................... 12293934
catalogFor(t){ ......................................... 12294165
fetchProfile / loadRegistryCatalog call site ........... 12294289
Pe("getMcpRegistry","network") (span) .................. 12294639
b$t=class (scheduler) .................................. 12294768
CWu=5*6e4 (refresh interval) ........................... 12295774
RWu={kind:"unread"} .................................... 12295879
AWu={kind:"ungoverned"} ................................ 12295899
L5e=class (profile source) ............................. 12296188
get registryCatalog(){ (L5e) ........................... 12297650
armTimedRefresh (call site) ............................ 12299422
F5e=class (governance facade) .......................... 12301332
get isEnterprise(){ .................................... 12301407
get mcpDisabled(){ ..................................... 12301477
get mcpReason(){ ....................................... 12301529
this.unreadable() (disabledReason call sites) .......... 12301552/12302635
get registryCatalog(){ (F5e) ........................... 12301874
getAccessMode(){ ....................................... 12301932
buildNotification(t){ .................................. 12302020
features(){ ............................................ 12302202
contentCollection: (informational) ..................... 12302356
disabledReason(t){ ..................................... 12302589
unreadable(){ (definition) ............................. 12302851
policy-refused (cache mapping) ......................... 12304557
function HWu( (cache key) .............................. 12308439
function een( (runner mapping) ......................... 12312218
function ren( (registry config builder) ................ 12314262
function nen( (accessMode filter) ...................... 12314437
function Q$t( (governance metrics) ..................... 12377726
GovernanceStatePushed: ................................. 12377766
J$t=class (broadcast dedupe) ........................... 12377821
governed=new F5e(...) .................................. 12405863
&&this.initializeMcp() (startup gate) .................. 12419215
reconcileMcpForAllSessions(){ .......................... 12422492
disableMcp(){ .......................................... 12424197
_kiro/mcp/status ....................................... 12424535/12584211
syncIdentity("initialize" .............................. 12430930
"_kiro/mcp/resetServer":t=>this.handleResetMcpServer ... 12436556
handleGetMcpPrompt( (map / impl) ....................... 12436642/12590568
handleGetMcpResource( (map / impl) ..................... 12436696/12590752
syncIdentity("newSession" .............................. 12459891
_kiro/mcp/governance_disabled .......................... 12463750
async handleIdentityChange( ............................ 12476431
pushGovernanceState(t,r,n){ ............................ 12477430
_kiro/governance/state ................................. 12477672/12699909
syncIdentity("prompt" .................................. 12492381
syncIdentity("loadSession" ............................. 12517150
stateMcpSwitch(gw(i)) (toggle write) ................... 12529502
else this.disableMcp() (toggle else) ................... 12529756
mcpManager:()=>this.governed.mcpDisabled ............... 12568174
getGovernanceState:()=> ................................ 12568235
resolveMcpServers(t){ .................................. 12577491
resolveRegistry(t){ ...mcpDisabled ..................... 12577602
async emitMcpStatus(t){ ................................ 12583448
async handleResetMcpServer(t){ ......................... 12589639
"MCP governance is disabled by your organization" ...... 12589715/12590098
governedMcpStatus(){ ................................... 12589936
(same, third occurrence) ............................... 12590822
getGovernanceNotification(){ ........................... 12610381
async refreshGovernance(){ ............................. 12610452
handlePolicyCheck(t){ .................................. 12613524
handlePermissionsExplain(t){ ........................... 12613941
handlePermissionsList(t){ .............................. 12614765
onGovernanceResolved: .................................. 12699837
onInitialized: (ws) .................................... 12700985
_kiro/governance/state (ws clientInit) ................. 12701006

# §2.5 addendum — client write paths (asserted 2026-10-16, same script)
Pbo={ (layer scope map) ................................ 11081441
"client-connection":"connection" (scope class) ......... 11081638
"client-connection":aIt (value codec) .................. 11810586
"This Client" (client-connection label) ................ 11812875
shown(t){return this.catalog.setting(IHr(t.krn).name) .. 11816116
"unread-key" (drop report) ............................. 11818086
"unknown-key" (drop report) ............................ 11818132
this.drops.report(t,this.source,n,{kind:"setting" ...... 11818194
_Ot=class (initialize intake) .......................... 11818254
EOt=class (sibling intake, kind Q0e) ................... 11818672
wire:w1("workflows") (first spec row) .................. 11821495
wire:F3o() (sessionEviction row) ....................... 11821778
LMu= (kiroAgent table) ................................. 11821917
Zce("knowledge","maxFiles") (spec row) ................. 11822141
wire:L3o( (knowledgeChunking row) ...................... 11822196
Zce("compaction","excludeMessages") (spec row) ......... 11822894
b9e=new nYr({startup:[],kiroAgent:LMu, (intake tables) . 11822944
session:[PMu,MMu,NMu,...FMu,DMu,OMu] (session table) ... 11822982
eue={name:"contribute-source",listing:0} ............... 11823040
UMu=k.object (contribution schema) ..................... 11823097
this.initializeIntake=new _Ot(b9e,i, (intake ctor) ..... 11888016
get registryUrl(){return this.value(Cm.mcpRegistryUrl)}  12301819

# §2.6 addendum — tamper surface (asserted 2026-10-16, same script)
extracted_kas_versions (sqlite migration) .............. 6887189
acp-server.js (embedded server-path literal) ........... 66799654
KIRO_KAS_SERVER_PATH (host resolver) ................... 34766077
"KAS bundle already extracted" (extraction dedup) ...... 34766360
"KAS extraction lock timed out; extracting without dedup" 34766388
"Cannot resolve the server for the v3 engine: KIRO_KAS" . 98824548
```

Reproduction: `F=<carve>/dist/server/acp-server.js; grep -obF '<literal>' "$F"` (byte offsets), then
`tail -c +<offset> "$F" | head -c 400`. Note GNU `grep -o` prints **line** offsets, which are
meaningless for the glued bundle — use `grep -obF`.

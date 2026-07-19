# cyril-0wyn — prove-it-prototype findings

Date: 2026-07-18/19. Target: KAS 2.13.0 (`@kiro/agent` 0.18.2), extracted
bundle at `~/.local/share/kiro-cli/kas/2.13.0-*`. cyril @ main (0.2.0-alpha.1).

## Q1 — What clientInfo does cyril actually put on the wire?

- **Probe**: `probe-a-dump-agent.sh` as the agent command under
  `cargo run --example test_bridge`; captured the live initialize frame.
- **Result**: `"clientInfo":{"name":"cyril","title":null,"version":"0.2.0-alpha.1"}`
  (pre-change baselines committed: `probe-a-baseline-capture.jsonl` default
  build, `probe-a-baseline-kas-build.jsonl` kas build; post-implementation
  capture with `title:"Cyril"` is `probe-a-post-impl-capture.jsonl`)
- **Oracle**: source text — `bridge.rs:660`
  `Implementation::new("cyril", env!("CARGO_PKG_VERSION"))` + workspace
  `version = "0.2.0-alpha.1"` (Cargo.toml:6). **AGREE.** No layer rewrites
  clientInfo en route.

## Q2 — Does shipped KAS actually warn-and-fallback on unknown names?

- **Probe**: `probe-b-name-ab.py` — standalone spawn of the 2.13.0
  `acp-server.js` (node, stdio, no auth needed for initialize), one
  initialize per name ∈ {cyril, kiro-cli, kiro-ide}; captures stderr AND
  the per-run `~/.kiro/logs/<ts>/kiro.log`.
- **Result** (`probe-b-results/`): `cyril` →
  `warn: Unrecognized clientInfo.name: 'cyril', falling back to inferred client type`;
  `kiro-cli`/`kiro-ide` → accepted silently (`Stored clientInfo.name: <name>`,
  no warn). **VERDICT: ALL-PASS.**
- **Oracle**: carved `resolveAgentContext` source
  (`oracle-resolveAgentContext.txt`). **AGREE.**
- **Probe bug found en route** (cause-3 disagreement, fixed): the KAS logger
  writes the log file through an async transport; killing the process right
  after the stdout response loses the initialize-handler lines. First run
  falsely showed "no warn". Fix: 3s flush wait + read the log file, not stderr.

## Q3 — Can the client detect its resolved identity from the wire?

- **Probe**: byte-diff of the three initialize responses.
- **Result**: identical except `logDir`/`filePath` values. **The resolved
  client type is not exposed by the initialize response** — the
  classification warn goes only to the server-side log file, whose path the
  initialize response happens to expose under `_meta`. Scope caveat
  (review 2026-07-19): only initialize was diffed; later session/tool/prompt
  traffic was NOT tested, and the allowlist difference plausibly surfaces
  downstream as available-tool differences. Do not cite this as a
  protocol-wide invisibility claim.

## New facts (not known before probing)

1. **The "inferred client type" fallback is environment-only** —
   `sandbox → kiro-web`, else `kiro-ide`. There is **no env-var or config
   override** for client type in `resolveAgentContext`. The hoped-for third
   option (honest name + env knob selecting the kiro-cli branch) **does not
   exist**. Only `KIRO_LOAD_ALL_REMOTE_TOOLS=true` (allowlist → `*`) exists,
   and it affects only remote tools, not persona/hooks.
2. **A fourth client-keyed behavior**: `honorsRepositories(ctx) =
   client === "kiro-web" || environment === "sandbox"` — repository honoring
   is never granted to kiro-ide/kiro-cli locally, but it's another branch to
   track (audit listed three effects; there are at least four).
3. `resolveRemoteToolAllowlist` carved verbatim
   (`oracle-resolveRemoteToolAllowlist.txt`): kiro-web → `*`; kiro-ide →
   channel-gated; kiro-cli → `[web_search] + searchMemories if memoryEnabled`;
   env bypass first.
4. **Standalone KAS completes initialize without any ACP-level auth
   exchange.** Narrowed (review 2026-07-19): the probe inherited `HOME`, and
   stderr shows `Auth: default token file` — ambient credential material was
   present, so this does NOT establish that a credential-less environment
   initializes; only that no ACP auth request/callback was needed before the
   initialize response.
5. `remote-tools-discovery.create {"client":"kiro-ide"}` fires at *startup*
   in all three runs — the discovery object is instantiated eagerly with the
   default before initialize. Corrected mechanism (review 2026-07-19,
   matching the Probe C addendum): the discovery's `getAllowedTools` closure
   reads `this.agentContext.client` **lazily at resolution time**, and
   initialize reassigns `this.agentContext`; `setClientType(...)` is the
   telemetry/activity-publisher side, not the discovery seam. Any future
   "allowlist actually applied" probe must instrument the lazy read
   (`Allowlist resolved`), not creation or setClientType.

## What I learned (gate sentence)

The escape hatch the design hoped for (honest clientInfo.name + an override
selecting the kiro-cli branch) does not exist — the fallback is inferred from
execution environment only, and the resulting misclassification is invisible
on the wire, so cyril must either accept the kiro-ide identity knowingly,
impersonate kiro-cli, or change KAS upstream.

## Probe C addendum (claim 8, slice 6 — 2026-07-19)

**INCONCLUSIVE by the anticipated path.** Standalone-spawn discovery fails
`TokenExpired` (-32000) before `[RemoteToolsDiscovery] Allowlist resolved`
(debug) ever fires, in BOTH arms (`probe-c-results/`). The settings key was
pinned from the bundle first: `memoryEnabled: isFeatureEnabled("memoryEnable")`
— the AgentSettings key is `memoryEnable`; the 2.13.0 "search_memories"
rename was the TOOL id, not the key. Verifying the searchMemories outcome
needs an auth-serviceable session; cyril-jrl1 narrows to exactly that
residue. One trap for future readers: `remote-tools-discovery.create
{"client":"kiro-ide"}` appears even in the name=kiro-cli treatment — that
line fires at STARTUP with the default client (findings fact 5); the
resolved allowlist reads `agentContext.client` lazily at discovery time, so
only `Allowlist resolved` is evidential, never the create line.

# kiro-cli 2.13.0 wire audit (2026-07-16, vs 2.12.3)

**Verdict: SAFE for cyril's current v2 path.** The v2 (Rust) ACP surface is frozen again at
symbol granularity; the release's substance is KAS-side (`@kiro/agent` 0.17.2 → **0.18.2**)
plus one new client-side extension surface (`_kiro/frontendToolCall`) that matters for the
KAS dialect track. Issue filed: **cyril-qc00** (frontendToolCall decline handler, KAS-2).

- Baseline: archived 2.12.3 (`~/.local/share/kiro-research/binaries/2.12.3/`). Direct
  succession — no release between 2.12.3 (2026-07-15) and 2.13.0 (2026-07-16).
- **Live same-day A/B: zero delta.** `probe-v2-surface-ab-2.11.0.py` against both archived
  binaries (2026-07-16): identical 24-command / 14-tool sets, identical 56 structural field
  paths across initialize / session/new / post-session notifications
  (`logs/v2-surface-{2.12.3,2.13.0}-ab-20260716.{jsonl,summary}`).
- **Backend-axis observation (not a binary change):** the *same* 2.12.3 binary advertised
  `tool_search` in its tool list on 2026-07-15 (15 tools) but not on 2026-07-16 (14 tools,
  both binaries). Same-day A/B proves it's not 2.13.0; attribute to a backend/config-side
  rollout or MCP/`toolSearch.*` settings dependence — worth re-checking next audit.

## Embedded changelog (6 items, 4 are V3/KAS)

- Added: [V3] **Introspect subagent** — answers questions about Kiro's features, helps write
  custom agents/hooks/steering
- Added: [V3] **Global hooks** — `~/.kiro/hooks/` applies to every workspace
- Changed: model-refusal errors no longer pin a toast (scrollback row only)
- Fixed: rate-limit errors persist in scrollback after the toast fades
- Fixed: [V3] always-accept approval loop on shell commands with backslash escapes
- Fixed: [V3] `HTTP_PROXY`/`HTTPS_PROXY` honored for backend API connections

## v2 Rust side (nm module-path diff, kiro-cli-chat 693.2 MB → 694.8 MB)

**ACP surface: frozen.** ACP handler module set byte-identical; `agent-client-protocol-0.10.4`
and `sacp-11.0.0` pins unchanged. AddOn billing shapes still dormant (counts equal).

Kiro-internal module deltas (all off-wire):

- **`agent::agent::agent_config::migration::{hooks,io,migrate,permissions,regex_to_glob,scan,tool_table}`**
  — the V2→"universal V2+V3" agent-config migration engine lands in Rust, paired with
  `chat_cli::cli::chat::internal::UpgradeAgentArgs` (`/upgrade-agent` becomes a Rust-side
  command; was tui.js-only in 2.12.3).
- **`agent::agent::ExecutingHooks`** + **`chat_cli::cli::agent::legacy::hooks`** — hooks
  execution/migration plumbing (global-hooks release theme).
- **`chat_cli::cli::chat::line_tracker`** — likely the rate-limit-scrollback persistence fix.
- `agent::agent::tools::identity::ToolCallIdentity`, `tools::mcp::McpTool` — refactors in the
  universal-agent tool crate. **`tools::mkdir` is gone** from that crate (glob and
  switch_to_execution remain; only their free-function module entries shifted). This crate is
  the internal universal agent, not the v2 ACP-advertised toolset — `mkdir` was never in the
  advertised list, and the live A/B confirms the advertised set is unchanged.
- Dependency adds skew toward **archive + image encoding**: `zip::crc32`, `zstd::stream`
  (read+write), `liblzma`, `libbz2`, `deflate64`, `flate2` bufread, `tiff::encoder`,
  `qoi::encode`, `exr` block writer, `sha1`, `simd_adler32`. Consistent with asset
  (re)packaging work; no wire effect observed.

Metric catalog deltas (telemetry only): new **`kiro_cli_cloud_session_total`**
(`cloud_event` attr; see tui.js below), `kiro_cli_feature_used_total` reports **`version_full`**
instead of `version_minor_bucket` (env `KIRO_VERSION_MINOR_BUCKET` deleted from tui.js),
`kiro_cli_user_turn_completed` **drops `conversation_id`** (privacy trim),
`kiro_cli.bedrock.empty_response.retries` gains outcome values `denied/recovered/still_empty`.

## tui.js (carve VERIFIED vs embedded sha; 12,656,313 → 12,660,250 bytes, +3.9 KB)

Archived: `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.13.0.js` (+`.sha256`).
Settings keys identical; command registry identical; stop-reason literals identical.

- **`_kiro/frontendToolCall` client handler + handshake cap (NEW)** — the v2 bun TUI now
  registers `{key:"frontendToolCall", value:true, method:"_kiro/frontendToolCall"}` and
  **declines every call** with `{outcome:"cancelled"}` (`[frontend-tool-call] declining
  unhosted client tool (toolCallId=…, title=…)`). Agent-side support existed in KAS 0.17.2;
  2.13.0 is the first *client* to advertise it. → cyril-qc00.
- **`memoryEnable` handshake cap (NEW, gated)** — cap-injection table `[["memory","memoryEnable"]]`:
  when the `memory` feature flag is on, the client adds `memoryEnable:{enabled:true}` next to
  `codeIntelligence/knowledge/thinking/subagentOrchestration`. Pairs with the KAS-side
  `search_memories` remote tool gate (below). **Kiro persistent memory is coming** — watch item;
  overlaps cyril's own memory-stage ambition.
- **`_kiro/sessions/changed` now consumed by the TUI** (existed on KAS since 2.8.1): bridged to
  an internal `session_roster_delta` stream event → `applySessionRosterDelta`. Multi-session
  roster UI activating.
- **Cloud-session lifecycle instrumentation**: `kiro_cli_cloud_session_total` with
  `cloud_event ∈ {detached, reattached, start_failed, fell_back_local, turned_off}` — the
  2.12.3 remote/cloud-session stack is being hardened toward launch.
- Removed strings: `code_review`, `tui-verbosity`, `KIRO_VERSION_MINOR_BUCKET`.

## KAS: @kiro/agent 0.17.2 → 0.18.2 (bundle carved from binary — see methodology note)

**Zero file adds/removes** (identical 2,680-file tree), **zero `_kiro/*`, `_session/*`,
`_message/*` method-string delta**. acp-server.js +13 KB; full line diff is only ~1,360
changed lines (bundle is unminified).

**Live KAS host-init leg (2026-07-16): zero delta.** `probe-kas-host-init-2.12.0.py` against
the archived 2.13.0 binary — the normalized initialize response is byte-identical to the
2.12.3 capture (`logs/kas-host-init-{2.12.3-20260715,2.13.0-20260716}.json`): same
`_meta.kiro` cap set, cloud caps still at dormant local values (`executionTargets ["local"]`,
`sessionSources ["local"]`, `sessionListScopes ["workspace"]`, `sourceProviders false`), no
`memoryEnable` (it's a *client*-side cap and feature-gated). The run also triggered real KAS
self-extraction (`kas/2.13.0-6b915aea…`), whose tree matches the carved bundle exactly
(identical acp-server.js sha, identical 2,680-file set) — validating the login-free carve.

Content deltas:

- **Introspect subagent (the changelog headline)** — new `INTROSPECT_DEFINITION`
  (`id:"introspect"`, "Answers questions about Kiro itself… using official kiro.dev
  documentation") whose prompt **fetches `https://kiro.dev/llms.txt` live** and follows the
  `.md` URLs. Distinct from the `Introspect` *SyncTool* (BM25 over the embedded docs index,
  `reference_kiro_embedded_docs_corpus`) which already existed in 0.17.2 and is listed in
  `PLAN_TOOLS`. Wire effect: new subagent name in the KAS custom-agent registry; plan-mode
  tool list includes `introspect`.
- **Global hooks** — hook loader gains `globalRoots` (`deps.globalHookRoots`), loading
  `~/.kiro/hooks/` for every workspace with case-normalized `hooksDirKey` dedup when a
  workspace root overlaps a global root.
- **Proxy support** — `getProxyAgent`/`getProxyRequestHandler` (HTTP, HTTPS + SOCKS via
  `SocksProxyAgent`) wired into backend SDK clients; proxy URLs redacted in logs.
- **`createdReason` enum: `thread` → `tangent`** (`["human","rewind","subagent","tangent"]`).
  Rides `session/new` `_meta.kiro.createdReason` (safeParse'd) and persisted session metadata
  used by `session/list` filtering. This aligns KAS session lineage with the long-standing v2
  `/tangent` mode (Amazon Q heritage: `enter_tangent_mode`/`exit_tangent_mode_with_tail`
  symbols) — **tangent/branch sessions are coming to KAS**. Relevant to cyril-nn85 metadata
  modeling.
- **Memory gate** — `resolveRemoteToolAllowlist(client, channel, {memoryEnabled})`: for client
  `"kiro-cli"`, remote (backend-executed) tools = `[web_search]` + **`search_memories`** when
  `isFeatureEnabled("memoryEnable")`. Tool constant existed in 0.17.2; the gate wiring is new.
  Dormant until the flag flips.
- **Infra-safety workspace scoping** — `resolveSafetyScopeKey(sessionId)`: deterministic
  workspace-folder-set hash scoping "formalized properties" so they persist/enforce across
  sessions in one workspace (undefined for relayed/empty-workspace sessions → single-session
  behavior). Backend binds to the authenticated user. Extends the dormant
  `chat.enableInfraSafetyMonitor` stack from 2.12.3.
- Telemetry tracking table catches up with existing tools: `knowledge`,
  `update_session_information`, **`c2s_query` ("code-to-spec query/view tools")**; abort
  classification now also covers permission-policy rejections.

### `_kiro/frontendToolCall` contract (for KAS-2)

Agent→client ext **request**; payload is ToolCallUpdate-shaped (`toolCallId`, `title`,
`rawInput?`…). Client advertises `frontendToolCall: true` in `KiroClientMeta`. Reference
client behavior (v2 TUI): reply `{outcome:"cancelled"}` for every call. In relayed/cloud
sessions it is one of three consent-callback kinds — `permission` (disconnect policy: defer,
5 min TTL), `frontendToolCall` (**fail**), `openUrl` (fail) — answered upstream via the
web-portal `RespondToFrontendToolCallCommand` (csrfToken + profileArn; csrf plumbing
pre-existed in 0.17.2).

## Embedded doc-manifest

Frozen: 86+118 manifests, merged 138 docs — **zero added/removed**; single delta is
`features/model-refusal-alerts.md` revalidated `2026-07-12` → `2026-07-14`. No unannounced
doc-level features this release.

## Methodology addendum — carving the KAS bundle without login

KAS assets normally self-extract to `~/.local/share/kiro-cli/kas/<ver>-<sha>/` on first KAS
launch, which **requires auth**. They can be carved offline: `kiro-cli-chat` embeds
**`kas-bundle.tar` as a gzip stream** (magic `1f 8b 08 08` + FNAME `kas-bundle.tar`, at offset
~7.19 MB in both 2.12.3 and 2.13.0 — early in the binary, far before the tui.js bundle).
`data.find(b'\x1f\x8b\x08\x08')` + check FNAME, then `zlib.decompressobj(31)` → 550 MB tar.
Note: the self-extract dir-name sha is **not** sha256 of the raw tar (2.12.3 tar hashes to
`5751dda1…`, dir says `88626245…`) — don't use one to predict the other. The 2.13.0 tar
sha256 = `659ee19d533453d50a1261438cf699c4705a668b2a8d52dbf01397634df13e34`.

## Cyril impact summary

| Finding | Action |
|---|---|
| v2 ACP surface frozen (live A/B + static, zero delta) | none |
| `tool_search` vanished from v2 tool list same-binary-over-time | backend/settings axis — recheck next audit |
| `_kiro/frontendToolCall` + client cap | **cyril-qc00** — decline handler in KAS dialect (KAS-2) |
| `createdReason` incl. `tangent` on `_meta.kiro` | fold into session-metadata modeling (cyril-nn85 context) |
| `memoryEnable` cap + `search_memories` remote tool | watch — dormant behind feature flag |
| Cloud-session lifecycle metrics/states | watch — remote sessions approaching launch |
| Global hooks in `~/.kiro/hooks/` | no cyril change (agent-side, off-wire) |

# kiro-cli 3.1.0 wire audit — one binary, V3 only, additive wire; cyril drops the new credits and the streamed sub-agent text (covers 2.33.0 … 3.0.0, 3.1.0)

Audited 2026-11-08. Baseline: 2.32.0 ([kiro-2.32.0-wire-audit.md](kiro-2.32.0-wire-audit.md)). Thirteen releases shipped after
2.32.0 (2.33.0, 2.33.1, 2.34.0, 2.34.1, 2.34.2, 2.35.0, 2.35.1, 2.35.2, 2.36.0, 2.37.0, 2.37.1, 3.0.0, 3.1.0); all fold into this
document. Scope: discovery on the **KAS (v3) track** — new or changed commands, messages carrying new fields, feature flags for new
or limited-release features. KAS is cyril's default engine (cyril-4c4d, 2026-10-19); v2 findings are demoted, and 3.0.0 retires the
Classic engine outright. Six lanes: kas-core, kas-workflow, tui and host static lanes, the standing **v3-coverage** parity inventory,
and one live lane (KAS 0.66.43 vs 0.66.91 paired on the same day, the archived 3.1.0 host, and the real cyril binary on KAS through hex1b).

**Verdict: SAFE for cyril's default (KAS) path — no wire break.** KAS 0.66.43 → 0.66.91 removes no `_kiro/*` method (123 → 138), no
`session_info_update` kind (25 → 29) and no `sessionUpdate` variant; initialize, session/new, prompt, permission, terminal and client-hook frames keep
their shape; 0 × -32601 on anything cyril sends (LIVE, legs 0–2); the real cyril binary (build e3a3868d) starts, renews its token through
`kiro-cli chat _ get-kas-token`, dispatches skills and drives `/model` `/effort` `/mode` on 0.66.91 (LIVE, leg 5), and the follow-up leg (§8a) confirmed Always-allow
persistence, `KIRO_HOME`, the workspace tier, MCP tool loading and the elicitation decline end to end — and found that the cyril-w55l
20× step-approval loop **no longer reproduces**. The release moves behaviour
through **additive fields, default flips and one host-side router**, and that is where cyril loses ground. One upgrade-day break sits outside the
default path: the explicit V2 wrapper (`--agent-engine v2`) is handed KAS by a 3.x host.

The three findings that matter most:

1. **Sub-agent text now streams on the parent session (P2, LIVE, new in 0.66.91) — cyril-lpdk.** The child's reply arrives as ordinary
   `agent_message_chunk` frames on the parent session id, distinguishable only by `_meta.kiro.agentSubtaskId`; the orphan "Subagent Response" update
   that cyril-1v25 learned to render is gone. cyril never reads `agentSubtaskId`, so the child's words are appended to the parent's reply as one message.
2. **Credits moved onto the wire and cyril drops them (P2, LIVE) — cyril-t2w5, cyril-bgvj.** Every turn now ends with `session_info_update
   kind:"session_usage"` carrying the cumulative session credits, replayed on `session/load`; workflow `node_complete`/`run_complete`/snapshots carry
   per-step `credits[]`. cyril keeps summing `turn_completion` itself, so `/usage` and the footer restart at zero after `/load` and `/workflow status`
   shows no step costs.
3. **Model fallback is on by default and invisible in cyril (P2, STATIC default flip; the kinds pre-exist) — cyril-aoby.** `model_fallback_{refusal,
   availability,auto_refusal}` flipped to `true` in 0.66.91; cyril drops `model_fallback` / `model_routed`, so a mid-session model swap leaves the toolbar
   stale. Adjacent and LIVE: a 3.x `kiro-cli acp` routes an unlisted client such as cyril to **v3**, so cyril's V2 wrapper must pin `--agent-engine v2`
   (cyril-k3hf).

Decided during the audit (owner): cyril gets its own "trust this folder?" gate before `session/new` (cyril-mq15, P2) — Kiro's 2.33.0 prompt is
TUI-local and KAS loads a workspace's hooks, agents, MCP servers and recipes on bare ACP without asking (LIVE).

Evidence: [`experiments/conductor-spike/audit-3.1.0/`](../experiments/conductor-spike/audit-3.1.0/) (`RUN-STATE.md`, `<lane>-findings.md`,
live captures); trimmed capture bundles `experiments/conductor-spike/kas-*-3.1.0.jsonl`.

## 1. Releases and artifacts

| ver | feed date | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS class | TUI class | host |
|---|---|---|---|---|---|---|---|---|
| 2.32.0 (OLD) | 2026-10-15 | 2026-10-15T21:16Z | d6fb1e94 | 9d1527e4… | 0.66.43 | A | a (13,723,687 B) | kiro-cli 43,931,168 · kiro-cli-chat 207,496,744 · kiro-cli-term 34,060,512 |
| 2.33.0 | 2026-10-20 | 2026-10-20T01:23Z | 2d6b9103 | 853af800… | 0.66.48 | B | b (14,019,537) | 42,892,192 · 208,371,944 · 33,371,488 |
| 2.33.1 | 2026-10-21 | 2026-10-21T16:05Z | 2d6b9103 | e798062c… | 0.66.58 | C | c (14,038,748) | 42,879,904 · 205,977,448 · 33,369,248 |
| 2.34.0 | 2026-10-23 | 2026-10-23T06:07Z | 835ff364 | 11dce928… | 0.66.62 | D | d (14,139,362) | 43,027,744 · 207,456,488 · 33,376,416 |
| 2.34.1 | 2026-10-26 | 2026-10-26T15:56Z | 835ff364 | da520f74… | 0.66.62 | D (+THIRD-PARTY-NOTICES) | e (14,164,936) | 43,026,336 · 207,512,872 · 33,379,680 |
| 2.34.2 | 2026-10-26 | 2026-10-26T23:54Z | 835ff364 | 960175e0… | 0.66.62 | D (==) | e (==) | 43,019,680 · 207,513,128 · 33,371,616 |
| 2.35.0 | 2026-10-27 | 2026-10-27T16:36Z | 835ff364 | 54775002… | 0.66.70 | E | f (14,178,633) | 43,028,000 · 207,583,784 · 33,376,672 |
| 2.35.1 | 2026-10-27 | 2026-10-27T21:25Z | 835ff364 | 83052eec… | 0.66.70 | E (==) | f (==) | 43,018,272 · 207,583,656 · 33,369,120 |
| 2.35.2 | 2026-10-28 | 2026-10-29T00:58Z | 835ff364 | 501417ab… | 0.66.70 | E (==) | f (==) | 43,015,584 · 207,583,912 · 33,380,128 |
| 2.36.0 | 2026-10-29 | 2026-10-29T23:03Z | 6a650005 | 8db78ee3… | 0.66.70 | E (==) | g (14,196,690) | 43,023,264 · 219,286,760 · 33,378,912 |
| 2.37.0 | 2026-10-30 | 2026-10-30T18:29Z | 8d1e04ec | e9df44ff… | 0.66.70 | E (==) | h (14,204,223) | 43,026,976 · 219,495,016 · 33,376,288 |
| 2.37.1 | 2026-10-31 | 2026-10-31T21:28Z | 1dcf6cd4 | 65e3d531… | 0.66.70 | E (tslib bump) | i (14,210,946) | 43,030,048 · 219,500,200 · 33,380,960 |
| 3.0.0 | 2026-11-02 | 2026-11-02T18:32Z | **unknown** | 224279a9… | 0.66.70 | E (semver bump) | i (==) | **single `kiro-cli` 219,494,120** |
| 3.1.0 (NEW) | 2026-11-05 | 2026-11-05T17:50Z | 5acd3b95 | 9efc6948… (== latest manifest) | 0.66.91 | F | j (14,291,547) | **single `kiro-cli` 220,246,888** |

- **Five real KAS deltas** (A→B→C→D→E→F). `diff -rq` between consecutive carves, licences excluded: 0.66.43→0.66.48 changes
  `acp-server.js`, `acp-callback-auth-provider.d.ts` and bumps `@cedar-policy/cedar-wasm`; 0.66.48→0.66.58 changes `acp-server.js`
  only; 0.66.58→0.66.62 changes `acp-server.js`, `acp-server.externals.json` and adds `tree-sitter-c`, `tree-sitter-cpp`,
  `node-addon-api`, `node-gyp-build`; 0.66.62→0.66.70 changes `acp-server.js` and `client-connections.d.ts`; 0.66.70→0.66.91
  changes `acp-server.js` (15,025,207 → 16,099,406 B), `externals.json`, `client-connections`, `env-auth-provider`,
  `file-auth-provider`, `kiro-session-config` `.d.ts`, adds `discovered-profile.d.ts`, `node-compile-cache.d.ts` and the
  `asn1`, `bcrypt-pbkdf`, `buildcheck`, `cpu-features` packages. 2.34.1→2.34.2, 2.35.0→2.35.2, 2.35.2→2.36.0→2.37.0 are
  byte-identical KAS; 2.37.1 and 3.0.0 differ from 2.37.0 only in `tslib` / `semver` (dependency bumps, same `@kiro/agent` 0.66.70).
- **TUI bundle:** nine distinct bundles; 2.34.2 == 2.34.1, 2.35.1 == 2.35.2 == 2.35.0, 3.0.0 == 2.37.1.
- **Host:** every binary distinct (all stripped). **3.0.0 merges everything into one `kiro-cli`** — the 2.37.1 `kiro-cli-chat` rebuilt and
  renamed (identical crate set, byte-identical `.rodata`, zero new source paths; §5) — and drops the 43 MB launcher, the 33 MB `kiro-cli-term`
  and the `q`/`qchat` shims.
  3.0.0 is the first release with `BUILD_HASH=unknown`. 2.33.0/2.33.1 share `2d6b9103`, 2.34.0–2.35.2 share `835ff364` (train stamps,
  trap 20).
- **Frames** (csize / dsize): four top-level zstd frames in every release. Node ELF 32,031,786 / 124,679,552 (v22.22.2, through
  2.35.2) → 32,070,008 / 124,902,584 (v22.23.4, 2.36.0+); Bun ELF 29,887,616 / 79,500,640 (1.4.2) → 41,444,530 / 107,308,936
  (1.4.4, 2.36.0+); KAS tar 20,826,442 / 159,590,400 (0.66.43) → 22,109,360 / 188,856,320 (0.66.62) → 22,830,152 / 193,198,080
  (0.66.91); TUI 2,133,192 / 13,723,687 → 2,284,344 / 14,291,547. Frame order changed at 2.37.0 (tar, Node, Bun, TUI → tar, Bun, TUI,
  Node); nested magics 62 → 96 from 2.36.0 (Bun 1.4.4 embeds more modules).
- **Origin and packaging:** 3.x serves `.tar.zst`, `.tar.gz`, `.zip`, `.rpm` and a `.deb` — **no `.tar.xz`** (HTTP 403). The
  manifest labels every Linux package `headless`, macOS `standalone`, the Windows MSI `full`. 3.1.0 `kirocli-x86_64-linux.tar.zst`
  sha256 `9efc6948…2477` == latest manifest. The 3.x tarball is `kirocli/bin/kiro-cli` + `install.sh` + `README` + `BUILD-INFO`;
  `install.sh` removes legacy shell integration and runs `kiro-cli _ cleanup-autocomplete-artifacts`.
- **Installed:** the machine still runs 2.32.0 (`~/.local/bin/kiro-cli`, tui.js `eba39f7b…`, self-extracted KAS 2.28.0–2.32.0);
  nothing newer has launched here, so there is no installed==archived proof for NEW and no self-extract to diff. Every finding below
  is "what changes the day the user upgrades".
- **Changelog source:** `stable/<ver>/feed.json` for each of the thirteen versions, copied to `audit-3.1.0/feed-<ver>.json`
  (28 + 16 + 28 + 1 + 1 + 15 + 1 + 1 + 15 + 18 + 4 + 3 + 57 = 188 lines). `audit-state.sh`'s bounded probe (minor+1..+2) missed
  2.35.x–2.37.x; the 3.0.0 feed's embedded 2.37.1 entry exposed the gap (method note §10).

### 1a. Changelog reconciliation

All 188 changelog lines of the thirteen feeds, each placed by the lane that owns the surface (merged by
`experiments/conductor-spike/reconcile-changelog-3.1.0.py` from the lanes' own reconciliation tables; `audit-3.1.0/changelog-reconciliation.md`).
"KAS-internal" = behaviour change with no new wire field; "TUI-local" = no wire part; `L Tn` = located in the TUI bundle (tui row Tn).
Live verdicts for the lines the static lanes could not pin are in §8.

| ver | # | line | location (lane: placement) |
|---|---|---|---|
| 2.33.0 | 1 | [added] [V3] Kiro asks you to trust a folder before it loads that folder's hooks, agents, MCP servers or workf… | tui: **L T16** — TUI-local gate, no frame to KAS (`workspaceTrust` 0 -> 70) · kas-core: host/TUI prompt (`resolveWorkspaceTrust`, `workspaceTrusted` file); KAS entry hard-codes `workspaceTrusted:!0`; gates pre-exist (Deltas) · kas-workflow: kas-core lane (trust); `_kiro/workflow/new` refusal text for out-of-force recipes unchanged since 0.66.43 — WF-L9 |
| 2.33.0 | 2 | [added] [V3] Workflow plan approvals show a compact step tree with expandable details | tui: **L T15** (`workflowPlan` 0 -> 3) · kas-core: kas-workflow / TUI · kas-workflow: TUI-only; `workflowPlan` builder/approval shape unchanged |
| 2.33.0 | 3 | [changed] Faster `grep` tool searches on large projects: files are now scanned in parallel across CPU cores | kas-core: KAS-internal (ripgrep concurrency), no wire |
| 2.33.0 | 4 | [changed] Mermaid flowcharts with subgraphs now draw in chat instead of showing their source | tui: TUI-local (renderer) · kas-core: TUI |
| 2.33.0 | 5 | [changed] [V3] Long sessions hold less memory, so the agent stays responsive after hours of use | kas-core: KAS-internal (not located statically; `sessionEviction` pre-exists) |
| 2.33.0 | 6 | [changed] [V3] Workflow GitHub PR watches now act on merge conflicts, failed checks, and answered threads | kas-core: kas-workflow (zod +`pullRequest, reviewThreads, baseRef, oid` @0.66.48) · kas-workflow: 0.66.48 `github-pr` handler (`mergeable,mergeStateStatus`, reviewThreads/baseRef GraphQL, conflict re-wake) + `wf-pr-responder` prompt |
| 2.33.0 | 7 | [fixed] [V3] Terminal title shows the session title when `chat.terminalTitle` is enabled | tui: TUI-local (`chat.terminalTitle` 1 == 1) · kas-core: TUI |
| 2.33.0 | 8 | [fixed] [V3] Replies Kiro starts on its own, such as after a workflow finishes, no longer cut off after a few … | kas-core: KAS-internal (agent-initiated prompt path) · kas-workflow: not located: wake path and `promptAgentInitiated` token-identical 0.66.43 -> 0.66.48; candidate TUI rendering or non-workflow turn code — WF-L7 |
| 2.33.0 | 9 | [fixed] [V3] `/agent`, `/model`, and `/effort` complete a partly typed argument inline again | tui: TUI-local · kas-core: TUI |
| 2.33.0 | 10 | [fixed] [V3] Editing sandbox network rules in `permissions.yaml` now applies to the running sandbox | kas-core: KAS-internal (`setNetworkRules`, `updateProxyPolicy`) |
| 2.33.0 | 11 | [fixed] [V3] `/sessions` no longer freezes for seconds at a time while its search index catches up | tui: TUI-local · kas-core: KAS-internal (session index) |
| 2.33.0 | 12 | [fixed] [V3] Fallback models can be set again from the `/model` pane and `/model fallback` | tui: **L T6** — `_kiro/modelFallback/{get,set}` added; **dark on KAS** · kas-core: `_kiro/configuration/set` fallback chains (0.66.62) / TUI; flags Deltas |
| 2.33.0 | 13 | [fixed] Clearer Mermaid node descriptions wrap across up to four rows, with an ellipsis for longer text | tui: TUI-local · host: TUI |
| 2.33.0 | 14 | [fixed] Starting Kiro from your home directory no longer lists your global agents twice | tui: no wire part; not located to a literal · kas-core: TUI / KAS-internal · host: host/`agent` crate (logic |
| 2.33.0 | 15 | [fixed] [V3] In your home directory, `/sandbox` no longer treats your global sandbox settings as project setti… | tui: TUI-local |
| 2.33.0 | 16 | [fixed] [V3] `kiro-cli chat --agent <agent> @agent-sop:<name>` runs the SOP instead of sending the text as typ… | tui: TUI/host argument preprocessing; no wire literal located · kas-core: TUI/host · host: host launch (logic) |
| 2.33.0 | 17 | [fixed] [V3] Cloud config keeps syncing after a request that never gets an answer | kas-core: cloud config sync — KAS cloud-config client internal (`fetch_cloud_config` hidden tool pre-exists); no wire field (hand-placed) |
| 2.33.0 | 18 | [fixed] [V3] `/code` in cloud sessions shows the sandbox workspace root and `lsp.json` path | tui: TUI-local (2.32.0 T16 fields) |
| 2.33.0 | 19 | [fixed] [V3] Sessions no longer show a cloud config sync card when your cloud config is empty | tui: TUI-local · kas-core: cloud / TUI |
| 2.33.0 | 20 | [fixed] [V3] The `code` tool reports diagnostics from Python and other slow language servers | kas-core: KAS-internal (LSP init) |
| 2.33.0 | 21 | [fixed] [V3] Throttling errors now tell an overloaded model apart from your own rate limit | kas-core: `display_error` variants (`reason:throttled`, `outcome:throttled` @0.66.58) — cyril handles only `mcp_connection_error` display_errors (kas.rs:634-651; cyril-58uv) |
| 2.33.0 | 22 | [fixed] [V3] Windows workflows no longer reject recipe files whose drive letter case differs | kas-workflow: not located (no literal change; code-only) |
| 2.33.0 | 23 | [fixed] [V3] Workflow review loops now stop once the reviewer approves instead of running to their limit | kas-workflow: 0.66.48 fileCheck `jsonPath` parser (`$.` prefix resolves; `[ ] * ..` rejected at load) [INFERENCE on causality] — WF-L5 |
| 2.33.0 | 24 | [fixed] [V3] Workflows now run when your organization disables the Auto model instead of failing | kas-core: kas-workflow · kas-workflow: 0.66.48 run-failure classification (`model_unavailable` <- `MODEL_TEMPORARILY_UNAVAILABLE`); the step model fallback chain (step -> workflow -> parent) is unchanged, so the "run" half is not located (body-unproven) |
| 2.33.0 | 25 | [fixed] Pressing Tab after `/model ` shows `set-current-as-default` as a sub-command again, instead of listing… | tui: TUI-local (`set-current-as-default` 6 == 6) · host: TUI |
| 2.33.0 | 26 | [fixed] [V3] `/code summary` now generates codebase documentation from the code intelligence overview instead … | tui: **L T24** (`summary:` 0 -> 1) · kas-core: TUI / KAS-internal |
| 2.33.0 | 27 | [security] [V3] Sandbox network rules now match host names regardless of letter case | kas-core: KAS sandbox network-rule matcher (internal; no literal change) — cyril has no sandbox (hand-placed) |
| 2.33.0 | 28 | [security] [V3] On Windows, hook commands no longer run a same-named program from the working directory | kas-core: KAS-internal (sandbox; hook spawn hardening) |
| 2.33.1 | 1 | [changed] [V3] When Workflows are enabled, a workflow now starts only when you explicitly ask for one | kas-core: kas-workflow (`workflowLaunch` zod @0.66.58) · kas-workflow: 0.66.58 `workflowLaunch` default `when-asked` + `# Workflow Orchestration` rewrite |
| 2.33.1 | 2 | [fixed] `/tools` lists each tool with one labelled line per field when the screen reader setting is on, so scr… | tui: TUI-local (`screen reader` 3 == 3) · host: TUI |
| 2.33.1 | 3 | [fixed] `kiro-cli mcp add --agent` now grants the new server to agents whose `tools` list lacks it, and upgrad… | host: **host** `cli/mcp` (logic |
| 2.33.1 | 4 | [fixed] [V3] Resuming a V2 session now tells you its saved tool approvals weren't carried over and how to keep… | tui: not located in the TUI (2.33.1 adds no such copy); KAS/host · kas-core: TUI · host: host/TUI (logic) |
| 2.33.1 | 5 | [fixed] [V3] Compaction retries without images when the model rejects history images | kas-core: KAS-internal (`summarization.skip_repeated_compaction` @0.66.58; image strip on compaction retry) (hand-placed) |
| 2.33.1 | 6 | [fixed] [V3] MCP sign-in no longer leaves a pinned OAuth callback port occupied afterward | kas-core: KAS-internal (MCP OAuth) |
| 2.33.1 | 7 | [fixed] [V3] Unavailable optional power MCP servers no longer block prompt turns | kas-core: KAS-internal (`resolvePowerServer`, later `kind:mcp_unavailable` @0.66.70) |
| 2.33.1 | 8 | [fixed] [V3] Long image sessions omit older images to stay within the request limit | kas-core: KAS-internal (`summarization.skip_repeated_compaction` @0.66.58; image strip) |
| 2.33.1 | 9 | [fixed] [V3] Workflow plan approvals open with the first offered option highlighted, so Enter selects it | tui: TUI-local · kas-core: TUI · kas-workflow: TUI-only |
| 2.33.1 | 10 | [fixed] HTML doctypes and tag brackets in code previews use theme colors with readable contrast | tui: TUI-local · host: TUI |
| 2.33.1 | 11 | [fixed] [V3] Custom agents running as subagents can search their own knowledge bases | kas-core: KAS-internal (`subagent.profileSkills.resolve` @0.66.58) |
| 2.33.1 | 12 | [fixed] [V3] MCP servers added from the registry launch the catalog's package version again | kas-core: KAS-internal |
| 2.33.1 | 13 | [fixed] [V3] Messages sent after a spec task run can keep updating its tasks | kas-core: KAS-internal (spec) |
| 2.33.1 | 14 | [fixed] [V3] Agents that use `@builtin` now include web search, as in V2 | kas-core: KAS-internal (tool tag expansion) · host: KAS |
| 2.33.1 | 15 | [fixed] [V3] Dispatched subagents now load their own skills instead of the parent agent's skills | kas-core: KAS-internal (`subagent.profileSkills.resolve` @0.66.58) (hand-placed) |
| 2.33.1 | 16 | [fixed] [V3] User agents keep their file allow and deny rules when a session starts in the home folder | kas-core: KAS-internal (policy load) |
| 2.34.0 | 1 | [added] [V3] Support for MCP elicitation, so MCP tools can ask you for input through a form or a link | tui: **L T8** · kas-core: **no KAS delta**: `_kiro/mcp/elicitation` call site identical (2 = 2), modes form/url pre-exist; TUI responder (Deltas) |
| 2.34.0 | 2 | [added] [V3] Type suggested changes directly into a workflow plan approval | tui: **L T14** — `reject_once` + `_meta.kiro.rejectionReason` · kas-core: kas-workflow (`planFeedback`, `appliedPlanFeedback` zod @0.66.58) · kas-workflow: TUI UI over the existing `_meta.kiro.rejectionReason`; engine side (0.66.58) persists it and builds `appliedPlanFeedback` |
| 2.34.0 | 3 | [added] [V3] Delete memories you no longer want from `/memories` | tui: **L T17** · kas-core: TUI over pre-existing `_kiro/memory/delete` (4 hits in 0.66.43) |
| 2.34.0 | 4 | [added] `/model edit` hides models you do not use from the `/model` picker | tui: **L T23** · kas-core: TUI · host: **host key** `chat.modelPicker.hiddenModels` + TUI |
| 2.34.0 | 5 | [added] [V3] Workflow use under Features in `/settings` to choose when Kiro starts workflows on its own | tui: **L T20** — `set_config_option configId:"workflowLaunch"` · kas-core: kas-workflow / TUI · kas-workflow: TUI `chat.workflowLaunch` -> `session/set_config_option workflowLaunch` (engine option 0.66.58) |
| 2.34.0 | 6 | [added] [V3] `--trust-workspace` skips the workspace trust prompt for one launch without saving the decision | tui: **L T16** (one-launch skip, no persistence) · host: **host flag** (clap text [0,1]) |
| 2.34.0 | 7 | [changed] [V3] Tool approvals list Allow once and Deny once first, then the options that create an allow or de… | tui: **L T13** · kas-core: TUI ordering; KAS `buildPermissionOptions` unchanged |
| 2.34.0 | 8 | [changed] [V3] The session dashboard asks `Do you want to delete …?` before deleting a session | tui: TUI-local (0 -> 1) · kas-core: TUI |
| 2.34.0 | 9 | [changed] [V3] Shell commands Kiro cannot parse are denied automatically instead of prompting you to approve t… | kas-core: KAS `PolicySession.unparseable()` (0.66.62; Deltas) |
| 2.34.0 | 10 | [changed] [V3] Subagents no longer get the MCP servers declared inline in their parent agent | kas-core: KAS-internal (`mcp.delegation.*`, `mcp.session.admission_denied` @0.66.62) |
| 2.34.0 | 11 | [fixed] Show `Working...` when the agent goes quiet while preparing a tool call instead of a frozen screen | tui: TUI-local (0 -> 1) · kas-core: TUI · host: TUI |
| 2.34.0 | 12 | [fixed] [V3] Headless mode no longer answers the agent's questions or approves requests that name no trusted t… | tui: TUI `noInteractive` guards; no wire change · host: **host** non-interactive client (`launch/secret_storage.rs` [0,8] = the saved-sign-in store) |
| 2.34.0 | 13 | [fixed] [V3] Headless mode reuses MCP sign-ins saved by interactive sessions | kas-core: host/TUI; MCP credential store · host: **host** non-interactive client (`launch/secret_storage.rs` [0,8] = the saved-sign-in store) |
| 2.34.0 | 14 | [fixed] [V3] `/sessions` opens and scrolls without freezing when you have many saved sessions | tui: TUI-local |
| 2.34.0 | 15 | [fixed] Enterprise overage status and consumed credits are visible in `/usage` | tui: **L** — TUI renders new fields of the existing `_kiro/account/getUsage` reply (`overage` 91 -> 103); field names are the kas-core lane's · kas-core: `_kiro/account/getUsage` consumer (TUI); no schema delta located · host: TUI/KAS (no host literal) |
| 2.34.0 | 16 | [fixed] [V3] Workflows pick up where they left off after a restart, without replanning or repeating steps | kas-core: kas-workflow · kas-workflow: 0.66.62 `classifyInterruptedNodes` (+ 0.66.43 pending hand-off/step-update restore) |
| 2.34.0 | 17 | [fixed] A refusal partway through a reply now continues on the fallback model instead of starting over | kas-core: model fallback (`model_fallback_auto_refusal` flag @0.66.58; default true @0.66.91) · host: host v2 api_client (logic) |
| 2.34.0 | 18 | [fixed] With `chat.allowIcons` off, the TUI drops box drawing and block characters, and arrow keys read as words | tui: TUI-local (1 == 1) · host: TUI |
| 2.34.0 | 19 | [fixed] Mermaid diagrams in chat render correctly in more cases instead of showing raw HTML or failing | tui: TUI-local · host: TUI |
| 2.34.0 | 20 | [fixed] [V3] Deny rules save at the scope you pick, and a call a rule blocks is no longer reported as cancelled | tui: **L** — consent echo now applies to `reject_always` (`Kk`: scope default `session` when a resource is chosen; `kiro:{consent` 1 -> 2) + KAS · kas-core: consent scope (pre-existing echo); status mapping literals unchanged in count; `policyDenial` moved into the tool_call_update builder (`withPolicyDenial`, hunks) — verify live (status `failed` + `_meta.kiro.policyDenial`) |
| 2.34.0 | 21 | [fixed] [V3] `/rewind` to a failed or unfinished turn now forks at the right point | kas-core: KAS-internal (`session.fork.snapshot_id_refused` @0.66.62) |
| 2.34.0 | 22 | [fixed] [V3] Tool approvals in the workflow monitor open on the current call and close with `q` | tui: TUI-local · kas-core: TUI · kas-workflow: TUI-only; no approval-shape change 0.66.58 -> 0.66.62 |
| 2.34.0 | 23 | [fixed] [V3] `kiro-cli acp` no longer shuts down unexpectedly while the agent is still running | kas-core: host (stdin/teardown); KAS `disposeAllSessionsOnce` unchanged in shape · host: host acp supervision (logic) |
| 2.34.0 | 24 | [fixed] Pasted images, and pasted or dictated text in menu and picker filters, now work as expected | tui: TUI-local · host: TUI |
| 2.34.0 | 25 | [fixed] [V3] Switching agents with `/agent` now loads the skills declared by the agent you switch to | kas-core: KAS-internal |
| 2.34.0 | 26 | [fixed] [V3] `ssh` and `ssh-keygen` no longer fail with `No user exists for uid` in the macOS sandbox | kas-core: sandbox / fs internal (macOS sandbox uid resolution); cyril has no sandbox (hand-placed) |
| 2.34.0 | 27 | [fixed] [V3] Deleting files outside the workspace folders no longer fails on Windows | kas-core: sandbox / fs internal |
| 2.34.0 | 28 | [security] [V3] Remote MCP servers over HTTP now follow only same-origin redirects | kas-core: KAS-internal (MCP client) · host: KAS (host rmcp unchanged) |
| 2.34.1 | 1 | [fixed] A queued message with blank lines no longer shows as multiple queued messages | kas-core: TUI · host: TUI |
| 2.34.2 | 1 | [changed] Kiro CLI now offers to switch you to V3 when you start a chat | kas-core: TUI/host · host: **host**: rollout `v3_prompt.treatment_percent` 50 → 100 (no code change) |
| 2.35.0 | 1 | [added] Warning when Kiro CLI runs as root or Administrator, with a global `app.warnOnElevatedPrivileges` sett… | tui: **L T26** (`warnOnElevatedPrivileges` 0 -> 2, `KIRO_ELEVATED_PRIVILEGES`) · host: **host** (`KIRO_ELEVATED_PRIVILEGES` [0,1], key [0,3], "Warning: Kiro CLI is running as root…") |
| 2.35.0 | 2 | [added] `/fullscreen` pins the prompt you are reading to the top row; click it to jump back to the prompt | tui: TUI-local · host: TUI |
| 2.35.0 | 3 | [changed] Original Kiro ghost greets you on startup and stays visible in the conversation | tui: TUI-local · host: TUI |
| 2.35.0 | 4 | [changed] A refused response on Auto is now retried so another model can answer instead of ending the turn | kas-core: `model_fallback_auto_refusal` (flag; backend-resolved until 0.66.91 default true) · host: **host** rollout `model_fallback_auto_refusal` → 100% all + `auth/mod.rs` |
| 2.35.0 | 5 | [changed] [V3] MCP servers and web tools now follow your organization's settings | kas-core: `Sr.mcpEnabled` startup configuration, `mcp_tools.enabled`/`web_tools.enabled` @0.66.70 (Deltas) |
| 2.35.0 | 6 | [fixed] Model fallback no longer hangs when a refused response contains a tool call | kas-core: KAS-internal / TUI · host: host v2 (logic) |
| 2.35.0 | 7 | [fixed] Dictation commands like "scratch that" and other bursts of Backspace now delete text in the chat promp… | tui: TUI-local · host: TUI |
| 2.35.0 | 8 | [fixed] [V3] Resuming a V2 session no longer warns that agent `default` was not found or drops the planner | host: host (`v2_builtin_default_agent.rs`, logic) |
| 2.35.0 | 9 | [fixed] [V3] On Linux, resuming a session after a reboot no longer reports it as open in another terminal | tui: TUI/host session lock; no wire · kas-core: host / KAS session liveness · host: host/KAS session lock (logic) |
| 2.35.0 | 10 | [fixed] `/chat` and `--resume` no longer time out when listing a large session history | tui: host IPC; no wire · kas-core: host/TUI · host: host v2 `persist.rs` (logic) |
| 2.35.0 | 11 | [fixed] [V3] `/mcp` now explains which configured servers your organization's MCP registry hid | tui: TUI render of existing `_kiro/mcp/status` data |
| 2.35.0 | 12 | [fixed] [V3] `kiro-cli mcp list` no longer shows a skipped registry server as active | host: **host** `cli/mcp` (logic) |
| 2.35.0 | 13 | [fixed] [V3] HTTP MCP servers that answer the protocol probe with HTTP 403 now connect instead of failing | kas-core: KAS-internal (`mcp.era.*` @0.66.70) / TUI |
| 2.35.0 | 14 | [fixed] [V3] A refusal with no output right after a tool result now ends the turn instead of looping | kas-core: KAS-internal (graph) |
| 2.35.0 | 15 | [fixed] [V3] `read_file` with an offset past the end of a file no longer reports the file as missing | kas-core: KAS-internal · host: KAS/TUI |
| 2.35.1 | 1 | [changed] Performance improvements | host: host-only release (logic; no literal); KAS and TUI byte-identical to 2.35.0 (hand-placed) |
| 2.35.2 | 1 | [changed] Retiring autocomplete desktop app - gradual rollout | kas-core: host |
| 2.36.0 | 1 | [added] [V3] Approve for me: Kiro approves routine actions, blocks risky ones and asks when unsure; turn it on… | tui: **L T10** (dark: `KIRO_TRUST_CLASSIFIER_MODE=enforce`) · kas-core: trust classifier (0.66.58; Deltas) + TUI `pit` rule · host: rollout `trust_classifier_enforce` → all users |
| 2.36.0 | 2 | [added] [V3] Shell approval rules offer a subcommand tier such as `git stash *` between the exact command and … | tui: **L T12** · kas-core: TUI-composed `match` pattern in the consent echo; KAS accepts any pattern (pre-existing) |
| 2.36.0 | 3 | [added] `kiro-cli agent upgrade` warns when the upgraded rules deny, hide, or ask for something V2 allowed | host: **host** (serde `V3PermissionRule [4]`, `DetectorConfig [9]`, "allowReadOnly let V2 read any file without asking…" [0,1]) |
| 2.36.0 | 4 | [added] [V3] Notice once per session for each agent whose V3 permission rules block what V2 allowed | tui: **L T27** (host IPC `permissionConflicts`) · kas-core: host / TUI · host: TUI (`KIRO_AGENT_UPGRADE_NOTICE` arrives 2.37.0) |
| 2.36.0 | 5 | [added] `acp.agentEngine` setting to choose the engine for `kiro-cli acp` | tui: **not in the TUI** (0 hits); host lane · kas-core: host (0 hits in KAS) · host: **host** [0,5] + `acp_engine.rs` router (the client lists are NOT in the feed) |
| 2.36.0 | 6 | [added] Click the Kiro logo ghost in `/fullscreen` to play the next of five animations | tui: TUI-local (`hop nod shake wink`) · host: TUI |
| 2.36.0 | 7 | [changed] [V3] Clearer questions with progress indicators during `/spec` requirements analysis | tui: TUI render + KAS; no new wire literal · kas-core: TUI / KAS spec |
| 2.36.0 | 8 | [fixed] [V3] Shell tool cards title themselves with the model's description and show the Shell command below i… | tui: TUI-local · host: KAS/TUI |
| 2.36.0 | 9 | [fixed] [V3] Rule rows on the approval prompt keep the file name of a long path instead of cutting it short | tui: TUI-local · kas-core: TUI · host: KAS/TUI |
| 2.36.0 | 10 | [fixed] Mermaid diagrams display titles and diagram-wide notes | tui: TUI-local · host: TUI |
| 2.36.0 | 11 | [fixed] Crash when an agent config lists unavailable MCP tools with non-ASCII names | tui: not located; host/KAS · kas-core: TUI / KAS-internal · host: host/TUI (logic) |
| 2.36.0 | 12 | [fixed] Indented lines in a wrapped tool command no longer render as an empty row in narrow terminals | tui: TUI-local · host: TUI |
| 2.36.0 | 13 | [security] [V3] Update the bundled Node.js runtime to 22.23.4 | kas-core: host runtime (RUN-STATE) · host: **host embedded frame** (124,679,552 → 124,902,584 B) |
| 2.36.0 | 14 | [security] Shell commands that set git config through env vars like `GIT_CONFIG_GLOBAL` now ask for approval | kas-core: shell policy (`GIT_CONFIG_GLOBAL` 0 hits in KAS: either host v2 or a regex table) — not located in KAS · host: **host v2** shell_permission (`GIT_CONFIG_GLOBAL` 0 → 2) |
| 2.36.0 | 15 | [security] Update the bundled Bun runtime to 1.4.4 for the Brotli decoder fix | host: **host embedded frame** (79,500,640 → 107,308,936 B) |
| 2.37.0 | 1 | [added] ACP clients can now sign in from a terminal when `kiro-cli acp` starts logged out | tui: **not in the TUI** (handler unchanged; `kiro-cli login` 16 == 16); host lane · kas-core: host; KAS side = `CredentialAbsentError` (0.66.48) lets the agent start without a token · host: **host** (`acp --login`, `cli/acp_sign_in.rs`, `terminal-auth` pointers) |
| 2.37.0 | 2 | [added] `KIRO_RUNTIME_DIR` sets where Kiro extracts its bundled runtime instead of the home directory | tui: **not in the TUI** (0 hits); host lane · host: **host** (`runtime_dir.rs`, `_ extract-runtime`) |
| 2.37.0 | 3 | [changed] Fullscreen keeps the prompt on the bottom rows, even with a short transcript or after Ctrl+L | tui: TUI-local · host: TUI |
| 2.37.0 | 4 | [changed] The Kiro logo ghost keeps its full color at rest instead of fading after the load animation | tui: TUI-local · host: TUI |
| 2.37.0 | 5 | [changed] On V3, V2-format agents are now upgraded automatically at launch; set `chat.enableAutoAgentUpgrade` … | tui: host (setting 0 hits in the TUI); the TUI part is the `KIRO_AGENT_UPGRADE_NOTICE` banner (T26) · host: **host** `launch/auto_migrate.rs` ("Auto-upgraded agent config to universal format" |
| 2.37.0 | 6 | [changed] [V3] Approval prompts in the main chat show only a multi-line command's first line until `ctrl+o` | tui: TUI-local · kas-core: TUI · host: TUI |
| 2.37.0 | 7 | [changed] [V3] The approval rule page highlights the scope choices, with the `s` hint beside them | tui: TUI-local · host: TUI |
| 2.37.0 | 8 | [changed] Mermaid diagrams preserve node shapes, type colors, and logical group markers | tui: TUI-local · host: TUI |
| 2.37.0 | 9 | [changed] `kiro-cli update` now suggests `brew upgrade --cask kiro-cli`, without `--greedy-auto-updates` | kas-core: TUI / host · host: **host** (`greedy-auto-updates` 2 → 0) |
| 2.37.0 | 10 | [fixed] `--no-interactive` runs that stop on an agent now say `stopping (exit 4)` instead of claiming a fallback | tui: host/TUI non-interactive; no wire · kas-core: host · host: **host** [0,1] |
| 2.37.0 | 11 | [fixed] [V3] `--no-interactive` runs stopped by a V2-format agent name `kiro-cli agent upgrade <dir>` | host: host (logic |
| 2.37.0 | 12 | [fixed] Clicks in fullscreen, including the scroll-to-bottom pill, no longer land one row above the target whe… | tui: TUI-local · host: TUI |
| 2.37.0 | 13 | [fixed] [V3] The entire-tool row on the approval rule page names the tool, such as Shell, instead of one call | tui: TUI-local (`entire tool${…}`) · host: TUI |
| 2.37.0 | 14 | [fixed] Mouse-wheel scrolling in `/fullscreen` works in AWS CloudShell and other terminals running older tmux … | tui: TUI-local · host: TUI |
| 2.37.0 | 15 | [fixed] The Kiro logo ghost stays on the home screen while a command menu such as `/agent` or a panel such as … | tui: TUI-local · host: TUI |
| 2.37.0 | 16 | [fixed] Mermaid flowcharts render nested containers instead of falling back to source | tui: TUI-local · host: TUI |
| 2.37.0 | 17 | [fixed] Mermaid diagrams fit long labels within narrow terminals | tui: TUI-local · host: TUI |
| 2.37.0 | 18 | [security] [V3] `/spec` prompts, review comments and `/spec view` summary rows no longer print escape sequence… | tui: TUI-local · kas-core: TUI · host: TUI |
| 2.37.1 | 1 | [changed] Refreshed look for the welcome screen, startup notices and What's new, without the startup tip | host: TUI |
| 2.37.1 | 2 | [changed] The `/fullscreen` scroll-to-bottom pill has a dark fill, lights up on hover, and its dot turns green… | host: TUI |
| 2.37.1 | 3 | [changed] [V3] An answer cut off at the model's output limit now says so and suggests asking Kiro to continue | kas-core: TUI (stop reason `max_tokens` pre-exists on the wire) · host: KAS/TUI |
| 2.37.1 | 4 | [fixed] [V3] Idle sessions use far less CPU tracking the engine's process tree | kas-core: host · host: **host** launch (logic) |
| 3.0.0 | 1 | [added] Kiro CLI V3 is now the default experience, with workflows, specs, cloud sessions and more | kas-workflow: policy; KAS 0.66.70 unchanged · host: **host**: `chat.agentEngine` persistence + switch prompts (strings since 2.37.0 |
| 3.0.0 | 2 | [removed] The Classic experience is retired; new capabilities are built in the terminal UI | host: **host** "Kiro CLI 3.0 no longer runs Classic (the v1 engine), so this run ignores the Classic selection." (2.37.0 string |
| 3.0.0 | 3 | [removed] Inline shell autocomplete and the CLI desktop app are retired | host: **host packaging**: launcher, term, `q`, `qchat` removed |
| 3.1.0 | 1 | [added] `PreCompact` and `PostCompact` hooks that run before and after a conversation is compacted | kas-core: KAS 0.66.91 (Deltas) |
| 3.1.0 | 2 | [added] `KIRO_DEFAULT_SETTINGS` names a root-owned file of default settings that your own `cli.json` overrides | tui: **L T25** |
| 3.1.0 | 3 | [added] `/usage` and an optional `Session credits` status line segment show the session credits so far | tui: **L T4** (`session_usage`) · kas-core: `session_usage` kind (Deltas) · kas-workflow: kas-core (`session_usage`); same `sumCreditsByUnit` as step credits |
| 3.1.0 | 4 | [added] Workflow step rows show the credits each step used | tui: **L T5** · kas-core: kas-workflow (`credits` on node payloads via `readSessionCredits`/`SessionUsage`) · kas-workflow: 0.66.91 `credits` on node state / `node_complete` / `run_complete` |
| 3.1.0 | 5 | [added] Session recap above the prompt after a few idle seconds; opt in through `/settings` under Features or … | tui: **L T2/T3** — engine-fed · host: **host key** `chat.enableSessionRecap` [0,3] + doc node (behaviour TUI) |
| 3.1.0 | 6 | [changed] Sub-agents can read the memories of the agent that dispatched them | kas-core: `memory.policy.parentResolved` (0.66.70) + 0.66.91 memory rollout |
| 3.1.0 | 7 | [changed] A prompt is refused with a sign-in message, and not saved, when Kiro cannot get a sign-in token | kas-core: `SignInRequiredError` (Deltas) |
| 3.1.0 | 8 | [changed] Agent `file://` resources are capped at 32 files and 100,000 characters; large files are cut or skip… | kas-core: new literal "…was cut to fit the agent's resource limits…" (numbers not extracted as literals) |
| 3.1.0 | 9 | [changed] Notifications queue into one row, most urgent first, with a `+N waiting` count | tui: TUI-local (` waiting` 5 -> 6) |
| 3.1.0 | 10 | [changed] Warnings and errors stay on screen until you dismiss them with a keypress | tui: TUI-local · kas-core: TUI |
| 3.1.0 | 11 | [changed] Startup cloud config, learnings and steering fetches no longer print tool cards; cloud config instea… | tui: TUI-local filter (`fetch_cloud_config` 0 -> 7) · kas-core: hidden-tool set `new Set(["fetch_cloud_config"])` pre-exists (1 = 1); the card suppression for learnings/steering is TUI |
| 3.1.0 | 12 | [changed] `web_fetch` asks sites for markdown, so docs pages keep their code blocks and links | kas-core: Accept header (Deltas) |
| 3.1.0 | 13 | [fixed] MCP tool results that return `structuredContent` alongside text or images now include that data as an … | tui: **not in the TUI** (0 hits); KAS (T29) · kas-core: **no KAS delta** (zero hunks); SDK helper in both |
| 3.1.0 | 14 | [fixed] Global skills now load in every session when `kiro-cli acp` is started from the home directory | kas-core: KAS-internal (skill discovery) |
| 3.1.0 | 15 | [fixed] `/agent swap kiro_default` now switches to the default agent | tui: not located to a TUI literal; KAS agent switch · kas-core: TUI |
| 3.1.0 | 16 | [fixed] A line of a reply occasionally missing or repeated in the chat transcript | tui: TUI-local |
| 3.1.0 | 17 | [fixed] A message sent while a sub-agent runs leaves the queued tray once the sub-agent reads it | tui: TUI-local |
| 3.1.0 | 18 | [fixed] A prompt keeps retrying an unanswered connection for 90 seconds instead of giving up sooner | kas-core: `KIRO_CONVERSE_CONNECTION_TIMEOUT_MS` pre-exists; KAS-internal |
| 3.1.0 | 19 | [fixed] A sub-agent stopped at its turn limit reports its unfinished work instead of an empty result | kas-core: Subagent Response text (Deltas) |
| 3.1.0 | 20 | [fixed] A token-endpoint outage fails the MCP tool call but keeps the server's sign-in | kas-core: `mcp.oauth.token_endpoint.answered` @0.66.91 |
| 3.1.0 | 21 | [fixed] A turn cut off by the output-token cap or the step limit now says it stopped early | tui: not in the TUI (`stopped early` 0); KAS copy · kas-core: stop reasons `max_tokens`/`max_turn_requests` pre-exist; wording is TUI; `[IterationLimit] Iteration limit reached` log · kas-workflow: kas-core |
| 3.1.0 | 22 | [fixed] `/context show` lists a `skill://` file that could not be loaded instead of leaving it out | kas-core: KAS-internal |
| 3.1.0 | 23 | [fixed] Sessions whose transcript grew past 512 MiB can be opened again | tui: TUI-local transcript store / host · kas-core: KAS-internal (persistence) |
| 3.1.0 | 24 | [fixed] Shell commands run in `bash` when it is available and `SHELL` is unset, instead of `/bin/sh` | kas-core: `getDefaultShell` (Deltas) |
| 3.1.0 | 25 | [fixed] Starting a session no longer deletes the logs of another session that is still running | kas-core: KAS-internal (log prune; `logsBasePath` 6 = 6) |
| 3.1.0 | 26 | [fixed] `str_replace` writes `$$`, `$&` and other `$` patterns in the new text as written | kas-core: KAS-internal |
| 3.1.0 | 27 | [fixed] Sub-agents load the `file://` resources their own agent profile declares | kas-core: `subagent.profileFiles.resolve` @0.66.91 |
| 3.1.0 | 28 | [fixed] A prompt sent just as a workflow run finishes is no longer dropped | kas-core: kas-workflow · kas-workflow: 0.66.91 `wakeTargetIsBusy` + `hasUnstoppedPromptInSetup` — WF-L4 |
| 3.1.0 | 29 | [fixed] Spec task runs record property-based test results without asking for approval | kas-core: KAS spec internal (property-based test result recording); no wire field (hand-placed) |
| 3.1.0 | 30 | [fixed] Workflow runs land their changes on your checked-out branch, not a leftover worktree branch | kas-workflow: not located in KAS 0.66.91 workflow code, prompts or recipes, nor in TUI 3.1.0 — WF-L10 |
| 3.1.0 | 31 | [fixed] `kiro-cli agent upgrade` and `/upgrade-agent` now warn when an agent lists V2 tools that V3 doesn't ha… | tui: host IPC; TUI renders the warning (`no-v3-tool` literal added 3.1.0) · kas-core: host · host: **host** ("V3 has no tool for these V2 tools, so V3 ignores them…" [0,1]) |
| 3.1.0 | 32 | [fixed] Tool cards in scrollback no longer show a `ctrl+o to toggle` hint after they stop responding to it | tui: TUI-local (`ctrl+o to toggle` 2 == 2; behavioural) · kas-core: TUI |
| 3.1.0 | 33 | [fixed] An empty model reply after a tool call ends the turn instead of retrying up to 300 times | kas-core: `graphs.empty_end_turn_after_tool_response` @0.66.91 |
| 3.1.0 | 34 | [fixed] MCP and web tools stay on for organization users when a later profile check fails | kas-core: `DiscoveredProfile` / governance (0.66.91) · host: host/KAS (new controlplane call `GetEffectiveEnterpriseManagementResourceWithToken` is the host-side candidate) [INFERENCE] |
| 3.1.0 | 35 | [fixed] MCP sign-in links disappear once their server connects, with the server named in the toast | tui: **L T31** — the TUI's 2.32.0 `_kiro/mcp/oauthCompleted` consumer is now fed: KAS 0.66.91 emits it (`oauthCompleted` 0 -> 2) and passes `serverName`/`oauthCompletion` on `_kiro/openExternalUrl` · kas-core: `_kiro/mcp/oauthCompleted` now sent (Deltas) |
| 3.1.0 | 36 | [fixed] "Run required tasks" on a spec finishes every required task instead of stopping partway | kas-core: KAS spec internal |
| 3.1.0 | 37 | [fixed] Shell commands that read input get end of input at once instead of waiting until they time out | kas-core: agent-hosted `stdio:["ignore",…]` (3 -> 5 sites); cyril nulls stdin already |
| 3.1.0 | 38 | [fixed] Mermaid sequence diagrams render multiline labels, branches, async arrows and `rect` regions correctly | tui: TUI-local |
| 3.1.0 | 39 | [fixed] `kiro-cli agent validate` now exits non-zero when the agent config is invalid or missing | kas-core: host · host: host `cli/agent` (logic) |
| 3.1.0 | 40 | [fixed] A damaged knowledge listing no longer prevents a session or sub-agent from starting | kas-core: `knowledge.listing.failed` @0.66.91 |
| 3.1.0 | 41 | [fixed] Agent requests reach the correct service hosts in isolated AWS regions | kas-core: `@aws-sdk/util-endpoints` + API-key region discovery (`DiscoveredProfile`) · host: host endpoint resolution (logic |
| 3.1.0 | 42 | [fixed] `PreToolUse` hooks that exit 2 block the tool even when they give no reason | kas-core: v2 hooks executor (Deltas) |
| 3.1.0 | 43 | [fixed] Stopping a Windows shell command also stops the child processes it started | kas-core: `taskkill_failed` kind @0.66.91 |
| 3.1.0 | 44 | [fixed] Sub-agent knowledge bases refresh when agent files change instead of serving stale text | kas-core: KAS-internal |
| 3.1.0 | 45 | [fixed] Resizing the terminal no longer blanks the chat or paints rows at the old width | tui: TUI-local |
| 3.1.0 | 46 | [fixed] Scrolled-back chat keeps the line you were reading when the terminal width changes | tui: TUI-local |
| 3.1.0 | 47 | [fixed] The footer's session credit total no longer resets to zero when a session is reopened | tui: **L T4** (`session_usage` with `replay:true`) · kas-core: `session_usage` replay on `session/load` |
| 3.1.0 | 48 | [fixed] `--no-interactive` runs and `kiro-cli acp` no longer print the agent's `[INFO]` startup lines on stderr | kas-core: KAS logger (`[KiroAgent]` info lines) / host; not pinned statically — check cyril's stderr capture live · host: **host** (logic |
| 3.1.0 | 49 | [fixed] `--no-interactive` output starts the text that follows a tool call on a new line | tui: host plain-text printer · kas-core: TUI/host · host: host launch text mode (logic) |
| 3.1.0 | 50 | [fixed] Cloud sessions in an unsupported region show as unavailable instead of failing every turn | tui: **L** — TUI maps the KAS error (`cloud_session_unsupported`, `CodeIntelligenceCloudSessionUnsupportedError` added 3.1.0) · kas-core: cloud |
| 3.1.0 | 51 | [fixed] Reopening a session after a crash during a save no longer drops the next entry written to its history | tui: TUI-local / host transcript · kas-core: KAS-internal (persistence) |
| 3.1.0 | 52 | [fixed] Pages over the 10 MB `web_fetch` limit are reported as too large instead of as a network error | kas-core: `kWe=10*1024*1024` pre-exists; error text change |
| 3.1.0 | 53 | [fixed] Zero-result `web_search` calls show the search provider's message instead of an empty result | kas-core: `parseSearchBody` + `readMessage` (`message:cJo(s)`) @0.66.91 |
| 3.1.0 | 54 | [fixed] Tool output stays on the same screen row when expanded or collapsed in fullscreen chat | tui: TUI-local |
| 3.1.0 | 55 | [security] Tool-use telemetry is exported when an organization turns on only an OpenTelemetry export, not just… | host: **host** telemetry (logic |
| 3.1.0 | 56 | [security] Remote MCP server registrations and credential folders are now owner-only on Unix | kas-core: `configuration.userLocalSettings.write` passes a `mode:` constant on mkdir (`mode:Mi`, minified name; the 0 -> 38 count is a rename artefact and the value was not read) — weak static evidence · host: KAS or host `secret_storage`/ `owner_only_sweep` (no new host literal |
| 3.1.0 | 57 | [security] [V2] `git` commands prefixed with `GIT_DIR` or `GIT_WORK_TREE` now ask for approval | kas-core: v2 host (out of lane) · host: **host v2** (`GIT_DIR` 0 → 2, `GIT_WORK_TREE` 0 → 2) |

## 2. cyril on KAS: parity and regression checks (cyril-c6hv)

KAS has been cyril's default engine since 2026-10-19 (cyril-4c4d closed), so this section replaces the 2.32.0 "flip blockers" with
two tables from the standing v3-coverage lane (`audit-3.1.0/v3-coverage-findings.md`, Matrix 1–9, ~200 rows): the **day-one gaps** a
cyril user on kiro-cli 3.1.0 / KAS 0.66.91 hits without doing anything special, and the **regression checks** for the eight fixes closed
since the 2.32.0 audit. cyril verdicts are at HEAD e3a3868d; live verdicts come from leg 5 (§8).

### 2a. Day-one gaps (ranked)

P1 = live-proven regression or billed/unsolicited work; P2 = a dropped field or missing route on the default engine; P3 = reachable only
through something the user turns on; P4 = cosmetic or dark. "new" = first reaches a cyril user with 0.66.91 / 3.1.0.

| # | P | what the user experiences | cyril entry point | evidence | issue | new? |
|---|---|---|---|---|---|---|
| 1 | **P1** | `/review cancel` ends the run, then KAS's abort wake opens a labelled turn on the main session and the model redoes the review (billed work outside the run's authorization) | `/review cancel` (`commands/workflow.rs:192`) | LIVE 2.26.0; wake trigger set unchanged through 0.66.91 | cyril-oswq | standing |
| 2 | ~~P1~~ **FIXED** | "Always allow" on a workflow step's approval looped 20×, then KAS rejected and the step failed | any `/review` / `/workflow run` approval answered Always-allow | LIVE §8a V3: one-step recipe, step approval answered Always allow (session) from cyril → exactly ONE overlay, step completed, `run_complete status:completed`, marker written; `re-evaluated {effect:allow, persistedEffect:allow}`. The `Permission flow exceeded 20 approval rounds` text still exists (2 = 2) but the path is not reached with cyril's consent echo | cyril-w55l → propose close | **fixed (0.66.91 + cyril-auxt)** |
| 3 | **P2** | the model can change mid-session (fallback on refusal/outage, auto re-send of a refused turn) with no trace; the toolbar keeps the picked model | every prompt | `model_fallback_{refusal,availability,auto_refusal}` default true in 0.66.91 (STATIC); `kas.rs:822` drops `model_fallback`/`model_routed`; `toolbar.rs:63-66`; LIVE KAS-L1 NOT-REACHED (no refusal occurred; `_kiro/configuration/state` now pushes 21 `modelFallback/<model>` connection settings) | cyril-aoby (raised to P2), cyril-ojum | **new** |
| 4 | **P2** | `/usage` "Credits" and the footer total restart at 0 after `/load` and never match Kiro's authoritative `session_usage` total | `/usage`, `/load` | LIVE leg 1 CONFIRMED (`session_usage` after every `turn_completion`, replayed on load; equals the running sum of `promptTurnSummaries`); `kas.rs:822`; §8a V5 sharpened it: cyril's `/usage` "Credits" is the HOME-wide `usage.sqlite3` ledger (`crates/cyril-core/src/usage.rs` `snapshot()`), unchanged by `/new`, `/load` or a fresh process — cyril has no per-session credit figure at all, while KAS pushes one on every turn and on load | cyril-t2w5 (P2) | **new** |
| 5 | **P2** | `/workflow status` rows show no per-step credits; run totals are a parent-metering estimate | `/workflow run`, `/review` (gate-off) | LIVE leg 2a CONFIRMED (`credits[]` on `node_complete`/`run_complete`/snapshot nodes; node credits == the step session's `session_usage`, run = sum); `convert/kas/workflow.rs:207-218,259-283,444-520` | cyril-bgvj (P2) | **new** |
| 6 | **P2** | the sub-agent's answer is appended to the parent's reply as one message (0.66.91 streams child text on the parent session tagged only by `_meta.kiro.agentSubtaskId`; the orphan "Subagent Response" is gone, so the cyril-1v25 path has nothing to render) | any prompt that spawns a sub-agent | LIVE-H CHANGED (§8) and §8a S5 on screen: one `Kiro:` message reading `ALPHAThe helper sub-agent read PROBE.txt…` (child chunk + parent chunks, no separator), the child's `Read File` row unlabelled, no "Subagent Response"; `agentSubtaskId` 0 hits in `crates/`; `convert/kas.rs:835` → `convert/mod.rs:506` | cyril-lpdk (P2), extends cyril-ebqu | **new** |
| 7 | P2 | auto-compaction is invisible (`summarization_*`), throttling/model `display_error`s are lost, `summarizationThreshold` not shown | long sessions | `kas.rs:628-651,822`; `kas.rs:698-760` | cyril-58uv, cyril-6s21 | standing (+1 field) |
| 8 | P2 | no `/context`, no `/compact` | typing them | no `_kiro/session/{context,compact}` route (Matrix 3) | cyril-1aq0 | standing |
| 9 | P2 | `chat.enableThinking:false` in cli.json is ignored at session start; three dead keys logged `unknown-key` per connection | every connection | LIVE leg 0 CONFIRMED (`configuration.intake.unknown-key` for `thinking` on both engines); `kas/settings.rs:77,80,91-96` | cyril-53qx | standing |
| 10 | P2 | default output style only (5 styles exist) | every prompt | no `_meta.kiro.outputStyle` (`domain_mediator/commands/session.rs:514-521`) | cyril-j66p | standing |
| 11 | P2 | org cloud config never syncs on cyril's spawn (`CLOUD_CONFIG_ENDPOINT` absent) | every spawn (org users) | host env table; `discovery.rs:363-369` | cyril-0b8t | standing |
| 12 | P2 | SIGTERM/SIGHUP can orphan the KAS node child | closing the terminal | LIVE 2.32.0 | cyril-qdcs | standing |
| 13 | P2 | grants recorded for other sessions (steps, sub-agents) are not remembered | workflow approvals in bursts | `always_allow.rs` has no cross-session store | cyril-7c0s | standing |
| 14 | P2 | with `chat.enableWorkflows=true`, the user's Kiro "Workflow use" choice is ignored (engine default `when-asked`) and a `/load` of a pre-0.66.58 session delegates everything (`legacy`) | `chat.enableWorkflows` + `/load` | LIVE WF-L2 CONFIRMED (option offered gate-on, round trip echoes `currentValue`; an OLD-created session loaded by NEW runs as delegate-all with no option); `kas/settings.rs:108-116`; `kas_config.rs:20-48`; leg 5 S3: `/mode` has no such row | cyril-asrc (P2; ADR-0013 decision 1: follow the user's Kiro setting) | **new** |
| 15 | P3 | KAS native memory stays off on cyril's spawn unless the user opts in; with `settings.memory` it works on both engines and 0.66.91 adds `_kiro/memory/changed` + list `total/counts`; `userMemoryOptIn:false` is a hard off-switch; cyril sends neither key | user agent file / handshake | LIVE KAS-L5: a bare spawn gets `memoryConfig {mode:"disabled"}` on 0.66.91 — the 0.66.58 rollout flip did NOT change the ACP default; `kas/settings.rs:76-86` | cyril-q8gq, cyril-0na7, cyril-s21o | standing (flip did not reach ACP clients) |
| 16 | P3 | a signed-out session shows "prompt failed: Sign in to Kiro to send a prompt." and stays usable, with no `kiro-cli login` hint or re-auth action | token loss mid-session | LIVE KAS-L3 CONFIRMED (`SignInRequiredError` -32000 when the auth callback errors, nothing persisted; an empty/null token → `ModelRegistryUnauthenticatedError` on both engines); `domain_mediator/commands/session.rs:540-548,577-586`; `state.rs:1193-1196` | cyril-0rn8 | **new (better than the 2.32.0 hang)** |
| 17 | P3 | an MCP tool that elicits input is declined (visibly) instead of asked | MCP tools that elicit | `kas/mcp.rs:50-55,70-82`; LIVE-B CONFIRMED on both engines; §8a S6 on screen: `{action:"decline"}` in 3 ms, a system message quoting the question, tool result `ELICIT_RESULT:{"action": "decline"}`, turn ends normally (no stuck turn) | cyril-0yme (P3) | standing |
| 18 | P3 | no session recap | a setting Kiro has and cyril lacks | `kas/settings.rs` has no `sessionRecap`; `kas.rs:822` drops `recap`; LIVE TUI-L1: the gate works and the `recap` kind exists on 0.66.43 too (3.1.0 added only the TUI setting) | cyril-5anx (P3) | opt-in, not a 3.1.0 delta |
| 19 | P3 | `KIRO_RUNTIME_DIR` / `_ extract-runtime` installs are invisible; the remedy text points the wrong way | packaged / runtime-dir installs | `discovery.rs:446-449,50-53`; LIVE HOST-L2: `extract-runtime` defaults to `<bin>/../lib/kiro-cli/<ver>-<hash>/`; orchestrator probe (§8): a plain `kiro-cli acp` with `KIRO_RUNTIME_DIR` unset still extracts to `<data dir>/kas/3.1.0-<sha>/` — default installs unaffected | cyril-8ts2 (P3) | new (opt-in) |
| 20 | P3 | autopilot cannot be turned off; sandbox cannot be enabled | `/mode` picker has no such rows | `kas_config.rs:20-48` | vh69 follow-up, cyril-6vo6 | standing |
| 21 | P3 | no `/permissions`-style listing of what Always-allow persisted; `_kiro/policy/changed` unlogged | after an Always-allow (user/workspace) | Matrix 3; `convert/kiro.rs:1305` | cyril-r5pl | standing |
| 22 | P3 | `/spawn` `/kill` `/msg` error, `/sessions` is empty | typing them | `commands/mod.rs:568-571`; KAS 0 hits for `_session/spawn` (`/steer` is NOT v2-only: KAS implements `_session/steer`) | cyril-a8se | standing |
| 23 | P3 | a missing/broken custom agent falls back silently | `/mode` to a broken agent | `_kiro/customAgent/*` dropped (`kiro.rs:1305`) | cyril-7xxk | standing |
| 24 | P3/P4 | the remote-MCP OAuth notice is not cleared by `_kiro/mcp/oauthCompleted` (only by the next `_kiro/mcp/status`) | remote MCP with OAuth | `engine.rs:143`; `callbacks.rs:190-193`; `kiro.rs:1305`; LIVE NOT-REACHED (no OAuth fixture) | cyril-1nto (P3) | **new (KAS now sends it)** |
| 25 | P4 | Always-allow offers exact + `<cmd> *` but not Kiro's `<cmd> <sub> *` tier | Always-allow scope page | `always_allow.rs:163-180`; LIVE TUI-L11: the tier is a client-composed glob, accepted identically by 0.66.43 and 0.66.91; `permissions.yaml` shape unchanged | cyril-0ax4 (P4) | standing |
| 26 | P4 | `KIRO_DEFAULT_SETTINGS` (managed defaults) not consulted; replay marking / `agentInitiated` replay rendered as typed; turn identity fields and `rateMultiplier` dropped; Approve-for-me dark | — | host; cyril-99ds; cyril-lxuo; tui T10/T11 | cyril-f4hi (P4), cyril-zfoo (P3, Approve for me) | new / standing |
| 27 | **P2** | cyril loads and runs a freshly cloned repo's `.kiro/` hooks, agents, MCP servers and workflow recipes without asking — the same posture as Kiro's `--trust-workspace` on every launch. Since 2.33.0 Kiro's own TUI asks "trust this folder?" first; the gate is TUI-local (`wgt()`), KAS is started with `workspaceTrusted:!0` hard-wired and never asks, so there is no wire to consume: a cyril gate would have to sit before `session/new`, like the TUI's | opening cyril in any workspace | tui T16; kas-core; LIVE-A (§8) and §8a S8 on screen: the workspace agent appears in `/mode`, the workspace MCP server connects (`/mcp` labels it `layer: workspace`), and a workspace hook executes on the host with no trust text and no permission request | **cyril-mq15 re-scoped P2 (owner decision 2026-11-08: cyril adds its own trust prompt before `session/new`)** | new (Kiro added the prompt; cyril's behaviour is unchanged) |

Checked, no row: `structuredContent` (no KAS delta); `_meta.kiro.preview` removal (cyril never read it); MCP elicitation flag (advisory);
`PreCompact`/`PostCompact` (host-mode hooks never see them; kas-mode gets `hook_update`); unparseable-command auto-deny (renders as a
denied call); shell default `/bin/bash` when `SHELL` unset (agent-hosted only; cyril hosts the terminal).

**Not the default path, but breaks on upgrade (host §5, cyril-k3hf P2):** the V2 wrapper spawn `cyril --agent-engine v2` sends a bare `kiro-cli acp`;
LIVE HOST-L1: a 3.1.0 host routes the unlisted name `cyril` to **v3** (node child spawned, `_meta.kiro` on initialize, 34 `extensionMethods`) despite
`--help`'s "v2 when neither is set"; `--agent-engine v2` or `acp.agentEngine:"v2"` restore v2; snapshot names such as `acp-probe` stay on v2. cyril's
fingerprint then fails with a remedy that names the same command (`fingerprint.rs:92-105`).

### 2b. Regression checks for the fixes closed since 2.32.0

| fix | what cyril does now | what in 0.66.91 / 3.1.0 could break it | leg 5 verdict |
|---|---|---|---|
| **cyril-k8wc** `shell_type` basename | `terminal_io.rs:283-285` → `host_shell.rs:216-222` `wire_name()`: `bash|zsh|sh|dash|ksh`, `fish`, `pwsh`, `powershell`; default host shell `$SHELL` else `bash` | 0.66.91's classifier `xp()` accepts every value cyril can send; the `posix`→`unknown` mapper is unchanged (`case"posix"` 1 = 1); the 3.1.0 `getDefaultShell` change is agent-hosted only | **PASS** (§8a S4: cyril answered `shellType:"fish"` — the user's login shell — KAS resolved `shellParser:"fish"`, no `unknown`, allow rules matched). Consequence: the 2.34.0 "unparseable commands auto-denied" protection is a bash/posix-parser property — under fish `echo "oops` was asked, persisted and executed (exit 127). Note on k8wc/auxt |
| **cyril-zbu7** token renewal | argv `kas/auth.rs:44-46,559-563`; envelope `:859-860`; fields `:895-914`; 30 s deadline; single-flight `:449-460`; stored fallback only on `HelperMissing` | argv/envelope byte-identical in all 14 host binaries and the TUI handler; risks: the 3.1.0 helper migrates `data.sqlite3` one-way (HOST-L4); `cps.auth_provider.unknown {provider:"none"}` when the reply lacks `provider` (LIVE leg 0) | **PASS** (S1: the first `getAccessToken` served from `chat _ get-kas-token`, `force_refresh:false`; LIVE-F: the 3.1.0 binary's envelope `{kind:"getKasToken",data:{accessToken,expiresAt,profileArn,provider}}` fits `auth.rs:896-914`) |
| **cyril-auxt** consent echo | `convert/kas.rs:128-190`: `capability`/`workspaceRoot` from the request, `scope` from `always_allow.rs:33-47`, `resource` from `:64-96`; only for an offered `allow_always`; no agent-JSON write on KAS | KAS intake unchanged (`scope||"session"`); the 2.36.0 subcommand tier is accepted but never composed; on 0.66.91 an unparseable command is auto-denied with no ask at all, so the `persistableConsent:false` scope-page worry is moot | **PASS end to end** (§8a S4): session scope suppresses the re-ask, `/new` re-asks, user scope writes `permissions.yaml` exactly as specified and a fresh session runs unasked; cyril offers no `git stash *` tier (cyril-0ax4) and 0.66.91 adds a fourth option `reject_always` ("Always deny") that cyril lists |
| **cyril-1v25** orphan `tool_call_update` | `cyril-ui/src/state.rs:555-560` tracks an update whose id never started as a new row | 0.66.91 no longer sends the orphan at all: the child's answer streams on the parent session tagged `agentSubtaskId` (LIVE-H) | **CHANGED** — the orphan path still renders "Tool Load" rows (§8a S6: two per session) but "Subagent Response" no longer exists on 0.66.91 (cyril-lpdk) |
| **cyril-oieu** command dispatch | `convert/kas.rs:493-548`: `_meta.kiro.type` ∈ `custom-agent|skill|prompt` → prompt text; `commands/mod.rs:654` | KAS 0.66.91 advertises the same three types; a new type value would be suppressed with a log | **PASS** (S2: `/probe-skill hello` ran as prompt text; `✓ Loaded skill`; one P3 display nit — blank approval preview for the skill-load tool call, cyril-smy8) |
| **cyril-vh69** `/model` `/effort` `/mode` | `commands/mod.rs:528-534`; `types/kas_config.rs:20-48,207-234,258-275`; one `SetConfigOption` per confirm; toolbar `toolbar.rs:54-82` | model option `_meta.kiro` keys unchanged (LIVE leg 1); `effortLevel`/`thinking` accepted-not-echoed on both engines; a `workflowLaunch` select arrives gate-on as an unsettable generic option | **PASS** (S3: full model catalog, `claude-sonnet-5` set, `/effort` low → badge `◇ low`, `/mode` seven rows; V4: badge updates without a BridgeError) |
| **cyril-d7ve** `KIRO_HOME` resolver | `kiro_agent_config.rs:94-152`; `kas/settings.rs:63-65` reads `<KIRO_HOME>/settings/cli.json` | host `KIRO_HOME` readers 7 → 9 (secret_storage.rs, 2.34.0) with the same semantics; 3.1.0 adds `KIRO_DEFAULT_SETTINGS` below cli.json (cyril reads cli.json only, cyril-f4hi) | **PASS** (§8a S7: absolute `KIRO_HOME` honoured by cyril and KAS — `kh-agent` listed, logs/sessions under it; relative value ignored by both with cyril's WARN). New: the relative value leaks through the environment to the `kiro-cli chat _ get-kas-token` helper, which creates `<cwd>/kh/settings/cli.json` — note on d7ve |
| **cyril-6lql** repeat-cap wake label | `workflow.rs:468-497,638-650,693-697`; header `turn_labels.rs:23-47` | 0.66.91 `wakeTargetIsBusy` queues completions as steering when the parent has a prompt in setup; `repeatCapNote` unchanged; LIVE leg 2a: the pause wake is unchanged on NEW; LIVE WF-L4: a prompt sent at `node_complete` runs at once on BOTH engines (the OLD "dropped prompt" prediction was wrong) | **PASS** (§8a V3: `─── ⚙ workflow "one-step" completed · agent follow-up ───` header and the footer `run 0.23 credits · follow-up 0.09 credits` rendered; the repeat-cap variant itself was not re-run) |

**Handshake drift one row deeper (Matrix 1):** cyril still sends `thinking`, `todoList`, `subagentOrchestration` (`kas/settings.rs:77,80,91-96`;
0.66.91 logs `configuration.intake.unknown-key`, LIVE leg 0) and does not send the TUI's `sessionRecap`, `memory`/`userMemoryOptIn` (load-bearing
since the memory rollout flipped), `c2s`, `largeToolOutputHandler`, nor the host-capability flags `telemetryEnabled`, `knowledge`,
`requirementsAnalysis`, `specPhaseCheckpoints`, `streamingShellContent`, `diagrams`, `mcpElicitation`, `userInput`, `secretStorage`, `authCallback`.
**New cyril surface since 2.32.0 closes two FOLLOW-UP rows:** `/mcp` (KAS builtin + panel; cyril-nk4o, cyril-tqr8 closed) and `/answer` (cyril-9104).
**Matrix 8 (v2-only code with nothing to talk to on 3.x, low):** `convert/kiro.rs` (`_kiro.dev/*`, still shipped by 3.1.0 until the January removal),
`--agent-engine v2` + `[agent] engine = "v2"`, the bare wrapper spawn (`bridge.rs:394`), V2 trust writes (`app.rs:2027-2040`), `/spawn /kill /msg /sessions`.

## 3. KAS 0.66.43 → 0.66.91

Five engine deltas (0.66.48 / 0.66.58 / 0.66.62 / 0.66.70 / 0.66.91). Set diffs (static, `audit-3.1.0/kas-core/`): `_kiro/*`
methods **123 → 138** (none removed), `session_info_update` kinds **25 → 29** (+`turn_timing` 0.66.58, `suggested_prompts` 0.66.62,
`network_configuration` and `session_usage` 0.66.91), feature flags **30 → 40**, AgentSettings zod keys **49 → 61**, `KIRO_*` literals
41 → 52, ACP methods 25 = 25, `sessionUpdate` variants 8 = 8. 0.66.91 is built with esbuild keepNames (`a(fn,"name")` after every
function), so raw literal counts inflate without behaviour change (method note §10). Every row below is STATIC unless a LIVE column says
otherwise; the cyril verdict is at HEAD e3a3868d.

| item | first ver (KAS) | gate / default | LIVE | cyril | issue |
|---|---|---|---|---|---|
| **Model fallback default ON**: registry defaults `model_fallback_refusal` false→true, `model_fallback_availability` false→true, `model_fallback_auto_refusal` (added 0.66.58) false→true; `_kiro/configuration/set` can write a per-model fallback chain | 3.1.0 (0.66.91) | on; `KIRO_FEATURE_MODEL_FALLBACK_{REFUSAL,AVAILABILITY,AUTO_REFUSAL}_ENABLED=false` | NOT-REACHED live (no refusal in the paired sweep); the default flip is STATIC from the flags registry | DROPPED: kinds `model_fallback` / `model_routed` fall through `convert/kas.rs:627` → `_ => None` at `kas.rs:822`; a cyril user's model can change mid-session with no UI trace | cyril-aoby (raised to P2) |
| **`session_usage` kind** `{kind:"session_usage", credits:[{usage,unit?,unitPlural?}]}` = `sumCreditsByUnit` over every `promptTurnSummaries`; sent after each `turn_completion`; rebuilt from `usage_summary` transcript rows and replayed on `session/load` | 3.1.0 (0.66.91) | always | **CONFIRMED** (`kas-live-sweep-3.1.0.jsonl#53`, replay `#388`; equals the running sum of turn summaries) | DROPPED (`kas.rs:822`); cyril sums turns itself (`session.rs:291`, `types/session.rs:627-662`) so a reopened session restarts at zero — the defect Kiro's 3.1.0 line fixes | cyril-t2w5 (P2) |
| **Sign-in refusal**: `refuseWhileSignedOut()` before persistence on every client prompt; with the acp-callback credential absent → JSON-RPC `-32000` `"Sign in to Kiro to send a prompt."` `data:{errorType:"SignInRequiredError", retryErrorType:"CLIENT_ERROR"}`, prompt not persisted. Identity tracker (`auth.identity.*`) and `CredentialAbsentError` since 0.66.48 | 3.1.0 (0.66.91); tracker 2.33.0 | always | **CONFIRMED** (error-reply arm; empty/null token → `ModelRegistryUnauthenticatedError` on both and the prompt IS saved) | generic `Notification::BridgeError` (`types/event.rs:332`, `cyril-ui/src/state.rs:1193`); no sign-in-specific mapping | NOTE cyril-0rn8 |
| **Trust classifier ("Approve for me")**: env `KIRO_TRUST_CLASSIFIER_MODE=enforce|off` (`resolveClassifierMode`, default off); rule effect `evaluate` added to the agent-file schema (`allow|deny|ask|evaluate`); off ⇒ evaluate rules filtered (`policy.reference.classifier_off`). Wire: `tool_call._meta.kiro.trustClassifierApproved:true`, a pending `tool_call_update` "flag line" with `_meta.kiro.trustClassifier{decision:"prompt",reason}`, `request_permission._meta.kiro.trustClassifier`. Kiro's TUI writes `{capability:"all",effect:"evaluate"}` into `permissions.yaml|json` | 2.33.1 (0.66.58); surfaced 2.36.0 | **dark**: env `enforce` on both sides; the `trust_classifier_enforce` rollout gate (2.34.0) was removed in 3.1.0 | **CONFIRMED** on NEW with `KIRO_TRUST_CLASSIFIER_MODE=enforce` + `effect:evaluate`: `trustClassifierApproved:true`, a second `permission-classifier` model call (billed) and NO `request_permission`; the `decision:"prompt"` branch NOT-REACHED; on OLD an `evaluate` rule is fatal (every tool denied) | NOT-SENT (cyril's spawn sets no env, `kas/discovery.rs`) and DROPPED (`convert/kas.rs:67-91` reads only `consent`/`rejectionReason`) | cyril-zfoo (P3, opt-in) |
| **Unparseable shell command** (2.34.0 "denied automatically"): `PolicySession.unparseable()` → `{effect:"ask"}` when no shell rule denies/asks, else `{effect:"deny", basis:{kind:"unparseable"}}`; `persistableConsent:false` + reason unchanged | 2.34.0 (0.66.62) | always | **CONFIRMED**: NEW asks nothing (`tool_call_update status:failed` "Tool call denied: Kiro could not parse…"); OLD showed a 2-option ask with `persistableConsent:false` | HANDLED as a failed tool call; `policyDenial` read via serde rename | NOTE cyril-58uv |
| Permission option builder unchanged (`accept`, `always-accept`, `reject`, `always-reject`); 2.34.0 option ORDER and the 2.36.0 **subcommand tier** `"git stash *"` are TUI-composed `consent.resource` patterns on the echo | — | — | **CONFIRMED** the tier is client-composed: `git stash *` consent works identically on OLD and NEW; user scope persists to `permissions.yaml` on both | cyril echoes capability/scope/resource/workspaceRoot (`convert/kas.rs:150-191`) but derives exact + base pattern only (`always_allow.rs:624-643`) | cyril-0ax4 (P4) |
| **Hooks `PreCompact`/`PostCompact`** (trigger table + camelCase aliases; env `compaction_id, reason, message_count, transcript_path` (+`summary`, `summary_path`); a blocked Pre hook aborts compaction, `hooks.compaction.blocked`; Post output → `agent_note`). The ACP `_kiro/hooks/list` trigger enum is unchanged — compaction never reaches host-mode hooks | 3.1.0 (0.66.91) | the v2 hook runtime needs `initialize.clientCapabilities._meta.kiro.hooks:{enabled:true, v2:true}` (without `v2` KAS logs `v2Hooks is disabled`) | **CONFIRMED** with the gate: `_kiro/hooks/didChange`, standalone `.kiro/hooks/*.json`, PreCompact + PostCompact fire on `_kiro/session/compact`, a KAS-executed PreToolUse exit 2 blocks | cyril advertises `hooks` without `v2` (`kas/hooks.rs:383`) → NOT-SENT: PreCompact/PostCompact never run for a cyril session | cyril-zmji (P3), NOTE cyril-497j |
| `PreToolUse` exit 2 with no reason blocks; `executeHook` reply shape unchanged | 3.1.0 | — | **CHANGED**: a CLIENT-executed hook (`_kiro/hooks/executeHook`, cyril's shape) that exits 2 with empty output does NOT block on either engine; only the KAS-executed path blocks silently | cyril replies `{exitCode, output}` (`hooks.rs:777-781`) — must carry a reason to block | cyril-zmji (P3) |
| **`_kiro/mcp/oauthCompleted {serverName, success, oauthFlowId}` now SENT**; `_kiro/openExternalUrl` params gain `oauthFlowId` beside `serverName`, `oauthCompletion` | 3.1.0 (0.66.91); TUI consumed it dark since 2.32.0 | `clientCapabilities._meta.kiro.openExternalUrl:true` | NOT-REACHED (no OAuth MCP fixture) | REACHABLE and DROPPED: cyril advertises `openExternalUrl` (`engine.rs:143`), reads only `url` (`kas/callbacks.rs:189-195`), notification falls to `convert/kiro.rs:1305` | cyril-1nto (P3) |
| **Memory rollout GA**: `memory_internal_enabled` `"disabled"`→`"all"`, `memory_external_enabled` false→true; policy still follows the agent file's `memory.mode` (default `disabled`), opt-out `settings.userMemoryOptIn:false`; sub-agents index the dispatcher's memory (0.66.70; 3.1.0 line). `_kiro/memory/{list,get,create,update,delete}` pre-exist | 2.33.1 (0.66.58) | per-agent `memory.mode` | **CONFIRMED** the ACP default is still OFF on 0.66.91 (`memoryConfig {mode:"disabled"}`, kiro.log `memoryOff`); with `settings.memory read_write` both engines store memories; `userMemoryOptIn:false` is a hard off ("Memory is not enabled."); `_kiro/memory/create` requires `memoryType, scopes, title, summary` (the covenant's bare `{content}` is wrong) | NOT-SENT (`settings.rs:77-97` marshals no `memory`/`userMemoryOptIn`); a user's agent file with `memory.mode` now takes effect under cyril | NOTE cyril-s21o, cyril-0na7 |
| **Memory sync**: `_kiro/memory/getSyncStatus {}` + `_kiro/memory/syncStatus` push (0.66.70), `_kiro/memory/sync {}` + `_kiro/memory/changed {total,counts}` (0.66.91); state union `notStarted|resolving|off|unavailable{reason}|suspended|syncing|synced|failed`; flags `memory_sync_enabled` false; BFF endpoint from `kiro-session-config` | 2.35.0 / 3.1.0 | sync off | **CONFIRMED** `_kiro/memory/changed {total, counts}` on a write; `getSyncStatus` → `{state:"off"}`; `_kiro/memory/syncStatus {state:"notStarted"→"unavailable",reason:"notSignedIn"}` pushed on connect | DROPPED (`kiro.rs:1305`) / NOT-SENT | NOTE cyril-s21o |
| **`turn_timing` kind** `{executionId, turnId, ttft:{modelRequestSentAt, firstTokenReceivedAt, agentOutput?}}`, opt-in via `session/new|load._meta.kiro.automaticTurnProfiling:{ttft?,perf?,contextBreakdown?}`; `perf` defers `turn_completion`+`session_usage` and adds `throughput` | 2.33.1 (0.66.58) | opt-in | **CONFIRMED** with the prompt-level opt-in: live data rides the last chunk (`firstTokenReceivedAt`…); `kind:"turn_timing"` is REPLAY-only on `session/load`; the session-level key NOT-REACHED | NOT-SENT (`domain_mediator/commands/session.rs:475` builds `session/new` without `_meta`) | — |
| **`suggested_prompts` kind** `{prompts[]}` after a top-level turn when hidden `_suggestedPrompts` is on (`KIRO_DISABLE_SUGGESTED_PROMPTS`) | 2.34.0 (0.66.62) | hidden, off | — | NOT-SENT / DROPPED | — |
| **Session recap**: catalog `sessionRecap` (hidden, off) / AgentSettings `_sessionRecap`; Kiro's TUI sends `settings.sessionRecap:{enabled}` from `chat.enableSessionRecap`; KAS runs `startRecap()` at turn end and pushes `kind:"recap" {text}` (emitter pre-exists in 0.66.43) | gate flipped in the TUI 3.1.0 | off ("one model call per turn") | **CONFIRMED** gate; **the `recap` kind exists on 0.66.43 too** — not a 3.1.0 engine delta | NOT-SENT (`settings.rs:77-95`) / DROPPED (`kas.rs:822`) | cyril-5anx (P3) |
| **`network_configuration` kind** + `_kiro/environment/get_network_configuration` (cloud downlink) | 3.1.0 | cloud sessions | — | n/a (no cloud sessions in cyril) | — |
| **Jobs protocol** `_kiro/job/update|output|cancel`, AgentSettings `universalBackgroundTool`, flag `universal_background_tool` (false); advertised only when the flag resolves true | 3.1.0 | flag off | — | NOT-SENT / DROPPED | NOTE cyril-7p7n |
| **Code-intelligence builds** `_kiro/codeIntelligence/{startBuild,cancelBuild}` + notification `indexStatus`; initialize `_meta.kiro.codeIndexAnalysisScopes`; gated by hidden `_c2s`; `tree-sitter-cpp` + `cpp` in the parser map (0.66.62); `KIRO_ARCHIFY_CLI_PATH` removed | 3.1.0 | `_c2s` off | **CHANGED**: `startBuild`/`cancelBuild` → `CodeIntelligenceNotEnabledError` without `settings.codeIntelligence`; `indexStatus` not advertised | NOT-SENT / DROPPED | NOTE cyril-1aq0 |
| **`_kiro/configuration/set {krn, value, layer}`** → `{setting}`; layers `user-synced` / `user-local` (3.1.0); errors `notGranted|locked|unavailable`; catalog rows 41 → 44 (+`_suggestedPrompts`, `sendMessageDeliveryLimit`, `sessionToolsMessaging`) | 2.34.0 (0.66.62) | — | **CONFIRMED** shape (`krn`, `layer` required); client writes to `tangentMode`/`thinking` at `user-local` REFUSED `notGranted` (`ConfigurationSetRefusedError`) | NOT-SENT | NOTE cyril-iq2c |
| **`_kiro/diagram/render {sessionId, diagramId, format: mermaid|svg}`**, initialize `diagramRenderFormats`, `elkjs` layout (0.66.62) | 2.33.0 (0.66.48) | `clientCapabilities._meta.kiro.diagrams` (off) | **CONFIRMED**: `_kiro/diagram/render` + `diagramRenderFormats ["mermaid","svg"]` only with the flag | NOT-SENT (`engine.rs:137-161`) | NOTE cyril-68ny |
| **Auto-compaction threshold**: AgentSettings `compaction.autoCompactionThreshold` (0..80, default 80), per-prompt `_meta.kiro.autoCompactionThreshold`, initialize `_meta.kiro.autoCompactionThreshold:{min,max,default}` | 2.33.1 (0.66.58) | optional | **CONFIRMED** (leg 0; `summarizationThreshold` also on `context_usage`) | NOT-SENT (`settings.rs:158-172`); initialize `_meta.kiro` not read (`fingerprint.rs:31`) | NOTE cyril-6s21 |
| `session/prompt._meta.kiro.displayText` now read (persisted/replayed user rows show it) and `_meta.kiro.agentInitiated` (+reason) marks agent-initiated prompts | 2.33.1 / 3.1.0 | optional | **CHANGED**: `session/load` replays `displayText` as the user chunk on BOTH engines (0.66.43 already did; the 0.66.58 attribution is wrong) | NOT-SENT / replayed chunk HANDLED generically | NOTE cyril-hzkb |
| `session/new._meta.kiro.shellEnvironment` per-folder env overlay; `session/set_config_option._meta.kiro.mcpServers` (client-provided servers, 0.66.62) | 3.1.0 / 2.34.0 | optional | **CHANGED**: `shellEnvironment` is applied by KAS (`session.shell_environment.applied`) but NOT forwarded to a client-hosted `terminal/create` env on either engine | NOT-SENT | NOTE cyril-7p7n |
| **Session tools** AgentSettings `sessionTools{awareness,messaging,cloud}` (triples → `sessionToolsAwareness|Messaging|Cloud` catalog rows), `sendMessage.deliveryLimit` (1..500, default 50); peer messaging requires workspace trust + messaging on; `_kiro/session/notify` sender `peer` (§4) | 3.1.0 | off | NOT-REACHED: `settings.sessionTools` is a known-but-unread key on NEW; no `send_message` tool was offered | NOT-SENT | NOTE cyril-53qx, cyril-fb1m |
| Agent-hosted shell: `getDefaultShell` → `SHELL || /bin/bash || /bin/sh`; stdin `ignore`; `_kiro/terminal/shell_type` and the `posix`→`unknown` mapper unchanged | 3.1.0 | agent-hosted only | — | n/a: cyril hosts the terminal and names the shell (cyril-k8wc closed) | — |
| Sub-agent turn limit wrap-up text rides the Subagent Response ("stopped at its turn limit… Its work is NOT done"); `web_fetch` Accept `text/markdown…` with 406/415 fallback; agent `file://` caps (32 files / 100k chars) with "was cut to fit" marker; empty reply after a tool call ends the turn | 3.1.0 | — | — | HANDLED as content (cyril-1v25 closed) / n/a | NOTE cyril-ebqu |
| MCP: elicitation call site identical 0.66.43 = 0.66.91 (`_kiro/mcp/elicitation {sessionId, toolCallId, elicitation{mode: form|url,…}}`, called unconditionally — the 2.34.0 line is the TUI gaining the responder); `structuredContent` extra JSON block: **no KAS delta** (SDK helper in both bundles); governance `mcpEnabled` startup value (0.66.70) and `DiscoveredProfile` keep-on (3.1.0); optional power servers no longer block turns; subagents lose inline MCP (0.66.62) | — / 2.35.0 / 3.1.0 | — | **CONFIRMED** elicitation on both engines; `structuredContent` extra block **NOT REPRODUCED** (text block only on both) | elicitation HANDLED with a visible decline (`kas/callbacks.rs:597`, `kas/mcp.rs`); `governance/state` DROPPED | NOTE cyril-0o7e |
| Initialize result `_meta.kiro`: +`autoCompactionThreshold` (0.66.58), +`diagramRenderFormats`, +`codeIndexAnalysisScopes`; `extensionMethods` += `get_network_configuration`, `configuration/set`, memory `getSyncStatus`/`sync`, conditional `job/*`, `diagram/render`, `codeIntelligence/*`; `authMethods` unchanged | 2.33.1 / 3.1.0 | — | **CONFIRMED** (leg 0: initialize diff per carve; 32 `extensionMethods` on a direct spawn, 34 through the host) | DROPPED (`fingerprint.rs:31` reads `agentCapabilities` for identity only) | — |
| Handshake intake: `clientCapabilities._meta.kiro` resolver 17 = 17 keys; cyril still sends `thinking`, `todoList`, `subagentOrchestration` → 3× `configuration.intake.unknown-key` | — | — | — | `settings.rs:77-97` | NOTE cyril-53qx |
| Workspace trust: `workspaceTrusted:!0` hard-coded at KAS's stdio and ws entry points; no CLI arg, env or handshake key; the 2.33.0 prompt and 2.34.0 `--trust-workspace` are host/TUI-side | — | — | **CONFIRMED** (LIVE-A): no trust frame on either engine; workspace agents, MCP servers, recipes and steering load with no answer | unaffected on the wire; cyril adds its own gate (cyril-mq15, P2, owner decision) | cyril-mq15 |
| Packaging: `ssh2` declared external with zero textual import (its optional deps are the new `asn1`/`bcrypt-pbkdf`/`cpu-features`/`buildcheck`); `@aws-sdk/util-endpoints` + `DiscoveredProfile` (isolated-region routing); Node compile cache (`<tmp>/kiro-agent-compile-cache-<uid>`, honours `NODE_COMPILE_CACHE`); `ACP_WS_HOST`; cedar-wasm 4.14.0 | 2.34.0 / 3.1.0 | — | — | n/a | — |

| `_meta.kiro.preview{file,modifiedContent,originalContent,…}` on `fs_write` tool calls REMOVED | 3.1.0 | — | **CONFIRMED** removal (leg 2a OLD vs NEW) | never read; cyril's diff view uses ACP `content[].diff` (`convert/mod.rs:207`) — the `preview.*` allowlist line is now dead | — |
| `_kiro/policy/changed` gains `errors[]`; an unsupported rule effect is skipped with a warning instead of failing the whole policy (OLD: `status:"failed"` + `_kiro/policy/error` and every tool denied) | 3.1.0 | — | **CONFIRMED** | DROPPED (`kiro.rs:1305`): a bad `permissions.yaml` silently denies every tool on 0.66.43 | NOTE cyril-r5pl |
| `_kiro/configuration/state` now pushes 21 `modelFallback/<model>` connection-layer settings at `session/new` (with `configurationState:true`) | ≤3.1.0 | opt-in | **CONFIRMED** (0 on OLD) | DROPPED (cyril does not advertise `configurationState`) | NOTE cyril-aoby, cyril-iq2c |
| Turn frames gain `executionId`, `turnId`, `timestamp`, `boundarySource:"observed"`; `turn_start` gains `userMessageTag` | 2.33.1 | — | **CONFIRMED** | DROPPED (allowlisted as turn identity, §9) | — |

**Flags** (`audit-3.1.0/kas-core/flags/`): added 0.66.58 `model_fallback_auto_refusal`, `memory_quality_enabled`, `memory_sync_enabled`;
0.66.70 `consolidate_disclosures`; 0.66.91 `memory_quality_{internal,external}_enabled`, `memory_sync_{internal,external}_enabled`,
`universal_background_tool`, `create_response_api_auxiliary`. Defaults changed: 0.66.58 memory rollout on; 0.66.91 the three model-fallback
flags on and `memory_quality_enabled` on. Removed: none. Env-reachable flags 15 → 20.

**Standing coverage (0.66.91 emissions vs cyril):** `session_info_update` kinds HANDLED 9/29 (`display_error`, `hook_update`,
`turn_completion`, `turn_start`, `turn_end`, `context_usage`, `steering_queued|injected|cleared`; `convert/kas.rs:627-760`); DROPPED 20
at `kas.rs:822`, among them `model_fallback`, `model_routed`, `session_usage`, `recap`, `turn_timing`, `interaction_resolved`,
`pending_interaction`, `summarization_*`, `user_message_id_assigned`. `_kiro/*` notifications HANDLED: `error/rate_limit`,
`system/notify`, `mcp/status`, `mcp/governance_disabled`, `powers/items_changed`, `session/notify`, `hooks/didChange|cancel`; DROPPED
silently at `convert/kiro.rs:1305`: `mcp/oauthCompleted` (reachable), `policy/changed`, `codeIntelligence/indexStatus`, `job/update`,
`memory/changed|syncStatus`, `customAgent/*` (cyril-7xxk), `governance/state`, `sessions/changed`, `spec/*`, `tools/*`, `knowledge/*`,
`safety/*`, `sandbox/status`, `steering/documents_changed`, `progressive_context/items_changed`, `c2s/view/changed`, `code_references`.

## 4. Workflows (v3)

Static (`audit-3.1.0/kas-workflow/`): **no `_kiro/workflow/*` method or lifecycle kind added or removed** across the five deltas
(14 requests: `new, invoke, load, inspect, list, pause, cancel, delete, listRecipes, listWatchHandlers, resumeAll, resume, retry,
update`; 9 event kinds + `recipes_changed` + `steps_queued`). Request schemas unchanged (`new` still takes `workflowPath|workflow,
inputs, parentSessionId, workspacePaths?, runLabel?, modelId?, effortLevel?` — no `workflowPrompt` on the client method). The seven
bundled recipes are byte-identical in all six carves; the recipe zod did not change (the 2.26.0 JSON schema stays valid; its missing
`include` node is the pre-existing cyril-inwg gap). Behaviour moved through fields and settings:

| item | first ver (KAS) | gate | LIVE | cyril | issue |
|---|---|---|---|---|---|
| **Step credits**: node state `credits:[{usage,unit?,unitPlural?}]`; `node_complete` carries the step's `credits`; every `run_complete` carries `credits` summed by unit (`runCompleteCreditFields`); filled from the step session's `session_usage`, also on fail/abort | 3.1.0 (0.66.91) | always (gate-off too) | **CONFIRMED** (`kas-workflow-gateoff-3.1.0.jsonl#193`; node credits == step `session_usage`, run = sum) | DROPPED: `WireNodeSnapshot` `convert/kas/workflow.rs:444-520`, `WireNodeCompleted` :259-283, `WireRunCompleted` :207-218; run totals still derived from parent metering (`workflow.rs:487,1319`; `cyril-ui/src/turn_labels.rs:108-121`) | cyril-bgvj (P2) |
| **`workflowLaunch` session config option** `{type:"select", id:"workflowLaunch", name:"Workflow use", options: when-asked "When asked" / large-tasks "Large tasks only" / delegate-all "Delegate all work"}`, default `when-asked`, offered only while `settings.workflows.enabled`; drives a `# Workflow launch setting` system-prompt block; a session recorded before 0.66.58 loads as **legacy = delegate-all**. Kiro's TUI restores `chat.workflowLaunch` via `set_config_option` after `session/new` | 2.33.1 (0.66.58); TUI `/settings` row 2.34.0 | workflows on | **CONFIRMED** incl. the legacy case: an OLD-created session loaded by NEW runs as delegate-all with no option offered | DROPPED on receipt (`KasConfigId::from_wire` knows model/mode/effortLevel, `types/kas_config.rs:20-48`); NOT-SENT (cyril reads only `chat.enableWorkflows`, `settings.rs:108-116,140-145`) | cyril-asrc (P2, ADR-0013 parity) |
| `# Workflow Orchestration` prompt rewritten: "always delegate" (0.66.43) → launch-setting rules (0.66.58) → step 3 "give the user's full task or its absolute path… never carry the task text as a launch input" (0.66.91) | 2.33.1 / 3.1.0 | on | — | n/a | NOTE cyril-xehm |
| **Plan feedback**: a rejected plan approval's `_meta.kiro.rejectionReason` is persisted on `interaction_resolved`, collected into run state `appliedPlanFeedback[{text,creatorSessionId,toolCallId,consentRound?,topLevelStepIds[]}]`, fed to the creator's revision and injected as `<approved_plan_feedback>` into affected steps. Approval shape (title `Add N to <name>`, Text content, `_meta.kiro.workflowPlan`) unchanged; typed suggestions (2.34.0) are a TUI UI over this field | 2.33.1 (0.66.58) | plan approvals | **CONFIRMED**: a revised plan follows `rejectionReason` on BOTH engines; `appliedPlanFeedback[]` NEW only; `<approved_plan_feedback>` never on the wire | HANDLED: cyril sends `rejectionReason` on reject (`convert/kas.rs:205-224`); `appliedPlanFeedback` on `inspect` DROPPED (`WireSnapshot` :414-443) | NOTE cyril-4u4a |
| `_kiro/workflow/list` rows gain `stepsCompleted`, `stepsTotal`, `currentNodeId?`, `waitingOnWatch` for running/paused runs | 2.33.0 (0.66.48) | always | **CONFIRMED** (live rows only; absent after completion) | DROPPED (`WireRunListEntry` :588-618; `/workflow list` prints started_at only) | NOTE cyril-4u4a |
| fileCheck `jsonPath` grammar: evaluator strips a leading `$` (so `$.verdict` resolves; 0.66.43 looked up key `$`); load-time `WorkflowFileCheckJsonPathUnsupportedError` for `[ ] * ..` — the probable cause of 2.33.0 "review loops stop once the reviewer approves" [INFERENCE] | 2.33.0 (0.66.48) | always | **CONFIRMED**: NEW rejects `a[0]` at `_kiro/workflow/new` ("unsupported '['") and `$.done` stops after one iteration; OLD accepts `a[0]` and runs both to the cap | error text reaches cyril generically | NOTE cyril-inwg |
| `github-pr` watch: `mergeable`, `mergeStateStatus`, review-thread ids, base-tip GraphQL, conflict re-wake; `wf-pr-responder` merges base on conflict and resolves threads | 2.33.0 (0.66.48) | always | — | n/a (payload rides `{{id.output}}`) | NOTE cyril-inwg |
| Restart continuity: `classifyInterruptedNodes` finishes/fails paused steps whose completion signal was persisted before shutdown and emits one `node_complete` per classified node before continuing | 2.34.0 (0.66.62) | always | **CONFIRMED** on both engines (SIGKILL mid-step, respawn, `load`/`inspect`/`resume`); the creator-run "without replanning" half NOT-REACHED | HANDLED (tracker applies `node_complete` in any order, `workflow.rs:874`) | — |
| Auto-wake: `wakePromptMeta` adds client profiling opt-ins to wake prompts (0.66.58); **`wakeTargetIsBusy`** (prompt in setup or persistence in flight ⇒ queue the completion as steering instead of racing a wake) = 3.1.0 "prompt not dropped"; trigger set unchanged (step info/warning/error, `run_complete`, repeat cap) | 2.33.1 / 3.1.0 | always | WF-L4 **REFUTED for OLD / same on both**: the prompt at `node_complete` runs at once and the completion arrives as `steering_injected`, 3/3 rounds each; WF-L7 NOT-REACHED (no truncated wake reply reproduced) | HANDLED by the busy-parent steering path (cyril-lki9 B5, `turn_labels.rs:59-87`) | — |
| `_kiro/session/notify` sender enum `["parent","step"]` → `+ "peer"`, + `notifyId`, `deliveryCount`; `send_message` tool gains `sessionIds[]` (max 16), outcomes `delivered|not_authorized|not_found|budget_exceeded|suppressed|cancelled|failed`, per-session budget | 3.1.0 (0.66.91) | `settings.sessionTools.messaging.enabled` (off) | NOT-REACHED (key known-but-unread on NEW; `send_message` not offered) | `peer` ⇒ whole message DROPPED with a warning (`convert/kas/notify.rs:73-77`); gate NOT-SENT | NOTE cyril-fb1m |
| Bundled agents: `wf-pr-responder` (0.66.48); `wf-coder`, `wf-planner`, `wf-design-reviewer`, `wf-workflow-creator` rewrite incl. `include` docs and validation list (0.66.70, unannounced); `wf-coder` blocker → warning, `wf-planner` one approach, `wf-code-reviewer` "no unasked behaviour" (0.66.91); `Explore` c2s_* → `code_index_*` | 2.33.0 / 2.35.0 / 3.1.0 | — | — | n/a | NOTE cyril-inwg |
| Dark: cloud-sandbox step environment ("Kiro Web cloud session's internal sandbox… Brazil packages on GitFarm"); `workflows.creatorEffort` accepted/echoed since 0.66.58, sent by no client; `workflowNotifications` / `setWorkflowNotificationDelivery` still TUI-only (absent from KAS) | — | — | — | — | — |

**ADR-0013 drift check:** the `workflow-from-prompt` recipe (`create-workflow` step, agent `wf-workflow-creator`, input
`workflow_brief`) and the creator brief wrapper are byte-identical 0.66.43 → 0.66.91 — nothing for cyril's pending `/workflow new`
(cyril-0asq) to re-copy. The TUI's gate marshal lost its rollout `available` half (`workflows`/`goal` now always sent from
`chat.enableWorkflows`, default false); cyril's `settings.rs:140-145` matches. One new drift row: `chat.workflowLaunch` →
`set_config_option workflowLaunch` (above). The step-session `allow_always` loop (cyril-w55l) is structurally unchanged
(`Permission flow exceeded 20 approval rounds` 2 = 2). Not located statically: 2.33.0 "replies no longer cut off" (WF-L7),
3.1.0 "changes land on your checked-out branch" (no worktree/branch logic in KAS or TUI 3.1.0; WF-L10).

## 5. Host: the 3.x single binary, Classic retirement, launcher

Static (`audit-3.1.0/host/`), with `--help`-class runs of the archived binaries under temp `HOME`/`XDG_DATA_HOME` only.

- **The 3.x "merge" is a deletion.** 3.0.0 `kirocli/bin/kiro-cli` (219,494,120 B) is the 2.37.1 `kiro-cli-chat` (219,500,200 B) rebuilt and
  renamed: identical third-party crate set (281 crates), byte-identical `.rodata`, **0** source paths added vs the union of the three 2.37.1
  binaries, and all 145 launcher-only (`q_cli`, `fig_*`, `zbus`, `dbus`) and 71 term-only (`figterm`, `alacritty_terminal`) source files gone. The
  chat binary had absorbed the launcher's user-facing commands at 2.34.0 (clap `bin_name` → `kiro-cli`, `--agent`, telemetry/uninstall/update
  cutover) and 2.35.0 (`integrations`). Removed command paths: `internal` (25 fig/shell helpers), `setup`, `debug`, `theme`, `launch`/`quit`/
  `restart`, `translate`, `dashboard`, `inline`, `completion`, `hook`; `doctor` is a stub; `--classic` survives as an alias of `--legacy-ui`. The
  top-level `_` namespace is `cleanup-autocomplete-artifacts`, `standalone-cohort`, `standalone-cutover`, `extract-runtime` (2.37.0); `chat _`
  (`get-kas-token` et al., 11 helpers) is unchanged.
- **"Classic retired" means the v1 engine / legacy UI, not the V2 ACP engine.** Host string (2.37.0): "Kiro CLI 3.0 no longer runs Classic (the v1
  engine), so this run ignores the Classic selection." 3.1.0 still ships `crates/chat-cli-v2/src` (523 paths), every `_kiro.dev/*` literal
  (count-identical), and `kiro-cli acp --agent-engine v2 --help` exits 0 on 3.0.0 and 3.1.0. Kiro's own prompt dates its removal: "V2 no longer
  receives new features and will be removed in January" (`launch/v3_launch_prompt.rs`, 2.37.0). The V2→V3 switch flow persists `chat.agentEngine=v3`;
  the `v3_prompt` rollout reached 100% at 2.34.2 ("offers to switch you to V3"); `v3_switch.rs` replaced `v3_ease_in.rs` in 3.1.0.
- **Standing P1 check (cyril-zbu7): PASS, static.** `kiro-cli chat _ get-kas-token [--reason] [--current-expires-at] [--force-refresh]` help is
  byte-identical 2.32.0 → 3.1.0 except the clap `bin_name` in the Usage line; `get-kas-token`/`getKasToken`/`force-refresh`/`_kiro/auth/getAccessToken`/
  `accessToken`/`expiresAt`/`profileArn`/`authMethod`/`kas_token.rs` counts are identical in all 14 host binaries; the 3.1.0 TUI handler is 2.32.0's
  byte-for-byte. Live verdict: §8 (LIVE-F).
- **The ACP engine router (2.36.0, `crates/chat-cli/src/cli/acp_engine.rs`) — the cyril-relevant surprise.** `kiro-cli acp` now peeks the ACP
  `initialize` request ("failed to read the ACP initialize request", "ACP initialize peek task failed", "resolved kiro-cli acp engine") and routes
  by `clientInfo.name` against an embedded telemetry snapshot of **1,080 client names that stay on v2** (frozen since 2.36.0) plus an exempt list
  (6 names in 2.36.0, **empty since 2.37.1**). `cyril` is in neither list. `acp --agent-engine <v1|v2|v3>` (`classic`/`kas` refused by clap)
  overrides the new `acp.agentEngine` setting ("'v2' (default) or 'v3'" in its description), but the router's comment assumes a v3 acp default, 3.0.0
  announces V3 as the default experience, and Kiro's own TUI started pinning `--agent-engine=v2` on its v2 spawn in the same release. **cyril's V2
  wrapper spawn is a bare `kiro-cli acp`** (`bridge.rs:394`, default argv `crates/cyril/src/main.rs:32-36`): if the 3.x default resolves to v3 for an
  unlisted client, cyril's V2Engine receives KAS and `fingerprint.rs:92-96` fails loud — with a remedy ("spawn a v2 agent (`kiro-cli acp`)") that
  names the command that produced the mismatch. The free path (direct `node … acp-server.js`) and the KAS wrapper (`kas/version.rs:343-364` appends
  `--agent-engine v3`) never meet the router. Live verdict on the default: §8 (HOST-L1); fix shape in §9.
- **How `kiro-cli acp` reaches KAS is unchanged**: `--transport=stdio` (`cli/mod.rs:1413` "spawning KAS process"); env `KIRO_CUSTOM_USER_AGENT`,
  `KIRO_TELEMETRY_IS_INTERNAL_AMAZON`, `KIRO_CLI_IS_CHILD_PROCESS`, `KIRO_REMOTE_SESSIONS_ENDPOINT`, `CLOUD_CONFIG_ENDPOINT`; overrides
  `KIRO_KAS_SERVER_PATH` / `KIRO_KAS_NODE_PATH`; `--auth-method cli|client` (default `cli`). 3.1.0 adds a log-dir ownership check before handing KAS
  the log path and "--login is not supported with --auth-method=client".
- **ACP sign-in from a terminal (2.37.0) is a wrapper-path feature:** `acp --login` ("Log in interactively and exit, for ACP clients that run terminal
  authentication"), new `cli/acp_sign_in.rs`, JSON pointers `/clientCapabilities/_meta/terminal-auth` and `/capabilities/auth/terminal`, the ACP
  `AuthMethod::Terminal{id,name,description,args,env,_meta}` type (`unstable_auth_methods`). Reading [STATIC, INFERENCE]: a logged-out `kiro-cli acp`
  whose client declared `_meta["terminal-auth"]` advertises a terminal auth method whose `args` point at `acp --login`. cyril advertises no
  `terminal-auth` and never reads `authMethods` → DROPPED; the free path (callback auth) is unaffected. Live shape: §8 (HOST-L3).
- **Runtime extraction (2.37.0) is opt-in:** `KIRO_RUNTIME_DIR` / `kiro-cli _ extract-runtime --dir` ("Defaults to KIRO_RUNTIME_DIR, else
  <prefix>/lib/kiro-cli beside this binary") install node/bun/tui.js/KAS into a shared root and prune other versions; the default data-dir layout
  (`kas/<ver>-<sha>`, sqlite `extracted_kas_versions` / `pinned_bin_versions`) is unchanged. cyril's discovery (`kas/discovery.rs:197-200`) never
  reads `KIRO_RUNTIME_DIR` — fine by default, blind to the opt-in (P3, §9). The 2.23.1+ stale-engine cleanup still applies to the shared data dir.
- **`KIRO_DEFAULT_SETTINGS` (3.1.0):** "a root-owned, read-only file of default settings that your own cli.json and workspace settings override"
  (`KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK`; `chat-cli-v2/src/database/default_settings.rs`; absolute path, regular file, root-owned, not
  group/other-writable, ≤1 MiB). cyril's cli.json reader (`kas/settings.rs:65`) does not consult it (P4, §9).
- **Rollout registry 23 → 26 → 15 rows.** `v3_prompt` 10%/75%i → 25%/100%i (2.33.1) → 50% (2.34.0) → **100% (2.34.2)**; `standalone_toolbox`
  0 → 100% (2.35.2, "retiring autocomplete desktop app"); `standalone_public` 0 → 100% (2.37.0); `trust_classifier_enforce` added 2.34.0 → 100%
  (2.36.0, "Approve for me") → removed 3.1.0 (env-only now); `model_fallback_auto_refusal` added 2.34.0 → all (2.35.0). 3.1.0 drops `rollout.rs`
  and 12 graduated rows (`v3_prompt`, `tui`, `voice`, `workflows`, `local_sandbox`, `remote_sandbox`, `session_dashboard`, `tangent`,
  `remote_changelog`, `trust_classifier_enforce`, `standalone_*`) and adds one dark row, **`memory_cloud_sync` 0% internal/nightly**.
- **Env, settings, docs.** `KIRO_*` +`KIRO_RUNTIME_DIR`, `KIRO_LAUNCH_3_0`, `KIRO_AGENT_UPGRADE_NOTICE` (2.37.0), `KIRO_ELEVATED_PRIVILEGES`,
  `KIRO_ROUTER_VERBOSE/VERSION` (2.35.0), `KIRO_TRUST_CLASSIFIER_MODE`, `KIRO_TEST_SYSTEM_ROOT` (2.34.0), `KIRO_DEFAULT_SETTINGS(_SKIP_OWNER_CHECK)`,
  `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED` (3.1.0); −`KIRO_STANDALONE_CUTOVER/NOTICE`, `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED`,
  `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`, `KIRO_ROLLOUT_FORCE_NIGHTLY` (3.1.0); the merge dropped 109 launcher/term names (`Q_*`,
  `KIRO_CLI_AUTOSUGGEST_*`). Settings 124 → 129 (+`standalone.autoEnroll` 2.33.1 −3.1.0, +`chat.modelPicker.hiddenModels`, `chat.workflowLaunch`,
  `install.releaseUrl` 2.34.0, +`app.warnOnElevatedPrivileges` 2.35.0, +`acp.agentEngine` 2.36.0, +`chat.launch30` 2.37.0,
  +`chat.enableSessionRecap` 3.1.0). Doc manifests regenerated for the first time since 2.32.0 (103 → 105, 140 → 142): +`features/default-settings.md`,
  +`features/terminal-program-status.md`, +`settings/session-recap.md` — all announced; no unannounced `category:feature` node (trap 12).
- **Packaging:** one binary + `install.sh` (installs `~/.local/bin/kiro-cli`, edits PATH in rc files, removes legacy shell integration with backups,
  `--global` → `/usr/local/bin`, refuses a root install into another user's HOME, glibc ≥2.34 gnu / ≥2.26 musl) + README; `.deb` now published;
  no `.tar.xz` for 3.x. Both install locations are in cyril's resolver fallbacks (cyril-igrx/d7ve closed).
- v2-only host deltas (demoted, trap 24): `[V2]` `GIT_DIR`/`GIT_WORK_TREE`/`GIT_CONFIG_GLOBAL` env-prefixed git commands now ask for approval
  (host `shell_permission`; cyril only observes permission requests); `agent upgrade` warns about V2 tools V3 lacks (`todo_list`); `--no-interactive`
  stops on a V2-format agent with `stopping (exit 4)`.

## 6. What Kiro's own TUI now sends and consumes

Static (`audit-3.1.0/tui/`): ten distinct bundles diffed; `_kiro/*` literals the TUI calls or handles **64 → 77** (+13, −0),
`kiro.dev/*` 38 = 38, slash commands 74 → 75 (+`/permissions`), rollout gates 10 → 7, hidden `chat _` subcommands 11 = 11
(`get-kas-token` among them).

- **Handshake.** `initialize.clientCapabilities._meta.kiro` gains `...Vn("c2s")&&{diagrams:!0}` (2.33.1; dark unless the host
  rollout lists `c2s`) and the host-capability flag `mcpElicitation` (`_kiro/mcp/elicitation`, 2.34.0; omitted under
  `KIRO_DISABLE_MCP_ELICITATION=1`). `.settings` gains **`sessionRecap:{enabled}`** (3.1.0, from `chat.enableSessionRecap`, default
  false, forced false in `--no-interactive`); `workflows`/`goal` are now always sent (the rollout `available` check is gone). The
  settings table still has no `todoList`/`subagentOrchestration` (cyril sends both — cyril-53qx). The 7 parsed `agentCapabilities._meta.kiro`
  keys are unchanged; `extensionMethods` now gates the TUI's `_kiro/memory/*`, `_kiro/configuration/*`, `_kiro/modelFallback/*` calls.
  cyril (`engine.rs:120-163`) sends `settings`, `openExternalUrl`, `hooks` only — NOT-SENT: `telemetryEnabled`, `knowledge`,
  `requirementsAnalysis`, `specPhaseCheckpoints`, `streamingShellContent`, `diagrams`, `mcpElicitation`, `userInput`, `secretStorage`,
  `authCallback`, `settings.sessionRecap`, `settings.memory`, `settings.c2s`, `settings.largeToolOutputHandler`.
- **Per-request `_meta`.** `session/prompt`: `displayText`, `outputStyle` unchanged (cyril sends neither — cyril-j66p, cyril-hzkb).
  Permission response `{kiro:{consent:{capability?, scope, resource?, workspaceRoot?}, rejectionReason?}}` unchanged in shape; new
  VALUES: `scope` defaults to `session` also for a `reject_always` with a resource; `resource` may be an exact command, the **2.36.0
  subcommand tier `"<cmd> <sub> *"`** (argv1 matches `^[A-Za-z][A-Za-z0-9_-]*$`, argv0 ∉ sudo/doas/env), `"<cmd> *"`, `"<dir>/**"` or `"*"`.
  The 2.34.0 option ORDER (`allow_once, reject_once, allow_always, reject_always`) is a TUI sort. Typed plan suggestions = `reject_once`
  + `rejectionReason`. `set_config_option` ids: `autopilot, effortLevel, mcpSandboxing, mode, model, sandbox, sandboxNetworkMode,
  thinking` + variables `workflowLaunch` (2.34.0), `memoryReflection`; cyril sets `model, thinking, mode, effortLevel`
  (`commands/mod.rs:1596,2023,2327,2690`).
- **Thirteen new client routes, none removed:** 2.33.0 `_kiro/modelFallback/{get,set}` (the KAS `/model fallback` path — **dark: KAS
  0.66.48–0.66.91 has no such method**, only `modelFallbackReporting:true`); 2.33.1 `_kiro/diagram/render`; 2.34.0
  `_kiro/mcp/elicitation` (handled), `_kiro/memory/delete`, `_kiro/permissions/list` → `{rules:[{capability,effect,scope,source,match?,
  exclude?}]}` re-fetched on the newly consumed notification `_kiro/policy/changed`; 3.1.0 `_kiro/codeIntelligence/{startBuild,
  cancelBuild,indexStatus}`, `_kiro/configuration/{getState,set}` and `_kiro/memory/getSyncStatus` (memory cloud sync, gate
  `memory_cloud_sync`). The 2.32.0 OAuth-completion handler is no longer dark (§3).
- **Kinds consumed.** +`session_usage` → footer `Credits: turn X • session Y` with `sessionCreditsReported` taking precedence over
  the local `turn_summary` sum, and the new status segment `Session credits`; +`recap` → recap panel (primary session only);
  `interaction_resolved` now reads `consentRound` + `replay`; `content`/`tool_call` `_meta.kiro.replay===true` → source `replay`
  (2.34.1). Inbound classifier meta `request_permission._meta.kiro.trustClassifier{decision,reason}` → flag line;
  `tool_call._meta.kiro.trustClassifierApproved` → "approved by Kiro".
- **Approve for me** = `/permissions policies approve-for-me` writing `{capability:"all",effect:"evaluate"}` into the workspace or
  user `permissions.yaml|json`, then `_kiro/permissions/list`; gated `agentEngine==="kas" && KIRO_TRUST_CLASSIFIER_MODE==="enforce"`
  on the TUI as on KAS — dark. Status-line segment ids `approveForMe` (2.34.0), `sessionCredits` (3.1.0).
- **Workspace trust is TUI-local.** `wgt()` gates startup (`agentEngine!=="kas"` ⇒ skip; `--no-interactive` ⇒ skip; `--trust-workspace`
  ⇒ skip for one launch); the decision persists in a TUI-owned file (`workspaceTrusted`, `workspaceTrustCase`); decline exits "Not
  trusted: … Kiro exited without loading this folder." No frame reaches KAS. `/trust` does not exist.
- **KAS spawn and auth byte-identical after renames:** argv `[…,"--experimental-wasm-modules",server,"--transport=stdio",
  "--auth=acp-callback",…policy/endpoint/region args]`, env `NODE_USE_SYSTEM_CA`, `KIRO_CUSTOM_USER_AGENT`,
  `KIRO_OWNED_KAS_{PARENT,CONTROL}`, `clientInfo.name:"kiro-cli"`, Windows supervisor `chat _ windows-kas-process|stop`. The
  `_kiro/auth/getAccessToken` responder runs `["chat","_","get-kas-token"(,"--force-refresh")]` and parses
  `{kind:"getKasToken",data:{accessToken,expiresAt,profileArn,authMethod?,provider?}}` / `{kind:"error"}` — **identical to cyril's**
  (`kas/auth.rs:44-46,557-560,859,872-907`; cyril-zbu7 closed). The 2.37.0 terminal sign-in and the 3.1.0 sign-in refusal have no TUI
  hunk (host/KAS side). `KIRO_RUNTIME_DIR`, `acp.agentEngine`: 0 TUI hits (host lane).
- **V2 in the TUI 3.1.0:** `--tui`, `--v3`, `--v2`, `--legacy-ui`, `--classic` are accepted-and-ignored (`type:"skip"`, since 2.37.1 =
  3.0.0); `--agent-engine <v>` is still parsed and forwarded; `==="v2"` survives only in session-list/telemetry classification. The
  2.34.2 "offers to switch you to V3" bundle is byte-identical to 2.34.1 — that offer lives in the Rust host.
- **Rollout gates 10 → 7:** removed in 3.1.0 `local_sandbox`, `remote_sandbox`, `session_dashboard`, `workflows` (sandbox, `/sessions`,
  `/goal`, `/workflow*`, `/autonomous`, `/tangent`, `/voice` lost their `feature:` gate — GA in the TUI) and `trust_classifier_enforce`
  (added 2.34.0); added `memory_cloud_sync` (3.1.0); `c2s` call sites 2 → 14. `KIRO_*` env in the TUI 95 → 103 (+`KIRO_DISABLE_OBSERVER_RESUME`,
  `KIRO_DISABLE_MCP_ELICITATION`, `KIRO_TRUST_CLASSIFIER_MODE`, `KIRO_ELEVATED_PRIVILEGES`, `KIRO_AGENT_UPGRADE_NOTICE`, `KIRO_LAUNCH_3_0`,
  `KIRO_DEFAULT_SETTINGS(_SKIP_OWNER_CHECK)`; −`KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`).
- **Slash commands:** +`/permissions` (policies: ask / approve for me); `/model` +`edit` (TUI-local hidden models) and `/model fallback`
  (dark on KAS); `/code` +`summary` and `/code init` drives `_kiro/codeIntelligence/startBuild`; `/memories` + delete + cloud-sync
  toggle; `/usage` + Session credits + enterprise overage columns (2.34.0, from the existing `_kiro/account/getUsage` reply);
  `/settings` Features + "Workflow use" + "Session recap". Host IPC `chat _ upgrade-agent --action conflicts` (2.36.0) feeds the
  per-session "V3 rules block what V2 allowed" notice. TUI-only chrome unchanged: OSC 7501, `chat.terminalTitle`, `chat.allowIcons`,
  output-style picker, memory controls (cyril-j9r4, cyril-j66p, cyril-0na7 unchanged).

## 7. Unannounced or dark features

| candidate | gate | default | evidence (lane) |
|---|---|---|---|
| Trust classifier / "Approve for me" wire marks | env `KIRO_TRUST_CLASSIFIER_MODE=enforce` on TUI and KAS + rule `effect:"evaluate"` | off | kas-core, tui T10/T11 |
| `/model fallback` on KAS (`_kiro/modelFallback/{get,set}`) | `extensionMethods` must list it — KAS 0.66.48–0.66.91 never does | dark (TUI ahead of KAS) | tui T6 |
| Background jobs (`_kiro/job/*`, `universalBackgroundTool`; TUI maps rollout `background_execution` → the setting) | flag `universal_background_tool` | false | kas-core, tui |
| Code-index builds (`_kiro/codeIntelligence/{startBuild,cancelBuild,indexStatus}`), diagrams (`diagrams:true`, `_kiro/diagram/render`, IR viewer) | hidden `_c2s` / rollout `c2s` | off | kas-core, tui T1/T7/T19 |
| Session tools: peer messaging / awareness / cloud sessions (`sessionTools.*`, `sender:"peer"`, `sessionIds[]`, delivery budget) | `settings.sessionTools.*` | off | kas-core, kas-workflow |
| Memory sync to the BFF (`_kiro/memory/{sync,getSyncStatus}`, `syncStatus`, `changed`; `_kiro/configuration/set krn:::setting/memorySync`) | flag `memory_sync_enabled`; TUI rollout `memory_cloud_sync` | off | kas-core, tui T18 |
| Memory quality gate | `memory_quality_enabled` | **true** (0.66.91) | kas-core |
| `turn_timing` profiling | `session/new|load._meta.kiro.automaticTurnProfiling.{ttft,perf,contextBreakdown}` | off | kas-core |
| Suggested prompts (`kind:"suggested_prompts"`) | hidden `_suggestedPrompts`, `KIRO_DISABLE_SUGGESTED_PROMPTS` | off | kas-core |
| Session recap (`kind:"recap"`) | `chat.enableSessionRecap` → `settings.sessionRecap` | off | kas-core, tui T2/T3 |
| `workflows.creatorEffort` / `workflowsCreatorEffort` echo | accepted since 0.66.58 | sent by no client | kas-workflow |
| Cloud-sandbox workflow step environment (Kiro Web, Brazil/GitFarm, CRUX) | AWS-internal | — | kas-workflow |
| Network configuration / SSH session (cloud), `_kiro/environment/get_network_configuration` | remote-sessions endpoint | unset | kas-core |
| `consolidate_disclosures`, `create_response_api_auxiliary` | flags | false / disabled | kas-core |
| Infrastructure safety (`infraSafetyMonitor|Enforce`) | `KIRO_INFRA_SAFETY_ROLLOUT_ENABLED=1` | off | tui (unchanged) |
| `KIRO_DEFAULT_SETTINGS` root-owned defaults file (`_SKIP_OWNER_CHECK`) | env | unset | tui T25, host |
| `_kiro/mcp/oauthCompleted` — **no longer dark**: KAS 0.66.91 sends it when the client advertised `openExternalUrl` | `openExternalUrl:true` | live | kas-core, tui T31 |
| `workflowNotifications` / `_kiro/session/setWorkflowNotificationDelivery` | TUI-only; absent from every KAS carve | dead | kas-workflow |

## 8. Live results

Live lane (`audit-3.1.0/live-findings.md`, 900 lines, 407 frame citations checked by `check-citations.py`): KAS 0.66.43 (2.32.0 carve on node
v22.22.2) vs 0.66.91 (3.1.0 carve on its own node v22.23.4), same day, same backend; the archived 3.1.0 host on empty stores; the real cyril binary
(build `e3a3868d`) on 0.66.91 through hex1b. Redacted bundles: `experiments/conductor-spike/kas-{live-sweep,workflow-gateoff,workflow-gateon,
subagent-parent-stream,signin-refusal,approve-for-me,mcp-elicitation,hooks-v2,workflow-launch-option,unparseable-command}-3.1.0.jsonl`.

| leg | what ran | result |
|---|---|---|
| 0 | `--help` of both servers (no help surface); initialize-only on 0.66.43 / 0.66.48 / 0.66.58 / 0.66.62 / 0.66.70 / 0.66.91 (callback auth) and 0.66.91 (no auth) | `extensionMethods` grow per carve as the static diff predicted (+`configuration/set` 0.66.62, +`memory/getSyncStatus` 0.66.70, +`memory/sync`, +`get_network_configuration` 0.66.91); `autoCompactionThreshold` from 0.66.58; `authMethods` unchanged; cyril's `thinking` key logs `configuration.intake.unknown-key` on both engines |
| 1 | paired sweep (initialize → session/new → read prompt → shell prompt with permission → method census → set_config_option census → close) | 0 × -32601 on anything cyril sends; `session_usage` after every `turn_completion` and replayed on `session/load` (NEW only); turn frames gain `executionId`/`turnId`/`timestamp`/`boundarySource`; `summarizationThreshold` on `context_usage`; `_kiro/memory/syncStatus` pushed on connect; `effortLevel`/`thinking` accepted-not-echoed on both; permission request shape unchanged |
| 2a/2b | paired workflow gate-off (6 runs) and gate-on (11 runs), fixed recipe set | all runs MATCH across engines; NEW adds `credits[]`, `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch`, `notifyId`; `_meta.kiro.preview` removed; `workflowLaunch` option offered gate-on; the pause wake is unchanged |
| 3 | 41 forwarded leads (KAS-L1–13, WF-L1–10, TUI-L1–13, HOST-L1–6) + LIVE-A–H | verdicts in the tables above; summary: CONFIRMED 28, CHANGED 7 (tier, deny-rule text, hooks client path, recap, displayText, shellEnvironment, code-index), REFUTED 3 (trust prompt, recipe trust gating, wake race), NOT-REACHED 7 (model-fallback frames, OAuth completion, peer messaging, wake reply length, memory handed to sub-agents, session-level `turn_timing`, `active_file`), NOT-RUN 2 (HOST-L6, WF-L10) |
| 4 | `sweep-new-fields.py --diff` per pair; `unread-fields.py` over every NEW capture cyril would receive | 70 UNREAD paths, all triaged (allowlist lines + the issues in §9); dropped-by-consumer items the key-name check cannot see: `session_usage`, `agentSubtaskId` on parent chunks, `recap`, `turn_timing`, workflow `credits[]` |
| 5 | cyril `--agent-engine kas` on 0.66.91 (temp HOME, real data home, `KIRO_KAS_SERVER_PATH` + `KIRO_AGENT_PATH` = 3.1.0 node) | S1 startup PASS (toolbar in ~2 s, token via `get-kas-token`, `shell_type` in 2 ms); S2 skill dispatch PASS; S3 `/thinking` `/model` `/effort` `/mode` PASS; V4 effort badge PASS; V5 `/usage` shows per-turn credits only (`session_usage` dropped); V6 node v22.23.4 runs the bundle; **S4–S8 and V3 NOT-RUN** (budget after the auth stop; seeds and launch lines in `live/tmp/leg5-launch.txt`) |
| 6 | v2 pair | DROPPED (Classic retired; budget) |

**Host (archived 3.1.0 `kiro-cli`, empty stores):** logged out, it answers `initialize` itself in 0.5 s with `authMethods:[{id:"kiro-login", …"Run
'kiro-cli login'…"}]` — or `{type:"terminal", id:"kiro-login-terminal", args:["--login"]}` when the client declares `_meta.terminal-auth` — and refuses
`session/new` with -32000 "Authentication required…" (2.36.0 and 2.35.2 launchers exit rc 1 instead: the 2.37.0 line). `--agent-engine v1` is accepted
without a panic. With `--auth-method client` (accepted only when the resolved engine is v3, which makes it a token-free route discriminator): the
unlisted name `cyril` → **v3** (node child, 34 `extensionMethods`, five forwarded `getAccessToken` requests); `--agent-engine v2`, `acp.agentEngine:"v2"`
and the snapshot name `acp-probe` → v2. `kiro-cli _ extract-runtime` installs to `$KIRO_RUNTIME_DIR/<ver>-<hash>/{node,bun,tui.js,kas/…}` (helper default
`<bin>/../lib/kiro-cli/`). **Orchestrator probe** (`probe-kas-host31-defaultdir-3.1.0.py`, `audit-3.1.0/orchestrator/host-defaultdir-3.1.0.jsonl`):
a plain `kiro-cli acp --agent-engine v3` with `KIRO_RUNTIME_DIR` unset extracts to `<data dir>/kiro-cli/kas/3.1.0-<sha256>/` and runs `<data dir>/kiro-cli/node`
— the layout cyril's discovery searches; the live lane's "P1 candidate" (nothing under `kas/`) came from arms that all had `KIRO_RUNTIME_DIR` set and is
**refuted**: default installs keep working, the runtime-dir layout is the opt-in P3 (cyril-8ts2).

**get-kas-token on 3.1.0 (LIVE-F):** against a copy of the real store, `kiro-cli chat _ get-kas-token` (no `--force-refresh`) printed
`{kind:"getKasToken", data:{accessToken, expiresAt, profileArn, provider:"Enterprise"}}`, `expiresAt` equal to the store row, idempotent, row unchanged —
cyril's parser fits. **Auth incident:** the next arm ran the archived `kiro-cli acp` on the same copy (the brief had said empty store); the copy's token
row changed within 6.7 s (host `state` +1 row, `extracted_kas_versions` pruned 4 → 2; the only auth log line `acp-peer.refreshToken.ok 8 ms`, a
callback read). Whether that consumed the single-use refresh token is unprovable (the copy was deleted); the real store was never opened by a 3.x
binary and its `expires_at` stayed 23:50:59Z. All later legs ran token-free; HOST-L6 was NOT-RUN. New trap: §10.

### 8a. Leg 5 follow-up (live5): the scenarios the live lane left unrun

Run 2026-11-09 01:25–02:11Z (`audit-3.1.0/live5-findings.md`, 115 evidence files under `live5/cyril-kas/`: per-step screen text and SVG, stdio
taps of the KAS child with tokens and ARNs redacted, cyril and KAS logs). cyril build `d1776687` (sources = e3a3868d) on KAS 0.66.91 / node v22.23.4
through hex1b; 11 metered turns + one workflow wake; no `kiro-cli` run by the lane; one cyril at a time. Method: cyril has no log-level knob
(`crates/cyril/src/main.rs:295-299` installs the JSON subscriber at INFO), so the wire came from a tee wrapper set as `KIRO_AGENT_PATH` that
records the KAS child's stdio and `exec`s the real node.

| scenario | verdict | what was seen |
|---|---|---|
| S4 trust / Always-allow (auxt, k8wc) | PASS; one CHANGED | 4-option ask (`reject_always` new); session scope suppresses the re-ask; `/new` re-asks; user scope writes `permissions.yaml` (`rules: - capability: shell / effect: allow / match: [git stash list]`) and a fresh session runs unasked. cyril reports the host's real shell (`fish` here), KAS resolves `shellParser:"fish"`; under that parser the unparseable `echo "oops` is **asked, persisted at user scope and executed** (exit 127) — the 2.34.0 auto-deny is a bash/posix-parser property; every probe had answered `bash` (trap 34) |
| S5 sub-agent (lpdk) | CONFIRMED | child text glued to the parent reply; `invoke_sub_agent` asks with `consent.capability:"subagent"` and a blank preview (smy8 class) |
| V5 `/load` credits (t2w5) | CHANGED framing | `/usage` Credits = the HOME-wide `usage.sqlite3` ledger (same figure in a fresh process); replayed `session_usage` and `turn_completion` ignored; a loaded session renders **no transcript** (only "Session created") — the open cyril-99ds (replay marking) |
| S6 MCP (1v25, 0yme) | CONFIRMED | two orphan "Tool Load" rows render; `probe_echo` runs after a `consent.capability:"mcp"` ask (`agentManagesTrust:true`); form elicitation declined in 3 ms with a visible message, no stuck turn; KAS advertises `elicitation:{form,url}` to MCP servers regardless of the client |
| S7 `KIRO_HOME` (d7ve) | PASS | absolute honoured by both; relative ignored by both with cyril's WARN — but it leaks to the `get-kas-token` helper, which writes `<cwd>/kh/settings/cli.json` |
| S8 workspace tier (mq15) | CONFIRMED | workspace agent, MCP server (`layer: workspace` in `/mcp`) and hook load and run with no trust text; the hook executed on the host without any permission request |
| V3 w55l | **REFUTED / fixed** | one approval overlay, step completes, `run_complete status:completed`, marker written; footer `run 0.23 · follow-up 0.09` (truncated, not rounded — bgvj) |
| S3b workflowLaunch (asrc) | CONFIRMED | KAS offers the option with `chat.enableWorkflows=true`; no cyril row in `/mode` or `/model` |

Token events: nine `getAccessToken` answers, all served by `kiro-cli chat _ get-kas-token` with `force_refresh:false` (`expires_at` 02:20:12Z
throughout); cyril's renewal across the hour boundary therefore stayed untested. Process hygiene: every `/quit` exited cyril within a second; the
node child's own exit lagged 1–60 s in three of nine runs (note on cyril-qdcs); nothing was left running.

**Corrections to static rows from live:** `recap` and `displayText` replay already existed on 0.66.43; the memory rollout flip did not change the
ACP default; MCP `structuredContent` produces no extra block on either engine; the wake race behaves the same on both engines; `shellEnvironment` is
applied agent-side only; client-executed hooks exiting 2 without output do not block on either engine. cyril's own S1 run also shows KAS labels the
ordinary `getAccessToken` round trip `acp-peer.refreshToken.ok` (809 ms = the `get-kas-token` subprocess), so that log line alone never proves an
OIDC refresh. Side observation: SIGTERM to cyril left its KAS node child alive (cyril-qdcs, open).

## 9. Issues

All labelled `kiro-3-1-0` plus `kas` (and `w1` / `v2` where they apply); `ready-for-agent` unless noted. The KAS parity epic cyril-c6hv gains the
three default-path children (cyril-lpdk, cyril-t2w5, cyril-asrc).

| id | P | title |
|---|---|---|
| cyril-mq15 (re-scoped) | P2 | Workspace trust prompt before `session/new`: ask before loading a folder's `.kiro` hooks, agents, MCP servers and recipes (owner decision 2026-11-08) |
| cyril-lpdk | P2 | KAS 0.66.91: sub-agent replies stream on the parent session (`agentSubtaskId`) and cyril renders them as parent text |
| cyril-t2w5 | P2 | KAS `session_usage`: adopt the authoritative session credit total (replayed on `session/load`) for `/usage` and the footer |
| cyril-bgvj | P2 | KAS 0.66.91 workflow step credits and run progress on the wire: model `credits[]` and `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch` |
| cyril-k3hf | P2 | V2 wrapper spawn must pin `--agent-engine v2`: kiro-cli ≥2.36.0 routes `kiro-cli acp` by `clientInfo.name` and a 3.x host hands the unlisted name `cyril` the v3 engine |
| cyril-asrc | P2 | Honour `chat.workflowLaunch` and expose KAS's `workflowLaunch` (Workflow use) option; sessions recorded before 0.66.58 load as delegate-all |
| cyril-aoby (raised) | P2 | KAS: handle the `model_fallback` / `model_routed` kinds — fallback is on by default since 0.66.91 |
| cyril-5anx | P3 | Session recap: map `chat.enableSessionRecap` to `settings.sessionRecap` and render the `recap` kind |
| cyril-zfoo | P3 | Approve for me on KAS (opt-in): set `KIRO_TRUST_CLASSIFIER_MODE=enforce` on the KAS child and render `trustClassifierApproved` rows |
| cyril-0yme | P3 | Interactive MCP elicitation: answer form and url elicitations like Kiro's TUI instead of a visible decline |
| cyril-8ts2 | P3 | KAS discovery is blind to `KIRO_RUNTIME_DIR` / `kiro-cli _ extract-runtime` installs (opt-in layout, 2.37.0+) |
| cyril-1nto | P3 | Clear the remote-MCP OAuth notice on `_kiro/mcp/oauthCompleted` and read `openExternalUrl {serverName, oauthCompletion, oauthFlowId}` |
| cyril-zmji | P3 | cyril-hosted hooks: an `executeHook` reply with exit code 2 and empty output does not block the tool on KAS; opt into hooks v2 for PreCompact/PostCompact |
| cyril-smy8 | P3 | Approval overlay shows a blank preview for KAS skill-load tool calls |
| cyril-0ax4 | P4 | Always-allow scope page: offer Kiro's `<cmd> <sub> *` subcommand tier |
| cyril-f4hi | P4 (needs-triage) | Consult `KIRO_DEFAULT_SETTINGS` (3.1.0 root-owned managed defaults) in cyril's settings marshal |

Notes appended this cycle (existing issues; the §8a follow-up added notes on cyril-k8wc, cyril-auxt, cyril-lpdk, cyril-1v25, cyril-t2w5,
cyril-0yme, cyril-mq15, cyril-d7ve, cyril-w55l (propose close), cyril-bgvj, cyril-asrc, cyril-qdcs, cyril-smy8, cyril-99ds, cyril-ebqu): cyril-aoby, cyril-ojum, cyril-0o7e, cyril-0rn8, cyril-53qx, cyril-58uv, cyril-6s21, cyril-s21o,
cyril-0na7, cyril-q8gq, cyril-hzkb, cyril-iq2c, cyril-68ny, cyril-7p7n, cyril-7c0s, cyril-497j, cyril-ebqu, cyril-r5pl, cyril-99ds, cyril-4u4a,
cyril-fb1m, cyril-inwg, cyril-xehm, cyril-0asq, cyril-w55l, cyril-1aq0, cyril-a7nk, cyril-cxd2, cyril-zjm5, cyril-tikf, cyril-c6hv, cyril-oswq,
cyril-lnxg, cyril-8alk, cyril-po2z, cyril-m8kq, cyril-a8se, cyril-7xxk, cyril-0b8t, cyril-qdcs, cyril-lxuo, cyril-j66p, cyril-4cz1, cyril-1v25 (closed),
cyril-zbu7 (closed).

Allowlist (`docs/wire-ignored-fields.txt`) lines added for the 70 UNREAD paths of the NEW captures (turn identity fields, compaction thresholds,
memory sync state, `notifyId`, `firstTokenReceivedAt`, workflow progress fields → cyril-bgvj, `trustClassifierApproved` → cyril-zfoo, `policyDenial`
read via serde rename, MCP form field names, `appliedPlanFeedback`/`originalUserMessages` → cyril-4u4a); the dead `session/update.update._meta.kiro.preview.*`
line removed. `unread-fields.py` exits 0 on the NEW captures afterwards.

## 10. Method notes for the next audit

Every item is also appended to `.claude/skills/kiro-release-audit/references/traps.md`.

- **Enumerate releases by probing the origin, not the bounded minor+2 window.** `audit-state.sh` missed 2.35.x–2.37.x and 3.0.0; a later feed's
  embedded older entry exposed the gap. Probe every `M.m.p` until two consecutive 403 minors.
- **A 3.x release has one binary.** `carve-frames.py` must fall back from `kiro-cli-chat` to `kiro-cli`; the host lane's scripts take the role-aware path.
- **0.66.91 is built with esbuild keepNames.** Literal counts inflate without behaviour change; a NEW-only literal is evidence only when its code is
  new, and `a(fn,"name")` labels make the bundle navigable by function name.
- **Flag defaults move without a schema or method change.** Run the flags extractor on every carve and bisect defaults, not only keys.
- **Hard-coded flags live at the entry point.** `workspaceTrusted:!0` sits in the stdio/ws bootstrap, not where its 89 consumers read it.
- **A changelog line can be true with zero engine delta** (elicitation, `structuredContent`): check the lexer diff for the keyword first.
- **The system `grep` is ugrep** and rejects `.{0,N}needle.{0,N}` on 14 MB single-line bundles; use the Python context extractors.
- **Ext-notification censuses miss variable registrations and conditional settings rows**; count spread-built flag objects by their container pattern.
- **Spread emit sites hide payload keys** from `kind:"…",payload:{…}` scans (`...She(state)` adds `credits`): read the emit sites.
- **A copy of the real auth store may be opened only by `get-kas-token` without `--force-refresh`.** `kiro-cli acp` on any store holding a token writes
  the token row within seconds and the client cannot tell a rewrite from a refresh. Engine-routing probes need no token: use an empty store and the
  `--auth-method client` discriminator. `acp-peer.refreshToken.ok` is the label of every callback read, not of an OIDC refresh.
- **A probe arm that sets `KIRO_RUNTIME_DIR` cannot answer where the default extraction goes.** Confirm a layout claim with the env unset and an empty
  `<bin>/../lib` (orchestrator probe); the live lane's P1 candidate fell to this.
- **The first run of a 3.x binary creates `data.sqlite3`, lock files and `cli.json` wherever HOME/XDG point and migrates an existing store**; every leg
  gets its own scratch dirs.
- **Lane reconciliation tables bundle feed lines**; a word-overlap merger placed 181/188 automatically, 7 by hand. Ask for one row per feed line.
- **Runner scripts must suffix captures by arm**: same-named arm captures overwrote each other (hooks batch 2 VOID, `turn_timing` session arm lost).
- **hex1b input:** a literal newline arrives as Ctrl+J and inserts `j`; `\uXXXX` escapes are sent literally; Ctrl+Q/Ctrl+C via `send_terminal_key` did
  not reach cyril. Use `send_terminal_key Enter`, kill the process to stop.
- **Covenant drift to fix:** `_kiro/memory/create` requires `memoryType, scopes, title, summary`; the doc lacks `configuration/set`, `diagram/render`,
  `memory/{sync,getSyncStatus,syncStatus,changed}`, `job/*`, `codeIntelligence/*`, `get_network_configuration`, `mcp/oauthCompleted`, the four new kinds,
  `effect:"evaluate"`, `automaticTurnProfiling`, `shellEnvironment`, `autoCompactionThreshold`, `sessionTools`.
- **cyril has no log-level env var**; the cyril-on-KAS method is a tee wrapper as `KIRO_AGENT_PATH` that records the KAS child's stdio and
  `exec`s node — it captures the access token and the profile ARN, so redact both before the files enter the tree.
- **The hex1b terminal inherits the user's `SHELL`** (fish here), and cyril reports that basename as `shell_type`; verdicts about shell parsing
  (auto-deny, rule tiers) depend on it. Record the shell per scenario and never assume bash.
- **`/usage` is a ledger, not a session view**: a `/new` + `/usage`, then a fresh process + `/load` + `/usage`, is the discriminator; without
  the second process the number would have read as "the replayed turn sum".
- **Count `"result":{"stopReason"`, not `"stopReason"`** when waiting for a turn to end — `turn_completion` info frames carry the key too.
- **Static lanes delivered on the full tier in 36–46 min each with no budget incident**; the live lane took 2 h 47 m and still left S4–S8 unrun. Next
  cycle: spawn the cyril-on-KAS leg as its own lane once the first static lane returns, and brief HOST-class routing probes as empty-store-only.

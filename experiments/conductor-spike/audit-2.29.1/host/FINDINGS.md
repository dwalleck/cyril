# Host lane — kiro-cli 2.28.0 → 2.29.0 / 2.29.1 (static)

Lane: **host** (Rust launcher `kiro-cli`, host binary `kiro-cli-chat`, term `kiro-cli-term`).
Audited 2026-10-11. Static only: **no engine spawned, no auth command run**. The only executions
were `--help` / `--help-all` / `agent <sub> --help` on the **archived** binaries under a temp
`HOME` + temp `XDG_DATA_HOME` (trap 6). Inputs:
`~/.local/share/kiro-research/binaries/<ver>/extracted/kirocli/bin/{kiro-cli,kiro-cli-chat,kiro-cli-term}`
+ each version's `BUILD-INFO`. Conclusions from the KAS carve cover 2.29.0 and 2.29.1 alike
(established facts); this lane additionally compares **2.29.0 vs 2.29.1** host binaries directly.

## Summary
1. **Visible CLI surface is frozen in 2.29.x.** `--help-all` for all three binaries is
   **byte-identical** (md5 `3a49e5a2…` launcher, `b551f29a…` chat, `58ee94de…` term) across
   2.28.0/2.29.0/2.29.1; all growth is in hidden helpers, host-module source paths and Rust code.
2. **Same-BUILD_HASH is not a content hash.** `BUILD_HASH=31e6bb3fad4a1fff9ba23aee436cef61c8579a58`
   is identical for 2.29.0/2.29.1 while `BUILD_DATE`/`BUILD_VERSION` and the binaries differ
   (chat 215572200 → 215787048 B, `.text` +190656, `.rodata` +4096, .gcc_except_table +9920).
   Both embedded payload ELFs (node, bun) are **byte-identical** in all three versions.
3. One new env name: `KIRO_CLI_IS_CHILD_PROCESS` (chat-cli only, 2.29.0+). One new settings key: none (119/119/119).
4. `--auth-method` gained a `client` variant (2.29.0); `cli` is now documented `(default)`.
   cyril already pins `cli` (reviewer) and already implements the client-side callback responder.
5. New subcommands: visible `agent upgrade <DIR>` (the folded 2.28.1 line) and hidden
   `_ cleanup-autocomplete-artifacts` / `_ standalone-cohort`, gated by the new 0% rollout flag
   `standalone_toolbox`.
6. Rollout registry: `standalone_toolbox` ADDED `treatment_percent=0`; `memory` and `memory_controls`
   internal→all (GA). **0 row deltas 2.29.0→2.29.1.**
7. No `_kiro.dev/*` / `_kiro/*` method-set change, no TuiCommand-args change, no doc-manifest change.
8. Classic deprecation wording is unchanged (`classic` 52/52/52, `deprecated` 129/129/129 counts).
9. cyril impact: 3 HANDLED surfaces confirmed (MCP OAuth URL, model-fallback notification,
   auth callback responder), 1 deliberate NOT-SENT (`--auth-method client`), no DROPPED breakage.
10. Two live-lane leads only: `--auth-method=client` handshake, and `KIRO_CLI_IS_CHILD_PROCESS`.

## Deltas

Legend: **direction** add/remove/change; **STATIC** = binary evidence only (trap 4: a lead);
**LIVE** = needs a spawned engine (this lane ran none). cyril verdicts are against
`crates/`, all `file:line` proven by grep in this session.

| item | version | gate / default | direction | STATIC/LIVE | cyril verdict + file:line | issue? |
|---|---|---|---|---|---|---|
| Same `BUILD_HASH` `31e6bb3f…`, different binaries | 2.29.0 vs 2.29.1 | n/a (build stamp) | change | STATIC | N/A — cyril gates on semver only, `crates/cyril-core/src/protocol/kas/version.rs:46,271-274` | no |
| `--auth-method` `client` variant + `(default)` on `cli` | 2.29.0 | acp only `--agent-engine=v3`; default `cli` | add | STATIC | **HANDLED (cli)**: reviewer pins `--auth-method cli` `crates/cyril-workbench/src/reviewer/runtime.rs:231`; **NOT-SENT (client)**: no `client` literal in crates/, though the callback responder exists `crates/cyril-core/src/protocol/kas/auth.rs:24,300` | maybe (opt-in lead) |
| AUTH_METHOD help rewritten ("Defaults to \"cli\"… use \"client\" to answer the engine's access-token requests from the ACP client") | 2.29.0 | n/a | change | STATIC | N/A — cyril parses no kiro help text | no |
| env `KIRO_CLI_IS_CHILD_PROCESS` | 2.29.0 | unset default; chat-cli only | add | STATIC | **NOT-SENT** — 0 hits in crates/; cyril's env set is HOME/USERPROFILE/KIRO_HOME/XDG_*/TMP* `runtime.rs:216-225` + KIRO_KAS_SERVER_PATH/KIRO_AGENT_PATH `discovery.rs:415,421` | no (lead) |
| rollout `standalone_toolbox` | 2.29.0 | `treatment_percent=0` (0%), segment=all, +1 cohort-override entry (digest `8f95b89bc47f4a04`) | add | STATIC | NOT-SENT (host rollout table is the launcher's; cyril reads no rollout table) | no |
| rollout `memory`, `memory_controls` | 2.29.0 | internal → **all** (GA) | change | STATIC | KAS lane's feature; row recorded here: cyril no table | no |
| rollout rows 2.29.0→2.29.1 | 2.29.1 | — | none (0 row deltas, overrides unchanged) | STATIC | N/A | no |
| hidden `_ cleanup-autocomplete-artifacts` | 2.29.0 | hidden (`Hidden helpers run by installers. Not for direct use`) | add (0→4) | STATIC | NOT-SENT | no |
| hidden `_ standalone-cohort` | 2.29.0 | hidden | add (0→2) | STATIC | NOT-SENT | no |
| `acp` help sentence: "Accepts and ignores the autocomplete bundle's `init` calls…" | 2.29.0 | n/a | add | STATIC | NOT-SENT (cyril never sends `init`) | no |
| **`agent upgrade <DIR>`** (folded 2.28.1 line) | 2.29.0 | visible; `--no-backup`, `--dry-run`, `-v` | add | STATIC | NOT-SENT (0 hits) | no |
| new source paths (chat) | 2.29.0 | — | add | STATIC | NOT-SENT | no |
| `tool_tags` in the V2 hook payload + telemetry dimension | 2.29.0 | V2 TUI hooks only | add (0→3) | STATIC | **DROPPED** — cyril's hook surface is the KAS registry `crates/cyril-core/src/protocol/kas/hooks.rs:25-42`; payload hooks are the host's | no |
| Classic `fs_write` symlink auto-approve fix | 2.29.0 | classic engine only | change | STATIC (logic) / LIVE (claim) | **DROPPED** — decision is engine-internal; cyril only converts the resulting permission frames `crates/cyril-core/src/protocol/convert/mod.rs:240,291` | no |
| model fallback applies without restart | 2.29.0 | n/a | change | STATIC | **HANDLED** (notification parse unchanged) `crates/cyril-core/src/protocol/convert/kiro.rs:1209` (`model/not_found` + `fallbackModel`), `:1168` (`agent/not_found` + `fallbackAgent`) | no |
| MCP OAuth `oauthUrl` surfaced (2.29.0) + issuer fix (2.29.1) | 2.29.0 / 2.29.1 | n/a | add / fix | STATIC | **HANDLED** `kiro.rs:1123-1143` → `Notification::McpOAuthRequest { server_name, url }`; issuer fix is engine-side `crates/agent/src/agent/mcp/oauth_util.rs` (47→46 call sites) | no |
| knowledge-search limits across bases | 2.29.0 | n/a | fix | STATIC | NOT-SENT — cyril embeds no semantic-search client | no |
| settings key set | 2.29.0 / 2.29.1 | 119 keys | none | STATIC | N/A (`settings.log`: 119/119/119, 0 added/removed) | no |
| `_kiro.dev/*` + `_kiro/*` method set | both | — | none | STATIC | N/A — trap-3 set diff identical, incl. `_kiro/auth/getAccessToken`, `_kiro/session/delete` (present in 2.28.0 too) | no |
| TuiCommand args (`*Args` structs) | both | — | none | STATIC | N/A (14 structs, identical) | no |
| doc manifest | both | — | none | STATIC | N/A (164 docs / 37 `category:feature`, byte-identical) | no |
| Classic deprecation wording | both | unchanged | none | STATIC | N/A (`classic` 52/52/52; `deprecated` 129/129/129; `sunset` 1/1/1) | no |
| serde shape churn (host) | 2.29.0 / 2.29.1 | — | change | STATIC | NOT-SENT (internal session-state/AWS-SDK shapes; no ACP wire surface) | no |
| launcher `crates/fig_request/src/reqwest_client.rs` +1 tracing call | 2.29.0 → reverted 2.29.1 | — | change | STATIC | NOT-SENT | no |

### Detail for the non-obvious rows

**Same-BUILD_HASH (acceptance a).** `BUILD-INFO` per version:

   2.28.0 BUILD_DATE=2026-10-05T23:47:51.564044+00:00 BUILD_HASH=877b466eda188c6dc44136df230f7e193a4edbfa BUILD_VERSION=2.28.0
   2.29.0 BUILD_DATE=2026-10-09T03:12:17.353320+00:00 BUILD_HASH=31e6bb3fad4a1fff9ba23aee436cef61c8579a58 BUILD_VERSION=2.29.0
   2.29.1 BUILD_DATE=2026-10-10T03:23:23.018134+00:00 BUILD_HASH=31e6bb3fad4a1fff9ba23aee436cef61c8579a58 BUILD_VERSION=2.29.1

So two builds one day apart share the hash while everything else moved. Proof the *content*
changed (not just the version string):
- sizes chat 215572200 → 215787048 (+214848); launcher 43928608 → 43929632 (+1024); term 34056416 → 34058976 (+2560);
  `raw/payload-inventory.txt`
- ELF sections (`raw/elf-sections.txt`) chat 2.29.0→2.29.1: `.text` +190656, `.rodata` +4096,
  `.gcc_except_table` +9920, `.eh_frame` +6624, `.rela.dyn` +2184, `.data.rel.ro` +2624, `.got` −72, `.data` −64
- source-level: `crates/agent/src/agent/mcp/oauth_util.rs` 47→46 tracing call sites,
  `crates/chat-cli/src/mcp_client/oauth_util.rs` 34→33, **new** source path
  `crates/chat-cli/src/launch/notification_order.rs`, ±line shifts in `fs_write.rs`,
  `tool_manager.rs`, `launch.rs` (`raw/srcloc-corrected.log`)
- reordering of adjacent `.rodata` literals proves code motion even where no literal was added
  (`Updating:`/`Appending to:` swap in the fs_write region, `raw/ctx-fswrite-*.log`)
[INFERENCE] `BUILD_HASH` is a **build-configuration/release-train stamp**, not a digest of the
produced artifact; a content-addressed hash of two different source revisions cannot be equal.
The payloads (embedded node ELF sha `81925c09…`, bun ELF sha `a83d2637…`) are byte-identical in
all three builds, so the differing bytes are the host Rust code plus the version/BUILD_DATE strings.

**Frames** (`frames-2.28.0.json` / `-2.29.0.json` / `-2.29.1.json`): KAS tar 27156222 → 27802249
→ 27802049 compressed (dsize 180664320 → 183224320 → 183224320); node ELF csize 32031786 /
dsize 124679552, sha `81925c0995b5c142…` **all three**; bun ELF csize 29887616 / dsize 79500640,
sha `a83d263767d839e4…` **all three**. Only the KAS frame moves, by exactly 200 compressed bytes
between 2.29.0 and 2.29.1 (the `@types/node` license file, per the established KAS carve fact).

**`agent upgrade` (acceptance b).** `kiro-cli-chat agent upgrade --help` on the archived binaries:
2.28.0 → `error: unrecognized subcommand 'upgrade'` (tip: `migrate`); 2.29.0/2.29.1 → full help
(`raw/help-agent-agent-upgrade-2.29.0.txt`): *"Upgrade agent configs in a directory to the
universal format both engines load. Rewrites top-level \*.json files in place (subdirectories are
not searched); already-universal, V3-only, out-of-sync, and externally managed agents are left
untouched"*, args `<DIR>` + `--no-backup` (*"Don't save each original as `<name>.json.bak` before
rewriting it"*) + `--dry-run`. Implementation: new source path
`crates/chat-cli/src/cli/agent/upgrade.rs` (`raw/paths/host-paths-delta.json`, `substring_in_old=0`),
registration `crates/chat-cli/src/cli/agent/root_command_args.rs` (+23 lines: 108→131),
`crates/chat-cli/src/cli/agent/mod.rs` (+1 line), migration engine
`crates/agent/src/agent/agent_config/migration/io.rs` (+23 lines: 136/152→159/175).
What it *rewrites* (agent-side literals, 2.29.0/2.29.1 only): refuses symlinked configs
(*"would fail: config is a symlink; the linked target would be modified in place"*), skips
*"already universal"*, *"not an agent config"*, *"managed externally"*, *"symlink to a file outside
the directory"*, *"not a regular file"*, *"out of sync (not rebuilt)"*, refuses when
*"too many existing backups"*, prints *"Upgrading agents in"* / *"Dry run … no files written"* /
*"Originals backed up next to each upgraded file"*. The identical migration machinery explains the
`/upgrade-agent` line ("rewrites legacy hook matchers such as `fs_read`") — that half is the V3
slash command, i.e. not this lane.

**Acceptance (c).** **No new `category:feature` node**: the doc manifest is byte-identical across
all three versions (164 documents, 37 `category:feature`, 0 added/removed/changed; embed stamps
`2026-09-02T18:38:18.266523` (103 docs) and `2026-09-28T19:09:06.985120` (140 docs); `raw/docman.log`).
The standing trap-12 check against `https://kiro.dev/sitemap.xml` (1192 `<loc>`, saved as
`raw/kiro-sitemap.xml`) finds 26 of 37 feature nodes whose file stem appears in **no** sitemap path
(list in `raw/docman-sitemap-check.txt`: `voice-mode`, `trust-configuration`, `paste-chips`,
`mcp-registry`, `planning-agent`, `turn-usage-summary`, …), but the count is 26/26/26 — the same in
OLD and NEW, so it is a pre-existing URL-consolidation property, not this cycle's finding. The
sitemap does carry `/docs/cli/v3/` and `/docs/acp/` (lastmod 2026-10-09) with no matching manifest node.

## Unreleased / dark candidates

1. **`standalone_toolbox` `treatment_percent=0` (segment=all) with one live cohort-override entry**
   (`raw/rollout.log:36,73`; override table at offset 100305468 (2.29.0) / 100309780 (2.29.1),
   digest `8f95b89bc47f4a04`, summary counts `{v3_prompt: 135 override entries, standalone_toolbox: 1
   override entry}` — `static-rollout-2.29.1.py` reports `len()` of each field's list, **not** a
   treatment value, so only the registry row's `treatment_percent=0` is a percentage).
   Text: *"Autocomplete decoupling cohort: the bundled Toolbox post-install hook asks this flag
   whether to move a user onto the standalone channel. It does not gate update routing … Starts at
   0%; ramp treatment_percent in later releases."* Its client-side machinery shipped in the same
   release: hidden `_ cleanup-autocomplete-artifacts` (0→4 occurrences), hidden `_ standalone-cohort`
   (*"Print this user's standalone cohort as one JSON line"*, 0→2), the `acp` help addition about
   ignoring the autocomplete bundle's `init` calls, and `crates/chat-cli/src/cli/internal.rs` (new).
2. **`memory` / `memory_controls` went internal→all (GA)** in 2.29.0 (`raw/rollout.log:71-72`) —
   no longer dark; the `/memories` UI itself is `[V3]` (KAS/TUI lanes).
3. **2.29.1 ships host code beyond its single feed line**: new module
   `crates/chat-cli/src/launch/notification_order.rs` (only in 2.29.1, no tracing call sites) is
   *unmentioned* by the feed; the two `oauth_util.rs` call-site reductions (47→46, 34→33) are that
   feed line's own change. The 2.29.1 entry lists only the MCP OAuth issuer fix.
   `v3_prompt`'s override-entry count (135) is identical in the 2.29.0 and 2.29.1 override digests
   (`8f95b89bc47f4a04`, both); whether it changed from 2.28.0 is not itemised — the script flags
   `overrides_changed: True` without printing a per-key override diff.
4. **Hidden helper roster** (chat binary, unchanged in 2.28.0 except the two new ones): `upgrade-agent`
   (7/7/7), `refresh-feed`, `get-kas-token`, `get-telemetry-credentials`, `test-seed-v1`,
   `import-session`, `export-session`, `windows-sandbox`, `windows-kas-stop`, `acp-client`,
   `set-default`, `api-key-lineage`, `diagnostics`, `print-system-prompt` (the last found via a help
   sweep, not as a standalone literal). Only
   `cleanup-autocomplete-artifacts` (0→4) and `standalone-cohort` (0→2) are new — count-checked in
   both versions (trap 2).

## Leads for the live lane (host-owned; this lane ran no process)

Executable paths below are the **archived** binaries + archived KAS; temp `HOME`/`XDG_DATA_HOME`; no
`login`/`acp` subcommand of the *installed* launcher (trap 6).

- **L1 `--auth-method=client` handshake (highest value).** Method/subcommand:
  `kiro-cli acp --agent-engine v3 --auth-method client` (archived 2.29.1 launcher, temp HOME with a
  valid credential store copied from a scratch login, or use cyril's own `data.sqlite3` path via
  `KIRO_HOME`). Params: `initialize` then `session/new`; handshake field to watch: a **server→client
  JSON-RPC request** `_kiro/auth/getAccessToken` (ACP-stripped `kiro/auth/getAccessToken`), expected
  *before* the first model call on v2/v3. Expected signal: the request arrives under
  `--auth-method client` and **not** under `--auth-method cli`; reply shape
  `{accessToken, expiresAt, profileArn}` (cyril's responder: `kas/auth.rs:24,300`). Contrast run:
  same command with `--auth-method cli`.
- **L2 `KIRO_CLI_IS_CHILD_PROCESS`.** Run the archived `kiro-cli` launcher (or `kiro-cli-chat`)
  twice, once with `KIRO_CLI_IS_CHILD_PROCESS=1` and once without, in a temp HOME with a stale
  feed/update marker. Expected signal: suppression (or not) of
  the auto-update/nudge path (`KIRO_NO_AUTO_UPDATE` sits adjacent in the same env table) and any
  differing `[launch]` log line; also test whether the *launcher* honours it (the literal is absent
  from the launcher binary, so it should not).
- **L3 `agent upgrade <dir>`.** Fixture dir with four top-level `*.json`: a legacy agent using
  `fs_read` hook matchers, a universal config, a symlink to a file outside the dir, and a
  non-agent JSON. Command: `kiro-cli-chat agent upgrade <fixture> --dry-run`, then without
  `--dry-run`. Expected signal: dry-run prints per-file decisions and writes nothing; real run
  rewrites only the legacy file in place and leaves `<name>.json.bak`; symlink → *"would fail:
  config is a symlink"* and no write. Zero auth, zero turns.
- **L4 `_ cleanup-autocomplete-artifacts --dry-run`.** Temp HOME seeded with the legacy links
  (`~/.local/bin/q`, `qchat`, `kiro-cli-term`, shell-integration lines). Expected signal: the
  dry-run lists exactly the unproven legacy artifacts and removes nothing.
- **L5 model-fallback mid-session (V2 acp).** Method: v2 ACP session, then change the default
  model mid-turn (`/model` or a settings edit). Expected signal: `_kiro.dev/model/not_found` with
  `fallbackModel` still arrives mid-session and the next turn uses the fallback **without a
  restart** (cyril parses it at `kiro.rs:1209`); same for `agent/not_found` + `fallbackAgent`.
- **L6 MCP OAuth URL over SSH/tmux (V2 acp).** Method: an MCP server whose remote requires OAuth;
  no browser available (`SSH_CONNECTION`/`TMUX` set). Expected signal: `_kiro.dev/mcp/oauth_request`
  carrying a non-empty `oauthUrl` + `serverName` (cyril drops the frame to `None` when `oauthUrl` is
  empty, `kiro.rs:1134-1140`). 2.29.1's issuer fix should be visible as the flow completing with an
  issuer-bearing server config.
- **L7 negative lead (do not chase).** `tool_tags` is **not** on the ACP wire — it is a host
  hook-payload key (`crates/agent/src/agent/task_executor/mod.rs:214/220` context) plus a telemetry
  dimension. A capture sweep for `tool_tags` will find nothing.
- **L8 rollout override probe (optional).** With the launcher's `KIRO_ROLLOUT_FEATURES`-style lever,
  the `standalone_toolbox` cohort-override entry is the only client-side route to exercise the
  standalone-enrolment prompt; the registry default is `treatment_percent=0`.

## Changelog reconciliation

`feed-2.29.1.json` — 2.29.1 (1 line):
- "MCP OAuth sign-in in classic and Rust V2 now works with servers that require an issuer" →
  **host**: engine-side `crates/agent/src/agent/mcp/oauth_util.rs` 47→46 tracing call sites and
  `crates/chat-cli/src/mcp_client/oauth_util.rs` 34→33, ±line shifts, 0 new literals
  (`raw/srcloc-corrected.log`); cyril NOT-SENT (it never performs MCP OAuth — it only surfaces
  `oauthUrl`, `kiro.rs:1123`). The `[V3]`/KAS half is not in my lane.

`feed-2.29.1.json` — 2.29.0 (22 lines):
| line | location / verdict |
|---|---|
| [V3] `/memories` configures memory access … | not in my lane (KAS/TUI); host-side trace: rollout `memory`/`memory_controls` internal→all |
| V2 TUI hook matchers widen | not in my lane (TUI bundle / hook matching); no host matcher literal changed |
| V2 TUI tool hooks receive `tool_tags` | **host**: new payload key `tool_tags` (0→3), `crates/agent/src/agent/task_executor/mod.rs:214/220` context + telemetry dimension; cyril DROPPED |
| [V3] `kiro-cli acp` uses your CLI login unless `--auth-method=client` | **host**: AUTH_METHOD help rewrite + `client` variant + `(default)` marker; cyril HANDLED (cli) / NOT-SENT (client) |
| [V3] Custom agents use tool settings for `mcp.json` | not in my lane (KAS); `_kiro.dev/mcp/*` literal set unchanged |
| [V3] Stdio MCP servers inherit full env | not in my lane (KAS) |
| Classic `fs_write` no longer auto-approves symlink writes outside `allowedPaths` | **host**: no new literal in the v2 fs_write path (`allowedPaths` 3/3/3, `auto-approve` 5/5/5); `fs_write.rs` call-site lines shift −1 at the allow/deny check (:657/:660/:662 → :656/:659/:661) with :487/:491/:555/:572/:592 fixed, and `Updating:`/`Appending to:` literals reorder; the only new `symlink` strings (9→11) are in the agent-config migration path. Behaviour claim = seller's; cyril DROPPED |
| Model fallback changes take effect without restarting | **host**: `crates/chat-cli/src/cli/chat/cli/model.rs` −40 lines; cyril HANDLED via `model/not_found`+`fallbackModel` `kiro.rs:1209` |
| MCP OAuth shows the authorization URL | **host/TUI**: cyril HANDLED `kiro.rs:1123-1143`; display half is the TUI lane |
| [V3] Non-interactive runs respect telemetry.enabled / KIRO_DISABLE_TELEMETRY | not in my lane (KAS); host settings key set unchanged (119) |
| [V3] `/upgrade-agent` rewrites legacy hook matchers | not in my lane (V3 slash command); its CLI twin `agent upgrade` is the host finding above (`root_command_args.rs` +23) |
| Knowledge search limits across all bases | **host (client side)**: `semantic-search-client/…/context_manager.rs` 2→1 calls (deltas [−84]), `…/index/bm25_index.rs` 2→1, `…/semantic_context.rs` [1,9]; cyril NOT-SENT |
| [V3] Sessions no longer close under heavy load | not in my lane |
| [V3] Custom agents as subagents load own `mcpServers` | not in my lane |
| [V3] Effort selector at session start | not in my lane |
| [V3] Images resized to fit the model | not in my lane |
| [V3] `KIRO_API_KEY` minted outside `us-east-1` | not in my lane; host `auth/social.rs` [0,1] shifts only |
| [V3] Readable retry message for non-JSON error page | not in my lane (`api_client/mod.rs` deltas are AWS-SDK/refresh churn; no new message literal in scope) |
| [V3] Resumed sessions keep `UserPromptSubmit` hook output | not in my lane (KAS/TUI hooks) |
| [V3] Stop cancels a prompt while MCP servers start | not in my lane (KAS); the −60/−71/−72/−79 shifts in `chat-cli/src/cli/mod.rs` are the KAS-spawn block moving |
| [V3] Prompt sent as a turn ends includes that turn | not in my lane |
| [V3] Manual steering files load mid-prompt | not in my lane |

`feed-2.28.1.json` — 2.28.1 (1 line):
- "`kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3" →
  **host**: located and proved folded into 2.29.0 (absent in 2.28.0's binary with
  `error: unrecognized subcommand 'upgrade'`; full help in 2.29.0/2.29.1). See acceptance (b).

`feed-2.28.1.json` — 2.28.0 entry (11 lines): these are the **baseline** release, reconciled by the
previous audit (`experiments/conductor-spike/audit-2.28.0/host-findings.md`); no 2.29.x work:
full-screen copy-on-select, classic→3.0 offer, `[V3]` hook matcher tool tags, `/model`+`/effort`
mid-turn pickers, V2 TUI sub-agent stall fix, activity-tray rows, Windows V2 stdio MCP argv,
voice-setting hint, agent-monitor return, `[V3]` interrupted-tool-call outcomes, `[V3]` long-session
memory growth. Host-lane corroboration for one of them: the `tool_tags` payload key exists in the
2.29.x host because the `[V3]` matcher line landed in the 2.28.0 engine — not re-audited here.

## Method notes

- **Trap 9**: all analysis from embedded `crates/*/src/*.rs` source-path strings, `tracing` call-site
  line numbers, `.rodata` literal sequences and the compiled-in rollout table. No `nm`/`rustfilt`.
- **Trap 3 (set diffs, both counts)**: every add/remove claim above was resolved by exact substring
  presence/count in **both** versions (e.g. `cleanup-autocomplete-artifacts` 0/4/4, `tool_tags`
  0/3/3, `KIRO_CLI_IS_CHILD_PROCESS` 0/1/1, `standalone-cohort` 0/2/2, `symlink` 9/11/11).
- **Glue artefact discipline**: env/vocab runs glue neighbouring literals
  (`KIRO_NO_AUTO_UPDATEKIRO_CLI_IS_CHILD_PROCESSKIRO_DESKTOP_RELEASE_URL`). Each candidate was
  substring-checked against the other version, so glue pairs such as
  `KIRO_ENABLE_LOCAL_SANDBOXTREATMENT` (2.28.0) vs `KIRO_ENABLE_LOCAL_SANDBOX`+`KIRO_ENABLED_FEATURE`
  (2.29.x), `KIRO_MODI`/`KIRO_MODH`, `KIRO_TRUST_CLASSIFIER_SHADOWI`, `AMAZON_Q_CHAT_SHELLF`,
  `RUST_MIN_STACKR` are **not** reported as env adds/removes; only `KIRO_CLI_IS_CHILD_PROCESS` is real.
- **hashdiff caveat**: comparing per-file line-number *sets* produces false "removed" hits when the
  raw digit run of a later number absorbs a neighbouring token (e.g. `[44,371]` vs `[44,371,40]`).
  `raw/srcloc-verify.txt` re-derives the corroborating per-file call counts; only rows that survive
  that check are in the table.
- **Rollout-table pitfall (caught in review)**: `static-rollout-2.29.1.py`'s per-key `summary`
  stores `len(vv)` for list-valued override fields, so a printed `{'v3_prompt': {'treatment': 135},
  'standalone_toolbox': {'treatment': 1}}` means **135 / 1 override entries**, not treatment values.
  Registry rows are the only place a percentage appears (`treatment_percent`, printed as
  `treatment=` by the same script). All rollout statements above use the corrected reading;
  the override digests (`8f95b89bc47f4a04`) are content hashes of the override object and are
  comparable across builds.
- **Trap 12**: doc manifest extracted as pairs `(embedded timestamp, member count)` and byte-compared
  across the three binaries (identity), then feature stems matched against a freshly fetched
  `sitemap.xml` (`raw/docman-sitemap-check.txt`).
- **zstd masking**: `raw/ctx.py` / `litcmp.py` blank the four large zstd frames (KAS tar, node ELF,
  bun ELF) so host `.rodata` contexts are not polluted by payload bytes; masked payload identity is
  instead taken from `frames-*.json` sha256 values.
- **Scripts** (this lane, `host/`): `static-host-strdiff-2.29.1.py`, `static-host-hashdiff-2.29.1.py`,
  `static-host-srcloc-2.29.1.py`, `static-host-env-2.29.1.py`, `static-host-paths-2.29.1.py`,
  `static-host-serde-2.29.1.py`, `static-host-settings-2.29.1.py`, `static-host-vocab-2.29.1.py`,
  `static-host-novel-words-2.29.1.py`, `static-rollout-2.29.1.py`, `static-doc-manifests-2.29.1.py`
  (all copies/adaptations of the 2.28.0 originals, which are untouched). Raw evidence under `host/raw/`;
  the auth/env/tool_tags/classic contexts are reproduced in `raw/auth-env-evidence-2.29.1.txt`.
- **Process discipline (trap 6)**: no `login`/`logout`/`whoami`/`acp`/`chat` subcommand of any
  binary was run, and the installed `~/.local/bin/kiro-cli` was never executed. Leads L1/L5/L6 are
  therefore *handed to the live lane*, not run here.
- Housekeeping: a runaway 13.6 GB dump from the cut-off first attempt (`host/raw/trailing-insertions.txt`)
  was deleted; `host/raw/` is now 156 MB.

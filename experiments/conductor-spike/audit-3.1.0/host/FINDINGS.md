# Host lane — kiro-cli 2.32.0 → 3.1.0 (static)

Lane: **host** (Rust launcher `kiro-cli`, host binary `kiro-cli-chat`, term `kiro-cli-term` on 2.x; the single `kiro-cli` on 3.x). Audited 2026-11-08.
Static only (trap 4): every row below is **STATIC**. Executions were `--help`-class invocations of the **archived** binaries under
`env -i HOME=<tmp> XDG_DATA_HOME=<tmp>/xdg XDG_CONFIG_HOME=<tmp>/cfg XDG_RUNTIME_DIR=<tmp>/run PATH=<archive bin>:/usr/bin:/bin`
(`raw/help.sh`, `raw/help-chain.sh`; every argv and rc in `raw/help/help.log` + `raw/help/help-chain.log`). The installed `~/.local/bin/kiro-cli`
was never run; no `login`/`logout`/`whoami`/`chat`/`acp`/`update` without `--help`; no `--help` run touched the temp HOME (the only writes were
the deliberate `_ cleanup-autocomplete-artifacts --dry-run`, which created `xdg/kiro-cli/data.sqlite3`, `telemetry-export-drops.lock`,
`run-receipts/.reaper.lock`, `.kiro/settings/cli.json{,.lock}` inside the temp HOME — a fresh-store side effect worth knowing before any live leg).
Inputs: `~/.local/share/kiro-research/binaries/<ver>/extracted/kirocli/bin/*` for all 14 versions + `BUILD-INFO`, `install.sh`, `README`,
`manifest.latest.json`; TUI bundles only for the two checks the brief allows (auth envelope, engine selection).
String evidence = zstd-frame-masked printable strings (`raw/mkstrings.py` → `tmp/strings/<ver>/<bin>.txt`; KAS tar, Node ELF, Bun ELF and TUI
frames zeroed, so KAS/TUI/Node/Bun text never counts). Counts are `[2.32.0, 3.1.0]` substring counts in the **host role** (kiro-cli-chat on 2.x,
kiro-cli on 3.x) unless a role is named (`raw/cnt.py`, `raw/first.py` = 14-version series with first-change attribution, `raw/ctx.py`).
Trap 2/3 applied: every add/remove was re-counted in both builds and in the first version where it appears; glue rejected where noted.

## Summary

1. **The 3.x "merge" is a deletion, not a fusion.** 3.0.0 `kirocli/bin/kiro-cli` (219,494,120 B) is the 2.37.1 `kiro-cli-chat` (219,500,200 B)
   rebuilt and renamed: identical third-party crate set (297 crate-versions / 281 crates), byte-identical `.rodata` (139,050,240), `.text` −5,824 B,
   **0** source paths added vs the union of the three 2.37.1 binaries, and **all** 145 launcher-only (`q_cli`, `fig_*`, `zbus`, `dbus`,
   `amzn-consolas-client`) and 71 term-only (`figterm`, `alacritty_terminal`) source files gone. The chat binary had already absorbed the launcher's
   user-facing commands: 2.34.0 (clap `bin_name` → `kiro-cli`, `--agent` shorthand, `cli/telemetry.rs`, `cli/uninstall.rs`, `cli/update/cutover.rs`)
   and 2.35.0 (`integrations`, `cli/integrations.rs`, `auth/mod.rs`). Removed command paths: `internal` (25 fig/shell helpers), `setup`, `debug`,
   `theme`, `launch`/`quit`/`restart`, `translate`, `dashboard`, `inline`, `completion`, `hook`; `doctor` is a stub ("has been removed", 2.34.0);
   `--classic` survives only as an alias of `--legacy-ui`. The 3.x `_` namespace = `cleanup-autocomplete-artifacts`, `standalone-cohort`,
   `standalone-cutover` (2.34.0), `extract-runtime` (2.37.0); `chat _` (get-kas-token et al.) is unchanged.
2. **What "Classic retired" means: the v1 engine / legacy UI harness, not the V2 ACP engine.** Host string (2.37.0): "Kiro CLI 3.0 no longer runs
   Classic (the v1 engine), so this run ignores the Classic selection." The V2 Rust engine is **still shipped in 3.1.0**: `crates/chat-cli-v2/src`
   paths 509 → 523, all `_kiro.dev/*` literals count-identical (41), `kiro-cli acp --agent-engine v2 --help` rc=0 on 3.0.0 and 3.1.0. Kiro's own
   prompt dates its removal: "V2 no longer receives new features and will be removed in January" (`launch/v3_launch_prompt.rs`, 2.37.0).
3. **STANDING P1 (cyril-zbu7): PASS.** `kiro-cli chat _ get-kas-token [--reason] [--current-expires-at] [--force-refresh]` — help byte-identical
   across 2.32.0 → 3.1.0 except the Usage line (`kiro-cli-chat chat _` → `kiro-cli chat _`, a clap `bin_name` change at 2.34.0, two releases before
   the binary was renamed); `get-kas-token`/`getKasToken`/`force-refresh`/`_kiro/auth/getAccessToken`/`accessToken`/`expiresAt`/`profileArn`/`authMethod`
   /`kas_token.rs` counts identical in all 14 host binaries; the 3.1.0 TUI handler is 2.32.0's byte-for-byte in argv and envelope.
4. **The cyril-relevant surprise is the ACP engine router (2.36.0, `crates/chat-cli/src/cli/acp_engine.rs`).** `kiro-cli acp` now peeks the ACP
   `initialize` request ("failed to read the ACP initialize request", "ACP initialize peek task failed", "resolved kiro-cli acp engine") and routes by
   `clientInfo.name` against an embedded telemetry snapshot of **1,080 client names that "stay on v2"** plus an exempt list (6 names in 2.36.0,
   **empty since 2.37.1**); the comment: "When the `kiro-cli acp` default is v3 (no --agent-engine flag, no acp.agentEngine setting), a client listed
   here stays on v2 unless it is also in v2_exempt_clients.txt". `cyril` is in neither list. Kiro's own TUI started pinning `--agent-engine=v2` on
   its v2 ACP spawn in the same release. cyril's V2 wrapper spawn is a bare `kiro-cli acp` (`bridge.rs:394`), so on a 3.x host whose acp default is
   v3 it receives KAS and cyril's fingerprint fails loud with a remedy that names a cyril flag (`fingerprint.rs:92-96`). STATIC; live decides the
   default (HOST-L1).
5. **Runtime extraction (2.37.0) added an opt-in layout, not a default change.** `KIRO_RUNTIME_DIR` / `kiro-cli _ extract-runtime --dir`
   ("Defaults to KIRO_RUNTIME_DIR, else <prefix>/lib/kiro-cli beside this binary") install node/bun/tui.js/KAS into a shared root and prune other
   versions; the default data-dir path (`kas/<ver>-<sha>`, sqlite `extracted_kas_versions`/`pinned_bin_versions`, "KAS bundle already extracted")
   is unchanged. cyril's discovery (`discovery.rs:197-200`) never reads `KIRO_RUNTIME_DIR` — fine by default, blind to the opt-in.
6. **Rollout registry 23 → 26 → 15 rows.** `v3_prompt` 10%/75%i → 25%/100%i (2.33.1) → 50% (2.34.0) → **100% (2.34.2 = "offers to switch you to
   V3")**; `standalone_toolbox` 0 → 100% (2.35.2 = "retiring autocomplete desktop app"); `standalone_public` 0 → 100% (2.37.0);
   `trust_classifier_enforce` added 2.34.0 → 100% all (2.36.0 = "Approve for me"); `model_fallback_auto_refusal` added 2.34.0 → all (2.35.0).
   3.1.0 drops `rollout.rs` and 12 graduated rows (`v3_prompt`, `tui`, `voice`, `workflows`, `local_sandbox`, `remote_sandbox`, `session_dashboard`,
   `tangent`, `remote_changelog`, `trust_classifier_enforce`, `standalone_*`) and adds one dark row, **`memory_cloud_sync` 0% internal/nightly**.
7. **Env/settings/doc deltas:** env + `KIRO_RUNTIME_DIR`, `KIRO_LAUNCH_3_0`, `KIRO_AGENT_UPGRADE_NOTICE` (2.37.0), `KIRO_ELEVATED_PRIVILEGES`,
   `KIRO_ROUTER_VERBOSE/VERSION` (2.35.0), `KIRO_TRUST_CLASSIFIER_MODE`, `KIRO_TEST_SYSTEM_ROOT` (2.34.0), `KIRO_STANDALONE_NOTICE` (2.33.1),
   `KIRO_DEFAULT_SETTINGS`(+`_SKIP_OWNER_CHECK`), `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED` (3.1.0); − `KIRO_STANDALONE_CUTOVER/NOTICE`,
   `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED`, `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`, `KIRO_ROLLOUT_FORCE_NIGHTLY` (3.1.0); the merge dropped 109
   launcher/term names (`Q_*` shell integration, `KIRO_CLI_AUTOSUGGEST_*`). Settings 124 → 129 (+`standalone.autoEnroll` 2.33.1,
   +`chat.modelPicker.hiddenModels`/`chat.workflowLaunch`/`install.releaseUrl` 2.34.0, +`app.warnOnElevatedPrivileges` 2.35.0, +`acp.agentEngine`
   2.36.0, +`chat.launch30` 2.37.0, +`chat.enableSessionRecap` 3.1.0, −`standalone.autoEnroll` 3.1.0). Doc manifests regenerated for the first time
   since 2.32.0 in 3.1.0 (103→105, 140→142): +`features/default-settings.md`, +`features/terminal-program-status.md`, +`settings/session-recap.md` —
   all announced in feeds; no unannounced `category:feature` node.
8. **ACP sign-in (2.37.0) is a wrapper-path feature.** `acp --login` ("Log in interactively and exit, for ACP clients that run terminal
   authentication"), new `cli/acp_sign_in.rs`, JSON pointers `/clientCapabilities/_meta/terminal-auth` and `/capabilities/auth/terminal`, and the
   ACP `AuthMethod::Terminal{id,name,description,args,env,_meta}` type (`unstable_auth_methods`). cyril advertises no `_meta.terminal-auth` and
   ignores `authMethods` (0 non-test hits) → DROPPED; the free path (callback auth) is unaffected.
9. **`KIRO_DEFAULT_SETTINGS` (3.1.0)**: "Load a root-owned, read-only file of default settings that your own cli.json and workspace settings override";
   `KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK`; new `chat-cli-v2/src/database/default_settings.rs`. cyril's cli.json reader (`kas/settings.rs:65`) does
   not consult it → DROPPED (a managed default such as `chat.enableWorkflows` would be invisible to cyril).
10. **Packaging:** 3.x ships one binary + `install.sh` (installs `~/.local/bin/kiro-cli`, edits PATH in rc files, removes legacy shell integration with
    backups, `--global` → `/usr/local/bin`, refuses a root install into another user's HOME, glibc ≥2.34 gnu / ≥2.26 musl) + README; origin serves
    `.tar.zst/.tar.gz/.zip/.rpm/.deb` only (no `.tar.xz`), Linux variant label `headless`.

## Deltas

Legend: counts `[OLD, NEW]` are host-role substring counts in masked strings (2.32.0 chat vs 3.1.0 merged) unless a version chain is given; "first" = first
release where the literal/behaviour appears. cyril verdicts grepped this session against `crates/*/src` at HEAD e3a3868d.

| item | first | gate/default | direction | STATIC | cyril verdict + file:line | issue/note? |
|---|---|---|---|---|---|---|
| **Single binary** `kirocli/bin/kiro-cli` (219,494,120 / 220,246,888 B) replaces `kiro-cli` launcher (43 MB) + `kiro-cli-chat` + `kiro-cli-term` + `q`/`qchat` shims; = the 2.37.1 chat binary (same 297/281 crate set, `.rodata` 139,050,240 both, 0 source paths added vs union, 145+71 launcher/term-only files dropped) | 3.0.0 | — | change | STATIC | HANDLED — cyril only ever names `kiro-cli`: free-path resolver `auth.rs` `resolve_free_kiro_cli` (PATH, then `~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin`), wrapper default `kiro-cli acp` (`crates/cyril/src/main.rs:32-36`); nothing references `kiro-cli-chat`/`-term` | NOTE cyril-a7nk |
| `chat _ get-kas-token` (`--reason`, `--current-expires-at`, `--force-refresh`; envelope `getKasToken`/`error`) | unchanged | — | none (14/14 count-identical; help diff = Usage line only) | STATIC | HANDLED `crates/cyril-core/src/protocol/kas/auth.rs:44-46` (`["chat","_","get-kas-token"]`, `--force-refresh`), envelope parse unchanged | no (zbu7 closed; HOST-L4 regression check) |
| **ACP engine router** `crates/chat-cli/src/cli/acp_engine.rs` [0,5]: peeks `initialize.clientInfo.name`; embedded snapshot of 1,080 names "stays on v2"; exempt list 6 (2.36.0: CodeCompanion.nvim, avante.nvim, Agentic.nvim, obsidian-agent-client, JetBrains.*, hermes-agent) → **0 (2.37.1+)**; `cyril` in neither | 2.36.0 | `--agent-engine` flag > `acp.agentEngine` setting > (default; snapshot keeps listed clients on v2 "when the `kiro-cli acp` default is v3") | add | STATIC | **NOT-HANDLED** — V2 wrapper spawn passes the agent command verbatim (bare `kiro-cli acp`) `crates/cyril-core/src/protocol/bridge.rs:394`; KAS wrapper pins `--agent-engine v3` `kas/version.rs:360-361` (safe); mismatch remedy names cyril's own flag `protocol/fingerprint.rs:92-96` | **NEW P2** (below) + NOTE a7nk |
| `acp.agentEngine` setting — "Agent engine for `kiro-cli acp`: 'v2' (default) or 'v3' (string)" [0,5]; `acp --help` loses `[default: v2]` ("Overrides the acp.agentEngine setting; v2 when neither is set") | 2.36.0 | unset | add | STATIC | unread — cyril reads cli.json only for `chat.enableWorkflows` etc. `kas/settings.rs:65,112` | folded into the P2 |
| `acp --login` + terminal auth method: `cli/acp_sign_in.rs` [0,1]; pointers `/clientCapabilities/_meta/terminal-auth` [0,1], `/capabilities/auth/terminal` [0,1]; "--login is not supported with --auth-method=client, which leaves sign-in to the ACP client" (3.1.0 text) | 2.37.0 | wrapper start while logged out; client must declare `clientCapabilities._meta.terminal-auth` | add | STATIC | DROPPED — `engine.rs` `client_capabilities` emits only `settings`/`openExternalUrl`/`hooks` under `_meta.kiro` (`protocol/engine.rs:128-160`); `authMethods` unread (0 non-test hits); free path = callback auth, unaffected | NOTE cyril-cxd2 / cyril-0rn8 |
| `kiro-cli acp` / `--no-interactive` no longer print the agent's `[INFO]` stderr lines | 3.1.0 | — | fix | STATIC (logic-only: `[INFO]` 0/0, `stderr` 27/27) | N/A on the free path (cyril owns node's stderr); wrapper path gets a quieter child | no |
| `KIRO_RUNTIME_DIR` [0,4] + `_ extract-runtime --dir` ("Runtime root. Defaults to KIRO_RUNTIME_DIR, else <prefix>/lib/kiro-cli beside this binary"); `embedded_tui/runtime_dir.rs` [0,7] ("ignoring KIRO_RUNTIME_DIR: not an absolute path", "cannot lock the runtime dir; pruning disabled for this process", asset tokens `lib` `kas` `node_modules` `@kiro` `acp-server.js` `bun` `tui.js` `node`) | 2.37.0 | opt-in (env or installer helper); default data-dir layout unchanged (`extracted_kas_versions`/`pinned_bin_versions` 3/3, `KAS bundle already extracted` present) | add | STATIC | DROPPED — `kas/discovery.rs:197-200` reads `KIRO_DATA_DIR`, `KIRO_KAS_SERVER_PATH`, `KIRO_AGENT_PATH` only; a `KIRO_RUNTIME_DIR` install (or a packaged `<prefix>/lib/kiro-cli`) is invisible unless `KIRO_KAS_SERVER_PATH` is set | **NEW P3** (below) |
| `KIRO_DEFAULT_SETTINGS` [0,8] + `KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK` [0,2]; `chat-cli-v2/src/database/default_settings.rs` [0,new]; "`X` cannot be changed in the read-only default settings; set it in your global or workspace settings instead"; "ignoring unknown keys in default settings file"; TUI: "only supported on Linux and macOS", 1 MiB cap | 3.1.0 | unset; precedence default < global `cli.json` < workspace | add | STATIC | DROPPED — `kas/settings.rs:65` reads `<KIRO_HOME>/.kiro/settings/cli.json` only | **NEW P4** (below) |
| Rollout registry: rows 23 → 26 (2.34.0: +`model_fallback_auto_refusal`, +`standalone_public`, +`trust_classifier_enforce`) → 15 (3.1.0: −12 graduated rows, +`memory_cloud_sync`); `v3_prompt` 10/75i → 25/100i (2.33.1) → 50 (2.34.0) → 100 (2.34.2); `standalone_toolbox` 0→5→20→50→100 (2.34.0…2.35.2); `standalone_public` 0→10→30→50→100 (2.34.0…2.37.0); `trust_classifier_enforce` int/nightly → all (2.36.0); `rollout.rs` path gone (3.1.0) but the JSON table remains (anchor moved) | 2.33.1…3.1.0 | compiled-in | change | STATIC | NOT-SENT (cyril never runs `kiro-cli chat`; `KIRO_ENABLED_FEATURES`/`KIRO_ROLLOUT_FEATURES` are launcher→TUI env) | NOTE a7nk; `memory_cloud_sync` → cross-ref kas lane / NOTE cyril-s21o |
| `background_execution` row reworded: TUI sends `universalBackgroundTool: { enabled: true }` on initialize, session/new, session/load (was `backgroundExecution: { enabled: true }` on session/new; `backgroundExecution` 2→0, `universalBackgroundTool` 0→1); KAS operator env `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED` replaces `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED` | 3.1.0 | 0% internal/nightly (dark) | change | STATIC | NOT-SENT (0 hits for either key) | no (dark); kas lane |
| **Classic (v1) retired**: "Kiro CLI 3.0 no longer runs Classic (the v1 engine), so this run ignores the Classic selection." [0,1]; "Kiro CLI 3.0 no longer includes Classic, and this install is missing the terminal UI. Reinstall Kiro CLI" [0,1]; `--legacy-ui` (`[aliases: --classic]`) and `--agent-engine v1` still parse | 2.37.0 (strings) / 3.0.0 (announced) | — | remove | STATIC | N/A (cyril never drove v1) | no |
| **V2 engine retained**, deprecation dated: "V2 no longer receives new features and will be removed in January" [0,1] (`launch/v3_launch_prompt.rs`, 2.37.0); `chat-cli-v2` paths 509→523; `_kiro.dev/` 40→41 (one `_kiro.dev/command` prefix literal at 2.33.0, see §Source paths) | 2.37.0 | — | deprecation | STATIC | cyril still ships `--agent-engine v2` (`crates/cyril/src/main.rs:41`, `types/agent_engine.rs:17`, `convert/kiro.rs`) | NOTE a7nk (date) |
| Engine-switch mechanics: `launch/v3_launch_prompt.rs` [0,new 2.37.0] ("Stay on V2, remind me later", "Stay on V2, don't ask again", "Switch to V3", "Keep V2"), `launch/v3_switch.rs` [0,new 3.1.0] replaces `launch/v3_ease_in.rs` (removed 3.1.0); `chat.agentEngine` persisted ("Could not save V3 as your default. To switch manually, run: kiro-cli settings chat.agentEngine v3"); env `KIRO_LAUNCH_3_0`, `KIRO_AGENT_UPGRADE_NOTICE` (2.37.0); setting `chat.launch30` "Opt this machine into the Kiro CLI 3.0 launch behavior before it ships (boolean)" (2.37.0) | 2.37.0 / 3.1.0 | — | change | STATIC | N/A | no |
| `chat --trust-workspace` ("Skip the \"Trust this workspace?\" prompt for this launch only (V3 only)") [0,1] | 2.34.0 | V3 | add | STATIC | N/A — host flag; the KAS-side trust prompt is the kas lane's | cross-ref kas lane |
| `agent upgrade` V2→V3 analysis: serde `struct V3PermissionRule [4]`, `struct DetectorConfig [9]` (2.36.0); "allowReadOnly let V2 read any file without asking. V3 has no rule for it…" [0,1] (2.36.0); "V3 has no tool for these V2 tools, so V3 ignores them. V2 still runs them, so the agent keeps them." [0,1] (3.1.0; `todo_list` dropped from the kiroUpgrade list) | 2.36.0 / 3.1.0 | — | add | STATIC | N/A | no |
| Elevated-privilege warning: `KIRO_ELEVATED_PRIVILEGES` [0,1], `app.warnOnElevatedPrivileges` "Warn when execution sessions run as root or Administrator (boolean, default: true)" [0,3], "Warning: Kiro CLI is running as root. Tools and commands can create root-owned files…" | 2.35.0 | on | add | STATIC | N/A | no |
| Host secret store for KAS: `launch/secret_storage.rs` [0,8] ("getting secret"/"setting secret"/"deleting secret", "copied the legacy secret store into KIRO_HOME", 0600) — the non-interactive client answers KAS secret storage so headless runs reuse MCP sign-ins | 2.34.0 | — | add | STATIC | cyril default-declines `_kiro/secret/*` (unchanged) | no |
| `standalone-cutover` helper [0,3] + `fig_install/src/standalone_cutover.rs` (launcher) + `KIRO_STANDALONE_NOTICE` (2.33.1) + `standalone.autoEnroll`; all removed/graduated by 3.1.0 (`KIRO_STANDALONE_CUTOVER` 1→0, `standalone.autoEnroll` table entry gone) | 2.34.0 → 3.1.0 | rollout `standalone_public`/`standalone_toolbox` 100% | add then retire | STATIC | N/A (macOS desktop → standalone cutover) | no |
| Settings keys: +`standalone.autoEnroll` (2.33.1), +`chat.modelPicker.hiddenModels`, +`chat.workflowLaunch` ("Workflow use for new KAS workflow sessions: when-asked, large-tasks, or delegate-all (string)"), +`install.releaseUrl` (2.34.0), +`app.warnOnElevatedPrivileges` (2.35.0), +`acp.agentEngine` (2.36.0), +`chat.launch30` (2.37.0), +`chat.enableSessionRecap` (3.1.0), −`standalone.autoEnroll` (3.1.0); `chat.agentEngine` description still says "'v2' (default)" | per version | — | add/remove | STATIC | none read by cyril except the `chat.enableWorkflows` family (`kas/settings.rs:78-112`); `chat.workflowLaunch` is a KAS-session setting cyril does not forward | no (`chat.workflowLaunch` → kas lane) |
| Env set (host role): + `KIRO_STANDALONE_NOTICE` (2.33.1), `KIRO_TEST_SYSTEM_ROOT`, `KIRO_TRUST_CLASSIFIER_MODE` (2.34.0), `KIRO_ELEVATED_PRIVILEGES`, `KIRO_ROUTER_VERBOSE`, `KIRO_ROUTER_VERSION` (2.35.0), `KIRO_AGENT_UPGRADE_NOTICE`, `KIRO_LAUNCH_3_0`, `KIRO_RUNTIME_DIR` (2.37.0), `KIRO_DEFAULT_SETTINGS`, `KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK`, `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED` (3.1.0); − `KIRO_STANDALONE_CUTOVER`, `KIRO_STANDALONE_NOTICE`, `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED`, `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`, `KIRO_ROLLOUT_FORCE_NIGHTLY` (3.1.0). Unchanged: `KIRO_HOME` (7→9 at 2.34.0 = +2 readers in `secret_storage.rs`), `KIRO_DATA_DIR` 3/3, `KIRO_KAS_SERVER_PATH` 2/2, `KIRO_KAS_NODE_PATH` 2/2, `KIRO_DISABLE_TELEMETRY` 2/2, `KIRO_ACP_RECORD_PATH` 0/0 (TUI-only) | per version | — | add/remove | STATIC | cyril reads `KIRO_HOME`, `KIRO_DATA_DIR`, `KIRO_KAS_SERVER_PATH`, `KIRO_AGENT_PATH` only (`kas/discovery.rs:197-200`, `kiro_agent_config.rs`) — none of the new names | no |
| Doc manifests regenerated (2026-11-04 / 105 docs, 2026-11-03 / 142 docs): + `features/default-settings.md` (feature, validated 2026-11-02), + `features/terminal-program-status.md` (feature, **draft**, validated 2026-10-13, OSC 7501), + `settings/session-recap.md`; changed `commands/agent.md` (+upgrade), `slash-commands/memories.md` (+delete), `slash-commands/model.md` (+hide/edit), `commands/settings.md`, `slash-commands/settings.md` | 3.1.0 | — | change | STATIC | N/A | no (all announced) |
| Dependencies (2.32.0 chat → 3.1.0): −`liblzma` 0.4.6 → +`lzma-rust2` 0.21.0 (pure-Rust xz), +`zstd` 0.14.3, +`typed-path`, `zip` 4.6.1 → 9.0.3, aws-sdk/smithy bumps, old `hyper` 0.14 / `h2` 0.3 / `rustls` 0.21 stack gone; 2.37.1 chat → 3.0.0: identical crate set; launcher-only crates dropped at the merge: `clap_complete`, `clap_complete_fig`, `tokio-tungstenite`, `wayland-*`, `wl-clipboard-rs`, `zvariant`, `prost-reflect`, `dircpy`, … (35) | 3.0.0 / 3.1.0 | — | change | STATIC | N/A | no |
| v2 `_kiro.dev/*` surface: 41 literals present in 3.1.0; full method names count-identical (`_kiro.dev/commands/{execute,options,available,model/options,agent/options,prompts/options}`, `_kiro.dev/mcp/*`, `_kiro.dev/telemetry/*` …); +1 bare `_kiro.dev/command` prefix literal (2.33.0) | 2.33.0 | — | none | STATIC | `convert/kiro.rs` dispatch unaffected | no |
| Host consumer copies of KAS pushes `_kiro/config/template`, `_kiro/governance/state` 2→1 | 3.1.0 | — | dedup | STATIC | N/A | no |
| `KiroControlPlaneBearerService.GetEffectiveEnterpriseManagementResourceWithToken` [0,10 "Enterprise Management"]: "Resolved device-code sign-in setting", "Enterprise Management enabled without a body", "…returned duplicate device-code settings" | 3.1.0 | org accounts | add (unannounced) | STATIC | N/A (login path; cyril never logs in) | no |
| 3.1.0 security lines: OTel tool-use export when only OTel export is on (logic; `KIRO_TELEMETRY_OTEL`/`KIRO_TELEMETRY_OTLP_ENDPOINT` 2/2, `telemetry-export-drops.json` 1/1, `KIRO_DISABLE_TELEMETRY` 2/2 unchanged); owner-only MCP registrations/credential folders (no new host literal; `owner_only_sweep.rs` since ≤2.32.0); [V2] `GIT_DIR`/`GIT_WORK_TREE` approval (`GIT_DIR` 0→2 host v2 shell_permission; `GIT_CONFIG_GLOBAL` 0→2 at 2.36.0) | 3.1.0 | — | fix | STATIC | cyril's direct spawn runs no host telemetry exporter; KAS-side telemetry is the kas lane's | NOTE cyril-zjm5 |
| Packaging: `install.sh` rewritten (PATH rc edits, legacy shell-integration removal + `kiro-cli _ cleanup-autocomplete-artifacts`, `--global`, `KIRO_CLI_NO_MODIFY_PATH`, root-into-user-HOME refusal, glibc ≥2.34 gnu / ≥2.26 musl); README rewritten; origin: no `.tar.xz` for 3.x, `.deb`/`.rpm`/`.zip`/`.tar.gz`/`.tar.zst`, Linux variant `headless` (unchanged label) | 3.0.0 | — | change | STATIC | HANDLED — install dirs `~/.local/bin` and `/usr/local/bin` are in cyril's resolver fallbacks (`auth.rs` `TUI_FALLBACK_DIRS`) | NOTE cyril-igrx (closed) |
| `acp --help`: "--auth-method is only supported with --agent-engine=v3" [0,1 3.1.0 text], `--sessions requires an interactive terminal…` ; `acp-client` hidden test client (`ACP test client … --agent <AGENT>`) present in both | 3.1.0 | — | text | STATIC | N/A | no |

## The 3.x binary — what merged, what was removed

**Identity (trap 20: decided by size/sha/sections/content, not BUILD_HASH).** `raw/sections.log`:

| binary | size | `.text` | `.rodata` | `.data.rel.ro` | source files (crates) |
|---|---|---|---|---|---|
| 2.32.0 `kiro-cli` (launcher) | 43,931,168 | 32,136,690 | 3,170,981 | 1,467,656 | 161 (`q_cli` 36, `fig_*`, `zbus` 27, `dbus`, `amzn-consolas-client`) |
| 2.32.0 `kiro-cli-chat` | 207,496,744 | 66,440,400 | 126,336,256 | 2,943,368 | 400 (`chat-cli` 168, `chat-cli-v2` 88, `agent` 62, `code-agent-sdk` 20, …) |
| 2.32.0 `kiro-cli-term` | 34,060,512 | 24,626,114 | 2,710,617 | 1,218,784 | 86 (`figterm` 16, `alacritty_terminal`, `fig_*`) |
| 2.37.1 `kiro-cli-chat` | 219,500,200 | 65,937,424 | 139,050,240 | 2,890,632 | 412 |
| 2.37.1 `kiro-cli` / `-term` | 43,030,048 / 33,380,960 | 31,577,826 / 24,160,610 | 3,154,597 / 2,698,329 | — | 160 / 84 |
| **3.0.0 `kiro-cli`** | **219,494,120** | 65,931,600 | **139,050,240** | 2,890,504 | **412** (= 2.37.1 chat; 0 added vs the 3-binary union) |
| 3.1.0 `kiro-cli` | 220,246,888 | 66,075,728 | 139,631,872 | 2,894,856 | 415 (+`v3_switch.rs`, +`default_settings.rs`, +`attempt_header.rs`, +controlplane `get_effective_enterprise_management_resource_with_token{,/builders}.rs`; −`v3_ease_in.rs`, −`rollout.rs`) |

The 3.0.0 binary differs from the 2.37.1 chat binary by 6,080 B total and `.text` −5,824 B; `BUILD_HASH=unknown` is a release-train artefact (3.1.0 has
`5acd3b95`). Payload frames: identical Node (v22.23.4, 124,902,584 B, sha `94efd062…`), Bun (1.4.4, 107,308,936 B, `10702245…`), TUI (= 2.37.1
`f9837a9b…` in 3.0.0; `de0a81d6…` in 3.1.0), KAS tar 188.9 MB (3.0.0) → 193.2 MB (3.1.0) — all other lanes' material. Frame order has been
tar, Bun, TUI, Node since 2.37.0 (`frames-<ver>.json`).

**Launcher → chat absorption timeline (clap help, `raw/help/*`, first-change attribution in `raw/help/help-chain.log`):**

| version | change in the chat binary (`kiro-cli-chat`) |
|---|---|
| 2.34.0 | clap `bin_name` becomes `kiro-cli` (every Usage line: `kiro-cli-chat …` → `kiro-cli …`); top-level `--agent <AGENT>` shorthand; `_ standalone-cutover`; new `cli/telemetry.rs`, `cli/uninstall.rs`, `cli/update/cutover.rs`, `launch/secret_storage.rs`; `doctor` stub text "`kiro-cli doctor` has been removed. Run `kiro-cli diagnostic` for system info, or `kiro-cli issue` to report a problem." [0,1] |
| 2.35.0 | `integrations` subcommand (install/uninstall/reinstall/status; `cli/integrations.rs`), `auth/mod.rs`; `chat --help` option reorder |
| 2.36.0 | `acp --help`: `[default: v2]` dropped, "Overrides the acp.agentEngine setting; v2 when neither is set" |
| 2.37.0 | `acp --login`; `_ extract-runtime` |
| 3.0.0 | launcher, term, `q`, `qchat` deleted from the tarball; the chat binary ships as `kiro-cli`; `--help` byte-identical to 2.37.1 chat `--help` |
| 3.1.0 | `--help`/`--help-all`/`acp --help`/`chat --help` byte-identical to 3.0.0 |

**Subcommand tree OLD vs NEW** (`raw/help/launcher-help-all-2.32.0.txt`, `raw/help/merged-help-all-3.1.0.txt`; hidden = accepted but not listed, probed with `--help`):

| 2.32.0 launcher (`kiro-cli --help-all`) | 3.1.0 (`kiro-cli --help`) | verdict |
|---|---|---|
| `debug`, `setup`, `theme`, `launch`, `quit`, `restart`, `translate`, `dashboard`, `inline`, `internal` (25 fig helpers: pre-cmd, local-state, callback, figterm-socket-path, inline-shell-completion, multiplexer, …) | `error: unrecognized subcommand` (rc=2) | **removed** = "inline shell autocomplete and the CLI desktop app are retired" |
| `doctor` | hidden stub: prints the removal message | removed (stub since 2.34.0) |
| `update` ("Update the Kiro application"; `-y`, `--relaunch-dashboard`, `--rollout`) | `update` ("Check for and install updates"; `--check`, `--force`) — the chat-side implementation | replaced |
| `integrations` (install/uninstall/reinstall/status) | `integrations` (same four; `integration` singular alias) — `cli/integrations.rs` since 2.35.0 | kept (reimplemented) |
| `init` ("Generate the dotfiles for the given shell") | hidden `init` ("Accepts and ignores the autocomplete bundle's init calls, which surviving shell integration lines still make on every new shell") | neutered |
| `settings`, `diagnostic`, `issue`, `login`, `logout`, `whoami`, `profile`, `user`, `chat`, `crew`, `mcp`, `powers`, `agent`, `acp` | all present (`user` = "Account commands, the same as the top-level login, logout, whoami and profile") | kept |
| — | `serve` ("Start a persistent WebSocket server for the v3 engine", `--port` 8082), `telemetry` (hidden: enable/disable/status), `uninstall` (hidden), `version` (hidden; `--changelog` remote), `acp-client` (hidden "ACP test client"), `_` (hidden installer helpers), `chat _` (hidden TUI IPC helpers) | chat-side, unchanged |
| `--classic` "Launch chat in classic (legacy) UI mode" | `--legacy-ui` "Launch chat in legacy UI mode [aliases: --classic]" (the chat binary's spelling since ≤2.32.0) | renamed |
| `--v3`, `--v2` "Use the V2 agent engine (the pre-3.0 default)", `--tui`, `--agent`, `--resume*`, `--cloud`, `--repo` | same | kept |

Removed from the merged binary's env table (merge record, `raw/env.log` "MERGE 2.37.1(launcher+chat+term) -> 3.0.0(single): +1 -109"): all `Q_*`
shell-integration names (`Q_SET_PARENT`, `Q_USER_PS1…`, `Q_PROMPT_*`, `Q_TERM_*`, `Q_SHELL_INTEGRATION_DISABLED`, `Q_DOTFILES_SOURCED`, …), the 14
`KIRO_CLI_AUTOSUGGEST_*` names, `KIRO_CLI_USING_ZSH_AUTOSUGGESTIONS`, `XDG_CURRENT_DESKTOP`/`XDG_SESSION_DESKTOP`/`XDG_DATA_DIRS`, `AWS_SECURITY_TOKEN`,
`RUST_SPANTRACE`. Removed launcher-only crates (`raw/deps-launcher-merge.log`): `clap_complete`, `clap_complete_fig`, `tokio-tungstenite`/`tungstenite`,
`wayland-*`, `wl-clipboard-rs`, `zvariant`, `prost-reflect`/`prost`, `dircpy`, `tree_magic_mini`, `iri-string`, `async-*` executor family (35 crates).

**What remains of the launcher's namespaces:** `integrations` and `update` (chat-side reimplementations), `init` (no-op), `_` (installer helpers).
`internal` is gone entirely; nothing from `crates/q_cli` or `crates/fig_*` survives as a source path (0 launcher-only or term-only paths kept).

**What remains of Classic/v2:** the V2 engine (`crates/chat-cli-v2`, 91 source files in 3.1.0, up from 88) and its `_kiro.dev/*` dialect are intact;
`--agent-engine v2` and `--v2` parse on both `acp` and `chat`. "Classic" (`--legacy-ui`/`--classic`, `--agent-engine v1`) parses but is ignored at
launch: "Kiro CLI 3.0 no longer runs Classic (the v1 engine), so this run ignores the Classic selection." The `chat --help` text still reads
`--agent-engine … "v1", "v2" (default), or "v3"` and the `chat.agentEngine` description "'v1', 'v2' (default), or 'v3'" — stale help, since the launch
path now persists `chat.agentEngine` through the V3 switch prompt (see §KAS auth path and acp engine selection).

## KAS auth path (b) and `acp` engine selection (c)

### (b) `kiro-cli chat _ get-kas-token` — PASS, no change in 13 releases

- Help text (`raw/help/merged-chat-gkt-3.1.0.txt` vs `chat-chat-gkt-2.32.0.txt`): identical except `Usage: kiro-cli-chat chat _ get-kas-token` →
  `Usage: kiro-cli chat _ get-kas-token` (clap `bin_name` change at 2.34.0). Options unchanged: `--reason <REASON>`, `--current-expires-at`,
  `--force-refresh` ("Force a network token refresh even when the cached token looks valid. Set by the TUI host handler when KAS sends forceRefresh on
  the getAccessToken covenant during mid-turn 401 recovery"). The subcommand sits under `chat _` in all 14 binaries; the top-level `_` is a different
  namespace (installer helpers) in all 14 — the TUI's and cyril's argv `["chat","_","get-kas-token"]` is correct on 3.x.
- Host literals, 14-version series (`raw/first.py`): `get-kas-token` 1, `getKasToken` 1, `force-refresh` 1, `forceRefresh` 2, `_kiro/auth/getAccessToken` 2,
  `accessToken` 4, `expiresAt` 3, `profileArn` 13, `authMethod` 5, `kas_token.rs` 3, `acp-callback` 2, `--auth=` 2 — **no change in any release**. The
  logged-out envelope text is unchanged too: "You are not logged in. Please log in with `kiro-cli login`." (beside `kirocli:auth:last-forced-refresh-at`).
- TUI 3.1.0 (`~/.local/share/kiro-research/tui-bundles/kiro-tui-3.1.0.js`, handler `$Wr`, the 2.32.0 `Rsr`): `let n=["chat","_","get-kas-token"];
  if(e?.forceRefresh)n.push("--force-refresh")` … `if(r.output.kind==="error")throw …` … `if(r.output.kind!=="getKasToken")throw …` … returns
  `{accessToken, expiresAt, profileArn, ...authMethod?, ...provider?}`; wired as `method:"_kiro/auth/getAccessToken"`. Byte-for-byte the 2.32.0 logic.
- **cyril verdict: HANDLED, no action.** `crates/cyril-core/src/protocol/kas/auth.rs:44` `GET_KAS_TOKEN_ARGS = ["chat","_","get-kas-token"]`, `:46`
  `--force-refresh`, envelope parse (`kind` `getKasToken`/`error`, `data.{accessToken,expiresAt,profileArn,authMethod?,provider?}`) matches. The
  `HelperMissing` fallback (`auth.rs` `KasTokenError::HelperMissing`, stored-token read in `kas/stored_token.rs`) will not trigger on 3.x.
  Residual risk for the live lane only: the 3.1.0 binary opens `data.sqlite3` with newer migrations (`Applying migration` / `INSERT INTO migrations`)
  — see HOST-L4 for the single-flight / copy-the-store caveat.

### (c) `kiro-cli acp` on 3.x — flag values, setting, router, spawn, sign-in

**Flag values.** `acp --agent-engine <ENGINE>`: `[possible values: v2, v1, v3]` in 2.32.0 and 3.1.0 alike; `classic`/`kas` are refused by clap
(`error: invalid value 'classic' for '--agent-engine <ENGINE>'`, rc=2; `raw/help/merged-acp-engine-classic-3.1.0.txt`). `v2` parses on 3.0.0/3.1.0
(rc=0). `v1` parses but the launch path has `internal error: entered unreachable code: V1 engine does not use the ACP launch path` (all 14) — a live
leg can tell whether `acp --agent-engine v1` panics or is remapped (HOST-L5, low).

**Default and precedence (2.36.0).** Help: "Agent engine to use: "v1", "v2", or "v3". Overrides the acp.agentEngine setting; v2 when neither is set".
Setting description: "Agent engine for `kiro-cli acp`: 'v2' (default) or 'v3' (string)". Tracing: "ignoring unrecognized acp.agentEngine setting".
BUT the router's embedded comment says the default can be v3: "When the `kiro-cli acp` default is v3 (no --agent-engine flag, no acp.agentEngine
setting), a client listed here stays on v2 unless it is also in v2_exempt_clients.txt." Three facts make the v3 default the working hypothesis for 3.x
[INFERENCE, STATIC]: (1) 3.0.0 announces "V3 is now the default experience"; (2) `chat.agentEngine` is written by the switch flow ("Could not save V3 as
your default. To switch manually, run: kiro-cli settings chat.agentEngine v3"; "# is your default (chat.agentEngine="), so a user who accepted the
2.34.2+ prompt already has v3 persisted; (3) Kiro's own TUI, which drives the V2 engine over `kiro-cli acp`, began pinning the flag in the same
release: `t.some((b)=>b==="--agent-engine"||b.startsWith("--agent-engine="))?[]:["--agent-engine=v2"]; l=c(e,["acp",...A,...t]` (TUI 2.36.0+; 0
in ≤2.35.0). A client that trusted the v2 default would not need that pin.

**The router (`crates/chat-cli/src/cli/acp_engine.rs`, 2.36.0; line numbers :65/:75/:91 → :95/:112/:128 in 3.1.0).** Strings: "failed to read the
ACP initialize request", "ACP initialize peek task failed", "resolved kiro-cli acp engine". Two embedded text files (`raw/acp-clients-header.txt`;
bulk in `tmp/acp-clients-<ver>.txt`):
- `acp_clients` snapshot: "# Snapshot of ACP clients seen in telemetry: the acp_client_name dimension on KiroCLI metrics (KUTS prod, last ~14 days as
  of 2026-10-27). One sanitized clientInfo.name per line, matched case-insensitively; a trailing `*` matches a prefix." — **1,080 rows in every build
  2.36.0 → 3.1.0** (frozen snapshot; e.g. `acp-probe`, `claude-code-acp-driver`, `codex-kiro`, `kiro-acp*`, `JetBrains.*`, `CodeCompanion.nvim`).
- `v2_exempt_clients.txt`: "# … Add a client here once it is confirmed to work on v3. # Owners and reasons per client: docs/oncall/acp_clients.md." —
  6 names in 2.36.0 (`CodeCompanion.nvim`, `avante.nvim`, `Agentic.nvim`, `obsidian-agent-client`, `JetBrains.*`, `hermes-agent`), **0 names in
  2.37.1, 3.0.0, 3.1.0** (every client that was "confirmed to work on v3" went back to v2 — or the exemption mechanism was abandoned).
- **`cyril` appears in neither list** (case-insensitive grep over all four builds). cyril's wrapper identity is always `cyril`: `protocol/identity.rs:22-27`
  forces `PresentAs::Cyril` on V2; `bridge.rs:489-490` asserts `client_info(PresentAs::Cyril).name == "cyril"`.

**What this means for cyril, by spawn path (`crates/cyril-core/src/protocol/bridge.rs:366-394`):**
- Free path (KAS default): direct `node … acp-server.js --transport=stdio --auth=acp-callback` — no `kiro-cli acp`, router never runs. Unaffected.
- Wrapper, `AgentEngine::Kas`: `kas/version.rs:343-364` appends `--agent-engine v3` (`flag_for_version` ≥2.8.0). Unaffected.
- Wrapper, `AgentEngine::V2` (`--agent-engine v2` / `[agent] engine = "v2"`): `bridge.rs:394` `AgentEngine::V2 => Ok((agent_command.clone(), None))`
  — the bare `kiro-cli acp` from `crates/cyril/src/main.rs:32-36`. If the 3.x default resolves to v3 for an unlisted client, cyril's V2Engine receives
  KAS; `fingerprint.rs:92-96` fails loud with "engine mismatch: cyril is driving the v2 engine but …; restart cyril with `--agent-engine kas`, or spawn a
  v2 agent (`kiro-cli acp`)" — the second remedy is exactly the command that produced the mismatch. **Fix shape (P2 below):** append `--agent-engine v2`
  to the V2 wrapper spawn for kiro-cli ≥2.7.1 (the value has been accepted since the flag existed), or pass `acp.agentEngine` is NOT an option for cyril
  (user-global setting). Also: if the user has pinned `acp.agentEngine=v3` or the router decides v3, cyril's `--agent-engine v2` flag beats both (flag >
  setting), so the pin is sufficient.

**How `kiro-cli acp` reaches the node KAS process (unchanged 2.32.0 → 3.1.0; `raw/ctx.py '--auth='`):** stdio path: "spawning KAS process" at
`cli/mod.rs:1413` with `--transport=stdio`; the `serve`/WS path: `--experimental-wasm-modules --transport=ws --auth=acp-callback` + `ACP_WS_PORT`;
env passed: `KIRO_CUSTOM_USER_AGENT`, `KIRO_TELEMETRY_IS_INTERNAL_AMAZON`, `KIRO_CLI_IS_CHILD_PROCESS`, `KIRO_REMOTE_SESSIONS_ENDPOINT`,
`CLOUD_CONFIG_ENDPOINT`; overrides `KIRO_KAS_SERVER_PATH` / `KIRO_KAS_NODE_PATH` ("Cannot resolve the server for the v3 engine: KIRO_KAS_SERVER_PATH is
not set and the embedded server is unavailable"); `--auth-method cli|client` (default `cli`; "--auth-method is only supported with --agent-engine=v3").
3.1.0 adds "log file not shared with the engine: only its owner may write its directory" [0,1] (log-dir ownership check before handing KAS the log
path) and "--login is not supported with --auth-method=client, which leaves sign-in to the ACP client" [0,1]. cyril's free spawn argv
(`discovery.rs` `--experimental-wasm-modules <server> --transport=stdio --auth=acp-callback`) still mirrors the host's non-path flags.

**ACP sign-in from a terminal (2.37.0).** Host side: `acp --login` ("Log in interactively and exit, for ACP clients that run terminal authentication"),
new source `crates/chat-cli/src/cli/acp_sign_in.rs` [0,1], JSON-pointer literals `/clientCapabilities/_meta/terminal-auth` [0,1] and
`/capabilities/auth/terminal` [0,1], the pre-existing serde shape `struct AuthMethodTerminal with 6 elements` (3/3; = ACP schema `AuthMethod::Terminal
{ id, name, description, args, env, _meta }`, feature `unstable_auth_methods`, `agent-client-protocol-schema-1.5.0/src/v1/agent.rs:568-582,920-950`:
"The client runs an interactive terminal for the user to authenticate via a TUI … args: Additional arguments to pass when running the agent binary").
Reading [INFERENCE, STATIC]: when `kiro-cli acp` starts logged out ("Os::new: failed to load the login token, continuing logged out") and the client
declared `clientCapabilities._meta["terminal-auth"]`, the `initialize` result's `authMethods` carries a terminal method whose `args` point at `acp --login`
(the agent binary is the client's own `kiro-cli` argv); the client runs it in a terminal, then calls `authenticate`. The live lane can capture the exact
shape at zero cost with a fresh HOME (HOST-L3). **cyril:** advertises no `_meta.terminal-auth` (`engine.rs:128-160` builds `_meta.kiro.{settings,
openExternalUrl,hooks}` only) and never reads `authMethods` → DROPPED; on the free path KAS asks cyril for the token instead, so this is wrapper-only
(cyril-cxd2 territory: today a logged-out wrapper start surfaces as the C14a gate / `get-kas-token` error envelope, not as an ACP auth method).

**3.1.0 "sign-in refusal"** ("A prompt is refused with a sign-in message, and not saved, when Kiro cannot get a sign-in token"): no new host literal; the
host's envelope message is unchanged, so the refusal lives in KAS/TUI (not in my lane). Related host-only 3.1.0 addition: controlplane
`GetEffectiveEnterpriseManagementResourceWithToken` ("Resolved device-code sign-in setting") — org-managed device-flow policy fetched with the bearer
token (login path; unannounced; cyril never logs in).

## Runtime extraction and data dir (d)

**Default path is unchanged (STATIC, 14/14):** `embedded_tui.rs` strings "KAS bundle already extracted", "KAS extraction lock timed out; extracting
without dedup", "extracted KAS node runtime", "failed to record KAS version heartbeat before extraction", the sqlite migrations
`009_extracted_kas_versions` / `010_pinned_bin_versions` (`CREATE TABLE extracted_kas_versions (`, `CREATE TABLE pinned_bin_versions (`) and the asset
sha256 constants (node `94efd062…`, bun `10702245…`, tui `de0a81d6…` in 3.1.0) are all present with identical counts. Nothing in the host names a new
default root; `KIRO_DATA_DIR` 3/3. So the self-extracted layout cyril discovers — `<data dir>/kas/<MAJOR.MINOR.PATCH>-<sha256>/node_modules/@kiro/agent/
dist/server/acp-server.js`, `<data dir>/node`, `<data dir>/bun`, `<data dir>/tui.js` — should still be what a 3.1.0 run produces (LIVE to confirm:
HOST-L2 records the tree after the first 3.1.0 run on a scratch data dir).

**The opt-in alternative (2.37.0):** `KIRO_RUNTIME_DIR` ("sets where Kiro extracts its bundled runtime instead of the home directory") and the installer
helper `kiro-cli _ extract-runtime [--dir <DIR>]` — "Install the embedded runtime into a shared directory and remove other versions there"; "Runtime root.
Defaults to KIRO_RUNTIME_DIR, else <prefix>/lib/kiro-cli beside this binary" (`raw/help/merged-underscore-extract-runtime-3.1.0.txt`). Source
`crates/chat-cli/src/embedded_tui/runtime_dir.rs` [0,7] (tracing at :102/:180/:238/:310): "ignoring KIRO_RUNTIME_DIR: not an absolute path" /
"KIRO_RUNTIME_DIR must be an absolute path, got", "runtime dir install lock unavailable; installing without it", "cannot lock the runtime dir; pruning
disabled for this process", "embedded runtime has an asset without its SHA256; runtime dir disabled", "this build carries no embedded runtime to
extract", "runtime install vanished", "failed to write bun/tui.js/node", "failed to unpack the KAS bundle"; path tokens glued in rodata:
`lib` `kas` `node_modules` `@kiro` `acp-server.js` `bun` `tui.js` `node`, plus an `.owner` / `.complete` marker pair. Layout [INFERENCE]: a per-version
subtree under the runtime root holding `node`, `bun`, `tui.js` and `lib/kas/node_modules/@kiro/agent/…/acp-server.js`, with `.owner`/`.complete` markers and
pruning of sibling versions ("remove other versions there") — not the data dir's `kas/<ver>-<sha>` naming. The exact tree is HOST-L2 (zero-cost, no auth).
"Stale extracted V3 engines" cleanup: no new 'stale'/'prune' literal in the data-dir path (`stale` 29/29); the only new pruning text is the runtime-dir
one above, so the cleanup the changelog implies is the runtime-dir "remove other versions" plus the pre-existing heartbeat-based data-dir cleanup.

**cyril verdict:** `crates/cyril-core/src/protocol/kas/discovery.rs:197-200` reads `KIRO_DATA_DIR`, `KIRO_KAS_SERVER_PATH`, `KIRO_AGENT_PATH`;
`:61-122` resolves `<data dir>/kas/<ver>-<sha>/…` with `dir_version` (exact `MAJOR.MINOR.PATCH-<64 hex>`). Default installs: HANDLED. A user or
distribution that sets `KIRO_RUNTIME_DIR` or runs `_ extract-runtime` (system package into `<prefix>/lib/kiro-cli`) gets `KasMissing::Server` with the
remedy "Run `kiro-cli acp --agent-engine v3` once to self-extract it, or set KIRO_KAS_SERVER_PATH" (`discovery.rs:50-53`) — the first half of that
remedy is wrong under a runtime dir (the host would extract into the runtime dir again). → P3 issue below.

## Env and settings set diffs (e, f)

**Env (`static-host-env-3.1.0.py` → `raw/env/host-env-delta.json`, `raw/env.log`; every add/remove re-counted; glue rejected: `AWS_MAX_ATTEMPTS*`,
`AWS_EXECUTION_ENVSSH_*`, `KIRO_DEBUG_SHELLC`, `KIRO_CLI_IS_CHILD_PROCESSW`, `KIRO_KAS_SERVER_PATH8KAS`, `RUST_MIN_STACK[FR]`, `Q_SHELLSHELLN`):**

| name | first | last | where / meaning (host prose) | cyril |
|---|---|---|---|---|
| `KIRO_STANDALONE_NOTICE` | 2.33.1 | 3.0.0 (−3.1.0) | standalone-channel cutover notice toggle (desktop app → standalone) | unread |
| `KIRO_TEST_SYSTEM_ROOT` | 2.34.0 | — | test hook (system root for integrations) | unread |
| `KIRO_TRUST_CLASSIFIER_MODE` | 2.34.0 | — | launcher → TUI: "the launcher exports KIRO_TRUST_CLASSIFIER_MODE=enforce (unless the user set it)" (Approve for me) | unread; cyril's spawn exports nothing → KAS default mode |
| `KIRO_ELEVATED_PRIVILEGES` | 2.35.0 | — | launcher → TUI root/Administrator warning | unread |
| `KIRO_ROUTER_VERBOSE`, `KIRO_ROUTER_VERSION` | 2.35.0 | — | command-router (`/usr/local/bin/kiro`) plumbing | unread |
| `KIRO_AGENT_UPGRADE_NOTICE`, `KIRO_LAUNCH_3_0` | 2.37.0 | — | launcher → TUI: V2-agent auto-upgrade notice; 3.0 launch prompt/notice state | unread |
| `KIRO_RUNTIME_DIR` | 2.37.0 | — | shared runtime root (see §d) | **unread** (P3) |
| `KIRO_DEFAULT_SETTINGS`, `KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK` | 3.1.0 | — | root-owned defaults file; owner check bypass | **unread** (P4) |
| `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED` | 3.1.0 | — | KAS operator override for the (0%) background-tool gate; replaces `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED` (−3.1.0) | unread (dark) |
| −`KIRO_STANDALONE_CUTOVER` (2.32.0 add) | — | 3.0.0 | removed 3.1.0 | — |
| −`KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED`, −`KIRO_ROLLOUT_FORCE_NIGHTLY` | — | 3.0.0 | removed with the graduated rollout rows | — |
| unchanged | | | `KIRO_HOME` 7→9 (2.34.0: `secret_storage.rs` "copied the legacy secret store into KIRO_HOME"), `KIRO_DATA_DIR` 3, `KIRO_KAS_SERVER_PATH` 2, `KIRO_KAS_NODE_PATH` 2, `KIRO_AGENT_PATH` 0 (cyril-only name), `KIRO_DISABLE_TELEMETRY` 2, `KIRO_TELEMETRY_OTEL` 2, `KIRO_TELEMETRY_OTLP_ENDPOINT` 2, `KIRO_ACP_RECORD_PATH` 0 (TUI), `KIRO_ROLLOUT_FORCE_INTERNAL` present, `KIRO_INFRA_SAFETY_ROLLOUT_ENABLED` 3→2 | cyril reads `KIRO_HOME`/`KIRO_DATA_DIR`/`KIRO_KAS_SERVER_PATH`/`KIRO_AGENT_PATH` only |

Merge: −109 names from the launcher/term (shell integration / autosuggest / desktop), +0 host names. Token totals (host role): 230 → 234 (2.35.0) → 235
(3.0.0) → 231 (3.1.0); launcher 149–152 and term 63–69 until deletion.

**Settings keys (`static-host-settings-3.1.0.py`, `raw/settings.log`): 124 → 124 → 124 → 126 (2.34.0) → 127 (2.35.0) → 127 (2.36.0, +1 −glue) → 128
(2.37.0) → 128 → 129 (3.1.0).** Descriptions from the glued description table (`raw/ctx.py`):

| key | first | description / reader |
|---|---|---|
| `standalone.autoEnroll` | 2.33.1 (table entry gone 3.1.0; prose remains in telemetry docs: "opted_out when the user set standalone.autoEnroll=false") | macOS desktop → standalone cutover opt-out |
| `chat.modelPicker.hiddenModels` | 2.34.0 | `/model edit` hidden models (TUI picker state) |
| `chat.workflowLaunch` | 2.34.0 | "Workflow use for new KAS workflow sessions: when-asked, large-tasks, or delegate-all (string)" — the /settings "Workflow use" control (feed 2.34.0 #5) |
| `install.releaseUrl` | 2.34.0 | update manifest override, read by `cli/update/mod.rs` ("failed to read the settings file for install.releaseUrl") |
| `app.warnOnElevatedPrivileges` | 2.35.0 | "Warn when execution sessions run as root or Administrator (boolean, default: true)" |
| `acp.agentEngine` | 2.36.0 | "Agent engine for `kiro-cli acp`: 'v2' (default) or 'v3' (string)" |
| `chat.launch30` | 2.37.0 | "Opt this machine into the Kiro CLI 3.0 launch behavior before it ships (boolean)" |
| `chat.enableSessionRecap` | 3.1.0 | "Show a short recap of the session above the prompt after the agent has been idle for a few seconds" (doc node `settings/session-recap.md`, keywords incl. `kas`) |
| unchanged of interest | | `chat.enableAutoAgentUpgrade` "… to the V3 format on V3 launches (boolean, default: true). When false, interactive V3 launches show a notice while any are pending." (2/2 → 3 at 2.37.0: one more reader); `chat.agentEngine` "Default agent engine: 'v1', 'v2' (default), or 'v3' (string)" (stale text); `chat.terminalTitle` 4/4; `telemetry.enabled` 2/2 |

cyril verdict: cyril reads `cli.json` only for the workflow/hook/trust family (`crates/cyril-core/src/protocol/kas/settings.rs:65-112`, `BOOL_MAP`
`:78-88`); none of the new keys is read or forwarded. `chat.workflowLaunch` is a KAS session setting Kiro's TUI forwards under
`_meta.kiro.settings` — whether cyril should mirror it is a kas-lane/coverage question (cross-ref), not a host finding.

## Rollout registry (g)

`static-rollout-3.1.0.py` over 2.32.0 → 3.0.0 (`raw/rollout/rollout.json`, `rollout-delta.json`; the 3.1.0 table lost its `tui` anchor and was
extracted by brace-matching every `"treatment_percent"` → `raw/rollout/rollout-3.1.0.json`). Full per-version table: `raw/rollout-table.md`.
Highlights (a percentage flip is a release-behaviour change; all rows "segment all" unless stated):

| row | 2.32.0 | changes | 3.1.0 |
|---|---|---|---|
| `v3_prompt` ("Who is offered the V3 ease-in prompt") | 10% / 75% internal | 25%/100%i (2.33.1) → 50% (2.34.0) → **100% (2.34.2** = "Kiro CLI now offers to switch you to V3 when you start a chat") | **row removed** (prompt unconditional; `v3_ease_in.rs` deleted, `v3_switch.rs` added) |
| `standalone_toolbox` (autocomplete decoupling cohort) | 0% | 5 (2.34.0) → 20 (2.34.1) → 50 (2.35.1) → **100 (2.35.2** = "Retiring autocomplete desktop app - gradual rollout"); segment all → none (2.37.0) | removed |
| `standalone_public` (macOS desktop → standalone cutover) | — | added 0% (2.34.0) → 10 (2.35.0) → 30 (2.35.2) → 50 (2.36.0) → **100 (2.37.0)** | removed |
| `trust_classifier_enforce` ("Approve for me … KIRO_TRUST_CLASSIFIER_MODE=enforce") | — | added 100% internal/nightly (2.34.0) → internal/all channels (2.34.1) → **all users (2.36.0** = "Approve for me" line) | removed (unconditional) |
| `model_fallback_auto_refusal` ("Re-send a turn refused on Auto…") | — | added 100% internal (2.34.0) → **100% all (2.35.0** = "A refused response on Auto is now retried") | kept |
| `memory_cloud_sync` ("KAS-only memory cloud sync in /memories: … memorySync at user-synced and user-local, read and written through KAS's configuration methods … Dark-shipped at 0%") | — | — | **added 0% internal/nightly** (dark; opt-in `KIRO_ENABLED_FEATURES='["memory_cloud_sync"]'`) |
| `background_execution` | 0% int/nightly | description rewritten 3.1.0: TUI sends `universalBackgroundTool: { enabled: true }` on initialize, session/new and session/load (was `backgroundExecution` on session/new) | 0% (dark) |
| `memory_controls` | 100 | description +deletion (2.34.0 = "Delete memories you no longer want from /memories") | kept |
| `managed_settings` | 0 int/nightly | description extended (2.35.0, `_kiro/governance/state` subscription) | 0 (dark) |
| removed at 3.1.0 as graduated GA rows | `tui` 50% int, `voice` 100, `workflows` 100, `local_sandbox` 100, `remote_sandbox` 100, `session_dashboard` 100, `tangent` 100, `remote_changelog` 100 stable | | all gone (12 rows incl. the three above) |
| unchanged | `cloud_config` 100 all, `infra_safety` 100 int, `kvim` 100 int, `lite` 100 int, `memory` 100 all, `c2s` 100 int/nightly, `trust_classifier_shadow` 100 int/nightly, `test*` | | kept |

Overrides table (`rollout_overrides`): `standalone_toolbox.treatment` cohort list 7 → 13 ids (2.33.0); `v3_prompt.treatment` 135 ids unchanged; the whole
override object is gone in 3.1.0 (`rollout_overrides` 2→0). `crates/chat-cli/src/rollout.rs` path 0 in 3.1.0 (the table is compiled from another
module; `KIRO_ENABLED_FEATURES`/`KIRO_ROLLOUT_FEATURES`/`KIRO_ROLLOUT_FORCE_INTERNAL` remain). cyril: NOT-SENT — these are `kiro-cli chat` → TUI
decisions; cyril's spawns carry none of `KIRO_ENABLED_FEATURES`.

## Source paths and Classic retirement (h, i, j)

**Source-path inventory (host role; `static-host-srcloc-3.1.0.py`, `static-host-paths-3.1.0.py`):** 400 (2.32.0) → 415 (3.1.0) files; per crate 2.32.0
→ 3.1.0: `chat-cli` 168 → 177, `chat-cli-v2` 88 → 91, `agent` 62, `code-agent-sdk` 20, `amzn-kiro-controlplane-bearer-rust-client` 8 → 10,
`amzn-codewhisperer-client` 13 → 14, others unchanged.

| version | files added (host) | files removed |
|---|---|---|
| 2.33.0 | `amzn-codewhisperer-client/src/endpoint_lib/host.rs` | — |
| 2.33.1 | `chat-cli/src/launch/v2_builtin_default_agent.rs` ("Resuming a V2 session no longer warns that agent `default` was not found" shipped 2.35.0 on this file) | — |
| 2.34.0 | `chat-cli-v2/src/agent/kas/export.rs` (V2↔KAS session zip export), `chat-cli/src/cli/update/cutover.rs`, `chat-cli/src/launch/secret_storage.rs`, `chat-cli/src/cli/telemetry.rs`, `chat-cli/src/cli/uninstall.rs` | — |
| 2.35.0 | `chat-cli/src/auth/mod.rs`, `chat-cli/src/cli/integrations.rs`, `chat-cli/src/util/../../../chat-cli-v2/src/util/open.rs` | `chat-cli/src/util/open.rs` (moved) |
| 2.36.0 | `chat-cli/src/cli/acp_engine.rs` | — |
| 2.37.0 | `chat-cli/src/cli/acp_sign_in.rs`, `chat-cli/src/embedded_tui/runtime_dir.rs`, `chat-cli/src/launch/v3_launch_prompt.rs` | — |
| 3.0.0 | — | — (host identical to 2.37.1 chat up to line shifts) |
| 3.1.0 | `chat-cli/src/launch/v3_switch.rs`, `chat-cli-v2/src/database/default_settings.rs`, `chat-cli-v2/src/api_client/attempt_header.rs`, controlplane `get_effective_enterprise_management_resource_with_token{,/builders}.rs` | `chat-cli/src/launch/v3_ease_in.rs`, `chat-cli/src/rollout.rs` |
| launcher 2.34.0 | `fig_install/src/standalone_cutover.rs`, `fig_install/src/update_lock.rs` | `fig_proto/src/mux.rs` |
| merge 3.0.0 | — | 145 launcher-only + 71 term-only files (crates `q_cli`, `fig_api_client`, `fig_auth`, `fig_aws_common`, `fig_diagnostic`, `fig_install`, `fig_integrations`, `fig_ipc`, `fig_log`, `fig_os_shim`, `fig_proto`, `fig_remote_ipc`, `fig_request`, `fig_settings`, `fig_telemetry`, `fig_telemetry_core`, `fig_util`, `zbus`, `dbus`, `amzn-consolas-client`, `figterm`, `alacritty_terminal`) |

None of the brief's guessed names (`trust*.rs`, `elicit*`, `credits*`) exists as a host source path in any of the 14 builds (the trust prompt,
elicitation and credits live in KAS/TUI; the host's MCP elicitation support is the pre-existing `elicitation` 16/16 in the v2 MCP client).

**Serde shapes of note (`static-host-serde-3.1.0.py`, `raw/serde.log`; the crew `SessionTool::*`/`TestCommand::*` variants flap between builds and are LTO
noise):** 2.33.0 +`struct CommandsAvailableNotification [5]`, +`McpServerAdvertisement [3]`, +`ToolAdvertisement [3]`, +`PromptInfo [4]`,
`AvailableCommand` 4 → {3,4}, +`WireHookAction::Command [1]` (v2 wire); 2.36.0 +`struct V3PermissionRule [4]`, +`struct DetectorConfig [9]`
(agent-upgrade analysis); 2.37.0 untagged `McpServer{,Http,Sse}` gains `AuthEnvVar`/`secret`/`AuthMethodEnvVar` arms (MCP env-var auth), `SessionConfigOption`
category gains `SessionAdditionalDirectoriesCapabilities`; 3.1.0 +`LoopError::InvalidJson [3]`.

**Classic retirement strings (i), first versions:**
- ≤2.32.0 (unchanged 3/3): "Classic is being deprecated" (…"with the Kiro CLI 3.0 release in October. V3 runs in the terminal UI and also brings
  workflows, cloud sessions, specs, and more."), `classic_nudge.rs`, `--legacy-ui` "Launch chat in legacy UI mode", "Preparing V3...".
- 2.34.2: `v3_prompt` 50 → 100% — the "offers to switch you to V3 when you start a chat" line is a rollout flip, not new code.
- 2.37.0: `launch/v3_launch_prompt.rs` ("Kiro CLI 3.0 is here. The new V3 agent brings workflows, simplified permissions, cloud sessions, sandbox, specs,
  and more. Learn more: https://kiro.dev/docs/cli/v3/V2 no longer receives new features and will be removed in January."; options "Stay on V2, remind me
  later" / "Stay on V2, don't ask again" / "Switch to V3" / "Keep V2"); `classic_nudge.rs` grows ("Kiro CLI 3.0 is here. It runs in the terminal UI and
  brings workflows, simplified permissions, cloud sessions, sandbox, specs, and more. The Classic experience is no longer supported.", "Kiro CLI 3.0 is
  here, and the Classic experience is no longer supported. This machine can't run the 3.0 agent, so the terminal UI opens with the previous agent.",
  "Kiro CLI saved your engine, but the restart still selected Classic. Run kiro-cli again to open the terminal UI."); `cli/mod.rs` launch decision
  "Kiro CLI 3.0 no longer runs Classic (the v1 engine), so this run ignores the Classic selection." and "Kiro CLI 3.0 no longer includes Classic, and this
  install is missing the terminal UI. Reinstall Kiro CLI: https://kiro.dev/docs/cli/installation/"; telemetry schema `v3_prompt_source` adds `launch`
  ("3.0 launch prompt for existing V2 users who never pinned an engine") and `launch_notice` ("for users who pinned V2 themselves"); `--agent-engine v1`
  literal next to the deprecation copy; `stopping (exit 4)` (`--no-interactive` stop on an agent); "V2 TUI keybinding…" descriptions.
- 3.1.0: `v3_ease_in.rs` removed ("You're using Kiro CLI V2. Switch to V3 today." 1 → 0), `v3_switch.rs` added; `agent upgrade` adds "V3 has no tool for
  these V2 tools, so V3 ignores them. V2 still runs them, so the agent keeps them." and drops `todo_list` from the kiroUpgrade tool list (`todo_list` 12/12
  overall — other readers remain).
- Never present: "retired"/"Retired" 0 in every host build — the changelog's word is not in the binary.

**v2 `_kiro.dev/*` surface (j):** 40 → 41 literal occurrences at 2.33.0 then flat; `static-host-methods-3.1.0.py` (`raw/methods.log`) finds every full
method token count-identical 2.32.0 → 3.1.0 (`_kiro.dev/commands/execute`, `/options`, `/available`, `/model/options`, `/agent/options`,
`/prompts/options`, `_kiro.dev/mcp/*`, `_kiro.dev/telemetry/*`, `_kiro.dev/session/terminate`, …); the only add is a bare `_kiro.dev/command` prefix
literal (6 → 7; the token `kiro.dev/commandMismatchedKmsKey` is that literal glued to the next rodata string) — a `starts_with` guard, not a method.
Other rows in `methods.log` are glue (`kiro.dev/account/usageEstimated`, `kiro.dev/docs/cli/installation/Cleaning`, `kiro/secH`, `kiro/openExterna…`,
`kiro/skills/allowedTools`, `kiro/worH/I`) — the underlying `_kiro/skills/*`, `_kiro/secret/*`, `_kiro/openExternalUrl` literals count 0 in both (they
belong to KAS/TUI). Verdict for `crates/cyril-core/src/protocol/convert/kiro.rs`: the v2 dialect it converts is intact in 3.1.0; the engine behind it is
dated for removal "in January" (host prose), so the file's horizon is the v2 engine's, not 3.0's.

## Doc-manifest delta (k)

`static-doc-manifests-3.1.0.py` (`raw/docman.log`, `raw/docman/doc-manifests-{0..13}.json`): both embedded manifests are byte-equivalent from 2.32.0
through 3.0.0 (`generated_at` 2026-10-12 / 103 docs and 2026-09-28 / 140 docs); 3.1.0 regenerates both (2026-11-04T00:59 / 105; 2026-11-03T11:57 /
142). Delta keyed by `path`:
- added `features/default-settings.md` — "Default settings (KIRO_DEFAULT_SETTINGS)", `feature`, validated 2026-11-02, description "Load a root-owned,
  read-only file of default settings that your own cli.json and workspace settings override", keywords `managed settings`, `cloudshell`, `precedence`
  (→ feed 3.1.0 #2; the `cloudshell` keyword names the deployment target).
- added `features/terminal-program-status.md` — "Terminal program status", `feature`, **status draft**, validated 2026-10-13, "OSC 7501 program status
  reports for the hosting terminal" (→ feed 2.32.0 #2, already announced; first appearance in a manifest).
- added `settings/session-recap.md` — `chat.enableSessionRecap`, `setting`, validated 2026-11-03 (→ feed 3.1.0 #5).
- changed: `commands/agent.md` (+upgrade/universal/migration/dry-run keywords), `slash-commands/memories.md` (+delete, validated 2026-10-19),
  `slash-commands/model.md` (+hide/edit), `commands/settings.md` and `slash-commands/settings.md` (validated 2026-11-02).
- removed 0. No `category:feature` node without a changelog line → no unannounced feature this cycle (trap 12); sitemap check not needed.

## Packaging (m)

| item | 2.32.0 … 2.37.1 | 3.0.0 / 3.1.0 |
|---|---|---|
| tarball contents | `kirocli/bin/{kiro-cli,kiro-cli-chat,kiro-cli-term,q,qchat}`, `install.sh`, `README`, `BUILD-INFO` | `kirocli/bin/kiro-cli`, `install.sh` (11,980 B), `README`, `BUILD-INFO` |
| `install.sh` | installs three binaries; `kiro-cli setup` / `integrations install dotfiles` (global); `Q_INSTALL_GLOBAL`, `KIRO_CLI_SKIP_SETUP`; glibc 2.34 x86_64 / 2.39 aarch64 | installs one binary to `~/.local/bin` (or `/usr/local/bin` with `--global`/`KIRO_CLI_INSTALL_GLOBAL`); refuses a root install into another user's HOME; glibc ≥2.34 (gnu) / ≥2.26 (musl, "still embeds runtimes built against this older glibc"); runs `kiro-cli _ cleanup-autocomplete-artifacts` (removes legacy shell integration, each edited rc file backed up; `--force` also removes `q`, `qchat`, `kiro-cli-term` links); appends `# Added by Kiro CLI` PATH lines to `.zshrc`/`.bashrc`+login file/`fish/conf.d/kiro-cli-path.fish`/`.profile` unless `--no-modify-path`/`KIRO_CLI_NO_MODIFY_PATH`; global installs print a hint that a legacy `~/.local/bin/kiro-cli` shadows the new one |
| README | "AcceptEnv Q_SET_PARENT / AllowStreamLocalForwarding yes" SSH notes | install/uninstall only: "installs the kiro-cli binary to ~/.local/bin … If the legacy bundled Kiro CLI left shell integration in your shell configuration files, it is removed" |
| origin packages (`manifest.latest.json`) | `.deb` (variant `full`), `.appimage` (`full`), `.tar.{gz,xz,zst}`/`.zip` (`headless`), musl variants, macOS `.dmg` (`full`) | `.tar.zst/.tar.gz/.zip/.rpm/.deb` only, **no `.tar.xz`** (403); Linux variant label `headless` (unchanged), macOS `standalone`, Windows MSI `full` (RUN-STATE) |
| `kiro-cli update` | launcher (`--rollout`, `--relaunch-dashboard`) | chat-side (`--check`, `--force`); 2.37.0 "suggests `brew upgrade --cask kiro-cli`, without `--greedy-auto-updates`" (`greedy-auto-updates` 2 → 0 at 2.37.0) |

cyril: the install locations (`~/.local/bin`, `/usr/local/bin`) are exactly cyril's resolver fallbacks (`auth.rs` `TUI_FALLBACK_DIRS` + `<home>/.local/bin`);
nothing to change. Note for the user's upgrade day: the 3.x installer removes the shell-integration lines the 2.x bundle left in rc files (backups kept).

## Unreleased / dark candidates

| candidate | gate | default | evidence (STATIC) |
|---|---|---|---|
| `memory_cloud_sync` — KAS memory cloud sync in /memories (`memorySync` at user-synced / user-local via KAS configuration methods; last-sync line; per-memory cloud state) | rollout row (3.1.0) | 0% internal/nightly | `raw/rollout/rollout-3.1.0.json`; "Dark-shipped at 0%; ramp internal nightly first … Opt in anywhere with KIRO_ENABLED_FEATURES='[\"memory_cloud_sync\"]'" |
| `universalBackgroundTool` (renamed background-execution gate on initialize/session/new/session/load; KAS env `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED`) | rollout `background_execution` | 0% internal/nightly | 3.1.0 description; `backgroundExecution` 2 → 0 |
| `managed_settings` (org-managed /config, `_kiro/governance/state` subscription) | rollout | 0% internal/nightly (unchanged) | row kept in 3.1.0 |
| Org-managed device-code sign-in policy via controlplane `GetEffectiveEnterpriseManagementResourceWithToken` | account type | — | 3.1.0 strings "Resolved device-code sign-in setting", "Enterprise Management enabled without a body" (no feed line) |
| ACP client engine routing table (1,080 names; exempt list emptied) | `kiro-cli acp` default | per client | §(c); not in any feed line |
| `chat.launch30` / `KIRO_LAUNCH_3_0` ("Opt this machine into the Kiro CLI 3.0 launch behavior before it ships") | setting/env | false | 2.37.0; now redundant on 3.x |
| `acp-client` hidden ACP test client | hidden subcommand | — | present 2.32.0 and 3.1.0 |
| `features/terminal-program-status.md` doc node | — | status `draft` | OSC 7501 shipped 2.32.0; doc still draft |

## Leads for the live lane (HOST-L1…)

Conventions for every leg: archived `binaries/3.1.0/extracted/kirocli/bin/` FIRST on PATH and record the child's `/proc/<pid>/exe` (trap 8 — there is
no `kiro-cli-chat` to shadow any more, but the installed 2.32.0 `kiro-cli` is still on the user's PATH); fresh temp `HOME` per spawn (trap 7). The
`XDG_DATA_HOME` column says whether the leg needs the REAL auth store (single-flight, trap 6) or a fresh one. None of these legs uses `--cloud`/`--repo`
(trap 32); none runs `kiro-cli acp --login`, `login` or `logout`. Note the fresh-store side effect seen even for `_ cleanup-autocomplete-artifacts
--dry-run`: the first run of the 3.1.0 binary creates `data.sqlite3`, `telemetry-export-drops.lock`, `run-receipts/.reaper.lock` and `.kiro/settings/cli.json`
in the data/home dirs it is given — give every leg its own scratch dirs.

- **HOST-L1 (highest; FRESH store, no auth needed) — which engine does `kiro-cli acp` give an unlisted client on 3.1.0?** Raw JSON-RPC driver sends
  `initialize {protocolVersion, clientCapabilities:{}, clientInfo:{name:"cyril",version:"0.0.0"}}` to `kiro-cli acp` with (a) no flag and no setting,
  (b) `--agent-engine v2`, (c) `--agent-engine v3`, (d) no flag + `~/.kiro/settings/cli.json` `{"acp.agentEngine":"v2"}`, (e) no flag +
  `{"chat.agentEngine":"v3"}`, (f) no flag with `clientInfo.name:"acp-probe"` (a snapshot name) and (g) `"JetBrains.Foo"` (ex-exempt prefix). Expected
  discriminators: the V2 engine answers `initialize` immediately with `agentCapabilities._meta` absent / `_kiro.dev` style and never spawns node; the v3
  route spawns `node … acp-server.js` (self-extracting ~190 MB into the scratch data dir — allow 60 s) and, with `--auth-method cli` (default), fails or
  stalls on the missing token while the `initialize` result (if any) carries `_meta.kiro`. Record `/proc/<pid>/exe` of any child, the `initialize` result,
  and stderr. **Verdict wanted:** the default for an unlisted client, and whether `acp.agentEngine` / `chat.agentEngine` move it. This decides the P2's
  priority (P2 if (a) ≠ v2).
- **HOST-L2 (FRESH store, no auth) — 3.1.0 extraction layout.** (1) `KIRO_RUNTIME_DIR=<scratch>/rt kiro-cli _ extract-runtime` on the archived 3.1.0 binary
  (→ `find <scratch>/rt -maxdepth 6` to record node/bun/tui.js/`lib/kas/…/acp-server.js`, `.owner`/`.complete` markers, version dir naming);
  (2) `kiro-cli _ extract-runtime --dir <scratch>/rt2` (same); (3) `KIRO_RUNTIME_DIR=<scratch>/rt kiro-cli acp --agent-engine v3` + `initialize` → does
  the host spawn node from the runtime dir (`/proc/<pid>/exe`, argv) and is the scratch data dir's `kas/` left empty? (4) control: plain `kiro-cli acp
  --agent-engine v3` with a scratch `XDG_DATA_HOME` → confirm `kas/<3.1.0's @kiro/agent version>-<sha256>/node_modules/@kiro/agent/dist/server/acp-server.js`,
  `node`, `bun`, `tui.js` and the `extracted_kas_versions`/`pinned_bin_versions` rows (`sqlite3 … .tables`) — the layout cyril's `discovery.rs` depends on.
  Snapshot before/after (trap 18 applies to the REAL data dir only if a leg ever uses it — these legs must not).
- **HOST-L3 (FRESH store, logged out by construction) — the 2.37.0 terminal auth method.** `kiro-cli acp --agent-engine v3` (and once with `v2`) with a fresh
  HOME/store; send `initialize` twice: with `clientCapabilities._meta:{"terminal-auth":true}` and without. Expected: the `initialize` result's `authMethods`
  holds `{type:"terminal", id, name, description, args:[…"--login"…]?, env?}` only when the meta is declared; record the exact object (ids, `args`, `_meta`)
  and whether a `session/new` without `authenticate` is refused with an auth error. Do NOT run the advertised command.
- **HOST-L4 (REAL store, single-flight, run ONLY while the user's access token is fresh, never `--force-refresh`) — cyril's hourly helper against 3.1.0.**
  `kiro-cli chat _ get-kas-token` on the archived 3.1.0 binary with `HOME=<tmp>`. CAVEAT the live driver must weigh: the 3.1.0 binary applies newer sqlite
  migrations to `data.sqlite3` on open ("Applying migration" / `INSERT INTO migrations`), a one-way change to the real store the installed 2.32.0 then has
  to read; and running it against a COPY of the store is only safe while no refresh is triggered (a refresh in the copy consumes the single-use refresh
  token and logs the real store out). Recommended form: copy `data.sqlite3` into a scratch `XDG_DATA_HOME`, run once while `expires_at` is >15 min away,
  expect `{kind:"getKasToken",data:{accessToken,expiresAt,profileArn,authMethod,provider?}}` with the SAME `expiresAt` as the store (proves no refresh
  happened), then delete the copy. Skip if the token is near expiry.
- **HOST-L5 (FRESH store, low) — v1 on 3.x.** `kiro-cli acp --agent-engine v1` + `initialize`, and `kiro-cli chat --agent-engine v1 --no-interactive --help`-less
  dry forms are NOT available without a prompt; so only the `acp` form: expect either the unreachable-code panic ("V1 engine does not use the ACP launch
  path") or a remap to v2/v3. Also `kiro-cli chat --legacy-ui --list-models` (no turn, needs auth → only if a real-store leg is already scheduled) to see
  "Kiro CLI 3.0 no longer runs Classic (the v1 engine), so this run ignores the Classic selection." on stderr.
- **HOST-L6 (cyril-on-3.1.0, REAL store, after L1) — the V2 wrapper mismatch as the user would see it.** `cyril --agent-engine v2 --agent-command
  <archive>/kiro-cli acp` with cyril HEAD recorded (trap 31): expect `fingerprint.rs:92-96` "engine mismatch: cyril is driving the v2 engine but …" if L1(a)
  resolved v3; then the same with `--agent-command <archive>/kiro-cli acp --agent-engine v2` as the proof the P2 fix works. Also run the normal cyril
  KAS wrapper and free path against 3.1.0 as the regression check of the closed fixes (zbu7/k8wc/auxt/oieu/vh69), which the v3-coverage lane owns.
- **Negative leads (do not chase in my lane's name):** `PreCompact`/`PostCompact` 0/0 in the host (KAS); `[INFO]` stderr suppression has no literal (observe
  stderr in L1(c)); `structuredContent`, `web_fetch` markdown, 90 s retry, bash-when-SHELL-unset are KAS; `/usage` session credits, recap, notification
  queue are TUI; `--cloud` text unchanged since 2.32.0 (no new cloud lead; trap 32).

## Changelog reconciliation (all thirteen feeds; `feed-<ver>.json`)

Rule: every line not tagged `[V3]` gets a location; `[V3]` lines are "not in my lane (KAS/TUI)" unless the host is the owner. Line numbers are the
feed's order. "logic" = behaviour change with no new host literal (count-identical strings).

**2.33.0 (28 lines)** — non-[V3]: #3 faster `grep` → KAS tool (not mine); #4, #13 Mermaid → TUI; #14 home-dir global agents listed twice → host/`agent`
crate (logic; `agent_config/load.rs` line shifts only); #25 Tab after `/model ` → TUI. [V3] host-side: #16 `kiro-cli chat --agent <agent> @agent-sop:<name>`
runs the SOP incl. `--no-interactive` → host launch (logic). All other [V3] lines (trust prompt, plan tree, PR watches, terminal title, sandbox rules, …) → KAS/TUI.
Host-only change not in the feed: `amzn-codewhisperer-client/src/endpoint_lib/host.rs` added; `_kiro.dev/command` prefix literal; serde
`CommandsAvailableNotification [5]`/`ToolAdvertisement [3]`/`McpServerAdvertisement [3]`; rollout override `standalone_toolbox.treatment` 7 → 13 cohort ids.

**2.33.1 (16)** — non-[V3]: #2 `/tools` screen-reader lines → TUI; #3 `kiro-cli mcp add --agent` grants the server and upgrades the agent file when
`chat.enableAutoAgentUpgrade` is on → **host** `cli/mcp` (logic; key 2/2); #10 HTML doctype colors → TUI. [V3] host-side: #4 resuming a V2 session tells
you approvals weren't carried over → host/TUI (logic); #14 `@builtin` includes web search → KAS. Host-only not in feed: `launch/v2_builtin_default_agent.rs`,
`KIRO_STANDALONE_NOTICE`, `standalone.autoEnroll`, `v3_prompt` 10/75i → 25/100i.

**2.34.0 (28)** — non-[V3]: #4 `/model edit` hides models → **host key** `chat.modelPicker.hiddenModels` + TUI; #11 `Working...` → TUI; #15 enterprise
overage in `/usage` → TUI/KAS (no host literal); #17 refusal partway continues on the fallback model → host v2 api_client (logic); #18 `chat.allowIcons`
→ TUI; #19 Mermaid → TUI; #24 pasted images/dictation → TUI. [V3] host-side: #6 `--trust-workspace` → **host flag** (clap text [0,1]); #12/#13 headless
mode answers/approvals + reuses MCP sign-ins → **host** non-interactive client (`launch/secret_storage.rs` [0,8] = the saved-sign-in store); #23
`kiro-cli acp` no longer shuts down while the agent runs → host acp supervision (logic); #28 same-origin redirects for HTTP MCP → KAS (host rmcp
unchanged). Others (elicitation, plan suggestions, delete memories, approval order, auto-deny unparseable commands, …) → KAS/TUI. Host-only not in feed:
clap `bin_name` → `kiro-cli`, `--agent` shorthand, `doctor` stub, `_ standalone-cutover`, `cli/telemetry.rs`, `cli/uninstall.rs`, `cli/update/cutover.rs`,
`chat-cli-v2/src/agent/kas/export.rs`, `install.releaseUrl`, `chat.workflowLaunch`, `KIRO_TRUST_CLASSIFIER_MODE`, rollout +3 rows, `v3_prompt` 25 → 50.

**2.34.1 (1)** — #1 queued message blank lines → TUI. Host-only: `standalone_toolbox` 5 → 20; `trust_classifier_enforce` nightly → all channels.

**2.34.2 (1)** — #1 "offers to switch you to V3 when you start a chat" → **host**: rollout `v3_prompt.treatment_percent` 50 → 100 (no code change).

**2.35.0 (15)** — non-[V3]: #1 root/Administrator warning + `app.warnOnElevatedPrivileges` → **host** (`KIRO_ELEVATED_PRIVILEGES` [0,1], key [0,3],
"Warning: Kiro CLI is running as root…"); #2 `/fullscreen` pin → TUI; #3 ghost → TUI; #4 refused response on Auto retried → **host** rollout
`model_fallback_auto_refusal` → 100% all + `auth/mod.rs`; #6 fallback hang → host v2 (logic); #7 dictation → TUI; #10 `/chat`/`--resume` large history →
host v2 `persist.rs` (logic). [V3] host-side: #8 resuming a V2 session default agent → host (`v2_builtin_default_agent.rs`, logic); #9 Linux resume after
reboot → host/KAS session lock (logic); #12 `kiro-cli mcp list` registry server → **host** `cli/mcp` (logic). #5 org settings for MCP/web, #11 `/mcp` hidden
servers, #13 HTTP 403 probe, #14 refusal loop, #15 `read_file` offset → KAS/TUI. Host-only not in feed: `integrations` subcommand, `cli/integrations.rs`,
`KIRO_ROUTER_VERBOSE/VERSION`, `standalone_public` 0 → 10.

**2.35.1 (1)** — "Performance improvements" → unattributable (host strings 425,788 → 425,742; `standalone_toolbox` 20 → 50).

**2.35.2 (1)** — "Retiring autocomplete desktop app - gradual rollout" → **host**: `standalone_toolbox` 50 → 100, `standalone_public` 10 → 30.

**2.36.0 (15)** — non-[V3]: #3 `kiro-cli agent upgrade` warns on deny/hide/ask → **host** (serde `V3PermissionRule [4]`, `DetectorConfig [9]`,
"allowReadOnly let V2 read any file without asking…" [0,1]); #5 `acp.agentEngine` → **host** [0,5] + `acp_engine.rs` router (the client lists are NOT in
the feed); #6 ghost animations → TUI; #10 Mermaid → TUI; #11 crash on non-ASCII MCP tool names → host/TUI (logic); #12 indented wrapped command → TUI;
#14 `GIT_CONFIG_GLOBAL` env approval → **host v2** shell_permission (`GIT_CONFIG_GLOBAL` 0 → 2); #15 Bun 1.4.4 → **host embedded frame** (79,500,640 →
107,308,936 B). [V3] host-side: #13 Node 22.23.4 → **host embedded frame** (124,679,552 → 124,902,584 B); #1 Approve for me → rollout
`trust_classifier_enforce` → all users; #4 per-agent V3-blocks notice → TUI (`KIRO_AGENT_UPGRADE_NOTICE` arrives 2.37.0). #2 shell rule tiers, #7 `/spec`
questions, #8/#9 approval rendering → KAS/TUI. Host-only not in feed: `standalone_public` 30 → 50; the TUI's `--agent-engine=v2` pin.

**2.37.0 (18)** — non-[V3]: #1 ACP clients sign in from a terminal → **host** (`acp --login`, `cli/acp_sign_in.rs`, `terminal-auth` pointers); #2
`KIRO_RUNTIME_DIR` → **host** (`runtime_dir.rs`, `_ extract-runtime`); #3 fullscreen rows → TUI; #4 ghost color → TUI; #5 V2-format agents auto-upgraded at
launch / `chat.enableAutoAgentUpgrade=false` → **host** `launch/auto_migrate.rs` ("Auto-upgraded agent config to universal format"; key readers 2 → 3);
#8 Mermaid → TUI; #9 `kiro-cli update` suggests `brew upgrade --cask kiro-cli` without `--greedy-auto-updates` → **host** (`greedy-auto-updates` 2 → 0);
#10 `--no-interactive` stop on an agent says `stopping (exit 4)` → **host** [0,1]; #12 fullscreen clicks → TUI; #14 tmux wheel → TUI; #15 ghost → TUI;
#16/#17 Mermaid → TUI. [V3] host-side: #11 `--no-interactive` runs stopped by a V2-format agent name `kiro-cli agent upgrade <dir>` → host (logic; the
literal exists only in 2.37.1's bundled feed copy); #6/#7/#13 approval prompt rendering → TUI; #18 `/spec` escape sequences → TUI. Host-only not in feed:
`launch/v3_launch_prompt.rs` ("…V2 … will be removed in January"), `KIRO_LAUNCH_3_0`, `chat.launch30`, Classic retirement strings, `standalone_public` → 100,
frame order change (tar, Bun, TUI, Node).

**2.37.1 (4)** — non-[V3]: #1 welcome/startup refresh → TUI; #2 pill → TUI. [V3]: #3 output-limit notice → KAS/TUI; #4 idle CPU tracking the engine's process
tree → **host** launch (logic). Host-only: exempt-client list emptied (6 → 0); 2.37.1 embeds a copy of the 2.37.0 feed (transient +1 counts for `stopping
(exit 4)`, `starts logged out`, `kiro-cli agent upgrade <dir>`, `chat.enableAutoAgentUpgrade` — changelog text in rodata, not code; trap for future lanes).

**3.0.0 (3)** — #1 V3 default experience → **host**: `chat.agentEngine` persistence + switch prompts (strings since 2.37.0; `chat --help` and the setting
description still say "v2 (default)"); #2 Classic retired → **host** "Kiro CLI 3.0 no longer runs Classic (the v1 engine), so this run ignores the Classic
selection." (2.37.0 string; `--legacy-ui`/`v1` still parse); #3 inline shell autocomplete and the CLI desktop app retired → **host packaging**: launcher,
term, `q`, `qchat` removed; 109 `Q_*`/`KIRO_CLI_AUTOSUGGEST_*` env names and 35 launcher crates gone; `init` no-op stub; `install.sh` removes shell integration.

**3.1.0 (57 lines, none tagged [V3]; #57 tagged [V2])** — host: #2 `KIRO_DEFAULT_SETTINGS` ✓ [0,8] (+`default_settings.rs`, doc node); #5 session recap →
**host key** `chat.enableSessionRecap` [0,3] + doc node (behaviour TUI); #31 `agent upgrade`/`/upgrade-agent` warn on V2 tools V3 lacks → **host** ("V3 has
no tool for these V2 tools, so V3 ignores them…" [0,1]); #39 `kiro-cli agent validate` exits non-zero → host `cli/agent` (logic); #41 correct service hosts
in isolated AWS regions → host endpoint resolution (logic; `attempt_header.rs` [0,2] is the nearest new file) [INFERENCE]; #34 MCP/web tools stay on for
org users when a later profile check fails → host/KAS (new controlplane call `GetEffectiveEnterpriseManagementResourceWithToken` is the host-side candidate)
[INFERENCE]; #48 `--no-interactive` and `kiro-cli acp` no longer print `[INFO]` startup lines → **host** (logic; 0 literal); #49 `--no-interactive` text
newline after a tool call → host launch text mode (logic); #55 OTel tool-use export security → **host** telemetry (logic; `KIRO_DISABLE_TELEMETRY`,
`telemetry.enabled`, `KIRO_TELEMETRY_OTEL*` unchanged); #56 owner-only MCP registrations / credential folders on Unix → KAS or host `secret_storage`/
`owner_only_sweep` (no new host literal; unattributable); #57 [V2] `GIT_DIR`/`GIT_WORK_TREE` git commands ask → **host v2** (`GIT_DIR` 0 → 2, `GIT_WORK_TREE` 0 → 2).
TUI: #3 `/usage` session credits, #4 step credits, #9 notification row, #10 sticky warnings, #11 startup cloud-config notice, #16 transcript line, #17 queued
tray, #32 ctrl+o hint, #35 MCP sign-in toast, #38 Mermaid, #45/#46 resize, #47 footer credits, #54 tool output row, #22 `/context show`, #15 `/agent swap`.
KAS: #1 PreCompact/PostCompact (host 0/0), #6 sub-agent memories, #7 sign-in refusal (host envelope unchanged), #8 `file://` caps, #12 `web_fetch` markdown,
#13 `structuredContent`, #14 global skills from `kiro-cli acp` in home dir (KAS/cwd; unattributable), #18 90 s retry, #19/#27/#40/#44 sub-agents, #20 token
endpoint outage, #21 output cap, #23 512 MiB transcript, #24 bash when `SHELL` unset, #25 logs of another session, #26 `str_replace` `$`, #28/#30/#36
workflows/specs, #29 property tests, #33 empty reply retries, #37 shell stdin EOF, #42 PreToolUse exit 2, #43 Windows child processes, #50 cloud region, #51
crash during save, #52/#53 web tools. Host-only not in feed: rollout table shrunk to 15 rows (+`memory_cloud_sync` dark), `rollout.rs`/`v3_ease_in.rs` gone,
`v3_switch.rs`, `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED`, `log file not shared with the engine: only its owner may write its directory`,
"--login is not supported with --auth-method=client", doc manifests regenerated, `standalone.*`/`KIRO_STANDALONE_*` removed, `lzma-rust2` replaces `liblzma`.

## Proposed issues / notes

Grepped `.rivets/issues.jsonl` (454 rows) for zbu7, igrx, d7ve, a7nk, 4c4d, evwh, dcc6, tikf, ci3j, a8se, oieu, auxt, cxd2, 0rn8, 0ssb, zjm5, s21o,
and for the topics (3.0, agentEngine, get-kas-token, KIRO_RUNTIME_DIR, KIRO_DEFAULT_SETTINGS, retire, Classic, sign in, logged out). No existing issue
covers the acp router, the runtime dir or the default-settings file.

- **NEW (P2, STATIC → P2 confirmed if HOST-L1(a) ≠ v2, else P3): "Wrapper V2 spawn must pin `--agent-engine v2`: kiro-cli ≥2.36.0 routes `kiro-cli acp`
  by `clientInfo.name` and the 3.x default is V3".** Entry point: `crates/cyril-core/src/protocol/bridge.rs:394` (`AgentEngine::V2 => Ok((agent_command.clone(),
  None))`) spawns the user's bare `kiro-cli acp` (default argv `crates/cyril/src/main.rs:32-36`). Since kiro-cli 2.36.0 `cli/acp_engine.rs` peeks the ACP
  `initialize` request and routes unlisted clients to the acp default; `cyril` is in neither embedded list (snapshot of 1,080 names; exempt list empty since
  2.37.1), Kiro's own TUI has pinned `--agent-engine=v2` since 2.36.0, and 3.0.0 makes V3 the default. Fix: append `--agent-engine v2` for V2 wrapper spawns
  on kiro-cli ≥2.7.1 (reuse `kas/version.rs:343-364` `build_wrapper_command` with flag `v2`; the flag has accepted `v2` since it existed) and reword
  `protocol/fingerprint.rs:92-96` (its "spawn a v2 agent (`kiro-cli acp`)" remedy is the command that produced the mismatch). Evidence: this file §(c),
  `raw/acp-clients-header.txt`, `raw/help/merged-acp-3.1.0.txt`, TUI 3.1.0 pin. Blocks nothing on the default (KAS free) path.
- **NEW (P3): "KAS discovery is blind to `KIRO_RUNTIME_DIR` / `kiro-cli _ extract-runtime` installs (kiro-cli 2.37.0+)".** Entry point
  `crates/cyril-core/src/protocol/kas/discovery.rs:197-200` (env reads) and `:50-53` (the `KasMissing::Server` remedy tells the user to run `kiro-cli acp
  --agent-engine v3`, which under a runtime dir extracts into the runtime dir again). Honour `KIRO_RUNTIME_DIR` (absolute-path rule as the host:
  "ignoring KIRO_RUNTIME_DIR: not an absolute path") and probe `<dirname(kiro-cli)>/../lib/kiro-cli` for packaged installs once HOST-L2 has recorded the
  tree. Default installs are unaffected (`kas/<ver>-<sha>` layout and `extracted_kas_versions` unchanged in 3.1.0).
- **NEW (P4, or fold into the d7ve lineage): "`KIRO_DEFAULT_SETTINGS` (kiro-cli 3.1.0) is not consulted by cyril's cli.json reader".** Entry point
  `crates/cyril-core/src/protocol/kas/settings.rs:65` (`settings/cli.json` under the KIRO_HOME-resolved dir). Host precedence: default file (root-owned,
  owner-checked unless `KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK`, 1 MiB cap, unknown keys ignored) < global `cli.json` < workspace. A managed default such as
  `chat.enableWorkflows=true` would enable workflows in Kiro's TUI but not in cyril. Low until a CloudShell/managed deployment is in scope (the doc node's
  keyword is `cloudshell`).
- **NOTE cyril-a7nk (open P1, "Kiro CLI 3.0 deprecates Classic/v2 — plan moving cyril's default engine off v2"):** 3.0.0 retired **Classic = the v1
  engine/legacy UI**, not the V2 ACP engine: 3.1.0 still ships `chat-cli-v2` (91 files), all `_kiro.dev/*` literals, and `acp --agent-engine v2` parses;
  Kiro's own prompt says "V2 no longer receives new features and will be removed in January". The acp default moved (router + `acp.agentEngine`, 2.36.0),
  `v3_prompt` reached 100% at 2.34.2, the ease-in code was deleted in 3.1.0 (`v3_switch.rs` replaces `v3_ease_in.rs`). cyril's `--agent-engine v2` keeps
  working only if cyril pins the flag (P2 above). `[agent] engine = "v2"` docs should carry the January horizon.
- **NOTE cyril-cxd2 (open P3, KAS wrapper login gate) + cyril-0rn8 (open P2, logged-out get-kas-token hang):** since 2.37.0 a logged-out `kiro-cli acp`
  can advertise an ACP terminal auth method (`AuthMethod::Terminal`, gated on `clientCapabilities._meta.terminal-auth`; `acp --login` is the command).
  A wrapper-mode cyril could surface that method instead of the C14a gate's error; the free path keeps the callback model. Exact wire shape: HOST-L3.
- **NOTE cyril-zjm5 (open P3, KAS telemetry policy):** 3.1.0 exports tool-use telemetry over OTel when an organization enables only the OTel export;
  `telemetry.enabled` / `KIRO_DISABLE_TELEMETRY` still stop it (host text). The exporter is the host/TUI's (`get-telemetry-credentials`,
  `telemetry-export-drops.json`), which cyril's direct spawn never runs; KAS-side KUTS is the kas lane's.
- **NOTE cyril-s21o (KAS agent memory overlap with cyril-memory):** 3.1.0 adds a dark rollout row `memory_cloud_sync` (0% internal/nightly) — KAS memory
  cloud sync with `memorySync` config at user-synced/user-local scopes "read and written through KAS's configuration methods"; cross-ref the kas lane for the
  `_kiro/configuration/*` keys.
- **NOTE cyril-tikf (open P3):** `kiro/sourceProviders/list` is still called by the host's non-interactive client (`launch.rs` region, 3.1.0); the cloud
  flags' help text is unchanged since 2.32.0. No new host cloud surface this cycle.
- **NOTE cyril-igrx (closed) / cyril-d7ve (closed):** 3.x `install.sh` installs to `~/.local/bin` (or `/usr/local/bin` with `--global`) — both in cyril's
  resolver fallbacks; `KIRO_HOME` gained two readers in 2.34.0 (`secret_storage.rs` copies the legacy secret store into KIRO_HOME) — consistent with the
  d7ve resolver, no action.
- Not filed: `KIRO_LAUNCH_3_0`/`chat.launch30`, `KIRO_ELEVATED_PRIVILEGES`, `app.warnOnElevatedPrivileges`, `chat.workflowLaunch` (kas-lane question),
  `install.releaseUrl`, `chat.modelPicker.hiddenModels`, `standalone.*`, `acp-client`, Enterprise Management device-code policy (login path), `[V2]`
  `GIT_DIR` approval (host v2 shell_permission; cyril observes permission requests only).

## Method notes

- **Masked strings for 14 builds** (`raw/mkstrings.py`; frames zeroed per binary, logged in `raw/mkstrings.log`): host-role string counts 425,7xx–426,389;
  launcher 216,880–217,227; term 175,870–176,007. Helpers: `raw/first.py <lit>…` prints the 14-version count series with first-change attribution (the
  workhorse for "first version"), `raw/cnt.py [-r role] [-v a,b]`, `raw/ctx.py <regex> [-w N] [-r role] [-v a,b]`. Roles: host = `kiro-cli-chat` on 2.x /
  `kiro-cli` on 3.x; launcher/term exist on 2.x only; `all` = union.
- **Role-aware copies of the static scripts** (`experiments/conductor-spike/static-host-{env,paths,srcloc}-3.1.0.py`) diff like roles across the chain and
  add a `merge` record comparing the first single binary against the UNION of the last three-binary release (launcher-only / term-only kept vs dropped).
  `static-rollout/-settings/-doc-manifests/-serde-3.1.0.py` take the host binary of each version directly; `static-host-methods/-deps-3.1.0.py` take
  `<ver>:<bin>` pairs so a 2.x chat binary can be diffed against the 3.x single binary. `raw/run-all.sh` runs everything; logs in `raw/*.log`, bulk in `tmp/`.
- **Rollout anchor trap (new):** `static-rollout-*.py` anchors on `"tui": {` + `cloud_config`; 3.1.0 removed the `tui` row, so the script reports "registry
  not found" while the 15-row table is still there. The 3.1.0 table was recovered by brace-matching every `"treatment_percent"` (`raw/rollout/rollout-3.1.0.json`).
  Next cycle's script should anchor on `"treatment_percent"` or on `cloud_config` alone.
- **Bundled-feed trap (new):** 2.37.1 embeds the previous release's changelog text (`KIRO_BUNDLED_FEED_FILE`), so 2.37.1 shows transient +1 counts for
  2.37.0 feature prose (`stopping (exit 4)`, `starts logged out`, `kiro-cli agent upgrade <dir>`); they vanish in 3.0.0. A literal that appears in exactly one
  release and matches a feed line verbatim is changelog text, not code.
- **Glue rejected (trap 2/3):** env `AWS_MAX_ATTEMPTS*`, `AWS_EXECUTION_ENVSSH_*`, `KIRO_DEBUG_SHELLC`, `KIRO_CLI_IS_CHILD_PROCESSW`, `KIRO_KAS_SERVER_PATH8KAS`,
  `RUST_MIN_STACK[FR]`, `KIRO_TEST_MODE3`, `KIRO_SESSION_IDSHELLZDOTDIR`, the launcher's `Q_SET_PARENT_CHET`/`Q_BUNDLE_METADAT…`; settings `telemetry.enabled…autoEnroll`
  runs; methods `kiro.dev/account/usageEstimated`, `kiro.dev/commandMismatchedKmsKey`, `kiro/secH`, `kiro/openExterna…`, `kiro/worH/I`, `_kiro.dev/mcp/st…`;
  serde `SessionTool::*`/`TestCommand::*`/`TodoList::*` variants that appear and disappear in alternate builds (LTO dedup).
- **Help sweep (trap 8 variant):** on 3.x there is no second binary to shadow, but the installed 2.32.0 launcher is on the user's PATH — every run used an
  absolute archive path and `PATH=<archive bin>:/usr/bin:/bin`. `--help` on 3.x created no files in the temp HOME; the one deliberate non-help invocation
  (`_ cleanup-autocomplete-artifacts --dry-run`) created the fresh-store files listed in the header. zsh word-splitting bit once (`$sc` with a space produced
  `unrecognized subcommand '_ extract-runtime'`); the redo with split args is recorded in `raw/help/help.log`.
- **Cross-lane references (read, not analysed):** TUI 3.1.0 `--agent-engine=v2` pin (`kiro-tui-3.1.0.js`, class with `v2SessionStartedSessions`), TUI
  `KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK` reader ("only supported on Linux and macOS", 1 MiB cap); ACP schema `AuthMethod::Terminal`
  (`~/.cargo/registry/src/index.crates.io-*/agent-client-protocol-schema-1.5.0/src/v1/agent.rs:568-582,920-950`).
- **Traps 6/13:** no backend contact, no auth, no session, no cloud. Background jobs: the two batch runs (mkstrings, run-all) completed; none left running.
- Raw evidence: `raw/` (help/, env/, rollout/, docman/, *.log, sections.log, rollout-table.md, acp-clients-header.txt, scripts) and `tmp/` (strings/,
  paths/, srcloc/, serde/, acp-clients-<ver>.txt, parts/, moved bulk logs; git-ignored).


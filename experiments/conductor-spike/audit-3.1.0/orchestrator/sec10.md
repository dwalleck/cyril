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
- **Static lanes delivered on the full tier in 36–46 min each with no budget incident**; the live lane took 2 h 47 m and still left S4–S8 unrun. Next
  cycle: spawn the cyril-on-KAS leg as its own lane once the first static lane returns, and brief HOST-class routing probes as empty-store-only.

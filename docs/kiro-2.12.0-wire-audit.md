# Kiro CLI 2.12.0 — wire audit (diff vs 2.11.1)

**Analyzed:** 2026-07-09 · **Release:** 2.12.0, BUILD_HASH `dadabd517d97d983075d024197e594c9499043be`, BUILD_DATE `2026-07-09T02:16:33Z` · **`kiro-cli-chat` sha256** `2ea2dc536b1a317c6fb8de6b452aa3cb42c8d7e7303fbeef4b178f8315fe7b00` (archived `~/.local/share/kiro-research/binaries/2.12.0/`).
**Method:** archived 2.11.1 vs fresh 2.12.0 — same-day binary-isolated v2 surface A/B (`probe-v2-surface-ab-2.11.0.py`, field-path set diff over init/session-new/all `_kiro.dev/*`); `nm`+`rustfilt` module-path diff + `.rs` source-path set diff (all three binaries); sentence-string set diff with byte-membership verification; KAS sha-gate + per-file content diff of both versioned extraction dirs + direct-spawn A/B + **host-path initialize A/B** (`probe-kas-host-init-2.12.0.py`, new scripted leg); doc-manifest delta; telemetry metric-catalog set diff; CLI `--help` surface diff; tui.js carve + archive.

**Verdict for cyril: SAFE — no code change required, safe to upgrade 2.11.x→2.12.0.** Every surface cyril touches is frozen: v2 wire field-path-identical (24 cmds / 15 tools), KAS `@kiro/agent` **0.8.0 code byte-identical** (`acp-server.js` sha unchanged), launch contract identical, CLI flags identical. The release is an **MCP-OAuth + TUI-cosmetics release** living entirely below/off cyril's wire.

---

## Changelog (announced 2026-07-08; hidden `version --changelog` == public)

```
Version 2.12.0 (2026-07-08)
  - Added: Support `client_secret` in MCP OAuth config for confidential clients
  - Changed: All TUI glyphs and symbols now respect the ASCII mode display setting
  - Changed: MCP OAuth `redirect_uri` now accepts full URLs with custom paths
             and validates loopback-only hosts
  - Changed: Skip Dynamic Client Registration when a custom `client_id` is
             configured in MCP OAuth config
  - security: Shell permission detector now catches unsafe flags in combined
              short options (e.g., `grep -iP`) that previously bypassed readonly
              classification
  - Fixed: MCP OAuth DCR now sends correct `client_name` for servers that
           require it (e.g. Figma)
```

Four of six items are MCP OAuth config plumbing (kiro↔MCP-server leg — **not** cyril's ACP wire; cyril neither writes MCP OAuth configs nor sees that traffic). The ASCII-glyph item is TUI rendering. The security item is the only behaviorally wire-adjacent one (below).

**2.11.1 stealth-watch closed:** `version --changelog=2.11.1` from the 2.12.0 binary still returns "No changelog information available" — the stealth hotfix stays permanently unlogged; no retroactive entry appeared.

## v2 (default `kiro-cli acp`) — wire FROZEN

- **Same-day A/B identical** (`logs/v2-surface-2.11.{1}-ab-20260709.*` vs `…-2.12.0-…`): 24 slash commands / 15 tools byte-identical; frame kinds and field-path sets of `initialize` resp, `session/new` resp, and every `_kiro.dev/*` notification IDENTICAL.
- **Module-path diff (nm+rustfilt), all three binaries:** kiro-cli-term zero delta; kiro-cli pure dep churn (`prost::encoding` in, `fig_proto::hook` out). kiro-cli-chat adds/removals are dep churn (schemars, similar, tokenizers, moka, `rmcp::model::serde_impl`, AWS SDK shape regen) + LTO outlining. `.rs` source-path sets: IDENTICAL for all three (259/161/86 files).
- **`which_mwinit`-trap candidates all disproven by byte-membership:** `chat_cli::cli::chat::tools::use_subagent` (60→66 hits, literal `tools::use_subagent` in BOTH), `chat_cli::rollout` "removed" (18→13, still present), `chat_cli_v2::os::fs` (4431→4451 `chat_cli_v2` hits) — all outlining churn, zero new functionality at module granularity.
- **CLI flag surface:** `kiro-cli --help`, `kiro-cli acp --help`, `kiro-cli-chat acp --help` all byte-identical.
- Sizes: chat −288KB, kiro-cli −14KB, term +5KB (dep-bump scale).

### Behavioral note — shell permission detector (security item)

The readonly-classifier fix means commands like `grep -iP` (unsafe flag folded into combined short options) that previously executed silently as "readonly" will now surface as `session/request_permission`. **No schema change; no cyril action** — cyril already renders permission requests generically. Expect occasional *new* permission prompts on 2.12.0 for command shapes that were silent on ≤2.11.x; that is upstream-intended, not a cyril regression.

## KAS / V3 — parity CONFIRMED, `@kiro/agent` 0.8.0 code untouched

Bundle sha gate moved `07694135…` → `d93d0c50…`, but (2.11.1 lesson applied) the content layer tells the real story:

- **Per-file sha256 of both versioned extraction dirs** (2,627 vs 2,628 files): the ONLY deltas are vendored **`protobufjs` 7.6.4 → 7.6.5** (10 dist/src files), **`@protobufjs/utf8`** refresh (+CHANGELOG.md — the one added file), and an **`@types/node`** refresh. **`node_modules/@kiro/agent/` is byte-identical including `dist/server/acp-server.js`** (sha `037e97980cd4…`, 19,564,098 bytes, self-version 0.8.0). Zero `_kiro/*` method-surface change by identity.
- **Direct-spawn live A/B** (`probe-kas-commands-tools-2.9.0.py`, both bundles, same day): output byte-identical (`logs/kas-surface-0.8.0-cli2.11.1-ab-20260709.log` vs `…cli2.12.0…`).
- **Host-path initialize A/B** (new scripted leg `probe-kas-host-init-2.12.0.py` — covers the Rust spawn/extraction/launch contract that bundle identity does not): `kiro-cli-chat acp --agent-engine kas` initialize responses identical after logDir-timestamp normalization (1,339 bytes; `logs/kas-host-init-2.11.1-20260709.json` vs `…2.12.0…`). Note kiro's log dirs use compact timestamps (`20260710T023818254`) — the probe normalizes both ISO and compact forms.
- Extraction dir: `~/.local/share/kiro-cli/kas/2.12.0-d93d0c501c01…/`.

## Off-wire lanes

- **tui.js CHANGED** — carved + sha-verified + archived (`kiro-tui-2.12.0.js`, 12,612,859 bytes, sha `999169fb54e656079a2818502727020dccee11e8c6dd8e68709c6b23c0ce9388`; 2.11.0's was 12,607,781 / `16a97c21…`). Net +5KB = the ASCII-mode glyph work; `ascii` grep hits in the bundle are all pre-existing asciidoc syntax-grammar names (false positives), so the toggle itself lives in the v1-Rust-TUI `/settings` path.
- **Doc manifest** (83+118, merged 135 — counts unchanged): zero docs added/removed; ONE field change — `slash-commands/settings.md` keywords gain `display`, `ascii`, `accessibility` (matches the glyph item); re-validated: `effort.md`, `settings.md`. New baseline committed as `docs/kiro-docs-index-2.12.0-{83,118,merged}.json`.
- **Telemetry metric catalog:** `kiro_cli*` metric-name set IDENTICAL (113=113). Apparent blob deltas around `user_turn_completed` attributes are string-pool boundary artifacts — all candidate tokens (`interceptor_state`, `conversation_id`, …) byte-present in both binaries.
- **Sentence-string set diff (membership-verified):** 733/712 raw survivors — far noisier than 2.11.1's ~30 because tui.js changed *inside* the binary (minifier identifier renames dominate). Every distinctive Rust-side token spot-checked (`GetKasTokenArgs`, `cost/speed/intelligence_priority`, `RetryWarning`, `KIRO_TEST_DB_PATH`, `acp_client_name`, `get-kas-token`, …) is byte-present in BOTH binaries with matching counts — zero real sentence-level adds/removes beyond the changelog. *(Lane caveat for future audits: when tui.js changes, this lane drowns in JS churn; filter candidates to non-JS-looking lines or lean on the targeted membership checks.)*

## Artifacts

- `experiments/conductor-spike/logs/v2-surface-{2.11.1,2.12.0}-ab-20260709.{jsonl,summary}`
- `experiments/conductor-spike/logs/kas-surface-0.8.0-cli2.11.{1,2.12.0}-ab-20260709.log` (direct-spawn)
- `experiments/conductor-spike/logs/kas-host-init-{2.11.1,2.12.0}-20260709.json` + `probe-kas-host-init-2.12.0.py`
- `docs/kiro-docs-index-2.12.0-{83,118,merged}.json`
- Binaries `~/.local/share/kiro-research/binaries/2.12.0/` (checksums.sha256 inside); tui bundle `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.12.0.js{,.sha256}`

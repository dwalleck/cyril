# Audit 2.29.1 — run state (started 2026-10-11)
OLD: 2.28.0  NEW: 2.29.1  folds: 2.28.1 (feed-only, no artifacts), 2.29.0  focus: v3/KAS backend, workflows, unreleased features  tier: full (attempted; all five lanes cut by the session usage limit before writing any FINDINGS) → resume as **constrained**

## Releases
| ver | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS class | TUI class | host class | headline |
|---|---|---|---|---|---|---|---|---|
| 2.28.1 | — (feed 2026-10-07) | — | — (403 for every artifact on the origin, Linux/Windows/mac) | — | — | — | — | feed-only: `kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3 — verify in the 2.29.x host |
| 2.29.0 | 2026-10-09T03:12Z | 31e6bb3f | 324185fb… | 0.66.33 | A | A | distinct | `/memories`, `kiro-cli acp` uses CLI login unless `--auth-method=client`, hooks tool_tags (V2), MCP env/agent-tool changes, 13 V3 fixes |
| 2.29.1 | 2026-10-10T03:23Z | 31e6bb3f (same) | edd84abc… (== latest manifest) | 0.66.33 | A (license file only) | A | distinct | MCP OAuth issuer fix |

KAS/TUI class A = byte-identical between 2.29.0 and 2.29.1 ⇒ KAS and TUI lanes audit 2.28.0 → 2.29.1 only. Host binaries differ between 2.29.0 and 2.29.1 ⇒ host lane covers both.

## Established facts
- Installed `kiro-cli` and `kiro-cli-chat` (installed 2026-10-11 15:43) are byte-identical to archived 2.29.1.
- KAS 2.29.1 carve == self-extracted `~/.local/share/kiro-cli/kas/2.29.1-32dbef42…/` (diff -rq empty).
- TUI 2.29.1 carve == installed `tui.js`; 2.28.0 → 2.29.1 TUI differs (55k statement-split diff lines; minified).
- KAS 0.66.26 → 0.66.33 file-level diff: `acp-server.js` 12.30 → 12.67 MB; `acp-server.externals.json`; NEW `api-key-region-probe.d.ts`; changed `client-connections`, `env-auth-provider`, `file-auth-provider`, `multiplex-stream`, `select-auth-provider` `.d.ts`; NEW dep `node_modules/@silvia-odwyer` (photon WASM image processing).
- Payload layout unchanged: 4 top-level zstd frames (KAS tar 180.66 → 183.22 MB; Node 124.68 MB same csize; Bun 79.50 MB same csize; TUI 13.634 → 13.643 MB); 62 nested magics are Bun's embedded modules (present in 2.28.0 too). See `frames-*.json`.
- Changelog: `feed-2.29.1.json` (2.29.1, 2.29.0) and `feed-2.28.1.json` (2.28.1, feed-only release: no tarball/MSI/dmg on the origin; its one line folds into this audit and must be located in the 2.29.x binaries by the host lane).
- Login state at 16:10 local: IAM Identity Center, `whoami` OK. Token single-flight rule applies to the live lane.
- Draft document skeleton with §1 filled: `docs/kiro-2.29.1-wire-audit.md`.

## Steps
- [x] 0 locate   - [x] 1 acquire+carve   - [ ] 2 lanes   - [ ] 3 fold   - [ ] 4 file   - [ ] 5 remember+commit

## Lanes
| lane | agent | status | FINDINGS sections complete | leads forwarded to live |
|---|---|---|---|---|
| kas-core | — | not started (first attempt cut at reading baselines) | none | — |
| kas-workflow | — | not started (cut at reading the brief) | none | — |
| tui | — | not started (cut at setup) | none | — |
| host | — | not started (cut at reading 2.28.0 findings) | none | — |
| live | — | not started; one 0.66.26 sweep leg had begun and "finished suspiciously fast" — treat as void, re-run | none | — |

## Dropped rows (lane finding -> reason not in the document)

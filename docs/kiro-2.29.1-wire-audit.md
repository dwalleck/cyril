# kiro-cli 2.29.1 wire audit — (headline TBD) (covers 2.28.1, 2.29.0, 2.29.1)

Audited 2026-10-11. Baseline: 2.28.0 ([kiro-2.28.0-wire-audit.md](kiro-2.28.0-wire-audit.md)).
Scope: **discovery, not only compatibility**, with the user's priority on the **v3/KAS backend,
workflows, and unreleased (gated) features**. Five lanes: KAS core static, KAS workflow static,
Rust host static, TUI-bundle static (Kiro's own TUI as the reference ACP client), and one live lane
with same-day paired captures of KAS 0.66.26 vs 0.66.33.

**Verdict: TBD.**

Per-lane evidence: [`experiments/conductor-spike/audit-2.29.1/`](../experiments/conductor-spike/audit-2.29.1/).

## 1. Releases and artifacts

| ver | build (UTC) | BUILD_HASH | `@kiro/agent` (KAS) | host / TUI | headline |
|---|---|---|---|---|---|
| 2.28.1 | feed 2026-10-07 | — | — (feed-only: every artifact 403 on the origin) | — | `kiro-cli agent upgrade <dir>` (agent-config upgrade for V2 and V3) — folded here |
| 2.29.0 | 2026-10-09T03:12Z | 31e6bb3f | **0.66.33** | changed | TBD |
| 2.29.1 | 2026-10-10T03:23Z | 31e6bb3f (same) | 0.66.33 (license file only) | changed | MCP OAuth issuer fix |

- 2.29.1 tarball sha256 `edd84abc…` matches `stable/latest/manifest.json`; 2.29.0 (`324185fb…`)
  has no versioned manifest on the origin (both tarballs are served). The installed `kiro-cli` and
  `kiro-cli-chat` are byte-identical to the archived 2.29.1 binaries.
- **2.29.0 and 2.29.1 share BUILD_HASH `31e6bb3f…` yet all three host binaries differ.** TBD (host lane).
- Embedded zstd payload layout unchanged (4 top-level frames; the 62 nested magics are Bun's own
  embedded modules showing through RAW blocks and are present in 2.28.0 too):

  | frame | 2.28.0 | 2.29.0 | 2.29.1 |
  |---|---|---|---|
  | KAS `kas-bundle.tar` | 180,664,320 | 183,224,320 | 183,224,320 |
  | Node ELF | 124,679,552 | same csize | same csize |
  | Bun ELF | 79,500,640 | same csize | same csize |
  | TUI bundle | 13,633,741 | 13,643,233 | 13,643,233 |

- KAS: the 2.29.0 and 2.29.1 carves differ only in `_preserved-licenses/@types/node/package.json`;
  the 2.29.1 carve is byte-identical to the self-extracted `~/.local/share/kiro-cli/kas/2.29.1-*/`.
  File-level 0.66.26 → 0.66.33 diff: `acp-server.js` (12.30 → 12.67 MB), `acp-server.externals.json`,
  NEW `api-key-region-probe.d.ts`, changed `client-connections`, `env-auth-provider`,
  `file-auth-provider`, `multiplex-stream`, `select-auth-provider` `.d.ts`; NEW dep `@silvia-odwyer`
  (photon, WASM image processing).
- TUI: 2.29.0 == 2.29.1 == installed `tui.js`.
- Archive: `~/.local/share/kiro-research/binaries/<ver>/`, `kas-carves/<ver>/`, `tui-bundles/kiro-tui-<ver>.js`.
  Carve tooling: `experiments/conductor-spike/carve-frames-2.29.1.py` (zstd frame inventory that does
  not truncate at nested magics — the bounded carve made Bun look like an 8 MiB ELF).
- Changelog source: `prod.download.cli.kiro.dev/stable/<ver>/feed.json` (copied to the evidence dir as
  `feed-2.29.1.json` and `feed-2.28.1.json`). A feed entry is not a release: 2.28.1 has a changelog line and
  no artifacts, so its line is verified against the 2.29.x binaries.

## 2. KAS 0.66.26 → 0.66.33 (core)

TBD (kas lane).

## 3. Workflows (v3)

TBD (kas-workflow + live lanes).

## 4. v2 (Rust) engine and launcher

TBD (host lane).

## 5. What Kiro's own TUI now sends and consumes (reference client)

TBD (tui lane).

## 6. Unannounced or dark features spotted

TBD.

## 7. Live results (KAS 0.66.26 vs 0.66.33, same day)

TBD (live lane).

## 8. Issues

TBD.

## 9. Method notes for the next audit

- Carve: do not bound a zstd frame at the next magic candidate; incompressible payloads (Bun) store
  RAW blocks that expose nested magics. Use `zstandard` `decompressobj` to get the exact consumed size.

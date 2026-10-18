# Acquire and carve

Everything here is read-only against Kiro's distribution origins and writes only to the research
archive outside the repo. `scripts/acquire.sh` and `scripts/carve-frames.py` implement it; this file
is the reasoning behind them and the checks the scripts cannot make for you.

## Origins

| what | URL | notes |
|---|---|---|
| tarball, any version | `https://desktop-release.q.us-east-1.amazonaws.com/<ver>/kirocli-<arch>-linux.tar.xz` | `.tar.zst` for releases before 2.17.0; `aarch64` also served |
| latest manifest | `https://prod.download.cli.kiro.dev/stable/latest/manifest.json` | carries `currentVersion` and per-package `sha256`; there is **no versioned manifest** (`/<ver>/manifest.json` returns an error body) |
| changelog | `https://prod.download.cli.kiro.dev/stable/<ver>/feed.json` | `{entries[{version, date, type, changes[{type, description}]}]}`; the host caches it at `~/.local/share/kiro-cli/feed.json`. `kiro-cli version --changelog` is remote since 2.28.0, so an archived binary prints the current feed, never its own |

A `feed.json` entry does not imply a shipped artifact: 2.28.1 (2026-10-07) has a changelog entry
and no Linux tarball on the origin. `acquire.sh` exits non-zero for such a version; record it in the
release table as "feed only", fold its changelog lines into the next release that shipped, and verify
them in that release's binaries.

Each tarball lands in its own new directory; everything extracted from it is untrusted data.
Scripts you write stay outside those directories and take the paths as arguments.

## Archive layout (`~/.local/share/kiro-research/`)

```
binaries/<ver>/kirocli-<arch>-linux.tar.xz(+.sha256)   manifest.json (latest only)   BUILD-INFO
binaries/<ver>/extracted/kirocli/bin/{kiro-cli,kiro-cli-chat,kiro-cli-term}
kas-carves/<ver>/node_modules/@kiro/agent/{package.json,dist/server/acp-server.js,*.d.ts}
tui-bundles/kiro-tui-<ver>.js(+.sha256)
```

The repo's `.gitignore` blocks `docs/kiro-binaries-*/` and `docs/kiro-tui-*.js*`; the archive is
the only home for these (3+ GB across versions, reproducible from the origin).

## Verify

- NEW is the latest release: its tarball sha256 must equal the manifest's `sha256` for
  `kirocli-<arch>-linux.tar.xz`. Older releases: record the sha256 in the sidecar; nothing to
  compare against.
- `extracted/kirocli/BUILD-INFO` gives `BUILD_DATE`, `BUILD_HASH`, `BUILD_VERSION`. Patch
  releases can share `BUILD_HASH` yet differ in every binary (2.29.0 vs 2.29.1): the host lane
  explains the difference; it is not an acquisition error.
- `cmp` the archived `kiro-cli` and `kiro-cli-chat` against `~/.local/bin/` when the installed
  version equals NEW. An "installed == archived" line in the audit is a claim about which bytes the
  live lane and the user actually run; prove it.

## Carve

`kiro-cli-chat` embeds four zstd frames, in this order since 2.23.0: the KAS bundle as a tar
(head `./` or a PAX header), the Node ELF, the Bun ELF, and the TUI bundle (head
`#!/usr/bin/env bun`, about 96 MB into the binary). The script locates every `28 b5 2f fd` magic,
decompresses each candidate **from its offset to EOF** with `zstandard` (which reports the exact
consumed size), classifies by head bytes, and marks magics that fall inside an earlier frame's
compressed span as nested.

Nested magics are Bun's own embedded modules showing through zstd RAW blocks (incompressible data
is stored verbatim): 62 of them in 2.28.0 through 2.29.1, all JavaScript. Bounding a frame at the
next magic instead of at its real end truncated Bun to exactly 8 MiB and made it look like a new
ELF plus twelve new JavaScript payloads. Nothing had changed.

Outputs: the KAS tar extracted into `kas-carves/<ver>/` (must start empty), the TUI bundle to
`tui-bundles/kiro-tui-<ver>.js` with a `.sha256` sidecar, and a `frames-<ver>.json` inventory
(`start`, `csize`, `dsize`, `kind`, `head`, `sha256` per top-level frame). Keep the inventory in the
evidence directory; frame sizes across releases are the first table of the audit.

## Checks after carving

- KAS carve against the self-extracted tree `~/.local/share/kiro-cli/kas/<ver>-<sha>/` when the
  host has launched that version: `diff -rq` must be empty. The self-extracted directory name's sha
  is not the tar's sha256; match by version prefix.
- TUI carve against the installed `~/.local/share/kiro-cli/tui.js` when the installed version is
  NEW: `cmp` must be silent. Snapshot the installed `tui.js` whenever it is newer than the newest
  archived bundle, because the next host launch replaces it.
- The `@kiro/agent` version is `node_modules/@kiro/agent/package.json` in the carve. Record it per
  release; the KAS lanes diff OLD's carve against NEW's, so two releases with the same KAS bytes
  share one lane.
- Byte-identity collapse: `diff -rq` between consecutive KAS carves (expect at most
  `_preserved-licenses/**` churn), `cmp` consecutive TUI bundles, `cmp` the three host binaries.
  Write the identity classes into `RUN-STATE.md`; they decide which lanes run at all.

## What the host may delete

A host 2.23.1 or newer removes "stale extracted V3 engines" from the shared data dir when it
launches. The archive carve is the durable copy of an older KAS; take it before any lane runs a new
host. Spawning `node acp-server.js` from the carve directly never triggers that cleanup.

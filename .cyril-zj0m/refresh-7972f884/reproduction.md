# Portable reproduction of the refreshed Zj evidence

This is a frozen evidence/replay package for cyril-zj0m, not a new authorization
or a self-contained executable distribution. The packaging worker executed none
of the commands below. Existing qualification and both real-process receipts are
archived unchanged. A future replay requires a fresh, explicit scoped Main GO;
the archived GO is historical provenance only. Do not reuse it as permission.

## Identity and prerequisites

- Qualified source HEAD: `7972f884fa35de6a0cffa8cb7e78349af4b7d3f3`.
- Qualifier's actual source tree: `858bfff965633bb3b876a13a701457936af17ec9`.
- Separately supplied Linux default-feature App binary SHA-256:
  `27f0642339b92a6e6f2330917060b9a50172bb5b5083821c7eec528014572ff0`.
  No App or benchmark executable is distributed. The source file/hash manifest
  is `source/qualified-source.source.json`; the binary receipt is
  `gates/qualified-binary.json`. This archive does not contain the whole repo.
- Linux PTY support, Python, `uv`, and offline-cached script dependencies are
  required. First-Diff harness declares `pyte==0.8.2`; frozen KAS harness declares
  `pyte` without a version. KAS Python/Node/pyte runtime versions were not captured
  as a complete environment lock, so no stronger dependency-reproduction claim
  is made. An installed Node executable discoverable under `/usr/bin:/bin` is
  required for the KAS command below.
- Extract into an owned location. `index.json` maps every archived input to its
  original provenance and bytes/SHA. Its own digest is stored externally in the
  publication manifest/handoff, avoiding a recursive self-hash. The published
  `archive-index.json` is byte-identical to archived `index.json`.
- Prepare a fresh, owned empty runner HOME, private cache location containing
  the required offline dependencies, fresh output parent, and externally supplied
  fresh GO JSON. A relocated first-Diff GO must name the absolute binary path,
  matching SHA, checked source revision, and `zj0m` scope. KAS has no GO CLI flag;
  its replay is allowed only by that separately supplied authorization.

## Archived layout

- `consumer/zj0m-refreshed-truecolor-01/harness-snapshot/{smoke.py,owned_peer.py}`:
  exact executed first-Diff harness bytes; do not substitute mutable scratch files.
- `reproduction/kas-mini-repo/.cyril-1v25/smoke.py`:
  unchanged frozen existing replay harness, not a reimplementation.
- `reproduction/kas-mini-repo/.cyril-1v25/receipts/kas-completed-output-trace.jsonl`:
  full approved sanitized Completed notification, including the captured session ID.
- `reproduction/kas-mini-repo/crates/cyril-core/tests/fixtures/fake-kiro-cli.sh`:
  synthetic token helper. Its executable bit is retained. The harness locates this
  relative to its own mini-repository root, so preserve this directory layout.
- `reproduction/kas-mini-repo/crates/cyril-ui/tests/fixtures/kas_completed_raw_output.json`:
  exact approved regression fixture. The CLI harness replays the trace above;
  this fixture is preserved as matching source provenance, not a substitute input.
- `gates/` and `source/` contain the authorized qualification inputs only.
  `review/` contains the separate accepted review and scope/provenance record.

## Original successful executions (records, not instructions to run here)

The exact commands/cwds are also retained in
`consumer/zj0m-refreshed-qualification.json`. Both launches passed first attempt.

First-Diff, cwd `/tmp`:

```sh
/home/dwalleck/.local/bin/uv run --script /tmp/cyril-perf-consumer-20261020/smoke.py --authorization /tmp/cyril-perf-consumer-20261020/zj0m-refreshed-go.json --scope zj0m --color-mode truecolor --output /tmp/cyril-perf-consumer-20261020/zj0m-refreshed-truecolor-01
```

KAS, cwd `/home/dwalleck/repos/cyril-wt-fix-cyril-1v25`:

```sh
env -i HOME=/tmp/cyril-perf-consumer-20261020/zj0m-refreshed-kas-runner-home PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8 UV_CACHE_DIR=/home/dwalleck/.cache/uv /home/dwalleck/.local/bin/uv run --offline --script .cyril-1v25/smoke.py --binary /tmp/cyril-zj0m-refreshed-20261020 --engine kas --capture .cyril-1v25/receipts/kas-completed-output-trace.jsonl --preserve-session-id --expect '✓ Run the exact sed/tr pipeline requested' --expect '| XAVT_BEGIN XAVT_END' --output /tmp/cyril-perf-consumer-20261020/zj0m-refreshed-kas-output-01
```

## Relocated replay after a new authorization

These are portable adaptations, not commands already exercised by the packaging
worker. Set the following environment variables to actual absolute paths before
using this block: `CYRIL_REPLAY_BINARY`, `CYRIL_REPLAY_GO`, `UV_BIN`,
`UV_CACHE_DIR`, `CYRIL_REPLAY_RUNNER_HOME`, and `CYRIL_REPLAY_OUTPUT_ROOT`.
The output parent and empty runner HOME must already be provisioned. The two
per-run output directories must not exist. Start in the extracted archive root.
No synthetic GO is generated and no real account, model or tool is needed.

```sh
ARCHIVE_ROOT="$PWD"
: "${CYRIL_REPLAY_BINARY:?Supply the separately qualified absolute binary path}"
: "${CYRIL_REPLAY_GO:?Supply fresh explicitly authorized scoped GO JSON}"
: "${UV_BIN:?Supply the absolute uv executable path}"
: "${UV_CACHE_DIR:?Supply an owned preseeded offline cache}"
: "${CYRIL_REPLAY_RUNNER_HOME:?Supply a fresh empty owned runner HOME}"
: "${CYRIL_REPLAY_OUTPUT_ROOT:?Supply an owned output parent}"

env -i HOME="$CYRIL_REPLAY_RUNNER_HOME" PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8 UV_CACHE_DIR="$UV_CACHE_DIR" "$UV_BIN" run --offline --script "$ARCHIVE_ROOT/consumer/zj0m-refreshed-truecolor-01/harness-snapshot/smoke.py" --authorization "$CYRIL_REPLAY_GO" --scope zj0m --color-mode truecolor --output "$CYRIL_REPLAY_OUTPUT_ROOT/first-diff"

(
  cd "$ARCHIVE_ROOT/reproduction/kas-mini-repo" || exit
  env -i HOME="$CYRIL_REPLAY_RUNNER_HOME" PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8 UV_CACHE_DIR="$UV_CACHE_DIR" "$UV_BIN" run --offline --script .cyril-1v25/smoke.py --binary "$CYRIL_REPLAY_BINARY" --engine kas --capture .cyril-1v25/receipts/kas-completed-output-trace.jsonl --preserve-session-id --expect '✓ Run the exact sed/tr pipeline requested' --expect '| XAVT_BEGIN XAVT_END' --output "$CYRIL_REPLAY_OUTPUT_ROOT/kas-completed"
)
```

The first-Diff harness itself hash-checks the GO binary, stages an exact private
copy, and whitelists the App environment. The KAS command relies on the external
qualified-binary identity/GO rather than adding checks to the frozen harness.
The helper returns a synthetic `AT-fake` envelope only. Node relays the owned
Python ACP peer. `rawInput.command` stays inert JSON: do not execute that captured
shell command. No terminal/filesystem callback, permission grant or live model
is part of either replay. The seeded pending ledger exists only to display and
reject an owned pending comparison; the only permission choice is `reject_once`.
Other entrypoints present in the byte-preserved scripts are outside this package's
authorization and evidence; no C6/native50k/focused-child claim is made.

## Repository gates and historical benchmark boundary

The original gate commands, cwd, exit codes, timestamps and raw log hashes are
recorded in `gates/canonical-handoff.json`; toolchain/default-feature environment
is `gates/environment.json`. Their observed outcome is 2,848 passed, zero failed,
13 ignored/unexercised; clippy workspace/all-targets with `-D warnings`, default
build and final fmt check passed. This is Linux/default-features only, not a
future replay instruction or evidence that a later publication tree was checked.

No benchmark was rerun on this refreshed head. All completed-write measurements,
work-counter/sensitivity and benchmark CLI receipts remain in the older sibling
`.cyril-zj0m/verification.md`, `publication-manifest.json`, `handoff.json` and
`verification-evidence.tar.gz`, pinned to tree
`6241931fecb663e9bb17e5cc0e76c35c12833828` / App binary
`b603b8c5ac261acb827d9a95c90010ea339abff6bbfd687ec5d9991a6732da00`.
Do not treat the new smoke duration or gate durations as performance measurements,
transfer old timing results to the new head, or invoke a benchmark from this package.

PTY/pyte ANSI/text/cells are actual process integration evidence, not native
font/glyph/emoji/CJK/combining-width qualification. The static review is a separate
head/first-parent integration review, not independent private-artifact inspection.

# Current Zj evidence recipe and pin boundaries

This is an exact record of the already exercised source/commands plus retained inputs. It is **not new authorization** to run gates, a benchmark, native tests or another consumer. Never delete or overwrite an old output directory to make a single-use command runnable.

## Source and archive

Checked assembled source: `1226b07c56bf010c8595c9309dcc175d4546850e`, with premerge HEAD3390f2cb and added parents8b803234/3a5b88b6. App SHA-256: `b9c988540861e4a9d8e3379d6a2833225bbe417e7ff1ec0aea40321677e687e1`. The publication's new documentation/evidence files are not represented as part of that checked tree.

Archive SHA-256: `3034f8b443f6ac01899517d70c6001f2cb67c3ef589f09052591e88e3d471459`. Archive index: `b8ac2dfe745bbfb8701969a7805478e049412960e151d482388c9453df95411e`. It contains:

- `gates/`: original canonical receipt/logs, command receipts, source/binary provenance, final source pin/manifest and source deltas. No private Git indexes or executable bytes.
- `source/`: exact qualified README and point-in-time integration binding (owners/callers/parent IDs/pair/policies).
- `consumer/owned02/`: all original 13 checkpoint pairs, raw ANSI/cast/wire, controls/acks, safety, final config/log/result and frozen harness snapshot.
- `consumer/owned02-launch.json`: successful command/exit/cleanup source binding.
- `decoder/owned02-prepare/`: unchanged reviewed bootstrap/adapter, actual fresh GO, preserved earlier UV GO, separate zero-App UV preflight failure, review patch/preparation and actual cleanup receipt.
- `diagnostics/`: original failed01 partial evidence and traceback/owned cleanup receipt, failed first offline-probe transcript, unchanged successful bounded proof/script, zero-App worker provenance failure, and Main's unchanged host-runtime proof. Outcomes stay separate.
- `review/`: independent source and bootstrap static review scopes, including point-in-time limits.

The canonical receipt and source manifest—not branch HEAD alone—identify the checked working contents. A fresh independent build must first establish those source identities and prerequisites; a new binary requires a newly qualified/pinned authorization. Historical binary hashes are not substituted for a newly built executable.

## Already exercised gate selectors

The following exact commands were run by Main's qualification owner; logs and exit0 receipts are archived. This worker did not rerun them. Initial format changed zero files.

```sh
cargo fmt --all
cargo test -p cyril-ui --lib tool_diff::tests
cargo test -p cyril-ui --lib traits::tests::tracked_diff_cache
cargo test -p cyril-ui --lib widgets::chat::tests::render_tool_call_
cargo test -p cyril-ui --lib widgets::chat::tests::completed_write_root_draw_reuses_diff_after_notifications_resize_and_theme_change -- --exact
cargo test -p cyril-ui --lib widgets::chat::tests::pending_approval_diff_preview_reuses_derivation_and_current_colors -- --exact
cargo test -p cyril-ui --lib widgets::approval::tests
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p cyril --bins
cargo fmt --all --check
```

Workspace2860pass/0fail/13ignored; six focused counts5/4/9/1/1/76, overlapping workspace. Live/reference/performance ignored lanes remain ignored. The source integration record also retains optional width/X5/Fiv selector recipes; do not describe those as separately executed focused lanes here merely because their tests are present in workspace.

## Exact successful owned02 command

The original frozen harness `c9ce8830…` and peer `6c5b5b64…` are in `consumer/owned02/harness-snapshot/`. Reviewed bootstrap `912c7b40…` and adapter `0ce4ddf5…` are in `decoder/owned02-prepare/`; full hashes are in handoff/index. The strict bootstrap retains its original absolute paths, proof/source/package pins and one-use output/cleanup guard. These are deliberate safety boundaries, not a relocatable CLI.

After Main's separate runtime-proof PASS and fresh explicit GO, the exact command executed from `/tmp` was:

```sh
env PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/dwalleck/.cache/uv/environments-v2/smoke-dfb66bf98567089b/lib/python3.13/site-packages /usr/bin/python /tmp/cyril-perf-consumer-20261020/zj0m-current-width-owned02-prepare/bootstrap.py --authorization /tmp/cyril-perf-consumer-20261020/zj0m-current-width-owned02-prepare/parent-go.json --output /tmp/cyril-perf-consumer-20261020/zj0m-current-width-truecolor-02
```

Host interpreter version3.14.8 was observed by Main. `PYTHONPATH` reuses the original pyte0.8.2/wcwidth0.9.2 pure-Python source; bytecode is not written. The installed Pyte screens source pin remains `43bcd10d5555f6a8e53a1f9c0e557a47ec801b3914533d5b51af5828a5242ecb`. No extra metadata wrapper, source inspection, API shim or new preflight logic is part of this command.

Result: original scenario PASS, all13, wrapper exit0, separate cleanup COMPLETE. The archived GO is **consumed**, not permission for a second execution. Reproducing this exact single-use layout requires a fresh isolated filesystem with the original absolute paths, same inputs/dependencies and newly authorized qualified binary. Relocating path constants or changing pins changes bootstrap bytes and requires explicit authorization/review; it is not silently covered by this receipt. Do not rerun the original UV `uv run --script smoke.py` recipe and call it the successful adapted host-runtime run.

## Original checkpoint contract (unchanged)

- `z01-write-start` (`.txt` and `.cells.json`)
- `z02-write-incremental` (`.txt` and `.cells.json`)
- `z03-write-completed` (`.txt` and `.cells.json`)
- `z04-write-resized` (`.txt` and `.cells.json`)
- `z05-unchanged-first-changed-second` (`.txt` and `.cells.json`)
- `z06-replaced-diff-source` (`.txt` and `.cells.json`)
- `z06a-retained-diff-new-palette` (`.txt` and `.cells.json`)
- `z06b-diff-scrolled-out` (`.txt` and `.cells.json`)
- `z06c-diff-browsed-back` (`.txt` and `.cells.json`)
- `z07-pending-comparison` (`.txt` and `.cells.json`)
- `z08-pending-resized` (`.txt` and `.cells.json`)
- `z09-completed-and-rejected` (`.txt` and `.cells.json`)
- `final` (`.txt` and `.cells.json`)

These exercise first-Diff coherent counts/body and second exclusion, unchanged-first suppression, replaced source, 160x48/100x32 resize, committed Gruvbox styling, actual offscreen eviction/PageUp return, pending matching-ID comparison/resize, deny-only response, chronology and completion. All13 pairs must be produced; a later adapter or weakened assertion is not equivalent.

## Exact unchanged runtime proof, separately attributed

Main—not the packaging worker—ran the unchanged script SHA-256 `e0e744c261b360693a6352ca5225b452a1099bc836636bff718d7da6ce15ffd6` under `/usr/bin/python` with the same PYTHONPATH/bytecode environment before authorizing the live consumer. It passed the unchanged fixtures/overwrite boundaries/11 captured checkpoints and final bounded proof, exit0/stderr empty. It did not itself launch App or establish all13 live checkpoints. The raw stdout hash reported by Main is `a75e6e59b5cb65478ba3dc2ff3787ffccfb4b13fd5039d9af3a2974852621949`; the retained capture file additionally includes a final `STDERR:` label, so its whole-file hash differs and is recorded separately.

Earlier worker provenance inspection failed before running this proof; the original UV bootstrap preflight failed before App spawn. Preserve both zero-App failures. The first decoder probe also failed its own libvterm assumption; do not erase it or claim the later stricter combining-payload predicate was executed.

## Nonclaims and publication

No new benchmark/performance comparison, general terminal conformance, graphical/native glyph/font qualification, real Kiro/model/auth/grant/file operation, native50000, C6/NK or CI/platform coverage is established. Benchmark6241931f and prior source7972f884 remain historical. The original first_diff EXCLUSIVE/first-frame/full-clear/skip/native-width obligations remain intact and separately attributed.

Main owns staging, PR192 retarget main, final publication/review/tracker and only then C6's qualified published-parent handoff. No current publication commit/tree is asserted checked by the source1226 gates.

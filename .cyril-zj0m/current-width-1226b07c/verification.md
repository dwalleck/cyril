# Current released-width Zj verification

## Current status and identity

The actual assembled source tree **`1226b07c56bf010c8595c9309dcc175d4546850e`** passed Main-owned qualification, independent static integration review, and a fresh owned first-Diff consumer with **all 13 original checkpoints, wrapper exit 0 and cleanup COMPLETE**. This packet is ready for Main publication, not a worker commit/push or CI claim.

The checked source is not HEAD alone. The uncommitted normal merge has prospective parents, in order:

1. `3390f2cb75dd8a7d9df9c6f646b92f868277f281` (Zj PR192).
2. `8b80323450013fd4a780d468b579298efb3725f6` (qualified X5).
3. `3a5b88b66b1267844ea764c4ff58e48294c93141` (approved common released-width prerequisite).

The normal `--no-commit --no-ff` merge was clean, with no conflict resolutions. Its automatic overlaps were README, traits, approval and Markdown. No manual production edit, diff/cache algorithm change, dependency repin or whole-file rollback was made. Five of seven Zj owners remain byte-identical to the premerge owner. `traits.rs` composes the private Fiv Display/Error repair; `approval.rs` composes the common physical-width helper. Theme/highlight identities and the Markdown full-style/cache owner are retained. Exact owner hashes and callers are in [handoff.json](handoff.json) and the retained source integration record.

The expected released pair is **ratatui-core 0.1.2 / ratatui-widgets 0.3.2**, facade ratatui0.30.0. Shared borrowed `CellWidth` measurement follows Span control filtering and accumulates in usize. Input remains character-wrapped with its codepoint-cursor contract; the Python oracle and raw input bytes are not repinned/normalized. Common floor repair, EOF/auth prerequisites, all five source policies and the 300000-raw-byte cap remain unchanged.

## Source and publication boundary

The tested default App binary is SHA-256 **`b9c988540861e4a9d8e3379d6a2833225bbe417e7ff1ec0aea40321677e687e1`** (101984664 bytes). The exact canonical gate receipt is SHA-256 **`9ffea80c46ce8b703f0ae6d9795290b7d80affdea0667d21cda1325218cf8dcd`**.

README and `.cyril-zj0m/current-integration.json` retain their qualified bytes. The latter is a point-in-time integration record: its not-yet-run status was true when written, and is superseded for current qualification status by this packet and `../current-handoff.json`, not by rewriting it. These new publication records are added **after** qualification; no later publication tree or commit is represented as already tested. Source manifests and versus-HEAD/versus-parent-staged patches are archived under `gates/`.

## Exercised gates, not rerun by this worker

Main's canonical receipt records:

- Full workspace: **2860 passed, 0 failed, 13 ignored**, across 42 direct Cargo targets. Nested filtered subprocess summaries are excluded.
- Six focused first-Diff lanes: **5 / 4 / 9 / 1 / 1 / 76 passed**. These overlap workspace/each other and are not additional distinct tests.
- Workspace all-target Clippy with `-D warnings`, default `cargo build -p cyril --bins`, final format check: **PASS**.
- Initial canonical format made **zero changes**; source was unchanged through the gates.

The exact commands/counts/log hashes are retained, rather than replaced by new checks. [reproduction.md](reproduction.md) lists selectors. Ignored live/performance/reference lanes are not promoted into PASS.

## Fresh actual first-Diff consumer

The original frozen harness and owned peer are unchanged:

- `smoke.py`: `c9ce883004155aef05e5a1dd68723be16e3507cc2873aff977e3cb554f34ec2a`.
- `owned_peer.py`: `6c5b5b64f1e5d4ec306b6028e299a96340891961dfaf0cdb046dc2b1240f9cfe`.

A reviewed external bootstrap (`912c7b40…`) installs only the exact previously executed in-memory Screen draw adapter (`0ce4ddf5…`) before `runpy` of the original harness. Both overwrite boundaries are covered; original display/cell APIs and all fixtures, geometry, inputs and assertions remain unchanged. No display substitution, exception-to-PASS, bespoke protocol parser, production change or pidfd shim was introduced.

The successful run used host **Python3.14.8** (`/usr/bin/python`, version parent-observed), with `PYTHONPATH` pointing to the original pure-Python pyte0.8.2/wcwidth0.9.2 site-packages and bytecode writes disabled. Main first ran the **unchanged** exact offline proof under that environment: exit0, stderr empty, all 11 prior checkpoint cells/styles and bounded decoder proof passed, with both host pidfd APIs observed. Static source review is retained; interpreter applicability is established by that actual proof and this actual consumer, not retroactively attributed to the reviewer.

The fresh consumer's original result is PASS: **13 text/cell checkpoint pairs, 18 wire frames, no unexpected callbacks**, first Diff counts/body, unchanged-first suppression, source replacement, current palette, scroll-away/return, pending preview and sole RejectOnly response. Chronology/final completion assertions finished. Wrapper exit0 and separate cleanup **COMPLETE — no proven-owned survivors or unproven marker matches** were both observed. Cleanup signals/observations were empty: normal harness shutdown left no matching survivors for external cleanup.

All original capture names/offsets, screens/styles, raw ANSI/cast, controls/acks, wire, safety, result and cleanup bytes are retained. Successful ANSI SHA-256: `7fdadf95f6c6cb7d400148883af877b80aff96e601877235c3f7669d5c149560`.

## Failed attempts stay failed

1. Original current-width01: App actually ran, but frozen Pyte raised IndexError before chronology visibility membership and again before final capture/cleanup. Eleven capture pairs exist; no result, z09 or final. It remains **FAIL/aborted**, not missing-needle proof or PASS. Exact proven-owned App/peer survivors were subsequently retired; original evidence is unchanged.
2. First offline probe: **FAIL** from an invalid libvterm continuation-tail blank assumption. Full trace/final proof was not reached.
3. Separately authorized unchanged completion probe: **PASS only as bounded offline proof**, not consumer acceptance.
4. UV owned02 bootstrap preflight: **FAIL**, missing required pidfd API pair; **zero Apps**.
5. Worker-added host provenance inspection: **FAIL**, `inspect.getsourcefile` rejected an lru-cache-wrapped callable before the exact proof ran; **zero proof replays/Apps**. This was an instrumentation error, not demonstrated interpreter/package incompatibility.
6. Main's direct unchanged host proof: **PASS**, then a separate fresh GO authorized the successful owned02 consumer.

No failed record was relabeled, repaired in place or deleted. The later stricter blank/combining-payload predicate was **not executed**. Bounded libvterm visual agreement is not universal terminal conformance. First transient orphan offset30332/castline1062 is not claimed as the final persistent-orphan offset. Source-backed overwrite semantics cite [xterm.js InputHandler.print](https://github.com/xtermjs/xterm.js/blob/9ba6c00a195c95fcf8292a2b9084d91450e5daae/src/common/InputHandler.ts#L520-L522) and its [right boundary](https://github.com/xtermjs/xterm.js/blob/9ba6c00a195c95fcf8292a2b9084d91450e5daae/src/common/InputHandler.ts#L641-L643).

## Independent review and limits

Both independent static reviews reported **no findings**, confidence0.96, with exact scope/limits retained in [review.txt](review.txt) and [review-provenance.json](review-provenance.json). Neither reviewer ran gates or proved general terminal conformance.

The first_diff boundary remains **EXCLUSIVE**; first-frame/full-clear/skip/native-width obligations are not weakened. PTY/pyte is not Ghostty/Windows Terminal/font proof. No new benchmark/timing/work-counter sensitivity, real Kiro/model/auth/grant/file operation, native50000, C6/NK, CI/allfeatures/platform or publication-tree qualification is claimed. Historical benchmark tree6241931f and source7972f884/its binary/archives retain their original attribution.

## Portable retained packet

`verification-evidence.tar.gz`: **3034f8b443f6ac01899517d70c6001f2cb67c3ef589f09052591e88e3d471459**, 3404442 bytes, 147 regular-file members. `archive-index.json` is byte-identical to archive `index.json`; SHA-256 **b8ac2dfe745bbfb8701969a7805478e049412960e151d482388c9453df95411e**. Every nonself member has exact bytes/hash/origin. Sorted regular files and zero tar/gzip times make metadata deterministic. Compiled binaries, isolated runner trees, credentials, private Git indexes and unrelated temp/worktree evidence are excluded.

Main owns final review/adjudication, PR192 retarget to main, staging/merge commit/push/tracker, and delivery of the **qualified published** Zj parent to C6. No such publication operation was performed by this worker.

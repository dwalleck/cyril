# cyril-zj0m Gilfoyle verification index

This is the owning evidence record for the actual completed-Write diff-reuse delivery. Main owns source integration, Cargo qualification, the issue PR, CI adjudication and tracker closure. The documentation worker changed only README and `.cyril-zj0m` workflow/evidence, and ran only the later explicitly authorized direct existing-executable unknown-mode check below: no Cargo, build, test, lint, formatter, measurement rerun, commit, push or tracker mutation.

All evidence paths below are relative to the portable `verification-evidence.tar.gz` archive unless linked as repository files. Its `index.json` records every member's original provenance and SHA-256, plus mappings for original ephemeral receipt links. No executable, private environment directory, credential store, runtime journal/log, private Git index or unrelated issue source is included.

## Checked delivery identity

- Actual qualified checkout tree: `6241931fecb663e9bb17e5cc0e76c35c12833828`.
- Observed HEAD: `6ab6d77e03b73a3a45d6d526fbc19641dace5513` (HEAD tree `90f24757311573c4e7d753b7efc86a5a96c19c1c`), not a fabricated implementation commit.
- Base: old [PR #189](https://github.com/dwalleck/cyril/pull/189) commit `194ec404a325843a1b002940784dda7296ef355c`, merged with common fixture prerequisite `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69`.
- Qualified whole-source manifest: `source/qualified-source.json`, SHA-256 `5862499da0e4db3f8acde23b0e735e46af351a350628449a0a454cee0db06b94`.
- Raw repaired qualification handoff: `gates/canonical-handoff.json`, SHA-256 `6e2743d4ceac88185f8010c087917f2ce89ddfae85a8d9389ef58c02d4b7c5f8`.
- Actual-App binary: `/tmp/cyril-zj0m-qualified-20261020`, SHA-256 `b603b8c5ac261acb827d9a95c90010ea339abff6bbfd687ec5d9991a6732da00`; provenance in `gates/qualified-binary.json`. The binary is not published.

The seven frozen issue-owned source bytes are:

| Path | SHA-256 |
| --- | --- |
| `crates/cyril-ui/Cargo.toml` | `dca84714e572fbd0543f20b150c928b09ace0e7ebc8ee34dae9dabc5c2b64c93` |
| `crates/cyril-ui/benches/conversation_render.rs` | `72e4dd8534216d23bf4ed2b1843ee05aa11cbcaf2ab393ca7bb6c6ec3b19e510` |
| `crates/cyril-ui/src/lib.rs` | `5e438d425bbab3621f36808025a6f4fd9b416e0941ce0ae7bbf91c6b22a1a4d6` |
| `crates/cyril-ui/src/tool_diff.rs` | `d4ae79652e8eca09f1c61145cddd23f8ee3f6723a5b04de3588cec1cafdc0036` |
| `crates/cyril-ui/src/traits.rs` | `7acbafe0c279c9b0134e5c96b906e474be54a2a10213216767d0d08df26f926d` |
| `crates/cyril-ui/src/widgets/approval.rs` | `76d03ff8fc99290683455afba94e6fd1edbb515bc267e2d5cde17fa184bd5234` |
| `crates/cyril-ui/src/widgets/chat.rs` | `9f763f63588e61b438111908f4ef7ba217709c54deed6b9fac3670f711c8ef53` |

`source/delivery-state.json` binds the qualification, source hashes, binary and historical boundary. README/workflow/evidence edits occurred after qualification and do not change those source paths. **The later parent output-alias refresh is not present in this tree.** No future stack merge, rebase, alias integration or c6la change is qualified by these receipts. Main must assess/reverify affected obligations after such changes.

## Obligation ledger

| Obligation | Result and evidence |
| --- | --- |
| C1: coherent first-Diff summary/body, exact counts/gutters, unchanged-first suppression | **PASS** — permanent tests in `gates/02-test.log`, original-base discriminating RED/GREEN in `sensitivity/`, and actual-App scenes in `consumer/zj0m-truecolor-01/`. |
| C2: clone/update/source lifetime, metadata/equivalent-input retention, replacement and current styles | **PASS** — permanent tracked-wrapper/root tests in the qualified workspace, fresh actual-source review, and actual width/palette/offscreen/back/replacement observations. Historical constructor counts are separately scoped below. |
| C3: bounded/context/ellipsis rows and pending approval without completion gate | **PASS** — qualified derivation/chat/approval tests, including `pending_approval_diff_preview_reuses_derivation_and_current_colors ... ok`, and actual pending ledger join/resize/RejectOnly wire response. |
| C4: no repeated unchanged Myers work and production-shaped repeat workload | **PASS** — `completed_write_root_draw_reuses_diff_after_notifications_resize_and_theme_change ... ok` in the actual-tree workspace; historical actual-constructor sensitivity; full-root sparse/dense/pending benchmark and separate CLI rejection below. |
| Assembled workspace | **PASS** — 2,845 passed, 0 failed, 13 existing ignored; ignored cases are not exercised. Top-level Cargo summaries exclude six nested one-test subprocess summaries to avoid double-counting. Their 4,464 filtered entries are nested filters, not missing top-level workspace coverage. UI library: 809 passed, 0 failed, 1 ignored. `gates/02-test.log` and `gates/canonical-handoff.json`. |
| Workspace/all-target Clippy, CLI build, format | **PASS** — fresh repaired-tree run, all exit 0; commands below and `gates/01-clippy.log`, `03-build.log`, `04-fmt-check.log`. |
| Independent substantive current-source review | **PASS** — ReviewDiffReuse freshly inspected the actual seven-file delivery at tree `6241931f` against observed HEAD `6ab6d77e`, including qualified new-file blobs, consuming seams, final formatting and mechanical benchmark predicate correction. Correct, confidence 0.98, no findings, F1 resolved. `review/actual-delivery-independent-review.json`. This is static review, not independent execution of the gates or consumer. |
| Permanent documentation | **Delivered after consumer PASS** — existing README Tool call history now documents first-Diff counts/body, bounded rows, retained derivation, replacement and pending comparison. |
| Windows readiness baseline | **N/A — requester-approved baseline exception**, not platform-green. `limits/windows-baseline-exception.md` and [public acceptance](https://github.com/dwalleck/cyril/pull/183#issuecomment-6284441811). Cancelled coverage is not green; new/differently attributed failures remain blocking. |
| Issue-specific commit/PR, CI and tracker closure | **PENDING — Main publication**, not performed by this worker. The exact file allowlist is [publication-manifest.json](publication-manifest.json). |

### Exact assembled commands and environment

The qualifier ran from the actual zj0m worktree with `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target` set explicitly:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p cyril
cargo fmt --all --check
cargo bench -p cyril-ui --bench conversation_render -- completed-write
```

The initial authorized format pass preceded the qualified source pin. Clippy/test/build/final-format/benchmark each exited 0. `gates/canonical-handoff.json` retains exact cwd, start time, elapsed time, exit status and log hash for each; `gates/environment.json` retains CPU, rustc/Cargo, profiles, target/features and only relevant build/color variables. Rust/Cargo 1.94.0, host `x86_64-unknown-linux-gnu`, AMD Ryzen AI MAX+ PRO 395; workspace tests use opt-level 1, CLI build uses dev/default `kas`, voice disabled. No explicit feature or target override was passed. Entrypoint-touch provenance is retained to distinguish actual checkout compilation from shared-target sibling artifacts.

## Completed-write benchmark

`gates/05-bench-completed-write.log` is the complete untrimmed build/run report and all 90 raw sample rows. Full `UiState`/typed notifications/root draw use ratatui TestBackend at 160×48, Cyril Dark/TrueColor, one process with sequential workloads and process caches shared between cases. Each fresh state/terminal reports one cold draw, then 100 warmup draws and 30 samples of 100 draws; fixture construction/validation are excluded from steady samples. Each workload's positive visibility/retention/count/truncation or pending-preview controls passed.

| Workload | Cold draw (ns) | Median sample-mean ns/frame | Nearest-rank p95 ns/frame |
| --- | ---: | ---: | ---: |
| Completed sparse 2,000 lines | 16,270,791 | 174,904.270 | 177,382.460 |
| Completed dense 20,000 lines | 1,418,938,757 | 178,005.725 | 180,280.280 |
| Pending preview control | 152,375 | 113,337.480 | 114,281.740 |

Bench profile inherits release (`opt-level=3`, fat LTO, one codegen unit, stripped symbols), with no profile overrides; the raw report retains target/features/compiler/CPU fields. These are observed runner-specific measurements, **not a timing acceptance threshold, calibrated frame budget, comparison with an unmeasured baseline, native-terminal latency or 50,000-message benchmark**. TestBackend excludes terminal I/O. Deterministic constructor tests, not a timing claim, establish the work-reuse property.

### Later direct unknown-mode rejection

Main explicitly authorized only a direct existing-qualified-benchmark unknown-mode invocation, not Cargo/build or another measurement run. The executable was resolved from the completed-write log's actual `Running` path, copied privately, and both original/copy were hashed before execution. Main attested no release builds had occurred since the zj0m benchmark. At `2026-10-20T15:55:20.741440+00:00`, the private copy was invoked with `unknown-mode`: **PASS**, exit **1**, empty stdout, explicit unknown-argument error plus usage on stderr, no workload/sample output. `gates/cli-rejection.json` and `.log` retain authorization, exact command, output and provenance.

Observed-now benchmark executable SHA-256: `1009bd865f876771dafd36aba2960e11ab6b59fe49ef1fd7bb65ae350a7543e9`. The original completed-write run did not capture that executable hash; it is **not backdated** into the original measurements or confused with the separate App binary hash. No binary is archived.

## Actual-App consumer outcomes and safety

The first-attempt `consumer/zj0m-truecolor-01` run uses the exact private tested App binary and scoped Main GO. `consumer/zj0m-qualification.json` plus result/wire/control/ack/safety/peer-state records, raw ANSI, asciinema cast, 13 screen/cell checkpoints and exact executed harness snapshots retain the proof:

- Write start → in-progress Diff update → completed update; exact first-Diff `+2 -1`, removed/inserted/context body and line-number gutters. The second-Diff marker is excluded.
- An unchanged first Diff suppresses both summary and changed body despite a changed second Diff; replacing the first source restores coherent counts/body.
- Real 160×48 → 100×32 → 160×48 resize; Gruvbox palette commit changes retained added-line RGB styling. Padding genuinely moves the diff offscreen; PageUp returns to the same counts/body/current style.
- A full pending Diff notification joins a matching-ID partial permission request. The pending comparison survives resize; the only option is RejectOnly, and the actual wire response selects `optionId: deny`.
- Palette returns to Cyril Dark; chronology text appears and actual prompt completion commits the turn.

This is a real Cyril CLI/App process, bridge, picker, terminal-event and ANSI/cell integration observation, **not** a live Kiro/model turn, native graphical-terminal font/glyph/emoji/CJK/combining-width proof, focused-child/native-50k qualification or a work-counter/timing result. Max history is configured 1,000 (default 500); 70 padding paragraphs are not 70 retained-message assertions. No external account/model, actual file operation, host filesystem/terminal callback or permission grant is used; peer state records no unexpected host callbacks. Only the isolated runner's whitelisted safety environment/config is published, never its HOME/XDG/cache/workspace/executable trees or runtime log.

## Historical evidence is not rewritten as current execution

### Original-base independent sensitivity

The retained six-probe run used base `8e5c325ebb5dac3a4ad89f71838fd198cc177543`, historical zj0m patch SHA-256 `5c0d35de8fd980228f9f595199f86014b70e6f7a376ae51c40d7ceb915a655f9` (60,053 bytes) and a separate x5xi patch. Baseline: 2 passed / 4 intended assertion failures; patched: 6 passed. The first-unchanged/later-changed header failed before and passed after; actual Myers counts were cold `2→1`, ten warm draws `20→0`, warm plus width/palette `24→0`, replacement `2→1`, with positive visible first-source/count/body/replacement controls. No compile-error or zero-test RED occurred. The x5xi eviction failure is not reassigned to zj0m.

`sensitivity/receipt.md`, raw logs, environment and RED/GREEN source manifests preserve that boundary. It establishes original defect sensitivity, not literal current assembled byte identity, a repeated instrumented run, timing or later c6la proof. Temporary probe modules were not ported; their documented source-fence limitation is retained. Current uninstrumented workspace tests, fresh seven-source review and actual-App smoke independently qualify the assembled delivery. `review/original-independent-review.json` likewise preserves the original review and F1 re-review without relabeling them current-source review.

### First actual qualification stopped on benchmark lint

`history/first-attempt-handoff.json`, `first-attempt-source.json`, format output and full Clippy failure retain actual tree `f1be5b9ac9aaedadaea6a6855dccac68db176db7` at the same HEAD. The dense benchmark's `line % 4 != 0` triggered `clippy::manual_is_multiple_of`; Clippy exited 101. That attempt stopped: **no workspace tests, CLI build, final format check, benchmark or qualified binary**. This remains a historical FAIL, not an accepted baseline failure or a masked pass. The owner made the bounded `!line.is_multiple_of(4)` correction; fresh source pin, all repaired-tree gates and benchmark then passed. The current independent review explicitly covers the correction. No retry/suppression/skipping/threshold relaxation is inferred.

The original combined qualification handoffs retain historical c6la fields as provenance only; no c6la status or qualification conclusion is supplied by this issue archive. `gates/qualified-binary.json` was captured before benchmark completion and thus still says `benchmark_pending: true`; the later canonical handoff/raw benchmark supplies completion. Neither original receipt is silently edited.

## Portable inspection and deliberate reproduction

Extract `verification-evidence.tar.gz` and start at `index.json`; member hashes and original-path/link mappings make every claimed receipt inspectable without `/tmp`, local OMP artifacts or the original machine. [handoff.json](handoff.json) records the archive identity; [publication-manifest.json](publication-manifest.json) is the exact publication file set with all source/document/evidence hashes. The manifest omits only its own recursive digest; Main's handoff reports that digest externally.

The executed smoke snapshot contains `smoke.py` and adjacent `owned_peer.py` (`pyte==0.8.2` via uv). For a deliberate reproduction, obtain a separately qualified App executable, make a fresh authorization JSON with its actual binary hash/source/test receipt and explicit zj0m GO, then run from that snapshot:

```sh
uv run --script smoke.py --authorization /absolute/path/to/fresh-zj0m-go.json --scope zj0m --color-mode truecolor --output /absolute/path/to/fresh-output
```

Archived GO is historical provenance, not authorization for different bytes or issues. The snapshot's unused other-scope entrypoint is retained only for byte-identical executed-harness identity, not coverage or a future batch plan. Cargo/benchmark commands above are reproduction recipes, not permission to run against changed source without Main's qualification ownership. Subsequent stack/output-alias publication needs its own assessment; this handoff never substitutes inherited evidence for changed-source proof.

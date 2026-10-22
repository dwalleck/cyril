# PR #189 refreshed x5xi publication evidence

This is the **refresh-only**, issue-scoped documentation/evidence package for
[cyril-x5xi, PR #189](https://github.com/dwalleck/cyril/pull/189).
[handoff.json](handoff.json) is the canonical new handoff;
[publication-manifest.json](publication-manifest.json) is the complete four-file
publication whitelist. Main owns the documentation-only commit and push.
The packaging worker ran no product, Cargo, test, formatter, linter, benchmark,
or harness command, and changed no source, README, tracker, or existing evidence.

## Fresh checked identity

| Identity | Recorded value |
| --- | --- |
| Refreshed source HEAD | `aca1aa503a5183c1183ec3bb62b3d61f0bdb8b77` |
| Actual qualified tree | `0280a33d5667e4da217fe572d42e0b9541df2a99` |
| Actual consumer executable SHA-256 | `8b28f0661276925b5d17b5333c75bc03ed24c2a61902c3c81651ae163e711785` |
| Executable profile/target | Default `[kas]`, voice disabled; dev opt-level 0/incremental; `x86_64-unknown-linux-gnu` |
| Binary in this package | **No**; the 101,931,840-byte executable is deliberately excluded |

The archive's `gates/x5xi/qualified-source.source.json` records the actual source
inventory; `qualified-binary.json` binds its tree and build to the executable.
Consumer authorization/safety receipts bind actual executions to that same hash.
These pins identify the checked source **before** Main's refresh documentation
commit. Adding these four files changes the whole-repository HEAD/tree; no future
whole-tree qualification is invented by this handoff.

### Distinct evidence layers

| Evidence | Result | Portable receipt inside `verification-evidence.tar.gz` |
| --- | --- | --- |
| Fresh full workspace gates | **PASS**: 2,833 passed, 0 failed, 13 ignored; ignored cases **not exercised** | `gates/x5xi/canonical-handoff.json`, all five full gate logs |
| Accepted refreshed static review | **correct**, confidence **0.97**, findings **none** | `review/fiv1-x5xi-original-refresh-review.txt`, `review/provenance.json` |
| Fresh assembled actual-process consumer | **PASS**, four first-attempt launches: three theme startups plus exact captured KAS completion replay | `consumer/x5xi-refreshed-qualification.json`, individual run receipts and output |
| Original source-fence A/B and defect sensitivity | **Retained old-pin evidence only**; not rerun/relabelled | `source/refresh-boundary.json` points to the unchanged original publication |
| Commit/push/PR CI/tracker action | **Not performed by this worker** | Main publication responsibility; not a product or packaging failure |

The qualifier's workspace total counts top-level Cargo-launched test/doc-test
summaries, excluding six nested child summaries to avoid double counting.
All five recorded commands exited 0, sequentially in the x5xi worktree with
`CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p cyril
cargo fmt --all --check
```

Exact cwd, start time, duration, command/log hashes, feature/profile/toolchain and
selected environment are in `gates/x5xi/canonical-handoff.json` and
`environment.json`. The approved original stack-level handoff is retained as
`gates/stack-canonical-handoff.json` for provenance only: its fiv1 metadata and
historical path references do not broaden this package's x5xi qualification.
No fiv1 directory or unrelated historical gate log is included.

## Accepted review and unchanged owners

The exact accepted review is the saved
`local://fiv1-x5xi-original-refresh-review.txt`, SHA-256
`196ebdf9a0adc51a76427c386eb91e538d4c14308a2405ecc787eb09436485ab`.
Its archived text is portable; no access to `local://` is required to inspect it.
It reviews x5xi HEAD above against `194ec404a325843a1b002940784dda7296ef355c`,
with the other refreshed stack head retained as context in the original text.
The latest `agent://ReviewRefreshedResultStacks` output is a **different Zj
review** and is intentionally not substituted.

This was a static source/README/tracker-intent review, not qualification. The
reviewer did not run gates/replays, inspect private qualification artifacts, or
independently establish the actual tree fingerprint. Main accepted its correct
0.97 verdict. Qualifier and consumer receipts independently supply the actual
tree/build/execution bindings. The review reports these owners byte-unchanged;
the qualified source inventory also matches their original handoff hashes:

| Owner | SHA-256 |
| --- | --- |
| `crates/cyril-ui/src/highlight.rs` | `f4142b29e3561b392a08e65eb5ca5827735e671577510f4d46a41a041f29fdc9` |
| `crates/cyril-ui/src/theme.rs` | `fefab546a9d63cf300d70c986f48ed485ddf99c1cf7f240b13e649cb58215eaf` |
| `crates/cyril-ui/src/widgets/markdown.rs` | `37719f3bf96799c5240660db27607ab43582ffce098bc21c5242bc4794b2ff2a` |

## Fresh actual consumer outcomes

All four launches passed on their first attempt against the **refreshed** binary,
not by transferring old-binary smoke results.

- `x5xi-refreshed-truecolor-01`, `x5xi-refreshed-none-01`, and
  `x5xi-refreshed-truecolor-return-01` each retain 14 decoded text/full-cell
  checkpoints, seven passing explicit checks, 11 bidirectional ACP frames,
  actual ANSI/cast/events, four control/ack pairs, peer state, safe configuration,
  safety receipt, and the exact executed `smoke.py`/`owned_peer.py` snapshots.
  The 14 include `final`; the original checkpoint numbering skips `06`.
- One committed Markdown/table/quote/code source and a separate live Markdown/code
  source reach the real App. Gruvbox preview changes both code backgrounds from
  `282c34` to `32302f`, while the control comment foreground stays `747369`.
  Cancel restores the exact baseline, commit retains the preview, and returning
  Cyril Dark restores the original styles. The `none` startup keeps default code
  foreground/background through preview/cancel/commit.
- Actual resize is 160×48 → 80×24 → 160×48 while streaming. Oldest/middle/follow
  scroll controls, later `LIVE_APPEND_CONTROL`, and turn completion are retained.
  Each theme receipt has `unexpected_host_callbacks=[]`.
- `x5xi-refreshed-kas-output-01` uses the **frozen original harness**, not a
  reimplementation. The captured session ID
  `sess_973df141-0181-47bf-977a-2f58ca21220f` is returned by `session/new`.
  The full approved sanitized Completed frame is replayed unchanged twice,
  including `rawOutput.output` and inert `rawInput.command`. The harness checks
  exact frame equality/count. Actual decoded output contains
  `✓ Run the exact sed/tr pipeline requested` and `| XAVT_BEGIN XAVT_END`
  **exactly once each**, between `BEFORE_TOOL_ROWS` and `AFTER_TOOL_ROWS`.
  Its actual wire, screen, ANSI, result and helper argv receipt are archived.

Exact successful outer commands and cwd `/tmp` for the three theme runs are in
`consumer/x5xi-refreshed-commands.json`. Each run's `safety.json` preserves the
actual App argv and whitelisted environment. The exact KAS command/cwd is in
`consumer/x5xi-refreshed-qualification.json` and `reproduction.json`; it used
`env -i`, private outer HOME, offline uv, and an absolute installed Node relay to
the owned Python peer. The helper argv receipt is only `chat _ get-kas-token`;
the supplied fake helper returns a synthetic token, not real credentials.

The KAS mini repository keeps the original relative lookup runnable:

| Archived input | SHA-256 |
| --- | --- |
| `kas-replay-repo/.cyril-1v25/smoke.py` | `30346dd420fdf9eafc9b00bb602e9928c471157ca310da49adaa0e1f4f63e3bb` |
| `kas-replay-repo/.cyril-1v25/receipts/kas-completed-output-trace.jsonl` | `f80aae8a8fac961f4224c505d112231a669c71784c4b7b993026bd5b5a4c5e71` |
| `kas-replay-repo/crates/cyril-ui/tests/fixtures/kas_completed_raw_output.json` | `0d613a49514f698c5dd3862d62a24b0fe5616f544d0078880898a441b68720cc` |

The byte-identical executable-script helper is retained at
`kas-replay-repo/crates/cyril-core/tests/fixtures/fake-kiro-cli.sh`; its hash and
mode, like every non-index member's, are recorded in `index.json`.

## Old-pin evidence stays old-pin

The original [verification record](../verification.md), [handoff](../handoff.json),
[publication manifest](../publication-manifest.json), and
[archive](../verification-evidence.tar.gz) remain unchanged and outside this
refresh's publication whitelist. The old archive SHA-256 remains
`afde35dcf77edf4fbb8d3d2a78417692be9380cd842700ba95db28e9180b155f`.
The old assembled delivery was recorded as published fiv1 base
`7eefe275fbec97f610677b2ee6433af996806599` plus its qualified x5xi changes;
the reviewed pre-refresh x5xi head is `194ec404a325843a1b002940784dda7296ef355c`.

The corrected original source-fence bg100 A/B keeps its original attribution:
approved widget BEFORE **1 executed FAIL**, AFTER **1 executed PASS**; forbidden
ThemeId widget **1 executed FAIL on both sides**. No refreshed-head A/B is claimed.
The older six-case sensitivity remains at original base
`8e5c325ebb5dac3a4ad89f71838fd198cc177543`, patch SHA-256
`9ac057ccb15ee7e809330f41bb3abf39259018ea61a8066950635eb02bd93585`:
baseline 2 passed/4 intended failures, patched 6 passed. Only irrelevant-role
cache eviction and syntax/fallback visible controls are x5xi sensitivity;
zj0m observations are not imported. Unchanged cache owners support retained
historical mechanism applicability, not a fabricated fresh instrumented run.

## Portable package, reproduction, and exclusions

Archive: `verification-evidence.tar.gz`, **1,627,807 bytes**, **166 regular-file
members** (165 individually hashed payloads plus `index.json`), SHA-256:

```text
bef26e3fd3e323503d2e65e4ceec0b1a68870bd9e6c565f898446cc5224c4689
```

`index.json` records exact member paths, original source provenance, bytes,
SHA-256 and modes; generated packaging receipts are explicitly labelled. Its
self-digest is omitted to avoid recursion; the external archive hash binds it.
All input copies preserve bytes. The archive is assembled from an **explicit
file whitelist**, never scratch-directory recursion, with normalized tar/gzip
metadata. `reproduction.json` preserves all historical command provenance and
complete, explicitly unexecuted portable commands for each launch, with fresh
output directories and a fresh scoped authorization receipt.

Inspection requires no ephemeral path, private environment or executable.
Deliberate re-execution requires a separately qualified binary/source receipt,
new explicit GO, Linux/POSIX PTY, Python/uv dependencies and Node for KAS. Theme
harness dependency is `pyte==0.8.2`; the frozen KAS harness declares unpinned
`pyte`. The archive is not a hermetic dependency/toolchain image. Portable KAS
commands document omitting historical `--offline` until a fresh cache has been
provisioned. The mini repository is replay support, not a full Cargo checkout.

No binary, private git index, isolated HOME/USERPROFILE/XDG/workspace/cache tree,
ambient private environment, credential store, unnecessary `cyril.log`, fiv1
sibling directory, unrelated issue evidence, or zj/c6/mcp historical log is
included. Approved source-inventory/joint-handoff metadata may contain original
paths and other tracked-file hashes; those are provenance strings, not copied
private indexes or other-scope payloads. The exact harness retains unused
zj0m/c6la entry points solely for byte identity; they were not exercised or
qualified, and their separate scenario modules are not packaged.

### Qualification and privacy limits

- Default-feature Linux qualification only; no Windows/macOS, all-features,
  benchmark, latency or other-platform-green claim. Existing ignored tests were
  not exercised. Prior Windows exceptions do not turn into refreshed green proof.
- Actual process → PTY/ANSI → pyte cell/text proof, **not** native graphical
  terminal font/glyph/emoji/CJK/combining-width qualification.
- Three separate startups, **not** a same-process color-mode transition.
- History configured to 1,000 messages, not native 50,000; no child-focus,
  performance-counter or later c6la/zj0m acceptance claim.
- No external account/model turn, real tool filesystem operation, host terminal
  callback request, permission grant or real credential access. Captured
  `rawInput.command` and `terminalId` are inert content, not executed requests.
- Packaging is complete with **no missing input prerequisite**. Main must add
  exactly the manifest's four new files, make the documentation-only PR189
  commit/push, and own subsequent PR CI/publication decisions. No rerun was
  performed merely to confirm already reported results.

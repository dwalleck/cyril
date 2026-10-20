# cyril-zj0m — actual refreshed-head verification package

This is the canonical **new** documentation/evidence-only refresh package for
HEAD `7972f884fa35de6a0cffa8cb7e78349af4b7d3f3`. Start with
[`handoff.json`](handoff.json); publish only the eight files named in
[`publication-manifest.json`](publication-manifest.json). Main owns the separate
documentation-only commit, push, PR/CI adjudication and tracker actions. The
packaging worker ran no source checks, formatter, product, consumer or benchmark,
and changed no existing record, production source, README or tracker.

## Exact evidence identities

| Layer | Pin / authority |
| --- | --- |
| Qualified HEAD | `7972f884fa35de6a0cffa8cb7e78349af4b7d3f3` |
| Actual qualifier source tree | `858bfff965633bb3b876a13a701457936af17ec9` |
| Private default-feature Linux App SHA-256 | `27f0642339b92a6e6f2330917060b9a50172bb5b5083821c7eec528014572ff0` |
| App size / profile | 101,966,368 bytes; dev opt-level0/incremental; default `[kas]`, voice off |
| Source manifest SHA-256 | `1ee93d2cb35d46e2368d741c9519a3aba45cfd36db63d342ddcfa54dac82c395` |
| Separate static review | ReviewRefreshedResultStacks; correct, confidence 0.97, no findings |
| Review first parent | `7a19fc742f109411c421af296addc5fe4db4d42b` |
| Review merged parent | `aca1aa503a5183c1183ec3bb62b3d61f0bdb8b77` |

The **qualifier** establishes the actual source tree and private artifact identity.
The separate reviewer inspected HEAD/first-parent static integration only; it did
not independently inspect the qualifier tree/private artifact or execute checks.
New documentation files will change publication-tree metadata. No claim is made
that a later publication commit/tree was the tree on which these gates ran.

## Archive and integrity

- Archive: [`verification-evidence.tar.gz`](verification-evidence.tar.gz).
- SHA-256: `13cd101ddc2e694d6d096866d0ebc84713bbd0e93e6b67cb05c339ec73212203`.
- Size: **782,586 bytes**; **79 regular-file members**; no directory entries.
- Archived `index.json` is byte-identical to published
  [`archive-index.json`](archive-index.json), SHA-256
  `d75bd394f936d1afde456094e610d625837225f35cd62fd8f16073a17518acf3`.
- Every one of the 78 nonself members has original-input provenance, SHA-256,
  byte count and mode in the index. The index's own digest is stored in the
  external handoff/manifest to avoid recursive self-hashing. The manifest hashes
  all seven other publication files; its own digest is supplied externally to Main.
- Archive selection is an explicit file whitelist, not a recursive copy of any
  scratch root. Supplied evidence/harness/fixture bytes are unchanged. Tar member
  timestamps/owners and gzip timestamp are normalized; executable script modes
  are retained. No compiled executable is included.

## Fresh repository qualification — already completed PASS

All five original commands are archived with cwd, timestamps, exit codes, raw
log paths/hashes in `gates/canonical-handoff.json`. Their cwd was
`/home/dwalleck/repos/cyril-wt-perf-cyril-zj0m`; the qualifier used shared
`CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`. These are records of the
completed qualifier's work, not commands rerun by this packaging assignment.

| Original command | Observed result | Archive member |
| --- | --- | --- |
| `cargo fmt --all` | PASS, exit 0; actual source tree unchanged by initial format | `gates/00-initial-fmt.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 | `gates/01-clippy-workspace.log` |
| `cargo test --workspace` | **2,848 passed / 0 failed / 13 ignored**; ignored are unexercised | `gates/02-test-workspace.log` |
| `cargo build -p cyril` | PASS, exit 0 | `gates/03-build-default.log` |
| `cargo fmt --all --check` | PASS, exit 0 | `gates/04-fmt-check.log` |

Test totals use 42 top-level summary groups and exclude six nested child
summaries to avoid double counting; measured and filtered-out totals are zero.
The authorized `gates/environment.json`, `gates/qualified-binary.json` and
`source/qualified-source.source.json` preserve toolchain/default-feature,
binary and whole-source hash provenance. Private Git indexes and unrelated
qualifier artifacts are excluded. Default-feature Linux only: no all-features,
other-platform or fresh performance qualification is asserted.

## Separate accepted review — not the fiv1/x5 joint review

[`review.txt`](review.txt) is the exact separately completed Zj review from
`local://zj0m-refreshed-review.txt`; [`review-provenance.json`](review-provenance.json)
records its pins, acceptance provenance and limits. Archived counterparts are
`review/accepted-review.txt` and `review/provenance.json`.

The review found no provable patch-introduced semantic conflict between the
retained selected-first-Diff cache and the inherited output alias/error fallback.
It checked cached summary/body dispatch versus independent output extraction,
Failed error precedence, owner-file preservation, the self-contained captured
fixture regression, and the manually resolved README's two intended contracts.
Verdict: **correct**, confidence **0.97**, **no findings**. This is a new
merge-delta static review at the head above, not a reused older verdict. It proves
neither fresh timing nor independent inspection of the qualified private binary.

## Fresh real-process consumer proof — both first attempts PASS

`consumer/zj0m-refreshed-go.json` preserves Main's explicit per-head scoped GO;
`consumer/zj0m-refreshed-qualification.json` is the owning detailed summary with
both exact executed commands/cwds, hashes and safety limits. This is actual
refreshed-binary evidence, **not a transfer of an old PASS**. Archived GO is
historical provenance, never authorization for a future replay.

### 1. First-Diff cache/render/approval integration

`consumer/zj0m-refreshed-truecolor-01/result.json`: **PASS**, 13 screen/cell
checkpoints, 18 ACP wire frames, `error=null`, `unexpected_host_callbacks=[]`.
The raw ANSI SHA-256 is
`effe300117612a7e95f3762bef66b96c48e42fc2f88f3239dd300850c571d41b`.

| Checkpoint stem (both `.txt` and `.cells.json`) | Actual exercised observation |
| --- | --- |
| `z01-write-start` | Owned Write start enters the real App/tool ledger. |
| `z02-write-incremental` | Explicit in-progress edit update precedes completion. |
| `z03-write-completed` | Selected first-Diff header is exact `+2 -1`; context/delete/insert body and line gutters render; changed second Diff is excluded. |
| `z04-write-resized` | Actual PTY resize 160×48 → 100×32 → 160×48 preserves the selected completed Diff. |
| `z05-unchanged-first-changed-second` | An unchanged first Diff suppresses changed counts/body despite a changed second Diff. |
| `z06-replaced-diff-source` | Actual source replacement restores exact counts and `new-first` / `new-second` content. |
| `z06a-retained-diff-new-palette` | Committing Gruvbox changes retained added-line style from fg `008000` / bg default to fg `98971a` / bg `282828`. |
| `z06b-diff-scrolled-out` | Seventy padding paragraphs put the completed Diff actually outside the viewport. |
| `z06c-diff-browsed-back` | Actual PageUp returns the retained exact counts/body with the new palette styles; PageDown returns follow. |
| `z07-pending-comparison` | Seeded pending Diff joins partial permission by matching session/tool ID and displays `PENDING_OLD` / `PENDING_NEW`. |
| `z08-pending-resized` | Actual resize preserves the pending comparison. |
| `z09-completed-and-rejected` | Only `reject_once` exists; actual response to `owned-deny-only` selects `optionId=deny`; palette returns to Cyril Dark and chronology completes. |
| `final` | `WRITE_CHRONOLOGY_AFTER` and actual turn/prompt completion are retained. |

The archive includes exactly the eight safe base files, nine control/ack pairs,
13 text/cell checkpoint pairs and two executed harness snapshots (54 files).
Base files are result, safety, safe final config, raw ANSI, cast, events, wire and
peer state. The script SHA is
`c9ce883004155aef05e5a1dd68723be16e3507cc2873aff977e3cb554f34ec2a`;
peer SHA is
`6c5b5b64f1e5d4ec306b6028e299a96340891961dfaf0cdb046dc2b1240f9cfe`.
Those are the executed snapshots, not a mutable later harness. The staged App
hash matches the qualified private binary. The App has private HOME/XDG/ws,
an empty private PATH and a whitelisted environment; none of those trees or the
staged App is archived. A seeded owned pending ledger is a display/join/reject
fixture, not proof of a real file edit or actual permission grant. This smoke
makes no Myers/work-counter, timing or complexity claim.

### 2. Exact approved KAS Completed output-alias integration

`consumer/zj0m-refreshed-kas-output-01/result.json`: **PASS** through the actual
new binary, frozen existing KAS harness and owned KAS ACP peer. No reimplementation
or later harness substitution was used. The complete sanitized approved Completed
frame and captured session ID
`sess_973df141-0181-47bf-977a-2f58ca21220f` are preserved exactly. The full Completed
notification appears unchanged **twice** in `agent-wire.jsonl`; its completed
header `✓ Run the exact sed/tr pipeline requested` and body
`| XAVT_BEGIN XAVT_END` each appear **once** between `BEFORE_TOOL_ROWS` and
`AFTER_TOOL_ROWS` in the actual decoded screen. `captured_frames_preserved=true`.

The five allowed receipt files are `result.json`, `screen.txt`, `terminal.ansi`,
`agent-wire.jsonl` and `kiro-cli-argv.txt`. Only the explicitly approved runner-HOME
`safety.json` is relocated alongside them; no other runner HOME/cache content is
included. The four approved replay source/input files preserve the mini-repo
layout required by the frozen helper lookup:

| Mini-repo file | SHA-256 / role |
| --- | --- |
| `.cyril-1v25/smoke.py` | `30346dd420fdf9eafc9b00bb602e9928c471157ca310da49adaa0e1f4f63e3bb`; unchanged frozen executed harness |
| `.cyril-1v25/receipts/kas-completed-output-trace.jsonl` | `f80aae8a8fac961f4224c505d112231a669c71784c4b7b993026bd5b5a4c5e71`; exact full sanitized notification trace |
| `crates/cyril-ui/tests/fixtures/kas_completed_raw_output.json` | `0d613a49514f698c5dd3862d62a24b0fe5616f544d0078880898a441b68720cc`; exact approved regression fixture |
| `crates/cyril-core/tests/fixtures/fake-kiro-cli.sh` | Synthetic helper only; exact bytes/SHA/mode in the index, not a credential store |

The original outer runner used `env -i` and offline uv; the frozen harness created
private App HOME/PATH/XDG/workspace. Absolute Node relayed an owned Python peer.
The helper supplies only a synthetic `AT-fake` token envelope. Captured
`rawInput.command` is inert JSON, never executed; `terminalId` is metadata, not a
terminal callback. Neither run uses a real account/model, real file operation,
filesystem/terminal callback or permission grant. The trace and UI regression
fixture remain exact full-frame inputs, not a reduced alias-only example.

## Portable reproduction and provenance

[`reproduction.md`](reproduction.md), also archived as `reproduction.md`, records
both successful original invocations and relocated frozen-harness instructions.
It preserves the KAS helper's mini-repo relative layout and explicitly requires
a separately supplied qualified binary, offline dependencies, installed Node and
a fresh explicit scoped GO. The archived scripts are unchanged even where they
contain other entrypoints; those are not newly authorized or qualified here.
The frozen KAS script declares unpinned `pyte`; no complete runtime-version lock
is claimed. Reproduction instructions themselves were not executed by packaging.

## Historical completed-write proof — preserved, not refreshed measurements

The older [`../verification.md`](../verification.md),
[`../publication-manifest.json`](../publication-manifest.json),
[`../handoff.json`](../handoff.json) and
[`../verification-evidence.tar.gz`](../verification-evidence.tar.gz) remain
untouched. The older `parent-refresh-20261020.json`, original patch, workflow and
review records also remain untouched. Their original publication file lists and
source hashes are historical records; this refresh does not rewrite them.

Historical completed-write benchmark/source-behavior proof remains pinned to:

- Source tree `6241931fecb663e9bb17e5cc0e76c35c12833828`.
- Private App binary `b603b8c5ac261acb827d9a95c90010ea339abff6bbfd687ec5d9991a6732da00`.
- Historical archive SHA-256
  `05c8d8abb9ea050dcb9bf0720218dc0eeaf53a593da7038cfc2956029bc896dc`
  (1,366,757 bytes, 88 members per the preserved old manifest).

The older package retains its completed-write TestBackend benchmark, 90 raw
sample rows, original-base sensitivity, old review/consumer evidence and later
separately authorized CLI-rejection receipt under their original boundaries.
**No benchmark was rerun** for this refresh, and no old timing is relabeled as a
new-head measurement. The accepted new static review establishes cache/alias
integration only; fresh consumer behavior/gates are not performance evidence.
Old manifest bytes/SHA and all other preexisting evidence records are separately
identified in the new manifest's `preserved_historical_records` and archive index.

## Explicit exclusions and limits

- Compiled binaries; staged copies; private Git indexes; HOME/USERPROFILE/XDG/
  workspace/cache/state/data/runtime trees; credentials/auth stores/journals;
  runtime `cyril.log`; unrelated private scratch files are excluded.
- Only the authorized qualification environment log, safe final config, raw
  consumer artifacts, exact executed harnesses, four KAS replay dependencies and
  the separate Zj review are selected. No unrelated issue package or joint
  fiv1/x5 review is seized. No recursive private-directory copy is used.
- PTY/pyte ANSI/text/cells are **actual process integration**, not native graphical
  font/glyph/emoji/CJK/combining-width proof. No native50k or focused-child claim;
  max_messages=1000/default500 and seventy paragraphs are bounded fixtures.
- Thirteen ignored tests remain unexercised. Default Linux only; no all-features
  or other-platform qualification. No fresh timing/work-counter/performance claim.
- No C6 execution, C6 acceptance, real account/model/tool file operation, host
  callback or grant. No new commit/push/PR/CI/tracker action by this worker.
- The original GO and old records are provenance, not permission for more runs.
  Main owns exact-file publication and any later explicitly authorized action.

## Exact publication boundary

The explicit allowlist is:

- `.cyril-zj0m/refresh-7972f884/archive-index.json`
- `.cyril-zj0m/refresh-7972f884/handoff.json`
- `.cyril-zj0m/refresh-7972f884/publication-manifest.json`
- `.cyril-zj0m/refresh-7972f884/reproduction.md`
- `.cyril-zj0m/refresh-7972f884/review-provenance.json`
- `.cyril-zj0m/refresh-7972f884/review.txt`
- `.cyril-zj0m/refresh-7972f884/verification-evidence.tar.gz`
- `.cyril-zj0m/refresh-7972f884/verification.md`


Main's next action is a separate documentation/evidence-only publication of
exactly this allowlist on the already refreshed Zj branch. Preserve all existing
records and qualified production bytes. The old publication manifest's larger
source/README list is not the allowlist for this new refresh. This package does
not grant a new consumer/native/benchmark run and does not close the tracker.

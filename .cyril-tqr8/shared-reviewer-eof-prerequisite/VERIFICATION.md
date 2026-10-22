# Shared reviewer EOF fixture prerequisite — scoped verification

## Identity and scope

- Source commit supplied by Main: `3730d5aa843aec8b6b36e0e7d42770a4824167bd`.
- Qualified source Git **tree**, not commit: `8186d065a18643af3053ea75acaedeff0b064a3b`.
- Qualification was recorded before that source commit, on base HEAD `8e5c325ebb5dac3a4ad89f71838fd198cc177543` plus the two-file test repair. The copied source bytes match the qualified receipt. No new Git ancestry/tree check is claimed by this artifact task.
- Canonical retained receipt: `proof/canonical-handoff.json`, SHA256 `2b9c887c60fafc891b3ddecc65b4a0d8404597c23b63f832bd95e94d18009e81`.
- Result: **SCOPED SHARED EOF TEST-ONLY PREREQUISITE PASS**. This is not full-workspace, integrated Tqr, C6, native-terminal, native Kiro, auth, or current application-consumer qualification. It does not waive any earlier failure, close an issue, authorize a run, or make a candidate PR-ready.
- Historical Linux missing-EOF failure cause: **UNPROVEN**. The controlled ordering experiment demonstrates a possible failure mechanism; it does not establish that mechanism caused the historical occurrence.

The repair changes only `tests/reviewer.rs` and `tests/fixtures/reviewer_peer.py` in `cyril-workbench`. The packaged production `src/reviewer/evidence.rs` is an unchanged reference/restored snapshot, not a product change. Source copies are evidence under this package's `source/` directory; the packager did not edit the original source files.

## What the two producer cases actually compare

Read `proof/owned-producer-proof/{old-prompt,post-flush}/result.json`, the corresponding `at-gate.json`, raw stdout/stderr `.log` streams, owned fixture journals, requests and synthetic evidence inputs.

**Both cases execute the SAME CORRECTED REAL FIXTURE**, SHA256 `e0f3efab80b799bd7a052d32fda8c9e959f181a70c0ad89b69e6498f0469d04a`. They do not compare an old fixture version against a new fixture version. Both use the same shim and gate immediately after the old `journal('prompt')` returns. Only pipe-teardown ordering changes:

| Case | Controlled teardown | Observed result |
|---|---|---|
| `old-prompt` | Close the stdout reader and stdin writer while the real producer is gated just after the old prompt marker; then release the gate | Exit1, real evidence processing reached, literal `BrokenPipeError`, no post-flush readiness, no `stdin-closed`, neither EOF-window event |
| `post-flush` | Release the same gate with pipes open; wait for real evidence reads and successful `waiting` stdout flush followed by `runtime-drop-ready`; then close the same pipes | Exit0, actual `waiting` JSON-RPC notification, prompt still unanswered/no terminal event, `stdin-closed`, both `cleanup-window` and `cleanup-window-end` with `evidence_exists:true` |

For owned cleanup **only in this producer experiment**, the shim replaces the fixture's exact known120s DEVNULL sleeper command with a real descendant waiting on an inherited release pipe. Closing the pipe lets that owned descendant exit naturally. This does not establish the original sleeper lifetime/process oracle. The separate actual Reviewer bridge test retains its original120s sleeper and process-state assertions. No native agent or billed model is involved; the recorded `session/prompt` input is sent only to the owned synthetic fixture.

## Actual bridge retention discrimination

Read these retained command logs in order:

1. `proof/02-actual-bridge-current-green.log`: repaired actual bridge regression GREEN.
2. `proof/03-actual-bridge-early-evidence-cleanup-red.log`: the named production-policy violation compiles and the single intended test executes. Its exact RED is `cleanup-window: evidence removed before core completion`, **left `Bool(false)`, right `true`**, exit101. It is not a compilation failure, missing-readiness failure, missing-EOF event, or unrelated panic.
3. `proof/04-actual-bridge-exact-restored-green.log`: the unchanged regression GREEN after exact restoration.

`proof/exact-production-mutation.patch` inserts only:

```rust
fs::remove_dir_all(&root).expect("owned early-evidence-cleanup mutation");
```

immediately after `let root = tree.root.keep();` in `EvidenceCleanup::drop`. It deliberately violates retention before core completion. It changes no assertion, deadline, waiter or lifecycle budget.

The exclusive original backup and successful finally/inverse-edit restoration both pin production SHA256 `626f7307fdcb0879e16aa947ce0840df57e9e262f9ba1aaf9b4a9c631efe85ff`. Mutated tree: `576fb47736e3195ab6206bac655cad00b35c7d7f`; restored tree: `8186d065a18643af3053ea75acaedeff0b064a3b`.

Retain the receipt chronology rather than conflating two files:

- `proof/early-evidence-cleanup-mutation/restore-receipt.json` records an initial rejected `tool.edit` argument shape **before source mutation/Cargo execution**. Its `inverse_specialized_edit:false` is not the actual mutant restoration proof.
- `proof/early-evidence-cleanup-mutation/restore-after-object-bridge-receipt.json` records the executed mutant's successful specialized inverse edit in `finally`: byte-exact, `inverse_specialized_edit:true`, no `git restore`, problem null. The canonical handoff records the same distinction and no producer/test retry.

The repair adds the post-flush readiness marker, reuses cached absolute evidence paths, and waits through both evidence removal and the EOF-window end under the existing15s deadline. Both retention assertions, eventual cleanup, PID/child checks and empty-parent assertion remain. Missing events remain hard failures with the specific event named. The5s event/readiness,1s startup,100ms stall,400ms fixture window,15s cleanup,120s actual bridge sleeper and2s production grace budgets were not increased.

## Affected qualification and independent review

The canonical receipt records seven qualification commands and their exact cwd, target environment, timestamps, exit codes and log hashes. The top-level producer command succeeds while containing the intentionally nonzero old-order case; the actual bridge mutant exit101 is the single expected failed qualification command, not an unexpected gate failure.

| Retained gate | Result |
|---|---|
| Producer ordering command | PASS; expected old-order failure and post-flush success |
| Actual bridge current / named mutant / exact restored | GREEN / intended retention-specific RED / GREEN |
| Full affected `cyril-workbench --test reviewer` target | **37passed,0failed,0ignored** |
| `cargo clippy -p cyril-workbench --all-targets -- -D warnings` | PASS |
| `cargo fmt --all --check` | PASS |
| Full workspace | **NOT RUN in this scoped attempt** |
| Integrated Tqr/C6, actual application UI or native/auth consumer | **NOT RUN / NOT QUALIFIED by this package** |

Independent source-and-recipe review by `ReviewReviewerEofFixture`: no findings, correct, confidence0.94. It did not execute the producer, mutation, tests or gates and did not establish historical causation. Its transcription is `review/source-review.log`. Empirical results belong to the separate qualifier receipt, not reviewer inspection.

The exact executed `proof/reviewer-eof-fixture-proof-recipe.py` has SHA256 `06a20aa55e588a75ae96b5c047f1b76f99ef9fdaca2bdde0185de9af8bbe9511`. Its original preparation-only banner describes its author, RepairReviewerEofFixture; it is preserved unchanged. QualifyPerformanceTrees later executed those exact bytes as recorded by command01. The clean repair patch SHA is `9c04a71ab9ca59f0c8d8ccd6a1db5d7f067337489ca676f996eaf8ef8d026baf`; the separate qualified native diff is retained too. Their byte differences are not represented as different repairs: the canonical receipt reports exact private-index patch equivalence and the two qualified source postimages.

## Portable allowlist and artifact-only validation

`INDEX.json` maps original explicitly authorized paths to portable copies. `MANIFEST.json` lists every payload file with SHA256, byte length and mode. The archive contains exactly that payload plus `MANIFEST.json`; its checksum sidecar pins both the archive and manifest. `VALIDATION.json` is an outside-archive, non-self-referential receipt of the requested artifact-only validation, not another product gate.

Producer stdout/stderr files originally named `.bin` contain only UTF-8 diagnostic text; they are copied byte-for-byte with `.log` names. Synthetic fixture journals contain owned events and environment **names**, not environment values. No raw environment snapshot, private Git index, broad source-manifest dump, credential store, private HOME, private Cyril journal/log, compiled binary or unknown scratch file is packaged. Source/recipe/patch/receipt bytes are not silently rewritten or redacted.

Only this new artifact directory was written. Existing Tqr B/C2954 publications were neither refreshed nor modified; their historical partial/workspace-blocked scope is not upgraded. This worker ran no Cargo, tests, lint, formatter, producer/consumer proof, mutation, native/account/auth operation, commit or merge. Its only checks are the explicitly requested artifact hash/size/membership validation against retained receipts and this manifest.

Main owns source/proof-doc commits, normal merges and any subsequently authorized integrated qualification. Nothing in this package authorizes rerunning its recipe or recorded commands.

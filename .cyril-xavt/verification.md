# cyril-xavt delivery verification

## Current parent-integration checkpoint — held afa2 + dbb10969

**PASS for exactly the sole HOST coordinator's three scoped Linux commands on the assembled parent-integration tree; new resulting-X-head CI remains PENDING.** Qualification ancestry is HEAD `afa2b9ecc84e660bdb471e6f1b6c4393c598905b` plus held MERGE_HEAD `dbb109697c01d5984222a56a901384faa1c10ec6` (updated PR190). These are held qualification pins, not an assertion that a new publication commit already exists.

The byte-preserved canonical `xavt-afa2-dbb-parent-integration-scoped-qualified-handoff.json` has SHA-256 `7bac20b649ab62341c7f56297cbdbb3cd0449b9c232f3ff336e702436b59f3e6`. Its fresh actual-source inventory contains **532 inputs**, descriptor `ce38fa24a3b5924b41fba1e4931afa05d036772de4b8191e484a1575108e6360`, inventory SHA-256 `03ed73be4ba0825c7ed76a7d8ad3a2f5ab75a79344be76d156e530008d0a55e2`. [Compact parent-integration evidence](evidence-parent-integration-afa2-dbb10969-index.json) retains only that canonical, its source inventory, three complete logs, and a reference-only map to existing evidence. Already-imported PR199/oracle and EJ proof is not duplicated.

Main's recorded import boundary contains 20 cached paths and exactly one actual source path, `crates/cyril/src/capture_forwarder.rs`. The HOST canonical records 531 prior input rows unchanged and that whole file moving from SHA-256 `b80f4de46766295ddd06940cae80abed68eef0e12494a969ba1ac4f70dccaebf` to the exact imported PR199/EJ SHA-256 `4fb824a80afdbf4cba3174e5b3fd55d625562d84a1d2357455c39cd9ee73fefc`; both boxed ReplayFrame source pins remain unchanged. These are retained Main/HOST identity findings, not additional worker source checks.

| Recorded command | Actual bounded result | Portable complete log |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS, exit0; no formatter applied | `qualification/logs/01-fmt-CHECK.log` |
| `cargo nextest run -p cyril --bin cyril -E 'test(capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown) \| test(app::tests::current_runtime_contract::shutdown::c10_shutdown_waits_bridge_then_drains_capture_then_stops_memory)'` | PASS, exit0; 2 passed, 467 skipped; both exact C9/C10 cases executed | `qualification/logs/02-default-C9-C10-nextest.log` |
| `cargo clippy -p cyril --all-targets -- -D warnings` | PASS, exit0; strict default App all-target Clippy | `qualification/logs/03-strict-default-App-alltargets-Clippy.log` |

The original batch-oracle [NONE / correct / confidence0.96 review](../.cyril-n3j7/evidence/capture-batch-oracle-20261022/source-review.md), prior X ordinary modal source review **NONE / correct / confidence0.97**, and [Replay repair review](review-decisions.md#pr191-f4--windows-default-replayframe-layout) **NONE / correct / confidence0.97** remain original reviews, not a new parent review. The earlier 532-input modal/replay qualifications, **13 replay tests /456 skipped**, original REDs, and actual owned-consumer/native boundaries below retain their recorded source and execution scope. Original fault discriminations and owned-consumer cases were not rerun. The original C9 153.901529ms CI FAIL remains FAIL, not a flake classification. The 100ms batch fence and two-second drain remain the original requirements; no numeric internal batch duration is claimed from whole-test elapsed time.

[CI39854475502](https://github.com/dwalleck/cyril/actions/runs/39854475502) is retained **prior component SUCCESS** at `afa2b9ecc84e660bdb471e6f1b6c4393c598905b`: 15 jobs succeeded and one conditional tooling job skipped. At that exact prior component, Windows DEFAULT strict Clippy and all six formerly unrun Windows product steps succeeded. This resolves the earlier repair-component CI obligation without altering the original failure/skip receipts; it is **not current parent-integration-head CI**. Main owns the resulting publication commit, its fresh CI and final propagation.

This metadata-only refresh performs no Cargo/formatter/application/native/auth or additional product check and changes no source, runtime input, dynamic summary, fixture, tracker or assertion. No private App copy, real-provider execution, native glyph/reflow, MCP13, final-assembly or whole-issue acceptance is added. The earlier sections below preserve their time-local “current/pending” wording as historical records; this checkpoint owns the latest qualification/CI distinction.

## Current PR191 qualification blocker

**PASS for exactly three scoped Linux repair commands; fresh repair-head Windows CI remains required.** Pre-repair PR191 head `75b6d148dd1936210399e725b17a6be75dce9b69`, CI39847823115, retains its Windows DEFAULT strict-Clippy FAIL on inline `ReplayFrame`; six subsequent Windows product steps did not run. The original hosted failure was not rerun, waived or relabeled green.

[Review decision F4 and its bounded repair record](review-decisions.md#pr191-f4--windows-default-replayframe-layout) own the diagnostic, Main DG, boxed replay-only representation, complete caller migration, unchanged production reachability, original ordinary review, and the sole HOST-coordinator three-command qualification. Its current full532 descriptor is `e8728c859208f056dc17fbfd44db5611ee2ed5a01d9fe9caf1ae2589de08db9e`: exactly two source rows changed, other530 and Cargo.lock unchanged. No production domain-type reshaping, live-path allocation, lint/config/platform weakening or test-assertion change is part of this repair.

The completed qualification is initial fmt CHECK PASS without format application, **13 replay-dependent App tests passed /456 skipped**, and strict explicit-KAS App/Core all-target Clippy PASS. [Minimal F4 portable proof](evidence-replay-repair532-e8728c85-index.json) retains the exact canonical, full532 manifest, three full logs, original source-review output, hosted failure/six skipped-step evidence and two-file diff, without binaries/full trees/nested archives. Ordinary source review is NONE/correct/confidence0.97; its review-time pending wording remains historical. No further Cargo or application-runtime execution was performed during this metadata refresh. Main must publish the repair, use its actual new HEAD for final propagation, and obtain fresh current-head CI; neither Windows, the interrupted final matrix nor final integration is PASS here. Records below retain their original historical source/results, with prior proof archives/indexes unchanged.

## Current held-af78 checkpoint

**PASS for the explicitly bounded current qualification and both owned consumer cases below.** Main still owns completing the held parent merge, committing, publishing PR191, and final cross-batch/platform qualification. This is not whole-issue, native-glyph, MCP13, or final-assembly acceptance.

### Source, binary, and approval boundary

- Worktree remains `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt`, held HEAD `bc6f48a21016320cc6491356f880f37463cf8c6e` plus MERGE_HEAD `af78b1615bd5b97b4c702542fd3ab193151bd4c1`. These are ancestry pins, not a claim that HEAD alone contains the assembled source.
- Current **532-input** descriptor: `b6a846bee5f24ab0c2d0b1a48f066dd69ae575abb82bc7033c3f42a56ff48308`. Inventory SHA-256: `3dc4772aa56a9cc5551c2a105ae3b13010cf2c6233286d5f8bd09cbdc858df16`. README is separately owner-pinned, not a claimed runtime input.
- Current private default App: `/tmp/cyril-xavt-overlay-wide-boundary-repaired-default-App-qualified-20261022T192920439038Z`, SHA-256 `8e2b7c3a82b360d4ca4f1c1b6410084bf3a5a9e1673612d04bfd6c61739ee7f0`, 102,230,376 bytes, mode0755. No binary is published or substituted from the shared target.
- Completed current qualification: `overlay-wide-boundary-repaired-X-local-qualified-attempt1-handoff.json`, SHA-256 `e6e36df131049a29fb154faf8968233de4da2c53df41d85e7d97d0e9e4aead89`. Its eleven current owner pins include README plus ten UI files; they are not the old eleven X-feature files.
- Original [design](design.md) requirements and approvals remain intact. The additional **Main DG** `approved-overlay-wide-boundary-repair.md`, SHA-256 `475c6c4ea2ff7f896b3f50397f4a1336cfbfa508d6b28f72d726248670bd5540`, owns the modal repair: evict whole painted spans intersecting the clipped rectangle at either edge, arbitrary widths, preserve disjoint cells/styles/other rows, use released CellWidth without production allocation/copy/normalization or backend/diff-flag changes. All twelve current Clear callers use existing `widgets/modal.rs`; the final MCP thirteenth caller remains separate integration.

All named receipts, approvals, manifests, logs, exact consumer instruments, and raw records resolve through the [current portable index](evidence-current-modal532-b6a846be-index.json); original receipt bytes and their absolute/local references are preserved rather than rewritten.

### Retained evidence and the observed failure

- Retain the prior exact-ten-command PASS, canonical `873df2d9…`. Its source scope was **530 execution-pinned inputs plus two posthoc fixtures**, not a retroactive532 qualification. The current532 inventory included both known fixtures before the fresh default UI/App execution.
- All197 current Core rows and root manifests/lock/toolchain/config match that retained checked state. Current Core dependencies were compiled through UI/App gates; Core-only units, the old ten-command matrix, and no-default jobs were **not replayed** for the UI-only modal repair.
- Preserve original captured892 PASS and original controlled FAIL (`xavt-af78-owned-runtime-handoff-b27bd94f.json`, `4961eea3…`). The controlled case failed at the reason wait before any narrow control: zero observations; raw ANSI and the exact frozen decoder/predicate reproduce an empty extracted body because `(8,12)` contains `ﾞ`, not the expected border. The internal Buffer skip mechanism remains **source-derived inference**, not instrumented App state or a timing/scheduler diagnosis.
- The original producer RED (`ef10a364…`) contains **two decisive released-backend border-emission failures at original render.rs1112**. The third failure at1142 is a separate restoration-oracle error, not another emission RED or proof that every old control-loop fixture completed. The corrected width-one ordinary-combining controls preserve disjoint cells/styles and require no close re-emission; actual crossing border/diff/backend/restoration obligations remain.

### Current modal qualification and source review

The initial fmt CHECK **failed** on initial source `c2fd9799…` and remains immutable (`2ce6db27…`). Main authorized one normal formatter confined to seven of the eleven owners. The following **seven subsequent command records succeeded**; the formatter is not misreported as an additional test gate:

| Command | Actual result on the recorded source |
| --- | --- |
| `cargo fmt --all` | One authorized formatter; current descriptor becomes `b6a846be…` |
| `cargo fmt --all -- --check` | PASS after formatting |
| `cargo test -p cyril-ui render::tests::overlay_occludes_chat_grapheme -- --nocapture` | 3 passed, including unchanged/changed real chat leads and literal width controls |
| `cargo test -p cyril-ui widgets::modal::tests::clear_ -- --nocapture` | 7 passed: both edges, arbitrary widths, clipping, preservation, persistent backend/restoration |
| `cargo test -p cyril-ui -p cyril` | **1,574 passed, 0 failed, 3 ignored**; focused cases overlap this run and are not added as unique tests |
| `cargo clippy -p cyril-ui -p cyril --all-targets -- -D warnings` | PASS |
| `cargo build -p cyril --bins` | PASS; fresh private default App copied before any other build |

Independent ordinary source review: **NONE / correct / confidence0.97**, receipt `overlay-wide-boundary-source-review.md`, SHA-256 `1e578f47aa86b01877bf86423b11fe34f281228c233b87341ebf45040bd62dce`. It reviewed the actual eleven-owner repair and affected producer/composition/released-diff/backend interactions. Main retained semantic applicability after the authoritative rustfmt-only normalization; this is not a new postformat review or raw byte identity. The review-time “qualification pending” language is historical chronology, not a replacement for the later receipts.

### Current two-case owned consumer

New Main GO and passed entry guards bound the current532 source/private8e App to a **binding-only copy** of b27: new harness `b2484210…`, exact diff `e82544f7…`, changing only original lines36–38,166,493–494. Case payloads, keys, predicates, budgets, captures, and cleanup are unchanged. Each case ran exactly once; controlled ran only after captured scenario and ownership cleanup passed.

| Case | Actual exercised result | Retained records |
| --- | --- | --- |
| Original captured892 | Exact full command+cwd; draft/navigation/reason retained; cwd-only/tiny Enter refused; one RejectOnce, no execution | 12 observations, 13 scenario captures |
| Controlled `{"command":"ｶﾞ\n:","path":"界界"}` | Home and End Enter inert in both phases; identifying two-row path retained; width12 whole pair emitted raw+decoded; restored80×40 full command+path+selected Deny before one RejectOnce | 10 observations, 11 scenario captures |

Canonical actual-capture handoff: `xavt-overlay-wide-boundary-owned-runtime-actual-captures-handoff.json`, SHA-256 `16906a511b6161cee06645138fd11a888dc3047860907d8e2dc5df13188c2599`. Exact evidence inventory: `49f25d414274ddf4ba5182cf06bf0b67f50ca8dea7b5d4bb60f32edc8b6b7109`.

Both original driver waits and actual App waits returned0. Node/peer exitcodes remain **UNOBSERVED**; pidfd readiness is not an exitcode. Both ordinary close and independent proven-owned cleanup were COMPLETE, no signals/survivors/unproven markers, with scratch removed only after attestation. The single143-check entry (`5dacd4c5…`) and single143-check postflight (`53b44b6b…`) passed. There were **22 observations, 24 scenario captures, plus two cleanup final screens**, no retries or extra waits. The first metadata classifier omitted letter-suffixed03b/07b/07c; its correction changes only classification, never raw bytes/assertions/runtime.

These were actual default-feature Cyril/KAS-route owned-peer cases, not real provider/account/auth/model/tool execution or native CJK glyph/reflow evidence. No historical mutants were rerun or newly claimed. Original live-KAS limitations and the separate PR183 output-alias exclusion below remain.

### Current portable evidence and publication

[Current compressed evidence](evidence-current-modal532-b6a846be.tar.gz) and its [member/alias index](evidence-current-modal532-b6a846be-index.json) retain canonical/manifests/logs/reviews/approved control contracts and original FAIL/new PASS ANSI/screens/wires/results without binaries, full trees, private HOME/token-helper logs, nested archives, or repeated identical payloads. The original delivery zip/index remain historical and unchanged in payload meaning.

[publication-files.json](publication-files.json) now owns the current delta against the held index, not the old21-path X-feature selection: eleven already-qualified repair owners plus the explicitly selected metadata/new evidence. Its own path is selected but its recursive self-hash is external in Main's final handoff. No source edits were made during final publication preparation.

## Historical October20 record

The remaining original receipt preserves the older860/private338/.98 proof and native limitations. Those identities and results are historical, not qualifications of current532/private8e; they are not rewritten into current PASS.

## Result and source boundary

**Implementation, repository qualification, independent source review, and bounded consumer handoff: PASS within the scopes below.** Main owns the remaining commit, tracker, and own-PR publication actions. This receipt does not declare those publication actions performed.

Actual worktree: `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt`, branch `fix/cyril-xavt`. Published EJ base: `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` (PR190), following un4p PR188. Common test-only auth prerequisite: `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69`; it is not an X feature file. Reconstruction history and the four formatting-context hunk dispositions are in [delivery-handoff.md](delivery-handoff.md).

The source was frozen after parent bg128/artifact860; finalization changes only README and X-owned documentation/evidence. **No contemporaneous source-tree hash was recorded during860.** Provenance therefore consists of the actual worktree, published parent and common prerequisite, approved eleven-file delta, exact successful gate chain, qualified binary digest, and later frozen-file identity. The post-gate digest below must not be presented as a gate-time hash or as a hash of a later published commit.

- Historical approved patch SHA-256: `9cfedec4145a65a367e274c569159630f59d105a3debbe933520390bd28ae662`.
- Historical corrected reconstruction/pre-format eleven-file manifest SHA-256: `f37fd6de5ea9a399bf86490fa57afb6fa3e4007e9279188054acb568b0e57209`.
- **Post-gate frozen eleven-file manifest SHA-256:** `65db1421f0740cc68938bbfa2efda51afde2d2ceb0919f1ba21bd3f0f4ef9a1f`. Formatting changed traits.rs, approval.rs and app.rs relative to the preceding manifest; this is not a new behavioral repair.
- **Actual qualified binary:** `/tmp/cyril-xavt-delivery-tested-20261020`, SHA-256 `33891699d8cfa71f5ce8e16fae3675b23c4e104d384a7f7d69416880ffebfb84`. It is private and excluded from publication.

The following ASCII manifest is lexicographically ordered, with two spaces after each lowercase digest and one LF after every line, including the final line. Its bytes are also retained as `source/source-freeze.sha256` in the portable archive.

```text
4ddefb5006ae3af60b67fc54e0e6abba8fd61c39ef3d3f605c7cdcf4168ee865  crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/always_allow.rs
43638a819e0aef1efb4a2a78fef5fb7dae3a54c2c30b1c27266e7c0f0f3f1644  crates/cyril-core/src/protocol/bridge/tests/harness.rs
fa03cec810c04914e04256eeca725724b73db5ecd5ebe0493200d1599e44d935  crates/cyril-core/src/protocol/convert/kas.rs
ff0efe842af37d5964ad76a9c2071746adb66b3ad9f8da15865c79d6b49799e3  crates/cyril-core/src/protocol/domain_mediator/mod.rs
85cb757f0ed84189ff7eab03716cb668038d1b5607e44ae27fbe58099cda2f59  crates/cyril-core/src/types/mod.rs
8c10073c6eedd28a0eb390d4d29e7bab57a928c5250802591536753e15ea17b6  crates/cyril-core/src/types/tool_call.rs
d1c7269f08e9a8ef2ebb1e39ac488b57854d0185283c9f97f096d942df70954a  crates/cyril-core/tests/fixtures/kas_permission/shell-preview-2.32.0.json
b415e01ba9b6c1b77d721bd6ad7522d2475b890c3969652b94a2764dcd1505b4  crates/cyril-ui/src/traits.rs
946ec9e2b951ff0a248446e006772f9896f2f02b62b78792fd81e2a43f368a63  crates/cyril-ui/src/widgets/approval.rs
d38fe62a05dd135dd6287d5e4618307ae03da330365ad2a898a346a222022784  crates/cyril-ui/src/widgets/workflow_plan.rs
6dedb23635c9fe9fd6ae82f186098eeb4995defce44aa16222032c31cdbcd6f6  crates/cyril/src/app.rs
```

## Actual-delivery repository gates

**PASS — parent bg128/artifact860, exit0.** Full authoritative output is portable at `gates/860.log`; Main supplied the exact command below, retained as `gates/860.command.txt`. The worktree was the actual delivery worktree named above, not the historical combined tree.

```sh
env -u CARGO_TARGET_DIR cargo fmt --all && touch crates/cyril-core/src/lib.rs crates/cyril-ui/src/lib.rs crates/cyril/src/main.rs && env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo clippy --workspace --all-targets -- -D warnings && env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo test --workspace && env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo build -p cyril && env -u CARGO_TARGET_DIR cargo fmt --all --check && cp /home/dwalleck/repos/cyril/target/debug/cyril /tmp/cyril-xavt-delivery-tested-20261020 && sha256sum /tmp/cyril-xavt-delivery-tested-20261020
```

| Obligation | Observed result |
| --- | --- |
| Workspace/all-target Clippy with warnings denied | PASS |
| Workspace tests, including doctests | **3,117 passed, 0 failed, 13 ignored**; ignored tests are not claimed executed |
| Default-feature `cargo build -p cyril` | PASS |
| Formatting and final `cargo fmt --all --check` | PASS |
| Copied qualified binary identity | SHA-256 above, matching final860 output |

The finalization worker ran none of these repository checks. Parsing retained results and hashing explicitly named artifacts did not rerun a check.

## Review and repair provenance

**PASS — fresh actual-X independent source review, correct/confidence0.98, no actionable behavioral findings.** Canonical portable receipt: `reviews/actual-source-review.md`, SHA-256 `61c62bac86aede3d71c2cd571437e355365cdc000c57541f1c27365256754f12`. ReviewShellApproval inspected the actual eleven-file delivery delta against published EJ, including all four reconstructed approval-widget hunks. This is not approval inferred from the earlier combined implementation. The reviewer ran no checks and does not independently attest runtime proof or a gate-time whole-tree hash.

The receipt explicitly retains applicability across final formatting and F3: only `/without_cwd/permission/_meta/kiro/consent/triggeringResource` is restored to the captured literal `echo SLEPT`; actual raw command/resource remain `sleep 45 && echo SLEPT`. Production, preview inputs and assertions are unchanged. No later semantic edits are covered by that applicability statement.

Historical sensitivity and repair evidence remains visible, not relabeled as actual-delivery gates:

| Receipt | Source and outcome |
| --- | --- |
| 454 | Pre-repair combined behavioral RED: 82 existing App cases pass; 8 whole-one-character Allow/reason cases fail; all 4 FE0F command/cwd quote-reachability cases fail. These are product-response/render failures, not setup or checker failures. |
| 542 | Repaired90 App cases and workspace3,117/13 ignored pass. The `xavt_review_` filter matches zero: **not** focused FE0F GREEN. Clippy fails on ReplayFrame layout; build does not run. |
| 575 | Correct explicit four-case `xavt_shell_fe0f_` filter passes. Interim boxing exposes ChatMessageKind layout; not final gate completion. |
| 688 | Final approved private `Option<Box<ShellCommand>>` owner-level representation and retired replay workaround pass assembled combined gates. Historical combined source only. |
| 860 | Actual reconstructed, F3-corrected, formatted X source passes the full chain above. This is the actual-delivery repository receipt. |

The initial combined correctness verdict was **incorrect** because of F1/F2; repaired independent rereview and fresh actual-source review are **correct**. [review-decisions.md](review-decisions.md) retains decisions and bounded repair history. No reported RED was rerun merely to reconfirm it.

## Original real-KAS acceptance — historical combined source

The original immutable nonsecret archive is retained byte-for-byte as `native-combined/original-cyril-xavt-live-nonsecret.zip`, SHA-256 `b4a68a15ada12b2cc73cf13b49b2d3c58ca353bfc8a3c5a74eef7e899689d9e7`. Its receipt identifies the tested **combined** binary SHA-256 `5cbd513e717bbc167d494b656e3cf40393f7bb82ae5bc04a2cc206391d087b08`, real Node/KAS bundle invocation, owned PTY, and isolation boundary. It is not the actual-delivery338 binary or a native desktop retention/reflow qualification.

Observed once, before the original one-time Allow:

- The joined wire supplied the complete **892-byte** command, SHA-256 `1c9636e678e54cb857c2b9e7079c45df8e0870995c1e43a51834d4902d56292c`, and cwd `/tmp/cyril-xavt-live.ZiHasL/ws`.
- Original session ID `sess_973df141-0181-47bf-977a-2f58ca21220f`; tool-call ID `run_command_toolu_bdrk_014qXfCGiFdVfYrawhVgy3TQ`; permission request4 had no rawInput and relied on the matching preceding pending tool call.
- Actual approval modal pages1–11/18 and8–18/18 reconstruct the full command, all24 identity-sed stages and final tr stage, plus exact supplied cwd. The title was not used as payload evidence.
- Navigation, reason draft/Escape, cwd-only and tiny-viewport refusal were retained before one offered AllowOnce. Completed wire then reports exitCode0 and exact stdout `XAVT_BEGIN XAVT_END\n`.

**Limits:** stdout did not render in Cyril; only the successful completed-tool header rendered. During prolonged inspection, ordinary token-helper callbacks timed out; final prompt returned -32000. Root cause is unknown: neither a clean final-model turn nor an auth root-cause diagnosis is claimed. The separate user-approved PR183 output-alias repair has its own controlled old/new proof and is **not included in X**.

The full parsed completed frame was retained afterward as the separately labeled `native-supplement/completed-tool-call-nonsecret.json`, SHA-256 `0d613a49514f698c5dd3862d62a24b0fe5616f544d0078880898a441b68720cc`. **It was not in the immutable original zip.** It corroborates output shape and execution, not rendered stdout. No second native command execution, account contact, or model call was performed or authorized during finalization.

## Actual-delivery exact captured-frame consumer proof

**PASS on the first attempt**, using the qualified338 binary and the established owned ACP-peer/PTY approach. Original evidence directory: `/tmp/cyril-xavt-delivery-owned-20261020-a`; all selected nonsecret results, frames, terminal bytes, captures and exact harness copies are portable under `actual-delivery-owned/`.

The peer reconstructs only ACP transport envelopes around safe original wire records4/5. It preserves the original tool-call pending status, execute kind, title, command/cwd, original session/tool IDs, four offered options and request4, including **rawInput absence on the permission frame**. `session/new` returns the original captured session ID; no frame rewrite, helper-domain projection, synthetic shell metadata, or completed frame is injected. Safe original wire digest is `40149c755431b350c36aeaa96fbeb1a0ed14e0346dbd8116b93c69942c31bae9`, identical to the original zip member. Fake auth uses the existing immutable repository `fake-kiro-cli.sh` harness prerequisite, isolated HOME/data/PATH and an owned Node launch wrapper, not real KAS/auth.

| Observation | Retained evidence and result |
| --- | --- |
| Full actual modal payload | `01-command-first-page.txt`, `02-command-page-01.txt`, `02-command-page-02.txt`; pages1–11/18 and8–18/18. Overlap matches; fixed-width drawn body cells reconstruct the exact892-byte command and cwd. `render-wire-reconciliation.json`: PASS; all24 segments present twice, no title-derived evidence. |
| Draft/context/selection | `03-reason-draft-command-start.txt` / `04-reason-draft-command-tail.txt`: reason draft remains while paging with command/editor drawn. Escape returns to selected Deny without answering (`05-options-restored.txt`). Chat draft `D` persists. |
| Cwd-only refusal | At80x14, body18–18/18 is only `Cwd:` while Allow is selected/drawn; Enter sends no response (`06`/`07` captures). |
| Tiny refusal | At12x10 no command body is drawn; Enter sends no response (`08`/`09` captures). Draft remains in the retained observations. |
| Restore and decide | At80x40 command-bearing first page returns with draft intact (`10`). Original offered Deny/reject_once is visibly selected (`11`), then one manual Enter delivers exactly `{"outcome":"selected","optionId":"reject"}` on request4. No rejection draft is submitted. |
| Clean bounded stop | `12-rejected-clean-stop.txt`, `result.json`, and agent/client wire: exactly one permission response, owned prompt end_turn, Cyril exit0. No tool completion is replayed after rejection; no command, filesystem/terminal callback, account/model call or durable grant occurs. |

The exact invocation is retained as `actual-delivery-owned/invocation.txt`. It runs only the captured-frame RejectOnce instrument; never execute the native archive's historical launch script or captured command. This is actual Cyril conversion/render/key/wire behavior decoded through a PTY, **not** another real-KAS run, native Unicode glyph/reflow proof, or execution/output-alias qualification. It complements the original real-KAS premise rather than replacing or relabeling it. The original first attempt is preserved; there was no checker repair or blind retry and no24-case EJ rerun.

## Portable evidence and exact publication selection

- Archive: [cyril-xavt-delivery-nonsecret.zip](cyril-xavt-delivery-nonsecret.zip), **1,845,417 bytes**, SHA-256 `3886ee35c06274160ae6b736671ecfa3e90467e034140564e42eaf7233d34c48`.
- [evidence-index.json](evidence-index.json) gives every selected payload's exact size/hash, the archive hash, internal manifest hash, source provenance and exclusions. The archive has38 entries, including its internal `payload-manifest.json`.
- The original21-path publication roster is retained as historical provenance in the current portable bundle. [publication-files.json](publication-files.json) now enumerates the current held-index delta; its self-hash remains external. It must not be read as the old eleven-X-feature-file manifest.

Selection was explicit: the allowed original native zip/manifest/receipt/wire and separately labeled completed supplement, the established EJ harness, new owned controlled-peer outputs, Main-authorized named nonsecret Cargo outputs454/542/575/688/860, canonical source review and source-identity records. Main explicitly clarified that these named Cargo outputs are permitted portable logs; the toolspill exclusion concerns credential-bearing/private-journal or unknown output. No recursive `/tmp/evidence` scan or private-journal inspection was used. **Excluded:** kas-private.jsonl, all private HOME/KIRO_HOME and Cyril logs, credential stores, compiled binaries, credential-bearing or unknown toolspills, unrelated source/fixtures, tracker data and output-alias repair. Evidence hashes identify bytes; they do not substitute for behavioral receipts or imply arbitrary secret detection.

Portable records are independently readable without the original temporary directories. Retained harnesses/invocation preserve their original absolute prerequisites; no compiled binary or real credential is bundled. No production/test/fixture repair, repository check, commit, push or tracker write was performed by the finalization worker. The only runtime work was the explicitly authorized bounded actual-delivery owned-peer proof.

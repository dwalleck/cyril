# cyril-xavt delivery verification

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
- [publication-files.json](publication-files.json) enumerates the exact issue-owned publication paths and hashes, distinguishing the eleven frozen source/test/fixture files from README and workflow/evidence. Its own path is explicit; its self-hash is intentionally external to avoid recursive hashing. Main receives a complete external manifest including that file's hash.

Selection was explicit: the allowed original native zip/manifest/receipt/wire and separately labeled completed supplement, the established EJ harness, new owned controlled-peer outputs, Main-authorized named nonsecret Cargo outputs454/542/575/688/860, canonical source review and source-identity records. Main explicitly clarified that these named Cargo outputs are permitted portable logs; the toolspill exclusion concerns credential-bearing/private-journal or unknown output. No recursive `/tmp/evidence` scan or private-journal inspection was used. **Excluded:** kas-private.jsonl, all private HOME/KIRO_HOME and Cyril logs, credential stores, compiled binaries, credential-bearing or unknown toolspills, unrelated source/fixtures, tracker data and output-alias repair. Evidence hashes identify bytes; they do not substitute for behavioral receipts or imply arbitrary secret detection.

Portable records are independently readable without the original temporary directories. Retained harnesses/invocation preserve their original absolute prerequisites; no compiled binary or real credential is bundled. No production/test/fixture repair, repository check, commit, push or tracker write was performed by the finalization worker. The only runtime work was the explicitly authorized bounded actual-delivery owned-peer proof.

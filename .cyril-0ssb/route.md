# cyril-0ssb — KIRO_DATA_DIR bundle discovery

Date: 2026-10-19. Actual publication worktree: `/home/dwalleck/repos/cyril-wt-fix-cyril-0ssb`, stacked on published d7ve commit `8146f7a53cd85088917bf64e05ed665f8183ecd3` / PR185. Separate 0ssb commit/PR; parent owns publication and tracker closure.

## Routing evidence

| Test | Verdict and evidence |
| --- | --- |
| T1 — empirical premise | No remaining premise. Initially unresolved override trim/leaf semantics are now answered by direct static inspection of current 2.32.0 Linux resolver VA `0xa1f4f20` and independent macOS 2.29.1 branch: test trimmed emptiness, return the original nonblank value, append no leaf. evidence.md owns the binary provenance, asserted literals and dataflow. Safe current CLI observation independently confirms default credential-store location. |
| T2 — structural module shape | No. `protocol/kas/discovery.rs` already owns default platform roots, extraction selection, and default credential-store path. The repair separates their existing private wrappers without changing a crate/public interface or responsibility owner. Preserve all callers of default_store_path. |
| T3 — production-scale risk | No. A bounded startup environment/path lookup; no additional extraction, scan, process, or concurrency mechanism. |
| T4 — explicit behavior | Yes. Full Rivets record requires a nonempty-after-trim `KIRO_DATA_DIR` to redirect bundle discovery to `<original override>/kas`; trimming tests emptiness only and a nonblank value is preserved as-is, as current binary evidence requires. Empty/whitespace/unset preserve platform defaults; credential storage ignores the variable on every platform. Existing XDG, Apple Library/Application Support and Windows LocalAppData/known-folder rules remain unchanged; explicit server/node overrides retain precedence. No real user-state mutation. |

## Route and artifact ownership

**Local**, corrected after authoritative current binary evidence resolved the exact external choice. This route owns implementation completion; evidence.md retains the factual investigation. No design/plan stage or structural approval is required: all changes stay inside existing private discovery ownership and existing interfaces are unchanged. A separate spec is unnecessary because the issue is explicit.

Implement one private lazy asset-root selector and use it only for bundle discovery; retain the existing platform default resolver for default_store_path. Parent serializes verification, reviews and publication. This worker runs no build/test/lint/formatter or changed-source consumer smoke, commit or push. The only executable research observations are those explicitly authorized and recorded in evidence.md. No feature budget/hard limit supplied.

## Completion obligations

- Pure fixtures: set/as-is (including padded and relative), unset, empty and Unicode whitespace values; override is the root rather than an appended `kiro-cli`/`Kiro-Cli` leaf; native platform defaults preserved.
- Store and bundle roots deliberately differ under an override. Isolated subprocess fixture drives production discovery and default_store_path with competing roots and receipt proving child assertions ran; no ambient KIRO_DATA_DIR leakage into default-root tests.
- Parent runs actual-defect RED/GREEN, focused discovery tests, required Rust gates and current-source consumer smoke.
- Parent performs current real CLI environment smoke in owned temporary HOME/data/runtime directories (no real credentials or real Kiro changes), confirms current override semantics and credential-store separation, then updates existing README documentation.
- Independent code review and this issue's own PR, distinct from profile-root validation.

Terminal criterion: stated behavior, caller migration, current upstream semantics and smoke, checks/docs, independent review and dedicated PR all complete with no blocking defect or missing required evidence. Current state: exact stacked-delivery implementation, fresh Linux workspace qualification and five-case actual-binary smoke, parent-prepared README and final independent review complete. Dedicated 0ssb commit/PR/publication remain Main-owned. Verification.md and portable receipts identify exercised versus retained evidence; no native Apple/Windows or authenticated upstream extraction acceptance is claimed.

## Delivered edit slice / parent handoff

`protocol/kas/discovery.rs` now selects a nonblank original KIRO_DATA_DIR only for `resolve_kas_command`; the default platform resolver and `default_store_path` remain independent. Default lookup is lazy under a valid override. Pure fixtures cover original padded/relative/Unicode paths and empty/unset/Unicode-whitespace/default behavior across all platform helper outputs. The environment-wrapper fixture now runs seven separate owned trees (unset, empty, whitespace, absolute, relative, padded, period-suffixed), reaches the actual discovery/login gate and asserts the unchanged credential-store default plus an executed-child receipt. Other private-child discovery fixtures explicitly remove KIRO_DATA_DIR to avoid ambient-state leakage.

No build/test/lint/formatter or smoke was executed by this implementation/publication-artifact worker. VerifyRootSensitivity freshly qualified this exact worktree with workspace tests (2826 runtime plus 2 compile-fail doctests passed; 13 existing ignored), workspace all-target clippy, default binary build, format check and all five original actual-binary smoke arms. Original source-bound full Linux matrix and 38-test focused discovery GREEN are retained rather than relabeled as fresh commands here. The exact workspace run confirms affected consumer fences executed; see verification.md and receipts/index.json.

README is already prepared by the parent with only 0ssb bundle-root paragraphs changed over PR185; profile documentation is inherited. It documents as-is nonblank KIRO_DATA_DIR, no appended platform leaf, stable relative discovery across --cwd, and unchanged native-default data.sqlite3. Existing real upstream CLI observation confirms store separation but logged-out startup did not reach extraction; evidence.md supplies exact upstream override semantics. Fresh actual-Cyril owned-peer startup passed on Linux, not real Node/KAS/backend auth or native Apple/Windows. Main reports final ReviewKiroData verdict correct (0.98) for this exact stacked delivery. No source, README, architecture, check, commit, tracker or primary/combined-worktree change was made during publication-artifact preparation.

## Accepted review repair checkpoint

ReviewKiroData identified F1: a relative bundle found under launch cwd was passed unchanged to the workspace child. Parent observed consumer RED artifact182: discovery/auth succeeded and actual child exited 41. Production now anchors relative kas_root with std::path::absolute before scan/selection, then pops only the known kas suffix for the unchanged resolver. Appending kas first avoids premature Windows final-component stripping (F3). Explicit server/node overrides remain verbatim; no canonicalization, extra platform leaf, credential relocation or new interface. These unchanged repairs passed fresh exact-delivery consumer execution and five-case binary smoke; review-decisions.md records F1–F4 and final review.

The cross-cwd regression uses actual AgentProcess spawn, selected-file loading, physical cwd and a completion receipt under private HOME/PATH/fake auth. The genuine seven-arm environment matrix covers padded/triple-period roots; native separator-terminated root precreation avoids Windows ancestor-normalization fixture failure (F4). Fresh workspace receipts confirm both fences and the as-is/lazy-default test executed. Fresh binary smoke confirms padded selection, different launch/agent cwd and native-store separation. This is actual Linux filesystem/process behavior, not a Windows simulation; native Windows execution is not claimed.

Original ignoring-KIRO_DATA_DIR RED/restored-GREEN remains with `local://root-sensitivity.md` and its identified original proof snapshot; parent-observed F1 RED remains artifact182. They are not relabeled as exact-delivery runs. The final production/fixture/Cargo bytes match the retained full matrix, and fresh exact-worktree checks below supply final assembled-delivery GREEN.

## Exact stacked-delivery verification and portable receipts

The canonical current receipt is [verification.md](verification.md); [receipts/index.json](receipts/index.json) binds base, scope, commands, counts, source/binary identity, final review and limits. Owning archives are [receipts/0ssb-qualification-evidence.zip](receipts/0ssb-qualification-evidence.zip) and [receipts/0ssb-smoke-evidence.zip](receipts/0ssb-smoke-evidence.zip). The qualification archive includes fresh exact-branch logs, checked source/README, affected-fence execution and source-identical retained full-matrix receipts with their original cwd/revision.

All fresh commands and original five actual-Cyril PTY smoke arms passed with exit 0. The immutable exact-delivery executable is SHA-256 `b569d68a7c059e64bfd6044a7fbdcec23be6cbbcb02344e44e6bc6e4e7b77ad7`, not the earlier combined-tree executable. Synthetic credentials/owned peers only; no real account state or unclaimed native/backend acceptance. Main owns dedicated stacked PR publication; no assigned local check/smoke failure remains.


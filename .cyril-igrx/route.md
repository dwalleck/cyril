# cyril-igrx — native KAS data directory

## Request and source state

Requester: “claim and implement cyril-igrx”. Issue initially claimed for `omp`. Worktree `fix/cyril-igrx-data-dir` starts at `344c8c71` (claim cherry-picked onto fetched upstream). Primary checkout's unrelated local commits and untracked files remain untouched. The tracker cherry-pick retained upstream's adjacent cyril-hzkb record and changed only cyril-igrx.

## Routing evidence

| Test | Verdict and evidence |
| --- | --- |
| T1: unresolved empirical premise | No. The issue identifies `dirs::data_local_dir`; locally installed dirs 6.0.0 `src/mac.rs:7,13` resolves home + `Library/Application Support`. The lowercase `kiro-cli` leaf is corroborated by the existing Python reference at `experiments/code-review-workflow/review_policy.py:91-97`, not a live macOS observation. Baseline discovery source used the Unix `.local/share` fallback on Apple. No external runtime choice remains unresolved. |
| T2: structural module shape | No. `protocol/kas/discovery.rs` already owns per-platform data-root resolution and both consumers. Windows already had a pure helper, known-folder fallback, and fixtures from `73abf91c` (PR #164). The repair adds the missing private Apple helper using that pattern; no public interface, schema, dependency, or ownership change. |
| T3: production-scale risk | No. One startup path join replaces another; no new I/O, concurrency, or hot-path work. |
| T4: explicit behavior | Yes. cyril-igrx requires platform-correct data roots shared by KAS extraction and `data.sqlite3`. Apple: `<home>/Library/Application Support/kiro-cli`, ignoring XDG/LOCALAPPDATA. Preserve Unix absolute-XDG precedence, invalid-XDG fallback, missing-home errors, overrides, extraction-version selection, and Windows local-AppData/known-folder behavior. WSL-located stores remain separate cyril-lwpm scope. |

## Route and terminal criterion

**Local.** `route.md` is the only workflow artifact. No specification, design, empirical probe, or implementation-plan stage is needed for this explicit private path-resolution repair. No supplied feature budget or relevant hard size limit.

Terminal criterion: deliver remaining Apple behavior, preserve and exercise existing Linux/Windows fixtures, perform focused regression and consumer smoke plus applicable local repository checks, update existing behavior documentation, and obtain independent review without unresolved blocking findings. Native macOS/Windows execution is provided by the repository CI matrix and is distinguished from cross-host fixtures; no native run will be claimed from Linux.

## Verification obligations

- Cross-host Apple fixture reaches real extraction selection and verifies credential-store location, including spaces/Unicode and missing home.
- Red-capable regression: restore the prior Apple `.local/share` suffix in production helper, observe exact directory-discovery failure, then restore and pass.
- Existing isolated-child environment fixture uses competing HOME/XDG/LOCALAPPDATA roots; Apple expectation corrected to the actual native layout. Existing Windows fixtures retained.
- Run current-source consumer smoke through the KAS startup discovery path in an isolated environment, without reading or modifying real credentials.
- Applicable Rust checks: formatting; workspace all-target/all-feature Clippy; workspace all-feature tests and doc tests; KAS-only build/lint/tests; default-feature/core-no-KAS checks.
- Independent review of production behavior, tests, and documentation.

## Delivered behavior and evidence

**PASS — Local terminal criterion.** Implementation, documentation, applicable local checks, and independent review are complete. No blocking finding or local-check failure remains. Branch closure and publication are authorized below.

Source under verification: `344c8c71` plus the discovery repair, SHA-256
`ad36deb8e26b05a3fa61871e869fa5a56e18b9f26e99762cf3ac4af6d0a4a35a`
for `crates/cyril-core/src/protocol/kas/discovery.rs`. Linux x86_64, pinned
Rust 1.94.0, existing machine-local sccache/mold configuration, shared
`CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`.

Apple discovery now uses `Library/Application Support/kiro-cli` for both
the extraction scan and credential store. The existing shared home lookup
and missing-home outcome are unchanged. Windows AppData/known-folder and
Linux XDG behavior are unchanged. The README documents all three native
roots. No public callers need migration; the existing auth callback and
startup gate continue to consume the shared resolver. Glossary: no new terms.

| Verification | Result and receipt |
| --- | --- |
| Focused discovery | PASS. `cargo test -p cyril-core --features kas --lib protocol::kas::discovery::tests`: 33 parent tests plus the isolated wrapper child test passed; artifact 132. Includes Apple filesystem discovery, exact-version selection, spaces/Unicode argument preservation, missing home, existing Windows fixtures and Linux XDG cases. |
| Defect sensitivity | PASS. Replacing only the Apple helper suffix with `.local/share/kiro-cli` made `apple_server_selection_and_store_share_native_root` fail with `Server(.../Profile ü With Spaces/.local/share/kiro-cli/kas/.../acp-server.js)`, exit 101; artifact 129. Restored source compared byte-identical to the pre-mutation backup, then the focused suite passed; artifact 132. |
| Consumer smoke | PASS. Built current-source `cargo build -p cyril --features kas --example test_bridge` (artifact 132), then ran it with `--agent-engine kas` under a temporary HOME, competing LOCALAPPDATA, spaced/Unicode absolute XDG root, an independently laid-out versioned server, and a fake node file. A throwaway Python subprocess driver required `BridgeDisconnected` with `KAS auth not servable from <xdg>/kiro-cli/data.sqlite3` and rejected `KAS bundle not found`. Observed exact matching store path and absent-store diagnostic. The harness's `No SessionCreated` is expected for this missing-credentials scenario, not a live-session pass. No real credentials, agent, or token refresh used. |
| Formatting | PASS. `cargo fmt --all -- --check`; artifact 138. |
| All-feature workspace | PASS. `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo nextest run --workspace --all-features`: 2,289 passed, 13 skipped; `cargo test --doc --workspace --all-features`: 2 passed; artifact 138. |
| KAS-only | PASS. `cargo build -p cyril -p cyril-core --features kas`; `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings`; `cargo nextest run -p cyril -p cyril-core --features kas`: 1,408 passed, 10 skipped; artifact 138. |
| Default features | PASS. `cargo clippy --workspace --all-targets -- -D warnings`; `cargo nextest run --workspace`: 2,287 passed, 13 skipped; artifact 138. |
| Core without KAS | PASS. `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings`; `cargo nextest run -p cyril-core --no-default-features`: 843 passed, 4 skipped; artifact 138. |
| Independent review | PASS. Read-only `ReviewDiscovery` reviewed production, fixtures, consumers, dirs source, and README against `344c8c71`; no findings. It confirmed platform cfg exclusivity, independent fixture paths, unchanged shared-home behavior, and shared startup/callback root. |

Platform limits: Apple policy and extraction discovery were exercised on Linux
through the production pure helper. The cfg-selected environment wrapper runs
on each host in the existing macOS/Windows CI matrix, but those native runs
were not executed in this session. No native-platform or live authenticated
KAS qualification is claimed. The unrelated WSL-hosted store request remains
in the verified open issue `cyril-lwpm`; no WSL behavior was changed.

## Publication authorization

Requester: “close the issue on the branch, push it, open a PR”.

Applies to `cyril-igrx` on `fix/cyril-igrx-data-dir`, targeting the repository's
default branch `main`. Close the tracker record on this branch, publish it,
and open the PR; no merge authorization. Production and tests remain at
`5020f5d4`, so the evidence above is retained unchanged for this tracker and
record-only publication update.

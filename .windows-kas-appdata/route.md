# Windows KAS AppData discovery — 2026-10-02

## Route: Local

- T1 — no unresolved layout premise. `experiments/code-review-workflow/review_policy.py:79-97` and its README identify the native Windows data root as `%LOCALAPPDATA%\Kiro-Cli`. Baseline `discovery.rs` sent Windows through the Unix HOME fallback. The workspace's dirs 6 Windows implementation exposes the local-app-data known folder through `data_local_dir()`.
- T2 — no public interface, schema, architectural dependency-direction, or responsibility change. Repair stays in private KAS discovery/auth; core reuses the existing workspace dirs dependency on Windows, and bundle and sqlite authentication keep one shared root.
- T3 — no material production-scale risk. Directory resolution remains bounded startup work; bundle scanning/version selection are unchanged.
- T4 — native Windows resolves its AppData Kiro-Cli directory for both the KAS entrypoint and credential store, ignores competing HOME/XDG values, preserves explicit server/node overrides and version selection, and reports missing local-app-data honestly. Linux XDG and macOS behavior remain unchanged. macOS and WSL-hosted credential discovery are outside this Windows-only request; the existing cyril-igrx issue includes the separate macOS gap.

## Verification obligations

Focused behavioral regression with an AppData path containing spaces; existing discovery/override/version regressions; real child-process environment wrapper coverage; live KAS free-path bridge smoke; cargo test and clippy after the atomic change; formatting check; independent review. Native Windows execution will be attempted and reported separately from cross-platform path tests.

## Evidence and state

Implementation and local verification are complete in the isolated `fix/windows-kas-appdata` worktree. Windows AppData discovery, known-folder fallback, shared auth root, platform-appropriate startup/callback diagnostics, and README setup instructions are implemented. Explicit server/node overrides and bundle-version selection remain unchanged. Publication authorized by the requester: “commit the changes, push, and open a PR”.

Checks:

- Linux discovery suite: 31 tests passed, including the nested real-environment wrapper and three new Windows-policy regressions; final portability-adjusted assertions rechecked (`cargo test -p cyril-core --features kas --lib protocol::kas::discovery::tests`, artifact 144).
- `cargo test --workspace --features kas`: passed after the final auth-message regression was added (artifact 162).
- `cargo clippy --workspace --features kas -- -D warnings`: passed on Linux after the final regression (artifact 162).
- `cargo fmt --all --check`: passed after the final regression (artifact 162).
- `cargo test -p cyril-core --features kas --test kas_freepath_smoke -- --ignored --nocapture`: actual KAS session and sentinel turn passed on Linux with Kiro 2.26.0 and Node 26.10.0 (artifact 138). Initial run reported expired credentials; after `kiro-cli whoami`, the smoke passed. No token values were exposed. Production behavior is unchanged by the later test-only addition, so this evidence is retained.
- `protocol::kas::callbacks::tests::auth_failure_hint_not_doubled` now routes the actual `KasMissing::NoHomeForStore.reason()` producer through notification composition. Restoring the old non-Windows recovery text made it fail with two login-command mentions (artifact 150); exact pre-mutation source was restored and the focused test passed, followed by the full workspace gate.
- `cargo xwin test -p cyril-core --features kas --lib --target x86_64-pc-windows-msvc protocol::kas::discovery::tests -- --nocapture`, with Wine as the runner: final 31 tests passed, including the Windows real-environment wrapper. Initial execution exposed only a test's incidental mixed-separator String comparison, corrected to Path comparison. The Windows build emitted 18 warnings in untouched host-I/O/shell/SDK test code; Windows Clippy was not claimed.
- Independent reviewers WindowsKasReview and PublicationFenceReview: no findings after the diagnostic, portable-path assertion, and real-producer regression repairs; see review-decisions.md.

Native Windows limitation: the VM is running, but WinRM rejected its configured credentials (HTTP 401), and the alternative helper's default credentials file does not exist. Wine is Windows-target execution, not a native Windows KAS end-to-end run. No native proof is claimed. The Windows and macOS combined tracker issue was not closed because this request repairs only Windows.

No unresolved implementation or required local-check failure remains. The temporary VM probe was removed.

Terminal criterion: requested repair and affected documentation delivered, behavioral and runtime checks recorded, independent review resolved, and any verification limitation stated without claiming native proof.

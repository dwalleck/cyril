# cyril-d7ve — validated Kiro profile root

Date: 2026-10-19. Worktree: `fix/cyril-d7ve-kiro-roots`, based on `8e5c325e`. Issue claimed by the parent; this work does not mutate the primary tracker.

## Routing evidence

| Test | Verdict and evidence |
| --- | --- |
| T1 — empirical premise | No. Complete extracted KAS 0.66.34 resolver `experiments/conductor-spike/audit-2.31.1/kas/bundle/hunks-f.txt:2157-2190` settles trimming, native absolute classification, nearest-existing-ancestor canonical validation and lexical accepted output. Current 2.32.0 TUI retains the policy; source/live evidence is linked in design.md. |
| T2 — structural module shape | Yes. `kiro_agent_config.rs` already owns shared home/profile resolution, used by usage sidecar; settings currently owns a duplicate selected-environment resolver and the host hooks registry bypasses both. Consolidation adds one crate-private explicit-input resolver seam in the existing owner, migrates settings, global agents and hooks, and retains the ambient wrapper for sidecar/global callers. No public external interface, wire/schema, dependency, or UI changes. |
| T3 — production-scale risk | No. Startup path resolution adds canonicalization bounded by path depth, with no directory scanning, subprocesses, concurrency, or hot-loop work. |
| T4 — explicit behavior | Yes. Full Rivets record and its 2026-10-19 PR #181 note require one Kiro-compatible validated root for selected-environment settings (including `chat.enableWorkflows`), usage sidecar, global agent trust configuration and KAS host hooks. Trim a supplied root; ignore relative, filesystem-root, HOME-equal/ancestor values with a warning and fall back to `<home>/.kiro`. Keep workspace agents/hooks precedence, platform path behavior, HOME then USERPROFILE fallback, absent-home outcome and existing launch-environment restrictions. Do not read or mutate real Kiro state in fixtures/smoke. |

## Route and artifact ownership

**Structural**, because settings' duplicate resolver is removed in favor of the existing profile owner's new explicit-input seam. Required artifacts: this route (change-workflow), design.md (falsifiable-design), plan.md (budgeted-plan). A separate specification is unnecessary: the issue's behavior and note are explicit. No feature budget or hard limit was supplied. Existing audit sources answer the external premise; no new empirical probe is presumed necessary for routing.

Design approval is recorded in design.md §Approval. The atomic caller migration and behavior-only regressions are delivered. Parent owns serialized verification, independent review and publication; no worker build/test/lint/format/changed-source smoke runs or commits/pushes.

## Completion obligations

- Explicit fake-home/environment fixtures for trimming, unset/empty, relative/root/home-containing rejection, valid descendants/siblings, no home, lexical and nearest-existing-ancestor/symlink path variants, and relevant native Windows paths; invalid input warns, valid/default input does not warn.
- Selected-environment settings fixture must carry `workflows` and `goal` from the validated fallback cli.json, with a competing invalid-root file proving the wrong path is not read.
- Isolated child-process consumer regression covering actual global trust persistence and mediator hook registration from the override/fallback, not only direct helper calls; sidecar keeps using the shared wrapper.
- Parent records pre-fix RED and post-fix GREEN for the actual defects, focused tests plus required Rust gates, and current-source consumer / real CLI environment smoke against owned temporary state. Native Apple/Windows claims distinguish CI execution from cross-host fixtures.
- Parent updates existing user behavior docs after smoke, obtains independent code review, and opens this issue's own PR. `cyril-0ssb` remains a distinct commit/PR with separate discovery/store behavior.

Terminal criterion: complete migration and requested behavior, applicable checks and real-consumer proof, docs, independent review and issue PR, without unresolved blocking defects or required evidence. Implementation, local qualification, consumer proof, docs and independent review are complete; publication is parent-owned.

## Delivered edit slice / parent checkpoint

- `crates/cyril-core/src/kiro_agent_config.rs`: shared explicit-input validated resolver and ambient wrapper; lexical resolution, nearest-existing-ancestor validation, native Windows rooted/case semantics, warning reasons; global agents now derives from it. Colocated default/trim/invalid/lexical/symlink/Windows boundary regressions.
- `crates/cyril-core/src/protocol/kas/settings.rs`: duplicate policy removed; selected environment feeds the shared resolver. Five new initialize-capabilities cases distinguish fallback/trimmed profile from opposing cli.json files, including USERPROFILE-only replacement.
- `crates/cyril-core/src/protocol/domain_mediator/mod.rs`: wiring-only shared-root migration.
- `crates/cyril-core/src/protocol/domain_mediator/tests/serial.rs`: `global_profile_drives_trust_and_inbound_hook_registry` reenters the test binary in two owned environments, exercises actual global trust writes and workspace precedence, proves inactive/invalid-root configs unchanged, checks actual mediator hook replies include the right global and workspace hooks, and writes a completion receipt. The ambient wrapper assertion is the same root captured by the intentionally unchanged sidecar call.

No worker verification was run. Suggested parent focused commands (each with `env -u CARGO_TARGET_DIR` or an explicit nonempty target directory):

```sh
cargo test -p cyril-core --lib kiro_agent_config::tests
cargo test -p cyril-core --features kas --lib protocol::kas::settings::tests
cargo test -p cyril-core --features kas --lib protocol::domain_mediator::tests::serial::global_profile_drives_trust_and_inbound_hook_registry -- --exact
cargo test -p cyril-core --features kas --lib protocol::kas::discovery::tests
```

For actual-defect RED/GREEN, parent can safely restore only the old global_agents_dir and mediator hook-root callsites (retaining helper/tests): the `override` child must fail. Separately restore the old selected-environment settings selection: the home-equal/ancestor/trimmed initialize cases must fail. Restore all source and require green. These are proposed discriminating checks, not performed evidence.

Current-source consumer smoke: the new isolated test drives real persistence and mediator construction without spawning a Kiro process or executing hooks. Required real CLI environment smoke remains parent-owned: launch the archived/current CLI with an environment whitelist and owned HOME/USERPROFILE/KIRO_HOME/XDG data/cache/config/runtime/cwd; seed opposing cli.json roots, observe actual settings/profile selection and keep auth/extraction limits explicit. The 0ssb evidence record already owns exact current asset semantics and a safe observed default-store path. No real credential or user state may be used to bypass login.

After smoke, update existing README Kiro-root behavior (profile validation and separate bundle/store roots), run remaining required Rust feature-matrix gates and substantive independent review, then commit/open one PR per issue. Source sets are disjoint: d7ve owns the four files above; 0ssb owns discovery.rs and its own artifacts. No tracker, lint, unrelated primary-worktree or external user-state changes were made.

## Final isolated delivery qualification

The delivery branch is `fix/cyril-d7ve`, based on `8e5c325e`, containing only the four profile files above and this issue's documentation/evidence. Bundle discovery remains byte-identical to the base; no 0ssb implementation or fixture is included.

- Original-defect sensitivity was executed in an isolated proof tree: restoring the old global/trust selection, the hook-only bypass, and selected-environment settings selection each produced its intended behavioral RED, followed by restored GREEN. The hook defect was checked independently, not hidden behind an earlier trust assertion.
- The exact delivery tree passed `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build -p cyril`, and the environment-unset `cargo fmt --all -- --check`. All affected profile/settings/consumer fences executed. Source byte identities allow reuse of the previously completed Linux feature matrix; the delivery checkout was freshly compiled before its own checks.
- Actual delivery-binary PTY smoke passed both trimmed-profile and rejected-ancestor cases. These seed only native-default bundles and leave `KIRO_DATA_DIR` unset. Assertions cover initialize/session settings, global plus workspace hooks, authentication callback, rendered marker, and graceful shutdown. Real external peers are owned fixtures; no live authentication or native Windows/macOS runtime claim.
- Main separately ran the archived real Kiro 2.32.0 CLI against owned state: padded profile selected `chat.enableWorkflows=true`; a HOME-equal override selected default `false`. No real credentials or account state were used.
- ReviewKiroHome independently found no actionable defects in the shared resolver and its callers. No production repair followed that review.
- README's **Kiro profile roots** section documents this issue only; the bundle override belongs to the following issue.

Exact commands, source hashes, scope and limits: [delivery verification](verification.md). Portable command logs, source identity, runnable smoke harness and terminal/wire receipts: `receipts/d7ve-qualification-evidence.zip`. No unresolved local defect or missing local check remains.

Platform CI is obtained on the PR. The requester explicitly accepted only the existing `cyril-u62f` Windows memory-runtime readiness failure as a documented baseline exception; new failures remain blocking. [Scope and cancelled-coverage evidence](https://github.com/dwalleck/cyril/pull/183#issuecomment-6284441811). This is not a claim that cancelled Windows tests passed.

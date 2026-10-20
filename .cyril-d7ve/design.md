# cyril-d7ve — one validated Kiro profile resolver

## Route and inputs

Structural route: route.md. Behavior is the complete issue plus its PR #181 note; no separate product decision remains. Authoritative implementation: `experiments/conductor-spike/audit-2.31.1/kas/bundle/hunks-f.txt:2157-2190` contains complete KAS 0.66.34 `P8o`/`O9u`/`A8o`; 2.32.0 TUI `ZGt`/`VGt`/`lAe` retains that policy (archive `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js`, provenance `experiments/conductor-spike/audit-2.32.0/tui-findings.md:1-5`). Existing live confirmation: `audit-2.31.1/live-findings.md:125-149`, absolute/relative/root/home arms. Hooks relocation there is static, not a claimed live pass.

## Behavior and preserved invariants

- Trim the supplied KIRO_HOME. Unset/empty/whitespace falls back silently to `<home>/.kiro`.
- Require native Node-equivalent absolute classification: on Unix a root slash; on Windows rooted paths (including root-relative slash/backslash) are eligible, drive-relative `C:foo` is not. Resolve lexical `.`/`..` and repeated separators without requiring the leaf to exist.
- For **validation only**, canonicalize candidate and home through their nearest existing ancestor, appending any nonexistent suffix. This is required by the extracted implementation: symlink aliases of HOME/ancestors/root must also be rejected. Do not weaken this to lexical starts-with.
- Reject filesystem root and a candidate containing/equal to home according to Node's relative-path check; warn with the invalid value and reason (`relative`, `root`, `contains-home`), then return the default. Windows path comparison follows native case-insensitive path identity, including drive/UNC roots. The relative-path predicate's `startsWith("..")` treatment is preserved rather than broadened to a text-prefix check.
- Return the **lexically resolved input spelling**, not the canonical validation spelling, for an accepted override. No directory creation, permission changes, or writes belong in resolution.
- Keep ambient HOME then USERPROFILE selection; settings passes its already selected launch environment and must never fall back to ambient values. A valid absolute override can resolve without a supplied home; absent/invalid override without home yields None. Existing Replacement restrictions for KAS Free/Host hooks remain unchanged.
- Workspace agents shadow globals and workspace hooks still merge with globals. Trust persistence continues refusing builtins, missing/corrupt files and invalid names; only the selected global root changes. Settings marshal and workflow-gate policy are unchanged.
- Sidecar still captures its root through the ambient wrapper; no new process or user-state behavior. Data/credential paths are cyril-0ssb, not profile state.

## Placement and module shape

| Owner | Interface / change | Forbidden |
| --- | --- | --- |
| `kiro_agent_config.rs` (retain/deepen) | Retain `kiro_home_dir() -> Option<PathBuf>` ambient wrapper; add crate-private environment-explicit `resolve_kiro_home(home: Option<&Path>, value: Option<&OsStr>) -> Option<PathBuf>`. Keep lexical/nearest-ancestor/ancestry implementation private. Global agents derives `kiro_home_dir()/agents`. | No generic path service, new module, environment mutation, config creation, or public external interface changes. |
| `protocol/kas/settings.rs` (delete duplicate policy) | Read HOME/USERPROFILE and KIRO_HOME via SpawnEnvironment::effective_var, pass them to shared resolver, then read cli.json. | No ambient environment fallback or duplicate validation. |
| `protocol/domain_mediator/mod.rs` (protected parent) | Wiring-only replacement of hardcoded HOME/.kiro with shared wrapper. | No resolver body or filesystem policy. |
| `usage/kiro_sidecar.rs` | Existing wrapper caller intentionally unchanged. | No duplicated path policy. |

The existing owner wins because home/profile policy already resides there and is non-UI core behavior. A new generic environment/path abstraction would spread one policy unnecessarily. No dependency is required: std native paths and filesystem canonicalization suffice. Compiler checks and independent review verify placement; no bespoke shape parser is warranted.

## Claims and falsification

| ID | Claim / cases | Verification / falsifier | Regression coverage | Status |
| --- | --- | --- | --- | --- |
| C1 | Shared resolver follows upstream trimming, default and invalid classes, lexical and nearest-existing-ancestor normalization; accepted symlink spelling retained. | Tempdir-based explicit-input tests, warning capture with invalid/valid controls; Unix symlinks to home/ancestor/root and nonexistent suffixes; native Windows drive/root-relative/UNC and case fixtures. Wrong root or wrong warning invalidates claim. | Colocated resolver tests using fake homes, no global env mutation. | PENDING — edits delivered; parent execution. |
| C2 | Selected environment's cli.json controls workflows/goal even when KIRO_HOME is rejected. | Plant opposing files under invalid candidate and fallback; initialize capabilities must reflect fallback, no parent environment access. Trimmed valid override selects override file. | Extend existing settings initialize consumer regression. | PENDING — edits delivered; parent execution. |
| C3 | Global trust and mediator host hooks use the same validated root; workspace precedence unchanged. | Isolated reentered test binary with owned HOME/USERPROFILE/KIRO_HOME, valid trimmed and invalid ancestor arms. Persist an actual grant to expected global config, verify inactive bytes unchanged, inspect registry reply from DomainMediator. Require child completion receipt. | Existing workspace precedence tests retained; new consumer regression in mediator tests. | PENDING — edits delivered; parent execution. |
| C4 | Actual consumers and upstream CLI agree without touching user state. | Parent current-source consumer smoke plus current real CLI env smoke in temporary HOME/data/runtime/cwd; use no real auth and no state-changing real-user commands. Record actual-defect RED/GREEN, required Rust gates and native-platform limits. | C1–C3 plus source-bound audit/live reference. | PENDING — parent verification and smoke. |
| C5 | Caller migration and ownership match above, independent review and own PR. | Parent independent code review after edits; all affected callers use one resolver, no duplicate default policy; README updated after smoke. | Compiler feature matrix and independent review. | PENDING — parent review/publication. |

Existing behavioral tests plus consumer execution are sufficient; no second resolver/oracle or speculative mutation machinery. For actual defect sensitivity parent can run new regressions against the base before applying production edits or temporarily restore the defective callsites under safe restoration.

## Non-goals

No credential relocation, agent launch-environment policy expansion, path translation redesign, config migration or directory creation, warning cache, global environment mutation, or new dependency. No tracker deferrals or acceptance reduction.

## Approval

2026-10-19 — explicit user approval relayed by Main: user selected **“Approve shared resolver”** in response to **“For Kiro roots, approve one shared core resolver reused by settings, global agents, hooks, and the sidecar? It will mirror Kiro’s validation, including rejecting symlink aliases of home/root, while keeping bundle and credential roots separate.”** This authorizes the shared-resolver placement and existing issue outcomes above; no risk or reduced acceptance is approved. Main also reports delegation of routine sign-offs within the existing thirteen-issue contracts, not risk acceptance or relaxed acceptance. Parent owns final verification, docs-after-smoke, independent review and dedicated PR.

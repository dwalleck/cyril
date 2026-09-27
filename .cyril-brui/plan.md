# Plan: cyril-brui

Design verified: `design.md` Falsification table complete, cheapest falsifier `PASS` (C2 baseline), no `FAIL`, every `PENDING` names "checkpointed-build, Slice 1 gate", Approval carries the requester's relayed verbatim words (2026-09-27), Module shape `N/A` per T2.

Module growth ledger: N/A — route/design record no module-shape change.

Partition arithmetic: Slice 1 diff estimate ≈ 60 production + 130 test lines = 190; churn margin 50 % (test-name and comment churn on the seven updated legacy fences) → ≈ 285 changed lines ≪ 4,000 → **one PR increment** (`fix/cyril-brui-kas-xdg-data-home` → `main`).

## Slice 1: honour `XDG_DATA_HOME` in the KAS data-dir resolution (root + store)

**Claim IDs:**             C1, C2, C3, C4, C5, C6
**Expected behavior:**     `route.md` T4 given/when/then: XDG absolute → `<xdg>/kiro-cli/{kas,data.sqlite3}`; unset → `<home>/.local/share/kiro-cli/…`; empty/relative → home fallback + exactly one WARN naming the value; root and store share the data dir on every branch.
**Oracle:**                `evidence.md` P1/P2 (strace of the real launcher) for C1/C3/C5; `dirs` source for C2; on-disk extraction layout for C4; production call-site grep for C6 — as recorded per row in `design.md`.
**Stress fixture:**        `invalid_xdg_data_home_falls_back_to_home_with_warning` runs three invalid values (`""`, `"   "`, `"relative/data"`) × two homes (`Some`, `None`) and asserts both the path and the WARN count/value per case; `kiro_data_dir_prefers_absolute_xdg_data_home` includes `/tmp/ü data` (Unicode + space) and `home=None`. Expected outcomes are written in the tests before implementation (red run recorded below).
**Regression fence:**      `crates/cyril-core/src/protocol/kas/discovery.rs` tests module — created in THIS slice: `kiro_data_dir_prefers_absolute_xdg_data_home`, `kiro_data_dir_defaults_to_home_local_share_without_xdg`, `invalid_xdg_data_home_falls_back_to_home_with_warning`, `kas_root_and_store_follow_xdg_data_home`, `kas_root_shares_kiro_data_dir` (rewritten). Run: `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas discovery`.
**Named mutation:**        from `design.md`: C1 delete the XDG branch; C3 (a) delete the `warn!`, (b) `is_absolute()` → `true`; C5/C6 make `store_path_from` re-derive from home. Applied by the checkpoint to the new fences, red observed, restored, green observed.
**Complexity/production scale:** N/A — reason: no new loop; two env reads + two joins per KAS spawn / per `getAccessToken` callback.
**Wall budget/phase:**     N/A — reason: one-off phase per spawn / per auth callback; no wall budget.
**Module shape:**          N/A — route/design record no module-shape change.
**Files:**                 `crates/cyril-core/src/protocol/kas/discovery.rs` (only production file); `.cyril-brui/*` artifacts.
**Estimate:**              1–2 h including gates on a shared machine.
**Diff estimate:**         ≈ 190 changed lines (see arithmetic above).
**PR increment:**          increment 1 of 1.
**Commands and expected results:**
- RED: `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas discovery` with the new fences and unchanged production → fails to build: the fences name `kiro_data_dir`, `kas_root_from`, `store_path_from`, `XDG_DATA_HOME_DEFAULT_REL`, `KIRO_DATA_DIR_NAME`, `STORE_FILE_NAME`, none of which exist (the product has no XDG-aware resolution to call).
- GREEN: same command after the production change → every discovery fence passes, including the seven legacy fences re-pointed at the data-dir parameter, with the C1–C6 expected paths and WARN counts exactly as asserted.
- MUTATION: each named mutation applied in turn → the named fence fails with the design row's expected red output; restored → green.
- Oracle comparison: the C1 fence's expected `/xdg/kiro-cli` and the C3 fence's expected `/home/u/.local/share/kiro-cli` are, item by item, the directory the strace probe showed the launcher creating under the same env shapes (`evidence.md` Comparisons).
- Gates (each `> /tmp/<name>.log 2>&1 && echo GATE-OK-<name>`): `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo nextest run --workspace --all-features`; `cargo test --doc --workspace --all-features`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo nextest run --workspace`; `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings`; `cargo nextest run -p cyril -p cyril-core --features kas`; `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings`; `cargo nextest run -p cyril-core --no-default-features` — all with `env -u CARGO_TARGET_DIR`.

## Self-review

1. Every design row (C1–C6) is assigned to Slice 1 exactly once; all `PENDING` falsifiers are discharged there. ✔
2. All fourteen fields present; conditional ones carry `N/A — reason`. ✔
3. Every fence is created in this slice; every fence carries its named mutation. ✔
4. No new loop; no always-on phase. ✔
5. Module shape N/A per route/design; single file touched. ✔
6. Partition rule applied (285 ≪ 4,000); one increment with a mergeable definition (the branch alone merges to `main`; verified by the ten gates plus CI's Windows/macOS legs). ✔
7. Tracker taxonomy: `cyril-lwpm`, `cyril-5db7`, `cyril-tpwn` verified via `rivets list`/`rivets show`; no new deferrals. ✔
8. Fence assertions assert the observable each mutation changes (path value, WARN count and `value` field, error variant payload). ✔
9. No slice declared complete here — the checkpoint record below is checkpointed-build's. ✔

## Slice 1 checkpoint (checkpointed-build)

Recorded after the build; see the "Checkpoint record" section appended below.

## Checkpoint record — Slice 1 (checkpointed-build)

Checked source: branch `fix/cyril-brui-kas-xdg-data-home` at base `f9bc81d8` + the working tree that becomes this slice's commit (one production file, `crates/cyril-core/src/protocol/kas/discovery.rs`; `git diff --stat`: 285 insertions, 54 deletions — within the plan's 285-line projection). Environment: Linux x86_64 (CachyOS 7.2.3), Rust 1.94.0 (`rust-toolchain.toml`), `cargo nextest`, worktree-local `target/` (`env -u CARGO_TARGET_DIR` per the coordinator's environment fix), machine shared with eleven sibling builds. Date: 2026-09-27.

Pre-slice plan critique: no loop budget, no timing claim, oracle for C1/C3/C5 is the external strace evidence (not the implementation); precondition "XDG must be absolute" is enforced at runtime by `Path::is_absolute` (load-bearing, not a `debug_assert!`). No concern raised.

Step 1 — impact analysis (signature/semantics changes): `resolve()` first parameter `home` → `data_dir` (private; callers: `resolve_kas_command` + 10 test call sites, all in `discovery.rs`, all updated). `default_store_path()` semantics now XDG-aware, signature unchanged; callers `crates/cyril-core/src/protocol/kas/auth.rs:306 respond_get_access_token` and `discovery.rs resolve_kas_command` (grep; `tethys callers default_store_path --lsp` indexed result agrees: 2 direct callers across 2 files). `KIRO_DATA_DIR_REL` removed; readers were `default_store_path` and the F19b pin only (grep). `KAS_ROOT_REL` value changed to data-dir-relative `"kas"`; readers all in `discovery.rs` (grep across `crates/`: no external reader).

Step 2 — helper search: in-source `XDG_DATA_HOME` resolvers: `crates/cyril-memory/src/paths.rs resolve_default_from` (different crate — cyril-core must stay persistence-free and cannot depend on cyril-memory; it also returns `Err` on relative HOME, a contract discovery keeps as `Option`); `kiro_agent_config::home_dir()` reused as the home source. Manifest search: `dirs`/`directories` are NOT workspace dependencies (only present in the registry via other tooling) — adding one is a net-new dep decision outside this fix; the two-branch rule is four lines. Decision: pure in-module resolver mirroring the existing `resolve_in_dirs` injectable-inputs convention.

Step 3 — TDD: RED `red-run.log` (exit 101, `E0425` for `kiro_data_dir`, `kas_root`, `store_path`, `KIRO_DATA_DIR_NAME`, `XDG_DATA_HOME_DEFAULT_REL`: the product had no XDG-aware resolution to call); GREEN `green-run.log` (28 passed: 24 retained + 4 new + the rewritten pin).

Step 4 — symmetry audit (new XDG branch beside the existing home branch): error handling — both return `Option`, `None` only when nothing resolves (unchanged `NoHome`/`NoHomeForStore` downstream); logging — the invalid-XDG WARN is the only new event, emitted once per resolution (spawn resolves once; `getAccessToken` resolves per callback — a misconfiguration is worth one line per token request); fallback — identical to the pre-change path; caller observability — callers see only the resolved path, as before (the WARN is the operator's signal); resource discipline — no I/O added; guard parity — the absolute-path guard is the only new guard and mirrors `dirs-sys`; acceptance symmetry — inputs newly accepted: absolute XDG (previously ignored); newly refused: none (relative/empty XDG were previously ignored too, now ignored-with-warning).

Gate (eleven items):

1. Affected unit tests — **PASS**: `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas discovery` → 28 passed (`green-run.log`).
2. Falsifiers C1–C6 — **PASS**: discharged by the same run (C1 `kiro_data_dir_prefers_absolute_xdg_data_home`, C2 `kiro_data_dir_defaults_to_home_local_share_without_xdg`, C3 `invalid_xdg_data_home_falls_back_to_home_with_warning`, C4/C5 `kas_root_and_store_follow_xdg_data_home`, C6 `kas_root_shares_kiro_data_dir`).
3. Stress fixture — **PASS**: the C3 fence's 3 invalid values × 2 homes (path + exactly-one-WARN + value + variable name per case, positive control on the valid value) and the C1 Unicode/space + no-home legs all hold in the green run.
4. Changed implementation vs independent oracle — **PASS**: on the env shapes the probe exercised, the implementation's outputs are item-by-item the directories the launcher created: absolute → `<xdg>/kiro-cli` (evidence P1: `<S>/xdg/kiro-cli/data.sqlite3` created, 0 `$HOME/.local/share` syscalls); relative and empty → `<home>/.local/share/kiro-cli` (evidence P2a/P2b: `mkdir("<S>/home/.local/share/kiro-cli")`, store created there). The static oracle (`dirs-6.0.0/src/lin.rs:11`, `dirs-sys::is_absolute_path`) predicts the same three outcomes.
5. Approved module shape — N/A — route and design record no module-shape change (single file, same owner; one env reader for `XDG_DATA_HOME` in cyril-core, verified by grep).
6. Production-scale budget — N/A — plan records no loop or wall budget (two env reads + joins per spawn / callback).
7. Regression fence green — **PASS**: the five fences above (`green-run.log`; re-confirmed after mutation restore, `mutation-runs.log` tail).
8. Named mutation red — **PASS** (`mutation-runs.log`, script `mutations.sh`): M1 delete-XDG-branch → red C1 + C3 + C4/C5 + C6 (4 failed); M2 delete `warn!` → red C3 only (1 failed: WARN count 0); M3 `is_absolute()`→`true` → red C3 only (path under the relative value); M4 stray segment in `store_path` → red C4/C5 + C6 (2 failed); M5 `.local/share`→`.local/state` → red C2 + C4/C5 default leg (2 failed); M6 root re-derived from the home-relative default → red C4/C5 + 8 retained root-consuming fences (9 failed). Every mutation applied (proof-line grep) and hit exactly the fences design.md assigns.
9. Fence restored green — **PASS**: `cmp` byte-exact restore, then 28 passed (`mutation-runs.log`, "green after restore").
10. Parity and reuse — receipt: new symbols `kiro_data_dir`, `kiro_data_dir_from_env`, `kas_root`, `store_path`, `XDG_DATA_HOME_DEFAULT_REL`, `KIRO_DATA_DIR_NAME`, `STORE_FILE_NAME`; sibling search (grep `XDG_DATA_HOME`, `data_local_dir`, `.local/share` across `crates/`; manifest grep for `dirs`/`directories`): the only sibling is `cyril-memory::paths::resolve_default_from`, not reusable across the crate boundary (step 2); `home_dir()` reused; the "data.sqlite3" literal now has one spelling (`STORE_FILE_NAME`) — previously it was typed twice (`default_store_path` doc + body). Test helpers: `abs_fixture` is new (no sibling: existing fixtures never needed an absolute path), `exists_set`/`node` reused; the tracing capture reuses `crate::test_support::capture_json_subscriber` (sibling of `domain_mediator/tests/serial.rs:22`). No construction written twice. Symmetry answers: step 4 above.
11. Preserved enforcement — the F19b constant-level pin (`KAS_ROOT_REL.strip_suffix("/kas") == KIRO_DATA_DIR_REL`) is **replaced, not dropped**: its detection set (root and store share the data-dir prefix) is reproduced by the rewritten `kas_root_shares_kiro_data_dir` on three env branches, and M4 proves the new fence detects a re-homed store — the predecessor's only recorded case. Authorized by the requester's decision "update the test to the new resolution rather than breaking the pin". No other gate, validator, or policy file touched.

Step 7 — stale-reference sweep: module doc, `KasMissing::{NoHome, NoHomeForStore}` docs, the constants block, `resolve`'s step-1 comment, `resolve_kas_command`'s F14 comment, `default_store_path`'s doc, and the `resolve_no_home_no_override_errors` comment all rewritten to the data-dir vocabulary; `KasMissing` reason strings unchanged (still literally true — design non-goal). No TODOs, no anonymous deferrals; tracker references `cyril-tpwn`, `cyril-lwpm`, `cyril-5db7` verified with `rivets show`/`rivets list`. Discipline grep over added lines: no `unwrap()`, `let _ =`, `#[allow`, `unwrap_or_default` in production (the single grep hit is the retained F14 comment text "no dead unwrap_or_default").

Repository gates (mirror of `.github/workflows/ci.yml`, run by `gate.sh` with real exit codes): see `gate-summary.txt` — recorded below once complete.

Repository gates — all `PASS` (`gate-summary.txt`, run 2026-09-27T05:41–05:44Z on base `f9bc81d8` + this working tree, i.e. the exact content of this slice's commit; each command `> /tmp/cyril-brui-<name>.log 2>&1 && echo GATE-OK-<name>`, 10/10 GATE-OK):

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo nextest run --workspace --all-features` | PASS — 2061 passed, 13 skipped |
| `cargo test --doc --workspace --all-features` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo nextest run --workspace` | PASS — 2059 passed, 13 skipped |
| `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | PASS |
| `cargo nextest run -p cyril -p cyril-core --features kas` | PASS — 1254 passed, 10 skipped |
| `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | PASS |
| `cargo nextest run -p cyril-core --no-default-features` | PASS — 782 passed, 4 skipped |

Windows/macOS legs: owned by the PR's CI jobs, not claimed from these Linux runs.

Size tripwire: 285 changed production-file lines + artifacts ≪ 4,000; single increment as planned. Drift check: upstream `main` still at `f9bc81d8` (this worktree's base) at commit time; no upstream movement to merge.

Slice 1 verdict: every applicable gate item `PASS` or contract-backed `N/A`; no `FAIL`. Final integration check = this single slice's assembled state (one file, one commit); no cross-slice interaction exists.

## Bounded repair 1 — Windows path-rendering assertion (final integration qualification failure)

- **Failure:** PR #140 CI `Test (windows-latest)` run 36298122772 / job 108560650096: `protocol::kas::discovery::tests::nothing_found_names_search_root` panicked at `discovery.rs:771` — `reason must name the searched root: KAS bundle not found at /home/u/.local/share/kiro-cli\kas\node_modules/@kiro/agent/dist/server/acp-server.js …`. Linux and macOS legs passed; 1240 Windows tests were cancelled unrun (nextest fail-fast).
- **Root cause:** test-only. The slice strengthened the retained fence's second assertion to `err.reason().contains("<DATA_DIR>/<KAS_ROOT_REL>")`, a `/`-joined literal; on Windows `Path::join` renders the new `kas` segment with `\` (`kiro-cli\kas`) while the `/`s come from the POSIX-style `DATA_DIR` fixture literal. The component-wise `Path::starts_with` on the preceding line passed. The production rendering (`KasMissing::Server(p).reason()` → `p.display()`) is the host's native rendering and is not wrong on Windows.
- **Governing obligation:** CLAUDE.md "CI failure triage" — cross-platform fixture idiom: compare component-wise or build the expected string from the same `Path` the code renders; `cfg(windows)` is CI-verified only. Approved behavior, oracle meaning, ownership and risk unchanged (technical correction under the contract's Approval semantics).
- **Correction:** `nothing_found_names_search_root` now computes `kas_root(Path::new(DATA_DIR)).display().to_string()` and asserts the reason contains that — the same join the code performs, so it renders `…/kiro-cli/kas` on unix and `…/kiro-cli\kas` on Windows. One test, six lines; no production change.
- **Next-wave static audit (nextest cancelled 1240 tests):** every other new assertion in the discovery tests compares `PathBuf`/`Option<PathBuf>`/`KasMissing` (component-wise `Path` equality) or `.parent()`; the WARN `value` check is a Debug string of a relative literal; the tests alphabetically after `nothing_found…` (`override_beats_versioned` … `versioned_beats_legacy`) are the pre-existing Path/override comparisons unchanged in mechanism (they passed on Windows before the slice, and `resolve_missing_server_reports_server_not_node` compares `KasMissing::Server(PathBuf)` component-wise). The 711 Windows tests that did run include all discovery tests alphabetically before the failure (`invalid_xdg…`, `kas_root_and_store…`, `kas_root_shares…`, `kiro_data_dir_*`), all PASS. No second wave expected.
- **Affected checks re-run locally on the corrected tree** (`repair1-gate.sh`, `repair1-gate-summary.txt`, 2026-09-27T06:01Z, 5/5 GATE-OK): `cargo fmt --all -- --check` PASS; `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` PASS; `cargo nextest run -p cyril -p cyril-core --features kas` PASS (1254 passed, 10 skipped); `cargo clippy --workspace --all-targets -- -D warnings` PASS; `cargo nextest run --workspace` PASS (2059 passed, 13 skipped).
- **Evidence disposition:** the Slice 1 record above is retained — falsifiers C1–C6, the six mutations, the oracle comparison and the all-features/doctest/no-default legs are unaffected (the repair changes one assertion's expected-string construction in a fence whose mutation M6 already proved red on the *variant* check; M6's red output on this assertion is reproduced by construction since the expected string still derives from `kas_root`). Windows/macOS proof is owned by the re-run CI legs; recorded below when green.
- **CI re-run on `ad2415d2` (run 36298943662):** all 13 checks pass — `Test (windows-latest)` job 108562875151 **PASS** (the leg that failed on `fcbfb7f1`), `Test (ubuntu-latest)`, `Test (macos-latest)`, `KAS Feature`, `Default Features`, `Lint`, `Format`, `Build (ubuntu/windows)`, `Validate Commits`, `Worktree Tooling`, `CI Success`. Final integration proof for the assembled branch is therefore complete on all three platforms; the Slice 1 verdict stands with no `FAIL`.

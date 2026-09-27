# Route: cyril-brui

Change: KAS discovery (`crates/cyril-core/src/protocol/kas/discovery.rs`) resolves the kiro-cli data dir as `$XDG_DATA_HOME/kiro-cli` when `XDG_DATA_HOME` is set, non-empty and absolute, else `$HOME/.local/share/kiro-cli` — for BOTH the KAS extraction root and the credential store — mirroring kiro-cli. An empty or relative `XDG_DATA_HOME` falls back to `HOME` with a logged warning naming the value.
Date: 2026-09-27
Base: `f9bc81d8` (main) in linked worktree `.claude/worktrees/agent-a1375686b0e68df53`, branch `fix/cyril-brui-kas-xdg-data-home`.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The design premise is "kiro-cli resolves its data dir with `dirs::data_local_dir` semantics: `XDG_DATA_HOME` wins when absolute; empty/relative values are ignored and `$HOME/.local/share` is used". Repository evidence before this change covered only the absolute-XDG half indirectly (the committed `experiments/conductor-spike/probe-kas-*.py` scripts run `kiro-cli acp` with `HOME=<tmp> XDG_DATA_HOME=~/.local/share` and their sessions authenticate — e.g. `probe-kas-baseline-2.21.0.py:37`, `probe-kas-content-chunk-2.21.2.py:69`); nothing covered the empty/relative fallback that the approved behavior mirrors. Under the contract's Evidence validity rule the premise was not covered → probed here. Result: `evidence.md` P1–P2 `PASS` (no-auth strace probe `probe-xdg-strace.sh` vs. the `dirs`/`dirs-sys` source and the binary's string table). | **yes** |
| 2 | Structural module shape | All edits stay inside `protocol/kas/discovery.rs`, which already owns "where is the KAS bundle / where is the credential store" (the module doc + `KIRO_DATA_DIR_REL`/`KAS_ROOT_REL`). No public interface changes: `pub(crate) fn default_store_path() -> Option<PathBuf>` keeps its signature (callers: `protocol/kas/auth.rs:306 respond_get_access_token`, `discovery.rs resolve_kas_command` — grep + `tethys callers default_store_path`); `resolve_kas_command()` unchanged; the private `resolve()` first parameter changes meaning from `<home>` to `<kiro data dir>` (all callers in-file: `resolve_kas_command` + the test module). No new module, no dependency-direction change (still `kiro_agent_config::home_dir()` + `std::env`), no responsibility move. Length gate: no repository file-length policy exists (`scripts/`, `.github/workflows/ci.yml` grep for line-count gates: none); discovery.rs is 806 lines (≈345 production + tests) and grows by ≈+40 production / +80 test lines within the same responsibility → no Length-review trigger. | no |
| 3 | Production-scale risk | Two env reads and two path joins once per KAS spawn and once per `getAccessToken` callback; no loops, no I/O added. | no |
| 4 | Explicit behavior | Given `XDG_DATA_HOME=/abs` (absolute, non-empty) and any `HOME`, when KAS discovery resolves the extraction root, then it scans `/abs/kiro-cli/kas` (versioned dirs / legacy layout unchanged beneath it) and a missing bundle names `/abs/kiro-cli/kas/...` in `KasMissing::Server`. — Given the same env, when the credential store path is resolved (`default_store_path`, the spawn login gate, `getAccessToken`), then it is `/abs/kiro-cli/data.sqlite3`. — Given `XDG_DATA_HOME` unset and `HOME=/home/u`, then root = `/home/u/.local/share/kiro-cli/kas`, store = `/home/u/.local/share/kiro-cli/data.sqlite3` (today's behavior, preserved). — Given `XDG_DATA_HOME` set but empty or relative (e.g. `relative/data`) and `HOME=/home/u`, then both resolve as if unset AND one `WARN` event naming the offending value is emitted (never silent). — Given `XDG_DATA_HOME` valid and no `HOME`/`USERPROFILE`, then resolution succeeds from XDG alone. — Given neither a valid `XDG_DATA_HOME` nor a home, then `KasMissing::NoHome` (root) / `KasMissing::NoHomeForStore` (store) as today. — The KAS root and the store always share one parent (the kiro data dir) on every branch (the F19b drift pin, kept at the new resolution level). Requester decisions supplied by the orchestrator fix all of this; no unresolved decision. | yes |

Unknown tests: none.

## Selected route

Empirical — one external premise (kiro-cli's `XDG_DATA_HOME` semantics, including the empty/relative fallback) had no valid repository evidence; it is discharged in `evidence.md` before design. Precedence: T1 fires, so Empirical even though T2–T3 are `no`.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict; the orchestrator relayed the requester's approved behavior, recorded verbatim in T4 and in design.md's Approval) |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 verdict): `evidence.md`, `probe-xdg-strace.sh`, `probe-xdg-strace{,-relative,-empty}.out` |
| design.md | falsifiable-design | required — compact: claims C1–C6, falsifiers, mutations, fences, approval |
| plan.md | budgeted-plan | required — single slice, single PR increment |

Oracle checkpoint in `checkpointed-build`: required — Empirical route; recorded in `plan.md` (Slice 1 checkpoint record).

## Downstream sequence

prove-it-prototype (done: evidence.md) → falsifiable-design (design.md) → budgeted-plan (plan.md) → checkpointed-build (Slice 1 gate record in plan.md). `interrogated-spec` skipped: T4 = yes.

## Terminal criterion

Empirical — `evidence.md` records `PASS` for P1 and P2; `design.md` and `plan.md` satisfy their owning stage's completion criteria; `plan.md`'s Slice 1 checkpoint records no `FAIL`.

Status: see the "Terminal criterion result" section appended below once the checkpoint is recorded.

## Obligation not run — live acceptance check

The issue's acceptance criterion also names `cargo run --example test_bridge -- --agent-engine kas` under `HOME=<tmp> XDG_DATA_HOME=~/.local/share` with no symlink. **Not run.** Reason (hard constraint from the orchestrator): kiro's OIDC refresh token is single-use and concurrent renewals log the user out (memory: `feedback_kiro_token_renewal_single_flight`); eleven sibling agents may be running kiro-cli concurrently, so no auth-touching kiro-cli command (`whoami`, `login`, `acp`, `test_bridge` against a real engine) may be run from this session. This is recorded as an obligation not run — not `N/A`, not passed. What IS proven without it: the resolution rule (design.md C1–C6, pure fences on every host), the no-auth strace probe showing the same env makes kiro-cli itself open `$XDG_DATA_HOME/kiro-cli/data.sqlite3`, and — since review round 1 (`review-decisions.md` F1/F2, claim C7) — the real env wrappers: `env_wrappers_resolve_xdg_data_home_like_kiro_cli` re-enters the test binary with a private `HOME`/`XDG_DATA_HOME`, a fake node and a fake versioned extraction, and asserts `default_store_path()` and `resolve_kas_command()`'s login gate both land on `<xdg>/kiro-cli/data.sqlite3` (HOME default on macOS/Windows, where kiro-cli ignores the variable). What the live run still uniquely proves: acceptance of a *real* kiro-cli extraction and a *real* login store under the XDG dir end to end — node spawned on the resolved bundle and `_kiro/auth/getAccessToken` answered from that store. It remains for the reviewer/orchestrator to run when no other kiro-cli session is live.

## Terminal criterion result

Empirical route — **holds** (2026-09-27):

- `evidence.md`: P1 `PASS`, P2 `PASS` (P3/P4 `N/A` as non-premises).
- `design.md`: Falsification table complete, cheapest falsifier `PASS`, Approval carries the requester's relayed verbatim decisions.
- `plan.md`: Slice 1 checkpoint record — eleven gate items `PASS` / contract-backed `N/A`, no `FAIL`; six named mutations red then restored green (`mutation-runs.log`); ten repository gates `PASS` (`gate-summary.txt`).
- Final T2 recheck: `discovery.rs` production grew by ≈+50 lines inside its existing responsibility; no new module, seam, or dependency direction; no length-review trigger — route unchanged.
- The live acceptance run remains the recorded **obligation not run** (section above), carried into the PR verbatim.

Bounded repair 1 (2026-09-27): the PR's Windows CI leg failed one retained fence on a `/`-vs-`\` expected-string construction (test-only; production rendering correct). Corrected per the repository's cross-platform fixture rule; kas + default legs re-run green locally (`plan.md` "Bounded repair 1"); the terminal criterion still holds pending the re-run Windows leg, recorded there.

Review round 1 (2026-09-27, `review-decisions.md`): F1/F2 accepted and applied in `ab8e8d15` — the env wrappers are now fenced (claim C7) and platform-gated like kiro-cli's `dirs` build; F3/F4/P1 recorded without change. Terminal criterion still holds: every artifact satisfies its owning stage, no `FAIL` in the checkpoint or the round's repair record, CI green on Linux/macOS/Windows (run 36300873906). The live acceptance run remains the recorded obligation not run.

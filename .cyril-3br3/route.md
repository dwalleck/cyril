# Route: cyril-3br3

Change: `process_wsl_distro` reads `CYRIL_WSL_DISTRO` with `.ok()`, collapsing
`VarError::NotPresent` (normal) and `VarError::NotUnicode` (corrupt config) into
silent "unset". Mirror `bind_agent_location`: match on `VarError`, `warn!` on
`NotUnicode(raw)` naming the variable and the raw value, then treat as unset.
Date: 2026-09-27

Requester decisions (approved scope, given with the task):

- Match on `VarError` in `process_wsl_distro`; `NotPresent` → `None` silently;
  `NotUnicode(raw)` → `warn!` with variable + raw value, then `None`.
- Leave the adjacent `current_dir().ok()` alone.
- Fence: a `#[cfg(unix)]` child-process test beside
  `env_non_unicode_falls_back_via_child_process` in
  `crates/cyril-core/tests/win_wsl_wiring.rs`, asserting the warning is emitted
  for a non-Unicode `CYRIL_WSL_DISTRO` and the distro is treated as unset.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Premises: (a) `std::env::var` yields `VarError::NotUnicode(OsString)` for non-UTF-8 bytes on unix — std contract already relied on by `bind_agent_location` (`crates/cyril-core/src/platform/path.rs:102-113`) and fenced by `env_non_unicode_falls_back_via_child_process` (`tests/win_wsl_wiring.rs:92-110`), green on this revision (baseline run below). (b) Non-UTF-8 env bytes reach a child via `Command::env` + `OsStr::from_bytes` — the same sibling fence exercises it. (c) A thread-local `tracing_subscriber::fmt` capture observes `tracing::warn!` from the calling thread — the repository's own idiom (`workflow.rs:1191-1205`, `convert/mod.rs:843-856`), and `OnceLock::get_or_init` runs its closure on the calling thread (std contract). No external/system premise is uncovered. | no |
| 2 | Structural module shape | Only production module touched: `crates/cyril-core/src/platform/path.rs` (1113 lines incl. ~600 lines of colocated tests). The edit is inside the private `process_wsl_distro` (line 265) — the existing "resolve the process distro from env + cwd" responsibility; owner unchanged (`platform::path`). No public interface changes: `to_native`, `to_agent`, `wsl_to_win`, `win_to_wsl`, `translate_paths_in_json` signatures and semantics untouched; no seam, schema, or dependency-direction change; nothing moves between modules. Test-side: `tests/win_wsl_wiring.rs` gains one fence and a local log-capture writer (non-production). Env-var confinement fence `env_var_confined_to_platform_path` (`path.rs:588`) scans `src/` only and exempts `platform/path.rs` — unaffected. Length gates: none defined in the repository (no `clippy.toml`/`.clippy.toml`; `grep too_many_lines\|max-lines\|file-length` over `Cargo.toml` and crate manifests → no hits), so no length-review trigger; projected production delta ≈ +10 lines (a `match` replacing `.ok()`), well within any margin. | no |
| 3 | Production-scale risk | One env read and at most one `warn!` event, executed once per process inside the `OnceLock` init. No latency, throughput, memory, concurrency, or data-volume dimension. | no |
| 4 | Explicit behavior | Behavior contract (the behavior source; `spec.md` is N/A): (1) Given `CYRIL_WSL_DISTRO` is unset, when the process distro is first resolved, then no warning is logged and resolution proceeds as today (cwd donation on Windows; `None` off Windows). (2) Given `CYRIL_WSL_DISTRO` holds a valid non-empty Unicode value, when resolved, then it is the distro on Windows and `None` off Windows — unchanged. (3) Given `CYRIL_WSL_DISTRO` holds a non-Unicode value, when the process distro is first resolved, then exactly one `warn` event is logged naming `CYRIL_WSL_DISTRO` and the raw value (Debug-formatted `OsString`, the `bind_agent_location` mirror), and the value is treated exactly as unset — cwd donation still applies on Windows; WSL-internal paths pass through when no distro results. (4) Given the host is not Windows, when resolved, then the distro is `None` regardless of env (unchanged, load-bearing `cfg!` gate); the corrupt-value warning still fires there, because the read now precedes the gate — this is entailed by the requester's fence decision (a unix child fence asserting the warning can only be honest if unix emits it) and mirrors `bind_agent_location`'s platform-independent read. (5) `std::env::current_dir().ok()` is unchanged — an unavailable cwd is not config corruption. No unresolved decisions. | yes |

Unknown tests: none

## Selected route

Local — behavior explicit and approved; a `match` replacing `.ok()` inside one
existing private responsibility, no public interface, seam, ownership, or
scale change, no uncovered external premise.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict; contract recorded in T4 evidence) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict) |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD (red fence first, minimal fix,
fence stays as the regression fence).

## Terminal criterion

Local — focused behavioral verification:

- Red: the new fence
  `distro_non_unicode_warns_and_treats_as_unset_via_child_process` in
  `crates/cyril-core/tests/win_wsl_wiring.rs` FAILS on the unmodified
  `process_wsl_distro` (no warning captured).
- Green: the same fence PASSES after the fix, alongside the whole
  `win_wsl_wiring` binary and the `platform::path` unit tests
  (`cargo nextest run -p cyril-core --test win_wsl_wiring` and
  `cargo nextest run -p cyril-core -- platform::path`).
- Full CI-mirror gate (fmt, clippy all-features/default/no-default-features,
  nextest all-features/default/no-default-features, doc tests) recorded in the
  PR body.

Results are appended below after the hand-off.

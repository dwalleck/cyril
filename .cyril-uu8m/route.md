# Route: cyril-uu8m

Change: Skip the `write_atomic_unwritable_parent_errs_target_intact` fixture in `crates/cyril-core/src/protocol/kas/host_io.rs` when euid is 0, with a visible logged reason mirroring the PR #86 guard in `host_shell.rs`; no production code changes.
Date: 2026-09-27

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The one system premise — euid 0 bypasses the 0o555 parent's write-permission bits, so `write_atomic` creates its temp file and succeeds and the test's `expect_err` panics — is covered by valid current evidence: (a) the issue text records the failure verified under `unshare -r cargo nextest`; (b) the same bug class was proven and fixed for `host_shell.rs::system_host_checks_effective_execute_permission` in PR #86 (commit `ec474ef8`), whose guard reads `nix::unistd::geteuid().is_root()`; (c) this session reproduced it red before any edit — see **Repro** under Terminal criterion (`unshare -r` on this host maps to uid 0: `unshare -r id` → `uid=0(root)`). `nix` with the `user` feature is already a `[target.'cfg(unix)'.dependencies]` entry of cyril-core (`crates/cyril-core/Cargo.toml:58-61`, workspace `Cargo.toml:70`), so `geteuid` needs no dependency change. | no |
| 2 | Structural module shape | Only the `#[cfg(test)] mod tests` body of `host_io.rs` changes: an early-return guard inside one existing test function. No public interface, schema, seam, dependency direction, or responsibility owner changes; `write_atomic` and `write_text_file` are untouched. The module stays under the existing `kas` feature gate (`crates/cyril-core/src/protocol/mod.rs:14-15`). Length gate: no repository length gate exists — `grep -rniE "max.?lines\|line.?count\|wc -l\|file.?length\|length.?gate" .github/workflows/ scripts/` returns nothing (exit 1), and `.github/workflows/ci.yml` runs only fmt/clippy/nextest/doc-test legs; `host_io.rs` is 583 lines including tests and the change adds ~10 test lines, so no length-review trigger. | no |
| 3 | Production-scale risk | Test-only change; no production code path, latency, throughput, memory, concurrency, or data-volume dimension is touched. | no |
| 4 | Explicit behavior | **Given** the test process runs with euid 0 (a root CI container or a rootless user namespace such as `unshare -r`), **when** `write_atomic_unwritable_parent_errs_target_intact` runs, **then** it prints `skipping unwritable-parent fixture: euid is 0 and root bypasses permission bits` to stderr and returns before creating the fixture, so nextest reports the test as passed with the skip line in its captured output (visible under `--success-output immediate`). **Given** euid is not 0, **when** the test runs, **then** the existing fixture and all three assertions execute unchanged (error names `create temp file in`, target content `OLD` byte-identical, mode preserved) and the test passes with no skip line. **Given** any euid, **when** the guard is present, **then** no production code in `host_io.rs` differs from `main`. Requester decisions fixed the guard shape (mirror `host_shell.rs:800-810` exactly, `eprintln!` reason, never a silent pass) and the receipts (root run shows the skip line; non-root run passes without it). | yes |

Unknown tests: none

## Selected route

Local — test-only fixture guard within one existing responsibility; the single system premise has valid current evidence (issue repro, PR #86 precedent, in-session red repro), no interface or ownership changes, and the behavior contract is complete.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict yes; requester decisions fix guard shape, message, and receipts) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict no; the red repro is recorded under Terminal criterion) |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD (red repro first, then the guard, then the focused verification below and the full CI-mirroring gate)

## Terminal criterion

Local — focused behavioral verification:

1. **Root (skip path):** `unshare -r env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas --success-output immediate write_atomic_unwritable_parent` passes and its captured stderr contains the skip line.
2. **Non-root (assertion path):** `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas --success-output immediate write_atomic_unwritable_parent` passes and its output contains no skip line (so the assertions executed).
3. Final T2 recheck records no new trigger.

Environment note: this tool shell exports `CARGO_TARGET_DIR=""`, which cargo rejects outright; every cargo command below is prefixed with `env -u CARGO_TARGET_DIR` so the worktree's own `target/` is used. `unshare` is `/usr/bin/unshare`; `-r` maps the caller to uid 0 in a new user namespace.

### Repro (red, before any edit)

Result: 2026-09-27 | revision `f9bc81d8` (main, unmodified) | `unshare -r env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas write_atomic_unwritable_parent` | **FAIL as predicted** (nextest exit 100). Receipt: `.cyril-uu8m/repro-red.log`. The write into the 0o555 parent succeeded under euid 0, so `expect_err` panicked: `panicked at crates/cyril-core/src/protocol/kas/host_io.rs:495:43: unwritable parent must fail the write: ()`. This is the T1 premise observed directly, not inferred.

### Focused verification (green, after the guard)

Result: 2026-09-27 | `unshare -r env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas --success-output immediate write_atomic_unwritable_parent` | **PASS** — captured stderr is exactly `skipping unwritable-parent fixture: euid is 0 and root bypasses permission bits` (GATE-OK-verify-root echoed). Receipt: `.cyril-uu8m/verify-root.log`.

Result: 2026-09-27 | `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas --success-output immediate write_atomic_unwritable_parent` | **PASS** — no stderr section, `grep -c "skipping unwritable-parent"` = 0, so the fixture and all three assertions executed (GATE-OK-verify-nonroot echoed). Receipt: `.cyril-uu8m/verify-nonroot.log`.

Red→green contrast: same command family, same revision base, only the guard differs — before: FAIL at `host_io.rs:495`; after: PASS with the logged skip under euid 0 and an unchanged assertion path under euid 1000.

### Final T2 recheck

No new trigger. `git diff --stat` touches only `crates/cyril-core/src/protocol/kas/host_io.rs` (+14 lines, all inside the `#[cfg(test)]` test body — a guard, comment, `eprintln!`, and `return`) plus the `.cyril-uu8m/` artifacts. No production line, interface, seam, dependency, or ownership changed; no repository length gate exists (T2 evidence above), and no comment or formatting was removed to shrink anything.

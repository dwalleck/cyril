# Route: cyril-s2hb

Change: native crtool gather/facts, cancellable diagnostics library, and resolved-shell executable prefix.
Date: 2026-09-30
Source baseline: cc5eba260572e08fd20a58c8b0cdd1aaec782484; upstream discovered through origin/HEAD.
Invocation: gilfoyle cyril-s2hb, repository-local skill exposed by an uncommitted symlink.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The Python crtool and selftest are an independent functional reference, but current fixtures do not prove Rust/Python command parsing, timeout/cancellation, Windows process behavior, or volatile-field/version behavior. New cross-platform process and functional-output probes are required; existing source inspection is not execution evidence. | yes |
| 2 | Structural module shape | Create the cyril-review leaf (no Tokio/core/ACP); core owns the prefix, existing HostShell retains sole shell resolution ownership, and main gains only hidden early dispatch. Main and HostShell are protected parents. New crate/interface/dependency seams trigger module review independent of size. Scout inspected repository gates and found no existing numeric per-module ceiling; the local gilfoyle module-shape reference and >4,000 cumulative-changed-line review gate apply. Existing total-line signals: main 376; HostShell 1,036; Python oracle 1,070; selftest 387. Projected new production 500–900, verification 600–1,400, uncertainty 300–500 plus artifacts/fixtures; exact production census and reviewed growth ledger belong in design. | yes |
| 3 | Production-scale risk | Gather retains diffs and spawns per-file git; facts scans usages per symbol; diagnostics captures potentially large output while enforcing timeout/cancellation. Pipe deadlock, descendant process lifetime, and memory/data-volume behavior require independent fixtures and explicit measured bounds without silently narrowing Python behavior. | yes |
| 4 | Explicit behavior | Issue acceptance is authoritative for hidden dispatch, explicit-argv Git, functional equivalence, stamp/refusal, prefix rejection and library-only diagnostics. Cancellation result/tree semantics, missing stamp behavior, deterministic timestamp/duration/version controls, and POSIX/Windows diagnostics command parsing need explicit decisions before probes/design. | no |

Unknown tests: none. T1 contains identified unverified premises; T4 contains identified unresolved observable details, not absent research.

## Selected route

Empirical — cross-platform process/output/cancellation premises require observed oracle evidence before design and implementation.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — resolve T4 cancellation, command parsing and parity details |
| evidence.md, probe.* | prove-it-prototype | required — discharge T1 on available actual hosts |
| design.md | falsifiable-design | required — new crate, prefix seam, protected-parent and process ownership |
| plan.md | budgeted-plan | required — atomic verified slices and growth ledger |

Oracle checkpoint in checkpointed-build: required — Empirical route.

## Downstream sequence

interrogated-spec → prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Every empirical premise records PASS in prove-it-prototype; every later artifact satisfies its owning stage's completion criterion; checkpointed-build has no FAIL. Required cargo fmt --check, cargo test, cargo clippy -- -D warnings and actual acceptance smoke must pass. Final independently reviewed PR head must have zero unresolved findings and passing required CI before merge and issue closure. No gate is currently claimed PASS.

## Execution authorization and caps

The user approved routine gilfoyle signoffs/recommended decisions, with the explicit limit: “Only auto-approve decisions regarding the gilfoyle skill”. Scope expansion alone is not a stop; unavailable required hosts and destructive operations are stops. Do not use preapproval to waive evidence or silently change the issue's accepted architecture/behavior. Decisions made under this delegation are recorded as agent decisions authorized by the user, never as invented verbatim user decisions.

Historical route-stage counters: implementation attempts 0/5 and independent PR review/fix rounds 0/5. Routing and prerequisite inspection were not implementation attempts; parent cyril-5gb3 remained unchanged at route creation.

## Functional-equivalence amendment — 2026-09-30

Requester correction (verbatim):
> “It didn't need to be a 1 to 1 copy of the python script, just functionally the same”

Further requester clarification (verbatim):
> “The results and output don't have to be byte equivalent”

These latest corrections supersede the route's former byte-output/one-to-one parity framing. The selected Empirical route remains appropriate for native lifecycle, host, process, and functional-output evidence. Semantic fields/types, array/file/symbol/usage sequence, meaningful content, status/error/context, patch content, raw capture integrity, UTF-8 text, Unicode whitespace scope handling, and safety/lifecycle/native gates remain active. Presentation-only line endings are not a native LF-only gate. Old byte/text-wrapper evidence is historical; checkpoint-A2.md owns current evidence and outstanding qualification. Counters are unchanged at implementation cycle12/15 (cycle11 historical) and review3/5.

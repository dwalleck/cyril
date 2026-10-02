# Code review — `HEAD`

- **Head:** `e398f6099680`  **Scope:** `.`  **Files:** 1  **Patch bytes:** 149
- **Pipeline:** 1 finder angles reported (**missing: b-removed-behavior**) → 1 candidates → 1 after dedup + 0 from the gap sweep
- **Verdicts:** CONFIRMED 0, PLAUSIBLE 0, REFUTED 0, UNVERIFIED 1 — 1 candidates took three votes, 0 changed outcome
- `CONFIRMED` = trigger and wrong outcome named, line quoted. `PLAUSIBLE` = mechanism real, trigger uncertain. `UNVERIFIED` = no verdict was produced; kept because this is a recall-mode review.

## Summary

| # | Id | Location | Verdict | Issue |
|---|---|---|---|---|
| 1 | C01 | `src/lib.rs:2` | UNVERIFIED | New function never called |

## Findings

### 1. New function never called

- **Where:** `src/lib.rs:2`  **Verdict:** UNVERIFIED  **Id:** C01  **Angles:** a-line-scan
- **Failure scenario:** dead code ships
- **Votes:** UNVERIFIED, UNVERIFIED, UNVERIFIED → UNVERIFIED
- **Verifier reasoning:** no verdict file was written

## Refuted during verification

Listed so they are not re-derived later.

None.

## Run warnings

- decisions.json unreadable (top level is not an object); no dedup applied
- candidates/sweep.json missing: the gap sweep did not report
- ranking.json missing; findings are in discovery order
- not ranked, appended in discovery order: C01

# Review comments

Conventional Comments format (conventionalcomments.org). Each block is postable as-is at the location above it.

## 1. `src/lib.rs:2` — C01

**question (non-blocking):** New function never called

dead code ships

_Automated review · unverified by vote (unverified, unverified, unverified) · C01_


# Code review — `HEAD`

- **Head:** `e398f6099680`  **Scope:** `.`  **Files:** 1  **Patch bytes:** 149
- **Pipeline:** 6 finder angles reported (**missing: d-language-pitfalls, e-wrapper-proxy**) → 14 candidates → 10 after dedup + 2 from the gap sweep
- **Verdicts:** CONFIRMED 4, PLAUSIBLE 2, REFUTED 1, UNVERIFIED 5 — 3 candidates took three votes, 3 changed outcome
- `CONFIRMED` = trigger and wrong outcome named, line quoted. `PLAUSIBLE` = mechanism real, trigger uncertain. `UNVERIFIED` = no verdict was produced; kept because this is a recall-mode review.

## Summary

| # | Id | Location | Verdict | Issue |
|---|---|---|---|---|
| 1 | C03 | `src/lib.rs:10` | CONFIRMED | Loop bound excludes the last item |
| 2 | C08 | `src/util.rs:8` | CONFIRMED | Duplicate helper 8 |
| 3 | C01 | `src/lib.rs:22` | CONFIRMED | Unchecked unwrap \| on parse |
| 4 | S01 | `src/io.rs:40` | PLAUSIBLE | Leaked file handle on the error path |
| 5 | C06 | `src/util.rs:7` | PLAUSIBLE | Duplicate helper 5 |
| 6 | C02 | `src/io.rs` | UNVERIFIED | Missing fsync before rename |
| 7 | C04 | `src/util.rs:2` | UNVERIFIED | Duplicate helper 2 |
| 8 | C05 | `src/util.rs:3` | UNVERIFIED | Duplicate helper 3 |
| 9 | C07 | `src/util.rs:7` | UNVERIFIED | Duplicate helper 7 |
| 10 | C09 | `src/app.rs:5` | CONFIRMED | Special case bolted onto the shared parser |
| 11 | S02 | `src/io.rs` | UNVERIFIED | Sweep candidate without an id |

## Findings

### 1. Loop bound excludes the last item

- **Where:** `src/lib.rs:10`  **Verdict:** CONFIRMED  **Id:** C03  **Angles:** b-removed-behavior, a-line-scan
- **Failure scenario:** last element skipped when n > 0, and the total under-counts by one
- **Votes:** REFUTED, CONFIRMED, CONFIRMED → CONFIRMED
- **Finder evidence:** 0..=n vs 0..n
- **Verifier evidence:** evidence for C03.v2
- **Verifier reasoning:** reasoning for C03.v2

### 2. Duplicate helper 8

- **Where:** `src/util.rs:8`  **Verdict:** CONFIRMED  **Id:** C08  **Angles:** cleanup
- **Failure scenario:** the copies drift apart
- **Accepted by design:** docs/x.md: accepted on purpose
- **Verifier evidence:** evidence for C08
- **Verifier reasoning:** reasoning for C08
- **Ranking note:** real, but the author accepted it

### 3. Unchecked unwrap \| on parse

- **Where:** `src/lib.rs:22`  **Verdict:** CONFIRMED  **Id:** C01  **Angles:** a-line-scan
- **Failure scenario:** non-numeric input crashes the CLI before any output is written, non-numeric input crashes the CLI before any output is written, non-numeric input crashes the CLI before any output is written, non-numeric input crashes the CLI before any output is written, 
- **Finder evidence:** x.unwrap()
- **Verifier evidence:** evidence for C01
- **Verifier reasoning:** reasoning for C01

### 4. Leaked file handle on the error path

- **Where:** `src/io.rs:40`  **Verdict:** PLAUSIBLE  **Id:** S01  **Angles:** sweep
- **Failure scenario:** an early return skips close
- **Votes:** REFUTED, PLAUSIBLE, UNVERIFIED → PLAUSIBLE
- **Accepted by design:** README: accepted limitation
- **Verifier evidence:** evidence for S01.v2
- **Verifier reasoning:** reasoning for S01.v2

### 5. Duplicate helper 5

- **Where:** `src/util.rs:7`  **Verdict:** PLAUSIBLE  **Id:** C06  **Angles:** cleanup, cleanup
- **Failure scenario:** the copies drift apart
- **Verifier evidence:** evidence for C06
- **Verifier reasoning:** reasoning for C06
- **Would confirm:** run it on an empty list

### 6. Missing fsync before rename

- **Where:** `src/io.rs`  **Verdict:** UNVERIFIED  **Id:** C02  **Angles:** a-line-scan
- **Failure scenario:** —
- **Verifier reasoning:** no verdict file was written

### 7. Duplicate helper 2

- **Where:** `src/util.rs:2`  **Verdict:** UNVERIFIED  **Id:** C04  **Angles:** cleanup, cleanup
- **Failure scenario:** the copies drift apart
- **Verifier evidence:** evidence for C04
- **Verifier reasoning:** reasoning for C04

### 8. Duplicate helper 3

- **Where:** `src/util.rs:3`  **Verdict:** UNVERIFIED  **Id:** C05  **Angles:** cleanup, cleanup
- **Failure scenario:** the copies drift apart
- **Verifier reasoning:** verdict file unreadable: Expecting property name enclosed in double quotes: line 1 column 2 (char 1)

### 9. Duplicate helper 7

- **Where:** `src/util.rs:7`  **Verdict:** UNVERIFIED  **Id:** C07  **Angles:** cleanup
- **Failure scenario:** the copies drift apart
- **Verifier reasoning:** verdict file is not a JSON object

### 10. Special case bolted onto the shared parser

- **Where:** `src/app.rs:5`  **Verdict:** CONFIRMED  **Id:** C09  **Angles:** altitude
- **Failure scenario:** every new flag adds a branch
- **Verifier evidence:** evidence for C09
- **Verifier reasoning:** reasoning for C09

### 11. Sweep candidate without an id

- **Where:** `src/io.rs`  **Verdict:** UNVERIFIED  **Id:** S02  **Angles:** sweep
- **Failure scenario:** the finder forgot the id
- **Verifier reasoning:** no verdict file was written

## Refuted during verification

Listed so they are not re-derived later.

- **C10** `src/lib.rs:3` — unwrap in non-test code *(votes: CONFIRMED, REFUTED, REFUTED)*  
  *Refuted:* reasoning for C10.v2

## Run warnings

- decision names unknown pid(s) ['ghost-9']; ignored those
- ranking names unknown or refuted id 'C99'; ignored
- not ranked, appended in discovery order: C02, C04, C05, C07, C09, S02

# Review comments

Conventional Comments format (conventionalcomments.org). Each block is postable as-is at the location above it.

## 1. `src/lib.rs:10` — C03

**issue (blocking,test):** Loop bound skips the last item

With `n > 0` the last element is never visited.

Use `0..n`.

## 2. `src/util.rs:8` — C08

**issue (non-blocking):** Duplicate helper drifts from the original

Reuse `util::helper`.

## 3. `src/lib.rs:22` — C01

Same defect as C03; no separate comment.

## 4. `src/io.rs:40` — S01

**thought (non-blocking):** Leaked file handle on the error path

an early return skips close

This looks deliberate: README: accepted limitation

## 5. `src/util.rs:7` — C06

**question (non-blocking):** Duplicate helper 5

the copies drift apart

## 6. `src/io.rs` — C02

**question (non-blocking):** Missing fsync before rename

## 7. `src/util.rs:2` — C04

**suggestion (if-minor,non-blocking):** Reuse the existing helper

## 8. `src/util.rs:3` — C05

**question (non-blocking):** Duplicate helper 3

the copies drift apart

## 9. `src/util.rs:7` — C07

**question (non-blocking):** Duplicate helper 7

the copies drift apart

## 10. `src/app.rs:5` — C09

**suggestion (non-blocking):** Special case bolted onto the shared parser

every new flag adds a branch

## 11. `src/io.rs` — S02

**question (non-blocking):** Sweep candidate without an id

the finder forgot the id


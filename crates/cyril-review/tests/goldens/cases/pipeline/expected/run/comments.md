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


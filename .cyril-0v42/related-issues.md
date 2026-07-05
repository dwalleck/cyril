# cyril-0v42 — related issues (prove-it-prototype step 0)

Tracker searched 2026-07-05 (`rivets list` + keyword grep: atomic, host-io,
fsync, tempfile, symlink). Bounded at 5 minutes per skill.

## Direct lineage

- **cyril-7bdu** (closed, PR #36) — KAS-5 host-I/O responders; this issue was
  discovered during its review. `write_text_file` landed as bare
  `create_dir_all` + `tokio::fs::write` (host_io.rs:43-56). The 2026-07-01
  issue-set review promoted 0v42 to P2: only open issue whose failure mode is
  silent user-data loss.

## Adjacent, non-blocking

- **cyril-8tq6** (open, P3) — WSL-internal path translation for a Windows
  host. Interacts: the atomic path must apply `to_native_checked` translation
  BEFORE canonicalize/temp placement, same boundary as today. No dependency.
- **cyril-g9vt** (open, P3) — ADR-0004 loop-mediation gate seam. The atomic
  write stays inside the resolver, below any future gate. No dependency.
- **cyril-ihj1** (open, P4) — bounded read for read_text_file. Sibling
  robustness item, read-side only. No overlap.
- **cyril-ykkc** (open, P3) — local gates never compile kas-gated code.
  PROCESS NOTE for this branch: every gate run must include `--features kas`
  or the changed code is never compiled/tested locally.
- **cyril-dn91** (open, P3) — feature-gating vs bound engine. Context for
  where the code lives (`#[cfg(feature = "kas")]`), no behavior overlap.

No existing ticket describes the atomic-write work itself beyond cyril-0v42.
No prior probe of tempfile persist/mode/symlink semantics exists in
`experiments/` or `.cyril-7bdu/`.

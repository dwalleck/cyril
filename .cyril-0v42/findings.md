# cyril-0v42 — prove-it-prototype findings

Probe: `probe/` (Rust, tempfile pinned `=3.27.0` — the workspace-locked
version; `current-write` replicates host_io.rs:47-54 verbatim).
Oracle: `oracle.sh` — re-measures every claim with coreutils
(stat/readlink/sha256sum), a gdb openat catchpoint (`trace_openat.py`,
kernel-boundary ground truth; strace absent on this host, perf needs root),
and python (libc rename). **Final run: 24 pass, 0 fail** (all 12 scenarios).

## The hazard (current code), confirmed

- **S1** — `tokio::fs::write` opens the target `O_TRUNC` (gdb catchpoint saw
  `trunc=1` on the target path): the ORIGINAL content is destroyed the moment
  the open succeeds, before one new byte lands. In-place semantics: same
  inode, mode preserved (755 stays 755).
- **S11** — SIGKILL mid-write left the target at **size 0**: old 25 bytes
  gone, no new content. The silent-data-loss failure mode is real and was
  reproduced live, not just inferred.
- **S2** — current path writes THROUGH a symlink (link preserved, destination
  updated). Any fix must match this.

## The two footguns (naive `NamedTempFile` + `persist`), confirmed

- **S3** — persist over a 0755 file leaves **0600** (temp's creation mode).
  Permission clobbering is real, exactly as the issue notes predicted.
- **S4** — persist over a symlink **replaces the symlink with a regular
  file**; the link destination keeps stale content. Behavior change vs
  today's write-through.

## The fixed sequence, verified end-to-end

`canonicalize(target)` (fresh-file fallback: `create_dir_all(parent)` +
`canonicalize(parent).join(file_name)`) → capture existing mode (else
`0o666 & !umask`) → `NamedTempFile::new_in(canonical_parent)` → `write_all`
→ `sync_all` → `set_permissions` → `persist(canonical)`:

- **S5** 0755 preserved, content replaced. **S6** symlink preserved,
  destination updated. **S7** fresh file in missing parents created at
  umask-derived 644. **S8** empty content ⇒ empty file (not a no-op).
- **S12** — same SIGKILL regime as S11: target intact (exactly old content).
  Old-or-new, never partial.

## What I learned that I didn't know before

**The default temp directory is environment-controlled (`$TMPDIR`), and in
this very harness it lands on the SAME filesystem as targets** — the first
S9 run "failed" because rust `NamedTempFile::new()` AND python `mkstemp`
both honored `TMPDIR=/home/dwalleck/.claude/tmp` and the rename succeeded.
Forced to `/tmp` (tmpfs), both error EXDEV(18). Consequence: any temp
placement other than `new_in(target's own parent)` is wrong in both
directions — it can EXDEV-fail when filesystems differ AND it silently loses
same-directory atomicity guarantees when they don't.

## Facts the design must absorb

1. `std::fs::canonicalize` **errors on a missing target** (S10) — the
   parent-fallback path is mandatory, not optional.
2. `canonicalize` **errors on a dangling symlink** (S10) — open decision:
   error out (distinct message) vs write-through-creating the destination
   (today's tokio behavior). Needs a design call.
3. **SIGKILL leaves temp-file litter** in the target directory (S12 note) —
   `Drop` cleanup cannot run on kill -9. Design must acknowledge (naming
   pattern / doc note), not pretend it can't happen.
4. Umask can be read **without unsafe** (workspace forbids unsafe;
   `libc::umask` is unsafe) via `/proc/self/status` `Umask:` line — verified
   correct against the shell's `umask` (S7). Linux-only; non-Linux needs a
   documented fallback.
5. Inode changes under rename-replacement (S3/S5) — hardlink groups break,
   and `tail -f`-style watchers detach. Same tradeoff every atomic-write tool
   (rustfmt, cargo, editors with atomic save) accepts; document it.
6. `tempfile` is currently `[dev-dependencies]`-only in cyril-core — must be
   promoted (optional dep tied to the `kas` feature) for production use.

## Hard-gate checklist

- [x] Probe written, runs against the workspace-locked tempfile version and
  a verbatim copy of the production call sequence
- [x] Oracle defined (coreutils + gdb catchpoint + python), produces output
- [x] Probe and oracle agree on all 12 scenarios (24 checks)
- [x] Learned: default temp dir is TMPDIR-controlled ⇒ `new_in(parent)` is
  load-bearing, not stylistic

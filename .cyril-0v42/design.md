# cyril-0v42 — falsifiable design: atomic `write_text_file`

## Purpose

`kas::host_io::write_text_file` (host_io.rs:43-56) is cyril's executor for
every KAS agent file write (ADR-0003). It currently does `create_dir_all` +
`tokio::fs::write` — the probe proved (S1 kernel trace, S11 live SIGKILL)
that the original content is destroyed at open (`O_TRUNC`) and an
interrupted write leaves a partial or empty file. Replace it with
temp-file-in-target-parent + fsync + atomic rename so an interrupted write
never leaves a partial file: the target is always byte-exactly old or
byte-exactly new.

## Probe basis

`.cyril-0v42/findings.md` + `oracle.sh` — **42 checks, 0 fail** across 20
scenarios. The final shape (`fixed3_atomic` in `probe/src/main.rs`) is the
implementation blueprint; every mechanical claim below was validated against
tempfile `=3.27.0` (the workspace-locked version) before this design was
written. Design iterations v1 (`/proc` umask; not portable to macOS) and v2
(`create_new`; ENOSPC litters an empty fresh target) were falsified by
probing and discarded — v3 uses `tempfile::Builder::permissions(0o666)`,
whose mode passes through `open(2)` where umask applies natively.

## Input shapes (step 2)

`req.path` (after existing `to_native_checked` translation — unchanged):

| shape | covered by |
|---|---|
| relative path | rejected today; unchanged contract (C-existing, fence kept) |
| absolute, existing regular file (0644 / 0755 / 0600) | C1, C2 |
| absolute, existing read-only file (0444) | C8 (open decision D1) |
| absolute, missing file, parent exists | C3 |
| absolute, missing file + missing parents | C3 (mkdir -p kept) |
| symlink → existing destination | C4 |
| dangling symlink | C7 (open decision D2) |
| directory | C7 |
| existing file in unwritable (r-x) parent dir | C1/C7 (open decision D3) |
| parent path contains symlinks | canonicalize resolves; folded into C4's mechanism |

`req.content`: empty (C5), ASCII (C1), multi-byte Unicode (C5), large
multi-MB (S11/S12 used 768MB; C1).

Environment: any `$TMPDIR` (C6); Linux/macOS/Windows (C9 — mode-bit claims
are cfg(unix); Windows gets `Permissions.readonly()` semantics, same API).

Out of scope shapes: file owned by another user but in a writable dir
(rename succeeds where today's open would EACCES — accepted: the directory
owner already holds delete/replace power over such entries, rename is
exactly the operation the dir permits); named pipes / device nodes as
targets (not production-reachable from a KAS code agent; metadata() gate
treats them as "existing, not dir" and replaces — same as any file).

## Removed-invariant sweep (step 2b)

The change is **subtractive**: it removes "writes happen in place through
the target's existing inode" and with it:

1. **"Overwriting requires file-write permission"** — rename needs only
   dir-write, so 0444 protection would silently vanish (S17 proved it).
   → restored explicitly by the readonly() gate, claim C8.
2. **"A write never needs directory-write permission"** — inverted: atomic
   write requires dir-write even when the file itself is writable (S20).
   → claim C7 (Err, no in-place fallback — open decision D3).
3. **"Inode number is stable across writes"** — hardlink groups break;
   `tail -f`/inotify watchers detach; open fds keep seeing old content.
   → noted safe: standard atomic-save tradeoff (rustfmt, editors); cyril
   has no in-process consumers of these files' inodes. Documented, no claim.
4. **"No foreign files ever appear in the target dir"** — a transient
   `.tmpXXXXXX` appears during write; SIGKILL orphans it (S12 note; Drop
   cannot run on kill -9). → documented; hidden dot-prefix keeps it out of
   globs. No cleanup daemon (negative space).
5. **Non-blocking invariant (ADR-0004/C4 of cyril-7bdu)** — noted safe:
   `spawn_blocking` offloads to the same blocking pool `tokio::fs` uses
   internally; the acp connection still spawns each inbound request
   (`rpc.rs:272`), so nothing new can pin the bridge thread. host_io.rs:9-18
   doc comment is updated to sanction this pattern. End-to-end fence:
   existing live smoke `kas_fs_host_io_smoke`.

## Claims

1. **C1 (atomicity)** — A write that fails or dies at any point leaves an
   existing target byte-identical to its pre-write content; a completed
   write leaves exactly the new content.
2. **C2 (mode preservation)** — An existing writable target's permission
   bits survive the write unchanged (0755 stays 0755, 0600 stays 0600).
3. **C3 (fresh-file parity)** — A missing target (parents auto-created)
   ends with the same mode today's path yields: `0666 & !umask`.
4. **C4 (symlink write-through)** — A symlink target remains a symlink and
   its resolved destination receives the new content atomically.
5. **C5 (byte-exactness)** — Content round-trips byte-exact including empty
   (empty file, not no-op) and multi-byte Unicode.
6. **C6 (temp placement)** — The temp file is created in the canonical
   target's parent; outcome is independent of `$TMPDIR` and filesystem
   topology.
7. **C7 (failure modes are inert)** — Directory target, dangling symlink,
   and unwritable parent each return a distinct Err and mutate nothing.
8. **C8 (read-only protection preserved)** — A read-only target
   (`Permissions::readonly()`) is refused with a distinct Err; file and
   mode untouched.
9. **C9 (dependency hygiene)** — The default build's dependency graph gains
   no tempfile; the `--features kas` build compiles, lints, and tests green.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| C1 | atomicity | SIGKILL mid-768MB write over 25-byte target; sha ∉ {old, full-new} falsifies | sha256sum + stat (S12) | 1m | **passed** | unit `failed_write_leaves_target_intact` (r-x parent ⇒ Err + content/mode intact, cfg(unix)); S12 stays audit-trail |
| C2 | mode kept | 0755 target; post-write mode ≠ 755 falsifies | stat (S13) | 10s | **passed** | unit `write_preserves_existing_mode` (0755 + 0600, cfg(unix)) |
| C3 | fresh parity | fresh path a/b/f.txt; mode ≠ 0666&~umask falsifies | shell umask arithmetic (S15) | 10s | **passed** | unit `fresh_write_matches_plain_create_mode` — control file made by `std::fs::File::create` in the same dir (today's mechanism = independent in-test oracle), cfg(unix) |
| C4 | symlink kept | link→dest; link no longer symlink OR dest lacks content falsifies | readlink + sha256sum (S14) | 10s | **passed** | unit `write_through_symlink_preserves_link` (cfg(unix)) |
| C5 | byte-exact | empty + Unicode writes; any byte diff falsifies | stat size + read-back (S8) | 10s | **passed** | existing unit `write_creates_parents_and_exact_content` (cross-platform, unchanged) |
| C6 | temp-in-parent | TMPDIR=/tmp (tmpfs), target on btrfs; EXDEV falsifies (S9 proved the boundary bites a TMPDIR impl) | rust persist + python rename EXDEV control (S18+S9) | 30s | **passed** | C1's fence asserts the Err names temp creation in the parent — a TMPDIR-placed temp yields a different failure point/message; S18 stays audit-trail |
| C7 | inert failures | dir target / dangling link / r-x parent; any mutation or Ok falsifies | ls/stat/sha before-after (S16, S19, S20) | 30s | **passed** | units `write_to_directory_target_errs` (cross-platform), `dangling_symlink_target_errs` (cfg(unix)), + C1 fence |
| C8 | readonly refused | 0444 target; Ok or content change falsifies (fixed2 shape FAILED this — S17 first run) | cat + stat (S17) | 10s | **passed** | unit `readonly_target_refused` (cross-platform via `set_readonly(true)`) |
| C9 | dep hygiene | `cargo tree` default graph contains tempfile ⇒ falsified; kas build red ⇒ falsified | cargo tree / CI | 3m | pending (build-time) | CI kas legs (build/clippy/nextest, ci.yml:140-147) + default-build legs |

Non-vacuity (named buggy implementations): today's `tokio::fs::write` fails
C1/C7-dangling (S11; creates dangling-link destinations); naive
`NamedTempFile+persist` fails C2/C3/C4 (S3/S4); the probed fixed2 shape
fails C8 (S17 first run); `NamedTempFile::new()` (TMPDIR) fails C6 (S9
EXDEV); an `if !content.is_empty()` guard fails C5. Every falsifier has a
distinct scenario ID and every fence a distinct test name — failures
localize to a claim.

**Cheapest falsifier status: the entire C1-C8 battery ran pre-approval — 42
oracle checks, 0 fail.**

## Open decisions (for the design pause)

- **D1 read-only target**: REFUSE with distinct Err (recommended; preserves
  today's EACCES protection — an agent cannot silently replace a file the
  user chmod'd 0444) — vs replace-anyway (rustfmt-style). Probed both:
  S17.
- **D2 dangling symlink target**: distinct Err, destination NOT created
  (recommended; hand-rolling link-chain resolution invites bugs and today's
  create-through behavior is a silent surprise) — vs today's behavior of
  creating the destination through the link. Probed: S16.
- **D3 unwritable parent + writable file**: distinct Err (recommended;
  atomicity is impossible there and a silent in-place fallback would
  reintroduce exactly the bug this issue exists to fix) — vs in-place
  fallback with warn. This is a behavior change: today such a write
  SUCCEEDS in place. Probed: S20.
- **D4 directory fsync**: skip (recommended; its failure mode is "old
  content survives", which the issue's AC explicitly permits; if ever
  added it must be unix-gated). Settled rationale per issue notes — no
  tracker entry needed.

## Negative space (what this deliberately does NOT do)

1. No in-place fallback when atomicity is impossible (D3) — Err, never
   silently non-atomic.
2. No ownership (chown) / xattr / ACL preservation — mode bits (unix) and
   readonly flag (windows) only; chown needs privileges cyril doesn't have.
3. No directory fsync (D4) — old-content-survives crash window accepted.
4. No temp-litter cleanup daemon — SIGKILL orphans one hidden `.tmpXXXXXX`;
   documented, dot-prefixed, auto-cleaned on every non-kill failure path
   via Drop.
5. No inode stability — hardlinks/watchers detach on rename, the standard
   atomic-save tradeoff.
6. No changes to `read_text_file`, `to_native_checked` (path translation —
   WSL-host concerns stay with cyril-8tq6), terminal responders
   (cyril-ufie, closed), or the loop-mediation gate seam (cyril-g9vt).

## Implementation sketch

One file + manifest:

- `crates/cyril-core/src/protocol/kas/host_io.rs`
  - new sync helper `write_atomic(path: &Path, content: &str) -> io::Result<()>`
    — the fixed3 sequence verbatim (canonicalize-or-parent-fallback →
    metadata gate: dir / readonly / dangling-symlink / fresh →
    `Builder::new()` [+ `.permissions(from_mode(0o666))` cfg(unix) when
    fresh] `.tempfile_in(canonical_parent)` → `write_all` → `sync_all` →
    `set_permissions(existing)` when existing → `persist`).
  - `write_text_file` resolver: clone `path`+`content`, run the helper in
    ONE `tokio::task::spawn_blocking` (issue-notes shape; single threadpool
    hop); map `JoinError` and each distinct io failure to `-32603` via
    `io_err`-style messages ("target is a directory", "target is a dangling
    symlink", "target is read-only", "create temp file in <dir>",
    "persist <path>").
  - update the module doc (host_io.rs:9-18) to sanction `spawn_blocking`
    as the approved sync form (async `tokio::fs` remains fine for reads).
- `crates/cyril-core/Cargo.toml` — `tempfile = { workspace = true,
  optional = true }` in `[dependencies]`; `kas = [..., "dep:tempfile"]`;
  dev-dependency entry stays (tests use tempdir regardless of feature).
- Windows note: production code touches no `PermissionsExt` outside
  `#[cfg(unix)]`; `canonicalize`'s `\\?\` prefix is written back via std
  APIs which accept it.

## Tracker references

cyril-8tq6, cyril-g9vt, cyril-ykkc, cyril-7bdu (closed), cyril-ufie
(closed) — all verified to exist via `rivets show`/`rivets list` during
step 0 and this design. No new deferrals created by this design; D1-D4 are
decisions resolved at the pause, not deferred work.

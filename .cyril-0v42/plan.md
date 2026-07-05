# cyril-0v42 — budgeted plan: atomic `write_text_file`

Design: `.cyril-0v42/design.md` (approved 2026-07-05; D1 refuse-readonly,
D2 dangling-symlink-error, D3 no-in-place-fallback, D4 no-dir-fsync).
Oracle basis: `oracle.sh` 42/42 green against the probe's `fixed3_atomic`.

**Gate battery per slice** (cyril-ykkc: the kas tree compiles ONLY with the
feature — a default-build gate never sees this code):

```
cargo nextest run -p cyril-core --features kas          # kas unit fences
cargo clippy -p cyril-core --features kas --all-targets -- -D warnings
cargo clippy --all-targets -- -D warnings               # default build stays green
cargo nextest run                                       # default tests stay green
cargo fmt --check                                       # verified green on main pre-branch
cargo test --doc -p cyril-core --features kas           # doctests (nextest skips them)
```

Run each command bare (real exit codes) — never `| tail`.

---

## Slice 1: `write_atomic` helper + tempfile dependency promotion

**Claim:** C1 (atomicity mechanics), C2 (mode preservation), C3
(fresh-file umask parity), C5 (byte-exact incl. empty) — the core sequence.
**Oracle:** probe `fixed3_atomic` semantics, already validated by oracle.sh
S13/S15/S8 (coreutils re-measurement); in-test independent oracle for C3 is
a control file created by `std::fs::File::create` (today's creation
mechanism) in the same tempdir.
**Stress fixture:**
- 0600 secret file → written → mode must STAY 0600 (fails under a bug that
  applies the fresh-file 0666&umask branch to existing targets — the
  mode-WIDENING direction S13 alone can't catch).
- 0755 script → stays 755 (fails under naive persist, probe S3).
- fresh `a/b/c.txt` with missing parents → exists, mode == control file's
  mode (fails under temp-default 0600 leak).
- empty content over existing content → empty file (fails under
  `is_empty` no-op guard).
- multi-byte Unicode `"héllo\n世界\n"` → byte-exact read-back.
**Loop budget:** no new loops; `write_all`/`sync_all` are O(bytes) single
pass, bytes ≤ ~10 MB per agent write in production, per-callback (not
always-on); ≲ 10 syscalls per write — within budget.
**Wall budget:** n/a (request-scoped, not always-on).
**Files:** `crates/cyril-core/Cargo.toml`,
`crates/cyril-core/src/protocol/kas/host_io.rs`

**Code (advisory):** `fixed3_atomic` from `probe/src/main.rs`, adapted:
`pub(crate) fn write_atomic(path: &Path, content: &str) -> std::io::Result<()>`
with distinct error messages ("target is a directory", "target is
read-only", "target is a dangling symlink", context on temp-create and
persist failures). Gates (dir/readonly/dangling) land here as part of the
sequence but their FENCES land in slice 2. Cargo.toml: `[dependencies]
tempfile = { workspace = true, optional = true }`; feature
`kas = [..., "dep:tempfile"]`; dev-dependency entry stays.
`#[cfg(unix)]` wraps only the `Builder::permissions(from_mode(0o666))`
fresh-file branch; everything else is cross-platform std.

**Verification:**
- [ ] Gate battery green
- [ ] Stress fixtures (5 above) pass as colocated unit tests
- [ ] Probe oracle unchanged-green: rerun `./oracle.sh` (env control)
- [ ] No new loops (budget vacuously holds)

## Slice 2: failure-mode gates fenced — refusals mutate nothing

**Claim:** C7 (dir target / dangling symlink / unwritable parent are inert
distinct Errs), C8 (read-only refused), C6 (temp-in-parent, via the
r-x-parent error-message assert).
**Oracle:** oracle.sh S16/S17/S19/S20 (ls/stat/cat/sha before-after) —
already green against the probe; the unit fences replicate those checks
against the PRODUCTION helper.
**Stress fixture:**
- dir target → Err mentions "directory"; dir still exists (fails under an
  impl that renames over / unlinks the dir).
- dangling symlink → Err mentions "dangling"; link intact; destination NOT
  created (fails under today's tokio create-through behavior — the named
  buggy impl).
- 0444 file → Err mentions "read-only"; content AND mode byte-identical
  (fails under the fixed2 shape, which S17 first-run proved replaces it).
- existing file in `chmod 555` parent → Err names temp creation in the
  parent dir; content intact (fails under a TMPDIR-placed-temp impl, which
  would fail later at persist with a different message — this assert IS
  C6's deterministic fence). cfg(unix); restore 755 in test teardown so
  tempdir cleanup works.
**Loop budget:** no new loops.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/kas/host_io.rs` (tests only)

**Verification:**
- [ ] Gate battery green
- [ ] All four fixtures pass; each failure names its claim (distinct tests)
- [ ] Probe oracle still green (no code change expected to affect it)
- [ ] Budgets vacuous

## Slice 3: wire the resolver — spawn_blocking, error mapping, symlink fence

**Claim:** C4 (symlink write-through at resolver level), C5 (resolver
behavior unchanged for parents/empty/Unicode — existing tests untouched),
plus the design's non-blocking rationale (module doc updated to sanction
`spawn_blocking`).
**Oracle:** oracle.sh S14 (readlink + sha256sum) for C4; the EXISTING
`write_creates_parents_and_exact_content` and
`relative_path_rejected_with_absolute_error` tests are the
regression oracle for "resolver contract unchanged" — they must pass
WITHOUT edits.
**Stress fixture:**
- symlink → dest: resolver write leaves link a symlink, dest gets content
  (fails under rename-over-the-link, probe S4's bug).
- JoinError path: not deterministically reachable in test (blocking task
  panics only on helper panic; helper returns Err instead) — mapped
  defensively to -32603; classified sanity-hint, no fence (documented
  here, not a doc-comment lie: no doc precondition claims otherwise).
- error passthrough: dir-target write via the RESOLVER yields -32603 whose
  message contains the helper's "directory" text (fails if spawn_blocking
  swallows/rewords the io error).
**Loop budget:** no new loops (one `spawn_blocking` hop per request;
per-callback, not always-on).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/kas/host_io.rs`

**Code (advisory):** `write_text_file` clones `(path, content)` into ONE
`tokio::task::spawn_blocking(move || write_atomic(&path, &content))`;
`JoinError` → `-32603 "write_text_file task failed"`; `io::Error` →
existing `io_err("write_text_file", ...)`. Module doc 9-18 reworded:
sync std::fs is sanctioned ONLY inside `spawn_blocking` (single hop);
naked sync calls on the bridge runtime remain forbidden.

**Verification:**
- [ ] Gate battery green
- [ ] Symlink + error-passthrough fixtures pass; existing tests pass unedited
- [ ] Probe oracle still green
- [ ] Budgets vacuous

## Slice 4: dependency hygiene + end-to-end verification

**Claim:** C9 (default build gains no tempfile; kas build fully green) +
design's C10 rationale (live smoke exercises the real bridge path).
**Oracle:** `cargo tree` (cargo's own resolver = independent of our
manifest edit intent); CI-mirror gate battery; live
`kas_fs_host_io_smoke` (a real KAS turn through cyril's bridge — the
binary-level oracle) if a working kiro-cli login exists, else recorded as
environment-blocked with the CI legs as the fence.
**Stress fixture:** `cargo tree -p cyril-core -e normal | grep tempfile`
must be EMPTY (fails if tempfile lands non-optional);
`cargo tree -p cyril-core -e normal --features kas | grep tempfile` must
show 3.27.x (fails if the feature wiring is wrong — dep silently absent
would only surface at kas compile, which the gate battery also catches).
**Loop budget:** n/a (no code).
**Wall budget:** n/a.
**Files:** none (evidence recorded in `.cyril-0v42/build-audit.md`)

**Verification:**
- [ ] Both cargo tree assertions hold
- [ ] Full gate battery green (mirrors ci.yml default + kas legs)
- [ ] Live smoke run (or environment-block recorded with reason)
- [ ] Probe oracle final rerun green; results recorded in build-audit.md

---

## Plan Self-Review

1. **Loops:** the only iteration introduced anywhere is byte I/O inside
   `write_all` (O(bytes), ≤ ~10 MB, request-scoped) — no gaps.
2. **Fixtures:** every fixture names its bug class (mode-widening,
   temp-default-mode leak, is_empty no-op, naive-persist clobber,
   create-through-dangling-link, silent readonly replace, TMPDIR temp
   placement, rename-over-symlink, error-swallowing spawn wrapper,
   non-optional dep leak) — none are happy-path-only.
3. **Doc preconditions:** "path absolute + translated" — load-bearing,
   ALREADY enforced at runtime by `to_native_checked` (-32602), unchanged.
   Module-doc threading rule — sanity hint for maintainers, enforced by
   review + the doc text itself; no runtime check possible or needed.
   JoinError mapping — sanity hint, documented in slice 3. No unenforced
   load-bearing preconditions.
4. **Write targets:** production code writes only to the wire
   (acp Result = data) and `tracing` (diagnostic) — no stdout/stderr
   prints. Test/audit output goes to the test harness and
   `.cyril-0v42/build-audit.md` (audit trail).
5. **Tracker references:** cyril-ykkc (gate discipline, verified),
   cyril-8tq6 / cyril-g9vt (explicitly untouched, verified),
   cyril-7bdu / cyril-ufie (closed lineage, verified). No new deferrals;
   nothing to file.

Claim coverage vs design: C1→S1, C2→S1, C3→S1, C4→S3, C5→S1+S3, C6→S2,
C7→S2, C8→S2, C9→S4, C10 rationale→S3 doc + S4 smoke. Complete.

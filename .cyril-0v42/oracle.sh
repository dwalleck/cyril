#!/usr/bin/env bash
# Independent oracle for cyril-0v42 probes. The probe reports via Rust
# std::fs; this script re-measures with coreutils (stat/readlink/sha256sum),
# a gdb openat catchpoint (kernel syscall ground truth — strace absent, perf
# needs root), and python (libc rename) — different mechanisms, same answers.
# Run from .cyril-0v42/: ./oracle.sh
set -u
PROBE="$(dirname "$0")/probe/target/debug/probe-0v42"
WORK="$(mktemp -d "$HOME/.cache/cyril-0v42-oracle.XXXXXX")" # /home = btrfs, same-fs as real targets
PASS=0; FAIL=0
ok()   { echo "PASS: $1"; PASS=$((PASS+1)); }
bad()  { echo "FAIL: $1"; FAIL=$((FAIL+1)); }
check(){ if [ "$2" = "$3" ]; then ok "$1 [$2]"; else bad "$1 [want $3, got $2]"; fi; }

NEW_SHA=$(printf 'NEW-CONTENT-FROM-PROBE\n' | sha256sum | cut -d' ' -f1)

echo "══ S1: current-write on existing 0755 file — kernel O_TRUNC, in-place inode, mode kept"
t="$WORK/s1.txt"; printf 'OLD\n' > "$t"; chmod 755 "$t"
ino_before=$(stat -c %i "$t")
timeout 120 gdb -batch -q -x "$(dirname "$0")/trace_openat.py" --args "$PROBE" current-write "$t" > "$WORK/s1.trace" 2>/dev/null
grep -q "s1\.txt.*trunc=1" "$WORK/s1.trace" && ok "S1 kernel shows O_TRUNC open on target" || bad "S1 no O_TRUNC observed"
check "S1 mode preserved by in-place write" "$(stat -c %a "$t")" "755"
check "S1 inode unchanged (in-place)" "$(stat -c %i "$t")" "$ino_before"
check "S1 content replaced" "$(sha256sum < "$t" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S2: current-write through a symlink — link preserved, destination written"
dest="$WORK/s2-dest.txt"; link="$WORK/s2-link.txt"
printf 'OLD\n' > "$dest"; ln -s "$dest" "$link"
"$PROBE" current-write "$link" > "$WORK/s2.out"
[ -L "$link" ] && ok "S2 symlink still a symlink" || bad "S2 symlink replaced"
check "S2 destination received content" "$(sha256sum < "$dest" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S3: naive-atomic over 0755 file — mode clobbered to 600? inode changes?"
t="$WORK/s3.txt"; printf 'OLD\n' > "$t"; chmod 755 "$t"
ino_before=$(stat -c %i "$t")
"$PROBE" naive-atomic "$t" > "$WORK/s3.out"
echo "  observed mode after naive persist: $(stat -c %a "$t") (footgun if 600)"
[ "$(stat -c %a "$t")" != "755" ] && ok "S3 naive persist does NOT preserve 755 (footgun confirmed)" || bad "S3 naive persist preserved mode (footgun refuted)"
[ "$(stat -c %i "$t")" != "$ino_before" ] && ok "S3 inode changed (rename semantics)" || bad "S3 inode unchanged"

echo "══ S4: naive-atomic over a symlink — symlink replaced by regular file?"
dest="$WORK/s4-dest.txt"; link="$WORK/s4-link.txt"
printf 'OLD\n' > "$dest"; ln -s "$dest" "$link"
"$PROBE" naive-atomic "$link" > "$WORK/s4.out"
[ ! -L "$link" ] && ok "S4 naive persist REPLACED the symlink (footgun confirmed)" || bad "S4 symlink survived (footgun refuted)"
check "S4 destination untouched (old content)" "$(cat "$dest")" "OLD"

echo "══ S5: fixed-atomic over 0755 file — mode preserved, content new"
t="$WORK/s5.txt"; printf 'OLD\n' > "$t"; chmod 755 "$t"
"$PROBE" fixed-atomic "$t" > "$WORK/s5.out"
check "S5 mode preserved" "$(stat -c %a "$t")" "755"
check "S5 content replaced" "$(sha256sum < "$t" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S6: fixed-atomic through a symlink — link preserved, destination written"
dest="$WORK/s6-dest.txt"; link="$WORK/s6-link.txt"
printf 'OLD\n' > "$dest"; ln -s "$dest" "$link"
"$PROBE" fixed-atomic "$link" > "$WORK/s6.out"
[ -L "$link" ] && ok "S6 symlink still a symlink" || bad "S6 symlink replaced"
check "S6 destination received content" "$(sha256sum < "$dest" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S7: fixed-atomic fresh file in missing parents — created, umask mode"
t="$WORK/s7/a/b/fresh.txt"
"$PROBE" fixed-atomic "$t" > "$WORK/s7.out"
want_mode=$(printf '%o' $(( 0666 & ~0$(umask) )))
check "S7 fresh-file mode is umask-derived" "$(stat -c %a "$t")" "$want_mode"
check "S7 content written" "$(sha256sum < "$t" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S8: fixed-atomic with EMPTY content (kill-atomic mb=0) — empty file, not a no-op"
t="$WORK/s8/empty.txt"
"$PROBE" kill-atomic "$t" 0 > "$WORK/s8.out"
[ -f "$t" ] && ok "S8 empty write created the file" || bad "S8 file absent"
check "S8 size is 0" "$(stat -c %s "$t")" "0"

echo "══ S9: cross-device persist (/tmp tmpfs -> /home btrfs) — EXDEV both ways"
# First run surprised us: default temp honors \$TMPDIR (harness sets it under
# /home => same fs => rename SUCCEEDED). Force /tmp to demonstrate the boundary;
# the lesson 'default temp dir is env-controlled' goes in findings.md.
t="$WORK/s9.txt"
TMPDIR=/tmp "$PROBE" exdev "$t" > "$WORK/s9.out"
grep -q 'raw_os_error=Some(18)' "$WORK/s9.out" && ok "S9 rust persist errored EXDEV(18)" || bad "S9 rust persist did not EXDEV: $(cat "$WORK/s9.out")"
py_errno=$(python3 - "$t" <<'EOF'
import sys, os, tempfile
fd, src = tempfile.mkstemp(dir="/tmp")
os.close(fd)
try:
    os.rename(src, sys.argv[1]); print("none")
except OSError as e:
    print(e.errno)
EOF
)
check "S9 python os.rename same boundary errno" "$py_errno" "18"

echo "══ S10: canonicalize on missing file vs dangling symlink"
"$PROBE" canon "$WORK/does-not-exist.txt" > "$WORK/s10a.out"
grep -q 'canon: Err' "$WORK/s10a.out" && ok "S10 canonicalize(missing file) errors (parent-fallback needed)" || bad "S10 canonicalize(missing) succeeded?"
ln -s "$WORK/nowhere-real" "$WORK/s10-dangling"
"$PROBE" canon "$WORK/s10-dangling" > "$WORK/s10b.out"
grep -q 'canon: Err' "$WORK/s10b.out" && ok "S10 canonicalize(dangling symlink) errors (edge to decide)" || bad "S10 canonicalize(dangling) succeeded?"

echo "══ S11: HAZARD — SIGKILL mid tokio::fs::write leaves partial file, old gone (best-effort timing)"
t="$WORK/s11.txt"; printf 'OLD-CONTENT-MUST-SURVIVE\n' > "$t"
old_sha=$(sha256sum < "$t" | cut -d' ' -f1)
"$PROBE" kill-write "$t" 768 > "$WORK/s11.out" 2>&1 &
pid=$!
# Fork-free watcher (shell stat-per-iter was too slow to hit the ~100ms window):
# kill the writer the instant the target size is neither old (25) nor full.
python3 - "$t" "$pid" <<'EOF'
import os, signal, sys, time
t, pid, full = sys.argv[1], int(sys.argv[2]), 768 << 20
deadline = time.monotonic() + 30
while time.monotonic() < deadline:
    try:
        sz = os.stat(t).st_size
    except OSError:
        continue
    if sz not in (25, full):
        os.kill(pid, signal.SIGKILL)
        break
EOF
wait "$pid" 2>/dev/null
sz=$(stat -c %s "$t"); new_sha=$(sha256sum < "$t" | cut -d' ' -f1)
echo "  post-kill size=$sz (full would be $((768 << 20)))"
if [ "$new_sha" != "$old_sha" ] && [ "$sz" -lt $((768 << 20)) ]; then
    ok "S11 target is PARTIAL: old content destroyed, new incomplete (hazard demonstrated)"
else
    echo "SKIP: S11 kill window missed (size=$sz) — strace O_TRUNC in S1 remains the deterministic proof"
fi

echo "══ S12: fixed-atomic under the same SIGKILL regime — target old-or-new, never partial"
t="$WORK/s12.txt"; printf 'OLD-CONTENT-MUST-SURVIVE\n' > "$t"
old_sha=$(sha256sum < "$t" | cut -d' ' -f1)
"$PROBE" kill-atomic "$t" 768 > "$WORK/s12.out" 2>&1 &
pid=$!
for _ in $(seq 1 20000); do
    if compgen -G "$WORK/.tmp*" > /dev/null || compgen -G "$WORK/tmp*" > /dev/null; then kill -9 "$pid" 2>/dev/null; break; fi
done
wait "$pid" 2>/dev/null
sz=$(stat -c %s "$t"); new_sha=$(sha256sum < "$t" | cut -d' ' -f1)
full_sha=$(head -c $((768 << 20)) /dev/zero | tr '\0' 'N' | sha256sum | cut -d' ' -f1)
if [ "$new_sha" = "$old_sha" ] || [ "$new_sha" = "$full_sha" ]; then
    ok "S12 target intact after kill (size=$sz): old-or-new, never partial"
else
    bad "S12 target corrupted: size=$sz, sha=$new_sha"
fi
leftover=$(find "$WORK" -maxdepth 1 -name '.tmp*' -o -maxdepth 1 -name 'tmp*' | wc -l)
echo "  note: $leftover leftover temp file(s) after SIGKILL (Drop cleanup cannot run on kill -9)"

echo "══ S13: fixed3-atomic (design shape) over 0755 — mode preserved, content new"
t="$WORK/s13.txt"; printf 'OLD\n' > "$t"; chmod 755 "$t"
"$PROBE" fixed3-atomic "$t" > "$WORK/s13.out"
check "S13 mode preserved" "$(stat -c %a "$t")" "755"
check "S13 content replaced" "$(sha256sum < "$t" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S14: fixed3-atomic through a symlink — link preserved, destination written"
dest="$WORK/s14-dest.txt"; link="$WORK/s14-link.txt"
printf 'OLD\n' > "$dest"; ln -s "$dest" "$link"
"$PROBE" fixed3-atomic "$link" > "$WORK/s14.out"
[ -L "$link" ] && ok "S14 symlink still a symlink" || bad "S14 symlink replaced"
check "S14 destination received content" "$(sha256sum < "$dest" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S15: fixed3-atomic fresh file in missing parents — umask mode WITHOUT /proc read"
t="$WORK/s15/a/b/fresh.txt"
"$PROBE" fixed3-atomic "$t" > "$WORK/s15.out"
want_mode=$(printf '%o' $(( 0666 & ~0$(umask) )))
check "S15 fresh-file mode is umask-derived (via create_new)" "$(stat -c %a "$t")" "$want_mode"
check "S15 content written" "$(sha256sum < "$t" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S16: fixed3-atomic on a DANGLING symlink — Err, link intact, destination NOT created"
ln -s "$WORK/s16-nowhere" "$WORK/s16-link"
"$PROBE" fixed3-atomic "$WORK/s16-link" > "$WORK/s16.out"
grep -q 'fixed3: Err' "$WORK/s16.out" && ok "S16 dangling symlink write errored" || bad "S16 dangling symlink write did not error: $(cat "$WORK/s16.out")"
[ -L "$WORK/s16-link" ] && ok "S16 link still present" || bad "S16 link gone"
[ ! -e "$WORK/s16-nowhere" ] && ok "S16 destination NOT silently created" || bad "S16 destination was created"

echo "══ S17: fixed3-atomic on 0444 read-only target — REFUSED, content untouched"
# fixed2 probing showed temp+rename silently bypasses read-only protection
# (rename needs only dir write). fixed3 adds the readonly() gate to match
# today's EACCES behavior: refuse and leave the file alone.
t="$WORK/s17.txt"; printf 'OLD\n' > "$t"; chmod 444 "$t"
"$PROBE" current-write "$t" > "$WORK/s17-current.out" 2>&1 || true
grep -q 'panicked\|denied' "$WORK/s17-current.out" && echo "  current-write on 0444: EACCES (in-place needs file write perm)"
"$PROBE" fixed3-atomic "$t" > "$WORK/s17.out"
grep -q 'fixed3: Err.*read-only' "$WORK/s17.out" && ok "S17 read-only target refused" || bad "S17 not refused: $(grep fixed3 "$WORK/s17.out")"
check "S17 content untouched" "$(cat "$t")" "OLD"
check "S17 mode untouched" "$(stat -c %a "$t")" "444"

echo "══ S18: fixed3-atomic with TMPDIR on a DIFFERENT filesystem — must still succeed"
# A TMPDIR-honoring implementation would EXDEV here (S9 proved the boundary
# is real); temp-in-target-parent is immune to TMPDIR entirely.
t="$WORK/s18.txt"; printf 'OLD\n' > "$t"
TMPDIR=/tmp "$PROBE" fixed3-atomic "$t" > "$WORK/s18.out"
grep -q 'fixed3: Ok' "$WORK/s18.out" && ok "S18 write succeeded despite cross-fs TMPDIR" || bad "S18 failed: $(grep fixed3 "$WORK/s18.out")"
check "S18 content replaced" "$(sha256sum < "$t" | cut -d' ' -f1)" "$NEW_SHA"

echo "══ S19: fixed3-atomic on a DIRECTORY target — distinct Err, dir intact"
mkdir -p "$WORK/s19-dir"
"$PROBE" fixed3-atomic "$WORK/s19-dir" > "$WORK/s19.out"
grep -q 'fixed3: Err.*directory' "$WORK/s19.out" && ok "S19 directory target refused with distinct error" || bad "S19: $(grep fixed3 "$WORK/s19.out")"
[ -d "$WORK/s19-dir" ] && ok "S19 directory still present" || bad "S19 directory gone"

echo "══ S20: fixed3-atomic with UNWRITABLE parent dir — Err, existing target intact"
mkdir -p "$WORK/s20-dir"; t="$WORK/s20-dir/f.txt"; printf 'OLD\n' > "$t"; chmod 666 "$t"; chmod 555 "$WORK/s20-dir"
"$PROBE" fixed3-atomic "$t" > "$WORK/s20.out"
grep -q 'fixed3: Err' "$WORK/s20.out" && ok "S20 unwritable parent errors (no in-place fallback)" || bad "S20: $(grep fixed3 "$WORK/s20.out")"
check "S20 target content intact" "$(cat "$t")" "OLD"
chmod 755 "$WORK/s20-dir"

echo
echo "════ ORACLE SUMMARY: $PASS pass, $FAIL fail (work dir kept at $WORK)"
exit $(( FAIL > 0 ))

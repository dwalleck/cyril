#!/bin/sh
# cyril-6bwr census oracle — the module-shape / dead-variant fence.
#
# Usage: no-show-picker-variant.sh [git-rev]
#   (no argument)  census the working tree
#   <git-rev>      census that revision instead — the positive control: run it
#                  against the pre-change base (f9bc81d8 / main) and it must be RED
#                  with six C1 sites (crates/cyril-core/src/commands/mod.rs x5,
#                  crates/cyril/src/app.rs x1) and four C5 sites
#                  (docs/omp-review-command-analysis.md x1,
#                  .agents/summary/{architecture,components,interfaces}.md x3),
#                  proving the census can see the things whose absence it asserts.
#
# Exit 0 = GREEN (every claim holds). Exit 1 = RED; one line per finding, each
# prefixed with the design.md claim id that decided it. Exit 2 = could not run:
# a missing surface or a failing grep is an error, never a GREEN (fail closed).
#
# Independent of rustc/clippy on purpose: a producer-less variant of a `pub` enum
# compiles without any warning, so the compiler cannot be the fence for C1.
set -u
cd "$(git rev-parse --show-toplevel)" || exit 2
rev="${1:-}"
fail=0

# Surfaces. Every one must exist in the censused tree, or the run is exit 2.
mod=crates/cyril-core/src/commands/mod.rs
docs="docs CLAUDE.md CONTEXT.md .agents/summary"

# require <path>... — abort (exit 2) unless every surface exists in the tree.
require() {
  for p in "$@"; do
    if [ -n "$rev" ]; then
      git cat-file -e "$rev:$p" 2>/dev/null || { echo "ERROR: surface $p is missing at $rev" >&2; exit 2; }
    else
      [ -e "$p" ] || { echo "ERROR: surface $p is missing" >&2; exit 2; }
    fi
  done
}

# census <ERE> <path>... — path-prefixed matching lines from the working tree or
# $rev. grep status 0/1 (matches / no matches) is success; any other status is an
# error and the caller aborts (exit 2) instead of reading "no output" as GREEN.
census() {
  pat="$1"
  shift
  if [ -n "$rev" ]; then
    out=$(git grep -n -E -e "$pat" "$rev" -- "$@")
    st=$?
    [ "$st" -gt 1 ] && { echo "ERROR: git grep exited $st for $rev -- $*" >&2; return 2; }
    printf '%s\n' "$out" | sed "s|^$rev:||"
  else
    out=$(grep -rn -E -e "$pat" "$@")
    st=$?
    [ "$st" -gt 1 ] && { echo "ERROR: grep exited $st for $*" >&2; return 2; }
    printf '%s\n' "$out"
  fi
  return 0
}

require crates "$mod" $docs

# C1 — no source under crates/ names the removed variant or its constructor.
hits=$(census 'ShowPicker|CommandResult::show_picker' crates) || exit 2
if [ -n "$hits" ]; then
  printf '%s\n' "$hits" | sed 's/^/C1 FAIL: /'
  fail=1
fi

# C4 — every variant a "same split as" doc phrase cites in commands/mod.rs is a
# declared variant of that file (four-space-indented `Name {`, `Name,` or `Name(`).
if [ -n "$rev" ]; then
  modsrc=$(git show "$rev:$mod") || { echo "ERROR: cannot read $rev:$mod" >&2; exit 2; }
else
  modsrc=$(cat "$mod") || { echo "ERROR: cannot read $mod" >&2; exit 2; }
fi
cited=$(printf '%s\n' "$modsrc" | grep -o 'split as [^.]*' | grep -o '`[A-Za-z]*`' | tr -d '`' | sort -u)
for name in $cited; do
  if ! printf '%s\n' "$modsrc" | grep -q -E "^    $name( \{|,|\()"; then
    echo "C4 FAIL: doc comment in $mod cites non-variant \`$name\`"
    fail=1
  fi
done

# C5 — no living document names the variant, bare or qualified: docs/, CLAUDE.md,
# CONTEXT.md and the .agents/summary/ surfaces the doc fences treat as live. Dated
# records under docs/plans/ describe the pre-ed13a75c design and are history.
dochits=$(census 'ShowPicker' $docs) || exit 2
dochits=$(printf '%s\n' "$dochits" | grep -v '^docs/plans/')
if [ -n "$dochits" ]; then
  printf '%s\n' "$dochits" | sed 's/^/C5 FAIL: /'
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "cyril-6bwr oracle: GREEN (C1 C4 C5)${rev:+ at $rev}"
fi
exit "$fail"

#!/bin/sh
# cyril-6bwr census oracle — the module-shape / dead-variant fence.
#
# Usage: no-show-picker-variant.sh [git-rev]
#   (no argument)  census the working tree
#   <git-rev>      census that revision instead — the positive control: run it
#                  against the pre-change base (f9bc81d8 / main) and it must be RED
#                  with the three known sites, proving the census can see the thing
#                  whose absence it asserts.
#
# Exit 0 = GREEN (every claim holds). Exit 1 = RED; one line per finding, each
# prefixed with the design.md claim id that decided it. Exit 2 = could not run.
#
# Independent of rustc/clippy on purpose: a producer-less variant of a `pub` enum
# compiles without any warning, so the compiler cannot be the fence for C1.
set -u
cd "$(git rev-parse --show-toplevel)" || exit 2
rev="${1:-}"
fail=0

# census <ERE> <path>... — path-prefixed matching lines from the working tree or $rev.
census() {
  pat="$1"
  shift
  if [ -n "$rev" ]; then
    git grep -n -E -e "$pat" "$rev" -- "$@" 2>/dev/null | sed "s|^$rev:||"
  else
    grep -rn -E -e "$pat" "$@" 2>/dev/null
  fi
}

# C1 — no source under crates/ names the removed variant or its constructor.
hits=$(census 'ShowPicker|CommandResult::show_picker' crates)
if [ -n "$hits" ]; then
  printf '%s\n' "$hits" | sed 's/^/C1 FAIL: /'
  fail=1
fi

# C4 — every variant a "same split as" doc phrase cites in commands/mod.rs is a
# declared variant of that file (four-space-indented `Name {`, `Name,` or `Name(`).
mod=crates/cyril-core/src/commands/mod.rs
if [ -n "$rev" ]; then
  modsrc=$(git show "$rev:$mod" 2>/dev/null) || { echo "C4 ERROR: cannot read $rev:$mod"; exit 2; }
else
  modsrc=$(cat "$mod") || { echo "C4 ERROR: cannot read $mod"; exit 2; }
fi
cited=$(printf '%s\n' "$modsrc" | grep -o 'split as [^.]*' | grep -o '`[A-Za-z]*`' | tr -d '`' | sort -u)
for name in $cited; do
  if ! printf '%s\n' "$modsrc" | grep -q -E "^    $name( \{|,|\()"; then
    echo "C4 FAIL: doc comment in $mod cites non-variant \`$name\`"
    fail=1
  fi
done

# C5 — no living document presents the variant as an existing path. Dated records
# under docs/plans/ describe the pre-ed13a75c design and are history, not docs.
dochits=$(census 'CommandResult(Kind)?::ShowPicker' docs CLAUDE.md CONTEXT.md | grep -v '^docs/plans/')
if [ -n "$dochits" ]; then
  printf '%s\n' "$dochits" | sed 's/^/C5 FAIL: /'
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "cyril-6bwr oracle: GREEN (C1 C4 C5)${rev:+ at $rev}"
fi
exit "$fail"

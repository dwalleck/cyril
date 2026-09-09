#!/usr/bin/env bash
# Named-mutation proofs for cyril-qaq0 (checkpointed-build obligation).
#
# Each mutation is applied to the working tree, the owning fence is run, and the
# original file is restored from a byte-exact backup. A mutation that does NOT
# turn its fence red aborts with a non-zero exit.
#
# Usage: bash .cyril-qaq0/oracles/mutations.sh
set -euo pipefail

cd "$(dirname "$0")/../.."
THEME=crates/cyril-ui/src/theme.rs
STATE=crates/cyril-ui/src/state.rs
RENDER=crates/cyril-ui/src/render.rs
BUILTIN=crates/cyril-core/src/commands/builtin.rs
BACKUP=$(mktemp -d)
FAILURES=0

restore_all() {
  for file in "$THEME" "$STATE" "$RENDER" "$BUILTIN"; do
    if [ -f "$BACKUP/$(basename "$file")" ]; then
      cp "$BACKUP/$(basename "$file")" "$file"
    fi
  done
}
trap 'restore_all; rm -rf "$BACKUP"' EXIT

backup() { cp "$1" "$BACKUP/$(basename "$1")"; }
restore() { cp "$BACKUP/$(basename "$1")" "$1"; }

expect_red() { # name, expected substring, command...
  local name="$1" needle="$2"; shift 2
  local output status
  set +e
  output=$("$@" 2>&1)
  status=$?
  set -e
  if [ "$status" -eq 0 ]; then
    echo "FAIL  $name: fence stayed green under the mutation"
    FAILURES=$((FAILURES + 1))
    return 0
  fi
  if ! printf '%s' "$output" | grep -qF "$needle"; then
    echo "FAIL  $name: red for the wrong reason (expected '$needle')"
    printf '%s\n' "$output" | tail -5
    FAILURES=$((FAILURES + 1))
    return 0
  fi
  echo "RED   $name -> $(printf '%s' "$output" | grep -F "$needle" | head -1 | cut -c1-140)"
}

expect_green() { # name, command...
  local name="$1"; shift
  if ! "$@" >/dev/null 2>&1; then
    echo "FAIL  $name: fence stayed red after restore"
    FAILURES=$((FAILURES + 1))
    return 0
  fi
  echo "GREEN $name after restore"
}

# --- M1: no-color leaks a role color (C6) ----------------------------------
backup "$THEME"
python3 - "$THEME" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
s = s.replace("""pub fn resolve_no_color(id: ThemeId) -> Theme {
    Theme {
        syntax: None,
        ..resolve_with(id, |_| Color::Reset)
    }
}""", """pub fn resolve_no_color(id: ThemeId) -> Theme {
    Theme {
        syntax: None,
        text: SourceColor::truecolor(source(id).text),
        ..resolve_with(id, |_| Color::Reset)
    }
}""", 1)
p.write_text(s)
PY
expect_red M1-no-color-leak "syntax expected Reset" cargo test -p cyril-ui --lib all_scene_theme_mode_combinations_pass
restore "$THEME"

# --- restoration: every fence green again ----------------------------------
expect_green C6-render-matrix cargo test -p cyril-ui --lib all_scene_theme_mode_combinations_pass

if [ "$FAILURES" -ne 0 ]; then
  echo "FAIL  $FAILURES mutation proof(s) did not behave as required"
  exit 1
fi
echo "PASS  all named mutations red, all fences green after restore"

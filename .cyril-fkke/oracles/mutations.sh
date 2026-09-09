#!/usr/bin/env bash
# Named-mutation proofs for cyril-fkke (checkpointed-build obligation).
#
# Each mutation is applied to the working tree, the owning fence is run, and the
# original file is restored from a byte-exact backup. A mutation that does NOT
# turn its fence red aborts with a non-zero exit.
#
# Usage: bash .cyril-fkke/oracles/mutations.sh
set -euo pipefail

cd "$(dirname "$0")/../.."
THEME=crates/cyril-ui/src/theme.rs
HIGHLIGHT=crates/cyril-ui/src/highlight.rs
RENDER=crates/cyril-ui/src/render.rs
CHAT=crates/cyril-ui/src/widgets/chat.rs
MARKDOWN=crates/cyril-ui/src/widgets/markdown.rs
BACKUP=$(mktemp -d)
FAILURES=0

restore_all() {
  for file in "$THEME" "$HIGHLIGHT" "$RENDER" "$CHAT" "$MARKDOWN"; do
    if [ -f "$BACKUP/$(basename "$file")" ]; then
      cp "$BACKUP/$(basename "$file")" "$file"
    fi
  done
}
# Always restore, even when a proof fails or the script is interrupted: a
# mutated working tree must never outlive this run.
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

# --- M1: dim a light-palette primary role (C2 contrast) ---------------------
backup "$THEME"
python3 - "$THEME" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
s = s.replace("accent_tertiary: SourceColor::Rgb(0x0b, 0x5c, 0xad)", "accent_tertiary: SourceColor::Rgb(0xb8, 0xb8, 0xc0)", 1)
p.write_text(s)
PY
expect_red M1-contrast "tier not met" cargo test -p cyril-ui --lib bundled_palette_contrast_contract
restore "$THEME"

# --- M2: push a muted role into a protected ANSI-16 speaker slot (C3) -------
backup "$THEME"
python3 - "$THEME" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
s = s.replace("muted: SourceColor::Rgb(0xa8, 0x99, 0x84)", "muted: SourceColor::Rgb(0x00, 0x00, 0xff)", 1)
p.write_text(s)
PY
expect_red M2-protected-slot "protected speaker slot" cargo test -p cyril-ui --lib muted_family_never_projects_into_protected_slots
restore "$THEME"

# --- M3: rename a syntax component so it no longer exists (C5) --------------
backup "$THEME"
python3 - "$THEME" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
s = s.replace('Self::InspiredGitHub => "InspiredGitHub"', 'Self::InspiredGitHub => "inspired-github"', 1)
p.write_text(s)
PY
expect_red M3-syntax-name "Syntect has no bundled theme named" cargo test -p cyril-ui --lib all_bundled_syntax_themes_exist
restore "$THEME"

# --- M4: make the highlight cache key ignore the theme colours (C7) ---------
# Cyril Dark and Gruvbox Dark share a syntax component, so a key built from
# content + language + syntax alone collides between them.
backup "$HIGHLIGHT"
python3 - "$HIGHLIGHT" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
start = s.index("    for color in [\n        theme.canvas,")
tail = "] {\n        color.hash(&mut hasher);\n    }\n"
end = s.index(tail, start) + len(tail)
p.write_text(s[:start] + s[end:])
PY
expect_red M4-cache-key "cache key collision" cargo test -p cyril-ui --lib cache_key_distinguishes_bundled_palettes
restore "$HIGHLIGHT"

# --- M5: drop the canvas painting entirely (C6) -----------------------------
# Painting unconditionally is NOT a valid mutation: `Reset` projects to
# `Reset`, so the observable is unchanged (verified 2026-09-09). Removing the
# paint is the bug class the fence exists to catch.
backup "$RENDER"
python3 - "$RENDER" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
start = s.index("    if theme.canvas != Color::Reset {")
tail = "set_style(area, Style::default().bg(theme.canvas));\n    }\n"
end = s.index(tail, start) + len(tail)
p.write_text(s[:start] + s[end:])
PY
expect_red M5-canvas-paint "no cell carried the palette canvas background" cargo test -p cyril-ui --lib light_palette_paints_canvas_background
restore "$RENDER"

# --- M7: drop role colors from the markdown cache key (C7) -----------------
backup "$MARKDOWN"
python3 - "$MARKDOWN" <<'PY'
import re, sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
start = s.index("    for color in [\n        theme.canvas,")
end = s.index("        color.hash(&mut hasher);\n    }\n", start) + len("        color.hash(&mut hasher);\n    }\n")
p.write_text(s[:start] + s[end:])
PY
expect_red M7-markdown-key "markdown cache key collision" cargo test -p cyril-ui --lib markdown_cache_key_distinguishes_bundled_palettes
restore "$MARKDOWN"

# --- M6: leak the palette registry into a widget (C8) ----------------------
backup "$CHAT"
python3 - "$CHAT" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
p.write_text("use crate::theme::ThemeId;\n" + s)
PY
expect_red M6-shape-widget "widget references 'ThemeId'" python3 .cyril-fkke/oracles/module_shape.py
restore "$CHAT"

# --- restoration: every fence green again ----------------------------------
expect_green C2-contrast cargo test -p cyril-ui --lib bundled_palette_contrast_contract
expect_green C3-protected-slot cargo test -p cyril-ui --lib muted_family_never_projects_into_protected_slots
expect_green C5-syntax cargo test -p cyril-ui --lib all_bundled_syntax_themes_exist
expect_green C7-cache-key cargo test -p cyril-ui --lib cache_key_distinguishes_bundled_palettes
expect_green C7-markdown-key cargo test -p cyril-ui --lib markdown_cache_key_distinguishes_bundled_palettes
expect_green C6-canvas-gate cargo test -p cyril-ui --lib reset_canvas_leaves_terminal_background_untouched
expect_green C8-shape python3 .cyril-fkke/oracles/module_shape.py

if [ "$FAILURES" -ne 0 ]; then
  echo "FAIL  $FAILURES mutation proof(s) did not behave as required"
  exit 1
fi
echo "PASS  all named mutations red, all fences green after restore"

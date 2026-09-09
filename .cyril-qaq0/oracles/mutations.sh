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
APP=crates/cyril/src/app.rs
CONFIG=crates/cyril-core/src/types/config.rs
BACKUP=$(mktemp -d)
FAILURES=0

restore_all() {
  for file in "$THEME" "$STATE" "$RENDER" "$BUILTIN" "$APP" "$CONFIG"; do
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

# --- M2: lenient theme-id parser (C1) --------------------------------------
backup "$THEME"
python3 - "$THEME" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
s = s.replace("""pub fn parse_theme_id(value: &str) -> Option<ThemeId> {
    BUNDLED_APPEARANCES
        .iter()
        .find(|(_, config_id, _)| *config_id == value)
        .map(|(theme, _, _)| *theme)
}""", """pub fn parse_theme_id(value: &str) -> Option<ThemeId> {
    Some(
        BUNDLED_APPEARANCES
            .iter()
            .find(|(_, config_id, _)| *config_id == value)
            .map(|(theme, _, _)| *theme)
            .unwrap_or(ThemeId::CyrilDark),
    )
}""", 1)
p.write_text(s)
PY
expect_red M2-lenient-theme-id "must be rejected" cargo test -p cyril-ui --lib parse_theme_id_covers_exactly_the_bundled_ids
restore "$THEME"

# --- M3: NO_COLOR checked before the explicit value (C3) -------------------
backup "$THEME"
python3 - "$THEME" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
anchor = "pub fn detect_color_mode(request: ColorModeRequest, environment: &ColorEnvironment) -> ColorMode {"
leak = anchor + """
    if environment
        .no_color
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        return ColorMode::None;
    }"""
assert anchor in s, "detect_color_mode signature not found"
s = s.replace(anchor, leak, 1)
p.write_text(s)
PY
expect_red M3-precedence-order "resolved to the wrong mode" cargo test -p cyril-ui --lib detection_precedence_matches_every_table_row
restore "$THEME"

# --- M4: startup drops the unknown-value diagnostic (C8) -------------------
backup "$APP"
python3 - "$APP" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
anchor = "        for diagnostic in &appearance.diagnostics {"
assert anchor in s, "diagnostic loop not found"
s = s.replace(anchor, "        for diagnostic in appearance.diagnostics.iter().take(0) {", 1)
p.write_text(s)
PY
expect_red M4-dropped-diagnostic "left == right" cargo test -p cyril --bin cyril unknown_theme_value_reports_one_visible_message
restore "$APP"

# --- M5: field-skipping deserializer swallows a wrong type (C11) -----------
backup "$CONFIG"
python3 - "$CONFIG" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
helper = """fn lenient_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Ok(
        Option::<toml::Value>::deserialize(deserializer)
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(str::to_owned)),
    )
}

"""
anchor = "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(default)]\npub struct UiConfig {"
assert anchor in s, "UiConfig declaration not found"
s = s.replace(anchor, helper + anchor, 1)
s = s.replace("""    #[serde(default)]
    pub theme: Option<String>,""", """    #[serde(default, deserialize_with = "lenient_string")]
    pub theme: Option<String>,""", 1)
p.write_text(s)
PY
expect_red M5-field-skipping-deserializer "rejection must be whole-file" cargo test -p cyril-core --lib wrong_typed_theme_falls_back_to_whole_file_defaults
restore "$CONFIG"

# --- M6: palette catalog leaks into cyril-core (C10) -----------------------
backup "$CONFIG"
python3 - "$CONFIG" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
s = 'pub const PLANTED_THEME: &str = "gruvbox-dark";\n' + s
p.write_text(s)
PY
expect_red M6-catalog-leak "bundled id literal" python3 .cyril-qaq0/oracles/module_shape.py
restore "$CONFIG"

# --- M7: startup default palette changes (C7) ------------------------------
# The design named `migrated_scenes_match_all_pinned_cells` for this mutation,
# but that fence resolves its theme directly (`truecolor_theme()`), so changing
# `UiState::new` cannot move it. Corrected 2026-09-09: the observable fence for
# the startup default is `new_state_uses_cyril_dark_truecolor`; the baseline
# test keeps proving the pixels independently.
backup "$STATE"
python3 - "$STATE" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
anchor = """            theme: resolve(ThemeId::CyrilDark, ColorMode::TrueColor),
            theme_id: ThemeId::CyrilDark,"""
assert anchor in s, "UiState::new default not found"
s = s.replace(anchor, """            theme: resolve(ThemeId::GruvboxDark, ColorMode::TrueColor),
            theme_id: ThemeId::GruvboxDark,""", 1)
p.write_text(s)
PY
expect_red M7-default-palette-changed "left == right" cargo test -p cyril-ui --lib new_state_uses_cyril_dark_truecolor
restore "$STATE"

# --- M8: /theme reaches the bridge (C9) ------------------------------------
backup "$BUILTIN"
python3 - "$BUILTIN" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
anchor = """    async fn execute(
        &self,
        _ctx: &CommandContext<'_>,
        _args: &str,
    ) -> crate::Result<CommandResult> {
        Ok(CommandResult::show_theme_picker())
    }"""
assert anchor in s, "ThemeCommand::execute not found"
leak = """    async fn execute(
        &self,
        ctx: &CommandContext<'_>,
        _args: &str,
    ) -> crate::Result<CommandResult> {
        ctx.bridge
            .send(BridgeCommand::NewSession {
                cwd: ctx.workspace.to_path_buf(),
            })
            .await?;
        Ok(CommandResult::show_theme_picker())
    }"""
s = s.replace(anchor, leak, 1)
p.write_text(s)
PY
expect_red M8-theme-reaches-bridge "/theme must not reach the agent" cargo test -p cyril --bin cyril theme_command_opens_picker_without_bridge_traffic
restore "$BUILTIN"

# --- M9: Esc keeps the preview (C5) ----------------------------------------
backup "$STATE"
python3 - "$STATE" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
anchor = """    pub fn picker_cancel(&mut self) {
        self.picker = None;
        // Esc must leave the committed appearance exactly as it was: dropping
        // the preview is what makes "discard" true rather than merely "close".
        self.theme_preview = None;
    }"""
assert anchor in s, "picker_cancel not found"
s = s.replace(anchor, """    pub fn picker_cancel(&mut self) {
        self.picker = None;
    }""", 1)
p.write_text(s)
PY
expect_red M9-esc-keeps-preview "Esc must restore the committed theme" cargo test -p cyril-ui --lib picker_cancel_discards_the_preview
restore "$STATE"

# --- M10: theme() ignores the preview (C4) ---------------------------------
backup "$STATE"
python3 - "$STATE" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
anchor = """    fn theme(&self) -> Theme {
        self.theme_preview.unwrap_or(self.theme)
    }"""
assert anchor in s, "TuiState::theme not found"
s = s.replace(anchor, """    fn theme(&self) -> Theme {
        self.theme
    }""", 1)
p.write_text(s)
PY
expect_red M10-preview-not-rendered "the filtered selection must preview" cargo test -p cyril-ui --lib theme_preview_drives_the_rendered_theme_until_commit
restore "$STATE"

# --- restoration: every fence green again ----------------------------------
expect_green C6-render-matrix cargo test -p cyril-ui --lib all_scene_theme_mode_combinations_pass
expect_green C1-theme-ids cargo test -p cyril-ui --lib parse_theme_id_covers_exactly_the_bundled_ids
expect_green C3-precedence cargo test -p cyril-ui --lib detection_precedence_matches_every_table_row
expect_green C8-diagnostic cargo test -p cyril --bin cyril unknown_theme_value_reports_one_visible_message
expect_green C11-whole-file cargo test -p cyril-core --lib wrong_typed_theme_falls_back_to_whole_file_defaults
expect_green C10-module-shape python3 .cyril-qaq0/oracles/module_shape.py
expect_green C7-default-palette cargo test -p cyril-ui --lib new_state_uses_cyril_dark_truecolor
expect_green C9-theme-local cargo test -p cyril --bin cyril theme_command_opens_picker_without_bridge_traffic
expect_green C5-esc-discards cargo test -p cyril-ui --lib picker_cancel_discards_the_preview
expect_green C4-preview-rendered cargo test -p cyril-ui --lib theme_preview_drives_the_rendered_theme_until_commit

if [ "$FAILURES" -ne 0 ]; then
  echo "FAIL  $FAILURES mutation proof(s) did not behave as required"
  exit 1
fi
echo "PASS  all named mutations red, all fences green after restore"

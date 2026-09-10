#!/usr/bin/env bash
# Named-mutation proofs for cyril-v19o (checkpointed-build obligation).
#
# Each mutation applies the exact buggy implementation from `design.md`'s Named
# mutation column to the working tree, runs the owning fence, and requires it to
# go RED. The file is then restored from a byte-exact backup and the same fence
# must go GREEN again. A mutation that does not turn its fence red aborts with a
# non-zero exit: a fence seen only green has not demonstrated defect detection.
#
# Mutations are appended per slice as their fences land.
#
# Usage: bash .cyril-v19o/oracles/mutations.sh [name-filter]
set -euo pipefail

cd "$(dirname "$0")/../.."
FILTER="${1:-}"

ADAPTER=crates/cyril-core/src/protocol/convert/kas/powers.rs
TYPE=crates/cyril-core/src/types/power.rs
WIDGET=crates/cyril-ui/src/widgets/powers_panel.rs
STATE=crates/cyril-ui/src/state.rs
APP=crates/cyril/src/app.rs
BUILTIN=crates/cyril-core/src/commands/builtin.rs
ENGINE=crates/cyril-core/src/protocol/engine.rs

BACKUP=$(mktemp -d)
FAILURES=0
MUTATIONS_RUN=0

restore_all() {
  for file in "$ADAPTER" "$TYPE" "$WIDGET" "$STATE" "$APP" "$BUILTIN" "$ENGINE"; do
    if [ -f "$BACKUP/$(echo "$file" | tr / _)" ]; then
      cp "$BACKUP/$(echo "$file" | tr / _)" "$file"
    fi
  done
}
# Always restore, even when a proof fails or the script is interrupted: a
# mutated working tree must never outlive this run.
trap 'restore_all; rm -rf "$BACKUP"' EXIT

backup() {
  local file="$1"
  if [ ! -f "$BACKUP/$(echo "$file" | tr / _)" ]; then
    cp "$file" "$BACKUP/$(echo "$file" | tr / _)"
  fi
}

# apply <file> <python-replacement>  — exact anchor replacement, loud on miss.
apply() {
  local file="$1" from="$2" to="$3"
  backup "$file"
  python3 - "$file" "$from" "$to" <<'PY'
import pathlib, sys
path, old, new = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
text = path.read_text()
if text.count(old) != 1:
    raise SystemExit(f"ANCHOR MISS in {path}: {old[:60]!r} occurs {text.count(old)}x")
path.write_text(text.replace(old, new))
PY
}

# prove <claim> <name> <file> <from> <to> <fence-command...>
prove() {
  local claim="$1" name="$2" file="$3" from="$4" to="$5"
  shift 5
  if [ -n "$FILTER" ] && [[ "$name" != *"$FILTER"* ]]; then
    return 0
  fi
  MUTATIONS_RUN=$((MUTATIONS_RUN + 1))
  apply "$file" "$from" "$to"
  if "$@" >/dev/null 2>&1; then
    echo "FAIL	$claim	$name	fence stayed GREEN under its mutation"
    FAILURES=$((FAILURES + 1))
  else
    echo "PASS	$claim	$name	fence went RED"
  fi
  restore_all
  if "$@" >/dev/null 2>&1; then
    echo "PASS	$claim	$name	fence GREEN after restore"
  else
    echo "FAIL	$claim	$name	fence still RED after restore"
    FAILURES=$((FAILURES + 1))
  fi
}

CORE_TEST=(cargo test -p cyril-core --features kas --lib)

# --- Slice 1: conversion -----------------------------------------------------

prove C1 swap-name-and-display-name "$ADAPTER" \
  '        Self::new(
            wire.name,
            wire.display_name,' \
  '        Self::new(
            wire.display_name.unwrap_or_default(),
            Some(wire.name),' \
  "${CORE_TEST[@]}" powers_frame_maps_every_field_from_the_capture

prove C2 empty-catalog-becomes-a-drop "$ADAPTER" \
  '    match parse(params) {
        Ok(powers) => Ok(Some(Notification::PowersChanged { powers })),' \
  '    match parse(params) {
        Ok(powers) if powers.is_empty() => Ok(None),
        Ok(powers) => Ok(Some(Notification::PowersChanged { powers })),' \
  "${CORE_TEST[@]}" empty_catalog_is_loaded_not_dropped

prove C3 malformed-frame-becomes-an-empty-catalog "$ADAPTER" \
  '        Err(error) => {
            tracing::warn!(
                method,
                field_path = %error.path(),
                error = %error.inner(),
                "malformed powers notification; not converted"
            );
            Ok(None)
        }' \
  '        Err(_error) => Ok(Some(Notification::PowersChanged { powers: Vec::new() })),' \
  "${CORE_TEST[@]}" malformed_powers_frames_drop_and_never_clear

# --- Slice 2: panel view -----------------------------------------------------

UI_TEST=(cargo test -p cyril-ui --lib)

prove C4 title-keeps-empty-display-name "$TYPE" \
  '            display_name: display_name.filter(|value| !value.is_empty()),' \
  '            display_name,' \
  "${CORE_TEST[@]}" title_falls_back_to_id_and_empty_strings_mean_absent

prove C5 no-sort-on-open "$STATE" \
  '        powers.sort_by_cached_key(|power| {
            (power.title().to_ascii_lowercase(), power.name().to_owned())
        });' \
  '        let _ = &mut powers;' \
  "${UI_TEST[@]}" powers_panel_orders_and_replaces

prove C5 refresh-strands-the-viewport "$STATE" \
  '            panel.scroll_offset = scroll.min(panel.powers.len().saturating_sub(1));' \
  '            panel.scroll_offset = scroll;' \
  "${UI_TEST[@]}" powers_panel_orders_and_replaces

prove C6 title-clamp-dropped "$WIDGET" \
  '            format!("  {}", truncate_and_pad(power.title(), text_width)),' \
  '            format!("  {}", power.title()),' \
  "${UI_TEST[@]}" wide_title_clamps_to_the_panel

prove C6 steering-marker-dropped "$WIDGET" \
  '        if power.has_steering_files() {
            meta.push_str(" · steering");
        }' \
  '        let _ = power.has_steering_files();' \
  "${UI_TEST[@]}" layout_matches_the_approved_row_shape

prove C6 empty-catalog-placeholder-dropped "$WIDGET" \
  '    if state.powers.is_empty() {' \
  '    if false {' \
  "${UI_TEST[@]}" empty_catalog_shows_placeholder

# --- Slice 3: command and wiring ---------------------------------------------

CYRIL_TEST=(cargo test -p cyril --features kas)

prove C5 push-opens-the-panel "$APP" \
  '        if let Notification::PowersChanged { ref powers } = notification
            && self.ui_state.refresh_powers_panel(powers.clone())
        {
            self.redraw_needed = true;
        }' \
  '        if let Notification::PowersChanged { ref powers } = notification {
            self.ui_state.show_powers_panel(powers.clone());
            self.redraw_needed = true;
        }' \
  "${CYRIL_TEST[@]}" powers_push_updates_without_opening_and_command_opens

prove C5 no-catalog-answer-dropped "$BUILTIN" \
  '            None => Ok(CommandResult::system_message(
                "No powers reported yet — start a KAS session first.".to_string(),
            )),' \
  '            None => Ok(CommandResult::dispatched()),' \
  "${CORE_TEST[@]}" powers_without_catalog_reports_and_with_catalog_opens

# --- Slice 4: the census and the transport -----------------------------------

prove C7 an-unusable-powers-method-cannot-be-called "$STATE" \
  '    pub fn has_powers_panel(&self) -> bool {' \
  '    /// A refresh affordance — exactly what this ticket forbids.
    pub fn refresh_powers_hack(&self) -> bool {
        let _method = "_kiro/powers/refresh";
        true
    }

    pub fn has_powers_panel(&self) -> bool {' \
  cargo test -p cyril --test powers_source_fence

# The transport fence's own claim: the frame must survive the real SDK2 path
# INTO the converter. Dropping the engine arm leaves the listener green (the
# frame is simply never recognized), which is the failure this proves.
prove C5 push-dropped-at-the-engine "$ENGINE" \
  '                if let Some(powers) = convert::kas::powers::to_notification(method, params)? {
                    return Ok(Some(powers));
                }' \
  '' \
  cargo test -p cyril-core --all-features --lib powers_push_survives_the_transport

if [ "$MUTATIONS_RUN" -eq 0 ]; then
  echo "FAIL	-	$FILTER	no mutation matched the filter"
  exit 1
fi
if [ "$FAILURES" -ne 0 ]; then
  echo "$FAILURES mutation proof(s) failed"
  exit 1
fi
echo "all $MUTATIONS_RUN named mutations proved red/green"

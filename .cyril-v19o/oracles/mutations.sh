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
TRAITS=crates/cyril-ui/src/traits.rs
APP=crates/cyril/src/app.rs
BUILTIN=crates/cyril-core/src/commands/builtin.rs
COMMANDS=crates/cyril-core/src/commands/mod.rs
ENGINE=crates/cyril-core/src/protocol/engine.rs
RENDER=crates/cyril-ui/src/render.rs
# The census's own read path is a fence target (round-3 `crlf-read-path-stops-
# normalizing`), so the fence file must be restored like production code.
FENCE=crates/cyril/tests/powers_source_fence.rs

BACKUP=$(mktemp -d)
FAILURES=0
MUTATIONS_RUN=0

restore_all() {
  for file in "$ADAPTER" "$TYPE" "$WIDGET" "$STATE" "$TRAITS" "$APP" "$BUILTIN" "$COMMANDS" "$ENGINE" "$RENDER" "$FENCE"; do
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
  '            display_name: display_name.filter(|value| !value.trim().is_empty()),' \
  '            display_name,' \
  "${CORE_TEST[@]}" title_falls_back_to_id_and_empty_strings_mean_absent

prove C4 whitespace-only-is-not-absent "$TYPE" \
  '            display_name: display_name.filter(|value| !value.trim().is_empty()),' \
  '            display_name: display_name.filter(|value| !value.is_empty()),' \
  "${CORE_TEST[@]}" whitespace_only_optionals_are_absent_and_blank_servers_are_dropped

prove C5 no-sort-on-open "$STATE" \
  '        ordered.sort_by_cached_key(|power| {
            (power.title().to_ascii_lowercase(), power.name().to_owned())
        });' \
  '        let _ = &mut ordered;' \
  "${UI_TEST[@]}" powers_panel_orders_and_replaces

prove C5 id-tie-break-dropped "$STATE" \
  '        ordered.sort_by_cached_key(|power| {
            (power.title().to_ascii_lowercase(), power.name().to_owned())
        });' \
  '        ordered.sort_by_cached_key(|power| power.title().to_ascii_lowercase());' \
  "${UI_TEST[@]}" powers_panel_orders_and_replaces

prove C5 refresh-strands-the-viewport "$STATE" \
  '        let scroll = panel
            .scroll_offset
            .min(self.max_powers_scroll(ordered.len()));' \
  '        let scroll = panel.scroll_offset;' \
  "${UI_TEST[@]}" powers_panel_orders_and_replaces

prove C5 scroll-clamp-assumes-the-max-window "$STATE" \
  '        len.saturating_sub(crate::render::powers_window(self, len).max(1))' \
  '        len.saturating_sub(crate::traits::MAX_VISIBLE_POWERS)' \
  "${UI_TEST[@]}" squeezed_viewport_reaches_the_last_power

prove C5 scroll-up-ignores-the-current-window "$STATE" \
  '            panel.scroll_offset = panel.scroll_offset.min(max).saturating_sub(powers);' \
  '            panel.scroll_offset = panel.scroll_offset.saturating_sub(powers);' \
  "${UI_TEST[@]}" scroll_up_moves_after_the_window_grows

prove C5 powers-window-ignores-the-placed-popup "$RENDER" \
  '    crate::widgets::powers_panel::placement(len, area, input_top)
        .map_or(0, |(_popup, window)| window)' \
  '    crate::traits::MAX_VISIBLE_POWERS' \
  "${UI_TEST[@]}" squeezed_viewport_reaches_the_last_power

prove C5 identical-push-reports-a-change "$STATE" \
  '        if panel.powers == ordered {
            return false;
        }' \
  '' \
  "${UI_TEST[@]}" powers_panel_orders_and_replaces

prove C5 hooks-identical-push-reports-a-change "$STATE" \
  '        if panel.hooks == ordered {
            return false;
        }' \
  '' \
  "${UI_TEST[@]}" refresh_replaces_contents_and_clamps_scroll

prove C6 title-clamp-dropped "$WIDGET" \
  '            format!("  {}", truncate_and_pad(power.title(), text_width)),' \
  '            format!("  {}", power.title()),' \
  "${UI_TEST[@]}" wide_title_clamps_to_the_panel

prove C6 steering-marker-dropped "$WIDGET" \
  '        let steering = if power.has_steering_files() && inner_width > steering_width() {
            STEERING_TOKEN
        } else {
            ""
        };' \
  '        let steering = "";' \
  "${UI_TEST[@]}" layout_matches_the_approved_row_shape

prove C6 steering-token-unbudgeted "$WIDGET" \
  '            truncate(
                &meta,
                inner_width.saturating_sub(UnicodeWidthStr::width(steering))
            )' \
  '            truncate(&meta, inner_width)' \
  "${UI_TEST[@]}" steering_marker_survives_a_truncated_meta_line

prove C6 viewport-scroll-not-clamped "$WIDGET" \
  '    let first_visible = state
        .scroll_offset
        .min(state.powers.len().saturating_sub(window));' \
  '    let first_visible = state.scroll_offset;' \
  "${UI_TEST[@]}" viewport_window_clamps_scroll

prove C6 empty-catalog-placeholder-dropped "$WIDGET" \
  '    if state.powers.is_empty() {' \
  '    if false {' \
  "${UI_TEST[@]}" empty_catalog_shows_placeholder

# --- Slice 3: command and wiring ---------------------------------------------

CYRIL_TEST=(cargo test -p cyril --features kas)

prove C5 push-opens-the-panel "$APP" \
  '        if let Notification::PowersChanged { ref powers } = notification
            && self.ui_state.refresh_powers_panel(powers)
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
                "No powers reported yet — powers come from the KAS engine, so this build needs \
                 --features kas and the session needs --agent-engine kas."
                    .to_string(),
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

# --- Slice 5: the PR-review fixes (PR122 verification tiers 1-4) --------------
#
# Every fence added or strengthened for the review gets its own named mutation.

# #12: a blank identifier is not an identifier. Without the `nonempty` guard an
# empty `name` renders as a blank BOLD row sorted above every real power.
prove C5 blank-name-renders-a-row "$ADAPTER" \
  '    #[serde(deserialize_with = "identified_name")]
    name: String,' \
  '    name: String,' \
  "${CORE_TEST[@]}" malformed_powers_frames_drop_and_never_clear

# #4: the census control is anchored to the CONVERTER module, not "somewhere in
# the tree" — the old control was satisfied by a doc comment and two test
# doubles. Assembling the method name from pieces (what a textual guard cannot
# see) must red it.
prove C7 control-anchored-to-the-converter "$ADAPTER" \
  'pub(crate) const METHOD: &str = "kiro/powers/items_changed";' \
  'pub(crate) const METHOD: &str = concat!("kiro/powers/", "items_changed");' \
  cargo test -p cyril --test powers_source_fence no_production_source_names_an_unusable_powers_method

# #4/#7: and a production mention of the unadvertised pull method must red the
# violator scan, not just the control.
prove C7 a-pull-method-in-production-cannot-ship "$ADAPTER" \
  'pub(crate) const METHOD: &str = "kiro/powers/items_changed";' \
  'pub(crate) const METHOD: &str = "kiro/powers/items_changed";
/// The unadvertised pull method this census exists to forbid.
pub(crate) const PULL_METHOD: &str = "_kiro/powers/list";' \
  cargo test -p cyril --test powers_source_fence no_production_source_names_an_unusable_powers_method

# #2: `/help` prints the registry's eager snapshot, so a name pushed after the
# snapshot is invisible.
prove C5 help-snapshot-misses-late-commands "$COMMANDS" \
  '        names.push("powers");
        registry.register(Arc::new(builtin::HelpCommand::new(&names)));' \
  '        registry.register(Arc::new(builtin::HelpCommand::new(&names)));
        names.push("powers");' \
  "${CORE_TEST[@]}" help_lists_every_registered_command

# #3/#20: the keyboard follows the topmost LAYER. Dropping the `.rev()` makes
# the predicate report the bottom-most open overlay instead.
prove C8 topmost-overlay-ignores-layer-priority "$STATE" \
  '            .rev()
            .find(|overlay| self.is_overlay_open(*overlay))' \
  '            .find(|overlay| self.is_overlay_open(*overlay))' \
  "${UI_TEST[@]}" topmost_overlay_orders_the_stack

# #3/#20: and the paint order is the key order reversed. Flipping the constant
# paints the approval prompt under the panel that clears its rect.
prove C8 paint-order-inverted "$TRAITS" \
  '    pub const ALL: [Overlay; 6] = [
        Overlay::Usage,
        Overlay::Code,
        Overlay::Powers,
        Overlay::Hooks,
        Overlay::Picker,
        Overlay::Approval,
    ];' \
  '    pub const ALL: [Overlay; 6] = [
        Overlay::Approval,
        Overlay::Picker,
        Overlay::Hooks,
        Overlay::Powers,
        Overlay::Code,
        Overlay::Usage,
    ];' \
  "${UI_TEST[@]}" topmost_overlay_paints_last

# #13: one predicate, three guards. The paste guard knew only about `/usage`.
prove C8 paste-guard-knows-only-usage "$APP" \
  '                if !self.ui_state.has_modal_overlay() {
                    self.ui_state.insert_text(&text);' \
  '                if !self.ui_state.has_usage_panel() {
                    self.ui_state.insert_text(&text);' \
  "${CYRIL_TEST[@]}" paste_mouse_and_voice_respect_every_overlay

# ── Round 3 (review of head 92cf3cde) ────────────────────────────────────────

# R1 / round-3 finding 2: the frame geometry comes from the terminal the frame is
# drawn into. Dropping the sync sends `/powers` back to `UiState::new`'s 80x24
# default, whose five-power window cannot reach the tail of a nine-power catalog
# on the 18-row terminal the fence draws into.
prove C8 draw-path-does-not-sync-the-frame-geometry "$APP" \
  '        self.ui_state.set_terminal_size(size.width, size.height);' \
  '' \
  "${CYRIL_TEST[@]}" drawing_syncs_the_frame_geometry_the_scroll_bound_reads

# R2 / round-3 finding 3: the CRLF fence judges the RAW CRLF text. Neutering the
# read path's normalization is what the fence's own assertion covers.
prove C7 crlf-read-path-stops-normalizing crates/cyril/tests/powers_source_fence.rs \
  'raw.replace("\r\n", "\n")' \
  'raw' \
  cargo test -p cyril --test powers_source_fence powers_census_is_line_ending_agnostic

# R3 / round-3 finding 4: the shape oracle's protected-parent net is live for
# `app.rs`. A new `App` field is the ledger's own stated falsifier
# (`design.md` → C8) and must fail the gate — it did not while `production_text`
# cut the file at line 133.
prove C8 module-shape-net-blind-to-a-new-App-field "$APP" \
  '    workflow_tracker: WorkflowTracker,' \
  '    workflow_tracker: WorkflowTracker,
    /// A field the ledger forbids: no powers responsibility may land on App.
    powers_cache: Vec<String>,' \
  python3 .cyril-v19o/oracles/module_shape.py

# R3: and the net reaches production lines outside the allowed regions, at any
# indentation — a line of new behavior in a function the ledger does not name is
# reported with that function.
prove C8 module-shape-net-blind-outside-the-allowed-regions "$APP" \
  '    fn toggle_voice(&mut self) {' \
  '    fn toggle_voice(&mut self) {
        let _probe = self.ui_state.powers_panel().is_some();' \
  python3 .cyril-v19o/oracles/module_shape.py

# R4 / round-3 finding 5: the guard's user-visible half. Dropping the notice
# leaves the user with no account of why the dictation vanished, and the buffer
# assertion cannot see that (the input is equally empty either way).
prove C8 voice-discard-not-announced "$APP" \
  '                    self.ui_state.add_system_message(
                        "Discarded a finished dictation: a panel is open (Esc closes it)."
                            .to_string(),
                    );' \
  '' \
  "${CYRIL_TEST[@]}" paste_mouse_and_voice_respect_every_overlay

# R4: and the notice is tied to the DROP, not to the transcript: emitting it on
# both paths keeps the drop assertion satisfied (one notice) and is caught only
# by the positive control's count.
prove C8 voice-notice-emitted-on-both-paths "$APP" \
  '                } else {
                    self.ui_state.insert_text(&text);
                }
            }
            VoiceEvent::Error(msg) => {' \
  '                } else {
                    self.ui_state.insert_text(&text);
                    self.ui_state.add_system_message(
                        "Discarded a finished dictation: a panel is open (Esc closes it)."
                            .to_string(),
                    );
                }
            }
            VoiceEvent::Error(msg) => {' \
  "${CYRIL_TEST[@]}" paste_mouse_and_voice_respect_every_overlay

# R5 / round-3 finding 5, the log half: a dropped paste that leaves no record
# was the finding itself.
prove C8 paste-discard-not-logged "$APP" \
  '                } else {
                    tracing::warn!(
                        chars = text.chars().count(),
                        "dropping a paste: a modal overlay owns the input"
                    );
                }' \
  '                }' \
  "${CYRIL_TEST[@]}" overlay_guards_log_what_they_drop

prove C8 voice-discard-not-logged "$APP" \
  '                    tracing::warn!(
                        chars = text.chars().count(),
                        "dropping a voice transcript: a modal overlay owns the input"
                    );
' \
  '' \
  "${CYRIL_TEST[@]}" overlay_guards_log_what_they_drop

# R5: and the log is tied to the drop, not to the event — logging on both paths
# satisfies the drop assertion and is caught by the control's absence check.
prove C8 paste-log-fires-unconditionally "$APP" \
  '                if !self.ui_state.has_modal_overlay() {
                    self.ui_state.insert_text(&text);
                    self.redraw_needed = true;' \
  '                tracing::warn!(
                    chars = text.chars().count(),
                    "dropping a paste: a modal overlay owns the input"
                );
                if !self.ui_state.has_modal_overlay() {
                    self.ui_state.insert_text(&text);
                    self.redraw_needed = true;' \
  "${CYRIL_TEST[@]}" overlay_guards_log_what_they_drop

if [ "$MUTATIONS_RUN" -eq 0 ]; then
  echo "FAIL	-	$FILTER	no mutation matched the filter"
  exit 1
fi
if [ "$FAILURES" -ne 0 ]; then
  echo "$FAILURES mutation proof(s) failed"
  exit 1
fi
echo "all $MUTATIONS_RUN named mutations proved red/green"

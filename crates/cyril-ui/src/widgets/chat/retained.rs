//! Transcript selection, presentation invalidation and fixed-gutter geometry.

use std::collections::HashSet;
use std::ops::Range;

use ratatui::prelude::*;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

use crate::message_history::{HistoryRevision, LiveRevision, MessageHistory, MessagePosition};
use crate::theme::Theme;
use crate::traits::{Activity, TuiState};
#[cfg(test)]
use crate::widgets::chat_layout;
use crate::widgets::chat_layout::{BlockLocation, Layout, PreparedBlock};

use super::presentation::{Preparation, Presentation, Sources};

const LIVE_LABEL: usize = 0;
const LIVE_TEXT: usize = 1;
const LIVE_THOUGHT: usize = 2;
const ACTIVITY: usize = 3;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WorkCounts {
    pub prepared_messages: usize,
    pub prepared_live: usize,
    pub painted_rows: usize,
    pub resolved_runs: usize,
    pub index_steps: usize,
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
struct PreparationCounts {
    prepared_messages: usize,
    prepared_live: usize,
}

#[cfg(test)]
thread_local! {
    static WORK: std::cell::Cell<PreparationCounts> =
        const { std::cell::Cell::new(PreparationCounts {
            prepared_messages: 0, prepared_live: 0,
        }) };
}

#[cfg(test)]
pub(crate) fn work_counts() -> WorkCounts {
    let prepared = WORK.with(std::cell::Cell::get);
    let physical = chat_layout::work_counts();
    WorkCounts {
        prepared_messages: prepared.prepared_messages,
        prepared_live: prepared.prepared_live,
        painted_rows: physical.painted_rows,
        resolved_runs: physical.resolved_runs,
        index_steps: physical.index_steps,
    }
}

#[cfg(test)]
fn count(update: impl FnOnce(&mut PreparationCounts)) {
    WORK.with(|work| {
        let mut value = work.get();
        update(&mut value);
        work.set(value);
    });
}

#[derive(Default)]
pub(crate) struct ChatRenderState {
    geometry: Option<(u16, Theme)>,
    revision: HistoryRevision,
    retained: Option<Range<MessagePosition>>,
    layout: Layout<Presentation>,
    child_name: Option<String>,
    live_key: Option<u64>,
    live_nonempty: bool,
    thought_key: Option<u64>,
    activity_key: Option<(bool, usize, u64)>,
}

impl ChatRenderState {
    pub(crate) fn invalidate(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn layout_metrics(&self) -> crate::render::LayoutMetrics {
        let mut metrics = crate::render::LayoutMetrics {
            messages: 0,
            rows: 0,
            owned_text_bytes: self.child_name.as_ref().map_or(0, String::len),
            ..crate::render::LayoutMetrics::default()
        };
        // Metrics are explicitly outside timed drawing. Deduplicate shared
        // Markdown allocations here; no pointer or registry survives this call.
        let mut shared = HashSet::new();
        let physical = self.layout.metrics(|presentation| {
            let (sources, owned) = presentation.metrics();
            metrics.source_slots += sources;
            metrics.owned_text_bytes += owned;
            for lines in presentation.shared_markdown() {
                if shared.insert(std::sync::Arc::as_ptr(lines).cast::<Line<'static>>()) {
                    metrics.owned_text_bytes += lines
                        .iter()
                        .flat_map(|line| &line.spans)
                        .map(|span| match &span.content {
                            std::borrow::Cow::Owned(text) => text.len(),
                            std::borrow::Cow::Borrowed(_) => 0,
                        })
                        .sum::<usize>();
                }
            }
        });
        metrics.messages = physical.blocks;
        metrics.rows = physical.rows;
        metrics.runs = physical.runs;
        metrics
    }

    fn synchronize(&mut self, history: &MessageHistory, width: u16, theme: &Theme) {
        let changes = history.changed_since(self.revision);
        let reset = changes.reset || self.geometry != Some((width, *theme));
        if reset {
            self.invalidate();
            self.geometry = Some((width, *theme));
            for position in changes.retained.start.0..changes.retained.end.0 {
                let block = prepare_message(history, MessagePosition(position), width, theme);
                self.layout.push(block);
            }
        } else if let Some(previous) = &self.retained {
            let removed = changes
                .retained
                .start
                .0
                .saturating_sub(previous.start.0)
                .min(self.layout.len());
            self.layout.discard_prefix(removed);
            // Latest-edit order is not transcript order. Append chronological slots
            // separately, then replace only pre-existing changed positions.
            for position in previous.end.0.max(changes.retained.start.0)..changes.retained.end.0 {
                let block = prepare_message(history, MessagePosition(position), width, theme);
                self.layout.push(block);
            }
            for position in changes.changed {
                if position.0 < previous.end.0 && position.0 >= changes.retained.start.0 {
                    let index = position.0 - changes.retained.start.0;
                    let block = prepare_message(history, position, width, theme);
                    self.layout.replace(index, block);
                }
            }
        }
        self.retained = Some(changes.retained);
        self.revision = history.revision();
    }

    fn synchronize_live(
        &mut self,
        revision: LiveRevision,
        text: Option<&str>,
        thought: Option<&str>,
        name: Option<&str>,
        width: u16,
        theme: &Theme,
    ) {
        let name_changed = self.child_name.as_deref() != name;
        if name_changed {
            let mut header = Preparation::default();
            if let Some(name) = name {
                header.child_header(name, theme);
            }
            self.layout
                .replace_leading(header.finish(Sources::default(), width));
            self.child_name = name.map(str::to_string);
        }
        let text_changed = self.live_key != Some(revision.text);
        let changed_text = if text_changed {
            let Some(text) = text else {
                panic!("changed live revision supplies its source");
            };
            self.live_nonempty = !text.is_empty();
            Some(text)
        } else {
            None
        };
        if text_changed || name_changed {
            let mut label = Preparation::default();
            if self.live_nonempty {
                label.live_label(name, theme);
            }
            self.layout
                .replace_trailing(LIVE_LABEL, label.finish(Sources::default(), width));
        }
        if let Some(text) = changed_text {
            #[cfg(test)]
            count(|work| work.prepared_live += 1);
            let mut live = Preparation::default();
            if !text.is_empty() {
                live.markdown(text, width, theme);
            }
            self.layout
                .replace_trailing(LIVE_TEXT, live.finish(Sources::default(), width));
            self.live_key = Some(revision.text);
        }
        if self.thought_key != Some(revision.thought) {
            #[cfg(test)]
            count(|work| work.prepared_live += 1);
            let mut live = Preparation::default();
            if name.is_none()
                && let Some(thought) = thought
            {
                live.thought(thought, true, theme);
            }
            self.layout.replace_trailing(
                LIVE_THOUGHT,
                live.finish(
                    Sources {
                        thought,
                        ..Sources::default()
                    },
                    width,
                ),
            );
            self.thought_key = Some(revision.thought);
        }
    }

    fn synchronize_activity(&mut self, key: Option<(bool, usize, u64)>, width: u16, theme: &Theme) {
        if self.activity_key != key {
            let mut activity = Preparation::default();
            activity.activity(key, theme);
            self.layout
                .replace_trailing(ACTIVITY, activity.finish(Sources::default(), width));
            self.activity_key = key;
        }
    }
}

fn prepare_message(
    history: &MessageHistory,
    position: MessagePosition,
    width: u16,
    theme: &Theme,
) -> PreparedBlock<Presentation> {
    #[cfg(test)]
    count(|work| work.prepared_messages += 1);
    let message = match history.get(position) {
        Some(message) => message.0,
        None => panic!("changed message remains retained"),
    };
    let mut preparation = Preparation::default();
    preparation.message(message, width, theme);
    preparation.blank();
    preparation.finish(Sources::message(message), width)
}

pub(super) fn activity_key(state: &dyn TuiState) -> Option<(bool, usize, u64)> {
    let running = match state.activity() {
        Activity::Sending | Activity::Waiting => false,
        Activity::ToolRunning => true,
        Activity::Streaming | Activity::Idle | Activity::Ready => return None,
    };
    let elapsed = state.activity_elapsed();
    let seconds = elapsed.map(|elapsed| elapsed.as_secs()).unwrap_or(0);
    let spinner = elapsed
        .map(|elapsed| {
            (elapsed.as_millis() / crate::spinner::SPINNER_FRAME_MS) as usize
                % crate::spinner::SPINNER_CHARS.len()
        })
        .unwrap_or(0);
    Some((running, spinner, seconds))
}

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    state: &dyn TuiState,
    theme: &Theme,
    retained: &mut ChatRenderState,
) {
    // Reserve the indicator column even while following or before overflow.
    // Scroll state must never change the retained content's wrapping width.
    let content = Rect {
        width: area.width - u16::from(area.width > 1),
        ..area
    };
    let child = state.subagent_ui().focused_stream();
    let (history, revision, thought) = if let Some(child) = child {
        (child.history(), child.live_revision(), None)
    } else {
        (
            state.history(),
            state.live_revision(),
            state.streaming_thought(),
        )
    };
    let name = child.map(|_| {
        let focused = state.subagent_ui().focused_session_id();
        focused
            .and_then(|id| state.subagent_tracker().get(id))
            .map(|info| info.session_name())
            .or_else(|| focused.map(|id| id.as_str()))
            .unwrap_or("subagent")
    });
    retained.synchronize(history, content.width, theme);
    // UiState's accessor filters withheld receipt tails. Consult it only after
    // the revision/identity guard, not on unchanged spinner/input redraws.
    let text = if retained.live_key != Some(revision.text) {
        Some(if let Some(child) = child {
            child.streaming_text()
        } else {
            state.streaming_text()
        })
    } else {
        None
    };
    retained.synchronize_live(revision, text, thought, name, content.width, theme);
    retained.synchronize_activity(
        if child.is_some() {
            None
        } else {
            activity_key(state)
        },
        content.width,
        theme,
    );
    if content.is_empty() {
        return;
    }
    let total = retained.layout.total_rows();
    let max_scroll = total.saturating_sub(usize::from(content.height));
    let start = if child.is_some() {
        max_scroll
    } else {
        state
            .chat_scroll_back()
            .map_or(max_scroll, |back| max_scroll.saturating_sub(back))
    };
    retained.layout.paint(
        frame.buffer_mut(),
        content,
        start,
        |location, presentation| {
            // Acquire the authoritative context once per intersecting block,
            // before resolving only that block's visible source runs.
            let sources = match location {
                BlockLocation::Indexed(index) => {
                    let first = match &retained.retained {
                        Some(range) => range.start.0,
                        None => panic!("prepared history range"),
                    };
                    let message = match history.get(MessagePosition(first + index)) {
                        Some(message) => message.0,
                        None => panic!("prepared message remains retained"),
                    };
                    Sources::message(message)
                }
                BlockLocation::Trailing(LIVE_TEXT | LIVE_THOUGHT) => Sources {
                    thought,
                    ..Sources::default()
                },
                BlockLocation::Leading | BlockLocation::Trailing(_) => Sources::default(),
            };
            move |slot| presentation.source_text(slot, sources)
        },
    );
    if area.width > 1 && total > usize::from(content.height) {
        let mut scroll = ScrollbarState::new(total).position(start);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            Rect::new(content.right(), content.y, 1, content.height),
            &mut scroll,
        );
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;

    use crate::state::UiState;
    use crate::traits::ChatMessage;

    use super::*;

    fn first_rows(state: &ChatRenderState) -> Option<*const chat_layout::wrap::WrappedRow> {
        (state.layout.len() > 0).then(|| {
            state
                .layout
                .block(BlockLocation::Indexed(0))
                .rows()
                .as_ptr()
        })
    }

    #[test]
    fn zero_and_one_column_use_full_content_and_one_to_two_keeps_preparation() {
        let theme = crate::traits::test_support::marker_theme();
        let mut source = UiState::new(8);
        source.add_system_message("ABCD".into());
        let mut retained = ChatRenderState::default();
        let mut terminal = Terminal::new(TestBackend::new(8, 8)).expect("test terminal");
        let symbols = ratatui::symbols::scrollbar::DOUBLE_VERTICAL;
        for width in [0, 1, 2, 1, 0] {
            let content_width = if width > 1 { width - 1 } else { width };
            let previous_geometry = retained.geometry;
            let previous_rows = first_rows(&retained);
            let before = work_counts();
            let markdown = crate::widgets::markdown::work_counts();
            let wraps = crate::widgets::chat::wrap_count();
            terminal
                .draw(|frame| {
                    render(
                        frame,
                        Rect::new(2, 2, width, 3),
                        &source,
                        &theme,
                        &mut retained,
                    );
                })
                .expect("narrow retained draw");
            assert_eq!(retained.geometry, Some((content_width, theme)));
            if previous_geometry == retained.geometry {
                assert_eq!(work_counts().prepared_messages, before.prepared_messages);
                assert_eq!(work_counts().prepared_live, before.prepared_live);
                assert_eq!(crate::widgets::markdown::work_counts(), markdown);
                assert_eq!(crate::widgets::chat::wrap_count(), wraps);
                assert_eq!(first_rows(&retained), previous_rows);
            }
            let buffer = terminal.backend().buffer();
            if width == 0 {
                assert_eq!(work_counts().painted_rows, before.painted_rows);
                assert!(buffer.content.iter().all(|cell| cell.symbol() == " "));
                continue;
            }
            assert_eq!(retained.layout.total_rows(), 5);
            assert_eq!(work_counts().painted_rows - before.painted_rows, 3);
            for (y, expected) in [(2, "C"), (3, "D")] {
                let cell = &buffer[(2, y)];
                assert_eq!(cell.symbol(), expected, "width {width} source column");
                assert_eq!(cell.fg, theme.system);
                assert!(cell.modifier.contains(Modifier::ITALIC));
            }
            for y in 0..8 {
                for x in 0..8 {
                    if x == 2 && [2, 3].contains(&y) {
                        continue;
                    }
                    let cell = buffer[(x, y)].symbol();
                    if width == 2 && x == 3 && (2..5).contains(&y) {
                        if y == 2 {
                            assert_eq!(cell, symbols.begin);
                        } else if y == 4 {
                            assert_eq!(cell, symbols.end);
                        } else {
                            assert!([symbols.track, symbols.thumb].contains(&cell));
                        }
                    } else {
                        assert_eq!(cell, " ", "paint escaped content/gutter at ({x}, {y})");
                    }
                }
            }
        }
    }

    #[test]
    fn revision_order_edits_append_and_prune_preserve_untouched_preparation() {
        let theme = crate::traits::test_support::marker_theme();
        let mut history = MessageHistory::from(vec![
            ChatMessage::system("first".into()),
            ChatMessage::system("untouched".into()),
            ChatMessage::system("last".into()),
        ]);
        let mut state = ChatRenderState::default();
        state.synchronize(&history, 30, &theme);
        let untouched_rows = state
            .layout
            .block(BlockLocation::Indexed(1))
            .rows()
            .as_ptr();
        let before = work_counts();
        let start = history.retained_range().start.0;
        assert!(
            history.edit(MessagePosition(start + 2), |message| *message =
                ChatMessage::system("last edited".into()))
        );
        assert!(history.edit(MessagePosition(start), |message| *message =
            ChatMessage::system("first edited".into())));
        assert!(
            history.edit(MessagePosition(start + 2), |message| *message =
                ChatMessage::system("last coalesced".into()))
        );
        history.push(ChatMessage::system("appended".into()));
        state.synchronize(&history, 30, &theme);
        assert_eq!(
            work_counts().prepared_messages - before.prepared_messages,
            3
        );
        assert_eq!(
            state
                .layout
                .block(BlockLocation::Indexed(1))
                .rows()
                .as_ptr(),
            untouched_rows
        );
        for (index, expected) in ["first edited", "untouched", "last coalesced", "appended"]
            .into_iter()
            .enumerate()
        {
            let message = history
                .get(MessagePosition(start + index))
                .expect("retained")
                .0;
            assert_eq!(
                state
                    .layout
                    .block(BlockLocation::Indexed(index))
                    .payload()
                    .source_text(0, Sources::message(message)),
                expected
            );
        }
        let before = work_counts();
        history.drop_prefix(1);
        state.synchronize(&history, 30, &theme);
        assert_eq!(work_counts().prepared_messages, before.prepared_messages);
        assert_eq!(
            state
                .layout
                .block(BlockLocation::Indexed(0))
                .rows()
                .as_ptr(),
            untouched_rows
        );
        assert_eq!(state.layout.len(), 3);
    }

    #[test]
    fn prune_can_overtake_the_entire_previous_prepared_range_between_frames() {
        let theme = crate::traits::test_support::marker_theme();
        let mut history = MessageHistory::from(vec![ChatMessage::system("old".into())]);
        let mut state = ChatRenderState::default();
        state.synchronize(&history, 30, &theme);
        for index in 0..12 {
            history.push(ChatMessage::system(format!("new {index}")));
        }
        history.drop_prefix(12);
        let before = work_counts();
        state.synchronize(&history, 30, &theme);
        assert_eq!(state.layout.len(), 1);
        assert_eq!(
            work_counts().prepared_messages - before.prepared_messages,
            1
        );
        let message = history
            .get(history.retained_range().start)
            .expect("last retained")
            .0;
        assert_eq!(
            state
                .layout
                .block(BlockLocation::Indexed(0))
                .payload()
                .source_text(0, Sources::message(message)),
            "new 11"
        );
    }

    #[test]
    fn child_name_change_prepares_header_and_label_without_reading_live_source() {
        let theme = crate::traits::test_support::marker_theme();
        let revision = LiveRevision {
            text: 1,
            thought: 0,
        };
        let mut state = ChatRenderState::default();
        state.synchronize_live(
            revision,
            Some("**retained Markdown**"),
            None,
            Some("alpha"),
            30,
            &theme,
        );
        let live_rows = state
            .layout
            .block(BlockLocation::Trailing(LIVE_TEXT))
            .rows()
            .as_ptr();
        let before = crate::widgets::markdown::work_counts();
        let work = work_counts();
        // No live source is supplied on the unchanged revision, even though the
        // generated child name/header changes.
        state.synchronize_live(revision, None, None, Some("beta"), 30, &theme);
        assert_eq!(crate::widgets::markdown::work_counts(), before);
        assert_eq!(work_counts().prepared_live, work.prepared_live);
        assert_eq!(
            state
                .layout
                .block(BlockLocation::Trailing(LIVE_TEXT))
                .rows()
                .as_ptr(),
            live_rows
        );
        assert_eq!(
            state
                .layout
                .block(BlockLocation::Leading)
                .payload()
                .source_text(0, Sources::default()),
            "─── beta "
        );
        assert_eq!(
            state
                .layout
                .block(BlockLocation::Trailing(LIVE_LABEL))
                .payload()
                .source_text(0, Sources::default()),
            "beta:"
        );
    }

    #[test]
    fn prefix_compaction_moves_metadata_without_rewrapping_retained_rows() {
        let theme = crate::traits::test_support::marker_theme();
        let mut history = MessageHistory::from(
            (0..100)
                .map(|index| ChatMessage::system(format!("row {index}")))
                .collect::<Vec<_>>(),
        );
        let mut state = ChatRenderState::default();
        state.synchronize(&history, 30, &theme);
        let retained_rows = state
            .layout
            .block(BlockLocation::Indexed(80))
            .rows()
            .as_ptr();
        let before = work_counts();
        let wraps = crate::widgets::chat::wrap_count();
        history.drop_prefix(80);
        state.synchronize(&history, 30, &theme);
        assert_eq!(work_counts().prepared_messages, before.prepared_messages);
        assert_eq!(crate::widgets::chat::wrap_count(), wraps);
        assert_eq!(
            state
                .layout
                .block(BlockLocation::Indexed(0))
                .rows()
                .as_ptr(),
            retained_rows
        );
        assert_eq!(state.layout.len(), 20);
        assert_eq!(state.layout.total_rows(), 40);
    }
}

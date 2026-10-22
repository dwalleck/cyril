use ratatui::buffer::CellWidth;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_segmentation::UnicodeSegmentation;

use crate::text;
use crate::theme::Theme;
use crate::traits::{ApprovalPhase, ApprovalState, PreviewBody, TuiState};

/// Render the permission approval overlay.
///
/// `input_top` is the absolute row of the input box's top border; the popup
/// is placed by [`super::modal::place`] so it never covers the input
/// (cyril-a14l C7) and windows its selection when clamped (C8).
pub fn render(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    match &state.phase {
        ApprovalPhase::SelectOption => {
            render_option_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::SelectTrust { .. } => {
            render_trust_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::EnterReason { .. } => {
            render_reason_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::SelectScope { .. } => {
            render_scope_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::EnterFolder { .. } => {
            render_folder_phase(frame, area, input_top, state, session_attribution, theme)
        }
    }
}

fn attribution_title<'a>(
    base: &'a str,
    session_attribution: Option<&str>,
    color: Color,
) -> Span<'a> {
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    match session_attribution {
        Some(session) => Span::styled(format!("{base}— {session} "), style),
        None => Span::styled(base, style),
    }
}

/// Center-scroll window start: keeps `selected` near the middle, clamped so
/// the window never starts before 0 or runs past the end. Mirrors the
/// suggestions widget's windowing so selection behavior feels uniform.
fn window_start(selected: usize, total: usize, visible: usize) -> usize {
    if total > visible && visible > 0 {
        selected.saturating_sub(visible / 2).min(total - visible)
    } else {
        0
    }
}

/// Desired widths of the option and reject-with-reason popups.
const OPTION_POPUP_WIDTH: u16 = 60;
const REASON_POPUP_WIDTH: u16 = 64;

/// Indent before each path row of the preview.
const PATH_INDENT: &str = "  ";
const ELLIPSIS: &str = "…";
const PATH_SEPARATORS: [&str; 2] = ["/", "\\"];

/// Fitted rows retain their visibility decision in the compact inline layout,
/// too. Literal filename text is never used as omission metadata.
struct FittedPath {
    rows: Vec<String>,
    target_shown: bool,
}

struct ElidedPath {
    text: String,
    elided: bool,
    tail_graphemes: usize,
}

/// The approval preview: the target path above the body built by
/// [`preview_lines`].
///
/// The path is the most important fact on a write approval, so it is laid
/// out first and never clipped at the border (cyril-rcub). When the popup has
/// rows to spare, the path wraps across as many rows as it needs and is
/// readable in full. When the popup is clamped and gives the path fewer rows
/// than that, the path is middle-elided into those rows, always keeping the
/// file name and as much leading context as fits. Wrapping alone would not
/// be enough, because clamping drops preview rows from the bottom, which is
/// where a wrapped path's file name sits.
struct Preview<'a> {
    path: Option<&'a str>,
    /// Columns available to a path row (popup inner width minus the indent).
    path_width: usize,
    /// Rows the whole path needs when wrapped.
    path_rows: usize,
    /// The chosen path fit is shared by allocation, painting and the guard.
    fitted_path: FittedPath,
    body: PreviewContent<'a>,
    body_width: usize,
    scrollable: bool,
    scroll_offset: usize,
}

impl<'a> Preview<'a> {
    fn new(state: &'a ApprovalState, theme: &Theme, area: Rect, desired_width: u16) -> Self {
        let path = state.tool_call.primary_path();
        // Keep the actual width for the confirm guard. Layout still needs
        // at least one column, even when the border/indent clips every glyph.
        let path_width = usize::from(super::modal::popup_width(area, desired_width))
            .saturating_sub(2 + text::width(PATH_INDENT));
        let path_rows = path.map_or(0, |path| wrapped_row_count(path, path_width.max(1)));
        let text_width = usize::from(super::modal::popup_width(area, desired_width))
            .saturating_sub(2 + text::width(TEXT_INDENT));
        let body = preview_lines(state, theme, text_width.max(1));
        let scrollable = matches!(body, PreviewContent::Shell(_))
            || state
                .workflow_plan
                .as_ref()
                .is_some_and(|plan| !plan.steps.is_empty());
        Self {
            path,
            path_width,
            path_rows,
            fitted_path: FittedPath {
                rows: Vec::new(),
                target_shown: false,
            },
            body,
            body_width: text_width,
            scrollable,
            scroll_offset: state.preview_scroll_offset,
        }
    }

    /// Bounded layout demand, independent of the scrollable body's length.
    fn len(&self) -> usize {
        let body_rows = if self.scrollable {
            self.body.len().min(super::workflow_plan::MAX_PLAN_ROWS)
        } else {
            self.body.len()
        };
        self.path_rows.saturating_add(body_rows)
    }

    fn has_path(&self) -> bool {
        self.path.is_some()
    }

    fn has_command(&self) -> bool {
        matches!(self.body, PreviewContent::Shell(_))
    }

    fn has_context(&self) -> bool {
        self.has_path() || self.has_command()
    }

    fn path_budget(&self, rows: usize) -> usize {
        rows.saturating_sub(usize::from(self.has_command()))
    }

    /// Keep the preferred allocation only when its actual fit identifies
    /// the target. More rows can expose an overwide whole-path prefix, so
    /// otherwise retain the smallest identifying fit, not just its budget.
    fn identifying_path_rows(&mut self, preferred: usize, available: usize) -> Option<usize> {
        let path = self.path?;
        let preferred = self.path_rows.min(preferred);
        self.fitted_path =
            fit_path_with_rows(path, self.path_width.max(1), preferred, self.path_rows);
        if self.path_width == 0 {
            return None;
        }
        if self.fitted_path.target_shown {
            return Some(preferred);
        }
        for rows in 1..=self.path_rows.min(available) {
            if rows == preferred {
                continue; // This exact fit has already failed.
            }
            let fitted = fit_path_with_rows(path, self.path_width, rows, self.path_rows);
            if fitted.target_shown {
                self.fitted_path = fitted;
                return Some(rows);
            }
        }
        // No identifying fit: preserve the preferred insufficient-space
        // rendering and its strict refusal, rather than the last candidate.
        None
    }

    /// A wholly drawn one-grapheme target is valid; an actually elided
    /// target still needs two retained tail graphemes.
    fn path_shown(&self) -> bool {
        !self.has_path() || (self.path_width > 0 && self.fitted_path.target_shown)
    }

    fn command_shown(&self, rows: usize) -> bool {
        let PreviewContent::Shell(shell) = &self.body else {
            return true;
        };
        if self.body_width == 0 {
            return false;
        }
        let window = self.body_window(rows);
        shell.command_shown(&window)
    }

    /// The path fitted into one run of `width` columns, for a row it shares
    /// with an option label. `None` without a path or without a column to
    /// show it in.
    fn inline_path(&self, width: usize) -> Option<FittedPath> {
        let path = self.path?;
        if width == 0 {
            return None;
        }
        Some(fit_path(path, width, 1))
    }

    fn body_window(&self, rows: usize) -> BodyWindow {
        // Use the retained fit, which can be shorter than a fresh fit to the
        // page's full preview capacity.
        let path_rows = self.fitted_path.rows.len();
        BodyWindow::new(
            self.body.len(),
            rows.saturating_sub(path_rows),
            self.scroll_offset,
            self.scrollable,
        )
    }

    /// The preview laid out in at most `rows` rows. The retained path fit
    /// takes its selected share; the body fills whatever is left.
    fn into_lines(self, rows: usize, theme: &Theme) -> Vec<Line<'a>> {
        let window = self.body_window(rows);
        let mut lines: Vec<Line<'a>> = Vec::with_capacity(rows);
        for row in self.fitted_path.rows {
            lines.push(Line::styled(
                format!("{PATH_INDENT}{row}"),
                Style::default().fg(theme.text_secondary),
            ));
        }
        self.body.extend_lines(&mut lines, &window, theme);
        if window.status {
            lines.push(Line::styled(
                format!(
                    "{TEXT_INDENT}↑ {}–{}/{} ↓ PgUp/PgDn",
                    window.start + 1,
                    window.start + window.visible,
                    window.total
                ),
                Style::default().fg(theme.subdued),
            ));
        }
        lines
    }
}

/// One bounded body viewport; the path, choices and editor never scroll.
struct BodyWindow {
    start: usize,
    visible: usize,
    total: usize,
    status: bool,
    border_hint: bool,
}

impl BodyWindow {
    fn new(total: usize, available: usize, offset: usize, scrollable: bool) -> Self {
        let available = if scrollable {
            available.min(super::workflow_plan::MAX_PLAN_ROWS)
        } else {
            available
        };
        let overflow = scrollable && total > available;
        let status = overflow && available >= 2;
        let visible = total.min(available.saturating_sub(usize::from(status)));
        let max_offset = total.saturating_sub(visible);
        Self {
            start: if scrollable && visible > 0 {
                offset.min(max_offset)
            } else {
                0
            },
            visible,
            total,
            status,
            border_hint: overflow && available == 1,
        }
    }
}

enum PreviewContent<'a> {
    Lines(Vec<Line<'a>>),
    Shell(ShellBody<'a>),
}

impl<'a> PreviewContent<'a> {
    fn len(&self) -> usize {
        match self {
            Self::Lines(lines) => lines.len(),
            Self::Shell(shell) => shell.total_rows,
        }
    }

    fn extend_lines(self, lines: &mut Vec<Line<'a>>, window: &BodyWindow, theme: &Theme) {
        match self {
            Self::Lines(body) => {
                lines.extend(body.into_iter().skip(window.start).take(window.visible));
            }
            Self::Shell(shell) => shell.extend_lines(lines, window, theme),
        }
    }
}

/// Count the complete command without allocating hidden rows; only the
/// current viewport is materialized. Raw command/cwd remain untouched.
struct ShellBody<'a> {
    shell: &'a cyril_core::types::ShellCommand,
    width: usize,
    command_rows: usize,
    /// Total meaningful command graphemes, capped at two by the count pass.
    command_glyphs: usize,
    total_rows: usize,
}

impl<'a> ShellBody<'a> {
    fn new(shell: &'a cyril_core::types::ShellCommand, width: usize) -> Self {
        let command = shell_rows(shell.command(), "", width, None, false, |_, _| {});
        let cwd_rows = shell.cwd().map_or(0, |cwd| {
            shell_rows(cwd, "Cwd: ", width, None, false, |_, _| {}).rows
        });
        Self {
            shell,
            width,
            command_rows: command.rows,
            command_glyphs: command.meaningful_graphemes,
            total_rows: command.rows.saturating_add(cwd_rows),
        }
    }

    fn extend_lines(self, lines: &mut Vec<Line<'a>>, window: &BodyWindow, theme: &Theme) {
        let end = window.start.saturating_add(window.visible);
        if window.start < self.command_rows {
            shell_rows(
                self.shell.command(),
                "",
                self.width,
                Some(window.start..end.min(self.command_rows)),
                true,
                |row, _| lines.push(Line::styled(row, Style::default().fg(theme.text_secondary))),
            );
        }
        if end > self.command_rows
            && let Some(cwd) = self.shell.cwd()
        {
            shell_rows(
                cwd,
                "Cwd: ",
                self.width,
                Some(window.start.saturating_sub(self.command_rows)..end - self.command_rows),
                true,
                |row, _| lines.push(Line::styled(row, Style::default().fg(theme.subdued))),
            );
        }
    }

    fn command_shown(&self, window: &BodyWindow) -> bool {
        let end = window
            .start
            .saturating_add(window.visible)
            .min(self.command_rows);
        if window.start >= end || self.command_glyphs == 0 {
            return false;
        }
        let mut visible = 0usize;
        shell_rows(
            self.shell.command(),
            "",
            self.width,
            Some(window.start..end),
            false,
            |_, glyphs| visible = (visible + glyphs).min(2),
        );
        visible >= self.command_glyphs
    }
}

struct ShellRows {
    rows: usize,
    /// Meaningful graphemes visited by a count pass, capped at two.
    meaningful_graphemes: usize,
}

/// Borrow safe graphemes intact, replacing only controls and bidi marks.
/// Splitting unsafe chunks also preserves CRLF as a space followed by LF,
/// rather than replacing the whole raw CRLF grapheme with one space.
fn shell_graphemes(text: &str) -> impl Iterator<Item = &str> {
    text.graphemes(true).flat_map(|grapheme| {
        grapheme
            .split_inclusive(|ch: char| ch.is_control() || super::workflow_plan::is_bidi_format(ch))
            .flat_map(|chunk| {
                let (safe, replacement) = match chunk.chars().next_back() {
                    Some('\n') => (&chunk[..chunk.len() - 1], Some("\n")),
                    Some(ch) if ch.is_control() || super::workflow_plan::is_bidi_format(ch) => {
                        (&chunk[..chunk.len() - ch.len_utf8()], Some(" "))
                    }
                    _ => (chunk, None),
                };
                [(!safe.is_empty()).then_some(safe), replacement]
                    .into_iter()
                    .flatten()
            })
    })
}

/// Sanitize while wrapping, retaining spaces and logical newlines. `None`
/// counts all rows; a range visits only its rows and stops at its end.
/// Materialize strings only for rendering, never for the visibility guard.
/// Nonmaterialized counts exclude whitespace and zero-width graphemes; the
/// viewport callback also excludes clipped graphemes. Totals are capped at two.
fn shell_rows(
    text: &str,
    prefix: &str,
    width: usize,
    range: Option<std::ops::Range<usize>>,
    materialize: bool,
    mut emit: impl FnMut(String, usize),
) -> ShellRows {
    if range.as_ref().is_some_and(std::ops::Range::is_empty) {
        return ShellRows {
            rows: 0,
            meaningful_graphemes: 0,
        };
    }
    let mut row_index = 0usize;
    let mut used = 0usize;
    let mut occupied = false;
    let mut row = String::new();
    let mut glyphs = 0usize;
    let mut meaningful_graphemes = 0usize;
    for grapheme in shell_graphemes(prefix).chain(shell_graphemes(text)) {
        let newline = grapheme == "\n";
        let columns = if newline {
            0
        } else {
            usize::from(grapheme.cell_width())
        };
        if newline || (occupied && used.saturating_add(columns) > width) {
            emit_shell_row(
                &mut row,
                glyphs,
                row_index,
                range.as_ref(),
                materialize,
                &mut emit,
            );
            row_index += 1;
            used = 0;
            occupied = false;
            glyphs = 0;
            if range.as_ref().is_some_and(|range| row_index >= range.end) {
                return ShellRows {
                    rows: row_index,
                    meaningful_graphemes,
                };
            }
            if newline {
                continue;
            }
        }
        let in_range = range
            .as_ref()
            .is_some_and(|range| range.contains(&row_index));
        let needs_glyph_count =
            !materialize && (meaningful_graphemes < 2 || (in_range && glyphs < 2));
        if needs_glyph_count
            && columns > 0
            && grapheme
                .chars()
                .any(|ch| !ch.is_whitespace() && text::char_width(ch) > 0)
        {
            meaningful_graphemes = (meaningful_graphemes + 1).min(2);
            if in_range && used.saturating_add(columns) <= width {
                glyphs = (glyphs + 1).min(2);
            }
        }
        if in_range && materialize {
            if row.is_empty() {
                row.push_str(TEXT_INDENT);
            }
            row.push_str(grapheme);
        }
        used = used.saturating_add(columns);
        occupied = true;
    }
    emit_shell_row(
        &mut row,
        glyphs,
        row_index,
        range.as_ref(),
        materialize,
        &mut emit,
    );
    ShellRows {
        rows: row_index + 1,
        meaningful_graphemes,
    }
}

fn emit_shell_row(
    row: &mut String,
    glyphs: usize,
    index: usize,
    range: Option<&std::ops::Range<usize>>,
    materialize: bool,
    emit: &mut impl FnMut(String, usize),
) {
    if range.is_some_and(|range| range.contains(&index)) {
        if materialize && row.is_empty() {
            row.push_str(TEXT_INDENT);
        }
        emit(std::mem::take(row), glyphs);
    }
}

/// Gap between an option label and a path sharing its row.
const INLINE_PATH_GAP: &str = "  ";

/// The selection marker column of an option row.
fn option_prefix(selected: bool) -> &'static str {
    if selected { "▸ " } else { "  " }
}

/// The base option-page allocation with one separate path row (cyril-rcub).
/// [`OptionPage`] reserves any additional identifying path rows by dropping
/// cosmetic rows before rendering. The selection always gets a row, and
/// at inner height one the path shares it after the label.
/// Calls without a path or command keep their message-first order. Cosmetic message,
/// blanks and separator return only after the identifying context fits;
/// the remaining preview and option window retain their established order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OptionLayout {
    message: bool,
    blank: bool,
    preview_rows: usize,
    separator: bool,
    option_rows: usize,
    /// The path is appended to the lone option row (height 1).
    inline_path: bool,
}

impl OptionLayout {
    fn new(inner: usize, preview_len: usize, has_path: bool) -> Self {
        let none = Self {
            message: false,
            blank: false,
            preview_rows: 0,
            separator: false,
            option_rows: 0,
            inline_path: false,
        };
        match inner {
            0 => none,
            1 => Self {
                option_rows: 1,
                inline_path: has_path,
                ..none
            },
            2 if has_path => Self {
                preview_rows: 1,
                option_rows: 1,
                ..none
            },
            2 => Self {
                message: true,
                option_rows: 1,
                ..none
            },
            3 if has_path => Self {
                message: true,
                preview_rows: 1,
                option_rows: 1,
                ..none
            },
            3 => Self {
                message: true,
                blank: true,
                option_rows: 1,
                ..none
            },
            n => {
                let remaining = n - 2; // message + blank
                let preview_rows = preview_len.min(remaining - 1);
                let separator = preview_rows > 0 && remaining - preview_rows >= 2;
                Self {
                    message: true,
                    blank: true,
                    preview_rows,
                    separator,
                    option_rows: remaining - preview_rows - usize::from(separator),
                    inline_path: false,
                }
            }
        }
    }
}

/// Split `text` into rows of at most `width` display columns. A grapheme
/// never splits: one that would straddle the edge starts the next row,
/// matching ratatui's rendering, including emoji presentation selectors.
///
/// Zero-width graphemes retain their original bytes without occupying a
/// display column.
pub(crate) fn wrap_columns(text: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut used = 0usize;
    for grapheme in text.graphemes(true) {
        // Raw controls stay in the source rows but occupy no painted cells.
        let grapheme_width = if grapheme.contains(char::is_control) {
            0
        } else {
            usize::from(grapheme.cell_width())
        };
        if used.saturating_add(grapheme_width) > width && !row.is_empty() {
            rows.push(std::mem::take(&mut row));
            used = 0;
        }
        row.push_str(grapheme);
        used = used.saturating_add(grapheme_width);
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

/// Lay `path` out in at most `rows` rows of `width` columns. The path is
/// wrapped whole when it fits; otherwise it is middle-elided
/// ([`elide_middle`]) into the space available.
fn fit_path(path: &str, width: usize, rows: usize) -> FittedPath {
    let path_rows = if rows == 0 {
        0
    } else {
        wrapped_row_count(path, width)
    };
    fit_path_with_rows(path, width, rows, path_rows)
}

/// The same fitter with the whole-path row count already known by Preview.
fn fit_path_with_rows(path: &str, width: usize, rows: usize, path_rows: usize) -> FittedPath {
    if rows == 0 {
        return FittedPath {
            rows: Vec::new(),
            target_shown: false,
        };
    }
    if path_rows <= rows {
        let wrapped = wrap_columns(path, width);
        let target_shown =
            path_grapheme_count(path) > 0 && wrapped.iter().all(|row| text::width(row) <= width);
        return FittedPath {
            rows: wrapped,
            target_shown,
        };
    }
    // A wide glyph that straddles a row edge wastes that row's last column,
    // so shrink the column budget until the elided path wraps into `rows`.
    // The loop ends: at a budget of zero the path elides to "", one row.
    let mut budget = width.saturating_mul(rows);
    loop {
        let elision = elide_middle(path, budget);
        let fitted = wrap_columns(&elision.text, width);
        if fitted.len() <= rows || budget == 0 {
            let target_shown = elision.tail_graphemes > usize::from(elision.elided)
                && fitted.iter().all(|row| text::width(row) <= width);
            return FittedPath {
                rows: fitted,
                target_shown,
            };
        }
        budget -= 1;
    }
}

/// Shorten `path` to at most `budget` display columns by replacing its
/// middle with `…`. The kept tail is the file name, plus its parent
/// directory when that fits too. Every remaining column goes to the leading
/// context. If the file name alone is wider than the budget, its end (the
/// extension) is what stays.
fn elide_middle(path: &str, budget: usize) -> ElidedPath {
    if text::width(path) <= budget {
        return ElidedPath {
            text: path.to_owned(),
            elided: false,
            tail_graphemes: path_grapheme_count(path),
        };
    }
    let Some(room) = budget.checked_sub(text::width(ELLIPSIS)) else {
        return ElidedPath {
            text: String::new(),
            elided: true,
            tail_graphemes: 0,
        };
    };
    // Ignore trailing separators when selecting the last two components.
    // Keep only whole-grapheme boundaries and borrow their byte offsets.
    let mut separators = path
        .grapheme_indices(true)
        .rev()
        .skip_while(|(_, grapheme)| PATH_SEPARATORS.contains(grapheme))
        .filter_map(|(offset, grapheme)| PATH_SEPARATORS.contains(&grapheme).then_some(offset));
    let name = separators.next().map_or(path, |offset| &path[offset..]);
    let parent = separators.next().map_or(name, |offset| &path[offset..]);
    let tail = if text::width(parent) <= room {
        parent
    } else {
        name
    };
    let (head, tail) = match room.checked_sub(text::width(tail)) {
        Some(head_room) => (leading_components(path, head_room), tail),
        None => ("", trailing_columns(tail, room)),
    };
    ElidedPath {
        text: format!("{head}{ELLIPSIS}{tail}"),
        elided: true,
        tail_graphemes: path_grapheme_count(tail),
    }
}

fn path_grapheme_count(text: &str) -> usize {
    let span = Span::raw(text);
    span.styled_graphemes(Style::default())
        .filter(|grapheme| grapheme.symbol.cell_width() > 0)
        .take(2)
        .count()
}

/// The longest prefix of `path` that ends just after a separator and is at
/// most `width` display columns wide, so every directory it shows is whole:
/// `/home/dwa…` could be `dwalleck` or `dwalleck-x`, `/home/…` cannot
/// mislead (cyril-rcub review round 2). Empty when not even the first
/// component fits.
fn leading_components(path: &str, width: usize) -> &str {
    let mut used = 0usize;
    let mut cut = 0usize;
    for (offset, grapheme) in path
        .grapheme_indices(true)
        .filter(|(_, grapheme)| !grapheme.contains(char::is_control))
    {
        used = used.saturating_add(usize::from(grapheme.cell_width()));
        if used > width {
            break;
        }
        if PATH_SEPARATORS.contains(&grapheme) {
            cut = offset + grapheme.len();
        }
    }
    &path[..cut]
}

/// `text` fitted to one row of `width` columns: whole when it fits,
/// otherwise cut and ended with `…`, so a clipped row says it was clipped.
fn clip_row(text: &str, width: usize) -> String {
    crate::text::truncate(text, width)
}

/// The longest suffix of `text` at most `width` display columns wide.
fn trailing_columns(text: &str, width: usize) -> &str {
    let mut used = 0usize;
    for (offset, grapheme) in text
        .grapheme_indices(true)
        .rev()
        .filter(|(_, grapheme)| !grapheme.contains(char::is_control))
    {
        used = used.saturating_add(usize::from(grapheme.cell_width()));
        if used > width {
            return &text[offset + grapheme.len()..];
        }
    }
    text
}

/// Max body lines in the approval preview (cyril-j1b3). Five matches the
/// chat widget's tool-output cap; the diff renderer applies its own 20-line
/// cap, so a diff-heavy preview is truncated before this budget matters.
const MAX_PREVIEW_LINES: usize = 5;

/// Indent before each text row of the preview body.
const TEXT_INDENT: &str = "    ";

/// Push at most `max_rows` content rows of wrapped, indented text and an
/// omission marker. Plan fallbacks count the marker inside that same budget;
/// ordinary text keeps its established five content rows plus marker.
/// Wrapping preserves information at the end of a long source line.
///
/// Runs every frame, so rows past the cap are counted, not built: a large
/// raw-input write is not re-wrapped into strings on every redraw.
fn push_bounded_text<'a, 's>(
    lines: &mut Vec<Line<'a>>,
    text_lines: impl Iterator<Item = &'s str>,
    width: usize,
    max_rows: usize,
    marker_in_budget: bool,
    theme: &Theme,
) {
    let mut total = 0usize;
    for line in text_lines {
        if total >= max_rows {
            total = total.saturating_add(wrapped_row_count(line, width));
            continue;
        }
        for row in wrap_columns(line, width) {
            total += 1;
            if total <= max_rows {
                lines.push(Line::styled(
                    format!("{TEXT_INDENT}{row}"),
                    Style::default().fg(theme.subdued),
                ));
            }
        }
    }
    if total > max_rows {
        let displaced = usize::from(marker_in_budget && max_rows > 0);
        lines.truncate(lines.len().saturating_sub(displaced));
        lines.push(Line::styled(
            format!(
                "{TEXT_INDENT}...{} more lines",
                total - max_rows + displaced
            ),
            Style::default().fg(theme.subdued),
        ));
    }
}

/// How many rows [`wrap_columns`] gives `text` at `width`, without building
/// them.
fn wrapped_row_count(text: &str, width: usize) -> usize {
    let mut rows = 1usize;
    let mut used = 0usize;
    let mut row_empty = true;
    for grapheme in text.graphemes(true) {
        let grapheme_width = if grapheme.contains(char::is_control) {
            0
        } else {
            usize::from(grapheme.cell_width())
        };
        if used.saturating_add(grapheme_width) > width && !row_empty {
            rows += 1;
            used = 0;
        }
        row_empty = false;
        used = used.saturating_add(grapheme_width);
    }
    rows
}

/// The line of KAS's plan text that summarises what an edit or a
/// replacement removes, adds and changes (`Diu`: "Compared with the replaced
/// steps: …"). `workflowPlan` does not carry it.
fn plan_comparison(tc: &crate::traits::TrackedToolCall) -> Option<&str> {
    tc.content().iter().find_map(|block| match block {
        cyril_core::types::ToolCallContent::Text(text) => text
            .lines()
            .find(|line| line.starts_with("Compared with the replaced steps:")),
        cyril_core::types::ToolCallContent::Diff { .. } => None,
    })
}

/// Build the approval-preview body from the joined tool-call snapshot. The
/// path rows above it are laid out by [`Preview`] (cyril-rcub).
///
/// A typed workflow plan uses its tree/comparison projection; typed shell
/// input uses a safe, navigable command/cwd body. Otherwise preserve diff →
/// Text content → raw-input text/content precedence and the existing bounds.
/// Missing payloads retain `Preview unavailable`; a title is not payload.
///
/// Runs every frame, so it never logs: a malformed payload's warning is
/// emitted once when the approval is installed (cyril-q0kg).
fn preview_lines<'a>(
    state: &'a ApprovalState,
    theme: &Theme,
    text_width: usize,
) -> PreviewContent<'a> {
    let tc = &state.tool_call;
    // A KAS workflow plan approval (cyril-8wh0) shows the typed plan's step
    // tree in place of KAS's Text rendering of the same plan, plus the one
    // line only the Text has: what an edit removes.
    if let Some(plan) = state
        .workflow_plan
        .as_ref()
        .filter(|plan| !plan.steps.is_empty())
    {
        return PreviewContent::Lines(super::workflow_plan::plan_lines(
            plan,
            plan_comparison(tc),
            text_width,
            theme,
        ));
    }
    // A plan approval without a usable typed plan — sent without its steps
    // (`omitted: "steps"`) or dropped as malformed — falls back to its Text,
    // given the plan's row budget rather than the generic five rows.
    let is_plan_approval = state.workflow_plan.is_some()
        || state.consent.as_ref().and_then(|consent| consent.tool_id()) == Some("update_workflow");
    let text_rows = if is_plan_approval {
        super::workflow_plan::MAX_PLAN_ROWS
    } else {
        MAX_PREVIEW_LINES
    };
    let mut lines: Vec<Line<'a>> = Vec::new();

    match tc.preview_body() {
        PreviewBody::Shell(shell) => {
            return PreviewContent::Shell(ShellBody::new(shell, text_width));
        }
        PreviewBody::Diff => super::chat::render_diff_lines(&mut lines, tc, theme),
        PreviewBody::Content(blocks) => push_bounded_text(
            &mut lines,
            blocks.iter().flat_map(|block| match block {
                cyril_core::types::ToolCallContent::Text(text) => text.lines(),
                // Classification guarantees Text only; a diff is never
                // flattened into text.
                cyril_core::types::ToolCallContent::Diff { .. } => "".lines(),
            }),
            text_width,
            text_rows,
            is_plan_approval,
            theme,
        ),
        PreviewBody::Text(text) => push_bounded_text(
            &mut lines,
            text.lines(),
            text_width,
            text_rows,
            is_plan_approval,
            theme,
        ),
        // Never coerce a non-string payload (cyril-j1b3 spec).
        PreviewBody::Absent | PreviewBody::Malformed { .. } => lines.push(Line::styled(
            "  Preview unavailable",
            Style::default().fg(theme.subdued),
        )),
    }
    PreviewContent::Lines(lines)
}

pub(crate) struct PreviewWindow {
    pub max_offset: usize,
    pub visible_rows: usize,
}

/// Current body navigation bounds from the same page/window the renderer
/// uses. Paths, choices and editors never contribute to the scroll range.
pub(crate) fn preview_window(state: &dyn TuiState) -> Option<PreviewWindow> {
    let approval = state.approval()?;
    let (width, height) = state.terminal_size();
    let area = Rect::new(0, 0, width, height);
    let input_top = crate::render::input_top(area, state);
    let theme = state.theme();
    let (popup, preview, rows) = match &approval.phase {
        ApprovalPhase::SelectOption => {
            let page = OptionPage::new(area, input_top, approval, &theme)?;
            (page.popup, page.preview, page.layout.preview_rows)
        }
        ApprovalPhase::EnterReason { editor, .. } => {
            let page =
                ReasonPage::new(area, input_top, approval, editor.notice().is_some(), &theme)?;
            (page.popup, page.preview, page.preview_rows)
        }
        _ => return None,
    };
    if !preview.scrollable || usize::from(popup.width.saturating_sub(2)) <= text::width(TEXT_INDENT)
    {
        return None;
    }
    let window = preview.body_window(rows);
    (window.visible > 0).then_some(PreviewWindow {
        max_offset: window.total.saturating_sub(window.visible),
        visible_rows: window.visible,
    })
}

/// Whether a confirm key may answer the front approval in the frame the
/// state reports (cyril-vojx): [`confirm_is_on_screen`] for the recorded
/// frame size. `false` without an approval.
///
/// Derived the way [`crate::render::powers_window`] is — from the frame size
/// `UiState` records on every draw and resize, and the frame's own layout —
/// so the key path recomputes the geometry the renderer draws from instead
/// of caching what the last frame drew.
pub(crate) fn confirm_on_screen(state: &dyn TuiState) -> bool {
    let Some(approval) = state.approval() else {
        return false;
    };
    let (width, height) = state.terminal_size();
    let area = Rect::new(0, 0, width, height);
    confirm_is_on_screen(
        area,
        crate::render::input_top(area, state),
        approval,
        &state.theme(),
    )
}

/// Whether a confirm key may answer `state` in a frame of `area` whose input
/// box starts at `input_top` (cyril-vojx).
///
/// A confirm answers the request, so it acts only on what the user can see:
/// the selected option (or, on the reject-with-reason page, the reason
/// editor) must be drawn, and when the call has a target path, the path must
/// be drawn beside it. The answer comes from the same page geometry each
/// phase's renderer draws from, so the guard and the frame cannot disagree.
/// The "Always allow" scope, trust and folder pages never show the path —
/// the operator saw it on the option page that led there — so for them the
/// drawn selection (or folder editor) is enough.
pub(crate) fn confirm_is_on_screen(
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    theme: &Theme,
) -> bool {
    match &state.phase {
        ApprovalPhase::SelectOption => OptionPage::new(area, input_top, state, theme)
            .is_some_and(|page| page.selection_shown_with_path(state.selected)),
        ApprovalPhase::EnterReason { editor, .. } => {
            ReasonPage::new(area, input_top, state, editor.notice().is_some(), theme)
                .is_some_and(|page| page.editor_shown_with_path())
        }
        ApprovalPhase::SelectTrust { .. } => {
            trust_popup(area, input_top, state).is_some_and(|popup| {
                has_inner_row(popup) && state.selected < state.trust_options.len()
            })
        }
        ApprovalPhase::SelectScope { .. } => {
            scope_popup(area, input_top, state).is_some_and(|popup| {
                has_inner_row(popup) && state.selected < state.scope_choices().len()
            })
        }
        ApprovalPhase::EnterFolder { .. } => {
            FolderPage::new(area, input_top, state).is_some_and(|page| page.editor_shown())
        }
    }
}

/// A popup with at least one cell inside its borders. The scope and trust
/// pages give that first row to the selected row's label and never wrap a
/// row, so a nonempty inner area is what makes them actionable.
fn has_inner_row(popup: Rect) -> bool {
    popup.height > 2 && popup.width > 2
}

/// The option page's geometry: the placed popup, the rows [`OptionLayout`]
/// gives each part and the window of options shown. One computation serves
/// the renderer and the confirm guard ([`confirm_is_on_screen`]).
struct OptionPage<'a> {
    popup: Rect,
    preview: Preview<'a>,
    layout: OptionLayout,
    show_feedback_hint: bool,
    /// First option shown.
    start: usize,
    /// Options shown, from `start`.
    visible: usize,
    /// The path fitted after the lone option row's label (height 1).
    inline_path: Option<FittedPath>,
}

impl<'a> OptionPage<'a> {
    /// `None` when no rows above the input can hold the popup.
    fn new(area: Rect, input_top: u16, state: &'a ApprovalState, theme: &Theme) -> Option<Self> {
        // Preview block (cyril-j1b3): the joined tool-call snapshot is
        // rendered above the options so a KAS stub permission request still
        // shows the file path and proposed content. Budget: the path's
        // wrapped rows + up to MAX_PREVIEW_LINES of body + blanks; clamped
        // below when space is tight (the path is then middle-elided,
        // cyril-rcub).
        let mut preview = Preview::new(state, theme, area, OPTION_POPUP_WIDTH);
        let show_feedback_hint = state.can_reject_with_reason();
        // options.len() is a handful of user-facing choices; the sum stays
        // far below u16::MAX, so try_from is infallible and the saturation is
        // defensive, not an error default (same pattern as the picker).
        let desired_height = u16::try_from(
            state
                .options
                .len()
                .saturating_add(preview.len())
                .saturating_add(6)
                .saturating_add(usize::from(show_feedback_hint)),
        )
        .unwrap_or(u16::MAX);
        let popup = super::modal::place(area, input_top, OPTION_POPUP_WIDTH, desired_height)?;

        // Inner rows inside the borders decide what fits, by the priority in
        // [`OptionLayout`]: the selection always, the path beside any option,
        // then message, blanks, the rest of the preview and the other options.
        let inner = usize::from(popup.height.saturating_sub(2));
        let mut layout = OptionLayout::new(inner, preview.len(), preview.has_context());
        let command_rows = usize::from(preview.has_command());
        if let Some(path_rows) = preview.identifying_path_rows(
            preview.path_budget(layout.preview_rows),
            preview.path_budget(inner.saturating_sub(1)),
        ) {
            let context_rows = path_rows.saturating_add(command_rows);
            for chrome in [
                &mut layout.separator,
                &mut layout.blank,
                &mut layout.message,
            ] {
                if layout.preview_rows >= context_rows {
                    break;
                }
                if *chrome {
                    *chrome = false;
                    layout.preview_rows += 1;
                }
            }
        }
        let mut option_rows = layout.option_rows;
        let show_feedback_hint = show_feedback_hint && option_rows > 1;
        if show_feedback_hint {
            option_rows -= 1;
        }

        let visible = state.options.len().min(option_rows);
        let start = window_start(state.selected, state.options.len(), visible);
        // Height 1 shows exactly one option row (the selection, via
        // `window_start`); the path shares it after the label.
        let inline_path = if layout.inline_path && visible == 1 {
            state.options.get(start).and_then(|opt| {
                let label_width = text::width(option_prefix(start == state.selected))
                    .saturating_add(text::width(&opt.label))
                    .saturating_add(text::width(INLINE_PATH_GAP));
                preview.inline_path(
                    usize::from(popup.width.saturating_sub(2)).saturating_sub(label_width),
                )
            })
        } else {
            None
        };
        Some(Self {
            popup,
            preview,
            layout,
            show_feedback_hint,
            start,
            visible,
            inline_path,
        })
    }

    /// Whether option `selected` has a row, with the path on screen beside
    /// it when the call has one: on the preview's first rows (the path is
    /// laid out ahead of the body) or appended to the lone option row.
    fn selection_shown_with_path(&self, selected: usize) -> bool {
        let selection_shown = has_inner_row(self.popup)
            && (self.start..self.start.saturating_add(self.visible)).contains(&selected);
        let path_shown = self.preview.path_shown()
            || self
                .inline_path
                .as_ref()
                .is_some_and(|path| path.target_shown);
        selection_shown && path_shown && self.preview.command_shown(self.layout.preview_rows)
    }
}

fn render_option_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let Some(OptionPage {
        popup: popup_area,
        preview,
        layout,
        show_feedback_hint,
        start,
        visible,
        inline_path,
    }) = OptionPage::new(area, input_top, state, theme)
    else {
        return; // no rows above the input can hold the popup
    };

    super::modal::clear(frame, popup_area);
    let border_hint = preview.body_window(layout.preview_rows).border_hint;

    let mut lines: Vec<Line> = Vec::new();
    if layout.message {
        lines.push(Line::styled(
            &state.message,
            Style::default().fg(theme.emphasis),
        ));
    }
    if layout.blank {
        lines.push(Line::default());
    }
    lines.extend(preview.into_lines(layout.preview_rows, theme));
    if layout.separator {
        lines.push(Line::default());
    }

    for (i, opt) in state.options.iter().enumerate().skip(start).take(visible) {
        let style = if i == state.selected {
            Style::default()
                .bg(theme.selection)
                .fg(theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_secondary)
        };
        let label = format!("{}{}", option_prefix(i == state.selected), opt.label);
        match inline_path.as_ref().and_then(|path| path.rows.first()) {
            Some(path) => lines.push(
                Line::from(vec![
                    Span::raw(label),
                    Span::styled(
                        format!("{INLINE_PATH_GAP}{path}"),
                        Style::default()
                            .fg(theme.text_secondary)
                            .remove_modifier(Modifier::BOLD),
                    ),
                ])
                .style(style),
            ),
            None => lines.push(Line::styled(label, style)),
        }
    }
    if show_feedback_hint {
        lines.push(Line::styled(
            "r  Reject with reason",
            Style::default().fg(theme.accent_quinary),
        ));
    }

    let title = attribution_title(" Permission Required ", session_attribution, theme.emphasis);
    let mut block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.emphasis));
    if border_hint {
        block = block.title_bottom(" PgUp/PgDn preview ");
    }
    let popup = Paragraph::new(lines).block(block);
    frame.render_widget(popup, popup_area);
}

/// The reject-with-reason page's geometry: the placed popup and which rows
/// fit inside it. One computation serves the renderer and the confirm guard
/// ([`confirm_is_on_screen`]).
struct ReasonPage<'a> {
    popup: Rect,
    inner: usize,
    preview: Preview<'a>,
    show_limit: bool,
    show_notice: bool,
    show_editor_label: bool,
    show_tool_title: bool,
    show_selected: bool,
    show_message: bool,
    show_blank: bool,
    preview_rows: usize,
    editor_rows: usize,
}

impl<'a> ReasonPage<'a> {
    /// `None` when no rows above the input can hold the popup.
    fn new(
        area: Rect,
        input_top: u16,
        state: &'a ApprovalState,
        has_notice: bool,
        theme: &Theme,
    ) -> Option<Self> {
        let mut preview = Preview::new(state, theme, area, REASON_POPUP_WIDTH);
        let desired_height = u16::try_from(
            preview
                .len()
                .saturating_add(14)
                .saturating_add(usize::from(has_notice)),
        )
        .unwrap_or(u16::MAX);
        let popup = super::modal::place(area, input_top, REASON_POPUP_WIDTH, desired_height)?;
        let inner = usize::from(popup.height.saturating_sub(2));

        // Keep the editor and identifying path/command context ahead of chrome.
        let command_rows = usize::from(preview.has_command());
        let context_rows = usize::from(preview.has_path()) + command_rows;
        let reserved = context_rows.min(inner.saturating_sub(1));
        let extra = reserved.saturating_sub(1);
        let mut show_limit = inner >= 2 + reserved;
        let mut show_notice = has_notice && inner >= 3 + 2 * reserved;
        let mut show_editor_label = inner >= 4 + extra;
        let mut show_tool_title = inner >= 6 + extra;
        let mut show_selected = inner >= 7 + extra;
        let mut show_message = inner >= 8 + extra;
        let mut show_blank = inner >= 9 + extra;
        let mut fixed = 1usize
            .saturating_add(usize::from(show_editor_label))
            .saturating_add(usize::from(show_limit))
            .saturating_add(usize::from(show_notice))
            .saturating_add(usize::from(show_tool_title))
            .saturating_add(usize::from(show_selected))
            .saturating_add(usize::from(show_message))
            .saturating_add(usize::from(show_blank));
        if let Some(path_rows) = preview.identifying_path_rows(
            preview.path_budget(inner.saturating_sub(fixed)),
            preview.path_budget(inner.saturating_sub(1)),
        ) {
            let context_rows = path_rows.saturating_add(command_rows);
            let mut missing = context_rows.saturating_sub(inner.saturating_sub(fixed));
            for chrome in [
                &mut show_blank,
                &mut show_message,
                &mut show_selected,
                &mut show_tool_title,
                &mut show_notice,
                &mut show_editor_label,
                &mut show_limit,
            ] {
                if missing == 0 {
                    break;
                }
                if *chrome {
                    *chrome = false;
                    fixed -= 1;
                    missing -= 1;
                }
            }
        }
        let preview_rows = preview.len().min(inner.saturating_sub(fixed));
        let editor_rows = inner
            .saturating_sub(fixed.saturating_add(preview_rows))
            .max(1);
        Some(Self {
            popup,
            inner,
            preview,
            show_limit,
            show_notice,
            show_editor_label,
            show_tool_title,
            show_selected,
            show_message,
            show_blank,
            preview_rows,
            editor_rows,
        })
    }

    /// Whether the editor row is drawn, with the path on screen when the
    /// call has one (the path leads the preview rows).
    fn editor_shown_with_path(&self) -> bool {
        has_inner_row(self.popup)
            && self.preview.path_shown()
            && self.preview.command_shown(self.preview_rows)
    }
}

fn render_reason_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let ApprovalPhase::EnterReason {
        chosen_option_id,
        editor,
    } = &state.phase
    else {
        return;
    };
    let selected_label = state
        .options
        .iter()
        .find(|option| option.id == *chosen_option_id)
        .map_or("Reject", |option| option.label.as_str());
    let Some(page) = ReasonPage::new(area, input_top, state, editor.notice().is_some(), theme)
    else {
        return;
    };
    let popup_area = page.popup;

    super::modal::clear(frame, popup_area);
    let inner = page.inner;
    if inner == 0 {
        return;
    }
    let ReasonPage {
        preview,
        show_limit,
        show_notice,
        show_editor_label,
        show_tool_title,
        show_selected,
        show_message,
        show_blank,
        preview_rows,
        editor_rows,
        ..
    } = page;
    let border_hint = preview.body_window(preview_rows).border_hint;
    let content_width = usize::from(popup_area.width.saturating_sub(2)).max(1);
    let editor_lines = super::input::text_cursor_lines(
        editor.text(),
        editor.cursor(),
        content_width,
        editor_rows,
        theme,
    );

    let mut lines: Vec<Line> = Vec::with_capacity(inner);
    if show_message {
        lines.push(Line::styled(
            &state.message,
            Style::default().fg(theme.emphasis),
        ));
    }
    if show_blank {
        lines.push(Line::default());
    }
    lines.extend(preview.into_lines(preview_rows, theme));
    if show_selected {
        lines.push(Line::styled(
            format!("Rejecting: {selected_label}"),
            Style::default().fg(theme.text_secondary),
        ));
    }
    if show_editor_label {
        lines.push(Line::styled(
            "Reason (Enter submits, Ctrl+J adds a line, Esc backs out):",
            Style::default().fg(theme.accent_quinary),
        ));
    }
    lines.extend(editor_lines);
    if show_tool_title {
        lines.push(Line::styled(
            format!("Request: {}", state.tool_call.title()),
            Style::default().fg(theme.text_secondary),
        ));
    }
    if show_limit {
        lines.push(Line::styled(
            format!(
                "Limit: {} Unicode characters",
                crate::feedback_editor::RejectionFeedback::max_scalars()
            ),
            Style::default().fg(theme.subdued),
        ));
    }
    if show_notice && let Some(notice) = editor.notice() {
        lines.push(Line::styled(notice, Style::default().fg(theme.warning)));
    }

    let title = attribution_title(" Reject with reason ", session_attribution, theme.emphasis);
    let mut block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.emphasis));
    if border_hint {
        block = block.title_bottom(" PgUp/PgDn preview ");
    }
    let popup = Paragraph::new(lines).block(block);
    frame.render_widget(popup, popup_area);
}

/// The trust page's placed popup; `None` when no rows above the input can
/// hold it. Shared by the renderer and [`confirm_is_on_screen`].
fn trust_popup(area: Rect, input_top: u16, state: &ApprovalState) -> Option<Rect> {
    // Each trust option: label line + display line + blank = 3 lines, plus
    // header. Trust tiers are a handful, so try_from is infallible; the
    // saturation is defensive, not an error default.
    let desired_height = u16::try_from(
        state
            .trust_options
            .len()
            .saturating_mul(3)
            .saturating_add(4),
    )
    .unwrap_or(u16::MAX);
    super::modal::place(area, input_top, 64, desired_height)
}

fn render_trust_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let Some(popup_area) = trust_popup(area, input_top, state) else {
        return; // no rows above the input can hold the popup
    };

    super::modal::clear(frame, popup_area);

    // With 5+ inner rows the header and separator fit above one full 3-row
    // item; tighter popups drop the header and window items directly (the
    // selected item's label renders first, so it survives any clamp).
    let inner = usize::from(popup_area.height.saturating_sub(2));
    let content_width = usize::from(popup_area.width.saturating_sub(2));
    let (show_header, item_rows) = if inner >= 5 {
        (true, (inner - 2) / 3)
    } else {
        (false, 1)
    };

    let mut lines: Vec<Line> = Vec::new();
    if show_header {
        lines.push(Line::styled(
            "Select trust level:",
            Style::default().fg(theme.accent_quinary),
        ));
        lines.push(Line::default());
    }

    let visible = state.trust_options.len().min(item_rows.max(1));
    let start = window_start(state.selected, state.trust_options.len(), visible);
    for (i, trust) in state
        .trust_options
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
    {
        let style = if i == state.selected {
            Style::default()
                .bg(theme.selection)
                .fg(theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_secondary)
        };
        let prefix = if i == state.selected { "▸ " } else { "  " };
        // Each tier takes exactly the three rows the window reserves: a
        // wrapped label or pattern list would push the selected tier out of
        // the popup while Enter still picked it (cyril-vojx review), so a
        // long row is cut and marked with `…` instead.
        lines.push(Line::styled(
            clip_row(&format!("{prefix}{}", trust.label), content_width),
            style,
        ));
        // Show the display string (pattern preview) dimmed below the label
        let display_style = if i == state.selected {
            Style::default().bg(theme.selection).fg(theme.subdued)
        } else {
            Style::default().fg(theme.subdued)
        };
        lines.push(Line::styled(
            clip_row(&format!("    {}", trust.display), content_width),
            display_style,
        ));
        // Blank separator between options — matches the 3-lines-per-option
        // height reserved above (label + display + blank).
        lines.push(Line::default());
    }

    let title = attribution_title(
        " Always Allow — Choose Scope ",
        session_attribution,
        theme.accent_quinary,
    );
    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );

    frame.render_widget(popup, popup_area);
}

/// Kiro's TUI label truncation (`UM`): more than 50 characters keep the first
/// 47 and an ellipsis.
fn truncate_label(text: &str) -> String {
    if text.chars().count() > 50 {
        format!("{}…", text.chars().take(47).collect::<String>())
    } else {
        text.to_owned()
    }
}

/// The scope page's row text, as Kiro's TUI words it (`isr`, `m2t`).
fn scope_row(
    choice: &cyril_core::always_allow::ScopeChoice,
    scope: cyril_core::always_allow::GrantScope,
    tool: Option<&str>,
) -> (String, String) {
    use cyril_core::always_allow::ScopeChoice;
    let scope_label = scope.label();
    match choice {
        ScopeChoice::Exact(resource) => (
            format!("Trust \"{}\"", truncate_label(resource)),
            format!("exact match · {scope_label}"),
        ),
        ScopeChoice::Pattern(pattern) => (
            format!("Trust \"{pattern}\""),
            format!("pattern · {scope_label}"),
        ),
        ScopeChoice::Folder(offer) => (
            format!("Allow a folder {}…", scope.folder_phrase()),
            format!("{} · {scope_label}", truncate_label(offer.prefill())),
        ),
        ScopeChoice::WholeTool => (
            match tool {
                Some(tool) => format!("Trust entire tool ({tool})"),
                None => "Trust entire tool".to_owned(),
            },
            scope_label.to_owned(),
        ),
    }
}

/// The scope page's `capability → resource` line, when the consent names
/// either.
fn scope_target(state: &ApprovalState) -> Option<String> {
    let consent = state.consent.as_ref();
    let capability = consent.and_then(|c| c.wire_capability());
    let resource = consent
        .and_then(|c| c.resource())
        .filter(|resource| !resource.is_empty());
    match (capability, resource) {
        (Some(capability), Some(resource)) => Some(format!("{capability} → {resource}")),
        (Some(only), None) | (None, Some(only)) => Some(only.to_owned()),
        (None, None) => None,
    }
}

/// The scope page's chrome rows: target + blank (when present), scope line +
/// blank, hint.
fn scope_chrome(has_target: bool) -> usize {
    usize::from(has_target) * 2 + 3
}

/// The scope page's placed popup; `None` when no rows above the input can
/// hold it. Shared by the renderer and [`confirm_is_on_screen`].
fn scope_popup(area: Rect, input_top: u16, state: &ApprovalState) -> Option<Rect> {
    let chrome = scope_chrome(scope_target(state).is_some());
    let desired_height = u16::try_from(
        state
            .scope_choices()
            .len()
            .saturating_mul(2)
            .saturating_add(chrome + 2),
    )
    .unwrap_or(u16::MAX);
    super::modal::place(area, input_top, 72, desired_height)
}

/// KAS "Always allow" scope page (cyril-auxt): `capability → resource`, the
/// scope line (`s` cycles), and one label + description pair per row.
fn render_scope_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let ApprovalPhase::SelectScope { scope, .. } = &state.phase else {
        return;
    };
    let choices = state.scope_choices();
    let target = scope_target(state);
    let tool = state.consent.as_ref().and_then(|c| c.tool_id());
    let chrome = scope_chrome(target.is_some());
    let Some(popup_area) = scope_popup(area, input_top, state) else {
        return;
    };
    super::modal::clear(frame, popup_area);

    // Rows win over chrome when clamped: the selected row's label is what
    // keeps the page actionable.
    let inner = usize::from(popup_area.height.saturating_sub(2));
    let show_chrome = inner >= chrome + 2;
    let row_lines = if show_chrome { inner - chrome } else { inner };
    let visible = choices.len().min((row_lines / 2).max(1));
    let start = window_start(state.selected, choices.len(), visible);

    let mut lines: Vec<Line> = Vec::new();
    if show_chrome {
        if let Some(target) = target {
            lines.push(Line::styled(
                target,
                Style::default().fg(theme.text_secondary),
            ));
            lines.push(Line::default());
        }
        let mut spans = vec![Span::styled(
            "Scope:  ",
            Style::default().fg(theme.text_secondary),
        )];
        for (index, option) in cyril_core::always_allow::GrantScope::ALL.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            if option == scope {
                spans.push(Span::styled(
                    format!("[{}]", option.label()),
                    Style::default()
                        .fg(theme.accent_quinary)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", option.label()),
                    Style::default().fg(theme.text_secondary),
                ));
            }
        }
        lines.push(Line::from(spans));
        lines.push(Line::default());
    }
    for (index, choice) in choices.iter().enumerate().skip(start).take(visible) {
        let (label, description) = scope_row(choice, *scope, tool);
        let selected = index == state.selected;
        let (label_style, description_style) = if selected {
            (
                Style::default()
                    .bg(theme.selection)
                    .fg(theme.text)
                    .add_modifier(Modifier::BOLD),
                Style::default().bg(theme.selection).fg(theme.subdued),
            )
        } else {
            (
                Style::default().fg(theme.text_secondary),
                Style::default().fg(theme.subdued),
            )
        };
        let prefix = if selected { "▸ " } else { "  " };
        lines.push(Line::styled(format!("{prefix}{label}"), label_style));
        lines.push(Line::styled(
            format!("    {description}"),
            description_style,
        ));
    }
    if show_chrome {
        lines.push(Line::styled(
            "↑↓ navigate · Enter select · s cycle scope · Esc back",
            Style::default().fg(theme.subdued),
        ));
    }

    let title = attribution_title(
        " Always Allow — Choose Scope ",
        session_attribution,
        theme.accent_quinary,
    );
    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );
    frame.render_widget(popup, popup_area);
}

/// Desired width of the folder editor popup.
const FOLDER_POPUP_WIDTH: u16 = 84;

/// The folder editor's validation line (Kiro's `h2t` / `vde`) and whether it
/// reports a refused folder. `None` outside the folder phase.
fn folder_status(state: &ApprovalState) -> Option<(String, bool)> {
    let ApprovalPhase::EnterFolder {
        scope,
        offer,
        editor,
        ..
    } = &state.phase
    else {
        return None;
    };
    Some(match offer.validate(editor.text()) {
        Ok(folder) => {
            let reach = match scope {
                cyril_core::always_allow::GrantScope::Session => "for this session only",
                other => other.folder_phrase(),
            };
            (
                format!(
                    "Allows writes in {} and every folder under it, {reach}.",
                    folder.path()
                ),
                false,
            )
        }
        Err(reason) => (reason.to_string(), true),
    })
}

/// The folder editor's geometry: the placed popup, the validation line
/// wrapped to the popup's width, and which rows fit. One computation serves
/// the renderer and [`confirm_is_on_screen`].
///
/// The editor row survives any clamp. The status joins only when all of its
/// rows fit above the editor — a long folder wraps the status, and drawing
/// it anyway would push the editor out of the popup while Enter still
/// granted the folder (cyril-vojx review).
struct FolderPage {
    popup: Rect,
    inner: usize,
    status_rows: Vec<String>,
    show_status: bool,
    show_hint: bool,
}

impl FolderPage {
    /// `None` outside the folder phase or when no rows above the input can
    /// hold the popup.
    fn new(area: Rect, input_top: u16, state: &ApprovalState) -> Option<Self> {
        let (status, _) = folder_status(state)?;
        let content_width = usize::from(super::modal::popup_width(area, FOLDER_POPUP_WIDTH))
            .saturating_sub(2)
            .max(1);
        let status_rows = wrap_columns(&status, content_width);
        // Borders, status, blank, editor, blank, hint: seven rows for a
        // one-row status, one more per extra status row.
        let desired_height = u16::try_from(status_rows.len().saturating_add(6)).unwrap_or(u16::MAX);
        let popup = super::modal::place(area, input_top, FOLDER_POPUP_WIDTH, desired_height)?;
        let inner = usize::from(popup.height.saturating_sub(2));
        let show_status = inner > status_rows.len();
        let show_hint = show_status && inner >= status_rows.len() + 3;
        Some(Self {
            popup,
            inner,
            status_rows,
            show_status,
            show_hint,
        })
    }

    /// Whether the folder editor row is drawn.
    fn editor_shown(&self) -> bool {
        has_inner_row(self.popup)
    }
}

/// KAS "Always allow" folder editor (cyril-auxt): Kiro's live validation line
/// (`h2t` / `vde`) above a one-line editor.
fn render_folder_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let ApprovalPhase::EnterFolder {
        scope,
        editor,
        attempted,
        ..
    } = &state.phase
    else {
        return;
    };
    let Some((_, refused)) = folder_status(state) else {
        return;
    };
    let status_style = Style::default().fg(if refused && *attempted {
        theme.danger
    } else {
        theme.text_secondary
    });

    let Some(FolderPage {
        popup: popup_area,
        inner,
        status_rows,
        show_status,
        show_hint,
    }) = FolderPage::new(area, input_top, state)
    else {
        return;
    };
    super::modal::clear(frame, popup_area);
    if inner == 0 {
        return;
    }
    let content_width = usize::from(popup_area.width.saturating_sub(2)).max(1);
    let mut lines: Vec<Line> = Vec::with_capacity(inner);
    if show_status {
        lines.extend(
            status_rows
                .into_iter()
                .map(|row| Line::styled(row, status_style)),
        );
    }
    if show_hint {
        lines.push(Line::default());
    }
    lines.extend(super::input::text_cursor_lines(
        editor.text(),
        editor.cursor(),
        content_width,
        1,
        theme,
    ));
    if show_hint {
        lines.push(Line::default());
        lines.push(Line::styled(
            "Enter to allow this folder · Esc back",
            Style::default().fg(theme.subdued),
        ));
    }

    let heading = format!(" Allow a folder {} ", scope.folder_phrase());
    let title = attribution_title(&heading, session_attribution, theme.accent_quinary);
    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );
    frame.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::UiState;
    use crate::traits::TuiState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use unicode_width::UnicodeWidthStr;

    fn option(id: &str, label: &str) -> cyril_core::types::PermissionOption {
        cyril_core::types::PermissionOption {
            id: cyril_core::types::PermissionOptionId::new(id),
            label: label.into(),
            kind: if id == "reject" {
                cyril_core::types::PermissionOptionKind::RejectOnce
            } else {
                cyril_core::types::PermissionOptionKind::AllowOnce
            },
            is_destructive: false,
        }
    }

    fn approval_tool_call() -> cyril_core::types::ToolCall {
        cyril_core::types::ToolCall::new(
            cyril_core::types::ToolCallId::new("tc_1"),
            "echo hello".into(),
            cyril_core::types::ToolKind::Execute,
            cyril_core::types::ToolCallStatus::Pending,
            None,
        )
    }

    fn approval_with(
        options: Vec<cyril_core::types::PermissionOption>,
        trust_options: Vec<cyril_core::types::TrustOption>,
        selected: usize,
        phase: ApprovalPhase,
    ) -> ApprovalState {
        ApprovalState {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: crate::traits::TrackedToolCall::new(approval_tool_call()),
            message: "Allow execution?".into(),
            options,
            trust_options,
            selected,
            preview_scroll_offset: 0,
            can_reject_with_reason: false,
            phase,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
            workflow_plan: None,
        }
    }

    fn approval_ui(options: Vec<cyril_core::types::PermissionOption>) -> UiState {
        let mut ui = UiState::new(500);
        ui.show_approval(cyril_core::types::PermissionRequest {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: approval_tool_call(),
            message: "Allow execution?".into(),
            options,
            trust_options: vec![],
            can_reject_with_reason: true,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
            workflow_plan: None,
        });
        ui
    }

    fn feedback_ui() -> UiState {
        let mut ui = approval_ui(vec![option("reject", "Reject")]);
        assert!(ui.approval_begin_feedback());
        ui
    }

    fn theme() -> Theme {
        crate::theme::resolve(
            crate::theme::ThemeId::CyrilDark,
            crate::theme::ColorMode::TrueColor,
        )
    }

    /// Flatten a `TestBackend` buffer into one string per row, joined by `\n`.
    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = *buffer.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_at(
        state: &ApprovalState,
        width: u16,
        height: u16,
        input_top: u16,
    ) -> Terminal<TestBackend> {
        render_at_with_attribution(state, width, height, input_top, None)
    }

    fn render_ui(ui: &UiState, width: u16, height: u16, input_top: u16) -> Terminal<TestBackend> {
        let Some(approval) = ui.approval() else {
            panic!("expected front approval");
        };
        render_at(approval, width, height, input_top)
    }

    fn render_at_with_attribution(
        state: &ApprovalState,
        width: u16,
        height: u16,
        input_top: u16,
        session_attribution: Option<&str>,
    ) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    input_top,
                    state,
                    session_attribution,
                    &theme(),
                );
            })
            .expect("draw");
        terminal
    }

    #[test]
    fn rejection_feedback_hint_only_for_eligible_option() {
        let mut ui = approval_ui(vec![
            option("allow", "Allow Once"),
            option("reject", "Reject"),
        ]);
        let before = render_ui(&ui, 80, 24, 24);
        assert!(!buffer_text(&before).contains("r  Reject with reason"));
        ui.approval_select_next();
        let terminal = render_ui(&ui, 80, 24, 24);
        assert!(buffer_text(&terminal).contains("r  Reject with reason"));
    }

    #[test]
    fn rejection_feedback_renders_limit_and_request_title() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste(&"界".repeat(4096));
        ui.approval_feedback_paste("x");
        let terminal = render_ui(&ui, 90, 30, 30);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("Request: echo hello"),
            "title missing:\n{text}"
        );
        assert!(
            text.contains("Limit: 4096 Unicode characters"),
            "limit missing:\n{text}"
        );
        assert!(
            text.contains("Reason limit reached"),
            "limit notice missing:\n{text}"
        );
    }
    #[test]
    fn rejection_feedback_cursor_visible_in_ordinary_popup() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste("ordinary cursor");
        let terminal = render_ui(&ui, 90, 30, 30);
        let text = buffer_text(&terminal);
        assert!(text.contains("ordinary cursor█"), "cursor missing:\n{text}");
    }

    #[test]
    fn rejection_feedback_cursor_visible_when_popup_is_cramped() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste("cramped cursor");
        let terminal = render_ui(&ui, 30, 16, 6);
        let text = buffer_text(&terminal);
        assert!(text.contains("cramped cursor█"), "cursor missing:\n{text}");
        for row in text.lines().skip(6) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{text}");
        }
    }

    #[test]
    fn rejection_feedback_overflow_keeps_limit_cursor_and_warning_visible() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste(&"界".repeat(4096));
        let before = render_ui(&ui, 30, 16, 6);
        let warning = theme().warning;
        let warning_cells = |terminal: &Terminal<TestBackend>| {
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .filter(|cell| cell.fg == warning && !cell.symbol().trim().is_empty())
                .count()
        };
        assert_eq!(
            warning_cells(&before),
            0,
            "exact-bound draft has no overflow"
        );
        ui.approval_feedback_paste("x");
        let after = render_ui(&ui, 30, 16, 6);
        let text = buffer_text(&after);
        assert!(text.contains("4096"), "limit clipped:\n{text}");
        assert!(text.contains('█'), "cursor clipped:\n{text}");
        assert!(
            warning_cells(&after) > 0,
            "overflow warning clipped:\n{text}"
        );
    }

    #[test]
    fn approval_renders() {
        let state = approval_with(
            vec![option("allow", "Allow Once"), option("reject", "Reject")],
            vec![],
            0,
            ApprovalPhase::SelectOption,
        );
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(text.contains("Allow Once"));
        assert!(text.contains("▸ Allow Once"));
    }

    fn trust_option(label: &str, display: &str) -> cyril_core::types::TrustOption {
        cyril_core::types::TrustOption {
            label: label.into(),
            display: display.into(),
            setting_key: "allowedCommands".into(),
            patterns: vec![display.into()],
        }
    }

    #[test]
    fn trust_phase_renders_each_tier_label_and_display() {
        let state = approval_with(
            vec![option("always", "Always Allow")],
            vec![
                trust_option("Full command", "echo hello"),
                trust_option("Base command", "echo *"),
            ],
            1,
            ApprovalPhase::SelectTrust {
                chosen_option_id: cyril_core::types::PermissionOptionId::new("always"),
            },
        );
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("Full command"),
            "missing tier 0 label:\n{text}"
        );
        assert!(
            text.contains("Base command"),
            "missing tier 1 label:\n{text}"
        );
        assert!(
            text.contains("echo hello"),
            "missing tier 0 display:\n{text}"
        );
        assert!(text.contains('▸'), "missing selection marker:\n{text}");
    }

    /// cyril-a14l C8: with the popup clamped to a 5-row region above the
    /// input (inner = 3 → message + blank + ONE option row), the selected
    /// LAST option is the one shown. Pre-a14l code rendered options 0..n in
    /// order and clipped the bottom — the ▸ row vanished exactly like probe
    /// S4 showed.
    #[test]
    fn approval_selection_visible_when_clamped() {
        let state = approval_with(
            vec![option("y", "Yes"), option("a", "Always"), option("n", "No")],
            vec![],
            2,
            ApprovalPhase::SelectOption,
        );
        // input_top=6 → region rows 1-5 → popup h=5, inner=3.
        let terminal = render_at(&state, 60, 16, 6);
        let text = buffer_text(&terminal);
        assert!(text.contains("▸ No"), "selected option missing:\n{text}");
        assert!(
            text.contains("Allow execution?"),
            "message dropped:\n{text}"
        );
        // Nothing may render at or below the input row.
        for row in text.lines().skip(6) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{text}");
        }
    }

    /// cyril-a14l C8 (trust phase): 3-row items window around the selected
    /// LAST item when the region holds one item.
    #[test]
    fn trust_selection_visible_when_clamped() {
        let state = approval_with(
            vec![option("always", "Always Allow")],
            vec![
                trust_option("Full command", "echo hello"),
                trust_option("Base command", "echo *"),
                trust_option("Any command", "*"),
            ],
            2,
            ApprovalPhase::SelectTrust {
                chosen_option_id: cyril_core::types::PermissionOptionId::new("always"),
            },
        );
        // input_top=8 → region rows 1-7 → popup h=7, inner=5 → header + 1 item.
        let terminal = render_at(&state, 60, 16, 8);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("▸ Any command"),
            "selected item missing:\n{text}"
        );
        for row in text.lines().skip(8) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{text}");
        }
    }

    #[test]
    fn renders_foreign_session_in_both_phases_without_displacing_selection() {
        let option_state = approval_with(
            vec![option("y", "Yes"), option("n", "No")],
            vec![],
            1,
            ApprovalPhase::SelectOption,
        );
        let option_terminal = render_at_with_attribution(&option_state, 80, 24, 24, Some("peer-α"));
        let option_text = buffer_text(&option_terminal);
        assert!(option_text.contains("Permission Required — peer-α"));

        let trust_state = approval_with(
            vec![option("always", "Always Allow")],
            vec![trust_option("Full command", "echo hello")],
            0,
            ApprovalPhase::SelectTrust {
                chosen_option_id: cyril_core::types::PermissionOptionId::new("always"),
            },
        );
        let trust_terminal = render_at_with_attribution(&trust_state, 80, 24, 24, Some("peer-α"));
        let trust_text = buffer_text(&trust_terminal);
        assert!(trust_text.contains("Always Allow — Choose Scope — peer-α"));

        let long_origin = "x".repeat(256);
        let option_clamped =
            render_at_with_attribution(&option_state, 24, 16, 4, Some(&long_origin));
        assert!(buffer_text(&option_clamped).contains("▸ No"));
        let trust_clamped = render_at_with_attribution(&trust_state, 24, 16, 4, Some(&long_origin));
        assert!(buffer_text(&trust_clamped).contains("▸ Full command"));
    }

    /// place() empty-rect contract: no region → nothing rendered, no panic.
    #[test]
    fn empty_region_renders_nothing() {
        let state = approval_with(
            vec![option("y", "Yes")],
            vec![],
            0,
            ApprovalPhase::SelectOption,
        );
        let terminal = render_at(&state, 60, 16, 1);
        let text = buffer_text(&terminal);
        assert_eq!(text.trim(), "", "expected empty frame:\n{text}");
    }

    /// One option in a 3-row region (inner = 1): the selection alone renders.
    #[test]
    fn single_row_popup_shows_selection_only() {
        let state = approval_with(
            vec![option("y", "Yes"), option("n", "No")],
            vec![],
            1,
            ApprovalPhase::SelectOption,
        );
        // input_top=4 → region rows 1-3 → popup h=3, inner=1.
        let terminal = render_at(&state, 60, 16, 4);
        let text = buffer_text(&terminal);
        assert!(text.contains("▸ No"), "selection missing:\n{text}");
        assert!(
            !text.contains("Allow execution?"),
            "message should be dropped"
        );
    }

    /// cyril-vojx: a popup clamped to inner height 0 draws its borders and
    /// nothing else — no option, no path — so the confirm guard reads it as
    /// not on screen; one more row (inner 1) brings the selection and its
    /// path back, and the guard with them.
    #[rstest::rstest]
    #[case::with_path(write_approval("/tmp/vojx/target.md"))]
    #[case::without_path(approval_with(
        vec![option("y", "Yes"), option("n", "No")],
        vec![],
        0,
        ApprovalPhase::SelectOption,
    ))]
    fn inner_height_zero_draws_no_option_and_cannot_be_confirmed(#[case] state: ApprovalState) {
        let area = Rect::new(0, 0, 80, 16);
        // input_top=3 → region rows 1-2 → popup h=2, inner=0.
        let terminal = render_at(&state, 80, 16, 3);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        assert!(
            rows[1].contains("Permission Required"),
            "top border expected on row 1:\n{screen}"
        );
        assert!(
            rows[2].contains('└'),
            "bottom border expected on row 2:\n{screen}"
        );
        assert!(!screen.contains('▸'), "no option may be drawn:\n{screen}");
        assert!(!screen.contains("/tmp/vojx"), "no path is drawn:\n{screen}");
        assert!(
            !confirm_is_on_screen(area, 3, &state, &theme()),
            "nothing on screen to confirm"
        );

        let roomier = render_at(&state, 80, 16, 4);
        let screen = screen_rows(&roomier).join("\n");
        assert!(
            screen.contains('▸'),
            "inner 1 draws the selection:\n{screen}"
        );
        assert!(
            confirm_is_on_screen(area, 4, &state, &theme()),
            "inner 1 shows the selection (and its path), so it confirms:\n{screen}"
        );
    }

    // ---------- cyril-rcub: long write paths stay identifiable ----------

    /// The live 2.32.0 path the overlay clipped to `…/conductor-spike/a`.
    const LONG_WRITE_PATH: &str = "/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-2.32.0/live/cyril-kas/S4b-write-outside-approval.txt";
    const LONG_WRITE_NAME: &str = "S4b-write-outside-approval.txt";
    /// Wide glyphs (2 columns each) in directories and the file name.
    const WIDE_WRITE_PATH: &str =
        "/home/用户/项目/文档/非常长的目录名称/另一个目录/设计说明书-最终版本.md";
    const WIDE_WRITE_NAME: &str = "设计说明书-最终版本.md";

    fn write_approval(path: &str) -> ApprovalState {
        approval_for_tool_call(ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_rcub"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": path, "text": "body"})),
        ))
    }

    /// Screen rows as a reader sees them: a wide glyph's blank continuation
    /// cell is skipped rather than read as a space.
    fn screen_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
        let buffer = terminal.backend().buffer();
        let area = *buffer.area();
        (0..area.height)
            .map(|y| {
                let mut row = String::new();
                let mut x = 0;
                while x < area.width {
                    let symbol = buffer[(x, y)].symbol();
                    row.push_str(symbol);
                    let advance = unicode_width::UnicodeWidthStr::width(symbol).max(1);
                    x = x.saturating_add(u16::try_from(advance).unwrap_or(1));
                }
                row
            })
            .collect()
    }

    /// The text between a popup row's side borders with the preview's
    /// two-column indent removed; `None` for rows outside the popup body.
    fn popup_body(row: &str) -> Option<&str> {
        let start = row.find('│')? + '│'.len_utf8();
        let end = row.rfind('│')?;
        let inner = row.get(start..end)?;
        Some(inner.strip_prefix("  ").unwrap_or(inner).trim_end())
    }

    /// Popup body rows concatenated, so a path wrapped across rows reads back
    /// as one string.
    fn joined_body(terminal: &Terminal<TestBackend>) -> String {
        screen_rows(terminal)
            .iter()
            .filter_map(|row| popup_body(row))
            .collect()
    }

    /// cyril-rcub fence (roomy): when the popup has rows to spare, the whole
    /// path is on screen — wrapped across rows, never clipped at the border.
    #[rstest::rstest]
    #[case::ascii_80x24(LONG_WRITE_PATH, 80, 24)]
    #[case::ascii_narrow_40x24(LONG_WRITE_PATH, 40, 24)]
    #[case::wide_chars_80x24(WIDE_WRITE_PATH, 80, 24)]
    #[case::wide_chars_narrow_30x24(WIDE_WRITE_PATH, 30, 24)]
    fn long_write_path_is_fully_visible_when_rows_allow(
        #[case] path: &str,
        #[case] width: u16,
        #[case] height: u16,
    ) {
        let terminal = render_at(&write_approval(path), width, height, height);
        let joined = joined_body(&terminal);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            joined.contains(path),
            "full path not readable on screen:\n{screen}"
        );
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
    }

    /// cyril-rcub fence (clamped): with a single preview row the path is
    /// middle-elided, keeping the file name whole plus leading context, and
    /// the options stay actionable.
    #[rstest::rstest]
    #[case::ascii_80(LONG_WRITE_PATH, LONG_WRITE_NAME, 80)]
    #[case::ascii_narrow_44(LONG_WRITE_PATH, LONG_WRITE_NAME, 44)]
    #[case::wide_chars_80(WIDE_WRITE_PATH, WIDE_WRITE_NAME, 80)]
    #[case::wide_chars_narrow_36(WIDE_WRITE_PATH, WIDE_WRITE_NAME, 36)]
    fn clamped_write_path_keeps_file_name(
        #[case] path: &str,
        #[case] name: &str,
        #[case] width: u16,
    ) {
        // input_top=7 → region rows 1-6 → popup h=6, inner=4: message,
        // blank, ONE preview row, ONE option row (no spare row for the
        // separator).
        let terminal = render_at(&write_approval(path), width, 16, 7);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        let path_row = rows
            .iter()
            .filter_map(|row| popup_body(row))
            .find(|body| body.contains(name))
            .unwrap_or_else(|| panic!("file name {name} not on screen:\n{screen}"));
        assert_whole_component_head(path, path_row, &screen);
        assert!(
            path_row.ends_with(name),
            "file name must end the row: {path_row:?}\n{screen}"
        );
        assert!(
            path_row.contains('…'),
            "elision must be marked: {path_row:?}\n{screen}"
        );
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
        for row in rows.iter().skip(7) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
    }

    /// The text before an elision is a whole-component prefix of `path`:
    /// empty, or ending at a separator (review round 2).
    fn assert_whole_component_head(path: &str, path_row: &str, screen: &str) {
        let Some((head, _)) = path_row.split_once('…') else {
            return; // nothing elided
        };
        assert!(
            path.starts_with(head) && (head.is_empty() || head.ends_with(['/', '\\'])),
            "head {head:?} is not a whole-component prefix of {path:?}\n{screen}"
        );
    }

    /// Popup body rows that end with the last `columns` display columns of
    /// `path` — evidence the path's end is on screen.
    fn rows_ending_with_path_tail(rows: &[String], path: &str, columns: usize) -> usize {
        let tail = trailing_columns(path, columns);
        rows.iter()
            .filter_map(|row| popup_body(row))
            .filter(|body| body.ends_with(tail))
            .count()
    }

    /// cyril-rcub review round 2 fence: at the cramped heights (inner 1-3)
    /// an option is actionable, so the path must be on screen beside it.
    /// Inner 3: message, path, option. Inner 2: path, option (the message is
    /// the first chrome dropped). Inner 1: the lone option row carries the
    /// elided path after its label.
    #[rstest::rstest]
    #[case::inner_1(4, false)]
    #[case::inner_2(5, false)]
    #[case::inner_3(6, true)]
    fn cramped_heights_show_path_beside_the_option(
        #[case] input_top: u16,
        #[case] message_shown: bool,
    ) {
        let terminal = render_at(&write_approval(LONG_WRITE_PATH), 80, 16, input_top);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        assert!(screen.contains("▸ Allow"), "selection missing:\n{screen}");
        assert_eq!(
            rows_ending_with_path_tail(&rows, LONG_WRITE_PATH, LONG_WRITE_NAME.width()),
            1,
            "file name must end exactly one row:\n{screen}"
        );
        assert_eq!(
            screen.contains("Write File"),
            message_shown,
            "message presence at this height:\n{screen}"
        );
        for row in rows.iter().skip(usize::from(input_top)) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
    }

    /// cyril-rcub review round 2: the layout table at every height and
    /// preview size. Rows never exceed `inner`; any non-empty popup shows
    /// the selection; with a path, any shown option has the path beside it
    /// (its own row, or inline at height 1).
    #[test]
    fn option_layout_never_shows_an_option_without_the_path() {
        for inner in 0usize..=12 {
            for preview_len in 0usize..=8 {
                for has_path in [false, true] {
                    if has_path && preview_len == 0 {
                        continue; // a path always yields at least one row
                    }
                    let layout = OptionLayout::new(inner, preview_len, has_path);
                    let context = format!("inner={inner} preview={preview_len} path={has_path}");
                    let used = usize::from(layout.message)
                        + usize::from(layout.blank)
                        + layout.preview_rows
                        + usize::from(layout.separator)
                        + layout.option_rows;
                    assert!(used <= inner, "over budget {layout:?}: {context}");
                    assert_eq!(
                        layout.option_rows >= 1,
                        inner >= 1,
                        "selection row: {context}"
                    );
                    assert!(
                        layout.preview_rows <= preview_len,
                        "invented rows: {context}"
                    );
                    if has_path && layout.option_rows >= 1 {
                        assert!(
                            layout.preview_rows >= 1 || layout.inline_path,
                            "option without path {layout:?}: {context}"
                        );
                    }
                    if !has_path {
                        assert!(!layout.inline_path, "inline without a path: {context}");
                    }
                }
            }
        }
    }

    /// cyril-rcub review round 2 sweep: across terminal widths and EVERY
    /// popup height, whenever an option row is visible the path's end (at
    /// least `…` plus the file name's last columns) is visible too; at 80
    /// columns the whole file name is. Nothing renders at or below the input.
    #[test]
    fn option_visible_implies_path_visible_at_every_height() {
        for (path, name) in [
            (LONG_WRITE_PATH, LONG_WRITE_NAME),
            (WIDE_WRITE_PATH, WIDE_WRITE_NAME),
        ] {
            for width in [24u16, 30, 36, 44, 60, 80] {
                for input_top in 2u16..=20 {
                    let terminal = render_at(&write_approval(path), width, 24, input_top);
                    let rows = screen_rows(&terminal);
                    let screen = rows.join("\n");
                    let context = format!("path={path} width={width} input_top={input_top}");
                    for row in rows.iter().skip(usize::from(input_top)) {
                        assert_eq!(row.trim(), "", "bled below input: {context}\n{screen}");
                    }
                    if !screen.contains('▸') {
                        continue; // no actionable row at this height
                    }
                    // Joined rows: a roomy popup wraps the name across rows.
                    let tail_columns = if width >= 80 { name.width() } else { 4 };
                    assert!(
                        joined_body(&terminal).contains(trailing_columns(path, tail_columns)),
                        "option visible without the path's end: {context}\n{screen}"
                    );
                }
            }
        }
    }

    /// cyril-rcub review round 2: the clamped reject-with-reason page. At
    /// inner 4 it holds the editor row, its label, the limit line and ONE
    /// preview row, which carries the elided path ending in the file name.
    #[test]
    fn clamped_reason_phase_keeps_file_name_and_cursor() {
        let mut ui = UiState::new(500);
        ui.show_approval(cyril_core::types::PermissionRequest {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: ToolCall::new(
                cyril_core::types::ToolCallId::new("tooluse_rcub"),
                "Write File".into(),
                ToolKind::Write,
                ToolCallStatus::Pending,
                Some(serde_json::json!({"path": LONG_WRITE_PATH, "text": "body"})),
            ),
            message: "Write File".into(),
            options: vec![option("reject", "Reject")],
            trust_options: vec![],
            can_reject_with_reason: true,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
            workflow_plan: None,
        });
        assert!(ui.approval_begin_feedback(), "feedback phase must open");
        ui.approval_feedback_paste("not that file");
        // input_top=7 → region rows 1-6 → popup h=6, inner=4.
        let terminal = render_ui(&ui, 80, 16, 7);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        let path_row = rows
            .iter()
            .filter_map(|row| popup_body(row))
            .find(|body| body.ends_with(LONG_WRITE_NAME))
            .unwrap_or_else(|| panic!("file name not on the reason page:\n{screen}"));
        assert!(path_row.contains('…'), "elision must be marked:\n{screen}");
        assert_whole_component_head(LONG_WRITE_PATH, path_row, &screen);
        assert!(
            screen.contains("not that file█"),
            "cursor clipped:\n{screen}"
        );
        for row in rows.iter().skip(7) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
        insta::assert_snapshot!("rcub_reason_phase_clamped_80x16", screen);
    }

    fn un4p_review_target_ui(target: &str, reason: bool) -> UiState {
        let tool = ToolCall::new(
            cyril_core::types::ToolCallId::new("un4p-review"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": target, "text": "body"})),
        );
        let mut ui = approval_ui_for(tool);
        if reason {
            assert!(ui.approval_begin_feedback());
            ui.approval_feedback_paste("why");
        }
        ui
    }

    /// Literal cells and outgoing diffs, not another width estimator, decide
    /// whether the compact option/reason page actually exposes its target.
    #[rstest::rstest]
    #[case::dakuten_pair("ｶﾞ", "ｶﾞ", " ", true, true)]
    #[case::handakuten_pair("ﾊﾟ", "ﾊﾟ", " ", true, true)]
    #[case::halfwidth_tail_does_not_fit("ｶﾞZ", "…", "Z", false, false)]
    #[case::handakuten_tail_does_not_fit("ﾊﾟZ", "…", "Z", false, false)]
    #[case::combining_dakuten("ｶ\u{3099}Z", "ｶ\u{3099}", "Z", true, false)]
    #[case::combining_handakuten("ﾊ\u{309a}Z", "ﾊ\u{309a}", "Z", true, false)]
    #[case::ordinary_combining("e\u{301}Z", "e\u{301}", "Z", true, false)]
    #[case::zero_cell_dakuten("\u{3099}", " ", " ", false, false)]
    #[case::zero_cell_handakuten("\u{309a}", " ", " ", false, false)]
    #[case::zero_cell_joiner("\u{200d}", " ", " ", false, false)]
    #[case::zero_cell_tab("\t", " ", " ", false, false)]
    #[case::zero_cell_nul("\0", " ", " ", false, false)]
    fn un4p_physical_path_cells_control_compact_confirmation(
        #[case] target: &str,
        #[case] first_cell: &str,
        #[case] last_cell: &str,
        #[case] confirmable: bool,
        #[case] continuation: bool,
        #[values(false, true)] reason: bool,
    ) {
        let ui = un4p_review_target_ui(target, reason);
        // Width 10 centers a six-cell popup at x=2. Its path has exactly
        // x=5..6 on y=2; y=3 retains the selected option or reason editor.
        let terminal = render_ui(&ui, 10, 16, 5);
        let painted = terminal.backend().buffer();
        assert_eq!(painted[(2, 1)].symbol(), "┌");
        assert_eq!(painted[(7, 2)].symbol(), "│");
        assert_eq!(painted[(5, 2)].symbol(), first_cell, "{target:?}");
        assert_eq!(painted[(6, 2)].symbol(), last_cell, "{target:?}");
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        for (x, expected) in [(5, first_cell), (6, last_cell)] {
            if expected != " " {
                assert!(
                    updates.iter().any(|(column, row, cell)| {
                        (*column, *row) == (x, 2) && cell.symbol() == expected
                    }),
                    "target cell ({x}, 2) missing from outgoing diff for {target:?}"
                );
            }
        }
        if continuation {
            assert!(
                !updates.iter().any(|(x, y, _)| (*x, *y) == (6, 2)),
                "continuation must not overwrite the halfwidth mark: {target:?}"
            );
        }
        let approval = ui.approval().expect("approval");
        assert_eq!(approval.tool_call.primary_path(), Some(target));
        assert_eq!(
            confirm_is_on_screen(Rect::new(0, 0, 10, 16), 5, approval, &theme()),
            confirmable,
            "confirmation must match the literal physical target cells: {target:?}"
        );
    }

    #[rstest::rstest]
    #[case::dakuten("ｶﾞZ", "ｶﾞ")]
    #[case::handakuten("ﾊﾟZ", "ﾊﾟ")]
    fn un4p_halfwidth_wrapped_path_keeps_final_cell_before_confirmation(
        #[case] target: &str,
        #[case] first_cell: &str,
    ) {
        let ui = un4p_review_target_ui(target, true);
        let terminal = render_ui(&ui, 10, 16, 8);
        let painted = terminal.backend().buffer();
        assert_eq!(painted[(5, 2)].symbol(), first_cell);
        assert_eq!(painted[(6, 2)].symbol(), " ");
        assert_eq!(painted[(5, 3)].symbol(), "Z");
        assert_eq!(painted[(7, 2)].symbol(), "│");
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        assert!(
            updates
                .iter()
                .any(|(x, y, cell)| { (*x, *y) == (5, 2) && cell.symbol() == first_cell })
        );
        assert!(!updates.iter().any(|(x, y, _)| (*x, *y) == (6, 2)));
        assert!(
            updates
                .iter()
                .any(|(x, y, cell)| { (*x, *y) == (5, 3) && cell.symbol() == "Z" })
        );
        assert!(confirm_is_on_screen(
            Rect::new(0, 0, 10, 16),
            8,
            ui.approval().expect("approval"),
            &theme()
        ));
    }

    #[rstest::rstest]
    #[case::clipped_dakuten_pair("ｶﾞ", " ", false)]
    #[case::clipped_handakuten_pair("ﾊﾟ", " ", false)]
    #[case::combining_dakuten("ｶ\u{3099}", "ｶ\u{3099}", true)]
    #[case::combining_handakuten("ﾊ\u{309a}", "ﾊ\u{309a}", true)]
    #[case::ordinary_combining("e\u{301}", "e\u{301}", true)]
    fn un4p_one_path_cell_uses_physical_fitted_row_guard(
        #[case] target: &str,
        #[case] first_cell: &str,
        #[case] confirmable: bool,
        #[values(false, true)] reason: bool,
    ) {
        let ui = un4p_review_target_ui(target, reason);
        let terminal = render_ui(&ui, 9, 16, 5);
        let painted = terminal.backend().buffer();
        assert_eq!(painted[(5, 2)].symbol(), first_cell);
        assert_eq!(painted[(6, 2)].symbol(), "│");
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        if first_cell != " " {
            assert!(
                updates
                    .iter()
                    .any(|(x, y, cell)| { (*x, *y) == (5, 2) && cell.symbol() == first_cell })
            );
        }
        assert_eq!(
            confirm_is_on_screen(
                Rect::new(0, 0, 9, 16),
                5,
                ui.approval().expect("approval"),
                &theme()
            ),
            confirmable,
            "a whole but physically clipped target grapheme cannot authorize"
        );
    }

    #[rstest::rstest]
    #[case::dakuten("ﾞ", " ﾞ")]
    #[case::handakuten("ﾟ", " ﾟ")]
    fn un4p_standalone_sound_mark_keeps_physical_context_and_diff_anchor(
        #[case] target: &str,
        #[case] painted_symbol: &str,
        #[values(9, 10)] width: u16,
        #[values(false, true)] reason: bool,
    ) {
        let ui = un4p_review_target_ui(target, reason);
        let terminal = render_ui(&ui, width, 16, 5);
        let painted = terminal.backend().buffer();
        // A leading Extend joins the last indent space in the same Span.
        // The two-cell symbol starts at x4; its mark occupies x5 physically.
        assert_eq!(painted[(4, 2)].symbol(), painted_symbol);
        assert_eq!(painted[(5, 2)].symbol(), " ");
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        assert!(
            updates
                .iter()
                .any(|(x, y, cell)| { (*x, *y) == (4, 2) && cell.symbol() == painted_symbol })
        );
        assert!(!updates.iter().any(|(x, y, _)| (*x, *y) == (5, 2)));
        let approval = ui.approval().expect("approval");
        assert_eq!(approval.tool_call.primary_path(), Some(target));
        assert!(confirm_is_on_screen(
            Rect::new(0, 0, width, 16),
            5,
            approval,
            &theme()
        ));
    }

    #[test]
    fn un4p_raw_controls_keep_bytes_without_taking_cells() {
        let raw = "a\tb\0c\u{7f}";
        assert_eq!(wrap_columns(raw, 2), ["a\tb\0", "c\u{7f}"]);
        assert_eq!(wrapped_row_count(raw, 2), 2);
        assert_eq!(wrap_columns("\r\n", 1), ["\r\n"]);
        assert_eq!(wrapped_row_count("\r\n", 1), 1);
        assert_eq!(leading_components("/\0a/\tb", 3), "/\0a/");
        assert_eq!(trailing_columns("a\0b\tc", 2), "\0b\tc");
        assert_eq!(wrap_columns("a\u{85}b", 2), ["a\u{85}b"]);
        assert_eq!(wrapped_row_count("a\u{85}b", 2), 1);
        assert_eq!(leading_components("/\u{85}a/\tb", 3), "/\u{85}a/");
        assert_eq!(trailing_columns("a\u{85}b\tc", 2), "\u{85}b\tc");
    }

    /// A literal filename glyph is not an inserted omission marker. Whole
    /// one-character targets are valid context when actually drawn in full.
    #[rstest::rstest]
    #[case::literal_ellipsis("/tmp/report…")]
    #[case::single_character("a")]
    fn un4p_review_literal_target_is_confirmable_when_fully_drawn(
        #[case] target: &str,
        #[values(false, true)] reason: bool,
    ) {
        let ui = un4p_review_target_ui(target, reason);
        let rows = screen_rows(&render_ui(&ui, 80, 24, 24));
        assert!(
            rows.iter()
                .filter_map(|row| popup_body(row))
                .any(|body| body == target),
            "fixture target must actually be fully drawn: {rows:?}"
        );
        assert!(
            confirm_is_on_screen(
                Rect::new(0, 0, 80, 24),
                24,
                ui.approval().expect("approval"),
                &theme()
            ),
            "fully drawn literal target must be confirmable: {rows:?}"
        );
    }

    /// At popup width6, the path has two actual cells after its indent:
    /// one #+VS16 grapheme, not the scalar-counted two. A second path row
    /// must reveal the second grapheme before confirmation can succeed.
    #[rstest::rstest]
    #[case::clipped_last_target_grapheme(5, false)]
    #[case::both_target_graphemes_drawn(8, true)]
    fn un4p_review_grapheme_path_guard_matches_actual_rendered_tail(
        #[case] input_top: u16,
        #[case] fully_drawn: bool,
    ) {
        let target = "#\u{fe0f}#\u{fe0f}";
        let ui = un4p_review_target_ui(target, true);
        let rows = screen_rows(&render_ui(&ui, 10, 16, input_top));
        let drawn: String = rows
            .iter()
            .filter_map(|row| popup_body(row))
            .flat_map(|body| body.chars().filter(|ch| matches!(*ch, '#' | '\u{fe0f}')))
            .collect();
        assert_eq!(
            drawn == target,
            fully_drawn,
            "actual target cells: {rows:?}"
        );
        assert_eq!(
            confirm_is_on_screen(
                Rect::new(0, 0, 10, 16),
                input_top,
                ui.approval().expect("approval"),
                &theme()
            ),
            fully_drawn,
            "confirmation must track the actual last target grapheme: {rows:?}"
        );
    }

    /// A two-row identifying tail outranks the cosmetic request message.
    #[test]
    fn un4p_option_identifying_path_rows_precede_message() {
        let state = write_approval("/tmp/a");
        // Terminal width ten leaves a six-cell popup, two path columns
        // after borders/indent, and three inner rows above input_top six.
        let terminal = render_at(&state, 10, 16, 6);
        let rows = screen_rows(&terminal);
        let body: Vec<_> = rows.iter().filter_map(|row| popup_body(row)).collect();
        assert_eq!(body, ["/…", "/a", "▸ Al"], "{rows:?}");
        assert!(confirm_is_on_screen(
            Rect::new(0, 0, 10, 16),
            6,
            &state,
            &theme()
        ));
    }

    /// Limit and a validation notice cannot take the second path row that
    /// makes a narrow target identifiable beside the reason editor.
    #[rstest::rstest]
    fn un4p_reason_identifying_path_rows_precede_limit_and_notice(
        #[values(false, true)] notice: bool,
    ) {
        let mut ui = un4p_review_target_ui("/tmp/a", true);
        if notice {
            ui.approval_feedback_paste(&"x".repeat(4_096));
            let ApprovalPhase::EnterReason { editor, .. } =
                &ui.approval().expect("reason approval").phase
            else {
                panic!("reason editor must stay active");
            };
            assert_eq!(editor.text(), "why");
            assert!(editor.notice().is_some());
        }
        let terminal = render_ui(&ui, 10, 16, 6);
        let rows = screen_rows(&terminal);
        let body: Vec<_> = rows.iter().filter_map(|row| popup_body(row)).collect();
        assert_eq!(body, ["/…", "/a", "why█"], "{rows:?}");
        assert!(confirm_is_on_screen(
            Rect::new(0, 0, 10, 16),
            6,
            ui.approval().expect("approval"),
            &theme()
        ));
    }

    /// More path rows are not necessarily identifying: whole wrapping can
    /// expose an overwide prefix that a smaller elided fit safely omits.
    #[rstest::rstest]
    fn un4p_nonmonotone_path_fit_remains_identifying(#[values(false, true)] reason: bool) {
        let ui = un4p_review_target_ui("界/ab", reason);
        let input_top = if reason { 14 } else { 10 };
        let terminal = render_ui(&ui, 9, 20, input_top);
        let painted = terminal.backend().buffer();
        assert_eq!(painted[(2, 1)].symbol(), "┌");
        assert_eq!(painted[(6, 1)].symbol(), "┐");
        assert_eq!(painted[(2, input_top - 1)].symbol(), "└");
        let rows = screen_rows(&terminal);
        let body: Vec<_> = rows.iter().filter_map(|row| popup_body(row)).collect();
        assert_eq!(body.len(), if reason { 11 } else { 7 }, "{rows:?}");
        assert!(
            body.iter()
                .any(|row| row.contains(if reason { '█' } else { '▸' })),
            "the editor/selection must actually be drawn: {rows:?}"
        );
        let path: Vec<_> = rows
            .iter()
            .filter_map(|row| {
                let start = row.find("│  ")? + "│  ".len();
                let end = row.rfind('│')?;
                row.get(start..end).map(str::trim_end)
            })
            .filter(|row| !row.is_empty())
            .collect();
        assert!(path.ends_with(&["a", "b"]), "fixture tail: {rows:?}");
        assert!(
            confirm_is_on_screen(
                Rect::new(0, 0, 9, 20),
                input_top,
                ui.approval().expect("approval"),
                &theme()
            ),
            "an available identifying fit must survive a larger preferred budget: {rows:?}"
        );
        assert_eq!(
            path,
            ["…", "a", "b"],
            "retain the physical elided fit rather than the clipped whole prefix: {rows:?}"
        );
    }

    /// cyril-un4p: even an overflow notice cannot displace the path at
    /// inner height two. The editor keeps its cursor and Esc remains usable.
    #[rstest::rstest]
    fn reason_path_precedes_limit_and_notice(#[values(false, true)] notice: bool) {
        let tool = ToolCall::new(
            cyril_core::types::ToolCallId::new("un4p"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": "/tmp/target.md", "text": "body"})),
        );
        let mut ui = approval_ui_for(tool);
        assert!(ui.approval_begin_feedback());
        ui.approval_feedback_paste("why");
        if notice {
            ui.approval_feedback_paste(&"x".repeat(4_096));
            assert!(
                ui.approval()
                    .and_then(|approval| match &approval.phase {
                        ApprovalPhase::EnterReason { editor, .. } => editor.notice(),
                        _ => None,
                    })
                    .is_some()
            );
        }
        let terminal = render_ui(&ui, 80, 16, 5);
        let rows = screen_rows(&terminal);
        let body: Vec<_> = rows.iter().filter_map(|row| popup_body(row)).collect();
        assert_eq!(body, ["/tmp/target.md", "why█"]);
        assert!(confirm_is_on_screen(
            Rect::new(0, 0, 80, 16),
            5,
            ui.approval().expect("approval"),
            &theme()
        ));
    }

    /// cyril-rcub (found by the clamped fence): the blank row after a shown
    /// preview is counted in the row budget, so the selected LAST option
    /// stays inside a clamped popup at every clamp height — and the path is
    /// not given up for the cosmetic separator (inner 4 has room for both).
    #[rstest::rstest]
    #[case::inner_4(7)]
    #[case::inner_5(8)]
    #[case::inner_6(9)]
    #[case::inner_7(10)]
    fn clamped_preview_keeps_selected_option_visible(#[case] input_top: u16) {
        let mut state = write_approval("/tmp/a.md");
        state.selected = 1;
        let terminal = render_at(&state, 80, 16, input_top);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        assert!(screen.contains("/tmp/a.md"), "preview dropped:\n{screen}");
        assert!(
            screen.contains("▸ Deny"),
            "selected option clipped:\n{screen}"
        );
        for row in rows.iter().skip(usize::from(input_top)) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
    }

    /// cyril-rcub: the reject-with-reason page shares the preview layout, so
    /// the path it is rejecting is readable there too.
    #[test]
    fn reason_phase_shows_full_write_path() {
        let mut ui = UiState::new(500);
        ui.show_approval(cyril_core::types::PermissionRequest {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: ToolCall::new(
                cyril_core::types::ToolCallId::new("tooluse_rcub"),
                "Write File".into(),
                ToolKind::Write,
                ToolCallStatus::Pending,
                Some(serde_json::json!({"path": LONG_WRITE_PATH, "text": "body"})),
            ),
            message: "Write File".into(),
            options: vec![option("reject", "Reject")],
            trust_options: vec![],
            can_reject_with_reason: true,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
            workflow_plan: None,
        });
        assert!(ui.approval_begin_feedback(), "feedback phase must open");
        let terminal = render_ui(&ui, 80, 30, 30);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            joined_body(&terminal).contains(LONG_WRITE_PATH),
            "full path not readable on the reason page:\n{screen}"
        );
        assert!(
            screen.contains("Reason ("),
            "editor label missing:\n{screen}"
        );
    }

    /// cyril-rcub snapshots: the roomy wrap at 80x24 and at a narrow width,
    /// and the clamped middle-elision.
    #[rstest::rstest]
    #[case::roomy_80x24("rcub_write_path_roomy_80x24", 80, 24, 24)]
    #[case::roomy_narrow_40x24("rcub_write_path_roomy_40x24", 40, 24, 24)]
    #[case::clamped_80x16("rcub_write_path_clamped_80x16", 80, 16, 7)]
    #[case::clamped_narrow_44x16("rcub_write_path_clamped_44x16", 44, 16, 7)]
    fn long_write_path_snapshots(
        #[case] name: &str,
        #[case] width: u16,
        #[case] height: u16,
        #[case] input_top: u16,
    ) {
        let terminal = render_at(&write_approval(LONG_WRITE_PATH), width, height, input_top);
        insta::assert_snapshot!(name, screen_rows(&terminal).join("\n"));
    }

    #[rstest::rstest]
    #[case::fits("/tmp/a.md", 20, "/tmp/a.md")]
    #[case::keeps_parent_and_name("/home/user/project/src/main.rs", 20, "/home/…/src/main.rs")]
    #[case::name_only_when_parent_too_wide(
        "/a/very-long-parent-directory/file.txt",
        16,
        "/a/…/file.txt"
    )]
    // Review round 2: `/home/dwa…` could be `dwalleck` or `dwalleck-x`; the
    // head stops at a separator so every directory shown is whole.
    #[case::head_never_shows_a_partial_directory(
        "/home/dwalleck-x/repos/cyril/notes.txt",
        26,
        "/home/…/cyril/notes.txt"
    )]
    #[case::relative_path_with_no_whole_head_component(
        "notes/long-directory-name/file.txt",
        14,
        "…/file.txt"
    )]
    #[case::name_wider_than_budget_keeps_its_end(
        "/x/an-extremely-long-file-name.txt",
        10,
        "…-name.txt"
    )]
    #[case::wide_glyphs("/用户/项目/设计.md", 12, "/…/设计.md")]
    #[case::wide_glyph_head_component("/用户/项目/文档/设计.md", 21, "/用户/…/文档/设计.md")]
    #[case::windows_separators(r"C:\Users\dev\projects\cyril\notes.md", 20, r"C:\…\cyril\notes.md")]
    #[case::trailing_separator("/home/user/projects/cyril/", 16, "/home/…/cyril/")]
    #[case::halfwidth_head_requires_physical_budget("/ｶﾞ/file", 7, "/…/file")]
    #[case::halfwidth_tail_is_whole("/long/ｶﾞZ", 4, "…ｶﾞZ")]
    #[case::halfwidth_tail_not_split("/long/ﾊﾟZ", 3, "…Z")]
    #[case::zero_budget("/a/b.md", 0, "")]
    fn elide_middle_keeps_file_name_and_leading_context(
        #[case] path: &str,
        #[case] budget: usize,
        #[case] expected: &str,
    ) {
        let elided = elide_middle(path, budget).text;
        assert_eq!(elided, expected);
        assert!(elided.width() <= budget, "over budget: {elided:?}");
    }

    #[test]
    fn halfwidth_preview_rows_and_clipping_match_painted_columns() {
        use ratatui::buffer::Buffer;
        use ratatui::widgets::Widget;

        for text in ["ｶﾞZ", "ﾊﾟZ"] {
            let rows = wrap_columns(text, 2);
            assert_eq!(rows, [&text[..text.len() - 1], "Z"]);
            assert_eq!(wrapped_row_count(text, 2), 2);
            assert_eq!(clip_row(text, 2), "…");
            let area = Rect::new(0, 0, 2, 2);
            let previous = Buffer::empty(area);
            let mut painted = previous.clone();
            Paragraph::new(rows.into_iter().map(Line::raw).collect::<Vec<_>>())
                .render(area, &mut painted);
            assert_eq!(painted[(0, 1)].symbol(), "Z", "{text:?}");
            assert!(
                previous
                    .diff(&painted)
                    .iter()
                    .any(|(x, y, cell)| (*x, *y) == (0, 1) && cell.symbol() == "Z")
            );
        }
        assert_eq!(clip_row("ｶ\u{3099}ZZ", 2), "ｶ\u{3099}…");
        assert_eq!(wrap_columns("ｶ\u{3099}Z", 2), ["ｶ\u{3099}Z"]);
        assert_eq!(trailing_columns("Zｶﾞ", 1), "");
        assert_eq!(trailing_columns("Zｶﾞ", 2), "ｶﾞ");
        assert_eq!(trailing_columns("Zｶ\u{3099}", 1), "ｶ\u{3099}");
    }

    #[rstest::rstest]
    #[case::wraps_whole_when_rows_allow("/abc/defgh/ij.md", 5, 4, &["/abc/", "defgh", "/ij.m", "d"])]
    #[case::elides_when_rows_short("/abc/defgh/ij.md", 5, 2, &["/…/ij", ".md"])]
    #[case::wide_glyph_straddle_shrinks_budget("/x/a文件", 3, 2, &["…文", "件"])]
    #[case::halfwidth_exact_fit("ｶﾞZ", 3, 1, &["ｶﾞZ"])]
    #[case::halfwidth_wraps("ｶﾞZ", 2, 2, &["ｶﾞ", "Z"])]
    #[case::handakuten_wraps("ﾊﾟZ", 2, 2, &["ﾊﾟ", "Z"])]
    #[case::halfwidth_elides("ｶﾞZ", 2, 1, &["…Z"])]
    #[case::combining_dakuten_fits("ｶ\u{3099}Z", 2, 1, &["ｶ\u{3099}Z"])]
    #[case::combining_handakuten_fits("ﾊ\u{309a}Z", 2, 1, &["ﾊ\u{309a}Z"])]
    #[case::no_rows("/abc/defgh/ij.md", 5, 0, &[])]
    fn fit_path_wraps_or_elides_into_rows(
        #[case] path: &str,
        #[case] width: usize,
        #[case] rows: usize,
        #[case] expected: &[&str],
    ) {
        assert_eq!(fit_path(path, width, rows).rows, expected);
    }

    /// Invariant sweep over widths and row budgets: never more rows than
    /// granted, no row wider than the width (a lone wide glyph in a
    /// one-column row excepted), the whole path whenever it fits, and the
    /// file name intact whenever the budget can hold it.
    #[test]
    fn fit_path_invariants_hold_across_widths_and_rows() {
        let paths = [
            (LONG_WRITE_PATH, LONG_WRITE_NAME),
            (WIDE_WRITE_PATH, WIDE_WRITE_NAME),
            (r"C:\Users\dev\projects\cyril\notes.md", "notes.md"),
            ("/tmp/a.md", "a.md"),
        ];
        for (path, name) in paths {
            for width in 1..=40usize {
                for rows in 1..=5usize {
                    let fitted = fit_path(path, width, rows).rows;
                    let joined = fitted.concat();
                    let context = format!("path={path} width={width} rows={rows} -> {fitted:?}");
                    assert!(fitted.len() <= rows, "too many rows: {context}");
                    for row in &fitted {
                        assert!(row.width() <= width.max(2), "row too wide: {context}");
                    }
                    if wrap_columns(path, width).len() <= rows {
                        assert_eq!(joined, path, "fitting path altered: {context}");
                    } else {
                        assert!(joined.contains(ELLIPSIS), "unmarked elision: {context}");
                    }
                    if width >= 2 && name.width() + 1 + rows <= width * rows {
                        assert!(joined.ends_with(name), "file name lost: {context}");
                    }
                }
            }
        }
    }

    /// A smaller identifying path fit leaves its unused row for the body,
    /// rather than letting a second, larger fit hide the degraded marker.
    #[rstest::rstest]
    fn ej7y_narrow_identifying_path_keeps_the_remaining_body_row(
        #[values(false, true)] reason: bool,
    ) {
        let tc = preview_tool_call(
            Some(serde_json::json!({"path": "界/ab", "text": 42})),
            false,
        );
        let mut ui = approval_ui_for(tc);
        if reason {
            assert!(ui.approval_begin_feedback());
            ui.approval_feedback_paste("why");
        }
        let input_top = if reason { 14 } else { 10 };
        let terminal = render_ui(&ui, 9, 20, input_top);
        let painted = terminal.backend().buffer();
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        // The retained three-row path leaves one of the four preview rows.
        // These are actual painted/outgoing cells, not another fitter's count.
        let body_cells = [
            (2, 1, "┌"),
            (6, 1, "┐"),
            (5, 4, "…"),
            (5, 5, "a"),
            (5, 6, "b"),
            (5, 7, "P"),
        ];
        let action_cells: &[(u16, u16, &str)] = if reason {
            &[(3, 10, "█")]
        } else {
            &[(3, 8, "▸")]
        };
        for &(x, y, symbol) in body_cells.iter().chain(action_cells) {
            assert_eq!(painted[(x, y)].symbol(), symbol, "cell ({x},{y})");
            assert!(
                updates.iter().any(|(column, row, cell)| {
                    (*column, *row) == (x, y) && cell.symbol() == symbol
                }),
                "missing outgoing cell ({x},{y})={symbol:?}"
            );
        }
    }

    // ---------- cyril-j1b3: joined approval preview ----------

    use cyril_core::types::{ToolCall, ToolCallContent, ToolCallStatus, ToolKind};

    fn approval_for_tool_call(tc: ToolCall) -> ApprovalState {
        ApprovalState {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: crate::traits::TrackedToolCall::new(tc),
            message: "Write File".into(),
            options: vec![option("accept", "Allow"), option("reject", "Deny")],
            trust_options: vec![],
            selected: 0,
            preview_scroll_offset: 0,
            can_reject_with_reason: false,
            phase: ApprovalPhase::SelectOption,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
            workflow_plan: None,
        }
    }

    /// C3 fence: a KAS Write File snapshot (rawInput {path, text}, no diff
    /// content yet) renders the path and the proposed text above the options.
    #[test]
    fn renders_joined_preview() {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_x"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({
                "path": "/tmp/specs/bug fix.md",
                "text": "# Héllo\nsecond line\nthird line"
            })),
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 30, 30);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("/tmp/specs/bug fix.md"),
            "path missing (unicode/spaces fixture):\n{text}"
        );
        assert!(text.contains("# Héllo"), "first text line missing:\n{text}");
        assert!(
            text.contains("third line"),
            "last text line missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }

    fn shell_approval(command: &str, cwd: Option<&str>) -> ApprovalState {
        let tool = ToolCall::new(
            cyril_core::types::ToolCallId::new("xavt"),
            "Run Command".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"command": command, "cwd": cwd})),
        )
        .with_content(vec![ToolCallContent::Text(
            "generic output must not hide the command".into(),
        )])
        .with_shell_command(cyril_core::types::ShellCommand::new(
            command.to_owned(),
            cwd.map(str::to_owned),
        ));
        let mut state = approval_for_tool_call(tool);
        state.message = "Allow this command?".into();
        state
    }

    fn shell_body_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
        screen_rows(terminal)
            .iter()
            .filter_map(|row| popup_body(row))
            .filter_map(|body| body.strip_prefix("  "))
            .map(str::to_owned)
            .collect()
    }

    /// The visible meaningful-command predicate uses physical cells, including
    /// standalone halfwidth marks, but never zero-cell or whitespace context.
    #[rstest::rstest]
    #[case::no_body_cell("ﾞ", 10, " ", false)]
    #[case::standalone_dakuten("ﾞ", 11, " ", true)]
    #[case::standalone_handakuten("ﾟ", 11, " ", true)]
    #[case::clipped_dakuten_pair("ｶﾞ", 11, " ", false)]
    #[case::clipped_handakuten_pair("ﾊﾟ", 11, " ", false)]
    #[case::exact_dakuten_pair("ｶﾞ", 12, "ｶﾞ", true)]
    #[case::exact_handakuten_pair("ﾊﾟ", 12, "ﾊﾟ", true)]
    #[case::combining_dakuten("ｶ\u{3099}", 11, "ｶ\u{3099}", true)]
    #[case::combining_handakuten("ﾊ\u{309a}", 11, "ﾊ\u{309a}", true)]
    #[case::ordinary_combining("e\u{301}", 11, "e\u{301}", true)]
    #[case::combining_only("\u{301}", 11, " ", false)]
    #[case::zero_cell_dakuten("\u{3099}", 11, " ", false)]
    #[case::zero_cell_handakuten("\u{309a}", 11, " ", false)]
    #[case::zero_cell_joiner("\u{200d}", 11, " ", false)]
    #[case::whitespace_and_controls(" \t\u{202e}", 11, " ", false)]
    fn xavt_shell_physical_cells_control_current_viewport_confirmation(
        #[case] command: &str,
        #[case] width: u16,
        #[case] first_cell: &str,
        #[case] confirmable: bool,
    ) {
        let state = shell_approval(command, None);
        let terminal = render_at(&state, width, 16, 5);
        let painted = terminal.backend().buffer();
        // Widths 10/11/12 center at x=2; after the border and four-space
        // indent, x=7 is respectively the border, one cell, or two cells.
        let expected = if width == 10 { "│" } else { first_cell };
        assert_eq!(painted[(7, 2)].symbol(), expected, "{command:?}");
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        if first_cell != " " {
            assert!(
                updates
                    .iter()
                    .any(|(x, y, cell)| { (*x, *y) == (7, 2) && cell.symbol() == first_cell })
            );
        }
        // The leading Extend belongs to the last indent space's two-cell
        // symbol, anchored at x6 rather than separately at x7.
        let standalone = match (command, width) {
            ("ﾞ", 11) => Some(" ﾞ"),
            ("ﾟ", 11) => Some(" ﾟ"),
            _ => None,
        };
        if let Some(symbol) = standalone {
            assert_eq!(painted[(6, 2)].symbol(), symbol);
            assert!(
                updates
                    .iter()
                    .any(|(x, y, cell)| { (*x, *y) == (6, 2) && cell.symbol() == symbol })
            );
            assert!(!updates.iter().any(|(x, y, _)| (*x, *y) == (7, 2)));
        }
        if width == 12 {
            assert_eq!(painted[(8, 2)].symbol(), " ");
            assert!(!updates.iter().any(|(x, y, _)| (*x, *y) == (8, 2)));
            assert_eq!(painted[(9, 2)].symbol(), "│");
        }
        let PreviewBody::Shell(shell) = state.tool_call.preview_body() else {
            panic!("typed shell fixture");
        };
        assert_eq!(shell.command(), command, "display must not normalize input");
        assert_eq!(
            confirm_is_on_screen(Rect::new(0, 0, width, 16), 5, &state, &theme()),
            confirmable,
            "only meaningful command cells in the current viewport may confirm"
        );
    }

    #[rstest::rstest]
    #[case::dakuten("ｶﾞ")]
    #[case::handakuten("ﾊﾟ")]
    fn xavt_shell_halfwidth_boundary_keeps_command_and_cwd_tails_reachable(
        #[case] glyph: &str,
        #[values(false, true)] cwd: bool,
        #[values(1, usize::MAX)] offset: usize,
    ) {
        let raw = if cwd {
            format!("/{}{glyph}Z", "x".repeat(46))
        } else {
            format!("{}{glyph}Z", "x".repeat(52))
        };
        let mut state = if cwd {
            shell_approval(":", Some(&raw))
        } else {
            shell_approval(&raw, None)
        };
        let area = Rect::new(0, 0, 80, 24);
        if cwd {
            assert!(confirm_is_on_screen(area, 5, &state, &theme()));
            state.preview_scroll_offset = 1;
        }
        let first = render_at(&state, 80, 24, 5);
        let painted = first.backend().buffer();
        assert_eq!(painted[(10, 1)].symbol(), "┌");
        assert_eq!(painted[(11, 3)].symbol(), "▸");
        assert_eq!(painted[(67, 2)].symbol(), glyph);
        assert_eq!(painted[(68, 2)].symbol(), " ");
        assert_eq!(painted[(69, 2)].symbol(), "│");
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        assert!(
            updates
                .iter()
                .any(|(x, y, cell)| { (*x, *y) == (67, 2) && cell.symbol() == glyph })
        );
        assert!(!updates.iter().any(|(x, y, _)| (*x, *y) == (68, 2)));
        assert_eq!(confirm_is_on_screen(area, 5, &state, &theme()), !cwd);

        state.preview_scroll_offset = if offset == usize::MAX {
            offset
        } else {
            offset + usize::from(cwd)
        };
        let tail = render_at(&state, 80, 24, 5);
        let painted_tail = tail.backend().buffer();
        assert_eq!(painted_tail[(15, 2)].symbol(), "Z");
        assert_eq!(painted_tail[(16, 2)].symbol(), " ");
        assert_eq!(painted_tail[(11, 3)].symbol(), "▸");
        assert!(
            painted
                .diff(painted_tail)
                .iter()
                .any(|(x, y, cell)| { (*x, *y) == (15, 2) && cell.symbol() == "Z" })
        );
        assert!(
            !confirm_is_on_screen(area, 5, &state, &theme()),
            "neither cwd nor a one-grapheme tail of a longer command is sufficient"
        );
        let PreviewBody::Shell(shell) = state.tool_call.preview_body() else {
            panic!("typed shell fixture");
        };
        if cwd {
            assert_eq!(shell.command(), ":");
            assert_eq!(shell.cwd(), Some(raw.as_str()));
        } else {
            assert_eq!(shell.command(), raw);
            assert_eq!(shell.cwd(), None);
        }
    }

    #[rstest::rstest]
    #[case::option(false, false)]
    #[case::reason(true, false)]
    #[case::reason_notice(true, true)]
    fn xavt_shell_identifying_path_and_command_precede_chrome(
        #[case] reason: bool,
        #[case] notice: bool,
    ) {
        let tool = ToolCall::new(
            cyril_core::types::ToolCallId::new("xavt-path-context"),
            "Run Command".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": "界界", "command": ":"})),
        )
        .with_shell_command(cyril_core::types::ShellCommand::new(":".into(), None));
        let mut ui = approval_ui_for(tool);
        if reason {
            assert!(ui.approval_begin_feedback());
            ui.approval_feedback_paste("why");
            if notice {
                ui.approval_feedback_paste(&"x".repeat(4_096));
                let ApprovalPhase::EnterReason { editor, .. } =
                    &ui.approval().expect("reason approval").phase
                else {
                    panic!("reason editor must stay active");
                };
                assert_eq!(editor.text(), "why");
                assert!(editor.notice().is_some());
            }
        }
        let terminal = render_ui(&ui, 11, 16, 7);
        let painted = terminal.backend().buffer();
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        // One path row can keep only one wide tail grapheme, insufficient
        // for an elided target. Two whole path rows, the command and the
        // selected action/editor must take priority over cosmetic chrome.
        let context_cells = [
            (2, 1, "┌"),
            (8, 1, "┐"),
            (5, 2, "界"),
            (5, 3, "界"),
            (7, 4, ":"),
        ];
        let action = if reason { (6, 5, "█") } else { (3, 5, "▸") };
        for &(x, y, symbol) in context_cells.iter().chain(std::iter::once(&action)) {
            assert_eq!(painted[(x, y)].symbol(), symbol, "cell ({x},{y})");
            assert!(updates.iter().any(|(column, row, cell)| {
                (*column, *row) == (x, y) && cell.symbol() == symbol
            }));
        }
        for y in [2, 3] {
            assert_eq!(painted[(6, y)].symbol(), " ");
            assert!(!updates.iter().any(|(x, row, _)| (*x, *row) == (6, y)));
        }
        let state = ui.approval().expect("approval");
        assert!(confirm_is_on_screen(
            Rect::new(0, 0, 11, 16),
            7,
            state,
            &theme()
        ));
        assert_eq!(state.tool_call.primary_path(), Some("界界"));
        let PreviewBody::Shell(shell) = state.tool_call.preview_body() else {
            panic!("typed shell fixture");
        };
        assert_eq!(shell.command(), ":");
    }

    #[rstest::rstest]
    fn xavt_shell_nonmonotone_path_keeps_its_fit_without_confirming_an_unpainted_command(
        #[values(false, true)] reason: bool,
    ) {
        let tool = ToolCall::new(
            cyril_core::types::ToolCallId::new("xavt-nonmonotone-context"),
            "Run Command".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": "界/ab", "command": ":"})),
        )
        .with_shell_command(cyril_core::types::ShellCommand::new(":".into(), None));
        let mut ui = approval_ui_for(tool);
        if reason {
            assert!(ui.approval_begin_feedback());
            ui.approval_feedback_paste("why");
        }
        let input_top = if reason { 14 } else { 10 };
        let terminal = render_ui(&ui, 9, 20, input_top);
        let painted = terminal.backend().buffer();
        let previous = ratatui::buffer::Buffer::empty(*painted.area());
        let updates = previous.diff(painted);
        let path_cells = [(5, 4, "…"), (5, 5, "a"), (5, 6, "b")];
        let action = if reason {
            (3, 10, "█")
        } else {
            (3, 8, "▸")
        };
        for &(x, y, symbol) in path_cells.iter().chain(std::iter::once(&action)) {
            assert_eq!(painted[(x, y)].symbol(), symbol, "cell ({x},{y})");
            assert!(updates.iter().any(|(column, row, cell)| {
                (*column, *row) == (x, y) && cell.symbol() == symbol
            }));
        }
        // The retained smaller identifying fit leaves a body row, but
        // width nine has no shell text cells. A path cannot authorize it.
        for x in [3, 4, 5] {
            assert_eq!(painted[(x, 7)].symbol(), " ");
            assert!(
                !updates
                    .iter()
                    .any(|(column, y, cell)| { (*column, *y) == (x, 7) && cell.symbol() == ":" })
            );
        }
        let state = ui.approval().expect("approval");
        assert!(!confirm_is_on_screen(
            Rect::new(0, 0, 9, 20),
            input_top,
            state,
            &theme()
        ));
        assert_eq!(state.tool_call.primary_path(), Some("界/ab"));
        let PreviewBody::Shell(shell) = state.tool_call.preview_body() else {
            panic!("typed shell fixture");
        };
        assert_eq!(shell.command(), ":");
    }

    /// cyril-xavt: the complete pipeline lives in the body, not a clipped
    /// title, and supplied cwd follows it. Expected wrapping is a literal
    /// ASCII fixture, independent of the production wrapper.
    #[test]
    fn shell_pipeline_and_cwd_render_in_the_approval_body() {
        let command = format!(
            "printf '%s' 'XAVT_BEGIN' | {}printf XAVT_END",
            "cat | ".repeat(12)
        );
        let state = shell_approval(&command, Some("/owned/workspace"));
        let terminal = render_at(&state, 80, 30, 30);
        let body = shell_body_rows(&terminal);
        assert_eq!(
            body.len(),
            4,
            "complete command plus cwd must be drawn: {body:?}"
        );
        assert_eq!(body[..3].concat(), command);
        assert_eq!(
            body.last().map(String::as_str),
            Some("Cwd: /owned/workspace")
        );
        let screen = screen_rows(&terminal).join("\n");
        assert!(!screen.contains("Preview unavailable"), "{screen}");
        assert!(!screen.contains("generic output"), "{screen}");
        assert!(screen.contains("▸ Allow"), "{screen}");
        insta::assert_snapshot!(
            body.join("\n"),
            @"printf '%s' 'XAVT_BEGIN' | cat | cat | cat | cat | cat\n | cat | cat | cat | cat | cat | cat | cat | printf XA\nVT_END\nCwd: /owned/workspace"
        );
    }

    /// Shell display preserves meaningful spaces/newlines, unlike a plan
    /// description's whitespace collapse. Unsafe controls/bidi become spaces.
    #[rstest::rstest]
    fn shell_display_cleans_controls_without_flattening_scripts(#[values(false, true)] cwd: bool) {
        let state = shell_approval(
            "printf  'a'\n  echo\tb\u{202E};c\u{1b}[31m",
            cwd.then_some("/tmp/\u{2066}project"),
        );
        let terminal = render_at(&state, 80, 30, 30);
        let body = shell_body_rows(&terminal);
        let mut expected = vec!["printf  'a'", "  echo b ;c [31m"];
        if cwd {
            expected.push("Cwd: /tmp/ project");
        }
        assert_eq!(body, expected);
        for ch in body.iter().flat_map(|row| row.chars()) {
            assert!(!ch.is_control() && !super::super::workflow_plan::is_bidi_format(ch));
        }
    }

    /// A supplied cwd can itself overflow. Reaching that tail must not make
    /// the choice confirmable with no command characters in the viewport.
    #[test]
    fn shell_confirmation_requires_command_not_only_cwd() {
        let cwd = format!("{}cwd_end", "/long".repeat(200));
        let mut state = shell_approval("echo COMMAND_TOKEN", Some(&cwd));
        let area = Rect::new(0, 0, 80, 30);
        assert!(confirm_is_on_screen(area, 30, &state, &theme()));
        state.preview_scroll_offset = usize::MAX;
        let terminal = render_at(&state, 80, 30, 30);
        let body = shell_body_rows(&terminal).concat();
        assert!(body.contains("cwd_end"), "{body}");
        assert!(!body.contains("COMMAND_TOKEN"), "{body}");
        assert!(!confirm_is_on_screen(area, 30, &state, &theme()));
        assert!(
            screen_rows(&terminal)
                .iter()
                .any(|row| row.contains("▸ Allow"))
        );
    }

    /// cyril-xavt F2: the emoji-presentation grapheme occupies two actual
    /// cells, so the closing quote must have its own reachable body row.
    #[rstest::rstest]
    #[case::next_row(1)]
    #[case::end(usize::MAX)]
    fn xavt_shell_fe0f_command_closing_quote_is_scroll_reachable(#[case] offset: usize) {
        let command = format!("printf '{}#\u{fe0f}'", "x".repeat(44));
        let mut state = shell_approval(&command, None);

        // 80 columns give a 60-column popup: borders + four-space indent
        // leave 54 body cells. input_top=5 clamps it to rows 1..5, with
        // exactly one body row at y=2 and the selected option at y=3.
        let initial = render_at(&state, 80, 24, 5);
        let buffer = initial.backend().buffer();
        assert_eq!(buffer[(10, 1)].symbol(), "┌");
        assert_eq!(buffer[(69, 4)].symbol(), "┘");
        assert_eq!(buffer[(11, 3)].symbol(), "▸");
        assert_eq!(
            shell_body_rows(&initial),
            [format!("printf '{}#\u{fe0f}", "x".repeat(44))]
        );
        assert_eq!(buffer[(67, 2)].symbol(), "#\u{fe0f}");
        assert_eq!(
            buffer[(68, 2)].symbol(),
            " ",
            "wide-grapheme continuation cell"
        );
        assert_eq!(
            buffer[(69, 2)].symbol(),
            "│",
            "the quote cannot overwrite the border"
        );

        state.preview_scroll_offset = offset;
        let scrolled = render_at(&state, 80, 24, 5);
        assert_eq!(
            scrolled.backend().buffer()[(15, 2)].symbol(),
            "'",
            "offset {offset} must expose the closing quote in actual cells:\n{}",
            screen_rows(&scrolled).join("\n")
        );
        assert_eq!(shell_body_rows(&scrolled), ["'"]);
        assert!(
            screen_rows(&scrolled)
                .iter()
                .any(|row| row.contains("▸ Allow")),
            "scrolling must leave the selected action visible"
        );
    }

    /// cyril-xavt F2: the supplied cwd uses the same body-cell budget as the
    /// command; its closing quote must survive without granting cwd authority.
    #[rstest::rstest]
    #[case::next_row(2)]
    #[case::end(usize::MAX)]
    fn xavt_shell_fe0f_cwd_closing_quote_is_scroll_reachable(#[case] offset: usize) {
        let command = "echo COMMAND_TOKEN";
        let cwd = format!("/{}#\u{fe0f}'", "x".repeat(46));
        let mut state = shell_approval(command, Some(&cwd));
        let area = Rect::new(0, 0, 80, 24);
        let initial = render_at(&state, 80, 24, 5);
        assert_eq!(shell_body_rows(&initial), [command]);
        assert!(confirm_is_on_screen(area, 5, &state, &theme()));

        // The command occupies the initial body row. Offset 1 reaches the
        // cwd; its five-cell "Cwd: " prefix leaves the same FE0F boundary.
        state.preview_scroll_offset = 1;
        let cwd_start = render_at(&state, 80, 24, 5);
        let buffer = cwd_start.backend().buffer();
        assert_eq!(buffer[(10, 1)].symbol(), "┌");
        assert_eq!(buffer[(69, 4)].symbol(), "┘");
        assert_eq!(buffer[(11, 3)].symbol(), "▸");
        assert_eq!(
            shell_body_rows(&cwd_start),
            [format!("Cwd: /{}#\u{fe0f}", "x".repeat(46))]
        );
        assert_eq!(buffer[(67, 2)].symbol(), "#\u{fe0f}");
        assert_eq!(
            buffer[(68, 2)].symbol(),
            " ",
            "wide-grapheme continuation cell"
        );
        assert_eq!(
            buffer[(69, 2)].symbol(),
            "│",
            "the quote cannot overwrite the border"
        );
        assert!(
            !confirm_is_on_screen(area, 5, &state, &theme()),
            "a cwd-only viewport cannot authorize the command"
        );

        state.preview_scroll_offset = offset;
        let scrolled = render_at(&state, 80, 24, 5);
        assert_eq!(
            scrolled.backend().buffer()[(15, 2)].symbol(),
            "'",
            "offset {offset} must expose the cwd's closing quote in actual cells:\n{}",
            screen_rows(&scrolled).join("\n")
        );
        assert_eq!(shell_body_rows(&scrolled), ["'"]);
        assert!(
            screen_rows(&scrolled)
                .iter()
                .any(|row| row.contains("▸ Allow")),
            "scrolling must leave the selected action visible"
        );
        assert!(
            !confirm_is_on_screen(area, 5, &state, &theme()),
            "the reachable cwd tail is still not command context"
        );
    }

    /// C3 bounds fence: 6-line raw-input text shows exactly 5 lines plus the
    /// omission marker; a 21-line diff is truncated by the diff renderer's
    /// own 20-line cap. Both must leave the options visible.
    #[test]
    fn bounds_preview() {
        let six_lines = (1..=6)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_text"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": "/tmp/a.md", "text": six_lines})),
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 40, 40);
        let text = buffer_text(&terminal);
        assert!(text.contains("line 5"), "fifth line missing:\n{text}");
        assert!(
            !text.contains("line 6"),
            "sixth line must be capped:\n{text}"
        );
        assert!(
            text.contains("...1 more lines"),
            "omission marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");

        let big_diff = (1..=21)
            .map(|i| format!("new line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_diff"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "/tmp/b.md".into(),
            old_text: None,
            new_text: big_diff,
        }]);
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 60, 60);
        let text = buffer_text(&terminal);
        assert!(text.contains("/tmp/b.md"), "diff path missing:\n{text}");
        assert!(
            text.contains("..."),
            "diff truncation marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }

    /// C4 fence: a stub with no path, no raw input, and no content renders
    /// the degraded marker while every option stays actionable.
    #[test]
    fn degraded_preview_keeps_options() {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_stub"),
            "Write File".into(),
            ToolKind::Other,
            ToolCallStatus::Pending,
            None,
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("Preview unavailable"),
            "degraded marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
        assert!(text.contains("Deny"), "second option must stay:\n{text}");
    }

    /// C4 malformed-payload fence: raw_input present but `text` non-string —
    /// the path still renders (it is valid) but the degraded marker must
    /// appear instead of coercing the value.
    #[test]
    fn malformed_raw_input_shows_marker_beside_path() {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_bad"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": "/tmp/ok.md", "text": 42})),
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(text.contains("/tmp/ok.md"), "path missing:\n{text}");
        assert!(
            text.contains("Preview unavailable"),
            "malformed payload must show the marker:\n{text}"
        );
        assert!(
            !text.contains("42"),
            "non-string text must not be coerced:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }

    // ---------- cyril-8wh0: Text content in the approval preview ----------

    /// The plan text KAS 0.66.43 sent as the `toolCall.content` Text block of
    /// a workflow plan approval (`kas-lead-wfplan-2.32.0.jsonl` frame 93,
    /// workspace path scrubbed to `<LIVE_TMP>` as in
    /// `experiments/conductor-spike/kas-workflow-plan-2.32.0.jsonl`).
    const LIVE_PLAN_TEXT: &str = "Steps to add after 'create-workflow':\n\nwrite-hello: step, agent wf-coder, model auto, effort low\n  Task: Create a file named hello.txt at the absolute path <LIVE_TMP>/ws-wfplan-nnzrhi04/hello.txt con...";

    /// The update_workflow approval as cyril joins it: the tracked tool call
    /// (title "Update Workflow", rawInput without a text field) merged with
    /// the request's Text content.
    fn plan_text_tool_call(text: &str) -> ToolCall {
        ToolCall::new(
            cyril_core::types::ToolCallId::new("toolu_bdrk_01QLZqv14pnXT2nFjjkTakjL"),
            "Update Workflow".into(),
            ToolKind::Other,
            ToolCallStatus::Pending,
            Some(serde_json::json!({
                "workflowId": "wf_6b22f0c99f407f0e",
                "action": "replace_remaining",
                "workflowRef": "generated://gen_09d8668976ecd154",
                "remainingSteps": []
            })),
        )
        .with_content(vec![ToolCallContent::Text(text.into())])
    }

    /// REGRESSION FENCE (cyril-8wh0): a permission request whose tool call
    /// carries a Text content block shows that text in the overlay instead of
    /// "Preview unavailable" — the live KAS workflow plan approval.
    #[test]
    fn text_content_renders_in_the_preview() {
        let mut state = approval_for_tool_call(plan_text_tool_call(LIVE_PLAN_TEXT));
        state.message = "Add 1 workflow step to wf_6b22f0c99f407f0e".into();
        let terminal = render_at(&state, 80, 24, 24);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            !screen.contains("Preview unavailable"),
            "the Text block must be shown:\n{screen}"
        );
        // Long plan lines wrap at the popup's text width rather than being
        // clipped at the border, so the step's effort stays on screen.
        for expected in [
            "Steps to add after 'create-workflow':",
            "write-hello: step, agent wf-coder, model auto,",
            "low",
            "Task: Create a file named hello.txt",
        ] {
            assert!(screen.contains(expected), "missing {expected:?}:\n{screen}");
        }
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
        insta::assert_snapshot!("8wh0_text_content_preview_80x24", screen);
    }

    /// cyril-8wh0: Text content is bounded like the raw-input text — five
    /// lines across all blocks, then the omission marker.
    #[test]
    fn text_content_preview_is_bounded() {
        let seven = (1..=7)
            .map(|i| format!("plan line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = plan_text_tool_call(&seven).with_content(vec![
            ToolCallContent::Text("plan line 1\nplan line 2".into()),
            ToolCallContent::Text(
                (3..=7)
                    .map(|i| format!("plan line {i}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ]);
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 40, 40);
        let text = buffer_text(&terminal);
        assert!(text.contains("plan line 5"), "fifth line missing:\n{text}");
        assert!(
            !text.contains("plan line 6"),
            "sixth line must be capped:\n{text}"
        );
        assert!(
            text.contains("...2 more lines"),
            "omission marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }

    // ---------- cyril-8wh0: the typed workflow plan in the overlay ----------

    fn node_id(text: &str) -> cyril_core::types::WorkflowNodeId {
        cyril_core::types::WorkflowNodeId::try_from(text.to_owned()).expect("node id")
    }

    fn plan_step(
        id: &str,
        agent: &str,
        effort: Option<(&str, cyril_core::types::WorkflowCascadeSource)>,
        task: Option<(&str, bool)>,
    ) -> cyril_core::types::WorkflowPlanNode {
        cyril_core::types::WorkflowPlanNode {
            node_id: node_id(id),
            kind: cyril_core::types::WorkflowPlanNodeKind::Step(
                cyril_core::types::WorkflowPlanStep {
                    agent_name: agent.to_owned(),
                    model_id: None,
                    model_source: None,
                    effort_level: effort.map(|(level, _)| level.to_owned()),
                    effort_source: effort.map(|(_, source)| source),
                    task: task.map(|(text, truncated)| cyril_core::types::WorkflowPlanTask {
                        text: text.to_owned(),
                        truncated,
                    }),
                    artifacts: Vec::new(),
                    completion: None,
                },
            ),
        }
    }

    /// The plan of the live 2.32.0 approval (`kas-lead-wfplan` frame 93), as
    /// cyril-core's converter types it (pinned there by
    /// `live_launch_plan_converts`).
    fn live_plan() -> cyril_core::types::WorkflowPlan {
        cyril_core::types::WorkflowPlan {
            workflow_id: cyril_core::types::WorkflowId::try_from("wf_6b22f0c99f407f0e".to_owned())
                .expect("workflow id"),
            proposed_by: Some(node_id("create-workflow")),
            kind: cyril_core::types::WorkflowPlanKind::Launch,
            workflow_ref: "generated://gen_09d8668976ecd154".to_owned(),
            kept_step_ids: vec![node_id("create-workflow")],
            steps: vec![plan_step(
                "write-hello",
                "wf-coder",
                Some(("low", cyril_core::types::WorkflowCascadeSource::Step)),
                Some((
                    "Create a file named hello.txt at the absolute path <LIVE_TMP>/ws-wfplan-nnzrhi04/hello.txt con",
                    true,
                )),
            )],
            omitted: None,
        }
    }

    fn plan_approval(plan: cyril_core::types::WorkflowPlan) -> ApprovalState {
        let mut state = approval_for_tool_call(plan_text_tool_call(LIVE_PLAN_TEXT));
        state.message = "Add 1 workflow step to wf_6b22f0c99f407f0e".into();
        state.workflow_plan = Some(plan);
        state
    }

    /// REGRESSION FENCE (cyril-8wh0): an `update_workflow` approval shows the
    /// typed plan's step list — header, kept steps, each step with its agent,
    /// model and effort, and its task — in place of KAS's text rendering.
    #[test]
    fn workflow_plan_renders_its_step_list() {
        let terminal = render_at(&plan_approval(live_plan()), 80, 24, 24);
        let screen = screen_rows(&terminal).join("\n");
        for expected in [
            "New workflow · 1 step after 'create-workflow'",
            "Keeps: create-workflow",
            "write-hello: wf-coder · model auto · effort low",
            "Task: Create a file named hello.txt",
        ] {
            assert!(screen.contains(expected), "missing {expected:?}:\n{screen}");
        }
        assert!(
            !screen.contains("Steps to add after"),
            "the typed plan replaces KAS's text rendering:\n{screen}"
        );
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
        insta::assert_snapshot!("8wh0_workflow_plan_80x24", screen);
    }

    /// A plan KAS had to send without its steps (`omitted: "steps"`) falls
    /// back to the request's Text content, which still lists them.
    #[test]
    fn plan_without_steps_falls_back_to_the_text() {
        let mut plan = live_plan();
        plan.steps.clear();
        plan.kept_step_ids.clear();
        plan.omitted = Some(cyril_core::types::WorkflowPlanOmission::Steps);
        let terminal = render_at(&plan_approval(plan), 80, 24, 24);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            screen.contains("Steps to add after 'create-workflow':"),
            "the text rendering must show:\n{screen}"
        );
        assert!(
            !screen.contains("New workflow ·"),
            "no empty plan header:\n{screen}"
        );
    }

    /// The step tree: nested nodes are indented under their parent, inherited
    /// settings name their source, repeats and parallels carry their rules,
    /// and a plan longer than the cap ends in an omission marker.
    #[test]
    fn workflow_plan_renders_nested_nodes() {
        use cyril_core::types::{
            WorkflowCascadeSource, WorkflowJoinPolicy, WorkflowPlanNode, WorkflowPlanNodeKind,
            WorkflowRepeatExhaustion, WorkflowRepeatStop,
        };
        let mut plan = live_plan();
        plan.kind = cyril_core::types::WorkflowPlanKind::Edit;
        plan.steps = vec![
            plan_step(
                "build",
                "wf-coder",
                Some(("high", WorkflowCascadeSource::Workflow)),
                Some(("Implement the parser", false)),
            ),
            WorkflowPlanNode {
                node_id: node_id("review-loop"),
                kind: WorkflowPlanNodeKind::Repeat {
                    steps: vec![plan_step("review", "wf-reviewer", None, None)],
                    max_iterations: 3,
                    on_max_iterations: WorkflowRepeatExhaustion::Pause,
                    stop: Some(WorkflowRepeatStop::When("the review passes".to_owned())),
                },
            },
            WorkflowPlanNode {
                node_id: node_id("fanout"),
                kind: WorkflowPlanNodeKind::Parallel {
                    branches: vec![
                        plan_step("docs", "wf-writer", None, None),
                        WorkflowPlanNode {
                            node_id: node_id("watch-ci"),
                            kind: WorkflowPlanNodeKind::Watch {
                                handler: "wf-ci-watcher".to_owned(),
                                idle_timeout_sec: Some(90.0),
                                config: None,
                            },
                        },
                    ],
                    join_policy: Some(WorkflowJoinPolicy::AllSettled),
                },
            },
        ];
        let terminal = render_at(&plan_approval(plan), 90, 40, 40);
        let screen = screen_rows(&terminal).join("\n");
        for expected in [
            "Edited workflow · 3 steps after 'create-workflow'",
            "build: wf-coder · model auto · effort high (workflow)",
            "  Task: Implement the parser",
            "review-loop: repeat · max 3 iterations, then pause ·",
            "  stops when the review passes",
            "  review: wf-reviewer · model auto · effort auto",
            "fanout: parallel · join allSettled",
            "  docs: wf-writer",
            "  watch-ci: watch · handler wf-ci-watcher · ends after",
            "    90s idle",
        ] {
            assert!(screen.contains(expected), "missing {expected:?}:\n{screen}");
        }
        insta::assert_snapshot!("8wh0_workflow_plan_nested_90x40", screen);
    }

    /// A plan longer than the row cap shows the cap and how many rows it
    /// left out, and the options stay on screen.
    #[test]
    fn workflow_plan_is_bounded() {
        workflow_plan_is_bounded_case();
    }

    /// A complete comparison has its own projection after the independently
    /// bounded tree; it must not replace an ordinary-width tree with a marker.
    #[rstest::rstest]
    #[case::simple_step(
        &["s"],
        &[
            "    Edited workflow · 1 step",
            "    s: a · model auto · effort auto",
        ]
    )]
    #[case::oversized_tree(
        &[
            "s01", "s02", "s03", "s04", "s05", "s06", "s07", "s08", "s09", "s10", "s11", "s12", "s13",
            "s14", "s15",
        ],
        &[
            "    Edited workflow · 15 steps",
            "    s01: a · model auto · effort auto",
            "    s02: a · model auto · effort auto",
            "    s03: a · model auto · effort auto",
            "    s04: a · model auto · effort auto",
            "    s05: a · model auto · effort auto",
            "    s06: a · model auto · effort auto",
            "    s07: a · model auto · effort auto",
            "    s08: a · model auto · effort auto",
            "    s09: a · model auto · effort auto",
            "    s10: a · model auto · effort auto",
            "    ...5 more lines",
        ]
    )]
    fn comparison_keeps_an_independent_bounded_tree_prefix(
        #[case] step_ids: &[&str],
        #[case] expected_tree: &[&str],
    ) {
        let mut plan = live_plan();
        plan.kind = cyril_core::types::WorkflowPlanKind::Edit;
        plan.proposed_by = None;
        plan.kept_step_ids.clear();
        plan.steps = step_ids
            .iter()
            .map(|id| plan_step(id, "a", None, None))
            .collect();
        let comparison = format!("Compared with the replaced steps: {}", "x".repeat(566));
        assert_eq!(comparison.chars().count(), 600, "fixture code-point count");

        let rows: Vec<String> =
            super::super::workflow_plan::plan_lines(&plan, Some(&comparison), 40, &theme())
                .iter()
                .map(ToString::to_string)
                .collect();
        assert_eq!(&rows[..expected_tree.len()], expected_tree);
        // Worked width-40 fixture: the prefix occupies one row, followed by
        // fifteen 36-character continuations and one 26-character final row.
        let mut expected_comparison = vec!["    Compared with the replaced steps:"];
        expected_comparison.extend(["        xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"; 15]);
        expected_comparison.push("        xxxxxxxxxxxxxxxxxxxxxxxxxx");
        assert_eq!(&rows[expected_tree.len()..], expected_comparison);
    }

    /// cyril-8wh0 review: what an edit removes is only in KAS's Text
    /// ("Compared with the replaced steps: …", not in `workflowPlan`), so it
    /// is drawn under the typed plan.
    #[test]
    fn workflow_plan_shows_what_an_edit_removes() {
        let mut plan = live_plan();
        plan.kind = cyril_core::types::WorkflowPlanKind::Edit;
        let mut state = plan_approval(plan);
        state.tool_call = crate::traits::TrackedToolCall::new(plan_text_tool_call(
            "Steps to add after 'edit-workflow-r1':\n\nwrite-hello: step, agent wf-coder, model auto, effort low\n\nCompared with the replaced steps: removed 'old-step'; added 'write-hello'; changed none.",
        ));
        let terminal = render_at(&state, 90, 40, 40);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            screen.contains("Compared with the replaced steps: removed"),
            "the comparison must show:\n{screen}"
        );
        assert!(screen.contains("'old-step'"), "the removed id:\n{screen}");
        assert!(
            screen.contains("Edited workflow · 1 step"),
            "under the typed plan:\n{screen}"
        );
    }

    /// cyril-8wh0 review: step details render as KAS words them — writes,
    /// the completion condition (each part, joined with "or"), a watch's
    /// config with non-string values as JSON — and an `omitted: "details"`
    /// plan says so.
    #[test]
    fn workflow_plan_renders_step_details() {
        use cyril_core::types::{
            WorkflowCompletionSignal, WorkflowFileCheck, WorkflowPlanNode, WorkflowPlanNodeKind,
            WorkflowPlanStep, WorkflowStopCondition,
        };
        let mut plan = live_plan();
        plan.steps = vec![
            WorkflowPlanNode {
                node_id: node_id("build"),
                kind: WorkflowPlanNodeKind::Step(WorkflowPlanStep {
                    agent_name: "wf-coder".into(),
                    model_id: Some("claude-sonnet-4.5".into()),
                    model_source: Some(cyril_core::types::WorkflowCascadeSource::Parent),
                    effort_level: None,
                    effort_source: None,
                    task: None,
                    artifacts: vec![("report".into(), "out/report.md".into())],
                    completion: Some(WorkflowStopCondition {
                        contains_text: Some("DONE".into()),
                        file_check: Some(WorkflowFileCheck {
                            path: "out/s.json".into(),
                            json_path: "$.ok".into(),
                            value: serde_json::json!(true),
                        }),
                        completion_signal: Some(WorkflowCompletionSignal::Success),
                    }),
                }),
            },
            WorkflowPlanNode {
                node_id: node_id("watch-ci"),
                kind: WorkflowPlanNodeKind::Watch {
                    handler: "wf-ci-watcher".into(),
                    idle_timeout_sec: None,
                    config: serde_json::json!({"branch": "main", "retries": 3})
                        .as_object()
                        .cloned(),
                },
            },
        ];
        plan.omitted = Some(cyril_core::types::WorkflowPlanOmission::Details);
        let terminal = render_at(&plan_approval(plan), 120, 50, 50);
        let screen = screen_rows(&terminal).join("\n");
        for expected in [
            "build: wf-coder · model claude-sonnet-4.5 (parent) ·",
            "effort auto",
            "Writes: report = out/report.md",
            "Waits for the user until out/s.json $.ok=true or",
            "output contains \"DONE\" or it signals success",
            "watch-ci: watch · handler wf-ci-watcher",
            "Config: branch = main, retries = 3",
            "(step details left out to fit the plan)",
        ] {
            assert!(screen.contains(expected), "missing {expected:?}:\n{screen}");
        }
    }

    /// cyril-8wh0 review: recipe strings are model output; a bidi override
    /// or a control character in one never reaches the terminal.
    #[test]
    fn workflow_plan_strips_bidi_and_control_characters() {
        let mut plan = live_plan();
        plan.steps = vec![plan_step(
            "evil\u{202E}txt.exe",
            "wf\u{1b}[31mcoder",
            None,
            None,
        )];
        // Checked on the rows the widget hands ratatui, before ratatui's own
        // grapheme filtering could hide a leak (review round 1).
        let rows: Vec<String> =
            crate::widgets::workflow_plan::plan_lines(&plan, None, 80, &theme())
                .iter()
                .map(|line| {
                    line.spans
                        .iter()
                        .map(|span| span.content.as_ref())
                        .collect()
                })
                .collect();
        assert!(
            rows.iter()
                .all(|row| !row.contains('\u{202E}') && !row.contains('\u{1b}')),
            "raw controls left the widget: {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row.contains("evil txt.exe: wf [31mcoder")),
            "cleaned as KAS cleans (each control becomes a space): {rows:?}"
        );
        let terminal = render_at(&plan_approval(plan), 90, 40, 40);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            screen.contains("evil txt.exe: wf [31mcoder"),
            "the cleaned row is what the screen shows:\n{screen}"
        );
    }

    /// Comparison continuations must remain actual drawn text at the first
    /// usable widths. Two cells are required to display a wide glyph at all.
    #[rstest::rstest]
    #[case::ascii_one_cell(11, 'x')]
    #[case::ascii_two_cells(12, 'x')]
    #[case::ascii_three_cells(13, 'x')]
    #[case::ascii_four_cells(14, 'x')]
    #[case::ascii_five_cells(15, 'x')]
    #[case::ascii_six_cells(16, 'x')]
    #[case::wide_two_cells(12, '界')]
    #[case::wide_three_cells(13, '界')]
    #[case::wide_four_cells(14, '界')]
    #[case::wide_five_cells(15, '界')]
    #[case::wide_six_cells(16, '界')]
    fn narrow_comparison_tail_is_drawn_without_clipped_continuation_indent(
        #[case] width: u16,
        #[case] fill: char,
    ) {
        let comparison = format!(
            "Compared with the replaced steps: removed 'old'; added {}TAIL_ID",
            fill.to_string().repeat(200),
        );
        let mut plan = live_plan();
        plan.kind = cyril_core::types::WorkflowPlanKind::Edit;
        let mut state = plan_approval(plan);
        state.tool_call = crate::traits::TrackedToolCall::new(plan_text_tool_call(&comparison));
        state.preview_scroll_offset = usize::MAX;
        let rows = screen_rows(&render_at(&state, width, 50, 50));
        let body: Vec<&str> = rows
            .iter()
            .filter_map(|row| popup_body(row))
            .filter_map(|body| body.strip_prefix("  "))
            .collect();
        let drawn: String = body
            .iter()
            .flat_map(|row| row.chars().filter(|ch| !ch.is_whitespace()))
            .collect();
        assert!(
            drawn.contains("TAIL_ID"),
            "comparison tail clipped: {rows:?}"
        );
        assert!(
            drawn.contains(fill),
            "retained comparison glyph clipped: {rows:?}"
        );
        assert!(body.len() <= 12, "body budget exceeded: {rows:?}");
    }

    /// cyril-ej7y: KAS caps characters, not rows or bytes. The retained end
    /// of a long comparison must survive even when it needs far over four
    /// rows, and every rendered viewport includes at most twelve body rows.
    #[rstest::rstest]
    #[case::below(599, "TAIL_ID")]
    #[case::exact(600, "TAIL_ID")]
    #[case::over(601, "TAIL_I...")]
    fn comparison_character_bound_survives_bounded_viewports(
        #[case] chars: usize,
        #[case] expected_tail: &str,
        #[values(24, 40, 60, 80)] width: u16,
        #[values('x', '界')] fill: char,
    ) {
        let prefix = "Compared with the replaced steps: removed 'old'; added ";
        let padding = chars - prefix.chars().count() - "TAIL_ID".chars().count();
        let comparison = format!("{prefix}{}TAIL_ID", fill.to_string().repeat(padding));
        let mut plan = live_plan();
        plan.kind = cyril_core::types::WorkflowPlanKind::Edit;
        let mut state = plan_approval(plan);
        state.tool_call = crate::traits::TrackedToolCall::new(plan_text_tool_call(&comparison));
        let initial = screen_rows(&render_at(&state, width, 50, 50));
        state.preview_scroll_offset = usize::MAX;
        let final_rows = screen_rows(&render_at(&state, width, 50, 50));
        let final_body: String = final_rows
            .iter()
            .filter_map(|row| popup_body(row))
            .flat_map(|body| body.chars().filter(|ch| !ch.is_whitespace()))
            .collect();
        assert!(
            final_body.contains(expected_tail),
            "missing {expected_tail}: {final_rows:?}"
        );
        if chars > 600 {
            assert!(
                !final_body.contains("TAIL_ID"),
                "beyond KAS cap: {final_rows:?}"
            );
        }
        for rows in [&initial, &final_rows] {
            let body_rows = rows
                .iter()
                .filter_map(|row| popup_body(row))
                .filter(|body| body.starts_with("  "))
                .count();
            assert!(body_rows <= 12, "body budget exceeded: {rows:?}");
            assert!(rows.iter().any(|row| row.contains("▸ Allow")), "{rows:?}");
        }
    }

    /// cyril-8wh0 review: a plan approval without a usable typed plan falls
    /// back to its Text with the plan's row budget, not the generic five.
    #[test]
    fn plan_text_fallback_gets_the_plan_budget() {
        let ten = (1..=10)
            .map(|n| format!("plan row {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut plan = live_plan();
        plan.steps.clear();
        plan.omitted = Some(cyril_core::types::WorkflowPlanOmission::Steps);
        let mut state = plan_approval(plan);
        state.tool_call = crate::traits::TrackedToolCall::new(plan_text_tool_call(&ten));
        let screen = screen_rows(&render_at(&state, 90, 40, 40)).join("\n");
        assert!(screen.contains("plan row 10"), "all ten rows:\n{screen}");

        let mut ordinary = approval_for_tool_call(plan_text_tool_call(&ten));
        ordinary.workflow_plan = None;
        let screen = screen_rows(&render_at(&ordinary, 90, 40, 40)).join("\n");
        assert!(screen.contains("plan row 5"), "five rows:\n{screen}");
        assert!(!screen.contains("plan row 6"), "capped at five:\n{screen}");
    }

    /// cyril-ej7y: a fallback's marker occupies its last budgeted row.
    /// Exactly twelve rows fit intact; overflowing text counts the displaced
    /// twelfth row among the omitted rows. Both Text sources use that budget.
    #[rstest::rstest]
    #[case::below(11, 11, None)]
    #[case::exact(12, 12, None)]
    #[case::one_over(13, 11, Some(2))]
    #[case::well_over(25, 11, Some(14))]
    fn plan_text_fallback_marker_is_inside_twelve_rows(
        #[case] count: usize,
        #[case] kept: usize,
        #[case] omitted: Option<usize>,
        #[values(false, true)] raw_text: bool,
        #[values(24, 40, 80)] width: u16,
    ) {
        let text = (1..=count)
            .map(|n| format!("fallback {n:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut plan = live_plan();
        plan.steps.clear();
        plan.omitted = Some(cyril_core::types::WorkflowPlanOmission::Steps);
        let mut state = plan_approval(plan);
        let tool = if raw_text {
            ToolCall::new(
                cyril_core::types::ToolCallId::new("fallback"),
                "Workflow plan".into(),
                ToolKind::Other,
                ToolCallStatus::Pending,
                Some(serde_json::json!({"text": text})),
            )
        } else {
            plan_text_tool_call(&text)
        };
        state.tool_call = crate::traits::TrackedToolCall::new(tool);
        let rows = screen_rows(&render_at(&state, width, 40, 40));
        let screen = rows.join("\n");
        let content_rows = rows.iter().filter(|row| row.contains("fallback ")).count();
        let marker_rows = rows.iter().filter(|row| row.contains("...")).count();
        assert_eq!(content_rows, kept, "{screen}");
        assert_eq!(marker_rows, usize::from(omitted.is_some()), "{screen}");
        assert!(content_rows + marker_rows <= 12, "{screen}");
        if let Some(omitted) = omitted {
            assert!(screen.contains(&format!("...{omitted}")), "{screen}");
            if width >= 40 {
                assert!(
                    screen.contains(&format!("...{omitted} more lines")),
                    "{screen}"
                );
            }
        }
        assert!(screen.contains("▸ Allow"), "{screen}");
    }

    fn workflow_plan_is_bounded_case() {
        let mut plan = live_plan();
        plan.steps = (1..=20)
            .map(|n| plan_step(&format!("step-{n:02}"), "wf-coder", None, None))
            .collect();
        let terminal = render_at(&plan_approval(plan), 90, 50, 50);
        let screen = screen_rows(&terminal).join("\n");
        // 22 rows (header, keeps, 20 steps) into a 12-row budget whose last
        // row is the marker: header, keeps and steps 1-9, then the marker.
        let max = crate::widgets::workflow_plan::MAX_PLAN_ROWS;
        let last_shown = max - 3;
        assert!(
            screen.contains(&format!("step-{last_shown:02}:")),
            "rows up to the cap:\n{screen}"
        );
        assert!(
            !screen.contains(&format!("step-{:02}:", last_shown + 1)),
            "rows past the cap:\n{screen}"
        );
        assert!(
            screen.contains(&format!("...{} more lines", 22 - (max - 1))),
            "omission marker:\n{screen}"
        );
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
    }

    // ---------- cyril-q0kg: malformed-preview warning is not per-frame ----------

    const MALFORMED_PREVIEW_WARNING: &str =
        "approval preview raw_input has no string text/content field";

    /// A permission-request tool call with the given raw input, optionally
    /// carrying diff content (which takes precedence over raw input).
    fn preview_tool_call(raw_input: Option<serde_json::Value>, with_diff: bool) -> ToolCall {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_q0kg"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            raw_input,
        );
        if with_diff {
            tc.with_content(vec![ToolCallContent::Diff {
                path: "/tmp/q0kg.md".into(),
                old_text: None,
                new_text: "new body".into(),
            }])
        } else {
            tc
        }
    }

    /// Install `tc` as an approval through the production `show_approval`
    /// path, with a reject option so the feedback phase can be entered too.
    fn approval_ui_for(tc: ToolCall) -> UiState {
        let mut ui = UiState::new(500);
        ui.show_approval(cyril_core::types::PermissionRequest {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: tc,
            message: "Write File".into(),
            options: vec![option("reject", "Reject")],
            trust_options: vec![],
            can_reject_with_reason: true,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
            workflow_plan: None,
        });
        ui
    }

    fn malformed_preview_warnings(events: &[serde_json::Value]) -> Vec<&serde_json::Value> {
        events
            .iter()
            .filter(|event| event["fields"]["message"] == MALFORMED_PREVIEW_WARNING)
            .collect()
    }

    /// cyril-q0kg fence: installing an approval and rendering it for many
    /// frames — in both the option and the reject-with-reason phase — logs
    /// the malformed-shape warning at most once for the tool call (the
    /// cyril-j1b3 intent), never once per frame. Payloads that render a body
    /// (string text, diff) or carry no raw input never warn.
    #[rstest::rstest]
    #[case::non_string_text(Some(serde_json::json!({"path": "/tmp/ok.md", "text": 42})), false, 1)]
    #[case::shell_command_shape(Some(serde_json::json!({"command": "ls -la"})), false, 1)]
    #[case::string_text(Some(serde_json::json!({"text": "hello"})), false, 0)]
    #[case::string_content(Some(serde_json::json!({"content": "hello"})), false, 0)]
    #[case::no_raw_input(None, false, 0)]
    #[case::diff_takes_precedence(Some(serde_json::json!({"command": "ls"})), true, 0)]
    fn malformed_preview_warning_is_not_repeated_per_frame(
        #[case] raw_input: Option<serde_json::Value>,
        #[case] with_diff: bool,
        #[case] expected_warnings: usize,
    ) {
        let (_guard, capture, dispatch) = cyril_core::test_support::capture_json_subscriber();
        tracing::dispatcher::with_default(&dispatch, || {
            let mut ui = approval_ui_for(preview_tool_call(raw_input, with_diff));
            for _ in 0..3 {
                render_ui(&ui, 80, 24, 24);
            }
            assert!(ui.approval_begin_feedback(), "feedback phase must open");
            for _ in 0..3 {
                render_ui(&ui, 80, 24, 24);
            }
        });

        let events = capture.captured();
        let warnings = malformed_preview_warnings(&events);
        assert_eq!(
            warnings.len(),
            expected_warnings,
            "malformed-preview warnings across 6 frames: {events:?}"
        );
        for warning in warnings {
            assert_eq!(warning["level"], "WARN", "at warn level: {warning:?}");
            assert_eq!(
                warning["fields"]["tool_call_id"], "tooluse_q0kg",
                "names the tool call: {warning:?}"
            );
            assert_eq!(
                warning["fields"]["raw_input_shape"], "object",
                "names the shape, never the value: {warning:?}"
            );
        }
    }

    /// cyril-q0kg: the degraded marker still renders on every frame for a
    /// payload that warned once at install time.
    #[test]
    fn malformed_preview_marker_survives_repeated_renders() {
        let ui = approval_ui_for(preview_tool_call(
            Some(serde_json::json!({"command": "ls -la"})),
            false,
        ));
        for _ in 0..2 {
            let text = buffer_text(&render_ui(&ui, 80, 24, 24));
            assert!(
                text.contains("Preview unavailable"),
                "degraded marker missing:\n{text}"
            );
        }
    }
}

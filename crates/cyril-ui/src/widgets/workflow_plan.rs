//! The step tree of a KAS workflow plan awaiting approval (cyril-8wh0),
//! drawn in the approval overlay's preview so the user sees what a run will
//! do — and what it will bill — before any step runs.
//!
//! The wording follows KAS's own plan text (`R_o`/`Aiu`/`Iiu` in the 0.66.43
//! bundle), which Kiro's TUI shows verbatim: one row per node, `id: summary`,
//! nested nodes indented, step details beneath their step.
//!
//! Every recipe string is untrusted model output. KAS cleans the strings of
//! its own text (`Ay`/`D_o`), but `workflowPlan` carries them raw, so this
//! module cleans them the same way ([`clean`]) before they reach the
//! terminal: a bidi override in a step id must not be able to make an
//! approval read differently from what it approves.

use crate::text;
use ratatui::prelude::*;

use crate::theme::Theme;
use cyril_core::types::{
    WorkflowCascadeSource, WorkflowPlan, WorkflowPlanKind, WorkflowPlanNode, WorkflowPlanNodeKind,
    WorkflowPlanOmission, WorkflowPlanStep, WorkflowRepeatStop, WorkflowStopCondition,
};

/// Independent tree-row budget, omission markers included. Also the maximum
/// visible preview body, including its scroll status. The complete comparison
/// follows the bounded tree and may need scrolling to reach.
pub(crate) const MAX_PLAN_ROWS: usize = 12;

/// Indent before every plan row, matching the preview's text rows.
const INDENT: &str = "    ";

/// KAS's cap on one value in its plan text (`DE`).
const VALUE_CHARS: usize = 160;

/// KAS's cap on a `Writes:` or `Config:` line (`yHr`).
const LIST_CHARS: usize = 600;

/// The plan's bounded tree and complete comparison as wrapped preview rows.
/// The approval viewport shows at most [`MAX_PLAN_ROWS`] of them at a time.
///
/// `comparison` is KAS's "Compared with the replaced steps: …" summary of an
/// edit or a replacement. It is not part of `workflowPlan` — KAS puts it only
/// in the request's Text — and it is the one place the user learns which
/// steps the plan removes, so it is drawn under the steps.
pub(crate) fn plan_lines(
    plan: &WorkflowPlan,
    comparison: Option<&str>,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut rows = vec![header(plan)];
    if !plan.kept_step_ids.is_empty() {
        let kept: Vec<String> = plan
            .kept_step_ids
            .iter()
            .map(|id| clean(id.as_str(), VALUE_CHARS))
            .collect();
        rows.push(format!("Keeps: {}", kept.join(", ")));
    }
    for node in &plan.steps {
        node_rows(node, 0, &mut rows);
    }
    if plan.omitted == Some(WorkflowPlanOmission::Details) {
        rows.push("(step details left out to fit the plan)".to_owned());
    }
    let tree: Vec<String> = rows
        .iter()
        .flat_map(|row| wrap_words(row, width, 4))
        .collect();
    // Keep the tree's budget independent of the comparison. Its complete
    // cleaned 600-codepoint value follows the bounded tree without another
    // row cap; both share the bounded approval viewport.
    let tail: Vec<String> = match comparison {
        Some(comparison) => wrap_words(
            &clean(comparison, LIST_CHARS),
            width,
            4.min(width.saturating_sub(2)),
        ),
        None => Vec::new(),
    };
    let tree = cap_rows(tree, MAX_PLAN_ROWS);

    tree.into_iter()
        .chain(tail)
        .enumerate()
        .map(|(index, row)| {
            let style = if index == 0 {
                Style::default().fg(theme.text_secondary)
            } else {
                Style::default().fg(theme.subdued)
            };
            Line::styled(format!("{INDENT}{row}"), style)
        })
        .collect()
}

/// `rows` cut to at most `max` rows: the first `max` as they are, or, when
/// there are more, the first `max - 1` and a marker naming how many rows
/// were left out (so the marker takes a row of the budget; with a budget of
/// one, only the marker). A zero budget yields no rows.
fn cap_rows(mut rows: Vec<String>, max: usize) -> Vec<String> {
    if max == 0 {
        return Vec::new();
    }
    if rows.len() <= max {
        return rows;
    }
    let kept = max.saturating_sub(1);
    let omitted = rows.len() - kept;
    rows.truncate(kept);
    rows.push(format!("...{omitted} more lines"));
    rows
}

/// A recipe string as KAS's plan text shows it (`Ay` over `D_o`): control
/// characters and bidi formatting characters become spaces, whitespace runs
/// collapse to one space, the ends are trimmed, and a string longer than
/// `max_chars` is cut there and ends in `...`.
fn clean(text: &str, max_chars: usize) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut space = false;
    for ch in text.chars() {
        if ch.is_whitespace() || ch.is_control() || is_bidi_format(ch) {
            space = !cleaned.is_empty();
            continue;
        }
        if space {
            cleaned.push(' ');
            space = false;
        }
        cleaned.push(ch);
    }
    if cleaned.chars().count() <= max_chars {
        return cleaned;
    }
    let mut cut: String = cleaned.chars().take(max_chars).collect();
    cut.push_str("...");
    cut
}

/// The bidi formatting characters KAS strips (`Riu`): the Arabic letter
/// mark, LRM/RLM, the embeddings and overrides, and the isolates.
fn is_bidi_format(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// Wrap one plan row at word boundaries into rows of at most `width`
/// columns. Continuations keep the row's indent plus `continuation_indent`:
/// four for the existing tree; comparisons shrink it to leave two usable
/// columns when possible, so narrow continuation text remains reachable.
/// A word wider than a row is split by column ([`super::approval::wrap_columns`]).
/// An existing tree indent can still exceed the available width and be clipped.
fn wrap_words(row: &str, width: usize, continuation_indent: usize) -> Vec<String> {
    let body = row.trim_start_matches(' ');
    let pad = &row[..row.len() - body.len()];
    let continuation = format!("{pad}{:width$}", "", width = continuation_indent);
    let mut rows = Vec::new();
    let mut current = pad.to_owned();
    let mut has_word = false;
    for word in body.split(' ') {
        let gap = usize::from(has_word);
        if text::width(&current) + gap + text::width(word) <= width {
            if has_word {
                current.push(' ');
            }
            current.push_str(word);
            has_word = true;
            continue;
        }
        if has_word {
            rows.push(std::mem::replace(&mut current, continuation.clone()));
        }
        // Split what does not fit: the first piece fills this row, every
        // later piece a continuation row of its own.
        let mut rest = word;
        loop {
            let room = width.saturating_sub(text::width(&current)).max(1);
            let pieces = super::approval::wrap_columns(rest, room);
            let Some(first) = pieces.first() else {
                break;
            };
            if pieces.len() == 1 {
                current.push_str(first);
                break;
            }
            current.push_str(first);
            rows.push(std::mem::replace(&mut current, continuation.clone()));
            rest = &rest[first.len()..];
        }
        has_word = true;
    }
    rows.push(current);
    rows
}

/// `New workflow · 2 steps after 'create-workflow'`. The count is the
/// plan's top-level nodes, as KAS counts them in the request's title ("Add 1
/// workflow step").
fn header(plan: &WorkflowPlan) -> String {
    let label = match plan.kind {
        WorkflowPlanKind::Launch => "New workflow",
        WorkflowPlanKind::Edit => "Edited workflow",
        WorkflowPlanKind::Replace => "Replacement steps",
    };
    let count = plan.steps.len();
    let noun = if count == 1 { "step" } else { "steps" };
    match &plan.proposed_by {
        Some(after) => format!(
            "{label} · {count} {noun} after '{}'",
            clean(after.as_str(), VALUE_CHARS)
        ),
        None => format!("{label} · {count} {noun}"),
    }
}

fn node_rows(node: &WorkflowPlanNode, depth: usize, rows: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    let (summary, children): (String, &[WorkflowPlanNode]) = match &node.kind {
        WorkflowPlanNodeKind::Step(step) => (step_summary(step), &[]),
        WorkflowPlanNodeKind::Sequence {
            steps,
            included_from,
        } => (
            match included_from {
                Some(include) => format!(
                    "sequence · included from {}",
                    clean(&include.name, VALUE_CHARS)
                ),
                None => "sequence".to_owned(),
            },
            steps,
        ),
        WorkflowPlanNodeKind::Repeat {
            steps,
            max_iterations,
            on_max_iterations,
            stop,
        } => {
            let noun = if *max_iterations == 1 {
                "iteration"
            } else {
                "iterations"
            };
            let mut summary =
                format!("repeat · max {max_iterations} {noun}, then {on_max_iterations}");
            match stop {
                Some(WorkflowRepeatStop::Condition(condition)) => {
                    summary.push_str(&format!(" · stops when {}", condition_text(condition)));
                }
                Some(WorkflowRepeatStop::When(rule)) => {
                    summary.push_str(&format!(" · stops when {}", clean(rule, VALUE_CHARS)));
                }
                None => {}
            }
            (summary, steps)
        }
        WorkflowPlanNodeKind::Parallel {
            branches,
            join_policy,
        } => (
            match join_policy {
                Some(policy) => format!("parallel · join {policy}"),
                None => "parallel".to_owned(),
            },
            branches,
        ),
        WorkflowPlanNodeKind::Watch {
            handler,
            idle_timeout_sec,
            ..
        } => {
            let handler = clean(handler, VALUE_CHARS);
            (
                match idle_timeout_sec {
                    Some(seconds) => {
                        format!("watch · handler {handler} · ends after {seconds}s idle")
                    }
                    None => format!("watch · handler {handler}"),
                },
                &[],
            )
        }
    };
    rows.push(format!(
        "{pad}{}: {summary}",
        clean(node.node_id.as_str(), VALUE_CHARS)
    ));
    detail_rows(node, &pad, rows);
    for child in children {
        node_rows(child, depth + 1, rows);
    }
}

fn step_summary(step: &WorkflowPlanStep) -> String {
    // KAS prints an unresolved model or effort as `auto` — the agent's own
    // default — and so does its plan text; absence is not an error.
    format!(
        "{} · model {}{} · effort {}{}",
        clean(&step.agent_name, VALUE_CHARS),
        clean(step.model_id.as_deref().unwrap_or("auto"), VALUE_CHARS),
        source_suffix(step.model_source),
        clean(step.effort_level.as_deref().unwrap_or("auto"), VALUE_CHARS),
        source_suffix(step.effort_source),
    )
}

/// Where an inherited setting comes from; nothing for the step's own.
fn source_suffix(source: Option<WorkflowCascadeSource>) -> &'static str {
    match source {
        Some(WorkflowCascadeSource::Workflow) => " (workflow)",
        Some(WorkflowCascadeSource::Parent) => " (parent)",
        Some(WorkflowCascadeSource::Step) | None => "",
    }
}

fn detail_rows(node: &WorkflowPlanNode, pad: &str, rows: &mut Vec<String>) {
    match &node.kind {
        WorkflowPlanNodeKind::Step(step) => {
            if let Some(task) = &step.task {
                let more = if task.truncated { "…" } else { "" };
                rows.push(format!(
                    "{pad}  Task: {}{more}",
                    clean(&task.text, VALUE_CHARS)
                ));
            }
            if !step.artifacts.is_empty() {
                let writes: Vec<String> = step
                    .artifacts
                    .iter()
                    .map(|(name, path)| format!("{name} = {path}"))
                    .collect();
                rows.push(format!(
                    "{pad}  Writes: {}",
                    clean(&writes.join(", "), LIST_CHARS)
                ));
            }
            if let Some(completion) = &step.completion {
                rows.push(format!(
                    "{pad}  Waits for the user until {}",
                    condition_text(completion)
                ));
            }
        }
        WorkflowPlanNodeKind::Watch {
            config: Some(config),
            ..
        } if !config.is_empty() => {
            let entries: Vec<String> = config
                .iter()
                .map(|(key, value)| format!("{key} = {}", value_text(value)))
                .collect();
            rows.push(format!(
                "{pad}  Config: {}",
                clean(&entries.join(", "), LIST_CHARS)
            ));
        }
        _ => {}
    }
}

/// A condition as KAS words it (`A_o`): its parts joined with `or`.
fn condition_text(condition: &WorkflowStopCondition) -> String {
    let mut parts = Vec::new();
    if let Some(check) = &condition.file_check {
        parts.push(format!(
            "{} {}={}",
            clean(&check.path, VALUE_CHARS),
            clean(&check.json_path, VALUE_CHARS),
            clean(&value_text(&check.value), VALUE_CHARS)
        ));
    }
    if let Some(text) = &condition.contains_text {
        parts.push(format!("output contains \"{}\"", clean(text, VALUE_CHARS)));
    }
    if let Some(signal) = condition.completion_signal {
        parts.push(format!("it signals {signal}"));
    }
    parts.join(" or ")
}

/// A JSON value as text: a string as-is, anything else as JSON.
fn value_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plan rows wrap at spaces; continuation rows keep the row's indent
    /// plus four columns; a word wider than a row is split by column; no
    /// row is wider than `width` while the indent leaves room.
    #[rstest::rstest]
    #[case::fits("  a: b c", 20, &["  a: b c"])]
    #[case::wraps_at_space("a: one two three", 9, &["a: one", "    two", "    three"])]
    #[case::keeps_indent("  x: alpha beta", 10, &["  x: alpha", "      beta"])]
    #[case::splits_long_word("t: abcdefghij", 7, &["t:", "    abc", "    def", "    ghi", "    j"])]
    #[case::splits_leading_long_word("abcdefghij: x", 6, &["abcdef", "    gh", "    ij", "    :", "    x"])]
    #[case::halfwidth_cells_wrap("a: ｶﾞ Z", 6, &["a: ｶﾞ", "    Z"])]
    #[case::combining_mark_still_fits("a: ｶ\u{3099} Z", 6, &["a: ｶ\u{3099} Z"])]
    #[case::empty("", 5, &[""])]
    fn wrap_words_wraps_at_spaces(
        #[case] row: &str,
        #[case] width: usize,
        #[case] expected: &[&str],
    ) {
        let rows = wrap_words(row, width, 4);
        assert_eq!(rows, expected);
        for row in &rows {
            assert!(text::width(row) <= width, "{row:?} wider than {width}");
        }
    }

    /// Literal rows and emitted cell coordinates distinguish physical halfwidth
    /// sound marks from ordinary combining marks, including long-word splits.
    #[rstest::rstest]
    #[case::voiced_current(
        "ｶﾞ x yy",
        &["ｶﾞ x", "    yy"],
        &[(0, 0, "ｶﾞ"), (3, 0, "x"), (4, 1, "y"), (5, 1, "y")]
    )]
    #[case::semivoiced_current(
        "ﾊﾟ x yy",
        &["ﾊﾟ x", "    yy"],
        &[(0, 0, "ﾊﾟ"), (3, 0, "x"), (4, 1, "y"), (5, 1, "y")]
    )]
    #[case::combining_voiced_current(
        "ｶ\u{3099} x yy",
        &["ｶ\u{3099} x yy"],
        &[(0, 0, "ｶ\u{3099}"), (2, 0, "x"), (4, 0, "y"), (5, 0, "y")]
    )]
    #[case::combining_semivoiced_current(
        "ﾊ\u{309A} x yy",
        &["ﾊ\u{309A} x yy"],
        &[(0, 0, "ﾊ\u{309A}"), (2, 0, "x"), (4, 0, "y"), (5, 0, "y")]
    )]
    #[case::voiced_next_word(
        "xxxx ｶﾞ",
        &["xxxx", "    ｶﾞ"],
        &[(0, 0, "x"), (1, 0, "x"), (2, 0, "x"), (3, 0, "x"), (4, 1, "ｶﾞ")]
    )]
    #[case::semivoiced_next_word(
        "xxxx ﾊﾟ",
        &["xxxx", "    ﾊﾟ"],
        &[(0, 0, "x"), (1, 0, "x"), (2, 0, "x"), (3, 0, "x"), (4, 1, "ﾊﾟ")]
    )]
    #[case::combining_voiced_next_word(
        "xxxx ｶ\u{3099}",
        &["xxxx ｶ\u{3099}"],
        &[(0, 0, "x"), (1, 0, "x"), (2, 0, "x"), (3, 0, "x"), (5, 0, "ｶ\u{3099}")]
    )]
    #[case::combining_semivoiced_next_word(
        "xxxx ﾊ\u{309A}",
        &["xxxx ﾊ\u{309A}"],
        &[(0, 0, "x"), (1, 0, "x"), (2, 0, "x"), (3, 0, "x"), (5, 0, "ﾊ\u{309A}")]
    )]
    #[case::voiced_long_word(
        "ｶﾞabcde",
        &["ｶﾞabcd", "    e"],
        &[(0, 0, "ｶﾞ"), (2, 0, "a"), (3, 0, "b"), (4, 0, "c"), (5, 0, "d"), (4, 1, "e")]
    )]
    #[case::semivoiced_long_word(
        "ﾊﾟabcde",
        &["ﾊﾟabcd", "    e"],
        &[(0, 0, "ﾊﾟ"), (2, 0, "a"), (3, 0, "b"), (4, 0, "c"), (5, 0, "d"), (4, 1, "e")]
    )]
    #[case::combining_voiced_long_word(
        "ｶ\u{3099}abcde",
        &["ｶ\u{3099}abcde"],
        &[(0, 0, "ｶ\u{3099}"), (1, 0, "a"), (2, 0, "b"), (3, 0, "c"), (4, 0, "d"), (5, 0, "e")]
    )]
    #[case::combining_semivoiced_long_word(
        "ﾊ\u{309A}abcde",
        &["ﾊ\u{309A}abcde"],
        &[(0, 0, "ﾊ\u{309A}"), (1, 0, "a"), (2, 0, "b"), (3, 0, "c"), (4, 0, "d"), (5, 0, "e")]
    )]
    fn wrap_words_matches_released_cell_columns_and_diff(
        #[case] row: &str,
        #[case] expected_rows: &[&str],
        #[case] expected_cells: &[(u16, u16, &str)],
    ) {
        let rows = wrap_words(row, 6, 4);
        assert_eq!(rows, expected_rows);

        let area = Rect::new(0, 0, 6, 2);
        let previous = Buffer::empty(area);
        let mut painted = previous.clone();
        for (y, row) in rows.iter().enumerate() {
            painted.set_line(0, y as u16, &Line::raw(row.as_str()), 6);
        }
        let diff = previous.diff(&painted);
        let cells: Vec<_> = diff
            .iter()
            .map(|(x, y, cell)| (*x, *y, cell.symbol()))
            .collect();
        assert_eq!(cells, expected_cells);
    }

    /// Comparisons use the adaptive three-column continuation at width five,
    /// while the inherited tree continues to use its four-column indent.
    #[rstest::rstest]
    #[case::voiced(
        "ｶﾞ x y",
        &["    ｶﾞ x", "       y"],
        &[(4, 0, "ｶﾞ"), (7, 0, "x"), (7, 1, "y")]
    )]
    #[case::semivoiced(
        "ﾊﾟ x y",
        &["    ﾊﾟ x", "       y"],
        &[(4, 0, "ﾊﾟ"), (7, 0, "x"), (7, 1, "y")]
    )]
    #[case::combining_voiced(
        "ｶ\u{3099} x y",
        &["    ｶ\u{3099} x y"],
        &[(4, 0, "ｶ\u{3099}"), (6, 0, "x"), (8, 0, "y")]
    )]
    #[case::combining_semivoiced(
        "ﾊ\u{309A} x y",
        &["    ﾊ\u{309A} x y"],
        &[(4, 0, "ﾊ\u{309A}"), (6, 0, "x"), (8, 0, "y")]
    )]
    fn comparison_tail_matches_released_cell_columns_and_diff(
        #[case] comparison: &str,
        #[case] expected_rows: &[&str],
        #[case] expected_cells: &[(u16, u16, &str)],
    ) {
        let plan = WorkflowPlan {
            workflow_id: cyril_core::types::WorkflowId::try_from("wf_6b22f0c99f407f0e".to_owned())
                .expect("workflow id"),
            proposed_by: None,
            kind: WorkflowPlanKind::Edit,
            workflow_ref: "generated://gen_09d8668976ecd154".to_owned(),
            kept_step_ids: Vec::new(),
            steps: Vec::new(),
            omitted: None,
        };
        let theme = crate::theme::resolve(
            crate::theme::ThemeId::CyrilDark,
            crate::theme::ColorMode::TrueColor,
        );
        let lines = plan_lines(&plan, Some(comparison), 5, &theme);
        let tail = &lines[lines.len() - expected_rows.len()..];
        let rows: Vec<_> = tail.iter().map(ToString::to_string).collect();
        assert_eq!(rows, expected_rows);

        let area = Rect::new(0, 0, 9, 2);
        let mut previous = Buffer::empty(area);
        previous.set_style(area, Style::default().fg(theme.subdued));
        let mut painted = previous.clone();
        for (y, line) in tail.iter().enumerate() {
            painted.set_line(0, y as u16, line, 9);
        }
        let diff = previous.diff(&painted);
        let cells: Vec<_> = diff
            .iter()
            .map(|(x, y, cell)| (*x, *y, cell.symbol()))
            .collect();
        assert_eq!(cells, expected_cells);
    }

    /// `cap_rows` keeps everything that fits; otherwise the marker takes the
    /// budget's last row and names every row it replaced.
    #[rstest::rstest]
    #[case::fits(3, 3, &["r1", "r2", "r3"])]
    #[case::cut(5, 3, &["r1", "r2", "...3 more lines"])]
    #[case::marker_only(5, 1, &["...5 more lines"])]
    #[case::no_rows(0, 0, &[])]
    #[case::zero_budget(5, 0, &[])]
    fn cap_rows_marks_what_it_cuts(
        #[case] count: usize,
        #[case] max: usize,
        #[case] expected: &[&str],
    ) {
        let rows: Vec<String> = (1..=count).map(|n| format!("r{n}")).collect();
        assert_eq!(cap_rows(rows, max), expected);
    }

    /// Recipe strings are cleaned as KAS cleans its own plan text: controls
    /// and bidi formatting characters never reach the terminal, whitespace
    /// collapses, and an overlong value is cut.
    #[rstest::rstest]
    #[case::plain("write-hello", 160, "write-hello")]
    #[case::bidi_override("evil\u{202E}txt.exe", 160, "evil txt.exe")]
    #[case::isolate_and_mark("a\u{2066}b\u{200F}c\u{061C}d", 160, "a b c d")]
    #[case::controls_and_newlines("line1\nline2\t\u{1b}[31mred", 160, "line1 line2 [31mred")]
    #[case::trims_ends("  padded  ", 160, "padded")]
    #[case::cut("abcdefghij", 4, "abcd...")]
    fn clean_strips_controls_and_bidi(
        #[case] text: &str,
        #[case] max_chars: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(clean(text, max_chars), expected);
    }
}

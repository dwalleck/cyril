//! The step tree of a KAS workflow plan awaiting approval (cyril-8wh0),
//! drawn in the approval overlay's preview so the user sees what a run will
//! do — and what it will bill — before any step runs.
//!
//! The wording follows KAS's own plan text (`R_o`/`Aiu`/`Iiu` in the 0.66.43
//! bundle), which Kiro's TUI shows verbatim: one row per node, `id: summary`,
//! nested nodes indented, step details beneath their step.

use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;
use cyril_core::types::{
    WorkflowCascadeSource, WorkflowPlan, WorkflowPlanKind, WorkflowPlanNode, WorkflowPlanNodeKind,
    WorkflowPlanOmission, WorkflowPlanStep, WorkflowRepeatStop, WorkflowStopCondition,
};

/// Rows of plan the preview shows before the omission marker. Larger than
/// the five-row text cap: a plan is what the user is approving, so every
/// step of an ordinary plan should be on screen.
pub(crate) const MAX_PLAN_ROWS: usize = 12;

/// Indent before every plan row, matching the preview's text rows.
const INDENT: &str = "    ";

/// The plan as preview rows of at most `width` columns (after the indent),
/// capped at [`MAX_PLAN_ROWS`] plus an omission marker.
pub(crate) fn plan_lines(plan: &WorkflowPlan, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let mut rows = vec![header(plan)];
    if !plan.kept_step_ids.is_empty() {
        let kept: Vec<&str> = plan.kept_step_ids.iter().map(|id| id.as_str()).collect();
        rows.push(format!("Keeps: {}", kept.join(", ")));
    }
    for node in &plan.steps {
        node_rows(node, 0, &mut rows);
    }
    if plan.omitted == Some(WorkflowPlanOmission::Details) {
        rows.push("(step details left out to fit the plan)".to_owned());
    }

    let wrapped: Vec<String> = rows.iter().flat_map(|row| wrap_words(row, width)).collect();
    let total = wrapped.len();
    let mut lines: Vec<Line<'static>> = wrapped
        .into_iter()
        .take(MAX_PLAN_ROWS)
        .enumerate()
        .map(|(index, row)| {
            let style = if index == 0 {
                Style::default().fg(theme.text_secondary)
            } else {
                Style::default().fg(theme.subdued)
            };
            Line::styled(format!("{INDENT}{row}"), style)
        })
        .collect();
    if total > MAX_PLAN_ROWS {
        lines.push(Line::styled(
            format!("{INDENT}...{} more lines", total - MAX_PLAN_ROWS),
            Style::default().fg(theme.subdued),
        ));
    }
    lines
}

/// Wrap one plan row at word boundaries into rows of at most `width`
/// columns. Continuation rows keep the row's indent plus two columns, so a
/// wrapped node still reads as part of its level; a word wider than a row is
/// split by column ([`super::approval::wrap_columns`]).
fn wrap_words(row: &str, width: usize) -> Vec<String> {
    let body = row.trim_start_matches(' ');
    let pad = &row[..row.len() - body.len()];
    let continuation = format!("{pad}  ");
    let mut rows = Vec::new();
    let mut current = pad.to_owned();
    let mut has_word = false;
    for word in body.split(' ') {
        let gap = usize::from(has_word);
        if current.width() + gap + word.width() <= width {
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
        let room = width.saturating_sub(current.width()).max(1);
        let mut pieces = super::approval::wrap_columns(word, room);
        // `wrap_columns` yields at least one row, even for an empty word.
        let Some(last) = pieces.pop() else {
            continue;
        };
        for piece in pieces {
            rows.push(format!("{current}{piece}"));
            current.clone_from(&continuation);
        }
        current.push_str(&last);
        has_word = true;
    }
    rows.push(current);
    rows
}

/// `New workflow · 2 steps after 'create-workflow'`.
fn header(plan: &WorkflowPlan) -> String {
    let label = match plan.kind {
        WorkflowPlanKind::Launch => "New workflow",
        WorkflowPlanKind::Edit => "Edited workflow",
        WorkflowPlanKind::Replace => "Replacement steps",
    };
    let count = plan.steps.len();
    let noun = if count == 1 { "step" } else { "steps" };
    match &plan.proposed_by {
        Some(after) => format!("{label} · {count} {noun} after '{after}'"),
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
                Some(include) => format!("sequence · included from {}", include.name),
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
                    summary.push_str(&format!(" · stops when {rule}"));
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
        } => (
            match idle_timeout_sec {
                Some(seconds) => format!("watch · handler {handler} · ends after {seconds}s idle"),
                None => format!("watch · handler {handler}"),
            },
            &[],
        ),
    };
    rows.push(format!("{pad}{}: {summary}", node.node_id));
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
        step.agent_name,
        step.model_id.as_deref().unwrap_or("auto"),
        source_suffix(step.model_source),
        step.effort_level.as_deref().unwrap_or("auto"),
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
                rows.push(format!("{pad}  Task: {}{more}", task.text));
            }
            if !step.artifacts.is_empty() {
                let writes: Vec<String> = step
                    .artifacts
                    .iter()
                    .map(|(name, path)| format!("{name} = {path}"))
                    .collect();
                rows.push(format!("{pad}  Writes: {}", writes.join(", ")));
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
            rows.push(format!("{pad}  Config: {}", entries.join(", ")));
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
            check.path,
            check.json_path,
            value_text(&check.value)
        ));
    }
    if let Some(text) = &condition.contains_text {
        parts.push(format!("output contains \"{text}\""));
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
    /// plus two columns; a word wider than a row is split by column; no
    /// row is ever wider than `width`.
    #[rstest::rstest]
    #[case::fits("  a: b c", 20, &["  a: b c"])]
    #[case::wraps_at_space("a: one two three", 9, &["a: one", "  two", "  three"])]
    #[case::keeps_indent("  x: alpha beta", 10, &["  x: alpha", "    beta"])]
    #[case::splits_long_word("t: abcdefghij", 6, &["t:", "  abcd", "  efgh", "  ij"])]
    #[case::empty("", 5, &[""])]
    fn wrap_words_wraps_at_spaces(
        #[case] row: &str,
        #[case] width: usize,
        #[case] expected: &[&str],
    ) {
        let rows = wrap_words(row, width);
        assert_eq!(rows, expected);
        for row in &rows {
            assert!(row.width() <= width, "{row:?} wider than {width}");
        }
    }
}

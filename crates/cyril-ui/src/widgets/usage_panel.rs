use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::theme::Theme;
use crate::traits::{UsagePage, UsagePanelState};

const MAX_DATA_ROWS: usize = 18;

/// Render the input-protected `/usage` modal.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &UsagePanelState,
    theme: &Theme,
) {
    let desired_rows = state.row_count().clamp(1, MAX_DATA_ROWS) as u16;
    let Some(popup_area) =
        crate::widgets::modal::place(area, input_top, 110, desired_rows.saturating_add(5))
    else {
        return;
    };
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(
            format!(" /usage · {} ", state.page.title()),
            Style::default()
                .fg(theme.accent_quinary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_quinary));

    let mut lines = vec![Line::from(vec![
        Span::styled(" ←/→ or Tab pages ", Style::default().fg(theme.subdued)),
        Span::styled(
            "· ↑/↓ scroll · Esc close",
            Style::default().fg(theme.subdued),
        ),
    ])];
    let data = page_lines(state, theme);
    let visible_rows = (popup_area.height as usize).saturating_sub(4);
    let end = state
        .scroll_offset
        .saturating_add(visible_rows)
        .min(data.len());
    lines.extend(data.into_iter().take(end).skip(state.scroll_offset));
    frame.render_widget(Paragraph::new(lines).block(block), popup_area);
}

fn page_lines(state: &UsagePanelState, theme: &Theme) -> Vec<Line<'static>> {
    match state.page {
        UsagePage::Overview => overview_lines(state, theme),
        UsagePage::Costs => cost_lines(state, theme),
        UsagePage::Providers => named_group_lines(&state.snapshot.providers, theme),
        UsagePage::Models => state
            .snapshot
            .models
            .iter()
            .map(|group| {
                let name = identity(group.provider.as_deref(), group.model.as_deref());
                summary_line(name, &group.summary, theme)
            })
            .collect(),
        UsagePage::Tools => state
            .snapshot
            .tools
            .iter()
            .map(|group| {
                let tokens = group
                    .total_tokens_share
                    .map(|value| format!("{value:.0}"))
                    .unwrap_or_else(|| "—".to_owned());
                Line::from(vec![
                    Span::styled(
                        format!("{:<14}", tool_kind_label(group.kind)),
                        Style::default().fg(theme.accent_violet),
                    ),
                    Span::styled(
                        format!(
                            "{:>6} calls  {:>4} errors  {:>10} tokens  {}",
                            group.calls,
                            group.errors,
                            tokens,
                            format_costs(&group.costs)
                        ),
                        Style::default().fg(theme.text_secondary),
                    ),
                ])
            })
            .collect(),
        UsagePage::Recent => state
            .snapshot
            .recent
            .iter()
            .map(|record| recent_line(record, theme, false))
            .collect(),
        UsagePage::Errors => state
            .snapshot
            .errors
            .iter()
            .map(|record| recent_line(record, theme, true))
            .collect(),
        UsagePage::Folders => named_group_lines(&state.snapshot.folders, theme),
    }
}

fn overview_lines(state: &UsagePanelState, theme: &Theme) -> Vec<Line<'static>> {
    let summary = &state.snapshot.overview;
    if summary.requests == 0 {
        return vec![Line::styled(
            "No usage recorded yet",
            Style::default().fg(theme.subdued),
        )];
    }
    let tokens = summary.tokens.as_ref();
    vec![
        metric_line("Requests", summary.requests.to_string(), theme),
        metric_line("Errors", summary.errors.to_string(), theme),
        metric_line(
            "Uncached input",
            optional_u64(tokens.map(|value| value.input)),
            theme,
        ),
        metric_line(
            "Output tokens",
            optional_u64(tokens.map(|value| value.output)),
            theme,
        ),
        metric_line(
            "Cache read",
            optional_u64(tokens.map(|value| value.cached_read)),
            theme,
        ),
        metric_line(
            "Cache write",
            optional_u64(tokens.map(|value| value.cached_write)),
            theme,
        ),
        metric_line(
            "Cache rate",
            optional_f64(summary.cache_rate.map(|value| value * 100.0), "%", 1),
            theme,
        ),
        metric_line(
            "Average TTFT",
            optional_f64(summary.avg_ttft_ms, "ms", 1),
            theme,
        ),
        metric_line(
            "Average duration",
            optional_f64(summary.avg_duration_ms, "ms", 1),
            theme,
        ),
        metric_line(
            "Output speed",
            optional_f64(summary.avg_tokens_per_second, " tok/s", 1),
            theme,
        ),
        metric_line("Cost", format_costs(&summary.costs), theme),
    ]
}

fn cost_lines(state: &UsagePanelState, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = state
        .snapshot
        .overview
        .costs
        .iter()
        .map(|cost| {
            metric_line(
                &format!("Total {}", cost.currency()),
                format_money(cost),
                theme,
            )
        })
        .collect();
    lines.extend(
        state
            .snapshot
            .models
            .iter()
            .filter(|group| !group.summary.costs.is_empty())
            .map(|group| {
                summary_line(
                    identity(group.provider.as_deref(), group.model.as_deref()),
                    &group.summary,
                    theme,
                )
            }),
    );
    if lines.is_empty() {
        lines.push(Line::styled(
            "No cost supplied by the agent",
            Style::default().fg(theme.subdued),
        ));
    }
    lines
}

fn named_group_lines(
    groups: &[cyril_core::types::NamedUsageGroup],
    theme: &Theme,
) -> Vec<Line<'static>> {
    if groups.is_empty() {
        return vec![Line::styled(
            "No data for this breakdown",
            Style::default().fg(theme.subdued),
        )];
    }
    groups
        .iter()
        .map(|group| {
            summary_line(
                group.name.clone().unwrap_or_else(|| "—".to_owned()),
                &group.summary,
                theme,
            )
        })
        .collect()
}

fn summary_line(
    name: String,
    summary: &cyril_core::types::UsageSummary,
    theme: &Theme,
) -> Line<'static> {
    let tokens = summary
        .tokens
        .as_ref()
        .map(|value| value.total.to_string())
        .unwrap_or_else(|| "—".to_owned());
    Line::from(vec![
        Span::styled(
            format!("{name:<30}"),
            Style::default().fg(theme.accent_violet),
        ),
        Span::styled(
            format!(
                "{:>6} req  {:>10} tokens  {:>8} err  {}",
                summary.requests,
                tokens,
                summary.errors,
                format_costs(&summary.costs)
            ),
            Style::default().fg(theme.text_secondary),
        ),
    ])
}

fn recent_line(
    record: &cyril_core::types::RecentUsage,
    theme: &Theme,
    show_error: bool,
) -> Line<'static> {
    let identity = identity(record.provider.as_deref(), record.model.as_deref());
    let detail = if show_error {
        record.error.clone().unwrap_or_else(|| "—".to_owned())
    } else {
        let tokens = record
            .tokens
            .as_ref()
            .map(|value| value.total().to_string())
            .unwrap_or_else(|| "—".to_owned());
        format!(
            "{tokens} tokens  {}ms  {}",
            record.duration_ms,
            record
                .cost
                .as_ref()
                .map(format_money)
                .unwrap_or_else(|| "—".to_owned())
        )
    };
    Line::from(vec![
        Span::styled(
            format!("{}  {identity:<26}  ", record.timestamp_ms),
            Style::default().fg(theme.subdued),
        ),
        Span::styled(
            detail,
            Style::default().fg(if show_error {
                theme.danger
            } else {
                theme.text_secondary
            }),
        ),
    ])
}

fn metric_line(label: &str, value: String, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<20}"), Style::default().fg(theme.subdued)),
        Span::styled(value, Style::default().fg(theme.text_secondary)),
    ])
}

fn optional_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_owned())
}

fn optional_f64(value: Option<f64>, suffix: &str, precision: usize) -> String {
    value
        .map(|value| format!("{value:.precision$}{suffix}"))
        .unwrap_or_else(|| "—".to_owned())
}

fn format_costs(costs: &[cyril_core::types::Money]) -> String {
    if costs.is_empty() {
        return "—".to_owned();
    }
    costs
        .iter()
        .map(format_money)
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_money(money: &cyril_core::types::Money) -> String {
    if money.currency() == "USD" {
        format!("${:.4}", money.amount())
    } else {
        format!("{:.4} {}", money.amount(), money.currency())
    }
}

fn identity(provider: Option<&str>, model: Option<&str>) -> String {
    match (provider, model) {
        (Some(provider), Some(model)) => format!("{provider}/{model}"),
        (None, Some(model)) => model.to_owned(),
        (Some(provider), None) => provider.to_owned(),
        (None, None) => "—".to_owned(),
    }
}

fn tool_kind_label(kind: cyril_core::types::ToolKind) -> &'static str {
    match kind {
        cyril_core::types::ToolKind::Read => "Read",
        cyril_core::types::ToolKind::Write => "Write",
        cyril_core::types::ToolKind::Execute => "Execute",
        cyril_core::types::ToolKind::Search => "Search",
        cyril_core::types::ToolKind::Think => "Think",
        cyril_core::types::ToolKind::Fetch => "Fetch",
        cyril_core::types::ToolKind::SwitchMode => "Switch mode",
        cyril_core::types::ToolKind::Other => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyril_core::types::{
        MetricCoverage, Money, NamedUsageGroup, TokenTotals, ToolKind, ToolUsageGroup,
        UsageSnapshot, UsageSummary,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn sample_snapshot() -> UsageSnapshot {
        let cost = match Money::try_new(0.125, "USD") {
            Ok(cost) => cost,
            Err(error) => panic!("valid cost: {error}"),
        };
        let summary = UsageSummary {
            requests: 2,
            successes: 1,
            cancelled: 0,
            errors: 1,
            provider_requests: None,
            retries: None,
            tokens: Some(TokenTotals {
                total: 250,
                input: 150,
                output: 60,
                thought: 0,
                cached_read: 40,
                cached_write: 0,
            }),
            token_coverage: MetricCoverage {
                observed: 2,
                unreported: 0,
                backend_gated: 0,
            },
            costs: vec![cost],
            cost_coverage: MetricCoverage {
                observed: 2,
                unreported: 0,
                backend_gated: 0,
            },
            charges: Vec::new(),
            cache_rate: Some(0.25),
            avg_duration_ms: Some(150.0),
            avg_ttft_ms: Some(25.0),
            avg_tokens_per_second: Some(200.0),
        };
        UsageSnapshot {
            overview: summary.clone(),
            providers: vec![NamedUsageGroup {
                name: Some("openai-codex".into()),
                summary: summary.clone(),
            }],
            models: vec![cyril_core::types::ModelUsageGroup {
                provider: Some("openai-codex".into()),
                model: Some("gpt-5.6-luna".into()),
                summary: summary.clone(),
            }],
            folders: vec![NamedUsageGroup {
                name: Some("/tmp/space and 日本語".repeat(20)),
                summary: summary.clone(),
            }],
            agent_types: Vec::new(),
            tools: vec![ToolUsageGroup {
                kind: ToolKind::Read,
                calls: 2,
                errors: 0,
                total_tokens_share: Some(250.0),
                output_tokens_share: Some(60.0),
                costs: summary.costs.clone(),
            }],
            recent: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn all_pages_render_and_clamp_above_input() {
        let theme = crate::traits::test_support::marker_theme();
        for page in UsagePage::ALL {
            for (width, height) in [(80, 24), (30, 10)] {
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                let input_top = height.saturating_sub(3);
                let state = UsagePanelState {
                    snapshot: sample_snapshot(),
                    page,
                    scroll_offset: 0,
                };
                terminal
                    .draw(|frame| {
                        frame.render_widget(
                            Paragraph::new("INPUT_MARKER"),
                            Rect::new(0, input_top, width, 1),
                        );
                        render(frame, frame.area(), input_top, &state, &theme);
                    })
                    .expect("draw usage page");
                let rendered = rows(&terminal);
                assert!(
                    rendered.iter().any(|row| row.contains(page.title())),
                    "missing page heading {page:?} at {width}x{height}"
                );
                assert!(
                    rendered[input_top as usize].contains("INPUT_MARKER"),
                    "usage modal covered input for {page:?} at {width}x{height}"
                );
            }
        }
    }

    #[test]
    fn empty_snapshot_renders_truthful_placeholder() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let state = UsagePanelState {
            snapshot: UsageSnapshot::default(),
            page: UsagePage::Overview,
            scroll_offset: 0,
        };
        let theme = crate::traits::test_support::marker_theme();
        terminal
            .draw(|frame| render(frame, frame.area(), 20, &state, &theme))
            .expect("draw empty usage");
        assert!(
            rows(&terminal)
                .iter()
                .any(|row| row.contains("No usage recorded yet"))
        );
    }

    #[test]
    #[ignore = "reference-workstation render budget"]
    fn usage_panel_render_budget_reference() {
        let backend = TestBackend::new(240, 80);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let state = UsagePanelState {
            snapshot: sample_snapshot(),
            page: UsagePage::Overview,
            scroll_offset: 0,
        };
        let theme = crate::traits::test_support::marker_theme();
        let started = std::time::Instant::now();
        for _ in 0..1_000 {
            terminal
                .draw(|frame| render(frame, frame.area(), 76, &state, &theme))
                .expect("draw usage budget frame");
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed <= std::time::Duration::from_secs(5),
            "1,000 usage frames exceeded 5ms/frame: {elapsed:?}"
        );
    }
}

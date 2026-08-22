use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::theme::Theme;
use crate::traits::{UsageAccountStatus, UsagePage, UsagePanelState};

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
        UsagePage::Context => context_lines(state, theme),
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
        UsagePage::Tools => tool_lines(state, theme),
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
        metric_line("Turns", summary.requests.to_string(), theme),
        metric_line("Successes", summary.successes.to_string(), theme),
        metric_line("Cancelled", summary.cancelled.to_string(), theme),
        metric_line("Errors", summary.errors.to_string(), theme),
        metric_line(
            "Provider requests",
            optional_u64(summary.provider_requests),
            theme,
        ),
        metric_line("Retries", optional_u64(summary.retries), theme),
        metric_line(
            "Uncached input",
            token_metric(tokens.map(|value| value.input), &summary.token_coverage),
            theme,
        ),
        metric_line(
            "Output tokens",
            token_metric(tokens.map(|value| value.output), &summary.token_coverage),
            theme,
        ),
        metric_line(
            "Cache read",
            token_metric(
                tokens.map(|value| value.cached_read),
                &summary.token_coverage,
            ),
            theme,
        ),
        metric_line(
            "Cache write",
            token_metric(
                tokens.map(|value| value.cached_write),
                &summary.token_coverage,
            ),
            theme,
        ),
        metric_line(
            "Cache rate",
            metric_f64(
                summary.cache_rate.map(|value| value * 100.0),
                "%",
                1,
                &summary.token_coverage,
            ),
            theme,
        ),
        metric_line(
            "Cache savings",
            metric_f64(None, "", 1, &summary.token_coverage),
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
            metric_f64(
                summary.avg_tokens_per_second,
                " tok/s",
                1,
                &summary.token_coverage,
            ),
            theme,
        ),
        metric_line("Credits", format_charges(&summary.charges), theme),
        metric_line(
            "Monetary cost",
            monetary_metric(summary, &summary.costs),
            theme,
        ),
    ]
}

fn cost_lines(state: &UsagePanelState, theme: &Theme) -> Vec<Line<'static>> {
    let summary = &state.snapshot.overview;
    let mut lines = vec![
        metric_line("Credits", format_charges(&summary.charges), theme),
        metric_line(
            "Monetary cost",
            monetary_metric(summary, &summary.costs),
            theme,
        ),
    ];
    lines.extend(
        state
            .snapshot
            .models
            .iter()
            .filter(|group| !group.summary.costs.is_empty() || !group.summary.charges.is_empty())
            .map(|group| {
                summary_line(
                    identity(group.provider.as_deref(), group.model.as_deref()),
                    &group.summary,
                    theme,
                )
            }),
    );
    lines.push(metric_line(
        "Account status",
        account_status(&state.account_status, state.account_fetched_at_ms),
        theme,
    ));
    if let Some(account) = state.account.as_ref() {
        lines.push(metric_line("Plan", account.plan_name.clone(), theme));
        lines.push(metric_line(
            "Billing reset",
            account.billing_cycle_reset.clone(),
            theme,
        ));
        lines.push(metric_line(
            "Overages",
            if account.overages_enabled {
                "enabled".to_owned()
            } else {
                "disabled".to_owned()
            },
            theme,
        ));
        for breakdown in &account.usage_breakdowns {
            lines.push(metric_line(
                &breakdown.display_name,
                format!(
                    "{:.2}/{:.2} ({:.1}%) · overage {:.2} @ {:.4} {}",
                    breakdown.used,
                    breakdown.limit,
                    breakdown.percentage,
                    breakdown.current_overages,
                    breakdown.overage_rate,
                    breakdown.currency
                ),
                theme,
            ));
        }
        for bonus in &account.bonus_credits {
            lines.push(metric_line(
                &bonus.name,
                format!(
                    "{:.2}/{:.2} · {}d remaining",
                    bonus.used, bonus.total, bonus.days_until_expiry
                ),
                theme,
            ));
        }
    }
    lines
}

fn context_lines(state: &UsagePanelState, theme: &Theme) -> Vec<Line<'static>> {
    let context = &state.snapshot.context;
    let Some(latest) = context.latest.as_ref() else {
        return vec![Line::styled(
            "No context usage supplied by the agent",
            Style::default().fg(theme.subdued),
        )];
    };
    let mut lines = vec![metric_line(
        "Latest context",
        format!("{:.1}%", latest.percentage),
        theme,
    )];
    if let Some(breakdown) = latest.breakdown.as_ref() {
        for (label, bucket) in [
            ("Context files", breakdown.context_files()),
            ("Session files", breakdown.session_files()),
            ("Tools", breakdown.tools()),
            ("Your prompts", breakdown.your_prompts()),
            ("Kiro responses", breakdown.kiro_responses()),
        ] {
            lines.push(metric_line(
                label,
                format!("{} tokens · {:.1}%", bucket.tokens(), bucket.percent()),
                theme,
            ));
        }
    } else {
        lines.push(metric_line("Breakdown", "n/a".to_owned(), theme));
    }
    lines.push(metric_line(
        "Compactions",
        context.compactions.to_string(),
        theme,
    ));
    lines.push(metric_line(
        "Sampled gains",
        context.sampled_compactions.to_string(),
        theme,
    ));
    lines.push(metric_line(
        "Total reduction",
        optional_f64(context.total_reduction_percentage_points, " pp", 1),
        theme,
    ));
    lines.push(metric_line(
        "Average reduction",
        optional_f64(context.average_reduction_percentage_points, " pp", 1),
        theme,
    ));
    lines
}

fn tool_lines(state: &UsagePanelState, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for group in &state.snapshot.tools {
        let name = group
            .name
            .clone()
            .unwrap_or_else(|| tool_kind_label(group.kind).to_owned());
        let error_rate = if group.calls == 0 {
            0.0
        } else {
            group.errors as f64 * 100.0 / group.calls as f64
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{name:<20}"),
                Style::default().fg(theme.accent_violet),
            ),
            Span::styled(
                format!(
                    "{} calls · {:.1}% err · {} · args {} · result {} · last {}",
                    group.calls,
                    error_rate,
                    format_charges(&group.charges),
                    optional_u64(group.argument_chars),
                    optional_u64(group.result_chars),
                    group.last_used_ms
                ),
                Style::default().fg(theme.text_secondary),
            ),
        ]));
        for model in &group.models {
            lines.push(Line::styled(
                format!(
                    "  ↳ {} · {} calls · {} errors",
                    identity(model.provider.as_deref(), model.model.as_deref()),
                    model.calls,
                    model.errors
                ),
                Style::default().fg(theme.subdued),
            ));
        }
    }
    if lines.is_empty() {
        lines.push(Line::styled(
            "No tool usage recorded",
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
    let tokens = token_metric(
        summary.tokens.as_ref().map(|value| value.total),
        &summary.token_coverage,
    );
    Line::from(vec![
        Span::styled(
            format!("{name:<30}"),
            Style::default().fg(theme.accent_violet),
        ),
        Span::styled(
            format!(
                "{} turns · {tokens} tokens · {} err · {} · {}",
                summary.requests,
                summary.errors,
                format_charges(&summary.charges),
                monetary_metric(summary, &summary.costs)
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
        let tokens = match record.tokens.as_ref() {
            Some(value) => value.total().to_string(),
            None if record.token_unavailable_reason.is_some() => "n/a (backend-gated)".to_owned(),
            None => "—".to_owned(),
        };
        let cost = match record.cost.as_ref() {
            Some(cost) => format_money(cost),
            None if record.cost_unavailable_reason.is_some() => "n/a (backend-gated)".to_owned(),
            None => "—".to_owned(),
        };
        format!(
            "{tokens} tokens · {}ms · {} · {cost}",
            record.duration_ms,
            format_charges(&record.charges)
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

fn token_metric(value: Option<u64>, coverage: &cyril_core::types::MetricCoverage) -> String {
    match value {
        Some(value) => value.to_string(),
        None if coverage.unavailable_reason().is_some() => "n/a (backend-gated)".to_owned(),
        None => "—".to_owned(),
    }
}

fn metric_f64(
    value: Option<f64>,
    suffix: &str,
    precision: usize,
    coverage: &cyril_core::types::MetricCoverage,
) -> String {
    match value {
        Some(value) => format!("{value:.precision$}{suffix}"),
        None if coverage.unavailable_reason().is_some() => "n/a (backend-gated)".to_owned(),
        None => "—".to_owned(),
    }
}

fn monetary_metric(
    summary: &cyril_core::types::UsageSummary,
    costs: &[cyril_core::types::Money],
) -> String {
    if costs.is_empty() && summary.cost_coverage.unavailable_reason().is_some() {
        "n/a (backend-gated)".to_owned()
    } else {
        format_costs(costs)
    }
}

fn format_charges(charges: &[cyril_core::types::MeteredAmount]) -> String {
    if charges.is_empty() {
        return "—".to_owned();
    }
    charges
        .iter()
        .map(|charge| {
            let unit = if (charge.amount() - 1.0).abs() < f64::EPSILON {
                charge.unit()
            } else {
                charge.unit_plural()
            };
            format!("{:.4} {unit}", charge.amount())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn account_status(status: &UsageAccountStatus, fetched_at_ms: Option<u64>) -> String {
    let fetched = fetched_at_ms
        .map(|timestamp| format!(" · fetched {timestamp}"))
        .unwrap_or_default();
    match status {
        UsageAccountStatus::Idle => "not queried".to_owned(),
        UsageAccountStatus::Loading => "loading".to_owned(),
        UsageAccountStatus::Fresh => format!("fresh{fetched}"),
        UsageAccountStatus::Unavailable(message) => format!("unavailable: {message}"),
        UsageAccountStatus::Stale(message) => format!("stale{fetched}: {message}"),
    }
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
                name: None,
                kind: ToolKind::Read,
                calls: 2,
                errors: 0,
                argument_chars: Some(12),
                result_chars: Some(24),
                last_used_ms: 1,
                total_tokens_share: Some(250.0),
                output_tokens_share: Some(60.0),
                costs: summary.costs.clone(),
                charges: Vec::new(),
                models: Vec::new(),
            }],
            context: Default::default(),
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
                let mut terminal = match Terminal::new(backend) {
                    Ok(terminal) => terminal,
                    Err(error) => panic!("test terminal: {error}"),
                };
                let input_top = height.saturating_sub(3);
                let state = UsagePanelState {
                    snapshot: sample_snapshot(),
                    page,
                    scroll_offset: 0,
                    account: None,
                    account_fetched_at_ms: None,
                    account_status: UsageAccountStatus::Idle,
                };
                match terminal.draw(|frame| {
                    frame.render_widget(
                        Paragraph::new("INPUT_MARKER"),
                        Rect::new(0, input_top, width, 1),
                    );
                    render(frame, frame.area(), input_top, &state, &theme);
                }) {
                    Ok(_) => {}
                    Err(error) => panic!("draw usage page: {error}"),
                }
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
        let mut terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => panic!("test terminal: {error}"),
        };
        let state = UsagePanelState {
            snapshot: UsageSnapshot::default(),
            page: UsagePage::Overview,
            scroll_offset: 0,
            account: None,
            account_fetched_at_ms: None,
            account_status: UsageAccountStatus::Idle,
        };
        let theme = crate::traits::test_support::marker_theme();
        match terminal.draw(|frame| render(frame, frame.area(), 20, &state, &theme)) {
            Ok(_) => {}
            Err(error) => panic!("draw empty usage: {error}"),
        }
        assert!(
            rows(&terminal)
                .iter()
                .any(|row| row.contains("No usage recorded yet"))
        );
    }
    #[test]
    fn kiro_full_mixed_pages_render_at_floor() {
        let mut snapshot = sample_snapshot();
        snapshot.overview.tokens = None;
        snapshot.overview.token_coverage = MetricCoverage {
            observed: 0,
            unreported: 0,
            backend_gated: 2,
        };
        snapshot.overview.costs.clear();
        snapshot.overview.cost_coverage = MetricCoverage {
            observed: 0,
            unreported: 0,
            backend_gated: 2,
        };
        snapshot.overview.charges = vec![
            cyril_core::types::MeteredAmount::try_new(0.25, "credit", "credits")
                .expect("valid credit"),
        ];
        snapshot.context = cyril_core::types::UsageContextSummary {
            latest: Some(cyril_core::types::UsageContextSample {
                context: cyril_core::types::TurnUsageContext::new(
                    cyril_core::types::SessionId::new("sess"),
                    "/tmp",
                    Some("anthropic/claude"),
                    cyril_core::types::UsageAgentType::Main,
                ),
                timestamp_ms: 1,
                percentage: 42.0,
                breakdown: Some(cyril_core::types::ContextBreakdown::new(
                    cyril_core::types::ContextBucket::new(1, 1.0),
                    cyril_core::types::ContextBucket::new(2, 2.0),
                    cyril_core::types::ContextBucket::new(3, 3.0),
                    cyril_core::types::ContextBucket::new(4, 4.0),
                    cyril_core::types::ContextBucket::new(5, 5.0),
                )),
            }),
            compactions: 1,
            sampled_compactions: 1,
            total_reduction_percentage_points: Some(30.0),
            average_reduction_percentage_points: Some(30.0),
        };
        snapshot.tools[0].name = Some("read_file".to_owned());
        snapshot.tools[0].charges = snapshot.overview.charges.clone();
        snapshot.tools[0].models = vec![cyril_core::types::ToolModelUsageGroup {
            provider: Some("anthropic".to_owned()),
            model: Some("claude".to_owned()),
            calls: 2,
            errors: 0,
        }];
        let account = cyril_core::types::UsageAccount {
            plan_name: "KIRO PRO MAX".to_owned(),
            billing_cycle_reset: "2026-09-01".to_owned(),
            overages_enabled: false,
            is_enterprise: false,
            usage_breakdowns: vec![cyril_core::types::UsageAccountBreakdown {
                resource_type: "CREDIT".to_owned(),
                display_name: "Credits".to_owned(),
                used: 10.0,
                limit: 100.0,
                percentage: 10.0,
                current_overages: 0.0,
                overage_rate: 0.04,
                overage_charges: Some(0.0),
                currency: "USD".to_owned(),
            }],
            bonus_credits: Vec::new(),
        };
        let theme = crate::traits::test_support::marker_theme();
        for (page, needles) in [
            (
                UsagePage::Overview,
                ["n/a (backend-gated)", "0.2500 credits"].as_slice(),
            ),
            (
                UsagePage::Costs,
                ["KIRO PRO MAX", "Monetary cost"].as_slice(),
            ),
            (
                UsagePage::Context,
                ["Latest context", "Context files"].as_slice(),
            ),
            (
                UsagePage::Tools,
                ["read_file", "anthropic/claude"].as_slice(),
            ),
        ] {
            let state = UsagePanelState {
                snapshot: snapshot.clone(),
                page,
                scroll_offset: 0,
                account: Some(account.clone()),
                account_fetched_at_ms: Some(42),
                account_status: UsageAccountStatus::Fresh,
            };
            let backend = TestBackend::new(120, 40);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| render(frame, frame.area(), 36, &state, &theme))
                .expect("draw Kiro usage page");
            let rendered = rows(&terminal).join("\n");
            for needle in needles {
                assert!(
                    rendered.contains(needle),
                    "{page:?} missing {needle:?}\n{rendered}"
                );
            }
            assert!(
                !rendered.contains("credits credits"),
                "charge unit rendered twice\n{rendered}"
            );
            if page == UsagePage::Overview {
                assert!(
                    rendered.lines().any(|line| {
                        line.contains("Uncached input") && line.contains("n/a (backend-gated)")
                    }),
                    "token placeholder is not explicit\n{rendered}"
                );
            }
        }
    }

    #[test]
    #[ignore = "reference-workstation render budget"]
    fn usage_panel_render_budget_reference() {
        let mut snapshot = sample_snapshot();
        snapshot.models = (0..10_000)
            .map(|index| cyril_core::types::ModelUsageGroup {
                provider: Some("provider".to_owned()),
                model: Some(format!("model-{index}")),
                summary: UsageSummary::default(),
            })
            .collect();
        let backend = TestBackend::new(120, 40);
        let mut terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => panic!("test terminal: {error}"),
        };
        let state = UsagePanelState {
            snapshot,
            page: UsagePage::Models,
            scroll_offset: 0,
            account: None,
            account_fetched_at_ms: None,
            account_status: UsageAccountStatus::Idle,
        };
        let theme = crate::traits::test_support::marker_theme();
        let started = std::time::Instant::now();
        match terminal.draw(|frame| render(frame, frame.area(), 36, &state, &theme)) {
            Ok(_) => {}
            Err(error) => panic!("draw usage budget frame: {error}"),
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed <= std::time::Duration::from_millis(16),
            "10,000-group usage frame exceeded 16ms: {elapsed:?}"
        );
    }
}

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::theme::Theme;
use crate::traits::{UsageAccountStatus, UsagePage, UsagePanelState, UsageRefreshStatus};

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
            format!(
                " /usage · {}{} ",
                state.page.title(),
                refresh_suffix(&state.refresh)
            ),
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

/// The panel-wide refresh status as it appears in the title.
///
/// Empty when idle: a marker that is always present says nothing. The failure
/// reason is truncated so a long database error cannot push the page name off
/// the border.
fn refresh_suffix(status: &UsageRefreshStatus) -> String {
    const REASON_BUDGET: usize = 40;
    match status {
        UsageRefreshStatus::Idle => String::new(),
        UsageRefreshStatus::Computing => " · computing…".to_owned(),
        UsageRefreshStatus::Refreshing => " · refreshing…".to_owned(),
        UsageRefreshStatus::Failed(reason) => {
            let mut shown: String = reason.chars().take(REASON_BUDGET).collect();
            if reason.chars().count() > REASON_BUDGET {
                shown.push('…');
            }
            format!(" · refresh failed: {shown}")
        }
    }
}

fn page_lines(state: &UsagePanelState, theme: &Theme) -> Vec<Line<'static>> {
    // Nothing has been computed yet. Deliberately NOT the "no usage recorded"
    // placeholder: an empty log and an unfinished first snapshot are different
    // facts and must not render the same (cyril-nanu S11).
    if state.refresh == UsageRefreshStatus::Computing {
        return vec![Line::styled(
            "Computing usage…",
            Style::default().fg(theme.subdued),
        )];
    }
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
            "Average TTFT",
            optional_f64(summary.avg_ttft_ms, "ms", 1),
            theme,
        ),
        metric_line(
            "TTFT tail",
            latency_tail(summary.p90_ttft_ms, summary.max_ttft_ms),
            theme,
        ),
        metric_line(
            "Average duration",
            optional_f64(summary.avg_duration_ms, "ms", 1),
            theme,
        ),
        metric_line(
            "Duration tail",
            latency_tail(summary.p90_duration_ms, summary.max_duration_ms),
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
        let plan = if account.is_enterprise {
            format!("{} · enterprise", account.plan_name)
        } else {
            account.plan_name.clone()
        };
        lines.push(metric_line("Plan", plan, theme));
        lines.push(metric_line(
            "Billing reset",
            account.billing_cycle_reset.clone(),
            theme,
        ));
        let overages = match (account.overages_enabled, account.overage_capable) {
            (true, _) => "enabled",
            (false, true) => "disabled · available",
            (false, false) => "unavailable",
        };
        lines.push(metric_line("Overages", overages.to_owned(), theme));
        for breakdown in &account.usage_breakdowns {
            let quota = if breakdown.has_limit {
                format!(
                    "{:.2}/{:.2} ({:.1}%)",
                    breakdown.used, breakdown.limit, breakdown.percentage
                )
            } else {
                format!("{:.2} used · no plan limit", breakdown.used)
            };
            let billed = breakdown.overage_charges.map_or_else(
                || "charge —".to_owned(),
                |charge| format!("charge {charge:.2} {}", breakdown.currency),
            );
            lines.push(metric_line(
                &format!("Account {}", breakdown.display_name),
                format!(
                    "{} · {quota} · overage {:.2} @ {:.4} {} · {billed}",
                    breakdown.resource_type,
                    breakdown.current_overages,
                    breakdown.overage_rate,
                    breakdown.currency
                ),
                theme,
            ));
        }
        for (index, add_on) in account.add_on_credits.iter().enumerate() {
            let status = if add_on.is_active {
                "active".to_owned()
            } else {
                "inactive".to_owned()
            };
            let expiry = add_on
                .expires_at
                .as_deref()
                .map_or_else(String::new, |expiry| format!(" · expires {expiry}"));
            lines.push(metric_line(
                &format!("Add-on credits {}", index + 1),
                format!("{:.2}/{:.2} · {status}{expiry}", add_on.used, add_on.total),
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
                    "{} calls · {:.1}% err · {} · {} · {} · args {} · result {} · last {}",
                    group.calls,
                    error_rate,
                    optional_f64(group.total_tokens_share, " tokens", 0),
                    format_costs(&group.costs),
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
                "{} turns · {tokens} tokens · {} err · {} · {} · p90 {}",
                summary.requests,
                summary.errors,
                format_charges(&summary.charges),
                monetary_metric(summary, &summary.costs),
                optional_f64(summary.p90_duration_ms, "ms", 0)
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

/// The latency tail for one metric: the nearest-rank p90 next to the largest
/// observed value. Both come from `cyril-core` already computed; this renders
/// them and derives nothing (see `tests/no_percentile_computation.rs`).
///
/// Collapses to a single "—" when neither was observed, rather than printing
/// "— p90 · — max", so absence reads as absence.
fn latency_tail(p90: Option<f64>, max: Option<f64>) -> String {
    if p90.is_none() && max.is_none() {
        return "—".to_owned();
    }
    format!(
        "{} p90 · {} max",
        optional_f64(p90, "ms", 0),
        optional_f64(max, "ms", 0)
    )
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
        UsageAccountStatus::Refreshing => format!("refreshing{fetched}"),
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
            p90_duration_ms: Some(180.0),
            max_duration_ms: Some(240.0),
            p90_ttft_ms: Some(40.0),
            max_ttft_ms: Some(55.0),
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
                    refresh: UsageRefreshStatus::Idle,
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
            refresh: UsageRefreshStatus::Idle,
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
        let credit = match cyril_core::types::MeteredAmount::try_new(0.25, "credit", "credits") {
            Ok(credit) => credit,
            Err(error) => panic!("valid credit: {error}"),
        };
        let money = match cyril_core::types::Money::try_new(0.125, "USD") {
            Ok(money) => money,
            Err(error) => panic!("valid money: {error}"),
        };
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
        snapshot.overview.charges = vec![credit];
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
        snapshot.tools[0].total_tokens_share = Some(250.0);
        snapshot.tools[0].costs = vec![money];
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
            is_enterprise: true,
            overage_capable: true,
            usage_breakdowns: vec![cyril_core::types::UsageAccountBreakdown {
                resource_type: "CREDIT".to_owned(),
                display_name: "Credits".to_owned(),
                used: 10.0,
                has_limit: false,
                limit: 999_999.0,
                percentage: 10.0,
                current_overages: 2.0,
                overage_rate: 0.04,
                overage_charges: Some(0.08),
                currency: "USD".to_owned(),
            }],
            bonus_credits: Vec::new(),
            add_on_credits: vec![cyril_core::types::UsageAddOnCredit {
                used: 2.0,
                total: 100.0,
                is_active: true,
                expires_at: Some("2026-10-01".to_owned()),
            }],
        };
        let theme = crate::traits::test_support::marker_theme();
        for (page, needles) in [
            (
                UsagePage::Overview,
                ["n/a (backend-gated)", "0.2500 credits"].as_slice(),
            ),
            (
                UsagePage::Costs,
                [
                    "KIRO PRO MAX · enterprise",
                    "Monetary cost",
                    "CREDIT · 10.00 used · no plan limit",
                    "charge 0.08 USD",
                    "Add-on credits 1",
                ]
                .as_slice(),
            ),
            (
                UsagePage::Context,
                ["Latest context", "Context files"].as_slice(),
            ),
            (
                UsagePage::Tools,
                ["read_file", "anthropic/claude", "250 tokens", "$0.1250"].as_slice(),
            ),
        ] {
            let state = UsagePanelState {
                snapshot: snapshot.clone(),
                refresh: UsageRefreshStatus::Idle,
                page,
                scroll_offset: 0,
                account: Some(account.clone()),
                account_fetched_at_ms: Some(42),
                account_status: UsageAccountStatus::Fresh,
            };
            assert_eq!(state.row_count(), page_lines(&state, &theme).len());
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
    fn sparse_cost_and_context_row_counts_match_rendered_lines() {
        let theme = crate::traits::test_support::marker_theme();
        for page in [UsagePage::Costs, UsagePage::Context] {
            let state = UsagePanelState {
                snapshot: cyril_core::types::UsageSnapshot::default(),
                refresh: UsageRefreshStatus::Idle,
                page,
                scroll_offset: 0,
                account: None,
                account_fetched_at_ms: None,
                account_status: UsageAccountStatus::Loading,
            };
            assert_eq!(state.row_count(), page_lines(&state, &theme).len());
        }
    }

    #[test]
    fn overview_metric_availability_is_provider_invariant() {
        let render_provider = |provider: &str| {
            let mut snapshot = sample_snapshot();
            snapshot.providers[0].name = Some(provider.to_owned());
            snapshot.overview.tokens = None;
            snapshot.overview.token_coverage = MetricCoverage {
                observed: 0,
                unreported: 0,
                backend_gated: 2,
            };
            let state = UsagePanelState {
                snapshot,
                refresh: UsageRefreshStatus::Idle,
                page: UsagePage::Overview,
                scroll_offset: 0,
                account: None,
                account_fetched_at_ms: None,
                account_status: UsageAccountStatus::Idle,
            };
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("test terminal");
            let theme = crate::traits::test_support::marker_theme();
            terminal
                .draw(|frame| render(frame, frame.area(), 36, &state, &theme))
                .expect("draw provider overview");
            rows(&terminal)
        };
        assert_eq!(render_provider("kiro"), render_provider("omp"));
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
            refresh: UsageRefreshStatus::Idle,
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

    /// The Overview page sizes its modal from a hardcoded `row_count()`, so
    /// that constant can silently drift from the lines actually rendered —
    /// and adding the two latency-tail lines is exactly the edit that does it.
    #[test]
    fn overview_row_count_matches_rendered_lines() {
        let theme = crate::traits::test_support::marker_theme();
        let state = UsagePanelState {
            snapshot: sample_snapshot(),
            refresh: UsageRefreshStatus::Idle,
            page: UsagePage::Overview,
            scroll_offset: 0,
            account: None,
            account_fetched_at_ms: None,
            account_status: UsageAccountStatus::Idle,
        };
        assert_eq!(
            state.row_count(),
            page_lines(&state, &theme).len(),
            "Overview row_count must equal the lines it renders"
        );
    }

    /// C11 — the p90/max fields reach the screen.
    ///
    /// They were computed by `cyril-core` and then dropped on the floor: this
    /// widget rendered only `avg_ttft_ms` and `avg_duration_ms`, so every
    /// `/usage` open and every panel refresh paid for statistics no user could
    /// see, and the latency-tail invisibility the work set out to fix stayed
    /// invisible (cyril-9kyk review). Asserted on rendered characters, not on
    /// the struct, because the struct was never the part that was missing.
    #[test]
    fn latency_tail_is_rendered_on_overview_and_grouped_pages() {
        let theme = crate::traits::test_support::marker_theme();
        let render_page = |page: UsagePage| -> String {
            let backend = TestBackend::new(120, 30);
            let mut terminal = match Terminal::new(backend) {
                Ok(terminal) => terminal,
                Err(error) => panic!("test terminal: {error}"),
            };
            let state = UsagePanelState {
                snapshot: sample_snapshot(),
                refresh: UsageRefreshStatus::Idle,
                page,
                scroll_offset: 0,
                account: None,
                account_fetched_at_ms: None,
                account_status: UsageAccountStatus::Idle,
            };
            match terminal.draw(|frame| render(frame, frame.area(), 27, &state, &theme)) {
                Ok(_) => {}
                Err(error) => panic!("draw usage page: {error}"),
            }
            rows(&terminal).join("\n")
        };

        let overview = render_page(UsagePage::Overview);
        for needle in ["180ms p90", "240ms max", "40ms p90", "55ms max"] {
            assert!(
                overview.contains(needle),
                "overview must render {needle}; got:\n{overview}"
            );
        }

        // Folders is omitted on purpose: its fixture name is a deliberate
        // width-stress string (`.repeat(20)`), so every row on that page is
        // clipped by design and would prove nothing about the p90 segment.
        for page in [UsagePage::Models, UsagePage::Providers] {
            let rendered = render_page(page);
            assert!(
                rendered.contains("p90 180ms"),
                "{page:?} rows must carry the duration p90; got:\n{rendered}"
            );
        }
    }

    /// Absence stays legible: a summary with no observed latency renders one
    /// "—", not "— p90 · — max".
    #[test]
    fn absent_latency_tail_collapses_to_a_single_dash() {
        assert_eq!(latency_tail(None, None), "—");
        assert_eq!(latency_tail(Some(12.0), None), "12ms p90 · — max");
    }

    fn render_with(refresh: UsageRefreshStatus, snapshot: UsageSnapshot) -> String {
        let theme = crate::traits::test_support::marker_theme();
        let backend = TestBackend::new(120, 30);
        let mut terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => panic!("test terminal: {error}"),
        };
        let state = UsagePanelState {
            snapshot,
            refresh,
            page: UsagePage::Overview,
            scroll_offset: 0,
            account: None,
            account_fetched_at_ms: None,
            account_status: UsageAccountStatus::Idle,
        };
        match terminal.draw(|frame| render(frame, frame.area(), 27, &state, &theme)) {
            Ok(_) => {}
            Err(error) => panic!("draw usage panel: {error}"),
        }
        rows(&terminal).join("\n")
    }

    /// cyril-nanu C4 — each refresh state renders its own marker and no other.
    ///
    /// The panel may show values a turn behind the log; that is only
    /// acceptable because it says so. A marker that is always present, or one
    /// that never appears, both fail the contract, so every state asserts both
    /// what must appear AND what must not — the absence assertions carry their
    /// own positive control that way.
    #[test]
    fn refresh_marker_matches_panel_state() {
        let computing = render_with(UsageRefreshStatus::Computing, UsageSnapshot::default());
        assert!(
            computing.contains("computing"),
            "the computing state must say so; got:\n{computing}"
        );
        assert!(
            !computing.contains("refreshing"),
            "the computing state must not claim to be refreshing; got:\n{computing}"
        );
        assert!(
            !computing.contains("No usage recorded yet"),
            "an unfinished first snapshot must NOT render the empty-log placeholder — \
             they are different facts; got:\n{computing}"
        );

        let refreshing = render_with(UsageRefreshStatus::Refreshing, sample_snapshot());
        assert!(
            refreshing.contains("refreshing"),
            "an in-flight recompute must be visible; got:\n{refreshing}"
        );
        assert!(
            refreshing.contains("Turns"),
            "the held values stay on screen while refreshing; got:\n{refreshing}"
        );

        let idle = render_with(UsageRefreshStatus::Idle, sample_snapshot());
        assert!(
            !idle.contains("refreshing") && !idle.contains("computing"),
            "an idle panel must carry no marker at all; got:\n{idle}"
        );

        // Positive control for the empty-log case: a COMPLETED snapshot over an
        // empty log renders the placeholder, which is what makes the computing
        // assertion above meaningful rather than vacuous.
        let empty_done = render_with(UsageRefreshStatus::Idle, UsageSnapshot::default());
        assert!(
            empty_done.contains("No usage recorded yet"),
            "a completed snapshot over an empty log renders the placeholder; got:\n{empty_done}"
        );
    }

    /// cyril-nanu C6 — a failed refresh keeps the values and states the failure.
    ///
    /// Today the failure is a `tracing::warn!` the operator never sees, so the
    /// panel silently shows stale numbers. The discriminating assertion is the
    /// status text: a handler that ignored errors entirely would also leave the
    /// values intact.
    #[test]
    fn failed_refresh_keeps_values_and_states_the_failure() {
        let failed = render_with(
            UsageRefreshStatus::Failed("database is locked".to_owned()),
            sample_snapshot(),
        );
        assert!(
            failed.contains("refresh failed"),
            "the failure must be stated on screen; got:\n{failed}"
        );
        assert!(
            failed.contains("database is locked"),
            "the reason must be shown, not swallowed; got:\n{failed}"
        );
        assert!(
            failed.contains("Turns"),
            "the last successful values stay on screen; got:\n{failed}"
        );

        // A long reason is truncated rather than pushing the page name off the
        // border.
        let long = "x".repeat(300);
        let truncated = render_with(UsageRefreshStatus::Failed(long), sample_snapshot());
        assert!(
            truncated.contains("Overview"),
            "a long failure reason must not displace the page name; got:\n{truncated}"
        );
    }
}

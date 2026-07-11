use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use unicode_width::UnicodeWidthStr;

use crate::cache::HashCache;
use crate::highlight;
use crate::text;
use crate::theme::Theme;

const MAX_BORDER_WIDTH: usize = 120;

static MARKDOWN_CACHE: LazyLock<Mutex<HashCache<Vec<Line<'static>>>>> =
    LazyLock::new(|| Mutex::new(HashCache::new(256)));

/// Convert a Markdown string into styled ratatui lines.
///
/// `width` controls table sizing, code padding, rules, and borders. Results are
/// cached by content, width, syntax component, and all 29 semantic colors.
pub fn render_with_theme(markdown: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let hash = markdown_cache_key(markdown, width, theme);

    if let Ok(cache) = MARKDOWN_CACHE.lock()
        && let Some(cached) = cache.get(hash)
    {
        return cached.clone();
    }

    let result = do_render(markdown, width, theme);

    if let Ok(mut cache) = MARKDOWN_CACHE.lock() {
        cache.insert(hash, result.clone());
    }

    result
}

fn markdown_cache_key(markdown: &str, width: usize, theme: &Theme) -> u64 {
    let mut hasher = DefaultHasher::new();
    markdown.hash(&mut hasher);
    width.hash(&mut hasher);
    theme.syntax.map(|syntax| syntax.name()).hash(&mut hasher);
    for color in [
        theme.canvas,
        theme.chrome,
        theme.code,
        theme.selection,
        theme.text,
        theme.muted,
        theme.border,
        theme.accent,
        theme.accent_alt,
        theme.user,
        theme.agent,
        theme.system,
        theme.info,
        theme.success,
        theme.warning,
        theme.danger,
        theme.diff_add,
        theme.diff_delete,
        theme.diff_context,
        theme.emphasis,
        theme.accent_tertiary,
        theme.accent_quaternary,
        theme.accent_quinary,
        theme.subdued,
        theme.subdued_positive,
        theme.subdued_negative,
        theme.soft_accent,
        theme.positive_accent,
        theme.inset_background,
    ] {
        color.hash(&mut hasher);
    }
    hasher.finish()
}

fn do_render(markdown: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(markdown, options);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut code_block_lang: Option<String> = None;
    let mut list_depth: usize = 0;
    let mut in_blockquote = false;
    let mut in_table = false;
    let mut table_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut is_table_header = false;
    // Buffered table: (is_header, cells) per row. Rendered on TagEnd::Table.
    let mut table_rows: Vec<(bool, Vec<String>)> = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => {
                let base = current_style(&style_stack);
                match tag {
                    Tag::Heading { level, .. } => {
                        flush_line(&mut lines, &mut current_spans);
                        let heading_style = match level as u8 {
                            1 => base
                                .fg(theme.accent_quinary)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                            2 => base.fg(theme.accent_quinary).add_modifier(Modifier::BOLD),
                            3 => base.fg(theme.text).add_modifier(Modifier::BOLD),
                            _ => base.fg(theme.text).add_modifier(Modifier::BOLD),
                        };
                        style_stack.push(heading_style);
                    }
                    Tag::Strong => {
                        style_stack.push(base.add_modifier(Modifier::BOLD));
                    }
                    Tag::Emphasis => {
                        style_stack.push(base.add_modifier(Modifier::ITALIC));
                    }
                    Tag::Strikethrough => {
                        style_stack.push(base.add_modifier(Modifier::CROSSED_OUT));
                    }
                    Tag::CodeBlock(kind) => {
                        in_code_block = true;
                        code_block_content.clear();
                        code_block_lang = match &kind {
                            CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                                Some(lang.to_string())
                            }
                            _ => None,
                        };
                        flush_line(&mut lines, &mut current_spans);
                        let border_style = Style::default().fg(theme.subdued);
                        let bg = theme.code;
                        let mut header_spans: Vec<Span<'static>> = Vec::new();
                        // Border fills to available width (capped at 120).
                        let border_width = width.min(MAX_BORDER_WIDTH);
                        match &code_block_lang {
                            Some(lang) => {
                                // Truncate language tag if it would overflow the
                                // border: "╭─── " (5) + lang + " ─" (2) = 7 min.
                                let max_lang = border_width.saturating_sub(7);
                                let display_lang = text::truncate(lang, max_lang);
                                header_spans.push(Span::styled("╭─── ", border_style));
                                header_spans.push(Span::styled(
                                    display_lang.clone(),
                                    Style::default()
                                        .fg(theme.accent_quinary)
                                        .add_modifier(Modifier::BOLD),
                                ));
                                let lang_cols = display_lang.width();
                                let fill_len = border_width.saturating_sub(lang_cols + 6).max(1);
                                header_spans.push(Span::styled(
                                    format!(" {}", "─".repeat(fill_len)),
                                    border_style,
                                ));
                            }
                            None => {
                                header_spans.push(Span::styled(
                                    "╭".to_string() + &"─".repeat(border_width.saturating_sub(1)),
                                    border_style,
                                ));
                            }
                        };
                        let mut header_line = Line::from(header_spans);
                        header_line.style = Style::default().bg(bg);
                        lines.push(header_line);
                        style_stack.push(Style::default().fg(theme.text).bg(theme.code));
                    }
                    Tag::List(_) => {
                        list_depth += 1;
                    }
                    Tag::Item => {
                        flush_line(&mut lines, &mut current_spans);
                        let indent = "  ".repeat(list_depth.saturating_sub(1));
                        current_spans.push(Span::styled(
                            format!("{indent}• "),
                            Style::default().fg(theme.accent_quinary),
                        ));
                    }
                    Tag::Table(_) => {
                        flush_line(&mut lines, &mut current_spans);
                        in_table = true;
                    }
                    Tag::TableHead => {
                        is_table_header = true;
                        table_row.clear();
                    }
                    Tag::TableRow => {
                        table_row.clear();
                    }
                    Tag::TableCell => {
                        current_cell.clear();
                    }
                    Tag::BlockQuote(_) => {
                        in_blockquote = true;
                        style_stack.push(base.fg(theme.subdued).add_modifier(Modifier::ITALIC));
                    }
                    Tag::Link { .. } => {
                        style_stack.push(
                            base.fg(theme.accent_tertiary)
                                .add_modifier(Modifier::UNDERLINED),
                        );
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough => {
                    style_stack.pop();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    style_stack.pop();

                    let code = code_block_content.trim_end_matches('\n');
                    let highlighted = highlight::highlight_block_with_theme(
                        code,
                        code_block_lang.as_deref(),
                        theme,
                    );
                    let bg = theme.code;

                    for spans in highlighted {
                        let mut line_spans = vec![Span::styled(
                            "│ ".to_string(),
                            Style::default().fg(theme.subdued),
                        )];
                        for (style, text) in spans {
                            line_spans.push(Span::styled(text, style));
                        }
                        let mut line = Line::from(line_spans);
                        line.style = Style::default().bg(bg);
                        lines.push(line);
                    }

                    let border_width = width.min(MAX_BORDER_WIDTH);
                    let mut footer_line = Line::from(Span::styled(
                        "╰".to_string() + &"─".repeat(border_width.saturating_sub(1)),
                        Style::default().fg(theme.subdued),
                    ));
                    footer_line.style = Style::default().bg(theme.code);
                    lines.push(footer_line);
                }
                TagEnd::Paragraph => {
                    flush_line(&mut lines, &mut current_spans);
                    lines.push(Line::from(""));
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    if list_depth == 0 {
                        lines.push(Line::from(""));
                    }
                }
                TagEnd::Item => {
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::Table => {
                    in_table = false;
                    let col_count = table_rows.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
                    let mut col_widths = vec![0usize; col_count];
                    for (_, row) in &table_rows {
                        for (i, cell) in row.iter().enumerate() {
                            if i < col_count {
                                col_widths[i] = col_widths[i].max(cell.width());
                            }
                        }
                    }

                    // Shrink columns proportionally if total exceeds width.
                    let separator_space = col_count.saturating_sub(1) * 3;
                    let total_content: usize = col_widths.iter().sum();
                    if col_count > 0 && total_content + separator_space > width {
                        let available = width.saturating_sub(separator_space);
                        // Even distribution: each column gets at least
                        // available/col_count, then distribute remainder
                        // to the widest columns to preserve proportionality.
                        let base = available / col_count.max(1);
                        let remainder = available.saturating_sub(base * col_count);

                        // Sort indices by original width (descending) so the
                        // widest columns get the extra space.
                        let mut indices: Vec<usize> = (0..col_count).collect();
                        indices.sort_by(|&a, &b| col_widths[b].cmp(&col_widths[a]));

                        for (rank, &idx) in indices.iter().enumerate() {
                            let extra = if rank < remainder { 1 } else { 0 };
                            col_widths[idx] = base + extra;
                        }
                    }

                    for (is_header, row) in &table_rows {
                        let mut spans: Vec<Span<'static>> = Vec::new();
                        for (i, cell) in row.iter().enumerate() {
                            if i > 0 {
                                spans.push(Span::styled(" │ ", Style::default().fg(theme.subdued)));
                            }
                            let max_w = col_widths.get(i).copied().unwrap_or(0);
                            let padded = text::truncate_and_pad(cell, max_w);
                            let style = if *is_header {
                                Style::default()
                                    .fg(theme.accent_quinary)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(theme.text)
                            };
                            spans.push(Span::styled(padded, style));
                        }
                        lines.push(Line::from(spans));
                        if *is_header {
                            let sep_width: usize =
                                col_widths.iter().sum::<usize>() + separator_space;
                            lines.push(Line::from(Span::styled(
                                "─".repeat(sep_width),
                                Style::default().fg(theme.subdued),
                            )));
                        }
                    }
                    table_rows.clear();
                    lines.push(Line::from(""));
                }
                TagEnd::TableHead => {
                    table_rows.push((true, table_row.clone()));
                    is_table_header = false;
                }
                TagEnd::TableRow => {
                    if !is_table_header {
                        table_rows.push((false, table_row.clone()));
                    }
                }
                TagEnd::TableCell => {
                    table_row.push(current_cell.clone());
                }
                TagEnd::BlockQuote(_) => {
                    in_blockquote = false;
                    style_stack.pop();
                }
                TagEnd::Link => {
                    style_stack.pop();
                }
                _ => {}
            },
            Event::Text(text) => {
                let style = current_style(&style_stack);
                if in_table {
                    current_cell.push_str(&text);
                } else if in_code_block {
                    code_block_content.push_str(&text);
                } else if in_blockquote {
                    // Prefix blockquote lines with a bar
                    for (i, bq_line) in text.lines().enumerate() {
                        if i > 0 {
                            flush_line(&mut lines, &mut current_spans);
                        }
                        if current_spans.is_empty() {
                            current_spans
                                .push(Span::styled("│ ", Style::default().fg(theme.subdued)));
                        }
                        current_spans.push(Span::styled(bq_line.to_string(), style));
                    }
                } else {
                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                current_spans.push(Span::styled(
                    format!("`{code}`"),
                    current_style(&style_stack).fg(theme.emphasis),
                ));
            }
            Event::SoftBreak => {
                current_spans.push(Span::raw(" "));
            }
            Event::HardBreak => {
                flush_line(&mut lines, &mut current_spans);
            }
            Event::Rule => {
                flush_line(&mut lines, &mut current_spans);
                let rule_width = width.min(MAX_BORDER_WIDTH);
                lines.push(Line::from(Span::styled(
                    "─".repeat(rule_width),
                    Style::default().fg(theme.subdued),
                )));
            }
            _ => {}
        }
    }

    flush_line(&mut lines, &mut current_spans);

    // Pad code-block lines so the dark background fills the full terminal width.
    // Intentionally uses `width` (not MAX_BORDER_WIDTH) — borders are decorative
    // and capped, but the background fill should reach the terminal edge.
    let bg = theme.code;
    for line in &mut lines {
        if line.style.bg != Some(bg) {
            continue;
        }
        let content_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
        if content_width < width {
            line.spans.push(Span::styled(
                " ".repeat(width - content_width),
                Style::default().bg(bg),
            ));
        }
    }

    lines
}

fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

fn flush_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        lines.push(Line::from(std::mem::take(spans)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{ColorMode, ThemeId};
    use ratatui::style::Color;
    use rstest::rstest;

    fn cyril_dark() -> Theme {
        crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor)
    }

    fn render(markdown: &str, width: usize) -> Vec<Line<'static>> {
        render_with_theme(markdown, width, &cyril_dark())
    }

    #[test]
    fn production_exposes_only_the_explicit_theme_entry_point() {
        let production = include_str!("markdown.rs")
            .split_once("#[cfg(test)]")
            .map_or(include_str!("markdown.rs"), |(production, _)| production);
        assert!(!production.contains("pub fn render("));
        assert!(!production.contains("crate::theme::resolve"));
        assert!(production.contains("pub fn render_with_theme("));
    }

    /// Extract plain text from rendered lines (ignoring styles).
    fn text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_md(md: &str) -> Vec<Line<'static>> {
        render(md, 200)
    }

    #[test]
    fn prose_constructs_use_marker_theme_roles() {
        let theme = crate::traits::test_support::marker_theme();
        let markdown = "# H1\n### H3\n\n- item\n\n> quote\n\n[link](https://example.com)\n\n| A | B |\n|---|---|\n| x | y |\n\ninline `code`\n\n---";
        let lines = render_with_theme(markdown, 80, &theme);
        let style_for = |needle: &str| {
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .find(|span| span.content == needle)
                .map(|span| span.style)
        };

        assert_eq!(
            style_for("H1").and_then(|style| style.fg),
            Some(theme.accent_quinary)
        );
        assert_eq!(style_for("H3").and_then(|style| style.fg), Some(theme.text));
        assert_eq!(
            style_for("• ").and_then(|style| style.fg),
            Some(theme.accent_quinary)
        );
        assert_eq!(
            style_for("│ ").and_then(|style| style.fg),
            Some(theme.subdued)
        );
        assert_eq!(
            style_for("quote").and_then(|style| style.fg),
            Some(theme.subdued)
        );
        assert_eq!(
            style_for("link").and_then(|style| style.fg),
            Some(theme.accent_tertiary)
        );
        assert_eq!(
            style_for("A").and_then(|style| style.fg),
            Some(theme.accent_quinary)
        );
        assert_eq!(style_for("x").and_then(|style| style.fg), Some(theme.text));
        assert_eq!(
            style_for("`code`").and_then(|style| style.fg),
            Some(theme.emphasis)
        );
        assert!(lines.iter().flat_map(|line| line.spans.iter()).any(|span| {
            span.content.chars().all(|character| character == '─')
                && span.style.fg == Some(theme.subdued)
        }));
    }

    #[test]
    fn code_block_uses_marker_theme_and_missing_syntax_fallback() -> anyhow::Result<()> {
        let theme = crate::traits::test_support::marker_theme();
        let lines = render_with_theme("```rust\nlet value = 42;\n```", 80, &theme);
        let header = &lines[0];
        assert_eq!(header.spans[0].style.fg, Some(theme.subdued));
        assert_eq!(header.spans[1].style.fg, Some(theme.accent_quinary));
        assert_eq!(header.style.bg, Some(theme.code));

        let body = lines
            .iter()
            .find(|line| line.spans.first().is_some_and(|span| span.content == "│ "))
            .ok_or_else(|| anyhow::anyhow!("missing code body"))?;
        assert_eq!(body.spans[0].style.fg, Some(theme.subdued));
        assert_eq!(body.style.bg, Some(theme.code));
        assert!(
            body.spans[1..]
                .iter()
                .filter(|span| !span.content.trim().is_empty())
                .all(|span| span.style.fg == Some(theme.text))
        );

        let footer = lines
            .last()
            .ok_or_else(|| anyhow::anyhow!("missing code footer"))?;
        assert_eq!(footer.spans[0].style.fg, Some(theme.subdued));
        assert_eq!(footer.style.bg, Some(theme.code));
        Ok(())
    }

    #[test]
    fn code_shapes_render_at_boundary_widths() {
        let theme = crate::traits::test_support::marker_theme();
        let wide = "x".repeat(500);
        let cases = [
            "```rust\nfn known() {}\n```".to_string(),
            "```mystery\nunknown language\n```".to_string(),
            "```\nabsent language\n```".to_string(),
            "```rust\n```".to_string(),
            format!("```text\n{wide}\n```"),
            "```text\nUnicode 世界\n```".to_string(),
        ];

        for width in [0, 7, 80, 120, 200] {
            for markdown in &cases {
                let lines = render_with_theme(markdown, width, &theme);
                assert!(lines.len() >= 2);
                assert_eq!(
                    lines.first().and_then(|line| line.style.bg),
                    Some(theme.code)
                );
                assert_eq!(
                    lines.last().and_then(|line| line.style.bg),
                    Some(theme.code)
                );
                let border_width = lines[0]
                    .spans
                    .iter()
                    .filter(|span| !span.content.trim().is_empty())
                    .map(|span| span.content.width())
                    .sum::<usize>();
                assert!(border_width <= 120);
            }
        }
    }

    #[test]
    fn syntax_and_markdown_caches_isolate_truecolor_and_no_color_in_both_orders() {
        let truecolor = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor);
        let no_color = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::None);
        for (markdown, first, second) in [
            (
                "```rust\nfn forward() -> u8 { 1 }\n```",
                &truecolor,
                &no_color,
            ),
            (
                "```rust\nfn reverse() -> u8 { 2 }\n```",
                &no_color,
                &truecolor,
            ),
        ] {
            let first_lines = render_with_theme(markdown, 80, first);
            let second_lines = render_with_theme(markdown, 80, second);
            let (colored, plain) = if first.text == Color::Reset {
                (&second_lines, &first_lines)
            } else {
                (&first_lines, &second_lines)
            };
            assert!(
                colored
                    .iter()
                    .flat_map(|line| line.spans.iter())
                    .any(|span| { matches!(span.style.fg, Some(Color::Rgb(_, _, _))) })
            );
            assert!(plain.iter().all(|line| {
                [line.style.fg, line.style.bg]
                    .into_iter()
                    .chain(
                        line.spans
                            .iter()
                            .flat_map(|span| [span.style.fg, span.style.bg]),
                    )
                    .flatten()
                    .all(|color| color == Color::Reset)
            }));
        }
    }

    #[test]
    fn hundred_kib_code_fallback_stays_linear_fixture() {
        let code = "x".repeat(100 * 1_024);
        let markdown = format!("```text\n{code}\n```");
        let lines = render_with_theme(&markdown, 200, &crate::traits::test_support::marker_theme());
        assert!(text(&lines).contains(&code));
    }

    #[test]
    fn markdown_cache_identity_contains_the_complete_theme() {
        let base = crate::traits::test_support::marker_theme();
        let baseline = markdown_cache_key("cache", 80, &base);
        macro_rules! assert_role_changes_key {
            ($field:ident) => {{
                let mut changed = base;
                changed.$field = Color::Indexed(255);
                assert_ne!(
                    markdown_cache_key("cache", 80, &changed),
                    baseline,
                    "{} missing from cache key",
                    stringify!($field)
                );
            }};
        }
        let mut changed_syntax = base;
        changed_syntax.syntax = Some(crate::theme::SyntaxTheme::Base16EightiesDark);
        assert_ne!(markdown_cache_key("cache", 80, &changed_syntax), baseline);
        assert_role_changes_key!(canvas);
        assert_role_changes_key!(chrome);
        assert_role_changes_key!(code);
        assert_role_changes_key!(selection);
        assert_role_changes_key!(text);
        assert_role_changes_key!(muted);
        assert_role_changes_key!(border);
        assert_role_changes_key!(accent);
        assert_role_changes_key!(accent_alt);
        assert_role_changes_key!(user);
        assert_role_changes_key!(agent);
        assert_role_changes_key!(system);
        assert_role_changes_key!(info);
        assert_role_changes_key!(success);
        assert_role_changes_key!(warning);
        assert_role_changes_key!(danger);
        assert_role_changes_key!(diff_add);
        assert_role_changes_key!(diff_delete);
        assert_role_changes_key!(diff_context);
        assert_role_changes_key!(emphasis);
        assert_role_changes_key!(accent_tertiary);
        assert_role_changes_key!(accent_quaternary);
        assert_role_changes_key!(accent_quinary);
        assert_role_changes_key!(subdued);
        assert_role_changes_key!(subdued_positive);
        assert_role_changes_key!(subdued_negative);
        assert_role_changes_key!(soft_accent);
        assert_role_changes_key!(positive_accent);
        assert_role_changes_key!(inset_background);
    }

    #[test]
    fn prose_geometry_is_theme_independent_at_boundary_widths() {
        let first = crate::traits::test_support::marker_theme();
        let mut second = first;
        second.text = Color::Indexed(101);
        second.subdued = Color::Indexed(102);
        second.emphasis = Color::Indexed(103);
        second.accent_tertiary = Color::Indexed(104);
        second.accent_quinary = Color::Indexed(105);
        let markdown = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n\n- outer\n  - nested\n\n> quote 世界\n\n[repeat](https://example.com) [repeat](https://example.com)\n\n| A | B |\n|---|---|\n| same | same |\n\ninline `code` and **bold** *italic* ~~strike~~\n\n---";

        let shape = |lines: &[Line<'static>]| {
            lines
                .iter()
                .map(|line| {
                    (
                        line.spans
                            .iter()
                            .map(|span| span.content.as_ref())
                            .collect::<String>(),
                        line.spans
                            .iter()
                            .map(|span| span.style.add_modifier)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        for width in [0, 1, 79, 80, 120, 121] {
            let first_lines = render_with_theme(markdown, width, &first);
            let second_lines = render_with_theme(markdown, width, &second);
            assert_eq!(shape(&first_lines), shape(&second_lines));
            assert_ne!(first_lines, second_lines);
        }
    }

    #[test]
    fn markdown_scene_shape_matches_pinned_baseline() -> anyhow::Result<()> {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::layout::{Constraint, Layout};
        use ratatui::widgets::Paragraph;

        const HEADINGS: &str = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
        const STRUCTURE: &str = "- outer\n  - nested\n\n> quote 世界\n\n[repeat](https://example.com) [repeat](https://example.com)";
        const FORMATTING: &str = "| A | B |\n|---|---|\n| same | same |\n\ninline `code` and **bold** *italic* ~~strike~~\n\n---";
        const CODE: &str = "```rust\nfn syntax_rgb() -> u8 { 42 }\n```\n\n```mystery\nunknown_fallback 世界\n```\n\n```\nlanguage_absent\n```";
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| {
            let [left, right] =
                Layout::horizontal([Constraint::Length(40), Constraint::Length(40)])
                    .areas(frame.area());
            let [heading_area, structure_area, formatting_area] = Layout::vertical([
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Min(1),
            ])
            .areas(left);
            frame.render_widget(Paragraph::new(render(HEADINGS, 40)), heading_area);
            frame.render_widget(Paragraph::new(render(STRUCTURE, 40)), structure_area);
            frame.render_widget(Paragraph::new(render(FORMATTING, 40)), formatting_area);
            frame.render_widget(Paragraph::new(render(CODE, 40)), right);
        })?;

        let expected = include_str!("../fixtures/conversation-theme-baseline.tsv")
            .lines()
            .skip(2)
            .filter_map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                (fields.first() == Some(&"markdown")).then_some(fields)
            })
            .map(|fields| {
                Ok((
                    fields
                        .get(3)
                        .ok_or_else(|| anyhow::anyhow!("missing Markdown symbol"))?
                        .to_string(),
                    fields
                        .get(6)
                        .ok_or_else(|| anyhow::anyhow!("missing Markdown modifier"))?
                        .parse::<u16>()?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let actual = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| {
                let mut symbol = String::with_capacity(cell.symbol().len() * 2);
                for byte in cell.symbol().as_bytes() {
                    symbol.push(HEX[(byte >> 4) as usize] as char);
                    symbol.push(HEX[(byte & 0x0f) as usize] as char);
                }
                (symbol, cell.modifier.bits())
            })
            .collect::<Vec<_>>();

        assert_eq!(actual.len(), 1_920);
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn thousand_by_twenty_table_stays_within_operation_fixture() {
        let header = (0..20)
            .map(|column| format!("H{column}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let separator = (0..20).map(|_| "---").collect::<Vec<_>>().join(" | ");
        let row = (0..20)
            .map(|column| format!("same-{column}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let mut markdown = format!("| {header} |\n| {separator} |\n");
        for _ in 0..1_000 {
            markdown.push_str("| ");
            markdown.push_str(&row);
            markdown.push_str(" |\n");
        }

        let lines = render_with_theme(&markdown, 120, &crate::traits::test_support::marker_theme());
        assert!(lines.len() >= 1_002);
    }

    #[test]
    fn render_plain_text() {
        let lines = render_md("Hello world");
        assert!(text(&lines).contains("Hello world"));
    }

    #[rstest]
    #[case("# H1", true)]
    #[case("## H2", true)]
    #[case("### H3", false)]
    fn render_headings(
        #[case] input: &str,
        #[case] uses_quinary_accent: bool,
    ) -> anyhow::Result<()> {
        let lines = render_md(input);
        assert!(!lines.is_empty());
        let first_styled = lines
            .iter()
            .find(|line| !line.spans.is_empty())
            .ok_or_else(|| anyhow::anyhow!("missing styled heading"))?;
        let fg = first_styled.spans[0]
            .style
            .fg
            .ok_or_else(|| anyhow::anyhow!("missing heading foreground"))?;
        let theme = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor);
        let expected = if uses_quinary_accent {
            theme.accent_quinary
        } else {
            theme.text
        };
        assert_eq!(fg, expected);
        Ok(())
    }

    #[test]
    fn render_bold_has_bold_modifier() {
        let lines = render_md("**bold**");
        let has_bold = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::BOLD))
        });
        assert!(has_bold);
    }

    #[test]
    fn render_italic_has_italic_modifier() {
        let lines = render_md("*italic*");
        let has_italic = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::ITALIC))
        });
        assert!(has_italic);
    }

    #[test]
    fn render_inline_code_uses_emphasis() {
        let lines = render_md("use `code` here");
        let theme = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor);
        let has_emphasis = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(theme.emphasis) && span.content.contains("code"))
        });
        assert!(has_emphasis);
    }

    #[test]
    fn render_code_block_has_border() {
        let lines = render_md("```rust\nfn main() {}\n```");
        let t = text(&lines);
        assert!(
            t.contains("╭─── rust"),
            "header should have rounded corner and language: {t}"
        );
        assert!(t.contains("╰─"), "footer should have rounded corner: {t}");
    }

    #[test]
    fn render_code_block_language_badge_uses_quinary_accent() {
        let lines = render_md("```rust\nfn main() {}\n```");
        let theme = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor);
        let has_accent = lines.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.style.fg == Some(theme.accent_quinary)
                    && span.style.add_modifier.contains(Modifier::BOLD)
                    && span.content.contains("rust")
            })
        });
        assert!(
            has_accent,
            "language badge should use quinary accent + Bold"
        );
    }

    #[test]
    fn render_code_block_lines_have_background() {
        let lines = render_md("```\ncode\n```");
        let bg = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor).code;
        let code_lines: Vec<_> = lines.iter().filter(|l| l.style.bg == Some(bg)).collect();
        // Header + 1 content line + footer = 3 lines with bg
        assert!(
            code_lines.len() >= 3,
            "header, content, and footer should all have dark background, got {}",
            code_lines.len()
        );
    }

    #[test]
    fn code_block_lines_padded_to_width() {
        let lines = render("```\nhi\n```", 40);
        let bg = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor).code;
        for line in &lines {
            if line.style.bg != Some(bg) {
                continue;
            }
            let total_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
            assert!(
                total_width >= 40,
                "code-block line should be padded to width 40, got {total_width}"
            );
        }
    }

    #[test]
    fn table_many_columns_at_narrow_width_fits() {
        let header: String = (0..8)
            .map(|i| format!("H{i}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let sep = "---|".repeat(8);
        let row: String = (0..8)
            .map(|i| format!("d{i}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let md = format!("| {header} |\n|{sep}\n| {row} |");
        let lines = render(&md, 30);
        for (i, line) in lines.iter().enumerate() {
            let lw: usize = line.spans.iter().map(|s| s.content.width()).sum();
            assert!(lw <= 30, "line {i} exceeds width 30: {lw}");
        }
    }

    #[test]
    fn render_list_items_have_bullet() {
        let lines = render_md("- item one\n- item two");
        let t = text(&lines);
        assert!(t.contains("• "));
    }

    #[test]
    fn render_blockquote_has_bar() {
        let lines = render_md("> quoted text");
        let t = text(&lines);
        assert!(t.contains("│ "));
    }

    #[test]
    fn render_horizontal_rule() {
        let lines = render_md("---");
        let t = text(&lines);
        assert!(t.contains("────"));
    }

    #[test]
    fn render_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let lines = render_md(md);
        let t = text(&lines);
        assert!(t.contains("A"));
        assert!(t.contains("B"));
        assert!(t.contains("1"));
        assert!(t.contains("─"));
    }

    #[test]
    fn render_table_multicolumn_alignment() {
        let md = "| File | Approach | Widget |\n|------|----------|--------|\n| chat.rs | Manual table | Table |\n| picker.rs | Manual popup | Popup |";
        let lines = render_md(md);
        let t = text(&lines);
        assert!(t.contains("File"), "header should have File: {t}");
        assert!(t.contains("Approach"), "header should have Approach: {t}");
        assert!(t.contains("chat.rs"), "data should have chat.rs: {t}");
        assert!(t.contains("picker.rs"), "data should have picker.rs: {t}");
    }

    #[test]
    fn render_empty_string() {
        let lines = render_md("");
        assert!(lines.is_empty());
    }

    #[test]
    fn render_caching_returns_same_result() {
        let a = render("# cached test", 80);
        let b = render("# cached test", 80);
        assert_eq!(a.len(), b.len());
        for (la, lb) in a.iter().zip(b.iter()) {
            assert_eq!(la.spans.len(), lb.spans.len());
        }
    }

    // === Width-aware regression tests ===

    #[test]
    fn render_at_narrow_width_does_not_panic() {
        // Narrow rendering should not panic. Code block content may exceed
        // the width (it wraps via the Paragraph), but tables, borders,
        // rules, and padding should all respect the width.
        let md = "# Hello\n\n```rust\nhi\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n---";
        let _ = render(md, 20);
    }

    #[test]
    fn render_at_zero_width_does_not_panic() {
        let _ = render("hello\n\n| A |\n|---|\n| 1 |", 0);
    }

    #[test]
    fn table_truncated_shows_ellipsis() {
        let md =
            "| Very long header name here | Another very long header |\n|---|---|\n| cell | data |";
        let lines = render(md, 30);
        let t = text(&lines);
        assert!(t.contains("…"), "truncated table should show …: {t}");
    }

    #[test]
    fn table_not_truncated_when_fits() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let lines = render(md, 80);
        let t = text(&lines);
        assert!(!t.contains("…"), "small table should not truncate: {t}");
    }

    #[test]
    fn table_single_column() {
        let md = "| Solo |\n|---|\n| val |";
        let lines = render(md, 80);
        let t = text(&lines);
        assert!(t.contains("Solo"));
        assert!(t.contains("val"));
    }

    #[test]
    fn code_block_wide_line_not_over_padded() {
        let long_code = "x".repeat(100);
        let md = format!("```\n{long_code}\n```");
        let lines = render(&md, 80);
        let bg = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor).code;
        for line in &lines {
            if line.style.bg != Some(bg) {
                continue;
            }
            let content_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
            // Lines wider than width should not get additional trailing spaces
            if content_width > 80 {
                let last_is_padding = line
                    .spans
                    .last()
                    .map(|s| s.content.trim().is_empty() && !s.content.is_empty())
                    .unwrap_or(false);
                assert!(!last_is_padding, "wide code line should not be padded");
            }
        }
    }

    #[test]
    fn code_block_border_adapts_to_width() {
        let lines_narrow = render("```rust\nhi\n```", 40);
        let lines_wide = render("```rust\nhi\n```", 100);
        let narrow_width: usize = lines_narrow[0]
            .spans
            .iter()
            .map(|s| s.content.width())
            .sum();
        let wide_width: usize = lines_wide[0].spans.iter().map(|s| s.content.width()).sum();
        assert!(
            wide_width > narrow_width,
            "wider terminal should produce wider border: narrow={narrow_width}, wide={wide_width}"
        );
    }

    #[test]
    fn cache_distinguishes_widths() {
        let a = render("```\nhi\n```", 40);
        let b = render("```\nhi\n```", 80);
        let a_widths: Vec<usize> = a
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.width()).sum())
            .collect();
        let b_widths: Vec<usize> = b
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.width()).sum())
            .collect();
        assert_ne!(
            a_widths, b_widths,
            "different widths should produce different output"
        );
    }

    #[test]
    fn tables_and_borders_fit_within_width() {
        // Tables, borders, rules, and padding should all fit within width.
        // Code block *content* may exceed width (it wraps via Paragraph).
        let md =
            "| Col A | Col B | Col C |\n|---|---|---|\n| data | more data | even more |\n\n---";
        for w in [30, 60, 80, 120] {
            let lines = render(md, w);
            for (i, line) in lines.iter().enumerate() {
                let lw: usize = line.spans.iter().map(|s| s.content.width()).sum();
                assert!(lw <= w, "width={w}, line {i} exceeds limit: {lw} cols");
            }
        }
    }

    #[test]
    fn horizontal_rule_adapts_to_width() {
        let lines_40 = render("---", 40);
        let lines_80 = render("---", 80);
        let rule_40: usize = lines_40
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.width())
            .sum();
        let rule_80: usize = lines_80
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.width())
            .sum();
        assert!(
            rule_80 > rule_40,
            "wider terminal should produce wider rule: 40={rule_40}, 80={rule_80}"
        );
    }
}

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use unicode_width::UnicodeWidthStr;

use crate::cache::HashCache;
use crate::highlight;
use crate::palette;
use crate::text;

static MARKDOWN_CACHE: LazyLock<Mutex<HashCache<Vec<Line<'static>>>>> =
    LazyLock::new(|| Mutex::new(HashCache::new(256)));

/// Convert a markdown string into styled ratatui Lines.
/// `width` controls layout decisions: table column sizing, code block
/// padding, horizontal rules, and border lengths. Cached by
/// `(content_hash, width)`.
pub fn render(markdown: &str, width: usize) -> Vec<Line<'static>> {
    let hash = {
        let mut h = DefaultHasher::new();
        markdown.hash(&mut h);
        width.hash(&mut h);
        h.finish()
    };

    if let Ok(cache) = MARKDOWN_CACHE.lock()
        && let Some(cached) = cache.get(hash)
    {
        return cached.clone();
    }

    let result = do_render(markdown, width);

    if let Ok(mut cache) = MARKDOWN_CACHE.lock() {
        cache.insert(hash, result.clone());
    }

    result
}

fn do_render(markdown: &str, width: usize) -> Vec<Line<'static>> {
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
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                            2 => base.fg(Color::Cyan).add_modifier(Modifier::BOLD),
                            3 => base.fg(Color::White).add_modifier(Modifier::BOLD),
                            _ => base.fg(Color::White).add_modifier(Modifier::BOLD),
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
                        let border_style = Style::default().fg(Color::DarkGray);
                        let bg = palette::CODE_BLOCK_BG;
                        let mut header_spans: Vec<Span<'static>> = Vec::new();
                        // Border fills to available width (capped at 120).
                        let border_width = width.min(palette::MAX_BORDER_WIDTH);
                        match &code_block_lang {
                            Some(lang) => {
                                header_spans.push(Span::styled("╭─── ", border_style));
                                header_spans.push(Span::styled(
                                    lang.clone(),
                                    Style::default()
                                        .fg(Color::Cyan)
                                        .add_modifier(Modifier::BOLD),
                                ));
                                let lang_cols = lang.width();
                                let fill_len =
                                    border_width.saturating_sub(lang_cols + 6).max(1);
                                header_spans.push(Span::styled(
                                    format!(" {}", "─".repeat(fill_len)),
                                    border_style,
                                ));
                            }
                            None => {
                                header_spans.push(Span::styled(
                                    "╭".to_string()
                                        + &"─".repeat(border_width.saturating_sub(1)),
                                    border_style,
                                ));
                            }
                        };
                        let mut header_line = Line::from(header_spans);
                        header_line.style = Style::default().bg(bg);
                        lines.push(header_line);
                        style_stack
                            .push(Style::default().fg(Color::White).bg(palette::CODE_BLOCK_BG));
                    }
                    Tag::List(_) => {
                        list_depth += 1;
                    }
                    Tag::Item => {
                        flush_line(&mut lines, &mut current_spans);
                        let indent = "  ".repeat(list_depth.saturating_sub(1));
                        current_spans.push(Span::styled(
                            format!("{indent}• "),
                            Style::default().fg(Color::Cyan),
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
                        style_stack.push(base.fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
                    }
                    Tag::Link { .. } => {
                        style_stack.push(base.fg(Color::Blue).add_modifier(Modifier::UNDERLINED));
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
                    let highlighted = highlight::highlight_block(code, code_block_lang.as_deref());
                    let bg = palette::CODE_BLOCK_BG;

                    for spans in highlighted {
                        let mut line_spans = vec![Span::styled(
                            "│ ".to_string(),
                            Style::default().fg(Color::DarkGray),
                        )];
                        for (style, text) in spans {
                            line_spans.push(Span::styled(text, style));
                        }
                        let mut line = Line::from(line_spans);
                        line.style = Style::default().bg(bg);
                        lines.push(line);
                    }

                    let border_width = width.min(palette::MAX_BORDER_WIDTH);
                    let mut footer_line = Line::from(Span::styled(
                        "╰".to_string() + &"─".repeat(border_width.saturating_sub(1)),
                        Style::default().fg(Color::DarkGray),
                    ));
                    footer_line.style = Style::default().bg(palette::CODE_BLOCK_BG);
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
                    let col_count =
                        table_rows.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
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
                        let scale = available as f64 / total_content.max(1) as f64;
                        let min_col = (available / col_count.max(1)).max(1);
                        for w in &mut col_widths {
                            *w = (*w as f64 * scale).floor().max(min_col as f64) as usize;
                        }
                        // Re-check: if floor pushed us over budget, hard-clamp
                        let post_total: usize = col_widths.iter().sum();
                        if post_total > available {
                            for w in &mut col_widths {
                                *w = min_col;
                            }
                        }
                    }

                    for (is_header, row) in &table_rows {
                        let mut spans: Vec<Span<'static>> = Vec::new();
                        for (i, cell) in row.iter().enumerate() {
                            if i > 0 {
                                spans.push(Span::styled(
                                    " │ ",
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                            let max_w = col_widths.get(i).copied().unwrap_or(0);
                            let padded = text::truncate_and_pad(cell, max_w);
                            let style = if *is_header {
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White)
                            };
                            spans.push(Span::styled(padded, style));
                        }
                        lines.push(Line::from(spans));
                        if *is_header {
                            let sep_width: usize = col_widths.iter().sum::<usize>()
                                + separator_space;
                            lines.push(Line::from(Span::styled(
                                "─".repeat(sep_width),
                                Style::default().fg(Color::DarkGray),
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
                                .push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));
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
                    current_style(&style_stack).fg(Color::Yellow),
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
                let rule_width = width.min(palette::MAX_BORDER_WIDTH);
                lines.push(Line::from(Span::styled(
                    "─".repeat(rule_width),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            _ => {}
        }
    }

    flush_line(&mut lines, &mut current_spans);

    // Pad code-block lines so the dark background fills the full terminal width.
    // Intentionally uses `width` (not MAX_BORDER_WIDTH) — borders are decorative
    // and capped, but the background fill should reach the terminal edge.
    let bg = palette::CODE_BLOCK_BG;
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
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;

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
        do_render(md, 200)
    }

    #[test]
    fn render_plain_text() {
        let lines = render_md("Hello world");
        assert!(text(&lines).contains("Hello world"));
    }

    #[rstest]
    #[case("# H1", Color::Cyan)]
    #[case("## H2", Color::Cyan)]
    #[case("### H3", Color::White)]
    fn render_headings(#[case] input: &str, #[case] expected_color: Color) {
        let lines = render_md(input);
        assert!(!lines.is_empty());
        let first_styled = lines.iter().find(|l| !l.spans.is_empty()).unwrap();
        let fg = first_styled.spans[0].style.fg.unwrap();
        assert_eq!(fg, expected_color);
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
    fn render_inline_code_has_yellow() {
        let lines = render_md("use `code` here");
        let has_yellow = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.fg == Some(Color::Yellow) && s.content.contains("code"))
        });
        assert!(has_yellow);
    }

    #[test]
    fn render_code_block_has_border() {
        let lines = render_md("```rust\nfn main() {}\n```");
        let t = text(&lines);
        assert!(t.contains("╭─── rust"), "header should have rounded corner and language: {t}");
        assert!(t.contains("╰─"), "footer should have rounded corner: {t}");
    }

    #[test]
    fn render_code_block_language_badge_is_cyan_bold() {
        let lines = render_md("```rust\nfn main() {}\n```");
        let has_cyan_bold = lines.iter().any(|l| {
            l.spans.iter().any(|s| {
                s.style.fg == Some(Color::Cyan)
                    && s.style.add_modifier.contains(Modifier::BOLD)
                    && s.content.contains("rust")
            })
        });
        assert!(has_cyan_bold, "language badge should be Cyan + Bold");
    }

    #[test]
    fn render_code_block_lines_have_background() {
        let lines = render_md("```\ncode\n```");
        let bg = palette::CODE_BLOCK_BG;
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
        let lines = do_render("```\nhi\n```", 40);
        let bg = palette::CODE_BLOCK_BG;
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
        let header: String = (0..8).map(|i| format!("H{i}")).collect::<Vec<_>>().join(" | ");
        let sep = "---|".repeat(8);
        let row: String = (0..8).map(|i| format!("d{i}")).collect::<Vec<_>>().join(" | ");
        let md = format!("| {header} |\n|{sep}\n| {row} |");
        let lines = do_render(&md, 30);
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
        let _ = do_render(md, 20);
    }

    #[test]
    fn render_at_zero_width_does_not_panic() {
        let _ = do_render("hello\n\n| A |\n|---|\n| 1 |", 0);
    }

    #[test]
    fn table_truncated_shows_ellipsis() {
        let md = "| Very long header name here | Another very long header |\n|---|---|\n| cell | data |";
        let lines = do_render(md, 30);
        let t = text(&lines);
        assert!(t.contains("…"), "truncated table should show …: {t}");
    }

    #[test]
    fn table_not_truncated_when_fits() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let lines = do_render(md, 80);
        let t = text(&lines);
        assert!(!t.contains("…"), "small table should not truncate: {t}");
    }

    #[test]
    fn table_single_column() {
        let md = "| Solo |\n|---|\n| val |";
        let lines = do_render(md, 80);
        let t = text(&lines);
        assert!(t.contains("Solo"));
        assert!(t.contains("val"));
    }

    #[test]
    fn code_block_wide_line_not_over_padded() {
        let long_code = "x".repeat(100);
        let md = format!("```\n{long_code}\n```");
        let lines = do_render(&md, 80);
        let bg = palette::CODE_BLOCK_BG;
        for line in &lines {
            if line.style.bg != Some(bg) {
                continue;
            }
            let content_width: usize =
                line.spans.iter().map(|s| s.content.width()).sum();
            // Lines wider than width should not get additional trailing spaces
            if content_width > 80 {
                let last_is_padding = line
                    .spans
                    .last()
                    .map(|s| s.content.trim().is_empty() && !s.content.is_empty())
                    .unwrap_or(false);
                assert!(
                    !last_is_padding,
                    "wide code line should not be padded"
                );
            }
        }
    }

    #[test]
    fn code_block_border_adapts_to_width() {
        let lines_narrow = do_render("```rust\nhi\n```", 40);
        let lines_wide = do_render("```rust\nhi\n```", 100);
        let narrow_width: usize = lines_narrow[0]
            .spans
            .iter()
            .map(|s| s.content.width())
            .sum();
        let wide_width: usize = lines_wide[0]
            .spans
            .iter()
            .map(|s| s.content.width())
            .sum();
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
        let md = "| Col A | Col B | Col C |\n|---|---|---|\n| data | more data | even more |\n\n---";
        for w in [30, 60, 80, 120] {
            let lines = do_render(md, w);
            for (i, line) in lines.iter().enumerate() {
                let lw: usize =
                    line.spans.iter().map(|s| s.content.width()).sum();
                assert!(
                    lw <= w,
                    "width={w}, line {i} exceeds limit: {lw} cols"
                );
            }
        }
    }

    #[test]
    fn horizontal_rule_adapts_to_width() {
        let lines_40 = do_render("---", 40);
        let lines_80 = do_render("---", 80);
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

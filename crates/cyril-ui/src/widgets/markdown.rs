use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::cache::HashCache;
use crate::highlight;

static MARKDOWN_CACHE: LazyLock<Mutex<HashCache<Vec<Line<'static>>>>> =
    LazyLock::new(|| Mutex::new(HashCache::new(256)));

/// Convert a markdown string into styled ratatui Lines. Cached by content hash.
pub fn render(markdown: &str) -> Vec<Line<'static>> {
    let hash = {
        let mut h = DefaultHasher::new();
        markdown.hash(&mut h);
        h.finish()
    };

    if let Ok(cache) = MARKDOWN_CACHE.lock()
        && let Some(cached) = cache.get(hash)
    {
        return cached.clone();
    }

    let result = do_render(markdown);

    if let Ok(mut cache) = MARKDOWN_CACHE.lock() {
        cache.insert(hash, result.clone());
    }

    result
}

fn do_render(markdown: &str) -> Vec<Line<'static>> {
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
                        let header = match &code_block_lang {
                            Some(lang) => format!("┌─ {lang} "),
                            None => "┌──".to_string(),
                        };
                        flush_line(&mut lines, &mut current_spans);
                        lines.push(Line::from(Span::styled(
                            header,
                            Style::default().fg(Color::DarkGray),
                        )));
                        style_stack
                            .push(Style::default().fg(Color::White).bg(Color::Rgb(35, 35, 35)));
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
                    for spans in highlighted {
                        let mut line_spans = vec![Span::styled(
                            "│ ".to_string(),
                            Style::default().fg(Color::DarkGray),
                        )];
                        for (style, text) in spans {
                            line_spans.push(Span::styled(text, style));
                        }
                        let mut line = Line::from(line_spans);
                        line.style = Style::default().bg(Color::Rgb(35, 35, 35));
                        lines.push(line);
                    }

                    lines.push(Line::from(Span::styled(
                        "└──",
                        Style::default().fg(Color::DarkGray),
                    )));
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
                    // Compute column widths across all rows
                    let col_count = table_rows.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
                    let mut col_widths = vec![0usize; col_count];
                    for (_, row) in &table_rows {
                        for (i, cell) in row.iter().enumerate() {
                            if i < col_count {
                                col_widths[i] = col_widths[i].max(cell.len());
                            }
                        }
                    }
                    // Render all rows with aligned columns
                    for (is_header, row) in &table_rows {
                        let mut spans: Vec<Span<'static>> = Vec::new();
                        for (i, cell) in row.iter().enumerate() {
                            if i > 0 {
                                spans.push(Span::styled(
                                    " │ ",
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                            let width = col_widths.get(i).copied().unwrap_or(0);
                            let padded = format!("{cell:<width$}");
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
                        // Add separator after header
                        if *is_header {
                            let sep_width: usize = col_widths.iter().sum::<usize>()
                                + (col_count.saturating_sub(1)) * 3;
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
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            _ => {}
        }
    }

    flush_line(&mut lines, &mut current_spans);
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

    #[test]
    fn render_plain_text() {
        let lines = do_render("Hello world");
        assert!(text(&lines).contains("Hello world"));
    }

    #[rstest]
    #[case("# H1", Color::Cyan)]
    #[case("## H2", Color::Cyan)]
    #[case("### H3", Color::White)]
    fn render_headings(#[case] input: &str, #[case] expected_color: Color) {
        let lines = do_render(input);
        assert!(!lines.is_empty());
        let first_styled = lines.iter().find(|l| !l.spans.is_empty()).unwrap();
        let fg = first_styled.spans[0].style.fg.unwrap();
        assert_eq!(fg, expected_color);
    }

    #[test]
    fn render_bold_has_bold_modifier() {
        let lines = do_render("**bold**");
        let has_bold = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::BOLD))
        });
        assert!(has_bold);
    }

    #[test]
    fn render_italic_has_italic_modifier() {
        let lines = do_render("*italic*");
        let has_italic = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::ITALIC))
        });
        assert!(has_italic);
    }

    #[test]
    fn render_inline_code_has_yellow() {
        let lines = do_render("use `code` here");
        let has_yellow = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.fg == Some(Color::Yellow) && s.content.contains("code"))
        });
        assert!(has_yellow);
    }

    #[test]
    fn render_code_block_has_border() {
        let lines = do_render("```rust\nfn main() {}\n```");
        let t = text(&lines);
        assert!(t.contains("┌─ rust"));
        assert!(t.contains("└──"));
    }

    #[test]
    fn render_list_items_have_bullet() {
        let lines = do_render("- item one\n- item two");
        let t = text(&lines);
        assert!(t.contains("• "));
    }

    #[test]
    fn render_blockquote_has_bar() {
        let lines = do_render("> quoted text");
        let t = text(&lines);
        assert!(t.contains("│ "));
    }

    #[test]
    fn render_horizontal_rule() {
        let lines = do_render("---");
        let t = text(&lines);
        assert!(t.contains("────"));
    }

    #[test]
    fn render_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let lines = do_render(md);
        let t = text(&lines);
        assert!(t.contains("A"));
        assert!(t.contains("B"));
        assert!(t.contains("1"));
        assert!(t.contains("─"));
    }

    #[test]
    fn render_empty_string() {
        let lines = do_render("");
        assert!(lines.is_empty());
    }

    #[test]
    fn render_caching_returns_same_result() {
        let a = render("# cached test");
        let b = render("# cached test");
        assert_eq!(a.len(), b.len());
        for (la, lb) in a.iter().zip(b.iter()) {
            assert_eq!(la.spans.len(), lb.spans.len());
        }
    }
}

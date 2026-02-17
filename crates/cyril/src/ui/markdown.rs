use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Convert a markdown string into styled ratatui Lines.
pub fn render(markdown: &str) -> Vec<Line<'static>> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(markdown, options);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;
    let mut list_depth: usize = 0;
    let mut in_blockquote = false;

    for event in parser {
        match event {
            Event::Start(tag) => {
                let base = current_style(&style_stack);
                match tag {
                    Tag::Heading { level, .. } => {
                        flush_line(&mut lines, &mut current_spans);
                        let heading_style = match level as u8 {
                            1 => base.fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
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
                        let lang = match &kind {
                            CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                                format!("┌─ {lang} ")
                            }
                            _ => "┌──".to_string(),
                        };
                        flush_line(&mut lines, &mut current_spans);
                        lines.push(Line::from(Span::styled(
                            lang,
                            Style::default().fg(Color::DarkGray),
                        )));
                        style_stack.push(
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Rgb(35, 35, 35)),
                        );
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
                    flush_code_line(&mut lines, &mut current_spans);
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
                if in_code_block {
                    for (i, code_line) in text.lines().enumerate() {
                        if i > 0 {
                            flush_code_line(&mut lines, &mut current_spans);
                        }
                        current_spans.push(Span::styled(
                            format!("│ {code_line}"),
                            style,
                        ));
                    }
                } else if in_blockquote {
                    // Prefix blockquote lines with a bar
                    for (i, bq_line) in text.lines().enumerate() {
                        if i > 0 {
                            flush_line(&mut lines, &mut current_spans);
                        }
                        if current_spans.is_empty() {
                            current_spans.push(Span::styled(
                                "│ ",
                                Style::default().fg(Color::DarkGray),
                            ));
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

fn flush_code_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        let mut line = Line::from(std::mem::take(spans));
        line.style = Style::default().bg(Color::Rgb(35, 35, 35));
        lines.push(line);
    }
}

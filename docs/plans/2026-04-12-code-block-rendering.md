# Modern Code Block Rendering — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve code block rendering with full-width background, rounded borders, language badge, and line numbers.

**Architecture:** All changes are in `markdown.rs`'s `do_render` function. The header/content/footer emission for `Tag::CodeBlock` / `TagEnd::CodeBlock` gets updated. No changes to `highlight.rs` or the highlight API — the `highlight_block` return type stays the same.

**Tech Stack:** Rust, ratatui, pulldown-cmark, syntect (unchanged)

---

### Task 1: Update code block header

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs` (lines 84-104, `Tag::CodeBlock` handler)

**Step 1: Replace header emission**

In the `Tag::CodeBlock` match arm, replace the current header code (lines 93-101):

```rust
let header = match &code_block_lang {
    Some(lang) => format!("┌─ {lang} "),
    None => "┌──".to_string(),
};
flush_line(&mut lines, &mut current_spans);
lines.push(Line::from(Span::styled(
    header,
    Style::default().fg(Color::DarkGray),
)));
```

With multi-span header using rounded corners, cyan badge, and fill:

```rust
flush_line(&mut lines, &mut current_spans);
let border_style = Style::default().fg(Color::DarkGray);
let bg = Color::Rgb(35, 35, 35);
let mut header_spans: Vec<Span<'static>> = Vec::new();
match &code_block_lang {
    Some(lang) => {
        header_spans.push(Span::styled("╭─── ", border_style));
        header_spans.push(Span::styled(
            lang.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let fill_len = 60usize.saturating_sub(lang.len() + 6);
        header_spans.push(Span::styled(
            format!(" {}", "─".repeat(fill_len)),
            border_style,
        ));
    }
    None => {
        header_spans.push(Span::styled(
            "╭".to_string() + &"─".repeat(59),
            border_style,
        ));
    }
};
let mut header_line = Line::from(header_spans);
header_line.style = Style::default().bg(bg);
lines.push(header_line);
```

**Step 2: Verify compilation**

```sh
cargo check -p cyril-ui
```

---

### Task 2: Add line numbers to code content

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs` (lines 148-165, `TagEnd::CodeBlock` handler)

**Step 1: Replace content line emission**

In the `TagEnd::CodeBlock` match arm, replace the current content code (lines 152-165):

```rust
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
```

With line-numbered content:

```rust
let code = code_block_content.trim_end_matches('\n');
let highlighted = highlight::highlight_block(code, code_block_lang.as_deref());
let total_lines = highlighted.len();
let gutter_width = if total_lines >= 100 {
    3
} else if total_lines >= 10 {
    2
} else {
    1
};
let gutter_style = Style::default().fg(Color::DarkGray);
let bg = Color::Rgb(35, 35, 35);

for (i, spans) in highlighted.into_iter().enumerate() {
    let line_no = i + 1;
    let mut line_spans = vec![
        Span::styled(
            format!("│ {line_no:>gutter_width$} │ "),
            gutter_style,
        ),
    ];
    for (style, text) in spans {
        line_spans.push(Span::styled(text, style));
    }
    let mut line = Line::from(line_spans);
    line.style = Style::default().bg(bg);
    lines.push(line);
}
```

**Step 2: Verify compilation**

```sh
cargo check -p cyril-ui
```

---

### Task 3: Update code block footer

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs` (lines 167-170, footer in `TagEnd::CodeBlock`)

**Step 1: Replace footer emission**

Replace the current footer (lines 167-170):

```rust
lines.push(Line::from(Span::styled(
    "└──",
    Style::default().fg(Color::DarkGray),
)));
```

With rounded corner, fill, and background:

```rust
let mut footer_line = Line::from(Span::styled(
    "╰".to_string() + &"─".repeat(59),
    Style::default().fg(Color::DarkGray),
));
footer_line.style = Style::default().bg(Color::Rgb(35, 35, 35));
lines.push(footer_line);
```

**Step 2: Verify full compilation and run all tests**

```sh
cargo test -p cyril-ui
```

Some existing tests will fail because they assert on `┌─` and `└──` characters.

---

### Task 4: Update tests

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs` (test module, line 312+)

**Step 1: Update `render_code_block_has_border` test**

The current test (line 384) asserts:
```rust
assert!(t.contains("┌─ rust"));
assert!(t.contains("└──"));
```

Update to match new border characters:
```rust
assert!(t.contains("╭─── rust"));
assert!(t.contains("╰─"));
```

**Step 2: Add test for line numbers in code blocks**

```rust
#[test]
fn render_code_block_has_line_numbers() {
    let lines = do_render("```rust\nlet x = 1;\nlet y = 2;\nlet z = 3;\n```");
    let t = text(&lines);
    assert!(t.contains("│ 1 │"), "should have line number 1: {t}");
    assert!(t.contains("│ 2 │"), "should have line number 2: {t}");
    assert!(t.contains("│ 3 │"), "should have line number 3: {t}");
}
```

**Step 3: Add test for language badge styling**

```rust
#[test]
fn render_code_block_language_badge_is_cyan_bold() {
    let lines = do_render("```rust\nfn main() {}\n```");
    let has_cyan_bold = lines.iter().any(|l| {
        l.spans.iter().any(|s| {
            s.style.fg == Some(Color::Cyan)
                && s.style.add_modifier.contains(Modifier::BOLD)
                && s.content.contains("rust")
        })
    });
    assert!(has_cyan_bold, "language badge should be Cyan + Bold");
}
```

**Step 4: Add test for code block background**

```rust
#[test]
fn render_code_block_lines_have_background() {
    let lines = do_render("```\ncode\n```");
    let bg = Color::Rgb(35, 35, 35);
    let code_lines: Vec<_> = lines
        .iter()
        .filter(|l| l.style.bg == Some(bg))
        .collect();
    // Header + 1 content line + footer = 3 lines with bg
    assert!(
        code_lines.len() >= 3,
        "header, content, and footer should all have dark background, got {}",
        code_lines.len()
    );
}
```

**Step 5: Run all tests**

```sh
cargo test -p cyril-ui
```

**Step 6: Commit**

---

## Verification

1. `cargo build` — compiles clean
2. `cargo test -p cyril-ui` — all tests pass
3. Manual testing with `cargo run`:
   - Send a prompt that produces a code block with a language tag
   - Verify: rounded corners `╭`/`╰`, cyan bold language label, line numbers, full-width dark background
   - Send a prompt that produces a code block without a language tag
   - Verify: same visual treatment, no language badge, just border
   - Verify: code blocks with 1 line, 10+ lines, and 100+ lines all align correctly

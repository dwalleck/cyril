# Width-Aware Markdown Renderer — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the markdown renderer width-aware so tables, code blocks, horizontal rules, and background padding all adapt to the terminal width, eliminating wrapping-induced layout breakage.

**Architecture:** Change `render()` to accept a `width: usize` parameter that flows into `do_render()`. All width-dependent rendering (code block padding, table column sizing, horizontal rules, code block borders) uses this value instead of hardcoded constants. The external `pad_to_width()` function is absorbed into the renderer itself. The cache key includes width so different terminal sizes produce different cached output. All callers in `chat.rs` pass `area.width`.

**Tech Stack:** Rust, ratatui 0.30, pulldown-cmark, unicode-width

---

## Context

The markdown renderer currently returns `Vec<Line>` with no knowledge of the viewport width. This causes:
- **Tables wider than the terminal wrap**, destroying column alignment
- **Code block backgrounds** require a separate `pad_to_width()` post-processing hack
- **Horizontal rules** are a fixed 40 chars regardless of terminal width
- **Code block borders** are a fixed 60 chars (`HEADER_WIDTH`)

Amazon Q's TUI solves this by passing `terminal_width` into the parser state. jcode solves it with a 5000-line width-aware renderer. Our approach: pass width as a first-class parameter, handle all width-dependent decisions inside the renderer.

## Files Involved

- **Modify:** `crates/cyril-ui/src/widgets/markdown.rs` — renderer signature, internals, tests
- **Modify:** `crates/cyril-ui/src/widgets/chat.rs` — update 3 `markdown::render()` call sites, remove 2 `pad_to_width()` calls
- **No new files.** `pad_to_width` is absorbed, not moved.

---

### Task 1: Change render signature to accept width

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs`

**Step 1: Update the public `render()` function signature**

Change:
```rust
pub fn render(markdown: &str) -> Vec<Line<'static>> {
```
To:
```rust
pub fn render(markdown: &str, width: usize) -> Vec<Line<'static>> {
```

Include `width` in the cache hash:
```rust
let hash = {
    let mut h = DefaultHasher::new();
    markdown.hash(&mut h);
    width.hash(&mut h);
    h.finish()
};
```

Pass `width` to `do_render`:
```rust
let result = do_render(markdown, width);
```

**Step 2: Update `do_render` signature**

```rust
fn do_render(markdown: &str, width: usize) -> Vec<Line<'static>> {
```

No internal changes yet — just thread the parameter through.

**Step 3: Update all callers in `chat.rs`**

Three call sites, all in functions that have `area: Rect`:
```rust
// Line 34: streaming text in main render
let md_lines = markdown::render(streaming, area.width as usize);

// Line 130: streaming text in subagent drill-in
let md_lines = markdown::render(streaming, area.width as usize);

// Line 180: committed agent text in render_message
let md_lines = markdown::render(text, width);
```

For `render_message`, add a `width: usize` parameter:
```rust
fn render_message(lines: &mut Vec<Line>, msg: &ChatMessage, width: usize) {
```

Update both call sites of `render_message` (lines 21 and 117):
```rust
render_message(&mut lines, msg, area.width as usize);
```

**Step 4: Update test helper**

In the test module, the `render_md` helper wraps `do_render`. Pass a generous default width:
```rust
fn render_md(md: &str) -> Vec<Line<'static>> {
    do_render(md, 200)
}
```

**Step 5: Verify compilation and tests**

```sh
cargo test -p cyril-ui
```

All existing tests should pass unchanged since `200` is wide enough that no truncation occurs.

**Step 6: Commit**

---

### Task 2: Move code block padding into the renderer

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs`
- Modify: `crates/cyril-ui/src/widgets/chat.rs`

**Step 1: Add padding inside `do_render` at the end**

Before the final `lines` return in `do_render`, add the padding logic that's currently in `pad_to_width`:

```rust
flush_line(&mut lines, &mut current_spans);

// Pad code-block lines so the dark background fills the full terminal width.
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
```

**Step 2: Remove `pad_to_width` calls from `chat.rs`**

Remove these two lines:
```rust
// Line ~55 in main render:
markdown::pad_to_width(&mut lines, area.width);

// Line ~136 in subagent drill-in:
markdown::pad_to_width(&mut lines, area.width);
```

**Step 3: Remove or deprecate the `pad_to_width` function from `markdown.rs`**

Delete the `pub fn pad_to_width(...)` function and its doc comment entirely. The `pad_to_width_extends_code_block_background` test should be updated to call `do_render` with a specific width and verify padding directly.

**Step 4: Update the padding test**

Replace the `pad_to_width_extends_code_block_background` test:
```rust
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
```

**Step 5: Verify**

```sh
cargo test -p cyril-ui
```

**Step 6: Commit**

---

### Task 3: Width-aware code block borders

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs`

**Step 1: Replace `HEADER_WIDTH` constant with `width` parameter**

In the `Tag::CodeBlock` handler, remove the reference to `HEADER_WIDTH` and use `width` instead. The border should fill to the available width, not a fixed 60 chars:

```rust
// Header with language badge
let border_width = width.min(120); // cap at 120 to avoid absurdly wide borders
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
```

Similarly update the footer:
```rust
let mut footer_line = Line::from(Span::styled(
    "╰".to_string() + &"─".repeat(border_width.saturating_sub(1)),
    Style::default().fg(Color::DarkGray),
));
```

**Step 2: Remove the module-level `HEADER_WIDTH` constant**

It's no longer needed since borders adapt to `width`.

**Step 3: Update the border test**

The `render_code_block_has_border` test still works since it checks for `╭─── rust` and `╰─` patterns, which are present regardless of width.

**Step 4: Verify**

```sh
cargo test -p cyril-ui
```

**Step 5: Commit**

---

### Task 4: Width-aware table column sizing

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs`

**Step 1: Add a `truncate_to_width` helper**

```rust
/// Truncate a string to at most `max_width` display columns, appending `…`
/// if truncation occurred.
fn truncate_to_width(s: &str, max_width: usize) -> String {
    if s.width() <= max_width {
        return s.to_string();
    }
    if max_width <= 1 {
        return "…".to_string();
    }
    let target = max_width - 1;
    let mut w = 0;
    let mut end = 0;
    for (i, ch) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > target {
            break;
        }
        w += cw;
        end = i + ch.len_utf8();
    }
    format!("{}…", &s[..end])
}
```

**Step 2: Update `TagEnd::Table` to use width for column sizing**

After computing natural column widths, check if they exceed `width` and proportionally shrink:

```rust
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

    // Shrink columns proportionally if total width exceeds available space.
    let separator_space = col_count.saturating_sub(1) * 3;
    let total_content: usize = col_widths.iter().sum();
    if col_count > 0 && total_content + separator_space > width {
        let available = width.saturating_sub(separator_space);
        let scale = available as f64 / total_content.max(1) as f64;
        for w in &mut col_widths {
            *w = (*w as f64 * scale).floor().max(4.0) as usize;
        }
    }

    // Render rows with aligned, possibly truncated columns.
    for (is_header, row) in &table_rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            let max_w = col_widths.get(i).copied().unwrap_or(0);
            let display = truncate_to_width(cell, max_w);
            let padded = format!("{display:<max_w$}");
            let style = if *is_header {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            spans.push(Span::styled(padded, style));
        }
        lines.push(Line::from(spans));
        if *is_header {
            let sep_width: usize = col_widths.iter().sum::<usize>() + separator_space;
            lines.push(Line::from(Span::styled(
                "─".repeat(sep_width),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    table_rows.clear();
    lines.push(Line::from(""));
}
```

**Step 3: Add tests for table width clamping**

```rust
#[test]
fn table_columns_fit_within_width() {
    let md = "| A long header | Another long header | Third long header |\n|---|---|---|\n| cell one content | cell two content | cell three content |";
    let lines = do_render(md, 50);
    for line in &lines {
        let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
        assert!(
            w <= 50,
            "table row should fit within 50 cols, got {w}"
        );
    }
    let t = text(&lines);
    assert!(t.contains("A long"), "header content should be present (possibly truncated)");
}

#[test]
fn table_columns_not_truncated_when_fits() {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |";
    let lines = do_render(md, 80);
    let t = text(&lines);
    assert!(t.contains("A"), "header A present");
    assert!(t.contains("B"), "header B present");
    assert!(t.contains("1"), "data 1 present");
    assert!(t.contains("2"), "data 2 present");
    // No truncation markers
    assert!(!t.contains("…"), "small table should not be truncated at 80 cols");
}
```

**Step 4: Verify**

```sh
cargo test -p cyril-ui
```

**Step 5: Commit**

---

### Task 5: Width-aware horizontal rule

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs`

**Step 1: Replace fixed horizontal rule with width-based rule**

Change:
```rust
Event::Rule => {
    flush_line(&mut lines, &mut current_spans);
    lines.push(Line::from(Span::styled(
        "────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));
}
```

To:
```rust
Event::Rule => {
    flush_line(&mut lines, &mut current_spans);
    let rule_width = width.min(120);
    lines.push(Line::from(Span::styled(
        "─".repeat(rule_width),
        Style::default().fg(Color::DarkGray),
    )));
}
```

**Step 2: Verify**

```sh
cargo test -p cyril-ui
```

**Step 3: Commit**

---

### Task 6: Comprehensive regression test suite

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs` (test module)

**Step 1: Add width boundary tests**

```rust
#[test]
fn render_at_narrow_width_does_not_panic() {
    let md = "# Hello\n\n```rust\nfn main() { println!(\"hello world\"); }\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n---";
    let lines = do_render(md, 20);
    for line in &lines {
        let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
        assert!(w <= 20, "no line should exceed width 20, got {w}");
    }
}

#[test]
fn render_at_zero_width_does_not_panic() {
    let _ = do_render("hello\n\n| A |\n|---|\n| 1 |", 0);
}
```

**Step 2: Add table regression tests**

```rust
#[test]
fn table_truncated_shows_ellipsis() {
    let md = "| Very long header name here | Another very long header |\n|---|---|\n| cell | data |";
    let lines = do_render(md, 30);
    let t = text(&lines);
    assert!(t.contains("…"), "truncated table should show …");
}

#[test]
fn table_single_column() {
    let md = "| Solo |\n|---|\n| val |";
    let lines = do_render(md, 80);
    let t = text(&lines);
    assert!(t.contains("Solo"));
    assert!(t.contains("val"));
}
```

**Step 3: Add code block regression tests**

```rust
#[test]
fn code_block_wide_line_not_over_padded() {
    let long_code = "x".repeat(100);
    let md = format!("```\n{long_code}\n```");
    let lines = do_render(&md, 80);
    let bg = palette::CODE_BLOCK_BG;
    for line in &lines {
        if line.style.bg != Some(bg) { continue; }
        // Content lines wider than width should not get additional padding
        let has_padding_span = line.spans.last()
            .map(|s| s.content.trim().is_empty() && s.content.len() > 0)
            .unwrap_or(false);
        let content_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
        if content_width > 80 {
            assert!(!has_padding_span, "wide code line should not be padded");
        }
    }
}

#[test]
fn code_block_border_adapts_to_width() {
    let lines_narrow = do_render("```rust\nhi\n```", 40);
    let lines_wide = do_render("```rust\nhi\n```", 100);
    let narrow_width: usize = lines_narrow[0].spans.iter()
        .map(|s| s.content.width()).sum();
    let wide_width: usize = lines_wide[0].spans.iter()
        .map(|s| s.content.width()).sum();
    assert!(
        wide_width > narrow_width,
        "wider terminal should produce wider border: narrow={narrow_width}, wide={wide_width}"
    );
}
```

**Step 4: Add cache correctness test**

```rust
#[test]
fn cache_distinguishes_widths() {
    let a = render("```\nhi\n```", 40);
    let b = render("```\nhi\n```", 80);
    let a_widths: Vec<usize> = a.iter()
        .map(|l| l.spans.iter().map(|s| s.content.width()).sum())
        .collect();
    let b_widths: Vec<usize> = b.iter()
        .map(|l| l.spans.iter().map(|s| s.content.width()).sum())
        .collect();
    assert_ne!(a_widths, b_widths, "different widths should produce different output");
}
```

**Step 5: Add the integration property test — the most important regression guard**

```rust
#[test]
fn all_rendered_lines_fit_within_width() {
    let md = "# Title\n\nSome text.\n\n```rust\nfn main() {}\n```\n\n| Col A | Col B | Col C |\n|---|---|---|\n| data | more data | even more |\n\n---\n\n> A blockquote";
    for width in [30, 60, 80, 120] {
        let lines = do_render(md, width);
        for (i, line) in lines.iter().enumerate() {
            let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
            assert!(
                w <= width,
                "width={width}, line {i} exceeds limit: {w} cols"
            );
        }
    }
}
```

**Step 6: Add horizontal rule width test**

```rust
#[test]
fn horizontal_rule_adapts_to_width() {
    let lines_40 = do_render("---", 40);
    let lines_80 = do_render("---", 80);
    let rule_40: usize = lines_40.iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.width())
        .sum();
    let rule_80: usize = lines_80.iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.width())
        .sum();
    assert!(rule_80 > rule_40, "wider terminal should produce wider rule");
}
```

**Step 7: Verify all tests pass**

```sh
cargo test -p cyril-ui
```

**Step 8: Commit**

---

### Task 7: Clean up and final verification

**Files:**
- Modify: `crates/cyril-ui/src/widgets/markdown.rs` — remove dead code
- Modify: `crates/cyril-ui/src/widgets/chat.rs` — verify no leftover `pad_to_width` references

**Step 1: Remove `HEADER_WIDTH` if still present**

Should have been removed in Task 3. Verify it's gone.

**Step 2: Remove `pad_to_width` import from `chat.rs` if still referenced**

Grep for any remaining references:
```sh
grep -n pad_to_width crates/cyril-ui/src/
```

**Step 3: Run full test suite**

```sh
cargo test
```

**Step 4: Manual testing**

```sh
cargo run
```

Test:
- Send a prompt that produces a code block — border and background should fill terminal width
- Send a prompt that produces a table — columns should fit within terminal width
- Resize the terminal — new renders should adapt to the new width
- Verify horizontal rules fill the terminal width

**Step 5: Commit**

---

## Verification

1. `cargo build` — compiles clean
2. `cargo test` — all tests pass
3. Manual testing:
   - Code blocks: background fills full width, borders adapt to terminal
   - Tables: columns fit within terminal, truncated with `…` only when necessary
   - Horizontal rules: fill terminal width
   - Resize: new content adapts (cached content may show old width until re-rendered)
   - Scrolling: unchanged behavior
   - Activity indicator: unchanged behavior

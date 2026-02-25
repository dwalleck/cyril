# Syntect Syntax Highlighting Integration

## Overview

This branch adds syntax highlighting to Cyril using the `syntect` crate. Code blocks in markdown responses and edit-tool diffs now render with language-aware coloring instead of monochrome white-on-gray text.

## What Changed

### 1. `crates/cyril/Cargo.toml`
Added dependency:
```toml
syntect = { version = "5", default-features = false, features = ["default-fancy"] }
```
`default-fancy` includes ~50 language grammars and the `base16-ocean.dark` theme.

### 2. `crates/cyril/src/ui/highlight.rs` (new file)
Encapsulates all syntect interaction behind three public functions:

- **`highlight_block(code, lang)`** — Highlights a full code block. Results are cached by hash(content, lang) in a 256-entry HashMap with full-clear eviction. Used by markdown code blocks.
- **`highlight_line(code, ext)`** — Highlights a single line by file extension. Uncached (diffs render once). Used by diff display.
- **`tint_with_diff_color(fg, diff_color)`** — Blends syntax fg with diff color at 70/30 ratio so added lines look greenish and deleted lines look reddish while preserving syntax color variation.

Internal globals (loaded once via `LazyLock`):
- `SYNTAX_SET` — `SyntaxSet::load_defaults_newlines()`
- `THEME_SET` — `ThemeSet::load_defaults()`
- `HIGHLIGHT_CACHE` — `Mutex<HashMap<u64, Vec<Vec<(Style, String)>>>>`

### 3. `crates/cyril/src/ui/mod.rs`
Added `pub mod highlight;` to register the new module.

### 4. `crates/cyril/src/ui/markdown.rs`
**Before:** Each `Event::Text` inside a code block immediately created monochrome spans.

**After:** Code block text accumulates in `code_block_content: String`. At `TagEnd::CodeBlock`, the entire block is highlighted via `highlight::highlight_block()`, then each line is emitted as multi-span with a `"│ "` DarkGray prefix and `Rgb(35,35,35)` background.

### 5. `crates/cyril/src/ui/tool_calls.rs`
**Before:** Each diff line was a single `Span::styled(format!("    {line_no:>4}{prefix} {line_text}"), style)`.

**After:** Each add/delete diff line becomes a `Vec<Span>`:
- Gutter span: line number + prefix in the diff color (Red/Green)
- Content spans: syntax-highlighted via `highlight::highlight_line()`, fg tinted with `tint_with_diff_color()`
- Context lines (`ChangeTag::Equal`): stay plain DarkGray, no highlighting

The file extension is extracted from `diff.path` to determine the syntax.

## Verification Checklist

Run these on a machine where cargo works:

```sh
cargo check                          # no compile errors
cargo test -p cyril-core             # existing tests unaffected (core crate untouched)
cargo run                            # test with prompts that generate:
```

- Rust/Python/JSON code blocks (verify highlighting)
- File edits (verify tinted diff highlighting)
- Unknown language code blocks (verify graceful fallback to plain text)
- Multiple code blocks in sequence (verify cache works)

## Design Notes

- Cache stores `Vec<Vec<(Style, String)>>` not `Vec<Line>` — decouples cache from ratatui line construction
- Line-level bg stays `Rgb(35,35,35)` — syntect only sets fg per-span
- Diff `highlight_line()` creates a fresh `HighlightLines` per call — acceptable because diff lines are discontinuous fragments
- `flush_code_line()` in markdown.rs is now unused on the main code path but retained in the file
- No new external crate besides syntect — `LazyLock` is in std since Rust 1.80

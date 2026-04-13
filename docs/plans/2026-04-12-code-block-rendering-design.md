# Modern Code Block Rendering — Design

## Problem

Code blocks in the chat renderer have several visual issues:
- Header (`┌─ rust`) and footer (`└──`) lack the dark background, creating a disconnected stripe
- Background only covers text content, not the full terminal width — looks ragged
- Borders are minimal (3 chars wide) and don't extend
- Language label is subtle gray, easy to miss
- No line numbers

## Design

All changes are in `crates/cyril-ui/src/widgets/markdown.rs`, specifically in how `do_render` emits code block header, content, and footer lines. No changes to `highlight.rs`, the syntect theme, caching, or the `highlight_block` API.

### Full-width background

All code block lines (header, content, footer) set `line.style.bg = Rgb(35, 35, 35)`. Ratatui fills `Line.style.bg` to the full paragraph width, so no width calculation is needed.

### Rounded borders with horizontal fill

Replace `┌`/`└` with `╭`/`╰`. Extend horizontal rules with a fixed 200-char `─` buffer — ratatui clips to the available width, so this works at any terminal size.

Header: `╭─── rust ─────...` (language in Cyan+Bold, rest in DarkGray)
Footer: `╰─────────────...` (all DarkGray)

### Line numbers

Right-aligned gutter with dimmed style, separated from code by `│`:

```
╭─── rust ──────────────────
│  1 │ let x = 1;
│  2 │ let y = 2;
│ 10 │ more_code();
╰───────────────────────────
```

Gutter width: `max(2, digit_count(total_lines))` to handle blocks from 1 to 999+ lines.

### Language badge

Language name styled `Cyan + Bold` in the header, distinct from the `DarkGray` border characters. For unlabeled code blocks, header is just `╭───────...`.

### Visual reference

```
╭─── rust ──────────────────────────────────────  <- DarkGray border + Cyan bold lang
│  1 │ fn main() {                                <- DarkGray gutter │ highlighted code
│  2 │     println!("hello");                     <- all on Rgb(35,35,35) background
│  3 │ }                                          <- full-width bg via Line.style
╰───────────────────────────────────────────────  <- DarkGray border, same bg
```

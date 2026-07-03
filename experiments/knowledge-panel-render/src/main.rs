//! Standalone ratatui render of the proposed cyril `knowledge` panel.
//!
//! Mirrors `crates/cyril-ui/src/widgets/crew_panel.rs` exactly — same
//! `Block::default().borders(ALL)` with a cyan ` title `, same per-row layout
//! (`{icon} ` + bold-white `{name:<20} ` + status), same `+N more` overflow in
//! yellow-italic. Draws to a `TestBackend` and dumps the real buffer, so this is
//! what the terminal would actually paint — not a mockup.
//!
//! `cargo run` prints a truecolor render at 80 cols (cyril's typical width) plus
//! a narrow 60-col render that shows the download row clipping at the border.
//! `cargo test` asserts the wire-driven content of each row.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

/// One knowledge-base row's state, 1:1 with the `_kiro/knowledge/indexing*`
/// notifications: a spinner while a base is between `indexingStarted` and
/// `indexingCompleted`, `✓`/`✗` once `indexingCompleted` settles it.
enum RowState {
    /// `best` base, first use: `indexingStarted` fired, model still downloading.
    Downloading { spinner: char, mb: u32 },
    /// `fast` base indexing in progress (between Started and Completed).
    Indexing { spinner: char, files: u32 },
    /// `indexingCompleted { status: "success", itemCount }`.
    Success { items: u32 },
    /// `indexingCompleted { status: "failed" }` — carries no itemCount.
    Failed { reason: &'static str },
}

struct Row {
    name: &'static str,
    state: RowState,
}

/// Faithful port of `crew_panel::render`'s drawing, retargeted at knowledge rows.
fn render_knowledge_panel(buf: &mut Buffer, area: Rect, rows: &[Row], hidden: u32) {
    let mut lines: Vec<Line> = Vec::new();

    for row in rows {
        let (icon, icon_color, status, status_color) = match &row.state {
            RowState::Downloading { spinner, mb } => (
                spinner.to_string(),
                Color::Yellow,
                format!("downloading model… ~{mb}MB"),
                Color::Yellow,
            ),
            RowState::Indexing { spinner, files } => (
                spinner.to_string(),
                Color::Cyan,
                format!("indexing… {files} files"),
                Color::DarkGray,
            ),
            RowState::Success { items } => (
                "✓".to_string(),
                Color::Green,
                format!("indexed · {items} items"),
                Color::DarkGray,
            ),
            RowState::Failed { reason } => (
                "✗".to_string(),
                Color::Red,
                format!("failed · {reason}"),
                Color::DarkGray,
            ),
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(icon_color)),
            Span::styled(
                format!("{:<20} ", row.name),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(status, Style::default().fg(status_color)),
        ]));
    }

    if hidden > 0 {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("+{hidden} more"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " knowledge ",
        Style::default().fg(Color::Cyan),
    ));
    Paragraph::new(lines).block(block).render(area, buf);
}

fn demo_rows() -> Vec<Row> {
    vec![
        Row {
            name: "api-docs",
            state: RowState::Downloading {
                spinner: '⠹',
                mb: 90,
            },
        },
        Row {
            name: "lighthouse-notes",
            state: RowState::Indexing {
                spinner: '⠋',
                files: 3,
            },
        },
        Row {
            name: "runbooks",
            state: RowState::Success { items: 128 },
        },
        Row {
            name: "legacy-logs",
            state: RowState::Failed {
                reason: "model load error",
            },
        },
    ]
}

fn draw(w: u16, h: u16) -> Buffer {
    let rows = demo_rows();
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("test terminal");
    terminal
        .draw(|frame| {
            render_knowledge_panel(frame.buffer_mut(), Rect::new(0, 0, w, h), &rows, 2);
        })
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn main() {
    // 4 rows + 1 overflow + 2 borders = 7 tall. 80 cols = cyril's typical width.
    let (w, h) = (80u16, 7u16);
    let buf = draw(w, h);
    println!("\n  ratatui {w}×{h} TestBackend — truecolor (from each cell's fg):\n");
    print_ansi(&buf, w, h);
    println!("\n  plain grid (exact glyphs + geometry ratatui painted):\n");
    print_plain(&buf, w, h);

    // Very narrow terminal: status text truncates at the inner edge — ratatui
    // clips gracefully (no panic, no body overflow). A real layout property a
    // mockup can't show (cf. rivets cyril-mdbp, narrow-terminal overflow).
    let (nw, nh) = (44u16, 7u16);
    let nbuf = draw(nw, nh);
    println!("\n  very narrow {nw}×{nh} — status truncates at the border, no overflow:\n");
    print_plain(&nbuf, nw, nh);
}

/// Emit the buffer with real 24-bit ANSI, coalescing runs of identical style so
/// the colored output stays legible (one SGR per run, reset at line end).
fn print_ansi(buf: &Buffer, w: u16, h: u16) {
    for y in 0..h {
        print!("  ");
        let mut cur: Option<String> = None;
        for x in 0..w {
            let cell = &buf[(x, y)];
            let (r, g, b) = rgb(cell.fg);
            let mut sgr = format!("38;2;{r};{g};{b}");
            if cell.modifier.contains(Modifier::BOLD) {
                sgr.push_str(";1");
            }
            if cell.modifier.contains(Modifier::ITALIC) {
                sgr.push_str(";3");
            }
            if cur.as_deref() != Some(&sgr) {
                print!("\x1b[0m\x1b[{sgr}m");
                cur = Some(sgr);
            }
            print!("{}", cell.symbol());
        }
        println!("\x1b[0m");
    }
}

fn print_plain(buf: &Buffer, w: u16, h: u16) {
    for y in 0..h {
        print!("  ");
        for x in 0..w {
            print!("{}", buf[(x, y)].symbol());
        }
        println!();
    }
}

/// Map the ratatui named colors this panel uses to cyril-faithful RGB (palette.rs
/// true-color where cyril defines it; One-Dark-ish ANSI for the rest).
fn rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Cyan => (86, 182, 194),      // border + title + fast spinner
        Color::Green => (129, 199, 132),    // AGENT_GREEN — ✓ success
        Color::Red => (224, 108, 117),      // ✗ failed
        Color::Yellow => (229, 192, 123),   // download spinner + "+N more"
        Color::White => (233, 236, 241),    // bold base names
        Color::DarkGray => (140, 140, 140), // MUTED_GRAY — status text
        _ => (215, 220, 227),               // Reset / default fg (borders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_text(buf: &Buffer, y: u16, w: u16) -> String {
        (0..w).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn success_row_shows_itemcount_and_check() {
        let buf = draw(80, 7);
        let line = row_text(&buf, 3, 80);
        assert!(line.contains('✓'), "success row missing ✓: {line}");
        assert!(line.contains("runbooks"));
        assert!(line.contains("128 items"), "itemCount missing: {line}");
    }

    #[test]
    fn failed_row_shows_cross_and_reason_no_itemcount() {
        let buf = draw(80, 7);
        let line = row_text(&buf, 4, 80);
        assert!(line.contains('✗'), "failed row missing ✗: {line}");
        assert!(line.contains("failed · model load error"));
        assert!(
            !line.contains("items"),
            "failed row must not show itemCount"
        );
    }

    #[test]
    fn downloading_row_flags_the_90mb_dependency() {
        let buf = draw(80, 7);
        let line = row_text(&buf, 1, 80);
        assert!(line.contains('⠹'), "spinner missing: {line}");
        assert!(line.contains("~90MB"), "download size missing: {line}");
    }

    #[test]
    fn overflow_row_matches_crew_panel_idiom() {
        let buf = draw(80, 7);
        assert!(row_text(&buf, 5, 80).contains("+2 more"));
    }

    #[test]
    fn cyan_border_and_title() {
        let buf = draw(80, 7);
        let top = row_text(&buf, 0, 80);
        assert!(top.starts_with("┌ knowledge ─"), "title/border: {top}");
        assert_eq!(buf[(0u16, 0u16)].fg, Color::Reset); // corner uses default fg
        assert_eq!(buf[(2u16, 0u16)].fg, Color::Cyan); // 'k' of title is cyan
    }
}

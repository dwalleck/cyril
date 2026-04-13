use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use ratatui::style::{Color, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::cache::HashCache;

const THEME_NAME: &str = "base16-eighties.dark";

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// A single highlighted line: a sequence of (style, text) spans.
type HighlightedLine = Vec<(Style, String)>;

/// A highlighted block: one `HighlightedLine` per source line.
type HighlightedBlock = Vec<HighlightedLine>;

static HIGHLIGHT_CACHE: LazyLock<Mutex<HashCache<HighlightedBlock>>> =
    LazyLock::new(|| Mutex::new(HashCache::new(256)));

/// Highlight a full code block. Cached by hash(content, lang).
pub fn highlight_block(code: &str, lang: Option<&str>) -> HighlightedBlock {
    let hash = {
        let mut h = DefaultHasher::new();
        code.hash(&mut h);
        lang.hash(&mut h);
        h.finish()
    };

    if let Ok(cache) = HIGHLIGHT_CACHE.lock()
        && let Some(cached) = cache.get(hash)
    {
        return cached.clone();
    }

    let result = do_highlight_block(code, lang);

    if let Ok(mut cache) = HIGHLIGHT_CACHE.lock() {
        cache.insert(hash, result.clone());
    }

    result
}

fn do_highlight_block(code: &str, lang: Option<&str>) -> HighlightedBlock {
    let syntax = lang
        .and_then(|l| SYNTAX_SET.find_syntax_by_token(l))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let theme = match THEME_SET.themes.get(THEME_NAME) {
        Some(t) => t,
        None => return plain_fallback(code),
    };
    let mut highlighter = HighlightLines::new(syntax, theme);

    code.lines()
        .map(|line| {
            let line_with_newline = format!("{line}\n");
            match highlighter.highlight_line(&line_with_newline, &SYNTAX_SET) {
                Ok(ranges) => ranges
                    .into_iter()
                    .map(|(style, text)| {
                        (syntect_to_ratatui(style), text.trim_end_matches('\n').to_string())
                    })
                    .collect(),
                Err(_) => vec![(Style::default().fg(Color::White), line.to_string())],
            }
        })
        .collect()
}

/// Highlight a single line (for diffs). Uncached.
pub fn highlight_line(code: &str, ext: Option<&str>) -> HighlightedLine {
    let syntax = ext
        .and_then(|e| {
            SYNTAX_SET
                .find_syntax_by_extension(e)
                .or_else(|| SYNTAX_SET.find_syntax_by_token(e))
        })
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let theme = match THEME_SET.themes.get(THEME_NAME) {
        Some(t) => t,
        None => return vec![(Style::default().fg(Color::White), code.to_string())],
    };
    let mut highlighter = HighlightLines::new(syntax, theme);

    let line_with_newline = format!("{code}\n");
    match highlighter.highlight_line(&line_with_newline, &SYNTAX_SET) {
        Ok(ranges) => ranges
            .into_iter()
            .map(|(style, text)| {
                (syntect_to_ratatui(style), text.trim_end_matches('\n').to_string())
            })
            .collect(),
        Err(_) => vec![(Style::default().fg(Color::White), code.to_string())],
    }
}

/// Blend syntax fg with diff color: 70% syntax + 30% diff tint.
pub fn tint_with_diff_color(fg: Color, diff_color: Color) -> Color {
    let (sr, sg, sb) = color_to_rgb(fg);
    let (dr, dg, db) = color_to_rgb(diff_color);

    Color::Rgb(
        ((sr as u16 * 7 + dr as u16 * 3) / 10) as u8,
        ((sg as u16 * 7 + dg as u16 * 3) / 10) as u8,
        ((sb as u16 * 7 + db as u16 * 3) / 10) as u8,
    )
}

/// Produce plain white-on-default fallback for every line.
fn plain_fallback(code: &str) -> HighlightedBlock {
    code.lines()
        .map(|line| vec![(Style::default().fg(Color::White), line.to_string())])
        .collect()
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Red => (255, 80, 80),
        Color::Green => (80, 255, 80),
        Color::DarkGray => (128, 128, 128),
        Color::White => (255, 255, 255),
        _ => (200, 200, 200),
    }
}

fn syntect_to_ratatui(style: SynStyle) -> Style {
    let fg = style.foreground;
    Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_block_returns_lines() {
        let code = "let x = 1;\nlet y = 2;";
        let result = highlight_block(code, Some("rs"));
        assert_eq!(result.len(), 2);
        // Each line should have at least one styled span
        assert!(!result[0].is_empty());
        assert!(!result[1].is_empty());
    }

    #[test]
    fn highlight_block_plain_text_fallback() {
        let code = "just some text";
        let result = highlight_block(code, None);
        assert_eq!(result.len(), 1);
        // The text content should be preserved
        let full_text: String = result[0].iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(full_text, "just some text");
    }

    #[test]
    fn highlight_block_caches_results() {
        let code = "fn main() {}";
        let first = highlight_block(code, Some("rs"));
        let second = highlight_block(code, Some("rs"));
        assert_eq!(first, second);
    }

    #[test]
    fn highlight_line_returns_spans() {
        let result = highlight_line("let x = 42;", Some("rs"));
        assert!(!result.is_empty());
        let full_text: String = result.iter().map(|(_, t)| t.as_str()).collect();
        assert!(full_text.contains("let"));
    }

    #[test]
    fn highlight_line_unknown_ext() {
        let result = highlight_line("hello world", Some("zzz_nonexistent"));
        assert!(!result.is_empty());
    }

    #[test]
    fn tint_blends_colors() {
        let result = tint_with_diff_color(Color::Rgb(100, 100, 100), Color::Rgb(200, 200, 200));
        assert_eq!(result, Color::Rgb(130, 130, 130));
    }

    #[test]
    fn tint_with_named_colors() {
        let result = tint_with_diff_color(Color::White, Color::Red);
        // White = (255,255,255), Red = (255,80,80)
        // R: (255*7 + 255*3)/10 = 255
        // G: (255*7 + 80*3)/10 = 202
        // B: (255*7 + 80*3)/10 = 202
        assert_eq!(result, Color::Rgb(255, 202, 202));
    }

    #[test]
    fn color_to_rgb_handles_unknown_variant() {
        let (r, g, b) = color_to_rgb(Color::Cyan);
        assert_eq!((r, g, b), (200, 200, 200));
    }

    #[test]
    fn syntect_to_ratatui_converts() {
        let syn_style = SynStyle {
            foreground: syntect::highlighting::Color { r: 100, g: 150, b: 200, a: 255 },
            ..SynStyle::default()
        };
        let style = syntect_to_ratatui(syn_style);
        assert_eq!(style.fg, Some(Color::Rgb(100, 150, 200)));
    }
}

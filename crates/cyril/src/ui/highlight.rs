use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use ratatui::style::{Color, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(|| SyntaxSet::load_defaults_newlines());
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

const CACHE_LIMIT: usize = 256;

static HIGHLIGHT_CACHE: LazyLock<Mutex<HashMap<u64, Vec<Vec<(Style, String)>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Highlight a full code block. Cached by hash(content, lang).
pub fn highlight_block(code: &str, lang: Option<&str>) -> Vec<Vec<(Style, String)>> {
    let hash = {
        let mut h = DefaultHasher::new();
        code.hash(&mut h);
        lang.hash(&mut h);
        h.finish()
    };

    if let Ok(cache) = HIGHLIGHT_CACHE.lock() {
        if let Some(cached) = cache.get(&hash) {
            return cached.clone();
        }
    }

    let result = do_highlight_block(code, lang);

    if let Ok(mut cache) = HIGHLIGHT_CACHE.lock() {
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(hash, result.clone());
    }

    result
}

fn do_highlight_block(code: &str, lang: Option<&str>) -> Vec<Vec<(Style, String)>> {
    let syntax = lang
        .and_then(|l| SYNTAX_SET.find_syntax_by_token(l))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let theme = &THEME_SET.themes["base16-ocean.dark"];
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
pub fn highlight_line(code: &str, ext: Option<&str>) -> Vec<(Style, String)> {
    let syntax = ext
        .and_then(|e| {
            SYNTAX_SET
                .find_syntax_by_extension(e)
                .or_else(|| SYNTAX_SET.find_syntax_by_token(e))
        })
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let theme = &THEME_SET.themes["base16-ocean.dark"];
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

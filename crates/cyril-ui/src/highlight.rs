use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use ratatui::style::{Color, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::cache::HashCache;
use crate::theme::Theme;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// A single highlighted line: a sequence of (style, text) spans.
type HighlightedLine = Vec<(Style, String)>;

/// A highlighted block: one `HighlightedLine` per source line.
type HighlightedBlock = Vec<HighlightedLine>;

static HIGHLIGHT_CACHE: LazyLock<Mutex<HashCache<HighlightedBlock>>> =
    LazyLock::new(|| Mutex::new(HashCache::new(256)));

/// Highlight a full code block. Cached by hash(content, language, complete theme).
pub fn highlight_block_with_theme(
    code: &str,
    lang: Option<&str>,
    theme: &Theme,
) -> HighlightedBlock {
    highlight_block_with_cache(&HIGHLIGHT_CACHE, code, lang, theme)
}

fn highlight_block_with_cache(
    cache: &Mutex<HashCache<HighlightedBlock>>,
    code: &str,
    lang: Option<&str>,
    theme: &Theme,
) -> HighlightedBlock {
    let hash = highlight_cache_key(code, lang, theme);

    if let Ok(cache) = cache.lock()
        && let Some(cached) = cache.get(hash)
    {
        return cached.clone();
    }

    let syntax_theme = theme
        .syntax
        .and_then(|syntax_theme| THEME_SET.themes.get(syntax_theme.name()));
    let result = do_highlight_block(code, lang, theme, syntax_theme);

    if let Ok(mut cache) = cache.lock() {
        cache.insert(hash, result.clone());
    }

    result
}

fn highlight_cache_key(code: &str, lang: Option<&str>, theme: &Theme) -> u64 {
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    lang.hash(&mut hasher);
    theme.syntax.map(|syntax| syntax.name()).hash(&mut hasher);
    for color in [
        theme.canvas,
        theme.chrome,
        theme.code,
        theme.selection,
        theme.text,
        theme.muted,
        theme.border,
        theme.accent,
        theme.accent_alt,
        theme.user,
        theme.agent,
        theme.system,
        theme.info,
        theme.success,
        theme.warning,
        theme.danger,
        theme.diff_add,
        theme.diff_delete,
        theme.diff_context,
        theme.emphasis,
        theme.accent_tertiary,
        theme.accent_quaternary,
        theme.accent_quinary,
        theme.subdued,
        theme.subdued_positive,
        theme.subdued_negative,
        theme.soft_accent,
        theme.positive_accent,
        theme.inset_background,
    ] {
        color.hash(&mut hasher);
    }
    hasher.finish()
}

fn do_highlight_block(
    code: &str,
    lang: Option<&str>,
    theme: &Theme,
    syntax_theme: Option<&syntect::highlighting::Theme>,
) -> HighlightedBlock {
    let Some(syntax_theme) = syntax_theme else {
        return plain_fallback(code, theme.text);
    };
    let syntax = lang
        .and_then(|language| SYNTAX_SET.find_syntax_by_token(language))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, syntax_theme);

    code.lines()
        .map(|line| {
            let line_with_newline = format!("{line}\n");
            normalize_highlight_result(
                highlighter.highlight_line(&line_with_newline, &SYNTAX_SET),
                line,
                theme.text,
            )
        })
        .collect()
}

/// Highlight a single line (for diffs). Uncached.
pub fn highlight_line_with_theme(code: &str, ext: Option<&str>, theme: &Theme) -> HighlightedLine {
    let Some(syntax_theme) = theme
        .syntax
        .and_then(|syntax_theme| THEME_SET.themes.get(syntax_theme.name()))
    else {
        return fallback_line(code, theme.text);
    };
    let syntax = ext
        .and_then(|extension| {
            SYNTAX_SET
                .find_syntax_by_extension(extension)
                .or_else(|| SYNTAX_SET.find_syntax_by_token(extension))
        })
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, syntax_theme);

    let line_with_newline = format!("{code}\n");
    normalize_highlight_result(
        highlighter.highlight_line(&line_with_newline, &SYNTAX_SET),
        code,
        theme.text,
    )
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

fn normalize_highlight_result<E>(
    result: Result<Vec<(SynStyle, &str)>, E>,
    original: &str,
    fallback_color: Color,
) -> HighlightedLine {
    match result {
        Ok(ranges) => ranges
            .into_iter()
            .map(|(style, text)| {
                (
                    syntect_to_ratatui(style),
                    text.trim_end_matches('\n').to_string(),
                )
            })
            .collect(),
        Err(_) => {
            tracing::warn!("syntax highlighting failed; using primary-text fallback");
            fallback_line(original, fallback_color)
        }
    }
}

fn fallback_line(text: &str, fallback_color: Color) -> HighlightedLine {
    vec![(Style::default().fg(fallback_color), text.to_string())]
}

/// Produce primary-text-on-default fallback for every line.
fn plain_fallback(code: &str, fallback_color: Color) -> HighlightedBlock {
    code.lines()
        .map(|line| fallback_line(line, fallback_color))
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
    use crate::theme::{ColorMode, ThemeId};

    fn cyril_dark() -> Theme {
        crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor)
    }

    #[test]
    fn production_exposes_only_explicit_theme_entry_points() {
        let production = include_str!("highlight.rs")
            .split_once("#[cfg(test)]")
            .map_or(include_str!("highlight.rs"), |(production, _)| production);
        assert!(!production.contains("pub fn highlight_block("));
        assert!(!production.contains("pub fn highlight_line("));
        assert!(!production.contains("crate::theme::resolve"));
        assert!(production.contains("pub fn highlight_block_with_theme("));
        assert!(production.contains("pub fn highlight_line_with_theme("));
    }

    #[test]
    fn themed_fallback_uses_primary_text_role() {
        let mut theme = crate::traits::test_support::marker_theme();
        theme.syntax = None;
        let result = highlight_block_with_theme("plain fallback", None, &theme);

        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .iter()
                .all(|(style, _)| style.fg == Some(theme.text))
        );
    }

    #[test]
    fn reset_primary_text_does_not_disable_an_available_syntax_theme() {
        let mut theme = cyril_dark();
        theme.text = Color::Reset;

        let block = uncached_block("fn main() {}", Some("rs"), &theme);
        let line = highlight_line_with_theme("fn main() {}", Some("rs"), &theme);

        assert!(
            block
                .iter()
                .flatten()
                .any(|(style, _)| style.fg != Some(Color::Reset))
        );
        assert!(line.iter().any(|(style, _)| style.fg != Some(Color::Reset)));
    }

    #[test]
    fn local_highlight_cache_records_rendered_entry() {
        let cache = Mutex::new(HashCache::new(256));
        let theme = cyril_dark();
        let code = "fn cached() {}";
        let highlighted = highlight_block_with_cache(&cache, code, Some("rs"), &theme);
        let key = highlight_cache_key(code, Some("rs"), &theme);
        assert_eq!(
            cache.lock().ok().and_then(|cache| cache.get(key).cloned()),
            Some(highlighted)
        );
    }

    fn uncached_block(code: &str, lang: Option<&str>, theme: &Theme) -> HighlightedBlock {
        let syntax_theme = theme
            .syntax
            .and_then(|syntax| THEME_SET.themes.get(syntax.name()));
        do_highlight_block(code, lang, theme, syntax_theme)
    }

    #[test]
    fn highlight_cache_eviction_matches_oldest_half_ledger() {
        let cache = Mutex::new(HashCache::new(256));
        let mut keys = Vec::with_capacity(257);
        let base = crate::traits::test_support::marker_theme();

        for index in 0..256usize {
            let mut theme = base;
            theme.text = Color::Indexed(index as u8);
            highlight_block_with_cache(&cache, "x", Some("rs"), &theme);
            keys.push(highlight_cache_key("x", Some("rs"), &theme));
        }
        highlight_block_with_cache(&cache, "y", Some("rs"), &base);
        keys.push(highlight_cache_key("y", Some("rs"), &base));
        assert_eq!(
            keys.iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            257,
            "fixture keys must be unique"
        );

        let ledger = match cache.lock() {
            Ok(ledger) => ledger,
            Err(error) => panic!("EVICTION local highlight lock failed: {error}"),
        };
        for key in &keys[..128] {
            assert!(ledger.get(*key).is_none(), "EVICTION retained oldest key");
        }
        for key in &keys[128..] {
            assert!(ledger.get(*key).is_some(), "EVICTION lost newer key");
        }
        drop(ledger);

        let mut repeated_theme = base;
        repeated_theme.text = Color::Indexed(200);
        assert_eq!(
            highlight_block_with_cache(&cache, "x", Some("rs"), &repeated_theme),
            uncached_block("x", Some("rs"), &repeated_theme),
            "EVICTION repeat changed output"
        );
    }

    #[test]
    fn highlight_cache_concurrent_alternating_modes_never_leak() {
        use std::sync::{Arc, Barrier};

        let cache = Arc::new(Mutex::new(HashCache::new(256)));
        let barrier = Arc::new(Barrier::new(8));
        let colored = cyril_dark();
        let plain = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::None);
        let colored_expected = uncached_block("x", Some("rs"), &colored);
        let plain_expected = uncached_block("x", Some("rs"), &plain);
        let mut workers = Vec::new();

        for worker in 0usize..8 {
            let cache = Arc::clone(&cache);
            let barrier = Arc::clone(&barrier);
            let colored_expected = colored_expected.clone();
            let plain_expected = plain_expected.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                for query in 0usize..100 {
                    let (theme, expected) = if (worker + query).is_multiple_of(2) {
                        (&colored, &colored_expected)
                    } else {
                        (&plain, &plain_expected)
                    };
                    assert_eq!(
                        highlight_block_with_cache(&cache, "x", Some("rs"), theme),
                        *expected,
                        "CONCURRENT worker {worker} query {query}"
                    );
                }
                100usize
            }));
        }

        let completed = workers
            .into_iter()
            .map(|worker| match worker.join() {
                Ok(completed) => completed,
                Err(_) => panic!("CONCURRENT highlight worker panicked"),
            })
            .sum::<usize>();
        assert_eq!(completed, 800, "CONCURRENT highlight query count");
    }

    #[test]
    fn highlight_cache_poison_computes_uncached_without_panic() {
        let cache = Mutex::new(HashCache::new(256));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = match cache.lock() {
                Ok(guard) => guard,
                Err(error) => panic!("POISON initial highlight lock failed: {error}"),
            };
            panic!("POISON highlight fixture");
        }));
        assert!(cache.is_poisoned());

        let theme = cyril_dark();
        assert_eq!(
            highlight_block_with_cache(&cache, "x", Some("rs"), &theme),
            uncached_block("x", Some("rs"), &theme),
            "POISON highlight fallback drifted"
        );
    }

    #[test]
    fn five_hundred_highlight_cache_hits_return_same_output() {
        let cache = Mutex::new(HashCache::new(256));
        let theme = cyril_dark();
        let expected = highlight_block_with_cache(&cache, "x", Some("rs"), &theme);
        for _ in 0..500 {
            assert_eq!(
                std::hint::black_box(highlight_block_with_cache(&cache, "x", Some("rs"), &theme,)),
                expected
            );
        }
    }

    #[test]
    fn complete_theme_participates_in_cache_identity() {
        let base = crate::traits::test_support::marker_theme();
        let baseline = highlight_cache_key("cache-key", Some("rs"), &base);
        macro_rules! assert_role_changes_key {
            ($field:ident) => {{
                let mut changed = base;
                changed.$field = Color::Indexed(255);
                assert_ne!(
                    highlight_cache_key("cache-key", Some("rs"), &changed),
                    baseline,
                    "{} missing from cache key",
                    stringify!($field)
                );
            }};
        }
        let mut changed_syntax = base;
        changed_syntax.syntax = Some(crate::theme::SyntaxTheme::Base16EightiesDark);
        assert_ne!(
            highlight_cache_key("cache-key", Some("rs"), &changed_syntax),
            baseline
        );
        assert_role_changes_key!(canvas);
        assert_role_changes_key!(chrome);
        assert_role_changes_key!(code);
        assert_role_changes_key!(selection);
        assert_role_changes_key!(text);
        assert_role_changes_key!(muted);
        assert_role_changes_key!(border);
        assert_role_changes_key!(accent);
        assert_role_changes_key!(accent_alt);
        assert_role_changes_key!(user);
        assert_role_changes_key!(agent);
        assert_role_changes_key!(system);
        assert_role_changes_key!(info);
        assert_role_changes_key!(success);
        assert_role_changes_key!(warning);
        assert_role_changes_key!(danger);
        assert_role_changes_key!(diff_add);
        assert_role_changes_key!(diff_delete);
        assert_role_changes_key!(diff_context);
        assert_role_changes_key!(emphasis);
        assert_role_changes_key!(accent_tertiary);
        assert_role_changes_key!(accent_quaternary);
        assert_role_changes_key!(accent_quinary);
        assert_role_changes_key!(subdued);
        assert_role_changes_key!(subdued_positive);
        assert_role_changes_key!(subdued_negative);
        assert_role_changes_key!(soft_accent);
        assert_role_changes_key!(positive_accent);
        assert_role_changes_key!(inset_background);
    }

    #[test]
    fn cache_never_leaks_truecolor_into_no_color_in_either_order() {
        let truecolor = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor);
        let no_color = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::None);
        for (code, first, second) in [
            ("fn forward_cache() -> u8 { 1 }", &truecolor, &no_color),
            ("fn reverse_cache() -> u8 { 2 }", &no_color, &truecolor),
        ] {
            let first_result = highlight_block_with_theme(code, Some("rs"), first);
            let second_result = highlight_block_with_theme(code, Some("rs"), second);
            let (colored, plain) = if first.text == Color::Reset {
                (&second_result, &first_result)
            } else {
                (&first_result, &second_result)
            };
            assert!(
                colored
                    .iter()
                    .flatten()
                    .any(|(style, _)| { matches!(style.fg, Some(Color::Rgb(_, _, _))) })
            );
            assert!(
                plain
                    .iter()
                    .flatten()
                    .all(|(style, _)| style.fg == Some(Color::Reset))
            );
        }
    }

    #[test]
    fn catalog_and_highlighter_failures_use_primary_text() {
        let theme = crate::traits::test_support::marker_theme();
        let missing_catalog = do_highlight_block("catalog", Some("rs"), &theme, None);
        assert!(
            missing_catalog[0]
                .iter()
                .all(|(style, _)| style.fg == Some(theme.text))
        );

        let failed = normalize_highlight_result(
            Err::<Vec<(SynStyle, &str)>, ()>(()),
            "highlighter",
            theme.text,
        );
        assert_eq!(failed, fallback_line("highlighter", theme.text));
    }

    #[test]
    fn five_hundred_cached_blocks_keep_themed_output() {
        let theme = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor);
        for _ in 0..500 {
            let result = highlight_block_with_theme("fn cached() {}", Some("rs"), &theme);
            assert!(!result.is_empty());
        }
    }

    #[test]
    fn highlight_block_returns_lines() {
        let code = "let x = 1;\nlet y = 2;";
        let result = highlight_block_with_theme(code, Some("rs"), &cyril_dark());
        assert_eq!(result.len(), 2);
        // Each line should have at least one styled span
        assert!(!result[0].is_empty());
        assert!(!result[1].is_empty());
    }

    #[test]
    fn highlight_block_plain_text_fallback() {
        let code = "just some text";
        let result = highlight_block_with_theme(code, None, &cyril_dark());
        assert_eq!(result.len(), 1);
        // The text content should be preserved
        let full_text: String = result[0].iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(full_text, "just some text");
    }

    #[test]
    fn highlight_block_caches_results() {
        let code = "fn main() {}";
        let theme = cyril_dark();
        let first = highlight_block_with_theme(code, Some("rs"), &theme);
        let second = highlight_block_with_theme(code, Some("rs"), &theme);
        assert_eq!(first, second);
    }

    #[test]
    fn highlight_line_returns_spans() {
        let result = highlight_line_with_theme("let x = 42;", Some("rs"), &cyril_dark());
        assert!(!result.is_empty());
        let full_text: String = result.iter().map(|(_, t)| t.as_str()).collect();
        assert!(full_text.contains("let"));
    }

    #[test]
    fn highlight_line_unknown_ext() {
        let result =
            highlight_line_with_theme("hello world", Some("zzz_nonexistent"), &cyril_dark());
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
            foreground: syntect::highlighting::Color {
                r: 100,
                g: 150,
                b: 200,
                a: 255,
            },
            ..SynStyle::default()
        };
        let style = syntect_to_ratatui(syn_style);
        assert_eq!(style.fg, Some(Color::Rgb(100, 150, 200)));
    }
}

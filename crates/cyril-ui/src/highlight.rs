use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use ratatui::style::{Color, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::cache::HashCache;
use crate::theme::{HighlightStyle, Theme};

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// A single highlighted line: a sequence of (style, text) spans.
type HighlightedLine = Vec<(Style, String)>;

/// A highlighted block: one `HighlightedLine` per source line.
type HighlightedBlock = Vec<HighlightedLine>;

static HIGHLIGHT_CACHE: LazyLock<Mutex<HashCache<HighlightedBlock>>> =
    LazyLock::new(|| Mutex::new(HashCache::new(256)));

/// Highlight a full code block. Cached by hash(content, language, highlight style).
pub fn highlight_block_with_theme(
    code: &str,
    lang: Option<&str>,
    theme: &Theme,
) -> HighlightedBlock {
    highlight_block_with_style(code, lang, HighlightStyle::from(theme))
}

pub(crate) fn highlight_block_with_style(
    code: &str,
    lang: Option<&str>,
    style: HighlightStyle,
) -> HighlightedBlock {
    highlight_block_with_cache(&HIGHLIGHT_CACHE, code, lang, style)
}

fn highlight_block_with_cache(
    cache: &Mutex<HashCache<HighlightedBlock>>,
    code: &str,
    lang: Option<&str>,
    style: HighlightStyle,
) -> HighlightedBlock {
    let hash = highlight_cache_key(code, lang, style);

    if let Ok(cache) = cache.lock()
        && let Some(cached) = cache.get(hash)
    {
        return cached.clone();
    }

    let syntax_theme = style
        .syntax
        .and_then(|syntax_theme| THEME_SET.themes.get(syntax_theme.name()));
    let result = do_highlight_block(code, lang, style, syntax_theme);

    if let Ok(mut cache) = cache.lock() {
        cache.insert(hash, result.clone());
    }

    result
}

fn highlight_cache_key(code: &str, lang: Option<&str>, style: HighlightStyle) -> u64 {
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    lang.hash(&mut hasher);
    style.hash(&mut hasher);
    hasher.finish()
}

fn do_highlight_block(
    code: &str,
    lang: Option<&str>,
    style: HighlightStyle,
    syntax_theme: Option<&syntect::highlighting::Theme>,
) -> HighlightedBlock {
    let Some(syntax_theme) = syntax_theme else {
        return plain_fallback(code, style.text);
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
                style.text,
            )
        })
        .collect()
}

/// Highlight a single line (for diffs). Uncached.
pub fn highlight_line_with_theme(code: &str, ext: Option<&str>, theme: &Theme) -> HighlightedLine {
    highlight_line_with_style(code, ext, HighlightStyle::from(theme))
}

fn highlight_line_with_style(
    code: &str,
    ext: Option<&str>,
    style: HighlightStyle,
) -> HighlightedLine {
    let Some(syntax_theme) = style
        .syntax
        .and_then(|syntax_theme| THEME_SET.themes.get(syntax_theme.name()))
    else {
        return fallback_line(code, style.text);
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
        style.text,
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
        let style = HighlightStyle::from(&theme);
        let highlighted = highlight_block_with_cache(&cache, code, Some("rs"), style);
        let key = highlight_cache_key(code, Some("rs"), style);
        assert_eq!(
            cache.lock().ok().and_then(|cache| cache.get(key).cloned()),
            Some(highlighted)
        );
    }

    fn uncached_block(code: &str, lang: Option<&str>, theme: &Theme) -> HighlightedBlock {
        let style = HighlightStyle::from(theme);
        let syntax_theme = style
            .syntax
            .and_then(|syntax| THEME_SET.themes.get(syntax.name()));
        do_highlight_block(code, lang, style, syntax_theme)
    }

    #[test]
    fn highlight_cache_eviction_matches_oldest_half_ledger() {
        let cache = Mutex::new(HashCache::new(256));
        let mut keys = Vec::with_capacity(257);
        let base = crate::traits::test_support::marker_theme();

        for index in 0..256usize {
            let mut theme = base;
            theme.text = Color::Indexed(index as u8);
            let style = HighlightStyle::from(&theme);
            highlight_block_with_cache(&cache, "x", Some("rs"), style);
            keys.push(highlight_cache_key("x", Some("rs"), style));
        }
        let base_style = HighlightStyle::from(&base);
        highlight_block_with_cache(&cache, "y", Some("rs"), base_style);
        keys.push(highlight_cache_key("y", Some("rs"), base_style));
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
            highlight_block_with_cache(
                &cache,
                "x",
                Some("rs"),
                HighlightStyle::from(&repeated_theme),
            ),
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
                        highlight_block_with_cache(
                            &cache,
                            "x",
                            Some("rs"),
                            HighlightStyle::from(theme),
                        ),
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
            highlight_block_with_cache(&cache, "x", Some("rs"), HighlightStyle::from(&theme)),
            uncached_block("x", Some("rs"), &theme),
            "POISON highlight fallback drifted"
        );
    }

    #[test]
    fn five_hundred_highlight_cache_hits_return_same_output() {
        let cache = Mutex::new(HashCache::new(256));
        let theme = cyril_dark();
        let style = HighlightStyle::from(&theme);
        let expected = highlight_block_with_cache(&cache, "x", Some("rs"), style);
        for _ in 0..500 {
            assert_eq!(
                std::hint::black_box(highlight_block_with_cache(&cache, "x", Some("rs"), style)),
                expected
            );
        }
    }

    #[test]
    fn highlight_cache_isolates_syntax_and_fallback_text_inputs() {
        let code = "fn main() -> u8 { 42 }";
        let colored = cyril_dark();
        let mut alternate_syntax = colored;
        alternate_syntax.syntax = Some(crate::theme::SyntaxTheme::Base16OceanLight);
        let plain = crate::traits::test_support::marker_theme();
        let mut alternate_text = plain;
        alternate_text.text = Color::Indexed(255);

        for (base, changed) in [(colored, alternate_syntax), (plain, alternate_text)] {
            let base_expected = uncached_block(code, Some("rs"), &base);
            let changed_expected = uncached_block(code, Some("rs"), &changed);
            assert_ne!(
                base_expected, changed_expected,
                "positive control must differ"
            );

            for (first, second) in [(base, changed), (changed, base)] {
                let cache = Mutex::new(HashCache::new(256));
                let first_style = HighlightStyle::from(&first);
                let second_style = HighlightStyle::from(&second);
                let first_key = highlight_cache_key(code, Some("rs"), first_style);
                let second_key = highlight_cache_key(code, Some("rs"), second_style);
                assert_ne!(first_key, second_key);

                for theme in [first, second, first] {
                    let style = HighlightStyle::from(&theme);
                    let expected = uncached_block(code, Some("rs"), &theme);
                    assert_eq!(
                        highlight_block_with_cache(&cache, code, Some("rs"), style),
                        expected
                    );
                    assert_eq!(
                        cached_entry(&cache, highlight_cache_key(code, Some("rs"), style)),
                        Some(expected)
                    );
                }
                assert!(cached_entry(&cache, first_key).is_some());
                assert!(cached_entry(&cache, second_key).is_some());
            }
        }
    }

    fn cached_entry(
        cache: &Mutex<HashCache<HighlightedBlock>>,
        key: u64,
    ) -> Option<HighlightedBlock> {
        match cache.lock() {
            Ok(cache) => cache.get(key).cloned(),
            Err(error) => panic!("local highlight cache lock failed: {error}"),
        }
    }

    #[test]
    fn irrelevant_theme_roles_reuse_highlight_cache_entries() {
        let code = "fn reused() {}";
        for base in [cyril_dark(), crate::traits::test_support::marker_theme()] {
            // Cover a colored syntax path and no-syntax fallback independently.
            for changed in [
                Theme {
                    canvas: Color::Indexed(255),
                    ..base
                },
                Theme {
                    chrome: Color::Indexed(255),
                    ..base
                },
                Theme {
                    code: Color::Indexed(255),
                    ..base
                },
                Theme {
                    selection: Color::Indexed(255),
                    ..base
                },
                Theme {
                    text_secondary: Color::Indexed(255),
                    ..base
                },
                Theme {
                    accent_violet: Color::Indexed(255),
                    ..base
                },
            ] {
                assert_ne!(changed, base);
                for (first, second) in [(base, changed), (changed, base)] {
                    // A miss at capacity would evict the first entry, making
                    // reuse observable without changing HashCache's API.
                    let cache = Mutex::new(HashCache::new(2));
                    let first_style = HighlightStyle::from(&first);
                    let second_style = HighlightStyle::from(&second);
                    let key = highlight_cache_key(code, Some("rs"), first_style);
                    assert_eq!(highlight_cache_key(code, Some("rs"), second_style), key);
                    let expected =
                        highlight_block_with_cache(&cache, code, Some("rs"), first_style);
                    let probe_code = "fn retained_probe() {}";
                    let probe_key = highlight_cache_key(probe_code, Some("rs"), first_style);
                    let probe =
                        highlight_block_with_cache(&cache, probe_code, Some("rs"), first_style);
                    assert_ne!(key, probe_key);

                    assert_eq!(
                        highlight_block_with_cache(&cache, code, Some("rs"), second_style),
                        expected
                    );
                    assert_eq!(cached_entry(&cache, key), Some(expected));
                    assert_eq!(cached_entry(&cache, probe_key), Some(probe));
                }
            }
        }
    }

    #[test]
    fn highlight_cache_preserves_content_and_language_inputs() {
        let cache = Mutex::new(HashCache::new(256));
        let style = HighlightStyle::from(&cyril_dark());
        let mut keys = std::collections::HashSet::new();
        for (code, lang) in [
            ("fn main() {}", Some("rs")),
            ("fn other() {}", Some("rs")),
            ("fn main() {}", None),
        ] {
            let key = highlight_cache_key(code, lang, style);
            assert!(
                keys.insert(key),
                "content and language must isolate entries"
            );
            let rendered = highlight_block_with_cache(&cache, code, lang, style);
            assert_eq!(cached_entry(&cache, key), Some(rendered));
        }
    }

    #[test]
    fn cache_identity_matches_resolved_highlight_styles_across_palettes_and_modes() {
        let code = "fn main() -> u8 { 42 }";
        for mode in [
            ColorMode::TrueColor,
            ColorMode::Ansi256,
            ColorMode::Ansi16,
            ColorMode::None,
        ] {
            let mut seen = std::collections::HashMap::new();
            let mut styles = std::collections::HashSet::new();
            let cache = Mutex::new(HashCache::new(256));
            for id in ThemeId::ALL
                .iter()
                .chain(ThemeId::ALL.iter().rev())
                .copied()
            {
                let theme = crate::theme::resolve(id, mode);
                let style = HighlightStyle::from(&theme);
                let key = highlight_cache_key(code, Some("rs"), style);
                if let Some(previous) = seen.insert(key, style) {
                    assert_eq!(
                        previous, style,
                        "different resolved highlight inputs collided in {mode:?}"
                    );
                }
                styles.insert(style);
                assert_eq!(
                    highlight_block_with_cache(&cache, code, Some("rs"), style),
                    uncached_block(code, Some("rs"), &theme),
                    "palette {} in {mode:?}",
                    id.name()
                );
            }
            assert_eq!(seen.len(), styles.len());
            if mode == ColorMode::None {
                assert_eq!(seen.len(), 1, "no-color palettes share reset style inputs");
            }
        }
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
        let mut theme = crate::traits::test_support::marker_theme();
        theme.syntax = Some(crate::theme::SyntaxTheme::Base16EightiesDark);
        let missing_catalog = do_highlight_block(
            "catalog\nsecond line",
            Some("rs"),
            HighlightStyle::from(&theme),
            None,
        );
        assert_eq!(
            missing_catalog,
            plain_fallback("catalog\nsecond line", theme.text)
        );

        let failed = normalize_highlight_result(
            Err::<Vec<(SynStyle, &str)>, ()>(()),
            "highlighter",
            theme.text,
        );
        assert_eq!(failed, fallback_line("highlighter", theme.text));
    }

    #[test]
    fn cached_error_fallback_isolates_text_but_reuses_irrelevant_roles() {
        let code = "fn failed_highlighter() {}";
        let base = cyril_dark();
        let mut alternate_text = base;
        alternate_text.text = Color::Indexed(255);
        let failed_block = |style: HighlightStyle| {
            vec![normalize_highlight_result(
                Err::<Vec<(SynStyle, &str)>, ()>(()),
                code,
                style.text,
            )]
        };
        assert_ne!(
            failed_block(HighlightStyle::from(&base)),
            failed_block(HighlightStyle::from(&alternate_text))
        );

        for (first, second) in [(base, alternate_text), (alternate_text, base)] {
            let cache = Mutex::new(HashCache::new(2));
            let first_style = HighlightStyle::from(&first);
            let second_style = HighlightStyle::from(&second);
            let first_key = highlight_cache_key(code, Some("rs"), first_style);
            let second_key = highlight_cache_key(code, Some("rs"), second_style);
            assert_ne!(first_key, second_key);

            // Syntect errors are not reliably input-triggerable. Seed the
            // local cache with the real error normalizer's fallback output.
            match cache.lock() {
                Ok(mut cache) => {
                    cache.insert(first_key, failed_block(first_style));
                    cache.insert(second_key, failed_block(second_style));
                }
                Err(error) => panic!("error fallback cache lock failed: {error}"),
            }
            for theme in [first, second, first] {
                let mut irrelevant_change = theme;
                irrelevant_change.canvas = Color::Indexed(255);
                irrelevant_change.chrome = Color::Indexed(255);
                irrelevant_change.code = Color::Indexed(255);
                irrelevant_change.selection = Color::Indexed(255);
                let style = HighlightStyle::from(&irrelevant_change);
                let expected = failed_block(HighlightStyle::from(&theme));
                assert_eq!(
                    highlight_block_with_cache(&cache, code, Some("rs"), style),
                    expected
                );
                assert_eq!(
                    cached_entry(&cache, highlight_cache_key(code, Some("rs"), style)),
                    Some(expected)
                );
            }
            assert_eq!(
                cached_entry(&cache, first_key),
                Some(failed_block(first_style))
            );
            assert_eq!(
                cached_entry(&cache, second_key),
                Some(failed_block(second_style))
            );
        }
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

use ratatui::style::Color;

/// Bundled visual theme identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    CyrilDark,
}

/// Explicit terminal color capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
    Ansi16,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceColor {
    Rgb(u8, u8, u8),
    Reset,
}

/// Syntax-highlighting component selected by a visual theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxTheme {
    Base16EightiesDark,
}

impl SyntaxTheme {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Base16EightiesDark => "base16-eighties.dark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceTheme {
    syntax: SyntaxTheme,
    canvas: SourceColor,
    chrome: SourceColor,
    code: SourceColor,
    selection: SourceColor,
    text: SourceColor,
    muted: SourceColor,
    border: SourceColor,
    accent: SourceColor,
    accent_alt: SourceColor,
    user: SourceColor,
    agent: SourceColor,
    system: SourceColor,
    info: SourceColor,
    success: SourceColor,
    warning: SourceColor,
    danger: SourceColor,
    diff_add: SourceColor,
    diff_delete: SourceColor,
    diff_context: SourceColor,
}

#[cfg(test)]
impl SourceTheme {
    fn roles(self) -> [(&'static str, SourceColor); 19] {
        [
            ("canvas", self.canvas),
            ("chrome", self.chrome),
            ("code", self.code),
            ("selection", self.selection),
            ("text", self.text),
            ("muted", self.muted),
            ("border", self.border),
            ("accent", self.accent),
            ("accent_alt", self.accent_alt),
            ("user", self.user),
            ("agent", self.agent),
            ("system", self.system),
            ("info", self.info),
            ("success", self.success),
            ("warning", self.warning),
            ("danger", self.danger),
            ("diff_add", self.diff_add),
            ("diff_delete", self.diff_delete),
            ("diff_context", self.diff_context),
        ]
    }
}

fn cyril_dark_source(id: ThemeId) -> SourceTheme {
    match id {
        ThemeId::CyrilDark => SourceTheme {
            syntax: SyntaxTheme::Base16EightiesDark,
            canvas: SourceColor::Reset,
            chrome: SourceColor::Rgb(0x1e, 0x1e, 0x2e),
            code: SourceColor::Rgb(0x28, 0x2c, 0x34),
            selection: SourceColor::Rgb(0x32, 0x32, 0x46),
            text: SourceColor::Rgb(0xff, 0xff, 0xff),
            muted: SourceColor::Rgb(0x8c, 0x8c, 0x8c),
            border: SourceColor::Rgb(0x8c, 0x8c, 0x8c),
            accent: SourceColor::Rgb(0x00, 0xff, 0xff),
            accent_alt: SourceColor::Rgb(0xb4, 0x8e, 0xad),
            user: SourceColor::Rgb(0x8a, 0xb4, 0xf8),
            agent: SourceColor::Rgb(0x81, 0xc7, 0x84),
            system: SourceColor::Rgb(0xb4, 0x8e, 0xad),
            info: SourceColor::Rgb(0x00, 0xff, 0xff),
            success: SourceColor::Rgb(0x00, 0xff, 0x00),
            warning: SourceColor::Rgb(0xff, 0xff, 0x00),
            danger: SourceColor::Rgb(0xff, 0x00, 0x00),
            diff_add: SourceColor::Rgb(0x00, 0xff, 0x00),
            diff_delete: SourceColor::Rgb(0xff, 0x00, 0x00),
            diff_context: SourceColor::Rgb(0x8c, 0x8c, 0x8c),
        },
    }
}

/// Resolved semantic colors consumed by renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub syntax: Option<SyntaxTheme>,
    pub canvas: Color,
    pub chrome: Color,
    pub code: Color,
    pub selection: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub user: Color,
    pub agent: Color,
    pub system: Color,
    pub info: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub diff_add: Color,
    pub diff_delete: Color,
    pub diff_context: Color,
}

impl SourceColor {
    const fn truecolor(self) -> Color {
        match self {
            Self::Rgb(r, g, b) => Color::Rgb(r, g, b),
            Self::Reset => Color::Reset,
        }
    }

    fn ansi256(self) -> Color {
        match self {
            Self::Rgb(r, g, b) => Color::Indexed(nearest_ansi256((r, g, b))),
            Self::Reset => Color::Reset,
        }
    }

    fn ansi16(self) -> Color {
        match self {
            Self::Rgb(r, g, b) => ANSI16_COLORS[usize::from(nearest_ansi16((r, g, b)))],
            Self::Reset => Color::Reset,
        }
    }
}

fn resolve_with(id: ThemeId, project: fn(SourceColor) -> Color) -> Theme {
    let source = cyril_dark_source(id);
    Theme {
        syntax: Some(source.syntax),
        canvas: project(source.canvas),
        chrome: project(source.chrome),
        code: project(source.code),
        selection: project(source.selection),
        text: project(source.text),
        muted: project(source.muted),
        border: project(source.border),
        accent: project(source.accent),
        accent_alt: project(source.accent_alt),
        user: project(source.user),
        agent: project(source.agent),
        system: project(source.system),
        info: project(source.info),
        success: project(source.success),
        warning: project(source.warning),
        danger: project(source.danger),
        diff_add: project(source.diff_add),
        diff_delete: project(source.diff_delete),
        diff_context: project(source.diff_context),
    }
}

const ANSI16_RGB: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (128, 0, 0),
    (0, 128, 0),
    (128, 128, 0),
    (0, 0, 128),
    (128, 0, 128),
    (0, 128, 128),
    (192, 192, 192),
    (128, 128, 128),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (0, 0, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

const ANSI16_COLORS: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

fn nearest_ansi256(rgb: (u8, u8, u8)) -> u8 {
    (16..=255)
        .min_by_key(|&index| (rgb_distance(rgb, xterm_rgb(index)), index))
        .unwrap_or(16)
}

fn nearest_ansi16(rgb: (u8, u8, u8)) -> u8 {
    ANSI16_RGB
        .into_iter()
        .enumerate()
        .min_by_key(|&(index, candidate)| (rgb_distance(rgb, candidate), index))
        .map_or(0, |(index, _)| index as u8)
}

fn xterm_rgb(index: u8) -> (u8, u8, u8) {
    if index < 232 {
        let offset = index - 16;
        let level = |value: u8| if value == 0 { 0 } else { 55 + 40 * value };
        (
            level(offset / 36),
            level((offset / 6) % 6),
            level(offset % 6),
        )
    } else {
        let gray = 8 + 10 * (index - 232);
        (gray, gray, gray)
    }
}

fn rgb_distance(left: (u8, u8, u8), right: (u8, u8, u8)) -> u32 {
    let square = |a: u8, b: u8| {
        let delta = i32::from(a) - i32::from(b);
        (delta * delta) as u32
    };
    square(left.0, right.0) + square(left.1, right.1) + square(left.2, right.2)
}

/// Resolve the built-in theme without reducing terminal color depth.
pub fn resolve_truecolor(id: ThemeId) -> Theme {
    resolve_with(id, SourceColor::truecolor)
}

/// Resolve the built-in theme against the fixed xterm 256-color palette.
pub fn resolve_ansi256(id: ThemeId) -> Theme {
    resolve_with(id, SourceColor::ansi256)
}

/// Resolve the built-in theme against the canonical ANSI-16 palette.
pub fn resolve_ansi16(id: ThemeId) -> Theme {
    resolve_with(id, SourceColor::ansi16)
}

/// Resolve the built-in theme without emitting color or syntax-color metadata.
pub fn resolve_no_color(id: ThemeId) -> Theme {
    Theme {
        syntax: None,
        ..resolve_with(id, |_| Color::Reset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syntect::highlighting::ThemeSet;

    const EXPECTED_ROLES: [&str; 19] = [
        "canvas",
        "chrome",
        "code",
        "selection",
        "text",
        "muted",
        "border",
        "accent",
        "accent_alt",
        "user",
        "agent",
        "system",
        "info",
        "success",
        "warning",
        "danger",
        "diff_add",
        "diff_delete",
        "diff_context",
    ];

    const EXPECTED_RGB: [(&str, SourceColor); 18] = [
        ("chrome", SourceColor::Rgb(0x1e, 0x1e, 0x2e)),
        ("code", SourceColor::Rgb(0x28, 0x2c, 0x34)),
        ("selection", SourceColor::Rgb(0x32, 0x32, 0x46)),
        ("text", SourceColor::Rgb(0xff, 0xff, 0xff)),
        ("muted", SourceColor::Rgb(0x8c, 0x8c, 0x8c)),
        ("border", SourceColor::Rgb(0x8c, 0x8c, 0x8c)),
        ("accent", SourceColor::Rgb(0x00, 0xff, 0xff)),
        ("accent_alt", SourceColor::Rgb(0xb4, 0x8e, 0xad)),
        ("user", SourceColor::Rgb(0x8a, 0xb4, 0xf8)),
        ("agent", SourceColor::Rgb(0x81, 0xc7, 0x84)),
        ("system", SourceColor::Rgb(0xb4, 0x8e, 0xad)),
        ("info", SourceColor::Rgb(0x00, 0xff, 0xff)),
        ("success", SourceColor::Rgb(0x00, 0xff, 0x00)),
        ("warning", SourceColor::Rgb(0xff, 0xff, 0x00)),
        ("danger", SourceColor::Rgb(0xff, 0x00, 0x00)),
        ("diff_add", SourceColor::Rgb(0x00, 0xff, 0x00)),
        ("diff_delete", SourceColor::Rgb(0xff, 0x00, 0x00)),
        ("diff_context", SourceColor::Rgb(0x8c, 0x8c, 0x8c)),
    ];

    fn resolved_roles(theme: Theme) -> [(&'static str, Color); 19] {
        [
            ("canvas", theme.canvas),
            ("chrome", theme.chrome),
            ("code", theme.code),
            ("selection", theme.selection),
            ("text", theme.text),
            ("muted", theme.muted),
            ("border", theme.border),
            ("accent", theme.accent),
            ("accent_alt", theme.accent_alt),
            ("user", theme.user),
            ("agent", theme.agent),
            ("system", theme.system),
            ("info", theme.info),
            ("success", theme.success),
            ("warning", theme.warning),
            ("danger", theme.danger),
            ("diff_add", theme.diff_add),
            ("diff_delete", theme.diff_delete),
            ("diff_context", theme.diff_context),
        ]
    }

    fn ansi16_index(color: Color) -> Option<u8> {
        match color {
            Color::Black => Some(0),
            Color::Red => Some(1),
            Color::Green => Some(2),
            Color::Yellow => Some(3),
            Color::Blue => Some(4),
            Color::Magenta => Some(5),
            Color::Cyan => Some(6),
            Color::Gray => Some(7),
            Color::DarkGray => Some(8),
            Color::LightRed => Some(9),
            Color::LightGreen => Some(10),
            Color::LightYellow => Some(11),
            Color::LightBlue => Some(12),
            Color::LightMagenta => Some(13),
            Color::LightCyan => Some(14),
            Color::White => Some(15),
            _ => None,
        }
    }

    fn synthetic_source() -> SourceTheme {
        SourceTheme {
            syntax: SyntaxTheme::Base16EightiesDark,
            canvas: SourceColor::Reset,
            chrome: SourceColor::Rgb(1, 0, 0),
            code: SourceColor::Rgb(2, 0, 0),
            selection: SourceColor::Rgb(3, 0, 0),
            text: SourceColor::Rgb(4, 0, 0),
            muted: SourceColor::Rgb(5, 0, 0),
            border: SourceColor::Rgb(6, 0, 0),
            accent: SourceColor::Rgb(7, 0, 0),
            accent_alt: SourceColor::Rgb(8, 0, 0),
            user: SourceColor::Rgb(9, 0, 0),
            agent: SourceColor::Rgb(10, 0, 0),
            system: SourceColor::Rgb(11, 0, 0),
            info: SourceColor::Rgb(12, 0, 0),
            success: SourceColor::Rgb(13, 0, 0),
            warning: SourceColor::Rgb(14, 0, 0),
            danger: SourceColor::Rgb(15, 0, 0),
            diff_add: SourceColor::Rgb(16, 0, 0),
            diff_delete: SourceColor::Rgb(17, 0, 0),
            diff_context: SourceColor::Rgb(18, 0, 0),
        }
    }

    #[test]
    fn source_shape_contains_every_semantic_role_once() {
        let actual: Vec<_> = synthetic_source()
            .roles()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(actual, EXPECTED_ROLES);
    }

    #[test]
    fn source_shape_has_one_reset_and_eighteen_rgb_roles() {
        let roles = synthetic_source().roles();
        let reset_count = roles
            .iter()
            .filter(|(_, color)| matches!(color, SourceColor::Reset))
            .count();
        let rgb_count = roles
            .iter()
            .filter(|(_, color)| matches!(color, SourceColor::Rgb(_, _, _)))
            .count();
        assert_eq!((reset_count, rgb_count), (1, 18));
    }

    #[test]
    fn cyril_dark_source_matches_the_signed_contract() {
        let source = cyril_dark_source(ThemeId::CyrilDark);
        let actual: Vec<_> = source
            .roles()
            .into_iter()
            .filter(|(_, color)| matches!(color, SourceColor::Rgb(_, _, _)))
            .collect();
        assert_eq!(actual, EXPECTED_RGB);
        assert_eq!(source.canvas, SourceColor::Reset);
        assert_eq!(source.syntax.name(), "base16-eighties.dark");
    }

    #[test]
    fn truecolor_preserves_rgb_values_and_reset() {
        let theme = resolve_truecolor(ThemeId::CyrilDark);
        assert_eq!(theme.canvas, Color::Reset);
        assert_eq!(theme.chrome, Color::Rgb(0x1e, 0x1e, 0x2e));
        assert_eq!(theme.text, Color::Rgb(0xff, 0xff, 0xff));
        assert_eq!(theme.muted, Color::Rgb(0x8c, 0x8c, 0x8c));
        assert_eq!(theme.accent, Color::Rgb(0x00, 0xff, 0xff));
        assert_eq!(theme.user, Color::Rgb(0x8a, 0xb4, 0xf8));
        assert_eq!(theme.syntax, Some(SyntaxTheme::Base16EightiesDark));
    }

    #[test]
    fn ansi256_uses_nearest_fixed_xterm_entry() {
        let theme = resolve_ansi256(ThemeId::CyrilDark);
        assert_eq!(theme.canvas, Color::Reset);
        assert_eq!(theme.chrome, Color::Indexed(235));
        assert_eq!(theme.code, Color::Indexed(236));
        assert_eq!(theme.selection, Color::Indexed(237));
        assert_eq!(theme.muted, Color::Indexed(245));
        assert_eq!(theme.user, Color::Indexed(111));
        assert_eq!(theme.syntax, Some(SyntaxTheme::Base16EightiesDark));
    }

    #[test]
    fn ansi256_ties_choose_the_lower_palette_index() {
        assert_eq!(nearest_ansi256((13, 13, 13)), 232);
    }

    #[test]
    fn ansi16_ties_choose_the_lower_palette_index() {
        assert_eq!(nearest_ansi16((64, 0, 0)), 0);
    }

    #[test]
    fn tie_break_is_candidate_order_independent() {
        let ansi256_rgb = (13, 13, 13);
        let reversed_ansi256 = (16..=255)
            .rev()
            .min_by_key(|&index| (rgb_distance(ansi256_rgb, xterm_rgb(index)), index))
            .unwrap_or(16);
        assert_eq!(reversed_ansi256, nearest_ansi256(ansi256_rgb));

        let ansi16_rgb = (64, 0, 0);
        let reversed_ansi16 = ANSI16_RGB
            .into_iter()
            .enumerate()
            .rev()
            .min_by_key(|&(index, candidate)| (rgb_distance(ansi16_rgb, candidate), index))
            .map_or(0, |(index, _)| index as u8);
        assert_eq!(reversed_ansi16, nearest_ansi16(ansi16_rgb));
    }

    #[test]
    fn ansi16_uses_nearest_canonical_entry_and_named_color() {
        let theme = resolve_ansi16(ThemeId::CyrilDark);
        assert_eq!(theme.canvas, Color::Reset);
        assert_eq!(theme.chrome, Color::Black);
        assert_eq!(theme.selection, Color::Blue);
        assert_eq!(theme.muted, Color::DarkGray);
        assert_eq!(theme.accent, Color::LightCyan);
        assert_eq!(theme.user, Color::Gray);
        assert_eq!(theme.agent, Color::DarkGray);
        assert_eq!(theme.success, Color::LightGreen);
        assert_eq!(theme.warning, Color::LightYellow);
        assert_eq!(theme.danger, Color::LightRed);
        assert_eq!(theme.syntax, Some(SyntaxTheme::Base16EightiesDark));
    }

    #[test]
    fn cyril_dark_syntax_theme_exists() {
        let themes = ThemeSet::load_defaults();
        assert!(
            themes
                .themes
                .contains_key(SyntaxTheme::Base16EightiesDark.name())
        );
        assert!(!themes.themes.contains_key("base16-eighties.drak"));
    }

    #[test]
    fn no_color_resets_every_role_and_disables_syntax_color() {
        let theme = resolve_no_color(ThemeId::CyrilDark);
        assert!(
            resolved_roles(theme)
                .into_iter()
                .all(|(_, color)| color == Color::Reset)
        );
        assert_eq!(theme.syntax, None);
    }

    #[test]
    fn seam_has_no_widget_references() {
        let widget_sources = [
            include_str!("widgets/approval.rs"),
            include_str!("widgets/chat.rs"),
            include_str!("widgets/code_panel.rs"),
            include_str!("widgets/crew_panel.rs"),
            include_str!("widgets/hooks_panel.rs"),
            include_str!("widgets/input.rs"),
            include_str!("widgets/markdown.rs"),
            include_str!("widgets/mod.rs"),
            include_str!("widgets/picker.rs"),
            include_str!("widgets/suggestions.rs"),
            include_str!("widgets/toolbar.rs"),
            include_str!("widgets/voice.rs"),
        ];
        let scanned_bytes: usize = widget_sources.iter().map(|source| source.len()).sum();

        assert!(widget_sources.len() <= 16);
        assert!(scanned_bytes <= 300_000);
        for source in widget_sources {
            assert!(!source.contains("crate::theme"));
            assert!(!source.contains("theme::"));
            assert!(!source.contains("ThemeId"));
            assert!(!source.contains("ColorMode"));
        }
    }

    #[test]
    fn emit_source_probe() {
        println!("BEGIN_THEME_PROBE");
        println!("role\trgb\tansi256\tansi16");
        let truecolor = resolved_roles(resolve_truecolor(ThemeId::CyrilDark));
        let ansi256 = resolved_roles(resolve_ansi256(ThemeId::CyrilDark));
        let ansi16 = resolved_roles(resolve_ansi16(ThemeId::CyrilDark));
        for (((name, source), (_, projected256)), (_, projected16)) in
            truecolor.into_iter().zip(ansi256).zip(ansi16)
        {
            if let (Color::Rgb(r, g, b), Color::Indexed(index256), Some(index16)) =
                (source, projected256, ansi16_index(projected16))
            {
                println!("{name}\t{r:02x}{g:02x}{b:02x}\t{index256}\t{index16}");
            }
        }
        println!("END_THEME_PROBE");
    }
}

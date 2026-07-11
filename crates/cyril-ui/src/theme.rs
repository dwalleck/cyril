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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntaxThemeId {
    Base16EightiesDark,
}

impl SyntaxThemeId {
    const fn name(self) -> &'static str {
        match self {
            Self::Base16EightiesDark => "base16-eighties.dark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceTheme {
    syntax: SyntaxThemeId,
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
            syntax: SyntaxThemeId::Base16EightiesDark,
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

#[cfg(test)]
mod tests {
    use super::*;

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

    fn synthetic_source() -> SourceTheme {
        SourceTheme {
            syntax: SyntaxThemeId::Base16EightiesDark,
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
    fn emit_source_probe() {
        println!("BEGIN_THEME_PROBE");
        println!("role\trgb");
        for (name, color) in cyril_dark_source(ThemeId::CyrilDark).roles() {
            if let SourceColor::Rgb(r, g, b) = color {
                println!("{name}\t{r:02x}{g:02x}{b:02x}");
            }
        }
        println!("END_THEME_PROBE");
    }
}

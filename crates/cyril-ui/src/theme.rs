use ratatui::style::Color;

macro_rules! bundled_theme_ids {
    (
        $(#[$meta:meta])*
        $visibility:vis enum $name:ident {
            $($variant:ident),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $visibility enum $name {
            $($variant),+
        }

        impl $name {
            /// Every bundled theme, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Stable identifier used by exhaustive contract probes.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),+
                }
            }
        }
    };
}

bundled_theme_ids! {
    /// Bundled visual theme identifier.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ThemeId {
        CyrilDark,
    }
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
    emphasis: SourceColor,
    accent_tertiary: SourceColor,
    accent_quaternary: SourceColor,
    accent_quinary: SourceColor,
    subdued: SourceColor,
    subdued_positive: SourceColor,
    subdued_negative: SourceColor,
    soft_accent: SourceColor,
    positive_accent: SourceColor,
    inset_background: SourceColor,
    text_secondary: SourceColor,
    accent_violet: SourceColor,
}

#[cfg(test)]
impl SourceTheme {
    fn roles(self) -> [(&'static str, SourceColor); 31] {
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
            ("emphasis", self.emphasis),
            ("accent_tertiary", self.accent_tertiary),
            ("accent_quaternary", self.accent_quaternary),
            ("accent_quinary", self.accent_quinary),
            ("subdued", self.subdued),
            ("subdued_positive", self.subdued_positive),
            ("subdued_negative", self.subdued_negative),
            ("soft_accent", self.soft_accent),
            ("positive_accent", self.positive_accent),
            ("inset_background", self.inset_background),
            ("text_secondary", self.text_secondary),
            ("accent_violet", self.accent_violet),
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
            emphasis: SourceColor::Rgb(0x80, 0x80, 0x00),
            accent_tertiary: SourceColor::Rgb(0x00, 0x00, 0x80),
            accent_quaternary: SourceColor::Rgb(0x80, 0x00, 0x80),
            accent_quinary: SourceColor::Rgb(0x00, 0x80, 0x80),
            subdued: SourceColor::Rgb(0x80, 0x80, 0x80),
            subdued_positive: SourceColor::Rgb(0x00, 0x80, 0x00),
            subdued_negative: SourceColor::Rgb(0x80, 0x00, 0x00),
            soft_accent: SourceColor::Rgb(0x8a, 0xb4, 0xf8),
            positive_accent: SourceColor::Rgb(0x81, 0xc7, 0x84),
            inset_background: SourceColor::Rgb(0x28, 0x2c, 0x34),
            text_secondary: SourceColor::Rgb(0xc0, 0xc0, 0xc0),
            accent_violet: SourceColor::Rgb(0xb0, 0x8d, 0xff),
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
    pub emphasis: Color,
    pub accent_tertiary: Color,
    pub accent_quaternary: Color,
    pub accent_quinary: Color,
    pub subdued: Color,
    pub subdued_positive: Color,
    pub subdued_negative: Color,
    pub soft_accent: Color,
    pub positive_accent: Color,
    pub inset_background: Color,
    pub text_secondary: Color,
    pub accent_violet: Color,
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
        emphasis: project(source.emphasis),
        accent_tertiary: project(source.accent_tertiary),
        accent_quaternary: project(source.accent_quaternary),
        accent_quinary: project(source.accent_quinary),
        subdued: project(source.subdued),
        subdued_positive: project(source.subdued_positive),
        subdued_negative: project(source.subdued_negative),
        soft_accent: project(source.soft_accent),
        positive_accent: project(source.positive_accent),
        inset_background: project(source.inset_background),
        text_secondary: project(source.text_secondary),
        accent_violet: project(source.accent_violet),
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

fn nearest_palette<I>(rgb: (u8, u8, u8), first: (u8, (u8, u8, u8)), candidates: I) -> u8
where
    I: IntoIterator<Item = (u8, (u8, u8, u8))>,
{
    let initial = (first.0, rgb_distance(rgb, first.1));
    candidates
        .into_iter()
        .fold(initial, |best, (index, candidate)| {
            let distance = rgb_distance(rgb, candidate);
            if (distance, index) < (best.1, best.0) {
                (index, distance)
            } else {
                best
            }
        })
        .0
}

fn nearest_ansi256(rgb: (u8, u8, u8)) -> u8 {
    nearest_palette(
        rgb,
        (16, xterm_rgb(16)),
        (17..=255).map(|index| (index, xterm_rgb(index))),
    )
}

fn nearest_ansi16(rgb: (u8, u8, u8)) -> u8 {
    nearest_palette(
        rgb,
        (0, ANSI16_RGB[0]),
        ANSI16_RGB
            .into_iter()
            .enumerate()
            .skip(1)
            .map(|(index, candidate)| (index as u8, candidate)),
    )
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

/// Resolve a built-in theme for an explicit terminal color capability.
pub fn resolve(id: ThemeId, mode: ColorMode) -> Theme {
    match mode {
        ColorMode::TrueColor => resolve_truecolor(id),
        ColorMode::Ansi256 => resolve_ansi256(id),
        ColorMode::Ansi16 => resolve_ansi16(id),
        ColorMode::None => resolve_no_color(id),
    }
}

/// Resolve the built-in theme without reducing terminal color depth.
pub fn resolve_truecolor(id: ThemeId) -> Theme {
    resolve_with(id, SourceColor::truecolor)
}

/// Resolve the built-in theme against the fixed xterm 256-color palette.
pub fn resolve_ansi256(id: ThemeId) -> Theme {
    resolve_with(id, SourceColor::ansi256)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ansi16ContractViolation {
    role: &'static str,
    color: Color,
}

fn apply_ansi16_semantics(mut theme: Theme) -> Result<Theme, Ansi16ContractViolation> {
    for (role, color) in [
        ("muted", theme.muted),
        ("border", theme.border),
        ("subdued", theme.subdued),
        ("diff_context", theme.diff_context),
    ] {
        if matches!(
            color,
            Color::LightBlue | Color::LightGreen | Color::LightMagenta
        ) {
            return Err(Ansi16ContractViolation { role, color });
        }
    }

    theme.user = Color::LightBlue;
    theme.agent = Color::LightGreen;
    theme.system = Color::LightMagenta;
    Ok(theme)
}

/// Resolve the built-in theme against the canonical ANSI-16 palette.
///
/// # Panics
///
/// Panics when a bundled theme projects a muted-family role into one of the
/// three protected speaker slots. Bundled themes are compile-time project data,
/// so this indicates a violated theme contract rather than invalid operator
/// input.
pub fn resolve_ansi16(id: ThemeId) -> Theme {
    match apply_ansi16_semantics(resolve_with(id, SourceColor::ansi16)) {
        Ok(theme) => theme,
        Err(error) => panic!(
            "bundled theme ANSI-16 contract violation: {} projects to protected speaker slot {:?}",
            error.role, error.color
        ),
    }
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

    bundled_theme_ids! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum RegistryStressTheme {
            Alpha,
            Beta,
            Gamma,
        }
    }

    #[test]
    fn bundled_theme_registry_is_complete_and_unique() {
        assert_eq!(ThemeId::ALL, &[ThemeId::CyrilDark]);
        assert_eq!(ThemeId::CyrilDark.name(), "CyrilDark");
    }

    #[test]
    fn bundled_theme_registry_keeps_middle_variants() {
        assert_eq!(
            RegistryStressTheme::ALL,
            &[
                RegistryStressTheme::Alpha,
                RegistryStressTheme::Beta,
                RegistryStressTheme::Gamma,
            ]
        );
        assert_eq!(RegistryStressTheme::Beta.name(), "Beta");
    }

    const EXPECTED_ROLES: [&str; 31] = [
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
        "emphasis",
        "accent_tertiary",
        "accent_quaternary",
        "accent_quinary",
        "subdued",
        "subdued_positive",
        "subdued_negative",
        "soft_accent",
        "positive_accent",
        "inset_background",
        "text_secondary",
        "accent_violet",
    ];

    const EXPECTED_RGB: [(&str, SourceColor); 30] = [
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
        ("emphasis", SourceColor::Rgb(0x80, 0x80, 0x00)),
        ("accent_tertiary", SourceColor::Rgb(0x00, 0x00, 0x80)),
        ("accent_quaternary", SourceColor::Rgb(0x80, 0x00, 0x80)),
        ("accent_quinary", SourceColor::Rgb(0x00, 0x80, 0x80)),
        ("subdued", SourceColor::Rgb(0x80, 0x80, 0x80)),
        ("subdued_positive", SourceColor::Rgb(0x00, 0x80, 0x00)),
        ("subdued_negative", SourceColor::Rgb(0x80, 0x00, 0x00)),
        ("soft_accent", SourceColor::Rgb(0x8a, 0xb4, 0xf8)),
        ("positive_accent", SourceColor::Rgb(0x81, 0xc7, 0x84)),
        ("inset_background", SourceColor::Rgb(0x28, 0x2c, 0x34)),
        ("text_secondary", SourceColor::Rgb(0xc0, 0xc0, 0xc0)),
        ("accent_violet", SourceColor::Rgb(0xb0, 0x8d, 0xff)),
    ];

    fn resolved_roles(theme: Theme) -> [(&'static str, Color); 31] {
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
            ("emphasis", theme.emphasis),
            ("accent_tertiary", theme.accent_tertiary),
            ("accent_quaternary", theme.accent_quaternary),
            ("accent_quinary", theme.accent_quinary),
            ("subdued", theme.subdued),
            ("subdued_positive", theme.subdued_positive),
            ("subdued_negative", theme.subdued_negative),
            ("soft_accent", theme.soft_accent),
            ("positive_accent", theme.positive_accent),
            ("inset_background", theme.inset_background),
            ("text_secondary", theme.text_secondary),
            ("accent_violet", theme.accent_violet),
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
            emphasis: SourceColor::Rgb(19, 0, 0),
            accent_tertiary: SourceColor::Rgb(20, 0, 0),
            accent_quaternary: SourceColor::Rgb(21, 0, 0),
            accent_quinary: SourceColor::Rgb(22, 0, 0),
            subdued: SourceColor::Rgb(23, 0, 0),
            subdued_positive: SourceColor::Rgb(24, 0, 0),
            subdued_negative: SourceColor::Rgb(25, 0, 0),
            soft_accent: SourceColor::Rgb(26, 0, 0),
            positive_accent: SourceColor::Rgb(27, 0, 0),
            inset_background: SourceColor::Rgb(28, 0, 0),
            text_secondary: SourceColor::Rgb(29, 0, 0),
            accent_violet: SourceColor::Rgb(30, 0, 0),
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
    fn source_shape_has_one_reset_and_thirty_rgb_roles() {
        let roles = synthetic_source().roles();
        let reset_count = roles
            .iter()
            .filter(|(_, color)| matches!(color, SourceColor::Reset))
            .count();
        let rgb_count = roles
            .iter()
            .filter(|(_, color)| matches!(color, SourceColor::Rgb(_, _, _)))
            .count();
        assert_eq!((reset_count, rgb_count), (1, 30));
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
    fn conversation_legacy_colors_are_representable() {
        let available = cyril_dark_source(ThemeId::CyrilDark).roles();
        let required = [
            SourceColor::Rgb(0x8a, 0xb4, 0xf8),
            SourceColor::Rgb(0x81, 0xc7, 0x84),
            SourceColor::Rgb(0xb4, 0x8e, 0xad),
            SourceColor::Rgb(0x8c, 0x8c, 0x8c),
            SourceColor::Rgb(0x28, 0x2c, 0x34),
            SourceColor::Rgb(0x80, 0x00, 0x00),
            SourceColor::Rgb(0x00, 0x80, 0x00),
            SourceColor::Rgb(0x80, 0x80, 0x00),
            SourceColor::Rgb(0x00, 0x00, 0x80),
            SourceColor::Rgb(0x80, 0x00, 0x80),
            SourceColor::Rgb(0x00, 0x80, 0x80),
            SourceColor::Rgb(0x80, 0x80, 0x80),
            SourceColor::Rgb(0xff, 0xff, 0xff),
        ];

        for color in required {
            assert!(
                available.iter().any(|(_, candidate)| *candidate == color),
                "legacy color {color:?} is not represented"
            );
        }
    }

    /// cyril-nrnq C1: every canonical RGB value in the modal legacy
    /// inventory (.cyril-nrnq/probe-styles.txt via the ghuu NAMED canon)
    /// is representable in the expanded contract.
    #[test]
    fn modal_legacy_colors_are_representable() {
        let available = cyril_dark_source(ThemeId::CyrilDark).roles();
        let required = [
            SourceColor::Rgb(0x32, 0x32, 0x46), // Rgb(50,50,70) selection bg
            SourceColor::Rgb(0xff, 0xff, 0xff), // Color::White
            SourceColor::Rgb(0x00, 0x80, 0x80), // Color::Cyan
            SourceColor::Rgb(0x80, 0x80, 0x80), // Color::DarkGray
            SourceColor::Rgb(0x80, 0x80, 0x00), // Color::Yellow
            SourceColor::Rgb(0x00, 0x80, 0x00), // Color::Green
            SourceColor::Rgb(0x80, 0x00, 0x00), // Color::Red
            SourceColor::Rgb(0xc0, 0xc0, 0xc0), // Color::Gray -> text_secondary
            SourceColor::Rgb(0xb0, 0x8d, 0xff), // matcher purple -> accent_violet
        ];

        for color in required {
            assert!(
                available.iter().any(|(_, candidate)| *candidate == color),
                "modal legacy color {color:?} is not represented"
            );
        }
    }

    /// cyril-dij8 C1: every canonical RGB value in the chrome legacy
    /// inventory (.cyril-dij8/probe-styles.txt via the ghuu NAMED canon)
    /// is representable in the 31-role contract — the first pure
    /// re-mapping batch (no expansion).
    #[test]
    fn chrome_legacy_colors_are_representable() {
        let available = cyril_dark_source(ThemeId::CyrilDark).roles();
        let required = [
            SourceColor::Rgb(0x1e, 0x1e, 0x2e), // Rgb(30,30,46) chrome bg
            SourceColor::Rgb(0xff, 0xff, 0xff), // Color::White
            SourceColor::Rgb(0x80, 0x80, 0x80), // Color::DarkGray
            SourceColor::Rgb(0xc0, 0xc0, 0xc0), // Color::Gray
            SourceColor::Rgb(0x80, 0x80, 0x00), // Color::Yellow
            SourceColor::Rgb(0x00, 0x80, 0x00), // Color::Green
            SourceColor::Rgb(0x80, 0x00, 0x00), // Color::Red
            SourceColor::Rgb(0x00, 0x80, 0x80), // Color::Cyan
            SourceColor::Rgb(0x80, 0x00, 0x80), // Color::Magenta
            SourceColor::Rgb(0x8a, 0xb4, 0xf8), // palette::USER_BLUE
            SourceColor::Rgb(0x8c, 0x8c, 0x8c), // palette::MUTED_GRAY
            SourceColor::Rgb(0xb4, 0x8e, 0xad), // palette::SYSTEM_MAUVE
        ];

        for (i, color_a) in required.iter().enumerate() {
            for color_b in required.iter().skip(i + 1) {
                assert_ne!(
                    color_a, color_b,
                    "chrome inventory transcription duplicates {color_a:?}"
                );
            }
        }
        for color in required {
            assert!(
                available.iter().any(|(_, candidate)| *candidate == color),
                "chrome legacy color {color:?} is not represented"
            );
        }
    }

    /// Production section of a widget source file (everything above the
    /// first `#[cfg(test)]`), shared by the source-fence tests.
    fn production_source(source: &str) -> &str {
        source
            .split_once("#[cfg(test)]")
            .map_or(source, |(production, _)| production)
    }

    /// cyril-nrnq C5: the four migrated modal widgets carry zero hardcoded
    /// color literals in production code (allowlist: empty).
    #[test]
    fn modal_widgets_have_no_legacy_color_sources() {
        let sources = [
            ("approval", include_str!("widgets/approval.rs")),
            ("picker", include_str!("widgets/picker.rs")),
            ("hooks_panel", include_str!("widgets/hooks_panel.rs")),
            ("code_panel", include_str!("widgets/code_panel.rs")),
        ];
        for (name, source) in sources {
            assert!(
                !production_source(source).contains("Color::"),
                "{name} still hardcodes a Color:: literal"
            );
        }
    }

    /// cyril-dij8 C4: the three migrated chrome widget files carry zero
    /// hardcoded color literals AND zero palette color-constant references
    /// in production code (the spinner constants are not colors and stay).
    /// One-shot non-vacuity control: this predicate FAILS against the
    /// pre-migration toolbar.rs at d4f105f (26 Color:: literals).
    #[test]
    fn chrome_widgets_have_no_legacy_color_sources() {
        let sources = [
            ("toolbar", include_str!("widgets/toolbar.rs")),
            ("crew_panel", include_str!("widgets/crew_panel.rs")),
            ("voice", include_str!("widgets/voice.rs")),
        ];
        let palette_colors = [
            "USER_BLUE",
            "AGENT_GREEN",
            "SYSTEM_MAUVE",
            "MUTED_GRAY",
            "CODE_BLOCK_BG",
        ];
        for (name, source) in sources {
            let production = production_source(source);
            assert!(
                !production.contains("Color::"),
                "{name} still hardcodes a Color:: literal"
            );
            for constant in palette_colors {
                assert!(
                    !production.contains(constant),
                    "{name} still references palette::{constant}"
                );
            }
        }
    }

    /// cyril-nrnq slice-2 stress: a duplicated marker value would blind the
    /// C4 wiring fences — all 31 marker roles must be pairwise distinct.
    #[test]
    fn marker_theme_roles_are_pairwise_distinct() {
        let roles = resolved_roles(crate::traits::test_support::marker_theme());
        for (i, (name_a, color_a)) in roles.iter().enumerate() {
            for (name_b, color_b) in roles.iter().skip(i + 1) {
                assert_ne!(
                    color_a, color_b,
                    "marker theme roles {name_a} and {name_b} share a value"
                );
            }
        }
    }

    #[test]
    fn first_five_compatibility_roles_match_signed_values() {
        let actual = cyril_dark_source(ThemeId::CyrilDark).roles();
        let expected = [
            ("emphasis", SourceColor::Rgb(0x80, 0x80, 0x00)),
            ("accent_tertiary", SourceColor::Rgb(0x00, 0x00, 0x80)),
            ("accent_quaternary", SourceColor::Rgb(0x80, 0x00, 0x80)),
            ("accent_quinary", SourceColor::Rgb(0x00, 0x80, 0x80)),
            ("subdued", SourceColor::Rgb(0x80, 0x80, 0x80)),
        ];

        assert_eq!(actual.len(), 31);
        for role in expected {
            assert!(
                actual.contains(&role),
                "missing compatibility role {role:?}"
            );
        }
    }

    #[test]
    fn complete_compatibility_contract_has_thirty_one_roles() {
        let actual = cyril_dark_source(ThemeId::CyrilDark).roles();
        let expected = [
            ("subdued_positive", SourceColor::Rgb(0x00, 0x80, 0x00)),
            ("subdued_negative", SourceColor::Rgb(0x80, 0x00, 0x00)),
            ("soft_accent", SourceColor::Rgb(0x8a, 0xb4, 0xf8)),
            ("positive_accent", SourceColor::Rgb(0x81, 0xc7, 0x84)),
            ("inset_background", SourceColor::Rgb(0x28, 0x2c, 0x34)),
        ];

        assert_eq!(actual.len(), 31);
        for role in expected {
            assert!(
                actual.contains(&role),
                "missing compatibility role {role:?}"
            );
        }
    }

    #[test]
    fn explicit_color_mode_dispatches_to_each_projection() {
        let id = ThemeId::CyrilDark;
        assert_eq!(resolve(id, ColorMode::TrueColor), resolve_truecolor(id));
        assert_eq!(resolve(id, ColorMode::Ansi256), resolve_ansi256(id));
        assert_eq!(resolve(id, ColorMode::Ansi16), resolve_ansi16(id));
        assert_eq!(resolve(id, ColorMode::None), resolve_no_color(id));
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
        assert_eq!(theme.emphasis, Color::Rgb(0x80, 0x80, 0x00));
        assert_eq!(theme.accent_tertiary, Color::Rgb(0x00, 0x00, 0x80));
        assert_eq!(theme.accent_quaternary, Color::Rgb(0x80, 0x00, 0x80));
        assert_eq!(theme.accent_quinary, Color::Rgb(0x00, 0x80, 0x80));
        assert_eq!(theme.subdued, Color::Rgb(0x80, 0x80, 0x80));
        assert_eq!(theme.subdued_positive, Color::Rgb(0x00, 0x80, 0x00));
        assert_eq!(theme.subdued_negative, Color::Rgb(0x80, 0x00, 0x00));
        assert_eq!(theme.soft_accent, Color::Rgb(0x8a, 0xb4, 0xf8));
        assert_eq!(theme.positive_accent, Color::Rgb(0x81, 0xc7, 0x84));
        assert_eq!(theme.inset_background, Color::Rgb(0x28, 0x2c, 0x34));
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
        let reversed_ansi256 = nearest_palette(
            ansi256_rgb,
            (255, xterm_rgb(255)),
            (16u8..255).rev().map(|index| (index, xterm_rgb(index))),
        );
        assert_eq!(reversed_ansi256, nearest_ansi256(ansi256_rgb));

        let ansi16_rgb = (64, 0, 0);
        let reversed_ansi16 = nearest_palette(
            ansi16_rgb,
            (15, ANSI16_RGB[15]),
            (0u8..15)
                .rev()
                .map(|index| (index, ANSI16_RGB[usize::from(index)])),
        );
        assert_eq!(reversed_ansi16, nearest_ansi16(ansi16_rgb));
    }

    fn set_muted_family_role(theme: &mut Theme, role: &str, color: Color) {
        match role {
            "muted" => theme.muted = color,
            "border" => theme.border = color,
            "subdued" => theme.subdued = color,
            "diff_context" => theme.diff_context = color,
            _ => panic!("unknown muted-family role: {role}"),
        }
    }

    #[test]
    fn ansi16_speaker_roles_use_semantic_slots() {
        for theme_id in ThemeId::ALL.iter().copied() {
            let theme = resolve_ansi16(theme_id);
            assert_eq!(theme.user, Color::LightBlue);
            assert_eq!(theme.agent, Color::LightGreen);
            assert_eq!(theme.system, Color::LightMagenta);
        }
    }

    #[test]
    fn ansi16_rejects_all_muted_speaker_slot_collisions() {
        let geometric = resolve_with(ThemeId::CyrilDark, SourceColor::ansi16);
        for role in ["muted", "border", "subdued", "diff_context"] {
            for color in [Color::LightBlue, Color::LightGreen, Color::LightMagenta] {
                let mut candidate = geometric;
                set_muted_family_role(&mut candidate, role, color);
                let error = match apply_ansi16_semantics(candidate) {
                    Err(error) => error,
                    Ok(_) => panic!("protected muted-family collision must fail"),
                };
                assert_eq!(error.role, role);
                assert_eq!(error.color, color);
            }
        }
    }

    #[test]
    fn ansi16_semantics_change_only_speaker_roles() {
        for theme_id in ThemeId::ALL.iter().copied() {
            let geometric = resolved_roles(resolve_with(theme_id, SourceColor::ansi16));
            let semantic = resolved_roles(resolve_ansi16(theme_id));
            let mut changed = 0;
            for ((source_name, source), (semantic_name, projected)) in
                geometric.into_iter().zip(semantic)
            {
                assert_eq!(source_name, semantic_name);
                let semantic_slot = match source_name {
                    "user" => Some(Color::LightBlue),
                    "agent" => Some(Color::LightGreen),
                    "system" => Some(Color::LightMagenta),
                    _ => None,
                };
                match semantic_slot {
                    Some(slot) => {
                        assert_eq!(projected, slot, "{source_name}");
                        if source != projected {
                            changed += 1;
                        }
                    }
                    None => assert_eq!(source, projected, "{source_name}"),
                }
            }
            // Signed claim C3 pins the changed-row count for CyrilDark only.
            // A future bundled theme whose speaker source already projects
            // geometrically onto its semantic slot is contract-valid, so its
            // count may be below 3 (mirrors acceptance-oracle.py scoping).
            if theme_id == ThemeId::CyrilDark {
                assert_eq!(changed, 3);
            }
        }
    }

    #[test]
    fn non_speaker_ansi16_remains_nearest() {
        for theme_id in ThemeId::ALL.iter().copied() {
            let source = resolved_roles(resolve_truecolor(theme_id));
            let projected = resolved_roles(resolve_ansi16(theme_id));
            let mut checked = 0;
            for ((source_name, source_color), (projected_name, projected_color)) in
                source.into_iter().zip(projected)
            {
                assert_eq!(source_name, projected_name);
                if matches!(source_name, "user" | "agent" | "system") {
                    continue;
                }
                if let Color::Rgb(r, g, b) = source_color {
                    let expected = ANSI16_COLORS[usize::from(nearest_ansi16((r, g, b)))];
                    assert_eq!(projected_color, expected, "{source_name}");
                    checked += 1;
                }
            }
            assert_eq!(checked, 27);
        }
    }

    #[test]
    fn resolution_is_deterministic_in_every_mode() {
        for theme_id in ThemeId::ALL.iter().copied() {
            for mode in [
                ColorMode::TrueColor,
                ColorMode::Ansi256,
                ColorMode::Ansi16,
                ColorMode::None,
            ] {
                assert_eq!(resolve(theme_id, mode), resolve(theme_id, mode));
            }
        }
    }

    #[test]
    fn ansi16_semantics_preserve_syntax_component() {
        for theme_id in ThemeId::ALL.iter().copied() {
            let geometric = resolve_with(theme_id, SourceColor::ansi16);
            assert_eq!(resolve_ansi16(theme_id).syntax, geometric.syntax);
            assert_eq!(resolve_no_color(theme_id).syntax, None);
        }
    }

    #[test]
    fn ansi16_uses_canonical_palette_and_semantic_speaker_slots() {
        let theme = resolve_ansi16(ThemeId::CyrilDark);
        assert_eq!(theme.canvas, Color::Reset);
        assert_eq!(theme.chrome, Color::Black);
        assert_eq!(theme.selection, Color::Blue);
        assert_eq!(theme.muted, Color::DarkGray);
        assert_eq!(theme.accent, Color::LightCyan);
        assert_eq!(theme.user, Color::LightBlue);
        assert_eq!(theme.agent, Color::LightGreen);
        assert_eq!(theme.system, Color::LightMagenta);
        assert_eq!(theme.success, Color::LightGreen);
        assert_eq!(theme.warning, Color::LightYellow);
        assert_eq!(theme.danger, Color::LightRed);
        // cyril-nrnq: #c0c0c0 IS ANSI16_RGB[7] — distance-0 must hit index 7
        // (off-by-one stress); #b08dff's Euclidean nearest is also Gray
        // (desaturated light purple; pinned by the independent brute-force
        // oracle in .cyril-nrnq/build-audit.md, not by intuition).
        assert_eq!(theme.text_secondary, Color::Gray);
        assert_eq!(theme.accent_violet, Color::Gray);
        assert_eq!(theme.syntax, Some(SyntaxTheme::Base16EightiesDark));
    }

    #[test]
    fn all_roles_project() {
        let truecolor = resolved_roles(resolve_truecolor(ThemeId::CyrilDark));
        let ansi256 = resolved_roles(resolve_ansi256(ThemeId::CyrilDark));
        let ansi16 = resolved_roles(resolve_ansi16(ThemeId::CyrilDark));
        let mut projected = 0;

        for (((source_name, source), (ansi256_name, projected256)), (ansi16_name, projected16)) in
            truecolor.into_iter().zip(ansi256).zip(ansi16)
        {
            assert_eq!(source_name, ansi256_name);
            assert_eq!(source_name, ansi16_name);
            if matches!(source, Color::Rgb(_, _, _)) {
                assert!(matches!(projected256, Color::Indexed(16..=255)));
                assert!(ansi16_index(projected16).is_some());
                projected += 1;
            }
        }

        assert_eq!(projected, 30);
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
    fn no_color_resets_all_29_roles() {
        let theme = resolve_no_color(ThemeId::CyrilDark);
        assert!(
            resolved_roles(theme)
                .into_iter()
                .all(|(_, color)| color == Color::Reset)
        );
        assert_eq!(theme.syntax, None);
    }

    #[test]
    fn widgets_only_use_the_explicit_theme() {
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
        let production_sources = widget_sources.map(production_source);
        let scanned_bytes: usize = production_sources.iter().map(|source| source.len()).sum();
        assert!(production_sources.len() <= 16);
        assert!(scanned_bytes <= 300_000);
        for source in production_sources {
            let source_without_allowed_seams = source
                .replace("use crate::theme::Theme;", "")
                .replace("use crate::theme::{ColorMode, Theme, ThemeId};", "");
            assert!(!source_without_allowed_seams.contains("crate::theme"));
            assert!(!source_without_allowed_seams.contains("theme::"));
            assert!(!source_without_allowed_seams.contains("ThemeId"));
            assert!(!source_without_allowed_seams.contains("ColorMode"));
            assert!(!source_without_allowed_seams.contains("resolve("));
        }
    }

    #[test]
    fn emit_theme_registry_probe() {
        println!("BEGIN_THEME_REGISTRY");
        println!("index\ttheme");
        for (index, theme_id) in ThemeId::ALL.iter().copied().enumerate() {
            println!("{index}\t{}", theme_id.name());
        }
        println!("END_THEME_REGISTRY");
    }

    fn probe_source(color: Color) -> String {
        match color {
            Color::Rgb(r, g, b) => format!("{r:02x}{g:02x}{b:02x}"),
            Color::Reset => "reset".into(),
            other => format!("unexpected:{other:?}"),
        }
    }

    fn probe_ansi256(color: Color) -> String {
        match color {
            Color::Indexed(index) => index.to_string(),
            Color::Reset => "reset".into(),
            other => format!("unexpected:{other:?}"),
        }
    }

    fn probe_ansi16(color: Color) -> String {
        match color {
            Color::Reset => "reset".into(),
            other => ansi16_index(other).map_or_else(
                || format!("unexpected:{other:?}"),
                |index| index.to_string(),
            ),
        }
    }

    #[test]
    fn emit_source_probe() {
        println!("BEGIN_THEME_PROBE");
        println!("theme\trole\tsource\tansi256\tansi16\tno_color\tcolor_syntax\tno_color_syntax");
        for theme_id in ThemeId::ALL.iter().copied() {
            let truecolor_theme = resolve_truecolor(theme_id);
            let ansi256_theme = resolve_ansi256(theme_id);
            let ansi16_theme = resolve_ansi16(theme_id);
            let no_color_theme = resolve_no_color(theme_id);
            let color_syntax = ansi16_theme.syntax.map_or("none", SyntaxTheme::name);
            let no_color_syntax = no_color_theme.syntax.map_or("none", SyntaxTheme::name);
            for ((((name, source), (_, projected256)), (_, projected16)), (_, no_color)) in
                resolved_roles(truecolor_theme)
                    .into_iter()
                    .zip(resolved_roles(ansi256_theme))
                    .zip(resolved_roles(ansi16_theme))
                    .zip(resolved_roles(no_color_theme))
            {
                println!(
                    "{}\t{name}\t{}\t{}\t{}\t{}\t{color_syntax}\t{no_color_syntax}",
                    theme_id.name(),
                    probe_source(source),
                    probe_ansi256(projected256),
                    probe_ansi16(projected16),
                    probe_source(no_color)
                );
            }
        }
        println!("END_THEME_PROBE");
    }

    #[test]
    fn emit_ansi16_collision_probe() {
        println!("BEGIN_ANSI16_COLLISION_PROBE");
        println!("input_role\tinput_color\tresult_role\tresult_color");
        let geometric = resolve_with(ThemeId::CyrilDark, SourceColor::ansi16);
        for role in ["muted", "border", "subdued", "diff_context"] {
            for color in [Color::LightBlue, Color::LightGreen, Color::LightMagenta] {
                let mut candidate = geometric;
                set_muted_family_role(&mut candidate, role, color);
                match apply_ansi16_semantics(candidate) {
                    Ok(_) => println!("{role}\t{}\taccepted\taccepted", probe_ansi16(color)),
                    Err(error) => println!(
                        "{role}\t{}\t{}\t{}",
                        probe_ansi16(color),
                        error.role,
                        probe_ansi16(error.color)
                    ),
                }
            }
        }
        println!("END_ANSI16_COLLISION_PROBE");
    }

    #[test]
    fn emit_ansi16_tie_probe() {
        println!("BEGIN_ANSI16_TIE_PROBE");
        println!("rgb\tindex");
        println!("400000\t{}", nearest_ansi16((64, 0, 0)));
        println!("END_ANSI16_TIE_PROBE");
    }

    #[test]
    fn emit_no_color_probe() {
        println!("BEGIN_NO_COLOR_PROBE");
        println!("theme\trole\tcolor");
        for theme_id in ThemeId::ALL.iter().copied() {
            for (name, color) in resolved_roles(resolve_no_color(theme_id)) {
                let value = if color == Color::Reset {
                    "reset"
                } else {
                    "concrete"
                };
                println!("{}\t{name}\t{value}", theme_id.name());
            }
            println!("{}\tsyntax\tnone", theme_id.name());
        }
        println!("END_NO_COLOR_PROBE");
    }
}

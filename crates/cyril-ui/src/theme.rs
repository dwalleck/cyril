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
    ///
    /// Declaration order is the operator-facing order used by the future
    /// activation picker (cyril-qaq0); `CyrilDark` stays first because it
    /// remains the startup default.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ThemeId {
        CyrilDark,
        CyrilLight,
        HighContrastDark,
        HighContrastLight,
        CatppuccinMocha,
        GruvboxDark,
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
///
/// Every variant names a component bundled with Syntect's default theme set;
/// `tests::all_bundled_syntax_themes_exist` fails if a name drifts out of that
/// set, so a palette cannot silently render unstyled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxTheme {
    Base16EightiesDark,
    Base16OceanLight,
    Base16OceanDark,
    InspiredGitHub,
    Base16MochaDark,
}

impl SyntaxTheme {
    /// Every bundled syntax component, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Base16EightiesDark,
        Self::Base16OceanLight,
        Self::Base16OceanDark,
        Self::InspiredGitHub,
        Self::Base16MochaDark,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Base16EightiesDark => "base16-eighties.dark",
            Self::Base16OceanLight => "base16-ocean.light",
            Self::Base16OceanDark => "base16-ocean.dark",
            Self::InspiredGitHub => "InspiredGitHub",
            Self::Base16MochaDark => "base16-mocha.dark",
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

fn source(id: ThemeId) -> SourceTheme {
    match id {
        // The fixed Cyril Dark palette: fixed brightened RGB held to per-tier WCAG targets (ADR 0007, tests::cyril_dark_contrast_contract).
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
            emphasis: SourceColor::Rgb(0xd7, 0xba, 0x7d),
            accent_tertiary: SourceColor::Rgb(0x6c, 0xb6, 0xff),
            accent_quaternary: SourceColor::Rgb(0xcd, 0x9e, 0xe6),
            accent_quinary: SourceColor::Rgb(0x56, 0xc7, 0xd0),
            subdued: SourceColor::Rgb(0x80, 0x80, 0x80),
            subdued_positive: SourceColor::Rgb(0x00, 0x80, 0x00),
            subdued_negative: SourceColor::Rgb(0xd9, 0x8a, 0x8a),
            soft_accent: SourceColor::Rgb(0x8a, 0xb4, 0xf8),
            positive_accent: SourceColor::Rgb(0x81, 0xc7, 0x84),
            inset_background: SourceColor::Rgb(0x28, 0x2c, 0x34),
            text_secondary: SourceColor::Rgb(0xc0, 0xc0, 0xc0),
            accent_violet: SourceColor::Rgb(0xb0, 0x8d, 0xff),
        },
        // Cyril Light: dark foregrounds on a painted light canvas (tier policy: .cyril-fkke/design.md).
        ThemeId::CyrilLight => SourceTheme {
            syntax: SyntaxTheme::Base16OceanLight,
            canvas: SourceColor::Rgb(0xff, 0xff, 0xff),
            chrome: SourceColor::Rgb(0xe8, 0xe8, 0xef),
            code: SourceColor::Rgb(0xf2, 0xf2, 0xf7),
            selection: SourceColor::Rgb(0xd7, 0xd7, 0xe8),
            text: SourceColor::Rgb(0x1c, 0x1c, 0x28),
            muted: SourceColor::Rgb(0x5f, 0x5f, 0x6b),
            border: SourceColor::Rgb(0x6b, 0x6b, 0x78),
            accent: SourceColor::Rgb(0x00, 0x69, 0x7a),
            accent_alt: SourceColor::Rgb(0x7a, 0x3f, 0x7a),
            user: SourceColor::Rgb(0x1a, 0x5f, 0xb4),
            agent: SourceColor::Rgb(0x1a, 0x6b, 0x3a),
            system: SourceColor::Rgb(0x7a, 0x3f, 0x7a),
            info: SourceColor::Rgb(0x00, 0x69, 0x7a),
            success: SourceColor::Rgb(0x15, 0x80, 0x3d),
            warning: SourceColor::Rgb(0x8a, 0x5a, 0x00),
            danger: SourceColor::Rgb(0xb3, 0x26, 0x1e),
            diff_add: SourceColor::Rgb(0x15, 0x80, 0x3d),
            diff_delete: SourceColor::Rgb(0xb3, 0x26, 0x1e),
            diff_context: SourceColor::Rgb(0x5f, 0x5f, 0x6b),
            emphasis: SourceColor::Rgb(0x7a, 0x4f, 0x00),
            accent_tertiary: SourceColor::Rgb(0x0b, 0x5c, 0xad),
            accent_quaternary: SourceColor::Rgb(0x7a, 0x3f, 0x9e),
            accent_quinary: SourceColor::Rgb(0x00, 0x66, 0x6b),
            subdued: SourceColor::Rgb(0x6b, 0x6b, 0x78),
            subdued_positive: SourceColor::Rgb(0x2f, 0x6b, 0x3a),
            subdued_negative: SourceColor::Rgb(0xa0, 0x4a, 0x4a),
            soft_accent: SourceColor::Rgb(0x1a, 0x5f, 0xb4),
            positive_accent: SourceColor::Rgb(0x1a, 0x6b, 0x3a),
            inset_background: SourceColor::Rgb(0xf2, 0xf2, 0xf7),
            text_secondary: SourceColor::Rgb(0x4a, 0x4a, 0x57),
            accent_violet: SourceColor::Rgb(0x6a, 0x3f, 0xb0),
        },
        // High Contrast Dark: AAA targets (>=7.0 primary, >=4.5 muted) on a painted black canvas.
        ThemeId::HighContrastDark => SourceTheme {
            syntax: SyntaxTheme::Base16OceanDark,
            canvas: SourceColor::Rgb(0x00, 0x00, 0x00),
            chrome: SourceColor::Rgb(0x00, 0x00, 0x00),
            code: SourceColor::Rgb(0x10, 0x10, 0x10),
            selection: SourceColor::Rgb(0x00, 0x40, 0x5a),
            text: SourceColor::Rgb(0xff, 0xff, 0xff),
            muted: SourceColor::Rgb(0xc0, 0xc0, 0xc0),
            border: SourceColor::Rgb(0xc0, 0xc0, 0xc0),
            accent: SourceColor::Rgb(0x00, 0xff, 0xff),
            accent_alt: SourceColor::Rgb(0xff, 0x9e, 0xf5),
            user: SourceColor::Rgb(0x7f, 0xb3, 0xff),
            agent: SourceColor::Rgb(0x7f, 0xff, 0xa0),
            system: SourceColor::Rgb(0xff, 0xa0, 0xff),
            info: SourceColor::Rgb(0x00, 0xff, 0xff),
            success: SourceColor::Rgb(0x00, 0xff, 0x00),
            warning: SourceColor::Rgb(0xff, 0xff, 0x00),
            danger: SourceColor::Rgb(0xff, 0x80, 0x80),
            diff_add: SourceColor::Rgb(0x00, 0xff, 0x00),
            diff_delete: SourceColor::Rgb(0xff, 0x80, 0x80),
            diff_context: SourceColor::Rgb(0xc0, 0xc0, 0xc0),
            emphasis: SourceColor::Rgb(0xff, 0xe0, 0x66),
            accent_tertiary: SourceColor::Rgb(0x8a, 0xb4, 0xff),
            accent_quaternary: SourceColor::Rgb(0xe0, 0xa0, 0xff),
            accent_quinary: SourceColor::Rgb(0x66, 0xe0, 0xe0),
            subdued: SourceColor::Rgb(0xb0, 0xb0, 0xb0),
            subdued_positive: SourceColor::Rgb(0x7f, 0xff, 0xa0),
            subdued_negative: SourceColor::Rgb(0xff, 0x9e, 0x9e),
            soft_accent: SourceColor::Rgb(0x7f, 0xb3, 0xff),
            positive_accent: SourceColor::Rgb(0x7f, 0xff, 0xa0),
            inset_background: SourceColor::Rgb(0x10, 0x10, 0x10),
            text_secondary: SourceColor::Rgb(0xd0, 0xd0, 0xd0),
            accent_violet: SourceColor::Rgb(0xc9, 0xa7, 0xff),
        },
        // High Contrast Light: AAA targets on a painted white canvas.
        ThemeId::HighContrastLight => SourceTheme {
            syntax: SyntaxTheme::InspiredGitHub,
            canvas: SourceColor::Rgb(0xff, 0xff, 0xff),
            chrome: SourceColor::Rgb(0xf0, 0xf0, 0xf0),
            code: SourceColor::Rgb(0xf0, 0xf0, 0xf0),
            selection: SourceColor::Rgb(0xc8, 0xdc, 0xf0),
            text: SourceColor::Rgb(0x00, 0x00, 0x00),
            muted: SourceColor::Rgb(0x44, 0x44, 0x44),
            border: SourceColor::Rgb(0x44, 0x44, 0x44),
            accent: SourceColor::Rgb(0x00, 0x4f, 0x66),
            accent_alt: SourceColor::Rgb(0x6a, 0x1f, 0x6a),
            user: SourceColor::Rgb(0x0a, 0x3f, 0x8f),
            agent: SourceColor::Rgb(0x0a, 0x4a, 0x1f),
            system: SourceColor::Rgb(0x6a, 0x1f, 0x6a),
            info: SourceColor::Rgb(0x00, 0x4f, 0x66),
            success: SourceColor::Rgb(0x0a, 0x5a, 0x1f),
            warning: SourceColor::Rgb(0x5a, 0x3a, 0x00),
            danger: SourceColor::Rgb(0x8f, 0x1a, 0x12),
            diff_add: SourceColor::Rgb(0x0a, 0x5a, 0x1f),
            diff_delete: SourceColor::Rgb(0x8f, 0x1a, 0x12),
            diff_context: SourceColor::Rgb(0x44, 0x44, 0x44),
            emphasis: SourceColor::Rgb(0x5a, 0x3a, 0x00),
            accent_tertiary: SourceColor::Rgb(0x0a, 0x3f, 0x8f),
            accent_quaternary: SourceColor::Rgb(0x6a, 0x1f, 0x6a),
            accent_quinary: SourceColor::Rgb(0x00, 0x4f, 0x66),
            subdued: SourceColor::Rgb(0x44, 0x44, 0x44),
            subdued_positive: SourceColor::Rgb(0x0a, 0x5a, 0x1f),
            subdued_negative: SourceColor::Rgb(0x8f, 0x1a, 0x12),
            soft_accent: SourceColor::Rgb(0x0a, 0x3f, 0x8f),
            positive_accent: SourceColor::Rgb(0x0a, 0x5a, 0x1f),
            inset_background: SourceColor::Rgb(0xf0, 0xf0, 0xf0),
            text_secondary: SourceColor::Rgb(0x33, 0x33, 0x33),
            accent_violet: SourceColor::Rgb(0x5a, 0x1f, 0xa0),
        },
        // Catppuccin Mocha: upstream Mocha colors adjusted only where a tier required it; Syntect has no Catppuccin component, so base16-mocha.dark is the nearest available syntax theme.
        ThemeId::CatppuccinMocha => SourceTheme {
            syntax: SyntaxTheme::Base16MochaDark,
            canvas: SourceColor::Rgb(0x1e, 0x1e, 0x2e),
            chrome: SourceColor::Rgb(0x18, 0x18, 0x25),
            code: SourceColor::Rgb(0x18, 0x18, 0x25),
            selection: SourceColor::Rgb(0x45, 0x47, 0x5a),
            text: SourceColor::Rgb(0xcd, 0xd6, 0xf4),
            muted: SourceColor::Rgb(0x93, 0x99, 0xb2),
            border: SourceColor::Rgb(0x6c, 0x70, 0x86),
            accent: SourceColor::Rgb(0x89, 0xdc, 0xeb),
            accent_alt: SourceColor::Rgb(0xcb, 0xa6, 0xf7),
            user: SourceColor::Rgb(0x89, 0xb4, 0xfa),
            agent: SourceColor::Rgb(0xa6, 0xe3, 0xa1),
            system: SourceColor::Rgb(0xcb, 0xa6, 0xf7),
            info: SourceColor::Rgb(0x89, 0xdc, 0xeb),
            success: SourceColor::Rgb(0xa6, 0xe3, 0xa1),
            warning: SourceColor::Rgb(0xf9, 0xe2, 0xaf),
            danger: SourceColor::Rgb(0xf3, 0x8b, 0xa8),
            diff_add: SourceColor::Rgb(0xa6, 0xe3, 0xa1),
            diff_delete: SourceColor::Rgb(0xf3, 0x8b, 0xa8),
            diff_context: SourceColor::Rgb(0x93, 0x99, 0xb2),
            emphasis: SourceColor::Rgb(0xf9, 0xe2, 0xaf),
            accent_tertiary: SourceColor::Rgb(0x89, 0xb4, 0xfa),
            accent_quaternary: SourceColor::Rgb(0xcb, 0xa6, 0xf7),
            accent_quinary: SourceColor::Rgb(0x94, 0xe2, 0xd5),
            subdued: SourceColor::Rgb(0x7f, 0x84, 0x9c),
            subdued_positive: SourceColor::Rgb(0x8f, 0xbf, 0x95),
            subdued_negative: SourceColor::Rgb(0xd9, 0x90, 0x9f),
            soft_accent: SourceColor::Rgb(0x89, 0xb4, 0xfa),
            positive_accent: SourceColor::Rgb(0xa6, 0xe3, 0xa1),
            inset_background: SourceColor::Rgb(0x18, 0x18, 0x25),
            text_secondary: SourceColor::Rgb(0xba, 0xc2, 0xde),
            accent_violet: SourceColor::Rgb(0xb4, 0xbe, 0xfe),
        },
        // Gruvbox Dark (medium): upstream Gruvbox colors adjusted only where a tier required it; base16-eighties.dark is the nearest bundled syntax theme.
        ThemeId::GruvboxDark => SourceTheme {
            syntax: SyntaxTheme::Base16EightiesDark,
            canvas: SourceColor::Rgb(0x28, 0x28, 0x28),
            chrome: SourceColor::Rgb(0x1d, 0x20, 0x21),
            code: SourceColor::Rgb(0x32, 0x30, 0x2f),
            selection: SourceColor::Rgb(0x50, 0x49, 0x45),
            text: SourceColor::Rgb(0xeb, 0xdb, 0xb2),
            muted: SourceColor::Rgb(0xa8, 0x99, 0x84),
            border: SourceColor::Rgb(0x92, 0x83, 0x74),
            accent: SourceColor::Rgb(0x8e, 0xc0, 0x7c),
            accent_alt: SourceColor::Rgb(0xd3, 0x86, 0x9b),
            user: SourceColor::Rgb(0x83, 0xa5, 0x98),
            agent: SourceColor::Rgb(0xb8, 0xbb, 0x26),
            system: SourceColor::Rgb(0xd3, 0x86, 0x9b),
            info: SourceColor::Rgb(0x8e, 0xc0, 0x7c),
            success: SourceColor::Rgb(0xb8, 0xbb, 0x26),
            warning: SourceColor::Rgb(0xfa, 0xbd, 0x2f),
            danger: SourceColor::Rgb(0xfb, 0x49, 0x34),
            diff_add: SourceColor::Rgb(0xb8, 0xbb, 0x26),
            diff_delete: SourceColor::Rgb(0xfb, 0x49, 0x34),
            diff_context: SourceColor::Rgb(0xa8, 0x99, 0x84),
            emphasis: SourceColor::Rgb(0xfa, 0xbd, 0x2f),
            accent_tertiary: SourceColor::Rgb(0x83, 0xa5, 0x98),
            accent_quaternary: SourceColor::Rgb(0xd3, 0x86, 0x9b),
            accent_quinary: SourceColor::Rgb(0x8e, 0xc0, 0x7c),
            subdued: SourceColor::Rgb(0x92, 0x83, 0x74),
            subdued_positive: SourceColor::Rgb(0x98, 0x97, 0x1a),
            subdued_negative: SourceColor::Rgb(0xcc, 0x66, 0x66),
            soft_accent: SourceColor::Rgb(0x83, 0xa5, 0x98),
            positive_accent: SourceColor::Rgb(0xb8, 0xbb, 0x26),
            inset_background: SourceColor::Rgb(0x32, 0x30, 0x2f),
            text_secondary: SourceColor::Rgb(0xd5, 0xc4, 0xa1),
            accent_violet: SourceColor::Rgb(0xd3, 0x86, 0x9b),
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
    let source = source(id);
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

/// Operator-facing spellings for every bundled palette: `(id, config id, label)`.
///
/// The config file is a public contract. These strings must not change when a
/// Rust variant is renamed, which is why they are declared rather than derived
/// from the variant name; `ThemeId::name()` stays a Rust-facing identity.
const BUNDLED_APPEARANCES: [(ThemeId, &str, &str); 6] = [
    (ThemeId::CyrilDark, "cyril-dark", "Cyril Dark"),
    (ThemeId::CyrilLight, "cyril-light", "Cyril Light"),
    (
        ThemeId::HighContrastDark,
        "high-contrast-dark",
        "High Contrast Dark",
    ),
    (
        ThemeId::HighContrastLight,
        "high-contrast-light",
        "High Contrast Light",
    ),
    (
        ThemeId::CatppuccinMocha,
        "catppuccin-mocha",
        "Catppuccin Mocha",
    ),
    (ThemeId::GruvboxDark, "gruvbox-dark", "Gruvbox Dark"),
];

/// The palette an absent or unrecognized `theme` setting falls back to.
pub const DEFAULT_THEME: ThemeId = ThemeId::CyrilDark;

/// The operator-facing id of a bundled palette.
pub fn theme_config_id(theme: ThemeId) -> &'static str {
    BUNDLED_APPEARANCES
        .iter()
        .find(|(id, _, _)| *id == theme)
        .map(|(_, config_id, _)| *config_id)
        .unwrap_or_else(|| BUNDLED_APPEARANCES[0].1)
}

/// The human-readable label of a bundled palette, for the `/theme` picker.
pub fn theme_label(theme: ThemeId) -> &'static str {
    BUNDLED_APPEARANCES
        .iter()
        .find(|(id, _, _)| *id == theme)
        .map(|(_, _, label)| *label)
        .unwrap_or_else(|| BUNDLED_APPEARANCES[0].2)
}

/// Parse an operator-facing palette id. Exact match only.
pub fn parse_theme_id(value: &str) -> Option<ThemeId> {
    BUNDLED_APPEARANCES
        .iter()
        .find(|(_, config_id, _)| *config_id == value)
        .map(|(theme, _, _)| *theme)
}

/// A requested color mode: a fixed mode, or "decide from the environment".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorModeRequest {
    Automatic,
    Fixed(ColorMode),
}

/// Parse an operator-facing `color_mode` value. Exact match only.
pub fn parse_color_mode(value: &str) -> Option<ColorModeRequest> {
    match value {
        "automatic" => Some(ColorModeRequest::Automatic),
        "truecolor" => Some(ColorModeRequest::Fixed(ColorMode::TrueColor)),
        "ansi256" => Some(ColorModeRequest::Fixed(ColorMode::Ansi256)),
        "ansi16" => Some(ColorModeRequest::Fixed(ColorMode::Ansi16)),
        "none" => Some(ColorModeRequest::Fixed(ColorMode::None)),
        _ => None,
    }
}

/// The process environment values color-mode detection depends on.
///
/// Injected rather than read: `cyril-ui` never touches `std::env`, so the
/// precedence is a pure function and its tests need no environment mutation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColorEnvironment {
    pub no_color: Option<String>,
    pub color_term: Option<String>,
    pub term: Option<String>,
    pub is_windows: bool,
}

/// First match wins. `ColorModeRequest::Fixed` short-circuits every rule.
pub fn detect_color_mode(request: ColorModeRequest, environment: &ColorEnvironment) -> ColorMode {
    if let ColorModeRequest::Fixed(mode) = request {
        return mode;
    }
    if environment
        .no_color
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        return ColorMode::None;
    }
    if matches!(
        environment.color_term.as_deref(),
        Some("truecolor" | "24bit")
    ) {
        return ColorMode::TrueColor;
    }
    if environment.is_windows {
        return ColorMode::TrueColor;
    }
    if environment
        .term
        .as_deref()
        .is_some_and(|term| term.contains("256color"))
    {
        return ColorMode::Ansi256;
    }
    if environment.term.as_deref() == Some("dumb") {
        return ColorMode::None;
    }
    ColorMode::TrueColor
}

/// A recognized key whose value was not recognized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceDiagnostic {
    pub key: &'static str,
    pub value: String,
    pub default: &'static str,
}

/// The appearance a process should start with, plus one diagnostic per
/// unrecognized value. An unrecognized value falls back to its own key's
/// default and leaves every other key alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupAppearance {
    pub theme: ThemeId,
    pub mode: ColorMode,
    pub diagnostics: Vec<AppearanceDiagnostic>,
}

impl StartupAppearance {
    /// The startup appearance with no configuration at all.
    pub fn default_with(environment: &ColorEnvironment) -> Self {
        Self {
            theme: DEFAULT_THEME,
            mode: detect_color_mode(ColorModeRequest::Automatic, environment),
            diagnostics: Vec::new(),
        }
    }
}

/// Resolve `[ui] theme` and `[ui] color_mode` into a startup appearance.
pub fn resolve_startup_appearance(
    theme: Option<&str>,
    color_mode: Option<&str>,
    environment: &ColorEnvironment,
) -> StartupAppearance {
    let mut diagnostics = Vec::new();
    let theme = match theme {
        None => DEFAULT_THEME,
        Some(value) => match parse_theme_id(value) {
            Some(theme) => theme,
            None => {
                diagnostics.push(AppearanceDiagnostic {
                    key: "theme",
                    value: value.to_owned(),
                    default: theme_config_id(DEFAULT_THEME),
                });
                DEFAULT_THEME
            }
        },
    };
    let request = match color_mode {
        None => ColorModeRequest::Automatic,
        Some(value) => match parse_color_mode(value) {
            Some(request) => request,
            None => {
                diagnostics.push(AppearanceDiagnostic {
                    key: "color_mode",
                    value: value.to_owned(),
                    default: "automatic",
                });
                ColorModeRequest::Automatic
            }
        },
    };
    StartupAppearance {
        theme,
        mode: detect_color_mode(request, environment),
        diagnostics,
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
        assert_eq!(
            ThemeId::ALL,
            &[
                ThemeId::CyrilDark,
                ThemeId::CyrilLight,
                ThemeId::HighContrastDark,
                ThemeId::HighContrastLight,
                ThemeId::CatppuccinMocha,
                ThemeId::GruvboxDark,
            ]
        );
        assert_eq!(ThemeId::CyrilDark.name(), "CyrilDark");
        assert_eq!(ThemeId::HighContrastLight.name(), "HighContrastLight");
        let mut names: Vec<&str> = ThemeId::ALL.iter().map(|id| id.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ThemeId::ALL.len(), "theme ids must be unique");
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
        ("emphasis", SourceColor::Rgb(0xd7, 0xba, 0x7d)),
        ("accent_tertiary", SourceColor::Rgb(0x6c, 0xb6, 0xff)),
        ("accent_quaternary", SourceColor::Rgb(0xcd, 0x9e, 0xe6)),
        ("accent_quinary", SourceColor::Rgb(0x56, 0xc7, 0xd0)),
        ("subdued", SourceColor::Rgb(0x80, 0x80, 0x80)),
        ("subdued_positive", SourceColor::Rgb(0x00, 0x80, 0x00)),
        ("subdued_negative", SourceColor::Rgb(0xd9, 0x8a, 0x8a)),
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
        let source = source(ThemeId::CyrilDark);
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
        let available = source(ThemeId::CyrilDark).roles();
        // cyril-leiq DELIBERATELY supersedes five dim VGA legacy colors that
        // the ghuu migration had preserved (they were unreadable on dark
        // terminals): Red 0x800000, Yellow-olive 0x808000, Blue 0x000080,
        // Magenta 0x800080, Cyan 0x008080. Those roles now carry brightened,
        // hue-preserving values (see cyril_dark_contrast_contract). The
        // remaining legacy colors are still preserved.
        let required = [
            SourceColor::Rgb(0x8a, 0xb4, 0xf8),
            SourceColor::Rgb(0x81, 0xc7, 0x84),
            SourceColor::Rgb(0xb4, 0x8e, 0xad),
            SourceColor::Rgb(0x8c, 0x8c, 0x8c),
            SourceColor::Rgb(0x28, 0x2c, 0x34),
            SourceColor::Rgb(0x00, 0x80, 0x00), // Green -> subdued_positive (unchanged)
            SourceColor::Rgb(0x80, 0x80, 0x80), // DarkGray -> subdued (unchanged)
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
        let available = source(ThemeId::CyrilDark).roles();
        // cyril-leiq supersedes the dim VGA Cyan/Yellow/Red legacy colors
        // (they mapped to accent_quinary/emphasis/subdued_negative, now
        // brightened for contrast — see cyril_dark_contrast_contract).
        let required = [
            SourceColor::Rgb(0x32, 0x32, 0x46), // Rgb(50,50,70) selection bg
            SourceColor::Rgb(0xff, 0xff, 0xff), // Color::White
            SourceColor::Rgb(0x80, 0x80, 0x80), // Color::DarkGray
            SourceColor::Rgb(0x00, 0x80, 0x00), // Color::Green
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
        let available = source(ThemeId::CyrilDark).roles();
        // cyril-leiq supersedes the dim VGA Yellow/Red/Cyan/Magenta legacy
        // colors (they mapped to emphasis/subdued_negative/accent_quinary/
        // accent_quaternary, now brightened — see cyril_dark_contrast_contract).
        let required = [
            SourceColor::Rgb(0x1e, 0x1e, 0x2e), // Rgb(30,30,46) chrome bg
            SourceColor::Rgb(0xff, 0xff, 0xff), // Color::White
            SourceColor::Rgb(0x80, 0x80, 0x80), // Color::DarkGray
            SourceColor::Rgb(0xc0, 0xc0, 0xc0), // Color::Gray
            SourceColor::Rgb(0x00, 0x80, 0x00), // Color::Green
            SourceColor::Rgb(0x8a, 0xb4, 0xf8), // was palette::USER_BLUE (module removed, cyril-6r3a)
            SourceColor::Rgb(0x8c, 0x8c, 0x8c), // was palette::MUTED_GRAY (module removed, cyril-6r3a)
            SourceColor::Rgb(0xb4, 0x8e, 0xad), // was palette::SYSTEM_MAUVE (module removed, cyril-6r3a)
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
        let actual = source(ThemeId::CyrilDark).roles();
        let expected = [
            ("emphasis", SourceColor::Rgb(0xd7, 0xba, 0x7d)),
            ("accent_tertiary", SourceColor::Rgb(0x6c, 0xb6, 0xff)),
            ("accent_quaternary", SourceColor::Rgb(0xcd, 0x9e, 0xe6)),
            ("accent_quinary", SourceColor::Rgb(0x56, 0xc7, 0xd0)),
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
        let actual = source(ThemeId::CyrilDark).roles();
        let expected = [
            ("subdued_positive", SourceColor::Rgb(0x00, 0x80, 0x00)),
            ("subdued_negative", SourceColor::Rgb(0xd9, 0x8a, 0x8a)),
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
        // cyril-leiq: the five brightened roles (contract-checked below).
        assert_eq!(theme.emphasis, Color::Rgb(0xd7, 0xba, 0x7d));
        assert_eq!(theme.accent_tertiary, Color::Rgb(0x6c, 0xb6, 0xff));
        assert_eq!(theme.accent_quaternary, Color::Rgb(0xcd, 0x9e, 0xe6));
        assert_eq!(theme.accent_quinary, Color::Rgb(0x56, 0xc7, 0xd0));
        assert_eq!(theme.subdued, Color::Rgb(0x80, 0x80, 0x80));
        assert_eq!(theme.subdued_positive, Color::Rgb(0x00, 0x80, 0x00));
        assert_eq!(theme.subdued_negative, Color::Rgb(0xd9, 0x8a, 0x8a));
        assert_eq!(theme.soft_accent, Color::Rgb(0x8a, 0xb4, 0xf8));
        assert_eq!(theme.positive_accent, Color::Rgb(0x81, 0xc7, 0x84));
        assert_eq!(theme.inset_background, Color::Rgb(0x28, 0x2c, 0x34));
        assert_eq!(theme.syntax, Some(SyntaxTheme::Base16EightiesDark));
    }

    // --- cyril-leiq: Cyril Dark contrast contract ------------------------------
    // The regression fence for AC3. Computes WCAG 2.x contrast in Rust
    // (independent of the Python probe `.cyril-leiq/probe_contrast.py`) and
    // asserts every conversation foreground role meets its tier target against
    // both representative dark backgrounds. The white-on-black == 21.0 anchor
    // validates the formula so the fence cannot pass vacuously. Fixed-RGB /
    // per-tier decision: docs/adr/0007-cyril-dark-contrast-contract.md.

    fn srgb_to_linear(c8: u8) -> f64 {
        let c = f64::from(c8) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn relative_luminance((r, g, b): (u8, u8, u8)) -> f64 {
        0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
    }

    fn contrast(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> f64 {
        let (a, b) = (relative_luminance(fg), relative_luminance(bg));
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    fn rgb_of(c: Color) -> (u8, u8, u8) {
        match c {
            Color::Rgb(r, g, b) => (r, g, b),
            other => panic!("Cyril Dark truecolor role must be Rgb, got {other:?}"),
        }
    }

    // Representative dark backgrounds: pure black (a bare terminal) and the
    // theme's own chrome. Chrome is the tighter of the two for light-ish
    // foregrounds, so a role passing both is safe on any dark bg between them.
    const BG_BLACK: (u8, u8, u8) = (0x00, 0x00, 0x00);
    const BG_CHROME: (u8, u8, u8) = (0x1e, 0x1e, 0x2e);

    // Per-tier targets (design FD-1) as (chrome_min, black_min): PRIMARY = AA
    // text on both; MUTED = AA large / UI on both; SATURATED keeps the standard
    // hue, quieter on chrome (>=3.0) but readable on black (>=4.5). The two legs
    // are enforced separately so the asymmetric SATURATED target is not silently
    // collapsed to its chrome leg.
    const PRIMARY: (f64, f64) = (4.5, 4.5);
    const MUTED: (f64, f64) = (3.0, 3.0);
    const SATURATED: (f64, f64) = (3.0, 4.5);

    #[test]
    fn cyril_dark_contrast_contract() {
        // Anchor: white-on-black is exactly 21.0 by definition — guards a
        // broken luminance formula from making the whole fence vacuous.
        assert!(
            (contrast((0xff, 0xff, 0xff), BG_BLACK) - 21.0).abs() < 0.01,
            "WCAG formula anchor failed"
        );

        let t = resolve_truecolor(ThemeId::CyrilDark);
        // (role color, (chrome_min, black_min)) for every conversation
        // FOREGROUND role — every distinct role, including the two diff roles
        // that share success/danger's RGB (an independent change to either
        // would otherwise slip the fence).
        let roles: [(&str, Color, (f64, f64)); 26] = [
            ("text", t.text, PRIMARY),
            ("user", t.user, PRIMARY),
            ("agent", t.agent, PRIMARY),
            ("system", t.system, PRIMARY),
            ("accent", t.accent, PRIMARY),
            ("accent_alt", t.accent_alt, PRIMARY),
            ("accent_violet", t.accent_violet, PRIMARY),
            ("info", t.info, PRIMARY),
            ("soft_accent", t.soft_accent, PRIMARY),
            ("positive_accent", t.positive_accent, PRIMARY),
            ("emphasis", t.emphasis, PRIMARY),
            ("accent_tertiary", t.accent_tertiary, PRIMARY),
            ("accent_quaternary", t.accent_quaternary, PRIMARY),
            ("accent_quinary", t.accent_quinary, PRIMARY),
            ("muted", t.muted, MUTED),
            ("border", t.border, MUTED),
            ("diff_context", t.diff_context, MUTED),
            ("text_secondary", t.text_secondary, MUTED),
            ("subdued", t.subdued, MUTED),
            ("subdued_positive", t.subdued_positive, MUTED),
            ("subdued_negative", t.subdued_negative, MUTED),
            ("success", t.success, SATURATED),
            ("diff_add", t.diff_add, SATURATED),
            ("warning", t.warning, SATURATED),
            ("danger", t.danger, SATURATED),
            ("diff_delete", t.diff_delete, SATURATED),
        ];
        for (name, color, (chrome_min, black_min)) in roles {
            let rgb = rgb_of(color);
            let (cb, cc) = (contrast(rgb, BG_BLACK), contrast(rgb, BG_CHROME));
            assert!(
                cc >= chrome_min && cb >= black_min,
                "{name} #{:02x}{:02x}{:02x}: contrast black {cb:.2} (>= {black_min}), \
                 chrome {cc:.2} (>= {chrome_min}) — tier not met",
                rgb.0,
                rgb.1,
                rgb.2
            );
        }
    }

    // --- cyril-fkke: bundled palette contract ----------------------------------
    // Cyril Dark keeps its own signed contract above (ADR 0007). The palettes
    // added here share the same tier vocabulary but declare their own intended
    // backgrounds, and the high-contrast variants raise every tier to AAA.
    // `.cyril-fkke/palette-oracle.py` recomputes the same numbers independently.

    /// Conversation foreground roles, by tier.
    const PRIMARY_ROLES: [&str; 14] = [
        "text",
        "user",
        "agent",
        "system",
        "accent",
        "accent_alt",
        "accent_violet",
        "info",
        "soft_accent",
        "positive_accent",
        "emphasis",
        "accent_tertiary",
        "accent_quaternary",
        "accent_quinary",
    ];
    const MUTED_ROLES: [&str; 7] = [
        "muted",
        "border",
        "diff_context",
        "text_secondary",
        "subdued",
        "subdued_positive",
        "subdued_negative",
    ];
    const SATURATED_ROLES: [&str; 5] = ["success", "diff_add", "warning", "danger", "diff_delete"];

    /// (terminal default background, chrome background) each palette is designed
    /// to be read on. Both legs are enforced, so a role readable on one surface
    /// and not the other fails.
    fn palette_backgrounds(id: ThemeId) -> [(&'static str, (u8, u8, u8)); 2] {
        match id {
            ThemeId::CyrilDark => [("black", BG_BLACK), ("chrome", BG_CHROME)],
            ThemeId::CyrilLight => [
                ("white", (0xff, 0xff, 0xff)),
                ("chrome", (0xe8, 0xe8, 0xef)),
            ],
            ThemeId::HighContrastDark => [
                ("black", (0x00, 0x00, 0x00)),
                ("chrome", (0x00, 0x00, 0x00)),
            ],
            ThemeId::HighContrastLight => [
                ("white", (0xff, 0xff, 0xff)),
                ("chrome", (0xf0, 0xf0, 0xf0)),
            ],
            ThemeId::CatppuccinMocha => [
                ("black", (0x00, 0x00, 0x00)),
                ("chrome", (0x18, 0x18, 0x25)),
            ],
            ThemeId::GruvboxDark => [
                ("black", (0x00, 0x00, 0x00)),
                ("chrome", (0x1d, 0x20, 0x21)),
            ],
        }
    }

    /// (primary, muted, saturated-on-default-background, saturated-on-chrome).
    fn palette_tiers(id: ThemeId) -> (f64, f64, f64, f64) {
        match id {
            ThemeId::HighContrastDark | ThemeId::HighContrastLight => (7.0, 4.5, 7.0, 4.5),
            _ => (4.5, 3.0, 4.5, 3.0),
        }
    }

    fn role_tiers(role: &str, id: ThemeId) -> Option<(f64, f64)> {
        let (primary, muted, saturated_default, saturated_chrome) = palette_tiers(id);
        if PRIMARY_ROLES.contains(&role) {
            Some((primary, primary))
        } else if MUTED_ROLES.contains(&role) {
            Some((muted, muted))
        } else if SATURATED_ROLES.contains(&role) {
            Some((saturated_default, saturated_chrome))
        } else {
            None
        }
    }

    #[test]
    fn bundled_palette_contrast_contract() {
        // Anchor: white-on-black is exactly 21.0 by definition.
        assert!((contrast((0xff, 0xff, 0xff), (0x00, 0x00, 0x00)) - 21.0).abs() < 0.01);

        let mut checked = 0;
        for id in ThemeId::ALL.iter().copied() {
            let theme = resolve_truecolor(id);
            let [(default_name, default_bg), (chrome_name, chrome_bg)] = palette_backgrounds(id);
            for (role, color) in resolved_roles(theme) {
                let Some((default_min, chrome_min)) = role_tiers(role, id) else {
                    continue;
                };
                let rgb = rgb_of(color);
                let on_default = contrast(rgb, default_bg);
                let on_chrome = contrast(rgb, chrome_bg);
                assert!(
                    on_default >= default_min && on_chrome >= chrome_min,
                    "{} {role} #{:02x}{:02x}{:02x}: contrast {default_name} {on_default:.2} \
                     (>= {default_min}), {chrome_name} {on_chrome:.2} (>= {chrome_min}) — \
                     tier not met",
                    id.name(),
                    rgb.0,
                    rgb.1,
                    rgb.2
                );
                checked += 1;
            }
        }
        // 6 palettes × 26 foreground roles — a dropped role would otherwise
        // shrink the loop silently.
        assert_eq!(checked, 156);
    }

    #[test]
    fn muted_family_never_projects_into_protected_slots() {
        const PROTECTED: [Color; 3] = [Color::LightBlue, Color::LightGreen, Color::LightMagenta];
        let mut checked = 0;
        for id in ThemeId::ALL.iter().copied() {
            // resolve_ansi16 panics on a contract violation, so reaching the
            // assertions is itself part of the claim.
            let theme = resolve_ansi16(id);
            for (role, color) in [
                ("muted", theme.muted),
                ("border", theme.border),
                ("subdued", theme.subdued),
                ("diff_context", theme.diff_context),
            ] {
                assert!(
                    !PROTECTED.contains(&color),
                    "{} {role} projected into protected speaker slot {color:?}",
                    id.name()
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 24);

        // Positive control: the check can fail. A synthetic muted role forced
        // into a protected slot is rejected by the same finalizer.
        for role in ["muted", "border", "subdued", "diff_context"] {
            let mut candidate = resolve_with(ThemeId::CyrilDark, SourceColor::ansi16);
            set_muted_family_role(&mut candidate, role, Color::LightGreen);
            assert!(
                apply_ansi16_semantics(candidate).is_err(),
                "finalizer accepted {role} in a protected speaker slot"
            );
        }
    }

    #[test]
    fn cyril_dark_hue_identity() {
        // The five brightened roles must keep their hue family — a value that
        // hit the contrast target by swapping hue (e.g. a red "link") is wrong.
        let t = resolve_truecolor(ThemeId::CyrilDark);
        let (r, g, b) = rgb_of(t.accent_tertiary);
        assert!(
            b >= r && b >= g,
            "link (accent_tertiary) must stay blue-dominant"
        );
        let (r, g, b) = rgb_of(t.subdued_negative);
        assert!(r >= g && r >= b, "subdued_negative must stay red-dominant");
        let (r, g, b) = rgb_of(t.accent_quinary);
        assert!(
            r < g && r < b,
            "accent_quinary must stay teal (red is the min)"
        );
        let (r, g, b) = rgb_of(t.accent_quaternary);
        assert!(
            g < r && g < b,
            "accent_quaternary must stay magenta (green is the min)"
        );
        let (r, g, b) = rgb_of(t.emphasis);
        assert!(
            r >= b && g >= b,
            "emphasis must stay warm (blue is the min)"
        );
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
            let source_roles = resolved_roles(resolve_truecolor(theme_id));
            let projected = resolved_roles(resolve_ansi16(theme_id));
            let mut checked = 0;
            for ((source_name, source_color), (projected_name, projected_color)) in
                source_roles.into_iter().zip(projected)
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
            // Palettes differ in how many non-speaker roles are explicit RGB
            // (Cyril Dark's canvas is Reset; the painted-canvas palettes declare
            // one), so the expected count comes from the palette itself rather
            // than a constant that silently drifts.
            let expected_rgb = source(theme_id)
                .roles()
                .into_iter()
                .filter(|(name, color)| {
                    !matches!(*name, "user" | "agent" | "system")
                        && matches!(color, SourceColor::Rgb(_, _, _))
                })
                .count();
            assert_eq!(checked, expected_rgb, "{}", theme_id.name());
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
    fn all_bundled_syntax_themes_exist() {
        let themes = ThemeSet::load_defaults();
        for syntax in SyntaxTheme::ALL.iter().copied() {
            assert!(
                themes.themes.contains_key(syntax.name()),
                "Syntect has no bundled theme named {:?} — a palette selecting it \
                 would silently render unstyled",
                syntax.name()
            );
        }
        // Positive control: a near-miss name is absent, so the loop above is
        // not passing because `contains_key` accepts anything.
        assert!(!themes.themes.contains_key("base16-eighties.drak"));
    }

    #[test]
    fn no_color_resets_every_role() {
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
            include_str!("widgets/modal.rs"),
            include_str!("widgets/picker.rs"),
            include_str!("widgets/suggestions.rs"),
            include_str!("widgets/toolbar.rs"),
            include_str!("widgets/usage_panel.rs"),
            include_str!("widgets/voice.rs"),
        ];
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widgets");
        let on_disk = std::fs::read_dir(&manifest_dir)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_dir.display()))
            .filter(|entry| {
                let entry = entry.as_ref().unwrap_or_else(|error| {
                    panic!(
                        "failed to read entry in {}: {error}",
                        manifest_dir.display()
                    )
                });
                entry.file_name().to_string_lossy().ends_with(".rs")
            })
            .count();
        assert_eq!(
            widget_sources.len(),
            on_disk,
            "widgets/ gained or lost a file — update this fence's include_str list"
        );
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

    // ── Appearance vocabulary and detection (cyril-qaq0) ────────────────────

    const REJECTED_THEME_IDS: [&str; 5] =
        ["", "CyrilDark", "cyril_dark", "cyril-dark ", "solarized"];
    const REJECTED_COLOR_MODES: [&str; 5] = ["", "auto", "24bit", "ANSI256", "none "];

    #[test]
    fn parse_theme_id_covers_exactly_the_bundled_ids() {
        let mut seen = std::collections::HashSet::new();
        for theme in ThemeId::ALL.iter().copied() {
            let config_id = theme_config_id(theme);
            assert_eq!(parse_theme_id(config_id), Some(theme), "{config_id}");
            assert!(seen.insert(config_id), "duplicate config id {config_id}");
            let label = theme_label(theme);
            assert!(!label.is_empty(), "{config_id} needs a label");
        }
        assert_eq!(seen.len(), ThemeId::ALL.len());
        for rejected in REJECTED_THEME_IDS {
            assert_eq!(
                parse_theme_id(rejected),
                None,
                "{rejected:?} must be rejected"
            );
        }
    }

    #[test]
    fn parse_color_mode_covers_exactly_the_five_values() {
        assert_eq!(
            parse_color_mode("automatic"),
            Some(ColorModeRequest::Automatic)
        );
        for (value, mode) in [
            ("truecolor", ColorMode::TrueColor),
            ("ansi256", ColorMode::Ansi256),
            ("ansi16", ColorMode::Ansi16),
            ("none", ColorMode::None),
        ] {
            assert_eq!(parse_color_mode(value), Some(ColorModeRequest::Fixed(mode)));
        }
        for rejected in REJECTED_COLOR_MODES {
            assert_eq!(
                parse_color_mode(rejected),
                None,
                "{rejected:?} must be rejected"
            );
        }
    }

    /// One row per line of the spec's precedence table, plus the combination
    /// rows. Names match `.cyril-qaq0/oracle-precedence.py` case for case.
    fn detection_cases() -> Vec<(&'static str, Option<&'static str>, ColorEnvironment)> {
        let env = |no_color: Option<&str>,
                   color_term: Option<&str>,
                   term: Option<&str>,
                   is_windows: bool| {
            ColorEnvironment {
                no_color: no_color.map(str::to_owned),
                color_term: color_term.map(str::to_owned),
                term: term.map(str::to_owned),
                is_windows,
            }
        };
        vec![
            (
                "explicit-beats-no-color",
                Some("ansi256"),
                env(Some("1"), Some("truecolor"), Some("xterm-256color"), false),
            ),
            (
                "explicit-none",
                Some("none"),
                env(None, Some("truecolor"), Some("xterm-256color"), false),
            ),
            (
                "explicit-ansi16",
                Some("ansi16"),
                env(Some("1"), None, Some("dumb"), true),
            ),
            (
                "no-color-nonempty",
                None,
                env(Some("1"), Some("truecolor"), Some("xterm-256color"), false),
            ),
            (
                "no-color-empty-is-unset",
                None,
                env(Some(""), Some("truecolor"), Some("xterm-256color"), false),
            ),
            (
                "colorterm-truecolor",
                None,
                env(None, Some("truecolor"), Some("xterm"), false),
            ),
            (
                "colorterm-24bit",
                None,
                env(None, Some("24bit"), Some("xterm"), false),
            ),
            (
                "colorterm-other-falls-through",
                None,
                env(None, Some("yes"), Some("xterm-256color"), false),
            ),
            (
                "windows-default",
                None,
                env(None, None, Some("xterm"), true),
            ),
            (
                "windows-loses-to-no-color",
                None,
                env(Some("1"), None, Some("xterm"), true),
            ),
            (
                "term-256color",
                None,
                env(None, None, Some("xterm-256color"), false),
            ),
            ("term-dumb", None, env(None, None, Some("dumb"), false)),
            (
                "term-other-defaults-truecolor",
                None,
                env(None, None, Some("xterm"), false),
            ),
            ("all-unset", None, env(None, None, None, false)),
        ]
    }

    fn request(value: &str) -> ColorModeRequest {
        match parse_color_mode(value) {
            Some(request) => request,
            None => panic!("detection case uses an unknown color_mode value {value:?}"),
        }
    }

    fn mode_config_id(mode: ColorMode) -> &'static str {
        match mode {
            ColorMode::TrueColor => "truecolor",
            ColorMode::Ansi256 => "ansi256",
            ColorMode::Ansi16 => "ansi16",
            ColorMode::None => "none",
        }
    }

    #[test]
    fn detection_precedence_matches_every_table_row() {
        for (name, explicit, environment) in detection_cases() {
            let request = explicit.map(request).unwrap_or(ColorModeRequest::Automatic);
            let mode = detect_color_mode(request, &environment);
            let expected = match name {
                "explicit-beats-no-color" => ColorMode::Ansi256,
                "explicit-none" => ColorMode::None,
                "explicit-ansi16" => ColorMode::Ansi16,
                "no-color-nonempty" | "windows-loses-to-no-color" | "term-dumb" => ColorMode::None,
                "colorterm-truecolor"
                | "colorterm-24bit"
                | "no-color-empty-is-unset"
                | "windows-default"
                | "term-other-defaults-truecolor"
                | "all-unset" => ColorMode::TrueColor,
                "colorterm-other-falls-through" | "term-256color" => ColorMode::Ansi256,
                other => panic!("unclassified case {other}"),
            };
            assert_eq!(
                mode_config_id(mode),
                mode_config_id(expected),
                "{name} resolved to the wrong mode"
            );
        }
    }

    #[test]
    fn startup_appearance_reports_one_diagnostic_per_unknown_key() {
        let environment = ColorEnvironment::default();
        let clean = resolve_startup_appearance(Some("gruvbox-dark"), Some("ansi16"), &environment);
        assert_eq!(clean.theme, ThemeId::GruvboxDark);
        assert_eq!(clean.mode, ColorMode::Ansi16);
        assert!(clean.diagnostics.is_empty());

        let unknown = resolve_startup_appearance(Some("cyrl-light"), Some("bogus"), &environment);
        assert_eq!(unknown.theme, DEFAULT_THEME);
        assert_eq!(unknown.mode, ColorMode::TrueColor);
        assert_eq!(unknown.diagnostics.len(), 2);
        assert_eq!(unknown.diagnostics[0].key, "theme");
        assert_eq!(unknown.diagnostics[0].value, "cyrl-light");
        assert_eq!(unknown.diagnostics[0].default, "cyril-dark");
        assert_eq!(unknown.diagnostics[1].key, "color_mode");
        assert_eq!(unknown.diagnostics[1].default, "automatic");

        // One unknown key must not disturb the other key's explicit value.
        let partial = resolve_startup_appearance(Some("bogus"), Some("ansi256"), &environment);
        assert_eq!(partial.mode, ColorMode::Ansi256);
        assert_eq!(partial.diagnostics.len(), 1);
    }

    /// Independent-oracle probe (`.cyril-qaq0/oracles/compare_appearance.py`).
    #[test]
    fn emit_appearance_probe() {
        println!("BEGIN_APPEARANCE_PROBE");
        for theme in ThemeId::ALL.iter().copied() {
            println!("theme\t{}\t{}", theme_config_id(theme), theme_label(theme));
        }
        for value in REJECTED_THEME_IDS {
            println!("theme_rejected\t{value}");
        }
        for value in REJECTED_COLOR_MODES {
            println!("mode_rejected\t{value}");
        }
        for (name, explicit, environment) in detection_cases() {
            let request = explicit.map(request).unwrap_or(ColorModeRequest::Automatic);
            println!(
                "case\t{name}\t{}",
                mode_config_id(detect_color_mode(request, &environment))
            );
        }
        println!("END_APPEARANCE_PROBE");
    }
}

use cyril_ui::theme::{resolve, ColorMode, Theme, ThemeId};
use ratatui::style::Color;

fn candidate(mode: ColorMode) -> Theme {
    let mut theme = resolve(ThemeId::CyrilDark, mode);
    if mode == ColorMode::Ansi16 {
        theme.user = Color::LightBlue;
        theme.agent = Color::LightGreen;
        theme.system = Color::LightMagenta;
    }
    theme
}

fn roles(theme: &Theme) -> [(&'static str, Color); 29] {
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
    ]
}

fn main() {
    println!("mode\trole\tbefore\tafter");
    let modes = [
        ("truecolor", ColorMode::TrueColor),
        ("ansi256", ColorMode::Ansi256),
        ("ansi16", ColorMode::Ansi16),
        ("none", ColorMode::None),
    ];
    for (mode_name, mode) in modes {
        let before = resolve(ThemeId::CyrilDark, mode);
        let after = candidate(mode);
        for ((name, old), (after_name, new)) in roles(&before).into_iter().zip(roles(&after)) {
            assert_eq!(name, after_name);
            println!("{mode_name}\t{name}\t{old:?}\t{new:?}");
        }
    }
}

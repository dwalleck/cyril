use std::fmt::Write;

const ROLES: [(&str, (u8, u8, u8)); 18] = [
    ("chrome", (30, 30, 46)),
    ("code", (40, 44, 52)),
    ("selection", (50, 50, 70)),
    ("text", (255, 255, 255)),
    ("muted", (140, 140, 140)),
    ("border", (140, 140, 140)),
    ("accent", (0, 255, 255)),
    ("accent_alt", (180, 142, 173)),
    ("user", (138, 180, 248)),
    ("agent", (129, 199, 132)),
    ("system", (180, 142, 173)),
    ("info", (0, 255, 255)),
    ("success", (0, 255, 0)),
    ("warning", (255, 255, 0)),
    ("danger", (255, 0, 0)),
    ("diff_add", (0, 255, 0)),
    ("diff_delete", (255, 0, 0)),
    ("diff_context", (140, 140, 140)),
];

const ANSI16: [(u8, u8, u8); 16] = [
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

fn xterm(index: u8) -> (u8, u8, u8) {
    if index < 232 {
        let n = index - 16;
        let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
        (level(n / 36), level((n / 6) % 6), level(n % 6))
    } else {
        let gray = 8 + 10 * (index - 232);
        (gray, gray, gray)
    }
}

fn distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).unsigned_abs().pow(2);
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

fn nearest<I>(rgb: (u8, u8, u8), candidates: I) -> u8
where
    I: IntoIterator<Item = (u8, (u8, u8, u8))>,
{
    candidates
        .into_iter()
        .min_by_key(|(i, c)| (distance(rgb, *c), *i))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn main() {
    let mut output = String::from("role\trgb\tansi256\tansi16\n");
    for (role, rgb) in ROLES {
        let a256 = nearest(rgb, (16u8..=255).map(|i| (i, xterm(i))));
        let a16 = nearest(
            rgb,
            ANSI16.into_iter().enumerate().map(|(i, c)| (i as u8, c)),
        );
        writeln!(
            output,
            "{role}\t{:02x}{:02x}{:02x}\t{a256}\t{a16}",
            rgb.0, rgb.1, rgb.2
        )
        .unwrap_or_else(|_| unreachable!());
    }
    print!("{output}");
}

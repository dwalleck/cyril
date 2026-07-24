#!/usr/bin/env python3
"""cyril-leiq PROBE: measure WCAG contrast of each Cyril Dark conversation
FOREGROUND role against representative dark backgrounds, and flag failures.

Independent of the renderer: parses the RGB literals straight out of
`cyril_dark_source` in theme.rs (regex), so it cannot inherit a bug from the
theme resolution code. The oracle (oracle_contrast.rs) reads the COMPILED
Theme a different way and must agree on the same ratios; the white-on-black
== 21.00 anchor validates the WCAG formula itself.

Smallest question: which conversation foreground roles fall below the AA
text target (4.5:1) on a dark terminal, and by how much?
"""
import re
import sys
from pathlib import Path

THEME = Path(__file__).resolve().parent.parent / "crates/cyril-ui/src/theme.rs"

# Representative dark backgrounds. canvas is Reset (the terminal's own bg), so
# the worst realistic case is near-black; chrome (0x1e1e2e) is where some
# surfaces sit.
BACKGROUNDS = {"black": (0x00, 0x00, 0x00), "chrome": (0x1E, 0x1E, 0x2E)}

# Roles that carry FOREGROUND text in conversation (chat.rs / markdown.rs).
# Pure backgrounds (canvas, chrome, code, selection, inset_background) are
# excluded — they are never a foreground.
FG_ROLES = {
    "text", "muted", "border", "accent", "accent_alt", "user", "agent",
    "system", "info", "success", "warning", "danger", "diff_add",
    "diff_delete", "diff_context", "emphasis", "accent_tertiary",
    "accent_quaternary", "accent_quinary", "subdued", "subdued_positive",
    "subdued_negative", "soft_accent", "positive_accent", "text_secondary",
    "accent_violet",
}
AA_TEXT = 4.5   # WCAG AA, normal text
AA_LARGE = 3.0  # WCAG AA, large text / UI components


def linearize(c8: int) -> float:
    c = c8 / 255.0
    return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4


def luminance(rgb) -> float:
    r, g, b = (linearize(x) for x in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(fg, bg) -> float:
    a, b = luminance(fg), luminance(bg)
    hi, lo = max(a, b), min(a, b)
    return (hi + 0.05) / (lo + 0.05)


def parse_source_roles() -> dict:
    """Extract `name: SourceColor::Rgb(0xNN, 0xNN, 0xNN)` from cyril_dark_source."""
    text = THEME.read_text()
    start = text.index("fn cyril_dark_source")
    end = text.index("\n}", start)
    body = text[start:end]
    out = {}
    for m in re.finditer(
        r"(\w+):\s*SourceColor::Rgb\(\s*0x([0-9a-fA-F]{2}),\s*0x([0-9a-fA-F]{2}),\s*0x([0-9a-fA-F]{2})\s*\)",
        body,
    ):
        name, r, g, b = m.group(1), *(int(m.group(i), 16) for i in (2, 3, 4))
        out[name] = (r, g, b)
    return out


def main() -> int:
    roles = parse_source_roles()

    # Formula anchor: white on black is exactly 21.00 by WCAG definition.
    anchor = contrast((255, 255, 255), (0, 0, 0))
    if abs(anchor - 21.0) > 0.01:
        print(f"ANCHOR FAILED: white/black = {anchor:.4f}, expected 21.00", file=sys.stderr)
        return 2
    print(f"anchor: white-on-black = {anchor:.2f} (WCAG max) — formula OK\n")

    fg = {k: v for k, v in roles.items() if k in FG_ROLES}
    failures = []
    print(f"{'role':<18} {'rgb':<10} {'vs black':>9} {'vs chrome':>10}  verdict")
    for name in sorted(fg):
        rgb = fg[name]
        cb = contrast(rgb, BACKGROUNDS["black"])
        cc = contrast(rgb, BACKGROUNDS["chrome"])
        worst = min(cb, cc)
        verdict = "OK" if worst >= AA_TEXT else ("large-only" if worst >= AA_LARGE else "FAIL<3")
        if worst < AA_TEXT:
            failures.append((name, rgb, round(cb, 2), round(cc, 2), verdict))
        print(f"{name:<18} #{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}   {cb:>8.2f} {cc:>9.2f}  {verdict}")

    print(f"\n{len(failures)} of {len(fg)} foreground roles below AA text (4.5:1) on the worst dark bg:")
    for name, rgb, cb, cc, v in failures:
        print(f"  {name:<18} #{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}  black={cb} chrome={cc}  [{v}]")

    # The claim the design will rest on: the named dim-VGA roles fail. Link
    # (accent_tertiary) is the headline; assert it is present and failing.
    link = fg.get("accent_tertiary")
    link_c = contrast(link, BACKGROUNDS["black"]) if link else None
    print(f"\nlink role (accent_tertiary {('#%02x%02x%02x' % link) if link else '?'}): "
          f"contrast vs black = {link_c:.2f} — {'FAIL' if link_c and link_c < AA_TEXT else 'ok'}")
    return 0 if failures else 1


if __name__ == "__main__":
    sys.exit(main())

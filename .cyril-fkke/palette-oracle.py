#!/usr/bin/env python3
"""Independent oracle for bundled palette contrast and limited-color projection.

Computes WCAG 2.x relative luminance/contrast from first principles (a
different implementation path than the Rust fence in theme.rs), plus
brute-force nearest-index search for the ANSI-256 and ANSI-16 projections.
Fails loudly when any palette role misses its tier or a protected ANSI-16
speaker slot is violated.

Usage:
  python3 .cyril-fkke/palette-oracle.py            # check candidate tables
  python3 .cyril-fkke/palette-oracle.py --emit     # emit TSV for comparison
"""

from __future__ import annotations

import sys

# --- palette tables (candidate values under test) ---------------------------

CYRIL_DARK = {
    "canvas": None,
    "chrome": "1e1e2e",
    "code": "282c34",
    "selection": "323246",
    "text": "ffffff",
    "muted": "8c8c8c",
    "border": "8c8c8c",
    "accent": "00ffff",
    "accent_alt": "b48ead",
    "user": "8ab4f8",
    "agent": "81c784",
    "system": "b48ead",
    "info": "00ffff",
    "success": "00ff00",
    "warning": "ffff00",
    "danger": "ff0000",
    "diff_add": "00ff00",
    "diff_delete": "ff0000",
    "diff_context": "8c8c8c",
    "emphasis": "d7ba7d",
    "accent_tertiary": "6cb6ff",
    "accent_quaternary": "cd9ee6",
    "accent_quinary": "56c7d0",
    "subdued": "808080",
    "subdued_positive": "008000",
    "subdued_negative": "d98a8a",
    "soft_accent": "8ab4f8",
    "positive_accent": "81c784",
    "inset_background": "282c34",
    "text_secondary": "c0c0c0",
    "accent_violet": "b08dff",
}

CYRIL_LIGHT = {
    "canvas": "ffffff",
    "chrome": "e8e8ef",
    "code": "f2f2f7",
    "selection": "d7d7e8",
    "text": "1c1c28",
    "muted": "5f5f6b",
    "border": "6b6b78",
    "accent": "00697a",
    "accent_alt": "7a3f7a",
    "user": "1a5fb4",
    "agent": "1a6b3a",
    "system": "7a3f7a",
    "info": "00697a",
    "success": "15803d",
    "warning": "8a5a00",
    "danger": "b3261e",
    "diff_add": "15803d",
    "diff_delete": "b3261e",
    "diff_context": "5f5f6b",
    "emphasis": "7a4f00",
    "accent_tertiary": "0b5cad",
    "accent_quaternary": "7a3f9e",
    "accent_quinary": "00666b",
    "subdued": "6b6b78",
    "subdued_positive": "2f6b3a",
    "subdued_negative": "a04a4a",
    "soft_accent": "1a5fb4",
    "positive_accent": "1a6b3a",
    "inset_background": "f2f2f7",
    "text_secondary": "4a4a57",
    "accent_violet": "6a3fb0",
}

HIGH_CONTRAST_DARK = {
    "canvas": "000000",
    "chrome": "000000",
    "code": "101010",
    "selection": "00405a",
    "text": "ffffff",
    "muted": "c0c0c0",
    "border": "c0c0c0",
    "accent": "00ffff",
    "accent_alt": "ff9ef5",
    "user": "7fb3ff",
    "agent": "7fffa0",
    "system": "ffa0ff",
    "info": "00ffff",
    "success": "00ff00",
    "warning": "ffff00",
    "danger": "ff8080",
    "diff_add": "00ff00",
    "diff_delete": "ff8080",
    "diff_context": "c0c0c0",
    "emphasis": "ffe066",
    "accent_tertiary": "8ab4ff",
    "accent_quaternary": "e0a0ff",
    "accent_quinary": "66e0e0",
    "subdued": "b0b0b0",
    "subdued_positive": "7fffa0",
    "subdued_negative": "ff9e9e",
    "soft_accent": "7fb3ff",
    "positive_accent": "7fffa0",
    "inset_background": "101010",
    "text_secondary": "d0d0d0",
    "accent_violet": "c9a7ff",
}

HIGH_CONTRAST_LIGHT = {
    "canvas": "ffffff",
    "chrome": "f0f0f0",
    "code": "f0f0f0",
    "selection": "c8dcf0",
    "text": "000000",
    "muted": "444444",
    "border": "444444",
    "accent": "004f66",
    "accent_alt": "6a1f6a",
    "user": "0a3f8f",
    "agent": "0a4a1f",
    "system": "6a1f6a",
    "info": "004f66",
    "success": "0a5a1f",
    "warning": "5a3a00",
    "danger": "8f1a12",
    "diff_add": "0a5a1f",
    "diff_delete": "8f1a12",
    "diff_context": "444444",
    "emphasis": "5a3a00",
    "accent_tertiary": "0a3f8f",
    "accent_quaternary": "6a1f6a",
    "accent_quinary": "004f66",
    "subdued": "444444",
    "subdued_positive": "0a5a1f",
    "subdued_negative": "8f1a12",
    "soft_accent": "0a3f8f",
    "positive_accent": "0a5a1f",
    "inset_background": "f0f0f0",
    "text_secondary": "333333",
    "accent_violet": "5a1fa0",
}

CATPPUCCIN_MOCHA = {
    "canvas": "1e1e2e",
    "chrome": "181825",
    "code": "181825",
    "selection": "45475a",
    "text": "cdd6f4",
    "muted": "9399b2",
    "border": "6c7086",
    "accent": "89dceb",
    "accent_alt": "cba6f7",
    "user": "89b4fa",
    "agent": "a6e3a1",
    "system": "cba6f7",
    "info": "89dceb",
    "success": "a6e3a1",
    "warning": "f9e2af",
    "danger": "f38ba8",
    "diff_add": "a6e3a1",
    "diff_delete": "f38ba8",
    "diff_context": "9399b2",
    "emphasis": "f9e2af",
    "accent_tertiary": "89b4fa",
    "accent_quaternary": "cba6f7",
    "accent_quinary": "94e2d5",
    "subdued": "7f849c",
    "subdued_positive": "8fbf95",
    "subdued_negative": "d9909f",
    "soft_accent": "89b4fa",
    "positive_accent": "a6e3a1",
    "inset_background": "181825",
    "text_secondary": "bac2de",
    "accent_violet": "b4befe",
}

GRUVBOX_DARK = {
    "canvas": "282828",
    "chrome": "1d2021",
    "code": "32302f",
    "selection": "504945",
    "text": "ebdbb2",
    "muted": "a89984",
    "border": "928374",
    "accent": "8ec07c",
    "accent_alt": "d3869b",
    "user": "83a598",
    "agent": "b8bb26",
    "system": "d3869b",
    "info": "8ec07c",
    "success": "b8bb26",
    "warning": "fabd2f",
    "danger": "fb4934",
    "diff_add": "b8bb26",
    "diff_delete": "fb4934",
    "diff_context": "a89984",
    "emphasis": "fabd2f",
    "accent_tertiary": "83a598",
    "accent_quaternary": "d3869b",
    "accent_quinary": "8ec07c",
    "subdued": "928374",
    "subdued_positive": "98971a",
    "subdued_negative": "cc6666",
    "soft_accent": "83a598",
    "positive_accent": "b8bb26",
    "inset_background": "32302f",
    "text_secondary": "d5c4a1",
    "accent_violet": "d3869b",
}

PALETTES = {
    "CyrilDark": (CYRIL_DARK, [("black", "000000"), ("chrome", "1e1e2e")], "standard"),
    "CyrilLight": (CYRIL_LIGHT, [("white", "ffffff"), ("chrome", "e8e8ef")], "standard"),
    "HighContrastDark": (
        HIGH_CONTRAST_DARK,
        [("black", "000000"), ("chrome", "000000")],
        "high",
    ),
    "HighContrastLight": (
        HIGH_CONTRAST_LIGHT,
        [("white", "ffffff"), ("chrome", "f0f0f0")],
        "high",
    ),
    "CatppuccinMocha": (
        CATPPUCCIN_MOCHA,
        [("black", "000000"), ("chrome", "181825")],
        "standard",
    ),
    "GruvboxDark": (GRUVBOX_DARK, [("black", "000000"), ("chrome", "1d2021")], "standard"),
}

BACKGROUND_ROLES = {"canvas", "chrome", "code", "selection", "inset_background"}

PRIMARY = [
    "text", "user", "agent", "system", "accent", "accent_alt", "accent_violet",
    "info", "soft_accent", "positive_accent", "emphasis", "accent_tertiary",
    "accent_quaternary", "accent_quinary",
]
MUTED = [
    "muted", "border", "diff_context", "text_secondary", "subdued",
    "subdued_positive", "subdued_negative",
]
SATURATED = ["success", "diff_add", "warning", "danger", "diff_delete"]

TIERS = {
    "standard": {"primary": 4.5, "muted": 3.0, "sat_default": 4.5, "sat_chrome": 3.0},
    "high": {"primary": 7.0, "muted": 4.5, "sat_default": 7.0, "sat_chrome": 4.5},
}

PROTECTED_ANSI16 = {12, 10, 13}  # LightBlue, LightGreen, LightMagenta indices
ANSI16_RGB = [
    (0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0), (0, 0, 128), (128, 0, 128),
    (0, 128, 128), (192, 192, 192), (128, 128, 128), (255, 0, 0), (0, 255, 0),
    (255, 255, 0), (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
]


def xterm_rgb(index: int) -> tuple[int, int, int]:
    if index < 232:
        offset = index - 16
        level = lambda value: 0 if value == 0 else 55 + 40 * value
        return (level(offset // 36), level((offset // 6) % 6), level(offset % 6))
    gray = 8 + 10 * (index - 232)
    return (gray, gray, gray)


def channel_luminance(c8: int) -> float:
    c = c8 / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def luminance(rgb: tuple[int, int, int]) -> float:
    r, g, b = rgb
    return (
        0.2126 * channel_luminance(r)
        + 0.7152 * channel_luminance(g)
        + 0.0722 * channel_luminance(b)
    )


def contrast(fg: tuple[int, int, int], bg: tuple[int, int, int]) -> float:
    a, b = luminance(fg), luminance(bg)
    hi, lo = (a, b) if a >= b else (b, a)
    return (hi + 0.05) / (lo + 0.05)


def rgb(hex_value: str) -> tuple[int, int, int]:
    return (int(hex_value[0:2], 16), int(hex_value[2:4], 16), int(hex_value[4:6], 16))


def nearest(rgb_value: tuple[int, int, int], candidates) -> int:
    best_index, best_distance = None, None
    for index, candidate in candidates:
        distance = sum((a - b) ** 2 for a, b in zip(rgb_value, candidate))
        if best_distance is None or (distance, index) < (best_distance, best_index):
            best_index, best_distance = index, distance
    return best_index


def nearest_ansi16(rgb_value):
    return nearest(rgb_value, enumerate(ANSI16_RGB))


def nearest_ansi256(rgb_value):
    return nearest(rgb_value, ((i, xterm_rgb(i)) for i in range(16, 256)))


def check() -> int:
    failures = []
    # Formula anchor: white on black is exactly 21.0 by definition.
    anchor = contrast((255, 255, 255), (0, 0, 0))
    if abs(anchor - 21.0) > 0.01:
        failures.append(f"WCAG anchor broken: {anchor}")

    for name, (palette, backgrounds, tier) in PALETTES.items():
        targets = TIERS[tier]
        for role in PRIMARY + MUTED + SATURATED:
            value = palette[role]
            if value is None:
                continue
            rgb_value = rgb(value)
            for bg_name, bg_hex in backgrounds:
                if bg_name == "canvas":
                    continue
                ratio = contrast(rgb_value, rgb(bg_hex))
                if role in PRIMARY:
                    minimum = targets["primary"]
                elif role in MUTED:
                    minimum = targets["muted"]
                else:
                    minimum = (
                        targets["sat_chrome"]
                        if bg_name == "chrome"
                        else targets["sat_default"]
                    )
                if ratio < minimum:
                    failures.append(
                        f"{name}/{role} #{value} on {bg_name}: {ratio:.2f} < {minimum}"
                    )
        # ANSI-16 protected speaker slots must stay free for the speaker roles.
        for role in ("muted", "border", "subdued", "diff_context"):
            value = palette[role]
            if value is None:
                continue
            index = nearest_ansi16(rgb(value))
            if index in PROTECTED_ANSI16:
                failures.append(f"{name}/{role} #{value} projects to protected slot {index}")

    for failure in failures:
        print(f"FAIL\t{failure}")
    if not failures:
        print(f"PASS\t{len(PALETTES)} palettes satisfy contrast and ANSI-16 constraints")
    return 1 if failures else 0


# cyril-q9dx accepted contract: in ANSI-16 the three speaker roles are pinned
# to distinct slots regardless of their source RGB, so identity survives the
# projection. The oracle encodes that contract rather than re-deriving it.
SPEAKER_SLOTS = {"user": 12, "agent": 10, "system": 13}


def emit() -> int:
    print("palette\trole\tsource\tansi256\tansi16")
    for name, (palette, _backgrounds, _tier) in PALETTES.items():
        for role, value in palette.items():
            if value is None:
                print(f"{name}\t{role}\treset\treset\treset")
                continue
            rgb_value = rgb(value)
            ansi16 = SPEAKER_SLOTS.get(role, nearest_ansi16(rgb_value))
            print(f"{name}\t{role}\t{value}\t{nearest_ansi256(rgb_value)}\t{ansi16}")
    return 0


if __name__ == "__main__":
    sys.exit(emit() if "--emit" in sys.argv[1:] else check())

#!/usr/bin/env python3
"""cyril-leiq cheapest falsifier: do the PROPOSED brightened role values hit
their tier target against BOTH representative dark backgrounds?

Reuses the WCAG contrast from probe_contrast.py (anchor-validated). Claim under
test: each fixed role meets its tier target vs #1e1e2e (chrome, the tighter of
the two representative dark bgs) AND vs #000000. Falsified if any proposed value
misses its target.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_contrast import contrast, BACKGROUNDS  # independent shared formula

# Tier targets as (chrome_min, black_min):
PRIMARY = (4.5, 4.5)   # readable emphasis / links: WCAG AA text
MUTED = (3.0, 3.0)     # intentionally de-emphasized: AA large / UI floor
SATURATED = (3.0, 4.5)  # standard signal hues: 3.0 chrome / 4.5 black

def h(hexstr):
    return (int(hexstr[0:2], 16), int(hexstr[2:4], 16), int(hexstr[4:6], 16))

# role -> (proposed_hex, (chrome_min, black_min)). Only roles that fail their
# tier are changed; passing roles use their CURRENT value as a regression guard.
PROPOSED = {
    # --- changed (tier-1/tier-2 failures) ---
    "accent_tertiary":   ("6cb6ff", PRIMARY),   # link blue, was 000080 (1.02)
    "accent_quaternary": ("cd9ee6", PRIMARY),   # magenta,   was 800080 (1.74)
    "accent_quinary":    ("56c7d0", PRIMARY),   # teal,      was 008080 (3.44)
    "subdued_negative":  ("d98a8a", MUTED),     # muted red, was 800000 (1.50)
    "emphasis":          ("d7ba7d", PRIMARY),   # gold,      was 808000 (3.91)
    # --- unchanged, must still clear their tier (regression guard) ---
    "subdued":           ("808080", MUTED),     # 4.15 chrome — passes muted
    "subdued_positive":  ("008000", MUTED),     # 3.19 chrome — passes muted
    "danger":            ("ff0000", SATURATED), # 4.10 chrome — standard red
    "text":              ("ffffff", PRIMARY),   # anchor, 16.40 chrome
    "user":              ("8ab4f8", PRIMARY),   # 7.78 chrome
}

def main() -> int:
    black, chrome = BACKGROUNDS["black"], BACKGROUNDS["chrome"]
    fails = 0
    print(
        f"{'role':<18} {'hex':<8} {'min C/B':<9} {'vs black':>8} "
        f"{'vs chrome':>9}  verdict"
    )
    for role, (hx, (chrome_min, black_min)) in PROPOSED.items():
        rgb = h(hx)
        cb, cc = contrast(rgb, black), contrast(rgb, chrome)
        ok = cc >= chrome_min and cb >= black_min
        if not ok:
            fails += 1
        print(
            f"{role:<18} #{hx:<7} {chrome_min:.1f}/{black_min:.1f} "
            f"{cb:>8.2f} {cc:>9.2f}  {'PASS' if ok else 'FAIL'}"
        )
    print(f"\n{'ALL PROPOSED VALUES HIT THEIR TIER TARGET' if fails==0 else str(fails)+' FAIL — revise'}")
    # Every changed value is checked directly from PROPOSED. Strict channel
    # dominance rejects grayscale ties while preserving each intended hue.
    link = h(PROPOSED["accent_tertiary"][0])
    negative = h(PROPOSED["subdued_negative"][0])
    teal = h(PROPOSED["accent_quinary"][0])
    magenta = h(PROPOSED["accent_quaternary"][0])
    gold = h(PROPOSED["emphasis"][0])
    hue_checks = {
        "link blue-dominant": link[2] > link[0] and link[2] > link[1],
        "negative red-dominant": negative[0] > negative[1] and negative[0] > negative[2],
        "teal red-min": teal[0] < teal[1] and teal[0] < teal[2],
        "magenta green-min": magenta[1] < magenta[0] and magenta[1] < magenta[2],
        "gold blue-min": gold[2] < gold[0] and gold[2] < gold[1],
    }
    hue_ok = all(hue_checks.values())
    failed_hues = [name for name, passed in hue_checks.items() if not passed]
    print(f"hue identity preserved for all changed roles: {hue_ok}")
    if failed_hues:
        print(f"failed hue checks: {', '.join(failed_hues)}")
    return 0 if fails == 0 and hue_ok else 1

if __name__ == "__main__":
    sys.exit(main())

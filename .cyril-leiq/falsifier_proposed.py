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
from probe_contrast import contrast, BACKGROUNDS  # independent, anchor-checked

# Tier targets (contrast ratio, minimum):
PRIMARY = 4.5   # readable emphasis / links: WCAG AA text
MUTED = 3.0     # intentionally de-emphasized: WCAG AA large / UI floor
SATURATED = 3.0  # standard signal hues used bold/short; keep the hue

def h(hexstr):
    return (int(hexstr[0:2], 16), int(hexstr[2:4], 16), int(hexstr[4:6], 16))

# role -> (proposed_hex, tier). Only the roles that FAIL their tier today are
# changed; passing roles are listed with their CURRENT value + expected PASS to
# prove the design leaves them alone (and they still clear their tier).
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
    print(f"{'role':<18} {'hex':<8} {'tier':<5} {'vs black':>8} {'vs chrome':>9}  verdict")
    for role, (hx, tier) in PROPOSED.items():
        rgb = h(hx)
        cb, cc = contrast(rgb, black), contrast(rgb, chrome)
        binding = min(cb, cc)  # chrome is the tighter bg
        ok = binding >= tier
        if not ok:
            fails += 1
        print(f"{role:<18} #{hx:<7} {tier:<5.1f} {cb:>8.2f} {cc:>9.2f}  {'PASS' if ok else 'FAIL'}")
    print(f"\n{'ALL PROPOSED VALUES HIT THEIR TIER TARGET' if fails==0 else str(fails)+' FAIL — revise'}")
    # Distinct hue check: link stays blue-dominant, red stays red-dominant, etc.
    hue_ok = (h('6cb6ff')[2] == max(h('6cb6ff')) and       # link: blue max
              h('d98a8a')[0] == max(h('d98a8a')) and        # neg: red max
              h('56c7d0')[0] == min(h('56c7d0')))           # teal: red min
    print(f"hue identity preserved (link blue-max, neg red-max, teal red-min): {hue_ok}")
    return 0 if fails == 0 and hue_ok else 1

if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""cyril-cc5e oracle v2: the NEW windowed picker model, computed independently
of the Rust implementation (pure arithmetic; supersedes oracle.py which
described the pre-fix behavior).

Usage:
  oracle-v2.py           -> SCENARIO lines for the probe + fence fixtures
  oracle-v2.py --walk    -> per-step windows for the 60x16 15-option walk (C5)
"""
import sys

def window(w, h, n, k, all_desc=True):
    desired_rows = min(n, 15)
    desc = 1 if all_desc and n > 0 else 0
    height = min(desired_rows + desc + 4, max(h - 4, 0))
    inner = max(height - 2, 0)
    r_opts = max(inner - 2 - desc, 0)
    rows = min(n, r_opts)
    if rows == 0:
        return (0, 0, height)
    start = max(0, min(k - rows // 2, n - rows))
    return (start, rows, height)

def scenario(name, w, h, n, k):
    start, rows, height = window(w, h, n, k)
    drawn = ",".join(f"opt-{i:02}" for i in range(start, start + rows))
    marker = start <= k < start + rows and rows > 0
    print(f"SCENARIO {name} w={w} h={h} n={n} sel={k} marker={str(marker).lower()} drawn={drawn}")

if "--walk" in sys.argv:
    for k in range(15):
        start, rows, height = window(60, 16, 15, k)
        print(f"WALK k={k} window=[{start},{start + rows}) height={height}")
    sys.exit(0)

# Probe scenarios (same inputs as probe_cc5e.rs, new expected outputs)
scenario("A-control-80x24", 80, 24, 30, 5)
scenario("B-deep-sel-80x24", 80, 24, 30, 20)
scenario("C-floor-60x16", 60, 16, 15, 14)
scenario("D-floor-top-60x16", 60, 16, 15, 0)
# Fence fixtures (window_contiguous_fill literals)
scenario("F1-tail-clamp", 80, 24, 20, 19)
scenario("F2-mid", 80, 24, 20, 10)
scenario("F3-one-past-cap", 80, 24, 16, 15)
scenario("F4-single", 80, 24, 1, 0)
scenario("F5-small-list", 80, 24, 5, 2)

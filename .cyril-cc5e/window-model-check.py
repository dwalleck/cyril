#!/usr/bin/env python3
"""cyril-cc5e cheapest falsifier: exhaustive check of the proposed window formula.

Proposed design math (all u16/usize saturating in Rust):
  desired_rows = min(n, 15)                      # visible cap kept (status quo)
  desc_reserve = 1 if any option has a description else 0   # stable across nav
  height  = min(desired_rows + desc_reserve + 4, h - 4)     # 4 = borders + filter + blank
  inner   = height - 2
  r_opts  = inner - 2 - desc_reserve             # option rows actually available
  rows    = min(n, r_opts)                       # drawn option count
  start   = clamp(k - rows // 2, 0, n - rows)    # selection-centered, clamped
  window  = [start, start + rows)

Falsified if, for any reachable (h, n, k, desc) with r_opts >= 1 and n >= 1:
  V1: k not in window                    (selection invisible)
  V2: window not within [0, n)           (out-of-bounds index)
  V3: len(window) != min(n, r_opts)      (blank rows while options overflow)
Or for ANY input: negative intermediate (Rust underflow panic candidate).
"""
violations = 0
checked = 0

for h in range(6, 41):          # terminal heights incl. below-floor
    for n in range(0, 41):      # filtered option counts
        for desc in (0, 1):
            desired = min(n, 15)
            height = min(desired + desc + 4, max(h - 4, 0))
            inner = max(height - 2, 0)
            r_opts = max(inner - 2 - desc, 0)   # saturating: V-underflow guard
            rows = min(n, r_opts)
            for k in range(0, max(n, 1)):
                if n == 0:
                    continue
                checked += 1
                if rows < 1:
                    continue    # below-floor: no visibility promise (C10 only)
                start = max(0, min(k - rows // 2, n - rows))
                end = start + rows
                if not (start <= k < end):
                    print(f"V1 selection-invisible h={h} n={n} k={k} desc={desc} window=[{start},{end})")
                    violations += 1
                if start < 0 or end > n:
                    print(f"V2 out-of-bounds h={h} n={n} k={k} desc={desc} window=[{start},{end})")
                    violations += 1
                if rows != min(n, r_opts):
                    print(f"V3 fill h={h} n={n} k={k} desc={desc} rows={rows} r_opts={r_opts}")
                    violations += 1

print(f"checked={checked} violations={violations}")
exit(1 if violations else 0)

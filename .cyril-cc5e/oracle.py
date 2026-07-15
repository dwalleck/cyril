#!/usr/bin/env python3
"""cyril-cc5e oracle: hand-derived picker geometry, independent of ratatui.

Model (derived by reading picker.rs constants, no rendering):
  width  = min(80, w - 4)
  visible = min(n, 15)                      # take(visible), always from index 0
  height = min(visible + 6, h - 4)
  inner  = height - 2                        # top/bottom border
  paragraph lines: [filter, blank] + for i in 0..visible:
      option i; if i == selected: description line follows
  a paragraph line is on screen iff its index < inner
Emits the same SCENARIO lines as the probe for a byte diff.
"""

def scenario(name, w, h, n, selected):
    visible = min(n, 15)
    height = min(visible + 6, h - 4)
    inner = height - 2

    lines = []  # each entry: option index or None (filter/blank/desc)
    lines.append(None)  # filter
    lines.append(None)  # blank
    for i in range(visible):
        lines.append(i)
        if i == selected:
            lines.append(None)  # description line of the selected item

    on_screen = lines[:inner]
    drawn = [f"opt-{i:02}" for i in on_screen if i is not None]
    marker = selected in [i for i in on_screen if i is not None]
    print(
        f"SCENARIO {name} w={w} h={h} n={n} sel={selected} "
        f"marker={str(marker).lower()} drawn={','.join(drawn)}"
    )

scenario("A-control-80x24", 80, 24, 30, 5)
scenario("B-deep-sel-80x24", 80, 24, 30, 20)
scenario("C-floor-60x16", 60, 16, 15, 14)
scenario("D-floor-top-60x16", 60, 16, 15, 0)

#!/usr/bin/env python3
"""v3-coverage lane (kiro-cli 2.31.1): count + quote literal occurrences in the
KAS 0.66.34 bundle and the TUI 2.31.1 bundle (static; reads files only).

usage: static-v3cov-ctx-2.31.1.py [--kas|--tui|--both] [-w WIDTH] [-n MAXHITS] PATTERN...
PATTERN is a literal substring (use re: prefix for a regex).
Prints, per bundle, the count and up to MAXHITS contexts of WIDTH chars (default 200).
"""
import os, re, sys

HOME = os.path.expanduser("~")
KAS = f"{HOME}/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/dist/server/acp-server.js"
KAS_OLD = f"{HOME}/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js"
TUI = f"{HOME}/.local/share/kiro-research/tui-bundles/kiro-tui-2.31.1.js"
TUI_OLD = f"{HOME}/.local/share/kiro-research/tui-bundles/kiro-tui-2.29.1.js"

def load(p):
    with open(p, "r", encoding="utf-8", errors="replace") as f:
        return f.read()

def main(argv):
    which = ["kas", "tui"]; width = 200; maxhits = 3; pats = []
    it = iter(argv)
    for a in it:
        if a == "--kas": which = ["kas"]
        elif a == "--tui": which = ["tui"]
        elif a == "--both": which = ["kas", "tui"]
        elif a == "--old": which = which + [w + "_old" for w in which]
        elif a == "-w": width = int(next(it))
        elif a == "-n": maxhits = int(next(it))
        else: pats.append(a)
    paths = {"kas": KAS, "tui": TUI, "kas_old": KAS_OLD, "tui_old": TUI_OLD}
    for w in which:
        text = load(paths[w])
        for p in pats:
            rx = re.compile(p[3:]) if p.startswith("re:") else re.compile(re.escape(p))
            hits = list(rx.finditer(text))
            print(f"[{w}] {p!r}: {len(hits)}")
            half = width // 2
            for m in hits[:maxhits]:
                s = max(0, m.start() - half); e = min(len(text), m.end() + half)
                print(f"   @{m.start()}: {text[s:e]!r}"[: width + 40])
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

#!/usr/bin/env python3
"""Static TUI gate/handshake extractor (kiro-cli 2.26.0 -> 2.28.0 audit).

Companion to static-tui-vocab-2.28.0.py. For each tui.js bundle, pulls the
handful of anchored code regions that define what Kiro's own TUI SENDS to the
engines, so they can be diffed release-to-release without trusting minified names:

  settings   - the KAS AgentSettings builder (anchor: the '[kas-settings] Built
               settings for initialize:' log line) -> rows like
               ["chat.enableThinking","thinking"], rollout-gated rows, defaults
  rollout    - rollout-gated setting pairs ([["memory","memoryEnable"],...])
  subcmds    - hidden host IPC subcommands: ["chat","_","<name>"]
  methods    - every _kiro/*, kiro.dev/*, kiro/*, session/* string literal
  agentcaps  - agentCapabilities._meta.kiro fields the TUI parses
  spawn      - KAS spawn argv / env region (anchor: "--transport=stdio")
  prompts    - `_meta.kiro.<x>` keys the TUI attaches to session/prompt
               (anchor: displayText/outputStyle meta wrappers)

Usage: python3 -I static-tui-gates-2.28.0.py BUNDLE [BUNDLE ...] > out.txt
Read-only; nothing is executed.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path


def around(t: str, needle: str, before: int, after: int) -> str:
    i = t.find(needle)
    return "" if i < 0 else t[max(0, i - before): i + len(needle) + after]


def main() -> int:
    for arg in sys.argv[1:]:
        p = Path(arg)
        t = p.read_text(encoding="utf-8", errors="replace")
        print(f"######## {p.name}")
        sb = around(t, "[kas-settings] Built settings for initialize:", 3200, 50)
        rows = sorted(set(re.findall(r'\["([a-zA-Z.]+)","([_a-zA-Z]+)"\]', sb)))
        print("settings.rows:", rows)
        print("settings.defaults:", sorted(set(re.findall(r'\{(?:[a-zA-Z]+:!0,?)+[^}]*\}', sb)))[:3])
        print("settings.rolloutChecks:", sorted(set(re.findall(r'[A-Za-z]{1,3}\("([a-z_0-9]+)"\)', sb))))
        print("settings.mentions:", sorted(set(k for k in ("subagentOrchestration", "memory", "workflowNotifications",
                                                          "toolSearch", "compaction", "knowledge", "c2s", "todoList",
                                                          "userMemoryOptIn", "infraSafetyMonitor") if k in sb)))
        m = re.search(r'=\[\["memory","memoryEnable"\][^;]{0,200}', t)
        print("rollout.pairs:", m.group(0) if m else None)
        print("subcmds:", sorted(set(re.findall(r'"chat","_","([a-z-]+)"', t))))
        meths = sorted(set(re.findall(r'"((?:_kiro|_?kiro\.dev|kiro|session)/[A-Za-z_/]+)"', t)))
        print(f"methods({len(meths)}):", meths)
        caps = around(t, "sourceProviders:typeof t.sourceProviders", 400, 300)
        print("agentcaps:", sorted(set(re.findall(r'([a-zA-Z]+):(?:n\(t\.|typeof t\.)', caps))))
        sp = around(t, '"--transport=stdio"', 300, 700)
        print("spawn.args:", re.findall(r'"(--[a-z-]+(?:=[a-z-]+)?)"|`(--[a-z-]+)=', sp))
        print("spawn.env:", sorted(set(re.findall(r'\b([A-Z][A-Z0-9_]{5,}):', sp))))
        pm = sorted(set(re.findall(r'kiro:\{\.\.\.[A-Za-z0-9_$]+\([a-z]\.kiro\),([a-zA-Z]+):', t)))
        print("prompt.meta.kiro keys:", pm)
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

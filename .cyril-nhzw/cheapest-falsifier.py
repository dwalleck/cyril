#!/usr/bin/env python3
"""cyril-nhzw cheapest falsifier (Claim 9 + table integrity, Claims 2/4/5).

Applies the design's mapping table (transcribed VERBATIM from v2 zme() in
kiro-tui-2.8.1.js) to the REAL ~/.kiro/settings/cli.json and asserts the
produced AgentSettings equals the hand-derived expected object.

Independent oracle: the expected AgentSettings is computed by walking zme()'s
extracted logic by hand (a different derivation than the table application here);
the mapping table itself came from the bundle, not invention. A wrong key name,
a missing default, or a dropped type-guard makes the produced object differ from
expected -> this FAILS. Runs in <1s, before any Rust.
"""
import json, os, sys

CLI = os.path.expanduser("~/.kiro/settings/cli.json")

# zme()'s [cli.json key -> AgentSettings key] table (verbatim from the bundle).
BOOL_MAP = [
    ("chat.enableThinking", "thinking"),
    ("chat.enableKnowledge", "knowledge"),
    ("chat.enableCodeIntelligence", "codeIntelligence"),
    ("chat.enableTodoList", "todoList"),
    ("chat.enableCheckpoint", "checkpoint"),
    ("chat.enableTangentMode", "tangentMode"),
    ("chat.disableAutoCompaction", "disableAutoCompaction"),
    ("chat.enableSubagent", "_subagent"),
    ("chat.enableDelegate", "_delegate"),
]
DEFAULTS_ON = ["codeIntelligence", "knowledge", "thinking", "subagentOrchestration"]


def marshal(e: dict) -> dict:
    """cyril's proposed AgentSettings marshaler (design under test)."""
    n = {}
    for src, dst in BOOL_MAP:
        v = e.get(src)
        if isinstance(v, bool):            # zme: typeof === 'boolean' (no coercion)
            n[dst] = {"enabled": v}
    for k in DEFAULTS_ON:                   # defaults applied only if absent
        if k not in n:
            n[k] = {"enabled": True}
    a = e.get("toolSearch.enabled")
    if isinstance(a, bool):
        ts = {"enabled": a}
        if isinstance(e.get("toolSearch.minPct"), (int, float)) and not isinstance(e.get("toolSearch.minPct"), bool):
            ts["minPct"] = e["toolSearch.minPct"]
        if isinstance(e.get("toolSearch.minTokens"), (int, float)) and not isinstance(e.get("toolSearch.minTokens"), bool):
            ts["minTokens"] = e["toolSearch.minTokens"]
        n["toolSearch"] = ts
    r = e.get("compaction.excludeContextWindowPercent")
    o = e.get("compaction.excludeMessages")
    numr = isinstance(r, (int, float)) and not isinstance(r, bool)
    numo = isinstance(o, (int, float)) and not isinstance(o, bool)
    if numr or numo:
        comp = {"enabled": True}
        if numr: comp["excludePercent"] = r
        if numo: comp["excludeMessages"] = o
        n["compaction"] = comp
    return n


# Independent oracle: hand-derived expected AgentSettings for THIS cli.json.
# (thinking+todoList present-true flat keys; knowledge/codeIntelligence/
# subagentOrchestration via default-on; toolSearch enabled with minPct/minTokens=0.)
EXPECTED = {
    "thinking": {"enabled": True},
    "todoList": {"enabled": True},
    "knowledge": {"enabled": True},
    "codeIntelligence": {"enabled": True},
    "subagentOrchestration": {"enabled": True},
    "toolSearch": {"enabled": True, "minPct": 0, "minTokens": 0},
}

with open(CLI) as f:
    settings = json.load(f)
got = marshal(settings)
print("produced AgentSettings:", json.dumps(got, sort_keys=True))
print("expected  AgentSettings:", json.dumps(EXPECTED, sort_keys=True))
if got == EXPECTED:
    print("\nPASS — marshaler matches the zme-derived oracle on the live cli.json.")
    sys.exit(0)
print("\nFAIL — mismatch:")
for k in sorted(set(got) | set(EXPECTED)):
    if got.get(k) != EXPECTED.get(k):
        print(f"  {k}: got={got.get(k)} expected={EXPECTED.get(k)}")
sys.exit(1)

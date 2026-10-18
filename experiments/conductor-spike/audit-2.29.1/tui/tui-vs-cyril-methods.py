#!/usr/bin/env python3
"""Trap-3 + trap-2 done properly for METHOD parity (not just version-to-version).

Direction matters and is the whole point:
  * OUTBOUND  = a method the TUI *calls* (sendExtMethod/sendExtNotification/
                callExtMethod/extMethod(...)). cyril is the ACP client too, so an
                outbound method cyril never calls is a *feature gap*, not a break.
  * INBOUND   = a method the TUI *answers* (capability handler table, or
                onExtNotification subscription). These are the server->client
                requests; if cyril cannot answer one, KAS gets MethodNotFound.
  * LITERAL   = every `_kiro/*`/`kiro.dev/*` literal, regardless of direction.

Usage: python3 tui-vs-cyril-methods.py [repo-root]
Writes: outbound-methods.txt, tui-vs-cyril-methods.txt
"""
import re, subprocess, sys, os

BUNDLES = "/home/dwalleck/.local/share/kiro-research/tui-bundles"
VERSIONS = ["2.28.0", "2.29.1"]
OUT_PATTERNS = [
    r'sendExtMethod\("([^"]+)"', r'sendExtNotification\("([^"]+)"',
    r'callExtMethod\("([^"]+)"', r'\.extMethod\(\s*"([^"]+)"',
    r'\bextMethod\(\s*"([^"]+)"', r'sendRequest\("([^"]+)"',
    r'\.request\(\s*"([^"]+)"', r'extMethod\(\s*this\.ext\(\s*"([^"]+)"',
]
LITERAL = r'"_kiro/[A-Za-z0-9_./-]*"|"(?:_kiro\.dev|kiro\.dev)/[A-Za-z0-9_./-]*"'


def load(v):
    return open(f"{BUNDLES}/kiro-tui-{v}.js", encoding="utf-8", errors="replace").read()


def outbound(text):
    out = set()
    for p in OUT_PATTERNS:
        out |= set(re.findall(p, text))
    return {m for m in out if "kiro" in m}


def literals(text):
    return {m.strip('"') for m in re.findall(LITERAL, text)}


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    here = os.path.join(root, "experiments/conductor-spike/audit-2.29.1/tui")
    texts = {v: load(v) for v in VERSIONS}
    outs = {v: outbound(texts[v]) for v in VERSIONS}
    lits = {v: literals(texts[v]) for v in VERSIONS}

    with open(os.path.join(here, "outbound-methods.txt"), "w") as f:
        for v in VERSIONS:
            f.write(f"# {v}: {len(outs[v])} outbound kiro method literals\n")
            for m in sorted(outs[v]):
                f.write(f"  {m}\n")
        a, b = outs[VERSIONS[0]], outs[VERSIONS[1]]
        f.write(f"\nadded:   {sorted(b - a)}\nremoved: {sorted(a - b)}\n")
        f.write(f"literal sets: {len(lits[VERSIONS[0]])} vs {len(lits[VERSIONS[1]])} "
                f"added={sorted(lits[VERSIONS[1]] - lits[VERSIONS[0]])} "
                f"removed={sorted(lits[VERSIONS[0]] - lits[VERSIONS[1]])}\n")

    # cyril corpus: every string literal in crates/*.rs (method names may be
    # written with or without the leading underscore, so compare both forms).
    corpus = subprocess.run(
        ["bash", "-lc", "find crates -name '*.rs' -print0 | xargs -0 cat"],
        capture_output=True, text=True, cwd=root).stdout
    cy = set(re.findall(r'"([^"\n]*)"', corpus))

    def has(m):
        return m in cy or (m.startswith("_kiro") and m[1:] in cy) or (m.startswith("_kiro.dev") and m[1:] in cy)

    with open(os.path.join(here, "tui-vs-cyril-methods.txt"), "w") as f:
        f.write("method\tcyril_literal?\n")
        for m in sorted(outs[VERSIONS[1]]):
            f.write(f"{m}\t{'YES' if has(m) else 'NO'}\n")
        f.write("\n# inbound-capability literals referenced by the 2.29.1 TUI, cyril check\n")
        for m in ["_kiro/secret/get", "_kiro/secret/store", "_kiro/secret/delete",
                  "_kiro/userInput", "_kiro/openExternalUrl", "_kiro/tools/content_chunk",
                  "_kiro/codeIntelligence", "_kiro/sandbox/status", "_kiro/sandbox/applyConfig",
                  "_kiro/mcp/status", "_kiro/mcp/governance_disabled", "_kiro/governance/state",
                  "_kiro/diagnostics/changed", "_kiro/steering/documents_changed",
                  "_kiro/session/setWorkflowNotificationDelivery", "kiro.dev/goal/status",
                  "_kiro/spec/phaseCheckpoint", "_kiro/spec/taskStatusChanged",
                  "kiro.dev/telemetry/identityChanged", "kiro.dev/telemetry/modeChanged",
                  "kiro.dev/telemetry/processHealth", "kiro.dev/telemetry/uiModeChanged"]:
            f.write(f"{m}\t{'YES' if has(m) else 'NO'}\n")
    print(open(os.path.join(here, "tui-vs-cyril-methods.txt")).read())


if __name__ == "__main__":
    main()

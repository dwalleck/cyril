#!/usr/bin/env python3
"""cyril-booz STATIC PROBE: three carve claims that reframe the issue premise.

C1  hooksBlock gating: the system-prompt hooks briefing is gated on the
    RESOLVED client (`agentContext.client`), and unrecognized clientInfo
    names fall back to kiro-ide -> cyril's sessions DO carry the hooksBlock
    (contradicting the tpfd findings' "KAS omits its briefing" attribution).
C2  hooksBlock content: the briefing is an AUTHORING briefing; it never
    mentions <HOOK_INSTRUCTION> -> it confers no instruction authority.
C3  framing asymmetry: PreToolUse/PostToolUse/PostFile injection sites wrap
    hook output in explicit authority framing ("...you must address"), while
    the sessionStart precomputed path appends BARE blocks with no framing.

Run against BOTH shipped bundles (2.13.0, 2.14.1) — cross-version agreement
is the independence axis for the carve itself.
"""
import re
import sys
from pathlib import Path

BUNDLES = {
    "2.13.0": Path.home() / ".local/share/kiro-cli/kas/2.13.0-6b915aea9d06da0c64b46da6eb0e2cf40bfe1db59d01ddb8d701ecdbdc874f39/node_modules/@kiro/agent/dist/server/acp-server.js",
    "2.14.1": Path.home() / ".local/share/kiro-cli/kas/2.14.1-7697bd37c50cbc76a6aaffad80b85a4d4e4f1b5fbf0b21ef2efce9883e56a15d/node_modules/@kiro/agent/dist/server/acp-server.js",
}

FRAMING_RE = re.compile(r"Each <HOOK_INSTRUCTION> block below is a separate[^`\\]*")


def carve(ver: str, path: Path) -> dict:
    data = path.read_text(errors="replace")
    out: dict[str, object] = {"version": ver}

    # C1a: the gate expression uses the agentContext-resolved client.
    out["gate_expr"] = bool(re.search(r'client2 === "kiro-ide" \? hooksBlock : ""', data))
    gb = data.find("function getBasePrompt")
    out["gate_reads_agentContext"] = "const client2 = agentContext.client" in data[gb : gb + 300]

    # C1b: unrecognized clientInfo.name falls back (0wyn: inferred type ->
    # kiro-ide when not sandbox). Capture the fallback log + the inference.
    m = re.search(r"Unrecognized clientInfo\.name[^`]{0,120}", data)
    out["fallback_log"] = m.group(0)[:140] if m else None
    # resolveAgentContext's if/else chain (0wyn oracle carve): unrecognized ->
    # sandbox ? kiro-web : kiro-ide.
    rc = data.find("function resolveAgentContext")
    chain = data[rc : rc + 500]
    out["sandbox_inference"] = (
        'executionEnvironment2 === "sandbox"' in chain
        and 'client2 = "kiro-web"' in chain
        and 'client2 = "kiro-ide"' in chain
    )

    # C2: the hooksBlock text never mentions HOOK_INSTRUCTION.
    i = data.find("hooksBlock = `<hooks>")
    j = data.find("`;", i)
    block = data[i:j] if i != -1 else ""
    out["hooksBlock_found"] = i != -1
    out["hooksBlock_mentions_HOOK_INSTRUCTION"] = "HOOK_INSTRUCTION" in block
    out["hooksBlock_is_authoring"] = "createHook" in block and "Open Kiro Hook UI" in block

    # C3a: framed sites — KAS's own authority formula at tool/file hook sites.
    out["framing_sites"] = len(FRAMING_RE.findall(data))
    fm = FRAMING_RE.search(data)
    out["framing_formula"] = fm.group(0)[:90] if fm else None

    # C3b: the sessionStart precomputed consumer appends a BARE block: between
    # handlePrecomputedTrigger and its updateUserPromptMessage return there is
    # no framing formula, and the append is the raw wrapper.
    h = data.find("async function handlePrecomputedTrigger")
    tail = data[h : data.find("function getPromptInformation", h)]
    out["precomputed_appends_bare"] = (
        "<HOOK_INSTRUCTION>" in tail and not FRAMING_RE.search(tail)
    )
    # The only preamble in that path is the runCommand content prefix:
    out["precomputed_prefix"] = "[Session Start Hook Output]" in tail
    return out


def main() -> int:
    rows = [carve(v, p) for v, p in BUNDLES.items() if p.exists()]
    for r in rows:
        print(r)
    if len(rows) != 2:
        print("PROBE INCOMPLETE: missing a bundle", file=sys.stderr)
        return 1
    a, b = rows
    agree = {k for k in a if k != "version" and a[k] == b[k]}
    diff = {k for k in a if k != "version" and a[k] != b[k]}
    print(f"\ncross-version agreement on {sorted(agree)}")
    if diff:
        print(f"DISAGREE on {sorted(diff)}")
    ok = (
        not diff
        and a["gate_expr"]
        and a["gate_reads_agentContext"]
        and a["fallback_log"]
        and a["sandbox_inference"]
        and a["hooksBlock_found"]
        and not a["hooksBlock_mentions_HOOK_INSTRUCTION"]
        and a["framing_sites"] >= 3
        and a["precomputed_appends_bare"]
    )
    print("PROBE:", "C1+C2+C3 HOLD on both bundles" if ok else "CLAIMS DO NOT HOLD")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

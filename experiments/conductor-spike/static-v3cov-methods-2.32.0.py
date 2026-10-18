#!/usr/bin/env python3
"""v3-coverage lane (kiro-cli 2.32.0): classify every `_kiro/*` method literal in
KAS 0.66.43 by direction from its KAS call shape, and count it in the TUI 2.32.0
bundle and in cyril's non-test Rust sources (cyril refers to `_kiro/x` as
`kiro/x` because the acp crate strips/adds the leading underscore).
Static: reads files only. usage: static-v3cov-methods-2.32.0.py <cyril-root> <surface.json>
Direction legend:
  C2A  client->agent request (KAS `case"_kiro/..."` dispatch)
  A2C-N agent->client notification (`extNotification("...")`, emitter tables)
  A2C-R agent->client request (`Nh(conn,"...")` / `extMethod("...")`)
  other prefixes / internal
"""
import json, os, re, sys

HOME = os.path.expanduser("~")
KAS = f"{HOME}/.local/share/kiro-research/kas-carves/2.32.0/node_modules/@kiro/agent/dist/server/acp-server.js"
TUI = f"{HOME}/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js"

def rd(p):
    with open(p, encoding="utf-8", errors="replace") as f: return f.read()

def cyril_hits(root, needle):
    out = []
    for d, _, fs in os.walk(os.path.join(root, "crates")):
        if "/tests" in d: continue
        for f in fs:
            if not f.endswith(".rs"): continue
            p = os.path.join(d, f)
            text = rd(p)
            # skip #[cfg(test)] mod tests tails crudely: cut at "mod tests {"
            cut = text.find("mod tests {")
            body = text if cut < 0 else text[:cut]
            for i, line in enumerate(body.splitlines(), 1):
                s = line.strip()
                if s.startswith("//"): continue
                if needle in line: out.append(f"{os.path.relpath(p, root)}:{i}")
    return out

def main(root, surface):
    kas, tui = rd(KAS), rd(TUI)
    lits = json.load(open(surface))["trees"][1]["method_literals"]
    c2a = set(re.findall(r'case"(_kiro/[A-Za-z_/]+)"', kas))
    a2cn = set(re.findall(r'extNotification\("(_kiro/[A-Za-z_/]+)"', kas))
    a2cr = set(re.findall(r'(?:Nh\([a-z.]+,|extMethod\()"(_kiro/[A-Za-z_/]+)"', kas))
    manual_n = {"_kiro/c2s/view/changed", "_kiro/configuration/state", "_kiro/powers/items_changed",
                "_kiro/progressive_context/items_changed", "_kiro/safety/statusChanged",
                "_kiro/steering/documents_changed", "_kiro/tools/didChange"} | {
                l for l in lits if l.startswith("_kiro/workflow/") and l.split("/")[-1] in
                ("run_start","node_start","node_complete","node_paused","node_failed","loop_iteration","watch_poll","paused","run_complete","steps_queued")}
    manual_r = {"_kiro/auth/getAccessToken","_kiro/hooks/executeHook","_kiro/hooks/sessionStart","_kiro/mcp/elicitation",
                "_kiro/terminal/shell_type","_kiro/tool/get_diagnostics","_kiro/tool/semantic_rename","_kiro/tool/smart_relocate",
                "_kiro/workspace/active_file","_kiro/workspace/currently_open_files","_kiro/userInput"}
    rows = []
    for l in lits:
        d = []
        if l in c2a: d.append("C2A")
        if l in a2cn or l in manual_n: d.append("A2C-N")
        if l in a2cr or l in manual_r: d.append("A2C-R")
        if not d: d.append("internal/prefix")
        q = '"' + l + '"'
        ch = cyril_hits(root, '"' + l[1:] + '"') + cyril_hits(root, q)
        rows.append({"method": l, "dir": "+".join(d), "kas": kas.count(q), "tui": tui.count(q),
                     "cyril_nontest": len(ch), "cyril_first": ch[:2]})
    json.dump(rows, sys.stdout, indent=1)
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))

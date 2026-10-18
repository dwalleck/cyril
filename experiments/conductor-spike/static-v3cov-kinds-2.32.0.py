#!/usr/bin/env python3
"""v3-coverage lane (kiro-cli 2.32.0): per session_info_update kind, count the
quoted literal in KAS 0.66.43 (+0.66.34), TUI 2.32.0, and cyril crates/, and
quote the first TUI and KAS context (<=200 chars). Static; reads files only.
usage: static-v3cov-kinds-2.32.0.py <cyril-repo-root>
"""
import os, re, sys, json

HOME = os.path.expanduser("~")
KAS = f"{HOME}/.local/share/kiro-research/kas-carves/2.32.0/node_modules/@kiro/agent/dist/server/acp-server.js"
KAS_OLD = f"{HOME}/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/dist/server/acp-server.js"
TUI = f"{HOME}/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js"
KINDS = ["turn_start","turn_end","turn_completion","context_usage","user_message_id_assigned",
 "focus_update","steering_inclusion","steering_queued","steering_injected","steering_cleared",
 "display_error","summarization_started","summarization_separator","summarization_completed",
 "summarization_failed","summary_message","recap","queued","hook_update","pending_interaction",
 "interaction_resolved","checkpoint_restore_availability","repositories_update","model_routed",
 "model_fallback","model_fallback_promoted"]

def rd(p):
    with open(p, encoding="utf-8", errors="replace") as f: return f.read()

def ctx(t, needle, skip=()):
    for m in re.finditer(re.escape(needle), t):
        s = t[max(0, m.start()-100): m.end()+100]
        if any(k in s for k in skip): continue
        return s.replace("\n", " ")[:200]
    return ""

def rust_hits(root, needle):
    hits = []
    for d, _, fs in os.walk(os.path.join(root, "crates")):
        for f in fs:
            if not f.endswith(".rs"): continue
            p = os.path.join(d, f)
            for i, line in enumerate(rd(p).splitlines(), 1):
                if needle in line:
                    hits.append(f"{os.path.relpath(p, root)}:{i}")
    return hits

def main(root):
    kas, kold, tui = rd(KAS), rd(KAS_OLD), rd(TUI)
    rows = []
    for k in KINDS:
        q = f'"{k}"'
        ch = rust_hits(root, q)
        prod = [h for h in ch if "/tests/" not in h and "probe" not in h]
        rows.append({"kind": k, "kas_new": kas.count(q), "kas_old": kold.count(q), "tui": tui.count(q),
                     "cyril_hits": len(ch), "cyril_first": prod[:3],
                     "tui_ctx": ctx(tui, q), "kas_ctx": ctx(kas, q)})
    json.dump(rows, sys.stdout, indent=1)
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

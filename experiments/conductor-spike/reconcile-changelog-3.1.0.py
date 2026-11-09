#!/usr/bin/env python3
"""reconcile-changelog-3.1.0.py — merge the static lanes' changelog-reconciliation tables into one row per feed line.
    python3 -I reconcile-changelog-3.1.0.py <audit-dir> > changelog-reconciliation.md
Backbone: feed-<ver>.json lines in feed order. Overlays: tui (per-version `| # | line | location |`), kas-core
(`| release | line | location |`) and kas-workflow (`| feed | line | location |`) matched by version + word overlap,
host (prose blocks with `#N … → location`). A lane's "not in my lane"/"not mine" placements are dropped; what remains is
joined. Lines with no placement are listed at the end for hand reconciliation.
"""
import json, re, sys, pathlib
AUD = pathlib.Path(sys.argv[1])
VERS = "2.33.0 2.33.1 2.34.0 2.34.1 2.34.2 2.35.0 2.35.1 2.35.2 2.36.0 2.37.0 2.37.1 3.0.0 3.1.0".split()
STOP = set("the a an and or of to in on for with from is are now no longer when your you its it as by at be that this than into after before instead".split())
def words(s):
    s = re.sub(r"\[V[23]\]", " ", s.lower()); s = re.sub(r"[^a-z0-9_./-]+", " ", s)
    return {w for w in s.split() if w not in STOP and len(w) > 1}
feed = {}
for v in VERS:
    d = json.load(open(AUD / f"feed-{v}.json"))
    for e in d["entries"]:
        if e.get("version") == v:
            feed[v] = [f"[{c.get('type')}] {c.get('description')}" for c in e.get("changes", [])]
rows = {v: [dict(line=l, loc=[]) for l in feed[v]] for v in VERS}
def skip(loc):
    l = loc.lower()
    return ("not in my lane" in l) or l.strip(" *—-") in ("", "not mine") or l.startswith("not mine")
def best(v, text):
    tw = words(text); bi, bs = None, 0.0
    for i, r in enumerate(rows[v]):
        fw = words(r["line"]); 
        if not fw or not tw: continue
        sc = len(tw & fw) / len(tw | fw)
        if sc > bs: bi, bs = i, sc
    return bi if bs >= 0.18 else None
# tui: ### <ver> (n) then | # | line | location |
cur = None
for ln in (AUD / "tui/FINDINGS.md").read_text().splitlines():
    m = re.match(r"^### (\d+\.\d+\.\d+)", ln)
    if m: cur = m.group(1) if m.group(1) in rows else None; continue
    if ln.startswith("## ") and not ln.startswith("## Changelog"): cur = None
    if cur and re.match(r"^\| \d+ \|", ln):
        c = [x.strip() for x in ln.strip("|").split("|")]
        if len(c) >= 3 and not skip(c[2]):
            i = int(c[0]) - 1
            if 0 <= i < len(rows[cur]): rows[cur][i]["loc"].append(f"tui: {c[2]}")
    # multi-version headers like "### 2.34.1 (1) · 2.34.2 (1)": rows numbered per header; handle the first version only (rare)
# kas-core, kas-workflow: table rows | ver | line | location |
for lane, col in (("kas-core", "release"), ("kas-workflow", "feed")):
    txt = (AUD / f"{lane}/FINDINGS.md").read_text()
    sec = txt.split("## Changelog reconciliation", 1)[1].split("\n## ", 1)[0]
    for ln in sec.splitlines():
        if not ln.startswith("| ") or ln.startswith("| ---") or ln.startswith("|---"): continue
        c = [x.strip() for x in ln.strip("|").split("|")]
        if len(c) < 3 or c[0] in (col, "feed", "release"): continue
        vers = re.findall(r"\d+\.\d+\.\d+", c[0])
        if skip(c[2]): continue
        for v in vers:
            if v not in rows: continue
            i = best(v, c[1])
            if i is not None: rows[v][i]["loc"].append(f"{lane}: {c[2]}")
# host: prose per version; "#N … → location" fragments
txt = (AUD / "host/FINDINGS.md").read_text()
sec = txt.split("## Changelog reconciliation", 1)[1].split("\n## ", 1)[0]
for block in re.split(r"\n\*\*(?=\d+\.\d+\.\d+)", sec):
    m = re.match(r"(\d+\.\d+\.\d+)", block)
    if not m or m.group(1) not in rows: continue
    v = m.group(1)
    for frag in re.finditer(r"#(\d+(?:\s*/\s*#\d+)*)([^#→]*?)→\s*([^;]+?)(?=(?:;|\.\s|$|#\d))", block):
        nums = re.findall(r"\d+", frag.group(1)); loc = frag.group(3).strip()
        if skip(loc) or "not mine" in loc.lower(): continue
        for n in nums:
            i = int(n) - 1
            if 0 <= i < len(rows[v]): rows[v][i]["loc"].append(f"host: {loc}")
print("| ver | # | line | location (lane: placement) |\n|---|---|---|---|")
missing = []
for v in VERS:
    for i, r in enumerate(rows[v], 1):
        line = r["line"]; short = (line[:110] + "…") if len(line) > 112 else line
        loc = " · ".join(dict.fromkeys(x.replace("\n", " ") for x in r["loc"])) if r["loc"] else "**UNPLACED**"
        if not r["loc"]: missing.append((v, i, line))
        print(f"| {v} | {i} | {short} | {loc} |")
print(f"\n_{sum(len(rows[v]) for v in VERS)} lines; {len(missing)} unplaced_")
for v, i, l in missing: print(f"- {v} #{i}: {l}")

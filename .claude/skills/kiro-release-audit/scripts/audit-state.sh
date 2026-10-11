#!/usr/bin/env bash
# audit-state.sh — step 0 of the kiro-release-audit skill. Read-only.
# Prints: installed version, archive, OLD (newest audit doc), latest manifest version, feed.json
# changelog for releases after OLD, @kiro/agent per carve, rivets kiro-<ver> labels, RUN-STATE.
set -uo pipefail
REPO=${REPO:-/home/dwalleck/repos/cyril}
ARCHIVE=${ARCHIVE:-$HOME/.local/share/kiro-research}
ORIGIN=https://desktop-release.q.us-east-1.amazonaws.com
FEED=https://prod.download.cli.kiro.dev/stable

echo "== installed"
if command -v kiro-cli >/dev/null 2>&1; then
  inst=$(kiro-cli --version 2>/dev/null | awk '{print $2}')
  echo "kiro-cli $inst  ($(readlink -f "$(command -v kiro-cli)"), mtime $(stat -c %y "$(readlink -f "$(command -v kiro-cli)")" | cut -c1-16))"
else
  inst=""; echo "kiro-cli not on PATH"
fi
echo "installed tui.js sha256: $(cut -c1-16 "$HOME/.local/share/kiro-cli/tui.js.sha256" 2>/dev/null || echo none)"
echo "self-extracted KAS trees: $(ls -d "$HOME"/.local/share/kiro-cli/kas/*/ 2>/dev/null | sed 's|.*/kas/||; s|-.*||' | sort -V | tr '\n' ' ')"

echo "== archive ($ARCHIVE)"
echo "binaries: $(ls "$ARCHIVE/binaries" 2>/dev/null | sort -V | tr '\n' ' ')"
echo "kas-carves: $(ls "$ARCHIVE/kas-carves" 2>/dev/null | sort -V | tr '\n' ' ')"
echo "tui-bundles: $(ls "$ARCHIVE/tui-bundles" 2>/dev/null | grep -v sha256 | sed 's/kiro-tui-//; s/\.js//' | sort -V | tr '\n' ' ')"
for d in "$ARCHIVE"/kas-carves/*/; do
  pj="$d/node_modules/@kiro/agent/package.json"
  [ -f "$pj" ] && printf '  %s -> @kiro/agent %s\n' "$(basename "$d")" "$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$pj")"
done | sort -V

echo "== OLD (newest audit doc)"
# newest audit doc whose verdict is written (a draft skeleton carries "Verdict: TBD" and is NOT a baseline)
old=""
for f in $(ls "$REPO"/docs/kiro-*-wire-audit.md 2>/dev/null | sed 's|.*/kiro-||; s|-wire-audit.md||' | sort -V); do
  grep -q 'Verdict: TBD' "$REPO/docs/kiro-$f-wire-audit.md" && { echo "draft (skipped as baseline): $f"; continue; }
  old=$f
done
echo "OLD=$old  ($(ls "$REPO"/docs/kiro-*-wire-audit.md | wc -l) audit docs)"

echo "== origin"
latest=$(curl -s --max-time 20 "$FEED/latest/manifest.json" | python3 -I -c 'import json,sys; d=json.load(sys.stdin); print(d.get("currentVersion") or d.get("version") or "")' 2>/dev/null)
echo "latest manifest: ${latest:-unreachable}"

echo "== releases after OLD (feed.json probe; bounded)"
python3 -I - "$old" "${latest:-}" "${inst:-}" "$FEED" "$ORIGIN" <<'EOF'
import json, sys, urllib.request
old, latest, inst, feed, origin = sys.argv[1:6]
def ver(s): return tuple(int(x) for x in s.split('.'))
def fetch(url):
    try:
        with urllib.request.urlopen(url, timeout=20) as r: return r.read()
    except Exception: return None
def head(url):
    try:
        req = urllib.request.Request(url, method="HEAD")
        with urllib.request.urlopen(req, timeout=20) as r: return r.status == 200
    except Exception: return False
cands = set(v for v in (latest, inst) if v)
if old:
    M, m, p = ver(old)
    cands |= {f"{M}.{m}.{p+i}" for i in range(1, 6)}
    cands |= {f"{M}.{m+j}.{k}" for j in range(1, 3) for k in range(0, 4)}
found = []
for v in sorted(cands, key=ver):
    if old and ver(v) <= ver(old): continue
    if latest and ver(v) > ver(latest): continue
    b = fetch(f"{feed}/{v}/feed.json")
    if not b: continue
    try: d = json.loads(b)
    except Exception: continue
    for e in d.get("entries", []):
        if e.get("version") == v:
            found.append(v)
            # a feed entry does not imply a shipped artifact (2.28.1 had a changelog and no tarball)
            art = "no Linux tarball on origin" if not any(
                head(f"{origin}/{v}/kirocli-x86_64-linux.{ext}") for ext in ("tar.xz", "tar.zst")) else "tarball present"
            print(f"-- {v} ({e.get('date')}) — {art}")
            for c in e.get("changes", []): print(f"   [{c.get('type')}] {c.get('description')}")
if not found: print("(no feed entries found after OLD; verify manually)")
EOF

echo "== rivets labels"
grep -o '"kiro-2-[0-9]*-[0-9]*"' "$REPO/.rivets/issues.jsonl" 2>/dev/null | sort | uniq -c | sort -k2 -V | tail -5

echo "== in-flight audits"
for rs in "$REPO"/experiments/conductor-spike/audit-*/RUN-STATE.md; do
  [ -f "$rs" ] || continue
  echo "-- $rs"; grep -E '^- \[|^\| ' "$rs" | head -30
done
[ -n "$(ls "$REPO"/experiments/conductor-spike/audit-*/RUN-STATE.md 2>/dev/null)" ] || echo "(none)"

#!/usr/bin/env bash
# 2.19.2: `--output-format stream-json` (non-interactive JSON Lines run events).
# "Requires the v2 or v3 engine" — capture the event vocabulary on both.
# HOME-isolated; real XDG_DATA_HOME keeps the token store reachable.
set -u
OUT=${1:-.}
KIRO=${KIRO_BIN:-$HOME/.local/bin/kiro-cli}
REAL_XDG=${XDG_DATA_HOME:-$HOME/.local/share}
for engine in v2 v3; do
  FH=$(mktemp -d -t sj-home-XXXX); CW=$(mktemp -d -t sj-cwd-XXXX)
  echo "######## engine=$engine"
  ( cd "$CW" && HOME="$FH" XDG_DATA_HOME="$REAL_XDG" timeout 240 "$KIRO" chat --no-interactive --agent-engine "$engine" --output-format stream-json "Reply with exactly: OK" ) \
      > "$OUT/stream-json-$engine-2.19.2.jsonl" 2> "$OUT/stream-json-$engine-2.19.2.stderr"
  echo "exit=$? lines=$(wc -l < "$OUT/stream-json-$engine-2.19.2.jsonl") stderr_lines=$(wc -l < "$OUT/stream-json-$engine-2.19.2.stderr")"
  python3 - "$OUT/stream-json-$engine-2.19.2.jsonl" <<'EOF'
import json, sys
from collections import Counter
c = Counter(); keys = {}
bad = 0
for l in open(sys.argv[1]):
    l = l.strip()
    if not l: continue
    try: o = json.loads(l)
    except Exception: bad += 1; print("  non-JSON line:", l[:120]); continue
    t = o.get("type") or o.get("event") or o.get("kind") or "?"
    c[t] += 1
    keys.setdefault(t, set()).update(o.keys())
    if t in ("session_start","session/new","init","initialize","run_start","turn_end","turn_complete","result","done","end","stop","error") or c[t] == 1:
        print("  sample:", json.dumps(o)[:300])
print("  types:", dict(c), "| non-JSON:", bad)
for t in keys: print(f"  keys[{t}] = {sorted(keys[t])}")
EOF
  head -c 600 "$OUT/stream-json-$engine-2.19.2.stderr"; echo
done
# v1 negative control (changelog: not supported on v1)
FH=$(mktemp -d -t sj-home-XXXX); CW=$(mktemp -d -t sj-cwd-XXXX)
echo "######## engine=v1 (expect refusal)"
( cd "$CW" && HOME="$FH" XDG_DATA_HOME="$REAL_XDG" timeout 60 "$KIRO" chat --no-interactive --agent-engine v1 --output-format stream-json "Reply with exactly: OK" ) 2>&1 | head -5

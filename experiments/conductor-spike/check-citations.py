#!/usr/bin/env python3
"""check-citations.py — verify a release-audit lane's frame citations against its captures.

    python3 -I check-citations.py <FINDINGS.md> [<FINDINGS.md> ...]

Citation format (lanes.md brief template):

    `<capture>.jsonl#<N>`                  frame N, 0-based line index (= Python enumerate;
                                           `grep -n` / `sed -n` are 1-based: subtract 1)
    `<capture>.jsonl#<N>"<expect>"`        ...and the frame's JSON must contain <expect>
    `<capture>.jsonl#<N>-<M>"<expect>"`    some frame in N..M contains <expect>

Captures resolve against the FINDINGS file's directory, then its parent (the audit
dir), then experiments/conductor-spike/. Prose "frame N" / "frames N" outside a
`file#N` citation is UNVERIFIABLE and fails, so a lane cannot pass by ignoring the
format; a report with no frame references at all (a static lane) passes.
`--prose-ok` downgrades UNVERIFIABLE to a warning, for reading pre-2.31.1 FINDINGS.

Exit 1 when a capture is missing, an index is out of range, an <expect> quote is
absent from the cited frame(s), or a prose frame reference is unverifiable; each
failure prints the frame actually there, so a base or neighbour error is visible.
"""
import json
import pathlib
import re
import sys

CITE = re.compile(r'([A-Za-z0-9_.\-]+\.jsonl)#(\d+)(?:-(\d+))?(?:"([^"]+)")?')
BARE = re.compile(r'\bframes? \d+')
ROOT = pathlib.Path(__file__).resolve().parent


def summarize(rec):
    msg = rec.get("msg", rec) if isinstance(rec, dict) else {}
    if not isinstance(msg, dict):
        return str(msg)[:120]
    parts = [str(rec.get("dir", ""))]
    if "method" in msg:
        parts.append(msg["method"])
    elif "error" in msg:
        parts.append(f"error id={msg.get('id')} {json.dumps(msg['error'])[:100]}")
    elif "result" in msg:
        parts.append(f"result id={msg.get('id')}")
    params = msg.get("params") or {}
    update = params.get("update") if isinstance(params, dict) else None
    if isinstance(update, dict):
        parts.append(str(update.get("sessionUpdate", "")))
        kind = ((update.get("_meta") or {}).get("kiro") or {}).get("kind")
        if kind:
            parts.append(f"kind={kind}")
    return " ".join(p for p in parts if p)


def resolve(name, findings):
    for base in (findings.parent, findings.parent.parent, ROOT):
        cand = base / name
        if cand.is_file():
            return cand
    return None


def main(paths, prose_ok=False):
    failures = warnings = checked = 0
    cache = {}
    for p in paths:
        findings = pathlib.Path(p).resolve()
        text = findings.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), 1):
            cites = list(CITE.finditer(line))
            for m in cites:
                name, lo, hi, expect = m.group(1), int(m.group(2)), m.group(3), m.group(4)
                hi = int(hi) if hi else lo
                cap = resolve(name, findings)
                where = f"{findings.name}:{lineno} {m.group(0)}"
                if cap is None:
                    print(f"FAIL {where}: capture not found")
                    failures += 1
                    continue
                if cap not in cache:
                    cache[cap] = cap.read_text(encoding="utf-8").splitlines()
                rows = cache[cap]
                if hi >= len(rows):
                    print(f"FAIL {where}: index out of range ({len(rows)} frames)")
                    failures += 1
                    continue
                checked += 1
                if expect and not any(expect in rows[i] for i in range(lo, hi + 1)):
                    print(f"FAIL {where}: {expect!r} not in frame(s) {lo}..{hi}")
                    for i in range(max(0, lo - 1), min(len(rows), hi + 2)):
                        print(f"       #{i}: {summarize(json.loads(rows[i]))}")
                    failures += 1
            if BARE.search(CITE.sub("", line)):
                warnings += 1
                level = "WARN" if prose_ok else "FAIL"
                print(f"{level} UNVERIFIABLE {findings.name}:{lineno}: {line.strip()[:140]}")
    print(f"checked {checked} citation(s): {failures} failure(s), {warnings} unverifiable line(s)")
    return 1 if failures or (warnings and not prose_ok) else 0


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--prose-ok"]
    if not args:
        sys.exit(__doc__)
    sys.exit(main(args, prose_ok="--prose-ok" in sys.argv[1:]))

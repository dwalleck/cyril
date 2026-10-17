#!/usr/bin/env python3
"""unread-fields.py — wire fields cyril receives but never names, as a standing per-audit check.

    python3 -I unread-fields.py [--allowlist F] [--src DIR ...] [--all] <capture.jsonl> [...]

sweep-new-fields.py answers "what is new in Kiro NEW vs OLD"; this answers "what does Kiro
send that cyril has never read", the standing gap a delta cannot show.

For every agent->client frame in the captures (lane-dir format {dir, msg}; header records
without `dir` are skipped) it walks EVERY key path. Arrays collapse to `[]` and every element
is walked (sweep-new-fields walks five); dynamic map keys (ids, paths, server or tool names)
collapse to `<*>`. Paths are rooted by method: a notification under its `method`, a response
under `result:<method of the client->agent request with the same id>`. A response to a method
cyril never sends is probe-only and skipped (counted in the summary); a notification whose
method cyril never names is reported once as UNREAD <method> (whole notification).

A key counts as NAMED when cyril or the pinned agent-client-protocol crates mention it: as a
string literal ("key"), a struct field (`key:` or its snake_case form, for serde rename_all),
or a `rename = "key"`. The output is a LOWER BOUND: every path listed is never named anywhere,
but a pass is "no obvious gaps", not coverage. Three gaps pass silently: matching is by key
name, not path (a `name` read anywhere covers every `name`); a field the ACP crate deserialises
counts as named even if cyril then ignores it; and a field no capture exercised is invisible.
The runtime replay test proposed on cyril-58uv is the stronger check. Only the topmost unnamed path of a subtree is
listed (its children are implied).

--allowlist (default docs/wire-ignored-fields.txt) holds fnmatch globs, one per line, each with
a `# reason (issue)` comment. Listed paths not matched by an allowlist entry fail (exit 1):
triage each into HANDLED (fix cyril), or an allowlist entry naming why it is ignored or the
issue that tracks it. --all also prints allowlisted paths.
"""
import argparse
import fnmatch
import json
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
REGISTRY = pathlib.Path.home() / ".cargo/registry/src"
DYNAMIC = re.compile(r"[/:\\]|^[0-9a-f]{8}-|^\d+$|^sess_|^toolu_|^tooluse_|\.(?:rs|ts|md|json|js)$")


def acp_sources():
    lock = (REPO / "Cargo.lock").read_text(encoding="utf-8")
    dirs = []
    for name in ("agent-client-protocol", "agent-client-protocol-schema"):
        m = re.search(rf'name = "{re.escape(name)}"\nversion = "([^"]+)"', lock)
        if m:
            dirs += sorted(REGISTRY.glob(f"*/{name}-{m.group(1)}"))
    return dirs


def vocabulary(dirs):
    lits, idents, renames, methods = set(), set(), set(), set()
    for d in dirs:
        for f in pathlib.Path(d).rglob("*.rs"):
            if "/tests/" in str(f):
                continue
            text = f.read_text(encoding="utf-8", errors="replace")
            lits.update(re.findall(r'"([A-Za-z_$][A-Za-z0-9_$]*)"', text))
            idents.update(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*:", text))
            renames.update(re.findall(r'rename\s*=\s*"([^"]+)"', text))
            methods.update(m.lstrip("_") for m in re.findall(r'"(_?[A-Za-z][A-Za-z0-9_.\-]*/[A-Za-z0-9_/\-]+)"', text))
    return lits, idents, renames, methods


def snake(key):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", key).lower()


def named(key, vocab):
    lits, idents, renames, _ = vocab
    return key in lits or key in renames or key in idents or snake(key) in idents


def norm_key(key):
    return "<*>" if DYNAMIC.search(key) or len(key) > 48 else key


def walk(obj, path, out, frame):
    if isinstance(obj, dict):
        for k, v in obj.items():
            p = f"{path}.{norm_key(k)}"
            out.setdefault(p, frame)
            walk(v, p, out, frame)
    elif isinstance(obj, list):
        for v in obj:
            walk(v, path + "[]", out, frame)


def collect(captures):
    paths = {}
    for cap in captures:
        requests = {}
        for idx, line in enumerate(pathlib.Path(cap).read_text(encoding="utf-8").splitlines()):
            rec = json.loads(line)
            msg = rec.get("msg")
            if "dir" not in rec or not isinstance(msg, dict):
                continue
            if rec["dir"] == "client->agent":
                if "method" in msg and "id" in msg:
                    requests[msg["id"]] = msg["method"]
                continue
            frame = f"{pathlib.Path(cap).name}#{idx}"
            if "method" in msg:
                walk(msg.get("params"), msg["method"], paths, frame)
            elif "result" in msg:
                walk(msg["result"], "result:" + requests.get(msg.get("id"), "?"), paths, frame)
    return paths


def load_allowlist(p):
    globs = []
    if p.is_file():
        for line in p.read_text(encoding="utf-8").splitlines():
            g = line.split("#", 1)[0].strip()
            if g:
                globs.append(g)
    return globs


def main():
    ap = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    ap.add_argument("captures", nargs="+")
    ap.add_argument("--allowlist", default=str(REPO / "docs/wire-ignored-fields.txt"))
    ap.add_argument("--src", nargs="*", default=[str(REPO / "crates")])
    ap.add_argument("--all", action="store_true")
    a = ap.parse_args()

    vocab = vocabulary([pathlib.Path(s) for s in a.src] + acp_sources())
    methods = vocab[3]
    paths = collect(a.captures)
    sent = lambda m: m.lstrip("_") in methods or ("/" not in m and m in vocab[0])
    probe_only = {p.split(".", 1)[0] for p in paths
                  if p.startswith("result:") and not sent(p.split(".", 1)[0][7:])}
    whole = {p.split(".", 1)[0] for p in paths
             if not p.startswith("result:") and not sent(p.split(".", 1)[0])}
    for root in sorted(whole):
        paths.setdefault(root, next(f for q, f in paths.items() if q.startswith(root + ".")))
    unnamed = sorted(set(whole) | {
        p for p in paths
        if "." in p and p.split(".", 1)[0] not in probe_only | whole
        and not named(p.rsplit(".", 1)[-1].rstrip("[]"), vocab)
        and p.rsplit(".", 1)[-1] != "<*>"})
    topmost = [p for p in unnamed if not any(p.startswith(q + ".") or p.startswith(q + "[]")
                                             for q in unnamed if q != p)]
    globs = load_allowlist(pathlib.Path(a.allowlist))
    fail = 0
    for p in topmost:
        # `[]` is the array marker in paths and allowlist lines alike, never an fnmatch class
        hit = next((g for g in globs
                    if fnmatch.fnmatchcase(p.replace("[]", "\x00"), g.replace("[]", "\x00"))), None)
        if hit and not a.all:
            continue
        tag = " (whole notification)" if p in whole else ""
        print(f"{'ALLOWED' if hit else 'UNREAD '} {p}{tag}  ({paths[p]})")
        fail += hit is None
    print(f"{len(paths)} paths; {len(probe_only)} probe-only result roots skipped "
          f"({', '.join(sorted(r[7:] for r in probe_only))}); {len(topmost)} topmost never named "
          f"by cyril/ACP; {fail} not in allowlist ({a.allowlist})")
    return 1 if fail else 0


if __name__ == "__main__":
    sys.exit(main())

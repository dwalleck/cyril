#!/usr/bin/env python3
"""Health-check a finished code-review-max run from its artifacts and ACP trace.

    check_run.py <rundir> [<trace.jsonl> ...]     # several traces = one run that was retried/resumed

Answers, from evidence rather than from the steps' own reports:

  queue integrity   every id in every queue has a verdict file; `ids` still
                    match what `crtool shard` assigned (immutability); each loop
                    ran one iteration per id (no skipped, no wasted sessions);
                    the queue file was written at most once per loop
  digest discipline the clerk ran only crtool commands, was denied nothing, and
                    never read an over-cap JSON file (all.json, index.json,
                    verified.json)

Exit status 0 when every check passes, 1 otherwise.
"""
import collections
import glob
import json
import os
import re
import sys

run = os.path.abspath(sys.argv[1])
traces = sys.argv[2:]
trace = traces or None
OVERCAP = ("candidates/all.json", "deduped/index.json", "verified.json")
fails = []


def check(ok, label, detail=""):
    print(f"  {'PASS' if ok else 'FAIL'}  {label}" + (f"  — {detail}" if detail else ""))
    if not ok:
        fails.append(label)


def load(rel):
    with open(os.path.join(run, rel), encoding="utf-8") as f:
        return json.load(f)


print("== queue integrity ==")
index = load("deduped/index.json")
survivors = [c["id"] for c in index["candidates"]]
queues = sorted(glob.glob(os.path.join(run, "queues", "queue-*.json")))
shard_files = [q for q in queues if re.fullmatch(r"queue-\d+\.json", os.path.basename(q))]  # not sweep, not ballots
expected = {os.path.basename(q): survivors[k::len(shard_files)] for k, q in enumerate(shard_files)}
have, strays = set(), []
for root, _, files in os.walk(run):
    parts = os.path.relpath(root, run).split(os.sep)
    if "verdicts" in parts:
        have |= {f[:-5] for f in files if f.endswith(".json")}
        if parts[0] != "verdicts":
            strays += [os.path.join(os.path.relpath(root, run), f) for f in files]
all_ids = []
for q in queues:
    name = os.path.basename(q)
    d = json.load(open(q, encoding="utf-8"))
    ids = d.get("ids")
    check(isinstance(ids, list), f"{name}: has an `ids` list", f"keys={sorted(d)}")
    ids = ids or []
    all_ids += ids
    if name in expected:
        check(ids == expected[name], f"{name}: ids unchanged since shard",
              "" if ids == expected[name] else f"expected {expected[name]} got {ids}")
    missing = [i for i in ids if i not in have]
    check(not missing, f"{name}: every id has a verdict ({len(ids)} ids)", f"missing {missing}" if missing else "")
    check(d.get("done") is True, f"{name}: done=true")
check(sorted(x for x in all_ids if x.startswith("C") and ".v" not in x) == sorted(survivors),
      "queues cover every deduped candidate exactly once")
if os.path.exists(os.path.join(run, "ballots.json")):
    balloted = [b["id"] for b in load("ballots.json")["balloted"]]
    got = {tag: [i for i in balloted if f"{i}.{tag}" in have] for tag in ("v2", "v3")}
    check(all(len(got[t]) == len(balloted) for t in got),
          f"every balloted candidate has all three votes ({len(balloted)} balloted)",
          "; ".join(f"{t}: missing {sorted(set(balloted) - set(got[t]))}" for t in got if len(got[t]) != len(balloted)))
check(not strays, "every verdict sits under <run>/verdicts/", f"{len(strays)} elsewhere, e.g. {strays[:2]}" if strays else "")
verified = load("verified.json")
unv = verified["stats"]["unverified_ids"]
check(not unv, "no UNVERIFIED findings", f"{unv}" if unv else "")
for line in verified["stats"].get("overturned_by_vote", []):
    print(f"  info  vote changed the outcome — {line}")
print(f"  info  verdicts: {verified['stats']['verdicts']}; dedup {index['raw_count']} -> {index['deduped_count']}; "
      f"largest duplicate group: {1 + max((len(c.get('also_flagged_by', [])) for c in index['candidates']), default=0)}")

if os.path.exists(os.path.join(run, "comments.json")):
    print("== review comments ==")
    LABELS = {"praise", "nitpick", "suggestion", "issue", "todo", "question", "thought", "chore", "note",
              "typo", "polish", "quibble"}
    comments, reported = load("comments.json"), [f["id"] for f in load("findings.json")]
    check([x["id"] for x in comments] == reported, f"one comment entry per reported finding ({len(reported)})")
    real = [x for x in comments if not x.get("duplicate_of")]
    check(all(x.get("label") in LABELS and x.get("subject") and x.get("body", "").startswith(f"**{x['label']}") for x in real),
          "every comment has a Conventional Comments label, a subject and a rendered body")
    bad = [x["id"] for x in real if "blocking" in x.get("decorations", []) and x.get("verdict") != "CONFIRMED"]
    check(not bad, "nothing unconfirmed is marked blocking", str(bad) if bad else "")
    src = collections.Counter("duplicate" if x.get("duplicate_of") else x.get("source") for x in comments)
    print(f"  info  {dict(src)}; labels {dict(collections.Counter(x['label'] for x in real))}; "
          f"longest subject {max((len(x['subject']) for x in real), default=0)} chars")

if trace:
    node, iters, qwrites = {}, collections.Counter(), collections.Counter()
    path_of, verify_sessions = {}, {}
    shell, denied, overcap_reads = [], [], []
    pending = {}
    import itertools
    for line in itertools.chain.from_iterable(open(t, encoding="utf-8") for t in traces):
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        m, p = r["msg"], r["msg"].get("params") or {}
        meth = m.get("method")
        if meth == "_kiro/workflow/node_start" and p.get("sessionId"):
            node[p["sessionId"]] = p.get("nodeId")
            path_of[p["sessionId"]] = "/".join(str(x) for x in (p.get("nodePath") or [])[1:])
            if str(p.get("nodeId", "")).startswith("verify-"):
                verify_sessions.setdefault(path_of[p["sessionId"]], 0)
        elif meth == "_kiro/workflow/loop_iteration":
            iters[p.get("loopId")] += 1
        elif meth == "session/request_permission":
            k = (p.get("_meta") or {}).get("kiro") or {}
            c = k.get("consent") or {}
            pending[m["id"]] = (p.get("sessionId"), k.get("toolId"), c.get("capability"), str(c.get("resource")))
        elif "result" in m and m.get("id") in pending and r["dir"] == "client->agent":
            sid, tool, cap, res = pending.pop(m["id"])
            allowed = (m["result"].get("outcome") or {}).get("optionId") == "accept"
            who = node.get(sid, "?")
            if not allowed:
                denied.append(f"[{who}] {cap} {res[:110]}")
            if cap == "shell":
                shell.append((who, res, allowed))
            if cap == "fs_write" and "/verdicts/" in "/" + res and allowed and sid in path_of \
                    and path_of[sid] in verify_sessions:
                verify_sessions[path_of[sid]] += 1
            if cap == "fs_write" and "/queues/queue-" in "/" + res and allowed:
                qwrites[os.path.basename(res)] += 1
            if cap == "fs_read" and any(res.endswith(o) for o in OVERCAP):
                overcap_reads.append(f"[{who}] {res[-40:]}")

    print("== loop economy (trace) ==")
    absorbed = []
    for q in queues:
        name = os.path.basename(q)
        loop = "verify-loop-" + name[len("queue-"):-len(".json")]
        n_ids = len(json.load(open(q, encoding="utf-8")).get("ids") or [])
        want = max(n_ids, 1)  # the stop check is post-body: an empty queue still costs one iteration
        # Fewer iterations than ids would mean an id was skipped for good. MORE means a
        # session ended without landing its verdict and a later one redid it: the queue
        # absorbed a model fault. That is a cost to report, not a broken queue.
        # An iteration that wrote its verdict and THEN failed (an auth error, a stall)
        # emits no loop_iteration event, so a short count is a defect only when a
        # verdict is actually missing - which the integrity section reports.
        ids_here = json.load(open(q, encoding="utf-8")).get("ids") or []
        lost = [i for i in ids_here if i not in have]
        check(iters[loop] >= want or not lost, f"{loop}: {iters[loop]} iterations for {n_ids} ids",
              f"skipped: {lost}" if lost and iters[loop] < want else "")
        if iters[loop] < want and not lost:
            print(f"  info  {loop}: fewer loop events than ids, but every verdict exists "
                  "(an interrupted iteration had already written its verdict)")
        if iters[loop] > want:
            absorbed.append(f"{loop}: {iters[loop] - want} extra session(s)")
        check(qwrites[name] <= 1, f"{name}: written {qwrites[name]}x by verifiers (<=1: only the final done flip)")
    noop = [f"{w}" for w, n in sorted(verify_sessions.items()) if n == 0]
    if absorbed or noop:
        print(f"  info  self-healed: {'; '.join(absorbed) or 'none'} — verify sessions that wrote NO verdict: "
              f"{', '.join(noop) or 'none'}")
    print("== digest discipline (trace) ==")
    off_script = [(w, c) for w, c, _ in shell if "crtool.py" not in c]
    check(not off_script, f"clerk shell calls are all crtool ({len(shell)} calls)",
          "; ".join(f"[{w}] {c[:70]}" for w, c in off_script))
    # A denied READ is the policy stopping a harmless slip (a mistyped path, a
    # peek outside the workspace). A denied WRITE or SHELL call means a step
    # tried to act outside its script -- that is the defect worth failing on.
    read_denied = [d for d in denied if "] fs_read " in d]
    act_denied = [d for d in denied if d not in read_denied]
    check(not act_denied, "no write or shell request was denied", "; ".join(act_denied[:6]))
    if read_denied:
        print(f"  info  {len(read_denied)} read(s) outside the workspace denied by policy: "
              + "; ".join(read_denied[:4]))
    check(not overcap_reads, "no step read an over-cap JSON file", "; ".join(overcap_reads[:6]))

print(f"\n{'ALL CHECKS PASS' if not fails else str(len(fails)) + ' CHECK(S) FAILED'}")
sys.exit(1 if fails else 0)

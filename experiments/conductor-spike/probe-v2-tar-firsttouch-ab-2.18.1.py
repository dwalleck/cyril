#!/usr/bin/env python3
"""LIVE A/B part 3: FIRST-TOUCH dangerous tar on a fresh untrusted session — PAID (3 turns/arm).

Parts 1-2 showed the dangerous_options table changes NOTHING once trust exists
(session allow_always) or under --trust-all-tools — dangerous tar runs freely
on both 2.18.0 and 2.18.1. Remaining cell: what does a permission request look
like FIRST-TOUCH for a dangerous command, per version? Candidate "block"
shapes: reduced options (allow_always withheld), a different title/annotation,
or an outright tool failure with no permission request at all.

  T1  `tar --to-command=cat -xf probe.tar`         new-in-2.18.1 flag, real
      extraction mode (the flag actually executes `cat` here)
  T2  `tar --use-compress-program=cat -tf probe.tar` the other new flag
  T3  `tar --checkpoint-action=echo=hi -tf probe.tar` in-version control:
      dangerous on BOTH versions

All answered allow_once; every permission request's full option list and every
tool_call status/content is captured.

    probe-v2-tar-firsttouch-ab-2.18.1.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tarfile, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

CWD = tempfile.mkdtemp(prefix="v2tarft-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
with open(os.path.join(CWD, "hello.txt"), "w") as fh:
    fh.write("hello from the cyril tar probe\n")
with tarfile.open(os.path.join(CWD, "probe.tar"), "w") as t:
    t.add(os.path.join(CWD, "hello.txt"), arcname="hello.txt")
os.remove(os.path.join(CWD, "hello.txt"))   # so -x genuinely extracts

env = dict(os.environ)
env["HOME"] = tempfile.mkdtemp(prefix="v2tarfthome-")
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp"], cwd=CWD, env=env, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
PERMS = []
EVENTS = []


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def pump(until, to=120, tag=""):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=2)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        OUT.write(raw + "\n")
        OUT.flush()
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        if m and rid is None and m.endswith("session/update"):
            u = pr.get("update") or {}
            su = u.get("sessionUpdate")
            if su in ("tool_call", "tool_call_update"):
                snips = []
                for c in u.get("content") or []:
                    t = ((c.get("content") or {}).get("text")
                         if isinstance(c.get("content"), dict) else None)
                    if t:
                        snips.append(t[:120])
                EVENTS.append((tag, su, u.get("title") or "", u.get("status") or "",
                               " | ".join(snips)))
        if rid is not None and m:
            if m == "session/request_permission":
                opts = pr.get("options", [])
                PERMS.append((tag, ((pr.get("toolCall") or {}).get("title") or ""),
                              [dict(x) for x in opts]))
                pick = next((x for x in opts if (x.get("optionId") or "") == "allow_once"),
                            next((x for x in opts
                                  if "allow" in ((x.get("kind") or "") + (x.get("optionId") or "")).lower()),
                                 opts[0] if opts else None))
                res = ({"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                       if pick else {"outcome": {"outcome": "cancelled"}})
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
            else:
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}) + "\n")
            p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 60)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 5)

for tag, cmd in [("T1-tocommand-x", "tar --to-command=cat -xf probe.tar"),
                 ("T2-compressprog", "tar --use-compress-program=cat -tf probe.tar"),
                 ("T3-checkpoint-ctl", "tar --checkpoint-action=echo=hi -tf probe.tar")]:
    n_perms = len(PERMS)
    print(f"--- {tag}: {cmd}")
    rid = req("session/prompt",
              {"sessionId": sid,
               "prompt": [{"type": "text",
                           "text": "Using execute_bash, run this exact shell command in the "
                                   f"current directory and show me its output: {cmd}"}]})
    r = pump(rid, 300, tag=tag)
    got = PERMS[n_perms:]
    print(f"    stopReason={((r or {}).get('result') or {}).get('stopReason')!r} "
          f"permission_requests={len(got)}")
    for _, title, opts in got:
        print(f"      perm title={title!r}")
        print(f"      options={json.dumps(opts)}")
    pump(-1, 6, tag=f"{tag}post")

print("\n=== tool events ===")
for tag, kind, title, status, snip in EVENTS:
    print(f"  [{tag}] {kind} {title!r} {status} :: {snip!r}")

OUT.write(json.dumps({"_perms": PERMS, "_events": EVENTS}) + "\n")
OUT.close()
p.stdin.close()
p.terminate()

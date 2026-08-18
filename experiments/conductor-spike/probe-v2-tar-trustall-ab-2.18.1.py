#!/usr/bin/env python3
"""LIVE A/B part 2: tar dangerous flags under --trust-all-tools — PAID (2 turns/arm).

Part 1 (probe-v2-tar-dangerous-ab-2.18.1.py) showed session allow_always
bypasses the dangerous_options table IDENTICALLY on 2.18.0 and 2.18.1 — even
for --checkpoint-action, dangerous on both. So the changelog's "Block" must
bite where the dangerous table is the ONLY remaining gate: --trust-all-tools.

  T1  `tar -tf probe.tar`                  control: expect silent auto-run on
      BOTH versions (proves trust-all is active, no permission request).
  T2  `tar --to-command=cat -tf probe.tar` discriminator: 2.18.0 = not in the
      table, should auto-run silently; 2.18.1 = dangerous, expect a permission
      request DESPITE trust-all, or a failed/blocked tool call.

Captures permission requests, tool_call statuses AND tool output content so a
block that manifests as an error result is visible.

    probe-v2-tar-trustall-ab-2.18.1.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tarfile, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

CWD = tempfile.mkdtemp(prefix="v2tartrust-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
with open(os.path.join(CWD, "hello.txt"), "w") as fh:
    fh.write("hello from the cyril tar probe\n")
with tarfile.open(os.path.join(CWD, "probe.tar"), "w") as t:
    t.add(os.path.join(CWD, "hello.txt"), arcname="hello.txt")

env = dict(os.environ)
env["HOME"] = tempfile.mkdtemp(prefix="v2tartrusthome-")
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--trust-all-tools"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
PERMS = []
EVENTS = []   # (tag, kind, title, status, content-snippet)


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
                        snips.append(t[:160])
                EVENTS.append((tag, su, u.get("title") or "", u.get("status") or "",
                               " | ".join(snips)))
            if su == "agent_message_chunk":
                EVENTS.append((tag, su, "", "",
                               ((u.get("content") or {}).get("text") or "")[:160]))
        if rid is not None and m:
            if m == "session/request_permission":
                opts = pr.get("options", [])
                ids = [x.get("optionId") for x in opts]
                pick = next((x for x in opts
                             if "allow" in ((x.get("kind") or "") + (x.get("optionId") or "")).lower()),
                            opts[0] if opts else None)
                PERMS.append((tag, ((pr.get("toolCall") or {}).get("title") or ""), ids))
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

for tag, cmd in [("T1-control", "tar -tf probe.tar"),
                 ("T2-tocommand", "tar --to-command=cat -tf probe.tar")]:
    n_perms = len(PERMS)
    print(f"--- {tag}: {cmd}")
    rid = req("session/prompt",
              {"sessionId": sid,
               "prompt": [{"type": "text",
                           "text": "Using execute_bash, run this exact shell command in the "
                                   f"current directory and show me its output: {cmd}"}]})
    r = pump(rid, 300, tag=tag)
    print(f"    stopReason={((r or {}).get('result') or {}).get('stopReason')!r} "
          f"permission_requests={len(PERMS[n_perms:])}")
    for _, title, ids in PERMS[n_perms:]:
        print(f"      perm title={title!r} options={ids}")
    pump(-1, 6, tag=f"{tag}post")

print("\n=== events ===")
for tag, kind, title, status, snip in EVENTS:
    print(f"  [{tag}] {kind} {title!r} {status} :: {snip!r}")

OUT.write(json.dumps({"_perms": PERMS, "_events": EVENTS}) + "\n")
OUT.close()
p.stdin.close()
p.terminate()

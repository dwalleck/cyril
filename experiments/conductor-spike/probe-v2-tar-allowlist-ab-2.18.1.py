#!/usr/bin/env python3
"""LIVE A/B part 4: dangerous tar vs agent-config allowedCommands — PAID (2 turns/arm).

THE cyril-relevant lane: cyril's trust-persistence adapter (kiro_agent_config.rs)
writes `toolsSettings.execute_bash.allowedCommands` patterns on AllowAlways.
If 2.18.1's dangerous_options veto bites HERE, a pattern like "tar( .*)?"
granted through cyril stops covering `--to-command`/`--use-compress-program`
on 2.18.1 — the changelog's "Block".

Setup: isolated HOME with agent JSON granting allowedCommands ["tar( .*)?"],
spawn `acp --agent tartest`.
  T1  `tar -tf probe.tar`                    control: allowlist active -> no
      prompt expected on BOTH versions.
  T2  `tar --to-command=cat -xf probe.tar`   discriminator: 2.18.0 = allowlist
      match, silent; 2.18.1 = dangerous veto -> permission request (or block).

    probe-v2-tar-allowlist-ab-2.18.1.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tarfile, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

CWD = tempfile.mkdtemp(prefix="v2tarallow-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
with open(os.path.join(CWD, "hello.txt"), "w") as fh:
    fh.write("hello from the cyril tar probe\n")
with tarfile.open(os.path.join(CWD, "probe.tar"), "w") as t:
    t.add(os.path.join(CWD, "hello.txt"), arcname="hello.txt")
os.remove(os.path.join(CWD, "hello.txt"))

HOME = tempfile.mkdtemp(prefix="v2tarallowhome-")
os.makedirs(os.path.join(HOME, ".kiro", "agents"), exist_ok=True)
with open(os.path.join(HOME, ".kiro", "agents", "tartest.json"), "w") as fh:
    json.dump({
        "name": "tartest",
        "description": "tar allowlist probe agent",
        "tools": ["execute_bash"],
        "toolsSettings": {
            "execute_bash": {"allowedCommands": ["tar( .*)?"]}
        },
    }, fh, indent=2)

env = dict(os.environ)
env["HOME"] = HOME
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent", "tartest"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
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
                EVENTS.append((tag, su, u.get("title") or "", u.get("status") or ""))
        if rid is not None and m:
            if m == "session/request_permission":
                opts = pr.get("options", [])
                PERMS.append((tag, ((pr.get("toolCall") or {}).get("title") or ""),
                              [x.get("optionId") for x in opts]))
                pick = next((x for x in opts if (x.get("optionId") or "") == "allow_once"),
                            opts[0] if opts else None)
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
                 ("T2-tocommand-x", "tar --to-command=cat -xf probe.tar")]:
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
    for _, title, ids in got:
        print(f"      perm title={title!r} options={ids}")
    pump(-1, 6, tag=f"{tag}post")

print("\n=== tool events ===")
for e in EVENTS:
    print(f"  {e}")

OUT.write(json.dumps({"_perms": PERMS, "_events": EVENTS}) + "\n")
OUT.close()
p.stdin.close()
p.terminate()

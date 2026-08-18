#!/usr/bin/env python3
"""LIVE A/B: tar dangerous-flag block (2.18.1 changelog security item) — PAID (4 turns/arm).

2.18.1 adds `--use-compress-program` and `--to-command` to the embedded
shell-permission `dangerous_options.tar` list (static diff of the JSON config
at ~5.6MB in kiro-cli-chat; 2.18.0 had only `--checkpoint-action`). Question
for the wire: what does "dangerous" DO to the ACP permission flow — and does
2.18.1 actually change behavior for the two new flags?

Design (4 turns, one session):
  T1  `tar -tf probe.tar`                        -> expect permission request
      (tar is not in safe_commands); answer ALLOW_ALWAYS to establish trust.
  T2  `tar -tf probe.tar` again                  -> calibrates allow_always
      scope: no prompt expected if trust sticks.
  T3  `tar --to-command=cat -tf probe.tar`       -> THE DISCRIMINATOR:
      2.18.0 = flag not dangerous, trusted tar should run without a prompt;
      2.18.1 = dangerous, expect the prompt to REAPPEAR despite trust (or an
      outright rejection).
  T4  `tar --checkpoint-action=echo=hi -tf probe.tar` -> IN-VERSION CONTROL:
      already dangerous on BOTH versions; shows what dangerous-under-trust
      looks like on each binary, so T3's delta is attributable.

Permission answers: T1 -> allow_always; later turns -> allow_once (capture is
the signal; approving lets the turn complete cleanly).

HOME-isolated per feedback_isolate_kiro_probes_with_home (real XDG_DATA_HOME
keeps v2 auth reachable).

    probe-v2-tar-dangerous-ab-2.18.1.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tarfile, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

CWD = tempfile.mkdtemp(prefix="v2tarab-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
with open(os.path.join(CWD, "hello.txt"), "w") as fh:
    fh.write("hello from the cyril tar probe\n")
with tarfile.open(os.path.join(CWD, "probe.tar"), "w") as t:
    t.add(os.path.join(CWD, "hello.txt"), arcname="hello.txt")

env = dict(os.environ)
env["HOME"] = tempfile.mkdtemp(prefix="v2tarabhome-")
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp"], cwd=CWD, env=env, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
PERMS = []          # (turn_tag, toolcall_title, [optionIds], picked)
TOOL_STATUS = []    # (turn_tag, title, status) from tool_call/tool_call_update
MODE = {"pick": "always"}   # how to answer permission requests this turn


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
                TOOL_STATUS.append((tag, u.get("title") or "", u.get("status") or ""))
        if rid is not None and m:
            if m == "session/request_permission":
                opts = pr.get("options", [])
                ids = [x.get("optionId") for x in opts]
                want = MODE["pick"]
                pick = (next((x for x in opts if "always" in (x.get("optionId") or "").lower()), None)
                        if want == "always" else None)
                if pick is None:
                    pick = next((x for x in opts
                                 if "allow" in ((x.get("kind") or "") + (x.get("optionId") or "")).lower()),
                                opts[0] if opts else None)
                PERMS.append((tag, ((pr.get("toolCall") or {}).get("title") or ""), ids,
                              (pick or {}).get("optionId")))
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

TURNS = [
    ("T1-baseline", "always", "tar -tf probe.tar"),
    ("T2-trusted",  "once",   "tar -tf probe.tar"),
    ("T3-tocommand", "once",  "tar --to-command=cat -tf probe.tar"),
    ("T4-checkpoint", "once", "tar --checkpoint-action=echo=hi -tf probe.tar"),
]
for tag, pick, cmd in TURNS:
    MODE["pick"] = pick
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
    for _, title, ids, picked in got:
        print(f"      perm title={title!r} options={ids} picked={picked}")
    pump(-1, 6, tag=f"{tag}post")

print("\n=== permission requests (all) ===")
for tag, title, ids, picked in PERMS:
    print(f"  [{tag}] {title!r} options={ids} picked={picked}")
print("\n=== tool_call statuses ===")
seen = set()
for tag, title, status in TOOL_STATUS:
    k = (tag, title, status)
    if k in seen:
        continue
    seen.add(k)
    print(f"  [{tag}] {title!r} status={status}")

OUT.write(json.dumps({"_perms": PERMS, "_tool_status": TOOL_STATUS}) + "\n")
OUT.close()
p.stdin.close()
p.terminate()

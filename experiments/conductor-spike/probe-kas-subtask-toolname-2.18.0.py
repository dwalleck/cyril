#!/usr/bin/env python3
"""Where does 2.18.0 put the RUNNING TOOL NAME on a live sub-agent row?

2.18.0 changelog: "[V3] Sub-agent rows name the tool each running agent is
executing, inline and in the ctrl+g monitor." The plain kas-turn subtask leg
(character counting) never made the child run a tool, so whatever field carries
the name was absent. This probe forces it: the sub-agent must READ A FILE, so
while the parent's agent-subtask tool_call row is in_progress the child is
executing fs_read — if the running-tool name is wire-visible, it must appear in
frames captured during that window.

Dumps EVERY agent-subtask-tagged frame verbatim plus any frame mentioning
fs_read/fsRead, and diffs the field-key set of agent-subtask frames against
what 2.17.0 emitted (kas-turn-2.17.0.jsonl had rawInput{name,prompt,
explanation,contextFiles} + _meta.kiro.{kind,agentSubtaskId} only).

ONE paid turn. HOME-isolated per feedback_isolate_kiro_probes_with_home.

    probe-kas-subtask-toolname-2.18.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

CWD = tempfile.mkdtemp(prefix="kas-subtool-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
pathlib.Path(CWD, "probe.txt").write_text("The magic number is 7311.\n")

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}


def scrub(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else scrub(obj[k]))
                for k in obj}
    if isinstance(obj, list):
        return [scrub(x) for x in obj]
    return obj


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute(
            "select value from auth_kv where key in "
            "('kirocli:odic:token','kirocli:social:token') order by key desc"
        ).fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    if row is None:
        raise SystemExit("logged out — no token")
    v = row[0]
    v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    parn = d.get("profile_arn")
    if not parn and prow:
        pv = prow[0]
        pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try:
            parn = json.loads(pv).get("arn")
        except Exception:
            pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": parn}


TOK = read_token()
env = dict(os.environ)
env["HOME"] = tempfile.mkdtemp(prefix="kas-subtoolhome-")
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
SUBTASK_FRAMES = []
TOOLISH = []


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def abspath(pth):
    return pth if os.path.isabs(pth or "") else os.path.join(CWD, pth or "")


def pump(until, to=420, tag=""):
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
        OUT.write(json.dumps(scrub(o)) + "\n")
        OUT.flush()
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        blob = json.dumps(o)
        if '"agent-subtask"' in blob:
            SUBTASK_FRAMES.append(scrub(o))
        elif "fs_read" in blob or "fsRead" in blob or "fs/read" in blob:
            TOOLISH.append((m or "response", blob[:160]))
        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                res = TOK
            elif m in ("fs/read_text_file", "_kiro/fs/read_file"):
                try:
                    res = {"content": pathlib.Path(abspath(pr.get("path"))).read_text()}
                except Exception as e:
                    res = {"content": f"(err {e})"}
            elif m == "session/request_permission":
                opts = pr.get("options", [])
                pick = next((x for x in opts
                             if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                res = ({"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                       if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                res = {}
            p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
            p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                           "terminal": True},
    "_meta": {"kiro": {"clientName": "cyril-audit"}},
})
pump(1, 40)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 120)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 8)

rid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text":
    "Use a subagent to read the file probe.txt in the current directory and "
    "report the magic number it contains. Do not read the file yourself — "
    "delegate the read to the subagent."}]})
r = pump(rid, 420, tag="turn")
print("stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 10, tag="post")

print(f"\n=== agent-subtask frames ({len(SUBTASK_FRAMES)}) ===")
KEYSETS = set()
for o in SUBTASK_FRAMES:
    u = ((o.get("params") or {}).get("update") or {})
    meta = ((u.get("_meta") or {}).get("kiro") or {})
    KEYSETS.add((tuple(sorted(u)), tuple(sorted(meta))))
    print(json.dumps(u)[:500])
    print("---")
print("\n=== distinct (update-keys, _meta.kiro-keys) sets ===")
for uk, mk in sorted(KEYSETS):
    print(f"  update{list(uk)} meta.kiro{list(mk)}")
print(f"\n=== non-subtask frames mentioning fs_read ({len(TOOLISH)}) ===")
for m, b in TOOLISH[:12]:
    print(f"  [{m}] {b}")

OUT.close()
p.stdin.close()
p.terminate()

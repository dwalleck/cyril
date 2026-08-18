#!/usr/bin/env python3
"""KAS surface + minimal turn for the 2.18.1 audit — mostly free, ONE paid mini-turn.

The KAS bundle is byte-identical 2.18.0→2.18.1 (@kiro/agent 0.38.7 both), so this
probe targets the BACKEND axis only:
  - initialize result (capabilities / extensionMethods / agentInfo) vs the
    kas-turn-2.18.0.jsonl capture
  - session/new configOptions — is `model` present (transiently absent at
    2.17.0), how many models, effort metadata intact?
  - ONE mini-turn ("Reply with exactly OK") to live-confirm turn-end ordering
    (session_info_update turn_end BEFORE the session/prompt response — the
    cyril-14ou stall-model invariant) against today's backend.

HOME-isolated per feedback_isolate_kiro_probes_with_home; own process group so
killpg reaps the node grandchild.

    probe-kas-surface-2.18.1.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, signal, sqlite3, subprocess, sys, tempfile, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")


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


home = tempfile.mkdtemp(prefix="kas-surf-")
env = os.environ.copy()
env["HOME"] = home
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
cwd = tempfile.mkdtemp(prefix="kas-surf-cwd-")

proc = subprocess.Popen(
    [KIRO, "acp", "--agent-engine", "kas"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    env=env, start_new_session=True,
)
nid = [0]
ORDER = []   # ordered markers for the turn-end ordering check


def send(obj):
    proc.stdin.write((json.dumps(obj) + "\n").encode())
    proc.stdin.flush()


def request(method, params):
    nid[0] += 1
    send({"jsonrpc": "2.0", "id": nid[0], "method": method, "params": params})
    return nid[0]


def pump(until, to=90):
    end = time.time() + to
    while time.time() < end:
        line = proc.stdout.readline()
        if not line:
            return None
        try:
            m = json.loads(line)
        except json.JSONDecodeError:
            continue
        OUT.write(line.decode() if isinstance(line, bytes) else line)
        OUT.flush()
        meth, rid = m.get("method"), m.get("id")
        if meth == "_kiro/auth/getAccessToken":
            send({"jsonrpc": "2.0", "id": rid, "result": read_token()})
            continue
        if meth and rid is not None:   # other host callbacks: empty-ok
            send({"jsonrpc": "2.0", "id": rid, "result": {}})
            continue
        if meth == "session/update":
            u = (m.get("params") or {}).get("update") or {}
            if u.get("sessionUpdate") == "session_info_update":
                ORDER.append(f"info:{u.get('kind')}")
        if rid == until and ("result" in m or "error" in m):
            ORDER.append(f"response:{until}")
            return m
    return None


iid = request("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
init = pump(iid, 60)
res = (init or {}).get("result") or {}
caps = res.get("agentCapabilities") or {}
meta = res.get("_meta") or {}
print("agentInfo:", json.dumps(res.get("agentInfo") or res.get("serverInfo") or {}))
print("capability keys:", sorted(caps))
ext = (meta.get("kiro") or {}).get("extensionMethods") or meta.get("extensionMethods") or []
print(f"extensionMethods ({len(ext)}):", ext)

nid2 = request("session/new", {"cwd": cwd, "mcpServers": []})
sess = pump(nid2, 90)
sres = (sess or {}).get("result") or {}
sid = sres.get("sessionId")
co = sres.get("configOptions") or []
print("sessionId:", sid)
print("configOptions:", [o.get("id") for o in co])
for o in co:
    if o.get("id") == "model":
        names = [x.get("name") for x in o.get("options", [])]
        effort = [x for x in o.get("options", []) if x.get("hasEffort") or x.get("effortLevels")]
        print(f"    model options ({len(names)}): {names[:10]}...")
        print(f"    options with effort metadata: {len(effort)}")

ORDER.clear()
tid = request("session/prompt",
              {"sessionId": sid, "prompt": [{"type": "text", "text": "Reply with exactly: OK"}]})
r = pump(tid, 240)
print("turn stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
print("ordering markers:", ORDER)
te = [i for i, x in enumerate(ORDER) if x == "info:turn_end"]
rp = [i for i, x in enumerate(ORDER) if x.startswith("response:")]
if te and rp:
    print("TURN-END-BEFORE-RESPONSE:", te[0] < rp[0])

OUT.close()
try:
    os.killpg(proc.pid, signal.SIGKILL)
except ProcessLookupError:
    pass

#!/usr/bin/env python3
"""Free same-day A/B: does KAS `session/new` still serve the `model` configOption?

Context (2.17.0 audit): the live turn-traffic leg on 2.17.0 returned configOptions
WITHOUT the `model` entry (2.16.2 capture: mode+model+autopilot+contentCollection;
2.17.0: mode+autopilot+contentCollection). acp-server.js is byte-identical across
both CLI versions, so the delta is either a backend rollout (time axis) or the
backend gating on the reported CLI version (binary axis via the wrapper's origin
version). This probe needs NO paid turn — configOptions arrives on session/new —
and runs the SAME handshake against both wrapper binaries same-day to decide.

Usage: probe-kas-configoptions-ab-2.17.0.py <kiro-cli-chat> [label]
HOME-isolated per feedback_isolate_kiro_probes_with_home.
"""
import json, os, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
LABEL = sys.argv[2] if len(sys.argv) > 2 else KIRO
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


home = tempfile.mkdtemp(prefix="kas-cfgopt-")
env = os.environ.copy()
env["HOME"] = home
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
cwd = tempfile.mkdtemp(prefix="kas-cfgopt-cwd-")

proc = subprocess.Popen(
    [KIRO, "acp", "--agent-engine", "kas"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    env=env,
    # Own process group: killing only the wrapper orphans the node grandchild
    # (acp-server.js survives stdin EOF — see reference_kiro_kas_turn_stall /
    # cyril-14ou). killpg at exit reaps the whole tree.
    start_new_session=True,
)
nid = [0]


def send(obj):
    proc.stdin.write((json.dumps(obj) + "\n").encode())
    proc.stdin.flush()


def request(method, params):
    nid[0] += 1
    send({"jsonrpc": "2.0", "id": nid[0], "method": method, "params": params})
    return nid[0]


deadline = time.time() + 60
init_id = request("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
new_id = None
result = None
while time.time() < deadline and result is None:
    line = proc.stdout.readline()
    if not line:
        break
    try:
        m = json.loads(line)
    except json.JSONDecodeError:
        continue
    if m.get("method") == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": m["id"], "result": read_token()})
    elif m.get("id") == init_id and "result" in m:
        new_id = request("session/new", {"cwd": cwd, "mcpServers": []})
    elif new_id is not None and m.get("id") == new_id and "result" in m:
        result = m["result"]

if result is None:
    print(f"{LABEL}: NO session/new result within 60s")
else:
    co = result.get("configOptions") or []
    print(f"{LABEL}: configOptions = {[o.get('id') for o in co]}")
    for o in co:
        if o.get("id") == "model":
            names = [x.get("name") for x in o.get("options", [])]
            print(f"    model options ({len(names)}): {names[:8]}...")
import signal
try:
    os.killpg(proc.pid, signal.SIGKILL)
except ProcessLookupError:
    pass

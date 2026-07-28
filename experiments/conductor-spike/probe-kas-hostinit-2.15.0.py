#!/usr/bin/env python3
"""KAS host-init leg capture for the 2.15.0 audit (@kiro/agent 0.25.17).

initialize + session/new ONLY — no prompt turn, so zero model call, zero
credits, zero content collection. Captures advertised capabilities,
extensionMethods, authMethods, sessionCapabilities, modes, configOptions, and
any unsolicited _kiro/* pushes at session start. Raw JSONL out for diffing
against the 2.14.1 (0.22.7) baseline.

Launched on cyril's real path: kiro-cli-chat acp --agent-engine kas.
Token self-sourced from data.sqlite3 (profileArn required — 2.10.0 gotcha).
HOME-isolated per feedback_isolate_kiro_probes_with_home (protect ~/.kiro/logs).

    probe-kas-hostinit-2.15.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
REAL_HOME = os.path.expanduser("~")


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
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    parn = d.get("profile_arn")
    if not parn and prow:
        pv = prow[0]; pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try: parn = json.loads(pv).get("arn")
        except Exception: pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": parn}


TOK = read_token()
CWD = tempfile.mkdtemp(prefix="kas-hostinit-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-home-")
env = dict(os.environ)
env["HOME"] = TMPH
env["XDG_DATA_HOME"] = os.path.join(REAL_HOME, ".local", "share")

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    p.stdin.flush()


def handle_serverreq(o):
    # answer host->client callbacks so nothing blocks; log them
    m = o.get("method"); rid = o.get("id")
    if m == "_kiro/auth/getAccessToken":
        rep(rid, TOK)
    else:
        rep(rid, {})


def pump(until, to=30):
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
        if o.get("method") and o.get("id") is not None:
            handle_serverreq(o)
        if o.get("id") == until and ("result" in o or "error" in o):
            return o
    return None


CLIENT_CAPS = {
    "fs": {"readTextFile": True, "writeTextFile": True},
    "terminal": True,
    "_kiro": {"clientName": "kiro-cli"},
}
initr = req("initialize", {"protocolVersion": 1, "clientCapabilities": CLIENT_CAPS})
r = pump(initr, 25)
print("INIT result keys:", list((r or {}).get("result", {}).keys()))
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
r2 = pump(nid, 30)
print("SESSION/NEW result keys:", list((r2 or {}).get("result", {}).keys()))
pump(-1, 5)  # drain unsolicited pushes
OUT.close()
p.stdin.close()
p.terminate()

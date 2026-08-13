#!/usr/bin/env python3
"""Does 2.18.0 KAS load NESTED AGENTS.md as steering, and is it wire-visible?

2.18.0 changelog: "[V3] Nested `AGENTS.md` files across the workspace tree are
loaded as steering context" (acp-server.js AGENTS.md refs 22→33). Prior state:
AGENTS.md loaded only from the workspace ROOT and ~/.kiro/steering/.

Setup: workspace with
  AGENTS.md                       (root — control, loaded on 2.17.0 too)
  src/deep/AGENTS.md              (nested — the 2.18.0 behavior under test)
Each file carries a distinctive magic token. One paid turn asks the agent to
repeat any magic tokens its steering mentions; the model answer is the
BEHAVIORAL check, while every wire frame is scanned for the nested file's PATH
(steering lists ride session_info_update / context payloads on KAS) — that's
the WIRE-VISIBILITY check that matters for cyril's context display.

HOME-isolated per feedback_isolate_kiro_probes_with_home.

    probe-kas-nested-agentsmd-2.18.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

CWD = tempfile.mkdtemp(prefix="kas-agentsmd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
pathlib.Path(CWD, "AGENTS.md").write_text(
    "# Steering\nThe root magic token is ROOT-STEER-77.\n")
deep = pathlib.Path(CWD, "src", "deep")
deep.mkdir(parents=True)
(deep / "AGENTS.md").write_text(
    "# Deep steering\nThe nested magic token is NESTED-STEER-42.\n")
(deep / "widget.py").write_text("x = 1\n")

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
env["HOME"] = tempfile.mkdtemp(prefix="kas-agentsmdhome-")
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
AGENT_TEXT = []
WIRE_HITS = []          # (frame_key, which_token/path matched)


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def pump(until, to=180, tag=""):
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
        for needle in ("src/deep/AGENTS.md", "NESTED-STEER-42", "ROOT-STEER-77", "AGENTS.md"):
            if needle in blob:
                key = m or ("response" if "result" in o else "?")
                if m and m.endswith("session/update"):
                    u = pr.get("update") or {}
                    key = f"{m}::{u.get('sessionUpdate')}"
                WIRE_HITS.append((tag, key, needle))
        if m and rid is None and m.endswith("session/update"):
            u = pr.get("update") or {}
            if u.get("sessionUpdate") == "agent_message_chunk":
                AGENT_TEXT.append((u.get("content") or {}).get("text", ""))
        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": TOK}) + "\n")
            elif m == "session/request_permission":
                opts = pr.get("options", [])
                pick = next((x for x in opts
                             if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
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


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                           "terminal": True},
    "_meta": {"kiro": {"clientName": "cyril-audit"}},
})
pump(1, 40)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 120, tag="new")
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 8, tag="settle")

rid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text":
    "Without using any tools: do your steering or context instructions mention any "
    "'magic token' values? If so repeat each token string exactly. If not, say NONE."}]})
r = pump(rid, 300, tag="turn")
print("stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 8, tag="post")

text = "".join(AGENT_TEXT)
print("\n=== agent answer ===")
print(text[:500])
print("\n=== behavioral verdict ===")
print("  root token seen:  ", "ROOT-STEER-77" in text)
print("  nested token seen:", "NESTED-STEER-42" in text)
print("\n=== wire visibility (frames mentioning steering paths/tokens) ===")
seen = {}
for tag, key, needle in WIRE_HITS:
    seen.setdefault((key, needle), 0)
    seen[(key, needle)] += 1
for (key, needle), c in sorted(seen.items()):
    print(f"  {c:3}x {key} :: {needle}")

OUT.close()
p.stdin.close()
p.terminate()

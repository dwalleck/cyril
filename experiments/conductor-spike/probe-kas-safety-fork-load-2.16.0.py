#!/usr/bin/env python3
"""KAS capability + session-lifecycle probe: _kiro/safety/*, session/fork, session/load.

Three gaps the 2.16.0 audit left open, all cheap (one small turn total):

B. `_kiro/safety/*` (cyril-3ald) — Infrastructure Safety is ENFORCEMENT: it can
   BLOCK a tool call. cyril has never seen a frame because the surface is gated on
   the client advertising the capability, and no probe ever did:
       capabilitiesFrom(kiroMeta): infrastructureSafety: kiroMeta.infrastructureSafety === true
   i.e. initialize.clientCapabilities._meta.kiro.infrastructureSafety. This probe
   advertises it (and c2sViews, same gate family), then watches for
   `_kiro/safety/{statusChanged,propertiesChanged}` pushes and calls
   `_kiro/safety/getProperties` directly.

D. `session/fork` + `session/load` (cyril-99ds) — 2.16.0 advertises the new
   `replayMarking: true` capability, whose contract is that replayed session/load
   updates carry `_meta.kiro.replay: true` so a client can separate replay from live
   updates interleaving during the load. Never observed. Also captures whether a
   fork carrying a user-chosen title sets `titleSetByUser` (the /tangent mechanism).

Costs: ONE short turn, so the loaded session has history worth replaying.

    probe-kas-safety-fork-load-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}


def redact(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else redact(obj[k]))
                for k in obj}
    if isinstance(obj, list):
        return [redact(x) for x in obj]
    return obj


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key in "
                        "('kirocli:odic:token','kirocli:social:token') order by key desc").fetchone()
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
CWD = tempfile.mkdtemp(prefix="kas-sfl-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-sfl-home-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
SAFETY = []
REPLAY = {"marked": 0, "unmarked": 0}
PHASE = ["init"]


def emit(d, e, m, parsed):
    OUT.write(json.dumps({"direction": d, "envelope": e, "method": m,
                          "parsed": redact(parsed)}) + "\n")
    OUT.flush()


def send(obj, method=None, envelope="request"):
    p.stdin.write(json.dumps(obj) + "\n")
    p.stdin.flush()
    emit("client_to_agent", envelope, method, obj)


def req(m, pr):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}, method=m)
    return i[0]


def reply(rid, res):
    send({"jsonrpc": "2.0", "id": rid, "result": res}, envelope="response")


def pump(until, to=120):
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
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        emit("agent_to_client",
             "notification" if (m and rid is None) else ("request" if m else "response"), m, o)
        if m and m.startswith("_kiro/safety/"):
            SAFETY.append((PHASE[0], m, pr))
            print(f"   <- SAFETY {m}: {json.dumps(pr)[:220]}")
        if PHASE[0] == "load" and m == "session/update":
            mk = ((pr.get("update") or {}).get("_meta") or {}).get("kiro") or {}
            REPLAY["marked" if mk.get("replay") is True else "unmarked"] += 1
        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                reply(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                reply(rid, {"shellType": "bash"})
            elif m == "session/request_permission":
                opts = pr.get("options", [])
                pick = next((x for x in opts if "allow" in
                             (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                      if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                reply(rid, {})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


# --- B: advertise the gated capabilities -------------------------------------
req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {
        "fs": {"readTextFile": True, "writeTextFile": True},
        "terminal": True,
        "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True,
                           "infrastructureSafety": True, "c2sViews": True}},
    },
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True,
                       "infrastructureSafety": True, "c2sViews": True}},
})
init = pump(1, 40)
caps = ((init or {}).get("result") or {}).get("agentCapabilities", {})
print("agentCapabilities._meta.kiro:",
      json.dumps((caps.get("_meta") or {}).get("kiro", {}))[:400])

nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 90)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 8)

print("\n=== B: _kiro/safety/getProperties ===")
r = pump(req("_kiro/safety/getProperties", {"sessionId": sid}), 45)
print("  ", json.dumps(r)[:700])

# --- one small turn so the session has history worth replaying ---------------
print("\n=== one short turn (for load/replay history) ===")
PHASE[0] = "turn"
r = pump(req("session/prompt", {"sessionId": sid,
                                "prompt": [{"type": "text", "text": "Reply with exactly: ping"}]}), 300)
print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 6)

# --- D: session/list, fork (tangent), load (replay) --------------------------
print("\n=== D: session/list ===")
r = pump(req("session/list", {}), 45)
res = (r or {}).get("result") or {}
sessions = res.get("sessions") or res.get("items") or []
print(f"   {len(sessions)} session(s); keys={sorted(sessions[0])[:12] if sessions else '-'}")

print("\n=== D: session/fork with a user-chosen title (the /tangent mechanism) ===")
PHASE[0] = "fork"
r = pump(req("session/fork", {"sessionId": sid, "cwd": CWD,
                              "title": "cyril audit tangent",
                              "_meta": {"kiro": {"createdReason": "tangent"}}}), 90)
print("  ", json.dumps(r)[:700])
forked = ((r or {}).get("result") or {}).get("sessionId")
pump(-1, 6)

print("\n=== D: session/load (replayMarking contract) ===")
PHASE[0] = "load"
r = pump(req("session/load", {"sessionId": sid, "cwd": CWD, "mcpServers": []}), 120)
print("  result:", json.dumps(r)[:400])
pump(-1, 10)
print(f"  replayed session/update frames: marked _meta.kiro.replay=true "
      f"-> {REPLAY['marked']} | unmarked -> {REPLAY['unmarked']}")

print("\n=== SUMMARY ===")
print(f"  safety frames seen: {len(SAFETY)}")
for ph, m, pr in SAFETY[:6]:
    print(f"    [{ph}] {m}: {json.dumps(pr)[:200]}")
print(f"  forked sessionId: {forked}")
print(f"  replay marking: {REPLAY}")

OUT.close()
p.stdin.close()
p.terminate()

#!/usr/bin/env python3
"""Provoke the agent->client half of the KAS surface — the responders cyril must implement.

The RPC sweep covered the 47 client->agent methods. The other 22 are PUSHED by the
agent, so they cannot be called — they have to be provoked, and every one of them is
something cyril has to ANSWER rather than invoke. Nothing in this tree has ever
systematically provoked them.

Two levers, both discovered in the bundle:

1. CAPABILITIES. resolveCapabilities() (src/platform/resolved-capabilities.ts) is the
   whole gate list, and the fs family is nested under `fs._meta.kiro`, NOT top-level
   `_meta.kiro` — which is why the RPC sweep's guess failed to move KAS off the ACP
   `fs/read_text_file` dialect:
       fs.readTextFile / fs.writeTextFile
       fs._meta.kiro.{readFile, writeFile, stat, readDirectory, delete}
       terminal
       _meta.kiro.{secretStorage, openExternalUrl, knowledge, infrastructureSafety,
                   c2sViews}
   This advertises ALL of them, so any surface gated on a capability can fire.

2. WORKSPACE STATE. A `.kiro/hooks/` config gives the hooks family something to
   trigger on; hooks/{list,sessionStart} are asked of the CLIENT at trigger points.

Turns are chosen to provoke distinct pushes:
  1. tool use          -> _kiro/fs/* dialect + pre-tool-use hooks/list
  2. ambiguous choice  -> _kiro/userInput (cyril-qo13: cyril can only return the
                          FIRST option today, so the real shape matters)
  3. external link     -> _kiro/openExternalUrl

Every agent->client frame is recorded with its direction and answered generically, so
nothing deadlocks and the shapes are captured even for methods we do not yet model.

Costs credits (three short turns).

    probe-kas-pushed-methods-2.16.0.py <kiro-cli-chat> <out.jsonl>
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
CWD = tempfile.mkdtemp(prefix="kas-push-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
pathlib.Path(CWD, "probe.txt").write_text("magic 4242\n")
# Give the hooks family something to trigger on.
hd = pathlib.Path(CWD, ".kiro", "hooks")
hd.mkdir(parents=True, exist_ok=True)
(hd / "audit.json").write_text(json.dumps({
    "name": "cyril-audit-hook",
    "trigger": "preToolUse",
    "command": "echo cyril-audit-hook-fired",
}, indent=2))
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)

TMPH = tempfile.mkdtemp(prefix="kas-push-home-")
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
TERMS = {}
PUSHED = {}      # method -> [params, ...] (first few)


def note(m, pr, kindtag):
    d = PUSHED.setdefault(m, {"kind": kindtag, "samples": []})
    if len(d["samples"]) < 2:
        d["samples"].append(pr)


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


def ap(x):
    return x if os.path.isabs(x or "") else os.path.join(CWD, x or "")


def answer(m, rid, pr):
    """Answer every agent->client request plausibly so nothing deadlocks."""
    if m == "_kiro/auth/getAccessToken":
        return reply(rid, TOK)
    if m == "_kiro/terminal/shell_type":
        return reply(rid, {"shellType": "bash"})
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try:
            return reply(rid, {"content": pathlib.Path(ap(pr.get("path"))).read_text()})
        except Exception as e:
            return reply(rid, {"content": f"(err {e})"})
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        try:
            f = pathlib.Path(ap(pr.get("path")))
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(pr.get("content", ""))
        except Exception:
            pass
        return reply(rid, {})
    if m == "_kiro/fs/stat":
        f = pathlib.Path(ap(pr.get("path")))
        return reply(rid, {"type": "directory" if f.is_dir() else "file",
                           "size": f.stat().st_size} if f.exists() else {})
    if m == "_kiro/fs/read_directory":
        try:
            return reply(rid, {"entries": [
                {"name": e.name, "type": "directory" if e.is_dir() else "file"}
                for e in pathlib.Path(ap(pr.get("path"))).iterdir()]})
        except Exception:
            return reply(rid, {"entries": []})
    if m == "_kiro/hooks/list":
        # answer with the workspace hook so executeHook has something to run
        return reply(rid, {"hooks": [{"id": "cyril-audit-hook",
                                      "name": "cyril-audit-hook",
                                      "command": "echo cyril-audit-hook-fired",
                                      "trigger": pr.get("trigger")}]})
    if m == "_kiro/hooks/sessionStart":
        return reply(rid, {"hooks": [], "results": []})
    if m == "_kiro/hooks/executeHook":
        return reply(rid, {"exitCode": 0, "stdout": "cyril-audit-hook-fired\n", "stderr": ""})
    if m == "_kiro/userInput":
        opts = pr.get("options") or pr.get("choices") or []
        # deliberately pick the LAST option — cyril-qo13 is that cyril can only
        # return the FIRST; picking the last proves the wire carries the choice.
        pick = opts[-1] if opts else None
        if isinstance(pick, dict):
            pick = pick.get("optionId") or pick.get("id") or pick.get("value")
        return reply(rid, {"optionId": pick} if pick else {"cancelled": True})
    if m == "_kiro/openExternalUrl":
        return reply(rid, {"opened": True})
    if m in ("_kiro/secret/get",):
        return reply(rid, {"value": None})
    if m in ("_kiro/secret/store", "_kiro/secret/delete"):
        return reply(rid, {})
    if m == "terminal/create":
        cmd, args = pr.get("command", ""), pr.get("args") or []
        tid = f"term-{len(TERMS)+1}"
        try:
            r = (subprocess.run([cmd, *args], cwd=pr.get("cwd") or CWD, capture_output=True,
                                text=True, timeout=60) if args else
                 subprocess.run(cmd, shell=True, cwd=pr.get("cwd") or CWD,
                                capture_output=True, text=True, timeout=60))
            TERMS[tid] = {"out": r.stdout + r.stderr, "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 127}
        return reply(rid, {"terminalId": tid})
    if m == "terminal/output":
        t = TERMS.get(pr.get("terminalId"), {"out": "", "code": 0})
        return reply(rid, {"output": t["out"], "truncated": False,
                           "exitStatus": {"exitCode": t["code"], "signal": None}})
    if m == "terminal/wait_for_exit":
        t = TERMS.get(pr.get("terminalId"), {"code": 0})
        return reply(rid, {"exitCode": t["code"], "signal": None})
    if m in ("terminal/release", "terminal/kill"):
        return reply(rid, {})
    if m == "session/request_permission":
        opts = pr.get("options", [])
        pick = next((x for x in opts if "allow" in
                     (x.get("kind", "") + x.get("optionId", "")).lower()),
                    opts[0] if opts else None)
        return reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                     if pick else {"outcome": {"outcome": "cancelled"}})
    return reply(rid, {})


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
        if m and m.startswith(("_kiro/", "_session/")):
            note(m, pr, "REQUEST" if rid is not None else "notification")
        if rid is not None and m:
            answer(m, rid, pr)
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {
        "fs": {"readTextFile": True, "writeTextFile": True,
               # the _kiro/fs/* dialect gate — nested under fs, not top-level
               "_meta": {"kiro": {"readFile": True, "writeFile": True, "stat": True,
                                  "readDirectory": True, "delete": True}}},
        "terminal": True,
        "_meta": {"kiro": {
            "clientName": "cyril-audit", "checkpoints": True,
            "secretStorage": True, "openExternalUrl": True, "knowledge": True,
            "infrastructureSafety": True, "c2sViews": True,
            "hooks": True, "userInput": True,
        }},
    },
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True,
                       "hooks": True, "userInput": True}},
})
pump(1, 40)
sid = (pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 90) or {}) \
    .get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 8)

TURNS = [
    ("tooluse", "Read the file probe.txt in this directory and reply with just the number."),
    ("choice",  "I want to rename probe.txt, but I have not told you the new name. "
                "Ask me to choose between exactly these three names before doing anything: "
                "alpha.txt, beta.txt, gamma.txt. Use your user-input/question mechanism "
                "to present the choice rather than just writing the options as text."),
    ("link",    "Open the URL https://kiro.dev/docs in my browser using whatever "
                "open-external-url capability you have. Do not just print the link."),
]
for tag, text in TURNS:
    print(f"\n########## turn: {tag} ##########")
    r = pump(req("session/prompt", {"sessionId": sid,
                                    "prompt": [{"type": "text", "text": text}]}), 420)
    print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
    pump(-1, 10)

print("\n=== agent->client methods observed ===")
for m in sorted(PUSHED):
    d = PUSHED[m]
    print(f"\n  {d['kind']:12} {m}")
    for smp in d["samples"]:
        print(f"      {json.dumps(smp)[:320]}")

OUT.close()
p.stdin.close()
p.terminate()

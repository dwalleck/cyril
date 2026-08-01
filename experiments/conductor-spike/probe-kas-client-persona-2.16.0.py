#!/usr/bin/env python3
"""A/B the KAS client PERSONA — does clientInfo.name change the agent surface?

Every probe in this tree has sent `_meta.kiro.clientName` and no `clientInfo`.
The bundle shows that field is not read for persona resolution at all:

    this.clientInfo = params.clientInfo ?? void 0;
    if (this.clientInfo?.name && !CLIENT_TYPES.includes(this.clientInfo.name))
      logger.warn("Unrecognized clientInfo.name ... falling back to inferred client type");
    this.agentContext = resolveAgentContext(this.executionEnvironment, this.clientInfo?.name, {
      specLinks: kiroMeta?.specLinks,
      requirementsAnalysis: kiroMeta?.requirementsAnalysis,
      userInput: kiroMeta?.userInput,
    });

    function resolveAgentContext(env, clientName, capabilities) {
      if (clientName === "kiro-web" || clientName === "kiro-ide" || clientName === "kiro-cli")
        client = clientName;
      else if (env === "sandbox") client = "kiro-web";
      else client = "kiro-ide";              // <-- unrecognized / absent lands HERE
      ...
    }

So the standard ACP `initialize.params.clientInfo.name` is the switch, and every
capture we have was taken under the **kiro-ide** persona by fall-through. prompts.ts
branches on `agentContext.client === "kiro-ide" | "kiro-web"`, and honorsRepositories()
keys off it too — so the persona plausibly explains several surfaces that never
appeared for us (OrchestrateSubAgent, _kiro/userInput, _kiro/openExternalUrl).

This runs the handshake under four identities and diffs the resulting surface.
Handshake + session/new only, NO prompt turn — FREE.

Arms:
  control    no clientInfo at all      (what every prior probe did -> kiro-ide)
  kiro-cli   clientInfo.name=kiro-cli  (what cyril arguably should send)
  kiro-ide   clientInfo.name=kiro-ide  (explicit, should match control)
  kiro-web   clientInfo.name=kiro-web

All arms also advertise _meta.kiro.{specLinks, requirementsAnalysis, userInput}
— the three agentContext capability flags, none of which any probe has set — in
BOTH `_meta.kiro` and `clientCapabilities._meta.kiro`, since which one feeds
`kiroMeta` here is not certain from the bundle.

    probe-kas-client-persona-2.16.0.py <kiro-cli-chat> <out-prefix>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
PREFIX = sys.argv[2]
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
KIRO_META = {"checkpoints": True,
             "specLinks": True, "requirementsAnalysis": True, "userInput": True,
             "hooks": {"enabled": True, "v2": True},
             "infrastructureSafety": True, "openExternalUrl": True,
             "knowledge": True, "secretStorage": True, "c2sViews": True}


def run_arm(name, client_info):
    out = open(f"{PREFIX}-{name}.jsonl", "w")
    cwd = tempfile.mkdtemp(prefix=f"kas-persona-{name}-")
    subprocess.run("git init -q -b main", cwd=cwd, shell=True)
    tmph = tempfile.mkdtemp(prefix=f"kas-persona-{name}-home-")
    env = dict(os.environ)
    env["HOME"] = tmph
    env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=cwd, env=env,
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, bufsize=1)
    q = queue.Queue()
    threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                     daemon=True).start()
    i = [0]
    seen = {"tools": set(), "commands": set(), "pushes": set()}

    def send(o, m=None, e="request"):
        p.stdin.write(json.dumps(o) + "\n")
        p.stdin.flush()
        out.write(json.dumps({"direction": "client_to_agent", "envelope": e,
                              "method": m, "parsed": redact(o)}) + "\n")

    def req(m, pr):
        i[0] += 1
        send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}, m)
        return i[0]

    def pump(until, to=90):
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
            out.write(json.dumps({"direction": "agent_to_client",
                                  "envelope": "notification" if (m and rid is None)
                                  else ("request" if m else "response"),
                                  "method": m, "parsed": redact(o)}) + "\n")
            if m and rid is None and m.startswith(("_kiro/", "_session/")):
                seen["pushes"].add(m)
            if m == "_kiro/tools/didChange":
                for t in pr.get("tools") or []:
                    seen["tools"].add(t.get("name") or t.get("id") or str(t)[:30])
            u = pr.get("update") or {}
            if u.get("sessionUpdate") == "available_commands_update":
                for c in u.get("availableCommands") or []:
                    seen["commands"].add(c.get("name") or str(c)[:30])
            if rid is not None and m:
                res = TOK if m == "_kiro/auth/getAccessToken" else (
                    {"shellType": "bash"} if m == "_kiro/terminal/shell_type" else {})
                send({"jsonrpc": "2.0", "id": rid, "result": res}, e="response")
                continue
            if rid == until and ("result" in o or "error" in o):
                return o
        return None

    init_params = {
        "protocolVersion": 1,
        "clientCapabilities": {
            "fs": {"readTextFile": True, "writeTextFile": True,
                   "_meta": {"kiro": {"readFile": True, "writeFile": True, "stat": True,
                                      "readDirectory": True}}},
            "terminal": True,
            "_meta": {"kiro": dict(KIRO_META)},
        },
        "_meta": {"kiro": dict(KIRO_META)},
    }
    if client_info:
        init_params["clientInfo"] = client_info
    init = pump(req("initialize", init_params), 60)
    sess = pump(req("session/new", {"cwd": cwd, "mcpServers": []}), 90)
    pump(-1, 10)
    p.stdin.close()
    p.terminate()
    out.close()

    ir = (init or {}).get("result") or {}
    sr = (sess or {}).get("result") or {}
    return {
        "arm": name,
        "initKiroMeta": (ir.get("agentCapabilities", {}).get("_meta") or {}).get("kiro", {}),
        "authMethods": [a.get("id") for a in ir.get("authMethods") or []],
        "modes": [m.get("id") for m in (sr.get("modes") or {}).get("availableModes", [])],
        "configOptions": [c.get("id") for c in sr.get("configOptions") or []],
        "sessionMeta": sr.get("_meta") or {},
        "tools": sorted(seen["tools"]),
        "commands": sorted(seen["commands"]),
        "pushes": sorted(seen["pushes"]),
    }


ARMS = [
    ("control", None),
    ("kiro-cli", {"name": "kiro-cli", "version": "2.16.0"}),
    ("kiro-ide", {"name": "kiro-ide", "version": "2.16.0"}),
    ("kiro-web", {"name": "kiro-web", "version": "2.16.0"}),
]
results = {}
for nm, ci in ARMS:
    print(f"########## arm: {nm} ##########")
    r = run_arm(nm, ci)
    results[nm] = r
    print(f"  tools({len(r['tools'])}) commands({len(r['commands'])}) "
          f"modes({len(r['modes'])}) pushes({len(r['pushes'])})")

base = results["control"]
print("\n=== DIFF vs control (no clientInfo -> falls through to kiro-ide) ===")
for nm, r in results.items():
    if nm == "control":
        continue
    print(f"\n-- {nm} --")
    any_delta = False
    for key in ("tools", "commands", "modes", "configOptions", "pushes", "authMethods"):
        a, b = set(base[key]), set(r[key])
        if a != b:
            any_delta = True
            print(f"   {key}: +{sorted(b-a)}  -{sorted(a-b)}")
    if json.dumps(base["initKiroMeta"], sort_keys=True) != json.dumps(r["initKiroMeta"], sort_keys=True):
        any_delta = True
        print(f"   initKiroMeta differs:\n     control={json.dumps(base['initKiroMeta'])[:260]}"
              f"\n     {nm}={json.dumps(r['initKiroMeta'])[:260]}")
    if json.dumps(base["sessionMeta"], sort_keys=True) != json.dumps(r["sessionMeta"], sort_keys=True):
        any_delta = True
        ba, bb = base["sessionMeta"], r["sessionMeta"]
        keys = set(ba) | set(bb)
        print("   sessionMeta deltas: " + ", ".join(
            f"{k}: {ba.get(k)!r}->{bb.get(k)!r}" for k in sorted(keys) if ba.get(k) != bb.get(k)))
    if not any_delta:
        print("   (identical to control)")

print("\n=== control tool/command inventory (for reference) ===")
print("  tools   :", base["tools"])
print("  commands:", base["commands"])

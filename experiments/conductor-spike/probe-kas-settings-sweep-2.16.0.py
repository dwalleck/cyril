#!/usr/bin/env python3
"""Per-flag A/B of the whole KAS client-settings surface (21 keys).

Every large finding this audit made came from flipping a gate — workflows.enabled
unlocked the workflow engine, hooks{enabled,v2} unlocked the hooks subsystem,
fs._meta.kiro.* switched the fs dialect. All were found one at a time by luck.
This enumerates the gate surface instead.

THE MASTER LEVER, from the initialize handler: settings sent at INITIALIZE do not
merely configure the session — they OVERRIDE the backend model-config feature flags:

    if (kiroMeta?.settings) {
      const initParsed = parseSettings(kiroMeta.settings);
      const rawKeys = new Set(Object.keys(kiroMeta.settings));
      const bridgedIsFeatureEnabled = (feature) =>
        rawKeys.has(feature) ? isSettingEnabled(initSettings, feature)
                             : prev.isFeatureEnabled(feature);
      setModelConfigProvider(bridgeFeatureFlags(prev, bridgedIsFeatureEnabled));
    }

That matters because several surfaces are gated on
`getModelConfigProvider().isFeatureEnabled(...)` rather than on session settings —
notably `subagentOrchestration` (which selects OrchestrateSubAgent over
InvokeSubAgent in the system prompt) and `infraSafetyMonitor`/`infraSafetyEnforce`
(which gate the Infrastructure Safety enforcement path). The earlier
orchestrate probe sent settings at session/new ONLY and saw no effect; this sends
them at initialize, where the bridge lives.

21 keys from the settings schema, one arm each plus a baseline. Each arm diffs the
handshake surface (modes, configOptions, extensionMethods, advertised commands,
session-start pushes, session/new._meta feature flags) against baseline.

Handshake + session/new only — NO prompt turn. FREE.

    probe-kas-settings-sweep-2.16.0.py <kiro-cli-chat> <out-prefix>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
PREFIX = sys.argv[2]
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

FLAGS = ["checkpoint", "codeIntelligence", "compaction", "disableAutoCompaction",
         "fta", "goal", "infraSafetyEnforce", "infraSafetyMonitor", "inlineAgents",
         "knowledge", "largeToolOutputHandler", "semanticReview", "sessionEviction",
         "specPlan", "steeringSupervisor", "subagentOrchestration", "tangentMode",
         "thinking", "todoList", "toolSearch", "workflows"]

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}


def redact(o):
    if isinstance(o, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and o[k] else redact(o[k])) for k in o}
    if isinstance(o, list):
        return [redact(x) for x in o]
    return o


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


def run_arm(name, settings):
    out = open(f"{PREFIX}-{name}.jsonl", "w")
    cwd = tempfile.mkdtemp(prefix=f"kas-set-{name}-")
    subprocess.run("git init -q -b main", cwd=cwd, shell=True)
    tmph = tempfile.mkdtemp(prefix=f"kas-set-{name}-home-")
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
    seen = {"cmds": set(), "pushes": set(), "toolcalls": set()}

    def send(o, m=None, e="request"):
        p.stdin.write(json.dumps(o) + "\n")
        p.stdin.flush()
        out.write(json.dumps({"direction": "client_to_agent", "envelope": e,
                              "method": m, "parsed": redact(o)}) + "\n")

    def req(m, pr):
        i[0] += 1
        send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}, m)
        return i[0]

    def pump(until, to=75):
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
            u = pr.get("update") or {}
            if u.get("sessionUpdate") == "available_commands_update":
                for c in u.get("availableCommands") or []:
                    seen["cmds"].add(c.get("name") or str(c)[:30])
            if u.get("sessionUpdate") in ("tool_call", "tool_call_update") and u.get("title"):
                seen["toolcalls"].add(u["title"])
            if rid is not None and m:
                res = TOK if m == "_kiro/auth/getAccessToken" else (
                    {"shellType": "bash"} if m == "_kiro/terminal/shell_type" else {})
                send({"jsonrpc": "2.0", "id": rid, "result": res}, e="response")
                continue
            if rid == until and ("result" in o or "error" in o):
                return o
        return None

    kiro_meta = {"checkpoints": True}
    if settings:
        kiro_meta["settings"] = settings          # <-- the bridge lives here
    init = pump(req("initialize", {
        "protocolVersion": 1,
        "clientInfo": {"name": "kiro-cli", "version": "2.16.0"},
        "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                               "terminal": True,
                               "_meta": {"kiro": dict(kiro_meta)}},
        "_meta": {"kiro": dict(kiro_meta)},
    }), 60)
    # also pass at session/new, so a session-scoped-only gate still shows up
    sess = pump(req("session/new", {"cwd": cwd, "mcpServers": [],
                                    **({"_meta": {"kiro": {"settings": settings}}} if settings else {})}), 90)
    pump(-1, 10)
    p.stdin.close()
    p.terminate()
    out.close()
    ir = (init or {}).get("result") or {}
    sr = (sess or {}).get("result") or {}
    km = (ir.get("agentCapabilities", {}).get("_meta") or {}).get("kiro", {})
    return {
        "extensionMethods": sorted(km.get("extensionMethods") or []),
        "initCapKeys": sorted(k for k in km if k != "logging"),
        "modes": sorted(m.get("id") for m in (sr.get("modes") or {}).get("availableModes", [])),
        "configOptions": sorted(c.get("id") for c in sr.get("configOptions") or []),
        "sessionMetaFlags": {k: v for k, v in (sr.get("_meta") or {}).items()
                             if isinstance(v, (bool, str)) and k not in
                             ("id", "title", "createdAt", "lastModifiedAt", "schemaVersion")},
        "cmds": sorted(seen["cmds"]),
        "pushes": sorted(seen["pushes"]),
        "toolcalls": sorted(seen["toolcalls"]),
    }


print("########## baseline ##########")
base = run_arm("baseline", None)
print("  ", json.dumps(base["sessionMetaFlags"]))

deltas = {}
for f in FLAGS:
    r = run_arm(f, {f: {"enabled": True}})
    d = {}
    for k in base:
        if isinstance(base[k], list):
            a, b = set(base[k]), set(r[k])
            if a != b:
                d[k] = {"+": sorted(b - a), "-": sorted(a - b)}
        else:
            ba, bb = base[k], r[k]
            if ba != bb:
                d[k] = {kk: [ba.get(kk), bb.get(kk)] for kk in set(ba) | set(bb)
                        if ba.get(kk) != bb.get(kk)}
    deltas[f] = d
    print(f"  {'DELTA' if d else '  =  '}  {f}"
          + (f"   {json.dumps(d)[:190]}" if d else ""))

print("\n=== FLAGS THAT CHANGED THE SURFACE ===")
hit = {k: v for k, v in deltas.items() if v}
if not hit:
    print("  (none)")
for k, v in hit.items():
    print(f"\n-- {k} --")
    print("  ", json.dumps(v, indent=1)[:900])
print(f"\n{len(hit)}/{len(FLAGS)} flags produced a handshake-visible delta.")
print("NOTE: a flag with no handshake delta may still change in-turn behaviour "
      "(prompt content, tool selection); this sweep only sees the handshake.")

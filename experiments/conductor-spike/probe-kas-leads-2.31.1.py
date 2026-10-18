#!/usr/bin/env python3
"""Static-lane leads for the kiro-cli 2.31.1 live lane (KAS 0.66.33 vs 0.66.34).

  KAS=<acp-server.js> PROBE_SCRATCH=<dir> python3 -I probe-kas-leads-2.31.1.py <probe> <out.jsonl>

probes: kirohome | mcp | wfdefaults | memory | sandbox
Imports probe-kas-live-2.31.1.py (SCENARIO=lib) for Conn/auth plumbing. Every
handshake writes the FULL path initialize.clientCapabilities._meta.kiro.* (trap 5);
positive control = configurationState:true -> `_kiro/configuration/state` push
(recorded per arm as `cfgstate_push`). One process per arm, strictly serial.
"""
import importlib.util, json, os, shutil, subprocess, sys, tempfile, time
PROBE = sys.argv[1]
OUT = sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["SCENARIO"] = "lib"
os.environ.setdefault("LABEL", f"lead-{PROBE}")
sys.argv = [sys.argv[0], OUT]
spec = importlib.util.spec_from_file_location("kaslive", os.path.join(HERE, "probe-kas-live-2.31.1.py"))
K = importlib.util.module_from_spec(spec); spec.loader.exec_module(K)
Conn, emit, res_or_err, short, log = K.Conn, K.emit, K.res_or_err, K.short, K.log
SCRATCH = K.SCRATCH
MCP_SERVER = os.path.join(HERE, "audit-2.31.1", "live", "fixtures", "mcp_probe_server.py")
R = {"probe": "lead-result", "lead": PROBE, "kas_version": K.SUMMARY.get("kas_version"), "kas": K.KAS}

def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f: f.write(text)

def init(c, settings=None, caps_meta=None, tag="init"):
    c.tag = tag
    caps = {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}
    ckiro = {"configurationState": True}
    ckiro.update(caps_meta or {})
    if settings is not None: ckiro["settings"] = settings
    caps["_meta"] = {"kiro": ckiro}
    prm = {"protocolVersion": 1, "clientCapabilities": caps,
           "clientInfo": {"name": "cyril-probe", "version": "audit-2.31.1"}}
    return c.call("initialize", prm, 150)

def new(c, ws, settings=None, tag="session_new"):
    c.tag = tag
    prm = {"cwd": ws, "mcpServers": []}
    if settings is not None: prm["_meta"] = {"kiro": {"settings": settings}}
    r = c.call("session/new", prm, 180)
    return r, ((r or {}).get("result") or {}).get("sessionId")

def notes(c, method=None, since=0.0):
    with c.cv:
        return [(ts, m, p) for ts, m, p in c.notes if ts >= since and (method is None or m == method)]

def tree(root, maxn=400):
    out = []
    if not os.path.isdir(root): return None
    for d, dirs, fs in os.walk(root):
        dirs[:] = [x for x in dirs if x not in (".git", "node_modules")]
        for f in fs:
            out.append(os.path.relpath(os.path.join(d, f), root))
            if len(out) >= maxn: return sorted(out)
    return sorted(out)

def stderr_grep(c, words):
    p = c.stderr.name
    try: txt = open(p, encoding="utf-8", errors="replace").read()
    except OSError: return {}
    return {w: [l[:400] for l in txt.splitlines() if w in l][:6] for w in words}

def mk_ws(tag):
    ws = tempfile.mkdtemp(prefix=f"ws-{tag}-", dir=SCRATCH)
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=ws)
    write(os.path.join(ws, "PROBE.txt"), "ALPHA\n")
    return ws

def mcp_json(name, logf):
    return json.dumps({"mcpServers": {name: {"command": sys.executable, "args": ["-I", MCP_SERVER, name, logf]}}})

def seed_profile(root, tag, logdir):
    """Seed a distinguishable Kiro profile dir (agents, steering, settings/mcp.json, hooks)."""
    write(os.path.join(root, "agents", f"{tag}-agent.json"), json.dumps({
        "name": f"{tag}-agent", "description": f"probe agent seeded under {tag}",
        "prompt": "You are a probe agent. Reply tersely.", "tools": ["read_file"]}, indent=1))
    write(os.path.join(root, "steering", f"{tag}-steer.md"),
          f"---\ninclusion: always\n---\n# {tag} steering\nIf asked for the profile marker, reply {tag.upper()}MARK.\n")
    write(os.path.join(root, "settings", "mcp.json"), mcp_json(f"{tag}srv", os.path.join(logdir, f"{tag}srv.log")))
    write(os.path.join(root, "hooks", f"{tag}-hook.json"), json.dumps({
        "name": f"{tag}-hook", "trigger": "userPromptSubmit", "command": f"echo {tag}HOOK"}))

def turn_summary(c, sid, text, tag, timeout=240):
    t = time.time()
    tr = K.prompt(c, sid, text, tag, timeout)
    ups = [p.get("update") or {} for _, m, p in notes(c, "session/update", t) if p.get("sessionId") == sid]
    txt = "".join((u.get("content") or {}).get("text") or "" for u in ups if u.get("sessionUpdate") == "agent_message_chunk")
    tcs = [{"su": u.get("sessionUpdate"), "id": u.get("toolCallId"), "title": u.get("title"), "kind": u.get("kind"),
            "status": u.get("status"), "rawInput": u.get("rawInput"), "meta_kiro": (u.get("_meta") or {}).get("kiro")}
           for u in ups if u.get("sessionUpdate") in ("tool_call", "tool_call_update") and (u.get("title") or u.get("rawInput") is not None)]
    return {"turn": tr, "text": txt[:600], "tool_calls": tcs[:12]}

def arm_common(c, ws, sid, t0):
    st = notes(c, "_kiro/steering/documents_changed", t0)
    mcp = notes(c, "_kiro/mcp/status", t0)
    return {"cfgstate_push": len(notes(c, "_kiro/configuration/state", t0)),
            "steering_docs": sorted({(d.get("name") or d.get("id") or str(d)[:60]) for _, _, p in st for d in (p.get("documents") or [])}),
            "mcp_status": [p for _, _, p in mcp][-2:],
            "note_methods": sorted({m for _, m, _ in notes(c, None, t0)})}

# ---------------------------------------------------------------- lead 1 KIRO_HOME
def p_kirohome(D):
    arms = ["unset", "absolute", "relative", "root", "home"]
    only = os.environ.get("ARMS")
    if only: arms = [a for a in arms if a in only.split(",")]
    for arm in arms:
        base = tempfile.mkdtemp(prefix=f"kh-{arm}-", dir=SCRATCH)
        home = os.path.join(base, "home"); os.makedirs(home)
        ws = mk_ws(arm)
        logdir = os.path.join(base, "mcplogs"); os.makedirs(logdir)
        seed_profile(os.path.join(home, ".kiro"), "home", logdir)
        kh_val, kh_root = None, None
        if arm == "absolute":
            kh_root = os.path.join(base, "khabs"); kh_val = kh_root
            seed_profile(kh_root, "kh", logdir)
        elif arm == "relative":
            kh_val = "khrel"; kh_root = os.path.join(ws, "khrel")
            seed_profile(kh_root, "kh", logdir)
        elif arm == "root":
            kh_val = "/"
        elif arm == "home":
            kh_val = home; kh_root = home
            seed_profile(home, "kh", logdir)    # i.e. <HOME>/agents etc.
        env = {"KIRO_LOG_LEVEL": "debug"}
        if kh_val is not None: env["KIRO_HOME"] = kh_val
        before_home = tree(home); before_kh = tree(kh_root) if kh_root else None
        t0 = time.time()
        c = Conn(f"kh-{arm}", extra_env=env, home=home, cwd=ws)
        A = {"KIRO_HOME": kh_val, "home": home, "kh_root": kh_root}
        try:
            ini = init(c, tag=f"{arm}:init")
            ir = (ini or {}).get("result") or {}
            A["logDir"] = ((((ir.get("agentCapabilities") or {}).get("_meta") or {}).get("kiro") or {}).get("logging") or {}).get("logDir")
            r, sid = new(c, ws, tag=f"{arm}:new")
            res = (r or {}).get("result") or {}
            A["sid"] = sid
            A["modes"] = [m.get("id") for m in ((res.get("modes") or {}).get("availableModes") or [])]
            time.sleep(6)
            A.update(arm_common(c, ws, sid, t0))
            c.tag = f"{arm}:hooks"; A["hooks_list"] = short(res_or_err(c.call("_kiro/hooks/list", {"sessionId": sid}, 30)), 400)
            c.tag = f"{arm}:getState"; gs = res_or_err(c.call("_kiro/configuration/getState", {"observer": "session", "sessionId": sid}, 45))
            if isinstance(gs, dict):
                A["getState_items"] = sorted({(r_.get("type"), (it.get("value") or {}).get("name") or (it.get("value") or {}).get("id"))
                                              for r_ in gs.get("resources") or [] for it in r_.get("items") or []
                                              if any(x in json.dumps(it) for x in ("home-", "kh-", "homesrv", "khsrv"))})
            A["turn"] = turn_summary(c, sid, "Reply with exactly the word OK.", f"{arm}:turn", 180) if sid else None
            time.sleep(3)
            A["stderr"] = stderr_grep(c, ["userConfigDir", "KIRO_HOME", "kiro-home", "contains-home"])
        finally:
            c.close()
        A["after_home"] = [x for x in (tree(home) or []) if x not in (before_home or [])][:80]
        A["after_kh"] = [x for x in (tree(kh_root) or []) if x not in (before_kh or [])][:80] if kh_root else None
        A["root_kiro_dirs"] = {p: os.path.exists(p) for p in ("/sessions", "/logs", "/.kiro")}
        A["ws_khrel_exists"] = os.path.isdir(os.path.join(ws, "khrel"))
        A["mcp_server_logs"] = sorted(os.listdir(logdir))
        D[arm] = A
        log(arm, json.dumps({k: A[k] for k in ("logDir", "modes", "steering_docs", "mcp_server_logs")})[:600])

# ---------------------------------------------------------------- lead 3 MCP on-demand
MCPQ = ("Use the probe_echo tool from the probesrv MCP server to echo the text ZEBRA, then reply with exactly the tool's output.")
def p_mcp(D):
    arms = [("none", None), ("ts_disabled", {"toolSearch": {"enabled": False}}),
            ("ts_enabled", {"toolSearch": {"enabled": True}}),
            ("never_defer", {"toolSearch": {"neverDefer": ["probesrv::probe_echo", "probe_echo", "@probesrv/probe_echo"]}})]
    only = os.environ.get("ARMS")
    if only: arms = [a for a in arms if a[0] in only.split(",")]
    for name, settings in arms:
        ws = mk_ws(name)
        logf = os.path.join(SCRATCH, f"mcp-{name}-{int(time.time())}.log")
        write(os.path.join(ws, ".kiro", "settings", "mcp.json"), mcp_json("probesrv", logf))
        t0 = time.time()
        c = Conn(f"mcp-{name}", extra_env={"KIRO_LOG_LEVEL": "debug"}, cwd=ws)
        A = {"settings": settings}
        try:
            init(c, settings=settings, tag=f"{name}:init")
            r, sid = new(c, ws, tag=f"{name}:new")
            time.sleep(10)
            A.update(arm_common(c, ws, sid, t0))
            A["tools_didChange"] = [p for _, _, p in notes(c, "_kiro/tools/didChange", t0)][-1:]
            c.tag = f"{name}:mcpstatus"
            A["turn"] = turn_summary(c, sid, MCPQ, f"{name}:turn", 300)
            A["mcp_status_after"] = [p for _, _, p in notes(c, "_kiro/mcp/status", t0)][-1:]
            A["stderr"] = stderr_grep(c, ["defer", "tool_load", "toolSearch", "tool_search", "mcp-only", "all-tools", "intake.unknown"])
        finally:
            c.close()
        try: A["mcp_server_log_methods"] = [ (json.loads(l).get("in") or {}).get("method") for l in open(logf) if '"in"' in l][:30]
        except OSError: A["mcp_server_log_methods"] = None
        D[name] = A
        log(name, json.dumps(A["turn"]["tool_calls"])[:500])

# ---------------------------------------------------------------- lead 4 workflow launch defaults
def p_wfdefaults(D):
    ws = mk_ws("wfd")
    wfdir = os.path.join(ws, ".kiro", "workflows")
    write(os.path.join(wfdir, "one.workflow.json"), json.dumps({
        "name": "one", "description": "one step", "injectOriginalUserRequest": False,
        "steps": [{"type": "step", "id": "s", "agent": "wf-coder", "prompt": "Reply with exactly ONE and nothing else. Do not use tools."}]}))
    c = Conn("wfd", extra_env={"KIRO_LOG_LEVEL": "debug"}, cwd=ws)
    try:
        init(c, settings=K.SETTINGS_ON, tag="wfd:init")
        r, sid = new(c, ws, settings=K.SETTINGS_ON, tag="wfd:new")
        res = (r or {}).get("result") or {}
        models = [o.get("value") for co in res.get("configOptions") or [] if co.get("id") == "model" for o in co.get("options") or []]
        D["models"] = models; D["workflowsEnabled"] = (res.get("_meta") or {}).get("workflowsEnabled")
        base = {"parentSessionId": sid, "workspacePaths": [ws], "inputs": {}}
        for k, extra, path in [("nonexist_modelId_int", {"modelId": 5}, "/nonexistent/x.workflow.json"),
                               ("nonexist_control", {}, "/nonexistent/x.workflow.json"),
                               ("nonexist_effort_bad", {"effortLevel": 7}, "/nonexistent/x.workflow.json"),
                               ("nonexist_modelId_str", {"modelId": "claude-sonnet-5"}, "/nonexistent/x.workflow.json")]:
            c.tag = "wfd:" + k
            D[k] = short(res_or_err(c.call("_kiro/workflow/new", dict(base, workflowPath=path, **extra), 60)), 700)
        pick = next((m for m in models if m not in ("auto",) and "haiku" in m), None) or next((m for m in models if m != "auto"), None)
        D["picked_model"] = pick
        c.tag = "wfd:new_valid"
        t0 = time.time()
        rr = res_or_err(c.call("_kiro/workflow/new", dict(base, workflowPath=os.path.join(wfdir, "one.workflow.json"),
                                                          modelId=pick, effortLevel="low"), 60))
        D["new_valid"] = short(rr, 700)
        wid = rr.get("workflowId") if isinstance(rr, dict) else None
        if wid:
            c.tag = "wfd:invoke"; D["invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)))
            done = K.wait_run(c, wid, ("completed", "failed", "aborted"), 400, t0)
            D["terminal"] = done[2].get("status") if done else "TIMEOUT"
            c.tag = "wfd:inspect"; ins = res_or_err(c.call("_kiro/workflow/inspect", {"workflowId": wid}, 60))
            D["inspect"] = short(ins, 3000)
            evs = K.wf_events(c, wid)
            D["events"] = [m.split("/")[-1] for _, m, _ in evs]
            D["run_start"] = short(next((p for _, m, p in evs if m.endswith("run_start")), None), 1500)
            ns = next((p for _, m, p in evs if m.endswith("node_start") and p.get("sessionId")), None)
            D["node_start"] = short(ns, 800)
            if ns:
                ssid = ns["sessionId"]
                D["step_session_config_updates"] = [[(co.get("id"), co.get("currentValue")) for co in (p.get("update") or {}).get("configOptions") or []]
                    for _, m, p in notes(c, "session/update", t0) if p.get("sessionId") == ssid and (p.get("update") or {}).get("sessionUpdate") == "config_option_update"][-2:]
                D["step_session_meta_model"] = [((p.get("update") or {}).get("_meta") or {}).get("kiro", {}).get("modelId")
                    for _, m, p in notes(c, "session/update", t0) if p.get("sessionId") == ssid and "modelId" in json.dumps((p.get("update") or {}).get("_meta"))][:3]
            if done: D["wake"] = K.wait_wake(c, sid, done[0], 200)
        D["stderr"] = stderr_grep(c, ["launch_defaults", "modelId", "effortLevel"])
    finally:
        c.close()

# ---------------------------------------------------------------- lead 5 memory default
def p_memory(D):
    arms = [("none", None), ("explicit_rw", {"memory": {"mode": "read_write", "reflection": True}}),
            ("explicit_disabled", {"memory": {"mode": "disabled"}})]
    for name, settings in arms:
        ws = mk_ws("mem" + name)
        t0 = time.time()
        c = Conn(f"mem-{name}", extra_env={"KIRO_LOG_LEVEL": "debug"}, cwd=ws)
        try:
            init(c, settings=settings, tag=f"{name}:init")
            r, sid = new(c, ws, tag=f"{name}:new")
            res = (r or {}).get("result") or {}
            time.sleep(3)
            D[name] = {"sent": settings, "memoryConfig": (res.get("_meta") or {}).get("memoryConfig"),
                       "meta_keys": sorted((res.get("_meta") or {}).keys()),
                       "memoryReflection": [co.get("currentValue") for co in res.get("configOptions") or [] if co.get("id") == "memoryReflection"],
                       "memory_list": short(res_or_err(c.call("_kiro/memory/list", {}, 30)), 300),
                       "cfgstate_push": len(notes(c, "_kiro/configuration/state", t0)),
                       "stderr": stderr_grep(c, ["memory", "Memory"])}
        finally:
            c.close()

# ---------------------------------------------------------------- lead 6 sandbox
def p_sandbox(D):
    arms = [("control", {}, []), ("set_config_sandbox", {}, []),
            ("argv_auto", {}, ["--sandbox=auto", "--sandbox-network-mode=default_blocked"]),
            ("argv_auto_rollout_env", {"KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED": "1"}, ["--sandbox=auto", "--sandbox-network-mode=default_blocked"]),
            ("rollout_env_only", {"KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED": "1"}, [])]
    for name, env, argv in arms:
        ws = mk_ws("sb" + name)
        t0 = time.time()
        env = dict(env, KIRO_LOG_LEVEL="debug")
        c = Conn(f"sb-{name}", extra_env=env, argv_extra=argv, cwd=ws)
        A = {"env": env, "argv": argv}
        try:
            ini = init(c, tag=f"{name}:init")
            A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 400)
            r, sid = new(c, ws, tag=f"{name}:new")
            res = (r or {}).get("result") or {}
            A["configOptions"] = [(co.get("id"), co.get("currentValue"), [o.get("value") for o in co.get("options") or []][:6]) for co in res.get("configOptions") or []]
            time.sleep(3)
            if name == "set_config_sandbox":
                for cid, val in (("sandbox", "enabled"), ("mcpSandboxing", "enabled")):
                    c.tag = f"{name}:set_{cid}"
                    A[f"set_{cid}"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": cid, "value": val}, 30)), 500)
                time.sleep(3)
            A["sandbox_status"] = [p for _, _, p in notes(c, "_kiro/sandbox/status", t0)][:4]
            A["note_methods"] = sorted({m for _, m, _ in notes(c, None, t0)})
            A["stderr"] = stderr_grep(c, ["sandbox", "Sandbox", "bubblewrap", "bwrap"])
        finally:
            c.close()
        D[name] = A
        log(name, json.dumps({k: A.get(k) for k in ("init", "sandbox_status")})[:400])

PROBES = {"kirohome": p_kirohome, "mcp": p_mcp, "wfdefaults": p_wfdefaults, "memory": p_memory, "sandbox": p_sandbox}
t_start = time.time()
try:
    PROBES[PROBE](R)
finally:
    R["elapsed_s"] = round(time.time() - t_start, 1)
    R["auth_events"] = K.AUTH_EVENTS
    emit(R)
    print(json.dumps(K.scrub(R), indent=1)[:15000])

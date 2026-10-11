#!/usr/bin/env python3
"""kas-core leads (L1-L8) for kiro-cli 2.29.1 live lane.

  KAS=<acp-server.js> PROBE_SCRATCH=<dir> probe-kas-leads-2.29.1.py <probe|all> <out.jsonl>

Every handshake writes the FULL path initialize.clientCapabilities._meta.kiro.*
(trap 5) and names a positive control. One probe per process.
"""
import importlib.util, json, os, select, socket, subprocess, sys, threading, time, base64
PROBE = sys.argv[1]
OUT = sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["SCENARIO"] = "lib"
os.environ.setdefault("LABEL", f"lead-{PROBE}")
sys.argv = [sys.argv[0], OUT]
spec = importlib.util.spec_from_file_location("kaslive", os.path.join(HERE, "probe-kas-live-2.29.1.py"))
K = importlib.util.module_from_spec(spec); spec.loader.exec_module(K)
Conn, emit, res_or_err, short, log = K.Conn, K.emit, K.res_or_err, K.short, K.log
WS, FAKE_HOME = K.WS, K.FAKE_HOME
R = {"probe": "lead-result", "lead": PROBE, "kas_version": K.SUMMARY.get("kas_version")}

# T3: allow a probe to leave a `session/request_permission` UNANSWERED (the driver's `_answer`
# always replies). Patched here, not in the shared driver, so the running workflow legs keep the
# module they loaded.
_original_answer = K.Conn._answer
def _answer_hold(self, o):
    if o.get("method") == "session/request_permission" and getattr(self, "perms_hold", False):
        emit({"probe": "perm_held", "id": o.get("id"),
              "toolCallId": ((o.get("params") or {}).get("toolCall") or {}).get("toolCallId")})
        return
    _original_answer(self, o)
K.Conn._answer = _answer_hold

def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f: f.write(text)

def git_ws():
    write(os.path.join(WS, "PROBE.txt"), "ALPHA\n")
    if not os.path.isdir(os.path.join(WS, ".git")):
        subprocess.run(["git", "init", "-q", "-b", "main"], cwd=WS)

def init(c, settings=None, caps_meta=None, kiro_meta=None, tag="init", name="cyril-probe"):
    """settings -> clientCapabilities._meta.kiro.settings (trap 5). caps_meta -> other
    clientCapabilities._meta.kiro keys (e.g. configurationState, inactiveItems)."""
    c.tag = tag
    caps = {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}
    ckiro = dict(caps_meta or {})
    if settings is not None: ckiro["settings"] = settings
    if ckiro: caps["_meta"] = {"kiro": ckiro}
    prm = {"protocolVersion": 1, "clientCapabilities": caps,
           "clientInfo": {"name": name, "version": "audit-2.29.1"}}
    if kiro_meta: prm["_meta"] = {"kiro": kiro_meta}
    return c.call("initialize", prm, 150)

def new(c, settings=None, tag="session_new"):
    c.tag = tag
    prm = {"cwd": WS, "mcpServers": []}
    if settings is not None: prm["_meta"] = {"kiro": {"settings": settings}}
    r = c.call("session/new", prm, 180)
    return r, ((r or {}).get("result") or {}).get("sessionId")

def notes(c, method=None, kind=None, since=0.0):
    with c.cv:
        out = []
        for ts, m, p in c.notes:
            if ts < since or (method and m != method): continue
            if kind and K.kiro_kind(p) != kind: continue
            out.append((ts, p))
        return out

def call(c, tag, m, prm, to=60):
    c.tag = tag
    return res_or_err(c.call(m, prm, to))

def tools_turn(c, sid, tag, q=None, timeout=300):
    t = time.time()
    turn = K.prompt(c, sid, q or K_TOOLS_Q, tag, timeout)
    ups = [p["update"] for _, p in notes(c, "session/update", since=t) if p.get("sessionId") == sid]
    names = []
    for u in ups:
        if u.get("sessionUpdate") == "agent_message_chunk":
            names.append((u.get("content") or {}).get("text") or "")
    txt = "".join(names)
    nb = sorted({x.strip().strip("`*").strip() for x in txt.replace("\n", ",").split(",") if x.strip()})
    return {"turn": turn, "text": txt[:500], "tool_names": nb}

K_TOOLS_Q = ("List the exact names (ids) of every tool available to you, comma-separated, nothing else. Do not call any tool.")

def log_dirs():
    d = os.path.join(FAKE_HOME, ".kiro", "logs")
    return set(os.listdir(d)) if os.path.isdir(d) else set()

def log_scan(before, words, maxlen=400):
    d = os.path.join(FAKE_HOME, ".kiro", "logs")
    out = {"dirs": sorted(log_dirs() - before), "counts": {w: 0 for w in words}, "samples": {}}
    for sub in out["dirs"]:
        for root, _, fs in os.walk(os.path.join(d, sub)):
            for f in fs:
                try: txt = open(os.path.join(root, f), encoding="utf-8", errors="replace").read()
                except OSError: continue
                for w in words:
                    out["counts"][w] += txt.count(w)
                    if w not in out["samples"] and w in txt:
                        i = txt.find(w); out["samples"][w] = txt[max(0, i-60):i+maxlen]
    return out

# ---------------------------------------------------------------- L7 settings intake
def p_intake(D):
    git_ws()
    arms = [("unknown_cyril_like", {"subagentOrchestration": {"enabled": True}, "todoList": {"enabled": True}}),
            ("control_known", {"thinking": {"enabled": True}})]
    for name, settings in arms:
        before = log_dirs()
        c = Conn(name)
        try:
            init(c, settings=settings, caps_meta={"configurationState": True}, tag=name + ":init")
            r, sid = new(c, tag=name + ":new")
            time.sleep(4)
            D[name] = {"sent_settings": settings, "init_ok": True, "sid": bool(sid)}
        finally:
            c.close()
        D[name]["log"] = log_scan(before, ("configuration.intake.unknown-key", "unknown-key", "intake",
                                            "subagentOrchestration", "todoList"))
        log(name, D[name]["log"]["counts"])

# ---------------------------------------------------------------- L2 configuration getState
def p_getstate(D):
    git_ws()
    arms = [("G0_no_configstate", {}, None),
            ("G1_configstate", {"configurationState": True}, None),
            ("G2_configstate_inactive", {"configurationState": True, "inactiveItems": True}, None)]
    for name, caps_meta, settings in arms:
        c = Conn(name)
        try:
            init(c, settings=settings, caps_meta=caps_meta, tag=name + ":init")
            time.sleep(3)
            after_init = notes(c, "_kiro/configuration/state")
            t = time.time()
            r, sid = new(c, tag=name + ":new")
            time.sleep(6)
            pulls = {}
            c.tag = name + ":pull_conn"
            pulls["observer_connection"] = short(res_or_err(c.call("_kiro/configuration/getState", {"observer": "connection"}, 45)), 6000)
            if sid:
                c.tag = name + ":pull_sess"
                pulls["observer_session"] = short(res_or_err(c.call("_kiro/configuration/getState",
                                                                    {"observer": "session", "sessionId": sid}, 45)), 6000)
                c.tag = name + ":pull_ws"
                pulls["observer_workspace"] = short(res_or_err(c.call("_kiro/configuration/getState",
                                                                      {"observer": "workspace", "roots": [WS]}, 45)), 3000)
            D[name] = {"caps_meta": caps_meta, "push_after_init": len(after_init),
                       "push_after_new": len(notes(c, "_kiro/configuration/state", since=t)),
                       "pulls": pulls, "sid": bool(sid)}
            log(name, "push_init", len(after_init), "pull_conn", str(pulls["observer_connection"])[:200])
        finally:
            c.close()

# ---------------------------------------------------------------- L1 model fallback
def p_fallback(D):
    git_ws()
    arms = [("F0_control_bogus", {}, "bogus-model-zz9"),
            ("F1_flags_bogus", {"KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED": "true",
                                "KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED": "true"}, "bogus-model-zz9"),
            ("F2_flags_auto", {"KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED": "true",
                               "KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED": "true"}, None)]
    for name, env_extra, model in arms:
        saved = {k: os.environ.get(k) for k in env_extra}
        os.environ.update(env_extra)
        c = Conn(name)
        for k, v in saved.items():
            if v is None: os.environ.pop(k, None)
            else: os.environ[k] = v
        try:
            init(c, caps_meta={"configurationState": True}, tag=name + ":init")
            r, sid = new(c, tag=name + ":new")
            if model:
                call(c, name + ":setmodel", "session/set_config_option",
                     {"sessionId": sid, "configId": "model", "value": model}, 30)
            t = time.time()
            turn = K.prompt(c, sid, "Reply with exactly OK.", name + ":turn", 300) if sid else None
            kinds = [K.kiro_kind(p) for _, p in notes(c, "session/update", since=t)
                     if p.get("sessionId") == sid]
            print("TURNDBG", turn, kinds)
            fb = [p for _, p in notes(c, "session/update", since=t)
                  if K.kiro_kind(p) == "model_fallback"]
            D[name] = {"env": env_extra, "model_set": model, "turn": turn,
                       "kinds": [k for k in kinds if k], "model_fallback_frames": fb,
                       "turn_start_control": kinds.count("turn_start"), "turn_end_control": kinds.count("turn_end")}
        finally:
            c.close()

# ---------------------------------------------------------------- L5 image placeholder
PNG = open(os.path.join(HERE, "valid.png.b64")).read().strip()
def p_image(D):
    git_ws()
    for name, model in [("text_only", "deepseek-3.2"), ("vision", "claude-sonnet-4.5")]:
        c = Conn(name)
        try:
            init(c, tag=name + ":init")
            r, sid = new(c, tag=name + ":new")
            if model:
                call(c, name + ":setmodel", "session/set_config_option",
                     {"sessionId": sid, "configId": "model", "value": model}, 30)
            c.tag = name + ":prompt"
            t = time.time()
            params = {"sessionId": sid, "prompt": [
                {"type": "text", "text": "What is in this image? If you cannot read it, say why."},
                {"type": "image", "mimeType": "image/png", "data": PNG}]}
            resp = c.call("session/prompt", params, 300)
            texts = []
            for _, p in notes(c, "session/update", since=t):
                if p.get("sessionId") != sid: continue
                u = p.get("update") or {}
                if u.get("sessionUpdate") == "agent_message_chunk":
                    texts.append({"text": (u.get("content") or {}).get("text"), "_meta": u.get("_meta")})
            D[name] = {"model": model, "stopReason": ((resp or {}).get("result") or {}).get("stopReason"),
                       "error": None if (resp and "result" in resp) else res_or_err(resp),
                       "assistant_text": texts,
                       "placeholder_seen": any("image omitted" in json.dumps(x) for x in texts),
                       "outcome_unchanged": any("unchanged" in json.dumps(x) for x in texts)}
        finally:
            c.close()

# ---------------------------------------------------------------- L6 cascade config precedence
def p_cascade(D):
    git_ws()
    cfg = json.dumps({"probeStartTurn": 1, "notifications": True})
    arms = [("default", {}), ("env_cascade", {"KIRO_FEATURE_CASCADE_CONFIG": cfg})]
    for name, env_extra in arms:
        saved = {k: os.environ.get(k) for k in env_extra}
        os.environ.update(env_extra)
        c = Conn(name)
        for k, v in saved.items():
            if v is None: os.environ.pop(k, None)
            else: os.environ[k] = v
        try:
            init(c, tag=name + ":init")
            r, sid = new(c, tag=name + ":new")
            K.prompt(c, sid, "Reply with exactly OK.", name + ":turn", 300) if sid else None
            time.sleep(3)
        finally:
            c.close()
        D[name] = {"env": env_extra, "log": log_scan(set(), ("handoff.config.resolved", "source\":\"env",
                                                              "source\":\"defaults", "cascade"))}
        log(name, D[name]["log"]["counts"], str(D[name]["log"]["samples"])[:300])

# ---------------------------------------------------------------- L8 MCP env inheritance
def p_mcpenv(D):
    git_ws()
    helper = os.path.join(WS, "mcp-env.py")
    write(helper, "#!/usr/bin/env python3\nimport os,sys,json\n"
                  "open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'mcp-env-seen.json'),'w').write("
                  "json.dumps({k:v for k,v in os.environ.items() if 'MARKER' in k or k in ('PATH','HOME','AWS_REGION')}))\n"
                  "for line in sys.stdin:\n"
                  "    try: o=json.loads(line)\n"
                  "    except Exception: continue\n"
                  "    if o.get('method')=='initialize':\n"
                  "        sys.stdout.write(json.dumps({'jsonrpc':'2.0','id':o['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{'tools':{}},'serverInfo':{'name':'env-echo','version':'1'}}})+'\\n'); sys.stdout.flush()\n")
    write(os.path.join(WS, ".kiro", "settings", "mcp.json"), json.dumps({
        "mcpServers": {"env-echo": {"command": sys.executable, "args": [helper]}}}))
    os.environ["MY_MARKER"] = "1"
    c = Conn("mcpenv")
    try:
        init(c, tag="mcpenv:init")
        r, sid = new(c, tag="mcpenv:new")
        time.sleep(12)
    finally:
        c.close()
    seen_path = os.path.join(WS, "mcp-env-seen.json")
    D["seen"] = json.load(open(seen_path)) if os.path.exists(seen_path) else None
    D["seen_file_exists"] = os.path.exists(seen_path)
    D["marker_inherited"] = bool(D["seen"] and "MY_MARKER" in D["seen"])

# ---------------------------------------------------------------- L3 multi-client
def p_multiclient(D):
    """L3 (multi-client ask fanout) is NOT PROVABLE with this harness.

    `acp-server.js` is spawned per Connection over its own stdio pipe, so two Conn objects are
    two independent KAS processes, not two clients of one server. Testing `resendOpenAsks` /
    `interaction_resolved` fanout needs two clients attached to ONE server (the ws/multiplex
    transport), which this lane does not drive. Recorded unproven; ZERO Kiro processes spawned.
    See FINDINGS 'Leads for the live lane' for the exact next probe."""
    D["status"] = "UNPROVEN"
    D["reason"] = ("stdio transport = one client per spawned process; no single-server multi-client "
                   "transport driven in this lane")
    D["next_probe"] = ("spawn one acp-server.js on a multiplex/ws transport (grep the bundle for "
                       "the transport flag), attach two clients, raise a tool ask on A, connect B "
                       "after the ask, assert B receives the open ask and both see interaction_resolved")

def p_commands(D):
    """Does NEW still advertise the bundled steering slash commands in available_commands_update?
    The sweep pair showed 5 OLD vs 2 NEW; this probe settles it with a longer settle and dumps
    every frame."""
    git_ws()
    c = Conn("commands")
    try:
        init(c, caps_meta={"configurationState": True}, tag="commands:init")
        r, sid = new(c, tag="commands:new")
        for wait in (8, 25, 25):
            time.sleep(wait)
            with c.cv:
                frames = [p for _, m, p in c.notes
                          if p.get("sessionId") == sid
                          and (p.get("update") or {}).get("sessionUpdate") == "available_commands_update"]
            D[f"frames_after_{wait}s"] = [[x.get("name") for x in (f.get("update") or {}).get("availableCommands") or []]
                                          for f in frames]
        with c.cv:
            steer = [p for _, m, p in c.notes if m == "_kiro/steering/documents_changed"]
        D["steering_notifs"] = len(steer)
        D["steering_docs"] = [[d.get("name"), d.get("type"), d.get("inclusion")] for d in (steer[-1].get("documents") or [])] if steer else []
        D["all_command_frames"] = [[x.get("name") for x in (f.get("update") or {}).get("availableCommands") or []]
                                   for f in [p for _, m, p in c.notes
                                             if p.get("sessionId") == sid
                                             and (p.get("update") or {}).get("sessionUpdate") == "available_commands_update"]]
    finally:
        c.close()

def p_ltoh(D):
    """CROSS-LANE CONFLICT: is `settings.largeToolOutputHandler` a key KAS knows, ignores, or logs
    as an unknown key? Scanned with KIRO_LOG_LEVEL=debug. Control: `thinking` (known)."""
    git_ws()
    for name, settings in [("ltoh_tui_key", {"largeToolOutputHandler": {"enabled": True}}),
                           ("control_thinking", {"thinking": {"enabled": True}})]:
        before = log_dirs()
        c = Conn(name)
        try:
            init(c, settings=settings, caps_meta={"configurationState": True}, tag=name + ":init")
            r, sid = new(c, tag=name + ":new")
            time.sleep(4)
            D[name] = {"sent": settings, "sid": bool(sid)}
        finally:
            c.close()
        D[name]["log"] = log_scan(before, ("largeToolOutputHandler", "unknown-key", "intake.unknown",
                                           "configuration.intake"))

def p_memory(D):
    """T1: settings.memory / memoryEnable / userMemoryOptIn -> session/new `_meta.memoryConfig` +
    configOption `memoryReflection`. No prompt. Control: arm (a) sends no memory key."""
    git_ws()
    arms = [("a_none", {}, {}),
            ("b_readonly", {"memory": {"mode": "read_only"}}, {}),
            ("c_disabled_optout", {"memory": {"mode": "disabled"}, "userMemoryOptIn": {"enabled": False}}, {}),
            ("d_memoryEnable", {"memoryEnable": {"enabled": True}}, {}),
            ("e_memoryEnable_internal", {"memoryEnable": {"enabled": True}}, {"KIRO_INTERNAL": "1"})]
    for name, settings, env_extra in arms:
        saved = {k: os.environ.get(k) for k in env_extra}
        os.environ.update(env_extra)
        c = Conn(name)
        for k, v in saved.items():
            if v is None: os.environ.pop(k, None)
            else: os.environ[k] = v
        try:
            init(c, settings=settings, tag=name + ":init")
            r, sid = new(c, tag=name + ":new")
            res = (r or {}).get("result") or {}
            meta = res.get("_meta") or {}
            co = [(o.get("id"), o.get("currentValue")) for o in res.get("configOptions") or []]
            D[name] = {"sent": settings, "env": env_extra, "sid": bool(sid),
                       "memoryConfig": meta.get("memoryConfig"),
                       "memoryConfig_kiro": (meta.get("kiro") or {}).get("memoryConfig"),
                       "meta_keys": sorted(meta.keys()),
                       "memoryReflection": [x for x in co if x[0] == "memoryReflection"],
                       "configOption_ids": [x[0] for x in co]}
        finally:
            c.close()

def p_override(D):
    """T2: session/new._meta.kiro.overrideSessionId honours a client-supplied id. Control: no field."""
    git_ws()
    c = Conn("override")
    try:
        init(c, tag="override:init")
        import uuid
        want = "sess_" + str(uuid.uuid4())
        c.tag = "override:new"
        r1 = c.call("session/new", {"cwd": WS, "mcpServers": [],
                                    "_meta": {"kiro": {"overrideSessionId": want}}}, 60)
        sid1 = ((r1 or {}).get("result") or {}).get("sessionId")
        c.tag = "override:new_same"
        r2 = c.call("session/new", {"cwd": WS, "mcpServers": [],
                                    "_meta": {"kiro": {"overrideSessionId": want}}}, 60)
        sid2 = ((r2 or {}).get("result") or {}).get("sessionId")
        c.tag = "override:control"
        r3 = c.call("session/new", {"cwd": WS, "mcpServers": []}, 60)
        sid3 = ((r3 or {}).get("result") or {}).get("sessionId")
        D.update({"wanted": want, "got_override": sid1, "honored": sid1 == want,
                  "repeat_same": sid2, "repeat_equal_first": sid2 == sid1,
                  "control_minted": sid3, "control_is_minted": bool(sid3) and sid3 != want,
                  "error": None if (r1 and "result" in r1) else res_or_err(r1)})
    finally:
        c.close()

def p_interaction(D):
    """T3: with a permission ask left UNANSWERED, does KAS emit an `interaction_resolved`
    session_info_update (e.g. on session/cancel)? Positive control: the ordinary answered path
    in every other capture shows only the selected-permission flow."""
    git_ws()
    c = Conn("interaction")
    c.held = []
    c.perms_hold = True
    try:
        init(c, tag="interaction:init")
        r, sid = new(c, tag="interaction:new")
        c.tag = "interaction:prompt"
        c.req("session/prompt", {"sessionId": sid, "prompt": [
            {"type": "text", "text": "Use your shell tool to run exactly: echo T3 > t3.txt   then reply DONE."}]})
        ask = c.wait_note(lambda m, p: m == "session/request_permission", 60, 0)
        D["ask_seen"] = bool(ask)
        D["ask_toolCallId"] = ((ask[2].get("toolCall") or {}).get("toolCallId") if ask else None)
        time.sleep(6)
        with c.cv:
            resolved = [p for _, m, p in c.notes
                        if K.kiro_kind(p) == "interaction_resolved"]
        D["resolved_before_cancel"] = resolved[-1].get("update", {}).get("_meta", {}).get("kiro") if resolved else None
        c.tag = "interaction:cancel"
        c.send({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}})
        end = time.time() + 30
        while time.time() < end:
            with c.cv:
                rs = [p for _, m, p in c.notes if K.kiro_kind(p) == "interaction_resolved"]
            if rs:
                break
            time.sleep(2)
        with c.cv:
            rs = [(ts, p) for ts, m, p in c.notes if K.kiro_kind(p) == "interaction_resolved"]
            D["resolved_after_cancel"] = [p.get("update", {}).get("_meta", {}).get("kiro") for _, p in rs]
            D["kinds_after"] = sorted({K.kiro_kind(p) for _, _, p in c.notes if K.kiro_kind(p)})
        D["note"] = ("cyril drops the kind at convert/kas.rs:525, so an unanswered ask would leave "
                     "the UI stuck; this probe measures whether the frame exists at all.")
    finally:
        c.close()

PROBES = {"intake": p_intake, "getstate": p_getstate, "fallback": p_fallback, "image": p_image,
          "cascade": p_cascade, "mcpenv": p_mcpenv, "multiclient": p_multiclient,
          "commands": p_commands, "ltoh": p_ltoh, "memory": p_memory, "override": p_override,
          "interaction": p_interaction}

def p_all():
    for k, fn in PROBES.items():
        D = R.setdefault(k, {})
        try: fn(D)
        except Exception:
            import traceback; D["exception"] = traceback.format_exc()
        emit({"probe": "section-result", "section": k, "data": D})

t0 = time.time()
try:
    (p_all if PROBE == "all" else (lambda: PROBES[PROBE](R.setdefault(PROBE, {}))))()
except Exception as e:
    import traceback; R["exception"] = traceback.format_exc()
finally:
    R["elapsed_s"] = round(time.time() - t0, 1)
    R["auth_events"] = K.AUTH_EVENTS
    tok = K.read_token()
    R["token_secs_left_at_end"] = round(K.secs_left(tok)) if tok else "LOGGED_OUT"
    emit(R)
    print(json.dumps(K.scrub(R), indent=1, default=str)[:20000])

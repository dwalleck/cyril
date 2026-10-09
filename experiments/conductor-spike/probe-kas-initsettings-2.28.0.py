#!/usr/bin/env python3
"""CORRECTED re-probe of three 2.28.0 KAS leads (T-L1, T-L6 memory, L1/L4 configuration/state).

Copy of probe-kas-leads-2.28.0.py with ONE plumbing fix: initialize `settings` now go to
`initialize.clientCapabilities._meta.kiro.settings` (merged with any caps_meta), which is the only
place KAS 0.66.26 reads them (`let r=t.clientCapabilities?._meta?.kiro ... this.clientMeta=r`,
`subagentOrchestrationActive(t){...Tw(this.clientMeta?.settings??{},"subagentOrchestration")}`,
`captureMemoryConfig` -> `this.clientMeta?.settings`, `configuration.stateStartup(W5(r?.settings))`).
The original put them at the initialize request's top-level `_meta.kiro.settings`, which KAS never
reads, so its orch/config/memory init-settings conclusions were measured against a no-op.
(`kiro_meta` is left at top-level `_meta.kiro` exactly as before; NOTE that KAS also reads
`telemetryEnabled` from clientCapabilities._meta.kiro, so the original L8 init_telemetryEnabled_false
variant had the same misplacement - not re-measured here.)

Reuses the Conn/auth/callback plumbing of probe-kas-live-2.28.0.py (imported in SCENARIO=lib mode).
One KAS process at a time; every frame -> <out.jsonl> as {ts, conn, dir, tag, msg}; a final
{"probe":"lead-result"} record carries the verdicts.

    KAS=<acp-server.js> PROBE_SCRATCH=<dir> probe-kas-initsettings-2.28.0.py <probe> <out.jsonl>

new probes (this file):
  orch2     T-L1: subagentOrchestration omitted vs {enabled:true} in clientCapabilities._meta.kiro.settings
            x workflows off/on (workflows+goal in session/new _meta.kiro.settings as before)
  memory    T-L6: clientCapabilities._meta.kiro.settings.memory disabled / read_only+noreflect vs control
            (+ a session/new memory positive control)
  confstate L1/L4: configurationState:true + init thinking off (+memory) vs controls; session/new thinking on
  all       orch2 + memory + confstate in one process / one capture
(the original probes are kept unchanged below; they now ALSO use the corrected placement)
"""
import importlib.util, json, os, select, socket, sys, threading, time, uuid

PROBE = sys.argv[1]
OUT = sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["SCENARIO"] = "lib"
os.environ.setdefault("LABEL", f"lead-{PROBE}")
sys.argv = [sys.argv[0], OUT]
spec = importlib.util.spec_from_file_location("kaslive", os.path.join(HERE, "probe-kas-live-2.28.0.py"))
K = importlib.util.module_from_spec(spec)
spec.loader.exec_module(K)
Conn, emit, res_or_err, short, log, WS, FAKE_HOME = K.Conn, K.emit, K.res_or_err, K.short, K.log, K.WS, K.FAKE_HOME
R = {"probe": "lead-result", "lead": PROBE, "kas_version": K.SUMMARY.get("kas_version")}

def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)

def init(c, kiro_meta=None, settings=None, caps_meta=None, tag="init", client_name="cyril-probe"):
    c.tag = tag
    caps = {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}
    # FIX (vs probe-kas-leads-2.28.0.py): KAS reads initialize settings ONLY from
    # clientCapabilities._meta.kiro.settings - merge them into the caps_meta object there.
    ckiro = dict(caps_meta or {})
    if settings is not None:
        ckiro["settings"] = settings
    if caps_meta is not None or settings is not None:
        caps["_meta"] = {"kiro": ckiro}
    prm = {"protocolVersion": 1, "clientCapabilities": caps,
           "clientInfo": {"name": client_name, "version": "audit-2.28.0"}}
    meta = dict(kiro_meta or {})
    if meta:
        prm["_meta"] = {"kiro": meta}
    return c.call("initialize", prm, 150)

def new(c, settings=None, tag="session_new"):
    c.tag = tag
    prm = {"cwd": WS, "mcpServers": []}
    if settings is not None:
        prm["_meta"] = {"kiro": {"settings": settings}}
    r = c.call("session/new", prm, 180)
    return r, ((r or {}).get("result") or {}).get("sessionId")

def notes(c, method=None, kind=None, since=0.0):
    with c.cv:
        out = []
        for ts, m, p in c.notes:
            if ts < since or (method and m != method):
                continue
            if kind and K.kiro_kind(p) != kind:
                continue
            out.append(p)
        return out

def call(c, tag, m, prm, to=60):
    c.tag = tag
    return res_or_err(c.call(m, prm, to))

def git_ws():
    import subprocess
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=WS)
    write(os.path.join(WS, "PROBE.txt"), "ALPHA\n")

# ------------------------------------------------------------------ CONNECT proxy
class Proxy:
    def __init__(self):
        self.sock = socket.socket(); self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", 0)); self.sock.listen(64)
        self.port = self.sock.getsockname()[1]
        self.hosts = []; self.fault = False; self.refused = []; self.live = set(); self.lock = threading.Lock()
        threading.Thread(target=self._accept, daemon=True).start()
    def _accept(self):
        while True:
            try:
                cs, _ = self.sock.accept()
            except OSError:
                return
            threading.Thread(target=self._handle, args=(cs,), daemon=True).start()
    def _handle(self, cs):
        try:
            data = b""
            while b"\r\n\r\n" not in data and len(data) < 65536:
                chunk = cs.recv(4096)
                if not chunk:
                    cs.close(); return
                data += chunk
            line = data.split(b"\r\n", 1)[0].decode(errors="replace")
            parts = line.split()
            method, target = (parts + ["", ""])[:2]
            ts = round(time.time(), 3)
            with self.lock:
                self.hosts.append({"ts": ts, "method": method, "target": target, "fault": self.fault})
            if self.fault:
                with self.lock:
                    self.refused.append(target)
                # FAULT_MODE=reset (default): drop the socket with no HTTP reply -> client sees a
                # network-class error (ECONNRESET/socket hang up); FAULT_MODE=502 -> HTTP 502 (server fault)
                if os.environ.get("FAULT_MODE", "reset") == "502":
                    cs.sendall(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                else:
                    try:
                        cs.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, b"\x01\x00\x00\x00\x00\x00\x00\x00")
                    except OSError:
                        pass
                cs.close(); return
            if method != "CONNECT":
                cs.sendall(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n"); cs.close(); return
            host, _, port = target.rpartition(":")
            up = socket.create_connection((host, int(port)), timeout=20)
            cs.sendall(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            pair = (cs, up)
            with self.lock:
                self.live.add(pair)
            try:
                socks = [cs, up]
                while True:
                    r, _, _ = select.select(socks, [], [], 1)
                    if self.fault:
                        break
                    done = False
                    for s in r:
                        d = s.recv(65536)
                        if not d:
                            done = True; break
                        (up if s is cs else cs).sendall(d)
                    if done:
                        break
            finally:
                with self.lock:
                    self.live.discard(pair)
                for s in pair:
                    try: s.close()
                    except Exception: pass
        except Exception as e:
            try: cs.close()
            except Exception: pass
    def env(self):
        u = f"http://127.0.0.1:{self.port}"
        return {"HTTPS_PROXY": u, "https_proxy": u, "HTTP_PROXY": u, "http_proxy": u,
                "NODE_USE_ENV_PROXY": "1", "NO_PROXY": "", "no_proxy": ""}

def with_env(extra):
    saved = {k: os.environ.get(k) for k in extra}
    os.environ.update(extra)
    return saved

def restore_env(saved):
    for k, v in saved.items():
        if v is None:
            os.environ.pop(k, None)
        else:
            os.environ[k] = v

# ======================================================================= probes
def p_config():
    git_ws()
    write(os.path.join(WS, ".kiro", "prompts", "hello.md"),
          "Say hello to $1 and then repeat the word $2 exactly once. Do not use tools.\n")
    write(os.path.join(WS, ".kiro", "prompts", "echoall.md"), "Reply with exactly: $ARGUMENTS\n")
    dump = os.path.join(os.path.dirname(OUT), f"confdump-{R['kas_version']}")
    saved = with_env({"KIRO_DUMP_CONFIGURATION": "1", "KIRO_DUMP_CONFIGURATION_DIR": dump})
    c = Conn("main")
    restore_env(saved)
    try:
        ini = init(c, settings={"thinking": {"enabled": False},
                                "memory": {"mode": "read_only", "reflection": False}},
                   caps_meta={"configurationState": True})
        ac = ((ini or {}).get("result") or {}).get("agentCapabilities") or {}
        R["agentCapabilities._meta.kiro.keys"] = sorted(((ac.get("_meta") or {}).get("kiro") or {}).keys())
        R["strictSessionLoad"] = ((ac.get("_meta") or {}).get("kiro") or {}).get("strictSessionLoad")
        time.sleep(2)
        R["confstate_after_init"] = notes(c, "_kiro/configuration/state")
        r, sid = new(c, settings={"thinking": {"enabled": True}})
        res = (r or {}).get("result") or {}
        R["session_new._meta.memoryConfig"] = (res.get("_meta") or {}).get("memoryConfig")
        R["session_new._meta.memoryConfigSource"] = (res.get("_meta") or {}).get("memoryConfigSource")
        R["configOptions"] = [(o.get("id"), o.get("currentValue")) for o in res.get("configOptions") or []]
        time.sleep(8)
        R["confstate_all"] = notes(c, "_kiro/configuration/state")
        with c.cv:
            ac_up = [p for _, m, p in c.notes if m == "session/update" and
                     (p.get("update") or {}).get("sessionUpdate") == "available_commands_update"]
        cmds = (ac_up[-1]["update"].get("availableCommands") if ac_up else []) or []
        R["available_commands_prompt_entries"] = [x for x in cmds if ((x.get("_meta") or {}).get("kiro") or {}).get("type") == "prompt"]
        R["available_commands_types"] = sorted({str(((x.get("_meta") or {}).get("kiro") or {}).get("type")) for x in cmds})
        R["L2_contribute"] = call(c, "L2", "_kiro/configuration/contribute",
                                  {"contributions": [{"layer": "client-connection", "owner": "stdio", "setting": "thinking", "value": "on"}]})
        R["L2_contribute_with_session"] = call(c, "L2", "_kiro/configuration/contribute",
                                  {"sessionId": sid, "contributions": [{"layer": "client-connection", "owner": "stdio", "setting": "thinking", "value": "on"}]})
        t = time.time()
        R["memoryReflection_set"] = short(call(c, "T-L6", "session/set_config_option", {"sessionId": sid, "configId": "memoryReflection", "value": "on"}), 600)
        time.sleep(3)
        R["memoryReflection_cfg_updates"] = [[(o.get("id"), o.get("currentValue")) for o in (p["update"].get("configOptions") or [])]
                                             for p in notes(c, "session/update", since=t) if p["update"].get("sessionUpdate") == "config_option_update"]
        R["confstate_after_memory_set"] = notes(c, "_kiro/configuration/state", since=t)
        for m in ("_kiro/terminal/moveToBackground", "_kiro/terminal/output", "_kiro/terminal/stopBackground", "_kiro/terminal/stopAllBackground"):
            R["T-L8 " + m] = call(c, "T-L8", m, {"sessionId": sid, "terminalId": "term-x", "toolCallId": "tc-x", "lines": 50})
        R["T-L7_requireExisting_missing"] = short(call(c, "T-L7", "session/load",
            {"sessionId": "sess_" + str(uuid.uuid4()), "cwd": WS, "mcpServers": [], "_meta": {"kiro": {"requireExisting": True}}}, 90), 800)
        # L5 + T-L5: breakdown + displayText
        T = {}
        for tag, meta, text in [
            ("L5_detailed", {"kiro": {"contextBreakdown": "detailed", "displayText": "short label A"}}, "Reply with exactly ONE."),
            ("L5_summary", {"kiro": {"contextBreakdown": "summary"}}, "Reply with exactly TWO."),
            ("L5_control", None, "Reply with exactly THREE."),
            ("L9_hello", None, "/hello world BANANA"),
            ("L9_echoall", None, "/echoall alpha beta"),
        ]:
            t = time.time()
            T[tag] = K.prompt(c, sid, text, tag, 300, meta=meta)
            tc = [p["update"]["_meta"]["kiro"] for p in notes(c, "session/update", kind="turn_completion", since=t)]
            T[tag]["turn_completion_keys"] = sorted({k for x in tc for k in x})
            T[tag]["contextBreakdown"] = [x.get("contextBreakdown") for x in tc if "contextBreakdown" in x]
            T[tag]["agent_text"] = "".join(((p["update"].get("content") or {}).get("text") or "")
                                           for p in notes(c, "session/update", since=t)
                                           if p["update"].get("sessionUpdate") == "agent_message_chunk")[:300]
        R["turns"] = T
        h = call(c, "T-L4", "_kiro/session/history", {"sessionId": sid, "limit": 100})
        R["T-L4_history_keys"] = sorted(h.keys()) if isinstance(h, dict) else h
        R["T-L4_history"] = short(h, 4000)
        if isinstance(h, dict) and h.get("oldestLoadedMessageId"):
            R["T-L4_history_before"] = short(call(c, "T-L4b", "_kiro/session/history",
                                                  {"sessionId": sid, "beforeMessageId": h["oldestLoadedMessageId"], "limit": 2}), 1500)
        R["permissions"] = c.perms
    finally:
        c.close()
    c2 = Conn("reload")
    try:
        init(c2, caps_meta={"configurationState": True}, tag="reload:init")
        R["reload_session_load"] = short(call(c2, "reload:load", "session/load", {"sessionId": sid, "cwd": WS, "mcpServers": []}, 180), 600)
        time.sleep(5)
        R["reload_user_chunks_meta"] = [((p["update"].get("_meta") or {}).get("kiro"), ((p["update"].get("content") or {}).get("text") or "")[:80])
                                        for p in notes(c2, "session/update")
                                        if p["update"].get("sessionUpdate") == "user_message_chunk"]
        R["reload_confstate"] = notes(c2, "_kiro/configuration/state")
    finally:
        c2.close()
    R["confdump_files"] = sorted(os.listdir(dump)) if os.path.isdir(dump) else None

def tools_from(c):
    with c.cv:
        tl = [p for _, m, p in c.notes if m == "_kiro/tools/didChange"]
    if not tl:
        return None
    last = tl[-1]
    items = last.get("tools") or last.get("items") or last
    names = []
    if isinstance(items, list):
        for t in items:
            names.append(t.get("name") or t.get("id") if isinstance(t, dict) else t)
    return {"names": sorted(n for n in names if n), "raw_keys": sorted(last.keys())}

def p_orch():
    git_ws()
    base = {"codeIntelligence": {"enabled": True}, "knowledge": {"enabled": True},
            "thinking": {"enabled": True}, "largeToolOutputHandler": {"enabled": True}}
    M = {}
    for name, extra in [("A_noorch_wfoff", {}),
                        ("B_orch_wfoff", {"subagentOrchestration": {"enabled": True}}),
                        ("C_noorch_wfon", {"workflows": {"enabled": True}, "goal": {"enabled": True}}),
                        ("D_orch_wfon", {"subagentOrchestration": {"enabled": True}, "workflows": {"enabled": True}, "goal": {"enabled": True}})]:
        s = {**base, **extra}
        c = Conn(name)
        try:
            init(c, settings=s, tag=name + ":init")
            r, sid = new(c, settings=s, tag=name + ":new")
            time.sleep(6)
            # the tool list is not on the wire (_kiro/tools/didChange carries only tag groups), so ask
            # the model and pair it with the toolSpecs char count from contextBreakdown (model-independent)
            t = time.time()
            turn = K.prompt(c, sid, "List the exact names of every tool available to you, comma-separated, nothing else.",
                            name + ":tools", 300, meta={"kiro": {"contextBreakdown": "summary"}})
            text = "".join(((p["update"].get("content") or {}).get("text") or "") for p in notes(c, "session/update", since=t)
                           if p.get("sessionId") == sid and p["update"].get("sessionUpdate") == "agent_message_chunk")
            names = sorted({x.strip().strip("`") for x in text.replace("\n", ",").split(",") if x.strip()})
            cb = [p["update"]["_meta"]["kiro"].get("contextBreakdown") for p in notes(c, "session/update", kind="turn_completion", since=t)]
            cb = [x for x in cb if x]
            M[name] = {"turn": turn, "model_tool_names": names,
                       "toolSpecs_chars": cb[0]["categories"]["toolSpecs"]["chars"] if cb else None,
                       "orchestrate": [n for n in names if "rchestrat" in n or "sub_agent" in n.lower() or "subagent" in n.lower() or "delegat" in n.lower()],
                       "workflow_tools": [n for n in names if "workflow" in n],
                       "workflowsEnabled": (((r or {}).get("result") or {}).get("_meta") or {}).get("workflowsEnabled")}
            log(name, M[name]["orchestrate"], M[name]["workflow_tools"])
        finally:
            c.close()
    R["matrix"] = M

def p_bg():
    git_ws()
    M = {}
    for name, caps_meta in [("with_terminalInput", {"terminalInput": True}), ("control_no_terminalInput", {})]:
        c = Conn(name)
        try:
            s = {"backgroundExecution": {"enabled": True}}
            init(c, settings=s, caps_meta=caps_meta, tag=name + ":init")
            r, sid = new(c, settings=s, tag=name + ":new")
            time.sleep(6)
            t = time.time()
            turn = K.prompt(c, sid,
                "Using your shell tool, start this command as an interactive BACKGROUND process (run_in_background / interactive): "
                "python3 -c 'input();print(42)'   Once it is running, send it a single newline on its stdin, "
                "wait for it to exit, and then reply with exactly what it printed. If you cannot send input to it, say NO_INPUT_SUPPORT.",
                name + ":turn", 600)
            with c.cv:
                tcs = [p["update"] for _, m, p in c.notes if m == "session/update" and p.get("sessionId") == sid
                       and (p.get("update") or {}).get("sessionUpdate") in ("tool_call", "tool_call_update")]
            M[name] = {"turn": turn, "callbacks": dict(c.callbacks),
                       "tool_calls": [{"title": u.get("title"), "status": u.get("status"), "rawInput": u.get("rawInput"),
                                       "kiro": (u.get("_meta") or {}).get("kiro")} for u in tcs][:30],
                       "session_info_kinds": sorted({K.kiro_kind(p) for p in notes(c, "session/update", since=t) if K.kiro_kind(p)})}
            with c.cv:
                M[name]["sessionUpdate_kinds"] = sorted({str((p.get("update") or {}).get("sessionUpdate")) for _, m, p in c.notes if m == "session/update"})
            log(name, json.dumps(turn), c.callbacks)
        finally:
            c.close()
    R["legs"] = M

def p_cascade():
    git_ws()
    cfg = {"cascadeEnabled": True, "cascadeProbeOnFirstRequest": True, "cascadeProbeStartTurn": 1,
           "cascadeClientNotificationEnabled": True}
    saved = with_env({"KIRO_FEATURE_CASCADE_CONFIG": json.dumps(cfg)})
    c = Conn("cascade")
    restore_env(saved)
    try:
        s = {"cascade": {"enabled": True}}
        init(c, settings=s, caps_meta={"configurationState": True})
        r, sid = new(c, settings=s)
        res = (r or {}).get("result") or {}
        mopt = next((o for o in res.get("configOptions") or [] if o.get("id") == "model"), {})
        vals = [o.get("value") for o in (mopt.get("options") or [])]
        R["model_values"] = vals
        R["auto_family"] = [v for v in vals if str(v).startswith("auto")]
        R["current_model"] = mopt.get("currentValue")
        T = {}
        for i, text in enumerate(["Reply with exactly ONE.",
                                  "Write a Python function that returns the nth Fibonacci number iteratively, in a code block, nothing else.",
                                  "Explain in two sentences what a B-tree is.",
                                  "Reply with exactly FOUR."]):
            t = time.time()
            i = str(i)
            T[i] = K.prompt(c, sid, text, f"cascade:t{i}", 300)
            T[i]["kinds"] = sorted({K.kiro_kind(p) for p in notes(c, "session/update", since=t) if K.kiro_kind(p)})
            T[i]["model_routed"] = [p["update"]["_meta"]["kiro"] for p in notes(c, "session/update", kind="model_routed", since=t)]
            T[i]["config_option_updates"] = [[(o.get("id"), o.get("currentValue")) for o in (p["update"].get("configOptions") or []) if o.get("id") == "model"]
                                             for p in notes(c, "session/update", since=t) if p["update"].get("sessionUpdate") == "config_option_update"]
            T[i]["turn_completion"] = [p["update"]["_meta"]["kiro"] for p in notes(c, "session/update", kind="turn_completion", since=t)]
        R["turns"] = T
    finally:
        c.close()
    # sidecar / persisted session state
    found = []
    for root, _, files in os.walk(os.path.join(FAKE_HOME, ".kiro", "sessions")):
        for f in files:
            if sid and sid in f or f.endswith(".json"):
                found.append(os.path.join(root, f))
    R["session_files"] = found[:30]
    for f in found:
        try:
            txt = open(f, encoding="utf-8").read()
        except Exception:
            continue
        if "cascade" in txt or "model_route" in txt or "classifier" in txt:
            R.setdefault("session_files_with_cascade", []).append({"file": f, "excerpt": txt[max(0, txt.find("cascade") - 200):txt.find("cascade") + 600]})

def p_telemetry():
    git_ws()
    px = Proxy()
    M = {}
    variants = [("default", {}, None), ("env_kuts_false", {"KIRO_FEATURE_KUTS_TELEMETRY_ENABLED": "false"}, None),
                ("init_telemetryEnabled_false", {}, {"telemetryEnabled": False})]
    if os.environ.get("TELEMETRY_ONLY_DEFAULT") == "1":
        variants = variants[:1]
    for name, env_extra, kmeta in variants:
        mark = len(px.hosts)
        saved = with_env({**px.env(), **env_extra})
        c = Conn(name)
        restore_env(saved)
        try:
            init(c, kiro_meta=kmeta, tag=name + ":init")
            r, sid = new(c, tag=name + ":new")
            turn = K.prompt(c, sid, "Reply with exactly OK.", name + ":turn", 300) if sid else None
            time.sleep(int(os.environ.get("TELEMETRY_IDLE", "45")))
        finally:
            # graceful first: close stdin and give KAS time to flush exporters before SIGTERM
            try:
                c.proc.stdin.close()
                c.proc.wait(timeout=int(os.environ.get("TELEMETRY_GRACE", "0")) or 0.1)
            except Exception:
                pass
            c.close()
        time.sleep(3)
        hs = px.hosts[mark:]
        M[name] = {"turn": turn, "connects": sorted({h["target"] for h in hs}),
                   "telemetry_connects": [h for h in hs if "telemetry" in h["target"]],
                   "n": len(hs)}
        log(name, M[name]["connects"])
    R["variants"] = M

def p_stepnote():
    git_ws()
    px = Proxy()
    wf = os.path.join(WS, ".kiro", "workflows", "net.workflow.json")
    write(wf, json.dumps({"name": "net", "injectOriginalUserRequest": False, "steps": [
        {"type": "step", "id": "n1", "agent": "wf-coder",
         "prompt": "Count slowly from 1 to 30, one number per line, then reply DONE."}]}))
    saved = with_env({**px.env(), "KIRO_WORKFLOW_TRANSIENT_RETRY_DELAYS_SEC": "3,3,3,3,3,3"})
    c = Conn("stepnote")
    restore_env(saved)
    try:
        init(c, settings=K.SETTINGS_ON)
        r, parent = new(c, settings=K.SETTINGS_ON)
        time.sleep(5)
        R["connects_before"] = sorted({h["target"] for h in px.hosts})
        c.tag = "wf:new"
        nw = c.call("_kiro/workflow/new", {"workflowPath": wf, "inputs": {}, "parentSessionId": parent, "workspacePaths": [WS]}, 60)
        wid = ((nw or {}).get("result") or {}).get("workflowId")
        R["new"] = "ok" if wid else short(res_or_err(nw))
        if not wid:
            return
        t0 = time.time()
        c.tag = "wf:invoke"
        c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)
        ns = c.wait_note(lambda m, p: m == "_kiro/workflow/node_start" and p.get("workflowId") == wid and p.get("sessionId"), 180, t0)
        if ns:
            c.wait_note(lambda m, p: m == "session/update" and p.get("sessionId") == ns[2]["sessionId"]
                        and (p.get("update") or {}).get("sessionUpdate") == "agent_message_chunk", 120, ns[0])
        c.tag = "FAULT"
        px.fault = True
        tf = time.time()
        log("fault ON")
        # hold the fault while retry-waits happen, up to 150 s or until a parent note arrives
        c.wait_note(lambda m, p: m == "_kiro/session/notify", 150, tf)
        time.sleep(10)
        px.fault = False
        log("fault OFF")
        c.tag = "RECOVER"
        d = K.wait_run(c, wid, ("completed", "failed", "aborted", "paused"), 600, t0)
        R["run_status"] = d[2].get("status") if d else "TIMEOUT"
        R["notify"] = notes(c, "_kiro/session/notify", since=tf)
        R["node_paused"] = [p for p in notes(c, "_kiro/workflow/node_paused", since=t0)]
        R["parent_turns"] = [K.kiro_kind(p) for p in notes(c, "session/update", since=tf)
                             if p.get("sessionId") == parent and K.kiro_kind(p) in ("turn_start", "turn_end")]
        if d and d[2].get("status") in ("completed", "failed", "aborted"):
            R["wake"] = K.wait_wake(c, parent, d[0])
        elif d:
            c.tag = "resume"
            R["resume"] = short(res_or_err(c.call("_kiro/workflow/resume", {"workflowId": wid}, 60)))
            d2 = K.wait_run(c, wid, ("completed", "failed", "aborted"), 600, time.time())
            R["run_status_after_resume"] = d2[2].get("status") if d2 else "TIMEOUT"
            if d2:
                R["wake"] = K.wait_wake(c, parent, d2[0])
        R["refused_connects"] = len(px.refused)
        R["parent_turns_total"] = [K.kiro_kind(p) for p in notes(c, "session/update", since=tf)
                                   if p.get("sessionId") == parent and K.kiro_kind(p) in ("turn_start", "turn_end")]
    finally:
        c.close()

def p_wfextra():
    git_ws()
    wf = os.path.join(WS, ".kiro", "workflows", "twoshell.workflow.json")
    write(wf, json.dumps({"name": "twoshell", "injectOriginalUserRequest": False, "steps": [
        {"type": "step", "id": "sh1", "agent": "wf-coder", "prompt": "Use your shell tool to run exactly: echo ONE > one.txt   Then reply ONE."},
        {"type": "step", "id": "sh2", "agent": "wf-coder", "prompt": "Use your shell tool to run exactly: echo TWO > two.txt   Then reply TWO."}]}))
    c = Conn("wfextra")
    # answer allow_ALWAYS for every permission in this probe
    orig = c.callback
    def cb(method, p):
        if method == "session/request_permission":
            opts = p.get("options") or []
            pick = next((x for x in opts if x.get("kind") == "allow_always"), None) or (opts[0] if opts else None)
            c.perms.append({"tag": c.tag, "sessionId": p.get("sessionId"), "title": (p.get("toolCall") or {}).get("title"),
                            "kinds": [x.get("kind") for x in opts], "picked": pick and pick.get("optionId"),
                            "consent": ((p.get("_meta") or {}).get("kiro") or {}).get("consent"),
                            "meta": p.get("_meta")})
            return {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
        return orig(method, p)
    c.callback = cb
    try:
        init(c, settings=K.SETTINGS_ON)
        r, parent = new(c, settings=K.SETTINGS_ON)
        time.sleep(8)
        tl = tools_from(c)
        R["workflow_tools"] = [n for n in (tl or {}).get("names", []) if "workflow" in n]
        t = time.time()
        R["tool_turn"] = K.prompt(c, parent,
            "Do these two things and report each tool's result verbatim. (1) If you have a tool named validate_workflow, call it with the "
            "definition {\"name\":\"bad\",\"steps\":[{\"type\":\"step\",\"id\":\"x\",\"agent\":\"wf-coder\"}]}; if you have no such tool, say NO_VALIDATE_TOOL. "
            "(2) Call save_workflow_definition with that same definition and report its result verbatim. Do not run any workflow.",
            "wfextra:tools", 400)
        with c.cv:
            R["tool_turn_calls"] = [{"title": p["update"].get("title"), "status": p["update"].get("status"),
                                     "rawInput": p["update"].get("rawInput"), "rawOutput": short(p["update"].get("rawOutput"), 1200)}
                                    for ts, m, p in c.notes if ts >= t and m == "session/update"
                                    and p["update"].get("sessionUpdate") in ("tool_call", "tool_call_update")
                                    and (p["update"].get("rawOutput") is not None or p["update"].get("sessionUpdate") == "tool_call")][:20]
        n0 = len(c.perms)
        wid, since = K.start_run(c, parent, wf, "twoshell")
        if wid:
            K.finish_run(c, parent, "twoshell", wid, since, 900)
        R["twoshell_perms"] = c.perms[n0:]
        R["twoshell"] = K.RUNS.get("twoshell")
    finally:
        c.close()

def p_history():
    """T-L4 follow-up (history with beforeMessageId) + L9 follow-up (`${1}` positional prompt args)."""
    git_ws()
    write(os.path.join(WS, ".kiro", "prompts", "hello.md"),
          "Say hello to ${1} and then repeat the word ${2} exactly once. Do not use tools.\n")
    c = Conn("history")
    try:
        init(c)
        r, sid = new(c)
        time.sleep(6)
        with c.cv:
            ac_up = [p for _, m, p in c.notes if m == "session/update" and (p.get("update") or {}).get("sessionUpdate") == "available_commands_update"]
        cmds = (ac_up[-1]["update"].get("availableCommands") if ac_up else []) or []
        R["hello_entry"] = [x for x in cmds if x.get("name") == "hello"]
        ids = []
        for i, text in enumerate(["Reply with exactly ONE.", "Reply with exactly TWO.", "/hello world BANANA"]):
            t = time.time()
            R[f"turn{i}"] = K.prompt(c, sid, text, f"hist:t{i}", 300)
            ids += [p["update"]["_meta"]["kiro"].get("userMessageId") for p in notes(c, "session/update", kind="user_message_id_assigned", since=t)]
            R[f"turn{i}"]["agent_text"] = "".join(((p["update"].get("content") or {}).get("text") or "") for p in notes(c, "session/update", since=t)
                                                   if p["update"].get("sessionUpdate") == "agent_message_chunk")[:300]
        R["user_message_ids"] = ids
        R["h_nobefore"] = short(call(c, "h1", "_kiro/session/history", {"sessionId": sid, "limit": 100}), 3000)
        if ids:
            R["h_before_last"] = short(call(c, "h2", "_kiro/session/history", {"sessionId": sid, "beforeMessageId": ids[-1], "limit": 100}), 4000)
            R["h_before_last_lim1"] = short(call(c, "h3", "_kiro/session/history", {"sessionId": sid, "beforeMessageId": ids[-1], "limit": 1}), 3000)
        R["h_bad_before"] = short(call(c, "h4", "_kiro/session/history", {"sessionId": sid, "beforeMessageId": "nope", "limit": 5}), 1000)
    finally:
        c.close()

def p_always():
    """allow_always on a MAIN-session shell approval: does KAS re-ask (consentRound) like it does on step sessions?"""
    git_ws()
    c = Conn("always")
    orig = c.callback
    def cb(method, p):
        if method == "session/request_permission":
            opts = p.get("options") or []
            pick = next((x for x in opts if x.get("kind") == "allow_always"), None) or (opts[0] if opts else None)
            c.perms.append({"tag": c.tag, "sessionId": p.get("sessionId"), "toolCallId": (p.get("toolCall") or {}).get("toolCallId"),
                            "title": (p.get("toolCall") or {}).get("title"), "picked": pick and pick.get("optionId"),
                            "consentRound": ((p.get("_meta") or {}).get("kiro") or {}).get("consentRound"),
                            "consent": ((p.get("_meta") or {}).get("kiro") or {}).get("consent")})
            return {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
        return orig(method, p)
    c.callback = cb
    try:
        init(c)
        r, sid = new(c)
        time.sleep(5)
        R["turn1"] = K.prompt(c, sid, "Use your shell tool to run exactly: echo AA > aa.txt   then reply with exactly DONE.", "always:t1", 300)
        R["turn2"] = K.prompt(c, sid, "Use your shell tool to run exactly: echo AA > aa.txt   again, then reply with exactly DONE2.", "always:t2", 300)
        R["perms"] = c.perms
        R["policy_errors"] = notes(c, "_kiro/policy/error")
        R["permissions_list"] = short(call(c, "plist", "_kiro/permissions/list", {"sessionId": sid}), 3000)
    finally:
        c.close()
    found = []
    for root in (WS, FAKE_HOME):
        for d, _, fs in os.walk(os.path.join(root, ".kiro")):
            for f in fs:
                if "perm" in f.lower() or "polic" in f.lower():
                    pth = os.path.join(d, f)
                    found.append({"file": pth, "head": open(pth, errors="replace").read()[:800]})
    R["permission_files"] = found

# ============================================================ corrected re-probes
TOOLS_Q = ("List the exact names (ids) of every tool available to you, comma-separated, nothing else. "
           "Do not call any tool.")

def log_dirs():
    d = os.path.join(FAKE_HOME, ".kiro", "logs")
    return set(os.listdir(d)) if os.path.isdir(d) else set()

def log_scan(before, words):
    """model-independent corroboration: count tool ids in the KAS log dirs created by this arm"""
    d = os.path.join(FAKE_HOME, ".kiro", "logs")
    out = {"dirs": sorted(log_dirs() - before), "counts": {w: 0 for w in words}}
    for sub in out["dirs"]:
        for root, _, fs in os.walk(os.path.join(d, sub)):
            for f in fs:
                try:
                    txt = open(os.path.join(root, f), encoding="utf-8", errors="replace").read()
                except OSError:
                    continue
                for w in words:
                    out["counts"][w] += txt.count(w)
    return out

def tool_turn(c, sid, tag):
    t = time.time()
    turn = K.prompt(c, sid, TOOLS_Q, tag, 300, meta={"kiro": {"contextBreakdown": "summary"}})
    text = "".join(((p["update"].get("content") or {}).get("text") or "") for p in notes(c, "session/update", since=t)
                   if p.get("sessionId") == sid and p["update"].get("sessionUpdate") == "agent_message_chunk")
    names = sorted({x.strip().strip("`").strip("*").strip() for x in text.replace("\n", ",").split(",") if x.strip()})
    cb = [p["update"]["_meta"]["kiro"].get("contextBreakdown") for p in notes(c, "session/update", kind="turn_completion", since=t)]
    cb = [x for x in cb if x]
    cats = cb[0]["categories"] if cb else {}
    return {"turn": turn, "model_tool_names": names, "n_tools": len(names),
            "toolSpecs_chars": (cats.get("toolSpecs") or {}).get("chars"),
            "memory_category": cats.get("memory"),
            "delegation": [n for n in names if n in ("orchestrate_subagent", "invoke_sub_agent") or "rchestrat" in n
                           or "sub_agent" in n.lower() or "subagent" in n.lower()],
            "workflow_tools": [n for n in names if "workflow" in n or n == "send_message"],
            "memory_tools": [n for n in names if "memor" in n.lower()]}

def sess_summary(r):
    res = (r or {}).get("result") or {}
    meta = res.get("_meta") or {}
    return {"result_keys": sorted(res.keys()), "_meta_keys": sorted(meta.keys()),
            "memoryConfig": meta.get("memoryConfig"), "memoryConfigSource": meta.get("memoryConfigSource"),
            "workflowsEnabled": meta.get("workflowsEnabled"),
            "_meta_kiro": short(meta.get("kiro"), 1500) if meta.get("kiro") is not None else None,
            "configOptions": [(o.get("id"), o.get("currentValue")) for o in res.get("configOptions") or []],
            "error": res_or_err(r) if r is not None and "error" in r else (None if r else "TIMEOUT")}

def confstate_rows(payloads):
    out = []
    for p in payloads:
        out.append({"view": p.get("view"),
                    "layers": [l.get("id") or l.get("name") for l in p.get("layers") or []],
                    "layer_names": {l.get("id"): l.get("name") for l in p.get("layers") or []},
                    "settings": [{"krn": x.get("krn"), "value": x.get("value"), "compose": x.get("compose"),
                                  "assertions": x.get("assertions"), "contributions": x.get("contributions"),
                                  "other_keys": sorted(set(x) - {"krn", "value", "compose", "assertions", "contributions"})}
                                 for x in p.get("settings") or []],
                    "resources_n": len(p.get("resources") or []),
                    "top_keys": sorted(p.keys())})
    return out

ORCH_WORDS = ("orchestrate_subagent", "invoke_sub_agent", "run_workflow", "subagentOrchestration")

def p_orch2(D):
    """T-L1: init-level subagentOrchestration (correct placement) x session-level workflows."""
    git_ws()
    base = {"codeIntelligence": {"enabled": True}, "knowledge": {"enabled": True},
            "thinking": {"enabled": True}, "largeToolOutputHandler": {"enabled": True}}
    orch = {"subagentOrchestration": {"enabled": True}}
    wf = {"workflows": {"enabled": True}, "goal": {"enabled": True}}
    arms = [  # name, init settings (clientCapabilities._meta.kiro.settings), session/new settings
        ("A_noorch_wfoff", base, base),
        ("B_orch_wfoff", {**base, **orch}, {**base, **orch}),
        ("C_noorch_wfon", base, {**base, **wf}),
        ("D_orch_wfon", {**base, **orch}, {**base, **orch, **wf}),
        # E = cyril-like: everything (incl. workflows/goal) ALSO in the init settings
        ("E_orch_wfon_alsoinit", {**base, **orch, **wf}, {**base, **orch, **wf}),
        # F = orchestration ONLY at session level (control for "session/new does not carry it")
        ("F_orchSessionOnly_wfon", base, {**base, **orch, **wf}),
    ]
    for name, init_s, sess_s in arms:
        before = log_dirs()
        c = Conn(name)
        try:
            ini = init(c, settings=init_s, tag=name + ":init")
            r, sid = new(c, settings=sess_s, tag=name + ":new")
            time.sleep(6)
            arm = {"init_settings_keys": sorted(init_s), "session_settings_keys": sorted(sess_s),
                   "init_ok": bool((ini or {}).get("result")), "session_new": sess_summary(r)}
            if sid:
                arm.update(tool_turn(c, sid, name + ":tools"))
            arm["didChange"] = tools_from(c)
            D[name] = arm
            log(name, arm.get("delegation"), arm.get("workflow_tools"), arm.get("toolSpecs_chars"))
        finally:
            c.close()
        D[name]["kas_log"] = log_scan(before, ORCH_WORDS)

def p_memory(D):
    """T-L6: init memory settings (correct placement) vs control, + session-level positive control."""
    git_ws()
    arms = [("M0_control", None, None),
            ("M1_init_disabled", {"memory": {"mode": "disabled"}}, None),
            ("M2_init_readonly_noreflect", {"memory": {"mode": "read_only", "reflection": False}}, None),
            ("M3_session_disabled", None, {"memory": {"mode": "disabled"}}),
            ("M4_init_readonly_session_readwrite", {"memory": {"mode": "read_only", "reflection": False}},
             {"memory": {"mode": "read_write"}})]
    for name, init_s, sess_s in arms:
        before = log_dirs()
        c = Conn(name)
        try:
            init(c, settings=init_s, tag=name + ":init")
            r, sid = new(c, settings=sess_s, tag=name + ":new")
            time.sleep(5)
            arm = {"init_settings": init_s, "session_settings": sess_s, "session_new": sess_summary(r)}
            if sid:
                arm.update(tool_turn(c, sid, name + ":tools"))
            D[name] = arm
            log(name, arm["session_new"]["memoryConfig"], arm.get("memory_tools"), arm.get("toolSpecs_chars"))
        finally:
            c.close()
        D[name]["kas_log"] = log_scan(before, ("memoryConfig", "memory.", "AB_MEMORY"))

def p_confstate(D):
    """L1/L4: does an init setting reach _kiro/configuration/state, in which layer, and how does a
    session/new setting compose with it?"""
    git_ws()
    scratch = os.environ.get("PROBE_SCRATCH") or os.path.dirname(OUT)
    arms = [  # name, init settings, session/new settings
        ("S0_control_noinit_sessOn", None, {"thinking": {"enabled": True}}),
        ("S1_initOff_mem_sessOn", {"thinking": {"enabled": False}, "memory": {"mode": "read_only", "reflection": False}},
         {"thinking": {"enabled": True}}),
        ("S2_initOff_mem_sessNone", {"thinking": {"enabled": False}, "memory": {"mode": "read_only", "reflection": False}},
         None),
        # S3: knowledge* is the only family in the kiro-agent intake table (XAt={startup:[],kiroAgent:Puu,...})
        ("S3_initKnowledge_thinkingOff_sessOn", {"knowledge": {"enabled": False, "maxFiles": 123},
                                                  "thinking": {"enabled": False}}, {"thinking": {"enabled": True}}),
    ]
    for name, init_s, sess_s in arms:
        dump = os.path.join(scratch, f"confdump-{name}")
        saved = with_env({"KIRO_DUMP_CONFIGURATION": "1", "KIRO_DUMP_CONFIGURATION_DIR": dump})
        c = Conn(name)
        restore_env(saved)
        try:
            init(c, settings=init_s, caps_meta={"configurationState": True}, tag=name + ":init")
            time.sleep(3)
            after_init = notes(c, "_kiro/configuration/state")
            t = time.time()
            r, sid = new(c, settings=sess_s, tag=name + ":new")
            time.sleep(8)
            D[name] = {"init_settings": init_s, "session_settings": sess_s,
                       "confstate_after_init": confstate_rows(after_init),
                       "confstate_after_new": confstate_rows(notes(c, "_kiro/configuration/state", since=t)),
                       "session_new": sess_summary(r)}
            if sid:
                # does init thinking:off actually reach the model? (thought chunks in a reasoning-y turn)
                t2 = time.time()
                D[name]["think_turn"] = K.prompt(c, sid, "What is 17*23? Think it through, then reply with just the number.",
                                                 name + ":think", 300)
                ups = [p["update"].get("sessionUpdate") for p in notes(c, "session/update", since=t2) if p.get("sessionId") == sid]
                D[name]["think_turn"]["agent_thought_chunks"] = ups.count("agent_thought_chunk")
                D[name]["confstate_after_turn"] = confstate_rows(notes(c, "_kiro/configuration/state", since=t2))
            log(name, json.dumps(D[name]["confstate_after_init"])[:300])
        finally:
            c.close()
        files = []
        for root, _, fs in os.walk(dump):
            for f in sorted(fs):
                pth = os.path.join(root, f)
                files.append(os.path.relpath(pth, dump))
        D[name]["confdump_files"] = files
        conn_dumps = [f for f in files if f.startswith("connection")]
        if conn_dumps:
            try:
                D[name]["confdump_connection_first"] = json.load(open(os.path.join(dump, sorted(conn_dumps)[-1])))
            except (OSError, ValueError) as e:
                D[name]["confdump_connection_first"] = f"unreadable: {e}"

def p_all():
    for key, fn in (("orch2", p_orch2), ("memory", p_memory), ("confstate", p_confstate)):
        D = R.setdefault(key, {})
        try:
            fn(D)
        except Exception:
            import traceback
            D["exception"] = traceback.format_exc()
        emit({"probe": "section-result", "section": key, "data": D})

PROBES = {"config": p_config, "orch": p_orch, "bg": p_bg, "cascade": p_cascade,
          "telemetry": p_telemetry, "stepnote": p_stepnote, "wfextra": p_wfextra, "history": p_history, "always": p_always,
          "orch2": lambda: p_orch2(R.setdefault("orch2", {})), "memory": lambda: p_memory(R.setdefault("memory", {})),
          "confstate": lambda: p_confstate(R.setdefault("confstate", {})), "all": p_all}
t0 = time.time()
try:
    PROBES[PROBE]()
except Exception as e:
    import traceback
    R["exception"] = traceback.format_exc()
finally:
    R["elapsed_s"] = round(time.time() - t0, 1)
    R["auth_events"] = K.AUTH_EVENTS
    tok = K.read_token()
    R["token_secs_left_at_end"] = round(K.secs_left(tok)) if tok else "LOGGED_OUT"
    emit(R)
    print(json.dumps(K.scrub(R), indent=1, default=str)[:20000])

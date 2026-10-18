#!/usr/bin/env python3
"""Leads + changelog checks for the kiro-cli 2.32.0 live lane (KAS 0.66.34 vs 0.66.43).

  KAS=<acp-server.js> PROBE_SCRATCH=<dir> python3 -I probe-kas-leads-2.32.0.py <probe> <out.jsonl>

probes (own + forwarded TUI leads L1 folder, L2 outstyle, L3 include, L5 diagrams, L6 interrupt):
  write     write approvals: out-of-workspace write (user explicitly confirms) and in-workspace write;
            records every session/request_permission params (options + _meta) and pending_interaction.
  outstyle  output styles: session/prompt _meta.kiro.outputStyle = none | explanatory | learning (one turn each).
  image     oversized image (9000x9000 PNG, over the provider's 8000 px edge limit) + a 1000x1000 control.
  fish      command approval with _kiro/terminal/shell_type = bash | fish, two commands each.
  export    _kiro/session/export path shapes (.zip / non-.zip) after one turn.
Imports probe-kas-live-2.32.0.py (SCENARIO=lib) for Conn/auth plumbing. Every handshake writes the FULL
path initialize.clientCapabilities._meta.kiro.* (trap 5); positive control = configurationState:true ->
`_kiro/configuration/state` push (recorded per arm as cfgstate_push). One process per arm, serial.
"""
import importlib.util, json, os, struct, subprocess, sys, tempfile, time, zlib, base64
PROBE = sys.argv[1]
OUT = sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["SCENARIO"] = "lib"
os.environ.setdefault("LABEL", f"lead-{PROBE}")
sys.argv = [sys.argv[0], OUT]
spec = importlib.util.spec_from_file_location("kaslive", os.path.join(HERE, "probe-kas-live-2.32.0.py"))
K = importlib.util.module_from_spec(spec); spec.loader.exec_module(K)
Conn, emit, res_or_err, short, log = K.Conn, K.emit, K.res_or_err, K.short, K.log
SCRATCH = K.SCRATCH
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
           "clientInfo": {"name": "cyril-probe", "version": "audit-2.32.0"}}
    return c.call("initialize", prm, 150)

def new(c, ws, tag="session_new"):
    c.tag = tag
    r = c.call("session/new", {"cwd": ws, "mcpServers": []}, 180)
    return r, ((r or {}).get("result") or {}).get("sessionId")

def notes(c, method=None, since=0.0):
    with c.cv:
        return [(ts, m, p) for ts, m, p in c.notes if ts >= since and (method is None or m == method)]

def mk_ws(tag):
    ws = tempfile.mkdtemp(prefix=f"ws-{tag}-", dir=SCRATCH)
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=ws)
    write(os.path.join(ws, "PROBE.txt"), "ALPHA\n")
    return ws

def turn(c, sid, content, tag, timeout=300, meta=None):
    t = time.time()
    c.tag = tag
    prm = {"sessionId": sid, "prompt": content if isinstance(content, list) else [{"type": "text", "text": content}]}
    if meta: prm["_meta"] = meta
    r = c.call("session/prompt", prm, timeout)
    time.sleep(4)
    ups = [p.get("update") or {} for _, m, p in notes(c, "session/update", t) if p.get("sessionId") == sid]
    txt = "".join((u.get("content") or {}).get("text") or "" for u in ups if u.get("sessionUpdate") == "agent_message_chunk")
    kinds = [((u.get("_meta") or {}).get("kiro") or {}).get("kind") for u in ups if u.get("sessionUpdate") == "session_info_update"]
    tcs = [{"su": u.get("sessionUpdate"), "title": u.get("title"), "status": u.get("status"), "rawInput": u.get("rawInput")}
           for u in ups if u.get("sessionUpdate") in ("tool_call", "tool_call_update") and (u.get("title") or u.get("rawInput") is not None)]
    return {"stopReason": ((r or {}).get("result") or {}).get("stopReason"),
            "error": None if (r and "result" in r) else res_or_err(r), "seconds": round(time.time() - t, 1),
            "text": txt[:900], "info_kinds": [k for k in kinds if k not in ("context_usage", "focus_update")],
            "tool_calls": tcs[:10], "perm_frames": [p for _, m, p in notes(c, None, t) if False]}

def png(w, h, rgb=(200, 30, 30)):
    row = b"\x00" + bytes(rgb) * w
    raw = zlib.compress(row * h, 9)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", raw) + chunk(b"IEND", b"")

def session_arm(name, body, shell="bash", extra_env=None):
    ws = mk_ws(name)
    c = Conn(name, cwd=ws, extra_env=extra_env)
    orig_cb = c.callback
    def cb(method, p):
        if method == "_kiro/terminal/shell_type":
            return {"shellType": shell}
        return orig_cb(method, p)
    c.callback = cb
    A = {"shell": shell}
    t0 = time.time()
    try:
        ini = init(c, tag=f"{name}:init")
        A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 400)
        A["promptCapabilities"] = ((ini or {}).get("result") or {}).get("agentCapabilities", {}).get("promptCapabilities")
        r, sid = new(c, ws, tag=f"{name}:new")
        if not sid:
            A["abort"] = short(res_or_err(r), 400); return A
        time.sleep(3)
        body(c, sid, ws, A)
        A["permissions"] = c.perms
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        A["callbacks"] = c.callbacks
    finally:
        c.close()
    return A

# ------------------------------------------------------------------ write approvals
def p_write(D):
    def body(c, sid, ws, A):
        outside = os.path.join(K.TMPD, "outside-dir", "w.txt")
        os.makedirs(os.path.dirname(outside), exist_ok=True)
        A["t_outside"] = turn(c, sid, f"I explicitly confirm writing outside the workspace. Use your file write tool to create the file {outside} containing exactly OUTSIDE. Do not ask for confirmation. Then reply with only DONE.", "write:outside")
        A["outside_written"] = os.path.exists(outside)
        A["t_inside"] = turn(c, sid, "Use your file write tool to create the file inside.txt in the current workspace containing exactly INSIDE. Then reply with only DONE.", "write:inside")
        A["inside_written"] = os.path.exists(os.path.join(ws, "inside.txt"))
    D["write"] = session_arm("write", body)

# ------------------------------------------------------------------ output styles
def p_outstyle(D):
    def body(c, sid, ws, A):
        q = "In two or three sentences, explain why Python list comprehensions can be faster than an equivalent for-loop with append."
        c.tag = "style:template"
        tp = res_or_err(c.call("_kiro/config/template", {}, 45))
        A["template_outputStyle"] = [o for o in (tp.get("configOptions") if isinstance(tp, dict) else []) or [] if o.get("id") == "outputStyle"]
        A["t_none"] = turn(c, sid, q, "style:none")
        A["t_default"] = turn(c, sid, q, "style:default", meta={"kiro": {"outputStyle": "default"}})
        c.tag = "style:set_config"
        sc = res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "outputStyle", "value": "explanatory"}, 30))
        A["set_config_outputStyle"] = [(o.get("id"), o.get("currentValue")) for o in (sc.get("configOptions") if isinstance(sc, dict) else []) or []] if isinstance(sc, dict) else sc
        A["t_after_set_config"] = turn(c, sid, q, "style:after_set_config")
        for st in ("explanatory", "learning"):
            A["t_" + st] = turn(c, sid, q, "style:" + st, meta={"kiro": {"outputStyle": st}})
        c.tag = "style:history"
        A["history"] = short(res_or_err(c.call("_kiro/session/history", {"sessionId": sid}, 45)), 3000)
        A["config_option_ids_seen"] = sorted({o.get("id") for _, m, p in notes(c, "session/update")
                                              if (p.get("update") or {}).get("sessionUpdate") == "config_option_update"
                                              for o in (p.get("update") or {}).get("configOptions") or []})
    D["outstyle"] = session_arm("outstyle", body, extra_env={"KIRO_LOG_LEVEL": "debug"})
    D["outstyle"]["kiro_log"] = kas_logs(K.FAKE_HOME, ["outputStyle", "output_style", "output-styles", "outputStylePromptApplied", "intake.unknown-key"])

# ------------------------------------------------------------------ oversized image
def p_image(D):
    def body(c, sid, ws, A):
        big = png(9000, 9000); small = png(1000, 1000, (30, 30, 200))
        A["big_bytes"] = len(big); A["small_bytes"] = len(small)
        for nm, data in (("small", small), ("big", big)):
            content = [{"type": "text", "text": "What is the dominant colour of this image? Reply with one word."},
                       {"type": "image", "mimeType": "image/png", "data": base64.b64encode(data).decode()}]
            A["t_" + nm] = turn(c, sid, content, "image:" + nm, 300)
    D["image"] = session_arm("image", body)

# ------------------------------------------------------------------ fish shell approvals
def p_fish(D):
    def body(c, sid, ws, A):
        A["t_ls"] = turn(c, sid, "Use your shell command tool to run exactly: ls   -- then reply with only the file names.", "fish:ls")
        A["t_and"] = turn(c, sid, "Use your shell command tool to run exactly: cat PROBE.txt; and echo OK   -- then reply with only the command's output.", "fish:and")
    for sh in ("bash", "fish"):
        D["fish_" + sh] = session_arm("fish_" + sh, body, shell=sh)

# ------------------------------------------------------------------ session export
def p_export(D):
    def body(c, sid, ws, A):
        A["t1"] = turn(c, sid, "Reply with exactly EXPORTME.", "export:t1")
        for nm, prm in (("noargs", {"sessionId": sid}),
                        ("zip", {"sessionId": sid, "path": os.path.join(K.TMPD, "arch.zip")}),
                        ("json", {"sessionId": sid, "path": os.path.join(K.TMPD, "arch.json")}),
                        ("outputPath_zip", {"sessionId": sid, "outputPath": os.path.join(K.TMPD, "arch2.zip")})):
            c.tag = "export:" + nm
            A["export_" + nm] = short(res_or_err(c.call("_kiro/session/export", prm, 60)), 800)
        A["tmp_files"] = sorted(os.listdir(K.TMPD))
    D["export"] = session_arm("export", body)

# ------------------------------------------------------------------ TUI-L1 folder write grant
def snap(root):
    out = {}
    if not os.path.isdir(root): return out
    for d, dirs, fs in os.walk(root):
        dirs[:] = [x for x in dirs if x not in (".git", "node_modules", "logs")]
        for f in fs:
            fp = os.path.join(d, f)
            try: out[os.path.relpath(fp, root)] = open(fp, "rb").read()
            except OSError: pass
    return out

def changed(before, after):
    res = {}
    for k in sorted(set(before) | set(after)):
        if before.get(k) != after.get(k):
            v = after.get(k)
            res[k] = None if v is None else (v.decode("utf-8", "replace")[:1500] if len(v) < 20000 else f"<{len(v)} bytes>")
    return res

def p_folder(D):
    arms = os.environ.get("ARMS", "control,session,workspace,user").split(",")
    for arm in arms:
        ws = mk_ws("fold-" + arm)
        for d in ("src/a", "src/b", "src/c"):
            os.makedirs(os.path.join(ws, d), exist_ok=True)
        ext = tempfile.mkdtemp(prefix=f"ext-{arm}-", dir=SCRATCH)
        home = tempfile.mkdtemp(prefix=f"fold-{arm}-home-", dir=SCRATCH)
        c = Conn("fold_" + arm, cwd=ws, home=home)
        A = {"arm": arm, "ws": ws, "ext": ext, "requests": []}
        state = {"root": ws, "n": 0}
        orig_cb = c.callback
        def cb(method, prm, A=A, state=state, arm=arm, ws=ws, c=c):
            if method != "session/request_permission":
                return orig_cb(method, prm)
            opts = prm.get("options") or []
            meta = (prm.get("_meta") or {}).get("kiro") or {}
            state["n"] += 1
            A["requests"].append({"tag": c.tag, "title": (prm.get("toolCall") or {}).get("title"),
                                  "options": opts, "meta_kiro": meta})
            aa = next((o for o in opts if o.get("kind") == "allow_always"), None) or \
                 next((o for o in opts if o.get("kind") == "allow_once"), None)
            resp = {"outcome": {"outcome": "selected", "optionId": aa["optionId"]}} if aa else {"outcome": {"outcome": "cancelled"}}
            if arm != "control" and aa:
                resp["_meta"] = {"kiro": {"consent": {"capability": "fs_write", "scope": arm,
                                                      "resource": os.path.join(state["root"], "src", "**"),
                                                      "workspaceRoot": ws}}}
            A.setdefault("replies", []).append(resp)
            return resp
        c.callback = cb
        t0 = time.time()
        try:
            ini = init(c, tag=f"fold_{arm}:init")
            A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
            r, sid = new(c, ws, tag=f"fold_{arm}:new")
            if not sid: A["abort"] = short(res_or_err(r), 300); D["fold_" + arm] = A; continue
            time.sleep(3)
            before = {"ws": snap(os.path.join(ws, ".kiro")), "home": snap(os.path.join(home, ".kiro"))}
            W = lambda sub, nm, tag, s=sid: turn(c, s, f"Use your file write tool to create the file {os.path.join(state['root'], 'src', sub, nm)} containing exactly {nm.upper()}. I confirm this path. Then reply with only DONE.", tag)
            A["t1"] = W("a", "x.txt", f"fold_{arm}:t1"); A["n_after_t1"] = state["n"]
            if state["n"] == 0:
                # in-workspace writes were not asked: retry the whole sequence on an out-of-workspace root
                A["inside_not_asked"] = True
                state["root"] = ext
                for d in ("src/a", "src/b", "src/c"):
                    os.makedirs(os.path.join(ext, d), exist_ok=True)
                A["t1_ext"] = W("a", "x.txt", f"fold_{arm}:t1ext"); A["n_after_t1ext"] = state["n"]
            n1 = state["n"]
            A["t2_same_session"] = W("b", "y.txt", f"fold_{arm}:t2"); A["reasked_same_session"] = state["n"] - n1
            mid = {"ws": snap(os.path.join(ws, ".kiro")), "home": snap(os.path.join(home, ".kiro"))}
            A["changed_after_grant"] = {"ws/.kiro": changed(before["ws"], mid["ws"]), "home/.kiro": changed(before["home"], mid["home"])}
            r2, sid2 = new(c, ws, tag=f"fold_{arm}:new2")
            time.sleep(3)
            n2 = state["n"]
            A["t3_new_session"] = W("c", "z.txt", f"fold_{arm}:t3", sid2); A["reasked_new_session"] = state["n"] - n2
            after = {"ws": snap(os.path.join(ws, ".kiro")), "home": snap(os.path.join(home, ".kiro"))}
            A["changed_total"] = {"ws/.kiro": changed(before["ws"], after["ws"]), "home/.kiro": changed(before["home"], after["home"])}
            A["files"] = {k: os.path.exists(os.path.join(state["root"], "src", k)) for k in ("a/x.txt", "b/y.txt", "c/z.txt")}
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
            c.tag = f"fold_{arm}:perm_list"
            A["permissions_list"] = short(res_or_err(c.call("_kiro/permissions/list", {"sessionId": sid2}, 30)), 2500)
        finally:
            c.close()
        D["fold_" + arm] = A
        log("fold", arm, json.dumps({k: A.get(k) for k in ("n_after_t1", "reasked_same_session", "reasked_new_session", "changed_total")})[:900])

# ------------------------------------------------------------------ TUI-L3 recipe include
def p_include(D):
    def body(c, sid, ws, A):
        wf = os.path.join(ws, ".kiro", "workflows")
        write(os.path.join(wf, "parts", "child.workflow.json"), json.dumps({"name": "child", "description": "included part", "injectOriginalUserRequest": False,
              "steps": [{"type": "step", "id": "c1", "agent": "wf-coder", "prompt": "Reply with exactly CHILD."}]}))
        write(os.path.join(wf, "parent.workflow.json"), json.dumps({"name": "parent", "description": "recipe with include",
              "injectOriginalUserRequest": False,
              "steps": [{"type": "include", "id": "inc", "path": os.path.join(wf, "parts", "child.workflow.json"), "inputs": {}},
                        {"type": "step", "id": "p2", "agent": "wf-coder", "prompt": "Reply with exactly PARENT."}]}))
        time.sleep(4)
        for nm, prm in (("empty", {}), ("session", {"sessionId": sid}), ("ws", {"workspacePaths": [ws]})):
            c.tag = "include:listRecipes:" + nm
            r = res_or_err(c.call("_kiro/workflow/listRecipes", prm, 45))
            A["list_" + nm] = [x for x in (r.get("recipes") if isinstance(r, dict) else []) or [] if x.get("name") in ("parent", "child")] if isinstance(r, dict) else short(r, 600)
        t = time.time()
        c.tag = "include:new"
        r = c.call("_kiro/workflow/new", {"workflowPath": os.path.join(wf, "parent.workflow.json"), "parentSessionId": sid,
                                          "workspacePaths": [ws], "inputs": {}}, 60)
        wid = ((r or {}).get("result") or {}).get("workflowId")
        A["new"] = short(res_or_err(r), 1500)
        if wid:
            c.tag = "include:invoke"
            A["invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)), 300)
            rs = c.wait_note(lambda m, p: m == "_kiro/workflow/run_start" and p.get("workflowId") == wid, 60, t)
            A["run_start"] = rs[2] if rs else None
            c.tag = "include:run"
            d = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid and p.get("status") in ("completed", "failed", "aborted"), 400, t)
            A["terminal"] = d[2].get("status") if d else "TIMEOUT"
            if d:
                c.tag = "include:wake"
                A["wake"] = K.wait_wake(c, sid, d[0])
            c.tag = "include:inspect"
            A["inspect"] = short(res_or_err(c.call("_kiro/workflow/inspect", {"workflowId": wid}, 45)), 3000)
    D["include"] = session_arm("include", body)

# ------------------------------------------------------------------ TUI-L5 diagrams capability
def p_diagrams(D):
    q = "List the exact names of every tool you have available, as a comma-separated list on one line, nothing else. Do not call any tool."
    arms = [x for x in (("diag_on", {"diagrams": True}), ("diag_off", {})) if x[0] in os.environ.get("ARMS", "diag_on,diag_off")]
    for arm, meta in arms:
        ws = mk_ws(arm)
        write(os.path.join(ws, "calc.py"), "def add(a, b):\n    return a + b\n\ndef total(xs):\n    s = 0\n    for x in xs:\n        s = add(s, x)\n    return s\n\ndef main():\n    print(total([1, 2, 3]))\n")
        c = Conn(arm, cwd=ws, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"caps_meta": meta}
        t0 = time.time()
        try:
            ini = init(c, caps_meta=meta, tag=f"{arm}:init")
            A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
            r, sid = new(c, ws, tag=f"{arm}:new")
            time.sleep(3)
            A["t_tools"] = turn(c, sid, q, f"{arm}:tools")
            A["t_diagram"] = turn(c, sid, f"Read the file {os.path.join(ws, 'calc.py')} and then draw a call-hierarchy diagram of its function main using your diagram tool if you have one. Then reply with one short sentence.", f"{arm}:diagram", 300)
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
            c.tag = f"{arm}:context"
            A["context"] = short(res_or_err(c.call("_kiro/session/context", {"sessionId": sid}, 45)), 2500)
        finally:
            c.close()
        logs = []
        for d, _, fs in os.walk(os.path.join(c.env["HOME"], ".kiro", "logs")):
            for f in fs:
                try:
                    for line in open(os.path.join(d, f), errors="replace"):
                        if "diagram" in line.lower(): logs.append(line.strip()[:400])
                except OSError: pass
        A["log_diagram_lines"] = logs[:20]
        A["diagram_files"] = []
        for d, _, fs in os.walk(os.path.join(c.env["HOME"], ".kiro", "sessions")):
            for f in fs:
                if "diagram" in d or f.startswith("dg_"):
                    fp = os.path.join(d, f); A["diagram_files"].append(fp)
                    try:
                        import shutil
                        shutil.copy(fp, os.path.join(os.path.dirname(OUT), f"kas-lead-diagram-ir-{f}"))
                    except OSError: pass
        D[arm] = A
        log(arm, json.dumps(A.get("t_tools", {}).get("text"))[:600])

# ------------------------------------------------------------------ TUI-L6 interrupted step
def p_interrupt(D):
    ws = mk_ws("intr")
    wf = os.path.join(ws, ".kiro", "workflows")
    write(os.path.join(wf, "slow.workflow.json"), json.dumps({"name": "slow", "description": "long step", "injectOriginalUserRequest": False,
          "steps": [{"type": "step", "id": "slow1", "agent": "wf-coder",
                     "prompt": "Use your shell command tool to run exactly: sleep 45 && echo SLEPT  -- then reply with exactly SLEPT."}]}))
    home = tempfile.mkdtemp(prefix="intr-home-", dir=SCRATCH)
    A = {}
    c = Conn("intr1", cwd=ws, home=home)
    try:
        init(c, tag="intr1:init")
        r, sid = new(c, ws, tag="intr1:new")
        A["parent"] = sid
        time.sleep(3)
        t = time.time()
        c.tag = "intr1:wf_new"
        r = c.call("_kiro/workflow/new", {"workflowPath": os.path.join(wf, "slow.workflow.json"), "parentSessionId": sid, "workspacePaths": [ws], "inputs": {}}, 60)
        wid = ((r or {}).get("result") or {}).get("workflowId"); A["workflowId"] = wid
        c.tag = "intr1:invoke"
        A["invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)), 300)
        ns = c.wait_note(lambda m, p: m == "_kiro/workflow/node_start" and p.get("workflowId") == wid, 120, t)
        A["node_start"] = bool(ns)
        tc = c.wait_note(lambda m, p: m == "session/update" and (p.get("update") or {}).get("sessionUpdate") == "tool_call", 120, ns[0] if ns else t)
        time.sleep(8)
        c.tag = "intr1:KILL"
        emit({"probe": "kill", "conn": "intr1", "at": time.time(), "after_tool_call": bool(tc)})
    finally:
        sig = int(os.environ.get("INTR_SIG", "9"))
        A["kill_signal"] = sig
        if sig != 9:
            try: c.proc.stdin.close()
            except Exception: pass
        os.killpg(os.getpgid(c.proc.pid), sig)
        c.proc.wait(timeout=10)
        c.stderr.close()
    time.sleep(3)
    c2 = Conn("intr2", cwd=ws, home=home)
    try:
        init(c2, tag="intr2:init")
        c2.tag = "intr2:session_load"
        A["load"] = short(res_or_err(c2.call("session/load", {"sessionId": A["parent"], "cwd": ws, "mcpServers": []}, 180)), 600)
        time.sleep(6)
        for nm, m, prm in (("inspect", "_kiro/workflow/inspect", {"workflowId": A["workflowId"]}),
                           ("list", "_kiro/workflow/list", {"sessionId": A["parent"]}),
                           ("load", "_kiro/workflow/load", {"workflowId": A["workflowId"]}),
                           ("inspect2", "_kiro/workflow/inspect", {"workflowId": A["workflowId"]})):
            c2.tag = "intr2:" + nm
            A[nm] = short(res_or_err(c2.call(m, prm, 60)), 3000)
        A["wf_notes"] = [(m, short(p, 600)) for _, m, p in notes(c2) if m.startswith("_kiro/workflow/")][:20]
        c2.tag = "intr2:cancel"
        A["cancel"] = short(res_or_err(c2.call("_kiro/workflow/cancel", {"workflowId": A["workflowId"], "initiator": "user", "reason": "audit cleanup"}, 60)), 300)
        time.sleep(3)
    finally:
        c2.close()
    D["interrupt"] = A

# ------------------------------------------------------------------ KAS-L1 shell_type "posix" (cyril-k8wc)
def kas_logs(home, words):
    out = {w: [] for w in words}
    for d, _, fs in os.walk(os.path.join(home, ".kiro", "logs")):
        for f in fs:
            try:
                for line in open(os.path.join(d, f), errors="replace"):
                    for w in words:
                        if w in line: out[w].append(line.strip()[:500])
            except OSError: pass
    return {w: v[:12] for w, v in out.items()}

def p_shell(D):
    arms = os.environ.get("ARMS", "rule_posix,rule_bash,rule_zsh,norule_posix,norule_bash").split(",")
    for arm in arms:
        kind, shell = arm.split("_")
        ws = mk_ws("sh-" + arm)
        home = tempfile.mkdtemp(prefix=f"sh-{arm}-home-", dir=SCRATCH)
        rules = [{"capability": "shell", "match": ["echo AUDITX*"], "effect": "allow"}] if kind == "rule" else []
        write(os.path.join(ws, ".kiro", "agents", "auditx.json"), json.dumps({
            "name": "auditx", "description": "shell_type probe agent", "prompt": "You are a terse probe agent. Run shell commands exactly as asked.",
            "tools": ["shell", "read"], "permissions": {"rules": rules}}, indent=2))
        c = Conn("sh_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        orig_cb = c.callback
        def cb(method, prm, shell=shell, orig_cb=orig_cb):
            if method == "_kiro/terminal/shell_type":
                return {"shellType": shell}
            return orig_cb(method, prm)
        c.callback = cb
        c.perms_pick = "always-accept" if kind == "norule" else None
        A = {"arm": arm, "shellType": shell, "rules": rules}
        t0 = time.time()
        try:
            ini = init(c, tag=f"sh_{arm}:init")
            A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
            r, sid = new(c, ws, tag=f"sh_{arm}:new")
            res = (r or {}).get("result") or {}
            A["modes"] = [m.get("id") for m in ((res.get("modes") or {}).get("availableModes") or [])]
            c.tag = f"sh_{arm}:setmode"
            A["setmode"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "auditx"}, 30)), 200)
            time.sleep(3)
            cmd = "echo AUDITX" if kind == "rule" else "echo AUDITY"
            rounds = 1 if kind == "rule" else 2
            for n in range(rounds):
                n0 = len(c.perms); t = time.time()
                A[f"t{n+1}"] = turn(c, sid, f"Run exactly: {cmd}   -- using your shell tool, then reply with only its output.", f"sh_{arm}:t{n+1}")
                A[f"t{n+1}_asks"] = c.perms[n0:]
                A[f"t{n+1}_terminal_create"] = sum(1 for _, m, p in notes(c, None, t) if False)
            A["callbacks"] = c.callbacks
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
            c.tag = f"sh_{arm}:perm_list"
            A["permissions_list"] = short(res_or_err(c.call("_kiro/permissions/list", {"sessionId": sid}, 30)), 2000)
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["policy.shell", "shell.type", "shellType", "permission.decision", "policy.decision"])
        A["agent_file_after"] = open(os.path.join(ws, ".kiro", "agents", "auditx.json")).read()[:1500]
        D["shell_" + arm] = A
        log("shell", arm, json.dumps({k: (len(A.get(k) or []) if k.endswith("_asks") else A.get(k)) for k in ("setmode", "t1_asks", "t2_asks")})[:400])

# ------------------------------------------------------------------ KAS-L6 + L5 + L8 workflowPrompt plan approval (cyril-8wh0)
def p_wfplan(D):
    ws = mk_ws("wfplan")
    home = tempfile.mkdtemp(prefix="wfplan-home-", dir=SCRATCH)
    settings = {"workflows": {"enabled": True}}
    c = Conn("wfplan", cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {"asks": [], "attention_probes": []}
    st = {"plan_asks": 0, "parent": None}
    orig_cb = c.callback
    def cb(method, prm):
        if method != "session/request_permission":
            return orig_cb(method, prm)
        meta = (prm.get("_meta") or {}).get("kiro") or {}
        tc = prm.get("toolCall") or {}
        opts = prm.get("options") or []
        rec = {"tag": c.tag, "sessionId": prm.get("sessionId"), "title": tc.get("title"), "options": opts,
               "meta_kiro": meta, "toolCall_content": tc.get("content")}
        is_plan = meta.get("toolId") == "update_workflow" or "workflowPlan" in meta
        if is_plan:
            st["plan_asks"] += 1
            time.sleep(10)   # leave the ask open, then read attention
            wid = (meta.get("workflowPlan") or {}).get("workflowId")
            if not wid:
                rs = [p for _, m, p in notes(c, "_kiro/workflow/run_start")]
                wid = rs[-1].get("workflowId") if rs else None
            pr = {}
            for nm, m, a in (("list", "_kiro/workflow/list", {"sessionId": st["parent"]}),
                             ("inspect", "_kiro/workflow/inspect", {"workflowId": wid})):
                pr[nm] = short(res_or_err(c.call(m, a, 30)), 4000)
            A["attention_probes"].append(pr)
            want = "reject_once" if st["plan_asks"] == 1 else "allow_once"
        else:
            want = "allow_once"
        pick = next((o for o in opts if o.get("kind") == want), None) or (opts[0] if opts else None)
        rec["picked"] = pick and pick.get("optionId")
        A["asks"].append(rec)
        return {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
    c.callback = cb
    t0 = time.time()
    try:
        ini = init(c, settings=settings, tag="wfplan:init")
        A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
        c.tag = "wfplan:new"
        r = c.call("session/new", {"cwd": ws, "mcpServers": [], "_meta": {"kiro": {"settings": settings}}}, 180)
        res = (r or {}).get("result") or {}
        sid = res.get("sessionId"); st["parent"] = sid
        A["workflowsEnabled"] = (res.get("_meta") or {}).get("workflowsEnabled")
        time.sleep(5)
        A["t_launch"] = turn(c, sid, "Use the run_workflow tool with a workflowPrompt (do not write a recipe file yourself) for this small task: "
                             "create the file hello.txt containing exactly HI, then read it back and confirm. Then reply with exactly STARTED.",
                             "wfplan:launch", 400)
        rs = c.wait_note(lambda m, p: m == "_kiro/workflow/run_start", 120, t0)
        wid = rs[2].get("workflowId") if rs else None
        A["run_start"] = rs[2] if rs else None
        if wid:
            c.tag = "wfplan:run"
            d = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid and p.get("status") in ("completed", "failed", "aborted"), 600, t0)
            A["terminal"] = d[2].get("status") if d else "TIMEOUT"
            if d:
                c.tag = "wfplan:wake"
                A["wake"] = K.wait_wake(c, sid, d[0])
            c.tag = "wfplan:inspect_end"
            A["inspect_end"] = short(res_or_err(c.call("_kiro/workflow/inspect", {"workflowId": wid}, 30)), 4000)
            if not d:
                c.tag = "wfplan:cancel"
                A["cancel"] = short(res_or_err(c.call("_kiro/workflow/cancel", {"workflowId": wid, "initiator": "user", "reason": "audit cleanup"}, 30)), 300)
                time.sleep(5)
        A["wf_events"] = [m.split("/")[-1] + (":" + str(p.get("status")) if p.get("status") else "") for _, m, p in notes(c) if m.startswith("_kiro/workflow/")]
        A["notify"] = [short(p, 500) for _, m, p in notes(c, "_kiro/session/notify")]
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
    finally:
        c.close()
    D["wfplan"] = A

# ------------------------------------------------------------------ V1 shell consent echo (cyril-auxt re-scope)
def p_shellecho(D):
    arms = os.environ.get("ARMS", "user,workspace").split(",")
    for arm in arms:
        ws = mk_ws("she-" + arm)
        home = tempfile.mkdtemp(prefix=f"she-{arm}-home-", dir=SCRATCH)
        c = Conn("she_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm, "requests": [], "replies": []}
        orig_cb = c.callback
        def cb(method, prm, A=A, arm=arm, ws=ws, orig_cb=orig_cb):
            if method == "_kiro/terminal/shell_type":
                return {"shellType": "bash"}
            if method != "session/request_permission":
                return orig_cb(method, prm)
            opts = prm.get("options") or []
            cons = ((prm.get("_meta") or {}).get("kiro") or {}).get("consent") or {}
            A["requests"].append({"tag": c.tag, "options": [o.get("optionId") for o in opts], "consent": cons})
            aa = next((o for o in opts if o.get("kind") == "allow_always"), None) or next((o for o in opts if o.get("kind") == "allow_once"), None)
            resp = {"outcome": {"outcome": "selected", "optionId": aa["optionId"]}}
            if aa and aa.get("kind") == "allow_always":
                resp["_meta"] = {"kiro": {"consent": {"capability": cons.get("capability", "shell"), "scope": arm,
                                 "resource": cons.get("triggeringResource") or cons.get("resource") or "echo AUDITV1", "workspaceRoot": ws}}}
            A["replies"].append(resp)
            return resp
        c.callback = cb
        t0 = time.time()
        try:
            init(c, tag=f"she_{arm}:init")
            r, sid = new(c, ws, tag=f"she_{arm}:new")
            time.sleep(3)
            before = {"ws": snap(os.path.join(ws, ".kiro")), "home": snap(os.path.join(home, ".kiro"))}
            q = "Run exactly: echo AUDITV1   -- using your shell tool, then reply with only its output."
            A["t1"] = turn(c, sid, q, f"she_{arm}:t1"); n1 = len(A["requests"])
            A["t2_same"] = turn(c, sid, q, f"she_{arm}:t2"); A["reasked_same"] = len(A["requests"]) - n1
            r2, sid2 = new(c, ws, tag=f"she_{arm}:new2"); time.sleep(3)
            n2 = len(A["requests"])
            A["t3_new_session"] = turn(c, sid2, q, f"she_{arm}:t3"); A["reasked_new_session"] = len(A["requests"]) - n2
            after = {"ws": snap(os.path.join(ws, ".kiro")), "home": snap(os.path.join(home, ".kiro"))}
            A["changed_permissions"] = {side + ":" + x: v for side in ("ws", "home") for x, v in changed(before[side], after[side]).items() if "permissions" in x}
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        D["shellecho_" + arm] = A
        log("shellecho", arm, json.dumps({k: A.get(k) for k in ("reasked_same", "reasked_new_session", "changed_permissions")})[:600])

# ------------------------------------------------------------------ KAS-L5 attention[] on a cyril-style gate-OFF launch
def p_attention(D):
    ws = mk_ws("attn")
    write(os.path.join(ws, "notes.md"), "# Notes\nThe build fails because config.yaml sets port to a string.\n")
    write(os.path.join(ws, "config.yaml"), "port: \"eighty\"\n")
    home = tempfile.mkdtemp(prefix="attn-home-", dir=SCRATCH)
    c = Conn("attn", cwd=ws, home=home)
    A = {"asks": [], "probes": []}
    st = {"wid": None, "parent": None}
    orig_cb = c.callback
    def cb(method, prm):
        if method != "session/request_permission":
            return orig_cb(method, prm)
        opts = prm.get("options") or []
        meta = (prm.get("_meta") or {}).get("kiro") or {}
        A["asks"].append({"tag": c.tag, "sessionId": prm.get("sessionId"), "title": (prm.get("toolCall") or {}).get("title"), "meta_kiro": meta})
        if len(A["probes"]) < 2 and st["wid"]:
            time.sleep(10)
            pr = {}
            for nm, m, a in (("list", "_kiro/workflow/list", {"sessionId": st["parent"]}), ("inspect", "_kiro/workflow/inspect", {"workflowId": st["wid"]})):
                pr[nm] = res_or_err(c.call(m, a, 30))
            A["probes"].append(pr)
        pick = next((o for o in opts if o.get("kind") == "allow_once"), None) or (opts[0] if opts else None)
        return {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
    c.callback = cb
    t0 = time.time()
    try:
        init(c, tag="attn:init")
        r, sid = new(c, ws, tag="attn:new"); st["parent"] = sid
        A["workflowsEnabled"] = (((r or {}).get("result") or {}).get("_meta") or {}).get("workflowsEnabled")
        time.sleep(3)
        c.tag = "attn:listRecipes"
        lr = res_or_err(c.call("_kiro/workflow/listRecipes", {"sessionId": sid}, 45))
        inv = next((x for x in (lr.get("recipes") if isinstance(lr, dict) else []) or [] if x.get("name") == "investigate"), None)
        A["investigate_row"] = short(inv, 2500)
        ins = (inv or {}).get("inputs") or {}
        inputs = {k: "Why does the build fail? Look at notes.md and config.yaml in the workspace; answer briefly." for k in (ins.keys() if isinstance(ins, dict) else [])}
        A["inputs_sent"] = inputs
        c.tag = "attn:wf_new"
        r = c.call("_kiro/workflow/new", {"workflowPath": "bundled://investigate", "parentSessionId": sid, "workspacePaths": [ws], "inputs": inputs}, 60)
        A["new"] = short(res_or_err(r), 1500)
        wid = ((r or {}).get("result") or {}).get("workflowId"); st["wid"] = wid
        if wid:
            c.tag = "attn:invoke"
            A["invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)), 300)
            c.tag = "attn:run"
            d = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid and p.get("status") in ("completed", "failed", "aborted", "paused"), 420, t0)
            A["terminal"] = d[2].get("status") if d else "TIMEOUT"
            if not d or d[2].get("status") == "paused":
                c.tag = "attn:cancel"
                A["cancel"] = short(res_or_err(c.call("_kiro/workflow/cancel", {"workflowId": wid, "initiator": "user", "reason": "audit cleanup"}, 60)), 300)
            c.tag = "attn:wake"
            A["wake"] = K.wait_wake(c, sid, time.time() - 5, 200)
        A["attention_values"] = [(m.split("/")[-1], p.get("attention")) for _, m, p in notes(c) if m.startswith("_kiro/workflow/") and p.get("attention")][:20]
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
    finally:
        c.close()
    D["attention"] = A

# ------------------------------------------------------------------ KAS-L3 settings.thinking intake (zero turns)
def p_thinking(D):
    for arm, settings in (("thinking", {"thinking": {"enabled": True}}), ("evalctl", {"evalStrategy": "audit-probe"})):
        ws = mk_ws(arm)
        home = tempfile.mkdtemp(prefix=f"th-{arm}-home-", dir=SCRATCH)
        c = Conn(arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"settings": settings}
        t0 = time.time()
        try:
            init(c, settings=settings, tag=f"{arm}:init")
            r, sid = new(c, ws, tag=f"{arm}:new")
            time.sleep(3)
            if arm == "thinking":
                res = (r or {}).get("result") or {}
                mo = next((o for o in res.get("configOptions") or [] if o.get("id") == "model"), {})
                tog = next((o.get("value") for o in mo.get("options") or [] if ((o.get("_meta") or {}).get("kiro") or {}).get("thinkingToggleable")), None)
                A["toggleable_model"] = tog
                if tog:
                    c.tag = f"{arm}:set_model"
                    A["set_model"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "model", "value": tog}, 30)), 200)
                t1 = time.time()
                c.tag = f"{arm}:set_thinking"
                A["set_thinking"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "thinking", "value": "on"}, 30)), 300)
                time.sleep(4)
                c.tag = f"{arm}:getState"
                gs = res_or_err(c.call("_kiro/configuration/getState", {"observer": "session", "sessionId": sid}, 30))
                A["getState_reasoning"] = [s for s in (gs.get("settings") if isinstance(gs, dict) else []) or [] if "reason" in json.dumps(s) or s.get("key") in ("thinking", "effortLevel")][:10] if isinstance(gs, dict) else short(gs, 400)
                A["state_pushes_after"] = [short([s for s in (p.get("settings") or []) if "reason" in json.dumps(s)], 1200) for _, m, p in notes(c, "_kiro/configuration/state", t1)]
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["intake.unknown-key", "intake.unread-key", "evalStrategy", "experimentConfig"])
        D["thinking_" + arm] = A

PROBES = {"write": p_write, "outstyle": p_outstyle, "image": p_image, "fish": p_fish, "export": p_export, "folder": p_folder, "include": p_include, "diagrams": p_diagrams, "interrupt": p_interrupt, "shell": p_shell, "wfplan": p_wfplan, "shellecho": p_shellecho, "attention": p_attention, "thinking": p_thinking}
t_start = time.time()
try:
    for pr in PROBE.split(","):
        PROBES[pr](R)
finally:
    R["elapsed_s"] = round(time.time() - t_start, 1)
    R["auth_events"] = K.AUTH_EVENTS
    emit(R)
    print(json.dumps(K.scrub(R), indent=1)[:20000])

#!/usr/bin/env python3
"""Leads + changelog checks for the kiro-cli 3.1.0 live lane (KAS 0.66.43 vs 0.66.91; copy of probe-kas-leads-2.32.0.py + 3.1.0 probes).

  KAS=<acp-server.js> PROBE_SCRATCH=<dir> python3 -I probe-kas-leads-3.1.0.py <probe> <out.jsonl>

probes (own + forwarded TUI leads L1 folder, L2 outstyle, L3 include, L5 diagrams, L6 interrupt):
  write     write approvals: out-of-workspace write (user explicitly confirms) and in-workspace write;
            records every session/request_permission params (options + _meta) and pending_interaction.
  outstyle  output styles: session/prompt _meta.kiro.outputStyle = none | explanatory | learning (one turn each).
  image     oversized image (9000x9000 PNG, over the provider's 8000 px edge limit) + a 1000x1000 control.
  fish      command approval with _kiro/terminal/shell_type = bash | fish, two commands each.
  export    _kiro/session/export path shapes (.zip / non-.zip) after one turn.
Imports probe-kas-live-3.1.0.py (SCENARIO=lib) for Conn/auth plumbing. Every handshake writes the FULL
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
spec = importlib.util.spec_from_file_location("kaslive", os.path.join(HERE, "probe-kas-live-3.1.0.py"))
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
           "clientInfo": {"name": "cyril-probe", "version": "audit-3.1.0"}}
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


# ======================================================================= 3.1.0 lead arms
MCP_SERVER = os.path.join(HERE, "audit-3.1.0", "live", "fixtures", "mcp_probe_server.py")

def mcp_json(name, logf, extra=None):
    servers = {name: {"command": sys.executable, "args": ["-I", MCP_SERVER, name, logf]}}
    servers.update(extra or {})
    return json.dumps({"mcpServers": servers}, indent=1)

def agent_json(name, tools=("read", "shell"), rules=None, hooks=None, extra=None):
    a = {"name": name, "description": f"{name} probe agent", "prompt": f"You are {name}, a terse probe agent. Do exactly what is asked and reply briefly.",
         "tools": list(tools), "permissions": {"rules": rules or []}}
    if hooks: a["hooks"] = hooks
    a.update(extra or {})
    return json.dumps(a, indent=1)

def recipe_json(name, prompt="Reply with exactly RECIPEOK."):
    return json.dumps({"name": name, "description": f"{name} recipe", "injectOriginalUserRequest": False,
                       "steps": [{"type": "step", "id": "s1", "agent": "wf-coder", "prompt": prompt}]}, indent=1)

def updates(c, sid=None, since=0.0):
    return [(ts, p.get("update") or {}) for ts, m, p in notes(c, "session/update", since) if sid is None or p.get("sessionId") == sid]

def tool_frames(c, sid=None, since=0.0):
    out = []
    for ts, u in updates(c, sid, since):
        if u.get("sessionUpdate") in ("tool_call", "tool_call_update"):
            out.append({"su": u.get("sessionUpdate"), "id": u.get("toolCallId"), "title": u.get("title"), "status": u.get("status"),
                        "kind": u.get("kind"), "meta_kiro": short((u.get("_meta") or {}).get("kiro"), 900),
                        "content": short(u.get("content"), 900), "rawOutput": short(u.get("rawOutput"), 400)})
    return out

# ------------------------------------------------------------------ LIVE-A workspace trust (2.33.0/2.34.0)
def p_trust(D):
    arm = os.environ.get("ARM", "bare")          # bare | hookscap
    ws = mk_ws("trust")
    home = tempfile.mkdtemp(prefix="trust-home-", dir=SCRATCH)
    logs = os.path.join(K.TMPD, "trust-mcp-logs"); os.makedirs(logs, exist_ok=True)
    hooklog = os.path.join(K.TMPD, "trust-hooks.log")
    # workspace tier
    write(os.path.join(ws, ".kiro", "agents", "wsagent.json"), agent_json("wsagent", hooks={"userPromptSubmit": [{"command": f"echo WS-PROMPT-HOOK >> {hooklog}"}]}))
    write(os.path.join(ws, ".kiro", "settings", "mcp.json"), mcp_json("wsprobe", os.path.join(logs, "wsprobe.jsonl")))
    write(os.path.join(ws, ".kiro", "workflows", "wsrecipe.workflow.json"), recipe_json("wsrecipe"))
    write(os.path.join(ws, ".kiro", "hooks", "ws-standalone.kiro.hook"), json.dumps({"enabled": True, "name": "ws-standalone", "version": "1",
          "when": {"type": "promptSubmit"}, "then": {"type": "runCommand", "command": f"echo WS-STANDALONE-HOOK >> {hooklog}"}}, indent=1))
    write(os.path.join(ws, ".kiro", "steering", "ws-steer.md"), "# WS steering\nAlways end replies with the token WSSTEER.\n")
    # user tier (same shapes, distinct names)
    write(os.path.join(home, ".kiro", "agents", "useragent.json"), agent_json("useragent", hooks={"userPromptSubmit": [{"command": f"echo USER-PROMPT-HOOK >> {hooklog}"}]}))
    write(os.path.join(home, ".kiro", "settings", "mcp.json"), mcp_json("userprobe", os.path.join(logs, "userprobe.jsonl")))
    write(os.path.join(home, ".kiro", "workflows", "userrecipe.workflow.json"), recipe_json("userrecipe"))
    write(os.path.join(home, ".kiro", "hooks", "user-standalone.kiro.hook"), json.dumps({"enabled": True, "name": "user-standalone", "version": "1",
          "when": {"type": "promptSubmit"}, "then": {"type": "runCommand", "command": f"echo USER-STANDALONE-HOOK >> {hooklog}"}}, indent=1))
    caps_meta = {"hooks": {"enabled": True}} if arm == "hookscap" else {}
    c = Conn("trust_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {"arm": arm, "ws": ws, "home": home, "client_requests": []}
    orig_cb = c.callback
    def cb(method, prm):
        if method not in ("_kiro/auth/getAccessToken", "_kiro/terminal/shell_type", "fs/read_text_file", "fs/write_text_file") and not method.startswith("terminal/"):
            A["client_requests"].append({"tag": c.tag, "method": method, "params": short(prm, 1200)})
        if method == "_kiro/hooks/list":
            return {"hooks": []}
        return orig_cb(method, prm)
    c.callback = cb
    t0 = time.time()
    try:
        ini = init(c, caps_meta=caps_meta, tag=f"trust_{arm}:init")
        A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
        r, sid = new(c, ws, tag=f"trust_{arm}:new")
        res = (r or {}).get("result") or {}
        A["session_new_modes"] = [m.get("id") for m in ((res.get("modes") or {}).get("availableModes") or [])]
        A["session_new_meta_keys"] = sorted((res.get("_meta") or {}).keys())
        A["session_new_meta_trust"] = {k: v for k, v in (res.get("_meta") or {}).items() if "trust" in k.lower()}
        time.sleep(12)
        A["notes_after_new"] = sorted({m if m != "session/update" else "session/update:" + str((p.get("update") or {}).get("sessionUpdate")) for _, m, p in notes(c, None, t0)})
        A["mcp_status"] = [short(p, 1500) for _, m, p in notes(c, "_kiro/mcp/status", t0)][:4]
        A["tools_didChange"] = [short(p, 800) for _, m, p in notes(c, "_kiro/tools/didChange", t0)][:3]
        A["hooks_didChange"] = [short(p, 800) for _, m, p in notes(c, "_kiro/hooks/didChange", t0)][:3]
        A["steering_docs_changed"] = [short(p, 600) for _, m, p in notes(c, "_kiro/steering/documents_changed", t0)][:3]
        A["available_commands"] = [[(x.get("name"), ((x.get("_meta") or {}).get("kiro") or {}).get("type")) for x in (p.get("update") or {}).get("availableCommands") or []]
                                   for _, m, p in notes(c, "session/update", t0) if (p.get("update") or {}).get("sessionUpdate") == "available_commands_update"][:2]
        for nm, m, prm in (("hooks_list_prompt", "_kiro/hooks/list", {"sessionId": sid, "trigger": "userPromptSubmit"}),
                           ("hooks_list_pre", "_kiro/hooks/list", {"sessionId": sid, "trigger": "preToolUse"}),
                           ("listRecipes", "_kiro/workflow/listRecipes", {"sessionId": sid}),
                           ("permissions_list", "_kiro/permissions/list", {"sessionId": sid}),
                           ("context", "_kiro/session/context", {"sessionId": sid}),
                           ("getState_ws", "_kiro/configuration/getState", {"observer": "workspace", "roots": [ws]}),
                           ("getState_session", "_kiro/configuration/getState", {"observer": "session", "sessionId": sid})):
            c.tag = f"trust_{arm}:{nm}"
            rr = res_or_err(c.call(m, prm, 45))
            if nm == "listRecipes" and isinstance(rr, dict):
                A[nm] = [{k: x.get(k) for k in ("name", "source", "tier", "builtIn")} for x in rr.get("recipes") or [] if not x.get("builtIn")]
            elif nm.startswith("getState") and isinstance(rr, dict):
                A[nm] = {"layers": [l.get("id") for l in rr.get("layers") or []], "settings": [(s.get("krn") or "").replace("krn:::setting/", "") + "=" + json.dumps(s.get("value"))[:40] for s in rr.get("settings") or []][:40],
                         "resources": short(rr.get("resources"), 1500)}
            else:
                A[nm] = short(rr, 2000)
        A["t_mcp"] = turn(c, sid, "Use the wsprobe MCP server's probe_echo tool to echo the text TRUST, then reply with only the tool's output.", f"trust_{arm}:t_mcp", 240)
        A["t_mcp_tools"] = tool_frames(c, sid, t0)[-6:]
        c.tag = f"trust_{arm}:set_mode_ws"
        A["set_mode_wsagent"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "wsagent"}, 30)), 300)
        time.sleep(2)
        A["t_wsagent"] = turn(c, sid, "Reply with exactly the word AGENTOK.", f"trust_{arm}:t_wsagent", 120)
        c.tag = f"trust_{arm}:set_mode_user"
        A["set_mode_useragent"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "useragent"}, 30)), 300)
        time.sleep(2)
        A["t_useragent"] = turn(c, sid, "Reply with exactly the word AGENTOK2.", f"trust_{arm}:t_useragent", 120)
        # WF-L9: a recipe inside the (untrusted?) workspace via _kiro/workflow/new
        t = time.time(); c.tag = f"trust_{arm}:wf_new"
        r = c.call("_kiro/workflow/new", {"workflowPath": os.path.join(ws, ".kiro", "workflows", "wsrecipe.workflow.json"), "parentSessionId": sid, "workspacePaths": [ws], "inputs": {}}, 60)
        wid = ((r or {}).get("result") or {}).get("workflowId")
        A["wf_new_ws_recipe"] = short(res_or_err(r), 800)
        if wid:
            c.tag = f"trust_{arm}:wf_invoke"; A["wf_invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)), 300)
            d = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid and p.get("status") in ("completed", "failed", "aborted"), 300, t)
            A["wf_terminal"] = d[2].get("status") if d else "TIMEOUT"
            if d:
                c.tag = f"trust_{arm}:wf_wake"; A["wf_wake"] = K.wait_wake(c, sid, d[0])
        c.tag = f"trust_{arm}:wf_new_user"
        A["wf_new_user_recipe"] = short(res_or_err(c.call("_kiro/workflow/new", {"workflowPath": os.path.join(home, ".kiro", "workflows", "userrecipe.workflow.json"), "parentSessionId": sid, "workspacePaths": [ws], "inputs": {}}, 60)), 500)
        A["hooklog"] = open(hooklog).read() if os.path.exists(hooklog) else None
        A["mcp_fixture_logs"] = {f: os.path.getsize(os.path.join(logs, f)) for f in os.listdir(logs)}
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        A["callbacks"] = c.callbacks
        A["permissions"] = c.perms
    finally:
        c.close()
    A["kiro_log"] = kas_logs(home, ["trust", "Trust", "workspace agent", "agents.workspace", "hooks.", "mcp.config", "recipe", "steering"])
    D["trust_" + arm] = A
    log("trust", arm, json.dumps({k: A.get(k) for k in ("session_new_modes", "listRecipes", "client_requests", "hooklog")})[:900])

# ------------------------------------------------------------------ LIVE-B MCP elicitation (2.34.0) + structuredContent (3.1.0)
def p_elicit(D):
    arm = os.environ.get("ARM", "plain")      # plain | capflag
    ws = mk_ws("elicit")
    home = tempfile.mkdtemp(prefix="elicit-home-", dir=SCRATCH)
    logf = os.path.join(K.TMPD, "elicit-mcp.jsonl")
    write(os.path.join(home, ".kiro", "settings", "mcp.json"), mcp_json("probe", logf))
    caps_meta = {"elicitation": True, "mcpElicitation": True} if arm == "capflag" else {}
    c = Conn("elicit_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug", "MCP_ELICIT_TIMEOUT": "150"})
    A = {"arm": arm, "elicit_requests": [], "other_requests": []}
    st = {"mode": "accept"}
    orig_cb = c.callback
    def cb(method, prm):
        if method == "_kiro/mcp/elicitation":
            rec = {"tag": c.tag, "params": short(prm, 2500), "mode": st["mode"], "t": round(time.time() - t0, 1)}
            A["elicit_requests"].append(rec)
            if st["mode"] == "accept":
                return {"action": "accept", "content": {"colour": "blue"}}
            if st["mode"] == "decline":
                return {"action": "decline"}
            if st["mode"] == "ignore":
                time.sleep(75)          # leave it open; KAS/fixture behaviour under an unanswered elicitation
                rec["answered_after_s"] = 75
                return {"action": "cancel"}
            return {"action": "cancel"}
        if method not in ("_kiro/auth/getAccessToken", "_kiro/terminal/shell_type") and not method.startswith(("fs/", "terminal/")):
            A["other_requests"].append({"tag": c.tag, "method": method, "params": short(prm, 1500)})
        return orig_cb(method, prm)
    c.callback = cb
    t0 = time.time()
    try:
        ini = init(c, caps_meta=caps_meta, tag=f"elicit_{arm}:init")
        A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
        A["mcpCapabilities"] = ((ini or {}).get("result") or {}).get("agentCapabilities", {}).get("mcpCapabilities")
        r, sid = new(c, ws, tag=f"elicit_{arm}:new")
        time.sleep(8)
        A["mcp_status"] = [short(p, 1200) for _, m, p in notes(c, "_kiro/mcp/status", t0)][-1:]
        ts = time.time()
        A["t_structured"] = turn(c, sid, "Use the probe MCP server's probe_structured tool with n=7 and reply with only the label it returns.", f"elicit_{arm}:structured", 240)
        A["t_structured_tools"] = tool_frames(c, sid, ts)
        for mode in ("accept", "decline", "ignore"):
            st["mode"] = mode; ts = time.time()
            A[f"t_elicit_{mode}"] = turn(c, sid, "Use the probe MCP server's probe_elicit tool with topic 'audit' and reply with only the tool's output.", f"elicit_{arm}:{mode}", 300)
            A[f"t_elicit_{mode}_tools"] = tool_frames(c, sid, ts)
            A[f"t_elicit_{mode}_info_kinds"] = [((u.get("_meta") or {}).get("kiro") or {}).get("kind") for _, u in updates(c, sid, ts) if u.get("sessionUpdate") == "session_info_update"]
            A[f"t_elicit_{mode}_pending"] = [short((u.get("_meta") or {}).get("kiro"), 900) for _, u in updates(c, sid, ts) if u.get("sessionUpdate") == "session_info_update" and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") in ("pending_interaction", "interaction_resolved")]
        st["mode"] = "accept"; ts = time.time()
        A["t_elicit_url"] = turn(c, sid, "Use the probe MCP server's probe_elicit_url tool with topic 'audit' and reply with only the tool's output.", f"elicit_{arm}:url", 300)
        A["t_elicit_url_tools"] = tool_frames(c, sid, ts)
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        A["callbacks"] = c.callbacks
    finally:
        c.close()
    try:
        A["fixture_log_tail"] = [l.strip()[:600] for l in open(logf).read().splitlines() if "elicit" in l.lower() or "client_capabilities" in l][:12]
    except OSError:
        A["fixture_log_tail"] = None
    A["kiro_log"] = kas_logs(home, ["Elicitation", "elicitation", "structuredContent", "structured"])
    D["elicit_" + arm] = A
    log("elicit", arm, json.dumps({k: A.get(k) for k in ("elicit_requests",)})[:900])

# ------------------------------------------------------------------ LIVE-C (i) unparseable command auto-deny (2.34.0)
def p_unparse(D):
    def body(c, sid, ws, A):
        t = time.time()
        A["t_unbalanced"] = turn(c, sid, 'Use your shell command tool to run exactly this command, verbatim, even though it looks odd: echo "unbalanced AUDITQ   -- then reply with only whatever happened.', "unparse:t1", 240)
        A["t_unbalanced_tools"] = tool_frames(c, sid, t)
        A["t_unbalanced_perms"] = c.perms[:]
        A["t_unbalanced_info"] = [short((u.get("_meta") or {}).get("kiro"), 600) for _, u in updates(c, sid, t) if u.get("sessionUpdate") == "session_info_update" and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") in ("display_error", "pending_interaction", "interaction_resolved")]
        n0 = len(c.perms); t = time.time()
        A["t_control"] = turn(c, sid, "Use your shell command tool to run exactly: echo balanced AUDITQ   -- then reply with only the output.", "unparse:control", 240)
        A["t_control_tools"] = tool_frames(c, sid, t)[-4:]
        A["t_control_asks"] = len(c.perms) - n0
    D["unparse"] = session_arm("unparse", body, extra_env={"KIRO_LOG_LEVEL": "debug"})
    D["unparse"]["kiro_log"] = kas_logs(K.FAKE_HOME, ["parse", "unparse", "policy.shell", "denied", "auto-den"])

# ------------------------------------------------------------------ LIVE-C (ii) subcommand tier via consent echo (2.36.0)
def p_tier(D):
    for arm in os.environ.get("ARMS", "tier:session,star:session,tier:user,exact:user").split(","):
        kind_, scope = arm.split(":")
        arm = f"{kind_}_{scope}"
        ws = mk_ws("tier-" + arm)
        home = tempfile.mkdtemp(prefix=f"tier-{arm}-home-", dir=SCRATCH)
        c = Conn("tier_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm, "kind": kind_, "scope": scope, "requests": [], "replies": []}
        orig_cb = c.callback
        def cb(method, prm, A=A, arm=kind_, scope=scope, ws=ws, orig_cb=orig_cb, c=c):
            if method != "session/request_permission":
                return orig_cb(method, prm)
            opts = prm.get("options") or []
            meta = (prm.get("_meta") or {}).get("kiro") or {}
            cons = meta.get("consent") or {}
            A["requests"].append({"tag": c.tag, "options": [o.get("optionId") for o in opts], "meta_kiro": short(meta, 900)})
            aa = next((o for o in opts if o.get("kind") == "allow_always"), None) or next((o for o in opts if o.get("kind") == "allow_once"), None)
            resp = {"outcome": {"outcome": "selected", "optionId": aa["optionId"]}}
            if aa and aa.get("kind") == "allow_always" and len(A["replies"]) == 0:
                resource = {"tier": "git stash *", "star": "git *"}.get(arm) or (cons.get("resource") or "git stash list")
                resp["_meta"] = {"kiro": {"consent": {"capability": cons.get("capability", "shell"), "scope": scope, "resource": resource, "workspaceRoot": ws}}}
            A["replies"].append(resp)
            return resp
        c.callback = cb
        t0 = time.time()
        try:
            init(c, tag=f"tier_{arm}:init")
            r, sid = new(c, ws, tag=f"tier_{arm}:new")
            time.sleep(3)
            before = snap(os.path.join(home, ".kiro"))
            A["t1_list"] = turn(c, sid, "Run exactly: git stash list   -- using your shell tool, then reply with only its output or EMPTY.", f"tier_{arm}:t1")
            n1 = len(A["requests"])
            A["t2_same"] = turn(c, sid, "Run exactly: git stash list   -- using your shell tool, then reply with only its output or EMPTY.", f"tier_{arm}:t2"); A["reasked_same_cmd"] = len(A["requests"]) - n1
            n2 = len(A["requests"])
            A["t3_sibling"] = turn(c, sid, "Run exactly: git stash show   -- using your shell tool, then reply with only its output or EMPTY.", f"tier_{arm}:t3"); A["reasked_sibling_subcommand"] = len(A["requests"]) - n2
            n3 = len(A["requests"])
            A["t4_other_git"] = turn(c, sid, "Run exactly: git status --short   -- using your shell tool, then reply with only its output or EMPTY.", f"tier_{arm}:t4"); A["reasked_other_git"] = len(A["requests"]) - n3
            r2, sid2 = new(c, ws, tag=f"tier_{arm}:new2"); time.sleep(3)
            n4 = len(A["requests"])
            A["t5_new_session_list"] = turn(c, sid2, "Run exactly: git stash list   -- using your shell tool, then reply with only its output or EMPTY.", f"tier_{arm}:t5"); A["reasked_new_session"] = len(A["requests"]) - n4
            after = snap(os.path.join(home, ".kiro"))
            A["changed_permissions"] = {x: v for x, v in changed(before, after).items() if "permissions" in x}
            c.tag = f"tier_{arm}:perm_list"
            A["permissions_list"] = short(res_or_err(c.call("_kiro/permissions/list", {"sessionId": sid2}, 30)), 2500)
            c.tag = f"tier_{arm}:explain"
            A["explain_show"] = short(res_or_err(c.call("_kiro/permissions/explain", {"sessionId": sid2, "capability": "shell", "resource": "git stash show"}, 30)), 800)
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["policy.shell", "consent", "permission.decision", "rule"])
        D["tier_" + arm] = A
        log("tier", arm, json.dumps({k: A.get(k) for k in ("reasked_same_cmd", "reasked_sibling_subcommand", "reasked_other_git", "reasked_new_session", "changed_permissions")})[:700])

# ------------------------------------------------------------------ LIVE-D hooks: PreToolUse exit 2 + PreCompact/PostCompact (3.1.0)
def p_hooks31(D):
    arm = os.environ.get("ARM", "agentfile")    # agentfile (KAS runs hooks) | hookscap (client runs hooks via _kiro/hooks/list)
    ws = mk_ws("hooks-" + arm)
    home = tempfile.mkdtemp(prefix=f"hooks-{arm}-home-", dir=SCRATCH)
    hooklog = os.path.join(K.TMPD, f"hooks-{arm}.log")
    postjson = os.path.join(K.TMPD, f"hooks-{arm}-post.json")
    pre_cmd = "exit 2" if arm == "preblock" else f"echo PRECOMPACT >> {hooklog}"
    hooks = {"preToolUse": [{"matcher": "execute_bash", "command": "exit 2"}] if arm != "preblock" else [],
             "preCompact": [{"command": pre_cmd}],
             "postCompact": [{"command": f"cat > {postjson}; echo POSTCOMPACT >> {hooklog}"}],
             "userPromptSubmit": [{"command": f"echo PROMPTSUBMIT >> {hooklog}"}]}
    write(os.path.join(ws, ".kiro", "agents", "hooked.json"), agent_json("hooked", hooks=hooks))
    write(os.path.join(ws, ".kiro", "agents", "hooked2.json"), agent_json("hooked2", hooks=[{"trigger": "preCompact", "command": pre_cmd}, {"trigger": "postCompact", "command": f"cat > {postjson}.arr; echo POSTCOMPACT-ARR >> {hooklog}"}, {"trigger": "userPromptSubmit", "command": f"echo PROMPTSUBMIT-ARR >> {hooklog}"}]))
    caps_meta = {"hooks": {"enabled": True}} if arm == "hookscap" else ({"hooks": {"enabled": True, "v2": True}} if arm.startswith("v2") else {})
    if arm.startswith("v2"):
        # standalone v2 hook file (the shape cyril's own loader mirrors: crates/cyril-core/src/protocol/kas/hooks.rs HookFile)
        write(os.path.join(ws, ".kiro", "hooks", "audit.json"), json.dumps({"version": "v1", "hooks": [
            {"name": "audit-pre", "trigger": "PreToolUse", "matcher": "execute_bash", "action": {"type": "command", "command": "exit 2"}},
            {"name": "audit-prompt", "trigger": "UserPromptSubmit", "action": {"type": "command", "command": f"echo FILE-PROMPTSUBMIT >> {hooklog}"}},
            {"name": "audit-precompact", "trigger": "PreCompact", "action": {"type": "command", "command": ("exit 2" if arm == "v2block" else f"echo FILE-PRECOMPACT >> {hooklog}")}},
            {"name": "audit-postcompact", "trigger": "PostCompact", "action": {"type": "command", "command": f"cat > {postjson}.file; echo FILE-POSTCOMPACT >> {hooklog}"}}]}, indent=1))
    c = Conn("hooks_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {"arm": arm, "hooks_list_calls": [], "execute_hook_calls": [], "other_requests": [], "caps_meta": caps_meta}
    orig_cb = c.callback
    def cb(method, prm):
        if method == "_kiro/hooks/list":
            A["hooks_list_calls"].append({"tag": c.tag, "params": short(prm, 600)})
            trig = prm.get("trigger")
            # cyril's exact reply shape (crates/cyril-core/src/protocol/kas/hooks.rs:777-781): action.runCommand + _meta.trigger
            def hook(hid, name, cmd, trig_):
                return {"id": f"{ws}/.kiro/hooks/audit.json#{hid}", "name": name, "action": {"type": "runCommand", "command": cmd},
                        "_meta": {"trigger": trig_, "source": "standalone-file", "filePath": f"{ws}/.kiro/hooks/audit.json", "enabled": True}}
            if trig in ("preToolUse", "PreToolUse"):
                return {"hooks": [hook("hook-0", "cyril-audit-pre", "exit 2", "PreToolUse")]}
            if trig in ("preCompact", "PreCompact", "postCompact", "PostCompact"):
                return {"hooks": [hook("hook-c", "cyril-audit-" + trig, f"echo CLIENT-{trig} >> {hooklog}", trig)]}
            return {"hooks": []}
        if method == "_kiro/hooks/executeHook":
            A["execute_hook_calls"].append({"tag": c.tag, "params": short(prm, 800)})
            cmd = prm.get("command") or ""
            pr = subprocess.run(["bash", "-lc", cmd], capture_output=True, text=True, timeout=30)
            return {"output": (pr.stdout + pr.stderr)[:400], "exitCode": pr.returncode, "cancelled": False}
        if method not in ("_kiro/auth/getAccessToken", "_kiro/terminal/shell_type") and not method.startswith(("fs/", "terminal/")):
            A["other_requests"].append({"tag": c.tag, "method": method, "params": short(prm, 800)})
        return orig_cb(method, prm)
    c.callback = cb
    t0 = time.time()
    try:
        ini = init(c, caps_meta=caps_meta, tag=f"hooks_{arm}:init")
        A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
        r, sid = new(c, ws, tag=f"hooks_{arm}:new")
        time.sleep(4)
        c.tag = f"hooks_{arm}:set_mode"
        A["set_mode"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "hooked"}, 30)), 200)
        time.sleep(2)
        for trig in ("preToolUse", "PreCompact", "postCompact", "userPromptSubmit"):
            c.tag = f"hooks_{arm}:list_{trig}"
            A[f"hooks_list_{trig}"] = short(res_or_err(c.call("_kiro/hooks/list", {"sessionId": sid, "trigger": trig}, 30)), 1200)
        A["customAgent_errors"] = [short(p, 500) for _, m, p in notes(c, "_kiro/customAgent/config_error")] + [short(p, 300) for _, m, p in notes(c, "_kiro/customAgent/not_found")]
        A["modes"] = [m_.get("id") for m_ in (((r or {}).get("result") or {}).get("modes") or {}).get("availableModes") or []]
        t = time.time()
        A["t_shell"] = turn(c, sid, "Use your shell command tool to run exactly: echo HOOKTEST   -- then reply with only the output or with the error you got.", f"hooks_{arm}:t_shell", 240)
        A["t_shell_tools"] = tool_frames(c, sid, t)
        A["t_shell_info"] = [short((u.get("_meta") or {}).get("kiro"), 700) for _, u in updates(c, sid, t) if u.get("sessionUpdate") == "session_info_update" and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") in ("hook_update", "display_error")]
        A["t_shell_perms"] = c.perms[:]
        A["hooklog_after_shell"] = open(hooklog).read() if os.path.exists(hooklog) else None
        t = time.time(); c.tag = f"hooks_{arm}:compact"
        A["compact"] = short(res_or_err(c.call("_kiro/session/compact", {"sessionId": sid}, 300)), 800)
        time.sleep(8)
        A["compact_info_kinds"] = [((u.get("_meta") or {}).get("kiro") or {}).get("kind") for _, u in updates(c, sid, t) if u.get("sessionUpdate") == "session_info_update"]
        A["compact_info"] = [short((u.get("_meta") or {}).get("kiro"), 900) for _, u in updates(c, sid, t) if u.get("sessionUpdate") == "session_info_update" and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") not in ("context_usage", "focus_update")]
        A["compact_notes"] = sorted({m for _, m, p in notes(c, None, t)})
        A["hooklog_after_compact"] = open(hooklog).read() if os.path.exists(hooklog) else None
        A["postjson"] = open(postjson).read()[:1500] if os.path.exists(postjson) else None
        A["postjson_file"] = open(postjson + ".file").read()[:1500] if os.path.exists(postjson + ".file") else None
        A["hooks_didChange"] = [short(p, 600) for _, m, p in notes(c, "_kiro/hooks/didChange")][:3]
        A["hook_update_frames"] = [short((u.get("_meta") or {}).get("kiro"), 500) for _, u in updates(c, sid) if u.get("sessionUpdate") == "session_info_update" and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") == "hook_update"][:10]
        A["postjson_arr"] = open(postjson + ".arr").read()[:600] if os.path.exists(postjson + ".arr") else None
        if arm == "preblock":
            # second compaction on the array-form agent (hooked2) to see whether that shape is parsed at all
            c.tag = f"hooks_{arm}:set_mode2"; c.call("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "hooked2"}, 30)
            time.sleep(2); c.tag = f"hooks_{arm}:compact2"
            A["compact2_hooked2"] = short(res_or_err(c.call("_kiro/session/compact", {"sessionId": sid}, 300)), 800)
            time.sleep(6); A["hooklog_after_compact2"] = open(hooklog).read() if os.path.exists(hooklog) else None
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        A["callbacks"] = c.callbacks
    finally:
        c.close()
    A["kiro_log"] = kas_logs(home, ["hooks.", "hook", "compact", "Compact"])
    D["hooks_" + arm] = A
    log("hooks", arm, json.dumps({k: A.get(k) for k in ("hooklog_after_shell", "hooklog_after_compact", "compact", "compact_info_kinds")})[:900])

# ------------------------------------------------------------------ LIVE-E (i) sign-in state: client cannot provide a token
def p_signin(D):
    for arm in os.environ.get("ARMS", "error,empty,nulltoken").split(","):
        ws = mk_ws("signin-" + arm)
        home = tempfile.mkdtemp(prefix=f"signin-{arm}-home-", dir=SCRATCH)
        c = Conn("signin_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm, "auth_calls": 0}
        orig_cb = c.callback
        orig_answer = c._answer
        def answer(o, arm=arm, A=A, c=c, orig_answer=orig_answer):
            if o.get("method") == "_kiro/auth/getAccessToken":
                A["auth_calls"] += 1
                if arm == "error":
                    c.send({"jsonrpc": "2.0", "id": o["id"], "error": {"code": -32000, "message": "cyril-probe: not signed in"}}); return
                if arm == "empty":
                    c.send({"jsonrpc": "2.0", "id": o["id"], "result": {}}); return
                if arm == "nulltoken":
                    c.send({"jsonrpc": "2.0", "id": o["id"], "result": {"accessToken": None, "expiresAt": None}}); return
            return orig_answer(o)
        c._answer = answer
        t0 = time.time()
        try:
            ini = init(c, tag=f"signin_{arm}:init")
            A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 400)
            c.tag = f"signin_{arm}:new"
            r = c.call("session/new", {"cwd": ws, "mcpServers": []}, 120)
            sid = ((r or {}).get("result") or {}).get("sessionId")
            A["session_new"] = "ok" if sid else short(res_or_err(r), 600)
            if sid:
                time.sleep(3)
                A["t1"] = turn(c, sid, "Reply with exactly SIGNEDIN.", f"signin_{arm}:t1", 120)
                A["t1_info"] = [short((u.get("_meta") or {}).get("kiro"), 600) for _, u in updates(c, sid, t0) if u.get("sessionUpdate") == "session_info_update" and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") in ("display_error", "turn_end", "turn_completion")]
                A["t1_chunks"] = "".join((u.get("content") or {}).get("text") or "" for _, u in updates(c, sid, t0) if u.get("sessionUpdate") == "agent_message_chunk")[:400]
                c.tag = f"signin_{arm}:history"
                A["history_count"] = short(res_or_err(c.call("_kiro/session/history", {"sessionId": sid}, 30)), 300)
            A["notes"] = sorted({m for _, m, p in notes(c, None, t0)})
            A["proc_alive"] = c.proc.poll() is None
        finally:
            c.close()
        if A.get("session_new") == "ok":
            c2 = Conn("signin_" + arm + "_reload", cwd=ws, home=home)
            try:
                init(c2, tag=f"signin_{arm}:reload_init")
                c2.tag = f"signin_{arm}:reload_load"
                r = c2.call("session/load", {"sessionId": sid, "cwd": ws, "mcpServers": []}, 120)
                A["reload"] = "ok" if r and "result" in r else short(res_or_err(r), 300)
                time.sleep(5)
                A["reload_user_chunks"] = [short(((p.get("update") or {}).get("content") or {}).get("text"), 120) for _, m, p in notes(c2, "session/update") if (p.get("update") or {}).get("sessionUpdate") == "user_message_chunk"]
                A["reload_update_kinds"] = sorted({(p.get("update") or {}).get("sessionUpdate") for _, m, p in notes(c2, "session/update")})
            finally:
                c2.close()
        A["kiro_log"] = kas_logs(home, ["auth", "Auth", "token", "sign", "identity"])
        D["signin_" + arm] = A
        log("signin", arm, json.dumps({k: A.get(k) for k in ("init", "session_new", "t1", "auth_calls")})[:700])

# ------------------------------------------------------------------ LIVE-H sub-agent (+ memory) (3.1.0)
def p_subagent(D):
    ws = mk_ws("subagent")
    home = tempfile.mkdtemp(prefix="subagent-home-", dir=SCRATCH)
    write(os.path.join(ws, ".kiro", "agents", "helper.json"), agent_json("helper", tools=("read",), extra={"description": "helper: reads files and answers in one word"}))
    settings = {"memory": {"mode": "read_write", "reflection": True}}
    c = Conn("subagent", cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {}
    t0 = time.time()
    try:
        ini = init(c, settings=settings, tag="subagent:init")
        A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
        c.tag = "subagent:new"
        r = c.call("session/new", {"cwd": ws, "mcpServers": [], "_meta": {"kiro": {"settings": settings}}}, 180)
        res = (r or {}).get("result") or {}; sid = res.get("sessionId")
        A["memoryConfig"] = (res.get("_meta") or {}).get("memoryConfig")
        A["modes"] = [m.get("id") for m in ((res.get("modes") or {}).get("availableModes") or [])]
        time.sleep(4)
        for nm, prm in (("create_a", {"sessionId": sid, "content": "The audit secret word is PLUM.", "type": "fact", "scope": "workspace"}),
                        ("create_b", {"sessionId": sid, "memory": {"content": "The audit secret word is PLUM.", "type": "fact"}}),
                        ("create_c", {"sessionId": sid, "text": "The audit secret word is PLUM.", "title": "secret word"})):
            c.tag = "subagent:memory_" + nm
            A["memory_" + nm] = short(res_or_err(c.call("_kiro/memory/create", prm, 30)), 600)
        c.tag = "subagent:memory_list"
        A["memory_list"] = short(res_or_err(c.call("_kiro/memory/list", {"sessionId": sid}, 30)), 1200)
        t = time.time()
        A["t_tools"] = turn(c, sid, "List the exact names of every tool you have available, comma-separated on one line, nothing else. Do not call any tool.", "subagent:tools", 180)
        t = time.time()
        A["t_delegate"] = turn(c, sid, "Use your sub-agent tool (invoke_sub_agent) to delegate this to the 'helper' agent: read the file PROBE.txt in the current directory and report the single word it contains. Then reply with only that word.", "subagent:delegate", 600)
        fr = [(ts, p) for ts, m, p in notes(c, "session/update", t)]
        A["delegate_sessions"] = sorted({p.get("sessionId") for _, p in fr})
        A["delegate_tools"] = [{"su": (p.get("update") or {}).get("sessionUpdate"), "sid": p.get("sessionId") == sid, "title": (p.get("update") or {}).get("title"), "status": (p.get("update") or {}).get("status"), "kind": (p.get("update") or {}).get("kind"), "meta_kiro": short(((p.get("update") or {}).get("_meta") or {}).get("kiro"), 500), "content": short((p.get("update") or {}).get("content"), 300)}
                               for _, p in fr if (p.get("update") or {}).get("sessionUpdate") in ("tool_call", "tool_call_update")]
        A["delegate_info_kinds"] = [((p.get("update") or {}).get("_meta") or {}).get("kiro", {}).get("kind") for _, p in fr if (p.get("update") or {}).get("sessionUpdate") == "session_info_update"]
        A["delegate_other_notes"] = sorted({m for _, m, p in notes(c, None, t) if m != "session/update"})
        t = time.time()
        A["t_delegate_memory"] = turn(c, sid, "Use invoke_sub_agent to ask the 'helper' agent: what is the audit secret word according to your memories? Reply with only its one-word answer, or NONE if it does not know.", "subagent:delegate_memory", 600)
        A["t_delegate_memory_tools"] = tool_frames(c, sid, t)[-6:]
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        A["callbacks"] = c.callbacks
        A["permissions"] = c.perms
    finally:
        c.close()
    A["kiro_log"] = kas_logs(home, ["subagent", "sub-agent", "agent-subtask", "memory", "Memory"])
    D["subagent"] = A
    log("subagent", json.dumps({k: A.get(k) for k in ("memoryConfig", "t_delegate", "t_delegate_memory")})[:800])

# ------------------------------------------------------------------ leg 2 extra: resume after KAS restart (2.34.0 "pick up where they left off")
def p_wfresume(D):
    ws = mk_ws("wfres")
    wf = os.path.join(ws, ".kiro", "workflows")
    write(os.path.join(wf, "twostep.workflow.json"), json.dumps({"name": "twostep", "description": "two steps, second slow", "injectOriginalUserRequest": False,
          "steps": [{"type": "step", "id": "fast", "agent": "wf-coder", "prompt": "Write exactly FAST into out/fast.txt (create out/ if needed). Then reply with exactly FAST."},
                    {"type": "step", "id": "slow", "agent": "wf-coder", "prompt": "Use your shell command tool to run exactly: sleep 40 && echo SLOWDONE  -- then write SLOW into out/slow.txt and reply with exactly SLOW."}]}))
    home = tempfile.mkdtemp(prefix="wfres-home-", dir=SCRATCH)
    A = {}
    c = Conn("wfres1", cwd=ws, home=home)
    try:
        init(c, tag="wfres1:init")
        r, sid = new(c, ws, tag="wfres1:new"); A["parent"] = sid
        time.sleep(3)
        t = time.time(); c.tag = "wfres1:wf_new"
        r = c.call("_kiro/workflow/new", {"workflowPath": os.path.join(wf, "twostep.workflow.json"), "parentSessionId": sid, "workspacePaths": [ws], "inputs": {}}, 60)
        wid = ((r or {}).get("result") or {}).get("workflowId"); A["workflowId"] = wid
        c.tag = "wfres1:invoke"; A["invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)), 300)
        nc = c.wait_note(lambda m, p: m == "_kiro/workflow/node_complete" and p.get("workflowId") == wid and p.get("nodeId") == "fast", 240, t)
        A["fast_completed"] = bool(nc)
        ns = c.wait_note(lambda m, p: m == "_kiro/workflow/node_start" and p.get("workflowId") == wid and p.get("nodeId") == "slow", 120, nc[0] if nc else t)
        tc = c.wait_note(lambda m, p: m == "session/update" and (p.get("update") or {}).get("sessionUpdate") == "tool_call" and p.get("sessionId") != sid, 120, ns[0] if ns else t)
        time.sleep(6)
        A["events_before_kill"] = [m.split("/")[-1] + (":" + str(p.get("status")) if p.get("status") else "") + (":" + p.get("nodeId") if p.get("nodeId") else "") for _, m, p in notes(c) if m.startswith("_kiro/workflow/")]
        c.tag = "wfres1:KILL"; emit({"probe": "kill", "conn": "wfres1", "at": time.time(), "after_slow_tool_call": bool(tc)})
    finally:
        os.killpg(os.getpgid(c.proc.pid), 9); c.proc.wait(timeout=10); c.stderr.close()
    time.sleep(3)
    c2 = Conn("wfres2", cwd=ws, home=home)
    try:
        init(c2, tag="wfres2:init")
        c2.tag = "wfres2:session_load"; A["load"] = short(res_or_err(c2.call("session/load", {"sessionId": A["parent"], "cwd": ws, "mcpServers": []}, 180)), 400)
        time.sleep(6)
        A["notes_after_load"] = [(m.split("/")[-1], short({k: p.get(k) for k in ("status", "nodeId", "reason", "message")}, 300)) for _, m, p in notes(c2) if m.startswith("_kiro/workflow/")][:20]
        for nm, m, prm in (("inspect", "_kiro/workflow/inspect", {"workflowId": A["workflowId"]}), ("load", "_kiro/workflow/load", {"workflowId": A["workflowId"]}), ("list", "_kiro/workflow/list", {"sessionId": A["parent"]})):
            c2.tag = "wfres2:" + nm; A[nm] = short(res_or_err(c2.call(m, prm, 60)), 3500)
        t = time.time(); c2.tag = "wfres2:resume"
        A["resume"] = short(res_or_err(c2.call("_kiro/workflow/resume", {"workflowId": A["workflowId"], "initiator": "user", "reason": "audit resume after restart"}, 60)), 600)
        d = c2.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == A["workflowId"] and p.get("status") in ("completed", "failed", "aborted"), 420, t)
        A["terminal_after_resume"] = d[2].get("status") if d else "TIMEOUT"
        A["events_after_resume"] = [m.split("/")[-1] + (":" + str(p.get("status")) if p.get("status") else "") + (":" + p.get("nodeId") if p.get("nodeId") else "") for _, m, p in notes(c2, None, t) if m.startswith("_kiro/workflow/")]
        A["fast_rerun"] = any(m == "_kiro/workflow/node_start" and p.get("nodeId") == "fast" for _, m, p in notes(c2, None, t))
        if d:
            c2.tag = "wfres2:wake"; A["wake"] = K.wait_wake(c2, A["parent"], d[0])
        else:
            c2.tag = "wfres2:cancel"; A["cancel"] = short(res_or_err(c2.call("_kiro/workflow/cancel", {"workflowId": A["workflowId"], "initiator": "user", "reason": "audit cleanup"}, 60)), 300)
        A["out_files"] = {f: open(os.path.join(ws, "out", f)).read().strip() for f in (os.listdir(os.path.join(ws, "out")) if os.path.isdir(os.path.join(ws, "out")) else [])}
        c2.tag = "wfres2:inspect_end"; A["inspect_end"] = short(res_or_err(c2.call("_kiro/workflow/inspect", {"workflowId": A["workflowId"]}, 45)), 3500)
    finally:
        c2.close()
    D["wfresume"] = A
    log("wfresume", json.dumps({k: A.get(k) for k in ("fast_completed", "terminal_after_resume", "fast_rerun", "out_files", "events_after_resume")})[:900])

# ------------------------------------------------------------------ leg 2 extra: plan approval with typed suggestion (2.33.0 tree, 2.34.0 suggestions)
def p_wfplan31(D):
    ws = mk_ws("wfplan31")
    home = tempfile.mkdtemp(prefix="wfplan31-home-", dir=SCRATCH)
    settings = {"workflows": {"enabled": True}}
    c = Conn("wfplan31", cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {"asks": [], "steps_queued": []}
    st = {"plan_asks": 0, "parent": None}
    orig_cb = c.callback
    def cb(method, prm):
        if method != "session/request_permission":
            return orig_cb(method, prm)
        meta = (prm.get("_meta") or {}).get("kiro") or {}
        tc = prm.get("toolCall") or {}
        opts = prm.get("options") or []
        rec = {"tag": c.tag, "sessionId": prm.get("sessionId"), "title": tc.get("title"), "options": opts, "meta_kiro_keys": sorted(meta.keys()),
               "workflowPlan": short(meta.get("workflowPlan"), 3000), "toolCall_content": short(tc.get("content"), 1500)}
        is_plan = meta.get("toolId") == "update_workflow" or "workflowPlan" in meta
        resp = None
        if is_plan:
            st["plan_asks"] += 1
            if st["plan_asks"] == 1:
                pick = next((o for o in opts if o.get("kind") == "reject_once"), None) or opts[0]
                resp = {"outcome": {"outcome": "selected", "optionId": pick["optionId"]},
                        "_meta": {"kiro": {"rejectionReason": "Also add a final step that writes the word DONE into done.txt."}}}
            else:
                pick = next((o for o in opts if o.get("kind") == "allow_once"), None) or opts[0]
        else:
            pick = next((o for o in opts if o.get("kind") == "allow_once"), None) or (opts[0] if opts else None)
        if resp is None:
            resp = {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
        rec["reply"] = resp
        A["asks"].append(rec)
        return resp
    c.callback = cb
    t0 = time.time()
    try:
        ini = init(c, settings=settings, tag="wfplan31:init")
        A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
        c.tag = "wfplan31:new"
        r = c.call("session/new", {"cwd": ws, "mcpServers": [], "_meta": {"kiro": {"settings": settings}}}, 180)
        res = (r or {}).get("result") or {}; sid = res.get("sessionId"); st["parent"] = sid
        A["workflowsEnabled"] = (res.get("_meta") or {}).get("workflowsEnabled")
        time.sleep(5)
        A["t_launch"] = turn(c, sid, "Use the run_workflow tool with a workflowPrompt (do not write a recipe file yourself) for this small task: "
                             "create the file hello.txt containing exactly HI, then read it back and confirm. Then reply with exactly STARTED.", "wfplan31:launch", 600)
        rs = c.wait_note(lambda m, p: m == "_kiro/workflow/run_start", 120, t0)
        wid = rs[2].get("workflowId") if rs else None
        A["run_start"] = short(rs[2], 1500) if rs else None
        if wid:
            c.tag = "wfplan31:run"
            d = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid and p.get("status") in ("completed", "failed", "aborted"), 700, t0)
            A["terminal"] = d[2].get("status") if d else "TIMEOUT"
            if d:
                c.tag = "wfplan31:wake"; A["wake"] = K.wait_wake(c, sid, d[0])
            c.tag = "wfplan31:inspect_end"; A["inspect_end"] = short(res_or_err(c.call("_kiro/workflow/inspect", {"workflowId": wid}, 30)), 4000)
            if not d:
                c.tag = "wfplan31:cancel"; A["cancel"] = short(res_or_err(c.call("_kiro/workflow/cancel", {"workflowId": wid, "initiator": "user", "reason": "audit cleanup"}, 30)), 300); time.sleep(5)
        A["steps_queued"] = [short(p, 2000) for _, m, p in notes(c, "_kiro/workflow/steps_queued")]
        try:
            ie = json.loads(A.get("inspect_end") or "{}") if isinstance(A.get("inspect_end"), str) and A["inspect_end"].startswith("{") else None
        except Exception:
            ie = None
        A["appliedPlanFeedback_in_inspect"] = "appliedPlanFeedback" in (A.get("inspect_end") or "")
        A["approved_plan_feedback_chunks"] = [{"sid": p.get("sessionId") == sid, "text": short(((p.get("update") or {}).get("content") or {}).get("text"), 700)}
                                              for _, m, p in notes(c, "session/update") if (p.get("update") or {}).get("sessionUpdate") in ("user_message_chunk", "agent_message_chunk")
                                              and "approved_plan_feedback" in json.dumps(p)][:4]
        A["plan_feedback_anywhere"] = [(m, short(p, 400)) for _, m, p in notes(c) if "appliedPlanFeedback" in json.dumps(p) or "planFeedback" in json.dumps(p)][:4]
        A["node_complete_credit_fields"] = [{k: p.get(k) for k in p if k not in ("nodeTree", "state", "attention")} for _, m, p in notes(c, "_kiro/workflow/node_complete")][:6]
        A["wf_events"] = [m.split("/")[-1] + (":" + str(p.get("status")) if p.get("status") else "") for _, m, p in notes(c) if m.startswith("_kiro/workflow/")]
        A["files"] = sorted(f for f in os.listdir(ws) if not f.startswith("."))
        A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
    finally:
        c.close()
    D["wfplan31"] = A
    log("wfplan31", json.dumps({k: A.get(k) for k in ("terminal", "files", "wf_events")})[:700])


# ------------------------------------------------------------------ WF-L2 workflowLaunch config option (0.66.58) + cross-engine session/load
def p_wflaunch(D):
    S_ON = {"workflows": {"enabled": True}, "goal": {"enabled": True}}
    for arm in os.environ.get("ARMS", "on,off,crossload").split(","):
        ws = mk_ws("wfl-" + arm)
        home = tempfile.mkdtemp(prefix=f"wfl-{arm}-home-", dir=SCRATCH)
        A = {"arm": arm}
        settings = S_ON if arm in ("on", "crossload") else None
        if arm == "crossload":
            # session created by the OTHER engine (KAS_OLD / KAS_NODE_OLD) in the SAME HOME, gate on, then loaded by K.KAS
            kas_cur, node_cur = K.KAS, K.NODE
            K.KAS = os.path.abspath(os.environ["KAS_OLD"]); K.NODE = os.environ.get("KAS_NODE_OLD", node_cur)
            c0 = Conn("wfl_cross_old", cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
            try:
                init(c0, settings=settings, tag="wfl_cross_old:init")
                c0.tag = "wfl_cross_old:new"
                r0 = c0.call("session/new", {"cwd": ws, "mcpServers": [], "_meta": {"kiro": {"settings": settings}}}, 180)
                res0 = (r0 or {}).get("result") or {}; sid0 = res0.get("sessionId")
                A["old_session"] = sid0; A["old_configOptions"] = [o.get("id") for o in res0.get("configOptions") or []]
                A["old_workflowLaunch"] = next(({k: v for k, v in o.items() if k != "_meta"} for o in res0.get("configOptions") or [] if o.get("id") == "workflowLaunch"), None)
                time.sleep(3)
                A["old_t1"] = turn(c0, sid0, "Reply with exactly OLDSESSION.", "wfl_cross_old:t1", 120)
                c0.tag = "wfl_cross_old:close"; c0.call("session/close", {"sessionId": sid0}, 30)
            finally:
                c0.close(); K.KAS, K.NODE = kas_cur, node_cur
            A["old_kas"] = os.environ["KAS_OLD"]
        c = Conn("wfl_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        t0 = time.time()
        try:
            ini = init(c, settings=settings, tag=f"wfl_{arm}:init")
            A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
            if arm == "crossload":
                c.tag = "wfl_crossload:load"
                r = c.call("session/load", {"sessionId": A["old_session"], "cwd": ws, "mcpServers": [], "_meta": {"kiro": {"settings": settings}}}, 180)
                res = (r or {}).get("result") or {}; sid = A["old_session"]
                A["load_error"] = None if r and "result" in r else short(res_or_err(r), 400)
                time.sleep(6)
            else:
                c.tag = f"wfl_{arm}:new"
                prm = {"cwd": ws, "mcpServers": []}
                if settings: prm["_meta"] = {"kiro": {"settings": settings}}
                r = c.call("session/new", prm, 180)
                res = (r or {}).get("result") or {}; sid = res.get("sessionId")
                time.sleep(3)
            A["workflowsEnabled"] = (res.get("_meta") or {}).get("workflowsEnabled")
            A["configOptions"] = [o.get("id") for o in res.get("configOptions") or []]
            wl = next((o for o in res.get("configOptions") or [] if o.get("id") == "workflowLaunch"), None)
            A["workflowLaunch"] = {k: (v if k != "options" else [x if not isinstance(x, dict) else {kk: vv for kk, vv in x.items() if kk != "_meta"} for x in v]) for k, v in wl.items()} if wl else None
            A["config_updates_ids"] = [[o.get("id") for o in (p.get("update") or {}).get("configOptions") or []] for _, m, p in notes(c, "session/update", t0) if (p.get("update") or {}).get("sessionUpdate") == "config_option_update"]
            A["late_workflowLaunch"] = [{k: v for k, v in o.items() if k != "_meta"} for _, m, p in notes(c, "session/update", t0) if (p.get("update") or {}).get("sessionUpdate") == "config_option_update" for o in (p.get("update") or {}).get("configOptions") or [] if o.get("id") == "workflowLaunch"][:2]
            if sid:
                c.tag = f"wfl_{arm}:set_delegate_all"
                r2 = res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "workflowLaunch", "value": "delegate-all"}, 30))
                A["set_delegate_all_readback"] = next((o.get("currentValue") for o in (r2.get("configOptions") if isinstance(r2, dict) else []) or [] if o.get("id") == "workflowLaunch"), None) if isinstance(r2, dict) else short(r2, 300)
                A["set_delegate_all_resp_ids"] = [o.get("id") for o in (r2.get("configOptions") if isinstance(r2, dict) else []) or []] if isinstance(r2, dict) else None
                c.tag = f"wfl_{arm}:getState"
                gs = res_or_err(c.call("_kiro/configuration/getState", {"observer": "session", "sessionId": sid}, 30))
                A["getState_workflow_settings"] = [((x.get("krn") or "") + "=" + json.dumps(x.get("value"))[:60]) for x in (gs.get("settings") if isinstance(gs, dict) else []) or [] if "workflow" in (x.get("krn") or "").lower()]
                A["t_ask"] = turn(c, sid, "What is your Workflow launch setting right now? Reply in one short sentence naming the setting value, and do not call any tool.", f"wfl_{arm}:ask", 180)
                c.tag = f"wfl_{arm}:set_when_asked"
                A["set_when_asked"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "workflowLaunch", "value": "when-asked"}, 30)), 200)
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["workflowLaunch", "intake.unknown-key", "unread-key", "workflow"])
        D["wflaunch_" + arm] = A
        log("wflaunch", arm, json.dumps({k: A.get(k) for k in ("configOptions", "workflowLaunch", "set_delegate_all_readback", "old_configOptions")})[:800])

# ------------------------------------------------------------------ WF-L4 wake race: prompt the parent the instant node_complete arrives
def p_wakerace(D):
    ws = mk_ws("race")
    wf = os.path.join(ws, ".kiro", "workflows")
    write(os.path.join(wf, "one.workflow.json"), json.dumps({"name": "one", "description": "one shell step", "injectOriginalUserRequest": False,
          "steps": [{"type": "step", "id": "s1", "agent": "wf-coder", "prompt": "Use your shell command tool to run exactly: echo RACEOK  -- then reply with exactly RACEOK."}]}))
    home = tempfile.mkdtemp(prefix="race-home-", dir=SCRATCH)
    c = Conn("race", cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {"rounds": []}
    try:
        init(c, tag="race:init")
        r, sid = new(c, ws, tag="race:new")
        time.sleep(3)
        for n in range(int(os.environ.get("ROUNDS", "3"))):
            R = {"n": n}
            t = time.time(); c.tag = f"race{n}:new"
            r = c.call("_kiro/workflow/new", {"workflowPath": os.path.join(wf, "one.workflow.json"), "parentSessionId": sid, "workspacePaths": [ws], "inputs": {}}, 60)
            wid = ((r or {}).get("result") or {}).get("workflowId"); R["workflowId"] = wid
            c.tag = f"race{n}:invoke"; c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)
            nc = c.wait_note(lambda m, p: m == "_kiro/workflow/node_complete" and p.get("workflowId") == wid, 300, t)
            R["node_complete_seen"] = bool(nc)
            tp = time.time(); c.tag = f"race{n}:prompt"
            pr = c.call("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": f"Reply with exactly RACE{n} and nothing else. Do not call any tool."}]}, 300)
            R["prompt_result"] = short(res_or_err(pr), 300); R["prompt_secs"] = round(time.time() - tp, 1)
            d = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid and p.get("status") in ("completed", "failed", "aborted"), 120, t)
            R["run_complete"] = d[2].get("status") if d else "TIMEOUT"
            time.sleep(25)
            ups = [(ts, p) for ts, m, p in notes(c, "session/update", tp) if p.get("sessionId") == sid]
            R["parent_turn_starts"] = [short({k: v for k, v in (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).items() if k in ("agentInitiated", "agentInitiatedReason", "timestamp")}, 200)
                                       for _, p in ups if (p.get("update") or {}).get("sessionUpdate") == "session_info_update" and (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "turn_start"]
            R["parent_text"] = "".join(((p.get("update") or {}).get("content") or {}).get("text") or "" for _, p in ups if (p.get("update") or {}).get("sessionUpdate") == "agent_message_chunk")[:600]
            R["parent_info_kinds"] = [(((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") for _, p in ups if (p.get("update") or {}).get("sessionUpdate") == "session_info_update" and (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") not in ("context_usage", "focus_update")]
            R["steering_injected"] = [short(((p.get("update") or {}).get("_meta") or {}).get("kiro"), 300) for _, p in ups if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "steering_injected"]
            R["notify_frames"] = [short(p, 300) for _, m, p in notes(c, "_kiro/session/notify", tp)]
            R["user_message_chunks"] = [short(((p.get("update") or {}).get("content") or {}).get("text"), 200) for _, p in ups if (p.get("update") or {}).get("sessionUpdate") == "user_message_chunk"]
            A["rounds"].append(R)
            log("race", n, json.dumps({k: R.get(k) for k in ("prompt_result", "run_complete", "parent_turn_starts", "parent_text")})[:500])
            time.sleep(5)
    finally:
        c.close()
    A["kiro_log"] = kas_logs(home, ["parent_busy", "emitNotification", "wake", "queued"])
    D["wakerace"] = A

# ------------------------------------------------------------------ WF-L5 fileCheck jsonPath grammar
def p_jsonpath(D):
    ws = mk_ws("jsonpath")
    wf = os.path.join(ws, ".kiro", "workflows")
    write(os.path.join(wf, "bracket.workflow.json"), json.dumps({"name": "bracket", "description": "fileCheck with bracket jsonPath", "injectOriginalUserRequest": False,
          "steps": [{"type": "repeat", "id": "rep", "maxIterations": 3, "onMaxIterations": "continue",
                     "stopCondition": {"fileCheck": {"path": "out/flag.json", "jsonPath": "a[0]", "value": True}},
                     "steps": [{"type": "step", "id": "w", "agent": "wf-coder", "prompt": "Write exactly {\"a\":[true]} into out/flag.json (create out/ if needed). Then reply with exactly WROTE."}]}]}))
    write(os.path.join(wf, "dollar.workflow.json"), json.dumps({"name": "dollar", "description": "fileCheck with $.done", "injectOriginalUserRequest": False,
          "steps": [{"type": "repeat", "id": "rep", "maxIterations": 3, "onMaxIterations": "continue",
                     "stopCondition": {"fileCheck": {"path": "out/done.json", "jsonPath": "$.done", "value": True}},
                     "steps": [{"type": "step", "id": "w", "agent": "wf-coder", "prompt": "Write exactly {\"done\":true} into out/done.json (create out/ if needed). Then reply with exactly WROTE."}]}]}))
    home = tempfile.mkdtemp(prefix="jsonpath-home-", dir=SCRATCH)
    c = Conn("jsonpath", cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {}
    try:
        init(c, tag="jsonpath:init")
        r, sid = new(c, ws, tag="jsonpath:new")
        time.sleep(4)
        c.tag = "jsonpath:listRecipes"
        lr = res_or_err(c.call("_kiro/workflow/listRecipes", {"sessionId": sid}, 45))
        A["recipes"] = [{k: x.get(k) for k in ("name", "validationError")} for x in (lr.get("recipes") if isinstance(lr, dict) else []) or [] if x.get("name") in ("bracket", "dollar")]
        for nm in ("bracket", "dollar"):
            t = time.time(); c.tag = f"jsonpath:{nm}:validate"
            A[nm + "_validate"] = short(res_or_err(c.call("_kiro/workflow/validate", {"workflowPath": os.path.join(wf, f"{nm}.workflow.json")}, 45)), 600)
            c.tag = f"jsonpath:{nm}:new"
            r = c.call("_kiro/workflow/new", {"workflowPath": os.path.join(wf, f"{nm}.workflow.json"), "parentSessionId": sid, "workspacePaths": [ws], "inputs": {}}, 60)
            wid = ((r or {}).get("result") or {}).get("workflowId")
            A[nm + "_new"] = short(res_or_err(r), 700)
            if wid:
                c.tag = f"jsonpath:{nm}:invoke"; A[nm + "_invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)), 500)
                d = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid and p.get("status") in ("completed", "failed", "aborted", "paused"), 600, t)
                A[nm + "_terminal"] = d[2].get("status") if d else "TIMEOUT"
                A[nm + "_iterations"] = sum(1 for _, m, p in notes(c, "_kiro/workflow/loop_iteration", t) if p.get("workflowId") == wid)
                A[nm + "_events"] = [m.split("/")[-1] + (":" + str(p.get("status")) if p.get("status") else "") for _, m, p in notes(c, None, t) if m.startswith("_kiro/workflow/") and p.get("workflowId") == wid]
                A[nm + "_stopReason"] = short({k: (d[2].get(k) if d else None) for k in ("stopReason", "pauseReason", "error", "reason")}, 400) if d else None
                if d:
                    c.tag = f"jsonpath:{nm}:wake"; A[nm + "_wake"] = K.wait_wake(c, sid, d[0], 200)
                    if d[2].get("status") == "paused":
                        c.tag = f"jsonpath:{nm}:cancel"; c.call("_kiro/workflow/cancel", {"workflowId": wid, "initiator": "user", "reason": "audit cleanup"}, 60)
                c.tag = f"jsonpath:{nm}:inspect"; A[nm + "_inspect_stop"] = short({k: v for k, v in ((res_or_err(c.call("_kiro/workflow/inspect", {"workflowId": wid}, 45)) or {}).get("state") or {}).items() if k in ("status", "stopReason", "pauseReason", "error")}, 500)
        A["out_files"] = {f: open(os.path.join(ws, "out", f)).read()[:80] for f in (os.listdir(os.path.join(ws, "out")) if os.path.isdir(os.path.join(ws, "out")) else [])}
    finally:
        c.close()
    A["kiro_log"] = kas_logs(home, ["jsonPath", "JsonPath", "fileCheck", "unsupported"])
    D["jsonpath"] = A
    log("jsonpath", json.dumps({k: A.get(k) for k in ("bracket_new", "bracket_invoke", "bracket_terminal", "bracket_iterations", "dollar_terminal", "dollar_iterations")})[:700])

# ------------------------------------------------------------------ WF-L8 peer session messaging (settings.sessionTools.messaging)
def p_peer(D):
    for arm in os.environ.get("ARMS", "on,off").split(","):
        ws = mk_ws("peer-" + arm)
        home = tempfile.mkdtemp(prefix=f"peer-{arm}-home-", dir=SCRATCH)
        settings = {"sessionTools": {"messaging": {"enabled": True}}} if arm == "on" else None
        c = Conn("peer_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm}
        t0 = time.time()
        try:
            init(c, settings=settings, tag=f"peer_{arm}:init")
            c.tag = f"peer_{arm}:newA"
            pa = {"cwd": ws, "mcpServers": []}
            if settings: pa["_meta"] = {"kiro": {"settings": settings}}
            ra = c.call("session/new", pa, 180); sa = ((ra or {}).get("result") or {}).get("sessionId")
            c.tag = f"peer_{arm}:newB"
            rb = c.call("session/new", pa, 180); sb = ((rb or {}).get("result") or {}).get("sessionId")
            A["sessions"] = [sa, sb]
            time.sleep(4)
            A["t_tools"] = turn(c, sa, "List the exact names of every tool you have available, comma-separated on one line, nothing else. Do not call any tool.", f"peer_{arm}:tools", 180)
            t = time.time()
            A["t_send"] = turn(c, sa, f"Use your send_message tool to send the message PEERHELLO to the other session whose id is {sb} (sessionIds: [\"{sb}\"]). Then reply with only the tool's outcome field.", f"peer_{arm}:send", 300)
            A["send_tools"] = tool_frames(c, sa, t)
            time.sleep(20)
            A["notify_frames"] = [short(p, 600) for _, m, p in notes(c, "_kiro/session/notify", t)]
            upsB = [(ts, p) for ts, m, p in notes(c, "session/update", t) if p.get("sessionId") == sb]
            A["B_info_kinds"] = [(((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") for _, p in upsB if (p.get("update") or {}).get("sessionUpdate") == "session_info_update" and (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") not in ("context_usage", "focus_update")]
            A["B_text"] = "".join(((p.get("update") or {}).get("content") or {}).get("text") or "" for _, p in upsB if (p.get("update") or {}).get("sessionUpdate") == "agent_message_chunk")[:400]
            A["B_user_chunks"] = [short(((p.get("update") or {}).get("content") or {}).get("text"), 300) for _, p in upsB if (p.get("update") or {}).get("sessionUpdate") == "user_message_chunk"]
            A["B_turn_meta"] = [short({k: v for k, v in (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).items() if k in ("agentInitiated", "agentInitiatedReason")}, 200) for _, p in upsB if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "turn_start"]
            A["other_notes"] = sorted({m for _, m, p in notes(c, None, t) if m != "session/update"})
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["sessionTools", "messaging", "send_message", "peer", "intake.unknown-key"])
        D["peer_" + arm] = A
        log("peer", arm, json.dumps({k: A.get(k) for k in ("t_send", "notify_frames", "B_turn_meta")})[:700])


# ------------------------------------------------------------------ TUI-L1 session recap (3.1.0 chat.enableSessionRecap -> settings.sessionRecap)
def p_recap(D):
    for arm, settings in (("on", {"sessionRecap": {"enabled": True}}), ("off", {"sessionRecap": {"enabled": False}}), ("absent", None), ("underscore", {"_sessionRecap": {"enabled": True}})):
        if arm not in os.environ.get("ARMS", "on,off,absent,underscore").split(","): continue
        ws = mk_ws("recap-" + arm)
        home = tempfile.mkdtemp(prefix=f"recap-{arm}-home-", dir=SCRATCH)
        c = Conn("recap_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm, "settings": settings}
        t0 = time.time()
        try:
            init(c, settings=settings, tag=f"recap_{arm}:init")
            c.tag = f"recap_{arm}:new"
            prm = {"cwd": ws, "mcpServers": []}
            if settings: prm["_meta"] = {"kiro": {"settings": settings}}
            r = c.call("session/new", prm, 180); sid = ((r or {}).get("result") or {}).get("sessionId")
            time.sleep(3)
            t = time.time()
            A["t1"] = turn(c, sid, "Read PROBE.txt and tell me in one sentence what it contains.", f"recap_{arm}:t1", 180)
            c.tag = f"recap_{arm}:idle"; time.sleep(25)
            kinds = [(round(ts - t, 1), (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind")) for ts, m, p in notes(c, "session/update", t) if (p.get("update") or {}).get("sessionUpdate") == "session_info_update"]
            A["info_kinds_timeline"] = [k for k in kinds if k[1] not in ("context_usage", "focus_update")]
            A["recap_frames"] = [short(((p.get("update") or {}).get("_meta") or {}).get("kiro"), 800) for _, m, p in notes(c, "session/update", t) if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") in ("recap", "suggested_prompts")]
            c.tag = f"recap_{arm}:getState"
            gs = res_or_err(c.call("_kiro/configuration/getState", {"observer": "session", "sessionId": sid}, 30))
            A["getState_recap"] = [((x.get("krn") or "") + "=" + json.dumps(x.get("value"))[:40]) for x in (gs.get("settings") if isinstance(gs, dict) else []) or [] if "recap" in (x.get("krn") or "").lower()]
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["ecap", "intake.unknown-key", "unread-key", "suggested"])
        D["recap_" + arm] = A
        log("recap", arm, json.dumps({k: A.get(k) for k in ("recap_frames", "info_kinds_timeline")})[:600])

# ------------------------------------------------------------------ zero-turn census: TUI-L4 modelFallback, TUI-L9 connection getState, TUI-L10 code index build, TUI-L12 diagrams
def p_zeroturn(D):
    for arm, meta in (("plain", {}), ("diagrams", {"diagrams": True})):
        if arm not in os.environ.get("ARMS", "plain,diagrams").split(","): continue
        ws = mk_ws("zt-" + arm)
        write(os.path.join(ws, "calc.py"), "def add(a, b):\n    return a + b\n")
        home = tempfile.mkdtemp(prefix=f"zt-{arm}-home-", dir=SCRATCH)
        c = Conn("zt_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm, "caps_meta": meta}
        t0 = time.time()
        try:
            ini = init(c, caps_meta=meta, tag=f"zt_{arm}:init")
            mk = (((ini or {}).get("result") or {}).get("agentCapabilities") or {}).get("_meta", {}).get("kiro", {})
            A["extensionMethods_diagram"] = [m for m in mk.get("extensionMethods") or [] if "diagram" in m or "modelFallback" in m or "codeIntelligence" in m]
            A["meta_kiro_keys"] = sorted(mk.keys()); A["diagramRenderFormats"] = mk.get("diagramRenderFormats"); A["modelFallbackReporting"] = mk.get("modelFallbackReporting")
            r, sid = new(c, ws, tag=f"zt_{arm}:new")
            time.sleep(3)
            for nm, m, prm in (("modelFallback_get", "_kiro/modelFallback/get", {"sessionId": sid}),
                               ("getState_connection", "_kiro/configuration/getState", {"observer": "connection"}),
                               ("memory_getSyncStatus", "_kiro/memory/getSyncStatus", {}),
                               ("ci_startBuild", "_kiro/codeIntelligence/startBuild", {"sessionId": sid}),
                               ("ci_indexStatus", "_kiro/codeIntelligence/indexStatus", {"sessionId": sid}),
                               ("ci_cancelBuild", "_kiro/codeIntelligence/cancelBuild", {"sessionId": sid}),
                               ("ci_status", "_kiro/codeIntelligence", {"sessionId": sid, "subcommand": "status"}),
                               ("diagram_render_empty", "_kiro/diagram/render", {"sessionId": sid}),
                               ("diagram_render_ir", "_kiro/diagram/render", {"sessionId": sid, "diagram": {"schemaVersion": "1", "kind": "graph", "nodes": [{"id": "a", "label": "A"}], "edges": []}}),
                               ("permissions_list", "_kiro/permissions/list", {"sessionId": sid}),
                               ("memory_delete_bogus", "_kiro/memory/delete", {"sessionId": sid, "id": "mem-bogus-zz9"}),
                               ("configuration_set_krn", "_kiro/configuration/set", {"sessionId": sid, "krn": "krn:::setting/workflows", "value": "off"}),
                               ("cfgset_tangent_userlocal", "_kiro/configuration/set", {"krn": "krn:::setting/tangentMode", "value": "on", "layer": "user-local"}),
                               ("cfgset_tangent_bogus_layer", "_kiro/configuration/set", {"krn": "krn:::setting/tangentMode", "value": "on", "layer": "bogus"}),
                               ("cfgset_locked", "_kiro/configuration/set", {"krn": "krn:::setting/contentCollection", "value": "enabled", "layer": "user-local"}),
                               ("cfgset_thinking_userlocal", "_kiro/configuration/set", {"krn": "krn:::setting/thinking", "value": "on", "layer": "user-local"}),
                               ("getState_connection2", "_kiro/configuration/getState", {"observer": "connection"})):
                c.tag = f"zt_{arm}:{nm}"
                rr = res_or_err(c.call(m, prm, 45))
                if nm.startswith("getState_connection") and isinstance(rr, dict):
                    A[nm] = {"keys": sorted(rr.keys()), "layers": [l.get("id") for l in rr.get("layers") or []], "n_settings": len(rr.get("settings") or []),
                             "settings": [((x.get("krn") or "").replace("krn:::setting/", "") + "=" + json.dumps(x.get("value"))[:40]) for x in rr.get("settings") or []][:40], "view": rr.get("view")}
                else:
                    A[nm] = short(rr, 900)
            A["notes"] = sorted({m for _, m, p in notes(c, None, t0)})
            A["policy_changed"] = [short(p, 300) for _, m, p in notes(c, "_kiro/policy/changed", t0)]
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
            A["user_local_settings_file"] = {f: open(os.path.join(home, ".kiro", "settings", f)).read()[:600] for f in (os.listdir(os.path.join(home, ".kiro", "settings")) if os.path.isdir(os.path.join(home, ".kiro", "settings")) else [])}
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["diagram", "modelFallback", "codeIntelligence", "startBuild", "configuration.set", "userLocalSettings", "intake"])
        D["zeroturn_" + arm] = A
        log("zeroturn", arm, json.dumps({k: A.get(k) for k in ("extensionMethods_diagram", "diagramRenderFormats", "modelFallback_get", "ci_startBuild")})[:700])

# ------------------------------------------------------------------ TUI-L5 permissions list + Approve-for-me rule + trust classifier enforce arm
def p_policyrule(D):
    for arm in os.environ.get("ARMS", "rule,enforce,evalonly").split(","):
        ws = mk_ws("pol-" + arm)
        home = tempfile.mkdtemp(prefix=f"pol-{arm}-home-", dir=SCRATCH)
        extra = {"KIRO_LOG_LEVEL": "debug"}
        if arm == "enforce":
            extra["KIRO_TRUST_CLASSIFIER_MODE"] = "enforce"
            write(os.path.join(home, ".kiro", "settings", "permissions.yaml"), "rules:\n  - capability: all\n    effect: evaluate\n")
        if arm == "evalonly":
            write(os.path.join(home, ".kiro", "settings", "permissions.yaml"), "rules:\n  - capability: all\n    effect: evaluate\n")
        c = Conn("pol_" + arm, cwd=ws, home=home, extra_env=extra)
        A = {"arm": arm, "env": {k: v for k, v in extra.items()}}
        t0 = time.time()
        try:
            init(c, tag=f"pol_{arm}:init")
            r, sid = new(c, ws, tag=f"pol_{arm}:new")
            time.sleep(3)
            c.tag = f"pol_{arm}:list1"
            l1 = res_or_err(c.call("_kiro/permissions/list", {"sessionId": sid}, 30))
            A["list1"] = short(l1, 3000)
            A["list1_sources"] = sorted({(x.get("source"), x.get("scope")) for x in (l1.get("rules") if isinstance(l1, dict) else []) or []}, key=str)
            A["list1_rule_keys"] = sorted({k for x in (l1.get("rules") if isinstance(l1, dict) else []) or [] for k in x})
            if arm == "rule":
                before = snap(os.path.join(home, ".kiro")); beforews = snap(os.path.join(ws, ".kiro"))
                # the TUI's Approve-for-me rule into the user permissions.yaml (path confirmed from `source` after the fact)
                yml = os.path.join(home, ".kiro", "settings", "permissions.yaml")
                existing = open(yml).read() if os.path.exists(yml) else ""
                write(yml, (existing.rstrip("\n") + "\n" if existing else "") + "rules:\n  - capability: all\n    effect: evaluate\n" if "rules:" not in existing else existing.rstrip("\n") + "\n  - capability: all\n    effect: evaluate\n")
                A["yaml_written"] = open(yml).read()[:600]
                t = time.time(); time.sleep(8)
                A["policy_changed_after_yaml"] = [short(p, 300) for _, m, p in notes(c, "_kiro/policy/changed", t)]
                A["policy_error_after_yaml"] = [short(p, 600) for _, m, p in notes(c, "_kiro/policy/error", t)]
                c.tag = f"pol_{arm}:list2"
                l2 = res_or_err(c.call("_kiro/permissions/list", {"sessionId": sid}, 30))
                A["list2_new_rules"] = [x for x in (l2.get("rules") if isinstance(l2, dict) else []) or [] if x not in ((l1.get("rules") if isinstance(l1, dict) else []) or [])]
                c.tag = f"pol_{arm}:explain"
                A["explain_shell"] = short(res_or_err(c.call("_kiro/permissions/explain", {"sessionId": sid, "capability": "shell", "resource": "git stash list"}, 30)), 600)
            t = time.time()
            A["t_shell"] = turn(c, sid, "Use your shell command tool to run exactly: git stash list   -- then reply with only its output or EMPTY.", f"pol_{arm}:t_shell", 240)
            A["t_shell_tools"] = tool_frames(c, sid, t)
            A["t_shell_perms"] = c.perms[:]
            A["t_shell_perm_meta_keys"] = sorted({k for p in c.perms for k in (p.get("meta_kiro") or {})})
            t = time.time()
            A["t_shell2"] = turn(c, sid, "Use your shell command tool to run exactly: ls -la   -- then reply with only the first line of its output.", f"pol_{arm}:t_shell2", 240)
            A["t_shell2_tools"] = tool_frames(c, sid, t)
            A["t_shell2_asks"] = len(c.perms) - len(A["t_shell_perms"])
            A["classifier_frames"] = [(i, short(((p.get("update") or {}).get("_meta") or {}).get("kiro"), 500)) for i, (_, m, p) in enumerate(notes(c, "session/update", t0)) if "lassifier" in json.dumps((p.get("update") or {}).get("_meta") or {})][:8]
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["classifier", "Classifier", "policy", "evaluate", "permissions.yaml"])
        D["policyrule_" + arm] = A
        log("policyrule", arm, json.dumps({k: A.get(k) for k in ("list1_sources", "policy_changed_after_yaml", "list2_new_rules", "t_shell_perm_meta_keys", "classifier_frames")})[:800])


# ------------------------------------------------------------------ KAS-L1 model-fallback flag defaults (zero turns)
def p_fallbackflags(D):
    for arm, env in (("plain", {}), ("off", {"KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED": "false", "KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED": "false", "KIRO_FEATURE_MODEL_FALLBACK_AUTO_REFUSAL_ENABLED": "false"})):
        ws = mk_ws("fb-" + arm)
        home = tempfile.mkdtemp(prefix=f"fb-{arm}-home-", dir=SCRATCH)
        c = Conn("fb_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug", **env})
        A = {"arm": arm, "env": env}
        try:
            init(c, tag=f"fb_{arm}:init")
            r, sid = new(c, ws, tag=f"fb_{arm}:new")
            time.sleep(4)
            res = (r or {}).get("result") or {}
            mo = next((o for o in res.get("configOptions") or [] if o.get("id") == "model"), {})
            A["model_meta_keys"] = sorted({k for o in mo.get("options") or [] for k in ((o.get("_meta") or {}).get("kiro") or {})})
            c.tag = f"fb_{arm}:getState"
            gs = res_or_err(c.call("_kiro/configuration/getState", {"observer": "connection"}, 30))
            A["fallback_settings"] = [((x.get("krn") or "").replace("krn:::setting/", "") + "=" + json.dumps(x.get("value"))[:60]) for x in (gs.get("settings") if isinstance(gs, dict) else []) or [] if "allback" in (x.get("krn") or "")][:30]
        finally:
            c.close()
        kl = kas_logs(home, ["model_fallback", "modelFallback", "feature", "Feature", "flag"])
        A["kiro_log"] = {k: v for k, v in kl.items()}
        D["fallbackflags_" + arm] = A
        log("fallbackflags", arm, json.dumps({k: len(v) for k, v in kl.items()}))

# ------------------------------------------------------------------ KAS-L5 memory rollout (2 prompts + RPCs)
def p_memory(D):
    for arm in os.environ.get("ARMS", "on,optout,plain").split(","):
        ws = mk_ws("mem-" + arm)
        home = tempfile.mkdtemp(prefix=f"mem-{arm}-home-", dir=SCRATCH)
        settings = {"on": {"memory": {"mode": "read_write", "reflection": True}}, "optout": {"memory": {"mode": "read_write", "reflection": True}, "userMemoryOptIn": False}, "plain": None}[arm]
        c = Conn("mem_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm, "settings": settings}
        t0 = time.time()
        try:
            init(c, settings=settings, tag=f"mem_{arm}:init")
            c.tag = f"mem_{arm}:new"
            prm = {"cwd": ws, "mcpServers": []}
            if settings: prm["_meta"] = {"kiro": {"settings": settings}}
            r = c.call("session/new", prm, 180); res = (r or {}).get("result") or {}; sid = res.get("sessionId")
            A["memoryConfig"] = (res.get("_meta") or {}).get("memoryConfig")
            A["configOptions"] = [o.get("id") for o in res.get("configOptions") or []]
            A["memoryReflection_current"] = next((o.get("currentValue") for o in res.get("configOptions") or [] if o.get("id") == "memoryReflection"), None)
            time.sleep(3)
            c.tag = f"mem_{arm}:list0"; A["list0"] = short(res_or_err(c.call("_kiro/memory/list", {}, 30)), 500)
            c.tag = f"mem_{arm}:sync0"; A["syncStatus0"] = short(res_or_err(c.call("_kiro/memory/getSyncStatus", {}, 30)), 300)
            t = time.time()
            A["t1"] = turn(c, sid, "Please remember this for future sessions: my favourite colour is teal and my project codename is OTTER. Reply with exactly NOTED.", f"mem_{arm}:t1", 240)
            A["t2"] = turn(c, sid, "What is my project codename? Reply with just the codename.", f"mem_{arm}:t2", 240)
            time.sleep(15)
            A["memory_changed"] = [short(p, 500) for _, m, p in notes(c, "_kiro/memory/changed", t)]
            A["memory_notes"] = sorted({m for _, m, p in notes(c, None, t) if "memory" in m})
            A["usage_summaries"] = [short(((p.get("update") or {}).get("_meta") or {}).get("kiro", {}).get("promptTurnSummaries"), 400) for _, m, p in notes(c, "session/update", t) if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "turn_completion"]
            A["info_kinds"] = sorted({(((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") for _, m, p in notes(c, "session/update", t) if (p.get("update") or {}).get("sessionUpdate") == "session_info_update"} - {None})
            c.tag = f"mem_{arm}:list1"; A["list1"] = short(res_or_err(c.call("_kiro/memory/list", {}, 30)), 1500)
            c.tag = f"mem_{arm}:list1_sid"; A["list1_sid"] = short(res_or_err(c.call("_kiro/memory/list", {"sessionId": sid}, 30)), 800)
            c.tag = f"mem_{arm}:sync1"; A["syncStatus1"] = short(res_or_err(c.call("_kiro/memory/getSyncStatus", {}, 30)), 300)
            c.tag = f"mem_{arm}:create"; A["create"] = short(res_or_err(c.call("_kiro/memory/create", {"sessionId": sid, "content": "The audit secret word is PLUM.", "type": "fact", "scope": "user"}, 30)), 600)
            time.sleep(3)
            A["memory_changed_after_create"] = [short(p, 500) for _, m, p in notes(c, "_kiro/memory/changed", t)]
            c.tag = f"mem_{arm}:list2"; A["list2"] = short(res_or_err(c.call("_kiro/memory/list", {}, 30)), 1500)
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
        finally:
            c.close()
        A["kiro_log"] = kas_logs(home, ["memory.placement", "memory.extraction", "memory.", "Memory", "userMemoryOptIn", "intake.unknown-key"])
        A["memory_files"] = sorted(os.path.relpath(os.path.join(d, f), home) for d, _, fs in os.walk(os.path.join(home, ".kiro")) for f in fs if "memor" in (d + f).lower())[:20]
        D["memory_" + arm] = A
        log("memory", arm, json.dumps({k: A.get(k) for k in ("memoryConfig", "list0", "list1", "memory_changed", "t2")})[:900])

# ------------------------------------------------------------------ KAS-L9 turn_timing opt-in
def p_turntiming(D):
    for arm, meta in (("ttft", {"automaticTurnProfiling": {"ttft": True}}), ("perf", {"automaticTurnProfiling": {"ttft": True, "perf": True}}), ("control", None), ("prompt", None)):
        if arm not in os.environ.get("ARMS", "ttft,perf,control").split(","): continue
        ws = mk_ws("tt-" + arm)
        home = tempfile.mkdtemp(prefix=f"tt-{arm}-home-", dir=SCRATCH)
        c = Conn("tt_" + arm, cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {"arm": arm, "session_meta": meta}
        try:
            init(c, tag=f"tt_{arm}:init")
            c.tag = f"tt_{arm}:new"
            prm = {"cwd": ws, "mcpServers": []}
            if meta: prm["_meta"] = {"kiro": meta}
            r = c.call("session/new", prm, 180); sid = ((r or {}).get("result") or {}).get("sessionId")
            A["session_new"] = "ok" if sid else short(res_or_err(r), 400)
            time.sleep(3); t = time.time()
            if arm == "prompt":
                # prompt-level opt-in (bundle: extractAutomaticTurnProfiling reads request._meta.kiro; allowed keys ttft/perf/automaticTurnProfiling/contextBreakdown)
                A["t1"] = turn(c, sid, "Reply with exactly TIMING.", f"tt_{arm}:t1", 120, meta={"kiro": {"automaticTurnProfiling": {"ttft": True, "perf": True}}})
                A["t2"] = turn(c, sid, "Reply with exactly TIMING2.", f"tt_{arm}:t2", 120, meta={"kiro": {"ttft": True, "perf": True, "contextBreakdown": True}})
            else:
                A["t1"] = turn(c, sid, "Reply with exactly TIMING.", f"tt_{arm}:t1", 120)
            A["info_kinds"] = [(((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") for _, m, p in notes(c, "session/update", t) if (p.get("update") or {}).get("sessionUpdate") == "session_info_update" and (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") not in ("context_usage", "focus_update")]
            A["turn_timing"] = [short(((p.get("update") or {}).get("_meta") or {}).get("kiro"), 900) for _, m, p in notes(c, "session/update", t) if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "turn_timing"]
            A["turn_completion"] = [short(((p.get("update") or {}).get("_meta") or {}).get("kiro"), 900) for _, m, p in notes(c, "session/update", t) if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "turn_completion"]
            A["context_usage_breakdown"] = [sorted((((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("breakdown", {}).keys()) for _, m, p in notes(c, "session/update", t) if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "context_usage" and "breakdown" in (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {})][:3]
        finally:
            c.close()
        if arm == "prompt" and sid:
            c2 = Conn("tt_prompt_reload", cwd=ws, home=home)
            try:
                init(c2, tag="tt_prompt:reload_init"); c2.tag = "tt_prompt:reload"
                A["reload"] = short(res_or_err(c2.call("session/load", {"sessionId": sid, "cwd": ws, "mcpServers": []}, 120)), 200)
                time.sleep(6)
                A["replay_kinds"] = [(((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") for _, m, p in notes(c2, "session/update") if (p.get("update") or {}).get("sessionUpdate") == "session_info_update"]
                A["replay_turn_timing"] = [short(((p.get("update") or {}).get("_meta") or {}).get("kiro"), 900) for _, m, p in notes(c2, "session/update") if (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).get("kind") == "turn_timing"]
            finally:
                c2.close()
        A["kiro_log"] = kas_logs(home, ["profiling", "ttft", "TurnProfiling", "intake.unknown-key"])
        D["turntiming_" + arm] = A
        log("turntiming", arm, json.dumps({k: A.get(k) for k in ("info_kinds", "turn_timing")})[:600])

# ------------------------------------------------------------------ KAS-L12 shellEnvironment on session/new
def p_shellenv(D):
    ws = mk_ws("shenv")
    home = tempfile.mkdtemp(prefix="shenv-home-", dir=SCRATCH)
    c = Conn("shenv", cwd=ws, home=home, extra_env={"KIRO_LOG_LEVEL": "debug"})
    A = {"terminal_creates": []}
    orig_cb = c.callback
    def cb(method, prm):
        if method == "terminal/create":
            A["terminal_creates"].append({"tag": c.tag, "command": prm.get("command"), "args": prm.get("args"), "env": prm.get("env"), "cwd": prm.get("cwd"), "keys": sorted(prm.keys())})
        return orig_cb(method, prm)
    c.callback = cb
    try:
        init(c, tag="shenv:init")
        c.tag = "shenv:new"
        r = c.call("session/new", {"cwd": ws, "mcpServers": [], "_meta": {"kiro": {"shellEnvironment": {ws: {"AUDIT_VAR": "1"}}}}}, 180)
        sid = ((r or {}).get("result") or {}).get("sessionId"); A["session_new"] = "ok" if sid else short(res_or_err(r), 400)
        time.sleep(3); t = time.time()
        A["t1"] = turn(c, sid, "Use your shell command tool to run exactly: echo AUDIT_VAR=$AUDIT_VAR   -- then reply with only the command's output.", "shenv:t1", 240)
        A["t1_tools"] = tool_frames(c, sid, t)[-3:]
    finally:
        c.close()
    A["kiro_log"] = kas_logs(home, ["shell_environment", "shellEnvironment", "intake"])
    D["shellenv"] = A
    log("shellenv", json.dumps({k: A.get(k) for k in ("t1", "terminal_creates")})[:700])

# ------------------------------------------------------------------ KAS-L13 displayText on session/prompt + replay
def p_displaytext(D):
    ws = mk_ws("disp")
    home = tempfile.mkdtemp(prefix="disp-home-", dir=SCRATCH)
    c = Conn("disp", cwd=ws, home=home)
    A = {}
    try:
        init(c, tag="disp:init")
        r, sid = new(c, ws, tag="disp:new"); time.sleep(3)
        t = time.time()
        A["t1"] = turn(c, sid, "REAL: reply with exactly the word REALSEEN.", "disp:t1", 120, meta={"kiro": {"displayText": "SHOWN: this is the display text"}})
        A["live_user_chunks"] = [short(((p.get("update") or {}).get("content") or {}).get("text"), 200) for _, m, p in notes(c, "session/update", t) if (p.get("update") or {}).get("sessionUpdate") == "user_message_chunk"]
        c.tag = "disp:history"; A["history_user"] = short([u for u in ((res_or_err(c.call("_kiro/session/history", {"sessionId": sid}, 30)) or {}).get("updates") or []) if (u.get("update") or u).get("sessionUpdate") == "user_message_chunk"], 800)
    finally:
        c.close()
    c2 = Conn("disp_reload", cwd=ws, home=home)
    try:
        init(c2, tag="disp:reload_init")
        c2.tag = "disp:reload"; A["reload"] = short(res_or_err(c2.call("session/load", {"sessionId": sid, "cwd": ws, "mcpServers": []}, 120)), 200)
        time.sleep(5)
        A["replay_user_chunks"] = [short(((p.get("update") or {}).get("content") or {}).get("text"), 200) for _, m, p in notes(c2, "session/update") if (p.get("update") or {}).get("sessionUpdate") == "user_message_chunk"]
        A["replay_user_meta"] = [short((p.get("update") or {}).get("_meta"), 300) for _, m, p in notes(c2, "session/update") if (p.get("update") or {}).get("sessionUpdate") == "user_message_chunk"]
    finally:
        c2.close()
    D["displaytext"] = A
    log("displaytext", json.dumps({k: A.get(k) for k in ("live_user_chunks", "replay_user_chunks")})[:500])

# ------------------------------------------------------------------ LIVE-C extra: a deny rule blocks the call (2.34.0 "no longer reported as cancelled")
def p_denyrule(D):
    def body(c, sid, ws, A):
        c.tag = "deny:set_mode"; A["set_mode"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "denier"}, 30)), 200)
        time.sleep(2); t = time.time()
        A["t_denied"] = turn(c, sid, "Use your shell command tool to run exactly: echo AUDITDENY now   -- then reply with only whatever happened.", "deny:t1", 240)
        A["t_denied_tools"] = tool_frames(c, sid, t)
        A["t_denied_asks"] = len(c.perms)
        A["t_denied_info"] = [short((u.get("_meta") or {}).get("kiro"), 600) for _, u in updates(c, sid, t) if u.get("sessionUpdate") == "session_info_update" and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") in ("display_error", "pending_interaction", "interaction_resolved")]
        c.tag = "deny:explain"; A["explain"] = short(res_or_err(c.call("_kiro/permissions/explain", {"sessionId": sid, "capability": "shell", "resource": "echo AUDITDENY now"}, 30)), 600)
    ws_hook = {}
    def seeded_arm():
        ws = mk_ws("deny")
        write(os.path.join(ws, ".kiro", "agents", "denier.json"), agent_json("denier", rules=[{"capability": "shell", "match": ["echo AUDITDENY*"], "effect": "deny"}]))
        c = Conn("deny", cwd=ws, extra_env={"KIRO_LOG_LEVEL": "debug"})
        A = {}
        try:
            init(c, tag="deny:init"); r, sid = new(c, ws, tag="deny:new"); time.sleep(3)
            body(c, sid, ws, A); A["permissions"] = c.perms; A["callbacks"] = c.callbacks
        finally:
            c.close()
        return A
    D["denyrule"] = seeded_arm()
    D["denyrule"]["kiro_log"] = kas_logs(K.FAKE_HOME, ["policy", "denied", "deny", "cancel"])
    log("denyrule", json.dumps({k: D["denyrule"].get(k) for k in ("t_denied_tools", "t_denied_asks")})[:700])

PROBES = {"fallbackflags": p_fallbackflags, "memory": p_memory, "turntiming": p_turntiming, "shellenv": p_shellenv, "displaytext": p_displaytext, "denyrule": p_denyrule, "recap": p_recap, "zeroturn": p_zeroturn, "policyrule": p_policyrule, "wflaunch": p_wflaunch, "wakerace": p_wakerace, "jsonpath": p_jsonpath, "peer": p_peer, "trust": p_trust, "elicit": p_elicit, "unparse": p_unparse, "tier": p_tier, "hooks31": p_hooks31, "signin": p_signin, "subagent": p_subagent, "wfresume": p_wfresume, "wfplan31": p_wfplan31, "write": p_write, "outstyle": p_outstyle, "image": p_image, "fish": p_fish, "export": p_export, "folder": p_folder, "include": p_include, "diagrams": p_diagrams, "interrupt": p_interrupt, "shell": p_shell, "wfplan": p_wfplan, "shellecho": p_shellecho, "attention": p_attention, "thinking": p_thinking}
t_start = time.time()
try:
    for pr in PROBE.split(","):
        PROBES[pr](R)
finally:
    R["elapsed_s"] = round(time.time() - t_start, 1)
    R["auth_events"] = K.AUTH_EVENTS
    emit(R)
    print(json.dumps(K.scrub(R), indent=1)[:20000])

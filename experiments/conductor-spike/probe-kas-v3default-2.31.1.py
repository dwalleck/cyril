#!/usr/bin/env python3
"""v3-default checks for the kiro-cli 2.31.1 live lane (orchestrator refocus, cyril-4c4d).

  KAS=<acp-server.js> PROBE_SCRATCH=<dir> python3 -I probe-kas-v3default-2.31.1.py <probe> <out.jsonl>

probes:
  handshake    A+C: cyril-exact KAS initialize (as built by crates/cyril-core/src/protocol/engine.rs
               client_capabilities() + kas/settings.rs marshal_agent_settings()) vs the TUI 2.31.1
               reference (`$tt(oT())` clientMeta) vs a positive-control arm; KIRO_LOG_LEVEL=debug;
               records intake unknown-key lines, getState(session) settings contributions per layer,
               session/new _meta.memoryConfig + memoryReflection.
  fallback     B: 2.29.1 recipe F0 control bogus model / F1 both fallback flags + bogus / F2 flags + auto;
               records every session_info_update kind and any model_fallback / model_fallback_promoted.
  sessionload  D: session/load replay. Arms: (a) load the leg-1 sweep session in its own HOME
               (SWEEP_HOME/SWEEP_SID env), (b) abandoned prompt: start a turn, SIGKILL KAS 3 s in,
               respawn same HOME, session/load; records replayed frames.
"""
import importlib.util, json, os, signal, subprocess, sys, tempfile, time
PROBE = sys.argv[1]
OUT = sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["SCENARIO"] = "lib"
os.environ.setdefault("LABEL", f"v3d-{PROBE}")
sys.argv = [sys.argv[0], OUT]
spec = importlib.util.spec_from_file_location("kaslive", os.path.join(HERE, "probe-kas-live-2.31.1.py"))
K = importlib.util.module_from_spec(spec); spec.loader.exec_module(K)
Conn, emit, res_or_err, short, log = K.Conn, K.emit, K.res_or_err, K.short, K.log
SCRATCH = K.SCRATCH
R = {"probe": "lead-result", "lead": PROBE, "kas_version": K.SUMMARY.get("kas_version"), "kas": K.KAS}

def mk_ws(tag):
    ws = tempfile.mkdtemp(prefix=f"ws-{tag}-", dir=SCRATCH)
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=ws)
    open(os.path.join(ws, "PROBE.txt"), "w").write("ALPHA\n")
    return ws

def notes(c, method=None, since=0.0):
    with c.cv:
        return [(ts, m, p) for ts, m, p in c.notes if ts >= since and (method is None or m == method)]

def stderr_lines(c, words):
    try: txt = open(c.stderr.name, encoding="utf-8", errors="replace").read()
    except OSError: return []
    return [l[:500] for l in txt.splitlines() if any(w in l for w in words)]

# ---- handshake arms ---------------------------------------------------------------
FS_META = {"kiro": {"readFile": True, "writeFile": True, "stat": True, "readDirectory": True, "delete": True}}
def cyril_caps(settings):
    # engine.rs client_capabilities(): fs(+_meta.kiro FS_OPS flags) + terminal, _meta.kiro{settings, hooks}
    # KasHooksMode default = Host -> hooks {enabled:true}; no configurationState / telemetryEnabled.
    return {"fs": {"readTextFile": True, "writeTextFile": True, "_meta": FS_META}, "terminal": True,
            "_meta": {"kiro": {"settings": settings, "hooks": {"enabled": True}}}}
CYRIL_INFO = {"name": "cyril", "version": "0.2.0-alpha.1", "title": "Cyril"}
# marshal_agent_settings() of the user's real ~/.kiro/settings/cli.json (flat keys chat.enableThinking,
# chat.enableTodoList, toolSearch.enabled/minPct/minTokens; nested "chat" object is NOT read by cyril)
CYRIL_USER = {"thinking": {"enabled": True}, "todoList": {"enabled": True},
              "codeIntelligence": {"enabled": True}, "knowledge": {"enabled": True},
              "subagentOrchestration": {"enabled": True},
              "toolSearch": {"enabled": True, "minPct": 0, "minTokens": 0}}
# marshal_agent_settings() of an empty cli.json (DEFAULTS_ON only)
CYRIL_EMPTY = {"codeIntelligence": {"enabled": True}, "knowledge": {"enabled": True},
               "thinking": {"enabled": True}, "subagentOrchestration": {"enabled": True}}
# TUI 2.31.1 oT() for the same cli.json, external user (KIRO_INTERNAL unset), workflows rollout available:
TUI_SETTINGS = {"workflows": {"enabled": True}, "goal": {"enabled": True}, "thinking": {"enabled": True},
                "codeIntelligence": {"enabled": True}, "knowledge": {"enabled": True},
                "largeToolOutputHandler": {"enabled": True},
                "workflowNotifications": {"enabled": True, "delivery": "steer"},
                "memory": {"mode": "disabled", "reflection": True}, "userMemoryOptIn": {"enabled": False},
                "toolSearch": {"enabled": True, "minPct": 0, "minTokens": 0}}
# TUI $tt(settings) clientMeta (telemetry off branch)
TUI_CAPS = {"_meta": {"kiro": {"telemetryEnabled": False, "knowledge": True, "hooks": {"enabled": True, "v2": True},
                               "requirementsAnalysis": True, "specPhaseCheckpoints": True,
                               "streamingShellContent": True, "settings": TUI_SETTINGS}}}
TUI_INFO = {"name": "kiro-cli", "version": "2.31.1"}

def p_handshake(D):
    arms = [("cyril_user", cyril_caps(CYRIL_USER), CYRIL_INFO),
            ("cyril_empty_clijson", cyril_caps(CYRIL_EMPTY), CYRIL_INFO),
            ("tui_ref", TUI_CAPS, TUI_INFO),
            ("control_cfgstate", {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True,
                                  "_meta": {"kiro": {"configurationState": True, "settings": {"thinking": {"enabled": True}}}}},
             {"name": "cyril-probe", "version": "audit-2.31.1"})]
    for name, caps, info in arms:
        ws = mk_ws(name)
        t0 = time.time()
        c = Conn(f"hs-{name}", extra_env={"KIRO_LOG_LEVEL": "debug"}, cwd=ws)
        A = {"clientCapabilities": caps, "clientInfo": info}
        try:
            c.tag = f"{name}:init"
            ini = c.call("initialize", {"protocolVersion": 1, "clientCapabilities": caps, "clientInfo": info}, 150)
            A["init"] = "ok" if ini and "result" in ini else short(res_or_err(ini), 300)
            c.tag = f"{name}:new"
            r = c.call("session/new", {"cwd": ws, "mcpServers": []}, 180)
            res = (r or {}).get("result") or {}
            sid = res.get("sessionId")
            meta = res.get("_meta") or {}
            A["memoryConfig"] = meta.get("memoryConfig")
            A["session_meta_flags"] = {k: meta.get(k) for k in ("workflowsEnabled", "disableAutoCompaction", "semanticReviewEnabled", "ftaEnabled", "specPlanEnabled")}
            A["configOptions"] = [(co.get("id"), co.get("currentValue")) for co in res.get("configOptions") or []]
            time.sleep(4)
            c.tag = f"{name}:getState"
            gs = res_or_err(c.call("_kiro/configuration/getState", {"observer": "session", "sessionId": sid}, 45))
            if isinstance(gs, dict):
                A["layers"] = [l.get("id") for l in gs.get("layers") or []]
                A["settings"] = [{"krn": s.get("krn"), "value": s.get("value"),
                                  "contributions": [(x.get("layer"), x.get("stated")) for x in s.get("contributions") or []]}
                                 for s in gs.get("settings") or []]
            else:
                A["getState"] = short(gs, 300)
            c.tag = f"{name}:getStateConn"
            gc = res_or_err(c.call("_kiro/configuration/getState", {"observer": "connection"}, 45))
            if isinstance(gc, dict):
                A["conn_settings"] = [{"krn": s.get("krn"), "value": s.get("value"),
                                       "contributions": [(x.get("layer"), x.get("stated")) for x in s.get("contributions") or []]}
                                      for s in gc.get("settings") or []]
            A["cfgstate_push"] = len(notes(c, "_kiro/configuration/state", t0))
            A["tools_tags"] = [t.get("tag") for _, _, p in notes(c, "_kiro/tools/didChange", t0) for t in p.get("tags") or []][:12]
            time.sleep(1)
            A["intake"] = stderr_lines(c, ["configuration.intake", "unknown-key", "clientInfo", "hooks", "memory"])
        finally:
            c.close()
        D[name] = A
        log(name, json.dumps({k: A.get(k) for k in ("init", "memoryConfig", "cfgstate_push")}), len(A.get("intake") or []))

# ---- fallback --------------------------------------------------------------------
FLAGS = {"KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED": "true", "KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED": "true"}
def p_fallback(D):
    arms = [("F0_control_bogus", {}, "bogus-model-zz9"), ("F1_flags_bogus", FLAGS, "bogus-model-zz9"),
            ("F2_flags_auto", FLAGS, None)]
    for name, env, model in arms:
        ws = mk_ws(name)
        c = Conn(name, extra_env=dict(env, KIRO_LOG_LEVEL="debug"), cwd=ws)
        try:
            c.tag = name + ":init"
            c.call("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True,
                   "_meta": {"kiro": {"configurationState": True}}}, "clientInfo": {"name": "cyril-probe", "version": "audit-2.31.1"}}, 150)
            c.tag = name + ":new"
            r = c.call("session/new", {"cwd": ws, "mcpServers": []}, 180)
            sid = ((r or {}).get("result") or {}).get("sessionId")
            if model:
                c.tag = name + ":setmodel"
                res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "model", "value": model}, 30))
            t = time.time()
            turn = K.prompt(c, sid, "Reply with exactly OK.", name + ":turn", 300) if sid else None
            ups = [p for _, m, p in notes(c, "session/update", t) if p.get("sessionId") == sid]
            kinds = [K.kiro_kind(p) for p in ups]
            fb = [((p.get("update") or {}).get("_meta") or {}).get("kiro") for p in ups if K.kiro_kind(p) in ("model_fallback", "model_fallback_promoted")]
            D[name] = {"env": env, "model_set": model, "turn": turn, "kinds": [k for k in kinds if k],
                       "model_fallback_frames": fb, "init_modelFallbackReporting": None,
                       "stderr": stderr_lines(c, ["fallback", "Fallback", "cascade", "handoff"])[:12]}
        finally:
            c.close()
        log(name, json.dumps(D[name]["turn"])[:300], D[name]["model_fallback_frames"])

# ---- session/load ----------------------------------------------------------------
def replay_summary(c, sid, since):
    out = []
    for ts, m, p in notes(c, None, since):
        if m == "session/update" and p.get("sessionId") == sid:
            u = p.get("update") or {}
            out.append(u.get("sessionUpdate") + (":" + str(K.kiro_kind(p)) if K.kiro_kind(p) else ""))
        elif m != "session/update":
            out.append(m)
    return out

def do_load(name, home, ws, sid, D):
    t0 = time.time()
    c = Conn(name, extra_env={"KIRO_LOG_LEVEL": "debug"}, home=home, cwd=ws)
    try:
        c.tag = name + ":init"
        c.call("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True,
               "_meta": {"kiro": {"configurationState": True}}}, "clientInfo": {"name": "cyril-probe", "version": "audit-2.31.1"}}, 150)
        c.tag = name + ":load"
        t1 = time.time()
        r = c.call("session/load", {"sessionId": sid, "cwd": ws, "mcpServers": []}, 180)
        t_resp = time.time()
        time.sleep(8)
        res = res_or_err(r)
        frames = replay_summary(c, sid, t1)
        before = [f for (ts, m, p), f in zip([x for x in notes(c, None, t1) if (x[1] != "session/update" or x[2].get("sessionId") == sid)], frames) if ts <= t_resp]
        texts = []
        for _, m, p in notes(c, "session/update", t1):
            u = p.get("update") or {}
            if p.get("sessionId") == sid and u.get("sessionUpdate") in ("user_message_chunk", "agent_message_chunk"):
                texts.append((u.get("sessionUpdate")[:5], ((u.get("content") or {}).get("text") or "")[:80],
                              ((u.get("_meta") or {}).get("kiro") or {}).get("replay") if isinstance(u.get("_meta"), dict) else None))
        D[name] = {"load_result": short(res, 600), "replay_frames": frames, "n_before_response": len(before),
                   "texts": texts[:30], "replay_meta_keys": sorted({k for _, m, p in notes(c, "session/update", t1)
                                                                    for k in (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {}).keys()}),
                   "stderr": stderr_lines(c, ["load", "replay", "abandon", "arrivedDuringLoad"])[:15]}
    finally:
        c.close()

def p_sessionload(D):
    sh, ss, sws = os.environ.get("SWEEP_HOME"), os.environ.get("SWEEP_SID"), os.environ.get("SWEEP_WS")
    if sh and ss:
        do_load("load_sweep", sh, sws, ss, D)
    # abandoned prompt: start a long turn, SIGKILL KAS mid-turn, reload
    home = tempfile.mkdtemp(prefix="abandon-home-", dir=SCRATCH)
    ws = mk_ws("abandon")
    c = Conn("abandon-1", home=home, cwd=ws)
    try:
        c.tag = "ab:init"
        c.call("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True,
               "_meta": {"kiro": {"configurationState": True}}}, "clientInfo": {"name": "cyril-probe", "version": "audit-2.31.1"}}, 150)
        c.tag = "ab:new"
        r = c.call("session/new", {"cwd": ws, "mcpServers": []}, 180)
        sid = ((r or {}).get("result") or {}).get("sessionId")
        D["abandon_sid"] = sid
        K.prompt(c, sid, "Reply with exactly the word FIRST.", "ab:p1", 120)
        c.tag = "ab:p2"
        c.req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text":
              "Write a numbered list of the first 60 prime numbers, one per line, with a one-sentence fact about each. Do not use tools."}]})
        st = c.wait_note(lambda m, p: m == "session/update" and p.get("sessionId") == sid and
                         (p.get("update") or {}).get("sessionUpdate") == "agent_message_chunk", 60, time.time() - 1)
        time.sleep(2)
        D["abandon_streaming_seen"] = bool(st)
        os.killpg(os.getpgid(c.proc.pid), signal.SIGKILL)
        time.sleep(2)
    finally:
        c.close()
    if sid:
        do_load("load_abandoned", home, ws, sid, D)

PROBES = {"handshake": p_handshake, "fallback": p_fallback, "sessionload": p_sessionload}
t_start = time.time()
try:
    PROBES[PROBE](R)
finally:
    R["elapsed_s"] = round(time.time() - t_start, 1)
    R["auth_events"] = K.AUTH_EVENTS
    emit(R)
    print(json.dumps(K.scrub(R), indent=1)[:20000])

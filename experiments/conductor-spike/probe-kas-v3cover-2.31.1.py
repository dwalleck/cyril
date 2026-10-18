#!/usr/bin/env python3
"""v3-coverage leads V1/V2/V3 for the kiro-cli 2.31.1 live lane (cyril-4c4d flip blockers).

  KAS=<acp-server.js> PROBE_SCRATCH=<dir> python3 -I probe-kas-v3cover-2.31.1.py <v1|v2|v3> <out.jsonl>

v1  seed a global skill + saved prompt (and workspace twins); record available_commands_update; send cyril's EXACT
    agent-command frame `_kiro.dev/commands/execute {sessionId, command:{command:<name>, args:{}|{value:..}}}`
    (commands/mod.rs AgentCommand::execute -> extensions.rs execute_params/send_extension) and
    `_kiro.dev/commands/options {command, sessionId, partial:""}` for every advertised command; control = one
    billed `session/prompt` "/<skill> …" turn.
v2  one billed turn asking for invoke_sub_agent delegation; record every frame carrying agentSubtaskId.
v3  session/set_config_option mode=plan vs session/set_mode plan; set_config_option model=bogus.
Handshake: full initialize.clientCapabilities._meta.kiro path, positive control configurationState:true.
"""
import importlib.util, json, os, subprocess, sys, tempfile, time
PROBE = sys.argv[1]; OUT = sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["SCENARIO"] = "lib"; os.environ.setdefault("LABEL", f"v3c-{PROBE}")
sys.argv = [sys.argv[0], OUT]
spec = importlib.util.spec_from_file_location("kaslive", os.path.join(HERE, "probe-kas-live-2.31.1.py"))
K = importlib.util.module_from_spec(spec); spec.loader.exec_module(K)
Conn, emit, res_or_err, short, log = K.Conn, K.emit, K.res_or_err, K.short, K.log
SCRATCH = K.SCRATCH
R = {"probe": "lead-result", "lead": PROBE, "kas_version": K.SUMMARY.get("kas_version"), "kas": K.KAS}

def write(p, t):
    os.makedirs(os.path.dirname(p), exist_ok=True); open(p, "w").write(t)
def notes(c, method=None, since=0.0):
    with c.cv:
        return [(ts, m, p) for ts, m, p in c.notes if ts >= since and (method is None or m == method)]
def mk_ws(tag):
    ws = tempfile.mkdtemp(prefix=f"ws-{tag}-", dir=SCRATCH)
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=ws)
    write(os.path.join(ws, "PROBE.txt"), "ALPHA\n"); return ws
def start(name, ws, env=None, home=None):
    c = Conn(name, extra_env=env, home=home, cwd=ws)
    c.tag = name + ":init"
    ini = c.call("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True,
                 "_meta": {"kiro": {"configurationState": True}}}, "clientInfo": {"name": "cyril-probe", "version": "audit-2.31.1"}}, 150)
    c.tag = name + ":new"
    r = c.call("session/new", {"cwd": ws, "mcpServers": []}, 180)
    return c, ini, r, ((r or {}).get("result") or {}).get("sessionId")

def p_v1(D):
    home = tempfile.mkdtemp(prefix="v1-home-", dir=SCRATCH); ws = mk_ws("v1")
    sk = "---\nname: {n}\ndescription: Probe skill {n}. Use when asked to run the {n} probe.\n---\n# {n}\nWhen invoked, reply with exactly {N}RAN followed by any arguments.\n"
    write(os.path.join(home, ".kiro", "skills", "probe-skill", "SKILL.md"), sk.format(n="probe-skill", N="SKILL"))
    write(os.path.join(ws, ".kiro", "skills", "ws-skill", "SKILL.md"), sk.format(n="ws-skill", N="WSSKILL"))
    write(os.path.join(home, ".kiro", "prompts", "probe-prompt.md"), "Reply with exactly PROMPTRAN and then $ARGUMENTS.\n")
    write(os.path.join(ws, ".kiro", "prompts", "ws-prompt.md"), "Reply with exactly WSPROMPTRAN and then $ARGUMENTS.\n")
    t0 = time.time()
    c, ini, r, sid = start("v1", ws, home=home)
    try:
        time.sleep(8)
        acu = [p for _, m, p in notes(c, "session/update", t0) if (p.get("update") or {}).get("sessionUpdate") == "available_commands_update"]
        cmds = (acu[-1]["update"].get("availableCommands") if acu else []) or []
        D["available_commands"] = [{"name": x.get("name"), "description": (x.get("description") or "")[:80], "input": x.get("input"),
                                    "meta_kiro": (x.get("_meta") or {}).get("kiro")} for x in cmds]
        D["acu_frames"] = len(acu)
        res = {}
        for x in cmds:
            n = x.get("name")
            for label, method, params in [
                ("execute_noargs", "_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": n, "args": {}}}),
                ("execute_value", "_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": n, "args": {"value": "hello"}}}),
                ("options", "_kiro.dev/commands/options", {"command": n, "sessionId": sid, "partial": ""})]:
                c.tag = f"v1:{label}:{n}"
                res[f"{n}|{label}"] = short(res_or_err(c.call(method, params, 30)), 500)
        # unprefixed spellings for completeness (not what cyril sends)
        for method in ("kiro.dev/commands/execute", "_kiro/commands/execute"):
            c.tag = f"v1:alt:{method}"
            res[f"alt|{method}"] = short(res_or_err(c.call(method, {"sessionId": sid, "command": {"command": "probe-skill", "args": {}}}, 30)), 500)
        D["dispatch"] = res
        D["notes_after_dispatch"] = sorted({m for _, m, _ in notes(c, None, t0)})
        # control: one billed prompt-text invocation of the skill
        if any(x.get("name") == "probe-skill" for x in cmds) and os.environ.get("V1_CONTROL", "1") == "1":
            t = time.time()
            D["control_turn"] = K.prompt(c, sid, "/probe-skill hello", "v1:control", 180)
            ups = [p.get("update") or {} for _, m, p in notes(c, "session/update", t) if p.get("sessionId") == sid]
            D["control_text"] = "".join((u.get("content") or {}).get("text") or "" for u in ups if u.get("sessionUpdate") == "agent_message_chunk")[:300]
            D["control_tools"] = [(u.get("sessionUpdate"), u.get("title"), json.dumps(u.get("rawInput"))[:200]) for u in ups if u.get("sessionUpdate") in ("tool_call", "tool_call_update") and u.get("title")][:6]
            D["control_user_echo"] = [((u.get("content") or {}).get("text") or "")[:200] for u in ups if u.get("sessionUpdate") == "user_message_chunk"][:3]
    finally:
        c.close()

def p_v2(D):
    ws = mk_ws("v2"); t0 = time.time()
    c, ini, r, sid = start("v2", ws)
    try:
        D["parent_sid"] = sid
        tr = K.prompt(c, sid, "Delegate this to a sub-agent using your sub-agent tool (invoke_sub_agent): the sub-agent must read PROBE.txt and report its single word. Then reply with exactly DONE: followed by that word.", "v2:turn", 420)
        D["turn"] = tr
        tagged, sids = [], {}
        for ts, m, p in notes(c, None, t0):
            s = json.dumps(p)
            psid = p.get("sessionId")
            sids[psid] = sids.get(psid, 0) + 1
            if "agentSubtaskId" in s:
                u = p.get("update") or {}
                mk = (u.get("_meta") or {}).get("kiro") or {}
                tagged.append({"method": m, "sessionId": psid, "is_parent": psid == sid, "su": u.get("sessionUpdate"),
                               "kind": mk.get("kind"), "meta_kiro_keys": sorted(mk.keys()), "agentSubtaskId": mk.get("agentSubtaskId"),
                               "title": u.get("title"), "toolKind": u.get("kind")})
        D["tagged_count"] = len(tagged); D["tagged"] = tagged[:40]; D["sessionIds_seen"] = sids
        D["tagged_sessions"] = sorted({(t["sessionId"], t["is_parent"]) for t in tagged}, key=str)
        D["tool_titles"] = [((p.get("update") or {}).get("title"), (p.get("update") or {}).get("kind"), p.get("sessionId") == sid)
                            for _, m, p in notes(c, "session/update", t0) if (p.get("update") or {}).get("sessionUpdate") == "tool_call"][:10]
    finally:
        c.close()

def p_v3(D):
    ws = mk_ws("v3"); t0 = time.time()
    c, ini, r, sid = start("v3", ws)
    try:
        res = (r or {}).get("result") or {}
        D["modes"] = res.get("modes"); D["cfg0"] = [(x.get("id"), x.get("currentValue")) for x in res.get("configOptions") or []]
        time.sleep(4)
        for label, method, params in [
            ("set_config_mode_plan", "session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "plan"}),
            ("set_mode_vibe", "session/set_mode", {"sessionId": sid, "modeId": "vibe"}),
            ("set_mode_plan", "session/set_mode", {"sessionId": sid, "modeId": "plan"}),
            ("set_config_mode_bogus", "session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "bogus-mode-zz9"}),
            ("set_mode_bogus", "session/set_mode", {"sessionId": sid, "modeId": "bogus-mode-zz9"}),
            ("set_config_model_bogus", "session/set_config_option", {"sessionId": sid, "configId": "model", "value": "bogus-model-zz9"}),
            ("set_model_bogus", "session/set_model", {"sessionId": sid, "modelId": "bogus-model-zz9"})]:
            c.tag = "v3:" + label
            t = time.time()
            rr = res_or_err(c.call(method, params, 30))
            time.sleep(2)
            ns = [(m, (p.get("update") or {}).get("sessionUpdate"), (p.get("update") or {}).get("currentModeId"),
                   [(x.get("id"), x.get("currentValue")) for x in (p.get("update") or {}).get("configOptions") or []])
                  for _, m, p in notes(c, None, t) if m == "session/update"]
            D[label] = {"resp": short(rr if not isinstance(rr, dict) or "configOptions" not in rr else
                                      {"configOptions": [(x.get("id"), x.get("currentValue")) for x in rr["configOptions"]]}, 600),
                        "notifications": ns}
    finally:
        c.close()


def p_l9(D):
    """lead 9 (reframed): does KAS load an `agent upgrade`d agent and which permission section does it honour?
    AGENT_FILE = agent JSON to install as $HOME/.kiro/agents/probe.json (state after cyril's V2-section append)."""
    home = tempfile.mkdtemp(prefix="l9-home-", dir=SCRATCH); ws = mk_ws("l9")
    write(os.path.join(home, ".kiro", "agents", "probe.json"), open(os.environ["AGENT_FILE"]).read())
    t0 = time.time()
    c, ini, r, sid = start("l9", ws, home=home, env={"KIRO_LOG_LEVEL": "debug"})
    try:
        res = (r or {}).get("result") or {}
        D["modes"] = [m.get("id") for m in ((res.get("modes") or {}).get("availableModes") or [])]
        c.tag = "l9:setmode"
        D["setmode"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "probe"}, 30)), 300)
        time.sleep(3)
        for tag, cmd in (("v2trust", "echo V2TRUST"), ("cyriltrust", "echo CYRILTRUST"), ("untrusted", "echo UNTRUSTED")):
            n0 = len(c.perms); t = time.time()
            D[tag] = {"turn": K.prompt(c, sid, f"Use your shell tool to run exactly: {cmd}   -- then reply with only its output.", "l9:" + tag, 180)}
            D[tag]["permission_asks"] = c.perms[n0:]
            D[tag]["shell_calls"] = [((p.get("update") or {}).get("title"), (p.get("update") or {}).get("status")) for _, m, p in notes(c, "session/update", t)
                                     if (p.get("update") or {}).get("sessionUpdate") in ("tool_call",)]
        c.tag = "l9:perms"
        D["permissions_list"] = short(res_or_err(c.call("_kiro/permissions/list", {"sessionId": sid}, 30)), 3000)
    finally:
        c.close()

def p_effort(D):
    """zero-turn: does set_config_option configId "effortLevel" validate against the current model's _meta.kiro.effortLevels?"""
    ws = mk_ws("eff"); t0 = time.time()
    c, ini, r, sid = start("eff", ws)
    def cfg(opts):
        return [(x.get("id"), x.get("currentValue")) for x in opts or []]
    def model_levels(opts, mid):
        for x in opts or []:
            if x.get("id") == "model":
                for o in x.get("options") or []:
                    if o.get("value") == mid: return (o.get("_meta") or {}).get("kiro")
    try:
        res = (r or {}).get("result") or {}
        opts = res.get("configOptions") or []
        D["cfg0"] = cfg(opts)
        mo = next((x for x in opts if x.get("id") == "model"), {})
        D["model_option_meta_kiro"] = {o.get("value"): (o.get("_meta") or {}).get("kiro") for o in mo.get("options") or []}
        D["effort_option0"] = next((x for x in opts if x.get("id") == "effortLevel"), None)
        claude = next((o.get("value") for o in mo.get("options") or [] if "claude" in str(o.get("value")) and ((o.get("_meta") or {}).get("kiro") or {}).get("effortLevels")), None) \
            or next((o.get("value") for o in mo.get("options") or [] if "claude" in str(o.get("value"))), None)
        D["claude_model"] = claude
        steps = [("set_model_claude", "model", claude), ("effort_none", "effortLevel", "none"), ("effort_bogus", "effortLevel", "bogus-zz9"),
                 ("effort_high", "effortLevel", "high"), ("effort_xhigh", "effortLevel", "xhigh"), ("effort_max", "effortLevel", "max")]
        for label, cid, val in steps:
            c.tag = "eff:" + label; t = time.time()
            rr = res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": cid, "value": val}, 30))
            time.sleep(1.5)
            if isinstance(rr, dict) and "configOptions" in rr:
                eo = next((x for x in rr["configOptions"] if x.get("id") == "effortLevel"), None)
                D[label] = {"resp_cfg": cfg(rr["configOptions"]), "effortLevel_option": eo and {"currentValue": eo.get("currentValue"),
                            "options": [o.get("value") for o in eo.get("options") or []]}, "model_meta": model_levels(rr["configOptions"], claude)}
            else:
                D[label] = {"resp": short(rr, 500)}
            D[label]["updates"] = [cfg((p.get("update") or {}).get("configOptions")) for _, m, p in notes(c, "session/update", t)
                                   if (p.get("update") or {}).get("sessionUpdate") == "config_option_update"]
    finally:
        c.close()

PROBES = {"v1": p_v1, "v2": p_v2, "v3": p_v3, "l9": p_l9, "effort": p_effort}
t_start = time.time()
try:
    PROBES[PROBE](R)
finally:
    R["elapsed_s"] = round(time.time() - t_start, 1); R["auth_events"] = K.AUTH_EVENTS
    emit(R); print(json.dumps(K.scrub(R), indent=1)[:20000])

#!/usr/bin/env python3
"""
How does plan mode transition into implementation? (2.11.0, @kiro/agent 0.8.0)

Two runs, one session each (the whole point is mid-session continuity):

  D. plan -> vibe handoff:
     set mode=plan -> turn 1 "make a plan to fix the bug in calc.py"
     -> set mode=vibe MID-SESSION -> turn 2 "implement the plan you just made"
     (turn 2 never restates the bug — forces reliance on carried context).
     Verifies: (a) mid-session switch takes effect, (b) plan-turn context
     survives the switch, (c) the workspace actually changes in turn 2 only.

  E. plan -> CLIENT-INJECTED custom agent handoff:
     session/new carries customAgents [{id:"impl-probe", agentMode:true,
     tools:"*", ...}] (2.7.1 audit: CustomAgentSource.CLIENT_PROVIDED,
     "injected via ACP newSession.customAgents", highest precedence).
     Checks whether the custom agent appears in the mode select, then
     plan-turn -> set mode=impl-probe -> implement-turn. Param placement is
     probed in two shapes (top-level customAgents, _meta.kiro.customAgents)
     — whichever registers wins; both recorded.

Oracle per phase = git porcelain snapshot BETWEEN turns (plan turn must be
clean; implement turn must dirty calc.py). Caps and auth as in the
plan-subagent probe.
"""
import json, os, pathlib, queue, sqlite3, subprocess, tempfile, threading, time

KIRO = "kiro-cli-chat"
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
HERE = os.path.dirname(os.path.abspath(__file__))
OUTDIR = os.path.join(HERE, "kas-plan-handoff-dumps")
os.makedirs(OUTDIR, exist_ok=True)

BUGGY = "def add(a, b):\n    return a - b  # BUG: should be a + b\n\n\nif __name__ == \"__main__\":\n    print(add(2, 3))\n"
PLAN_PROMPT = "There is a bug in calc.py. Make a short plan to fix it. Do not fix it yet."
IMPL_PROMPT = "Now implement the plan you just made."

CUSTOM_AGENT = {
    "id": "impl-probe",
    "description": "Probe implementation agent: applies planned code edits directly.",
    "prompt": "You are an implementation agent. Apply the requested or previously planned code edits directly and verify them. Be brief.",
    "tools": "*",
    "agentMode": True,
}


def log(*a):
    print(" ".join(str(x) for x in a), flush=True)


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    if row is None:
        raise SystemExit("logged out; run `kiro-cli login`")
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    profile_arn = d.get("profile_arn")
    if not profile_arn and prow:
        pv = prow[0]; pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try: profile_arn = json.loads(pv).get("arn")
        except Exception: pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": profile_arn}


def seed_workspace(label):
    cwd = tempfile.mkdtemp(prefix=f"kas-handoff-{label}-")
    subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
                   cwd=cwd, shell=True)
    pathlib.Path(cwd, "calc.py").write_text(BUGGY)
    subprocess.run("git add -A && git commit -qm baseline", cwd=cwd, shell=True)
    return cwd


def porcelain(cwd):
    return subprocess.run("git status --porcelain", cwd=cwd, shell=True,
                          capture_output=True, text=True).stdout.strip()


class Session:
    def __init__(self, label, cwd, new_session_extra=None):
        self.label, self.cwd = label, cwd
        self.proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=cwd,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
        assert self.proc.stdin and self.proc.stdout
        self.stdin, self.stdout = self.proc.stdin, self.proc.stdout
        self.msgs = queue.Queue()
        threading.Thread(target=lambda: ([self.msgs.put(l.strip()) for l in self.stdout if l.strip()],
                                         self.msgs.put(None)), daemon=True).start()
        self._id = 0
        self.frames, self.perms, self.agent_text = [], [], []
        self.turn_ends = 0
        caps = {"_meta": {"kiro": {"userInput": True, "subagentOrchestration": True}}}
        self.call("initialize", {"protocolVersion": 1, "clientCapabilities": caps})
        params = {"cwd": cwd, "mcpServers": []}
        if new_session_extra:
            params.update(new_session_extra)
        nr = self.call("session/new", params)
        self.new_result = (nr or {}).get("result") or {}
        self.new_error = (nr or {}).get("error")
        self.sid = self.new_result.get("sessionId")

    def _req(self, m, p):
        self._id += 1
        self.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self._id, "method": m, "params": p}) + "\n")
        self.stdin.flush()
        return self._id

    def _reply(self, rid, res):
        self.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
        self.stdin.flush()

    def _handle(self, o):
        m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
        self.frames.append({"dir": "in", "method": m, "id": rid, "params": p})
        if rid is None:
            if m and "session/update" in m:
                u = (p.get("update") or {}) if isinstance(p, dict) else {}
                if isinstance(u, dict):
                    kind = u.get("sessionUpdate")
                    meta = ((u.get("_meta") or {}).get("kiro") or {})
                    if kind == "agent_message_chunk":
                        self.agent_text.append(((u.get("content") or {}).get("text") or ""))
                    if kind == "session_info_update" and meta.get("kind") == "turn_end":
                        self.turn_ends += 1
            return
        if m == "_kiro/auth/getAccessToken": self._reply(rid, read_token()); return
        if m == "_kiro/terminal/shell_type": self._reply(rid, {"shellType": "bash"}); return
        if m == "_kiro/userInput":
            opts = p.get("options", [])
            pick = next((o2 for o2 in opts if isinstance(o2, dict) and o2.get("recommended")),
                        opts[0] if opts else None)
            ans = (pick.get("title") if isinstance(pick, dict) else pick) if pick else "yes"
            self._reply(rid, {"action": "answered", "answer": ans}); return
        if m == "session/request_permission":
            self.perms.append((p.get("toolCall", {}) or {}).get("title") or "?")
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                        opts[0] if opts else None)
            self._reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                        if pick else {"outcome": {"outcome": "cancelled"}})
            return
        self._reply(rid, {})

    def call(self, method, params, to=40):
        rid = self._req(method, params); end = time.time() + to
        while time.time() < end:
            try: raw = self.msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: return None
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: self._handle(o)
            elif o.get("id") == rid: return o
        return None

    def mode_list(self, cfg=None):
        cfg = cfg if cfg is not None else (self.new_result.get("configOptions") or [])
        mo = next((c for c in cfg if c.get("id") == "mode"), {})
        return [v.get("value") for v in (mo.get("options") or [])]

    def set_mode(self, mode):
        r = self.call("session/set_config_option",
                      {"sessionId": self.sid, "configId": "mode", "value": mode})
        res = (r or {}).get("result")
        cfg = (res or {}).get("configOptions") if isinstance(res, dict) else None
        cur = next((c.get("currentValue") for c in (cfg or []) if c.get("id") == "mode"), None)
        return cur, (r or {}).get("error")

    def turn(self, text, timeout=420):
        before = self.turn_ends
        pid = self._req("session/prompt", {"sessionId": self.sid,
                                           "prompt": [{"type": "text", "text": text}]})
        end = time.time() + timeout
        resp = None
        while time.time() < end and self.turn_ends == before:
            try: raw = self.msgs.get(timeout=2)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: self._handle(o)
            elif o.get("id") == pid: resp = o.get("result") or o.get("error")
        g = time.time() + 6
        while time.time() < g:
            try: raw = self.msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: self._handle(o)
        return resp

    def close(self):
        try: self.stdin.close()
        except Exception: pass
        self.proc.terminate()


def dump(label, obj):
    path = os.path.join(OUTDIR, f"{label}.json")
    pathlib.Path(path).write_text(json.dumps(obj, indent=2) + "\n")
    log(f"  dump: {path}")


def run_d():
    log("\n===== D: plan -> vibe mid-session handoff =====")
    cwd = seed_workspace("d")
    s = Session("d", cwd)
    log(f"  session={s.sid} modes={s.mode_list()}")
    cur, _ = s.set_mode("plan")
    log(f"  mode -> {cur!r}")
    s.turn(PLAN_PROMPT)
    after_plan = porcelain(cwd)
    log(f"  after plan turn: porcelain={after_plan!r} (must be clean)")
    cur, err = s.set_mode("vibe")
    log(f"  MID-SESSION mode -> {cur!r} err={err}")
    s.turn(IMPL_PROMPT)
    after_impl = porcelain(cwd)
    calc = pathlib.Path(cwd, "calc.py").read_text()
    fixed = "a + b" in calc
    log(f"  after impl turn: porcelain={after_impl!r} calc_fixed={fixed} perms={s.perms}")
    dump("d-plan-to-vibe", {
        "session_new_result": s.new_result, "after_plan_porcelain": after_plan,
        "after_impl_porcelain": after_impl, "calc_fixed": fixed, "perms": s.perms,
        "agent_text": "".join(s.agent_text), "frames": s.frames,
    })
    s.close()
    return after_plan == "" and fixed


def run_e():
    log("\n===== E: plan -> CLIENT-INJECTED custom agent handoff =====")
    for shape_name, extra in [
        ("top-level", {"customAgents": [CUSTOM_AGENT]}),
        ("_meta.kiro", {"_meta": {"kiro": {"customAgents": [CUSTOM_AGENT]}}}),
    ]:
        cwd = seed_workspace(f"e-{shape_name.replace('.', '-')}")
        s = Session("e", cwd, new_session_extra=extra)
        if not s.sid:
            log(f"  shape={shape_name}: session/new FAILED: {json.dumps(s.new_error)[:200]}")
            s.close(); continue
        modes = s.mode_list()
        registered = "impl-probe" in modes
        log(f"  shape={shape_name}: session={s.sid} impl-probe in mode list: {registered} modes={modes}")
        if not registered:
            # try selecting it anyway — registry may accept ids not surfaced as modes
            cur, err = s.set_mode("impl-probe")
            log(f"  shape={shape_name}: blind set mode=impl-probe -> {cur!r} err={json.dumps(err)[:120] if err else None}")
            if cur != "impl-probe":
                s.close(); continue
        cur, _ = s.set_mode("plan")
        log(f"  mode -> {cur!r}")
        s.turn(PLAN_PROMPT)
        after_plan = porcelain(cwd)
        log(f"  after plan turn: porcelain={after_plan!r}")
        cur, err = s.set_mode("impl-probe")
        log(f"  MID-SESSION mode -> {cur!r} err={err}")
        s.turn(IMPL_PROMPT)
        after_impl = porcelain(cwd)
        calc = pathlib.Path(cwd, "calc.py").read_text()
        fixed = "a + b" in calc
        log(f"  after impl turn: porcelain={after_impl!r} calc_fixed={fixed} perms={s.perms}")
        dump(f"e-custom-{shape_name.replace('.', '-')}", {
            "shape": shape_name, "session_new_result": s.new_result,
            "impl_probe_registered": registered, "after_plan_porcelain": after_plan,
            "after_impl_porcelain": after_impl, "calc_fixed": fixed, "perms": s.perms,
            "agent_text": "".join(s.agent_text), "frames": s.frames,
        })
        s.close()
        if fixed:
            return True, shape_name
    return False, None


def main():
    v = subprocess.run([KIRO, "--version"], capture_output=True, text=True).stdout.strip()
    log(f"# binary={v}")
    d_ok = run_d()
    e_ok, e_shape = run_e()
    log("\n===== SUMMARY =====")
    log(f"  D plan->vibe handoff, context carried, impl wrote: {d_ok}")
    log(f"  E plan->custom-agent handoff worked: {e_ok} (shape={e_shape})")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Follow-up to probe-kas-plan-handoff-2.11.0.py, which found that a mid-session
`session/set_config_option` mode switch is ACCEPTED (currentValue updates,
rebuild broadcast) but does NOT rebind the agent for later turns — both
handoff attempts kept answering as the planning agent (schema-accepted !=
functional, again).

This probe tests the two remaining candidate mechanisms for a functional
mid-session switch, in one session each:

  F. ACP-standard `session/set_mode {sessionId, modeId}` — KAS populates the
     standard `modes` block on session/new, so the config select and the ACP
     mode state may be different layers.
  G. per-turn `_meta.kiro.modeId` on session/prompt (the 2.7.1 semantic-review
     probe used this meta on initialize/session/new; untested per-turn).

Flow per run: set plan (via the mechanism under test where applicable) ->
plan turn -> switch to vibe (mechanism under test) -> "implement the plan you
just made" -> oracle. Oracle fixed from the first probe's false positive:
`"return a + b" in calc` AND porcelain dirty (the buggy comment contains the
literal "a + b").
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
    cwd = tempfile.mkdtemp(prefix=f"kas-handoff2-{label}-")
    subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
                   cwd=cwd, shell=True)
    pathlib.Path(cwd, "calc.py").write_text(BUGGY)
    subprocess.run("git add -A && git commit -qm baseline", cwd=cwd, shell=True)
    return cwd


def porcelain(cwd):
    return subprocess.run("git status --porcelain", cwd=cwd, shell=True,
                          capture_output=True, text=True).stdout.strip()


def fixed(cwd):
    return "return a + b" in pathlib.Path(cwd, "calc.py").read_text()


class Session:
    def __init__(self, cwd):
        self.cwd = cwd
        self.proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=cwd,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
        assert self.proc.stdin and self.proc.stdout
        self.stdin, self.stdout = self.proc.stdin, self.proc.stdout
        self.msgs = queue.Queue()
        threading.Thread(target=lambda: ([self.msgs.put(l.strip()) for l in self.stdout if l.strip()],
                                         self.msgs.put(None)), daemon=True).start()
        self._id = 0
        self.frames, self.perms = [], []
        self.agent_text = []
        self.turn_ends = 0
        caps = {"_meta": {"kiro": {"userInput": True}}}
        self.call("initialize", {"protocolVersion": 1, "clientCapabilities": caps})
        nr = self.call("session/new", {"cwd": cwd, "mcpServers": []})
        self.new_result = (nr or {}).get("result") or {}
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

    def turn(self, text, extra_params=None, timeout=420):
        before = self.turn_ends
        params = {"sessionId": self.sid, "prompt": [{"type": "text", "text": text}]}
        if extra_params:
            params.update(extra_params)
        pid = self._req("session/prompt", params)
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


def run_f():
    log("\n===== F: ACP-standard session/set_mode =====")
    cwd = seed_workspace("f")
    s = Session(cwd)
    modes = s.new_result.get("modes") or {}
    log(f"  session={s.sid} ACP modes block: current={modes.get('currentModeId')!r} "
        f"available={[m.get('id') for m in modes.get('availableModes', [])]}")
    r1 = s.call("session/set_mode", {"sessionId": s.sid, "modeId": "plan"})
    log(f"  set_mode plan -> result={json.dumps((r1 or {}).get('result'))[:120]} error={json.dumps((r1 or {}).get('error'))[:160]}")
    if (r1 or {}).get("error"):
        log("  set_mode unsupported — F inconclusive at the RPC layer")
        s.close()
        return {"supported": False}
    s.turn(PLAN_PROMPT)
    p1 = porcelain(cwd)
    log(f"  after plan turn: porcelain={p1!r} fixed={fixed(cwd)}")
    r2 = s.call("session/set_mode", {"sessionId": s.sid, "modeId": "vibe"})
    log(f"  MID-SESSION set_mode vibe -> result={json.dumps((r2 or {}).get('result'))[:120]} error={json.dumps((r2 or {}).get('error'))[:160]}")
    s.turn(IMPL_PROMPT)
    p2 = porcelain(cwd)
    ok = fixed(cwd)
    log(f"  after impl turn: porcelain={p2!r} fixed={ok} perms={s.perms}")
    pathlib.Path(OUTDIR, "f-set-mode.json").write_text(json.dumps({
        "plan_porcelain": p1, "impl_porcelain": p2, "fixed": ok, "perms": s.perms,
        "set_mode_results": [(r1 or {}), (r2 or {})],
        "agent_text": "".join(s.agent_text), "frames": s.frames}, indent=2) + "\n")
    s.close()
    return {"supported": True, "plan_clean": p1 == "", "impl_fixed": ok}


def run_g():
    log("\n===== G: per-turn _meta.kiro.modeId on session/prompt =====")
    cwd = seed_workspace("g")
    s = Session(cwd)
    log(f"  session={s.sid}")
    s.turn(PLAN_PROMPT, extra_params={"_meta": {"kiro": {"modeId": "plan"}}})
    p1 = porcelain(cwd)
    log(f"  after plan turn (meta modeId=plan): porcelain={p1!r} fixed={fixed(cwd)}")
    s.turn(IMPL_PROMPT, extra_params={"_meta": {"kiro": {"modeId": "vibe"}}})
    p2 = porcelain(cwd)
    ok = fixed(cwd)
    log(f"  after impl turn (meta modeId=vibe): porcelain={p2!r} fixed={ok} perms={s.perms}")
    pathlib.Path(OUTDIR, "g-prompt-meta.json").write_text(json.dumps({
        "plan_porcelain": p1, "impl_porcelain": p2, "fixed": ok, "perms": s.perms,
        "agent_text": "".join(s.agent_text), "frames": s.frames}, indent=2) + "\n")
    s.close()
    return {"plan_clean": p1 == "", "impl_fixed": ok}


def main():
    v = subprocess.run([KIRO, "--version"], capture_output=True, text=True).stdout.strip()
    log(f"# binary={v}")
    f = run_f()
    g = run_g()
    log("\n===== SUMMARY =====")
    log(f"  F session/set_mode: {f}")
    log(f"  G prompt-meta modeId: {g}")


if __name__ == "__main__":
    main()

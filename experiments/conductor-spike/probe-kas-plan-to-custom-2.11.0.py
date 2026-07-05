#!/usr/bin/env python3
"""
The last unverified leg of the plan->implementation matrix (2.11.0,
@kiro/agent 0.8.0): plan turn -> CLIENT-INJECTED custom agent implements,
switched via the per-turn `_meta.kiro.modeId` on session/prompt (the only
functional mid-session switch — the session-level setters are cosmetic; see
probe-kas-plan-handoff2-2.11.0.py). Also verifies conversation context
carries into a custom-agent turn: the implement prompt never restates the
bug or the file.

Arc: session/new with `_meta.kiro.customAgents:[impl-probe]` (the shape that
registers; top-level is silently ignored) -> assert impl-probe in the mode
select -> turn 1 modeId=plan (plan only, porcelain must stay clean) ->
turn 2 modeId=impl-probe "Now implement the plan you just made" -> oracle:
porcelain dirty, `return a + b` in calc.py.
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


def main():
    v = subprocess.run([KIRO, "--version"], capture_output=True, text=True).stdout.strip()
    log(f"# binary={v}")
    cwd = tempfile.mkdtemp(prefix="kas-plan2custom-")
    subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
                   cwd=cwd, shell=True)
    pathlib.Path(cwd, "calc.py").write_text(BUGGY)
    subprocess.run("git add -A && git commit -qm baseline", cwd=cwd, shell=True)

    proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=cwd,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    assert proc.stdin and proc.stdout
    stdin, stdout = proc.stdin, proc.stdout
    msgs = queue.Queue()
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in stdout if l.strip()],
                                     msgs.put(None)), daemon=True).start()
    state = {"id": 0, "turn_ends": 0}
    frames, perms, agent_text = [], [], []

    def req(m, p):
        state["id"] += 1
        stdin.write(json.dumps({"jsonrpc": "2.0", "id": state["id"], "method": m, "params": p}) + "\n")
        stdin.flush()
        return state["id"]

    def reply(rid, res):
        stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
        stdin.flush()

    def handle(o):
        m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
        frames.append({"dir": "in", "method": m, "id": rid, "params": p})
        if rid is None:
            if m and "session/update" in m:
                u = (p.get("update") or {}) if isinstance(p, dict) else {}
                if isinstance(u, dict):
                    kind = u.get("sessionUpdate")
                    meta = ((u.get("_meta") or {}).get("kiro") or {})
                    if kind == "agent_message_chunk":
                        agent_text.append(((u.get("content") or {}).get("text") or ""))
                    if kind == "session_info_update" and meta.get("kind") == "turn_end":
                        state["turn_ends"] += 1
            return
        if m == "_kiro/auth/getAccessToken": reply(rid, read_token()); return
        if m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"}); return
        if m == "_kiro/userInput":
            opts = p.get("options", [])
            pick = next((o2 for o2 in opts if isinstance(o2, dict) and o2.get("recommended")),
                        opts[0] if opts else None)
            ans = (pick.get("title") if isinstance(pick, dict) else pick) if pick else "yes"
            reply(rid, {"action": "answered", "answer": ans}); return
        if m == "session/request_permission":
            perms.append((p.get("toolCall", {}) or {}).get("title") or "?")
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                        opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                  if pick else {"outcome": {"outcome": "cancelled"}})
            return
        reply(rid, {})

    def call_sync(method, params, to=40):
        rid = req(method, params); end = time.time() + to
        while time.time() < end:
            try: raw = msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: return None
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
            elif o.get("id") == rid: return o
        return None

    def turn(text, mode_id, timeout=360):
        before = state["turn_ends"]
        pid = req("session/prompt", {"sessionId": sid,
                                     "prompt": [{"type": "text", "text": text}],
                                     "_meta": {"kiro": {"modeId": mode_id}}})
        end = time.time() + timeout
        while time.time() < end and state["turn_ends"] == before:
            try: raw = msgs.get(timeout=2)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
            elif o.get("id") == pid: pass
        g = time.time() + 6
        while time.time() < g:
            try: raw = msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)

    def porcelain():
        return subprocess.run("git status --porcelain", cwd=cwd, shell=True,
                              capture_output=True, text=True).stdout.strip()

    call_sync("initialize", {"protocolVersion": 1,
                             "clientCapabilities": {"_meta": {"kiro": {"userInput": True}}}})
    nr = call_sync("session/new", {"cwd": cwd, "mcpServers": [],
                                   "_meta": {"kiro": {"customAgents": [CUSTOM_AGENT]}}})
    res = (nr or {}).get("result") or {}
    global sid
    sid = res.get("sessionId")
    cfg = res.get("configOptions") or []
    mo = next((c for c in cfg if c.get("id") == "mode"), {})
    modes = [v2.get("value") for v2 in (mo.get("options") or [])]
    log(f"session={sid} impl-probe registered: {'impl-probe' in modes} modes={modes}")

    turn(PLAN_PROMPT, "plan")
    p1 = porcelain()
    log(f"after plan turn (modeId=plan): porcelain={p1!r}")

    marker = len(agent_text)
    turn(IMPL_PROMPT, "impl-probe")
    p2 = porcelain()
    calc = pathlib.Path(cwd, "calc.py").read_text()
    fixed = "return a + b" in calc
    log(f"after impl turn (modeId=impl-probe): porcelain={p2!r} fixed={fixed} perms={perms}")
    log(f"impl-turn text: {''.join(agent_text[marker:])[:400]!r}")

    pathlib.Path(OUTDIR, "h-plan-to-custom.json").write_text(json.dumps({
        "impl_probe_registered": "impl-probe" in modes,
        "plan_porcelain": p1, "impl_porcelain": p2, "fixed": fixed, "perms": perms,
        "agent_text": "".join(agent_text), "frames": frames}, indent=2) + "\n")
    log(f"dump: {OUTDIR}/h-plan-to-custom.json")
    log(f"\nVERDICT: plan(clean)={p1==''} custom-agent-implemented={fixed}")
    try: stdin.close()
    except Exception: pass
    proc.terminate()


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
cyril-497j wire probe: does the KAS Stop-hook `confirm` block actually drive the
documented ACP flow? (kiro-cli 2.13.0, @kiro/agent 0.18.2)

Static analysis (src/hooks/triggers/stop.ts) predicts, per Stop-hook with confirm:
  1. synthetic pending tool_call, toolCallId `hook-confirm-<uuid>`, kind "other",
     _meta.kiro.hookConfirm {kind:"stop", hookName, question}
  2. standard session/request_permission whose options map from confirm.options
     (optionId=id, name=label, kind = allow_once if run:true else reject_once)
  3. chosen run:false option WITH continueReason -> hook command skipped, turn-stop
     converted to keep-working, reason appended as a NEW HUMAN MESSAGE wrapped in
     <HOOK_INSTRUCTION>...</HOOK_INSTRUCTION> and the agent graph restarts
     (applyStopDecision). Guard: `onAgentStopHooksExecuted` (shared-graph-nodes.ts)
     means Stop hooks fire at most ONCE per turn - the continuation is unguarded
  4. terminal tool_call_update with _meta.kiro.hookConfirm.decision
  5. chosen run:false option WITHOUT continueReason -> turn actually ends

Setup: workspace `.kiro/hooks/finish-gate.json` (kasHookFileSchema v1) with a
Stop-trigger hook: options opt-run (run:true), opt-skip (run:false, no reason),
opt-more (run:false, continueReason orders the agent to say TURTLES).
Handshake: clientCapabilities._meta.kiro.hooks = {enabled:true, v2:true}
(`enabled` turns hooks on; `v2` activates the standalone .kiro/hooks loader —
the gate is `hooksConfig.v2 === true` in the bundle).

One turn: trivial prompt -> confirm #1: pick opt-more -> expect continuation +
TURTLES -> confirm #2: pick opt-skip -> expect prompt response (turn end).
Token self-sourced from data.sqlite3; throwaway git cwd; full wire dumped.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.13.0/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
HERE = os.path.dirname(os.path.abspath(__file__))
DUMP = os.path.join(HERE, "kas-hook-confirm-2.13.0.json")

HOOK_FILE = {
    "version": "v1",
    "hooks": [{
        "name": "finish-gate",
        "trigger": "Stop",
        "action": {"type": "command", "command": "echo gate-ran"},
        "confirm": {
            "question": "PROBE: the agent wants to finish. What do you want?",
            "options": [
                {"id": "opt-run",  "label": "Run gate command", "run": True},
                {"id": "opt-skip", "label": "Skip and finish",  "run": False},
                {"id": "opt-more", "label": "Keep working",     "run": False,
                 "continueReason": "PROBE-CONTINUE: reply with exactly the word TURTLES and then stop."},
            ],
        },
    }],
}


def log(*a): print(" ".join(str(x) for x in a), flush=True)


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    profile_arn = d.get("profile_arn")
    if not profile_arn and prow:
        pv = prow[0]; pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try: profile_arn = json.loads(pv).get("arn")
        except Exception: pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": profile_arn}


def main():
    cwd = tempfile.mkdtemp(prefix="kas-hookconfirm-")
    subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
                   cwd=cwd, shell=True)
    pathlib.Path(cwd, "README.md").write_text("# probe\n")
    hooks_dir = pathlib.Path(cwd, ".kiro", "hooks"); hooks_dir.mkdir(parents=True)
    (hooks_dir / "finish-gate.json").write_text(json.dumps(HOOK_FILE, indent=2))
    subprocess.run("git add -A && git commit -qm baseline", cwd=cwd, shell=True)

    proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=cwd,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    assert proc.stdin and proc.stdout
    PIN, POUT = proc.stdin, proc.stdout
    msgs = queue.Queue()
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)),
                     daemon=True).start()

    _id = [0]
    frames = []                 # full inbound wire
    confirm_requests = []       # session/request_permission frames identified as hookConfirm
    hookconfirm_toolcalls = []  # tool_call / tool_call_update updates tagged _meta.kiro.hookConfirm
    agent_text = []             # accumulated agent_message_chunk text
    prompt_response = []        # the session/prompt JSON-RPC response (turn end signal)
    prompt_rid = []
    confirm_plan = ["opt-more", "opt-skip"]   # decision sequence for successive confirms

    def send(obj):
        PIN.write(json.dumps(obj) + "\n"); PIN.flush()

    def req(m, p):
        _id[0] += 1
        send({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p})
        return _id[0]

    def reply(rid, res):
        send({"jsonrpc": "2.0", "id": rid, "result": res})

    def handle(o):
        m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
        if m is not None:
            frames.append({"dir": "in", "method": m, "id": rid, "params": p})
        if rid is None and m == "session/update":
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            if isinstance(u, dict):
                meta = ((u.get("_meta") or {}).get("kiro") or {})
                if u.get("sessionUpdate") == "agent_message_chunk":
                    c = u.get("content") or {}
                    if isinstance(c, dict) and c.get("type") == "text":
                        agent_text.append(c.get("text") or "")
                if "hookConfirm" in meta:
                    hookconfirm_toolcalls.append({
                        "sessionUpdate": u.get("sessionUpdate"),
                        "toolCallId": u.get("toolCallId"),
                        "title": u.get("title"), "status": u.get("status"),
                        "kind": u.get("kind"), "hookConfirm": meta["hookConfirm"],
                    })
            return
        if rid is None:
            return
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token()); return
        if m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"}); return
        if m == "session/request_permission":
            opts = p.get("options", []) or []
            tc = p.get("toolCall") or {}
            is_confirm = str(tc.get("toolCallId", "")).startswith("hook-confirm-") or \
                {o2.get("optionId") for o2 in opts} & {"opt-run", "opt-skip", "opt-more"}
            if is_confirm:
                pick = confirm_plan.pop(0) if confirm_plan else "opt-skip"
                confirm_requests.append({"toolCall": tc, "options": opts, "picked": pick})
                log(f"  [confirm #{len(confirm_requests)}] q={tc.get('title')!r} "
                    f"options={[(o2.get('optionId'), o2.get('kind')) for o2 in opts]} -> {pick}")
                reply(rid, {"outcome": {"outcome": "selected", "optionId": pick}})
            else:
                pick = next((x for x in opts if "allow" in str(x.get("kind", "")).lower()),
                            opts[0] if opts else None)
                log(f"  [tool-permission] {tc.get('title')!r} -> {pick and pick.get('optionId')}")
                reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                      if pick else {"outcome": {"outcome": "cancelled"}})
            return
        reply(rid, {})

    def pump_until(cond, timeout):
        end = time.time() + timeout
        while time.time() < end:
            try: raw = msgs.get(timeout=1)
            except queue.Empty:
                if cond(): return True
                continue
            if raw is None: return cond()
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o:
                handle(o)
            elif prompt_rid and o.get("id") == prompt_rid[0]:
                prompt_response.append(o)
            elif "id" in o:
                pass
            if cond(): return True
        return cond()

    def call_sync(method, params, to=60):
        rid = req(method, params)
        got = [None]
        end = time.time() + to
        while time.time() < end:
            try: raw = msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
            elif o.get("id") == rid: got[0] = o; break
        return got[0]

    caps = {"terminal": True, "_meta": {"kiro": {"hooks": {"enabled": True, "v2": True}}}}
    ir = call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": caps})
    log("initialize:", "ok" if ir and "result" in ir else json.dumps(ir))
    nr = call_sync("session/new", {"cwd": cwd, "mcpServers": []})
    sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
    log("session:", sid)
    if not sid:
        proc.terminate(); return
    pump_until(lambda: False, 5)  # drain post-session notifications

    prompt_rid.append(req("session/prompt", {
        "sessionId": sid,
        "prompt": [{"type": "text", "text": "Reply with exactly the word HELLO and end your turn."}]}))
    done = pump_until(lambda: bool(prompt_response), 300)
    text = "".join(agent_text)

    # ---- verdicts ----
    v = {}
    v["V1 pending tool_call tagged hookConfirm"] = any(
        t["sessionUpdate"] == "tool_call" and t["hookConfirm"].get("question")
        for t in hookconfirm_toolcalls)
    kinds = {o2.get("optionId"): o2.get("kind")
             for c in confirm_requests for o2 in c["options"]}
    v["V2 option kinds map run->allow_once / !run->reject_once"] = (
        kinds.get("opt-run") == "allow_once" and kinds.get("opt-skip") == "reject_once"
        and kinds.get("opt-more") == "reject_once") if confirm_requests else False
    v["V3 once-per-turn guard: exactly ONE confirm despite continuation"] = len(confirm_requests) == 1
    v["V3b TURTLES (reason text obeyed) in continuation"] = "TURTLES" in text.upper()
    v["V4 terminal tool_call_update carries decision"] = any(
        t["sessionUpdate"] == "tool_call_update" and "decision" in t["hookConfirm"]
        for t in hookconfirm_toolcalls)
    v["V5 turn ended after guarded continuation (prompt response arrived)"] = bool(done and prompt_response)

    log("\n===== verdicts =====")
    for k, ok in v.items():
        log(f"  {'PASS' if ok else 'FAIL'}  {k}")
    log("\nagent text:", json.dumps(text[:400]))
    log("hookConfirm tool_call frames:", json.dumps(hookconfirm_toolcalls, indent=2)[:1500])
    log("stopReason:", json.dumps(((prompt_response[0] if prompt_response else {}) or {}).get("result") or {}))

    pathlib.Path(DUMP).write_text(json.dumps({
        "binary": KIRO, "handshake_meta": caps["_meta"], "hook_file": HOOK_FILE,
        "verdicts": {k: bool(x) for k, x in v.items()},
        "confirm_requests": confirm_requests,
        "hookconfirm_toolcalls": hookconfirm_toolcalls,
        "agent_text": text, "prompt_response": prompt_response[0] if prompt_response else None,
        "frame_methods": [f["method"] for f in frames],
        "frames": frames,
    }, indent=2) + "\n")
    log("dump:", DUMP)
    PIN.close(); proc.terminate()


if __name__ == "__main__":
    main()

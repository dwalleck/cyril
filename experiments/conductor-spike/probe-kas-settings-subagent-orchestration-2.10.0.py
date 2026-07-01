#!/usr/bin/env python3
"""
cyril-nhzw prove-it-prototype (v2): does KAS READ
`clientCapabilities._meta.kiro.settings` from the `initialize` handshake and
change behavior? Covenant §3: `subagentOrchestration` true -> `orchestrate_subagent`,
false/absent -> `invoke_sub_agent`.

v1 was oracle-blind: the forced-turn prompt NAMED both tools, KAS echoed the
prompt in agent_message_chunk, and the string scan matched both in both runs.

v2 fixes the oracle:
  * DETERMINISTIC channel: dump the full session-setup wire (initialize result +
    session/new result + first notifications, NO prompt) per config to a file and
    diff A vs B — no prompt, so no contamination. If the tool set / commands /
    agentCapabilities differ, the setting is read.
  * BEHAVIORAL channel (only if setup is identical): one delegation turn with a
    NEUTRAL prompt (never names a tool). Capture each `tool_call` frame's identity
    (title + _meta.kiro.toolId/kind) and the turn_end usedTools. The subagent
    tool the agent actually CALLS is the answer.

Same binary (2.10.0 `acp --agent-engine v3`), same day, toggle only the setting.
Token self-sourced; throwaway git cwd; records tool NAMES only (never the token).
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.10.0/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
SUBAGENT_TOOLS = ("orchestrate_subagent", "invoke_sub_agent")
OUTDIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "kas-nhzw-dumps")
os.makedirs(OUTDIR, exist_ok=True)
DO_TURN = os.environ.get("NHZW_TURN") == "1"   # opt-in behavioral channel


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


def run_config(label, settings, dumpname):
    cwd = tempfile.mkdtemp(prefix="kas-nhzw-")
    subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
                   cwd=cwd, shell=True)
    pathlib.Path(cwd, "README.md").write_text("# probe\n")
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
    frames = []            # every inbound frame (method-bearing), for the dump
    tool_calls = []        # {title, toolId, kind} for each tool_call notification
    used_tools = set()
    turn_ends = [0]

    def req(m, p):
        _id[0] += 1
        PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n"); PIN.flush()
        return _id[0]

    def reply(rid, res):
        PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()

    def handle(o):
        m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
        frames.append({"dir": "in", "method": m, "id": rid, "params": p})
        if rid is None:
            if m and "session/update" in m:
                u = (p.get("update") or {}) if isinstance(p, dict) else {}
                if isinstance(u, dict):
                    meta = ((u.get("_meta") or {}).get("kiro") or {})
                    if u.get("sessionUpdate") == "tool_call":
                        tool_calls.append({"title": u.get("title"),
                                           "toolId": meta.get("toolId") or meta.get("tool_id"),
                                           "kind": meta.get("kind")})
                    if u.get("sessionUpdate") == "session_info_update" and meta.get("kind") == "turn_end":
                        turn_ends[0] += 1
                        for s in (meta.get("promptTurnSummaries") or []):
                            used_tools.update(s.get("usedTools") or [])
            return
        if m == "_kiro/auth/getAccessToken": reply(rid, read_token()); return
        if m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"}); return
        if m == "session/request_permission":
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                        opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                  if pick else {"outcome": {"outcome": "cancelled"}})
            return
        reply(rid, {})

    def pump(to):
        end = time.time() + to
        while time.time() < end:
            try: raw = msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: return False
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
        return True

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

    caps: dict[str, object] = {"terminal": True}
    if settings is not None:
        caps["_meta"] = {"kiro": {"settings": settings}}
    ir = call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": caps})
    nr = call_sync("session/new", {"cwd": cwd, "mcpServers": []}) if (ir and "result" in ir) else None
    sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
    pump(6)  # drain post-session notifications (available commands / tools)

    ok = bool(sid)
    if ok and DO_TURN:
        # NEUTRAL prompt — never names the exact tool id, so nothing to
        # echo-contaminate the orchestrate_subagent/invoke_sub_agent string scan.
        # Strong instruction so the model actually delegates rather than answering.
        prompt = ("This is a test of your sub-agent delegation capability. You MUST "
                  "use your sub-agent delegation capability and MUST NOT answer "
                  "directly. Delegate a subtask to a sub-agent: have the sub-agent "
                  "reply with exactly the word BANANA. Then tell me the single word "
                  "the sub-agent returned.")
        req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
        before = turn_ends[0]; end = time.time() + 180
        while time.time() < end and turn_ends[0] <= before:
            if not pump(10): break

    # Dump the full wire for offline inspection / A-vs-B diff.
    dump = {
        "label": label, "sent_settings": settings, "ok": ok,
        "initialize_result": (ir or {}).get("result"),
        "session_new_result": (nr or {}).get("result"),
        "tool_calls": tool_calls, "used_tools": sorted(used_tools),
        "frame_methods": [f["method"] for f in frames],
        "frames": frames,
    }
    path = os.path.join(OUTDIR, dumpname)
    pathlib.Path(path).write_text(json.dumps(dump, indent=2) + "\n")

    # String presence of the two tool names across the WHOLE setup wire (excluding
    # agent_message_chunk text, to avoid prompt/answer echo contaminating it).
    setup_blob = json.dumps({k: v for k, v in dump.items() if k != "frames"})
    for f in frames:
        u = ((f.get("params") or {}).get("update") or {})
        if isinstance(u, dict) and u.get("sessionUpdate") not in ("agent_message_chunk", "agent_thought_chunk"):
            setup_blob += json.dumps(f)
    seen = {t for t in SUBAGENT_TOOLS if t in setup_blob}

    PIN.close(); proc.terminate()
    log(f"\n===== {label} =====")
    log("  sent _meta.kiro.settings:", json.dumps(settings) if settings is not None else "(none)")
    log("  ok:", ok, " frames:", len(frames), " tool_calls:", len(tool_calls), " usedTools:", sorted(used_tools))
    log("  subagent tool names in setup wire (non-chat):", sorted(seen) or "(none)")
    log("  dump:", path)
    return seen, used_tools, tool_calls, ok


def main():
    log(f"# binary={KIRO}  DO_TURN={DO_TURN}")
    on = run_config("A: subagentOrchestration={enabled:true}",
                    {"subagentOrchestration": {"enabled": True}}, "A_on.json")
    time.sleep(2)
    off = run_config("B: no _meta.kiro.settings (cyril today)", None, "B_off.json")
    on_seen, on_used, _on_tc, on_ok = on
    off_seen, off_used, _off_tc, off_ok = off

    log("\n===== A/B RESULT =====")
    log(f"  A setup subagent tools: {sorted(on_seen) or '(none)'}   usedTools={sorted(on_used)}")
    log(f"  B setup subagent tools: {sorted(off_seen) or '(none)'}  usedTools={sorted(off_used)}")

    log("\n===== ORACLE VERDICT =====")
    if not (on_ok and off_ok):
        log("  INCONCLUSIVE — a run failed to reach session/new (auth/precondition?).")
    elif on_seen == off_seen and not (on_used or off_used):
        log("  SETUP IDENTICAL on the deterministic channel — the flag does not change")
        log("  the advertised tool set. Re-run with NHZW_TURN=1 for the behavioral")
        log("  channel (which tool the agent actually calls). Inspect the dumps for a")
        log("  system-prompt / agentCapabilities delta the string scan missed.")
    elif "orchestrate_subagent" in on_seen and "orchestrate_subagent" not in off_seen:
        log("  CONFIRMED — subagentOrchestration:true exposes orchestrate_subagent;")
        log("  absent does not. KAS reads _meta.kiro.settings. nhzw is real.")
    else:
        log("  PARTIAL/UNEXPECTED — inspect the dumps in", OUTDIR)
    log("  diff:  diff <(jq -S . kas-nhzw-dumps/A_on.json) <(jq -S . kas-nhzw-dumps/B_off.json)")


if __name__ == "__main__":
    main()

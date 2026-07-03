#!/usr/bin/env python3
"""
Exercise the KAS modes the 2.7.1 audit left un-run: `plan`, `bug-fix`, `quick-spec`
(audit §"Settings / modes not exercised"). 2.10.0 binary (@kiro/agent 0.3.299).

Per mode, in a fresh throwaway git workspace seeded with a buggy calc.py:
  1. session/new                  -> enumerate configOptions, assert mode id exists
  2. session/set_config_option    -> mode = <target>; response is source of truth
  3. one prompt turn              -> pump to session_info_update turn_end
  4. filesystem oracle            -> git status --porcelain + calc.py content +
                                     .kiro/ tree (quick-spec should emit spec docs)

A/B design: `plan` and `bug-fix` get the IDENTICAL prompt ("fix the bug in
calc.py") so the write/no-write delta is attributable to the mode, not the ask.
`quick-spec` gets a small feature ask and advertises userInput so the clarifying
questions land on the wire (answered with the recommended option).

Caps: userInput only — NO fs/terminal, so file+shell I/O stays in-process and a
missing responder can't hang the turn. Token self-sourced (incl. profileArn from
the state table — the 2.10.0 "profileArn is required" gotcha); never logged.
"""
import json, os, pathlib, queue, sqlite3, subprocess, tempfile, threading, time

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.10.0/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
HERE = os.path.dirname(os.path.abspath(__file__))
OUTDIR = os.path.join(HERE, "kas-modes-dumps")
os.makedirs(OUTDIR, exist_ok=True)

BUGGY = "def add(a, b):\n    return a - b  # BUG: should be a + b\n\n\nif __name__ == \"__main__\":\n    print(add(2, 3))\n"
FIX_PROMPT = "There is a bug in calc.py. Fix it."
SPEC_PROMPT = "Add a subtract(a, b) function to calc.py, with a simple test file."

RUNS = [
    ("plan",       FIX_PROMPT,  420),
    ("bug-fix",    FIX_PROMPT,  420),
    ("quick-spec", SPEC_PROMPT, 900),
]


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
        raise SystemExit("no kirocli:odic:token in auth_kv — logged out; run `kiro-cli login` and retry")
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    profile_arn = d.get("profile_arn")
    if not profile_arn and prow:
        pv = prow[0]; pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try: profile_arn = json.loads(pv).get("arn")
        except Exception: pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": profile_arn}


def seed_workspace():
    cwd = tempfile.mkdtemp(prefix="kas-modes-")
    subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
                   cwd=cwd, shell=True)
    pathlib.Path(cwd, "calc.py").write_text(BUGGY)
    pathlib.Path(cwd, "README.md").write_text("# probe workspace\n")
    subprocess.run("git add -A && git commit -qm baseline", cwd=cwd, shell=True)
    return cwd


def run_mode(mode, prompt, turn_timeout):
    cwd = seed_workspace()
    proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=cwd,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    assert proc.stdin and proc.stdout
    PIN, POUT = proc.stdin, proc.stdout
    msgs = queue.Queue()
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)),
                     daemon=True).start()
    _id = [0]
    frames, tool_calls, userinputs, perms = [], [], [], []
    update_kinds, used_tools = set(), set()
    turn_ends = [0]
    prompt_response = [None]

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
                    kind = u.get("sessionUpdate"); update_kinds.add(kind)
                    meta = ((u.get("_meta") or {}).get("kiro") or {})
                    if kind == "tool_call":
                        tool_calls.append({"title": u.get("title"),
                                           "toolId": meta.get("toolId") or meta.get("tool_id"),
                                           "kind": meta.get("kind"), "status": u.get("status")})
                    if kind == "session_info_update" and meta.get("kind") == "turn_end":
                        turn_ends[0] += 1
                        for s in (meta.get("promptTurnSummaries") or []):
                            used_tools.update(s.get("usedTools") or [])
            return
        if m == "_kiro/auth/getAccessToken": reply(rid, read_token()); return
        if m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"}); return
        if m == "_kiro/userInput":
            userinputs.append(p)
            opts = p.get("options", [])
            pick = next((o2 for o2 in opts if isinstance(o2, dict) and o2.get("recommended")),
                        opts[0] if opts else None)
            ans = (pick.get("title") if isinstance(pick, dict) else pick) if pick else "yes"
            log(f"  -> userInput Q={p.get('question')!r} answering {ans!r}")
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

    log(f"\n===== mode={mode} cwd={cwd} =====")
    caps = {"_meta": {"kiro": {"userInput": True}}}
    ir = call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": caps})
    nr = call_sync("session/new", {"cwd": cwd, "mcpServers": []}) if (ir and "result" in ir) else None
    res = (nr or {}).get("result") or {}
    sid = res.get("sessionId")
    cfg = res.get("configOptions") or []
    mode_opt = next((c for c in cfg if c.get("id") == "mode"), {})
    # 2.10.0 option entries are {value, name} — value is the settable id
    mode_ids = [v.get("value") or v.get("id") for v in (mode_opt.get("options") or [])]
    log(f"  session={sid}  modes advertised: {mode_ids}")

    setres = None
    if sid and mode in mode_ids:
        sr = call_sync("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": mode})
        setres = (sr or {}).get("result")
        new_cfg = (setres or {}).get("configOptions") if isinstance(setres, dict) else setres
        cur = next((c.get("currentValue") for c in (new_cfg or cfg or []) if c.get("id") == "mode"), None) \
            if isinstance(new_cfg, list) else None
        log(f"  set_config_option mode={mode} -> response currentValue={cur!r}")
    elif sid:
        log(f"  !! mode {mode!r} not in advertised options — sending set anyway to record behavior")
        sr = call_sync("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": mode})
        setres = (sr or {}).get("result")

    timed_out = False
    if sid:
        pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
        end = time.time() + turn_timeout
        while time.time() < end and turn_ends[0] == 0:
            try: raw = msgs.get(timeout=2)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
            elif o.get("id") == pid:
                prompt_response[0] = o.get("result") or o.get("error")
                log(f"  prompt response: {json.dumps(prompt_response[0])[:200]}")
        if turn_ends[0] == 0 and prompt_response[0] is None:
            timed_out = True
            log(f"  !! TIMEOUT after {turn_timeout}s (no turn_end, no prompt response)")
        else:
            # grace drain for trailing notifications
            g = time.time() + 8
            while time.time() < g:
                try: raw = msgs.get(timeout=1)
                except queue.Empty: continue
                if raw is None: break
                try: o = json.loads(raw)
                except Exception: continue
                if "method" in o: handle(o)

    # ---- filesystem oracle ----
    porcelain = subprocess.run("git status --porcelain", cwd=cwd, shell=True,
                               capture_output=True, text=True).stdout.strip()
    calc_now = pathlib.Path(cwd, "calc.py").read_text()
    kiro_tree = [str(p.relative_to(cwd)) for p in pathlib.Path(cwd).rglob("*")
                 if ".git" not in p.parts and p.is_file()]
    calc_changed = calc_now != BUGGY
    log(f"  oracle: calc.py changed={calc_changed}  git-porcelain={porcelain!r}")
    log(f"  oracle: files now: {sorted(kiro_tree)}")
    log(f"  tool_calls={len(tool_calls)}  userInputs={len(userinputs)}  perms={perms}")
    log(f"  usedTools={sorted(used_tools)}  updateKinds={sorted(k for k in update_kinds if k)}")

    dump = {
        "mode": mode, "prompt": prompt, "cwd": cwd, "timed_out": timed_out,
        "initialize_result": (ir or {}).get("result"),
        "session_new_result": res,
        "set_config_option_result": setres,
        "prompt_response": prompt_response[0],
        "tool_calls": tool_calls, "userinputs": userinputs, "permissions": perms,
        "used_tools": sorted(used_tools),
        "update_kinds": sorted(k for k in update_kinds if k),
        "oracle": {"calc_changed": calc_changed, "git_porcelain": porcelain,
                   "files": sorted(kiro_tree), "calc_now": calc_now},
        "frames": frames,
    }
    path = os.path.join(OUTDIR, f"{mode}.json")
    pathlib.Path(path).write_text(json.dumps(dump, indent=2) + "\n")
    log(f"  dump: {path}")

    try: PIN.close()
    except Exception: pass
    proc.terminate()
    return dump


def main():
    log(f"# binary={KIRO}")
    results = {}
    for mode, prompt, to in RUNS:
        results[mode] = run_mode(mode, prompt, to)
        time.sleep(3)

    log("\n===== SUMMARY =====")
    for mode, d in results.items():
        o = d["oracle"]
        log(f"  {mode:11s} timed_out={d['timed_out']} calc_changed={o['calc_changed']} "
            f"userInputs={len(d['userinputs'])} usedTools={d['used_tools']} "
            f"new_files={[f for f in o['files'] if f not in ('calc.py', 'README.md')]}")


if __name__ == "__main__":
    main()

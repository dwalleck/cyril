#!/usr/bin/env python3
"""
KAS 0.52.1 (kiro-cli 2.19.2) dark-feature probe: AB_MEMORY_EXTERNAL / persistent agent memory.

The memory subsystem (memory tool list/get/add/update/delete over
~/.kiro/memories/memories.jsonl, a "# Memory" index injected into msg0 at
session start, the remote `searchMemories` tool, and — new in 0.52.1 — a
turn-end memory-EXTRACTION subagent) is gated by
  experimentValue = AB_MEMORY_INTERNAL !== "disabled" ? INTERNAL : AB_MEMORY_EXTERNAL
  enabled = (bool ? value : value=="all" || (value=="insider" && insider channel)) && userOptIn !== false
Defaults are off for everyone; 0.52.1 adds the env override
KIRO_FEATURE_MEMORY_EXTERNAL_ENABLED (honored "in both directions").

Arms (each in its own throwaway HOME, real XDG_DATA_HOME for auth):
  on  — KIRO_FEATURE_MEMORY_EXTERNAL_ENABLED=true
        session#1: turn A "remember: I prefer tabs" (expect memory add tool call)
                   turn B one more fact (pushes the extraction watermark past 4 msgs)
                   wait for the extraction subagent (90 s timeout in KAS) → kiro.log memory.extraction.*
        session#2: one cheap turn → KIRO_DUMP_REQUESTS shows whether msg0 carries "# Memory" + the index
  off — control: session#1 turn A only.
Observables per arm: _kiro/tools/didChange roster (memory tool?), every tool_call
title/rawInput, all notification methods, the request dumps (memory index in
history[0], `memory`/`searchMemories` in the tool roster), ~/.kiro/memories/*
under the fake HOME, and kiro.log memory.* lines.

    probe-kas-memory-external-2.19.2.py [on|off|both]   (default both)
"""
import glob, json, os, re, signal, subprocess, sys, tempfile, threading, queue, time, sqlite3

SCRATCH = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
KIRO = os.path.expanduser("~/.local/bin/kiro-cli")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def profile_arn():
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()

def read_token():
    c = sqlite3.connect(AUTH_DB)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally:
        c.close()
    if not row:
        return None
    v = row[0]
    v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}


def run_arm(arm):
    fake_home = tempfile.mkdtemp(prefix=f"mem-{arm}-home-")
    cwd = tempfile.mkdtemp(prefix=f"mem-{arm}-ws-")
    subprocess.run("git init -q -b main && git remote add origin https://example.invalid/cyril-audit/memprobe.git", cwd=cwd, shell=True)
    dumps = os.path.join(fake_home, "dumps")
    env = dict(os.environ)
    env["HOME"] = fake_home
    env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")
    env["KIRO_DUMP_REQUESTS"] = "1"
    env["KIRO_DUMP_REQUESTS_DIR"] = dumps
    env.pop("KIRO_FEATURE_MEMORY_EXTERNAL_ENABLED", None)
    if arm == "on":
        env["KIRO_FEATURE_MEMORY_EXTERNAL_ENABLED"] = "true"
    trace_path = os.path.join(SCRATCH, f"kas-memory-external-{arm}-2.19.2-trace.jsonl")
    trace = open(trace_path, "w")
    proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=cwd, env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=open(os.path.join(SCRATCH, f"mem-{arm}-stderr.log"), "w"),
                            text=True, bufsize=1, start_new_session=True)
    assert proc.stdin and proc.stdout
    msgs = queue.Queue()
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in proc.stdout if l.strip()], msgs.put(None)), daemon=True).start()
    ids = [10]
    notifs = []

    def record(direction, obj):
        trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": obj}) + "\n"); trace.flush()

    def send(obj):
        record("client->agent", obj)
        proc.stdin.write(json.dumps(obj) + "\n"); proc.stdin.flush()

    def req(m, p):
        ids[0] += 1
        send({"jsonrpc": "2.0", "id": ids[0], "method": m, "params": p}); return ids[0]

    def handle(o):
        m = o["method"]
        if m == "_kiro/auth/getAccessToken":
            send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
        elif m == "session/request_permission":
            opts = o.get("params", {}).get("options", [])
            pick = next((x for x in opts if x.get("kind") == "allow_once"), opts[0] if opts else None)
            res = {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
            send({"jsonrpc": "2.0", "id": o["id"], "result": res})
        elif m == "_kiro/terminal/shell_type":
            send({"jsonrpc": "2.0", "id": o["id"], "result": {"shellType": "bash"}})
        else:
            send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

    def pump(until_id=None, timeout=60, idle_exit=None):
        end = time.time() + timeout; last = time.time()
        while time.time() < end:
            try:
                raw = msgs.get(timeout=1)
            except queue.Empty:
                if idle_exit and time.time() - last > idle_exit:
                    return None
                continue
            if raw is None:
                return None
            last = time.time()
            try:
                o = json.loads(raw)
            except Exception:
                continue
            record("agent->client", o)
            if "method" in o and "id" in o:
                handle(o)
            elif "method" in o:
                notifs.append(o)
            elif "id" in o and until_id is not None and o["id"] == until_id:
                return o
        return None

    print(f"\n######## arm={arm}  HOME={fake_home}")
    pump(req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                            "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}}), 60)
    new = pump(req("session/new", {"cwd": cwd, "mcpServers": []}), 60)
    res = (new or {}).get("result") or {}
    sid = res.get("sessionId")
    meta = res.get("_meta") or {}
    print("session#1:", sid, "| _meta keys:", sorted(meta.keys()))
    print("   memory-ish _meta:", {k: v for k, v in meta.items() if "emor" in k})
    pump(timeout=8, idle_exit=4)

    def tool_roster():
        for n in notifs:
            if n["method"] == "_kiro/tools/didChange":
                s = json.dumps(n["params"])
                names = sorted(set(re.findall(r'"name":\s*"([a-zA-Z_]+)"', s)))
                return names, ("memory" in names), ("searchMemories" in s or "search_memories" in s)
        return [], False, False

    names, has_mem, has_search = tool_roster()
    print(f"   tools/didChange roster ({len(names)}): memory={has_mem} searchMemories={has_search} | {names}")

    def turn(text, to=300):
        t0 = time.time()
        r = pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": text}]}), to)
        print(f"   turn: {json.dumps((r or {}).get('result') or (r or {}).get('error'))[:120]} ({time.time()-t0:.1f}s)")

    turn("Please remember this for future sessions: I prefer TABS over spaces in all code, and I want commit messages in imperative mood. "
         "Use your memory tool to store it (if you have one), then reply with exactly: SAVED")
    if arm == "on":
        turn("One more durable fact about this project: the build command is `cargo build --release` and tests run with `cargo nextest run`. "
             "Store it too, then reply with exactly: OK")
        # let the turn-end extraction subagent (MEMORY_EXTRACTION_TIMEOUT_MS=90s) run
        print("   waiting up to 100 s for memory extraction …")
        pump(timeout=100, idle_exit=45)

    # tool calls seen on the wire (main + any subagent session)
    print("   tool_calls on the wire:")
    for n in notifs:
        if n["method"] == "session/update":
            u = n["params"]["update"]
            if u.get("sessionUpdate") == "tool_call":
                kiro = ((u.get("_meta") or {}).get("kiro") or {})
                print(f"     sid={n['params']['sessionId'][:14]} title={u.get('title')!r} rawInput={json.dumps(u.get('rawInput'))[:160]} kind={kiro.get('kind')}")

    if arm == "on":
        new2 = pump(req("session/new", {"cwd": cwd, "mcpServers": []}), 60)
        sid2 = ((new2 or {}).get("result") or {}).get("sessionId")
        print("session#2:", sid2)
        pump(timeout=6, idle_exit=3)
        t0 = time.time()
        r = pump(req("session/prompt", {"sessionId": sid2, "prompt": [{"type": "text", "text": "What do you remember about my preferences? Answer in one line."}]}), 240)
        print(f"   turn: {json.dumps((r or {}).get('result') or (r or {}).get('error'))[:120]} ({time.time()-t0:.1f}s)")
        text = "".join((n["params"]["update"].get("content") or {}).get("text", "") for n in notifs
                       if n["method"] == "session/update" and n["params"]["sessionId"] == sid2
                       and n["params"]["update"].get("sessionUpdate") == "agent_message_chunk")
        print("   session#2 answer:", repr(text[:300]))
        pump(timeout=6, idle_exit=3)

    trace.close()
    try:
        os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
    except Exception:
        proc.terminate()
    time.sleep(1)

    print("   notification methods:", sorted({n["method"] for n in notifs}))
    # on-disk store
    print("   ~/.kiro/memories under fake HOME:")
    for p in glob.glob(os.path.join(fake_home, ".kiro", "memories", "**", "*"), recursive=True):
        if os.path.isfile(p):
            print(f"     {os.path.relpath(p, fake_home)} ({os.path.getsize(p)} B)")
            for line in open(p, errors="replace").read().splitlines()[:6]:
                print("       ", line[:300])
    # request dumps: msg0 memory block + tool roster
    files = sorted(glob.glob(os.path.join(dumps, "**", "*.json"), recursive=True))
    print(f"   request dumps: {len(files)}")
    for f in files:
        try:
            d = json.load(open(f))
        except Exception:
            continue
        s = json.dumps(d)
        conv = ((d.get("request") or {}).get("conversationState") or {})
        hist = conv.get("history") or []
        msg0 = json.dumps(hist[0]) if hist else ""
        tools = sorted(set(re.findall(r'"toolSpecification":\s*\{"name":\s*"([^"]+)"', s)))
        agent = (d.get("invocation") or {}).get("agentName")
        has_block = "# Memory" in msg0 or "<index>" in msg0
        idx = None
        m = re.search(r"<index>(.*?)</index>", msg0.encode().decode("unicode_escape"), re.S) if has_block else None
        if m:
            idx = m.group(1).strip()[:400]
        print(f"     {os.path.relpath(f, dumps)}: agent={agent} msg0_memory_block={has_block} memory_tool={'memory' in tools} searchMemories={'searchMemories' in s} tools={len(tools)}")
        if idx is not None:
            print("        index:", repr(idx))
    # kiro.log memory lines
    print("   kiro.log memory.* lines:")
    for lg in glob.glob(os.path.join(fake_home, ".kiro", "logs", "*", "kiro.log")):
        for line in open(lg, errors="replace"):
            if re.search(r'"memory\.|memory\.extraction|memory\.injection|MemoryExtraction|\[Memory\]', line):
                try:
                    o = json.loads(line); print("     ", o.get("level"), o.get("message")[:230])
                except Exception:
                    print("     ", line[:230].rstrip())
    print("   TRACE:", trace_path)


arms = sys.argv[1] if len(sys.argv) > 1 else "both"
for arm in (["on", "off"] if arms == "both" else [arms]):
    run_arm(arm)

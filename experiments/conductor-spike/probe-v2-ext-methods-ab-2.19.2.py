#!/usr/bin/env python3
"""
v2 (Rust) engine A/B for the 2.19.2 audit — same-day, two binaries, one backend.

2.19.2's kiro-cli-chat gains `chat_cli_v2::agent::acp::extension_request::
{ExtensionRequestKind::{parse,recognize}, ProductionExecutor, respond}` — a new
extension-request dispatcher whose string table spells out:
  _kiro.dev/commands/execute, _kiro.dev/commands/options,
  _kiro.dev/session/list, _kiro.dev/session/terminate,
  _kiro.dev/settings/list, _kiro.dev/settings/set,
  _session/steer, _session/steer/clear, _session/spawn, _message/send
Several of those full names have ZERO string hits in 2.19.1 (they were built by
concatenation), so the static diff cannot say whether any method is NEW.
This probe calls each one on both binaries and records the response class:
  -32601 (method not found) / domain error / result.

Also captures the ordinary v2 turn baseline on both (session/new result,
commands/available, one cheap prompt -> _kiro.dev/metadata) and prints a
field-path set diff so binary-side wire drift shows up.

    probe-v2-ext-methods-ab-2.19.2.py <kiro-cli-chat-OLD> <kiro-cli-chat-NEW> <out-prefix>

HOME-isolated (real XDG_DATA_HOME keeps v2 auth reachable). The settings/set
call uses a NONSENSE key so nothing real is written.
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

OLD, NEW, PREFIX = sys.argv[1], sys.argv[2], sys.argv[3]


def paths(o, p=""):
    out = set()
    if isinstance(o, dict):
        for k, v in o.items():
            out |= paths(v, f"{p}.{k}" if p else k)
    elif isinstance(o, list):
        for v in o[:3]:
            out |= paths(v, p + "[]")
    else:
        out.add(p)
    return out


def run(binpath, label):
    out = open(f"{PREFIX}-{label}.jsonl", "w")
    CWD = tempfile.mkdtemp(prefix=f"v2ab-{label}-")
    subprocess.run("git init -q -b main", cwd=CWD, shell=True)
    env = dict(os.environ)
    env["HOME"] = tempfile.mkdtemp(prefix=f"v2abhome-{label}-")
    env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")
    state = {}
    ids = [0]
    frames = []
    q = queue.Queue()

    def spawn():
        pr = subprocess.Popen([binpath, "acp", "--agent-engine", "v2"], cwd=CWD, env=env,
                              stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                              stderr=open(f"{PREFIX}-{label}-stderr.log", "a"),
                              text=True, bufsize=1)
        state["p"] = pr
        threading.Thread(target=lambda: ([q.put(l.strip()) for l in pr.stdout if l.strip()], q.put(None)), daemon=True).start()
        return pr

    p = spawn()

    def rec(d, m):
        frames.append({"dir": d, "msg": m})
        out.write(json.dumps({"ts": time.time(), "dir": d, "msg": m}) + "\n"); out.flush()

    def send(m):
        rec("client->agent", m)
        try:
            state["p"].stdin.write(json.dumps(m) + "\n"); state["p"].stdin.flush()
        except BrokenPipeError:
            state["died"] = True

    def req(method, params):
        ids[0] += 1
        send({"jsonrpc": "2.0", "id": ids[0], "method": method, "params": params}); return ids[0]

    def pump(until=None, to=90, idle=None):
        end = time.time() + to; last = time.time()
        while time.time() < end:
            try:
                raw = q.get(timeout=1)
            except queue.Empty:
                if idle and time.time() - last > idle:
                    return None
                continue
            if raw is None:
                state["died"] = True
                return None
            last = time.time()
            try:
                m = json.loads(raw)
            except Exception:
                continue
            rec("agent->client", m)
            if "method" in m and "id" in m:
                if m["method"] == "session/request_permission":
                    opts = m["params"].get("options", [])
                    pick = next((o for o in opts if o.get("kind") == "allow_once"), opts[0] if opts else None)
                    send({"jsonrpc": "2.0", "id": m["id"], "result": {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}})
                else:
                    send({"jsonrpc": "2.0", "id": m["id"], "result": {}})
            elif "id" in m and until is not None and m["id"] == until:
                return m
        return None

    def classify(r):
        if state.get("died") or state["p"].poll() is not None:
            return f"AGENT DIED (exit={state['p'].poll()})"
        if r is None:
            return "TIMEOUT"
        if "error" in r:
            e = r["error"]
            return f"ERROR code={e.get('code')} msg={str(e.get('message'))[:90]!r} data={json.dumps(e.get('data'))[:120]}"
        return "RESULT " + json.dumps(r.get("result"))[:220]

    print(f"\n######## {label}: {binpath}")
    sid_box = {}

    def handshake(verbose):
        state.pop("died", None)
        iid = req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"}, "clientCapabilities": {}})
        init = pump(iid, 60)
        if verbose:
            print("initialize:", json.dumps((init or {}).get("result"))[:400])
        nid = req("session/new", {"cwd": CWD, "mcpServers": []})
        new = pump(nid, 90)
        res = (new or {}).get("result") or {}
        sid_box["sid"] = res.get("sessionId")
        if verbose:
            print("session/new keys:", sorted(res.keys()), "| modes:", json.dumps(res.get("modes"))[:200], "| configOptions:", json.dumps(res.get("configOptions"))[:100])
        pump(to=6, idle=3)

    handshake(True)
    sid = sid_box["sid"]

    results = {}
    calls = [
        ("_kiro.dev/commands/options", {"sessionId": "<sid>", "command": "model"}),
        ("_kiro.dev/settings/list", {}),
        ("_kiro.dev/settings/list", {"sessionId": "<sid>"}),
        ("_kiro.dev/settings/set", {"key": "probe.nonexistent.key", "value": True}),
        ("_kiro.dev/settings/set", {"sessionId": "<sid>", "key": "probe.nonexistent.key", "value": True}),
        ("_kiro.dev/session/list", {"cwd": CWD}),
        ("_kiro.dev/session/list", {}),
        ("_kiro.dev/session/terminate", {"sessionId": "00000000-0000-4000-8000-00000000dead"}),
        ("_session/steer/clear", {"sessionId": "<sid>"}),
        ("_message/send", {"sessionId": "00000000-0000-4000-8000-00000000dead", "message": "probe"}),
        ("_kiro.dev/nonexistent/method", {}),  # negative control: what does a truly unknown method return?
    ]
    for method, params in calls:
        params = {k: (sid if v == "<sid>" else v) for k, v in params.items()}
        rid = req(method, params)
        r = pump(rid, 30)
        c = classify(r)
        results[f"{method} {json.dumps(params).replace(sid or '', '<sid>')[:60]}"] = c
        print(f"  {method:32} {json.dumps(params).replace(sid or '', '<sid>')[:70]:70} -> {c}")
        if state.get("died") or state["p"].poll() is not None:
            print("     !! agent process died — respawning for the remaining calls")
            while not q.empty():
                q.get()
            spawn(); handshake(False); sid = sid_box["sid"]

    t0 = time.time()
    pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Reply with exactly: OK"}]})
    resp = pump(pid, 240)
    print("prompt:", classify(resp), f"({time.time()-t0:.1f}s)")
    pump(to=6, idle=3)
    state["p"].terminate()
    out.close()
    # field-path inventory of agent->client frames by method/kind
    inv = {}
    for f in frames:
        if f["dir"] != "agent->client":
            continue
        m = f["msg"]
        key = m.get("method") or "response"
        if key == "session/update":
            key += ":" + str((m.get("params") or {}).get("update", {}).get("sessionUpdate"))
        if key == "_kiro.dev/session/update":
            key += ":" + str((m.get("params") or {}).get("update", {}).get("sessionUpdate"))
        inv.setdefault(key, set()).update(paths(m.get("params") if "params" in m else (m.get("result") or m.get("error") or {})))
    return results, inv


ro, io = run(OLD, "old")
rn, inv_new = run(NEW, "new")
print("\n######## A/B method verdicts (old -> new)")
for k in rn:
    a, b = ro.get(k, "?"), rn[k]
    flag = "SAME" if a.split(" ")[0] == b.split(" ")[0] and (a[:60] == b[:60]) else "DIFF"
    print(f"  [{flag}] {k}\n        old: {a}\n        new: {b}")
print("\n######## field-path drift (agent->client), per method/kind")
for key in sorted(set(io) | set(inv_new)):
    a, b = io.get(key, set()), inv_new.get(key, set())
    if a != b:
        print(f"  {key}: +{sorted(b - a)} -{sorted(a - b)}")
    else:
        print(f"  {key}: identical ({len(a)} paths)")

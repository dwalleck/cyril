#!/usr/bin/env python3
"""
Probe: does KAS call `_kiro/fs/*` (or base ACP `fs/*`) host callbacks, and what
client advertisement flips it out of in-process fs? Answers cyril's KAS-5
(fs-callback responder) implementation question against a live, directly-spawned
KAS server (the free path from probe-kas-direct-spawn-2.8.1.py).

GATING (read from acp-server.js resolveCapabilities @379683):
  - clientCapabilities.fs.readTextFile===true        -> base   fs/read_text_file
  - clientCapabilities.fs.writeTextFile===true       -> base   fs/write_text_file
  - clientCapabilities.fs._meta.kiro.readFile===true -> superset _kiro/fs/read_file
  - clientCapabilities.fs._meta.kiro.writeFile===true-> superset _kiro/fs/write_file
  - ...stat/readDirectory/delete                     -> _kiro/fs/{stat,read_directory,delete}
  - nothing advertised                               -> in-process (agent touches disk)

Three runs, identical prompt (create a file, then read it back):
  A) no fs caps          -> expect ZERO callbacks, file written in-process to cwd
  B) base fs caps        -> expect fs/read_text_file + fs/write_text_file
  C) base + kiro fs caps -> expect _kiro/fs/* superset (does enhanced win over base?)

The responder performs REAL disk I/O so the agent's read-after-write is consistent
— it is, in effect, the host-side fs responder cyril must implement for KAS-5.

Run: python3 probe-kas-fs-callbacks-2.8.1.py
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

SERVER = os.path.expanduser(
    "~/.local/share/kiro-cli/kas/node_modules/@kiro/agent/dist/server/acp-server.js"
)
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-fs-callbacks-2.8.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

PROMPT = ("Create a file named probe_kas.txt in the current working directory whose "
          "exact contents are the single line: KAS_FS_PROBE_OK . Then read that file "
          "back and tell me its contents. Use your file tools.")

def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try:
            row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
        finally:
            c.close()
    except Exception:
        return None
    if not row:
        return None
    v = row[0]; v = v.decode("utf-8", "replace") if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
            "profileArn": d.get("profile_arn"), "provider": d.get("provider")}

# --- the host-side fs responder (this is the KAS-5 surface cyril must implement) -
def fs_handle(method, params):
    """Perform real disk I/O for an fs host callback; return the ACP response."""
    path = params.get("path", "")
    if method in ("fs/read_text_file", "_kiro/fs/read_file"):
        with open(path, "r", encoding="utf-8") as fh:
            content = fh.read()
        line, limit = params.get("line"), params.get("limit")
        if line or limit:
            lines = content.split("\n")
            start = (line - 1) if line else 0
            content = "\n".join(lines[start:start + limit] if limit else lines[start:])
        return {"content": content}
    if method in ("fs/write_text_file", "_kiro/fs/write_file"):
        rng = ((params.get("_meta") or {}).get("kiro") or {}).get("range")
        if rng is not None and os.path.exists(path):
            # range splice (strReplace/insert path) — keep responder honest
            with open(path, "r", encoding="utf-8") as fh:
                old = fh.read().split("\n")
            s = rng.get("start", {}); e = rng.get("end", {})
            sl, el = s.get("line", 0), e.get("line", len(old))
            old[sl:el] = params.get("content", "").split("\n")
            data = "\n".join(old)
        else:
            data = params.get("content", "")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(data)
        return {}
    if method == "_kiro/fs/stat":
        st = os.stat(path)
        ftype = "directory" if os.path.isdir(path) else ("symlink" if os.path.islink(path) else "file")
        return {"type": ftype, "size": st.st_size}
    if method == "_kiro/fs/read_directory":
        entries = []
        for name in os.listdir(path):
            full = os.path.join(path, name)
            t = "directory" if os.path.isdir(full) else ("symlink" if os.path.islink(full) else "file")
            entries.append({"name": name, "type": t})
        return {"entries": entries}
    if method == "_kiro/fs/delete":
        if os.path.isdir(path):
            import shutil; shutil.rmtree(path)
        elif os.path.exists(path):
            os.remove(path)
        return {}
    return {}

FS_METHODS = {"fs/read_text_file", "fs/write_text_file", "_kiro/fs/read_file",
              "_kiro/fs/write_file", "_kiro/fs/stat", "_kiro/fs/read_directory", "_kiro/fs/delete"}

def run(label, client_caps):
    cwd = tempfile.mkdtemp(prefix=f"kas-fs-{label}-")
    target = os.path.join(cwd, "probe_kas.txt")
    argv = ["node", "--experimental-wasm-modules", SERVER, "--transport=stdio"]
    proc = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    PIN, POUT = proc.stdin, proc.stdout
    assert PIN and POUT
    msgs = queue.Queue()
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)),
                     daemon=True).start()
    _id = [0]
    def req(method, params):
        _id[0] += 1
        PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": method, "params": params}) + "\n"); PIN.flush()
        return _id[0]
    def reply(rid, res):
        PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()

    obs = {"fs_calls": {}, "fs_sample": {}, "getAccessToken": 0, "other_inbound": {}, "stop": None}
    def pump(until_id, timeout=180):
        end = time.time() + timeout
        while time.time() < end:
            try:
                raw = msgs.get(timeout=2)
            except queue.Empty:
                continue
            if raw is None:
                return None
            try:
                o = json.loads(raw)
            except Exception:
                continue
            if "method" in o and "id" in o:                 # agent -> client request
                m = o["method"]; p = o.get("params", {})
                if m in FS_METHODS:
                    obs["fs_calls"][m] = obs["fs_calls"].get(m, 0) + 1
                    if m not in obs["fs_sample"]:
                        obs["fs_sample"][m] = {k: p.get(k) for k in ("path", "line", "limit", "content", "_meta") if k in p}
                    try:
                        reply(o["id"], fs_handle(m, p))
                    except Exception as e:
                        reply(o["id"], {"message": f"host fs error: {e}"})
                elif m == "_kiro/auth/getAccessToken":
                    obs["getAccessToken"] += 1
                    reply(o["id"], read_token() or {})
                elif m == "session/request_permission":
                    opts = p.get("options", [])
                    pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                                opts[0] if opts else None)
                    reply(o["id"], {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                          if pick else {"outcome": {"outcome": "cancelled"}})
                else:
                    obs["other_inbound"][m] = obs["other_inbound"].get(m, 0) + 1
                    reply(o["id"], {})
                continue
            if "id" in o and o.get("id") == until_id:
                return o
        return None

    try:
        req("initialize", {"protocolVersion": 1, "clientCapabilities": client_caps}); pump(1, 60)
        nid = req("session/new", {"cwd": cwd, "mcpServers": []})
        nr = pump(nid, 60)
        assert nr and "result" in nr, f"session/new failed: {nr}"
        sid = nr["result"]["sessionId"]
        pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
        pr = pump(pid, 240)
        obs["stop"] = pr.get("result", {}).get("stopReason") if pr and "result" in pr else (
            "ERROR:" + json.dumps(pr.get("error")) if pr and "error" in pr else "timeout")
    finally:
        try: PIN.close()
        except Exception: pass
        proc.terminate()
        try: proc.wait(timeout=5)
        except Exception: proc.kill()

    obs["file_on_disk"] = os.path.exists(target)
    obs["file_content"] = (open(target).read().strip() if obs["file_on_disk"] else None)
    return obs

RUNS = [
    ("A-none", {}),
    ("B-base", {"fs": {"readTextFile": True, "writeTextFile": True}}),
    ("C-kiro", {"fs": {"readTextFile": True, "writeTextFile": True,
                       "_meta": {"kiro": {"readFile": True, "writeFile": True,
                                          "stat": True, "readDirectory": True, "delete": True}}}}),
]

results = {}
for label, caps in RUNS:
    log(f"\n{'='*60}\nRUN {label}  clientCapabilities={json.dumps(caps)}\n{'='*60}")
    r = run(label, caps)
    results[label] = r
    log(f"  stopReason       : {r['stop']}")
    log(f"  getAccessToken   : {r['getAccessToken']} (expect 0 — free path)")
    log(f"  fs callbacks     : {json.dumps(r['fs_calls']) or '{}'}")
    log(f"  other inbound    : {json.dumps(r['other_inbound']) or '{}'}")
    log(f"  file on disk     : {r['file_on_disk']}  content={r['file_content']!r}")
    for m, s in r["fs_sample"].items():
        log(f"    sample {m}: {json.dumps(s)[:300]}")

log(f"\n{'='*60}\nSUMMARY\n{'='*60}")
for label, _ in RUNS:
    r = results[label]
    fams = sorted(r["fs_calls"].keys())
    mode = ("in-process (no fs callbacks)" if not fams
            else "base ACP fs/*" if all(not m.startswith("_kiro") for m in fams)
            else "kiro superset _kiro/fs/*" if all(m.startswith("_kiro") for m in fams)
            else "MIXED base + kiro")
    log(f"  {label:8} -> {mode:32} calls={json.dumps(r['fs_calls'])} disk={r['file_on_disk']} ok={r['file_content']=='KAS_FS_PROBE_OK'}")
log(f"\n# log: {LOG}")

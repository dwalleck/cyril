#!/usr/bin/env python3
"""
EXPERIMENT: Does KAS load a `inclusion: fileMatch` steering doc into context when the
client reports a matching file as "open"?

Design (A/B/C with controls):
  Steering fixtures written into <cwd>/.kiro/steering/ before session/new:
    - always-canary.md   : inclusion: always              -> POSITIVE CONTROL, must load every time
    - tsx-canary.md      : inclusion: fileMatch, **/*.tsx -> THE DOC UNDER TEST

  Three conditions, each a FRESH kiro-cli process (no session/openFiles bleed):
    [match]   open files = [<cwd>/src/App.tsx]   -> expect tsx-canary LOADED
    [nomatch] open files = [<cwd>/src/App.txt]   -> expect tsx-canary NOT loaded (gate is the pattern, not "any file")
    [none]    open files = []                     -> expect tsx-canary NOT loaded (documented bare-CLI behavior)

  Open files are fed through EVERY plausible channel and we log which one KAS consumes:
    1. server->client callback `_kiro/workspace/currently_open_files` (answered with the list)
    2. server->client callback `_kiro/workspace/active_file`
    3. a `_meta.kiro` push on session/new and session/prompt
  Every server->client REQUEST method+shape is logged, so we learn definitively whether
  KAS even asks for open files in a bare-ACP context.

  Oracle (did the doc enter context?):
    - notification `_kiro/steering/documents_changed`  -> {documents:[...]}
    - session/update session_info_update kind `steering_inclusion` -> {steeringDocuments:[...]}
    - notification `_kiro/progressive_context/items_changed` -> the AVAILABLE catalog (sanity: doc is discoverable)

The turn is trivial ("ok") and we cancel as soon as the steering signal arrives, since
populate-steering runs BEFORE the LLM call -- keeps cost ~nil and dodges long Opus thinking.

In-process I/O. Auth token self-sourced from the local sqlite; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, shutil

CANDIDATES = [
    os.environ.get("KIRO_BIN"),
    os.path.expanduser("~/.local/share/kiro-research/binaries/2.8.1/kiro-cli-chat"),
    os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat"),
    shutil.which("kiro-cli-chat"),
    shutil.which("kiro-cli"),
]
KIRO = next((c for c in CANDIDATES if c and os.path.exists(c)), None)
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-filematch-steering.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

if not KIRO:
    log("FATAL: no kiro-cli-chat binary found. Set KIRO_BIN=/path/to/kiro-cli-chat"); raise SystemExit(1)
CMD = [KIRO, "acp", "--agent-engine", "kas"]  # built post-guard so KIRO is narrowed to str
log(f"# binary: {KIRO}")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    if not row:
        return {}
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

def shape(v, depth=0):
    if depth > 6:
        return "..."
    if isinstance(v, dict):
        return {k: shape(x, depth + 1) for k, x in v.items()}
    if isinstance(v, list):
        return ([shape(v[0], depth + 1), f"...(len={len(v)})"] if v else [])
    return type(v).__name__

# ---- steering fixtures (written once into a shared workspace) ----
CWD = tempfile.mkdtemp(prefix="kas-filematch-")
os.makedirs(os.path.join(CWD, ".kiro", "steering"), exist_ok=True)
os.makedirs(os.path.join(CWD, "src"), exist_ok=True)
with open(os.path.join(CWD, ".kiro", "steering", "always-canary.md"), "w") as f:
    f.write("---\ninclusion: always\n---\nALWAYS_CANARY_TOKEN: this steering always applies.\n")
with open(os.path.join(CWD, ".kiro", "steering", "tsx-canary.md"), "w") as f:
    f.write('---\ninclusion: fileMatch\nfileMatchPattern: "**/*.tsx"\n---\nTSX_FILEMATCH_CANARY_TOKEN: applies only when editing .tsx files.\n')
# real files on disk so any existence check passes; abs paths (minimatch ** spans '/')
APP_TSX = os.path.join(CWD, "src", "App.tsx")
APP_TXT = os.path.join(CWD, "src", "App.txt")
open(APP_TSX, "w").write("export const App = () => null;\n")
open(APP_TXT, "w").write("plain text\n")

def ident_list(docs):
    """Pull human identifiers out of a steering documents_changed / steeringDocuments payload."""
    out = []
    for d in (docs or []):
        if isinstance(d, str):
            out.append(d)
        elif isinstance(d, dict):
            out.append(d.get("name") or d.get("path") or d.get("uri") or d.get("title") or json.dumps(d)[:80])
        else:
            out.append(str(d))
    return out

def run_condition(label, open_files):
    proc = subprocess.Popen(CMD, cwd=CWD,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    assert proc.stdin and proc.stdout
    PIN, POUT = proc.stdin, proc.stdout
    msgs = queue.Queue()
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
    _id = [0]
    def req(m, p):
        _id[0] += 1; PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n"); PIN.flush(); return _id[0]
    def reply(rid, res):
        PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()

    captured = {
        "s2c_requests": [],         # every server->client request method (the discovery deliverable)
        "open_files_asked": False,  # did KAS call a workspace open-files/active-file callback?
        "steering_docs_changed": [],# list of identifier-lists over the turn
        "steering_inclusion": [],   # steeringDocuments[] from session_info_update
        "progressive_items": None,  # available catalog
    }
    kiro_open_meta = {"openFiles": open_files, "currentlyOpenFiles": open_files,
                      "activeFile": (open_files[0] if open_files else None)}

    def handle(o):
        m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
        if rid is not None:  # server -> client REQUEST
            captured["s2c_requests"].append({"method": m, "params": shape(p)})
            lm = (m or "").lower()
            if m == "_kiro/auth/getAccessToken":
                reply(rid, read_token())
            elif "open_files" in lm or "currently_open" in lm or "openfiles" in lm:
                captured["open_files_asked"] = True
                # answer with several plausible shapes; KAS reads whichever key it wants
                reply(rid, {"files": open_files, "openFiles": open_files, "paths": open_files, "uris": open_files})
            elif "active_file" in lm or "activefile" in lm:
                captured["open_files_asked"] = True
                af = open_files[0] if open_files else None
                reply(rid, {"activeFile": af, "file": af, "path": af, "uri": af})
            elif m == "_kiro/steering/get_documents":
                reply(rid, {})  # let KAS use its own loader; don't override
            elif m == "_kiro/terminal/shell_type":
                reply(rid, {"shellType": "bash"})
            elif m == "session/request_permission":
                opts = p.get("options", [])
                pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
                reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                reply(rid, {})
            return
        # server -> client NOTIFICATION
        if m == "_kiro/steering/documents_changed":
            captured["steering_docs_changed"].append(ident_list(p.get("documents")))
        elif m == "_kiro/progressive_context/items_changed":
            if captured["progressive_items"] is None:
                captured["progressive_items"] = ident_list(p.get("items"))
        elif m == "session/update" or (m or "").endswith("/session/update"):
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            if isinstance(u, dict) and u.get("sessionUpdate") == "session_info_update":
                kk = (((u.get("_meta") or {}).get("kiro")) or {})
                if kk.get("kind") == "steering_inclusion":
                    captured["steering_inclusion"].append(ident_list(kk.get("steeringDocuments")))

    def pump(until, to=60, stop_on_steering=False):
        end = time.time() + to
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
            if "method" in o:
                handle(o)
                if stop_on_steering and captured["steering_docs_changed"]:
                    return "steering-seen"
                if o.get("id") == until and "result" in o:
                    return o
            elif "id" in o and o["id"] == until:
                return o
        return None

    # handshake: advertise _meta.kiro + standard fs/terminal so KAS will use callbacks
    req("initialize", {
        "protocolVersion": 1,
        "clientCapabilities": {
            "fs": {"readTextFile": True, "writeTextFile": True},
            "terminal": True,
            "_meta": {"kiro": {"hooks": {"enabled": True}, "userInput": False}},
        },
    })
    pump(1, 20)
    nid = req("session/new", {"cwd": CWD, "mcpServers": [], "_meta": {"kiro": kiro_open_meta}})
    nr = pump(nid, 40)
    if not (nr and isinstance(nr, dict) and "result" in nr):
        log(f"[{label}] session/new FAILED: {nr}"); proc.terminate(); return captured
    sid = nr["result"]["sessionId"]
    pid = req("session/prompt", {"sessionId": sid,
                                 "prompt": [{"type": "text", "text": "ok"}],
                                 "_meta": {"kiro": kiro_open_meta}})
    # capture steering early, then bail
    r = pump(pid, 60, stop_on_steering=True)
    if r == "steering-seen":
        try:
            PIN.write(json.dumps({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}}) + "\n"); PIN.flush()
        except Exception:
            pass
        pump(pid, 4)
    PIN.close(); proc.terminate()
    try:
        proc.wait(timeout=5)
    except Exception:
        proc.kill()
    return captured

# ---- run the three conditions ----
conds = [("match", [APP_TSX]), ("nomatch", [APP_TXT]), ("none", [])]
results = {}
for label, files in conds:
    log(f"\n##### condition [{label}] open_files={[os.path.basename(x) for x in files]} #####")
    c = run_condition(label, files)
    results[label] = c
    # de-dup s2c request methods for readability
    methods = sorted({r["method"] for r in c["s2c_requests"] if r["method"]})
    log(f"  server->client request methods seen: {methods}")
    log(f"  open-files/active-file callback invoked by KAS: {c['open_files_asked']}")
    log(f"  progressive_context AVAILABLE catalog: {c['progressive_items']}")
    last_docs = c["steering_docs_changed"][-1] if c["steering_docs_changed"] else []
    log(f"  steering documents_changed (last): {last_docs}")
    log(f"  steering documents_changed (all):  {c['steering_docs_changed']}")
    log(f"  steering_inclusion updates:        {c['steering_inclusion']}")

# ---- verdict ----
def loaded(label, token):
    c = results[label]
    pool = []
    for lst in c["steering_docs_changed"]:
        pool += lst
    for lst in c["steering_inclusion"]:
        pool += lst
    return any(token in s for s in pool)

log("\n===== VERDICT =====")
for label, _ in conds:
    a = loaded(label, "always")
    t = loaded(label, "tsx")
    log(f"  [{label:7s}] always-canary loaded={a}   tsx-fileMatch loaded={t}")
log("\nExpected if fileMatch fires from client-reported open files:")
log("  always: True/True? (always=True everywhere)   tsx -> True only in [match]")
log("If tsx is False in [match] too: either KAS never asked for open files (see callback log)")
log("  or our open-files channel/shape was wrong (see server->client request methods).")
log(f"\n# full log: {LOG}")
logf.close()

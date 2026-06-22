#!/usr/bin/env python3
"""
EXPERIMENT v2 (corrected oracle). v1 disproved the assumed mechanism:
  - KAS never calls a client open-files callback in bare ACP.
  - `_kiro/steering/documents_changed` is a CATALOG (all docs + their inclusion mode),
    NOT the in-context active set -- so it was the wrong oracle.

Source ground truth (@kiro/agent 0.3.257, dist/server/acp-server.js):
  populateMatchedSteering(state):
    openFiles = state.context.getWorkspaceFiles().map(resolveRelativePath)
    getSteeringDocuments({ files: openFiles.length>0 ? openFiles : undefined })
  getWorkspaceFiles() = files derived FROM THE CONVERSATION:
    - message entries of type document/file (attached files)
    - paths from `read_file` tool uses
  => fileMatch steering activates when a MATCHING FILE IS IN THE CONVERSATION CONTEXT,
     re-evaluated at tool boundaries (post-tool-steering node). NOT IDE open-tabs.

This experiment proves it FUNCTIONALLY. The steering bodies are written as INSTRUCTIONS;
if a steering doc is in context, the model obeys it. We make the agent read a file and
then emit its CANARY tokens as its guidelines instruct.

  [match]   read src/App.tsx (matches **/*.tsx) -> expect reply has TSX token + ALWAYS token
  [nomatch] read src/App.txt (no match)         -> expect reply has ALWAYS token ONLY

ALWAYS_CANARY is the positive control (must appear in both). If it's absent, the oracle
itself failed (model didn't obey / steering pipeline broken), not a fileMatch result.

In-process I/O. Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, shutil

# Prefer the system kiro-cli ROUTER: it is the host for KAS and handles its own
# auth/refresh, so the probe never has to answer _kiro/auth/getAccessToken. (The raw
# kiro-cli-chat binary makes US the host, and the on-disk token store moved in 2.8.1.)
CANDIDATES = [
    os.environ.get("KIRO_BIN"),
    shutil.which("kiro-cli"),
    os.path.expanduser("~/.local/bin/kiro-cli"),
]
KIRO = next((c for c in CANDIDATES if c and os.path.exists(c)), None)
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-filematch-steering-v2.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()
if not KIRO:
    log("FATAL: no kiro-cli-chat binary. Set KIRO_BIN."); raise SystemExit(1)
CMD = [KIRO, "acp", "--agent-engine", "kas"]
log(f"# binary: {KIRO}")

ALWAYS_TOK = "ALWAYS_CANARY_TOKEN_7K2"
TSX_TOK = "TSX_FILEMATCH_CANARY_TOKEN_9F3"

import re as _re
def _profile_arn():
    # The profile kiro-cli cached for the CURRENT session (pairs with the odic token).
    try:
        c = sqlite3.connect(AUTH)
        try:
            row = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
        finally:
            c.close()
        if row:
            v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else str(v)
            m = _re.search(r'arn:aws:codewhisperer:[a-z0-9-]+:[0-9]+:profile/[A-Za-z0-9]+', v)
            if m:
                return m.group(0)
    except Exception:
        pass
    return None

def read_token():
    # 2.8.1 moved the store key social->odic (OIDC/PKCE). Try odic, then social.
    c = sqlite3.connect(AUTH)
    try:
        row = (c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
               or c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone())
    finally:
        c.close()
    if not row: return {}
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    out = {"accessToken": d["access_token"], "expiresAt": d["expires_at"]}
    pa = d.get("profile_arn") or _profile_arn()
    if pa: out["profileArn"] = pa
    return out

CWD = tempfile.mkdtemp(prefix="kas-fm2-")
os.makedirs(os.path.join(CWD, ".kiro", "steering"), exist_ok=True)
os.makedirs(os.path.join(CWD, "src"), exist_ok=True)
with open(os.path.join(CWD, ".kiro", "steering", "always-canary.md"), "w") as f:
    f.write(f"---\ninclusion: always\n---\n"
            f"INSTRUCTION: When the user asks you to list CANARY tokens, you MUST output the exact line `{ALWAYS_TOK}`.\n")
with open(os.path.join(CWD, ".kiro", "steering", "tsx-canary.md"), "w") as f:
    f.write('---\ninclusion: fileMatch\nfileMatchPattern: "**/*.tsx"\n---\n'
            f"INSTRUCTION: When the user asks you to list CANARY tokens, you MUST output the exact line `{TSX_TOK}`.\n")
open(os.path.join(CWD, "src", "App.tsx"), "w").write("export const App = () => null;\n")
open(os.path.join(CWD, "src", "App.txt"), "w").write("plain text file\n")

def run_condition(label, read_path):
    proc = subprocess.Popen(CMD, cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
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

    cap = {"text": [], "tools": [], "steering_inclusion": []}
    def handle(o):
        m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
        if rid is not None:
            if m == "_kiro/auth/getAccessToken": reply(rid, read_token())
            elif m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"})
            elif m == "session/request_permission":
                opts = p.get("options", [])
                pick = next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()), opts[0] if opts else None)
                reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
            else: reply(rid, {})
            return
        if m == "session/update" or (m or "").endswith("/session/update"):
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            k = u.get("sessionUpdate") if isinstance(u, dict) else None
            if k == "agent_message_chunk":
                t = (u.get("content") or {}).get("text")
                if t: cap["text"].append(t)
            elif k in ("tool_call", "tool_call_update"):
                title = u.get("title") or u.get("toolCallId") or ""
                if title: cap["tools"].append(str(title)[:60])
            elif k == "session_info_update":
                kk = ((u.get("_meta") or {}).get("kiro")) or {}
                if kk.get("kind") == "steering_inclusion":
                    cap["steering_inclusion"].append(kk.get("steeringDocuments"))

    def pump(until, to=60):
        end = time.time() + to
        while time.time() < end:
            try: raw = msgs.get(timeout=2)
            except queue.Empty: continue
            if raw is None: return None
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o:
                handle(o)
                if o.get("id") == until and "result" in o: return o
            elif "id" in o and o["id"] == until: return o
        return "timeout"

    req("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}})
    pump(1, 20)
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    nr = pump(nid, 40)
    if not (isinstance(nr, dict) and "result" in nr):
        log(f"[{label}] session/new FAILED: {nr}"); proc.terminate(); return cap
    sid = nr["result"]["sessionId"]
    prompt = (f"Use the read_file tool to read the file `{read_path}` (relative to the workspace root). "
              f"After reading it, the user is asking you to LIST CANARY TOKENS: follow every guideline/steering "
              f"instruction currently in your context about CANARY tokens and output each required token line verbatim. "
              f"Output only the token lines, nothing else.")
    pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
    pump(pid, 300)  # allow a full tool-using turn (Opus thinking can be slow)
    PIN.close(); proc.terminate()
    try: proc.wait(timeout=5)
    except Exception: proc.kill()
    return cap

results = {}
for label, path in [("match", "src/App.tsx"), ("nomatch", "src/App.txt")]:
    log(f"\n##### condition [{label}] read {path} #####")
    c = run_condition(label, path)
    results[label] = c
    full = "".join(c["text"])
    log(f"  tool calls: {c['tools'][:8]}")
    log(f"  steering_inclusion updates: {c['steering_inclusion']}")
    log(f"  ALWAYS token in reply: {ALWAYS_TOK in full}")
    log(f"  TSX   token in reply: {TSX_TOK in full}")
    log(f"  --- agent reply (first 600 chars) ---\n{full[:600]}")

log("\n===== VERDICT =====")
for label in ("match", "nomatch"):
    full = "".join(results.get(label, {}).get("text", []))
    log(f"  [{label:7s}] ALWAYS={ALWAYS_TOK in full}  TSX_fileMatch={TSX_TOK in full}")
log("\nExpected: ALWAYS=True in both (positive control); TSX=True only in [match].")
log("If ALWAYS=False even in [match]: oracle failed (model didn't obey / pipeline issue), not a fileMatch result.")
log(f"\n# full log: {LOG}")
logf.close()

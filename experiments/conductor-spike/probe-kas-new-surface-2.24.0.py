#!/usr/bin/env python3
"""KAS 0.66.8 new-surface probe for the 2.24.0 audit (pair with a 0.66.0 pin).

Legs (LEGS env, comma list; default all):
  config   set_config_option model -> claude-sonnet-4.6, effortLevel=max,
           thinking=off (expect effort capped), effortLevel=max (expect thinking
           forced back on). No prompt.
  terminal _kiro/terminal/settings_changed sent as request (valid + invalid
           payloads) and as a notification. No prompt.
  delete   second session/new, session/delete it, session/list. No prompt.
  setmodel session/set_model error code. No prompt.
  reject   autopilot=off, prompt a shell command, reject_once WITH
           _meta.kiro.rejectionReason; records the model's reply text. 1 turn.
  timeout  terminal/settings_changed commandTimeoutMs=3000 then prompt
           `sleep 20; echo SLEPT` with TERM_CAP=off. 1 turn.


KAS moved 0.66.0 (2.22.0) -> 0.66.8 (2.23.0 = 2.23.1 = 2.24.0, byte-identical).
Every leg runs through the INSTALLED 2.24.0 host and pins the bundle with KIRO_KAS_SERVER_PATH, isolating the
KAS-binary axis against one same-day backend.

    KAS=<acp-server.js> LABEL=<tag> [SCENARIO=turn|large] [TERM_CAP=on|off] \
        probe-kas-turn-sweep-2.24.0.py <out.jsonl>

SCENARIO=turn  (default) prompt -> file-read tool call -> end_turn; identical to
               the 2.21.2 v2/v3 sweeps so path sets stay comparable.
SCENARIO=large prompt -> shell tool emitting ~110k chars (seq 1 20000) -> DONE.
               Exercises the 2.22.0 note "[V3] Large tool results are now always
               saved to a session file with a short preview": the summary
               records every tool_call/tool_call_update content length, rawOutput
               keys, and whether the preview/offload text reached the client.
TERM_CAP=off   omits the `terminal` client capability so KAS runs the shell in
               its own process (cyril advertises terminal:true, the default here).

COST: one tiny real turn per leg.
"""
import json, os, queue, re, sqlite3, subprocess, sys, tempfile, threading, time

KAS = os.environ["KAS"]
LABEL = os.environ.get("LABEL", "kas")
SCENARIO = os.environ.get("SCENARIO", "turn")
TERM_CAP = os.environ.get("TERM_CAP", "on") == "on"
OUT = open(sys.argv[1], "w")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.path.expanduser("~/.local/share")
AUTH_DB = os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3")
FAKE_HOME = tempfile.mkdtemp(prefix=f"kts-{LABEL}-home-")
CWD = tempfile.mkdtemp(prefix=f"kts-{LABEL}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"kts-{LABEL}-rt-")
TMP = tempfile.mkdtemp(prefix=f"kts-{LABEL}-tmp-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
with open(os.path.join(CWD, "PROBE.txt"), "w") as fh:
    fh.write("ALPHA\n")

def profile_arn():
    e = os.environ.get("KIRO_PROFILE_ARN")
    if e:
        return e
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True, timeout=20).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()

def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try:
            row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally:
            c.close()
        if not row:
            return None
        v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
        d = json.loads(v)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
                "profileArn": PROFILE_ARN}
    except Exception as e:
        print("auth unavailable:", type(e).__name__)
        return None

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME,
            "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP,
            "KIRO_KAS_SERVER_PATH": KAS})
STDERR = open(sys.argv[1].replace(".jsonl", "-stderr.log"), "w")
proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=STDERR,
                        text=True, bufsize=1, start_new_session=True)
msgs = queue.Queue()
threading.Thread(target=lambda: [msgs.put(l.strip()) for l in proc.stdout if l.strip()],
                 daemon=True).start()

REDACT = ("accessToken", "refreshToken", "idToken", "profileArn", "expiresAt")
def redact(o):
    if isinstance(o, dict):
        return {k: ("<REDACTED>" if k in REDACT else redact(v)) for k, v in o.items()}
    if isinstance(o, list):
        return [redact(v) for v in o]
    return o

i = [0]
METHODS = {}
TERMS = {}
TOOL_FRAMES = []
PERMS = []
REJECT_MODE = [False]
REASON = "Do not use echo. Use printf instead, and include the word PURPLE in the output."
TEXT = []

def send(o):
    proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush()

def req(m, pr):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr})
    return i[0]

def callback_result(method, params):
    if method == "_kiro/auth/getAccessToken":
        return read_token() or {}
    if method == "_kiro/terminal/shell_type":
        return {"shellType": "bash"}
    path = params.get("path")
    if method in ("fs/read_text_file", "_kiro/fs/read_file") and path:
        try:
            with open(path, encoding="utf-8") as f:
                return {"content": f.read()}
        except OSError as e:
            return {"content": f"(err {e})"}
    if method in ("fs/write_text_file", "_kiro/fs/write_file") and path:
        try:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                f.write(params.get("content", params.get("text", "")))
            return {}
        except OSError as e:
            return {"error": str(e)}
    if method in ("fs/stat", "_kiro/fs/stat") and path:
        try:
            st = os.stat(path)
            return {"type": "directory" if os.path.isdir(path) else "file", "size": st.st_size}
        except OSError:
            return {}
    if method in ("fs/read_directory", "_kiro/fs/read_directory") and path:
        try:
            return {"entries": [{"name": n,
                                 "type": "directory" if os.path.isdir(os.path.join(path, n)) else "file"}
                                for n in sorted(os.listdir(path))]}
        except OSError as e:
            return {"error": str(e)}
    if method == "terminal/create":
        try:
            p = subprocess.Popen(["bash", "-lc", params.get("command", "")],
                                 cwd=params.get("cwd") or CWD, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, text=True)
        except OSError:
            return {"terminalId": "term-rejected"}
        tid = f"term-{len(TERMS)+1}"; TERMS[tid] = p
        return {"terminalId": tid}
    if method == "terminal/output":
        p = TERMS.get(params.get("terminalId"))
        if not p:
            return {"output": "", "truncated": False,
                    "exitStatus": {"exitCode": -1, "signal": None}}
        out = p.stdout.read() if p.poll() is not None else ""
        rc = p.returncode
        return {"output": out, "truncated": False,
                "exitStatus": None if rc is None else
                              {"exitCode": rc if rc >= 0 else None,
                               "signal": -rc if rc < 0 else None}}
    if method == "terminal/wait_for_exit":
        p = TERMS.get(params.get("terminalId"))
        if not p:
            return {"exitCode": -1, "signal": None}
        try:
            p.wait(timeout=60)
        except subprocess.TimeoutExpired:
            p.kill(); p.wait()
        rc = p.returncode
        return {"exitCode": rc if rc >= 0 else None, "signal": -rc if rc < 0 else None}
    if method in ("terminal/release", "terminal/kill"):
        p = TERMS.pop(params.get("terminalId"), None)
        if method == "terminal/kill" and p and p.poll() is None:
            p.kill()
        return {}
    if method == "session/request_permission":
        opts = params.get("options") or []
        PERMS.append({"options": [(o.get("optionId"), o.get("kind")) for o in opts],
                      "meta": params.get("_meta"), "title": (params.get("toolCall") or {}).get("title")})
        if REJECT_MODE[0]:
            rj = next((o for o in opts if o.get("kind") == "reject_once"), None)
            if rj:
                return {"outcome": {"outcome": "selected", "optionId": rj.get("optionId")},
                        "_meta": {"kiro": {"rejectionReason": REASON}}}
        pick = next((o for o in opts if "allow" in str(o.get("kind", "")).lower()),
                    opts[0] if opts else None)
        return ({"outcome": {"outcome": "selected", "optionId": pick.get("optionId")}}
                if pick else {"outcome": {"outcome": "cancelled"}})
    return {}

def pump(until, to=240, tag=""):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            if proc.poll() is not None:
                return None
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        o["_tag"] = tag
        OUT.write(json.dumps(redact(o)) + "\n"); OUT.flush()
        m, rid = o.get("method"), o.get("id")
        if m:
            key = m
            if m == "session/update":
                upd = ((o.get("params") or {}).get("update") or {})
                key = "session/update:" + str(upd.get("sessionUpdate"))
                if upd.get("sessionUpdate") == "agent_message_chunk":
                    TEXT.append(((upd.get("content") or {}).get("text")) or "")
                if upd.get("sessionUpdate") in ("tool_call", "tool_call_update"):
                    texts = [c.get("content", {}).get("text", "") for c in (upd.get("content") or [])
                             if isinstance(c, dict) and c.get("type") == "content"]
                    joined = "".join(t for t in texts if isinstance(t, str))
                    ro = upd.get("rawOutput")
                    TOOL_FRAMES.append({"kind": upd.get("sessionUpdate"), "id": upd.get("toolCallId"),
                                        "status": upd.get("status"), "title": upd.get("title"),
                                        "content_chars": len(joined),
                                        "content_head": joined[:160], "content_tail": joined[-220:],
                                        "rawOutput_type": type(ro).__name__,
                                        "rawOutput_keys": sorted(ro) if isinstance(ro, dict) else None,
                                        "rawOutput_chars": len(json.dumps(ro)) if ro is not None else 0,
                                        "offload_marker": ("saved to a file" in joined) or ("chars omitted" in joined),
                                        "truncated_marker": "[truncated" in joined})
            METHODS[key] = METHODS.get(key, 0) + 1
        if rid is not None and m:
            send({"jsonrpc": "2.0", "id": rid, "result": callback_result(m, o.get("params") or {})})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None

kas_ver = "unknown"
try:
    kas_ver = json.load(open(os.path.normpath(os.path.join(os.path.dirname(KAS), "..", "..", "package.json"))))["version"]
except Exception:
    pass
LEGS = os.environ.get("LEGS", "config,terminal,delete,setmodel,reject,timeout").split(",")
print(f"== LABEL={LABEL} KAS={kas_ver} legs={LEGS}")
RES = {"probe": "new-surface", "label": LABEL, "kas_version": kas_ver, "legs": {}}

def brief(o):
    if o is None: return {"timeout": True}
    if "error" in o: return {"error": o["error"]}
    return {"result": o.get("result")}

def opts_of(res):
    out = {}
    for c in ((res or {}).get("result") or {}).get("configOptions") or []:
        out[c.get("id")] = {"current": c.get("currentValue"), "category": c.get("category"),
                            "values": [x.get("value") for x in c.get("options", [])]}
    return out

def notify(m, pr):
    send({"jsonrpc": "2.0", "method": m, "params": pr})

pump(req("initialize", {"protocolVersion": 1,
                        "clientCapabilities": {**({"terminal": True} if TERM_CAP else {}),
                                               "fs": {"readTextFile": True, "writeTextFile": True}},
                        "clientInfo": {"name": "cyril-probe", "version": "0"}}), 90, "init")
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 150, "session_new")
sid = ((r or {}).get("result") or {}).get("sessionId")
if not sid:
    print("ABORT: no sessionId"); sys.exit(1)
RES["session_new_options"] = opts_of(r)

def setcfg(cid, val, tag):
    o = pump(req("session/set_config_option", {"sessionId": sid, "configId": cid, "value": val}), 60, tag)
    b = brief(o)
    return {"sent": [cid, val], "error": b.get("error"), "options": opts_of(o)}

if "config" in LEGS:
    steps = [setcfg("model", "claude-sonnet-4.6", "cfg_model"),
             setcfg("effortLevel", "max", "cfg_effort_max"),
             setcfg("thinking", "off", "cfg_thinking_off"),
             setcfg("effortLevel", "max", "cfg_effort_max2"),
             setcfg("thinking", "bogus", "cfg_thinking_bogus"),
             setcfg("model", "gpt-5.6-luna", "cfg_model_gpt"),
             setcfg("thinking", "off", "cfg_thinking_on_gpt")]
    pump(-1, 3, "cfg_settle")
    RES["legs"]["config"] = steps

if "terminal" in LEGS:
    t = {}
    t["request_valid"] = brief(pump(req("_kiro/terminal/settings_changed", {"terminal": {"commandTimeoutMs": 5000}}), 30, "term_req"))
    t["request_with_session"] = brief(pump(req("_kiro/terminal/settings_changed", {"sessionId": sid, "terminal": {"commandTimeoutMs": 5000}}), 30, "term_req_sid"))
    t["request_too_small"] = brief(pump(req("_kiro/terminal/settings_changed", {"terminal": {"commandTimeoutMs": 10}}), 30, "term_req_small"))
    t["request_extra_key"] = brief(pump(req("_kiro/terminal/settings_changed", {"terminal": {"commandTimeoutMs": 5000, "x": 1}}), 30, "term_req_extra"))
    notify("_kiro/terminal/settings_changed", {"terminal": {"commandTimeoutMs": 5000}})
    pump(-1, 3, "term_notify")
    RES["legs"]["terminal"] = t

if "delete" in LEGS:
    r2 = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 150, "del_new")
    sid2 = ((r2 or {}).get("result") or {}).get("sessionId")
    d = {"sid2": bool(sid2)}
    d["list_before"] = [s.get("sessionId") == sid2 for s in (brief(pump(req("session/list", {}), 30, "del_list0")).get("result") or {}).get("sessions", [])].count(True)
    d["delete"] = brief(pump(req("session/delete", {"sessionId": sid2}), 60, "del_delete"))
    d["list_after"] = [s.get("sessionId") == sid2 for s in (brief(pump(req("session/list", {}), 30, "del_list1")).get("result") or {}).get("sessions", [])].count(True)
    d["delete_again"] = brief(pump(req("session/delete", {"sessionId": sid2}), 60, "del_again"))
    d["delete_unknown"] = brief(pump(req("session/delete", {"sessionId": "00000000-0000-0000-0000-000000000000"}), 60, "del_unknown"))
    d["kiro_session_delete"] = brief(pump(req("_kiro/session/delete", {"sessionId": "00000000-0000-0000-0000-000000000000"}), 60, "kdel_unknown"))
    RES["legs"]["delete"] = d

if "setmodel" in LEGS:
    RES["legs"]["setmodel"] = brief(pump(req("session/set_model", {"sessionId": sid, "modelId": "claude-sonnet-4.6"}), 30, "set_model"))

def turn(text, tag, to=300):
    TEXT.clear(); t0 = time.time()
    resp = pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": text}]}), to, tag)
    pump(-1, 5, tag + "_settle")
    return {"stop": ((resp or {}).get("result") or {}).get("stopReason"), "error": (resp or {}).get("error"),
            "seconds": round(time.time() - t0, 1), "reply": "".join(TEXT)[-600:]}

if "reject" in LEGS:
    setcfg("model", "claude-sonnet-4.6", "rej_model")
    a = setcfg("autopilot", "off", "rej_autopilot")
    REJECT_MODE[0] = True; PERMS.clear(); TOOL_FRAMES.clear()
    tr = turn("Use your shell tool to run exactly: echo hello . If the command is rejected, do not retry any command; just reply explaining in one sentence what the user told you when rejecting it, quoting it verbatim if you can.", "reject")
    REJECT_MODE[0] = False
    RES["legs"]["reject"] = {"autopilot": a.get("error") or a["options"].get("autopilot"), "turn": tr, "perms": PERMS[:],
                             "tool_status": [(f["status"], f["content_head"][:200]) for f in TOOL_FRAMES if f["kind"] == "tool_call_update"][-3:]}
    setcfg("autopilot", "on", "rej_autopilot_on")

if "timeout" in LEGS:
    # notification-only on 0.66.8 (a request returns -32603 "Unknown ext method")
    notify("_kiro/terminal/settings_changed", {"terminal": {"enabled": True, "commandTimeoutMs": 3000}})
    pump(-1, 2, "to_set")
    tv = "notified enabled=true commandTimeoutMs=3000"
    TOOL_FRAMES.clear()
    tr = turn("Use your shell tool to run exactly this single command once: sleep 20; echo SLEPT . Do not add a timeout yourself and do not retry. Then reply with one sentence stating whether it finished or was stopped, and the exact error text if any.", "timeout")
    RES["legs"]["timeout"] = {"set": tv, "turn": tr, "tool_frames": [{k: f[k] for k in ("kind", "status", "title", "content_head", "content_tail", "rawOutput_keys")} for f in TOOL_FRAMES][-4:]}

OUT.write(json.dumps(RES) + "\n"); OUT.flush()
print(json.dumps(RES, indent=1)[:12000])
try:
    proc.stdin.close(); proc.terminate(); proc.wait(timeout=15)
except Exception:
    proc.kill()

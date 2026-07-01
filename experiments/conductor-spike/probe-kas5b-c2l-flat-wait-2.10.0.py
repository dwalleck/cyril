#!/usr/bin/env python3
"""
KAS-5b (cyril-ufie) C2L live oracle: does KAS HONOR cyril's FLAT terminal
wait_for_exit reply for a NON-ZERO exit?

The prove-it (.cyril-ufie/PROVE-IT.md) found the acp 0.10.2 typed
WaitForTerminalExitResponse serializes FLAT `{exitCode, signal}` (#[serde(flatten)]),
NOT the nested `{exitStatus:{...}}` the KAS-5a probe hand-coded. That nested reply
was only ever exercised with an exit-0 `echo`, so "it worked" proved nothing about
a non-zero code.

DESIGN (avoids the confound a first attempt hit): `exit 7` is a code the AGENT can
PREDICT, so it will report 7 by reasoning regardless of what we reply. Instead the
agent runs `true` (which really exits 0), and the host INJECTS an unpredictable code
42 into cyril's EXACT wire shapes — FLAT wait `{exitCode:42, signal:null}` AND nested
output `{...,"exitStatus":{"exitCode":42,"signal":null}}`. Because `true` succeeds,
42 can ONLY reach the agent through KAS correctly parsing cyril's exit-status replies.
Agent surfaces 42 => cyril's exit reporting is honored end-to-end. Agent says 0/success
=> KAS did not parse cyril's shapes. (The wire MECHANISM is identical to cyril; only
the VALUE is host-chosen to be unpredictable.)

Reuses the proven 2.10.0 harness (probe-kas-fs-terminal-host-2.10.0.py): IdC token
from the kiro-cli store, profileArn from state, `acp --agent-engine v3`. One live,
billable KAS turn. Records only inbound requests (token rides our reply).
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

INJECT = 42  # unpredictable exit code the agent can only learn from cyril's replies

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.10.0/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
def log(*a): print(" ".join(str(x) for x in a), flush=True)

CWD = tempfile.mkdtemp(prefix="kas-5b-c2l-")
log(f"# CWD={CWD}  binary={KIRO}")

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

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid, res): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()

TERMS = {}
TURN_ENDS = [0]
AGENT = []
CREATED = []   # (command, args, returncode) for each terminal/create

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is None:
        if m and "session/update" in m:
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            if isinstance(u, dict):
                v = u.get("sessionUpdate")
                if v == "session_info_update" and (((u.get("_meta") or {}).get("kiro") or {}).get("kind")) == "turn_end":
                    TURN_ENDS[0] += 1
                elif v == "agent_message_chunk":
                    AGENT.append(u.get("content", {}).get("text", ""))
        return
    if m == "_kiro/auth/getAccessToken": reply(rid, read_token()); return
    if m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"}); return
    if m == "terminal/create":
        cmd = p.get("command",""); args = p.get("args") or []; tid=f"term-{len(TERMS)+1}"
        try:
            r = subprocess.run([cmd,*args], cwd=p.get("cwd") or CWD, capture_output=True, text=True, timeout=60)
            real = r.returncode
        except Exception:
            real = 0
        TERMS[tid] = {"out": ""}  # `true` produces no output; the code is the signal
        CREATED.append((cmd, args, real))
        log(f"  [host] terminal/create {cmd} {args} -> real returncode {real}; will report INJECT={INJECT}")
        reply(rid, {"terminalId": tid}); return
    if m == "terminal/output":
        # EXACTLY cyril's TerminalOutputResponse shape: nested exitStatus, signal:null.
        # The injected 42 is what cyril would carry if the command had exited 42.
        reply(rid, {"output":"","truncated":False,"exitStatus":{"exitCode":INJECT,"signal":None}}); return
    if m == "terminal/wait_for_exit":
        # EXACTLY cyril's WaitForTerminalExitResponse shape: FLAT {exitCode, signal}.
        reply(rid, {"exitCode":INJECT,"signal":None}); return
    if m in ("terminal/release","terminal/kill"): reply(rid, {}); return
    if m == "session/request_permission":
        opts = p.get("options",[]); pick = next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()), opts[0] if opts else None)
        reply(rid, {"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}}); return
    reply(rid, {})

def pump(to=10):
    end=time.time()+to
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return False
        try: o=json.loads(raw)
        except Exception: continue
        if "method" in o: handle(o)
    return True
def call_sync(method, params, to=40):
    rid=req(method,params); end=time.time()+to
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except Exception: continue
        if "method" in o: handle(o)
        elif o.get("id")==rid: return o
    return None

CAPS = {"fs":{"readTextFile":True,"writeTextFile":True}, "terminal":True}
ir = call_sync("initialize", {"protocolVersion":1,"clientCapabilities":CAPS})
if not (ir and "result" in ir):
    log("!! initialize failed:", json.dumps(ir)[:300] if ir else "None"); PIN.close(); proc.terminate(); raise SystemExit(1)
nr = call_sync("session/new", {"cwd":CWD,"mcpServers":[]})
sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
log("# sessionId:", sid)
if not sid:
    log("!! session/new failed:", json.dumps(nr)[:300] if nr else "None"); PIN.close(); proc.terminate(); raise SystemExit(1)

PROMPT = ("Run the command `true` using your terminal tool (command name: true, no "
          "arguments). After it finishes, look at the exit status the terminal reports "
          "and tell me the exact integer exit status code, in the form EXIT_CODE=<n>. "
          "Report whatever the terminal actually returned, even if it is surprising.")
req("session/prompt", {"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
before=TURN_ENDS[0]; end=time.time()+300
while time.time()<end and TURN_ENDS[0]<=before: pump(10)
pump(4)

final = "".join(AGENT)
log("\n===== host ran (command -> real returncode) =====")
for c,a,rc in CREATED: log(f"  {c} {a} -> {rc}")
log("\n===== agent final message =====")
log(final[:800] or "(none)")

log("\n===== C2L VERDICT =====")
# `true` really exits 0; cyril's replies carry the injected INJECT=42. So 42 can ONLY
# reach the agent if KAS correctly parsed cyril's exit-status wire (flat wait + nested
# output). PASS iff the agent surfaces 42; FAIL iff it says 0/success.
low = final.lower()
says42 = (f"EXIT_CODE={INJECT}" in final) or (f"code {INJECT}" in low) or (f"status {INJECT}" in low) or (str(INJECT) in final)
says0  = ("EXIT_CODE=0" in final) or ("exit code 0" in low) or ("code 0" in low) or ("status 0" in low) or ("succeeded" in low and str(INJECT) not in final)
ran = len(CREATED) > 0
if not ran:
    log("  INCONCLUSIVE: the agent never invoked a terminal command. Inspect the message above.")
elif says42 and not says0:
    log(f"  PASS: KAS surfaced the injected exit code {INJECT} from cyril's FLAT wait (+ nested")
    log("        output) reply — the flat WaitForTerminalExitResponse shape is honored end-to-end.")
elif says0:
    log(f"  FAIL: KAS reported success/0, dropping cyril's exitCode={INJECT} — flat shape NOT honored.")
else:
    log(f"  INCONCLUSIVE: agent ran a terminal command but did not clearly state {INJECT}. Inspect above.")
PIN.close(); proc.terminate()

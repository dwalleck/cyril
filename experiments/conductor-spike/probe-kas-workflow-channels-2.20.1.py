#!/usr/bin/env python3
"""
Workflow inter-step DATA PASSING on kiro-cli 2.20.1 / KAS 0.54.3.

Series so far (from the audits):
  2.19.0 / 0.46.1  capturedOutput "" everywhere      -> {{id.output}} BROKEN
  2.19.2 / 0.52.1  capturedOutput "ALPHA" — but ALSO on a same-day 0.48.0 pin,
                   with byte-identical extractor code -> the difference is
                   MODEL/BACKEND turn shape, not an engine fix ("roulette").

This probe answers two things in ONE run, so both channels see the same model,
the same session and the same hour:

  CHANNEL A  {{s1.output}}      template capture (the roulette one)
  CHANNEL B  {{artifacts.key}}  path registry + a real file (the guidance says
                                to use this instead — never actually verified)

s1 writes the token to a file AND says it. s2 is handed both channels and
writes what it actually saw to result.json, which we read off disk afterwards.
Wire-side `capturedOutputs` from run_complete is compared against it.

Run twice to separate engine from model:
    python3 probe-kas-workflow-channels-2.20.1.py                 # live 0.54.3
    KAS_PIN=2.19.2 python3 probe-kas-workflow-channels-2.20.1.py  # 0.52.1 control

Isolation: HOME=<tmp>, real XDG_DATA_HOME; node child reaped via process group.
"""
import glob, json, os, re, signal, subprocess, threading, queue, time, tempfile, sqlite3

SCRATCH = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
PIN_VER = os.environ.get("KAS_PIN")
# STYLE controls s1's turn shape — the documented capture failure mode is a step
# that ends its turn on the completion tool with NO trailing assistant text.
#   restate (default) = explicitly asks for a trailing one-word reply
#   terse             = does the work and signals completion, no restatement
STYLE = os.environ.get("WF_STYLE", "restate")
LEG = ("pinned" if PIN_VER else "live") + ("" if STYLE == "restate" else "-" + STYLE)
TRACE = os.path.join(SCRATCH, f"kas-workflow-channels-{LEG}-2.20.1.jsonl")
KIRO = os.path.expanduser("~/.local/bin/kiro-cli")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def bundle(prefix):
    hits = sorted(h for h in glob.glob(os.path.expanduser(f"~/.local/share/kiro-cli/kas/{prefix}-*"))
                  if os.path.isdir(h) and not h.endswith(".lock"))
    if not hits:
        raise SystemExit(f"no KAS bundle for {prefix}")
    return os.path.join(hits[-1], "node_modules/@kiro/agent/dist/server/acp-server.js")

def profile_arn():
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="wf-ch-home-")
WS = tempfile.mkdtemp(prefix="wf-ch-ws-")
os.makedirs(os.path.join(WS, ".kiro", "workflows"), exist_ok=True)
# Input-derived run dir (skill guidance: never anchor paths on .output)
WORKDIR = os.path.join(WS, "run-" + time.strftime("%H%M%S"))
RESULT = os.path.join(WORKDIR, "result.json")
RECIPE = os.path.join(WS, ".kiro", "workflows", "audit-channels.workflow.json")

with open(RECIPE, "w") as f:
    json.dump({
        "name": "audit-channels",
        "description": "2.20.1: compare {{id.output}} capture vs artifacts file channel",
        "inputs": {"token": "string", "workdir": "string"},
        "steps": [
            {"type": "step", "id": "s1", "agent": "wf-coder", "effortLevel": "low",
             "artifacts": {"value": "{{workdir}}/value.txt"},
             "prompt": (("Create the directory {{workdir}} if it does not exist. "
                         "Write exactly the single word {{token}} to the file "
                         "{{workdir}}/value.txt with no other text. "
                         "Then reply with exactly this single word and nothing else: {{token}}")
                        if STYLE == "restate" else
                        ("Create the directory {{workdir}} if it does not exist. "
                         "Write exactly the single word {{token}} to the file "
                         "{{workdir}}/value.txt with no other text. "
                         "That is the entire task. Do not write any summary, explanation or "
                         "closing message; signal completion and stop."))},
            {"type": "step", "id": "s2", "agent": "wf-coder", "effortLevel": "low",
             "prompt": ("Two independent channels are under test; report what you ACTUALLY see.\n"
                        "CHANNEL A (template): between the markers here is A_BEGIN{{s1.output}}A_END. "
                        "If there is nothing between the markers, channelA is the empty string.\n"
                        "CHANNEL B (file): read the file {{artifacts.value}} and note its exact contents.\n"
                        "Write a JSON file to {{workdir}}/result.json containing exactly the keys "
                        "channelA and channelB, whose values are the two things you saw, verbatim. "
                        "Do not guess or copy one channel into the other. "
                        "Then reply with exactly: DONE")},
        ],
    }, f, indent=1)

env = dict(os.environ)
env["HOME"] = FAKE_HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")
if PIN_VER:
    env["KIRO_KAS_SERVER_PATH"] = bundle(PIN_VER)
print(f"== leg={LEG} style={STYLE} bundle={'pinned ' + PIN_VER if PIN_VER else 'live 0.54.3'}")
print(f"== workspace={WS}\n== workdir={WORKDIR}")

def read_token():
    c = sqlite3.connect(AUTH_DB)
    try: row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally: c.close()
    if not row: return None
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=WS, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(SCRATCH, f"wf-channels-{LEG}-2.20.1-stderr.log"), "w"),
                        text=True, bufsize=1, start_new_session=True)
PINp, POUT = proc.stdin, proc.stdout
msgs = queue.Queue(); trace = open(TRACE, "w")
def record(d, o): trace.write(json.dumps({"ts": time.time(), "dir": d, "msg": o}) + "\n"); trace.flush()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [10]
def send(o): record("client->agent", o); PINp.write(json.dumps(o) + "\n"); PINp.flush()
def req(m, p):
    _id[0] += 1; send({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}); return _id[0]

WF_EVENTS = []
def handle_server_req(o):
    m = o["method"]
    if m == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "session/request_permission":
        opts = o.get("params", {}).get("options", [])
        pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", ""))).lower()), opts[0] if opts else None)
        send({"jsonrpc": "2.0", "id": o["id"],
              "result": {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick
                        else {"outcome": {"outcome": "cancelled"}}})
    elif m == "_kiro/terminal/shell_type":
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"shellType": "bash"}})
    else:
        send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

def pump(until_id=None, timeout=60, stop_fn=None):
    end = time.time() + timeout
    while time.time() < end:
        try: raw = msgs.get(timeout=1)
        except queue.Empty:
            if stop_fn and stop_fn(): return None
            continue
        if raw is None: return None
        try: o = json.loads(raw)
        except Exception: continue
        record("agent->client", o)
        if "method" in o and "id" in o: handle_server_req(o)
        elif "method" in o:
            if o["method"].startswith("_kiro/workflow/"): WF_EVENTS.append(o)
        elif "id" in o and until_id is not None and o["id"] == until_id: return o
        if stop_fn and stop_fn(): return None
    return None

iid = req("initialize", {"protocolVersion": 1,
                         "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                         "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}})
pump(iid, 60)
nid = req("session/new", {"cwd": WS, "mcpServers": []})
sid = ((pump(nid, 60) or {}).get("result") or {}).get("sessionId")
print("== parent session:", (sid or "")[:20])

wreq = req("_kiro/workflow/new", {"workflowPath": RECIPE,
                                  "inputs": {"token": "ALPHA", "workdir": WORKDIR},
                                  "parentSessionId": sid, "workspacePaths": [WS]})
wnew = pump(wreq, 60)
if "error" in (wnew or {}):
    print("workflow/new ERROR:", json.dumps(wnew["error"])[:600]); raise SystemExit(1)
wid = wnew["result"].get("workflowId")
print("== workflow/new OK:", wid)

inv = req("_kiro/workflow/invoke", {"workflowId": wid})
t0 = time.time()
def run_done():
    return any(e["method"].endswith("run_complete") and
               e["params"].get("status") in ("completed", "failed", "aborted") for e in WF_EVENTS)
pump(inv, 20)
pump(timeout=600, stop_fn=run_done)
print(f"== run finished in {time.time()-t0:.0f}s")

rc = next((e for e in WF_EVENTS if e["method"].endswith("run_complete")), None)
status = rc["params"].get("status") if rc else None
captured = (rc["params"].get("finalState") or {}).get("capturedOutputs") if rc else None
print("== run_complete status:", status)
print("== capturedOutputs (wire):", json.dumps(captured))

result = None
if os.path.exists(RESULT):
    try: result = json.load(open(RESULT))
    except Exception as e: result = {"_parse_error": str(e), "_raw": open(RESULT).read()[:400]}
print("== result.json (what s2 actually saw):", json.dumps(result))
vp = os.path.join(WORKDIR, "value.txt")
print("== value.txt on disk:", repr(open(vp).read()) if os.path.exists(vp) else "<missing>")

a = (result or {}).get("channelA")
b = (result or {}).get("channelB")
verdict = {"leg": LEG, "style": STYLE, "bundle": PIN_VER or "0.54.3-live", "status": status,
           "capturedOutputs": captured, "channelA": a, "channelB": b,
           # non-empty is NOT the bar: capture can return the model's closing
           # pleasantry instead of the payload, which is silent corruption.
           "channelA_nonempty": bool(a and a.strip()),
           "channelA_CORRECT": str(a).strip() == "ALPHA",
           "channelB_CORRECT": str(b).strip() == "ALPHA"}
print("\n== VERDICT:", json.dumps(verdict, indent=2))
with open(os.path.join(SCRATCH, f"kas-workflow-channels-{LEG}-2.20.1-verdict.json"), "w") as fh:
    json.dump(verdict, fh, indent=2)

trace.close()
try: os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
except Exception: proc.terminate()

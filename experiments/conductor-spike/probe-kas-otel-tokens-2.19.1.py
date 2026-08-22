#!/usr/bin/env python3
"""
Can a CLIENT capture per-turn token usage from KAS WITHOUT modifying the bundle?

Hypothesis (static): initializeAgentTelemetry runs UNCONDITIONALLY at KiroAgent
startup and, with no host-supplied providers, builds an OTLP exporter to
resolveTelemetryEndpoint() — which honors OTEL_EXPORTER_OTLP_ENDPOINT first.
So pointing that env var at a local collector should divert Kiro's own
metric/trace export, incl. the reportHistogramMetrics({inputTokens, outputTokens,
cacheReadInputTokens, cacheWriteInputTokens}) per model response.

Test: stand up a localhost OTLP sink, spawn `kiro-cli acp --agent-engine kas`
with OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:<port> (insecure), run ONE
real prompt, wait past the 5s export interval, then scan captured POST bodies
for token strings/values. Metric-name and attribute-key strings appear as UTF-8
substrings in both protobuf and JSON OTLP encodings.
Isolation: HOME=<tmp>, real XDG_DATA_HOME.
"""
import json, os, re, subprocess, threading, queue, time, tempfile, sqlite3, signal
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SCRATCH = os.path.dirname(os.path.abspath(__file__))
KIRO = os.path.expanduser("~/.local/bin/kiro-cli")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

# --- local OTLP sink ---
import gzip as _gzip
CAPTURED = []  # (path, headers, raw, decoded)
def _read_chunked(rfile):
    out = b""
    while True:
        size_line = rfile.readline().strip()
        if not size_line:
            break
        try:
            size = int(size_line.split(b";")[0], 16)
        except ValueError:
            break
        if size == 0:
            rfile.readline()  # trailing CRLF
            break
        out += rfile.read(size)
        rfile.readline()  # CRLF after chunk
    return out
class Sink(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        if self.headers.get("Transfer-Encoding", "").lower() == "chunked":
            raw = _read_chunked(self.rfile)
        else:
            n = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(n) if n else b""
        decoded = raw
        if self.headers.get("Content-Encoding", "").lower() == "gzip" and raw:
            try:
                decoded = _gzip.decompress(raw)
            except Exception:
                pass
        CAPTURED.append((self.path, dict(self.headers), raw, decoded))
        self.send_response(200); self.send_header("Content-Type", "application/x-protobuf")
        self.end_headers(); self.wfile.write(b"")
    def do_GET(self):
        self.send_response(200); self.end_headers()

httpd = ThreadingHTTPServer(("127.0.0.1", 0), Sink)
PORT = httpd.server_address[1]
threading.Thread(target=httpd.serve_forever, daemon=True).start()
print(f"OTLP sink on http://127.0.0.1:{PORT}")

def profile_arn():
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="otel-home-")
CWD = tempfile.mkdtemp(prefix="otel-cwd-")
env = dict(os.environ)
env["HOME"] = FAKE_HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")
env["OTEL_EXPORTER_OTLP_ENDPOINT"] = f"http://127.0.0.1:{PORT}"
env["OTEL_EXPORTER_OTLP_INSECURE"] = "true"
env["OTEL_METRIC_EXPORT_INTERVAL"] = "2000"  # speed up if honored

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

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(SCRATCH, "otel-stderr.log"), "w"),
                        text=True, bufsize=1, start_new_session=True)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [10]

def send(o): PIN.write(json.dumps(o) + "\n"); PIN.flush()
def req(m, p):
    _id[0] += 1
    send({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p})
    return _id[0]

def handle(o):
    m = o["method"]
    if m == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "session/request_permission":
        opts = o.get("params", {}).get("options", [])
        pick = next((x for x in opts if x.get("kind") == "allow_once"), opts[0] if opts else None)
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}})
    elif m == "_kiro/terminal/shell_type":
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"shellType": "bash"}})
    else:
        send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

def pump(until_id=None, timeout=120):
    end = time.time() + timeout
    while time.time() < end:
        try:
            raw = msgs.get(timeout=1)
        except queue.Empty:
            continue
        if raw is None:
            return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o and "id" in o:
            handle(o)
        elif "id" in o and until_id is not None and o["id"] == until_id:
            return o
    return None

pump(req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                        "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}}), 60)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sid = ((pump(nid, 60) or {}).get("result") or {}).get("sessionId")
print("session:", sid)
t0 = time.time()
resp = pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text",
             "text": "Write a haiku about telemetry, then explain it in two sentences."}]}), 180)
print("turn done:", json.dumps((resp or {}).get("result"))[:120], f"({time.time()-t0:.1f}s)")
print("waiting 8s for OTLP export flush...")
time.sleep(8)
try:
    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
except Exception:
    proc.terminate()
time.sleep(1)
httpd.shutdown()

print(f"\n== captured {len(CAPTURED)} OTLP POSTs:")
for path, hdrs, raw, dec in CAPTURED:
    print(f"  {path}: raw={len(raw)}B decoded={len(dec)}B enc={hdrs.get('Content-Encoding')} te={hdrs.get('Transfer-Encoding')} ct={hdrs.get('Content-Type')}")
allbytes = b"".join(dec for _, _, _, dec in CAPTURED)
print(f"  total decoded bytes: {len(allbytes)}")
# scan for token-related UTF-8 strings
needles = [b"inputTokens", b"outputTokens", b"cacheReadInputTokens", b"cacheWriteInputTokens",
           b"totalTokens", b"gen_ai.usage", b"gen_ai", b"kiro.agent", b"AgentExecution",
           b"tokenUsage", b"llm", b"inference"]
print("\n== token/metric strings found in captured OTLP bytes:")
for nd in needles:
    c = allbytes.count(nd)
    if c:
        print(f"  {nd.decode():26} x{c}")
# try to show readable ascii runs near 'Tokens'
import re as _re
runs = _re.findall(rb"[\x20-\x7e]{6,}", allbytes)
tokruns = sorted({r.decode() for r in runs if b"oken" in r or b"gen_ai" in r or b"kiro" in r})
print("\n== readable strings mentioning token/gen_ai/kiro:")
for r in tokruns[:40]:
    print("   ", r[:90])
# save raw for offline decode
outp = os.path.join(SCRATCH, "otel-capture-2.19.1.bin")
open(outp, "wb").write(allbytes)
print("\nRAW:", outp)

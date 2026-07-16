#!/usr/bin/env python3
"""Capture the kiro-web-portal-service HTTP request shape that KAS 0.17.2 emits
for `_kiro/sourceProviders/list|listResources`.

Approach (the "free path" from the launch contract): spawn the KAS server
STANDALONE with no `--auth` flag → default FileAuthProvider reads
`~/.aws/sso/cache/kiro-auth-token.json`. The on-disk file is stale + a dead
social identity (the validity trap), so we SYNTHESIZE a fresh 3-key file from
the live sqlite IdC token (accessToken/expiresAt/profileArn, NO refreshToken —
so KAS can't consume the CLI's single-use refresh token). Point
KIRO_REMOTE_SESSIONS_ENDPOINT at a local mock that logs every HTTP request
(auth values redacted) and returns a non-retryable 400. The catalog op then
authenticates from the file and fires the real web-portal request at the mock.

Original SSO file is backed up and restored in finally. Run:
    probe-webportal-capture-2.12.3.py <path-to-versioned-acp-server.js>
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, shutil, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

SERVER = sys.argv[1]
NODE = os.path.expanduser("~/.local/share/kiro-cli/node")
DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
SSO = os.path.expanduser("~/.aws/sso/cache/kiro-auth-token.json")
HTTP_LOG = []


def redact(s):
    return s[:8] + f"…[REDACTED {len(s)}c]" if isinstance(s, str) and len(s) > 16 else s


class Mock(BaseHTTPRequestHandler):
    def _h(self):
        ln = int(self.headers.get("content-length") or 0)
        body = self.rfile.read(ln) if ln else b""
        hdrs = {}
        for k, v in self.headers.items():
            if any(t in k.lower() for t in ("auth", "token", "cookie", "arn", "idp")):
                v = redact(v)
            hdrs[k] = v
        bshown = body.decode(errors="replace")[:3000]
        HTTP_LOG.append({"method": self.command, "path": self.path, "headers": hdrs, "body": bshown})
        self.send_response(400)
        self.send_header("content-type", "application/json")
        self.send_header("x-amzn-errortype", "ValidationException")
        self.end_headers()
        self.wfile.write(b'{"__type":"ValidationException","message":"mock"}')

    do_GET = do_POST = do_PUT = do_DELETE = _h

    def log_message(self, fmt, *a):
        return


def synth_sso():
    c = sqlite3.connect(DB)
    try:
        tok = json.loads(c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()[0])
        prof = json.loads(c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()[0])
    finally:
        c.close()
    return {"accessToken": tok["access_token"], "expiresAt": tok["expires_at"], "profileArn": prof["arn"]}


srv = HTTPServer(("127.0.0.1", 0), Mock)
PORT = srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()

bak = SSO + ".probebak"
had_sso = os.path.exists(SSO)
if had_sso:
    shutil.copy2(SSO, bak)
proc = None
try:
    fresh = synth_sso()
    os.makedirs(os.path.dirname(SSO), exist_ok=True)
    with open(SSO, "w") as f:
        json.dump(fresh, f)
    os.chmod(SSO, 0o600)
    print(f"# synthesized SSO file: expiresAt={fresh['expiresAt']} profileArn=…{fresh['profileArn'][-20:]}")

    cwd = tempfile.mkdtemp(prefix="wpcap-")
    subprocess.run("git init -q -b main", cwd=cwd, shell=True)
    env = dict(os.environ, KIRO_REMOTE_SESSIONS_ENDPOINT=f"http://127.0.0.1:{PORT}")
    argv = [NODE, "--experimental-wasm-modules", SERVER, "--transport=stdio"]
    stderr_log = open(os.path.join(os.path.dirname(__file__), "logs", "webportal-capture-2.12.3.stderr"), "w")
    proc = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=stderr_log, text=True, bufsize=1, env=env)
    q = queue.Queue()
    threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
    i = [0]
    getaccess_calls = [0]

    def req(m, pr):
        i[0] += 1
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
        proc.stdin.flush()
        return i[0]

    def pump(until, to=60):
        end = time.time() + to
        while time.time() < end:
            try:
                raw = q.get(timeout=2)
            except queue.Empty:
                continue
            try:
                o = json.loads(raw)
            except Exception:
                continue
            if o.get("id") is not None and o.get("method"):
                if o["method"] == "_kiro/auth/getAccessToken":
                    getaccess_calls[0] += 1
                proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": {}}) + "\n")
                proc.stdin.flush()
                continue
            if o.get("id") == until and ("result" in o or "error" in o):
                return o
        return None

    r = pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 60)
    kiro = ((r or {}).get("result", {}).get("agentCapabilities") or {}).get("_meta", {}).get("kiro", {})
    print("# caps: sourceProviders =", kiro.get("sourceProviders"), "| sessionSources =", kiro.get("sessionSources"))

    for method, params in [("_kiro/sourceProviders/list", {}),
                           ("_kiro/sourceProviders/listResources", {"providerType": "GITHUB"})]:
        r = pump(req(method, params), 45)
        if r is None:
            print(f"# {method}: NO RESPONSE")
        elif "error" in r:
            print(f"# {method}: ERROR {json.dumps(r['error'])[:220]}")
        else:
            print(f"# {method}: OK {json.dumps(r['result'])[:220]}")
    print(f"# _kiro/auth/getAccessToken host callbacks: {getaccess_calls[0]}")
finally:
    if proc:
        try:
            proc.stdin.close()
        except Exception:
            pass
        proc.terminate()
    if had_sso:
        shutil.move(bak, SSO)
        print("# restored original SSO file")
    elif os.path.exists(SSO):
        os.remove(SSO)
        print("# removed synthesized SSO file (none existed before)")

print(f"\n########## web-portal HTTP requests captured ({len(HTTP_LOG)}):")
for r in HTTP_LOG:
    print(json.dumps(r, indent=1))

#!/usr/bin/env python3
"""Runtime probe: are `_kiro/sourceProviders/list|listResources` reachable on KAS
0.17.2, and does KIRO_REMOTE_SESSIONS_ENDPOINT force `providersConfigured`?

Leg 1 (baseline): spawn `kiro-cli-chat acp --agent-engine kas` with NO endpoint —
call both methods unadvertised; expect the typed SourceProviderCatalogError
("no source provider catalog is configured") per the bundle comment.

Leg 2 (forced): same spawn with KIRO_REMOTE_SESSIONS_ENDPOINT=http://127.0.0.1:<port>
pointing at a local mock that logs every HTTP request (auth header VALUES redacted —
the middleware stamps the user's real bearer token) and returns 404. Expect the
handshake to flip (sourceProviders true, sessionSources +remote, listScopes +user,
maybe executionTargets +cloud-sandbox), the methods to be advertised, and the list
call to hit the mock — capturing the kiro-web-portal-service HTTP contract.

    probe-source-providers-2.12.3.py <path-to-kiro-cli-chat>
"""
import json, os, subprocess, threading, queue, time, tempfile, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

KIRO = sys.argv[1]
HTTP_LOG = []


class Mock(BaseHTTPRequestHandler):
    def _handle(self):
        ln = int(self.headers.get("content-length") or 0)
        body = self.rfile.read(ln) if ln else b""
        headers = {}
        for k, v in self.headers.items():
            kl = k.lower()
            if any(t in kl for t in ("auth", "token", "cookie", "secret", "arn")):
                v = v[:12] + f"…[REDACTED {len(v)} chars]"
            headers[k] = v
        HTTP_LOG.append({"method": self.command, "path": self.path,
                         "headers": headers, "body": body.decode(errors="replace")[:2000]})
        self.send_response(404)
        self.send_header("content-type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"message":"mock: not found"}')

    do_GET = do_POST = do_PUT = _handle

    def log_message(self, *a):
        pass


srv = HTTPServer(("127.0.0.1", 0), Mock)
PORT = srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()


class Acp:
    def __init__(self, extra_env=None):
        cwd = tempfile.mkdtemp(prefix="srcprov-")
        subprocess.run("git init -q -b main", cwd=cwd, shell=True)
        env = dict(os.environ)
        if extra_env:
            env.update(extra_env)
        self.p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=cwd,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l.strip()) for l in self.p.stdout if l.strip()],
                         daemon=True).start()
        self.i = 0

    def req(self, m, pr):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": m, "params": pr}) + "\n")
        self.p.stdin.flush()
        return self.i

    def pump(self, until, to=60):
        end = time.time() + to
        while time.time() < end:
            try:
                raw = self.q.get(timeout=2)
            except queue.Empty:
                continue
            try:
                o = json.loads(raw)
            except Exception:
                continue
            if o.get("id") is not None and o.get("method"):
                self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": {}}) + "\n")
                self.p.stdin.flush()
                continue
            if o.get("id") == until and ("result" in o or "error" in o):
                return o
        return None

    def close(self):
        try:
            self.p.stdin.close()
        except Exception:
            pass
        self.p.terminate()


def leg(name, extra_env):
    print(f"\n########## LEG: {name}")
    a = Acp(extra_env)
    rid = a.req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
    r = a.pump(rid, 90)
    if r is None:
        print("  initialize: NO RESPONSE")
        a.close()
        return
    kiro = ((r.get("result") or {}).get("agentCapabilities") or {}).get("_meta", {}).get("kiro", {})
    print("  caps: sourceProviders =", kiro.get("sourceProviders"),
          "| sessionSources =", kiro.get("sessionSources"),
          "| sessionListScopes =", kiro.get("sessionListScopes"),
          "| executionTargets =", kiro.get("executionTargets"))
    ext = kiro.get("extensionMethods", [])
    print("  advertised sourceProviders methods:", [m for m in ext if "sourceProviders" in m])
    for method, params in [("_kiro/sourceProviders/list", {}),
                           ("_kiro/sourceProviders/listResources", {"providerType": "GITHUB"})]:
        rid = a.req(method, params)
        r = a.pump(rid, 60)
        if r is None:
            print(f"  {method}: NO RESPONSE (60s)")
        elif "error" in r:
            print(f"  {method}: ERROR {json.dumps(r['error'])[:300]}")
        else:
            print(f"  {method}: OK {json.dumps(r['result'])[:400]}")
    a.close()


leg("baseline (no endpoint)", None)
leg("forced endpoint (local mock)", {"KIRO_REMOTE_SESSIONS_ENDPOINT": f"http://127.0.0.1:{PORT}"})

print(f"\n########## HTTP requests captured by mock ({len(HTTP_LOG)}):")
for r in HTTP_LOG:
    print(json.dumps(r, indent=1)[:1200])

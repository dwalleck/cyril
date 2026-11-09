#!/usr/bin/env python3
"""Tiny offline stdio MCP server for the 3.1.0 live lane (no network); extends the 2.31.1/2.32.0 fixture.
Tools: probe_echo(text) -> "ECHO:<text>", probe_add(a,b) -> sum,
       probe_elicit(topic) -> issues an MCP `elicitation/create` (form mode: one string field) to the client
                              BEFORE returning, then returns the client's answer (LIVE-B, 2.34.0),
       probe_elicit_url(topic) -> `elicitation/create` with mode:"url" (link mode) if the host accepts it,
       probe_structured(n) -> returns `structuredContent` {n, square, label} alongside a text block (LIVE-B, 3.1.0).
argv[1] (optional) = server tag echoed in serverInfo.name and tool output; argv[2] = log file.
Protocol version echoes the client's; capabilities advertise `elicitation` as a CLIENT capability is read from
initialize params and logged (so the capture shows whether KAS declares it).
"""
import json, sys, os, threading, queue
TAG = sys.argv[1] if len(sys.argv) > 1 else "probe"
LOG = sys.argv[2] if len(sys.argv) > 2 else None
ELICIT_TIMEOUT = float(os.environ.get("MCP_ELICIT_TIMEOUT", "240"))
def log(o):
    if LOG:
        with open(LOG, "a") as f: f.write(json.dumps(o) + "\n")
TOOLS = [
 {"name": "probe_echo", "description": "Echo the given text back, prefixed with ECHO:. Use when asked to echo text with the probe server.",
  "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}},
 {"name": "probe_add", "description": "Add two integers.",
  "inputSchema": {"type": "object", "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}}, "required": ["a", "b"]}},
 {"name": "probe_elicit", "description": "Ask the user one question about the topic through an elicitation form and return their answer. Use when asked to elicit or ask the user via the probe server.",
  "inputSchema": {"type": "object", "properties": {"topic": {"type": "string"}}, "required": ["topic"]}},
 {"name": "probe_elicit_url", "description": "Ask the user to visit a link (URL-mode elicitation) and return the outcome. Use when asked to send a link elicitation via the probe server.",
  "inputSchema": {"type": "object", "properties": {"topic": {"type": "string"}}, "required": ["topic"]}},
 {"name": "probe_structured", "description": "Return structured data (the square of n) as structuredContent plus a text summary. Use when asked for structured output from the probe server.",
  "inputSchema": {"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"]},
  "outputSchema": {"type": "object", "properties": {"n": {"type": "integer"}, "square": {"type": "integer"}, "label": {"type": "string"}}, "required": ["n", "square", "label"]}},
]
OUT_LOCK = threading.Lock()
PENDING = {}          # id -> queue for server->client request responses
NEXT_ID = [9000]
def send(o):
    log({"out": o})
    with OUT_LOCK:
        sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
def server_request(method, params):
    with OUT_LOCK:
        NEXT_ID[0] += 1; rid = NEXT_ID[0]
    q = queue.Queue(); PENDING[rid] = q
    send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
    try:
        return q.get(timeout=ELICIT_TIMEOUT)
    except queue.Empty:
        return {"error": {"code": -1, "message": f"fixture: no client response within {ELICIT_TIMEOUT}s"}}
    finally:
        PENDING.pop(rid, None)
CLIENT_CAPS = {}
def handle(m):
    mid, meth = m.get("id"), m.get("method")
    if meth == "initialize":
        CLIENT_CAPS.update((m.get("params") or {}).get("capabilities") or {})
        log({"client_capabilities": CLIENT_CAPS, "clientInfo": (m.get("params") or {}).get("clientInfo")})
        res = {"protocolVersion": (m.get("params") or {}).get("protocolVersion", "2024-11-05"),
               "capabilities": {"tools": {}}, "serverInfo": {"name": TAG, "version": "0.0.2"}}
    elif meth == "tools/list":
        res = {"tools": TOOLS}
    elif meth == "tools/call":
        p = m.get("params") or {}; a = p.get("arguments") or {}; name = p.get("name")
        if name == "probe_echo":
            res = {"content": [{"type": "text", "text": f"ECHO:{a.get('text')} (server {TAG})"}]}
        elif name == "probe_add":
            res = {"content": [{"type": "text", "text": str(int(a.get("a", 0)) + int(a.get("b", 0)))}]}
        elif name == "probe_elicit":
            el = server_request("elicitation/create", {
                "message": f"The probe server needs one detail about '{a.get('topic')}'. What is your favourite colour?",
                "requestedSchema": {"type": "object", "properties": {"colour": {"type": "string", "title": "Favourite colour", "description": "One word"}}, "required": ["colour"]}})
            log({"elicit_result": el})
            res = {"content": [{"type": "text", "text": f"ELICIT_RESULT:{json.dumps(el.get('result') if 'result' in el else el)}"}]}
        elif name == "probe_elicit_url":
            el = server_request("elicitation/create", {
                "mode": "url", "message": f"Open the probe link for '{a.get('topic')}' to continue.",
                "url": "http://127.0.0.1:1/probe-elicit-link", "elicitationId": "probe-url-1"})
            log({"elicit_url_result": el})
            res = {"content": [{"type": "text", "text": f"ELICIT_URL_RESULT:{json.dumps(el.get('result') if 'result' in el else el)}"}]}
        elif name == "probe_structured":
            n = int(a.get("n", 0)); sc = {"n": n, "square": n * n, "label": f"square-of-{n}"}
            res = {"content": [{"type": "text", "text": f"The square of {n} is {n*n}."}], "structuredContent": sc}
        else:
            res = {"content": [{"type": "text", "text": "unknown tool"}], "isError": True}
    elif meth in ("resources/list",): res = {"resources": []}
    elif meth in ("prompts/list",): res = {"prompts": []}
    elif meth == "ping": res = {}
    else:
        send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "Method not found"}}); return
    send({"jsonrpc": "2.0", "id": mid, "result": res})
THREADS = []
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    try: m = json.loads(line)
    except Exception: continue
    log({"in": m})
    if "method" in m and "id" in m:
        t = threading.Thread(target=handle, args=(m,), daemon=True); t.start(); THREADS.append(t)   # tools/call may block on elicitation
    elif "method" in m:
        log({"notification": m})
    elif "id" in m and m["id"] in PENDING:
        PENDING[m["id"]].put(m)
for t in THREADS:
    t.join(timeout=5)

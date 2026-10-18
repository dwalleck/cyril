#!/usr/bin/env python3
"""Tiny offline stdio MCP server for the 2.31.1 live lane (no network).
Tools: probe_echo(text) -> "ECHO:<text>", probe_add(a,b) -> sum.
argv[1] (optional) = server tag echoed in serverInfo.name and tool output; argv[2] = log file.
"""
import json, sys, os
TAG = sys.argv[1] if len(sys.argv) > 1 else "probe"
LOG = sys.argv[2] if len(sys.argv) > 2 else None
def log(o):
    if LOG:
        with open(LOG, "a") as f: f.write(json.dumps(o) + "\n")
TOOLS = [
 {"name": "probe_echo", "description": "Echo the given text back, prefixed with ECHO:. Use when asked to echo text with the probe server.",
  "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}},
 {"name": "probe_add", "description": "Add two integers.",
  "inputSchema": {"type": "object", "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}}, "required": ["a", "b"]}},
]
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    try: m = json.loads(line)
    except Exception: continue
    log({"in": m})
    mid, meth = m.get("id"), m.get("method")
    if mid is None: continue
    if meth == "initialize":
        res = {"protocolVersion": (m.get("params") or {}).get("protocolVersion", "2024-11-05"),
               "capabilities": {"tools": {}}, "serverInfo": {"name": TAG, "version": "0.0.1"}}
    elif meth == "tools/list":
        res = {"tools": TOOLS}
    elif meth == "tools/call":
        p = m.get("params") or {}; a = p.get("arguments") or {}
        if p.get("name") == "probe_echo": txt = f"ECHO:{a.get('text')} (server {TAG})"
        elif p.get("name") == "probe_add": txt = str(int(a.get("a", 0)) + int(a.get("b", 0)))
        else: txt = "unknown tool"
        res = {"content": [{"type": "text", "text": txt}]}
    elif meth in ("resources/list",): res = {"resources": []}
    elif meth in ("prompts/list",): res = {"prompts": []}
    elif meth == "ping": res = {}
    else:
        out = {"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "Method not found"}}
        sys.stdout.write(json.dumps(out) + "\n"); sys.stdout.flush(); continue
    out = {"jsonrpc": "2.0", "id": mid, "result": res}
    log({"out": out})
    sys.stdout.write(json.dumps(out) + "\n"); sys.stdout.flush()

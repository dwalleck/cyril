#!/usr/bin/env python3
"""Synthetic stdio MCP canary; records only process and protocol observations."""
import json
import os
from pathlib import Path
import sys
import time

root = Path(sys.argv[1]).resolve(strict=True)
label = sys.argv[2]
assert root.name.startswith('cyril-jlxx-') and root.parent == Path('/tmp')
marker = root / (label + '.jsonl')
def record(event, **fields):
    with marker.open('a') as output:
        output.write(json.dumps({'event': event, 'time_ns': time.time_ns(), 'pid': os.getpid(), **fields}) + '\n')
record('startup')
for line in sys.stdin:
    request = json.loads(line)
    method = request.get('method')
    record('request', method=method)
    if 'id' not in request:
        continue
    if method == 'initialize':
        result = {'protocolVersion': request['params']['protocolVersion'], 'capabilities': {'tools': {}}, 'serverInfo': {'name': label, 'version': '1.0'}}
    elif method == 'tools/list':
        result = {'tools': [{'name': 'read_canary', 'description': 'Return the synthetic read-only canary string. No publication or external access.', 'inputSchema': {'type': 'object', 'properties': {}, 'additionalProperties': False}, 'annotations': {'readOnlyHint': True, 'destructiveHint': False, 'idempotentHint': True, 'openWorldHint': False}}]}
    elif method == 'tools/call':
        record('invocation', tool=request['params']['name'])
        result = {'content': [{'type': 'text', 'text': 'SYNTHETIC_MCP_CANARY_' + label}], 'isError': False}
    elif method == 'ping':
        result = {}
    else:
        print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'error': {'code': -32601, 'message': 'Unsupported method'}}), flush=True)
        continue
    print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': result}), flush=True)
record('stdin_closed')

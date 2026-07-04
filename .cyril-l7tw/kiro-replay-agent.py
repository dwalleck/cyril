#!/usr/bin/env python3
"""cyril-l7tw replay agent: speaks real ACP frames captured from the committed
kiro-cli 2.11.0 v2 live trace (experiments/conductor-spike/v2-live-session-
trace-2.11.0.jsonl). On session/prompt it streams agent_message_chunk updates
forever (200ms cadence) and never completes the turn — the probe SIGKILLs it
mid-stream. The system under test is cyril's bridge, not this script.
"""
import json
import sys
import threading
import time

# Verbatim result frames from the 2.11.0 trace (lists trimmed, shapes intact).
INIT_RESULT = {"protocolVersion": 1, "agentCapabilities": {"loadSession": True, "promptCapabilities": {"image": True, "audio": False, "embeddedContext": False}, "mcpCapabilities": {"http": True, "sse": False}, "sessionCapabilities": {}, "auth": {}}, "authMethods": [], "agentInfo": {"name": "Kiro CLI Agent", "title": "Kiro CLI Agent", "version": "2.11.0"}}
NEW_RESULT = {"sessionId": "786acc7e-e731-4bd1-84c9-fca7cd6b2bfc", "modes": {"currentModeId": "kiro_default", "availableModes": [{"id": "code-reviewer", "name": "code-reviewer", "description": "Reviews code for adherence to project guidelines, style guides, and best practices."}]}, "models": {"currentModelId": "claude-haiku-4.5", "availableModels": [{"modelId": "auto", "name": "auto", "description": "Models chosen by task for optimal usage and consistent quality"}]}}

SESSION_ID = NEW_RESULT["sessionId"]
out_lock = threading.Lock()


def send(obj):
    with out_lock:
        sys.stdout.write(json.dumps(obj) + "\n")
        sys.stdout.flush()


def stream_forever():
    n = 0
    while True:
        n += 1
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": SESSION_ID,
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": f"{n}\n"}}}})
        time.sleep(0.2)


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    method, mid = msg.get("method"), msg.get("id")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": mid, "result": INIT_RESULT})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": mid, "result": NEW_RESULT})
    elif method == "session/prompt":
        threading.Thread(target=stream_forever, daemon=True).start()
        # No response ever sent — the probe kills us mid-stream.
    elif mid is not None:
        send({"jsonrpc": "2.0", "id": mid, "result": {}})
    # Notifications from the client are ignored.

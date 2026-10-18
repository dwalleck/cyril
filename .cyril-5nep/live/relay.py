#!/usr/bin/env -S python3 -I
"""Stdio relay standing in for KAS's `node` (via KIRO_AGENT_PATH).

Runs the real node (RELAY_REAL_NODE) with the arguments cyril passes, copies
client->agent bytes through unchanged, and in agent->client
`_kiro/hooks/list` requests replaces a present `toolTags` with the JSON value
in RELAY_TAGS. Every rewrite is logged to RELAY_LOG as JSONL. With RELAY_TAGS
unset the relay is a pure pass-through that still logs hooks/list frames.
"""
import json
import os
import subprocess
import sys
import threading

real_node = os.environ["RELAY_REAL_NODE"]
log_path = os.environ["RELAY_LOG"]
tags_env = os.environ.get("RELAY_TAGS")
replacement = json.loads(tags_env) if tags_env is not None else None
log_lock = threading.Lock()


def log(record):
    with log_lock, open(log_path, "a") as out:
        out.write(json.dumps(record) + "\n")


child = subprocess.Popen(
    [real_node, *sys.argv[1:]], stdin=subprocess.PIPE, stdout=subprocess.PIPE
)
assert child.stdin is not None and child.stdout is not None


def client_to_agent():
    for line in sys.stdin.buffer:
        child.stdin.write(line)
        child.stdin.flush()
    child.stdin.close()


threading.Thread(target=client_to_agent, daemon=True).start()

for line in child.stdout:
    try:
        message = json.loads(line)
    except ValueError:
        message = None
    if isinstance(message, dict) and message.get("method") == "_kiro/hooks/list":
        params = message.get("params", {})
        record = {"id": message.get("id"), "original": json.loads(json.dumps(params))}
        if replacement is not None and "toolTags" in params:
            params["toolTags"] = replacement
            line = (json.dumps(message) + "\n").encode()
            record["rewritten"] = params
        log(record)
    sys.stdout.buffer.write(line)
    sys.stdout.buffer.flush()

sys.exit(child.wait())

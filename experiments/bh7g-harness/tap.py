#!/usr/bin/env python3
"""bh7g wire tap: sits between cyril-core's bridge and the real `node acp-server.js`.

Invoked (via node-shim.sh) as: tap.py <capture.jsonl> <real-node> <server.js> <args...>
Pumps stdin -> child.stdin and child.stdout -> stdout line-by-line (ACP frames are
newline-delimited JSON), recording every frame as {ts, dir, msg} with auth values
redacted. child.stderr is passed through to our stderr and logged as text.
"""

import json
import subprocess
import sys
import threading
import time

REDACT = {"accessToken", "access_token", "expiresAt", "expires_at", "profileArn", "profile_arn"}


def redact(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in REDACT else redact(v)) for k, v in obj.items()}
    if isinstance(obj, list):
        return [redact(v) for v in obj]
    return obj


def main():
    cap_path, real_node, *rest = sys.argv[1:]
    cap = open(cap_path, "a", buffering=1)
    lock = threading.Lock()

    def log(direction, payload):
        with lock:
            cap.write(json.dumps({"ts": time.time(), "dir": direction, "msg": payload}) + "\n")

    child = subprocess.Popen(
        [real_node, *rest],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    def pump(src, dst, direction):
        for line in iter(src.readline, b""):
            try:
                dst.write(line)
                dst.flush()
            except (BrokenPipeError, ValueError):
                break
            try:
                log(direction, redact(json.loads(line)))
            except (json.JSONDecodeError, UnicodeDecodeError):
                log(direction + ":raw", line[:2000].decode("utf-8", "replace"))
        log(direction + ":eof", None)
        if direction == "client->agent":
            # Consumer died or closed stdin. acp-server.js does NOT exit on
            # stdin EOF (bh7g orphan finding) — reap it after a grace period.
            def reap():
                time.sleep(15)
                if child.poll() is None:
                    log("tap-reap", "killing node after stdin EOF grace")
                    child.kill()
            threading.Thread(target=reap, daemon=True).start()

    def pump_err():
        for line in iter(child.stderr.readline, b""):
            sys.stderr.buffer.write(line)
            sys.stderr.buffer.flush()
            log("agent-stderr", line[:2000].decode("utf-8", "replace"))
        log("agent-stderr:eof", None)

    threads = [
        threading.Thread(target=pump, args=(sys.stdin.buffer, child.stdin, "client->agent"), daemon=True),
        threading.Thread(target=pump, args=(child.stdout, sys.stdout.buffer, "agent->client"), daemon=True),
        threading.Thread(target=pump_err, daemon=True),
    ]
    for t in threads:
        t.start()
    rc = child.wait()
    log("child-exit", rc)
    time.sleep(0.2)
    sys.exit(rc)


if __name__ == "__main__":
    main()

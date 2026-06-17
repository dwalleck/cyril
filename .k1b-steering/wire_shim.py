#!/usr/bin/env python3
# ORACLE (prove-it-prototype, K1b mid-turn steering). A transparent stdio shim:
# cyril's bridge spawns THIS instead of kiro-cli, so every newline-delimited
# JSON-RPC frame crossing the wire is timestamped, independent of cyril's own
# notification parsing. Direction tags:
#   C2A = client(cyril) -> agent(kiro)   (e.g. session/prompt request, _session/steer)
#   A2C = agent(kiro) -> client(cyril)   (e.g. session/prompt RESPONSE, steering echoes)
# Usage (as agent command):  python3 wire_shim.py acp --trust-all-tools
import subprocess, sys, threading, time

LOG = open("/tmp/k1b_wire.log", "w", buffering=1)
T0 = time.monotonic()


def stamp() -> str:
    return f"{time.monotonic() - T0:8.3f}"


def tag_of(line: bytes) -> str:
    s = line.decode("utf-8", "replace")
    for m in ("session/prompt", "_session/steer", "session/cancel", "steering_",
              '"result"', '"error"', "stop_reason", "session/new"):
        if m in s:
            return m
    return "(other)"


proc = subprocess.Popen(["kiro-cli"] + sys.argv[1:],
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE)


def pump(src, dst, direction):
    for line in iter(src.readline, b""):
        LOG.write(f"{stamp()} {direction} {tag_of(line)} :: {line.decode('utf-8','replace')}")
        dst.write(line)
        dst.flush()
    try:
        dst.close()
    except Exception:
        pass


# agent -> client on its own thread; client -> agent on the main thread.
threading.Thread(target=pump, args=(proc.stdout, sys.stdout.buffer, "A2C"), daemon=True).start()
pump(sys.stdin.buffer, proc.stdin, "C2A")
proc.wait()

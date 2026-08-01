#!/usr/bin/env python3
"""Does the model's `[STEERING <id>: …]` receipt reach cyril as agent_message_chunks?

cyril-3qwa harvests that trailer out of the streaming agent text. The whole
design rests on one assumption the AWS prompt logs CANNOT confirm — they capture
what the backend received and what it replied, not how the reply is framed on
the ACP wire. If the receipt arrived as anything other than
`session/update: agent_message_chunk` (a distinct variant, an `_kiro/*`
extension, a separate notification), the harvester would never see it.

This drives real turns through the v2 mock backend, which streams each inner
string of the scripted response as its own chunk. Scripting the trailer SPLIT
ACROSS chunk boundaries reproduces exactly the case the harvester must survive:
a marker that is never whole in any single notification.

ZERO CREDITS: no network, no real model call.

    probe-steering-receipt-chunking-2.16.0.py <path-to-kiro-cli-chat> <out.jsonl>

Isolation follows reference/feedback_isolate_kiro_probes_with_home: a scratch
HOME per run so the probe never touches the real ~/.kiro.
"""
import json, os, subprocess, threading, queue, time, tempfile, sys

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

HOME = tempfile.mkdtemp(prefix="steerreceipt-home-")
CWD = tempfile.mkdtemp(prefix="steerreceipt-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

RECEIPT_ID = "steer-3f2a9c14-7b6d-4e05-9a81-2c5d8e0b41f7"
# Turn 1: trailer split mid-id and mid-note — the adversarial chunking.
# Turn 2: trailer whole in one chunk — the easy case, as a control.
MOCK = os.path.join(CWD, "mock.json")
with open(MOCK, "w") as fh:
    json.dump([
        ["Committed and pushed.\n\n[STEER", "ING " + RECEIPT_ID[:12],
         RECEIPT_ID[12:] + ": Applied dir", "ectly — used the region sheet.]"],
        ["done. [STEERING %s: whole.]" % RECEIPT_ID],
    ], fh)

env = dict(os.environ)
env["KIRO_MOCK_CHAT_RESPONSE"] = MOCK
env["HOME"] = HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")

p = subprocess.Popen([KIRO, "acp"], cwd=CWD, env=env, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
CHUNKS = []      # every agent_message_chunk text, in arrival order
OTHER = []       # any other notification method/variant seen during the turns


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m,
                              "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def pump(until_id, budget=30.0):
    """Drain frames until the response to `until_id` arrives."""
    deadline = time.time() + budget
    while time.time() < deadline:
        try:
            line = q.get(timeout=0.4)
        except queue.Empty:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        OUT.write(line + "\n")
        if msg.get("method") == "session/update":
            up = msg.get("params", {}).get("update", {})
            kind = up.get("sessionUpdate")
            if kind == "agent_message_chunk":
                CHUNKS.append(up.get("content", {}).get("text", ""))
            else:
                OTHER.append(kind)
        elif "method" in msg:
            OTHER.append(msg["method"])
        if msg.get("id") == until_id and ("result" in msg or "error" in msg):
            return msg
    return None


pump(req("initialize", {"protocolVersion": 1,
                        "clientCapabilities": {"fs": {"readTextFile": False,
                                                      "writeTextFile": False}},
                        "clientInfo": {"name": "cyril", "version": "probe"}}))
sess = pump(req("session/new", {"cwd": CWD, "mcpServers": []}))
sid = sess["result"]["sessionId"]

for turn in (1, 2):
    CHUNKS.clear()
    pump(req("session/prompt", {"sessionId": sid,
                                "prompt": [{"type": "text", "text": "go"}]}))
    joined = "".join(CHUNKS)
    print(f"--- turn {turn} ---")
    print(f"  chunks           : {len(CHUNKS)}")
    print(f"  per-chunk texts  : {CHUNKS}")
    print(f"  reassembled      : {joined!r}")
    whole_in_one = any("[STEERING" in c and "]" in c for c in CHUNKS)
    print(f"  marker whole in a single chunk? {whole_in_one}")
    print(f"  marker present after reassembly? {'[STEERING' in joined and joined.rstrip().endswith(']')}")
    print(f"  non-chunk notifications during turn: {sorted(set(OTHER))}")

p.stdin.close()
p.wait(timeout=5)
OUT.close()
print("\nwire log:", sys.argv[2])

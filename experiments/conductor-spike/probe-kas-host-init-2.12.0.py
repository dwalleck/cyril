#!/usr/bin/env python3
"""Host-path KAS parity leg for release audits: spawn `<kiro-cli-chat> acp
--agent-engine kas` (the real Rust launch contract, NOT direct node spawn),
send initialize, capture the response, normalize volatile fields (logDir
timestamps), and dump canonical JSON for A/B diffing between binary versions.

    probe-kas-host-init-2.12.0.py <path-to-kiro-cli-chat> <out.json>

The bundle content being byte-identical does not cover this leg — the Rust
side owns extraction, node discovery, and CLI args. First used 2.11.1 audit
(inline); scripted for 2.12.0."""
import json, re, subprocess, threading, queue, sys, tempfile, time

KIRO, OUT = sys.argv[1], sys.argv[2]
CWD = tempfile.mkdtemp(prefix="kas-hostinit-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                          "params": {"protocolVersion": 1, "clientCapabilities": {}}}) + "\n")
p.stdin.flush()
resp = None
end = time.time() + 60
while time.time() < end:
    try:
        raw = q.get(timeout=2)
    except queue.Empty:
        continue
    try:
        o = json.loads(raw)
    except Exception:
        continue
    if o.get("id") == 1 and "result" in o:
        resp = o
        break
p.terminate()
if resp is None:
    sys.exit("no initialize response within 60s")
canon = json.dumps(resp, sort_keys=True, indent=1)
# logDir carries a per-run timestamped path — normalize for byte diffing
# (both dashed ISO and the compact 20260710T023818254 form kiro logs use)
canon = re.sub(r'[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9-:.]+', 'TS', canon)
canon = re.sub(r'[0-9]{8}T[0-9]{9}', 'TS', canon)
canon = re.sub(r'kas-hostinit-[a-z0-9_]+', 'CWD', canon)
open(OUT, "w").write(canon + "\n")
print(f"wrote {OUT} ({len(canon)} bytes)")

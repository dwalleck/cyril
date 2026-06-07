#!/usr/bin/env python3
"""
Probe: 2.5.0 "Trusting a tool now automatically approves all other pending
invocations of the same tool in the current batch."

Question for cyril: when the agent fires several same-tool permission requests
in parallel and the client answers ONE with allow_always, does the backend
auto-resolve the SIBLING requests in a way the client can observe — or does it
leave them dangling (which would strand stale permission overlays in cyril)?

Method:
  - Prompt the agent to run several independent shell commands at once.
  - Collect every session/request_permission. Answer ONLY the first one
    (allow_always); deliberately leave the rest UNANSWERED.
  - Watch whether: (a) the turn still completes (=> backend didn't need our
    answers for the siblings, i.e. it auto-resolved them), and (b) the backend
    sends anything referencing the unanswered request ids (a cancellation /
    retraction cyril could use to dismiss overlays).
"""
import json
import os
import subprocess
import threading
import time
from collections import Counter

KIRO = os.path.expanduser(os.environ.get("KIRO_BIN", "~/.local/bin/kiro-cli-chat"))
CWD = "/home/dwalleck/repos/cyril"
LOG = "/tmp/cyril-probe-trust-batch.log"
PROMPT = (
    "Run these three commands, each as its OWN separate shell tool call, and issue "
    "all three at once (in parallel) before waiting: `echo alpha`, `echo bravo`, "
    "`echo charlie`. Do not combine them into one command."
)


def main():
    log = open(LOG, "w")
    proc = subprocess.Popen([KIRO, "acp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, cwd=CWD)
    inc, lock, nid = [], threading.Lock(), [1]

    def reader():
        while True:
            ln = proc.stdout.readline()
            if not ln:
                return
            t = ln.decode(errors="replace").rstrip("\n")
            log.write(f"S->C {t}\n"); log.flush()
            try:
                with lock:
                    inc.append(json.loads(t))
            except json.JSONDecodeError:
                pass
    threading.Thread(target=reader, daemon=True).start()

    def send(method, params, resp=True):
        m = {"jsonrpc": "2.0", "method": method, "params": params}
        if resp:
            m["id"] = nid[0]; nid[0] += 1
        log.write(f"C->S {json.dumps(m)}\n"); log.flush()
        proc.stdin.write((json.dumps(m) + "\n").encode()); proc.stdin.flush()
        return m.get("id")

    def send_resp(rid, result):
        m = {"jsonrpc": "2.0", "id": rid, "result": result}
        log.write(f"C->S {json.dumps(m)}\n"); log.flush()
        proc.stdin.write((json.dumps(m) + "\n").encode()); proc.stdin.flush()

    def waitfor(rid, t=20):
        d = time.time() + t
        while time.time() < d:
            with lock:
                for f in inc:
                    if f.get("id") == rid and ("result" in f or "error" in f):
                        return f
            time.sleep(0.05)
        return None

    # Answer ONLY the first permission request (allow_always); track the rest.
    perm_seen = []          # (req_id, tool title) in arrival order
    perm_answered = []      # req_ids we responded to

    def perm_thread():
        first_done = [False]
        while True:
            with lock:
                frames = list(inc)
            for f in frames:
                if f.get("method") == "session/request_permission" and f.get("id") not in [p[0] for p in perm_seen]:
                    p = f.get("params", {})
                    title = (p.get("toolCall") or {}).get("title") or p.get("title") or "?"
                    perm_seen.append((f["id"], title))
                    if not first_done[0]:
                        opts = p.get("options", [])
                        always = next((o for o in opts if o.get("kind") == "allow_always"),
                                      next((o for o in opts if o.get("kind") == "allow_once"), None))
                        if always:
                            send_resp(f["id"], {"outcome": {"outcome": "selected", "optionId": always["optionId"]}})
                            perm_answered.append(f["id"])
                            first_done[0] = True
                    # else: deliberately leave unanswered
            time.sleep(0.05)
    threading.Thread(target=perm_thread, daemon=True).start()

    send("initialize", {"protocolVersion": 1, "clientCapabilities": {},
                        "clientInfo": {"name": "cyril-probe", "version": "0.0.1"}})
    time.sleep(0.4)
    sid = waitfor(send("session/new", {"cwd": CWD, "mcpServers": []}), 20)["result"]["sessionId"]
    print(f"[*] session {sid}; prompting for 3 parallel shell calls, answering only the first permission")

    with lock:
        cutoff = len(inc)
    rid = send("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
    r = waitfor(rid, 60)
    completed = r is not None and "result" in r
    stop = (r or {}).get("result", {}).get("stopReason")
    time.sleep(1.0)

    # Analysis
    with lock:
        turn = inc[cutoff:]
    answered_ids = set(perm_answered)
    unanswered = [(rid_, title) for (rid_, title) in perm_seen if rid_ not in answered_ids]

    # Did the backend reference any unanswered request id afterwards? (cancellation/retraction)
    referenced = []
    for f in turn:
        blob = json.dumps(f)
        for (rid_, _title) in unanswered:
            if f.get("method") and f.get("id") != rid_ and str(rid_) in blob and f.get("method") != "session/request_permission":
                referenced.append((f.get("method"), rid_))

    method_counts = Counter(f.get("method") for f in turn if f.get("method"))

    print("\n========== TRUST-BATCH RESULTS ==========")
    print(f"permission requests received : {len(perm_seen)}")
    for rid_, title in perm_seen:
        mark = "ANSWERED(always)" if rid_ in answered_ids else "left UNANSWERED"
        print(f"    id={rid_}  [{mark}]  tool={title}")
    print(f"prompt completed            : {completed}  (stopReason={stop})")
    print(f"unanswered requests         : {len(unanswered)}")
    print(f"backend referenced unanswered ids afterwards: {referenced or 'NONE'}")
    print("\nInterpretation:")
    if completed and len(unanswered) > 0 and not referenced:
        print("  >> Turn COMPLETED despite unanswered sibling requests, with NO retraction.")
        print("     => backend auto-approved siblings server-side; client gets NO signal.")
        print("     => cyril GAP: stale permission overlays for the unanswered siblings.")
    elif completed and not unanswered:
        print("  >> Agent serialized (only one permission at a time) — batch not exercised.")
    elif not completed:
        print("  >> Turn did NOT complete — backend still waiting on sibling responses (no client-side auto-approve).")
    print("\nmethods this turn:", dict(method_counts))
    print("full log:", LOG)
    proc.terminate()


if __name__ == "__main__":
    main()

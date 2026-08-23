#!/usr/bin/env python3
"""omp acp surface spike — can cyril-style raw ACP drive oh-my-pi?

Sequence (one paid mini-turn at the end):
  1. initialize   — advertise fs+terminal client caps, capture agentInfo /
                    authMethods / agentCapabilities
  2. authenticate — methodId "agent" (use local ~/.omp credentials)
  3. session/new  — capture sessionId, configOptions, modes
  4. session/list — standard-ACP session listing (kiro v1/v2 stubs this)
  5. _omp/sessions/listAll {limit:3} + _omp/usage — ext-method dialect
  6. session/prompt "Reply with exactly: OK" — record every session/update
     kind, host callbacks, and the stopReason
  7. session/close

Real HOME on purpose: omp's "agent" auth method reads ~/.omp. cwd is a
throwaway dir so tool use (none expected) cannot touch repos. All frames
both directions land in the out file as {ts, dir, msg} JSONL
(KIRO_ACP_RECORD_PATH-compatible for diff-acp-wire.py).

    probe-omp-acp-surface.py <out.jsonl>
"""

import json
import os
import signal
import subprocess
import sys
import tempfile
import time

OUT = open(sys.argv[1], "w")
cwd = tempfile.mkdtemp(prefix="omp-spike-cwd-")

proc = subprocess.Popen(
    ["omp", "acp"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=open(sys.argv[1] + ".stderr", "wb"),
    start_new_session=True,
)
nid = [0]
UPDATE_KINDS = []  # ordered sessionUpdate markers for the turn
HOST_CALLS = []    # agent->client requests seen


def record(direction, msg):
    OUT.write(json.dumps({"ts": time.time(), "dir": direction, "msg": msg}) + "\n")
    OUT.flush()


def send(obj):
    record("client->agent", obj)
    proc.stdin.write((json.dumps(obj) + "\n").encode())
    proc.stdin.flush()


def request(method, params):
    nid[0] += 1
    send({"jsonrpc": "2.0", "id": nid[0], "method": method, "params": params})
    return nid[0]


def answer_host_request(m):
    """Respond to an agent->client request; reject anything that would act."""
    meth, rid = m["method"], m["id"]
    HOST_CALLS.append(meth)
    if meth == "session/request_permission":
        opts = (m.get("params") or {}).get("options") or []
        reject = next((o for o in opts if "reject" in (o.get("kind") or "")), None)
        chosen = reject or (opts[0] if opts else None)
        outcome = (
            {"outcome": "selected", "optionId": chosen["optionId"]}
            if chosen
            else {"outcome": "cancelled"}
        )
        send({"jsonrpc": "2.0", "id": rid, "result": {"outcome": outcome}})
    else:
        # fs/terminal callbacks are not expected for a pure-text turn; an
        # empty result keeps the agent from hanging while staying visible
        # in the capture.
        send({"jsonrpc": "2.0", "id": rid, "result": {}})


def pump(until, to=90):
    end = time.time() + to
    while time.time() < end:
        line = proc.stdout.readline()
        if not line:
            return None
        try:
            m = json.loads(line)
        except json.JSONDecodeError:
            continue
        record("agent->client", m)
        meth, rid = m.get("method"), m.get("id")
        if meth and rid is not None:
            answer_host_request(m)
            continue
        if meth == "session/update":
            u = (m.get("params") or {}).get("update") or {}
            kind = u.get("sessionUpdate")
            if kind == "session_info_update":
                kind = f"session_info_update:{u.get('kind')}"
            UPDATE_KINDS.append(kind)
        if rid == until and ("result" in m or "error" in m):
            return m
    return None


def result_of(m):
    return (m or {}).get("result") or {}


print("=== 1. initialize ===")
iid = request(
    "initialize",
    {
        "protocolVersion": 1,
        "clientCapabilities": {
            "fs": {"readTextFile": True, "writeTextFile": True},
            "terminal": True,
        },
        "clientInfo": {"name": "cyril-spike", "version": "0.0.0"},
    },
)
res = result_of(pump(iid, 60))
print("agentInfo:", json.dumps(res.get("agentInfo")))
print("authMethods:", json.dumps(res.get("authMethods")))
print("agentCapabilities:", json.dumps(res.get("agentCapabilities")))

print("=== 2. authenticate(agent) ===")
aid = request("authenticate", {"methodId": "agent"})
auth = pump(aid, 30)
print("auth response:", json.dumps({k: v for k, v in (auth or {}).items() if k in ("result", "error")}))

print("=== 3. session/new ===")
sid_req = request("session/new", {"cwd": cwd, "mcpServers": []})
sres = result_of(pump(sid_req, 120))
sid = sres.get("sessionId")
print("sessionId:", sid)
print("modes:", json.dumps(sres.get("modes")))
for o in sres.get("configOptions") or []:
    opts = o.get("options") or []
    names = [x.get("name") or x.get("value") for x in opts]
    print(f"configOption {o.get('id')!r} current={o.get('currentValue') or o.get('current')}: "
          f"{len(opts)} options {names[:8]}{'...' if len(names) > 8 else ''}")

print("=== 4. session/list ===")
lid = request("session/list", {})
lres = pump(lid, 30)
lr = result_of(lres)
sessions = lr.get("sessions")
if sessions is None:
    print("session/list raw:", json.dumps(lres and {k: lres[k] for k in ("result", "error") if k in lres}))
else:
    print(f"session/list: {len(sessions)} sessions; first: "
          + json.dumps(sessions[0] if sessions else None)[:300])

print("=== 5. ext methods ===")
eid = request("_omp/sessions/listAll", {"limit": 3})
er = pump(eid, 30)
print("_omp/sessions/listAll:", json.dumps(result_of(er))[:400])
uid = request("_omp/usage", {})
ur = pump(uid, 30)
print("_omp/usage:", json.dumps(result_of(ur))[:400])

print("=== 6. mini prompt turn ===")
UPDATE_KINDS.clear()
tid = request(
    "session/prompt",
    {"sessionId": sid, "prompt": [{"type": "text", "text": "Reply with exactly: OK"}]},
)
tr = pump(tid, 240)
print("stopReason:", result_of(tr).get("stopReason"), "| error:", (tr or {}).get("error"))
print("update kinds in order:", UPDATE_KINDS)
print("host callbacks seen:", HOST_CALLS)

print("=== 7. session/close ===")
cid = request("session/close", {"sessionId": sid})
cr = pump(cid, 30)
print("close:", json.dumps(cr and {k: cr[k] for k in ("result", "error") if k in cr}))

OUT.close()
try:
    os.killpg(proc.pid, signal.SIGKILL)
except ProcessLookupError:
    pass

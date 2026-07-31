#!/usr/bin/env python3
"""Full-surface A/B of KAS **real turn** traffic — the audit lane that has never existed.

Every other KAS probe in this tree is targeted at one feature (steering, hooks,
modes, orchestrate, …). Nothing captures the whole turn surface in one
binary-parameterized run suitable for an A/B, which is why the 2.16.0 audit had
to record "no KAS turn traffic" as a residual gap. This is that harness.

Unlike v2 there is NO mock backend for KAS (`KIRO_MOCK_CHAT_RESPONSE` is read by
the two Rust crates; KAS has its own TS backend client), so **every turn here is
a paid model call.** Run it deliberately.

Runs on cyril's real path: `kiro-cli-chat acp --agent-engine kas`.

SCENARIO — four turns chosen to maximise wire surface per paid turn:
  1. read      — provokes fs tool calls + `_kiro/fs/*` host callbacks
  2. exit-code — runs a command that exits NON-ZERO. Doubles as the residual
                 live falsifier named in reference_kiro_terminal_wait_exit_reply_shape:
                 reply FLAT and assert KAS surfaces the real code.
  3. write     — provokes fs_write + the kiro-snapshot-v2:// checkpoint URIs
  4. subtask   — provokes the KAS subagent path (`_meta.kiro.kind: agent-subtask`)

HOST CALLBACK REPLY SHAPES — the load-bearing detail. Per
reference_kiro_terminal_wait_exit_reply_shape, `terminal/wait_for_exit` is
**FLAT `{exitCode, signal}`** (serde flatten, no wrapper) while `terminal/output`
is **nested** under `exitStatus`. probe-kas-fs-terminal-host-2.10.0.py replies
nested for BOTH; that was tolerated only because it ran `echo` (exit 0) and would
silently zero a non-zero code. This probe uses the typed shapes and turn 2 proves it.

OUTPUT is written directly in `diff-acp-wire.py`'s record format
(`{direction, envelope, method, parsed}`) so the two legs can be diffed with no
adapter:

    probe-kas-turn-traffic-ab-2.16.0.py <kiro-cli-chat> <out.jsonl>
    diff-acp-wire.py kas-turn-2.15.0.jsonl kas-turn-2.16.0.jsonl \\
        --label-old 2.15.0 --label-new 2.16.0

NOTE the differ aggregates field-paths per message key, which (as the 2.16.0 v2
turn audit found the hard way) can hide a change in WHICH frame carries a field.
This probe therefore also prints an ordered per-frame summary.

HOME-isolated per feedback_isolate_kiro_probes_with_home.
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
NONZERO_EXIT = 3          # turn 2's expected exit code

CWD = tempfile.mkdtemp(prefix="kas-turn-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
pathlib.Path(CWD, "probe.txt").write_text("The magic number is 4242.\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)

TURNS = [
    ("read",
     "Read the file probe.txt in the current directory and reply with just the magic number."),
    ("exitcode",
     f"Run this exact shell command: sh -c 'exit {NONZERO_EXIT}'. "
     f"Then tell me the numeric exit code it returned. Do not run anything else."),
    ("write",
     "Create a file named summary.txt containing exactly one line: done. Then stop."),
    ("subtask",
     "Use a subagent to count the characters in the string 'kiro'. Report only the number."),
]


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute(
            "select value from auth_kv where key in "
            "('kirocli:odic:token','kirocli:social:token') order by key desc"
        ).fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    if row is None:
        raise SystemExit("logged out — no token")
    v = row[0]
    v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    parn = d.get("profile_arn")
    if not parn and prow:
        pv = prow[0]
        pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try:
            parn = json.loads(pv).get("arn")
        except Exception:
            pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": parn}


TOK = read_token()
TMPH = tempfile.mkdtemp(prefix="kas-turnhome-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()

i = [0]
TERMS = {}
CALLBACKS = []      # (turn_tag, method) every server->client request
FRAMES = []         # (turn_tag, key) ordered, for the per-frame summary
AGENT = {}          # turn_tag -> concatenated agent text


# Our own reply to `_kiro/auth/getAccessToken` carries a live bearer token, so a
# naive capture writes credentials to disk — the defect cyril-hhgw tracks in two
# older probes. Scrub at emit time so the capture is committable by construction
# rather than needing a post-hoc pass someone can forget.
SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}


def scrub(obj):
    """Deep-copy with credential values replaced. Applied only on the way to the log."""
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else scrub(obj[k]))
                for k in obj}
    if isinstance(obj, list):
        return [scrub(x) for x in obj]
    return obj


def emit(direction, envelope, method, parsed):
    OUT.write(json.dumps({"direction": direction, "envelope": envelope,
                          "method": method, "parsed": scrub(parsed)}) + "\n")
    OUT.flush()


def send(obj, method=None, envelope="request"):
    p.stdin.write(json.dumps(obj) + "\n")
    p.stdin.flush()
    emit("client_to_agent", envelope, method, obj)


def req(m, pr):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}, method=m)
    return i[0]


def reply(rid, res):
    send({"jsonrpc": "2.0", "id": rid, "result": res}, envelope="response")


def rerr(rid, msg):
    send({"jsonrpc": "2.0", "id": rid, "error": {"code": -32000, "message": msg}},
         envelope="response")


def abspath(pth):
    return pth if os.path.isabs(pth or "") else os.path.join(CWD, pth or "")


def handle_request(m, rid, pr, tag):
    CALLBACKS.append((tag, m))
    if m == "_kiro/auth/getAccessToken":
        return reply(rid, TOK)
    if m == "_kiro/terminal/shell_type":
        return reply(rid, {"shellType": "bash"})
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try:
            return reply(rid, {"content": pathlib.Path(abspath(pr.get("path"))).read_text()})
        except Exception as e:
            return rerr(rid, f"read failed: {e}")
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        try:
            ap = pathlib.Path(abspath(pr.get("path")))
            ap.parent.mkdir(parents=True, exist_ok=True)
            ap.write_text(pr.get("content", ""))
            return reply(rid, {})
        except Exception as e:
            return rerr(rid, f"write failed: {e}")
    if m == "_kiro/fs/read_directory":
        try:
            entries = [{"name": e.name, "type": "directory" if e.is_dir() else "file"}
                       for e in pathlib.Path(abspath(pr.get("path"))).iterdir()]
            return reply(rid, {"entries": entries})
        except Exception as e:
            return rerr(rid, str(e))
    if m == "_kiro/fs/stat":
        ap = pathlib.Path(abspath(pr.get("path")))
        if not ap.exists():
            return rerr(rid, "not found")
        return reply(rid, {"type": "directory" if ap.is_dir() else "file",
                           "size": ap.stat().st_size})
    if m == "_kiro/fs/delete":
        try:
            pathlib.Path(abspath(pr.get("path"))).unlink()
            return reply(rid, {})
        except Exception as e:
            return rerr(rid, str(e))
    if m == "terminal/create":
        # KAS sends `command` as a FULL SHELL STRING with no `args` array —
        # observed live: {"command": "sh -c 'exit 3'"}. Exec'ing [command, *args]
        # argv-style looks for a file of that literal name and fails ENOENT,
        # which is exactly the defect cyril-6bol tracks ("create runs argv with
        # no shell"). A correct host runs it THROUGH a shell.
        cmd, args = pr.get("command", ""), pr.get("args") or []
        tid = f"term-{len(TERMS) + 1}"
        try:
            if args:
                r = subprocess.run([cmd, *args], cwd=pr.get("cwd") or CWD,
                                   capture_output=True, text=True, timeout=60)
            else:
                r = subprocess.run(cmd, shell=True, cwd=pr.get("cwd") or CWD,
                                   capture_output=True, text=True, timeout=60)
            TERMS[tid] = {"out": r.stdout + r.stderr, "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 127}
        return reply(rid, {"terminalId": tid})
    if m == "terminal/output":
        t = TERMS.get(pr.get("terminalId"), {"out": "", "code": 0})
        # terminal/output: exit_status is a NAMED field -> NESTED (correct here)
        return reply(rid, {"output": t["out"], "truncated": False,
                           "exitStatus": {"exitCode": t["code"], "signal": None}})
    if m == "terminal/wait_for_exit":
        t = TERMS.get(pr.get("terminalId"), {"code": 0})
        # FLAT — WaitForTerminalExitResponse.exit_status is #[serde(flatten)].
        # Replying nested here silently zeroes non-zero exits. Turn 2 proves it.
        return reply(rid, {"exitCode": t["code"], "signal": None})
    if m in ("terminal/release", "terminal/kill"):
        return reply(rid, {})
    if m == "session/request_permission":
        opts = pr.get("options", [])
        pick = next((x for x in opts
                     if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                    opts[0] if opts else None)
        return reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                     if pick else {"outcome": {"outcome": "cancelled"}})
    return reply(rid, {})


def pump(until, to=90, tag="init"):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=2)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        m, rid = o.get("method"), o.get("id")
        pr = o.get("params") or {}
        env = "notification" if (m and rid is None) else ("request" if m else "response")
        emit("agent_to_client", env, m, o)

        if m and rid is None:
            key = m
            if m.endswith("session/update"):
                u = pr.get("update") or {}
                v = u.get("sessionUpdate")
                kind = ((u.get("_meta") or {}).get("kiro") or {}).get("kind")
                key = f"{m}::{v}" + (f"[{kind}]" if kind else "")
                if v == "agent_message_chunk":
                    AGENT[tag] = AGENT.get(tag, "") + (u.get("content") or {}).get("text", "")
            FRAMES.append((tag, key))
        if rid is not None and m:
            handle_request(m, rid, pr, tag)
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {
        "fs": {"readTextFile": True, "writeTextFile": True},
        "terminal": True,
    },
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True}},
})
pump(1, 40)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 90)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 6)

for tag, text in TURNS:
    print(f"\n########## turn: {tag} ##########")
    t0 = time.time()
    rid = req("session/prompt", {"sessionId": sid,
                                 "prompt": [{"type": "text", "text": text}]})
    r = pump(rid, 420, tag=tag)
    stop = ((r or {}).get("result") or {}).get("stopReason")
    print(f"  stopReason={stop!r}  {time.time() - t0:.1f}s")
    print(f"  agent: {(AGENT.get(tag) or '')[:220]!r}")
    pump(-1, 8, tag=f"{tag}post")

print("\n=== frames per turn ===")
per = {}
for tag, key in FRAMES:
    per.setdefault(tag, {}).setdefault(key, 0)
    per[tag][key] += 1
for tag in per:
    print(f"\n-- {tag} --")
    for k in sorted(per[tag]):
        print(f"   {per[tag][k]:3}x {k}")

print("\n=== host callbacks invoked ===")
cb = {}
for tag, m in CALLBACKS:
    cb[m] = cb.get(m, 0) + 1
for m in sorted(cb):
    print(f"  {cb[m]:3}x {m}")

print("\n=== FALSIFIER: non-zero terminal exit (flat wait_for_exit reply) ===")
codes = {t: v["code"] for t, v in TERMS.items()}
said = AGENT.get("exitcode", "")
print(f"  host-side terminal exit codes: {codes}")
print(f"  agent reported: {said[:240]!r}")
# The claim under test: we reply FLAT {exitCode,signal} and KAS surfaces the REAL
# code. That is only tested if the host actually produced NONZERO_EXIT — otherwise
# the agent quoting the number proves nothing (it can infer it from the prompt).
host_ok = NONZERO_EXIT in codes.values()
if not host_ok:
    print(f"  VERDICT: VOID — host never produced exit {NONZERO_EXIT} "
          f"(got {sorted(codes.values())}); the agent's answer cannot distinguish "
          f"wire truth from inference. Fix the host, re-run.")
elif str(NONZERO_EXIT) in said:
    print(f"  VERDICT: PASS — host exited {NONZERO_EXIT}, replied FLAT, agent surfaced it.")
else:
    print(f"  VERDICT: FAIL — host exited {NONZERO_EXIT} but the agent did not report it; "
          f"the flat reply may not be reaching the model.")

OUT.close()
p.stdin.close()
p.terminate()

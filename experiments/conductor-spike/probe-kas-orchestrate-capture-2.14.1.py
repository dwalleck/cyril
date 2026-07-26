#!/usr/bin/env python3
"""LIVE KAS OrchestrateSubAgent wire capture (2.14.1 / @kiro/agent 0.22.7).

Spawns the faithful CLI path `kiro-cli-chat acp --agent-engine kas`, runs one real turn
that asks the agent to orchestrate a multi-stage subagent pipeline, and records every
session/update + _kiro/* frame with full subagent tagging. Specifically hunts for:
  - an OrchestrateSubAgent tool_call whose rawInput carries {task, stages[], repeat}
  - agent-subtask tagging (_meta.kiro.{kind, agentSubtaskId}) + the ACP ToolKind
  - any workflow-progress / loop_iteration frames (expected: none — no emitter ships)
Answers _kiro/auth/getAccessToken from a token file, auto-approves permissions, answers
_kiro/userInput if asked. Costs credits.

Usage:
    probe-kas-orchestrate-capture-2.14.1.py <kiro-cli-chat> <out.jsonl> [fresh-token.json]

The optional third argument is REQUIRED in practice: the on-disk
`~/.aws/sso/cache/kiro-auth-token*.json` are stale because kiro-cli refreshes into its
SQLite `auth_kv`, not the JSON, so the default path yields `-32000 TokenInvalidError`.

Building fresh-token.json — measured under GITHUB SOCIAL AUTH (2026-07-26, kiro-cli 2.14.2):
    sqlite3 ~/.local/share/kiro-cli/data.sqlite3 \
        "select value from auth_kv where key='kirocli:social:token'"
Under social auth `auth_kv` is plaintext JSON and the row carries snake_case
{access_token, expires_at, profile_arn, provider, refresh_token} — profile_arn included.
Rewrite to the camelCase shape this probe expects:
    {"accessToken": …, "expiresAt": …, "profileArn": …, "provider": …, "authMethod": …}

AUTH METHOD MATTERS — the above is n=1. Key name varies (`kirocli:social:token`,
`kirocli:odic:token`, `kirocli:external-idp:token`), and profile_arn is only guaranteed
in the row for social ("social token has no profile ARN, treating as invalid"); other
methods resolve it via list_available_profiles ("Lazily resolved profileArn from
list_available_profiles"), and Builder ID has a keychain path distinct from the DB.
Under IdC/Builder ID the older recipe — read `kirocli:odic:token` and merge profileArn
from kiro-auth-token-cli.json — is the correct one. Measure your own row; do not assume
the social shape. See docs/kiro-2.14.1-wire-audit.md.

Credential hygiene: auth responses are sent to KAS in full but written to the capture
with secrets replaced by "<redacted>", matching the committed
kas-live-session-trace-2.11.0.jsonl convention. Never commit an unredacted capture.
"""
import json, os, subprocess, threading, queue, time, tempfile, sys

USAGE = ("Usage: probe-kas-orchestrate-capture-2.14.1.py "
         "<kiro-cli-chat> <out.jsonl> [fresh-token.json]")
TOKEN_RECIPE = (
    "Build fresh-token.json from the CLI's own store. Under GITHUB SOCIAL auth:\n"
    "  sqlite3 ~/.local/share/kiro-cli/data.sqlite3 \\\n"
    "    \"select value from auth_kv where key='kirocli:social:token'\"\n"
    "-> plaintext JSON {access_token, expires_at, profile_arn, provider,\n"
    "refresh_token}; rewrite to camelCase {accessToken, expiresAt, profileArn,\n"
    "provider, authMethod}.\n"
    "AUTH METHOD MATTERS: key is kirocli:odic:token (IdC/Builder ID) or\n"
    "kirocli:external-idp:token elsewhere, and profile_arn is only guaranteed in the\n"
    "row for social — other methods resolve it via list_available_profiles, so merge\n"
    "profileArn from kiro-auth-token-cli.json there. See\n"
    "docs/kiro-2.14.1-wire-audit.md.")

if len(sys.argv) < 3:
    print(USAGE, file=sys.stderr)
    sys.exit(2)

KIRO = sys.argv[1]
OUT = sys.argv[2]
TOKEN = sys.argv[3] if len(sys.argv) > 3 else os.path.expanduser(
    "~/.aws/sso/cache/kiro-auth-token-cli.json")

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer"}


def load_token(path):
    """Load and validate the token once, before spawning Kiro.

    Fails loudly and early: a token problem must not surface as a successful-looking
    capture with zero frames. Error text never includes credential material.
    """
    try:
        with open(path) as fh:
            d = json.load(fh)
    except FileNotFoundError:
        sys.exit(f"FATAL: token file not found: {path}\n{TOKEN_RECIPE}")
    except json.JSONDecodeError as e:
        sys.exit(f"FATAL: token file is not valid JSON ({path}): line {e.lineno} col {e.colno}")
    except OSError as e:
        sys.exit(f"FATAL: cannot read token file ({path}): {e.strerror}")
    if not isinstance(d, dict):
        sys.exit(f"FATAL: token file must contain a JSON object ({path})")
    missing = [k for k in ("accessToken", "expiresAt", "profileArn")
               if not str(d.get(k) or "").strip()]
    if missing:
        sys.exit(f"FATAL: token file missing/empty required field(s): {', '.join(missing)} "
                 f"({path}). Present keys: {sorted(d)}")
    return {"accessToken": d["accessToken"], "expiresAt": d["expiresAt"],
            "profileArn": d["profileArn"], "provider": d.get("provider"),
            "authMethod": d.get("authMethod")}


def redact(obj):
    """Deep-copy with credential values replaced. Applied only on the way to the log."""
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else redact(obj[k]))
                for k in obj}
    if isinstance(obj, list):
        return [redact(x) for x in obj]
    return obj


TOKEN_PAYLOAD = load_token(TOKEN)
print(f"[ok] token validated (profileArn present, expiresAt={TOKEN_PAYLOAD['expiresAt']})")

CWD = tempfile.mkdtemp(prefix="kas-orch-cap-")
# Not auto-deleted: the orchestrated stages write their output here, which is part of
# the capture's evidence. Path is printed at exit.
subprocess.run("git init -q -b main", cwd=CWD, shell=True, check=False)
log = open(OUT, "w")
proc = None


def rec(direction, obj):
    log.write(json.dumps({"d": direction, **redact(obj)}) + "\n")
    log.flush()


def send(o):
    proc.stdin.write(json.dumps(o) + "\n")
    proc.stdin.flush()
    rec("C->A", o)


def req(m, p):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p})
    return i[0]


def rep(rid, res):
    send({"jsonrpc": "2.0", "id": rid, "result": res})


def fail(what, resp):
    """A JSON-RPC error or timeout is a failed capture, not a zero-result capture."""
    if resp is None:
        detail = "no response (timeout)"
    elif "error" in resp:
        e = resp["error"] or {}
        detail = f"error {e.get('code')} {e.get('message')!r}"
    else:
        detail = "malformed response (no result)"
    print(f"\nFATAL: {what} failed: {detail}", file=sys.stderr)
    print("Capture is INVALID — do not read zero-frame counts as evidence.", file=sys.stderr)
    sys.exit(1)


i = [0]
inbound = {}; updates = {}; subtask_ids = set(); meta_kinds = {}
tool_rows = []            # (kind, acpToolKind, title, metaKind, subtaskId, rawInputKeys)
orchestrate_inputs = []   # rawInput of any orchestrate/invoke tool_call
workflow_frames = [0]; loop_frames = [0]; auth_calls = [0]; userinput_calls = [0]


def is_workflow_progress(upd, meta):
    """Both documented recognition paths (see docs/kiro-2.14.1-wire-audit.md ln 113, 126).

    tui.js's convertAcpUpdateToEvent inspects each user_message_chunk for
    _meta.kiro.notification.kind == "workflow-progress", OR a _meta.kiro.messageId /
    notifyId beginning "wf-progress-". The flat _meta.kiro.kind form is kept as a
    defensive third path.
    """
    if str((meta.get("notification") or {}).get("kind")) == "workflow-progress":
        return True
    for key in ("messageId", "notifyId"):
        if str(meta.get(key) or "").startswith("wf-progress-"):
            return True
    return (str(meta.get("kind")) == "workflow-progress"
            or str(upd.get("kind")) == "workflow-progress")


def on_notify(o):
    m = o.get("method"); p = o.get("params", {}) or {}
    inbound[m] = inbound.get(m, 0) + 1
    rec("A->C", o)
    if o.get("id") is not None:  # agent->client REQUEST
        if m == "_kiro/auth/getAccessToken":
            auth_calls[0] += 1; rep(o["id"], dict(TOKEN_PAYLOAD))
        elif m == "session/request_permission":
            opts = (p.get("options") or [])
            allow = next((x for x in opts if "allow" in json.dumps(x).lower()),
                         opts[0] if opts else None)
            oid = allow.get("optionId") if isinstance(allow, dict) else None
            rep(o["id"], {"outcome": {"outcome": "selected", "optionId": oid}})
        elif m == "_kiro/userInput":
            userinput_calls[0] += 1
            rep(o["id"], {"action": "answered", "answer": "Yes, proceed with the defaults."})
        else:
            rep(o["id"], {})
        return
    if m == "session/update":
        upd = p.get("update") or {}
        kind = upd.get("sessionUpdate", "?")
        updates[kind] = updates.get(kind, 0) + 1
        meta = ((upd.get("_meta") or {}).get("kiro") or {})
        sid = meta.get("agentSubtaskId") or upd.get("agentSubtaskId")
        if sid: subtask_ids.add(sid)
        if is_workflow_progress(upd, meta):
            workflow_frames[0] += 1
        if "loop_iteration" in json.dumps(upd): loop_frames[0] += 1
        if kind in ("tool_call", "tool_call_update"):
            mk = meta.get("kind")
            if mk: meta_kinds[mk] = meta_kinds.get(mk, 0) + 1
            ri = upd.get("rawInput") or {}
            rik = sorted(ri.keys()) if isinstance(ri, dict) else None
            title = upd.get("title") or upd.get("toolCallId")
            tool_rows.append((kind, upd.get("kind"), title, mk, (sid or "")[:8], rik))
            name = (ri.get("name") if isinstance(ri, dict) else "") or (title or "")
            if isinstance(ri, dict) and ("stages" in ri or "orchestrate" in str(name).lower()
                                         or "task" in ri and "stages" in ri):
                orchestrate_inputs.append(ri)


def pump(until, to):
    end = time.time() + to
    while time.time() < end:
        try: raw = q.get(timeout=2)
        except queue.Empty: continue
        try: o = json.loads(raw)
        except Exception: continue
        if "method" in o: on_notify(o)
        if until is not None and o.get("id") == until and ("result" in o or "error" in o):
            rec("A->C", o); return o
    return None


def drain(quiet_for=5.0, cap=60.0):
    """Process frames still queued behind the prompt response.

    Notifications the agent emitted before finishing are often still in flight; ending
    the capture at the response drops them from the raw log and the histograms.
    """
    end = time.time() + cap
    last = time.time()
    n = 0
    while time.time() < end and time.time() - last < quiet_for:
        try:
            raw = q.get(timeout=1)
        except queue.Empty:
            continue
        try: o = json.loads(raw)
        except Exception: continue
        if "method" in o:
            on_notify(o); n += 1; last = time.time()
    return n


try:
    proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    q = queue.Queue()
    threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()],
                     daemon=True).start()

    init = pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 25)
    if init is None or "error" in init or "result" not in init:
        fail("initialize", init)
    print("[ok] initialize")

    sn = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 45)
    if sn is None or "error" in sn or not (sn.get("result") or {}).get("sessionId"):
        fail("session/new", sn)
    sid = sn["result"]["sessionId"]
    print(f"[ok] sessionId: {sid}")

    PROMPT = ("Use the OrchestrateSubAgent tool to run a multi-stage pipeline for this task. "
              "Stage 1 ('explore'): list what .txt files exist in the workspace. "
              "Stage 2 ('write-alpha', depends on explore): create alpha.txt containing only ALPHA. "
              "Stage 3 ('write-beta', depends on explore): create beta.txt containing only BETA. "
              "Stages 2 and 3 should run in parallel. Keep each stage minimal. "
              "Then summarize in one sentence what was created.")
    r = pump(req("session/prompt", {"sessionId": sid,
                                    "prompt": [{"type": "text", "text": PROMPT}]}), 480)
    if r is None or "error" in r or "result" not in r:
        fail("session/prompt", r)
    stop = r["result"].get("stopReason")

    trailing = drain()

    print(f"\n=== stopReason: {stop} === (drained {trailing} trailing frame(s))")
    print("auth getAccessToken calls:", auth_calls[0], "| userInput calls:", userinput_calls[0])
    print("INBOUND methods:", json.dumps(inbound))
    print("session/update kinds:", json.dumps(updates))
    print("tool_call _meta.kiro.kind histogram:", json.dumps(meta_kinds))
    print("distinct agentSubtaskId:", len(subtask_ids))
    print("workflow-progress frames:", workflow_frames[0], "| loop_iteration frames:", loop_frames[0])
    print("orchestrate/invoke rawInputs captured:", len(orchestrate_inputs))
    for ri in orchestrate_inputs[:3]:
        print("  rawInput keys:", sorted(ri.keys()),
              "| has stages:", "stages" in ri, "| has repeat:", "repeat" in ri)
        print("  rawInput:", json.dumps(ri)[:900])
    print("\n--- tool_call timeline (upd | acpKind | title | metaKind | subtask | rawInputKeys) ---")
    for t in tool_rows[:60]:
        print("  ", " | ".join(str(x) for x in t))
    print(f"\nfull raw wire log -> {OUT}")
    print(f"workspace (stage output, not auto-deleted) -> {CWD}")
finally:
    if proc is not None:
        try:
            if proc.stdin and not proc.stdin.closed:
                proc.stdin.close()
        except OSError:
            pass
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
    log.close()

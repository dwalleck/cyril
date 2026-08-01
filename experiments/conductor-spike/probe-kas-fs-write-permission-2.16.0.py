#!/usr/bin/env python3
"""Capture the WRITE half of the `_kiro/fs/*` dialect — the half no capture holds.

PR #73 (cyril-kf2g) implements five `_kiro/fs/*` responders. Three of them are
live-observed: `read_file`, `stat`, `read_directory` all appear in
kas-pushed-2.16.0.jsonl. The other two have NEVER been seen on the wire —
`_kiro/fs/write_file` and `_kiro/fs/delete` appear only inside probe scripts.
So two load-bearing claims rest on carved source alone:

  Q1 ORDERING. Does a `session/request_permission` precede `_kiro/fs/write_file`?
     The bare-ACP handler doc (client.rs) says KAS sends a separate permission
     request for `fs/write_text_file`; the carved JS says permission is raised at
     the TOOL-APPROVAL layer, not per-callback, so the dialect switch should not
     change the posture. If that reading is wrong, PR #73 silently moves agent
     writes from a gated path to an ungated one. Same question for `delete`,
     whose module doc asserts "KAS raises no session/request_permission" — also
     carved-only.

  Q2 RANGE SHAPE. `_meta.kiro.range` is modeled entirely from carved source.
     A partial edit must arrive RANGED; if cyril ignored the range it would turn
     every partial edit into a full-file overwrite, so the real payload matters.
     Offsets are UTF-16 code units (the agent measures them in a JS string).

Design: three turns, each provoking one shape, with permission frames tracked
per turn so ordering is evidence rather than inference.

  1. edit     -> a partial edit of a seeded file      => RANGED write
  2. create   -> a brand new file                     => UNRANGED write
  3. delete   -> remove a file                        => _kiro/fs/delete

The write responder implements the splice faithfully (UTF-16, `spliceRange`
semantics) rather than clobbering, so the agent's follow-up reads see the file
it thinks it wrote and the turn does not derail on our own infidelity.

Costs credits (three short turns).

    probe-kas-fs-write-permission-2.16.0.py <kiro-cli> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}

SEED = "alpha\nbravo\ncharlie\ndelta\necho\n"


def redact(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else redact(obj[k]))
                for k in obj}
    if isinstance(obj, list):
        return [redact(x) for x in obj]
    return obj


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key in "
                        "('kirocli:odic:token','kirocli:social:token') order by key desc").fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
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


# ---------------------------------------------------------------- UTF-16 splice
def u16len(s):
    return len(s.encode("utf-16-le")) // 2


def u16_to_byte(s, target):
    """Byte offset of the `target`-th UTF-16 code unit (clamped to len)."""
    n = 0
    for i, ch in enumerate(s):
        if n >= target:
            return i
        n += 2 if ord(ch) > 0xFFFF else 1
    return len(s)


def splice_range(content, rng, new_text):
    """Port of `spliceRange` — the same semantics cyril's kiro_fs.rs ports."""
    if not rng:
        return new_text
    lines = content.split("\n")
    start = rng.get("start") or {}
    end = rng.get("end")
    sl = start.get("line", 0)
    sc = start.get("character", 0)
    if sl >= len(lines):                      # out-of-range start appends past end
        return content + new_text
    head = lines[:sl]
    sline = lines[sl]
    prefix = sline[:u16_to_byte(sline, sc)]
    if end is None:                           # absent end == "to the last line"
        return "\n".join(head + [prefix + new_text])
    el = end.get("line", sl)
    ec = end.get("character", 0)
    if el >= len(lines):
        return "\n".join(head + [prefix + new_text])
    eline = lines[el]
    suffix = eline[u16_to_byte(eline, ec):]
    return "\n".join(head + [prefix + new_text + suffix] + lines[el + 1:])


TOK = read_token()
CWD = tempfile.mkdtemp(prefix="kas-fswrite-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
pathlib.Path(CWD, "notes.txt").write_text(SEED)
pathlib.Path(CWD, "scratch.txt").write_text("delete me\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)

TMPH = tempfile.mkdtemp(prefix="kas-fswrite-home-")
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
TURN = ["setup"]
EVENTS = []          # ordered (turn, kind, method, detail) — the ordering evidence


def ev(kind, method, detail=None):
    EVENTS.append({"turn": TURN[0], "kind": kind, "method": method, "detail": detail})


def emit(d, e, m, parsed):
    OUT.write(json.dumps({"turn": TURN[0], "direction": d, "envelope": e,
                          "method": m, "parsed": redact(parsed)}) + "\n")
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


def ap(x):
    return x if os.path.isabs(x or "") else os.path.join(CWD, x or "")


def answer(m, rid, pr):
    if m == "_kiro/auth/getAccessToken":
        return reply(rid, TOK)
    if m == "_kiro/terminal/shell_type":
        return reply(rid, {"shellType": "bash"})

    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try:
            txt = pathlib.Path(ap(pr.get("path"))).read_text()
        except Exception as e:
            return reply(rid, {"content": f"(err {e})"})
        # `line` is 0-BASED on this dialect and the slice rejoins with \n.
        line, limit = pr.get("line"), pr.get("limit")
        if line is not None or limit is not None:
            ls = txt.split("\n")
            s = line or 0
            txt = "\n".join(ls[s:s + limit] if limit else ls[s:])
        return reply(rid, {"content": txt})

    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        rng = ((pr.get("_meta") or {}).get("kiro") or {}).get("range")
        ev("WRITE", m, {"path": pr.get("path"), "ranged": rng is not None, "range": rng})
        try:
            f = pathlib.Path(ap(pr.get("path")))
            f.parent.mkdir(parents=True, exist_ok=True)
            if rng is None:
                f.write_text(pr.get("content", ""))
            else:
                existing = f.read_text() if f.exists() else ""
                f.write_text(splice_range(existing, rng, pr.get("content", "")))
        except Exception:
            pass
        return reply(rid, {})

    if m == "_kiro/fs/delete":
        ev("DELETE", m, {"path": pr.get("path"), "params": pr})
        try:
            f = pathlib.Path(ap(pr.get("path")))
            if f.is_dir() and not f.is_symlink():
                __import__("shutil").rmtree(f)
            else:
                f.unlink()
        except Exception:
            pass
        return reply(rid, {})

    if m == "_kiro/fs/stat":
        f = pathlib.Path(ap(pr.get("path")))
        if not f.exists():
            return reply(rid, {})
        return reply(rid, {"type": "directory" if f.is_dir() else "file",
                           "size": f.stat().st_size})
    if m == "_kiro/fs/read_directory":
        try:
            return reply(rid, {"entries": [
                {"name": e.name, "type": "directory" if e.is_dir() else "file"}
                for e in pathlib.Path(ap(pr.get("path"))).iterdir()]})
        except Exception:
            return reply(rid, {"entries": []})

    if m == "_kiro/hooks/list":
        return reply(rid, {"hooks": []})
    if m == "_kiro/hooks/sessionStart":
        return reply(rid, {"hooks": [], "results": []})

    if m == "terminal/create":
        cmd, args = pr.get("command", ""), pr.get("args") or []
        tid = f"term-{len(TERMS)+1}"
        try:
            r = (subprocess.run([cmd, *args], cwd=pr.get("cwd") or CWD, capture_output=True,
                                text=True, timeout=60) if args else
                 subprocess.run(cmd, shell=True, cwd=pr.get("cwd") or CWD,
                                capture_output=True, text=True, timeout=60))
            TERMS[tid] = {"out": r.stdout + r.stderr, "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 127}
        return reply(rid, {"terminalId": tid})
    if m == "terminal/output":
        t = TERMS.get(pr.get("terminalId"), {"out": "", "code": 0})
        return reply(rid, {"output": t["out"], "truncated": False,
                           "exitStatus": {"exitCode": t["code"], "signal": None}})
    if m == "terminal/wait_for_exit":
        t = TERMS.get(pr.get("terminalId"), {"code": 0})
        return reply(rid, {"exitCode": t["code"], "signal": None})
    if m in ("terminal/release", "terminal/kill"):
        return reply(rid, {})

    if m == "session/request_permission":
        tc = pr.get("toolCall") or {}
        ev("PERMISSION", m, {"title": tc.get("title"), "toolCallId": tc.get("toolCallId"),
                             "kind": tc.get("kind")})
        opts = pr.get("options", [])
        pick = next((x for x in opts if "allow" in
                     (x.get("kind", "") + x.get("optionId", "")).lower()),
                    opts[0] if opts else None)
        return reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                     if pick else {"outcome": {"outcome": "cancelled"}})
    return reply(rid, {})


def pump(until, to=120):
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
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        emit("agent_to_client",
             "notification" if (m and rid is None) else ("request" if m else "response"), m, o)
        if rid is not None and m:
            answer(m, rid, pr)
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {
        # The dialect gate: nested under `fs`, NOT top-level `_meta.kiro`.
        "fs": {"readTextFile": True, "writeTextFile": True,
               "_meta": {"kiro": {"readFile": True, "writeFile": True, "stat": True,
                                  "readDirectory": True, "delete": True}}},
        "terminal": True,
    },
    "clientInfo": {"name": "kiro-cli", "title": "Cyril", "version": "probe"},
})
pump(1, 40)
sid = (pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 90) or {}) \
    .get("result", {}).get("sessionId")
print("sessionId:", sid)
if not sid:
    print("!! no session — auth or bundle problem; aborting")
    OUT.close(); p.terminate(); sys.exit(1)
pump(-1, 8)

TURNS = [
    ("edit", "The file notes.txt in this directory has five lines. Change ONLY the "
             "third line, from 'charlie' to 'CHARLIE-EDITED'. Leave every other line "
             "exactly as it is. Edit the existing file in place."),
    ("create", "Create a new file called fresh.txt in this directory containing "
               "exactly the single line: hello-from-kas"),
    ("delete", "Delete the file scratch.txt from this directory."),
]
for tag, text in TURNS:
    TURN[0] = tag
    print(f"\n########## turn: {tag} ##########")
    r = pump(req("session/prompt", {"sessionId": sid,
                                    "prompt": [{"type": "text", "text": text}]}), 420)
    print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
    pump(-1, 10)

# ------------------------------------------------------------------- verdicts
print("\n=== ordered events per turn ===")
for e in EVENTS:
    print(f"  [{e['turn']:<7}] {e['kind']:<11} {e['method']}")
    if e["detail"]:
        print(f"                  {json.dumps(e['detail'])[:300]}")

writes = [e for e in EVENTS if e["kind"] == "WRITE"]
deletes = [e for e in EVENTS if e["kind"] == "DELETE"]
kiro_writes = [e for e in writes if e["method"] == "_kiro/fs/write_file"]
ranged = [e for e in kiro_writes if e["detail"]["ranged"]]


def gated(target):
    """Was a permission frame seen in the same turn BEFORE this event?"""
    idx = EVENTS.index(target)
    return any(e["kind"] == "PERMISSION" and e["turn"] == target["turn"]
               for e in EVENTS[:idx])


print("\n=== VERDICTS ===")
print(f"V0 dialect taken for writes : {'YES' if kiro_writes else 'NO'} "
      f"({len(kiro_writes)} _kiro/fs/write_file, "
      f"{len([e for e in writes if e['method'] == 'fs/write_text_file'])} bare-ACP)")
print(f"V1 ranged write observed    : {'YES' if ranged else 'NO'}")
for e in ranged:
    print(f"     range = {json.dumps(e['detail']['range'])}")
print(f"V2 writes preceded by perm  : "
      f"{sum(1 for e in kiro_writes if gated(e))}/{len(kiro_writes)}")
print(f"V3 delete observed          : {'YES' if deletes else 'NO'}; "
      f"preceded by perm: {sum(1 for e in deletes if gated(e))}/{len(deletes)}")
print(f"V4 total permission frames  : "
      f"{len([e for e in EVENTS if e['kind'] == 'PERMISSION'])}")

print("\n=== resulting files ===")
for name in ("notes.txt", "fresh.txt", "scratch.txt"):
    f = pathlib.Path(CWD, name)
    print(f"  {name}: {'MISSING' if not f.exists() else repr(f.read_text())}")
print(f"\n(workspace: {CWD})")

OUT.close()
p.stdin.close()
p.terminate()

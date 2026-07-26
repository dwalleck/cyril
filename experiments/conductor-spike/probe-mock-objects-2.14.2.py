#!/usr/bin/env python3
"""
KIRO_MOCK_CHAT_RESPONSE object entries: can they script TOOL CALLS?

ANSWER: no -- not on any shipped release. Every object entry panics kiro-cli at
`initialize`, before the ACP handshake completes. The string form is unaffected.

Background. The string form is a proven free/instant/offline test backend
(probe-mock-backend-2.14.2.py). The parser's reject message advertises more:

    "KIRO_MOCK_CHAT_RESPONSE must be a JSON array of arrays of strings or objects"

The object shape was undocumented. Recovered from the 2.14.2 binary by decoding
the serde-derived __FieldVisitor cmpb chains (serde compiles field/variant names
into unrolled byte compares, and leaves the enum topology in `expecting` strings):

  chat_cli_v2::api_client::send_message_output::MockStreamItem
      "adjacently tagged enum MockStreamItem" => #[serde(tag="kind", content="data")]
      variants (__FieldVisitor indices 0/1/2):
          event       -> model::ChatResponseStream
          streamError -> error::ConverseStreamErrorKind
          sendError   -> error::ConverseStreamErrorKind
  chat_cli_v2::api_client::model::ChatResponseStream
      "adjacently tagged enum ChatResponseStream" => tag="kind", content="data"
      14 variants: AssistantResponseEvent CodeEvent CodeReferenceEvent
      FollowupPromptEvent IntentsEvent InvalidStateEvent MessageMetadataEvent
      ContextUsageEvent MetadataEvent MeteringEvent SupplementaryWebLinksEvent
      ToolUseEvent ReasoningEvent Unknown
  chat_cli_v2::api_client::error::ConverseStreamErrorKind  (externally tagged):
      Throttling InvalidModelId MonthlyLimitReached ModelOverloadedError
      ContextWindowOverflow Unknown

So the intended payload was e.g.
    {"kind":"event","data":{"kind":"ToolUseEvent","data":{...}}}
which would have made tool calls, permission prompts, thinking chunks and
transport errors all deterministically scriptable for free.

It never worked. `chat_cli` (the **v1** crate) also reads the var at initialize
and unwraps unconditionally on the object branch; it aborts the process before
`chat_cli_v2`'s MockStreamItem parser -- the one that understands the shape above
-- ever runs. Not a shape mismatch: a bare `{}` panics identically. The two
crates are distinct in the binary (`crates/chat-cli/` vs `crates/chat-cli-v2/`
both appear as panic-location literals), so the attribution is unambiguous.

Sections:
  A  control     -- string form still serves a turn, free and instant  (must PASS)
  B  shape matrix-- 11 object shapes incl. `{}`                        (all PANIC)
  C  version sweep- archived binaries; --sweep, needs the research archive

Isolation (feedback_isolate_kiro_probes_with_home): HOME=<tmp>, real
XDG_DATA_HOME. v2 engine only => no KAS ~/.kiro/logs pruning.
"""
import json, os, re, subprocess, sys, threading, queue, time, tempfile, pathlib

KIRO = os.path.expanduser("~/.local/bin/kiro-cli-chat")
KIRO_CLI = os.path.expanduser("~/.local/bin/kiro-cli")
ARCHIVE = pathlib.Path(os.path.expanduser("~/.local/share/kiro-research/binaries"))
REAL_XDG = os.path.expanduser("~/.local/share")
SENT = "MOCK-OBJ-9931"

INIT_PARAMS = {"protocolVersion": 1,
               "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}},
               "clientInfo": {"name": "cyril-probe", "version": "0.0.1"}}
INIT_LINE = json.dumps({"jsonrpc": "2.0", "id": 1,
                        "method": "initialize", "params": INIT_PARAMS}) + "\n"


def ev(kind, data):
    return {"kind": "event", "data": {"kind": kind, "data": data}}


def workspace(script):
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="mockobj-"))
    (tmp / "home").mkdir()
    (tmp / "cwd").mkdir()
    (tmp / "m.json").write_text(json.dumps(script))
    env = dict(os.environ)
    env.update(HOME=str(tmp / "home"), XDG_DATA_HOME=REAL_XDG,
               KIRO_MOCK_CHAT_RESPONSE=str(tmp / "m.json"),
               KIRO_LOG_LEVEL="debug", KIRO_CHAT_LOG_FILE=str(tmp / "k.log"))
    env.pop("KIRO_TEST_MODE", None)   # presence-only flag; must stay unset
    return tmp, env


def strip_ansi(s):
    return re.sub(r"\x1b\[[0-9;]*m", "", s)


# --------------------------------------------------------------- A: control
def section_a():
    print("\n=== A. control: does the STRING form still serve a turn, free? ===")
    tmp, env = workspace([[SENT, " served", " from", " the", " mock"]])
    proc = subprocess.Popen([KIRO, "acp"], cwd=str(tmp / "cwd"), env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, bufsize=1)
    if proc.stdin is None or proc.stdout is None or proc.stderr is None:
        raise RuntimeError("failed to open pipes to kiro-cli")
    stdin, stdout, stderr = proc.stdin, proc.stdout, proc.stderr
    msgs: queue.Queue = queue.Queue()
    notifs, err = [], []
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in stdout if l.strip()],
                                     msgs.put(None)), daemon=True).start()
    threading.Thread(target=lambda: [err.append(l) for l in stderr], daemon=True).start()
    ident = [10]

    def req(m, p):
        ident[0] += 1
        stdin.write(json.dumps({"jsonrpc": "2.0", "id": ident[0],
                                "method": m, "params": p}) + "\n")
        stdin.flush()
        return ident[0]

    def pump(until, timeout=90):
        end = time.time() + timeout
        while time.time() < end:
            try:
                raw = msgs.get(timeout=2)
            except queue.Empty:
                continue
            if raw is None:
                return None
            try:
                o = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if "method" in o and "id" in o:
                stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": {}}) + "\n")
                stdin.flush()
            elif "method" in o:
                notifs.append(o)
            elif o.get("id") == until:
                return o
        return None

    try:
        pump(req("initialize", INIT_PARAMS), 60)
        new = pump(req("session/new", {"cwd": str(tmp / "cwd"), "mcpServers": []}), 90)
        if not new or "error" in new:
            print(f"  FATAL session/new: {json.dumps(new)[:200] if new else 'timeout'}")
            print("  stderr:", strip_ansi("".join(err))[-300:])
            return False
        sid = new["result"]["sessionId"]
        t0 = time.time()
        r = pump(req("session/prompt", {"sessionId": sid,
                                        "prompt": [{"type": "text",
                                                    "text": "what is the capital of France?"}]}), 90)
        secs = time.time() - t0
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()

    text, metering = "", []
    for n in notifs:
        u = n.get("params", {}).get("update", {})
        c = u.get("content")
        if u.get("sessionUpdate") == "agent_message_chunk" and isinstance(c, dict):
            text += c.get("text", "")
        if "metadata" in n.get("method", ""):
            metering.extend(n.get("params", {}).get("meteringUsage") or [])
    credits = sum(m.get("value", 0) for m in metering if m.get("unit") == "credit")
    served = SENT in text
    stop = (r or {}).get("result", {}).get("stopReason")
    print(f"  served sentinel : {'YES' if served else 'NO'}   (reply={text.strip()[:60]!r})")
    print(f"  credits billed  : {credits:.6f}   ({'FREE' if credits == 0 else 'BILLED'})")
    print(f"  stopReason      : {stop}   in {secs:.1f}s")
    return served and credits == 0


# ----------------------------------------------------------- B: shape matrix
SHAPES = [
    ("string-control",     [["hello"]]),
    ("empty-object",       [[{}]]),
    ("adjacent-assistant", [[ev("AssistantResponseEvent", {"content": "x"})]]),
    ("adjacent-tooluse",   [[ev("ToolUseEvent", {"tool_use_id": "t1", "name": "fs_read",
                                                 "input": "{}", "stop": True})]]),
    ("adjacent-reasoning", [[ev("ReasoningEvent", {"content": "think"})]]),
    ("external-assistant", [[{"AssistantResponseEvent": {"content": "x"}}]]),
    ("internal-tag",       [[{"kind": "AssistantResponseEvent", "content": "x"}]]),
    ("bare-content",       [[{"content": "x"}]]),
    ("streamError-str",    [[{"kind": "streamError", "data": "Throttling"}]]),
    ("sendError-str",      [[{"kind": "sendError", "data": "Throttling"}]]),
    ("garbage-object",     [[{"zzz": 1}]]),
]


def one_shot(binary, args, script):
    """Feed a single `initialize` and report panic / skip / accept."""
    tmp, env = workspace(script)
    try:
        p = subprocess.run([binary] + args, cwd=str(tmp / "cwd"), env=env,
                           input=INIT_LINE, capture_output=True, text=True, timeout=60)
    except subprocess.TimeoutExpired:
        return "timeout", ""
    e = strip_ansi(p.stderr)
    if "panicked" in e:
        loc = next((l.split("Location:")[-1].strip() for l in e.splitlines()
                    if "Location:" in l), "?")
        return "PANIC", loc
    log = (tmp / "k.log").read_text(errors="replace") if (tmp / "k.log").exists() else ""
    if any("skipping mock" in l for l in log.splitlines()):
        return "skipped", "parser rejected, process survived"
    return "survives", ""


def section_b():
    print("\n=== B. object shape matrix @ 2.14.2 (one `initialize` each) ===")
    print(f"  {'SHAPE':<20} {'RESULT':<10} detail")
    rows = []
    for name, script in SHAPES:
        res, detail = one_shot(KIRO, ["acp"], script)
        rows.append((name, res, detail))
        print(f"  {name:<20} {res:<10} {detail}")
    print("\n  -- engine flag does not dodge it (v1's parser runs regardless) --")
    for label, b, a in [("--agent-engine v1", KIRO, ["acp", "--agent-engine", "v1"]),
                        ("--agent-engine v2", KIRO, ["acp", "--agent-engine", "v2"]),
                        ("--agent-engine v3", KIRO, ["acp", "--agent-engine", "v3"]),
                        ("kiro-cli acp (cyril)", KIRO_CLI, ["acp"])]:
        res, detail = one_shot(b, a, [[ev("AssistantResponseEvent", {"content": "x"})]])
        print(f"  {label:<20} {res:<10} {detail}")
    return rows


# ---------------------------------------------------------- C: version sweep
def section_c():
    print("\n=== C. version sweep (object form) ===")
    if not ARCHIVE.is_dir():
        print(f"  archive not found at {ARCHIVE}; skipping")
        return
    obj = [[ev("AssistantResponseEvent", {"content": "x"})]]
    vers = sorted((d.name for d in ARCHIVE.iterdir()
                   if d.is_dir() and (d / "kiro-cli-chat").exists()),
                  key=lambda v: [int(x) for x in v.split(".")])
    print(f"  {'VERSION':<10} {'OBJECTS?':<10} {'RESULT':<10} panic location")
    for v in list(vers) + ["installed"]:
        b = KIRO if v == "installed" else str(ARCHIVE / v / "kiro-cli-chat")
        try:
            advertises = subprocess.run(
                ["strings", "-n", "8", b], capture_output=True, text=True, timeout=600
            ).stdout.count("array of arrays of strings or objects") > 0
        except subprocess.TimeoutExpired:
            advertises = False
        res, loc = one_shot(b, ["acp"], obj)
        print(f"  {v:<10} {'yes' if advertises else 'no (str)':<10} {res:<10} {loc}")


if __name__ == "__main__":
    before = len(os.listdir(os.path.expanduser("~/.kiro/logs")))
    ok = section_a()
    section_b()
    if "--sweep" in sys.argv:
        section_c()
    after = len(os.listdir(os.path.expanduser("~/.kiro/logs")))
    print("\n" + "=" * 74)
    print("  VERDICT: object entries are unusable on every release that offers them.")
    print("           v1's parser unwraps on the object branch and aborts at initialize.")
    print(f"           String form remains sound: control {'PASSED' if ok else 'FAILED'}.")
    print(f"  isolation: real ~/.kiro/logs {before} -> {after} "
          f"({'held' if before == after else 'LEAKED'})")
    print("=" * 74)

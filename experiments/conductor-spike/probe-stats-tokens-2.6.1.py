#!/usr/bin/env python3
"""Probe whether /stats input_tokens/output_tokens populate on the 2.6.x wire.

Sends model/effort via commands/execute as REQUESTS (cyril's real ExtRequest path),
runs one turn, then issues /stats as a request and prints the per-request token fields.

  KIRO_BIN=~/.local/bin/kiro-cli-chat PROBE_MODEL=claude-opus-4.8 PROBE_EFFORT=high \
      PROBE_PROMPT='Think step by step: ...' python3 probe-stats-tokens-2.6.1.py
  KIRO_BIN=... PROBE_MODEL=claude-haiku-4.5 \
      PROBE_PROMPT='Run the shell command `echo hello` using your shell tool.' python3 ...
"""
import json, os, subprocess, threading, time, sys

KIRO = os.path.expanduser(os.environ.get("KIRO_BIN", "~/.local/bin/kiro-cli-chat"))
CWD = os.getcwd()
TAG = os.environ.get("PROBE_TAG", "stats")
LOG_PATH = f"/tmp/cyril-probe-stats-{TAG}.log"
MODEL = os.environ.get("PROBE_MODEL", "claude-haiku-4.5")
EFFORT = os.environ.get("PROBE_EFFORT")  # optional
PROMPT = os.environ.get("PROBE_PROMPT", "Say hello in one word.")


def main() -> int:
    log_file = open(LOG_PATH, "w")
    print(f"[setup] binary={KIRO}  model={MODEL}  effort={EFFORT}  log={LOG_PATH}")
    proc = subprocess.Popen([KIRO, "acp"], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=CWD)
    assert proc.stdin is not None and proc.stdout is not None
    incoming: list[dict] = []
    lock = threading.Lock()

    def reader():
        while True:
            line = proc.stdout.readline()
            if not line:
                return
            text = line.decode("utf-8", errors="replace").rstrip("\n")
            log_file.write(f"S->C {text}\n"); log_file.flush()
            try:
                with lock:
                    incoming.append(json.loads(text))
            except json.JSONDecodeError:
                pass

    threading.Thread(target=reader, daemon=True).start()
    next_id = [1]

    def send(method, params, want_response=True):
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        if want_response:
            msg["id"] = next_id[0]; next_id[0] += 1
        log_file.write(f"C->S {json.dumps(msg)}\n"); log_file.flush()
        proc.stdin.write((json.dumps(msg) + "\n").encode()); proc.stdin.flush()
        return msg.get("id")

    def send_response(req_id, result):
        msg = {"jsonrpc": "2.0", "id": req_id, "result": result}
        proc.stdin.write((json.dumps(msg) + "\n").encode()); proc.stdin.flush()
        log_file.write(f"C->S {json.dumps(msg)}\n"); log_file.flush()

    def auto_approve():
        seen = set()
        while True:
            with lock:
                frames = list(incoming)
            for f in frames:
                if f.get("method") == "session/request_permission" and f.get("id") not in seen:
                    seen.add(f["id"])
                    opts = f.get("params", {}).get("options", [])
                    allow = next((o for o in opts if o.get("kind") == "allow_once"), opts[0] if opts else None)
                    if allow:
                        send_response(f["id"], {"outcome": {"outcome": "selected", "optionId": allow["optionId"]}})
                        print(f"    [auto-approved tool: {allow.get('optionId')}]")
            time.sleep(0.1)

    threading.Thread(target=auto_approve, daemon=True).start()

    def wait_for(req_id, timeout=300.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            with lock:
                for f in incoming:
                    if f.get("id") == req_id and ("result" in f or "error" in f):
                        return f
            time.sleep(0.1)
        return None

    rid = send("initialize", {"protocolVersion": 1,
        "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}, "terminal": False},
        "clientInfo": {"name": "cyril", "version": "0.2.0"}})
    if not wait_for(rid, 15):
        print("[ERROR] initialize failed"); proc.terminate(); return 1

    rid = send("session/new", {"cwd": CWD, "mcpServers": []})
    r = wait_for(rid, 30)
    if not r or "error" in r:
        print(f"[ERROR] session/new failed: {r}"); proc.terminate(); return 1
    session_id = r["result"]["sessionId"]
    print(f"[ok] session={session_id}")

    # model as REQUEST (await the "Model changed to ..." confirmation).
    # NOTE: commands/execute MUST carry sessionId — as a request without it the binary exits.
    rid = send("_kiro.dev/commands/execute",
               {"command": {"command": "model", "args": {"value": MODEL}}, "sessionId": session_id})
    mr = wait_for(rid, 30)
    if not mr or "error" in mr or not mr.get("result", {}).get("success"):
        print(f"[ERROR] model switch to {MODEL} failed: {mr} — aborting "
              f"(token rows below would be mis-attributed to the wrong model)")
        proc.terminate(); return 1
    got = mr["result"].get("data", {}).get("model", {}).get("id")
    print(f"[model] changed to {got!r}: {mr['result'].get('message')}")
    if got != MODEL:
        print(f"[WARN] active model is {got!r}, expected {MODEL!r} — token rows may mis-attribute")

    if EFFORT:
        rid = send("_kiro.dev/commands/execute",
                   {"command": {"command": "effort", "args": {"value": EFFORT}}, "sessionId": session_id})
        er = wait_for(rid, 30)
        print(f"[effort] {json.dumps(er.get('result', er.get('error')))[:160] if er else 'NO RESPONSE'}")

    # the turn
    print(f"[prompt] {PROMPT[:70]}...")
    rid = send("session/prompt", {"sessionId": session_id, "prompt": [{"type": "text", "text": PROMPT}]})
    pr = wait_for(rid, 300)
    if not pr or "error" in pr:
        print(f"[WARN] turn did not complete cleanly: {json.dumps(pr.get('error') if pr else None)[:160]}\n"
              f"       => any 'tokens null' reading below is INCONCLUSIVE (no usage accrued), not a confirmed finding")
    stop = pr.get("result", {}).get("stopReason") if pr else None
    # did the turn use tools / think?
    with lock:
        frames = list(incoming)
    variants = {}
    for f in frames:
        if f.get("method") == "session/update":
            u = f.get("params", {}).get("update", {}).get("sessionUpdate")
            variants[u] = variants.get(u, 0) + 1
    print(f"[turn] stop={stop}  updates={variants}")

    # /stats as REQUEST — print token fields
    rid = send("_kiro.dev/commands/execute",
               {"command": {"command": "stats", "args": {}}, "sessionId": session_id})
    sr = wait_for(rid, 30)
    print("\n========== /stats RESULT ==========")
    if not sr:
        print("NO /stats RESPONSE"); proc.terminate(); return 1
    result = sr.get("result", sr.get("error"))
    # locate stats[] wherever it lives in the result envelope
    blob = json.dumps(result)
    print(f"raw (first 900B): {blob[:900]}")
    # extract token fields explicitly
    def walk(o):
        if isinstance(o, dict):
            if "input_tokens" in o or "output_tokens" in o:
                yield o
            for v in o.values():
                yield from walk(v)
        elif isinstance(o, list):
            for v in o:
                yield from walk(v)
    rows = list(walk(result))
    print(f"\nstat rows with token fields: {len(rows)}")
    for row in rows:
        print(f"  input_tokens={row.get('input_tokens')!r}  output_tokens={row.get('output_tokens')!r} "
              f"had_tool_use={row.get('had_tool_use')!r}  request_id={str(row.get('request_id'))[:8]}")
    proc.terminate()
    return 0


if __name__ == "__main__":
    sys.exit(main())

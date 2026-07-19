#!/usr/bin/env python3
"""cyril-0wyn Probe B: live A/B of KAS clientInfo.name recognition (2.13.0).

Spawns the extracted KAS acp-server.js directly (standalone stdio; no
ACP-level auth exchange is needed before the initialize response — note the
process may still find ambient token material under $HOME), sends one
initialize per candidate name, and captures the arm's own log file.

Claim under test (from static carve, .cyril-0wyn/oracle-resolveAgentContext.txt):
  - name in {kiro-web, kiro-ide, kiro-cli} -> accepted silently
  - any other name -> logs "Unrecognized clientInfo.name: '<x>', falling back
    to inferred client type" and resolves to kiro-ide (local env)
Oracle = that carved source. Probe = the running server's observable output.

Review hardening (2026-07-19): select-based reads so the deadline actually
fires on a silent server; log capture bound to the exact logDir returned in
THAT arm's initialize response (no cross-process contamination); an arm only
passes with a successful initialize result AND its handler's own
"Stored clientInfo.name" line present.
"""
import glob, json, select, subprocess, sys, time
from pathlib import Path

SERVER = glob.glob(str(Path.home() / ".local/share/kiro-cli/kas/2.13.0-*"
                       "/node_modules/@kiro/agent/dist/server/acp-server.js"))[0]
OUTDIR = Path(__file__).parent / "probe-b-results"
OUTDIR.mkdir(exist_ok=True)

NAMES = ["cyril", "kiro-cli", "kiro-ide"]

def read_response(p, want_id: int, deadline_s: float):
    """Line-read stdout with a real deadline (select before every read)."""
    buf = ""
    deadline = time.time() + deadline_s
    while time.time() < deadline:
        ready, _, _ = select.select([p.stdout], [], [], 0.25)
        if not ready:
            continue
        chunk = p.stdout.readline()
        if not chunk:
            time.sleep(0.05)
            continue
        buf += chunk
        try:
            msg = json.loads(chunk)
        except json.JSONDecodeError:
            continue
        if msg.get("id") == want_id:
            return msg
    return None

def find_log_dir(obj):
    """Walk the response for a 'logDir' value (initialize exposes it under _meta)."""
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k == "logDir" and isinstance(v, str):
                return v
            found = find_log_dir(v)
            if found:
                return found
    elif isinstance(obj, list):
        for v in obj:
            found = find_log_dir(v)
            if found:
                return found
    return None

def run_one(name: str) -> dict:
    p = subprocess.Popen(
        ["node", "--experimental-wasm-modules", SERVER],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, bufsize=1, start_new_session=True,
    )
    assert p.stdin is not None and p.stdout is not None
    req = {"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
        "protocolVersion": 1,
        "clientCapabilities": {"fs": {"readTextFile": False,
                                      "writeTextFile": False},
                               "terminal": False},
        "clientInfo": {"name": name, "version": "0.0.0-probe"},
    }}
    try:
        p.stdin.write(json.dumps(req) + "\n")
        p.stdin.flush()
    except BrokenPipeError:
        pass
    response = read_response(p, 0, 12.0)
    ok_response = bool(response and "result" in response)
    log_dir = find_log_dir(response) if response else None
    # The KAS logger writes its file through an async transport; killing
    # immediately after the stdout response loses the initialize-handler
    # lines (observed: log ends at "Platform initialized").
    time.sleep(3.0)
    p.terminate()
    try:
        _, stderr = p.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        p.kill()
        _, stderr = p.communicate()
    # Log capture is bound to THIS arm's own logDir from its initialize
    # response — a glob over ~/.kiro/logs could pick up a concurrent Kiro
    # process's lines (review finding 8).
    logtext = ""
    if log_dir and (Path(log_dir) / "kiro.log").exists():
        logtext = (Path(log_dir) / "kiro.log").read_text(errors="replace")
    haystack = stderr + "\n" + logtext
    unrecognized = [l for l in haystack.splitlines() if "Unrecognized clientInfo.name" in l]
    stored = [l for l in haystack.splitlines() if "Stored clientInfo.name" in l]
    (OUTDIR / f"stderr-{name}.log").write_text(stderr)
    (OUTDIR / f"kiro-log-{name}.log").write_text(logtext)
    (OUTDIR / f"init-response-{name}.json").write_text(
        json.dumps(response, indent=2) if response else "NO RESPONSE")
    return {"name": name, "ok_response": ok_response, "log_dir": log_dir,
            "unrecognized_warn": unrecognized, "stored_line": stored}

def main() -> int:
    results = [run_one(n) for n in NAMES]
    verdicts = []
    for r in results:
        expect_warn = r["name"] not in ("kiro-web", "kiro-ide", "kiro-cli")
        # An arm passes only when the handshake demonstrably ran: successful
        # initialize result + the handler's own Stored line + the expected
        # warn presence/absence (review finding 9 — no vacuous passes).
        ok = (r["ok_response"] and bool(r["stored_line"])
              and bool(r["unrecognized_warn"]) == expect_warn)
        verdicts.append(ok)
        print(f"{r['name']:>10}: response={r['ok_response']} "
              f"stored={'yes' if r['stored_line'] else 'NO'} "
              f"logDir={'bound' if r['log_dir'] else 'MISSING'} "
              f"warn={'YES' if r['unrecognized_warn'] else 'no'} "
              f"(expected {'YES' if expect_warn else 'no'}) "
              f"{'PASS' if ok else 'FAIL'}")
        for l in r["unrecognized_warn"] + r["stored_line"]:
            print(f"            | {l.strip()[:140]}")
    print("VERDICT:", "ALL-PASS" if all(verdicts) else "MISMATCH")
    return 0 if all(verdicts) else 1

if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""cyril-0wyn Probe B: live A/B of KAS clientInfo.name recognition (2.13.0).

Spawns the extracted KAS acp-server.js directly (standalone stdio, no auth
needed for initialize), sends one initialize per candidate name, and captures
stderr + the initialize response.

Claim under test (from static carve, .cyril-0wyn/oracle-resolveAgentContext.txt):
  - name in {kiro-web, kiro-ide, kiro-cli} -> accepted silently
  - any other name -> logs "Unrecognized clientInfo.name: '<x>', falling back
    to inferred client type" and resolves to kiro-ide (local env)
Oracle = that carved source. Probe = the running server's observable output.
"""
import glob, json, subprocess, sys, time
from pathlib import Path

SERVER = glob.glob(str(Path.home() / ".local/share/kiro-cli/kas/2.13.0-*"
                       "/node_modules/@kiro/agent/dist/server/acp-server.js"))[0]
OUTDIR = Path(__file__).parent / "probe-b-results"
OUTDIR.mkdir(exist_ok=True)

NAMES = ["cyril", "kiro-cli", "kiro-ide"]

LOGROOT = Path.home() / ".kiro/logs"

def run_one(name: str) -> dict:
    dirs_before = set(LOGROOT.glob("*")) if LOGROOT.exists() else set()
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
    deadline = time.time() + 12
    response = None
    while time.time() < deadline and response is None:
        line = p.stdout.readline()
        if not line:
            time.sleep(0.1)
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("id") == 0:
            response = msg
    # The KAS logger writes ~/.kiro/logs/<ts>/kiro.log through an async
    # transport; killing immediately after the stdout response loses the
    # initialize-handler lines (observed: log ends at "Platform initialized").
    time.sleep(3.0)
    p.terminate()
    try:
        _, stderr = p.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        p.kill()
        _, stderr = p.communicate()
    new_dirs = (set(LOGROOT.glob("*")) - dirs_before) if LOGROOT.exists() else set()
    logtext = "\n".join((d / "kiro.log").read_text(errors="replace")
                        for d in sorted(new_dirs) if (d / "kiro.log").exists())
    haystack = stderr + "\n" + logtext
    unrecognized = [l for l in haystack.splitlines() if "Unrecognized clientInfo.name" in l]
    stored = [l for l in haystack.splitlines()
              if "Stored clientInfo.name" in l or "remote-tools-discovery.create" in l]
    (OUTDIR / f"stderr-{name}.log").write_text(stderr)
    (OUTDIR / f"kiro-log-{name}.log").write_text(logtext)
    (OUTDIR / f"init-response-{name}.json").write_text(
        json.dumps(response, indent=2) if response else "NO RESPONSE")
    return {"name": name, "got_response": response is not None,
            "unrecognized_warn": unrecognized, "stored_line": stored}

def main() -> int:
    results = [run_one(n) for n in NAMES]
    verdicts = []
    for r in results:
        expect_warn = r["name"] not in ("kiro-web", "kiro-ide", "kiro-cli")
        ok = bool(r["unrecognized_warn"]) == expect_warn
        verdicts.append(ok)
        print(f"{r['name']:>10}: response={r['got_response']} "
              f"warn={'YES' if r['unrecognized_warn'] else 'no'} "
              f"(expected {'YES' if expect_warn else 'no'}) "
              f"{'PASS' if ok else 'FAIL'}")
        for l in r["unrecognized_warn"] + r["stored_line"]:
            print(f"            | {l.strip()[:140]}")
    print("VERDICT:", "ALL-PASS" if all(verdicts) else "MISMATCH")
    return 0 if all(verdicts) else 1

if __name__ == "__main__":
    sys.exit(main())

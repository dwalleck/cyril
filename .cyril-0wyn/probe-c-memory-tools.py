#!/usr/bin/env python3
"""cyril-0wyn Probe C (claim 8): does name=kiro-cli + memoryEnable surface
search_memories in KAS's resolved remote-tool allowlist?

Treatment: clientInfo.name="kiro-cli" + _meta.kiro.settings
           {"memoryEnable": {"enabled": true}}   <- OBJECT shape (review
           finding 10: bare `true` never arms the gate; the cap-injection
           table and cyril's own BOOL_MAP marshal both use {enabled:<bool>})
Control:   clientInfo.name="cyril" + the SAME settings
Expected (oracle = carved resolveRemoteToolAllowlist):
  treatment allowlist includes search_memories; control (kiro-ide fallback,
  stable channel) does not. PASS requires BOTH arms to produce a parsed
  `[RemoteToolsDiscovery] Allowlist resolved` payload (review finding 14 —
  a treatment hit plus a crashed control is INCONCLUSIVE, not PASS), and
  only that structured payload counts as evidence (finding 13 — incidental
  mentions of the tool id in settings/catalog/error lines are not).
Discovery needs an auth-serviceable session; without one the arms die at
TokenExpired before resolution and the verdict is INCONCLUSIVE.
"""
import glob, json, os, re, select, subprocess, sys, time
from pathlib import Path

SERVER = glob.glob(str(Path.home() / ".local/share/kiro-cli/kas/2.13.0-*"
                       "/node_modules/@kiro/agent/dist/server/acp-server.js"))[0]
OUTDIR = Path(__file__).parent / "probe-c-results"
OUTDIR.mkdir(exist_ok=True)

def rpc(i, method, params):
    return json.dumps({"jsonrpc": "2.0", "id": i, "method": method,
                       "params": params}) + "\n"

def read_response(p, want_id: int, deadline_s: float):
    deadline = time.time() + deadline_s
    while time.time() < deadline:
        ready, _, _ = select.select([p.stdout], [], [], 0.25)
        if not ready:
            continue
        chunk = p.stdout.readline()
        if not chunk:
            time.sleep(0.05)
            continue
        try:
            msg = json.loads(chunk)
        except json.JSONDecodeError:
            continue
        if msg.get("id") == want_id:
            return msg
    return None

def find_log_dir(obj):
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

def parse_allowlists(logtext: str):
    """Only the structured Allowlist-resolved payload is evidence."""
    out = []
    for line in logtext.splitlines():
        if "Allowlist resolved" not in line:
            continue
        m = re.search(r'"allowlist":\s*(\[[^\]]*\]|"\*")', line)
        if m:
            try:
                out.append(json.loads(m.group(1)))
            except json.JSONDecodeError:
                continue
    return out

def run_one(tag: str, name: str) -> dict:
    env = dict(os.environ, LOG_LEVEL="debug", KIRO_LOG_LEVEL="debug")
    p = subprocess.Popen(["node", "--experimental-wasm-modules", SERVER],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, text=True, bufsize=1,
                         start_new_session=True, env=env)
    assert p.stdin is not None and p.stdout is not None
    p.stdin.write(rpc(0, "initialize", {
        "protocolVersion": 1,
        "clientCapabilities": {
            "fs": {"readTextFile": False, "writeTextFile": False},
            "terminal": False,
            "_meta": {"kiro": {"settings": {"memoryEnable": {"enabled": True}}}},
        },
        "clientInfo": {"name": name, "version": "0.0.0-probe"},
    }))
    p.stdin.flush()
    response = read_response(p, 0, 12.0)
    init_ok = bool(response and "result" in response)
    log_dir = find_log_dir(response) if response else None
    if init_ok:
        # Only a successfully initialized arm proceeds (review finding 11).
        p.stdin.write(rpc(1, "session/new", {"cwd": str(OUTDIR), "mcpServers": []}))
        p.stdin.flush()
        time.sleep(6)  # give discovery + the async log transport time
    p.terminate()
    try:
        _, stderr = p.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        p.kill(); _, stderr = p.communicate()
    logtext = ""
    if log_dir and (Path(log_dir) / "kiro.log").exists():
        logtext = (Path(log_dir) / "kiro.log").read_text(errors="replace")
    (OUTDIR / f"kiro-log-{tag}.log").write_text(logtext)
    (OUTDIR / f"stderr-{tag}.log").write_text(stderr)
    allowlists = parse_allowlists(logtext)
    return {"tag": tag, "init_ok": init_ok, "log_dir_bound": bool(log_dir),
            "allowlists": allowlists,
            "discovery_lines": [l for l in logtext.splitlines()
                                if "RemoteToolsDiscovery" in l][:8]}

def has_memories(allowlists) -> bool:
    for a in allowlists:
        if a == "*" or (isinstance(a, list) and any("memor" in str(t).lower() for t in a)):
            return True
    return False

def main() -> int:
    treatment = run_one("treatment-kiro-cli", "kiro-cli")
    control = run_one("control-cyril", "cyril")
    for r in (treatment, control):
        print(f"== {r['tag']}: init_ok={r['init_ok']} logDir={'bound' if r['log_dir_bound'] else 'MISSING'} "
              f"allowlists={r['allowlists'] or 'NONE-RESOLVED'}")
        for l in r["discovery_lines"]:
            print(f"   | {l.strip()[:150]}")
    if not (treatment["init_ok"] and control["init_ok"]):
        print("VERDICT: INCONCLUSIVE — an arm failed initialize")
        return 2
    if not (treatment["allowlists"] and control["allowlists"]):
        print("VERDICT: INCONCLUSIVE — allowlist not resolved in BOTH arms "
              "(discovery needs an auth-serviceable session); jrl1 keeps the residue")
        return 2
    t_has, c_has = has_memories(treatment["allowlists"]), has_memories(control["allowlists"])
    if t_has and not c_has:
        print("VERDICT: PASS — search_memories on treatment only (matches carve)")
        return 0
    print(f"VERDICT: MISMATCH — treatment={t_has} control={c_has}")
    return 1

if __name__ == "__main__":
    sys.exit(main())

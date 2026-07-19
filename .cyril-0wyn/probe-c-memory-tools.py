#!/usr/bin/env python3
"""cyril-0wyn Probe C (claim 8, timeboxed): does name=kiro-cli + memoryEnable
surface search_memories in KAS's resolved remote-tool allowlist?

Treatment: clientInfo.name="kiro-cli" + _meta.kiro.settings {memoryEnable:true}
Control:   clientInfo.name="cyril"    + the SAME settings
Expected (oracle = carved resolveRemoteToolAllowlist):
  treatment allowlist includes search_memories; control (kiro-ide fallback,
  stable channel) does not. The control arm exists so a pass can't be
  explained by the settings flag alone unlocking the tool for everyone.
Observable: `[RemoteToolsDiscovery] Allowlist resolved` (debug) in the fresh
~/.kiro/logs/<ts>/kiro.log; discovery may require auth/network — if the line
never appears, the verdict is INCONCLUSIVE (recorded, jrl1 narrows).
"""
import glob, json, os, subprocess, sys, time
from pathlib import Path

SERVER = glob.glob(str(Path.home() / ".local/share/kiro-cli/kas/2.13.0-*"
                       "/node_modules/@kiro/agent/dist/server/acp-server.js"))[0]
OUTDIR = Path(__file__).parent / "probe-c-results"
OUTDIR.mkdir(exist_ok=True)
LOGROOT = Path.home() / ".kiro/logs"

def rpc(i, method, params):
    return json.dumps({"jsonrpc": "2.0", "id": i, "method": method,
                       "params": params}) + "\n"

def run_one(tag: str, name: str) -> dict:
    dirs_before = set(LOGROOT.glob("*")) if LOGROOT.exists() else set()
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
            "_meta": {"kiro": {"settings": {"memoryEnable": True}}},
        },
        "clientInfo": {"name": name, "version": "0.0.0-probe"},
    }))
    p.stdin.flush()
    deadline = time.time() + 10
    while time.time() < deadline:
        line = p.stdout.readline()
        if not line:
            time.sleep(0.1); continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("id") == 0:
            break
    p.stdin.write(rpc(1, "session/new", {"cwd": str(OUTDIR), "mcpServers": []}))
    p.stdin.flush()
    time.sleep(6)  # give discovery + the async log transport time
    p.terminate()
    try:
        _, stderr = p.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        p.kill(); _, stderr = p.communicate()
    new_dirs = (set(LOGROOT.glob("*")) - dirs_before) if LOGROOT.exists() else set()
    logtext = "\n".join((d / "kiro.log").read_text(errors="replace")
                        for d in sorted(new_dirs) if (d / "kiro.log").exists())
    (OUTDIR / f"kiro-log-{tag}.log").write_text(logtext)
    (OUTDIR / f"stderr-{tag}.log").write_text(stderr)
    hay = logtext + "\n" + stderr
    return {
        "tag": tag,
        "allowlist_lines": [l for l in hay.splitlines() if "Allowlist resolved" in l],
        "memories_hits": [l for l in hay.splitlines()
                          if "search_memories" in l or "searchMemories" in l],
        "discovery_lines": [l for l in hay.splitlines() if "RemoteToolsDiscovery" in l
                            or "remote-tools-discovery" in l],
    }

def main() -> int:
    treatment = run_one("treatment-kiro-cli", "kiro-cli")
    control = run_one("control-cyril", "cyril")
    for r in (treatment, control):
        print(f"== {r['tag']}")
        for k in ("discovery_lines", "allowlist_lines", "memories_hits"):
            for l in r[k][:6]:
                print(f"   {k[:9]} | {l.strip()[:150]}")
    t_has = bool(treatment["memories_hits"])
    c_has = bool(control["memories_hits"])
    resolved_seen = bool(treatment["allowlist_lines"] or control["allowlist_lines"])
    if not resolved_seen and not (t_has or c_has):
        print("VERDICT: INCONCLUSIVE — allowlist resolution never observed "
              "(discovery likely needs auth/network); jrl1 keeps the residue")
        return 2
    if t_has and not c_has:
        print("VERDICT: PASS — search_memories on treatment only (matches carve)")
        return 0
    print(f"VERDICT: MISMATCH — treatment={t_has} control={c_has}")
    return 1

if __name__ == "__main__":
    sys.exit(main())

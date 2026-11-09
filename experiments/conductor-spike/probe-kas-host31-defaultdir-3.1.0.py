#!/usr/bin/env python3
"""probe-kas-host31-defaultdir-3.1.0.py — where does a 3.1.0 `kiro-cli acp --agent-engine v3` put the runtime when KIRO_RUNTIME_DIR is UNSET?
Zero-auth: fresh empty HOME/XDG dirs, `--auth-method client` so the host forwards _kiro/auth/getAccessToken to us and we answer with an error.
The archived binary is COPIED into <scratch>/bin/kiro-cli so `<bin>/../lib/kiro-cli` starts empty; afterwards we list <scratch>/lib and
<XDG_DATA_HOME>/kiro-cli and record the node child's /proc/<pid>/exe. Usage: python3 -I probe-kas-host31-defaultdir-3.1.0.py <scratch> <out.jsonl>
"""
import json, os, shutil, subprocess, sys, threading, time, glob
SCR = os.path.abspath(sys.argv[1]); OUT = open(sys.argv[2], "w", buffering=1)
SRC = os.path.expanduser("~/.local/share/kiro-research/binaries/3.1.0/extracted/kirocli/bin/kiro-cli")
os.makedirs(f"{SCR}/bin", exist_ok=True); shutil.copy2(SRC, f"{SCR}/bin/kiro-cli")
for d in ("home", "xdg-data", "xdg-config", "ws"): os.makedirs(f"{SCR}/{d}", exist_ok=True)
env = {k: v for k, v in os.environ.items() if not k.startswith("KIRO_") and k not in ("XDG_DATA_HOME", "XDG_CONFIG_HOME", "HOME")}
env.update(HOME=f"{SCR}/home", XDG_DATA_HOME=f"{SCR}/xdg-data", XDG_CONFIG_HOME=f"{SCR}/xdg-config", PATH=f"{SCR}/bin:" + env.get("PATH", ""))
assert "KIRO_RUNTIME_DIR" not in env
def emit(**r): r["ts"] = time.time(); OUT.write(json.dumps(r) + "\n")
p = subprocess.Popen([f"{SCR}/bin/kiro-cli", "acp", "--agent-engine", "v3", "--auth-method", "client"], cwd=f"{SCR}/ws", env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
emit(event="spawn", argv=p.args, pid=p.pid, env={k: env[k] for k in ("HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME")}, KIRO_RUNTIME_DIR=None)
def rd():
    for line in p.stdout:
        line = line.strip()
        if not line: continue
        try: msg = json.loads(line)
        except Exception: emit(dir="a2c-raw", raw=line[:300]); continue
        emit(dir="a2c", msg=msg if len(line) < 4000 else {"method": msg.get("method"), "id": msg.get("id"), "truncated": True, "keys": list(msg.keys())})
        if msg.get("method") == "_kiro/auth/getAccessToken" and "id" in msg:
            reply = {"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32000, "message": "probe: no token (zero-auth arm)"}}
            p.stdin.write(json.dumps(reply) + "\n"); p.stdin.flush(); emit(dir="c2a", msg=reply)
def rde():
    for line in p.stderr: emit(dir="stderr", line=line.rstrip()[:300])
threading.Thread(target=rd, daemon=True).start(); threading.Thread(target=rde, daemon=True).start()
init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True,
        "_meta": {"kiro": {"configurationState": True}}}, "clientInfo": {"name": "cyril", "version": "0.0.0-probe"}}}
p.stdin.write(json.dumps(init) + "\n"); p.stdin.flush(); emit(dir="c2a", msg=init)
deadline = time.time() + 150; child = None
while time.time() < deadline:
    time.sleep(2)
    try:
        kids = subprocess.run(["pgrep", "-P", str(p.pid)], capture_output=True, text=True).stdout.split()
        for k in kids:
            exe = os.path.realpath(f"/proc/{k}/exe"); cmd = open(f"/proc/{k}/cmdline", "rb").read().replace(b"\0", b" ").decode()[:400]
            if "node" in exe or "acp-server" in cmd:
                child = {"pid": int(k), "exe": exe, "cmdline": cmd}
    except Exception as e: emit(event="pgrep-error", error=str(e))
    if child: break
    if p.poll() is not None: break
emit(event="child", child=child, host_rc=p.poll())
def tree(root, maxdepth=6):
    out = []
    for dp, dn, fn in os.walk(root):
        depth = dp[len(root):].count(os.sep)
        if depth > maxdepth: dn[:] = []; continue
        for d in dn: out.append(os.path.join(dp, d) + "/")
        for f in fn:
            if f in ("acp-server.js", "node", "bun", "tui.js", "package.json", ".complete", "data.sqlite3") or depth <= 2: out.append(os.path.join(dp, f))
    return sorted(out)[:120]
emit(event="layout", scratch_lib=tree(f"{SCR}/lib"), data_dir=tree(f"{SCR}/xdg-data/kiro-cli"), home_local_lib=tree(f"{SCR}/home/.local"), kas_in_data_dir=glob.glob(f"{SCR}/xdg-data/kiro-cli/kas/*"), lib_versions=glob.glob(f"{SCR}/lib/kiro-cli/*"))
time.sleep(3)
try: p.terminate(); p.wait(timeout=10)
except Exception: p.kill()
subprocess.run(["pkill", "-f", f"{SCR}/"], capture_output=True)
emit(event="done", host_rc=p.returncode)
print("PROBE-DONE")

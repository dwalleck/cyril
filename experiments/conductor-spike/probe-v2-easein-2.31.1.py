#!/usr/bin/env python3
"""H2: V3 ease-in never touches `kiro-cli acp` (zero auth: temp HOME + temp XDG_DATA_HOME).

  python3 -I probe-v2-easein-2.31.1.py <archive-bin-dir> <out.jsonl>

Arms: control | KIRO_ENABLED_FEATURES='["v3_prompt"]' | KIRO_ROLLOUT_FORCE_INTERNAL=1.
Per arm: spawn `kiro-cli acp`, record /proc exe tree, send initialize only, read for 15 s,
close stdin; record first stdout bytes, any "V3"/"Switch" text on stdout/stderr, and a
before/after snapshot (path, size, sha256) of every file under the temp HOME + XDG dirs.
"""
import hashlib, json, os, subprocess, sys, tempfile, threading, time
BIN = os.path.abspath(sys.argv[1]); OUTP = sys.argv[2]
OUT = open(OUTP, "w")
def snap(root):
    r = {}
    for d, _, fs in os.walk(root):
        for f in fs:
            p = os.path.join(d, f)
            try: b = open(p, "rb").read()
            except OSError: continue
            r[os.path.relpath(p, root)] = (len(b), hashlib.sha256(b).hexdigest()[:16])
    return r
def tree(pid):
    out, todo = [], [pid]
    while todo:
        x = todo.pop()
        try: out.append({"pid": x, "exe": os.readlink(f"/proc/{x}/exe")})
        except OSError: continue
        todo += [int(k) for k in subprocess.run(["ps", "-o", "pid=", "--ppid", str(x)], capture_output=True, text=True).stdout.split()]
    return out
for arm, extra in (("control", {}), ("v3_prompt", {"KIRO_ENABLED_FEATURES": '["v3_prompt"]'}),
                   ("force_internal", {"KIRO_ROLLOUT_FORCE_INTERNAL": "1"})):
    scr = tempfile.mkdtemp(prefix=f"easein-{arm}-", dir=os.environ.get("PROBE_SCRATCH"))
    env = {"HOME": os.path.join(scr, "home"), "XDG_DATA_HOME": os.path.join(scr, "xdg"),
           "XDG_CONFIG_HOME": os.path.join(scr, "cfg"), "XDG_RUNTIME_DIR": os.path.join(scr, "rt"),
           "PATH": BIN + ":/usr/bin:/bin", "TERM": "xterm-256color", **extra}
    for k in ("HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_RUNTIME_DIR"): os.makedirs(env[k])
    before = snap(scr)
    p = subprocess.Popen([os.path.join(BIN, "kiro-cli"), "acp"], cwd=env["HOME"], env=env, stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    outb, errb = [], []
    threading.Thread(target=lambda: [outb.append(l) for l in iter(lambda: p.stdout.read1(65536), b"")], daemon=True).start()
    threading.Thread(target=lambda: [errb.append(l) for l in iter(lambda: p.stderr.read1(65536), b"")], daemon=True).start()
    time.sleep(1.5); tr = tree(p.pid)
    init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1,
            "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True},
            "clientInfo": {"name": "cyril-probe", "version": "audit-2.31.1"}}}
    try:
        p.stdin.write((json.dumps(init) + "\n").encode()); p.stdin.flush()
    except BrokenPipeError:
        pass
    for _ in range(30):
        if p.poll() is not None: break
        time.sleep(0.5)
    try: p.stdin.close()
    except Exception: pass
    try: p.wait(timeout=10)
    except Exception:
        os.killpg(os.getpgid(p.pid), 9)
    time.sleep(0.5)
    so, se = b"".join(outb), b"".join(errb)
    after = snap(scr)
    rec = {"arm": arm, "env_extra": extra, "proc_tree": tr, "rc": p.returncode,
           "stdout_first_bytes": so[:40].decode(errors="replace"), "stdout_len": len(so),
           "stdout_lines_nonjson": [l[:200] for l in so.decode(errors="replace").splitlines() if not l.startswith("{")][:10],
           "init_result_keys": None,
           "hits": {w: (so.count(w.encode()), se.count(w.encode())) for w in ("V3", "Switch", "You're using Kiro CLI V2", "agentEngine", "v3_prompt")},
           "stderr_tail": se.decode(errors="replace")[-1500:],
           "files_new_or_changed": {k: v for k, v in after.items() if before.get(k) != v}}
    for l in so.decode(errors="replace").splitlines():
        try:
            o = json.loads(l)
            if o.get("id") == 1: rec["init_result_keys"] = sorted((o.get("result") or {}).keys()) or o.get("error")
        except Exception: pass
    cfgs = {}
    for k in rec["files_new_or_changed"]:
        if k.endswith(".json") and "settings" in k:
            try: cfgs[k] = open(os.path.join(scr, k)).read()[:2000]
            except OSError: pass
    rec["settings_json"] = cfgs
    OUT.write(json.dumps(rec) + "\n"); OUT.flush()
    print(arm, json.dumps({k: rec[k] for k in ("rc", "stdout_first_bytes", "hits", "init_result_keys")}), list(rec["files_new_or_changed"])[:12])

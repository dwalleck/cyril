#!/usr/bin/env python3
"""Archived kiro-cli 3.1.0 HOST probes for the 3.1.0 live lane (LIVE-E ii, LIVE-F).

  PROBE_SCRATCH=<lane>/tmp python3 -I probe-kas-host31-3.1.0.py <probe> <out.jsonl>

probes
  help        `kiro-cli --help`, `kiro-cli chat --help`, `kiro-cli chat _ get-kas-token --help`, `kiro-cli acp --help` with an
              EMPTY temp XDG_DATA_HOME (zero auth-store risk). Records argv, rc, stdout/stderr.
  loggedout   LIVE-E (ii): `kiro-cli acp` with an EMPTY store (logged out) + a client `initialize`; records everything the host
              writes to stdout/stderr for up to ACP_WAIT seconds (device-code URL? ACP authenticate? error?), the child process
              tree (/proc/<pid>/exe), then stops it. Never sees the real store.
  token       LIVE-F: (1) renew ONCE via the INSTALLED launcher `kiro-cli user whoami` (real store, temp HOME) so the access token
              is fresh; (2) `cp -a ~/.local/share/kiro-cli` into a fresh owner-only temp XDG_DATA_HOME; (3) run the ARCHIVED 3.1.0
              `kiro-cli chat _ get-kas-token` (NO --force-refresh) against the COPY; parse stdout JSON in-process, record only
              `kind`, data key names, expiresAt, authMethod/provider; (4) compare the copy's auth_kv token row + schema with the
              real store's; delete the copy. If the copy's token row changed, print STOP-AUTH so the lane halts auth work.
The archived binary NEVER gets the real XDG_DATA_HOME. Tokens are redacted from every record.
"""
import hashlib, json, os, shutil, signal, sqlite3, stat, subprocess, sys, tempfile, time
PROBE, OUT_PATH = sys.argv[1], sys.argv[2]
OUT = open(OUT_PATH, "w", buffering=1)
REAL_HOME = os.path.expanduser("~")
REAL_DATA = os.path.join(REAL_HOME, ".local", "share")
REAL_DB = os.path.join(REAL_DATA, "kiro-cli", "data.sqlite3")
ARCH = os.path.expanduser("~/.local/share/kiro-research/binaries/3.1.0/extracted/kirocli/bin/kiro-cli")
ARCH_BIN = os.path.dirname(ARCH)
INSTALLED = os.path.expanduser("~/.local/bin/kiro-cli")
SCRATCH = os.environ.get("PROBE_SCRATCH") or tempfile.gettempdir()
T0 = time.time()
REDACT = ("accesstoken", "refreshtoken", "idtoken", "profilearn", "authorization", "access_token", "refresh_token", "id_token", "token", "start_url", "device-registration")
def scrub(o, k=""):
    kl = str(k).lower()
    if any(r in kl for r in REDACT) and kl not in ("expiresat", "expires_at", "kind", "token_row_sha256", "token_row_changed", "auth_kv_keys", "token_keys", "data_keys", "tokenkeys"):
        return "<REDACTED>"
    if isinstance(o, dict): return {kk: scrub(v, kk) for kk, v in o.items()}
    if isinstance(o, list): return [scrub(v, k) for v in o]
    if isinstance(o, str) and (o.startswith("aoa") or o.count(".") == 2 and len(o) > 200): return "<REDACTED-STR>"
    return o
def emit(r):
    OUT.write(json.dumps(scrub(r)) + "\n")
def log(*a): print(f"[{time.time()-T0:6.1f}]", *a, flush=True)
def strip_secrets(text):
    import re
    text = re.sub(r"aoa[A-Za-z0-9_\-]{20,}", "<REDACTED-aoa>", text)
    text = re.sub(r"eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}", "<REDACTED-JWT>", text)
    text = re.sub(r"arn:aws[^\s\"']{0,200}", "<REDACTED-ARN>", text)
    return text
def temp_env(data_home, home=None):
    home = home or tempfile.mkdtemp(prefix=f"host31-{PROBE}-home-", dir=SCRATCH)
    cfg = tempfile.mkdtemp(prefix=f"host31-{PROBE}-cfg-", dir=SCRATCH)
    rt = tempfile.mkdtemp(prefix=f"host31-{PROBE}-rt-", dir=SCRATCH)
    env = dict(os.environ, HOME=home, XDG_DATA_HOME=data_home, XDG_CONFIG_HOME=cfg, KIRO_RUNTIME_DIR=rt, XDG_RUNTIME_DIR=rt,
               PATH=ARCH_BIN + ":" + os.environ.get("PATH", ""))
    for k in ("KIRO_KAS_SERVER_PATH", "KIRO_HOME", "KIRO_AGENT_PATH"):
        env.pop(k, None)
    return env, {"home": home, "data": data_home, "cfg": cfg, "rt": rt}
def run(argv, env, timeout=60, stdin=None):
    t = time.time()
    try:
        pr = subprocess.run(argv, env=env, capture_output=True, text=True, timeout=timeout, input=stdin)
        rec = {"argv": argv, "rc": pr.returncode, "stdout": strip_secrets(pr.stdout)[:6000], "stderr": strip_secrets(pr.stderr)[:6000], "secs": round(time.time() - t, 2)}
    except subprocess.TimeoutExpired as e:
        rec = {"argv": argv, "rc": "TIMEOUT", "stdout": strip_secrets((e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or ""))[:6000],
               "stderr": strip_secrets((e.stderr or b"").decode() if isinstance(e.stderr, bytes) else (e.stderr or ""))[:6000], "secs": round(time.time() - t, 2)}
    return rec
def db_rows(path):
    c = sqlite3.connect(f"file:{path}?mode=ro", uri=True, timeout=10)
    try:
        tables = [r[0] for r in c.execute("select name from sqlite_master where type='table' order by name")]
        schema = {r[0]: r[1] for r in c.execute("select name, sql from sqlite_master where type='table'")}
        auth = {}
        for k, v in c.execute("select key, value from auth_kv"):
            v = v.decode() if isinstance(v, (bytes, bytearray)) else v
            auth[k] = {"sha256": hashlib.sha256(v.encode()).hexdigest(), "len": len(v)}
            try:
                auth[k]["json_keys"] = sorted(json.loads(v).keys())
                auth[k]["expires_at"] = json.loads(v).get("expires_at")
            except Exception:
                pass
        counts = {t: c.execute(f"select count(*) from {t}").fetchone()[0] for t in tables}
        mig = [tuple(r) for r in c.execute("select * from migrations")] if "migrations" in tables else None
        extracted = [tuple(r) for r in c.execute("select * from extracted_kas_versions")] if "extracted_kas_versions" in tables else None
        pinned = [tuple(r) for r in c.execute("select * from pinned_bin_versions")] if "pinned_bin_versions" in tables else None
    finally:
        c.close()
    return {"tables": tables, "schema_sha": {t: hashlib.sha256((s or "").encode()).hexdigest()[:12] for t, s in schema.items()}, "auth_kv": auth,
            "counts": counts, "migrations": mig, "extracted_kas_versions": extracted, "pinned_bin_versions": pinned}

emit({"probe": "preflight", "lead": PROBE, "archived": ARCH, "archived_sha256": hashlib.sha256(open(ARCH, "rb").read()).hexdigest(),
      "archived_size": os.path.getsize(ARCH), "installed": INSTALLED, "installed_version": run([INSTALLED, "--version"], dict(os.environ), 20)["stdout"].strip()})

if PROBE == "help":
    data = tempfile.mkdtemp(prefix="host31-help-data-", dir=SCRATCH)
    env, dirs = temp_env(data)
    emit({"probe": "dirs", **dirs})
    for argv in ([ARCH, "--version"], [ARCH, "--help"], [ARCH, "chat", "--help"], [ARCH, "chat", "_", "--help"], [ARCH, "chat", "_", "get-kas-token", "--help"],
                 [ARCH, "acp", "--help"], [ARCH, "settings", "--help"], [ARCH, "agent", "--help"]):
        r = run(argv, env, 60)
        emit({"probe": "help", **r}); log(argv[1:], "rc", r["rc"], "out", len(r["stdout"]), "err", len(r["stderr"]))
    emit({"probe": "data_dir_after", "entries": sorted(os.listdir(data)), "kiro_cli_dir": sorted(os.listdir(os.path.join(data, "kiro-cli"))) if os.path.isdir(os.path.join(data, "kiro-cli")) else None})

elif PROBE == "loggedout":
    data = tempfile.mkdtemp(prefix="host31-lo-data-", dir=SCRATCH)
    env, dirs = temp_env(data)
    emit({"probe": "dirs", **dirs})
    wait_s = int(os.environ.get("ACP_WAIT", "45"))
    ws = tempfile.mkdtemp(prefix="host31-lo-ws-", dir=SCRATCH)
    argv = [ARCH, "acp"] + ([x for x in os.environ.get("ACP_ARGS", "").split() if x])
    err_path = OUT_PATH.replace(".jsonl", "-acp-stderr.log")
    with open(err_path, "w") as errf:
        proc = subprocess.Popen(argv, cwd=ws, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errf, text=True, bufsize=1, start_new_session=True)
        emit({"probe": "proc", "argv": argv, "pid": proc.pid, "exe": os.readlink(f"/proc/{proc.pid}/exe")})
        init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1,
                "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True, "_meta": {"kiro": {"configurationState": True, "telemetryEnabled": False}}},
                "clientInfo": {"name": "cyril-probe", "version": "audit-3.1.0"}}}
        proc.stdin.write(json.dumps(init) + "\n"); proc.stdin.flush()
        emit({"ts": round(time.time(), 3), "dir": "client->agent", "msg": init})
        import threading
        lines = []
        def reader():
            for line in proc.stdout:
                line = line.rstrip("\n")
                try: o = json.loads(line)
                except Exception: o = {"raw": line[:4000]}
                lines.append(o); emit({"ts": round(time.time(), 3), "dir": "agent->client", "msg": o})
                if isinstance(o, dict) and o.get("method") and "id" in o:
                    res = {}
                    if o["method"] == "_kiro/terminal/shell_type": res = {"shellType": "bash"}
                    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": res}) + "\n"); proc.stdin.flush()
                    emit({"ts": round(time.time(), 3), "dir": "client->agent", "msg": {"id": o["id"], "result": res}})
        threading.Thread(target=reader, daemon=True).start()
        t = time.time(); children = {}
        while time.time() - t < wait_s and proc.poll() is None:
            time.sleep(1)
            try:
                for pid in subprocess.run(["pgrep", "-P", str(proc.pid)], capture_output=True, text=True).stdout.split():
                    if pid not in children:
                        try: children[pid] = {"exe": os.readlink(f"/proc/{pid}/exe"), "cmdline": open(f"/proc/{pid}/cmdline", "rb").read().replace(b"\0", b" ").decode()[:400], "at": round(time.time() - t, 1)}
                        except OSError: pass
            except Exception: pass
            # if we got an initialize result, also try session/new and authenticate so the sign-in path is exercised
            if any(isinstance(o, dict) and o.get("id") == 1 and ("result" in o or "error" in o) for o in lines) and not children.get("_sent_new"):
                children["_sent_new"] = True
                r1 = next(o for o in lines if isinstance(o, dict) and o.get("id") == 1)
                am = ((r1.get("result") or {}).get("authMethods") or [])
                if am:
                    msg = {"jsonrpc": "2.0", "id": 2, "method": "authenticate", "params": {"methodId": am[0].get("id")}}
                    proc.stdin.write(json.dumps(msg) + "\n"); proc.stdin.flush(); emit({"ts": round(time.time(), 3), "dir": "client->agent", "msg": msg})
                msg = {"jsonrpc": "2.0", "id": 3, "method": "session/new", "params": {"cwd": ws, "mcpServers": []}}
                proc.stdin.write(json.dumps(msg) + "\n"); proc.stdin.flush(); emit({"ts": round(time.time(), 3), "dir": "client->agent", "msg": msg})
        alive = proc.poll() is None
        emit({"probe": "children", "children": children, "alive_at_end": alive, "rc": proc.poll()})
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=10)
        except Exception:
            try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            except Exception: pass
    emit({"probe": "stderr", "lines": [strip_secrets(l)[:800] for l in open(err_path, errors="replace").read().splitlines()][:200]})
    emit({"probe": "data_dir_after", "entries": sorted(os.listdir(data)), "kiro_cli_dir": sorted(os.listdir(os.path.join(data, "kiro-cli")))[:40] if os.path.isdir(os.path.join(data, "kiro-cli")) else None,
          "runtime_dir": sorted(os.listdir(dirs["rt"]))[:40], "home_kiro": sorted(os.listdir(os.path.join(dirs["home"], ".kiro")))[:40] if os.path.isdir(os.path.join(dirs["home"], ".kiro")) else None})
    # kiro logs under temp home
    logs = []
    for d, _, fs in os.walk(dirs["home"]):
        for f in fs:
            if f.endswith(".log"):
                try:
                    for line in open(os.path.join(d, f), errors="replace"):
                        if any(w in line.lower() for w in ("auth", "sign", "login", "token", "device", "error", "warn")): logs.append(os.path.relpath(os.path.join(d, f), dirs["home"]) + ": " + strip_secrets(line.strip())[:500])
                except OSError: pass
    for d, _, fs in os.walk(dirs["rt"]):
        for f in fs:
            if f.endswith(".log"):
                try:
                    for line in open(os.path.join(d, f), errors="replace"):
                        if any(w in line.lower() for w in ("auth", "sign", "login", "token", "device", "error", "warn")): logs.append(os.path.relpath(os.path.join(d, f), dirs["rt"]) + ": " + strip_secrets(line.strip())[:500])
                except OSError: pass
    emit({"probe": "host_logs", "lines": logs[:150]})

elif PROBE == "token":
    # (1) renew once via the INSTALLED launcher against the REAL store (temp HOME, real XDG_DATA_HOME)
    before_real = db_rows(REAL_DB)
    home1 = tempfile.mkdtemp(prefix="host31-token-whoami-home-", dir=SCRATCH)
    env1 = dict(os.environ, HOME=home1, XDG_DATA_HOME=REAL_DATA)
    for k in ("KIRO_KAS_SERVER_PATH", "KIRO_HOME", "KIRO_AGENT_PATH"): env1.pop(k, None)
    r = run([INSTALLED, "user", "whoami", "--format", "json"], env1, 90)
    try: who = json.loads(r["stdout"])
    except Exception: who = None
    after_real = db_rows(REAL_DB)
    tok = after_real["auth_kv"].get("kirocli:odic:token") or {}
    emit({"probe": "renew", "rc": r["rc"], "secs": r["secs"], "stderr": r["stderr"][:400], "whoami_keys": sorted(who.keys()) if isinstance(who, dict) else None,
          "accountType": (who or {}).get("accountType") if isinstance(who, dict) else None,
          "token_row_changed": (before_real["auth_kv"].get("kirocli:odic:token") or {}).get("sha256") != tok.get("sha256"),
          "expires_at_after": tok.get("expires_at")})
    log("whoami rc", r["rc"], "token changed", (before_real["auth_kv"].get("kirocli:odic:token") or {}).get("sha256") != tok.get("sha256"), "expires", tok.get("expires_at"))
    if r["rc"] != 0 or not tok:
        emit({"probe": "abort", "reason": "whoami failed or token row absent"}); sys.exit("ABORT: renew failed")
    # (2) copy the store into a fresh owner-only temp XDG_DATA_HOME
    data = tempfile.mkdtemp(prefix="host31-token-data-", dir=SCRATCH); os.chmod(data, 0o700)
    shutil.copytree(os.path.join(REAL_DATA, "kiro-cli"), os.path.join(data, "kiro-cli"), symlinks=True,
                    ignore=shutil.ignore_patterns("kas", "node", "bun", "tui.js", "*.tar*", "logs"))
    for d, dirs_, fs in os.walk(data):
        os.chmod(d, 0o700)
        for f in fs:
            try: os.chmod(os.path.join(d, f), 0o600)
            except OSError: pass
    copy_db = os.path.join(data, "kiro-cli", "data.sqlite3")
    before_copy = db_rows(copy_db)
    emit({"probe": "copy", "data": data, "copied": sorted(os.listdir(os.path.join(data, "kiro-cli"))), "db_before": {k: v for k, v in before_copy.items() if k != "auth_kv"},
          "auth_kv_keys": sorted(before_copy["auth_kv"].keys())})
    # (3) archived 3.1.0 get-kas-token (NO --force-refresh) against the COPY
    env, dirs = temp_env(data)
    emit({"probe": "dirs", **dirs})
    r2 = run([ARCH, "chat", "_", "get-kas-token"], env, 120)
    parsed = None; rec = {"probe": "get-kas-token", "argv": [ARCH, "chat", "_", "get-kas-token"], "rc": r2["rc"], "secs": r2["secs"], "stderr": r2["stderr"][:2000], "stdout_len": len(r2["stdout"])}
    try:
        parsed = json.loads(r2["stdout"])
        d = parsed.get("data") if isinstance(parsed, dict) else None
        rec.update({"kind": parsed.get("kind"), "top_keys": sorted(parsed.keys()), "data_keys": sorted(d.keys()) if isinstance(d, dict) else type(d).__name__,
                    "expiresAt": (d or {}).get("expiresAt") if isinstance(d, dict) else None, "authMethod": (d or {}).get("authMethod") if isinstance(d, dict) else None,
                    "provider": (d or {}).get("provider") if isinstance(d, dict) else None,
                    "accessToken_present": bool((d or {}).get("accessToken")) if isinstance(d, dict) else None,
                    "accessToken_len": len((d or {}).get("accessToken") or "") if isinstance(d, dict) else None})
    except Exception as e:
        rec["parse_error"] = repr(e); rec["stdout_head_redacted"] = strip_secrets(r2["stdout"])[:600]
    rec["expiresAt_equals_store_expires_at"] = (rec.get("expiresAt") == tok.get("expires_at")) if rec.get("expiresAt") else None
    rec["store_expires_at"] = tok.get("expires_at")
    emit(rec); log("get-kas-token rc", r2["rc"], "kind", rec.get("kind"), "data_keys", rec.get("data_keys"), "expiresAt==store", rec["expiresAt_equals_store_expires_at"])
    # also the --help on the copy-free path was done in `help`; here record a second call to see idempotence
    r3 = run([ARCH, "chat", "_", "get-kas-token"], env, 120)
    emit({"probe": "get-kas-token#2", "rc": r3["rc"], "secs": r3["secs"], "same_stdout": r3["stdout"] == r2["stdout"], "stderr": r3["stderr"][:800]})
    # (3b) HOST-L1 with a token: `kiro-cli acp` (a: no flag, b: --agent-engine v2, c: --agent-engine v3) against the COPY; initialize + session/new, NO prompt
    import threading
    for arm, args in (("a", []), ("b", ["--agent-engine", "v2"]), ("c", ["--agent-engine", "v3"])):
        if arm not in os.environ.get("ENGINE_ARMS", "a,b,c").split(","): continue
        ws = tempfile.mkdtemp(prefix=f"host31-tok-eng-{arm}-ws-", dir=SCRATCH)
        env_e = dict(env); env_e["PATH"] = ARCH_BIN + ":" + env_e["PATH"]
        home_e = tempfile.mkdtemp(prefix=f"host31-tok-eng-{arm}-home-", dir=SCRATCH); env_e["HOME"] = home_e
        rt_e = tempfile.mkdtemp(prefix=f"host31-tok-eng-{arm}-rt-", dir=SCRATCH); env_e["KIRO_RUNTIME_DIR"] = rt_e; env_e["XDG_RUNTIME_DIR"] = rt_e
        err_path = OUT_PATH.replace(".jsonl", f"-tokeng-{arm}-stderr.log")
        A = {"arm": arm, "args": args, "home": home_e, "rt": rt_e}
        t0 = time.time(); lines = []
        with open(err_path, "w") as errf:
            proc = subprocess.Popen([ARCH, "acp"] + args, cwd=ws, env=env_e, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errf, text=True, bufsize=1, start_new_session=True)
            A["pid"] = proc.pid; A["exe"] = os.readlink(f"/proc/{proc.pid}/exe")
            def reader():
                try:
                    for line in proc.stdout:
                        try: o = json.loads(line.rstrip("\n"))
                        except Exception: o = {"raw": line[:2000]}
                        lines.append((round(time.time() - t0, 2), o))
                        if isinstance(o, dict) and o.get("method") and "id" in o:
                            res = {"shellType": "bash"} if o["method"] == "_kiro/terminal/shell_type" else {}
                            try: proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": res}) + "\n"); proc.stdin.flush()
                            except Exception: pass
                except Exception: pass
            threading.Thread(target=reader, daemon=True).start()
            init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True, "_meta": {"kiro": {"configurationState": True, "telemetryEnabled": False}}}, "clientInfo": {"name": "cyril", "version": "0.0.0"}}}
            proc.stdin.write(json.dumps(init) + "\n"); proc.stdin.flush()
            children = {}; sent_new = False
            while time.time() - t0 < 150 and proc.poll() is None:
                time.sleep(0.5)
                try:
                    for pid in subprocess.run(["pgrep", "-P", str(proc.pid)], capture_output=True, text=True).stdout.split():
                        if pid not in children:
                            try: children[pid] = {"exe": os.readlink(f"/proc/{pid}/exe"), "cmdline": open(f"/proc/{pid}/cmdline", "rb").read().replace(b"\0", b" ").decode()[:400], "at": round(time.time() - t0, 1)}
                            except OSError: pass
                except Exception: pass
                r1 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 1), None)
                if r1 and not sent_new:
                    sent_new = True; A["init_at"] = round(time.time() - t0, 2)
                    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 3, "method": "session/new", "params": {"cwd": ws, "mcpServers": []}}) + "\n"); proc.stdin.flush()
                r3 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 3), None)
                if r3:
                    time.sleep(3); break
            A["alive_at_end"] = proc.poll() is None; A["rc"] = proc.poll(); A["secs"] = round(time.time() - t0, 1)
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=10)
            except Exception:
                try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
                except Exception: pass
        r1 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 1), None)
        r3 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 3), None)
        A["children"] = children
        ir = ((r1 or {}).get("result") or {}) if r1 else {}
        A["init_agentInfo"] = ir.get("agentInfo"); A["init_meta_kiro_keys"] = sorted((ir.get("agentCapabilities") or {}).get("_meta", {}).get("kiro", {}).keys()); A["init_authMethods"] = ir.get("authMethods"); A["init_error"] = (r1 or {}).get("error")
        A["extensionMethods_n"] = len((ir.get("agentCapabilities") or {}).get("_meta", {}).get("kiro", {}).get("extensionMethods") or [])
        A["session_new"] = {"ok": bool((r3 or {}).get("result")), "error": (r3 or {}).get("error"), "configOptions": [o.get("id") for o in ((r3 or {}).get("result") or {}).get("configOptions") or []], "meta_keys": sorted(((r3 or {}).get("result") or {}).get("_meta", {}).keys())} if r3 else None
        A["other_frames"] = [(t, (o.get("method") or ("result" if "result" in o else "error" if "error" in o else "raw"))) for t, o in lines if not (isinstance(o, dict) and o.get("id") in (1, 3))][:40]
        A["stderr"] = [strip_secrets(l)[:400] for l in open(err_path, errors="replace").read().splitlines()][:40]
        A["kas_dir_in_copy"] = sorted(os.listdir(os.path.join(data, "kiro-cli", "kas")))[:5] if os.path.isdir(os.path.join(data, "kiro-cli", "kas")) else None
        A["runtime_dir"] = sorted(os.listdir(rt_e))[:10]
        A["copy_dir_entries"] = sorted(os.listdir(os.path.join(data, "kiro-cli")))[:30]
        emit({"probe": "token-engine", **scrub(A)})
        log("token-engine", arm, "children", [(c["exe"].split("/")[-1], c["cmdline"][:90]) for c in children.values()], "meta_kiro", A["init_meta_kiro_keys"][:3], "session_new", json.dumps(A["session_new"])[:160], "secs", A["secs"])
        # auth-row guard after EVERY arm
        chk = db_rows(copy_db)["auth_kv"].get("kirocli:odic:token") or {}
        if chk.get("sha256") != (before_copy["auth_kv"].get("kirocli:odic:token") or {}).get("sha256"):
            emit({"probe": "token-engine-STOP", "arm": arm, "reason": "copy token row changed"}); print("STOP-AUTH: copy token row changed during engine arm", arm, flush=True); break
    # (4) compare the copy's token row + schema with the real store
    after_copy = db_rows(copy_db); real_now = db_rows(REAL_DB)
    tok_copy = after_copy["auth_kv"].get("kirocli:odic:token") or {}; tok_real = real_now["auth_kv"].get("kirocli:odic:token") or {}
    changed = tok_copy.get("sha256") != (before_copy["auth_kv"].get("kirocli:odic:token") or {}).get("sha256")
    real_changed = tok_real.get("sha256") != tok.get("sha256")
    emit({"probe": "compare", "copy_token_row_changed": changed, "real_token_row_changed_meanwhile": real_changed,
          "copy_auth_kv_keys_after": sorted(after_copy["auth_kv"].keys()), "copy_auth_kv_other_rows_changed": [k for k in set(before_copy["auth_kv"]) | set(after_copy["auth_kv"]) if (before_copy["auth_kv"].get(k) or {}).get("sha256") != (after_copy["auth_kv"].get(k) or {}).get("sha256")],
          "copy_tables_before": before_copy["tables"], "copy_tables_after": after_copy["tables"],
          "copy_schema_changed": [t for t in set(before_copy["schema_sha"]) | set(after_copy["schema_sha"]) if before_copy["schema_sha"].get(t) != after_copy["schema_sha"].get(t)],
          "copy_counts_before": before_copy["counts"], "copy_counts_after": after_copy["counts"],
          "copy_migrations_before": before_copy["migrations"], "copy_migrations_after": after_copy["migrations"],
          "copy_extracted_kas_after": after_copy["extracted_kas_versions"], "copy_pinned_after": after_copy["pinned_bin_versions"],
          "copy_dir_after": sorted(os.listdir(os.path.join(data, "kiro-cli")))})
    log("copy token changed", changed, "schema changed", [t for t in set(before_copy["schema_sha"]) | set(after_copy["schema_sha"]) if before_copy["schema_sha"].get(t) != after_copy["schema_sha"].get(t)])
    shutil.rmtree(data, ignore_errors=True)
    emit({"probe": "cleanup", "copy_deleted": not os.path.exists(data)})
    if changed:
        print("STOP-AUTH: the archived binary refreshed the token on the COPY; the real store's refresh token may be stale", flush=True)
elif PROBE == "engine":
    # HOST-L1 / L3 / L5: which engine does `kiro-cli acp` give a client; terminal auth method; v1 path. Fresh store per arm, no auth.
    import threading, re
    ARMS = os.environ.get("ARMS", "a,b,c,d,e,f,g,h,j,k").split(",")
    BIN = os.environ.get("HOST_BIN", ARCH)
    BIN_DIR = os.path.dirname(BIN)
    wait_s = int(os.environ.get("ACP_WAIT", "75"))
    arms = {
        "a": {"args": [], "client": "cyril", "settings": None, "meta": None},
        "b": {"args": ["--agent-engine", "v2"], "client": "cyril", "settings": None, "meta": None},
        "c": {"args": ["--agent-engine", "v3"], "client": "cyril", "settings": None, "meta": None},
        "d": {"args": [], "client": "cyril", "settings": {"acp.agentEngine": "v2"}, "meta": None},
        "e": {"args": [], "client": "cyril", "settings": {"chat.agentEngine": "v3"}, "meta": None},
        "f": {"args": [], "client": "acp-probe", "settings": None, "meta": None},
        "g": {"args": [], "client": "JetBrains.Foo", "settings": None, "meta": None},
        "h": {"args": ["--agent-engine", "v3"], "client": "cyril", "settings": None, "meta": {"terminal-auth": True}},
        "j": {"args": ["--agent-engine", "v2"], "client": "cyril", "settings": None, "meta": {"terminal-auth": True}},
        "k": {"args": ["--agent-engine", "v1"], "client": "cyril", "settings": None, "meta": None},
        "m": {"args": [], "client": "cyril", "settings": {"acp.agentEngine": "v3"}, "meta": None},
        # --auth-method client: the host forwards the v3 engine's token requests to the ACP client -> no store needed (zero auth risk)
        "a2": {"args": ["--auth-method", "client"], "client": "cyril", "settings": None, "meta": None},
        "b2": {"args": ["--auth-method", "client", "--agent-engine", "v2"], "client": "cyril", "settings": None, "meta": None},
        "c2": {"args": ["--auth-method", "client", "--agent-engine", "v3"], "client": "cyril", "settings": None, "meta": None},
        "d2": {"args": ["--auth-method", "client"], "client": "cyril", "settings": {"acp.agentEngine": "v2"}, "meta": None},
        "f2": {"args": ["--auth-method", "client"], "client": "acp-probe", "settings": None, "meta": None},
        "g2": {"args": ["--auth-method", "client"], "client": "JetBrains.Foo", "settings": None, "meta": None},
        "k2": {"args": ["--auth-method", "client", "--agent-engine", "v1"], "client": "cyril", "settings": None, "meta": None},
    }
    for arm in ARMS:
        spec = arms[arm]
        data = tempfile.mkdtemp(prefix=f"host31-eng-{arm}-data-", dir=SCRATCH)
        env, dirs = temp_env(data)
        env["PATH"] = BIN_DIR + ":" + env["PATH"]
        if spec["settings"]:
            os.makedirs(os.path.join(dirs["home"], ".kiro", "settings"), exist_ok=True)
            open(os.path.join(dirs["home"], ".kiro", "settings", "cli.json"), "w").write(json.dumps(spec["settings"]))
        ws = tempfile.mkdtemp(prefix=f"host31-eng-{arm}-ws-", dir=SCRATCH)
        argv = [BIN, "acp"] + spec["args"]
        err_path = OUT_PATH.replace(".jsonl", f"-eng-{arm}-stderr.log")
        A = {"arm": arm, "spec": spec, "bin": BIN, "dirs": dirs}
        t0 = time.time()
        with open(err_path, "w") as errf:
            try:
                proc = subprocess.Popen(argv, cwd=ws, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errf, text=True, bufsize=1, start_new_session=True)
            except OSError as e:
                emit({"probe": "engine", **A, "spawn_error": repr(e)}); continue
            A["pid"] = proc.pid; A["exe"] = os.readlink(f"/proc/{proc.pid}/exe")
            caps = {}
            if spec["meta"]: caps["_meta"] = spec["meta"]
            init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": caps, "clientInfo": {"name": spec["client"], "version": "0.0.0"}}}
            lines = []
            def reader():
                try:
                    for line in proc.stdout:
                        line = line.rstrip("\n")
                        try: o = json.loads(line)
                        except Exception: o = {"raw": line[:3000]}
                        lines.append((round(time.time() - t0, 2), o))
                        if isinstance(o, dict) and o.get("method") and "id" in o:
                            if o["method"] == "_kiro/auth/getAccessToken":
                                try: proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "error": {"code": -32000, "message": "cyril-probe: no token (host-engine probe)"}}) + "\n"); proc.stdin.flush()
                                except Exception: pass
                                continue
                            res = {"shellType": "bash"} if o["method"] == "_kiro/terminal/shell_type" else {}
                            try: proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": res}) + "\n"); proc.stdin.flush()
                            except Exception: pass
                except Exception: pass
            threading.Thread(target=reader, daemon=True).start()
            try:
                proc.stdin.write(json.dumps(init) + "\n"); proc.stdin.flush()
            except Exception as e:
                A["stdin_error"] = repr(e)
            children = {}
            sent_new = False
            while time.time() - t0 < wait_s and proc.poll() is None:
                time.sleep(0.5)
                try:
                    for pid in subprocess.run(["pgrep", "-P", str(proc.pid)], capture_output=True, text=True).stdout.split():
                        if pid not in children:
                            try: children[pid] = {"exe": os.readlink(f"/proc/{pid}/exe"), "cmdline": open(f"/proc/{pid}/cmdline", "rb").read().replace(b"\0", b" ").decode()[:300], "at": round(time.time() - t0, 1)}
                            except OSError: pass
                except Exception: pass
                r1 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 1), None)
                if r1 and not sent_new:
                    sent_new = True; A["init_at"] = round(time.time() - t0, 2)
                    msg = {"jsonrpc": "2.0", "id": 3, "method": "session/new", "params": {"cwd": ws, "mcpServers": []}}
                    try: proc.stdin.write(json.dumps(msg) + "\n"); proc.stdin.flush()
                    except Exception: pass
                r3 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 3), None)
                if r3 and time.time() - t0 > 3:
                    time.sleep(2); break
            A["alive_at_end"] = proc.poll() is None; A["rc"] = proc.poll(); A["secs"] = round(time.time() - t0, 1)
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=10)
            except Exception:
                try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
                except Exception: pass
        r1 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 1), None)
        r3 = next((o for _, o in lines if isinstance(o, dict) and o.get("id") == 3), None)
        A["children"] = children
        A["init_result"] = scrub(r1) if r1 else None
        A["init_has_meta_kiro"] = bool(((r1 or {}).get("result") or {}).get("agentCapabilities", {}).get("_meta", {}).get("kiro")) if r1 else None
        A["authMethods"] = ((r1 or {}).get("result") or {}).get("authMethods") if r1 else None
        A["session_new"] = scrub(r3) if r3 else None
        A["other_frames"] = [(t, (o.get("method") or ("result" if "result" in o else "error" if "error" in o else "raw"))) for t, o in lines if not (isinstance(o, dict) and o.get("id") in (1, 3))][:40]
        A["stderr"] = [strip_secrets(l)[:600] for l in open(err_path, errors="replace").read().splitlines()][:80]
        kas_dir = os.path.join(data, "kiro-cli", "kas")
        A["data_dir"] = {"entries": sorted(os.listdir(os.path.join(data, "kiro-cli")))[:30] if os.path.isdir(os.path.join(data, "kiro-cli")) else None,
                         "kas": sorted(os.listdir(kas_dir))[:10] if os.path.isdir(kas_dir) else None,
                         "runtime_dir": sorted(os.listdir(dirs["rt"]))[:30]}
        for sub in ("kas",):
            if os.path.isdir(kas_dir):
                for v in sorted(os.listdir(kas_dir))[:1]:
                    acp = os.path.join(kas_dir, v, "node_modules", "@kiro", "agent", "dist", "server", "acp-server.js")
                    A["kas_acp_server"] = {"path": acp, "exists": os.path.exists(acp), "size": os.path.getsize(acp) if os.path.exists(acp) else None,
                                           "sha256": hashlib.sha256(open(acp, "rb").read()).hexdigest() if os.path.exists(acp) else None}
        db = os.path.join(data, "kiro-cli", "data.sqlite3")
        if os.path.exists(db):
            try: A["db"] = {k: v for k, v in db_rows(db).items() if k in ("tables", "extracted_kas_versions", "pinned_bin_versions", "migrations")}
            except Exception as e: A["db"] = repr(e)
        emit({"probe": "engine", **A})
        log("engine", arm, "exe", os.path.basename(A.get("exe", "?")), "init_meta_kiro", A["init_has_meta_kiro"], "children", [c["exe"].split("/")[-1] for c in children.values()], "authMethods", json.dumps(A["authMethods"])[:200], "secs", A["secs"])
        # keep the ~190 MB extraction only for the first v3 arm; delete the rest
        if arm not in ("c",):
            shutil.rmtree(data, ignore_errors=True)
        else:
            A_keep = data
    emit({"probe": "engine-done"})

elif PROBE == "extract":
    # HOST-L2: extraction layout
    data = tempfile.mkdtemp(prefix="host31-extract-data-", dir=SCRATCH)
    env, dirs = temp_env(data)
    for argv, key in (([ARCH, "_", "extract-runtime"], "extract_runtime_default"),
                      ([ARCH, "_", "extract-runtime", "--dir", os.path.join(dirs["rt"], "rt2")], "extract_runtime_dir"),
                      ([ARCH, "_", "--help"], "underscore_help")):
        r = run(argv, env, 180)
        emit({"probe": key, **r})
        log(key, "rc", r["rc"], "secs", r["secs"])
    def tree(root, depth=6):
        out = []
        for d, ds, fs in os.walk(root):
            lvl = d[len(root):].count(os.sep)
            if lvl > depth: ds[:] = []; continue
            out.append(d[len(root):] or "/")
            for f in fs[:8]: out.append(d[len(root):] + "/" + f)
            if len(fs) > 8: out.append(d[len(root):] + f"/... (+{len(fs)-8} files)")
            if len(out) > 400: break
        return out
    emit({"probe": "runtime_tree", "rt": tree(dirs["rt"], 5), "data": tree(data, 4)})
    log("extract done")

elif PROBE == "extract-default":
    data = tempfile.mkdtemp(prefix="host31-xd-data-", dir=SCRATCH)
    env, dirs = temp_env(data)
    env.pop("KIRO_RUNTIME_DIR", None)
    r = run([ARCH, "_", "extract-runtime"], env, 180)
    emit({"probe": "extract_runtime_no_env", **r}); log("extract-default rc", r["rc"], r["stdout"][:200])
    def tree(root, depth=3):
        out = []
        for d, ds, fs in os.walk(root):
            lvl = d[len(root):].count(os.sep)
            if lvl > depth: ds[:] = []; continue
            out.append(d[len(root):] or "/")
            for f in fs[:6]: out.append(d[len(root):] + "/" + f)
            if len(out) > 150: break
        return out
    emit({"probe": "trees", "home": tree(dirs["home"], 4), "data": tree(data, 3), "cfg": tree(dirs["cfg"], 3)})
    for root in (dirs["home"], data):
        shutil.rmtree(root, ignore_errors=True)
else:
    sys.exit("unknown probe")
emit({"probe": "done", "elapsed_s": round(time.time() - T0, 1)})
log("done")

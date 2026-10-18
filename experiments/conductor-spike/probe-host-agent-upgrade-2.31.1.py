#!/usr/bin/env python3
"""H5: does cyril's trust append (serde_json round-trip, sorted keys, 2-space pretty) make
`kiro-cli-chat agent upgrade` treat an upgraded agent as user-edited (kiroUpgrade fingerprint)?

  python3 -I probe-host-agent-upgrade-2.31.1.py <archive-bin-dir> <out.json>

Zero auth: temp HOME + temp XDG_DATA_HOME. Arms (each its own fixture dir):
  A control      : upgrade v2 agent -> re-run upgrade (no edit)
  B v2edit_raw   : upgrade -> hand-edit a V2 setting in place (append to allowedCommands, key order kept) -> upgrade
  C cyril_append : upgrade -> cyril persist_trust_grant emulation (json.loads, append to
                   toolsSettings.execute_bash.allowedCommands, json.dumps(indent=2, sort_keys=True)) -> upgrade
  D cyril_noop_reformat : upgrade -> re-serialize only (sorted keys, no semantic change) -> upgrade
"""
import json, os, subprocess, sys, tempfile, shutil
BIN = os.path.abspath(sys.argv[1]); OUTP = sys.argv[2]
SCR = tempfile.mkdtemp(prefix="h5-", dir=os.environ.get("PROBE_SCRATCH"))
HOME = os.path.join(SCR, "home"); XDG = os.path.join(SCR, "xdg"); os.makedirs(HOME); os.makedirs(XDG)
ENV = {"HOME": HOME, "XDG_DATA_HOME": XDG, "XDG_CONFIG_HOME": os.path.join(SCR, "cfg"),
       "XDG_RUNTIME_DIR": os.path.join(SCR, "rt"), "PATH": BIN + ":/usr/bin:/bin", "TERM": "dumb", "NO_COLOR": "1"}
os.makedirs(ENV["XDG_RUNTIME_DIR"])
V2 = {"name": "probe", "description": "probe v2 agent", "prompt": "You are a probe.",
      "tools": ["fs_read", "execute_bash"], "allowedTools": ["fs_read"],
      "toolsSettings": {"execute_bash": {"allowedCommands": ["echo V2TRUST"]}, "fs_read": {"allowedPaths": ["./docs"]}}}
def up(d, tag):
    r = subprocess.run([os.path.join(BIN, "kiro-cli-chat"), "agent", "upgrade", d], env=ENV,
                       capture_output=True, text=True, timeout=120)
    return {"tag": tag, "rc": r.returncode, "out": r.stdout[-2500:], "err": r.stderr[-1500:]}
def rd(p): return open(p).read()
RES = {"bin": BIN, "version": subprocess.run([os.path.join(BIN, "kiro-cli"), "--version"], env=ENV, capture_output=True, text=True).stdout.strip(), "arms": {}}
for arm in ("A_control", "B_v2edit_raw", "C_cyril_append", "D_cyril_noop_reformat"):
    d = os.path.join(SCR, arm); os.makedirs(d)
    p = os.path.join(d, "probe.json")
    open(p, "w").write(json.dumps(V2, indent=2))
    A = {"steps": [up(d, "upgrade1")], "after_upgrade1": rd(p)}
    u1 = json.loads(rd(p))
    if arm == "B_v2edit_raw":
        s = rd(p); s = s.replace('"echo V2TRUST"', '"echo V2TRUST",\n        "echo CYRILTRUST"', 1); open(p, "w").write(s)
    elif arm == "C_cyril_append":
        o = json.loads(rd(p))
        o.setdefault("toolsSettings", {}).setdefault("execute_bash", {}).setdefault("allowedCommands", []).append("echo CYRILTRUST")
        open(p, "w").write(json.dumps(o, indent=2, sort_keys=True))
    elif arm == "D_cyril_noop_reformat":
        o = json.loads(rd(p)); open(p, "w").write(json.dumps(o, indent=2, sort_keys=True))
    A["after_edit"] = rd(p)
    A["steps"].append(up(d, "upgrade2"))
    A["after_upgrade2"] = rd(p)
    A["kiroUpgrade_1"] = u1.get("kiroUpgrade"); 
    try: A["kiroUpgrade_2"] = json.loads(rd(p)).get("kiroUpgrade")
    except Exception as e: A["kiroUpgrade_2"] = repr(e)
    A["files"] = sorted(os.listdir(d))
    RES["arms"][arm] = A
RES["home_tree"] = [os.path.relpath(os.path.join(a, f), SCR) for a, _, fs in os.walk(SCR) for f in fs if "/A_" not in a and "/B_" not in a and "/C_" not in a and "/D_" not in a][:50]
json.dump(RES, open(OUTP, "w"), indent=1)
for k, A in RES["arms"].items():
    print("==", k); [print("  ", s["tag"], s["rc"], s["out"].strip().replace("\n", " | ")[:500], s["err"].strip()[:200]) for s in A["steps"]]

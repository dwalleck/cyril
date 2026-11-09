#!/usr/bin/env python3
"""Seed a temp HOME + git workspace for the 3.1.0 cyril-on-KAS leg (leg 5) and print a launch line.

  PROBE_SCRATCH=<lane>/tmp python3 -I probe-kas-cyrilseed-3.1.0.py <scenario>

Seeds (all scenarios): $HOME/.kiro/skills/probe-skill/SKILL.md, $HOME/.kiro/prompts/probe-prompt.md,
workspace PROBE.txt, `git init` + `git add -A`. Per scenario:
  S4  $HOME/.kiro/agents/trustprobe.json (V3 permissions.rules: []) + cli.json chat.defaultAgent=trustprobe
  S6  $HOME/.kiro/settings/mcp.json -> the offline stdio fixture live/fixtures/mcp_probe_server.py
  S7  KIRO_HOME=<tmp>/kh with kh-agent; $HOME/.kiro/agents/home-agent.json
  S8  workspace tier (LIVE-A shape): <ws>/.kiro/agents/wsagent.json + <ws>/.kiro/settings/mcp.json (fixture "wsprobe") + <ws>/.kiro/workflows/wsrecipe
  S5  $HOME/.kiro/agents/helper.json (read-only sub-agent target)
Prints a shell `env ... cyril --agent-engine kas` line for the hex1b terminal; never spawns Kiro itself.
"""
import json, os, subprocess, sys, tempfile
SC = sys.argv[1]
SCRATCH = os.environ["PROBE_SCRATCH"]
REPO = "/home/dwalleck/repos/cyril"
LANE = os.path.join(REPO, "experiments/conductor-spike/audit-3.1.0/live")
KAS_NEW = os.path.expanduser("~/.local/share/kiro-research/kas-carves/3.1.0/node_modules/@kiro/agent/dist/server/acp-server.js")
NODE = os.path.expanduser("~/.local/share/kiro-research/binaries/3.1.0/node")   # node_NEW v22.23.4
home = tempfile.mkdtemp(prefix=f"cyril-{SC}-home-", dir=SCRATCH)
ws = tempfile.mkdtemp(prefix=f"cyril-{SC}-ws-", dir=SCRATCH)
def w(p, t):
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "w").write(t)
sk = "---\nname: probe-skill\ndescription: Probe skill. Use when asked to run the probe-skill probe.\n---\n# probe-skill\nWhen invoked, reply with exactly SKILLRAN followed by any arguments.\n"
w(f"{home}/.kiro/skills/probe-skill/SKILL.md", sk)
w(f"{home}/.kiro/prompts/probe-prompt.md", "Reply with exactly PROMPTRAN and then $ARGUMENTS.\n")
w(f"{ws}/PROBE.txt", "ALPHA\n")
env = {"HOME": home, "XDG_DATA_HOME": os.path.expanduser("~/.local/share"),
       "KIRO_KAS_SERVER_PATH": KAS_NEW, "KIRO_AGENT_PATH": NODE}
if SC == "S4":
    w(f"{home}/.kiro/agents/trustprobe.json", json.dumps({
        "name": "trustprobe", "description": "trust probe agent", "prompt": "You are a terse probe agent.",
        "tools": ["*"], "permissions": {"rules": []}}, indent=2))
if SC == "S6":
    w(f"{home}/.kiro/settings/mcp.json", json.dumps({"mcpServers": {"probesrv": {
        "command": sys.executable, "args": ["-I", f"{LANE}/fixtures/mcp_probe_server.py", "probesrv", f"{SCRATCH}/{os.path.basename(home)}-mcp.log"]}}}))
if SC == "S5":
    w(f"{home}/.kiro/agents/helper.json", json.dumps({"name": "helper", "description": "helper: reads files and answers in one word",
        "prompt": "You are helper, a terse sub-agent. Read what you are asked and answer in one word.", "tools": ["read"], "permissions": {"rules": []}}, indent=2))
if SC == "S8":
    w(f"{ws}/.kiro/agents/wsagent.json", json.dumps({"name": "wsagent", "description": "workspace-tier agent (trust probe)",
        "prompt": "You are wsagent, a terse probe agent. Reply with exactly WSAGENT when greeted.", "tools": ["read", "shell"], "permissions": {"rules": []}}, indent=2))
    w(f"{ws}/.kiro/settings/mcp.json", json.dumps({"mcpServers": {"wsprobe": {
        "command": sys.executable, "args": ["-I", f"{LANE}/fixtures/mcp_probe_server.py", "wsprobe", f"{SCRATCH}/{os.path.basename(home)}-wsmcp.log"]}}}))
    w(f"{ws}/.kiro/workflows/wsrecipe.workflow.json", json.dumps({"name": "wsrecipe", "description": "workspace recipe", "injectOriginalUserRequest": False,
        "steps": [{"type": "step", "id": "s1", "agent": "wf-coder", "prompt": "Reply with exactly RECIPEOK."}]}, indent=1))
if SC == "S7":
    kh = os.path.join(os.path.dirname(home), os.path.basename(home) + "-kh")
    w(f"{kh}/agents/kh-agent.json", json.dumps({"name": "kh-agent", "description": "agent seeded under KIRO_HOME",
        "prompt": "Reply KHAGENT.", "tools": ["read_file"]}, indent=2))
    w(f"{home}/.kiro/agents/home-agent.json", json.dumps({"name": "home-agent", "description": "agent seeded under HOME/.kiro",
        "prompt": "Reply HOMEAGENT.", "tools": ["read_file"]}, indent=2))
    env["KIRO_HOME"] = kh
subprocess.run(["git", "init", "-q", "-b", "main"], cwd=ws, check=True)
subprocess.run(["git", "add", "-A"], cwd=ws, check=True)
print(json.dumps({"home": home, "ws": ws, "env": env}))
print("cd " + ws + " && env " + " ".join(f"{k}={v}" for k, v in env.items()) + f" {REPO}/target/debug/cyril --agent-engine kas")

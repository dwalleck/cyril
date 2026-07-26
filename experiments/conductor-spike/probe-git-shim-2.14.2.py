#!/usr/bin/env python3
"""
Probe: end-to-end verification of the `git` PATH shim (experiments/agent-shims/git)
through a live kiro-cli-chat 2.14.2 ACP session.

Re-confirms on a CURRENT binary what the 2.5.1 side-channel capture established:
that $AGENT_CONTEXT_OUT lands in the tool result's rawOutput Json as
`agent_notes`, and that $AGENT_DISPLAY_OUT streams as tool_call_update content.

Unlike the cargo shim (which is about token economy — full log to the human, a
summary to the model), the git shim delivers labeled side-band facts while
keeping stdout byte-exact.

MEASURED CHANNEL SEMANTICS (this probe's original run corrected a wrong
assumption — $AGENT_CONTEXT_OUT is NOT model-only):
  stdout             -> model (tool result `stdout`) AND human (streamed content)
  $AGENT_DISPLAY_OUT -> human ONLY (streamed content; absent from tool result)
  $AGENT_CONTEXT_OUT -> model as rawOutput.Json.agent_notes AND ALSO streamed to
                        the human as tool_call_update content
Only the display channel is one-audience. Q3c below pins the surprising half so a
future change in either direction shows up as a failing check.

Setup is self-contained and touches no real repository: a throwaway git repo
with one linked worktree is created under a temp dir and used as the session cwd.

Questions:
  Q1. Context channel — does agent_notes carry the shim's repo-state facts?
  Q2. Display channel — does the human-only marker stream as tool_call_update
      content?
  Q3. Channel boundaries — facts and marker absent from stdout (a, b), and the
      context-channel text IS additionally streamed as content (c).
  Q4. Fidelity — is stdout still unmodified real `git status` output?
  Q5. Behavior — does the agent report the right branch?

Writes a wire log to /tmp/cyril-probe-git-shim.log and prints findings.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time

KIRO = os.path.expanduser("~/.local/bin/kiro-cli-chat")
REPO = "/home/dwalleck/repos/cyril"
SHIM_DIR = os.path.join(REPO, "experiments", "agent-shims")
LOG_PATH = "/tmp/cyril-probe-git-shim.log"

FACTS_MARKER = "repo-state facts"
DISPLAY_MARKER = "[git-shim] annotated"


def build_fixture() -> tuple[str, str]:
    """Throwaway repo + one linked worktree. Returns (workdir, repo_path)."""
    work = tempfile.mkdtemp(prefix="git-shim-probe-")
    repo = os.path.join(work, "repo")
    os.makedirs(repo)

    def g(*args, cwd=repo):
        subprocess.run(["git", *args], cwd=cwd, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    g("init", "-q", "-b", "trunk", ".")
    g("config", "user.email", "probe@example.invalid")
    g("config", "user.name", "probe")
    with open(os.path.join(repo, "tracked.txt"), "w") as f:
        f.write("base\n")
    g("add", "tracked.txt")
    g("commit", "-q", "-m", "base")
    g("branch", "-q", "sidebranch")
    g("worktree", "add", "-q", os.path.join(work, "wt"), "sidebranch")
    # Leave the tree dirty so the counts in the facts are non-trivial.
    with open(os.path.join(repo, "tracked.txt"), "a") as f:
        f.write("modified\n")
    open(os.path.join(repo, "untracked.txt"), "w").close()
    return work, repo


def main() -> int:
    work, cwd = build_fixture()
    log_file = open(LOG_PATH, "w")
    env = dict(os.environ)
    env["PATH"] = SHIM_DIR + ":" + env["PATH"]
    print(f"[setup] binary   = {KIRO}")
    print(f"[setup] shim dir = {SHIM_DIR} (prepended to PATH)")
    print(f"[setup] fixture  = {cwd} (throwaway; trunk + linked worktree on sidebranch)")
    print(f"[setup] wire log = {LOG_PATH}\n")

    proc = subprocess.Popen(
        [KIRO, "acp"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        cwd=cwd, env=env,
    )
    incoming: list[dict] = []
    lock = threading.Lock()

    def reader():
        assert proc.stdout is not None
        while True:
            line = proc.stdout.readline()
            if not line:
                return
            text = line.decode("utf-8", errors="replace").rstrip("\n")
            log_file.write(f"S->C {text}\n")
            log_file.flush()
            try:
                with lock:
                    incoming.append(json.loads(text))
            except json.JSONDecodeError:
                pass

    threading.Thread(target=reader, daemon=True).start()
    next_id = [1]

    def send(method, params):
        msg = {"jsonrpc": "2.0", "id": next_id[0], "method": method, "params": params}
        next_id[0] += 1
        line = json.dumps(msg)
        log_file.write(f"C->S {line}\n")
        log_file.flush()
        assert proc.stdin is not None
        proc.stdin.write((line + "\n").encode())
        proc.stdin.flush()
        return msg["id"]

    def respond(req_id, result):
        line = json.dumps({"jsonrpc": "2.0", "id": req_id, "result": result})
        log_file.write(f"C->S {line}\n")
        log_file.flush()
        assert proc.stdin is not None
        proc.stdin.write((line + "\n").encode())
        proc.stdin.flush()

    def auto_approve():
        seen = set()
        while True:
            with lock:
                frames = list(incoming)
            for f in frames:
                if f.get("method") == "session/request_permission" and f.get("id") not in seen:
                    seen.add(f["id"])
                    opts = f.get("params", {}).get("options", [])
                    allow = next((o for o in opts if o.get("kind") == "allow_once"),
                                 opts[0] if opts else None)
                    if allow:
                        respond(f["id"], {"outcome": {"outcome": "selected",
                                                      "optionId": allow["optionId"]}})
                        print(f"[perm] auto-approved {allow['optionId']!r}")
                elif f.get("method", "").startswith(("fs/", "terminal/")) and f.get("id") not in seen:
                    seen.add(f["id"])
                    respond(f["id"], {})
            time.sleep(0.2)

    threading.Thread(target=auto_approve, daemon=True).start()

    def wait(req_id, timeout=60.0):
        end = time.time() + timeout
        while time.time() < end:
            with lock:
                for f in incoming:
                    if f.get("id") == req_id and ("result" in f or "error" in f):
                        return f
            time.sleep(0.1)
        return None

    if not wait(send("initialize", {
        "protocolVersion": 1,
        "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False},
                               "terminal": False},
        "clientInfo": {"name": "cyril-probe", "version": "0.0.1"},
    }), 30):
        print("[ERROR] no initialize response")
        proc.terminate()
        return 1
    print("[1] initialize OK")

    new = wait(send("session/new", {"cwd": cwd, "mcpServers": []}), 60)
    if not new or "error" in new:
        print(f"[ERROR] session/new failed: {new}")
        proc.terminate()
        return 1
    sid = new["result"]["sessionId"]
    print(f"[2] session/new OK -> {sid}")
    time.sleep(2)

    prompt = ("Run this exact shell command: git status --short --branch\n"
              "Then tell me, in one short sentence, which git branch this "
              "directory is on. Do not run any other commands.")
    print("[3] prompt sent — waiting up to 300s…")
    resp = wait(send("session/prompt", {"sessionId": sid,
                                        "prompt": [{"type": "text", "text": prompt}]}), 300)
    if not resp:
        print("[ERROR] turn did not complete")
        proc.terminate()
        return 1
    print(f"[3] turn complete: {resp.get('result')}")
    time.sleep(2)
    proc.terminate()

    with lock:
        frames = list(incoming)

    exec_result, streamed, agent_text = None, "", ""
    for f in frames:
        upd = f.get("params", {}).get("update", {})
        kind = upd.get("sessionUpdate")
        if kind == "agent_message_chunk" and isinstance(upd.get("content"), dict):
            agent_text += upd["content"].get("text", "")
        if kind not in ("tool_call", "tool_call_update"):
            continue
        for item in (upd.get("rawOutput") or {}).get("items", []):
            if isinstance(item, dict) and "Json" in item and "stdout" in item["Json"]:
                exec_result = item["Json"]
        for c in upd.get("content") or []:
            inner = c.get("content", {})
            if isinstance(inner, dict):
                streamed += inner.get("text", "")

    print("\n" + "=" * 70 + "\nFINDINGS\n" + "=" * 70)
    if not exec_result:
        print("[INCONCLUSIVE] no execute tool result with stdout on the wire — "
              "the agent likely did not run the shell command.")
        print(f"agent said: {agent_text[:300]!r}")
        shutil.rmtree(work, ignore_errors=True)
        return 1

    stdout = exec_result.get("stdout", "")
    notes = exec_result.get("agent_notes", "") or ""
    checks = {
        "Q1 agent_notes carries repo-state facts": FACTS_MARKER in notes,
        "Q2 display marker streamed as content": DISPLAY_MARKER in streamed,
        "Q3a facts absent from stdout": FACTS_MARKER not in stdout,
        "Q3b display marker absent from stdout": DISPLAY_MARKER not in stdout,
        "Q3c context text ALSO streamed as content (not model-only)":
            FACTS_MARKER in streamed,
        "Q4 stdout is unmodified git output": "## trunk" in stdout and "[git-shim]" not in stdout,
        "Q5 agent names the branch": "trunk" in agent_text.lower(),
    }
    for k, v in checks.items():
        print(f"  {'PASS' if v else 'FAIL'}  {k}")
    print(f"\n  facts say worktree-kind=main: {'yes' if 'worktree-kind: main' in notes else 'NO'}")
    print(f"  facts list the sibling worktree: {'yes' if 'sidebranch' in notes else 'NO'}")
    print(f"\n--- model-visible stdout ({len(stdout)} chars) ---\n{stdout}")
    print(f"--- agent_notes (model only, {len(notes)} chars) ---\n{notes}")
    print(f"--- streamed display content ({len(streamed)} chars) ---\n{streamed[:600]}")
    print(f"--- agent reply ---\n{agent_text.strip()[:400]}")

    for f in frames:
        if "metadata" in f.get("method", "") and "meteringUsage" in json.dumps(f.get("params", {})):
            print(f"\ncost: {json.dumps(f['params'].get('meteringUsage'))}")

    shutil.rmtree(work, ignore_errors=True)
    return 0 if all(checks.values()) else 2


if __name__ == "__main__":
    sys.exit(main())

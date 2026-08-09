#!/usr/bin/env python3
"""A/B probe: same todo-forcing prompt on a GPT vs a Claude model over KAS —
does the GPT leg emit `tasks` as a numeric-keyed object (record) on the FIRST
todo_list create attempt while the Claude leg emits a proper array?

Context: gpt-5.6-terra sessions show todo_list create failing Zod validation
twice per create ("Expected array, received object" + missing task_description),
succeeding only via the lenient record branch. Hypothesis: provider schema
adaptation for OpenAI strict structured outputs degrades anyOf[array,null]
properties server-side. This probe isolates the MODEL axis: identical binary,
prompt, cwd shape, and day.

    probe-todo-schema-ab-2.12.3.py [kiro-cli-binary]

Model selection uses `session/set_config_option {configId:"model"}` — the model
config option is delivered asynchronously by ModelRegistryManager after
session/new (KAS 0.17.2), so the probe pumps for it before selecting.

Ground truth is read from the KAS session store (~/.kiro/sessions/*/<sid>/
messages.jsonl) after each leg; the wire is only used to drive the turn and
cancel once the first todo_list create settles. All frames land in
logs/todo-ab-2.12.3-<date>.jsonl."""
import glob
import json
import os
import queue
import signal
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time

KIRO = sys.argv[1] if len(sys.argv) > 1 else "kiro-cli"


def load_auth():
    """Token + profileArn from kiro-cli's sqlite store (the ARN is NOT in the
    token JSON — it lives separately in the state table)."""
    db = sqlite3.connect(os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3"))
    row = db.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    if not row:
        sys.exit("no auth token row — run `kiro-cli login` first")
    tok = json.loads(row[0])
    prow = db.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    arn = json.loads(prow[0]).get("arn") if prow else None
    if not arn:
        sys.exit("no profile arn in state table — turns would die (profileArn required)")
    return {"accessToken": tok["access_token"], "expiresAt": tok["expires_at"], "profileArn": arn}


AUTH = load_auth()
DATE = "20260715"
LOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "logs",
                   f"todo-ab-2.12.3-{DATE}.jsonl")
PROMPT = ("Use your todo list tool to create a task list with exactly two tasks: "
          "(1) inventory the files in this directory, (2) summarize what you found. "
          "After creating the task list, do NOT execute the tasks - immediately end "
          "your turn and tell me the list is ready.")

logf = open(LOG, "a")


def wlog(leg, direction, msg):
    if isinstance(msg, dict) and isinstance(msg.get("result"), dict) and "accessToken" in msg["result"]:
        msg = {**msg, "result": {**msg["result"], "accessToken": "[REDACTED]"}}
    logf.write(json.dumps({"ts": time.time(), "leg": leg, "dir": direction, "msg": msg}) + "\n")
    logf.flush()


def find_model_option(node):
    """Walk arbitrary JSON for a select config option whose id is 'model'."""
    if isinstance(node, dict):
        if node.get("id") == "model" or node.get("configId") == "model":
            for k in ("options", "values", "choices"):
                if isinstance(node.get(k), list):
                    return node
        for v in node.values():
            hit = find_model_option(v)
            if hit:
                return hit
    elif isinstance(node, list):
        for v in node:
            hit = find_model_option(v)
            if hit:
                return hit
    return None


def option_values(opt):
    vals = []
    for k in ("options", "values", "choices"):
        for item in opt.get(k) or []:
            if isinstance(item, dict):
                vid = item.get("value") or item.get("id")
                if isinstance(vid, str):
                    vals.append(vid)
            elif isinstance(item, str):
                vals.append(item)
    return vals


class Acp:
    def __init__(self, leg):
        self.leg = leg
        self.cwd = tempfile.mkdtemp(prefix=f"todoab-{leg}-")
        subprocess.run("git init -q -b main", cwd=self.cwd, shell=True)
        with open(os.path.join(self.cwd, "README.md"), "w") as fh:
            fh.write("# probe fixture\nTwo-file fixture for a todo-list probe.\n")
        with open(os.path.join(self.cwd, "notes.txt"), "w") as fh:
            fh.write("nothing to see here\n")
        self.p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=self.cwd,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1,
                                  start_new_session=True)
        self.q = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()
        self.i = 0
        self.notes = []          # collected notifications
        self.todo_calls = {}     # toolCallId -> latest status, for early cancel

    def _reader(self):
        for line in self.p.stdout:
            line = line.strip()
            if line:
                self.q.put(line)

    def _send(self, obj):
        wlog(self.leg, "client->agent", obj)
        self.p.stdin.write(json.dumps(obj) + "\n")
        self.p.stdin.flush()

    def req(self, m, pr):
        self.i += 1
        self._send({"jsonrpc": "2.0", "id": self.i, "method": m, "params": pr})
        return self.i

    def notify(self, m, pr):
        self._send({"jsonrpc": "2.0", "method": m, "params": pr})

    def _answer_server_req(self, o):
        if o.get("method") == "_kiro/auth/getAccessToken":
            self._send({"jsonrpc": "2.0", "id": o["id"], "result": AUTH})
            return
        if o.get("method") == "session/request_permission":
            opts = (o.get("params") or {}).get("options") or []
            allow = next((x for x in opts if x.get("kind") == "allow_once"), None) \
                or next((x for x in opts if "allow" in str(x.get("kind"))), None) \
                or (opts[0] if opts else None)
            oid = (allow or {}).get("optionId")
            result = {"outcome": {"outcome": "selected", "optionId": oid}} if oid \
                else {"outcome": {"outcome": "cancelled"}}
            print(f"    [perm] auto-answered with {oid}")
        else:
            result = {}
        self._send({"jsonrpc": "2.0", "id": o["id"], "result": result})

    def _track_todo(self, o):
        """Watch session/update frames for Task List tool calls (early-cancel signal)."""
        upd = ((o.get("params") or {}).get("update") or {})
        if upd.get("sessionUpdate") not in ("tool_call", "tool_call_update"):
            return
        blob = json.dumps(upd)
        tcid = upd.get("toolCallId")
        if tcid and ("todo_list" in blob or "Task List" in blob or tcid in self.todo_calls):
            st = upd.get("status")
            if st:
                self.todo_calls[tcid] = st
                print(f"    [todo] {tcid[:22]} -> {st}")

    def pump(self, until=None, to=60, stop_when=None):
        end = time.time() + to
        while time.time() < end:
            try:
                raw = self.q.get(timeout=2)
            except queue.Empty:
                if stop_when and stop_when():
                    return "stopped"
                continue
            try:
                o = json.loads(raw)
            except Exception:
                continue
            wlog(self.leg, "agent->client", o)
            if o.get("id") is not None and o.get("method"):
                self._answer_server_req(o)
                continue
            if o.get("method"):
                self.notes.append(o)
                self._track_todo(o)
                if stop_when and stop_when():
                    return "stopped"
                continue
            if until is not None and o.get("id") == until and ("result" in o or "error" in o):
                return o
        return None

    def close(self):
        try:
            self.p.stdin.close()
        except Exception:
            pass
        try:
            os.killpg(os.getpgid(self.p.pid), signal.SIGTERM)
        except Exception:
            self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except Exception:
            try:
                os.killpg(os.getpgid(self.p.pid), signal.SIGKILL)
            except Exception:
                pass


def read_store(session_id):
    hits = glob.glob(os.path.expanduser(f"~/.kiro/sessions/*/{session_id}/messages.jsonl"))
    if not hits:
        return None, []
    calls = []
    for line in open(hits[0]):
        try:
            rec = json.loads(line)
        except Exception:
            continue
        p = rec.get("payload") or {}
        if p.get("type") == "tool_call" and p.get("toolName") == "todo_list":
            calls.append(p)
    return hits[0], calls


def leg(name, model_substrings):
    print(f"\n########## LEG {name}: want model matching {model_substrings}")
    a = Acp(name)
    try:
        rid = a.req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
        if a.pump(rid, 90) is None:
            print("  initialize: NO RESPONSE — aborting leg")
            return None
        rid = a.req("session/new", {"cwd": a.cwd, "mcpServers": []})
        r = a.pump(rid, 60)
        if r is None or "error" in r:
            print("  session/new failed:", json.dumps(r)[:300] if r else "no response")
            return None
        res = r["result"]
        sid = res.get("sessionId")
        print("  sessionId =", sid)

        opt = find_model_option(res)
        deadline = time.time() + 30
        while opt is None and time.time() < deadline:
            a.pump(until=None, to=3)
            for n in a.notes:
                opt = find_model_option(n)
                if opt:
                    break
        if opt is None:
            print("  NO model config option surfaced in 30s — cannot pin model; aborting leg")
            return None
        vals = option_values(opt)
        print("  model option values:", vals)
        current = opt.get("currentValue") or opt.get("value")
        pick = None
        for want in model_substrings:
            pick = next((v for v in vals if want in v), None)
            if pick:
                break
        if pick is None:
            print(f"  no value matches {model_substrings} — aborting leg")
            return None
        print(f"  selecting model {pick} (current={current})")
        rid = a.req("session/set_config_option", {"sessionId": sid, "configId": "model", "value": pick})
        r = a.pump(rid, 30)
        if r is None or "error" in r:
            print("  set_config_option failed:", json.dumps(r)[:300] if r else "no response")
            return None

        print("  sending prompt...")
        rid = a.req("session/prompt", {"sessionId": sid,
                                       "prompt": [{"type": "text", "text": PROMPT}]})

        def first_todo_settled():
            return any(s in ("completed", "failed") for s in a.todo_calls.values()) \
                and all(s not in ("pending", "in_progress", "executing") for s in a.todo_calls.values()) \
                and len(a.todo_calls) > 0

        # Let the turn run until the todo create settles (incl. retries) or timeout.
        settled_at = None
        end = time.time() + 300
        resp = None
        while time.time() < end:
            r = a.pump(until=rid, to=5)
            if isinstance(r, dict):
                resp = r
                break
            if first_todo_settled():
                if settled_at is None:
                    settled_at = time.time()
                # grace period so an immediate retry after a failure is captured
                if time.time() - settled_at > 20:
                    print("  todo call(s) settled — cancelling turn to save tokens")
                    a.notify("session/cancel", {"sessionId": sid})
                    resp = a.pump(until=rid, to=60)
                    break
            else:
                settled_at = None
        print("  prompt response:", json.dumps(resp)[:200] if resp else "none (timeout)")
        time.sleep(3)  # let the store flush
        return sid
    finally:
        a.close()


def report(name, sid):
    print(f"\n========== STORE VERDICT: leg {name} (session {sid})")
    if not sid:
        print("  leg did not run")
        return
    path, calls = read_store(sid)
    print("  store:", path)
    meta = {}
    if path:
        sj = os.path.join(os.path.dirname(path), "session.json")
        if os.path.exists(sj):
            meta = json.load(open(sj))
    print("  session.json modelId =", meta.get("modelId"))
    creates = [c for c in calls if (c.get("args") or {}).get("command") == "create"]
    if not creates:
        print("  NO todo_list create calls recorded")
        return
    for n, c in enumerate(creates, 1):
        tasks = (c.get("args") or {}).get("tasks")
        shape = "array" if isinstance(tasks, list) else type(tasks).__name__
        nulls = sorted(k for k, v in (c.get("args") or {}).items() if v is None)
        print(f"  create attempt {n}: status={c.get('status')} tasks-shape={shape} "
              f"toolCallId={c.get('toolCallId', '')[:20]} explicit-nulls={nulls}")
        print(f"    tasks={json.dumps(tasks)[:220]}")


ver = subprocess.run([KIRO, "--version"], capture_output=True, text=True)
print("binary:", (ver.stdout or ver.stderr).strip(), "| log:", LOG)

sid_a = leg("A-gpt", ["gpt-5.6-terra", "gpt-5.6", "gpt"])
sid_b = leg("B-claude", ["claude-sonnet", "claude-haiku", "claude"])

report("A-gpt", sid_a)
report("B-claude", sid_b)
logf.close()

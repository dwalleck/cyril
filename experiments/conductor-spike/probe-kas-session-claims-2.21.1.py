#!/usr/bin/env python3
"""Live paired probe for the KAS 2.21.1 release-note claims.

The probe deliberately drives the released launcher (not node directly) with the
KAS bundle pinned by KIRO_KAS_SERVER_PATH.  It runs three fresh, same-workload
families:

* ``session-hook``: a SessionStart precomputed result and two turns; inspect
  persisted session/prompt evidence rather than attributing a model echo.
* ``change-notify``: mutate exactly one isolated steering file and one isolated
  skill file after startup settles; count notification payloads per mutation.
* ``model-defaults``: save a valid model/effort on the parent session, switch
  between an unconfigured and model-pinned workspace agent, and run a workflow
  with an inherited and explicitly pinned step.  Verdicts use wire config state.

Every run uses a temporary HOME and runtime paths, while XDG_DATA_HOME points at
an existing data directory solely for the already-installed Kiro assets/auth DB.
Token material and callback payloads are redacted before they reach JSONL or
verdict artifacts.  The child process is always in its own process group.
"""
from __future__ import annotations

import hashlib
import json
import os
import queue
import re
import shutil
import signal
import sqlite3
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from typing import Any

LEG = os.environ.get("LEG", "new")
if LEG not in {"old", "new"}:
    raise SystemExit("LEG must be old or new")

OUTDIR = Path(os.environ.get("PROBE_OUT", Path(__file__).resolve().parent))
OUTDIR.mkdir(parents=True, exist_ok=True)
KIRO = os.environ.get(
    "KIRO_BIN",
    "/home/dwalleck/.local/share/kiro-research/binaries/"
    + ("2.21.0/kiro-cli" if LEG == "old" else "2.21.1/extracted/kiro-cli"),
)
KAS_PIN = os.environ.get(
    "KAS_PIN",
    "/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.1/"
    + ("2.21.0" if LEG == "old" else "2.21.1")
    + "/tree/node_modules/@kiro/agent/dist/server/acp-server.js",
)
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", "/home/dwalleck/.local/share")
AUTH_DB = os.environ.get("KIRO_AUTH_DB", str(Path(DATA_HOME) / "kiro-cli" / "data.sqlite3"))
TRACE = OUTDIR / f"kas-session-claims-{LEG}-2.21.1.jsonl"
VERDICT = OUTDIR / f"kas-session-claims-{LEG}-2.21.1-verdict.json"
STDERR = OUTDIR / f"kas-session-claims-{LEG}-2.21.1-stderr.log"
PROMPT_EVIDENCE = OUTDIR / f"kas-session-claims-{LEG}-2.21.1-prompt-evidence.txt"

# Identical across the paired legs: each marker is unique to this probe family,
# but not to a release, so structural comparisons are meaningful.
SESSION_MARKER = "KAS_SESSION_START_CLAIM_MARKER_2_21_1"
STEER_BASE = "KAS_STEERING_BASELINE_2_21_1"
STEER_MUT = "KAS_STEERING_MUTATION_2_21_1"
SKILL_BASE = "KAS_SKILL_BASELINE_2_21_1"
SKILL_MUT = "KAS_SKILL_MUTATION_2_21_1"
SAVED_MODEL = "claude-sonnet-5"
PINNED_MODEL = "claude-opus-4.8"
SAVED_EFFORT = "low"
PINNED_EFFORT = "max"

SENSITIVE_KEYS = {
    "accesstoken", "refreshtoken", "authorization", "clientsecret", "secret",
    "password", "profilearn", "expiresat", "token", "kirokey",
}


def scrub(value: Any, key: str = "") -> Any:
    if key.lower().replace("_", "") in SENSITIVE_KEYS:
        return "<REDACTED>"
    if isinstance(value, dict):
        return {k: scrub(v, k) for k, v in value.items()}
    if isinstance(value, list):
        return [scrub(v, key) for v in value]
    return value


def canonical(value: Any) -> str:
    return json.dumps(scrub(value), sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value).encode()).hexdigest()[:16]


def profile_arn() -> str | None:
    explicit = os.environ.get("KIRO_PROFILE_ARN")
    if explicit:
        return explicit
    try:
        out = subprocess.run(
            [KIRO, "user", "whoami"], capture_output=True, text=True, timeout=15,
        ).stdout
        match = re.search(r"arn:aws:codewhisperer:\S+", out)
        return match.group(0) if match else None
    except Exception:
        return None


PROFILE_ARN = profile_arn()


def read_token() -> dict[str, Any]:
    try:
        with sqlite3.connect(AUTH_DB) as db:
            row = db.execute(
                "SELECT value FROM auth_kv WHERE key='kirocli:odic:token'"
            ).fetchone()
        if not row:
            return {}
        raw = row[0]
        if isinstance(raw, (bytes, bytearray)):
            raw = raw.decode()
        data = json.loads(raw)
        if not isinstance(data, dict):
            return {}
        result = {
            "accessToken": data.get("access_token"),
            "expiresAt": data.get("expires_at"),
            "profileArn": PROFILE_ARN,
        }
        return {k: v for k, v in result.items() if v is not None}
    except Exception:
        return {}


def marker_evidence(roots: list[Path], marker: str) -> list[dict[str, Any]]:
    """Capture bounded, redacted snippets only from marker-bearing files."""
    evidence: list[dict[str, Any]] = []
    seen: set[Path] = set()
    for root in roots:
        if not root:
            continue
        try:
            candidates = [root] if root.is_file() else list(root.rglob("*"))
        except OSError:
            continue
        for path in candidates:
            try:
                if path in seen or not path.is_file() or path.stat().st_size > 5_000_000:
                    continue
                seen.add(path)
                text = path.read_bytes().decode("utf-8", "replace")
            except OSError:
                continue
            count = text.count(marker)
            if count == 0:
                continue
            snippets = []
            pos = 0
            for _ in range(min(count, 4)):
                pos = text.find(marker, pos)
                if pos < 0:
                    break
                snippet = text[max(0, pos - 180):pos + len(marker) + 180]
                snippet = re.sub(r"(?i)(accessToken|refreshToken|authorization|profileArn|expiresAt|password|secret)\\s*[:=]\\s*[^,}\\s]+", r"\\1=<REDACTED>", snippet)
                snippets.append(snippet.replace("\\n", "\\\\n"))
                pos += len(marker)
            evidence.append({"path": str(path), "markerCount": count, "snippets": snippets})
    return evidence


class Probe:
    def __init__(self, family: str, session_start_output: bool = True):
        self.family = family
        self.session_start_output = session_start_output
        self.home = Path(tempfile.mkdtemp(prefix=f"kas-claims-{LEG}-{family}-home-"))
        self.workspace = Path(tempfile.mkdtemp(prefix=f"kas-claims-{LEG}-{family}-workspace-"))
        self.runtime = Path(tempfile.mkdtemp(prefix=f"kas-claims-{LEG}-{family}-runtime-"))
        self.tmp = Path(tempfile.mkdtemp(prefix=f"kas-claims-{LEG}-{family}-tmp-"))
        self.home_kiro = self.home / ".kiro"
        self.workspace_kiro = self.workspace / ".kiro"
        (self.workspace / ".git").mkdir(exist_ok=True)
        subprocess.run(["git", "init", "-q", "-b", "main"], cwd=self.workspace, check=True)
        self.trace = open(TRACE, "a", encoding="utf-8", buffering=1)
        self.stderr = open(STDERR, "a", encoding="utf-8", buffering=1)
        self.proc: subprocess.Popen[str] | None = None
        self.lines: queue.Queue[Any] = queue.Queue()
        self.next_id = 100
        self.messages: list[dict[str, Any]] = []
        self.requests: dict[str, int] = {}
        self.request_params: list[dict[str, Any]] = []
        self.updates: list[dict[str, Any]] = []
        self.responses: dict[int, dict[str, Any]] = {}
        self.turn_text: dict[str, str] = {}
        self.log_dir: str | None = None
        self.session_id: str | None = None
        self.terms: dict[str, subprocess.Popen[str]] = {}
        self.session_start_calls = 0
        self.session_start_payloads: list[dict[str, Any]] = []
        self.governance: list[dict[str, Any]] = []
        self.hook_exec_calls: list[dict[str, Any]] = []
        self._seed()

    def _seed(self) -> None:
        """Seed only disposable HOME/workspace config used by this family."""
        (self.home_kiro / "steering").mkdir(parents=True, exist_ok=True)
        (self.home_kiro / "skills" / "claims-skill").mkdir(parents=True, exist_ok=True)
        (self.home_kiro / "steering" / "claims-steering.md").write_text(
            f"# claims steering\n\n{STEER_BASE}\n", encoding="utf-8"
        )
        (self.home_kiro / "skills" / "claims-skill" / "SKILL.md").write_text(
            f"---\nname: claims-skill\ndescription: claims probe\n---\n\n{SKILL_BASE}\n",
            encoding="utf-8",
        )
        (self.workspace_kiro / "hooks").mkdir(parents=True, exist_ok=True)
        (self.workspace_kiro / "hooks" / "session-start.json").write_text(
            json.dumps({
                "version": 1,
                "hooks": [{
                    "name": "claims-session-start",
                    "trigger": "SessionStart",
                    "action": {"type": "command", "command": "printf " + SESSION_MARKER},
                    "enabled": True,
                }],
            }, indent=2) + "\n", encoding="utf-8",
        )
        (self.workspace_kiro / "agents").mkdir(parents=True, exist_ok=True)
        (self.workspace_kiro / "agents" / "claims-inherit.json").write_text(
            json.dumps({
                "name": "claims-inherit", "description": "no model configured",
                "prompt": "Reply exactly CLAIMS_INHERIT.", "tools": [],
                "dispatchKind": "custom-agent",
            }, indent=2) + "\n", encoding="utf-8",
        )
        (self.workspace_kiro / "agents" / "claims-pinned.json").write_text(
            json.dumps({
                "name": "claims-pinned", "description": "pinned model probe",
                "model": PINNED_MODEL, "prompt": "Reply exactly CLAIMS_PINNED.",
                "tools": [], "dispatchKind": "custom-agent",
            }, indent=2) + "\n", encoding="utf-8",
        )

    def record(self, direction: str, obj: dict[str, Any]) -> None:
        safe = dict(obj)
        if safe.get("method") == "_kiro/auth/getAccessToken":
            safe["params"] = "<REDACTED>"
        line = {"ts": time.time(), "dir": direction, "family": self.family, "msg": scrub(safe)}
        self.trace.write(json.dumps(line, sort_keys=True, ensure_ascii=False) + "\n")
        self.messages.append(line)

    def start(self) -> None:
        env = dict(os.environ)
        env.update({
            "HOME": str(self.home), "XDG_DATA_HOME": DATA_HOME,
            "XDG_RUNTIME_DIR": str(self.runtime), "TMPDIR": str(self.tmp),
            "KIRO_KAS_SERVER_PATH": KAS_PIN,
        })
        self.proc = subprocess.Popen(
            [KIRO, "acp", "--agent-engine", "kas"], cwd=self.workspace, env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr,
            text=True, bufsize=1, start_new_session=True,
        )
        assert self.proc.stdin and self.proc.stdout
        threading.Thread(target=self._reader, daemon=True).start()
        self.record("probe", {"family": self.family, "launcher": KIRO, "kasPin": KAS_PIN,
                               "home": str(self.home), "workspace": str(self.workspace),
                               "config": {"savedModel": SAVED_MODEL, "savedEffort": SAVED_EFFORT,
                                          "pinnedModel": PINNED_MODEL, "pinnedEffort": PINNED_EFFORT}})

    def _reader(self) -> None:
        assert self.proc and self.proc.stdout
        for line in self.proc.stdout:
            if not line.strip():
                continue
            try:
                self.lines.put(json.loads(line))
            except json.JSONDecodeError:
                self.lines.put({"_parse_error": line[:500]})
        self.lines.put(None)

    def send(self, obj: dict[str, Any]) -> None:
        assert self.proc and self.proc.stdin
        self.record("client->agent", obj)
        self.proc.stdin.write(json.dumps(obj, ensure_ascii=False) + "\n")
        self.proc.stdin.flush()

    def request(self, method: str, params: dict[str, Any] | None = None) -> int:
        self.next_id += 1
        obj: dict[str, Any] = {"jsonrpc": "2.0", "id": self.next_id, "method": method}
        if params is not None:
            obj["params"] = params
        self.send(obj)
        return self.next_id

    def reply(self, rid: Any, result: Any) -> None:
        self.send({"jsonrpc": "2.0", "id": rid, "result": result})

    def _safe_workspace(self, path: str | None) -> bool:
        if not path:
            return False
        try:
            return os.path.commonpath([str(self.workspace), os.path.abspath(path)]) == str(self.workspace)
        except (TypeError, ValueError):
            return False

    def _hook_for(self, trigger: str | None) -> dict[str, Any] | None:
        # Keep the control leg genuinely hook-free apart from the explicit
        # SessionStart callback (which returns ``results: []`` there).
        if not self.session_start_output:
            return None
        if trigger not in ("SessionStart", "sessionStart"):
            return None
        return {
            "id": f"claims-{trigger or 'unknown'}", "name": f"claims-{trigger or 'unknown'}",
            "action": {"type": "runCommand", "command": "printf " + SESSION_MARKER},
            "approved": True,
        }

    def handle_request(self, obj: dict[str, Any]) -> None:
        method = obj.get("method", "")
        params = obj.get("params") or {}
        self.requests[method] = self.requests.get(method, 0) + 1
        self.request_params.append({"method": method, "keys": sorted(params)})
        rid = obj.get("id")
        if rid is None:
            return
        if method == "_kiro/auth/getAccessToken":
            self.reply(rid, read_token())
        elif method == "_kiro/terminal/shell_type":
            self.reply(rid, {"shellType": "bash"})
        elif method == "_kiro/hooks/list":
            hook = self._hook_for(params.get("trigger"))
            self.reply(rid, {"hooks": [hook] if hook else []})
        elif method == "_kiro/hooks/sessionStart":
            self.session_start_calls += 1
            payload = {"trigger": params.get("trigger"), "sessionId": params.get("sessionId")}
            self.session_start_payloads.append(payload)
            results = []
            if self.session_start_output:
                results = [{
                    "id": "claims-session-start", "hookId": "claims-session-start",
                    "name": "claims-session-start", "originalType": "runCommand",
                    "content": SESSION_MARKER,
                }]
            self.reply(rid, {"results": results})
        elif method == "_kiro/hooks/executeHook":
            command = str(params.get("command", ""))
            # Only execute the fixed probe command returned by this host.
            result: dict[str, Any] = {"output": SESSION_MARKER, "exitCode": 0, "cancelled": False}
            self.hook_exec_calls.append({"name": params.get("hookName"), "command": command,
                                         "result": {"exitCode": 0, "hasOutput": True}})
            self.reply(rid, result)
        elif method in ("fs/read_text_file", "_kiro/fs/read_file"):
            path = params.get("path")
            try:
                if not self._safe_workspace(path):
                    raise OSError("path outside probe workspace")
                self.reply(rid, {"content": Path(path).read_text(encoding="utf-8")})
            except OSError as exc:
                self.reply(rid, {"content": f"(probe read error: {exc})"})
        elif method in ("fs/write_text_file", "_kiro/fs/write_file"):
            path = params.get("path")
            try:
                if not self._safe_workspace(path):
                    raise OSError("path outside probe workspace")
                Path(path).parent.mkdir(parents=True, exist_ok=True)
                Path(path).write_text(str(params.get("content", params.get("text", ""))), encoding="utf-8")
                self.reply(rid, {})
            except OSError as exc:
                self.reply(rid, {"error": str(exc)})
        elif method in ("fs/stat", "_kiro/fs/stat"):
            path = params.get("path")
            try:
                if not self._safe_workspace(path):
                    raise OSError("path outside probe workspace")
                stat = os.stat(path)
                self.reply(rid, {"type": "directory" if os.path.isdir(path) else "file", "size": stat.st_size})
            except OSError:
                self.reply(rid, {})
        elif method in ("fs/read_directory", "_kiro/fs/read_directory"):
            path = params.get("path")
            try:
                if not self._safe_workspace(path):
                    raise OSError("path outside probe workspace")
                entries = [{"name": p.name, "type": "directory" if p.is_dir() else "file"}
                           for p in sorted(Path(path).iterdir(), key=lambda x: x.name)]
                self.reply(rid, {"entries": entries})
            except OSError as exc:
                self.reply(rid, {"error": str(exc)})
        elif method == "terminal/create":
            cwd = params.get("cwd") or str(self.workspace)
            if not self._safe_workspace(cwd):
                self.reply(rid, {"terminalId": "term-rejected"})
            else:
                try:
                    child = subprocess.Popen(["bash", "-lc", str(params.get("command", ""))], cwd=cwd,
                                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
                    tid = f"term-{len(self.terms) + 1}"; self.terms[tid] = child
                    self.reply(rid, {"terminalId": tid})
                except OSError:
                    self.reply(rid, {"terminalId": "term-rejected"})
        elif method == "terminal/wait_for_exit":
            child = self.terms.get(params.get("terminalId"))
            if not child:
                self.reply(rid, {"exitCode": -1, "signal": None})
            else:
                try: child.wait(timeout=30)
                except subprocess.TimeoutExpired: child.kill(); child.wait()
                self.reply(rid, {"exitCode": child.returncode if child.returncode >= 0 else None,
                                 "signal": -child.returncode if child.returncode < 0 else None})
        elif method == "terminal/output":
            child = self.terms.get(params.get("terminalId"))
            output = ""
            if child and child.stdout and child.poll() is not None:
                output = child.stdout.read()
            self.reply(rid, {"output": output, "truncated": False,
                             "exitStatus": {"exitCode": child.returncode if child and child.returncode >= 0 else None,
                                             "signal": -child.returncode if child and child.returncode < 0 else None}})
        elif method in ("terminal/release", "terminal/kill"):
            child = self.terms.pop(params.get("terminalId"), None)
            if method == "terminal/kill" and child and child.poll() is None:
                child.kill()
            self.reply(rid, {})
        elif method == "session/request_permission":
            options = params.get("options") or []
            pick = next((o for o in options if o.get("kind") == "allow_once" or "allow" in str(o.get("optionId", "")).lower()), options[0] if options else None)
            self.reply(rid, {"outcome": {"outcome": "selected", "optionId": pick.get("optionId")}} if pick else {"outcome": {"outcome": "cancelled"}})
        else:
            self.reply(rid, {})

    def consume(self, obj: dict[str, Any]) -> None:
        self.record("agent->client", obj)
        if obj.get("id") is not None and ("result" in obj or "error" in obj):
            self.responses[obj["id"]] = obj
        if obj.get("method") and obj.get("id") is not None:
            self.handle_request(obj)
            return
        method = obj.get("method", "")
        params = obj.get("params") or {}
        self.requests[method] = self.requests.get(method, 0)
        if method == "_kiro/governance/state":
            self.governance.append(scrub(params))
        if method == "session/update":
            update = params.get("update") or {}
            if isinstance(update, dict):
                event = {"ts": time.time(), "sessionId": params.get("sessionId"),
                         "sessionUpdate": update.get("sessionUpdate"),
                         "method": method, "payload": scrub(update)}
                self.updates.append(event)
                if update.get("sessionUpdate") == "agent_message_chunk":
                    sid = params.get("sessionId", "")
                    self.turn_text[sid] = self.turn_text.get(sid, "") + str((update.get("content") or {}).get("text", ""))
        if method == "_kiro/steering/documents_changed":
            self.updates.append({"ts": time.time(), "sessionId": params.get("sessionId"), "method": method, "payload": scrub(params)})
        if method == "_kiro/progressive_context/items_changed":
            self.updates.append({"ts": time.time(), "sessionId": params.get("sessionId"), "method": method, "payload": scrub(params)})
        if method == "_kiro/sessions/changed" and params.get("upserted"):
            pass

    def pump(self, until_id: int | None = None, timeout: float = 60, idle_exit: float | None = None) -> dict[str, Any] | None:
        deadline = time.time() + timeout
        last = time.time()
        found = None
        while time.time() < deadline:
            try:
                obj = self.lines.get(timeout=min(0.5, max(0.05, deadline - time.time())))
            except queue.Empty:
                if idle_exit is not None and time.time() - last >= idle_exit:
                    break
                continue
            if obj is None:
                break
            last = time.time()
            if obj.get("_parse_error"):
                self.record("agent-parse-error", obj)
                continue
            self.consume(obj)
            if until_id is not None and obj.get("id") == until_id and ("result" in obj or "error" in obj):
                found = obj
                break
        return found

    def wait_idle(self, idle: float = 4, timeout: float = 30) -> None:
        self.pump(timeout=timeout, idle_exit=idle)

    def config_updates(self, after: float = 0) -> list[dict[str, Any]]:
        return [u for u in self.updates if u["ts"] >= after and u.get("payload", {}).get("sessionUpdate") in ("config_option_update", "available_commands_update")]

    def cleanup(self) -> None:
        if self.proc is not None:
            if self.proc.poll() is None:
                try:
                    os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
                    self.proc.wait(timeout=5)
                except (ProcessLookupError, subprocess.TimeoutExpired):
                    try: os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
                    except ProcessLookupError: pass
        for child in self.terms.values():
            if child.poll() is None:
                try: child.kill()
                except ProcessLookupError: pass
        self.trace.flush(); self.trace.close(); self.stderr.flush(); self.stderr.close()


def common_initialize(p: Probe, hooks: bool = False) -> dict[str, Any]:
    meta = {"kiro": {"hooks": {"enabled": True}}} if hooks else None
    caps: dict[str, Any] = {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}
    if meta:
        caps["_meta"] = meta
    iid = p.request("initialize", {"protocolVersion": 1,
        "clientInfo": {"name": "cyril-session-claims-2.21.1", "version": "2.21.1"},
        "clientCapabilities": caps})
    init = p.pump(iid, 90) or {}
    p.log_dir = (((init.get("result") or {}).get("agentCapabilities") or {}).get("_meta") or {}).get("kiro", {}).get("logging", {}).get("logDir")
    return init


def new_session(p: Probe, hooks: bool = False) -> dict[str, Any]:
    params: dict[str, Any] = {"cwd": str(p.workspace), "mcpServers": []}
    if hooks:
        params["_meta"] = {"kiro": {"hooks": {"enabled": True}}}
    nid = p.request("session/new", params)
    result = p.pump(nid, 120) or {}
    p.session_id = (result.get("result") or {}).get("sessionId")
    return result


def run_session_hook(session_start_output: bool = True) -> dict[str, Any]:
    family = "session-hook-with" if session_start_output else "session-hook-control"
    p = Probe(family, session_start_output=session_start_output)
    result: dict[str, Any] = {"family": family, "leg": LEG,
                              "settings": {"hooks": {"enabled": True}, "marker": SESSION_MARKER,
                                           "twoTurns": True, "sessionStartOutput": session_start_output}}
    try:
        p.start()
        init = common_initialize(p, hooks=True)
        new = new_session(p, hooks=True)
        # The session-start hook is part of the first turn's graph setup.  Keep
        # the two prompts identical across legs; no model text is used as proof.
        first_id = p.request("session/prompt", {"sessionId": p.session_id,
            "prompt": [{"type": "text", "text": "First probe turn. Reply briefly."}]})
        first = p.pump(first_id, 240) or {}
        p.wait_idle(idle=3, timeout=8)
        second_id = p.request("session/prompt", {"sessionId": p.session_id,
            "prompt": [{"type": "text", "text": "Second probe turn. Reply briefly."}]})
        second = p.pump(second_id, 240) or {}
        p.wait_idle(idle=3, timeout=8)
        # The workspace hook fixture is not authoritative evidence.  Search
        # only KAS-owned logs/session records so a control leg cannot count its
        # own fixture marker as persisted hook content.
        roots = [p.home_kiro / "sessions", p.home_kiro / "logs"]
        evidence = marker_evidence(roots, SESSION_MARKER)
        session_start_records: list[dict[str, Any]] = []
        authoritative_turn_types: list[str] = []
        authoritative_files: list[dict[str, Any]] = []
        for root in roots:
            try:
                files = list(root.rglob("*")) if root.exists() else []
            except OSError:
                files = []
            for path in files:
                try:
                    if not path.is_file() or path.stat().st_size > 5_000_000:
                        continue
                    text = path.read_bytes().decode("utf-8", "replace")
                except OSError:
                    continue
                authoritative_files.append({"path": str(path), "bytes": len(text.encode()),
                                            "markerCount": text.count(SESSION_MARKER),
                                            "hookInstructionCount": text.count("HOOK_INSTRUCTION")})
                if path.name == "messages.jsonl":
                    try:
                        records = [json.loads(line) for line in text.splitlines() if line.strip()]
                    except json.JSONDecodeError:
                        records = []
                    authoritative_turn_types = [
                        str(r.get("payload", {}).get("type", "")) for r in records
                    ]
                    session_start_records = [
                        scrub(r) for r in records
                        if r.get("payload", {}).get("type") == "session_start"
                    ]
                    authoritative_files[-1].update({
                        "userTurnCount": sum(r.get("payload", {}).get("type") == "user" for r in records),
                        "sessionStartRecordCount": sum(r.get("payload", {}).get("type") == "session_start" for r in records),
                        "markerSessionStartCount": sum(
                            r.get("payload", {}).get("type") == "session_start"
                            and SESSION_MARKER in json.dumps(r)
                            for r in records
                        ),
                    })
        result.update({"initialize": init.get("result"), "sessionNew": new.get("result"),
                       "sessionStartCalls": p.session_start_calls,
                       "sessionStartPayloads": p.session_start_payloads,
                       "hookExecuteCalls": p.hook_exec_calls,
                       "turnText": scrub(p.turn_text), "requests": p.requests,
                       "governance": p.governance,
                       "authoritativeTurnTypes": authoritative_turn_types,
                       "authoritativeSessionStartRecords": session_start_records,
                       "promptLogDir": p.log_dir, "authoritativeFiles": authoritative_files,
                       "markerFileEvidence": evidence,
                       "markerFileCount": sum(e["markerCount"] for e in evidence)})
    finally:
        p.cleanup()
    return result


def notification_window(p: Probe, start: float, end: float) -> dict[str, Any]:
    events = [u for u in p.updates if start <= u["ts"] <= end and u["method"] in (
        "_kiro/steering/documents_changed", "_kiro/progressive_context/items_changed",
    )]
    counts: dict[str, int] = {}
    payloads: dict[str, dict[str, Any]] = {}
    for event in events:
        method = event["method"]
        counts[method] = counts.get(method, 0) + 1
        payload = event["payload"]
        key = f"{method}:{digest(payload)}"
        if key not in payloads:
            payloads[key] = {"method": method, "digest": digest(payload), "count": 0, "payload": payload}
        payloads[key]["count"] += 1
    return {"eventCount": len(events), "methodCounts": counts,
            "distinctPayloads": list(payloads.values()),
            "duplicatePayloads": [v for v in payloads.values() if v["count"] > 1]}


def run_change_notify() -> dict[str, Any]:
    p = Probe("change-notify")
    steer = p.home_kiro / "steering" / "claims-steering.md"
    skill = p.home_kiro / "skills" / "claims-skill" / "SKILL.md"
    result: dict[str, Any] = {"family": "change-notify", "leg": LEG,
                              "settings": {"homeSteering": str(steer), "homeSkill": str(skill),
                                           "steeringMutation": STEER_MUT, "skillMutation": SKILL_MUT}}
    try:
        p.start()
        init = common_initialize(p)
        new = new_session(p)
        p.wait_idle(idle=5, timeout=35)
        startup_end = time.time()
        steer.write_text(f"# claims steering\n\n{STEER_MUT}\n", encoding="utf-8")
        steer_at = time.time()
        p.wait_idle(idle=2, timeout=14)
        steer_end = time.time()
        skill.write_text("---\nname: claims-skill\ndescription: claims probe\n---\n\n" + SKILL_MUT + "\n", encoding="utf-8")
        skill_at = time.time()
        p.wait_idle(idle=2, timeout=14)
        skill_end = time.time()
        result.update({"initialize": init.get("result"), "sessionNew": new.get("result"),
                       "startupSettleAt": startup_end, "steeringMutationAt": steer_at,
                       "skillMutationAt": skill_at, "steeringWindow": notification_window(p, steer_at, steer_end),
                       "skillWindow": notification_window(p, skill_at, skill_end),
                       "requests": p.requests})
    finally:
        p.cleanup()
    return result


def model_state(events: list[dict[str, Any]], value: str | None = None) -> list[dict[str, Any]]:
    states = []
    for event in events:
        payload = event.get("payload") or {}
        if payload.get("sessionUpdate") != "config_option_update":
            continue
        options = payload.get("configOptions") or []
        selected = {str(o.get("id")): o.get("currentValue") for o in options if isinstance(o, dict)}
        if value is None or selected.get("model") == value or selected.get("effortLevel") == value:
            states.append({"ts": event["ts"], "selected": selected,
                           "modelOption": next((o for o in options if o.get("id") == "model"), None),
                           "effortOption": next((o for o in options if o.get("id") == "effortLevel"), None)})
    return states


def run_model_defaults() -> dict[str, Any]:
    p = Probe("model-defaults")
    result: dict[str, Any] = {"family": "model-defaults", "leg": LEG,
                              "settings": {"savedModel": SAVED_MODEL, "savedEffort": SAVED_EFFORT,
                                           "pinnedModel": PINNED_MODEL, "pinnedEffort": PINNED_EFFORT,
                                           "agents": {"inherit": "workspace/.kiro/agents/claims-inherit.json",
                                                      "pinned": "workspace/.kiro/agents/claims-pinned.json"}}}
    try:
        p.start()
        init = common_initialize(p)
        new = new_session(p)
        sid = p.session_id
        saved_model_at = time.time()
        smid = p.request("session/set_config_option", {"sessionId": sid, "configId": "model", "value": SAVED_MODEL})
        sm = p.pump(smid, 45) or {}
        saved_effort_at = time.time()
        seid = p.request("session/set_config_option", {"sessionId": sid, "configId": "effortLevel", "value": SAVED_EFFORT})
        se = p.pump(seid, 45) or {}
        # First switch to the unconfigured agent while the saved defaults are
        # active.  Run the workflow in that state so its inherited step has a
        # clean parent default; the pinned step then supplies its own values.
        inherit_at = time.time()
        imid = p.request("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "claims-inherit"})
        inherit = p.pump(imid, 60) or {}
        p.wait_idle(idle=2, timeout=8)
        workflow = {"name": "claims-model-defaults", "description": "paired model default probe",
                    "inputs": {}, "steps": [
                        {"type": "step", "id": "inherit", "agent": "claims-inherit",
                         "prompt": "Do not use tools. Reply exactly CLAIMS_INHERIT."},
                        {"type": "step", "id": "pinned", "agent": "claims-pinned",
                         "modelId": PINNED_MODEL, "effortLevel": PINNED_EFFORT,
                         "prompt": "Do not use tools. Reply exactly CLAIMS_PINNED."},
                    ]}
        wid = p.request("_kiro/workflow/new", {"workflow": workflow, "inputs": {},
            "parentSessionId": sid, "workspacePaths": [str(p.workspace)],
            "parentModelId": SAVED_MODEL, "parentEffortLevel": SAVED_EFFORT})
        wn = p.pump(wid, 90) or {}
        workflow_id = (wn.get("result") or {}).get("workflowId")
        workflow_events_start = len(p.updates)
        run_response: dict[str, Any] | None = None
        if workflow_id:
            iid = p.request("_kiro/workflow/invoke", {"workflowId": workflow_id})
            p.pump(iid, 90)
            deadline = time.time() + 720
            while time.time() < deadline and not any(
                m["msg"].get("method") == "_kiro/workflow/run_complete" for m in p.messages
            ):
                p.pump(timeout=30)
            run_response = next((m["msg"] for m in p.messages if m["msg"].get("method") == "_kiro/workflow/run_complete"), None)
        workflow_events = p.updates[workflow_events_start:]
        # Finally switch the same parent to the model-pinned profile.  This
        # makes profile precedence visible in the parent config update too.
        pinned_at = time.time()
        pmid = p.request("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "claims-pinned"})
        pinned = p.pump(pmid, 60) or {}
        p.wait_idle(idle=2, timeout=8)
        direct_events = [e for e in p.config_updates(saved_model_at) if e.get("sessionId") == sid]
        result.update({"initialize": init.get("result"), "sessionNew": new.get("result"),
                       "savedModelResponse": scrub(sm), "savedEffortResponse": scrub(se),
                       "inheritModeResponse": scrub(inherit), "pinnedModeResponse": scrub(pinned),
                       "directConfigStates": model_state(direct_events),
                       "directConfigStatesAtInherit": model_state([e for e in direct_events if e["ts"] >= inherit_at]),
                       "directConfigStatesAtPinned": model_state([e for e in direct_events if e["ts"] >= pinned_at]),
                       "workflowNew": scrub(wn), "workflowId": workflow_id,
                       "workflowRunComplete": scrub(run_response),
                       "workflowConfigStates": model_state(workflow_events),
                       "requests": p.requests})
    finally:
        p.cleanup()
    return result

def main() -> None:
    # A single JSONL contains all three fresh family traces for this leg.  The
    TRACE.write_text("", encoding="utf-8")
    STDERR.write_text("", encoding="utf-8")
    results = [run_session_hook(True), run_session_hook(False), run_change_notify(), run_model_defaults()]
    verdict = {"leg": LEG, "launcher": KIRO, "kasPin": KAS_PIN,
               "binaryPair": {"old": "/home/dwalleck/.local/share/kiro-research/binaries/2.21.0/kiro-cli",
                              "new": "/home/dwalleck/.local/share/kiro-research/binaries/2.21.1/extracted/kiro-cli"},
               "families": results,
               "comparisonKeys": {"sessionHook": "WITH/CONTROL sessionStart callback and persisted marker evidence (not model echo)",
                                   "changeNotify": "post-settle per-file method/payload counts and duplicate digests",
                                   "modelDefaults": "wire config_option_update currentValue plus workflow node state"},
               "redaction": "auth callback params/results and sensitive keys redacted"}
    with open(VERDICT, "w", encoding="utf-8") as fh:
        json.dump(scrub(verdict), fh, indent=2, sort_keys=True, ensure_ascii=False)
    with open(PROMPT_EVIDENCE, "w", encoding="utf-8") as fh:
        fh.write("Authoritative KAS-owned session records (sanitized; no model echo attribution):\n")
        for family in results[:2]:
            fh.write(f"\n[{family['family']}] markerFileCount={family.get('markerFileCount', 0)} governance={family.get('governance')}\n")
            fh.write(f"turnTypes={family.get('authoritativeTurnTypes', [])}\n")
            for record in family.get("authoritativeSessionStartRecords", []):
                fh.write("session_start=" + json.dumps(record, sort_keys=True, ensure_ascii=False) + "\n")
            for item in family.get("markerFileEvidence", []):
                fh.write(f"{item['path']} markerCount={item['markerCount']}\n")
                for snippet in item.get("snippets", []):
                    fh.write(f"  {snippet}\n")
        fh.write("\nKAS governance reports promptLogging=false on the live social-profile control; authoritative backend prompt capture is unavailable. Model-generated text is retained only in the raw trace and is not used for hook attribution.\n")
    print(f"== leg={LEG} launcher={KIRO} kas_pin={KAS_PIN}")
    print(f"== trace={TRACE}")
    print(f"== verdict={VERDICT}")
    print(f"== prompt_evidence={PROMPT_EVIDENCE}")


if __name__ == "__main__":
    main()

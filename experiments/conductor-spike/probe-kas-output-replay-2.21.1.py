#!/usr/bin/env python3
"""Paired KAS output-storage and session-replay probe for Kiro 2.21.1.

The probe runs the same controlled tool turn against the 2.21.0 and 2.21.1
CLI launchers, pinning KAS 0.54.8/0.58.7 respectively with
``KIRO_KAS_SERVER_PATH``.  It then closes and relaunches each CLI with the
same temporary HOME/workspace and calls the supported ACP ``session/load``.

The JSONL captures retain sanitized raw JSON-RPC frames.  The verdict records
wire payload observations, the actual on-disk marker file (if any), original
versus replayed tool-card fields, and a provider-refusal coverage gap.  Auth
callback values are never recorded.

Usage:
  probe-kas-output-replay-2.21.1.py OLD_KIRO NEW_KIRO OUTPUT_PREFIX
"""
from __future__ import annotations

import hashlib
import json
import os
import queue
import re
import signal
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from typing import Any


if len(sys.argv) != 4:
    raise SystemExit(__doc__)

OLD, NEW, PREFIX = sys.argv[1:]
REAL_HOME = os.environ.get("HOME", str(Path.home()))
REAL_XDG = os.path.join(REAL_HOME, ".local", "share")
AUTH_DB = os.path.join(REAL_XDG, "kiro-cli", "data.sqlite3")
OLD_PIN = "/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.1/2.21.0/tree/node_modules/@kiro/agent/dist/server/acp-server.js"
NEW_PIN = "/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.1/2.21.1/tree/node_modules/@kiro/agent/dist/server/acp-server.js"
OUTPUT_PREFIX = "OUTPUT_REPLAY_2211_"
OUTPUT_END = "_OUTPUT_REPLAY_2211_ENDMARK"
OUTPUT_BODY = OUTPUT_PREFIX + ("X" * 32000) + OUTPUT_END + "\n"
ENABLE_LARGE_OUTPUT = os.environ.get("KAS_LARGE_OUTPUT_ENABLED", "1") != "0"
LARGE_OUTPUT_ARM = "forced" if ENABLE_LARGE_OUTPUT else "default"


def settings_meta() -> dict[str, Any]:
    return (
        {"_meta": {"kiro": {"settings": {"largeToolOutputHandler": {"enabled": True}}}}}
        if ENABLE_LARGE_OUTPUT
        else {}
    )

SECRET_KEY = re.compile(
    r"(?:access.?token|refresh.?token|id.?token|authorization|password|secret|api.?key|private.?key|credential|cookie|profile.?arn|expires.?at)",
    re.I,
)
TOKEN_TEXT = re.compile(
    r"(?:Bearer\s+|ksk_[A-Za-z0-9._-]+|AKIA[A-Z0-9]+|eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)"
)
UUID_TEXT = re.compile(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b", re.I)


def scrub(value: Any, key: str = "") -> Any:
    if key and SECRET_KEY.search(key):
        return "<redacted>"
    if isinstance(value, dict):
        return {str(k): scrub(v, str(k)) for k, v in value.items()}
    if isinstance(value, list):
        return [scrub(v, key) for v in value]
    if isinstance(value, str):
        return TOKEN_TEXT.sub("<redacted>", value)
    return value


def dynamic(value: Any, key: str = "") -> Any:
    if isinstance(value, dict):
        return {str(k): dynamic(v, str(k)) for k, v in value.items()}
    if isinstance(value, list):
        return [dynamic(v, key) for v in value]
    if isinstance(value, str):
        if key in {
            "sessionId",
            "toolCallId",
            "messageId",
            "requestId",
            "replayId",
            "workflowId",
            "nodeId",
            "branchId",
            "createdAt",
            "updatedAt",
            "lastModifiedAt",
            "expiresAt",
        }:
            return f"<{key}>"
        value = UUID_TEXT.sub("<uuid>", value)
        return re.sub(r"/tmp/kas-output-replay-[^/\s]+", "/<workspace>", value)
    return value


def all_paths(value: Any, prefix: str = "") -> set[str]:
    out: set[str] = set()
    if isinstance(value, dict):
        for key, child in value.items():
            path = f"{prefix}.{key}" if prefix else str(key)
            out.add(path)
            out |= all_paths(child, path)
    elif isinstance(value, list):
        for child in value[:8]:
            out |= all_paths(child, prefix + "[]")
    return out


def response_class(frame: dict[str, Any] | None) -> dict[str, Any]:
    if frame is None:
        return {"kind": "timeout"}
    if "error" in frame:
        return {
            "kind": "error",
            "code": frame["error"].get("code"),
            "message": frame["error"].get("message"),
            "data": dynamic(scrub(frame["error"].get("data"))),
        }
    return {"kind": "result", "result": dynamic(scrub(frame.get("result")))}


def read_token() -> dict[str, Any]:
    """Read the installed login only long enough to answer KAS auth."""
    conn = sqlite3.connect(AUTH_DB)
    try:
        row = conn.execute(
            "select value from auth_kv where key in "
            "('kirocli:odic:token','kirocli:social:token') order by key desc"
        ).fetchone()
        profile = conn.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        conn.close()
    if row is None:
        return {}
    raw = row[0]
    raw = raw.decode() if isinstance(raw, (bytes, bytearray)) else raw
    auth = json.loads(raw)
    profile_arn = auth.get("profile_arn")
    if not profile_arn and profile:
        profile_raw = profile[0]
        profile_raw = profile_raw.decode() if isinstance(profile_raw, (bytes, bytearray)) else profile_raw
        try:
            profile_arn = json.loads(profile_raw).get("arn")
        except (TypeError, ValueError):
            profile_arn = None
    return {
        "accessToken": auth.get("access_token"),
        "expiresAt": auth.get("expires_at"),
        "profileArn": profile_arn,
    }


def short_leaf(value: Any, path: str = "") -> list[dict[str, Any]]:
    """Summarize string leaves while retaining marker and full-length evidence."""
    found: list[dict[str, Any]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}.{key}" if path else str(key)
            found.extend(short_leaf(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value[:8]):
            found.extend(short_leaf(child, f"{path}[{index}]"))
    elif isinstance(value, str) and (OUTPUT_PREFIX in value or OUTPUT_END in value):
        found.append(
            {
                "path": path,
                "length": len(value),
                "contains_expected_body": OUTPUT_BODY in value or OUTPUT_BODY.rstrip("\n") in value,
                "starts_with_expected": value.startswith(OUTPUT_BODY) or value.startswith(OUTPUT_BODY.rstrip("\n")),
                "ends_with_expected": value.endswith(OUTPUT_BODY) or value.endswith(OUTPUT_BODY.rstrip("\n")),
                "has_unique_end_marker": OUTPUT_END in value,
                "head": value[:80],
                "tail": value[-80:],
            }
        )
    return found


def sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def inspect_disk(home: str, cwd: str) -> dict[str, Any]:
    """Inspect the persisted storage itself, not only ACP/model output."""
    roots = [home, cwd]
    tool_dirs: list[dict[str, Any]] = []
    marker_files: list[dict[str, Any]] = []
    visited: set[str] = set()
    for root in roots:
        for directory, dirs, files in os.walk(root, followlinks=False):
            dirs[:] = [d for d in dirs if not os.path.islink(os.path.join(directory, d))]
            rel_directory = os.path.relpath(directory, root)
            if "tool-output" in directory.lower():
                entries = []
                for name in sorted(files):
                    path = os.path.join(directory, name)
                    try:
                        stat = os.stat(path, follow_symlinks=False)
                    except OSError:
                        continue
                    entries.append({"name": name, "bytes": stat.st_size})
                tool_dirs.append({"root": root, "relative": rel_directory, "entries": entries})
            for name in files:
                path = os.path.join(directory, name)
                if path in visited or os.path.islink(path):
                    continue
                visited.add(path)
                try:
                    stat = os.stat(path, follow_symlinks=False)
                    if stat.st_size > 8 * 1024 * 1024:
                        continue
                    data = Path(path).read_bytes()
                except OSError:
                    continue
                if OUTPUT_END.encode() not in data and OUTPUT_PREFIX.encode() not in data:
                    continue
                text = data.decode("utf-8", errors="replace")
                marker_files.append(
                    {
                        "root": root,
                        "relative": os.path.relpath(path, root),
                        "bytes": len(data),
                        "sha256": hashlib.sha256(data).hexdigest(),
                        "contains_expected_body": OUTPUT_BODY in text or OUTPUT_BODY.rstrip("\n") in text,
                        "starts_with_expected": text.startswith(OUTPUT_BODY) or text.startswith(OUTPUT_BODY.rstrip("\n")),
                        "ends_with_expected": text.endswith(OUTPUT_BODY) or text.endswith(OUTPUT_BODY.rstrip("\n")),
                        "has_unique_end_marker": OUTPUT_END in text,
                        "head": text[:80],
                        "tail": text[-80:],
                    }
                )
    return {"tool_output_directories": tool_dirs, "marker_files": marker_files}


class Leg:
    def __init__(self, binary: str, label: str, pin: str):
        self.binary = binary
        self.label = label
        self.pin = pin
        self.out_path = f"{PREFIX}-{label}.jsonl"
        self.err_path = f"{PREFIX}-{label}-stderr.log"
        self.out = open(self.out_path, "w", encoding="utf-8")
        self.q: queue.Queue[str | None] = queue.Queue()
        self.frames: list[dict[str, Any]] = []
        self.parse_errors = 0
        self.requests: dict[int, str] = {}
        self.next_id = 0
        self.permissions: list[dict[str, Any]] = []
        self.server_requests: list[str] = []
        self.terminals: dict[str, dict[str, Any]] = {}
        self.next_terminal_id = 0
        self.phase = "startup"
        self.cwd = tempfile.mkdtemp(prefix="kas-output-replay-cwd-")
        self.home = tempfile.mkdtemp(prefix="kas-output-replay-home-")
        self.runtime = tempfile.mkdtemp(prefix="kas-output-replay-runtime-")
        self.env = dict(os.environ)
        self.env["HOME"] = self.home
        self.env["XDG_DATA_HOME"] = REAL_XDG
        self.env["TMPDIR"] = self.runtime
        self.env["KIRO_KAS_SERVER_PATH"] = self.pin
        Path(self.cwd, "README.txt").write_text("KAS_OUTPUT_REPLAY_2211_FILE_FIXTURE\n", encoding="utf-8")
        self.proc: subprocess.Popen[str] | None = None
        self.start_process()

    def start_process(self) -> None:
        self.q = queue.Queue()
        stderr = open(self.err_path, "a", encoding="utf-8")
        proc = subprocess.Popen(
            [self.binary, "acp", "--agent-engine", "kas"],
            cwd=self.cwd,
            env=self.env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=stderr,
            text=True,
            bufsize=1,
            start_new_session=True,
        )
        self.proc = proc
        assert proc.stdout is not None

        def read_stdout() -> None:
            assert proc.stdout is not None
            for line in proc.stdout:
                if line.strip():
                    self.q.put(line.strip())
            self.q.put(None)

        threading.Thread(target=read_stdout, daemon=True, name=f"kas-replay-{self.label}-reader").start()

    def record(self, direction: str, message: dict[str, Any], auth_reply: bool = False) -> None:
        safe = {"jsonrpc": "2.0", "id": message.get("id"), "method": message.get("method")} if auth_reply else scrub(message)
        record = {"seq": len(self.frames), "ts": time.time(), "phase": self.phase, "dir": direction, "msg": safe}
        self.frames.append(record)
        self.out.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")
        self.out.flush()

    def send(self, message: dict[str, Any], auth_reply: bool = False) -> None:
        self.record("client->agent", message, auth_reply=auth_reply)
        if self.proc is None or self.proc.stdin is None:
            return
        try:
            self.proc.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
            self.proc.stdin.flush()
        except (BrokenPipeError, OSError):
            return

    def request(self, method: str, params: dict[str, Any]) -> int:
        self.next_id += 1
        self.requests[self.next_id] = method
        self.send({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        return self.next_id

    def handle_server_request(self, message: dict[str, Any]) -> None:
        method = str(message.get("method"))
        self.server_requests.append(method)
        params = message.get("params") or {}
        if method == "_kiro/auth/getAccessToken":
            reply = {"jsonrpc": "2.0", "id": message.get("id"), "result": read_token()}
            self.send(reply, auth_reply=True)
        elif method == "_kiro/terminal/shell_type":
            self.send({"jsonrpc": "2.0", "id": message.get("id"), "result": {"shellType": "bash"}})
        elif method == "terminal/create":
            self.next_terminal_id += 1
            terminal_id = f"term-{self.next_terminal_id}"
            child_env = dict(self.env)
            for entry in params.get("env") or []:
                if isinstance(entry, dict) and entry.get("name"):
                    child_env[str(entry["name"])] = str(entry.get("value", ""))
            command = str(params.get("command", ""))
            cwd = str(params.get("cwd") or self.cwd)
            try:
                child = subprocess.Popen(
                    ["bash", "-lc", command],
                    cwd=cwd,
                    env=child_env,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    start_new_session=True,
                )
                try:
                    output, _ = child.communicate(timeout=180)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(os.getpgid(child.pid), signal.SIGKILL)
                    except (ProcessLookupError, OSError):
                        pass
                    output, _ = child.communicate()
                exit_code = child.returncode
            except (OSError, subprocess.SubprocessError) as exc:
                output = f"(host error: {type(exc).__name__}: {exc})"
                exit_code = 1
            self.terminals[terminal_id] = {"output": output, "exitCode": exit_code, "signal": None}
            self.send({"jsonrpc": "2.0", "id": message.get("id"), "result": {"terminalId": terminal_id}})
        elif method == "terminal/wait_for_exit":
            terminal = self.terminals.get(str(params.get("terminalId")), {"exitCode": 1, "signal": None})
            self.send(
                {
                    "jsonrpc": "2.0",
                    "id": message.get("id"),
                    "result": {"exitCode": terminal["exitCode"], "signal": terminal["signal"]},
                }
            )
        elif method == "terminal/output":
            terminal = self.terminals.get(
                str(params.get("terminalId")), {"output": "", "exitCode": 1, "signal": None}
            )
            self.send(
                {
                    "jsonrpc": "2.0",
                    "id": message.get("id"),
                    "result": {
                        "output": terminal["output"],
                        "truncated": False,
                        "exitStatus": {"exitCode": terminal["exitCode"], "signal": terminal["signal"]},
                    },
                }
            )
        elif method in ("terminal/release", "terminal/kill"):
            self.terminals.pop(str(params.get("terminalId")), None)
            self.send({"jsonrpc": "2.0", "id": message.get("id"), "result": {}})
        elif method == "session/request_permission":
            self.permissions.append(scrub(params))
            options = params.get("options") or []
            selected = next(
                (option for option in options if "allow" in (str(option.get("kind", "")) + str(option.get("optionId", ""))).lower()),
                options[0] if options else None,
            )
            result = {"outcome": {"outcome": "selected", "optionId": selected.get("optionId")}} if selected else {"outcome": {"outcome": "cancelled"}}
            self.send({"jsonrpc": "2.0", "id": message.get("id"), "result": result})
        else:
            self.send({"jsonrpc": "2.0", "id": message.get("id"), "result": {}})

    def consume(self, raw: str) -> dict[str, Any] | None:
        try:
            message = json.loads(raw)
        except json.JSONDecodeError:
            self.parse_errors += 1
            return None
        self.record("agent->client", message)
        if message.get("method") and "id" in message:
            self.handle_server_request(message)
            return None
        return message

    def wait_for(self, request_id: int, timeout: float) -> dict[str, Any] | None:
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                raw = self.q.get(timeout=min(2.0, max(0.05, deadline - time.time())))
            except queue.Empty:
                continue
            if raw is None:
                return None
            message = self.consume(raw)
            if message and message.get("id") == request_id and ("result" in message or "error" in message):
                return message
        return None

    def drain(self, seconds: float) -> None:
        deadline = time.time() + seconds
        while time.time() < deadline:
            try:
                raw = self.q.get(timeout=min(0.5, max(0.02, deadline - time.time())))
            except queue.Empty:
                continue
            if raw is None:
                return
            self.consume(raw)

    def close(self, finalize: bool = False) -> int | None:
        try:
            if self.proc and self.proc.stdin:
                self.proc.stdin.close()
        except OSError:
            pass
        if self.proc and self.proc.poll() is None:
            try:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
                self.proc.wait(timeout=10)
            except (ProcessLookupError, OSError, subprocess.TimeoutExpired):
                try:
                    if self.proc.poll() is None:
                        os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
                except (ProcessLookupError, OSError):
                    pass
                try:
                    self.proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    pass
        if finalize:
            self.out.close()
        return self.proc.poll() if self.proc else None

    def field_paths(self) -> set[str]:
        paths: set[str] = set()
        for frame in self.frames:
            if frame["dir"] == "agent->client":
                paths |= all_paths(frame["msg"])
        return paths


def session_update(frame: dict[str, Any]) -> dict[str, Any]:
    message = frame.get("msg") or {}
    if message.get("method") != "session/update":
        return {}
    return (message.get("params") or {}).get("update") or {}


def response_payloads(leg: Leg, phase: str) -> list[dict[str, Any]]:
    payloads: list[dict[str, Any]] = []
    for frame in leg.frames:
        if frame["phase"] != phase or frame["dir"] != "agent->client":
            continue
        message = frame["msg"]
        update = session_update(frame)
        kind = update.get("sessionUpdate")
        if kind in {"tool_call", "tool_call_update"} or message.get("method") == "_kiro/tools/content_chunk":
            payloads.append(message)
    return payloads


def card_index(leg: Leg, phase: str) -> dict[str, dict[str, Any]]:
    cards: dict[str, dict[str, Any]] = {}
    for frame in leg.frames:
        if frame["phase"] != phase or frame["dir"] != "agent->client":
            continue
        update = session_update(frame)
        if update.get("sessionUpdate") not in {"tool_call", "tool_call_update"}:
            continue
        tool_id = update.get("toolCallId")
        if not tool_id:
            continue
        kiro_meta = ((update.get("_meta") or {}).get("kiro") or {})
        event = {
            "sessionUpdate": update.get("sessionUpdate"),
            "toolCallId": tool_id,
            "title": update.get("title"),
            "kind": update.get("kind"),
            "status": update.get("status"),
            "rawInput": dynamic(scrub(update.get("rawInput"))),
            "rawOutput": short_leaf(update.get("rawOutput"), "rawOutput"),
            "content": short_leaf(update.get("content"), "content"),
            "metaKeys": sorted((update.get("_meta") or {}).keys()),
            "outputTransformation": scrub(kiro_meta.get("outputTransformation")),
        }
        entry = cards.setdefault(tool_id, {"events": []})
        entry["events"].append(event)
        entry["first"] = entry["events"][0]
        entry["last"] = event
    return cards


def wire_observations(leg: Leg, phase: str) -> dict[str, Any]:
    observations: list[dict[str, Any]] = []
    for message in response_payloads(leg, phase):
        update = ((message.get("params") or {}).get("update") or {})
        if message.get("method") == "_kiro/tools/content_chunk":
            payload = (message.get("params") or {}).get("content")
            observations.extend(short_leaf(payload, "params.content"))
        else:
            observations.extend(short_leaf(update.get("rawOutput"), "update.rawOutput"))
            observations.extend(short_leaf(update.get("content"), "update.content"))
    return {
        "marker_observations": observations,
        "contains_full_expected_body": any(x["contains_expected_body"] for x in observations),
        "max_marker_string_length": max((x["length"] for x in observations), default=0),
        "payload_count": len(response_payloads(leg, phase)),
    }

def output_transformations(leg: Leg, phase: str) -> list[dict[str, Any]]:
    transformations: list[dict[str, Any]] = []
    for frame in leg.frames:
        if frame["phase"] != phase or frame["dir"] != "agent->client":
            continue
        update = session_update(frame)
        transformation = (((update.get("_meta") or {}).get("kiro") or {}).get("outputTransformation"))
        if transformation:
            transformations.append(
                {"toolCallId": update.get("toolCallId"), "transformation": scrub(transformation)}
            )
    return transformations


def run_leg(binary: str, pin: str, label: str) -> dict[str, Any]:
    leg = Leg(binary, label, pin)
    result: dict[str, Any] = {
        "label": label,
        "large_output_arm": LARGE_OUTPUT_ARM,
        "binary": binary,
        "kas_pin": pin,
        "cwd": leg.cwd,
        "home": leg.home,
        "runtime": leg.runtime,
        "assertions": {},
        "original": {},
        "load": {},
    }
    sid: str | None = None
    process_exits: list[int | None] = []
    try:
        initialize_id = leg.request(
            "initialize",
            {
                "protocolVersion": 1,
                "clientInfo": {"name": "cyril-kas-output-replay-2.21.1", "version": "1"},
                "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True},
                **settings_meta(),
            },
        )
        initialize = leg.wait_for(initialize_id, 90)
        result["initialize"] = response_class(initialize)
        result["initialize_fields"] = sorted(all_paths((initialize or {}).get("result") or {}))
        result["assertions"]["initialize_ok"] = bool(initialize and "result" in initialize)

        new_id = leg.request(
            "session/new",
            {
                "cwd": leg.cwd,
                "mcpServers": [],
                **settings_meta(),
            },
        )
        session_new = leg.wait_for(new_id, 120)
        new_result = (session_new or {}).get("result") or {}
        sid = new_result.get("sessionId")
        result["session_new"] = response_class(session_new)
        result["assertions"]["session_new_ok"] = bool(sid)
        leg.drain(5)

        command = "python3 -c 'import sys;sys.stdout.write(\"OUTPUT_REPLAY_2211_\" + (\"X\" * 32000) + \"_OUTPUT_REPLAY_2211_ENDMARK\\n\")'"
        prompt = (
            "Use execute_bash exactly once, with no other tools, and run this exact command verbatim: "
            + command
            + " Do not alter the command or shorten its output. Wait for completion, then reply exactly DONE_REPLAY_2211."
        )
        leg.phase = "original"
        prompt_id = leg.request("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
        original_response = leg.wait_for(prompt_id, 420)
        leg.drain(10)
        result["original"] = {
            "prompt": prompt,
            "response": response_class(original_response),
            "wire": wire_observations(leg, "original"),
            "tool_cards": card_index(leg, "original"),
            "output_transformations": output_transformations(leg, "original"),
            "permission_count": len(leg.permissions),
        }
        result["assertions"]["original_prompt_ok"] = bool(original_response and "result" in original_response)
        result["assertions"]["original_marker_on_wire"] = result["original"]["wire"]["payload_count"] > 0 and bool(result["original"]["wire"]["marker_observations"])
        result["assertions"]["original_offloaded"] = any(
            item["transformation"].get("kind") == "offloaded"
            for item in result["original"]["output_transformations"]
        )
        result["original"]["disk_after_turn"] = inspect_disk(leg.home, leg.cwd)
        result["assertions"]["original_marker_on_disk"] = any(
            item["contains_expected_body"] for item in result["original"]["disk_after_turn"]["marker_files"]
        )
        result["original"]["session_id"] = sid
        result["original"]["expected_output_bytes"] = len(OUTPUT_BODY.encode())
        process_exits.append(leg.close())

        # Relaunch against the same persisted temporary HOME/workspace. This is
        # the supported ACP resume path, not a model prompt asking it to recall.
        leg.phase = "load"
        leg.start_process()
        reload_init_id = leg.request(
            "initialize",
            {
                "protocolVersion": 1,
                "clientInfo": {"name": "cyril-kas-output-replay-2.21.1", "version": "1"},
                "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True},
            },
        )
        reload_init = leg.wait_for(reload_init_id, 90)
        list_id = leg.request("session/list", {})
        session_list = leg.wait_for(list_id, 90)
        load_id = leg.request("session/load", {"sessionId": sid, "cwd": leg.cwd, "mcpServers": []})
        load_response = leg.wait_for(load_id, 420)
        leg.drain(12)
        result["load"] = {
            "initialize": response_class(reload_init),
            "session_list": response_class(session_list),
            "response": response_class(load_response),
            "wire": wire_observations(leg, "load"),
            "tool_cards": card_index(leg, "load"),
            "output_transformations": output_transformations(leg, "load"),
        }
        result["assertions"]["load_response_ok"] = bool(load_response and "result" in load_response)
        result["assertions"]["load_replayed_updates"] = bool(response_payloads(leg, "load"))
        process_exits.append(leg.close(finalize=True))

        original_cards = result["original"]["tool_cards"]
        replay_cards = result["load"]["tool_cards"]
        shared_ids = sorted(set(original_cards) & set(replay_cards))
        result["replay_comparison"] = {
            "original_tool_ids": sorted(original_cards),
            "replayed_tool_ids": sorted(replay_cards),
            "shared_tool_ids": shared_ids,
            "all_original_ids_replayed": bool(original_cards) and set(original_cards) <= set(replay_cards),
            "cards": {
                tool_id: {
                    "original_first": original_cards[tool_id]["first"],
                    "replayed_first": replay_cards[tool_id]["first"],
                    "title_equal": original_cards[tool_id]["first"].get("title") == replay_cards[tool_id]["first"].get("title"),
                    "raw_input_equal": original_cards[tool_id]["first"].get("rawInput") == replay_cards[tool_id]["first"].get("rawInput"),
                }
                for tool_id in shared_ids
            },
        }
        result["assertions"]["replayed_tool_card_ids"] = result["replay_comparison"]["all_original_ids_replayed"]
        result["assertions"]["replayed_titles_equal"] = bool(shared_ids) and all(
            item["title_equal"] for item in result["replay_comparison"]["cards"].values()
        )
        result["assertions"]["replayed_raw_inputs_equal"] = bool(shared_ids) and all(
            item["raw_input_equal"] for item in result["replay_comparison"]["cards"].values()
        )
    except Exception as exc:
        result["fatal_error"] = f"{type(exc).__name__}: {exc}"
        try:
            process_exits.append(leg.close(finalize=True))
        except Exception:
            pass
    finally:
        result["process_exits"] = process_exits
        result["server_requests"] = sorted(set(leg.server_requests))
        result["permission_requests"] = leg.permissions
        result["parse_errors"] = leg.parse_errors
        result["field_paths"] = sorted(leg.field_paths())
        result["frame_count"] = len(leg.frames)
    return result


def main() -> None:
    old = run_leg(OLD, OLD_PIN, "old")
    new = run_leg(NEW, NEW_PIN, "new")
    old_fields, new_fields = set(old.get("field_paths", [])), set(new.get("field_paths", []))
    verdict = {
        "schema": "cyril.kas-output-replay-audit.2.21.1",
        "large_output_arm": LARGE_OUTPUT_ARM,
        "same_workload": True,
        "same_day_backend_control": True,
        "large_output": {
            "command_output_bytes": len(OUTPUT_BODY.encode()),
            "historical_wire_cap_bytes": 30000,
            "unique_end_marker": OUTPUT_END,
            "command": "python3 -c '<marker prefix> + (X * 32000) + <unique end marker>'",
        },
        "old": old,
        "new": new,
        "comparison": {
            "old_field_count": len(old_fields),
            "new_field_count": len(new_fields),
            "only_old_fields": sorted(old_fields - new_fields),
            "only_new_fields": sorted(new_fields - old_fields),
            "field_sets_identical": old_fields == new_fields,
            "tool_ids_same": old.get("replay_comparison", {}).get("original_tool_ids") == new.get("replay_comparison", {}).get("original_tool_ids"),
            "original_disk_marker_old": old.get("assertions", {}).get("original_marker_on_disk", False),
            "original_disk_marker_new": new.get("assertions", {}).get("original_marker_on_disk", False),
            "replayed_tool_cards_old": old.get("assertions", {}).get("replayed_tool_card_ids", False),
            "replayed_tool_cards_new": new.get("assertions", {}).get("replayed_tool_card_ids", False),
        },
        "provider_refusal": {
            "status": "gap_not_live_triggered",
            "reason": "No safe deterministic provider-refusal trigger was available; the probe did not send abusive or content-policy prompts.",
            "static_precise_mapping": {
                "release_claim": "Surface provider refusal reasons instead of a generic failed-to-generate-a-response error.",
                "host_error_types": ["chat_cli::api_client::error::ConverseStreamError", "chat_cli_v2::api_client::error::ConverseStreamError", "ConverseStreamErrorKind"],
                "host_mapping_markers": ["ConverseStreamError::error_code", "ConverseStreamError -> agent_loop::StreamError", "ConverseStreamError as telemetry::ReasonCode"],
                "evidence": "static-2.21.1/static-2.21.1-report.json lines 162, 271, 430-433, 1626-1628, 2320, 2370-2371, 9515-9519; release text is embedded in 2.21.1 kiro-cli strings.",
                "limit": "Stripped binary symbols identify the typed error and reason-code branches but not a safe backend refusal payload; live reason/category/explanation values remain unobserved.",
            },
        },
        "limitations": [
            "Tool output and model wording are backend-dependent; raw ACP observations are kept separately from model final text.",
            "Temporary HOME/workspace roots are retained in captures only as paths; auth replies are represented by redacted callback records.",
            "A session/load response is only counted as replay when replay-phase notifications and the supported load response are both captured.",
        ],
    }
    verdict_path = f"{PREFIX}-verdict.json"
    with open(verdict_path, "w", encoding="utf-8") as stream:
        json.dump(verdict, stream, indent=2, ensure_ascii=False)
        stream.write("\n")
    print(
        json.dumps(
            {
                "verdict": verdict_path,
                "old_frames": old.get("frame_count"),
                "new_frames": new.get("frame_count"),
                "disk_marker": {"old": old.get("assertions", {}).get("original_marker_on_disk"), "new": new.get("assertions", {}).get("original_marker_on_disk")},
                "replay_cards": {"old": old.get("assertions", {}).get("replayed_tool_card_ids"), "new": new.get("assertions", {}).get("replayed_tool_card_ids")},
                "only_new_fields": verdict["comparison"]["only_new_fields"],
                "only_old_fields": verdict["comparison"]["only_old_fields"],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

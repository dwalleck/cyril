#!/usr/bin/env python3
"""Paired live ACP/v2 wire audit for Kiro 2.21.1 versus 2.21.0.

This is intentionally a raw JSON-RPC probe: each binary is run directly against
one same-day backend, with a private HOME and disposable workspace.  The JSONL
captures contain every sanitized frame in order.  No Rust bridge or installed
binary is modified.

Usage:
  probe-v2-live-ab-2.21.1.py OLD_KIRO_CHAT NEW_KIRO_CHAT OUTPUT_PREFIX

The output prefix is used for ``-old.jsonl``, ``-new.jsonl``, stderr files, and
``-verdict.json``.  Authentication callback values and token-like strings are
redacted before they reach a capture.
"""
from __future__ import annotations

import hashlib
import json
import os
import queue
import re
import signal
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

SENSITIVE_KEY = re.compile(
    r"(?:access.?token|refresh.?token|id.?token|authorization|password|secret|api.?key|private.?key|credential|cookie)",
    re.I,
)
TOKEN_TEXT = re.compile(r"(?:Bearer\s+|ksk_[A-Za-z0-9._-]+|AKIA[A-Z0-9]+|eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)")
UUID_TEXT = re.compile(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b", re.I)


def scrub(value: Any, key: str = "") -> Any:
    """Redact credentials before storing or printing a frame."""
    if key and SENSITIVE_KEY.search(key):
        return "<redacted>"
    if isinstance(value, dict):
        return {str(k): scrub(v, str(k)) for k, v in value.items()}
    if isinstance(value, list):
        return [scrub(v, key) for v in value]
    if isinstance(value, str):
        if TOKEN_TEXT.search(value):
            return TOKEN_TEXT.sub("<redacted>", value)
        return value
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


def redact_dynamic(value: Any, key: str = "") -> Any:
    """Normalize run-specific IDs/timestamps while retaining values of interest."""
    if isinstance(value, dict):
        return {k: redact_dynamic(v, k) for k, v in value.items()}
    if isinstance(value, list):
        return [redact_dynamic(v, key) for v in value]
    if isinstance(value, str):
        if key in {
            "sessionId", "toolCallId", "messageId", "requestId", "replayId",
            "workflowId", "nodeId", "parentSessionId", "branchId", "updatedAt",
            "createdAt", "lastModifiedAt", "expiresAt", "cwd",
        }:
            return f"<{key}>"
        value = UUID_TEXT.sub("<uuid>", value)
        # Workspace paths are private random temp directories and are not a wire change.
        value = re.sub(r"/(?:tmp|var/tmp)/v2-audit-[^/\s]+", "/<workspace>", value)
        return value
    return value


def response_class(frame: dict[str, Any] | None) -> dict[str, Any]:
    if frame is None:
        return {"kind": "timeout"}
    if "error" in frame:
        error = frame["error"]
        return {
            "kind": "error",
            "code": error.get("code"),
            "message": error.get("message"),
            "data": redact_dynamic(error.get("data")),
        }
    return {"kind": "result", "result": redact_dynamic(frame.get("result"))}


class Leg:
    def __init__(self, binary: str, label: str):
        self.binary = binary
        self.label = label
        self.out_path = f"{PREFIX}-{label}.jsonl"
        self.err_path = f"{PREFIX}-{label}-stderr.log"
        self.out = open(self.out_path, "w", encoding="utf-8")
        self.q: queue.Queue[str | None] = queue.Queue()
        self.frames: list[dict[str, Any]] = []
        self.req_method: dict[int, str] = {}
        self.next_id = 0
        self.dead = False
        self.cwd = tempfile.mkdtemp(prefix="v2-audit-", dir="/tmp")
        Path(self.cwd, "input.txt").write_text("V2_AUDIT_2211_FILE_MARKER\n", encoding="utf-8")
        self.env = dict(os.environ)
        self.env["HOME"] = tempfile.mkdtemp(prefix=f"v2-audit-home-{label}-")
        # Keep the installed auth/data store reachable while relocating Kiro logs/state.
        self.env["XDG_DATA_HOME"] = REAL_XDG
        self.env["TMPDIR"] = tempfile.mkdtemp(prefix=f"v2-audit-runtime-{label}-")
        self.proc: subprocess.Popen[str] | None = None
        self.started = time.time()
        self.permissions: list[dict[str, Any]] = []
        self.server_requests: list[str] = []
        self.start_process()

    def start_process(self) -> None:
        stderr = open(self.err_path, "w", encoding="utf-8")
        self.proc = subprocess.Popen(
            [self.binary, "acp", "--agent-engine", "v2"],
            cwd=self.cwd,
            env=self.env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=stderr,
            text=True,
            bufsize=1,
            start_new_session=True,
        )
        assert self.proc.stdout is not None

        def read_stdout() -> None:
            assert self.proc is not None and self.proc.stdout is not None
            for line in self.proc.stdout:
                if line.strip():
                    self.q.put(line.strip())
            self.q.put(None)

        threading.Thread(target=read_stdout, daemon=True, name=f"v2-{self.label}-reader").start()

    def record(self, direction: str, message: dict[str, Any]) -> None:
        safe = scrub(message)
        record = {"seq": len(self.frames), "ts": time.time(), "dir": direction, "msg": safe}
        self.frames.append(record)
        self.out.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")
        self.out.flush()

    def send(self, message: dict[str, Any]) -> None:
        self.record("client->agent", message)
        if self.proc is None or self.proc.stdin is None:
            self.dead = True
            return
        try:
            self.proc.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
            self.proc.stdin.flush()
        except (BrokenPipeError, OSError):
            self.dead = True

    def request(self, method: str, params: dict[str, Any]) -> int:
        self.next_id += 1
        self.req_method[self.next_id] = method
        self.send({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        return self.next_id

    def handle_server_request(self, message: dict[str, Any]) -> None:
        method = str(message.get("method"))
        self.server_requests.append(method)
        params = message.get("params") or {}
        if method == "session/request_permission":
            self.permissions.append(scrub(params))
            options = params.get("options") or []
            selected = next(
                (option for option in options if "allow" in str(option.get("kind", "")).lower()),
                options[0] if options else None,
            )
            if selected:
                result = {"outcome": {"outcome": "selected", "optionId": selected.get("optionId")}}
            else:
                result = {"outcome": {"outcome": "cancelled"}}
        elif method == "_kiro/auth/getAccessToken":
            # Direct v2 normally uses the local CLI credential path. Never invent or
            # capture a credential if a callback unexpectedly appears.
            result = {"error": {"code": -32001, "message": "probe refuses auth callback"}}
        else:
            # v2 host callbacks seen by older probes are params-free acknowledgements.
            result = {"result": {}}
        # RequestPermissionResponse is itself the JSON-RPC result object. The
        # previous probe accidentally emitted outcome at the JSON-RPC envelope
        # level, which Kiro correctly parsed as null and rejected.
        if method == "session/request_permission":
            reply = {"jsonrpc": "2.0", "id": message.get("id"), "result": result}
        else:
            reply = {"jsonrpc": "2.0", "id": message.get("id"), **result}
        self.send(reply)

    def read_until(self, wanted: set[int], timeout: float) -> dict[int, dict[str, Any]]:
        found: dict[int, dict[str, Any]] = {}
        end = time.time() + timeout
        while time.time() < end and wanted - found.keys():
            try:
                raw = self.q.get(timeout=min(1.0, max(0.05, end - time.time())))
            except queue.Empty:
                continue
            if raw is None:
                self.dead = True
                break
            try:
                message = json.loads(raw)
            except json.JSONDecodeError:
                continue
            self.record("agent->client", message)
            if "method" in message and "id" in message:
                self.handle_server_request(message)
            elif "id" in message and ("result" in message or "error" in message):
                if message["id"] in wanted:
                    found[message["id"]] = message
        return found

    def drain(self, seconds: float = 2.0) -> None:
        end = time.time() + seconds
        while time.time() < end:
            try:
                raw = self.q.get(timeout=min(0.2, max(0.01, end - time.time())))
            except queue.Empty:
                continue
            if raw is None:
                self.dead = True
                return
            try:
                message = json.loads(raw)
            except json.JSONDecodeError:
                continue
            self.record("agent->client", message)
            if "method" in message and "id" in message:
                self.handle_server_request(message)

    def close(self) -> None:
        try:
            if self.proc and self.proc.stdin:
                self.proc.stdin.close()
        except OSError:
            pass
        if self.proc and self.proc.poll() is None:
            try:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
                self.proc.wait(timeout=5)
            except (ProcessLookupError, subprocess.TimeoutExpired, OSError):
                try:
                    os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
                except (ProcessLookupError, OSError):
                    pass
                try:
                    self.proc.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    pass
        self.out.close()

    def notification_keys(self) -> list[str]:
        keys: list[str] = []
        for frame in self.frames:
            if frame["dir"] != "agent->client":
                continue
            message = frame["msg"]
            method = message.get("method") or "response"
            if method == "session/update":
                update = (message.get("params") or {}).get("update") or {}
                method += ":" + str(update.get("sessionUpdate"))
            keys.append(method)
        return keys

    def field_paths(self) -> set[str]:
        result: set[str] = set()
        for frame in self.frames:
            if frame["dir"] != "agent->client":
                continue
            message = frame["msg"]
            payload = message.get("params") if "params" in message else (message.get("result") or message.get("error") or {})
            result |= all_paths(payload)
        return result

    def version(self) -> str:
        run = subprocess.run([self.binary, "--version"], env=self.env, text=True, capture_output=True, timeout=30)
        return (run.stdout or run.stderr).strip()

    def sha256(self) -> str:
        digest = hashlib.sha256()
        with open(self.binary, "rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        return digest.hexdigest()


def session_update(frame: dict[str, Any]) -> dict[str, Any]:
    message = frame.get("msg") or {}
    if message.get("method") not in {"session/update", "_kiro.dev/session/update"}:
        return {}
    return (message.get("params") or {}).get("update") or {}


def run_leg(binary: str, label: str) -> dict[str, Any]:
    leg = Leg(binary, label)
    result: dict[str, Any] = {
        "label": label,
        "binary": binary,
        "version": leg.version(),
        "sha256": leg.sha256(),
        "cwd": leg.cwd,
        "assertions": {},
        "extension": {},
        "workload": {},
    }
    sid: str | None = None
    try:
        initialize_id = leg.request(
            "initialize",
            {
                "protocolVersion": 1,
                "clientInfo": {"name": "cyril-v2-2.21.1-audit", "version": "1"},
                "clientCapabilities": {},
            },
        )
        initialize = leg.read_until({initialize_id}, 90).get(initialize_id)
        result["initialize"] = response_class(initialize)
        initialize_result = (initialize or {}).get("result") or {}
        result["initialize_capabilities"] = redact_dynamic(initialize_result)
        result["assertions"]["initialize_result"] = initialize is not None and "result" in initialize

        new_id = leg.request("session/new", {"cwd": leg.cwd, "mcpServers": []})
        session_new = leg.read_until({new_id}, 120).get(new_id)
        result["session_new"] = response_class(session_new)
        new_result = (session_new or {}).get("result") or {}
        sid = new_result.get("sessionId")
        result["session_new_keys"] = sorted(new_result.keys())
        result["model_surface"] = redact_dynamic(new_result.get("models"))
        result["mode_surface"] = redact_dynamic(new_result.get("modes"))
        result["config_surface"] = redact_dynamic(new_result.get("configOptions"))
        result["assertions"]["session_new_session_id"] = bool(sid)
        leg.drain(5)

        # Full advertised option surface used by prior v2 audits. Invalid is a
        # negative control; all calls are read-only except a model reset to auto.
        option_results: dict[str, Any] = {}
        for command in ("model", "mode", "effort", "bogus"):
            rid = leg.request("_kiro.dev/commands/options", {"sessionId": sid, "command": command})
            option_results[command] = response_class(leg.read_until({rid}, 60).get(rid))
        result["command_options"] = option_results
        config_id = leg.request(
            "session/set_config_option",
            {"sessionId": sid, "configId": "model", "value": "auto"},
        )
        result["set_config_option"] = response_class(leg.read_until({config_id}, 45).get(config_id))
        leg.drain(2)

        # Previously covered extension methods, with an unknown-method control.
        calls = [
            ("_kiro.dev/settings/list", {}),
            ("_kiro.dev/settings/list", {"sessionId": sid}),
            ("_kiro.dev/settings/set", {"key": "probe.nonexistent.key", "value": True}),
            ("_kiro.dev/settings/set", {"sessionId": sid, "key": "probe.nonexistent.key", "value": True}),
            ("_kiro.dev/session/list", {"cwd": leg.cwd}),
            ("_kiro.dev/session/list", {}),
            ("_kiro.dev/session/terminate", {"sessionId": "00000000-0000-4000-8000-00000000dead"}),
            ("_session/steer/clear", {"sessionId": sid}),
            ("_message/send", {"sessionId": "00000000-0000-4000-8000-00000000dead", "message": "probe"}),
            ("_kiro.dev/nonexistent/method", {}),
        ]
        for method, params in calls:
            rid = leg.request(method, params)
            response = leg.read_until({rid}, 45).get(rid)
            result["extension"][method + " " + json.dumps(redact_dynamic(params), sort_keys=True)] = response_class(response)
            leg.drain(0.3)

        # Successful tool turn: a known file read and exact shell command in a
        # disposable workspace. Approval requests are answered allow_once and
        # retained in the capture, so tool/permission ordering is observable.
        output_path = os.path.join(leg.cwd, "result.txt")
        prompt = (
            f"Use the file tool to read exactly {os.path.join(leg.cwd, 'input.txt')}. "
            f"Then use the shell tool to run exactly: printf 'V2_AUDIT_2211_SHELL_MARKER\\n'. "
            f"Finally use the file tool to write exactly this text to {output_path}: "
            "V2_AUDIT_2211_FILE_MARKER|V2_AUDIT_2211_SHELL_MARKER. "
            "Do the tool actions, do not merely explain them, and reply exactly DONE."
        )
        prompt_id = leg.request("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
        success_response = leg.read_until({prompt_id}, 360).get(prompt_id)
        leg.drain(5)
        result["workload"]["success"] = response_class(success_response)
        result["workload"]["success_prompt"] = prompt
        result["workload"]["success_output_exists"] = os.path.exists(output_path)
        result["workload"]["success_output"] = Path(output_path).read_text(encoding="utf-8") if os.path.exists(output_path) else None
        success_events = []
        tool_ids: set[str] = set()
        tool_names: list[str] = []
        for frame in leg.frames:
            update = session_update(frame)
            if update:
                kind = update.get("sessionUpdate")
                if kind in {"turn_start", "turn_completion", "turn_end"}:
                    success_events.append({"kind": kind, "stopReason": (update.get("_meta") or {}).get("kiro", {}).get("stopReason")})
                if kind in {"tool_call", "tool_call_update"}:
                    tool_id = update.get("toolCallId")
                    if tool_id:
                        tool_ids.add(tool_id)
                    raw = update.get("rawInput") or {}
                    if isinstance(raw, dict):
                        if "path" in raw:
                            tool_names.append("file")
                        if "command" in raw:
                            tool_names.append("shell")
        result["workload"]["success_events"] = success_events
        result["workload"]["tool_names"] = sorted(set(tool_names))
        result["workload"]["permission_count"] = len(leg.permissions)
        result["assertions"]["success_prompt_result"] = (success_response or {}).get("result", {}).get("stopReason") == "end_turn"
        result["assertions"]["success_streamed_text"] = any(
            session_update(frame).get("sessionUpdate") == "agent_message_chunk" for frame in leg.frames
        )
        result["assertions"]["success_file_tool"] = "file" in tool_names or result["workload"]["success_output_exists"]
        result["assertions"]["success_shell_tool"] = "shell" in tool_names
        result["assertions"]["approval_lifecycle"] = bool(leg.permissions)

        # ACP cancellation is a notification (no id), sent while the long
        # shell turn is active.  Wait for the original prompt response; a
        # request-shaped cancel is a different protocol operation.
        cancel_id = leg.request(
            "session/prompt",
            {
                "sessionId": sid,
                "prompt": [{"type": "text", "text": "Run this exact shell command and wait for it: sleep 20; printf CANCEL_MARKER. Do not answer before it finishes."}],
            },
        )
        cancel_sent = False
        cancel_response: dict[str, Any] | None = None
        end = time.time() + 60
        while time.time() < end and (cancel_response is None or not cancel_sent):
            try:
                raw = leg.q.get(timeout=0.5)
            except queue.Empty:
                if not cancel_sent and time.time() + 59 > end:
                    leg.send({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}})
                    cancel_sent = True
                continue
            if raw is None:
                leg.dead = True
                break
            try:
                message = json.loads(raw)
            except json.JSONDecodeError:
                continue
            leg.record("agent->client", message)
            if "method" in message and "id" in message:
                leg.handle_server_request(message)
            elif "id" in message and ("result" in message or "error" in message):
                if message["id"] == cancel_id:
                    cancel_response = message
            if not cancel_sent:
                update = session_update(leg.frames[-1])
                if update or message.get("method") in {"session/update", "_kiro.dev/session/update"}:
                    leg.send({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}})
                    cancel_sent = True
        if cancel_sent and cancel_response is None:
            cancel_response = leg.read_until({cancel_id}, 45).get(cancel_id)
        leg.drain(4)
        result["workload"]["cancel"] = response_class(cancel_response)
        result["workload"]["cancel_notification_sent"] = cancel_sent
        cancel_stop_reasons = []
        for frame in leg.frames:
            update = session_update(frame)
            if update.get("sessionUpdate") == "session_info_update":
                meta = (update.get("_meta") or {}).get("kiro", {})
                if meta.get("kind") in {"turn_completion", "turn_end"}:
                    cancel_stop_reasons.append(meta.get("stopReason"))
        result["workload"]["cancel_stop_reasons"] = cancel_stop_reasons
        result["assertions"]["cancel_notification_sent"] = cancel_sent
        result["assertions"]["cancel_turn_observed"] = "cancelled" in cancel_stop_reasons or (cancel_response or {}).get("result", {}).get("stopReason") == "cancelled"
        # Error control after the successful and cancellation workloads.
        error_id = leg.request("_kiro.dev/nonexistent/method", {})
        error_response = leg.read_until({error_id}, 30).get(error_id)
        result["workload"]["error_control"] = response_class(error_response)
        result["assertions"]["error_response_observed"] = (error_response or {}).get("error") is not None

        # Metadata is kept as a compact value inventory in the verdict; full
        # values and every order position remain in the JSONL capture.
        result["notification_order"] = leg.notification_keys()
        result["field_paths"] = sorted(leg.field_paths())
        result["server_requests"] = sorted(set(leg.server_requests))
        result["permission_requests"] = leg.permissions
        result["frame_count"] = len(leg.frames)
    finally:
        leg.close()
        result["process_exit"] = leg.proc.poll() if leg.proc else None
        result["captured_frames"] = len(leg.frames)
    return result


def canonical_extension(leg: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in leg.get("extension", {}).items()}


def main() -> None:
    old = run_leg(OLD, "old")
    new = run_leg(NEW, "new")
    old_fields, new_fields = set(old.get("field_paths", [])), set(new.get("field_paths", []))
    old_order, new_order = old.get("notification_order", []), new.get("notification_order", [])
    old_ext, new_ext = canonical_extension(old), canonical_extension(new)
    verdict = {
        "schema": "cyril.v2-wire-audit.2.21.1",
        "same_day_backend_control": True,
        "old": old,
        "new": new,
        "comparison": {
            "field_paths_old": len(old_fields),
            "field_paths_new": len(new_fields),
            "only_new_fields": sorted(new_fields - old_fields),
            "only_old_fields": sorted(old_fields - new_fields),
            "extension_keys_only_new": sorted(set(new_ext) - set(old_ext)),
            "extension_keys_only_old": sorted(set(old_ext) - set(new_ext)),
            "extension_value_differences": sorted(key for key in set(old_ext) & set(new_ext) if old_ext[key] != new_ext[key]),
            "notification_order_identical": old_order == new_order,
            "notification_order_old": old_order,
            "notification_order_new": new_order,
        },
        "limitations": [
            "Backend responses are live and can vary in model wording, timing, usage, and tool choice.",
            "Dynamic IDs, timestamps, and disposable workspace paths are normalized only in comparison fields; raw sanitized ordering is retained in captures.",
            "A tool leg that chooses not to execute a requested tool is reported as a workload limitation, not as a protocol removal.",
        ],
    }
    with open(f"{PREFIX}-verdict.json", "w", encoding="utf-8") as stream:
        json.dump(verdict, stream, indent=2, ensure_ascii=False)
        stream.write("\n")
    print(json.dumps({"verdict": f"{PREFIX}-verdict.json", "old_frames": old["captured_frames"], "new_frames": new["captured_frames"], "only_new_fields": verdict["comparison"]["only_new_fields"], "only_old_fields": verdict["comparison"]["only_old_fields"]}, indent=2))


if __name__ == "__main__":
    main()

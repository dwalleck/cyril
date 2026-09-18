#!/usr/bin/env python3
"""Retained KAS/v3 probe for MCP frames, failures, and ACP credit fields.

The probe uses real HOME and a caller-selected cwd. It captures every parsed
JSON-RPC frame in redacted JSONL, scans both directions for credit/usage/token
fields, and records exact redacted MCP status payloads. Registry MCP is left to
Kiro's native configuration/governance path; no registry entry is injected into
ACP's command/url mcpServers shape.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import queue
import re
import shutil
import signal
import subprocess
import sys
import threading
import time
from typing import Any

SECRET_KEYS = {
    "accesstoken", "access_token", "refreshtoken", "refresh_token",
    "idtoken", "id_token", "clientsecret", "client_secret", "bearer",
    "authorization", "profilearn", "profile_arn", "password", "secret",
}
SAFE_STRING_KEYS = {
    "errorType",
    "interactionType",
    "jsonrpc",
    "kind",
    "method",
    "optionId",
    "outcome",
    "sessionUpdate",
    "status",
    "type",
}

CREDIT_TERMS = (
    "credit", "credits", "meter", "metering", "effort", "token", "usage",
    "cost", "price",
)
ALLOWED_ENV_NAMES = {
    "PATH", "HOME", "USERPROFILE", "HOMEDRIVE", "HOMEPATH", "APPDATA",
    "LOCALAPPDATA", "PROGRAMDATA", "PROGRAMFILES", "PROGRAMFILES(X86)",
    "COMMONPROGRAMFILES", "COMMONPROGRAMFILES(X86)", "SYSTEMROOT", "WINDIR",
    "COMSPEC", "PATHEXT", "OS", "PROCESSOR_ARCHITECTURE",
    "NUMBER_OF_PROCESSORS", "TEMP", "TMP", "LANG", "LC_ALL",
    "XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME",
    "NO_PROXY", "SSL_CERT_FILE", "SSL_CERT_DIR", "NODE_EXTRA_CA_CERTS",
}
SENSITIVE_ENV_MARKERS = (
    "TOKEN", "SECRET", "PASSWORD", "CREDENTIAL", "API_KEY", "ACCESS_KEY",
    "PROFILE_ARN", "BEARER", "AUTH", "AWS_", "AZURE_",
)


def redact(value: Any, key: str = "") -> Any:
    lowered = key.lower()
    if (
        lowered in SECRET_KEYS
        or "token" in lowered
        or lowered.endswith("credential")
    ):
        return "<redacted>"
    if isinstance(value, dict):
        return {name: redact(item, name) for name, item in value.items()}
    if isinstance(value, list):
        return [redact(item, key) for item in value]
    if isinstance(value, str):
        return value if key in SAFE_STRING_KEYS else "<redacted>"
    return value


def value_shape(value: Any, depth: int = 0) -> Any:
    if depth > 6:
        return "..."
    if isinstance(value, dict):
        return {key: value_shape(item, depth + 1) for key, item in value.items()}
    if isinstance(value, list):
        if not value:
            return []
        return [value_shape(value[0], depth + 1), f"...(len={len(value)})"]
    return type(value).__name__


def collect_credit_hits(value: Any, path: str = "") -> list[dict[str, Any]]:
    hits: list[dict[str, Any]] = []
    if isinstance(value, dict):
        for key, item in value.items():
            child_path = f"{path}.{key}" if path else key
            matching_terms = [term for term in CREDIT_TERMS if term in key.lower()]
            if matching_terms:
                hits.append({
                    "path": child_path,
                    "matched_terms": matching_terms,
                    "key": key,
                    "value_type": type(item).__name__,
                    "value_shape": value_shape(redact(item, key)),
                })
            hits.extend(collect_credit_hits(item, child_path))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            hits.extend(collect_credit_hits(item, f"{path}[{index}]"))
    elif isinstance(value, str):
        matching_terms = [term for term in CREDIT_TERMS if term in value.lower()]
        if matching_terms:
            hits.append({
                "path": path,
                "matched_terms": matching_terms,
                "value_type": "str",
                "value_shape": "str",
            })
    return hits


def find_binary() -> str:
    configured = os.environ.get("KIRO_BIN")
    if configured:
        return configured
    resolved = shutil.which("kiro-cli")
    if resolved:
        return resolved
    if os.name == "nt":
        candidate = Path.home() / "AppData/Local/Kiro-Cli/kiro-cli.exe"
        if candidate.exists():
            return str(candidate)
    raise RuntimeError("kiro-cli was not found; set KIRO_BIN")


class Probe:
    def __init__(self, binary: str, cwd: Path, capture: Path) -> None:
        self.binary = binary
        self.cwd = cwd
        self.capture = capture
        self.capture.parent.mkdir(parents=True, exist_ok=True)
        self.capture_file = self.capture.open("w", encoding="utf-8", buffering=1)
        self.stderr: list[str] = []
        self.messages: queue.Queue[str | None] = queue.Queue()
        self.request_id = 0
        self.process: subprocess.Popen[str] | None = None
        self.frames: list[dict[str, Any]] = []
        self.credit_frames: list[dict[str, Any]] = []
        self.server_requests: dict[str, int] = {}
        self.auth_requested = False
        self.permission_count = 0
        self.permission_requests: list[dict[str, Any]] = []
        self.child_env_names: list[str] = []
        self.excluded_sensitive_env_names: list[str] = []
        self.teardown_status: dict[str, Any] = {}
        self.mcp_turn = os.environ.get("MCP_TURN") == "1"

    def record(self, direction: str, frame: dict[str, Any]) -> None:
        hits = collect_credit_hits(frame)
        if hits:
            self.credit_frames.append({
                "dir": direction,
                "method": frame.get("method"),
                "hits": hits,
            })
        safe = redact(frame)
        entry = {"ts": time.time(), "dir": direction, "msg": safe}
        self.capture_file.write(json.dumps(entry, sort_keys=True) + "\n")
        self.frames.append(entry)

    def start(self) -> None:
        env = {
            name: value
            for name, value in os.environ.items()
            if name.upper() in ALLOWED_ENV_NAMES
        }
        env.setdefault("HOME", str(Path.home()))
        self.excluded_sensitive_env_names = sorted(
            name for name in os.environ
            if name not in env
            and any(marker in name.upper() for marker in SENSITIVE_ENV_MARKERS)
        )
        kwargs: dict[str, Any] = {
            "cwd": str(self.cwd),
            "env": env,
            "stdin": subprocess.PIPE,
            "stdout": subprocess.PIPE,
            "stderr": subprocess.PIPE,
            "text": True,
            "bufsize": 1,
        }
        if os.name == "nt":
            kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            kwargs["start_new_session"] = True
        self.process = subprocess.Popen(
            [self.binary, "acp", "--agent-engine", "v3", "--auth-method", "cli"],
            **kwargs,
        )
        self.child_env_names = sorted(env)
        threading.Thread(target=self._read_stdout, daemon=True).start()
        threading.Thread(target=self._read_stderr, daemon=True).start()

    def _read_stdout(self) -> None:
        assert self.process is not None and self.process.stdout is not None
        for line in self.process.stdout:
            if line.strip():
                self.messages.put(line.strip())
        self.messages.put(None)

    def _read_stderr(self) -> None:
        assert self.process is not None and self.process.stderr is not None
        self.stderr.extend(self.process.stderr)

    def send(self, frame: dict[str, Any]) -> None:
        assert self.process is not None and self.process.stdin is not None
        self.record("client->agent", frame)
        self.process.stdin.write(json.dumps(frame) + "\n")
        self.process.stdin.flush()

    def request(self, method: str, params: dict[str, Any]) -> int:
        self.request_id += 1
        self.send({"jsonrpc": "2.0", "id": self.request_id, "method": method, "params": params})
        return self.request_id

    def reply(self, request_id: Any, result: dict[str, Any]) -> None:
        self.send({"jsonrpc": "2.0", "id": request_id, "result": result})

    def callback(self, frame: dict[str, Any]) -> dict[str, Any]:
        method = str(frame.get("method", ""))
        self.server_requests[method] = self.server_requests.get(method, 0) + 1
        if method == "_kiro/terminal/shell_type":
            return {"shellType": "powershell" if os.name == "nt" else "bash"}
        if method == "_kiro/auth/getAccessToken":
            self.auth_requested = True
            raise RuntimeError("unexpected _kiro/auth/getAccessToken callback")
        if method == "session/request_permission":
            self.permission_count += 1
            self.permission_requests.append(redact(frame))
            details = json.dumps(frame).lower()
            if self.mcp_turn and "aws-docs" in details and "search_documentation" in details:
                options = ((frame.get("params") or {}).get("options") or [])
                selected = next(
                    (option for option in options
                     if "allow" in (str(option.get("kind", "")) + str(option.get("optionId", ""))).lower()),
                    None,
                )
                if selected:
                    return {"outcome": {"outcome": "selected", "optionId": selected["optionId"]}}
            return {"outcome": {"outcome": "cancelled"}}
        return {}

    def pump(self, until_id: int | None, timeout: float, idle_exit: float | None = None) -> dict[str, Any] | None:
        deadline = time.time() + timeout
        last_frame = time.time()
        while time.time() < deadline:
            try:
                raw = self.messages.get(timeout=1)
            except queue.Empty:
                if idle_exit is not None and time.time() - last_frame >= idle_exit:
                    return None
                continue
            if raw is None:
                return None
            last_frame = time.time()
            try:
                frame = json.loads(raw)
            except json.JSONDecodeError:
                continue
            self.record("agent->client", frame)
            if frame.get("method") and "id" in frame:
                self.reply(frame["id"], self.callback(frame))
            if until_id is not None and frame.get("id") == until_id and ("result" in frame or "error" in frame):
                return frame
        return None

    def stop(self) -> None:
        if self.process is not None and self.process.poll() is None:
            pid = self.process.pid
            try:
                if os.name == "nt":
                    result = subprocess.run(
                        ["taskkill", "/PID", str(pid), "/T", "/F"],
                        capture_output=True,
                        text=True,
                        timeout=10,
                    )
                    self.teardown_status = {
                        "method": "taskkill_tree",
                        "returncode": result.returncode,
                    }
                else:
                    os.killpg(os.getpgid(pid), signal.SIGTERM)
                    self.teardown_status = {"method": "killpg", "signal": "SIGTERM"}
                self.process.wait(timeout=10)
            except (OSError, subprocess.TimeoutExpired) as error:
                self.teardown_status["error"] = type(error).__name__
                if self.process.poll() is None:
                    self.process.kill()
                    self.process.wait(timeout=10)
            self.teardown_status["process_exited"] = self.process.poll() is not None
        self.capture_file.close()

    def run(self) -> dict[str, Any]:
        self.start()
        result: dict[str, Any] | None = None
        try:
            initialize_id = self.request(
                "initialize",
                {
                    "protocolVersion": 1,
                    "clientInfo": {"name": "cyril-kas-mcp-credit-probe", "version": "0.1.0"},
                    "clientCapabilities": {
                        "fs": {"readTextFile": True, "writeTextFile": True},
                        "terminal": True,
                        "_meta": {"kiro": {"settings": {"subagentOrchestration": {"enabled": False}}}},
                    },
                },
            )
            initialize = self.pump(initialize_id, 120)
            if not initialize or "error" in initialize:
                raise RuntimeError(f"initialize failed: {initialize}")

            session_request_id = self.request("session/new", {"cwd": str(self.cwd), "mcpServers": []})
            session_new = self.pump(session_request_id, 180)
            if not session_new or "error" in session_new:
                raise RuntimeError(f"session/new failed: {session_new}")
            sid = (session_new.get("result") or {}).get("sessionId")
            if not sid:
                raise RuntimeError("session/new returned no sessionId")
            self.pump(None, 12, idle_exit=5)

            prompt_id = self.request(
                "session/prompt",
                {
                    "sessionId": sid,
                    "prompt": [{
                        "type": "text",
                        "text": (
                            "Use the aws-docs MCP server search_documentation tool exactly once "
                            "to search for AWS CodeBuild User Guide. Do not use built-in tools. "
                            "Then reply exactly MCP-TURN-PROBE-OK."
                            if self.mcp_turn
                            else "Do not use any tools. Reply exactly MCP-TURN-PROBE-OK after receiving this prompt."
                        ),
                    }],
                },
            )
            prompt = self.pump(prompt_id, 180)
            if not prompt or "error" in prompt:
                raise RuntimeError(f"session/prompt failed: {prompt}")
            self.pump(None, 12, idle_exit=5)
            turn_completion_frames = [
                entry["msg"]
                for entry in self.frames
                if entry["msg"].get("method") == "session/update"
                and ((entry["msg"].get("params") or {}).get("update") or {}).get("_meta", {}).get("kiro", {}).get("kind") == "turn_completion"
            ]
            successful_turns = [
                frame for frame in turn_completion_frames
                if ((frame.get("params") or {}).get("update") or {}).get("_meta", {}).get("kiro", {}).get("status") == "success"
            ]
            if not successful_turns:
                raise RuntimeError("session/prompt completed without a successful turn_completion update")

            notification_counts: dict[str, int] = {}
            mcp_status_payloads: list[dict[str, Any]] = []
            mcp_failure_entries: list[dict[str, Any]] = []
            mcp_error_frames: list[dict[str, Any]] = []
            for entry in self.frames:
                message = entry["msg"]
                method = message.get("method") if isinstance(message, dict) else None
                if method:
                    notification_counts[method] = notification_counts.get(method, 0) + 1
                if method == "_kiro/mcp/status":
                    params = message.get("params") or {}
                    if isinstance(params, dict):
                        mcp_status_payloads.append(params)
                        for server in params.get("servers", []):
                            if isinstance(server, dict) and server.get("status") in {"failed", "error"}:
                                mcp_failure_entries.append(server)
                if method and "mcp" in method.lower() and (message.get("error") or "error" in (message.get("params") or {})):
                    mcp_error_frames.append(message)

            result = {
                "completed": True,
                "binary": Path(self.binary).name,
                "engine": "v3",
                "cwd": "<repo>",
                "session_id": "<redacted>",
                "initialize_result_keys": sorted((initialize.get("result") or {}).keys()),
                "session_new_result_keys": sorted((session_new.get("result") or {}).keys()),
                "notification_counts": dict(sorted(notification_counts.items())),
                "mcp_status_frames": notification_counts.get("_kiro/mcp/status", 0),
                "mcp_status_payloads": redact(mcp_status_payloads),
                "mcp_failure_entries": redact(mcp_failure_entries),
                "mcp_error_frames": redact(mcp_error_frames),
                "tools_did_change_frames": notification_counts.get("_kiro/tools/didChange", 0),
                "server_requests": dict(sorted(self.server_requests.items())),
                "permission_requests": self.permission_requests,
                "auth_callback_requested": self.auth_requested,
                "permission_count": self.permission_count,
                "credit_terms": list(CREDIT_TERMS),
                "credit_related_frames": self.credit_frames,
                "credit_related_frame_count": len(self.credit_frames),
                "child_env_names": self.child_env_names,
                "excluded_sensitive_env_names": self.excluded_sensitive_env_names,
                "stderr_line_count": len(self.stderr),
                "teardown": None,
                "capture": self.capture.name,
            }
        finally:
            self.stop()
        if result is None:
            raise RuntimeError("probe produced no result")
        result["teardown"] = self.teardown_status
        return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cwd", default=os.getcwd())
    parser.add_argument("--capture", required=True)
    parser.add_argument("--summary", required=True)
    args = parser.parse_args()
    capture = Path(args.capture).resolve()
    summary_path = Path(args.summary).resolve()
    probe = Probe(find_binary(), Path(args.cwd).resolve(strict=True), capture)
    try:
        summary = probe.run()
    except Exception as error:
        summary = {
            "completed": False,
            "error": "<redacted>",
            "capture": probe.capture.name,
            "frames_seen": len(probe.frames),
            "server_requests": dict(sorted(probe.server_requests.items())),
            "permission_requests": probe.permission_requests,
            "auth_callback_requested": probe.auth_requested,
            "child_env_names": probe.child_env_names,
            "excluded_sensitive_env_names": probe.excluded_sensitive_env_names,
            "stderr_line_count": len(probe.stderr),
            "teardown": probe.teardown_status,
            "credit_related_frames": [
                {"dir": entry["dir"], "method": entry["msg"].get("method"), "hits": hits}
                for entry in probe.frames
                if (hits := collect_credit_hits(entry["msg"]))
            ],
        }
        summary_path.parent.mkdir(parents=True, exist_ok=True)
        summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps(summary, indent=2, sort_keys=True), file=sys.stderr)
        return 1
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

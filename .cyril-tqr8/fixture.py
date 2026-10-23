#!/usr/bin/env python3
"""Loopback-only, synthetic MCP/OAuth fixture; starts no clients or browsers.

Stdout is one noncredential JSON manifest. The retained private temporary root
contains isolated configuration and a redacted JSONL lifecycle/request log.
Tokens, codes, PKCE material, and registrations exist only in process memory.
This is an acceptance instrument, not an OAuth service for production use.
"""

from __future__ import annotations

import argparse
import base64
from datetime import datetime, timezone
import hashlib
import hmac
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ipaddress
import json
import math
import os
from pathlib import Path
import re
import secrets
import signal
import socket
import tempfile
import threading
import time
from typing import cast
from urllib.parse import parse_qs, urlencode, urlsplit, urlunsplit


MAX_BODY = 64 * 1024
BODY_TIMEOUT = 10.0
SILENT_SECONDS = 120.0
KAS_TIMEOUT_MILLISECONDS = 150_000
CLIENT_TTL = 3600
CODE_TTL = 120
TOKEN_TTL = 600
SCOPE = "fixture.read"
PRM_PATH = "/.well-known/oauth-protected-resource/mcp/oauth"
AS_PATH = "/.well-known/oauth-authorization-server"
PKCE_CHALLENGE = re.compile(r"[A-Za-z0-9_-]{43}\Z")
PKCE_VERIFIER = re.compile(r"[A-Za-z0-9._~-]{43,128}\Z")


class RequestError(Exception):
    def __init__(self, status, error, description):
        super().__init__(description)
        self.status = status
        self.error = error
        self.description = description


def loopback_redirect(value):
    if not isinstance(value, str) or not value or len(value) > 2048:
        return False
    if any(ord(character) <= 32 or ord(character) == 127 for character in value):
        return False
    try:
        parsed = urlsplit(value)
        host = parsed.hostname
        port = parsed.port
        if (
            parsed.scheme != "http"
            or not host
            or port is None
            or not 1 <= port <= 65535
            or parsed.username is not None
            or parsed.password is not None
            or parsed.fragment
        ):
            return False
        return host == "localhost" or ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


def form_fields(encoded: str) -> dict[str, str]:
    try:
        fields = parse_qs(encoded, keep_blank_values=True, strict_parsing=True, errors="strict")
    except (UnicodeError, ValueError):
        raise RequestError(400, "invalid_request", "Malformed form parameters.") from None
    return {key: values[0] for key, values in fields.items()}


class FixtureState:
    def __init__(self, root, silent, slow_ready):
        self.root = root
        self.stop = threading.Event()
        self.lock = threading.Lock()
        self.log_lock = threading.Lock()
        self.clients = {}
        self.codes = {}
        self.tokens = {}
        self.slow_ready = slow_ready
        self.paths = {
            "/mcp/ready": "ready",
            "/mcp/oauth": "oauth",
            "/mcp/failed": "failed",
        }
        if silent:
            self.paths["/mcp/silent"] = "silent"
        if slow_ready is not None:
            self.paths["/mcp/slow-ready"] = "slow-ready"
        self.waiters = {name: threading.Event() for name in self.paths.values()}
        self.log_path = root / "requests.jsonl"
        self.log_path.touch(mode=0o600)
        self.base = ""

    def record(self, **fields):
        entry = {"timestamp": datetime.now(timezone.utc).isoformat(), **fields}
        with self.log_lock, self.log_path.open("a", encoding="utf-8") as output:
            output.write(json.dumps(entry, separators=(",", ":")) + "\n")

    def marker(self, name):
        # Names originate only from fixture constants, never request values.
        self.record(marker=name)

    def prune_locked(self):
        now = time.monotonic()
        for collection in (self.clients, self.codes, self.tokens):
            expired = [key for key, entry in collection.items() if entry["expires"] <= now]
            for key in expired:
                del collection[key]

    def bearer_valid(self, authorization):
        if not authorization:
            return False
        parts = authorization.split()
        if len(parts) != 2 or parts[0].lower() != "bearer":
            return False
        with self.lock:
            self.prune_locked()
            return parts[1] in self.tokens

    def register_waiter(self, name):
        # Bind the request before announcing it as waiting: DELETE rotates
        # the generation only for subsequent requests, not this one.
        with self.lock:
            return self.waiters[name]

    def wait(self, cancelled, seconds):
        deadline = time.monotonic() + seconds
        while not self.stop.is_set() and not cancelled.is_set():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return True
            self.stop.wait(min(remaining, 0.1))
        return False

    def cancel_waiters(self, name):
        with self.lock:
            self.waiters[name].set()
            self.waiters[name] = threading.Event()


class FixtureServer(ThreadingHTTPServer):
    daemon_threads = True
    block_on_close = False
    allow_reuse_address = False

    def __init__(self, state):
        self.state = state
        self.connections = set()
        self.connections_lock = threading.Lock()
        super().__init__(("127.0.0.1", 0), FixtureHandler)

    def get_request(self):
        connection, address = super().get_request()
        with self.connections_lock:
            self.connections.add(connection)
        return connection, address

    def close_request(self, request):
        with self.connections_lock:
            self.connections.discard(request)
        super().close_request(request)

    def cancel_connections(self):
        with self.connections_lock:
            connections = tuple(self.connections)
        for connection in connections:
            try:
                connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass  # A concurrently completed connection is already closed.

    def handle_error(self, request, client_address):
        # The stdlib traceback may include request data; retain only a marker.
        self.state.marker("http.handler_error")


class FixtureHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "CyrilSyntheticFixture/1"
    sys_version = ""

    @property
    def state(self):
        return cast(FixtureServer, self.server).state

    def setup(self):
        super().setup()
        self.connection.settimeout(BODY_TIMEOUT)

    def log_message(self, format, *args):
        # BaseHTTPRequestHandler otherwise logs authorization query strings.
        pass

    def safe_path(self):
        try:
            path = urlsplit(getattr(self, "path", "")).path
        except ValueError:
            return "<invalid>"
        known = set(self.state.paths) | {
            PRM_PATH,
            "/.well-known/oauth-protected-resource",
            AS_PATH,
            "/register",
            "/authorize",
            "/token",
        }
        return path if path in known else "<unknown>"

    def respond(self, status, payload=None, headers=None):
        body = b"" if payload is None else json.dumps(
            payload, ensure_ascii=True, allow_nan=False, separators=(",", ":")
        ).encode("utf-8")
        self.close_connection = True
        self.send_response(status)
        if payload is not None:
            self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.send_header("Cache-Control", "no-store")
        for name, value in (headers or {}).items():
            self.send_header(name, value)
        self.end_headers()
        method = getattr(self, "command", "")
        self.state.record(
            method=method if method in {"GET", "POST", "DELETE", "HEAD", "OPTIONS", "PUT", "PATCH"} else "<other>",
            path=self.safe_path(),
            status=status,
        )
        if method != "HEAD" and body:
            self.wfile.write(body)

    def send_error(self, code, message=None, explain=None):
        # Never echo stdlib parser errors containing request lines or headers.
        self.respond(code, {"error": "http_request_error", "error_description": "Malformed or unsupported HTTP request."})

    def do_GET(self):
        self.dispatch()

    def do_POST(self):
        self.dispatch()

    def do_DELETE(self):
        self.dispatch()

    def do_HEAD(self):
        self.dispatch()

    def do_OPTIONS(self):
        self.dispatch()

    def dispatch(self):
        try:
            try:
                target = urlsplit(self.path)
            except ValueError:
                raise RequestError(400, "invalid_request", "Malformed request target.") from None
            if target.scheme or target.netloc or target.fragment:
                raise RequestError(400, "invalid_request", "Only origin-form request targets are supported.")
            if self.state.stop.is_set():
                raise RequestError(503, "fixture_stopping", "Fixture is shutting down.")
            if target.path in self.state.paths:
                self.mcp(self.state.paths[target.path])
            elif target.path in {PRM_PATH, "/.well-known/oauth-protected-resource", AS_PATH}:
                self.metadata(target.path)
            elif target.path == "/register":
                self.register()
            elif target.path == "/authorize":
                self.authorize(target.query)
            elif target.path == "/token":
                self.token()
            else:
                raise RequestError(404, "not_found", "No fixture endpoint exists at this path.")
        except RequestError as error:
            self.respond(error.status, {"error": error.error, "error_description": error.description})
        except (ConnectionError, TimeoutError, OSError):
            self.close_connection = True
            self.state.marker("http.connection_closed")
        except Exception:
            self.state.marker("http.internal_error")
            self.respond(500, {"error": "fixture_internal_error", "error_description": "Unexpected fixture failure; see the redacted lifecycle marker."})

    def require_method(self, method):
        if self.command != method:
            self.respond(405, {"error": "method_not_allowed", "error_description": "Use " + method + " for this endpoint."}, {"Allow": method})
            return False
        return True

    def read_body(self, content_type):
        if self.headers.get("Transfer-Encoding") is not None:
            raise RequestError(400, "invalid_request", "Send Content-Length, not Transfer-Encoding.")
        try:
            length = int(self.headers.get("Content-Length", ""))
        except ValueError:
            raise RequestError(411, "invalid_request", "A valid Content-Length is required.") from None
        if not 0 <= length <= MAX_BODY:
            raise RequestError(413, "invalid_request", "Request body must be at most 64 KiB.")
        if self.headers.get_content_type() != content_type:
            raise RequestError(415, "invalid_request", "Expected Content-Type " + content_type + ".")
        try:
            body = self.rfile.read(length)
        except TimeoutError:
            raise RequestError(408, "invalid_request", "Request body stalled for ten seconds.") from None
        if len(body) != length:
            raise RequestError(400, "invalid_request", "Request body ended before Content-Length.")
        return body

    def read_json(self):
        try:
            return json.loads(self.read_body("application/json").decode("utf-8"))
        except (UnicodeError, ValueError, RecursionError):
            raise RequestError(400, "invalid_json", "Request body must contain valid UTF-8 JSON.") from None

    def metadata(self, path):
        if not self.require_method("GET"):
            return
        if path == AS_PATH:
            self.state.marker("oauth.authorization_metadata")
            result = {
                "issuer": self.state.base,
                "authorization_endpoint": self.state.base + "/authorize",
                "token_endpoint": self.state.base + "/token",
                "registration_endpoint": self.state.base + "/register",
                "response_types_supported": ["code"],
                "grant_types_supported": ["authorization_code"],
                "code_challenge_methods_supported": ["S256"],
                "token_endpoint_auth_methods_supported": ["none"],
                "scopes_supported": [SCOPE],
            }
        else:
            self.state.marker("oauth.resource_metadata")
            result = {
                "resource": self.state.base + "/mcp/oauth",
                "authorization_servers": [self.state.base],
                "bearer_methods_supported": ["header"],
                "scopes_supported": [SCOPE],
            }
        self.respond(200, result)

    def register(self):
        if not self.require_method("POST"):
            return
        body = self.read_json()
        if not isinstance(body, dict):
            raise RequestError(400, "invalid_client_metadata", "Registration must be a JSON object.")
        redirects = body.get("redirect_uris")
        if not isinstance(redirects, list) or not redirects or not all(loopback_redirect(uri) for uri in redirects):
            raise RequestError(400, "invalid_redirect_uri", "Register HTTP loopback redirect URIs with explicit valid ports and no fragment or user information.")
        if body.get("token_endpoint_auth_method", "none") != "none":
            raise RequestError(400, "invalid_client_metadata", "Only public clients using token_endpoint_auth_method none are supported.")
        requested_grants = body.get("grant_types", ["authorization_code"])
        if (
            not isinstance(requested_grants, list)
            or "authorization_code" not in requested_grants
            or any(grant not in ("authorization_code", "refresh_token") for grant in requested_grants)
            or body.get("response_types", ["code"]) != ["code"]
        ):
            raise RequestError(400, "invalid_client_metadata", "Request authorization_code with code responses; refresh eligibility may be requested but is not granted.")
        # KAS requests refresh eligibility in DCR by default. Grant only the
        # code flow below; discovery, /token and issued credentials stay honest.
        with self.state.lock:
            self.state.prune_locked()
            client_id = "fixture-client-" + secrets.token_urlsafe(18)
            self.state.clients[client_id] = {"redirects": tuple(redirects), "expires": time.monotonic() + CLIENT_TTL}
        self.state.marker("oauth.client_registered")
        self.respond(201, {
            "client_id": client_id,
            "client_id_issued_at": int(time.time()),
            "redirect_uris": redirects,
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
        })

    def authorize(self, query):
        if not self.require_method("GET"):
            return
        fields = form_fields(query)
        redirect = fields.get("redirect_uri", "")
        state = fields.get("state", "")
        challenge = fields.get("code_challenge", "")
        if not loopback_redirect(redirect):
            raise RequestError(400, "invalid_request", "redirect_uri must be an HTTP loopback URI with an explicit valid port and no fragment or user information.")
        if fields.get("response_type") != "code":
            raise RequestError(400, "unsupported_response_type", "Only response_type code is supported.")
        if not state:
            raise RequestError(400, "invalid_request", "A nonempty state is required.")
        if fields.get("code_challenge_method") != "S256" or not PKCE_CHALLENGE.fullmatch(challenge):
            raise RequestError(400, "invalid_request", "A valid S256 code_challenge is required.")
        if fields.get("resource", self.state.base + "/mcp/oauth") != self.state.base + "/mcp/oauth":
            raise RequestError(400, "invalid_target", "The resource must be this fixture's OAuth MCP endpoint.")
        scope = fields.get("scope", SCOPE)
        if set(scope.split()) - {SCOPE}:
            raise RequestError(400, "invalid_scope", "Only fixture.read is supported; offline access is unavailable.")
        with self.state.lock:
            self.state.prune_locked()
            client_id = fields.get("client_id", "")
            registration = self.state.clients.get(client_id)
            if registration is None or redirect not in registration["redirects"]:
                raise RequestError(400, "invalid_request", "client_id and redirect_uri must match a live fixture registration.")
            code = "fixture-code-" + secrets.token_urlsafe(24)
            self.state.codes[code] = {
                "client": client_id,
                "redirect": redirect,
                "challenge": challenge,
                "scope": scope,
                "expires": time.monotonic() + CODE_TTL,
            }
        target = urlsplit(redirect)
        added = urlencode({"code": code, "state": state})
        location = urlunsplit((target.scheme, target.netloc, target.path, target.query + ("&" if target.query else "") + added, ""))
        self.state.marker("oauth.authorization_redirect")
        # The fixture does not follow this redirect or contact the callback.
        self.respond(302, headers={"Location": location})

    def token(self):
        if not self.require_method("POST"):
            return
        try:
            fields = form_fields(self.read_body("application/x-www-form-urlencoded").decode("utf-8"))
        except UnicodeError:
            raise RequestError(400, "invalid_request", "Token form must be UTF-8.") from None
        if fields.get("grant_type") != "authorization_code":
            raise RequestError(400, "unsupported_grant_type", "Only authorization_code is supported; there is no refresh path.")
        verifier = fields.get("code_verifier", "")
        digest = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode("utf-8")).digest()).rstrip(b"=").decode("ascii")
        with self.state.lock:
            self.state.prune_locked()
            code = fields.get("code", "")
            entry = self.state.codes.get(code)
            if (
                entry is None
                or entry["client"] not in self.state.clients
                or entry["client"] != fields.get("client_id")
                or entry["redirect"] != fields.get("redirect_uri")
                or not PKCE_VERIFIER.fullmatch(verifier)
                or not hmac.compare_digest(entry["challenge"], digest)
                or fields.get("resource", self.state.base + "/mcp/oauth") != self.state.base + "/mcp/oauth"
            ):
                self.state.marker("oauth.token_rejected")
                raise RequestError(400, "invalid_grant", "Authorization code is invalid, expired, used, or does not match the client, redirect, resource, or PKCE verifier.")
            token = "fixture-access-" + secrets.token_urlsafe(32)
            self.state.tokens[token] = {"expires": time.monotonic() + TOKEN_TTL}
            del self.state.codes[code]
            scope = entry["scope"]
        self.state.marker("oauth.token_issued")
        self.respond(200, {"access_token": token, "token_type": "Bearer", "expires_in": TOKEN_TTL, "scope": scope})

    def mcp(self, name):
        if self.command not in {"POST", "DELETE"}:
            self.respond(405, {"error": "method_not_allowed", "error_description": "This stateless Streamable HTTP fixture supports POST and DELETE, not SSE GET."}, {"Allow": "POST, DELETE"})
            return
        if name == "failed":
            self.state.marker("failed.unavailable")
            raise RequestError(503, "synthetic_unavailable", "Deliberately unavailable synthetic MCP endpoint.")
        if name == "oauth" and not self.state.bearer_valid(self.headers.get("Authorization")):
            self.state.marker("oauth.bearer_required")
            self.respond(401, {"error": "unauthorized", "error_description": "A live synthetic fixture Bearer token is required."}, {"WWW-Authenticate": 'Bearer resource_metadata="' + self.state.base + PRM_PATH + '", scope="' + SCOPE + '"'})
            return
        if self.command == "DELETE":
            self.state.cancel_waiters(name)
            self.state.marker(name + ".deleted")
            self.respond(200, {})
            return
        try:
            message = self.read_json()
        except RequestError as error:
            if error.error == "invalid_json":
                self.respond(400, {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": error.description}})
                return
            raise
        if not isinstance(message, dict):
            self.rpc_error(None, -32600, "Expected one JSON-RPC object; batches are unsupported.", 400)
            return
        request_id = message.get("id")
        if message.get("jsonrpc") != "2.0" or not isinstance(message.get("method"), str) or ("id" in message and not (request_id is None or type(request_id) in (str, int, float))) or (isinstance(request_id, float) and not math.isfinite(request_id)):
            self.rpc_error(None, -32600, "Invalid JSON-RPC version, method, or id.", 400)
            return
        method = message["method"]
        if "id" not in message:
            if method == "notifications/initialized":
                self.state.marker(name + ".initialized")
            else:
                self.state.marker(name + ".notification")
            self.respond(202)
            return
        if name == "silent":
            cancelled = self.state.register_waiter(name)
            self.state.marker("silent.waiting")
            if self.state.wait(cancelled, SILENT_SECONDS):
                self.state.marker("silent.timed_out")
                raise RequestError(504, "synthetic_silent_timeout", "Silent MCP wait reached its 120-second bound.")
            self.state.marker("silent.cancelled")
            raise RequestError(503, "synthetic_wait_cancelled", "Silent MCP wait cancelled by DELETE or fixture shutdown.")
        if name == "slow-ready" and method == "initialize":
            cancelled = self.state.register_waiter(name)
            self.state.marker("slow-ready.waiting")
            if not self.state.wait(cancelled, self.state.slow_ready):
                self.state.marker("slow-ready.cancelled")
                raise RequestError(503, "synthetic_wait_cancelled", "Slow MCP initialize cancelled by DELETE or fixture shutdown.")
            self.state.marker("slow-ready.delay_finished")
        params = message.get("params", {})
        if not isinstance(params, dict):
            self.rpc_error(request_id, -32602, "Parameters must be a JSON object.")
            return
        try:
            result = self.rpc_result(name, method, params)
        except LookupError:
            self.state.marker(name + ".unsupported_request")
            self.rpc_error(request_id, -32601, "Method not found; server/discover is intentionally unsupported.")
            return
        except ValueError as error:
            self.rpc_error(request_id, -32602, str(error))
            return
        self.state.marker(name + "." + method.replace("/", "."))
        self.respond(200, {"jsonrpc": "2.0", "id": request_id, "result": result})

    def rpc_error(self, request_id, code, message, status=200):
        self.respond(status, {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}})

    def rpc_result(self, name, method, params):
        text = "SYNTHETIC_MCP_CANARY::" + name
        uri = "fixture://" + name + "/canary"
        if method == "initialize":
            version = params.get("protocolVersion")
            if not isinstance(version, str) or not version:
                raise ValueError("initialize requires a nonempty protocolVersion.")
            return {
                "protocolVersion": version,
                "serverInfo": {"name": "cyril-fixture-" + name, "version": "1.0.0"},
                "capabilities": {
                    "tools": {"listChanged": False},
                    "prompts": {"listChanged": False},
                    "resources": {"subscribe": False, "listChanged": False},
                },
            }
        if method == "ping":
            return {}
        if method == "tools/list":
            return {"tools": [{
                "name": "read_canary",
                "description": "Return synthetic read-only text; no external access or side effects.",
                "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
                "annotations": {"readOnlyHint": True, "destructiveHint": False, "idempotentHint": True, "openWorldHint": False},
            }]}
        if method == "tools/call":
            if params.get("name") != "read_canary" or params.get("arguments", {}) != {}:
                raise ValueError("Only read_canary with empty object arguments is available.")
            return {"content": [{"type": "text", "text": text}], "isError": False}
        if method == "prompts/list":
            return {"prompts": [{"name": "synthetic_prompt", "description": "One synthetic acceptance prompt.",
                                 "arguments": [{"name": "marker", "description": "Synthetic text echoed without execution.", "required": True}]}]}
        if method == "prompts/get":
            arguments = params.get("arguments", {})
            if (params.get("name") != "synthetic_prompt" or not isinstance(arguments, dict)
                    or set(arguments) != {"marker"} or not isinstance(arguments["marker"], str)):
                raise ValueError("synthetic_prompt requires exactly one string argument named marker.")
            return {"description": "Synthetic acceptance prompt.", "messages": [{"role": "user", "content": {"type": "text", "text": text + "::" + arguments["marker"]}}]}
        if method == "resources/list":
            return {"resources": [{"uri": uri, "name": "synthetic_canary", "description": "One synthetic text resource.", "mimeType": "text/plain"}]}
        if method == "resources/templates/list":
            return {"resourceTemplates": [{"uriTemplate": "fixture://" + name + "/{name}", "name": "synthetic_canary_template", "description": "Use name=canary to read the single synthetic resource.", "mimeType": "text/plain"}]}
        if method == "resources/read":
            if params.get("uri") != uri:
                raise ValueError("Only the advertised synthetic canary resource URI is available.")
            return {"contents": [{"uri": uri, "mimeType": "text/plain", "text": text}]}
        raise LookupError(method)


def slow_seconds(value):
    try:
        seconds = float(value)
    except ValueError:
        raise argparse.ArgumentTypeError("--slow-ready requires a number from 0 through 80 seconds") from None
    if not math.isfinite(seconds) or not 0 <= seconds <= 80:
        raise argparse.ArgumentTypeError("--slow-ready must be finite and between 0 and 80 seconds")
    return seconds


def isolated_manifest(state):
    root = state.root
    home = root / "home"
    workspace = root / "workspace"
    environment = {
        "HOME": str(home),
        "KIRO_HOME": str(home / ".kiro"),
        "USERPROFILE": str(home),
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_CACHE_HOME": str(root / "cache"),
        "XDG_DATA_HOME": str(root / "data"),
        "XDG_STATE_HOME": str(root / "state"),
        "XDG_RUNTIME_DIR": str(root / "runtime"),
        "TMPDIR": str(root / "tmp"),
        "TMP": str(root / "tmp"),
        "TEMP": str(root / "tmp"),
    }
    directories = {Path(value) for value in environment.values()}
    directories.add(workspace / ".kiro" / "settings")
    directories.update(home / ".kiro" / name for name in ("settings", "hooks", "cache", "data", "tmp"))
    for directory in directories:
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    # Selected KAS consumes timeout in milliseconds, including catalog synchronization.
    servers = {"fixture-" + name: state.base + path for path, name in state.paths.items()}
    config = workspace / ".kiro" / "settings" / "mcp.json"
    config.write_text(json.dumps({"mcpServers": {
        name: {"url": url, "timeout": KAS_TIMEOUT_MILLISECONDS, "versionNegotiation": "legacy", "waitForReady": False}
        for name, url in servers.items()
    }}, indent=2) + "\n", encoding="utf-8")
    return {
        "root": str(root),
        "workspace": str(workspace),
        "base_url": state.base,
        "servers": servers,
        "environment": environment,
        "mcp_config": str(config),
        "request_log": str(state.log_path),
        "resource_metadata_url": state.base + PRM_PATH,
        "authorization_server_metadata_url": state.base + AS_PATH,
        "config_engine": "kas",
        "timeout_unit": "milliseconds",
        "timeout_milliseconds": KAS_TIMEOUT_MILLISECONDS,
        "slow_ready_seconds": state.slow_ready,
        "silent_timeout_seconds": SILENT_SECONDS if "silent" in state.paths.values() else None,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--silent", action="store_true", help="Add an MCP endpoint that waits up to 120 seconds; DELETE and signals cancel waits.")
    parser.add_argument("--slow-ready", type=slow_seconds, metavar="SECONDS", help="Add an MCP endpoint whose initialize waits 0..80 seconds.")
    args = parser.parse_args()
    os.umask(0o077)
    root = Path(tempfile.mkdtemp(prefix="cyril-mcp-fixture-")).resolve()
    state = FixtureState(root, args.silent, args.slow_ready)
    server = FixtureServer(state)
    state.base = "http://127.0.0.1:" + str(server.server_port)

    def stop(_signum, _frame):
        state.stop.set()

    previous = {sig: signal.signal(sig, stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    serving = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.1}, name="fixture-http", daemon=True)
    try:
        manifest = isolated_manifest(state)
        state.marker("fixture.started")
        serving.start()
        print(json.dumps(manifest, separators=(",", ":")), flush=True)
        state.stop.wait()
    finally:
        state.stop.set()
        state.marker("fixture.stopping")
        if serving.ident is not None:
            server.shutdown()
            serving.join()
        server.cancel_connections()
        server.server_close()
        with state.lock:
            state.clients.clear()
            state.codes.clear()
            state.tokens.clear()
        state.marker("fixture.stopped")
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        # Retain only this fixture's root for parent inspection; delete nothing.


if __name__ == "__main__":
    main()

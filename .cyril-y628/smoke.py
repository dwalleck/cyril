#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["pyte"]
# ///
"""Drive the real Cyril binary through a PTY and a controlled ACP agent.

This is evidence infrastructure, not a production adapter. The raw response
oracle deliberately knows only ACP JSON and the approved operator actions.
"""
import codecs
import fcntl
import json
import os
import pathlib
import pty
import select
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import time

REASON = '  Use printf — 雪.\nKeep\tquotes "ok".  '


def agent():
    evidence = pathlib.Path(os.environ["CYRIL_FEEDBACK_SMOKE_EVIDENCE"])
    prompt_id = None
    permission_index = 0

    def send(message):
        print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

    def permission():
        nonlocal permission_index
        permission_index += 1
        send({"id": 100 + permission_index, "method": "session/request_permission", "params": {
            "sessionId": "sess_feedback_smoke",
            "toolCall": {"toolCallId": f"tool-{permission_index}", "title": f"FEEDBACK_REQUEST_{permission_index}", "status": "pending", "kind": "execute", "rawInput": {"command": "printf safe"}},
            "options": [{"optionId": f"deny-{permission_index}", "name": "Reject once", "kind": "reject_once"}],
        }})

    for line in sys.stdin:
        message = json.loads(line)
        with (evidence / "wire.jsonl").open("a") as log:
            log.write(json.dumps(message, ensure_ascii=False) + "\n")
        method = message.get("method")
        ident = message.get("id")
        if method == "initialize":
            send({"id": ident, "result": {"protocolVersion": 1, "agentCapabilities": {"loadSession": True, "_meta": {"kiro": {}}}, "agentInfo": {"name": "feedback-smoke", "version": "0.66.8"}, "authMethods": []}})
        elif method == "session/new":
            send({"id": ident, "result": {"sessionId": "sess_feedback_smoke"}})
        elif method == "session/prompt":
            prompt_id = ident
            permission()
        elif method and ident is not None:
            send({"id": ident, "result": {}})
        elif not method and isinstance(ident, int) and 101 <= ident <= 105:
            (evidence / f"response-{ident - 100}.json").write_text(json.dumps(message, ensure_ascii=False, indent=2))
            if permission_index < 5:
                permission()
            else:
                send({"method": "session/update", "params": {"sessionId": "sess_feedback_smoke", "update": {"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "FEEDBACK_SMOKE_COMPLETE"}}}})
                send({"id": prompt_id, "result": {"stopReason": "end_turn"}})


def smoke(binary):
    import pyte

    root = pathlib.Path(__file__).resolve().parent
    evidence = pathlib.Path(os.environ.get("CYRIL_FEEDBACK_SMOKE_OUTPUT", root / "smoke-evidence"))
    evidence.mkdir(exist_ok=True)
    if any(evidence.glob("response-*.json")):
        raise RuntimeError("Choose a fresh smoke-evidence directory; previous receipts must not masquerade as this run")
    screen = pyte.Screen(100, 32)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder("utf-8")("replace")
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
    with tempfile.TemporaryDirectory(prefix="cyril-feedback-smoke-") as scratch:
        # Existing KAS Free-path fixture convention: isolate HOME, seed only
        # synthetic auth rows, and override the server rather than real login.
        store = pathlib.Path(scratch) / ".local/share/kiro-cli/data.sqlite3"
        store.parent.mkdir(parents=True)
        with sqlite3.connect(store) as database:
            database.execute("CREATE TABLE auth_kv (key TEXT PRIMARY KEY, value TEXT)")
            database.execute("CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT)")
            database.execute("INSERT INTO auth_kv VALUES (?, ?)", ("kirocli:odic:token", json.dumps({"access_token": "AT-synthetic-smoke", "expires_at": "2099-01-01T00:00:00Z"})))
            database.execute("INSERT INTO state VALUES (?, ?)", ("api.codewhisperer.profile", json.dumps({"arn": "arn:aws:codewhisperer:us-east-1:1:profile/SMOKE"})))
        server = pathlib.Path(scratch) / "mock-server.js"
        server.write_text(
            "const {spawn}=require('node:child_process');"
            f"const child=spawn({json.dumps(sys.executable)},[{json.dumps(str(pathlib.Path(__file__).resolve()))},'agent'],{{stdio:'inherit'}});"
            "child.on('exit',code=>process.exit(code ?? 1));"
            "process.on('SIGTERM',()=>child.kill('SIGTERM'));"
        )
        env = os.environ.copy()
        env.update(TERM="xterm-256color", COLORTERM="truecolor", HOME=scratch, USERPROFILE=scratch, XDG_CONFIG_HOME=scratch, KIRO_KAS_SERVER_PATH=str(server), CYRIL_FEEDBACK_SMOKE_EVIDENCE=str(evidence))
        process = subprocess.Popen([str(pathlib.Path(binary).resolve()), "--cwd", scratch, "--agent-engine", "kas", "--prompt", "Exercise rejection feedback"], stdin=slave, stdout=slave, stderr=slave, env=env, cwd=scratch, start_new_session=True)
        os.close(slave)
        raw = (evidence / "terminal.ansi").open("wb")
        started = time.monotonic()

        def drain(seconds=0.2):
            until = time.monotonic() + seconds
            while time.monotonic() < until:
                ready, _, _ = select.select([master], [], [], max(0, until - time.monotonic()))
                if not ready:
                    break
                try:
                    data = os.read(master, 65536)
                except OSError:
                    break
                if not data:
                    break
                raw.write(data)
                raw.flush()
                stream.feed(decoder.decode(data))

        def text():
            return "\n".join(screen.display)

        def await_text(needle):
            deadline = time.monotonic() + 20
            while needle not in text():
                if time.monotonic() > deadline or process.poll() is not None:
                    raise AssertionError(f"Missing {needle!r}; exit={process.poll()}\n{text()}")
                drain()

        def send(value):
            os.write(master, value.encode())
            drain()

        def capture(name):
            drain()
            (evidence / f"{name}.txt").write_text(text())

        def paste(value):
            send("\x1b[200~" + value + "\x1b[201~")

        try:
            await_text("FEEDBACK_REQUEST_1")
            capture("01-plain-choice")
            send("\r")
            await_text("FEEDBACK_REQUEST_2")
            send("r")
            first_line, second_line = REASON.split("\n", 1)
            paste(first_line)
            send("\n")  # Actual raw-mode Ctrl+J byte, not a fabricated KeyEvent.
            assert not (evidence / "response-2.json").exists(), "Ctrl+J submitted instead of inserting newline"
            paste(second_line)
            send("!")
            send("\x7f")
            # Raw enhanced-key bytes exercise Crossterm and the actual editor.
            # This proves event handling, not a native Windows keyboard layout.
            send("\x1b[64;7u\x1b[8364;8u\x1b[106;7u")
            capture("02-multiline-editor")
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 16, 44, 0, 0))
            screen.resize(lines=16, columns=44)
            os.killpg(process.pid, signal.SIGWINCH)
            drain(0.4)
            capture("03-cramped-editor")
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
            screen.resize(lines=32, columns=100)
            os.killpg(process.pid, signal.SIGWINCH)
            drain(0.4)
            send("\r")
            await_text("FEEDBACK_REQUEST_3")
            send("r")
            paste("x" * 4096)
            send("z")
            capture("04-limit-notice")
            send("\r")
            await_text("FEEDBACK_REQUEST_4")
            send("r")
            paste(" \n\t ")
            send("\r")
            await_text("FEEDBACK_REQUEST_5")
            send("r")
            paste("never submitted")
            send("\x1b")
            capture("05-escape-back")
            assert not (evidence / "response-5.json").exists(), "Esc from editor answered the permission"
            send("\r")
            await_text("FEEDBACK_SMOKE_COMPLETE")
            capture("06-complete")
            expected_reasons = {2: REASON + "@€j", 3: "x" * 4096}
            for index in range(1, 6):
                response = json.loads((evidence / f"response-{index}.json").read_text())
                expected: dict[str, object] = {"outcome": {"outcome": "selected", "optionId": f"deny-{index}"}}
                if index in expected_reasons:
                    expected["_meta"] = {"kiro": {"rejectionReason": expected_reasons[index]}}
                assert response.get("result") == expected, f"C2/C5/C6 response-{index}: {response} != {expected}"
            summary = {"result": "PASS", "binary": str(pathlib.Path(binary).resolve()), "seconds": round(time.monotonic() - started, 3), "responses": 5, "scenarios": ["plain reject", "multiline paste and edit", "AltGr-shaped raw key events", "cramped viewport", "4096-scalar boundary and overflow", "whitespace plain reject", "Esc-back without decision"], "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root.parent, text=True).strip()}
            (evidence / "result.json").write_text(json.dumps(summary, indent=2))
            print(json.dumps(summary, indent=2))
        finally:
            if process.poll() is None:
                os.write(master, b"\x11")
                drain(0.3)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)
            raw.close()
            os.close(master)


if __name__ == "__main__":
    if "agent" in sys.argv:
        agent()
    elif len(sys.argv) == 2:
        smoke(sys.argv[1])
    else:
        raise SystemExit("usage: uv run --script .cyril-y628/smoke.py /path/to/cyril")

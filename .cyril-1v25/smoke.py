#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["pyte"]
# ///
"""Replay captured orphan tool frames through the real Cyril process and TUI.

Fixture-backed wiring proof, not a new live-upstream qualification. All account,
workspace, auth-helper and server effects are confined to an owned temp directory.
"""
import argparse
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

BEFORE = "BEFORE_TOOL_ROWS"
AFTER = "AFTER_TOOL_ROWS"


def session_marker(marker, number, two_sessions):
    return f"{marker}_SESSION_{number}" if two_sessions else marker


def agent():
    engine = os.environ["CYRIL_TOOL_SMOKE_ENGINE"]
    evidence = Path(os.environ["CYRIL_TOOL_SMOKE_OUTPUT"])
    capture = Path(os.environ["CYRIL_TOOL_SMOKE_CAPTURE"])
    session = "sess_missing_updates" if engine == "kas" else "70000000-0000-4000-8000-000000000001"
    two_sessions = os.environ.get("CYRIL_TOOL_SMOKE_TWO_SESSIONS") == "1"
    preserve_session_id = os.environ.get("CYRIL_TOOL_SMOKE_PRESERVE_SESSION_ID") == "1"
    if preserve_session_id:
        captured_sessions = {
            json.loads(line)["msg"]["params"]["sessionId"]
            for line in capture.read_text().splitlines()
        }
        if len(captured_sessions) != 1 or two_sessions:
            raise AssertionError("unchanged-frame replay requires one captured session")
        session = captured_sessions.pop()
    session_count = 0

    def send(message):
        frame = {"jsonrpc": "2.0", **message}
        with (evidence / "agent-wire.jsonl").open("a") as log:
            log.write(json.dumps(frame, ensure_ascii=False) + "\n")
        print(json.dumps(frame, ensure_ascii=False), flush=True)

    def update(value):
        send({"method": "session/update", "params": {"sessionId": session, "update": value}})

    def text(value):
        update({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": value}})

    for line in sys.stdin:
        frame = json.loads(line)
        if two_sessions:
            with (evidence / "client-wire.jsonl").open("a") as log:
                log.write(json.dumps(frame, ensure_ascii=False) + "\n")
        method = frame.get("method")
        ident = frame.get("id")
        if method == "initialize":
            capabilities: dict[str, object] = {"loadSession": True}
            if engine == "kas":
                capabilities["_meta"] = {"kiro": {}}
            send({"id": ident, "result": {"protocolVersion": 1, "agentCapabilities": capabilities,
                  "agentInfo": {"name": "missing-tool-smoke", "version": "0.66.43"}, "authMethods": []}})
        elif method == "session/new":
            session_count += 1
            if two_sessions:
                session = (
                    f"sess_missing_updates_{session_count}" if engine == "kas"
                    else f"70000000-0000-4000-8000-{session_count:012d}"
                )
            send({"id": ident, "result": {"sessionId": session}})
        elif method == "session/prompt":
            text(session_marker(BEFORE, session_count, two_sessions) + "\n")
            repeated = []
            for recorded_line in capture.read_text().splitlines():
                recorded = json.loads(recorded_line)["msg"]
                if "id" in recorded or "method" not in recorded:
                    raise AssertionError("smoke fixture must contain only inbound notifications")
                if not preserve_session_id:
                    recorded["params"]["sessionId"] = session
                send(recorded)
                if recorded["params"]["update"]["sessionUpdate"] == "tool_call_update":
                    repeated.append(recorded)
            for repeated_update in repeated:
                send(repeated_update)
            text("\n" + session_marker(AFTER, session_count, two_sessions))
            if engine == "kas":
                update({"sessionUpdate": "session_info_update", "_meta": {"kiro": {
                    "turnEnd": {"stopReason": "end_turn"}, "kind": "turn_end", "stopReason": "end_turn"
                }}})
            send({"id": ident, "result": {"stopReason": "end_turn"}})
        elif method and ident is not None:
            send({"id": ident, "result": {}})


def smoke(args):
    import pyte

    binary = Path(args.binary).resolve(strict=True)
    capture = Path(args.capture).resolve(strict=True)
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    node = shutil.which("node") if args.engine == "kas" else None
    if args.engine == "kas" and node is None:
        raise RuntimeError("KAS smoke requires an installed Node executable")
    script = Path(__file__).resolve()
    repository = script.parent.parent
    fixture = repository / "crates/cyril-core/tests/fixtures/fake-kiro-cli.sh"
    screen = pyte.Screen(180, 55)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder("utf-8")("replace")

    with tempfile.TemporaryDirectory(prefix="cyril-missing-tools-") as scratch_name:
        scratch = Path(scratch_name)
        for name in ["bin", "home", "config", "data", "ws"]:
            (scratch / name).mkdir()
        (scratch / "bin/kiro-cli").symlink_to(fixture)
        server = scratch / "server.js"
        server.write_text(
            "const {spawn}=require('node:child_process');"
            f"const child=spawn({json.dumps(sys.executable)},[{json.dumps(str(script))},'agent'],{{stdio:'inherit'}});"
            "child.on('exit',code=>process.exit(code ?? 1));"
            "process.on('SIGTERM',()=>child.kill('SIGTERM'));"
        )
        environment = os.environ.copy()
        environment.update(
            TERM="xterm-256color", COLORTERM="truecolor", PATH=str(scratch / "bin"),
            SHELL="/bin/bash",
            HOME=str(scratch / "home"), USERPROFILE=str(scratch / "home"),
            XDG_CONFIG_HOME=str(scratch / "config"), XDG_DATA_HOME=str(scratch / "data"),
            CYRIL_TOOL_SMOKE_ENGINE=args.engine, CYRIL_TOOL_SMOKE_CAPTURE=str(capture),
            CYRIL_TOOL_SMOKE_OUTPUT=str(output),
            CYRIL_TOOL_SMOKE_TWO_SESSIONS="1" if args.two_sessions else "0",
            CYRIL_TOOL_SMOKE_PRESERVE_SESSION_ID="1" if args.preserve_session_id else "0",
        )
        for key in ["KIRO_AGENT_PATH", "KIRO_KAS_SERVER_PATH", "CYRIL_AGENT_LOCATION", "CYRIL_AGENT_HOST_OS"]:
            environment.pop(key, None)
        if node:
            environment.update(KIRO_AGENT_PATH=str(Path(node).resolve()), KIRO_KAS_SERVER_PATH=str(server))
        command = [str(binary), "--cwd", str(scratch / "ws"), "--agent-engine", args.engine,
                   "--prompt", "Replay missing-start tool rows"]
        if args.engine == "v2":
            command.extend(["--agent-command", sys.executable, str(script), "agent"])
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 55, 180, 0, 0))
        try:
            process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave,
                                       env=environment, cwd=scratch / "ws", start_new_session=True)
        finally:
            os.close(slave)
        raw = (output / "terminal.ansi").open("wb")

        def drain(seconds=0.2):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                ready, _, _ = select.select([master], [], [], max(0, deadline - time.monotonic()))
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

        def visible():
            return "\n".join(screen.display)

        def wire_rows(name):
            path = output / name
            if not path.exists():
                return []
            return [
                json.loads(line)
                for line in path.read_text().splitlines(keepends=True)
                if line.endswith("\n")
            ]

        def created_sessions():
            return [
                frame["result"]["sessionId"]
                for frame in wire_rows("agent-wire.jsonl")
                if "sessionId" in frame.get("result", {})
            ]

        def prompt_completed(session):
            prompt_ids = {
                frame["id"]
                for frame in wire_rows("client-wire.jsonl")
                if frame.get("method") == "session/prompt"
                and frame["params"]["sessionId"] == session
            }
            return any(
                frame.get("id") in prompt_ids
                and frame.get("result", {}).get("stopReason") == "end_turn"
                for frame in wire_rows("agent-wire.jsonl")
            )

        def await_observation(predicate, description):
            deadline = time.monotonic() + 30
            while not predicate():
                if process.poll() is not None or time.monotonic() > deadline:
                    raise AssertionError(f"Missing {description}; exit={process.poll()}\n{visible()}")
                drain()

        def session_block(text, number):
            before = session_marker(BEFORE, number, True)
            after = session_marker(AFTER, number, True)
            if text.count(before) != 1 or text.count(after) != 1:
                raise AssertionError(f"Expected one marker pair for session {number}\n{text}")
            start, end = text.index(before), text.index(after)
            if start >= end:
                raise AssertionError(f"Wrong session {number} marker chronology\n{text}")
            block = text[start:end]
            for expected in args.expect:
                if block.count(expected) != 1:
                    raise AssertionError(f"Session {number} needs one {expected!r}\n{text}")
            return block

        scenario_receipt: dict[str, object] = {}
        try:
            if args.two_sessions:
                first_after = session_marker(AFTER, 1, True)
                await_observation(
                    lambda: len(created_sessions()) == 1
                    and prompt_completed(created_sessions()[0])
                    and first_after in visible(),
                    "first session completed on wire and in the TUI",
                )
                first_text = visible()
                first_block = session_block(first_text, 1)
                (output / "screen-session-1.txt").write_text(first_text)
                os.write(master, b"\x1b[200~/new\x1b[201~\r")
                await_observation(
                    lambda: len(created_sessions()) == 2
                    and f"Session created: {created_sessions()[1]}" in visible(),
                    "the /new result applied to the actual TUI",
                )
                sessions = created_sessions()
                if len(set(sessions)) != 2:
                    raise AssertionError(f"Peer must mint distinct session IDs: {sessions}")
                os.write(
                    master,
                    b"\x1b[200~Replay the same tool IDs in the second session\x1b[201~\r",
                )
                second_after = session_marker(AFTER, 2, True)
                await_observation(
                    lambda: prompt_completed(sessions[1]) and second_after in visible(),
                    "second ordinary prompt completed on wire and in the TUI",
                )
                text = visible()
                if session_block(text, 1) != first_block:
                    raise AssertionError(f"Second session changed the first session's tool history\n{text}")
                session_block(text, 2)
                if not text.index(first_after) < text.index(session_marker(BEFORE, 2, True)):
                    raise AssertionError(f"Session histories are out of order\n{text}")
                update_ids = {session: set() for session in sessions}
                for frame in wire_rows("agent-wire.jsonl"):
                    params = frame.get("params", {})
                    update = params.get("update", {})
                    if update.get("sessionUpdate") == "tool_call_update":
                        update_ids[params["sessionId"]].add(update["toolCallId"])
                if not update_ids[sessions[0]] or update_ids[sessions[0]] != update_ids[sessions[1]]:
                    raise AssertionError(f"The two sessions must reuse the captured tool IDs: {update_ids}")
                scenario_receipt.update(
                    scenario="two-sessions-reused-tool-ids",
                    sessions=sessions,
                    tool_ids=sorted(update_ids[sessions[0]]),
                    expected_count_per_label=2,
                )
            else:
                deadline = time.monotonic() + 30
                while AFTER not in visible():
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise AssertionError(f"TUI replay did not finish; exit={process.poll()}\n{visible()}")
                    drain()
                drain(0.5)
                text = visible()
            (output / "screen.txt").write_text(text)
            expected_count = 2 if args.two_sessions else 1
            for expected in args.expect:
                if text.count(expected) != expected_count:
                    raise AssertionError(f"Expected exactly {expected_count} {expected!r}\n{text}")
                if not args.two_sessions and not text.index(BEFORE) < text.index(expected) < text.index(AFTER):
                    raise AssertionError(f"Wrong arrival chronology for {expected!r}\n{text}")
            if args.preserve_session_id:
                wire = wire_rows("agent-wire.jsonl")
                for line in capture.read_text().splitlines():
                    original = json.loads(line)["msg"]
                    expected_copies = (
                        2 if original["params"]["update"]["sessionUpdate"] == "tool_call_update"
                        else 1
                    )
                    if wire.count(original) != expected_copies:
                        raise AssertionError("peer did not replay the captured frame unchanged")
                scenario_receipt["captured_frames_preserved"] = True
            receipt = {"result": "PASS", "binary": str(binary), "engine": args.engine,
                       "capture": str(capture), "expected": args.expect,
                       "proof": "real Cyril binary, controlled ACP peer, PTY-decoded TUI"}
            receipt.update(scenario_receipt)
            (output / "result.json").write_text(json.dumps(receipt, indent=2))
            print(json.dumps(receipt, indent=2))
        finally:
            if not (output / "screen.txt").exists():
                (output / "screen.txt").write_text(visible())
            if process.poll() is None:
                os.write(master, b"\x11")
                drain(0.3)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=5)
            raw.close()
            os.close(master)
            log = scratch / "home/.config/cyril/cyril.log"
            if log.exists():
                shutil.copyfile(log, output / "cyril.log")
            helper_log = scratch / "bin/kiro-cli-argv.txt"
            if helper_log.exists():
                shutil.copyfile(helper_log, output / "kiro-cli-argv.txt")


if __name__ == "__main__":
    if sys.argv[1:] == ["agent"]:
        agent()
    else:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--binary", required=True)
        parser.add_argument("--engine", choices=["kas", "v2"], required=True)
        parser.add_argument("--capture", required=True)
        parser.add_argument("--expect", action="append", required=True)
        session_options = parser.add_mutually_exclusive_group()
        session_options.add_argument("--two-sessions", action="store_true", help="drive /new, then reuse the same captured tool IDs in a second ordinary prompt")
        session_options.add_argument("--preserve-session-id", action="store_true", help="return the fixture's sole session ID from session/new and replay every captured frame unchanged")
        parser.add_argument("--output", required=True, help="fresh evidence directory (must not exist)")
        smoke(parser.parse_args())

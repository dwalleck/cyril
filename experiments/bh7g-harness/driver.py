#!/usr/bin/env python3
"""bh7g driver: runs the instrumented cyril-core Wayfinder probe with automated
answers until the missing-terminal wedge reproduces (or a round/run cap is hit).

Per run it produces, under captures/run-N/:
  wire.jsonl    — raw ACP frames both directions ({ts, dir, msg}, via tap.py)
  stdout.jsonl  — consumer view: stamped chunks of probe stdout ({ts, text})
  stderr.jsonl  — cyril-core tracing (mediator dispositions) + KAS stderr, stamped
  verdict.json  — {outcome: completed|wedged|cap|spawn-error, rounds, ...}

Wedge definition: after we submit an answer (a new turn starts), no probe stdout
at all for WEDGE_SILENCE seconds AFTER streamed text has stopped, with no
"turn_completed" event line since the answer. Turns today ran 18-45s.
"""

import json
import os
import pathlib
import selectors
import signal
import subprocess
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
PROBE_BIN = HERE / "target/debug/bh7g-probe"
ANSWER = (
    "Use the stated recommended answer for every question in this round. "
    "If a question has no stated recommendation, choose the simplest viable option and note it.\n"
)
WEDGE_SILENCE = 210  # seconds of total stdout silence after last output before declaring wedge
RUN_CAP_S = 1800     # absolute per-run wall clock
MAX_ROUNDS = 10
MAX_RUNS = int(os.environ.get("BH7G_MAX_RUNS", "4"))

ANSWER_PROMPT = b"Answer this round; preserve Q numbers: "
ORDINAL_PROMPT = b"Choose an ordinal: "


def run_once(idx: int) -> dict:
    run_dir = HERE / "captures" / f"run-{idx}"
    run_dir.mkdir(parents=True, exist_ok=True)
    fixture = run_dir / "fixture"
    fixture.mkdir(exist_ok=True)
    if not (fixture / ".rivets").is_dir():
        subprocess.run(
            ["rivets", "init", "--prefix", "trailprobe", "--yes", "--quiet"],
            cwd=fixture, check=True,
        )

    env = os.environ.copy()
    env["KIRO_AGENT_PATH"] = str(HERE / "node-shim.sh")
    env["BH7G_TAP_LOG"] = str(run_dir / "wire.jsonl")
    env["RUST_LOG"] = "cyril_core=debug"
    # Pin the KAS bundle: kiro-cli upgraded to 2.17.0 mid-research; once its
    # assets self-extract, unpinned spawns would silently switch bundles.
    env["KIRO_KAS_SERVER_PATH"] = str(
        pathlib.Path.home()
        / ".local/share/kiro-cli/kas/2.16.2-7148833c96036873df6f5a5ae0e54cf433f11bd9db1a5d788c4ff7db941bceeb/node_modules/@kiro/agent/dist/server/acp-server.js"
    )

    out_log = open(run_dir / "stdout.jsonl", "w", buffering=1)
    err_log = open(run_dir / "stderr.jsonl", "w", buffering=1)

    proc = subprocess.Popen(
        [str(PROBE_BIN), str(fixture)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        env=env, start_new_session=True,
    )
    sel = selectors.DefaultSelector()
    os.set_blocking(proc.stdout.fileno(), False)
    os.set_blocking(proc.stderr.fileno(), False)
    sel.register(proc.stdout, selectors.EVENT_READ, "out")
    sel.register(proc.stderr, selectors.EVENT_READ, "err")

    t0 = time.time()
    tail = b""
    rounds_answered = 0
    turns_completed = 0
    last_output = time.time()
    answered_since_turn_end = False
    outcome = None

    def stamp(log, text):
        log.write(json.dumps({"ts": time.time(), "text": text}) + "\n")

    while outcome is None:
        if proc.poll() is not None:
            # drain, then classify by exit
            time.sleep(0.3)
            for key, _ in sel.select(timeout=0):
                data = key.fileobj.read()
                if data:
                    stamp(out_log if key.data == "out" else err_log, data.decode("utf-8", "replace"))
                    if key.data == "out":
                        tail += data
            outcome = "completed" if b"PERSISTED_WAYFINDER_MAP" in tail else f"exited-rc-{proc.returncode}"
            break
        if time.time() - t0 > RUN_CAP_S:
            outcome = "run-cap"
            break
        events = sel.select(timeout=5)
        for key, _ in events:
            data = key.fileobj.read()
            if not data:
                continue
            text = data.decode("utf-8", "replace")
            stamp(out_log if key.data == "out" else err_log, text)
            if key.data != "out":
                continue
            last_output = time.time()
            tail = (tail + data)[-16384:]
            if b'"event":"turn_completed"' in data or b'"event": "turn_completed"' in data:
                turns_completed += 1
                answered_since_turn_end = False
            if tail.endswith(ANSWER_PROMPT):
                if rounds_answered >= MAX_ROUNDS:
                    outcome = "round-cap"
                    break
                proc.stdin.write(ANSWER.encode())
                proc.stdin.flush()
                rounds_answered += 1
                answered_since_turn_end = True
                tail = b""
                print(f"  [run {idx}] answered round {rounds_answered}", flush=True)
            elif tail.endswith(ORDINAL_PROMPT):
                proc.stdin.write(b"1\n")
                proc.stdin.flush()
                tail = b""
                print(f"  [run {idx}] chose ordinal 1", flush=True)
        if not events and time.time() - last_output > WEDGE_SILENCE:
            outcome = "wedged"
            break

    cancel_result = None
    if outcome == "wedged":
        # cyril-14ou arm: cancel the stalled turn and watch what comes back.
        print(f"  [run {idx}] wedge detected — injecting CANCEL", flush=True)
        try:
            proc.stdin.write(b"CANCEL\n")
            proc.stdin.flush()
        except (BrokenPipeError, ValueError):
            cancel_result = "stdin-dead"
        if cancel_result is None:
            deadline = time.time() + 90
            seen = b""
            while time.time() < deadline:
                for key, _ in sel.select(timeout=5):
                    data = key.fileobj.read()
                    if data:
                        stamp(out_log if key.data == "out" else err_log, data.decode("utf-8", "replace"))
                        if key.data == "out":
                            seen += data
                if b'"event":"turn_completed"' in seen or b'"event": "turn_completed"' in seen:
                    cancel_result = "turn_completed-after-cancel"
                    break
                if proc.poll() is not None:
                    cancel_result = f"probe-exited-rc-{proc.returncode}"
                    break
            else:
                cancel_result = "no-terminal-within-90s"
            tail_txt = seen.decode("utf-8", "replace")[-500:]
        else:
            tail_txt = ""

    kas_log_dirs = sorted(pathlib.Path.home().glob(".kiro/logs/*"), key=lambda p: p.name)
    verdict = {
        "outcome": outcome,
        "rounds_answered": rounds_answered,
        "turns_completed": turns_completed,
        "elapsed_s": round(time.time() - t0, 1),
        "answered_since_last_turn_end": answered_since_turn_end,
        "cancel_result": cancel_result,
        "cancel_tail": tail_txt if outcome == "wedged" else None,
        "kas_log_dir": str(kas_log_dirs[-1]) if kas_log_dirs else None,
    }
    (HERE / "captures" / f"run-{idx}" / "verdict.json").write_text(json.dumps(verdict, indent=2))
    # tear down the whole process group (probe + shim + node)
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        time.sleep(2)
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    return verdict


def main():
    for i in range(1, MAX_RUNS + 1):
        print(f"[bh7g] run {i} starting", flush=True)
        v = run_once(i)
        print(f"[bh7g] run {i}: {json.dumps(v)}", flush=True)
        if v["outcome"].startswith("exited-rc") and v["turns_completed"] == 0:
            print("[bh7g] probe failed before first turn (spawn/auth error?) — stopping", flush=True)
            return
    print("[bh7g] no wedge captured within run cap", flush=True)


if __name__ == "__main__":
    main()

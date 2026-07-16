#!/usr/bin/env python3
"""cyril-a14l ORACLE: run the REAL cyril binary on a real 60x16 pty and
reconstruct the final screen with pyte (an independent VT100 emulator).

Independent mechanism vs the probe: real binary + crossterm + real App/UiState
+ pyte, instead of TestBackend + MockTuiState. Agreement target: geometry
(input border rows, suggestion rows, status row), not exact chat content.

Usage: venv/bin/python oracle-pty.py <path-to-cyril> <scenario> <workdir>
  scenario "draft":  bracketed-paste a 10-line draft
  scenario "at":     type "@probe" to trigger file autocomplete
"""

import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time

import pyte

COLS, ROWS = 60, 16


def main() -> None:
    binary, scenario, workdir = sys.argv[1], sys.argv[2], sys.argv[3]

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    proc = subprocess.Popen(
        [binary, "--agent-command", "sleep", "300"],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        cwd=workdir,
        start_new_session=True,
        env={**os.environ, "TERM": "xterm-256color"},
    )
    os.close(slave)

    screen = pyte.Screen(COLS, ROWS)
    stream = pyte.ByteStream(screen)

    def pump(seconds: float) -> None:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master], [], [], 0.1)
            if master in ready:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    return
                if not data:
                    return
                stream.feed(data)

    pump(5.0)  # initial render

    if scenario == "draft":
        draft = "\n".join(f"draft-{i}" for i in range(1, 11))
        os.write(master, b"\x1b[200~" + draft.encode() + b"\x1b[201~")
    elif scenario == "at":
        for ch in "@probe":
            os.write(master, ch.encode())
            pump(0.15)
    else:
        raise SystemExit(f"unknown scenario {scenario}")

    pump(3.0)  # let the frame settle

    print(f"=== ORACLE {scenario} ({COLS}x{ROWS}) ===")
    for y, line in enumerate(screen.display):
        print(f"{y:2}|{line}|")

    os.killpg(proc.pid, 9)


if __name__ == "__main__":
    main()

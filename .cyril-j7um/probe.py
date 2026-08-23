#!/usr/bin/env python3
"""Probe M0's Unix lock, SQLite reopen, and private socket premises."""

from __future__ import annotations

import argparse
import fcntl
import json
import os
from pathlib import Path
import signal
import socket
import sqlite3
import subprocess
import sys
import time


def wait_for(path: Path) -> None:
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.01)
    raise RuntimeError(f"timed out waiting for {path}")


def hold_lock(path: Path, ready: Path) -> None:
    with path.open("a+b") as lock_file:
        fcntl.flock(lock_file, fcntl.LOCK_EX)
        ready.touch()
        time.sleep(30)


def hold_sqlite(path: Path, ready: Path) -> None:
    connection = sqlite3.connect(path, timeout=1)
    connection.execute("BEGIN IMMEDIATE")
    connection.execute("UPDATE schema_version SET version = 2 WHERE singleton = 1")
    ready.touch()
    time.sleep(30)


def initialize_store(path: Path) -> str:
    connection = sqlite3.connect(path)
    try:
        journal_mode = connection.execute("PRAGMA journal_mode = WAL").fetchone()[0]
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute(
            "CREATE TABLE schema_version ("
            "singleton INTEGER PRIMARY KEY CHECK (singleton = 1), "
            "version INTEGER NOT NULL CHECK (version > 0))"
        )
        connection.execute("INSERT INTO schema_version VALUES (1, 1)")
        connection.commit()
        return str(journal_mode)
    finally:
        connection.close()


def run(root: Path) -> None:
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(root, 0o700)
    script = Path(__file__).resolve()

    lock_path = root / "runtime.lock"
    lock_ready = root / "lock.ready"
    lock_holder = subprocess.Popen(
        [sys.executable, str(script), "--hold-lock", str(lock_path), str(lock_ready)]
    )
    wait_for(lock_ready)
    with lock_path.open("a+b") as contender:
        try:
            fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
            blocked_while_held = False
        except BlockingIOError:
            blocked_while_held = True
    lock_holder.send_signal(signal.SIGKILL)
    lock_holder.wait(timeout=5)
    with lock_path.open("a+b") as after_exit:
        fcntl.flock(after_exit, fcntl.LOCK_EX | fcntl.LOCK_NB)
        reacquired_after_kill = True
        fcntl.flock(after_exit, fcntl.LOCK_UN)

    memory_path = root / "memory.sqlite3"
    knowledge_path = root / "knowledge.sqlite3"
    memory_journal = initialize_store(memory_path)
    knowledge_journal = initialize_store(knowledge_path)
    sqlite_ready = root / "sqlite.ready"
    sqlite_holder = subprocess.Popen(
        [sys.executable, str(script), "--hold-sqlite", str(memory_path), str(sqlite_ready)]
    )
    wait_for(sqlite_ready)
    sqlite_holder.send_signal(signal.SIGKILL)
    sqlite_holder.wait(timeout=5)

    reopened_versions: dict[str, int] = {}
    reopened_journals: dict[str, str] = {}
    for name, path in (("memory", memory_path), ("knowledge", knowledge_path)):
        connection = sqlite3.connect(path)
        try:
            reopened_versions[name] = int(
                connection.execute(
                    "SELECT version FROM schema_version WHERE singleton = 1"
                ).fetchone()[0]
            )
            reopened_journals[name] = str(
                connection.execute("PRAGMA journal_mode").fetchone()[0]
            )
        finally:
            connection.close()

    socket_path = root / "runtime.sock"
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        listener.bind(str(socket_path))
        os.chmod(socket_path, 0o600)
        socket_mode = oct(socket_path.stat().st_mode & 0o777)
    finally:
        listener.close()

    result = {
        "lock": {
            "blocked_while_held": blocked_while_held,
            "reacquired_after_kill": reacquired_after_kill,
        },
        "root_mode": oct(root.stat().st_mode & 0o777),
        "socket_mode": socket_mode,
        "sqlite": {
            "initial_journals": {
                "knowledge": knowledge_journal,
                "memory": memory_journal,
            },
            "reopened_journals": reopened_journals,
            "reopened_versions": reopened_versions,
        },
    }
    print(json.dumps(result, indent=2, sort_keys=True))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", nargs="?", type=Path)
    parser.add_argument("--hold-lock", nargs=2, type=Path)
    parser.add_argument("--hold-sqlite", nargs=2, type=Path)
    args = parser.parse_args()

    if args.hold_lock is not None:
        hold_lock(*args.hold_lock)
        return
    if args.hold_sqlite is not None:
        hold_sqlite(*args.hold_sqlite)
        return
    if args.root is None:
        parser.error("root is required")
    run(args.root)


if __name__ == "__main__":
    main()

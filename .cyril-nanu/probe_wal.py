#!/usr/bin/env python3
"""P2 — can a second connection hold a consistent read while the writer commits?

D5 requires the nine rollups to run inside one deferred read transaction on a
connection separate from the writer's, with the writer still appending on the
event loop. Three things must hold:

  1. the reader sees ONE point in time for the whole transaction, even though a
     commit lands in the middle of it;
  2. the writer is never blocked into SQLITE_BUSY by the open reader;
  3. after the reader commits, it sees the new row (so it is a snapshot, not a
     stale cache).

Mechanism: two rusqlite-equivalent connections in ONE process via Python's
sqlite3, with the same pragmas UsageLog sets (usage.rs:716-726) — journal_mode
WAL and a 250 ms busy timeout.

Control: the same scenario in journal_mode=DELETE. If WAL and DELETE behave
identically, this probe is not measuring what it claims and the result is void.

Data: generated to production shape in a temp dir; never touches the operator's
real usage.sqlite3.

Run: python3 .cyril-nanu/probe_wal.py
"""
import sqlite3
import tempfile
from pathlib import Path

SEED_ROWS = 1000
BUSY_TIMEOUT_MS = 250          # usage.rs:51 BUSY_TIMEOUT


def connect(path, mode):
    conn = sqlite3.connect(str(path), isolation_level=None, timeout=BUSY_TIMEOUT_MS / 1000)
    conn.execute(f"PRAGMA journal_mode = {mode};")
    conn.execute(f"PRAGMA busy_timeout = {BUSY_TIMEOUT_MS};")
    return conn


def scenario(mode):
    """Returns (c1, c2, c3, writer_error) for one journal mode."""
    with tempfile.TemporaryDirectory(prefix="cyril-nanu-wal-") as tmp:
        db = Path(tmp) / "usage.sqlite3"
        setup = connect(db, mode)
        setup.execute(
            "CREATE TABLE usage_turns (id INTEGER PRIMARY KEY, session_id TEXT NOT NULL,"
            " duration_ms INTEGER NOT NULL)"
        )
        setup.executemany(
            "INSERT INTO usage_turns (session_id, duration_ms) VALUES (?, ?)",
            [(f"s{i}", i % 500) for i in range(SEED_ROWS)],
        )
        setup.close()

        reader = connect(db, mode)
        writer = connect(db, mode)

        # 1. Reader opens a deferred read transaction and takes its first read.
        reader.execute("BEGIN DEFERRED")
        c1 = reader.execute("SELECT COUNT(*) FROM usage_turns").fetchone()[0]

        # 2. Writer commits a row while that transaction is still open.
        writer_error = None
        try:
            writer.execute("BEGIN IMMEDIATE")
            writer.execute(
                "INSERT INTO usage_turns (session_id, duration_ms) VALUES ('mid-snapshot', 42)"
            )
            writer.execute("COMMIT")
        except sqlite3.Error as error:            # SQLITE_BUSY lands here
            writer_error = f"{type(error).__name__}: {error}"
            try:
                writer.execute("ROLLBACK")
            except sqlite3.Error:
                pass

        # 3. Reader's SECOND read inside the same transaction — the isolation test.
        c2 = reader.execute("SELECT COUNT(*) FROM usage_turns").fetchone()[0]
        reader.execute("COMMIT")

        # 4. After committing, the reader must observe the new state.
        c3 = reader.execute("SELECT COUNT(*) FROM usage_turns").fetchone()[0]
        reader.close()
        writer.close()
        return c1, c2, c3, writer_error


def main():
    for mode in ("WAL", "DELETE"):
        c1, c2, c3, writer_error = scenario(mode)
        consistent = c1 == c2
        writer_ok = writer_error is None
        advanced = c3 == c1 + 1 if writer_ok else c3 == c1
        print(f"--- journal_mode = {mode}")
        print(f"  count at read 1 (txn open)   : {c1}")
        print(f"  count at read 2 (same txn)   : {c2}")
        print(f"  count after txn commit       : {c3}")
        print(f"  writer error                 : {writer_error or 'none'}")
        print(f"  CONSISTENT (read1 == read2)  : {consistent}")
        print(f"  WRITER UNBLOCKED             : {writer_ok}")
        print(f"  READER ADVANCES after commit : {advanced}")


if __name__ == "__main__":
    main()

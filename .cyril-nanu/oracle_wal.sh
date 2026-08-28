#!/usr/bin/env bash
# P2 oracle — same answer, different failure mechanism.
#
# The probe uses two connections inside ONE process via Python's sqlite3
# binding. This oracle uses two separate OS PROCESSES driving the `sqlite3`
# CLI, coordinated through a FIFO so the writer commits while the reader's
# deferred transaction is demonstrably still open. Different binary, different
# binding, different process model: an in-process special case, a Python
# binding quirk, or a mis-sequenced transaction in the probe shows up here.
#
# Run: .cyril-nanu/oracle_wal.sh
set -uo pipefail

run_mode() {
	local mode=$1
	local tmp; tmp=$(mktemp -d -t cyril-nanu-wal-XXXXXX)
	local db="$tmp/usage.sqlite3" ctl="$tmp/ctl" out="$tmp/reader.out"

	sqlite3 "$db" "
		PRAGMA journal_mode = $mode;
		CREATE TABLE usage_turns (id INTEGER PRIMARY KEY, session_id TEXT NOT NULL, duration_ms INTEGER NOT NULL);
		WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n < 1000)
		INSERT INTO usage_turns (session_id, duration_ms) SELECT 's'||n, n%500 FROM seq;
	" >/dev/null

	mkfifo "$ctl"
	sqlite3 "$db" < "$ctl" > "$out" 2>&1 &
	local reader_pid=$!
	exec 3>"$ctl"

	# Reader: open a deferred transaction and take its first read.
	# Sentinel-tagged reads: `PRAGMA busy_timeout` echoes its own value as a
	# result row, so positional line parsing silently reads the pragma echo as
	# the first count. Tag each read instead.
	printf 'PRAGMA busy_timeout = 250;\nBEGIN DEFERRED;\nSELECT '"'"'C1='"'"'||COUNT(*) FROM usage_turns;\n' >&3
	sleep 0.4

	# Writer: a SEPARATE process commits while that transaction is open.
	local writer_err writer_rc
	# Capture sqlite3's OWN status before filtering: piping through `grep -v`
	# to drop the pragma echo makes grep's "nothing matched" exit code (1)
	# masquerade as a writer failure on the success path.
	writer_raw=$(sqlite3 "$db" "PRAGMA busy_timeout = 250; INSERT INTO usage_turns (session_id, duration_ms) VALUES ('mid-snapshot', 42);" 2>&1)
	writer_rc=$?
	writer_err=$(printf '%s\n' "$writer_raw" | grep -v '^250$' | grep -v '^$' || true)

	# Reader: second read in the SAME transaction, then commit and read again.
	printf 'SELECT '"'"'C2='"'"'||COUNT(*) FROM usage_turns;\nCOMMIT;\nSELECT '"'"'C3='"'"'||COUNT(*) FROM usage_turns;\n' >&3
	exec 3>&-
	wait "$reader_pid" 2>/dev/null
	rm -f "$ctl"

	local c1 c2 c3
	c1=$(grep -m1 '^C1=' "$out" | cut -d= -f2)
	c2=$(grep -m1 '^C2=' "$out" | cut -d= -f2)
	c3=$(grep -m1 '^C3=' "$out" | cut -d= -f2)
	echo "--- journal_mode = $mode"
	echo "  count at read 1 (txn open)   : $c1"
	echo "  count at read 2 (same txn)   : $c2"
	echo "  count after txn commit       : $c3"
	if [ "$writer_rc" -eq 0 ]; then
		echo "  writer error                 : none"
	else
		echo "  writer error                 : rc=$writer_rc ${writer_err}"
	fi
	[ "$c1" = "$c2" ] && echo "  CONSISTENT (read1 == read2)  : True" || echo "  CONSISTENT (read1 == read2)  : False"
	[ "$writer_rc" -eq 0 ] && echo "  WRITER UNBLOCKED             : True" || echo "  WRITER UNBLOCKED             : False"
	if [ "$writer_rc" -eq 0 ]; then
		[ "$c3" = "$((c1 + 1))" ] && echo "  READER ADVANCES after commit : True" || echo "  READER ADVANCES after commit : False"
	else
		[ "$c3" = "$c1" ] && echo "  READER ADVANCES after commit : True" || echo "  READER ADVANCES after commit : False"
	fi
	rm -rf "$tmp"
}

run_mode WAL
run_mode DELETE

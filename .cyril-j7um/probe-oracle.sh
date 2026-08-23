#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
	echo "usage: probe-oracle.sh <probe-root>" >&2
	exit 2
fi
root=$1

root_mode=$(stat -c '%a' "$root")
socket_mode=$(stat -c '%a' "$root/runtime.sock")
memory_journal=$(sqlite3 "$root/memory.sqlite3" 'PRAGMA journal_mode;')
knowledge_journal=$(sqlite3 "$root/knowledge.sqlite3" 'PRAGMA journal_mode;')
memory_version=$(sqlite3 "$root/memory.sqlite3" 'SELECT version FROM schema_version WHERE singleton = 1;')
knowledge_version=$(sqlite3 "$root/knowledge.sqlite3" 'SELECT version FROM schema_version WHERE singleton = 1;')

oracle_lock="$root/oracle.lock"
ready="$root/oracle-lock.ready"
(
	exec 9>"$oracle_lock"
	flock -x 9
	touch "$ready"
	sleep 30
) &
holder=$!
for _ in $(seq 1 500); do
	if [ -e "$ready" ]; then
		break
	fi
	sleep 0.01
done
if [ ! -e "$ready" ]; then
	kill -KILL "$holder" 2>/dev/null || true
	wait "$holder" 2>/dev/null || true
	echo "oracle lock holder did not become ready" >&2
	exit 1
fi
if flock -n "$oracle_lock" -c true; then
	blocked_while_held=false
else
	blocked_while_held=true
fi
kill -KILL "$holder"
wait "$holder" 2>/dev/null || true
if flock -n "$oracle_lock" -c true; then
	reacquired_after_kill=true
else
	reacquired_after_kill=false
fi

printf '%s\n' \
	"lock.blocked_while_held=$blocked_while_held" \
	"lock.reacquired_after_kill=$reacquired_after_kill" \
	"root.mode=$root_mode" \
	"socket.mode=$socket_mode" \
	"sqlite.memory.journal=$memory_journal" \
	"sqlite.knowledge.journal=$knowledge_journal" \
	"sqlite.memory.version=$memory_version" \
	"sqlite.knowledge.version=$knowledge_version"

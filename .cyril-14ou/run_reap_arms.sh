#!/usr/bin/env bash
# cyril-14ou Q2 oracle wrapper: pgrep set-diff around each teardown arm.
#
# Hardened per PR #94 review (S1 + SP5):
#  - probe-owned matching only: candidate nodes are children of THIS probe's
#    PID (a concurrent cyril/KAS session's node has a different parent and is
#    never counted — and never killed);
#  - READY is REQUIRED (a probe that dies before a session is a run failure,
#    not a vacuous pass), and each arm must observe >= 1 probe-owned node;
#  - stderr is kept per arm; probe exit codes and oracle failures exit nonzero.
set -u
PROBE="$(dirname "$0")/probe/target/debug/cyril-14ou-probe"
fail=0

# Nodes owned by a given probe pid: direct children matching the KAS server.
snap_owned() { pgrep -P "$1" -f "acp-server.js" | sort; }

for arm in shutdown drop abort; do
  fifo="/tmp/c14ou-$arm.fifo"; rm -f "$fifo"; mkfifo "$fifo"
  "$PROBE" "$arm" < "$fifo" > "/tmp/c14ou-$arm.out" 2>"/tmp/c14ou-$arm.err" &
  probe_pid=$!
  exec 9>"$fifo"   # hold the write end open
  ready=0
  for _ in $(seq 1 60); do
    grep -q READY "/tmp/c14ou-$arm.out" 2>/dev/null && { ready=1; break; }
    kill -0 "$probe_pid" 2>/dev/null || break
    sleep 1
  done
  if [ "$ready" -ne 1 ]; then
    echo "ARM=$arm FAIL: probe never reached READY (stderr: /tmp/c14ou-$arm.err)"
    tail -3 "/tmp/c14ou-$arm.err" 2>/dev/null
    kill "$probe_pid" 2>/dev/null
    exec 9>&-; rm -f "$fifo"; fail=1; continue
  fi
  during=$(snap_owned "$probe_pid")
  if [ -z "$during" ]; then
    echo "ARM=$arm FAIL: READY but no probe-owned node observed (oracle blind)"
    kill "$probe_pid" 2>/dev/null
    exec 9>&-; rm -f "$fifo"; fail=1; continue
  fi
  echo go >&9      # release the probe into its teardown arm
  wait "$probe_pid"; probe_rc=$?
  exec 9>&-; rm -f "$fifo"
  sleep 3
  survivors=""
  for pid in $during; do
    kill -0 "$pid" 2>/dev/null && survivors="$survivors $pid"
  done
  # abort exits via SIGABRT by design; other arms must exit cleanly.
  if [ "$arm" != "abort" ] && [ "$probe_rc" -ne 0 ]; then
    echo "ARM=$arm FAIL: probe exit=$probe_rc (stderr: /tmp/c14ou-$arm.err)"; fail=1
  fi
  echo "ARM=$arm nodes_during=[$(echo $during | tr '\n' ' ')] survivors_after=[$survivors ]"
  # clean up survivors so arms stay independent — probe-owned pids only.
  for pid in $survivors; do kill -9 "$pid" 2>/dev/null; done
done
exit "$fail"

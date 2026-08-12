#!/usr/bin/env bash
# cyril-14ou Q2 oracle wrapper: pgrep set-diff around each teardown arm.
set -u
PROBE="$(dirname "$0")/probe/target/debug/cyril-14ou-probe"
snap() { pgrep -f "acp-server.js" | sort; }

for arm in shutdown drop abort; do
  before=$(snap)
  fifo="/tmp/c14ou-$arm.fifo"; rm -f "$fifo"; mkfifo "$fifo"
  "$PROBE" "$arm" < "$fifo" > "/tmp/c14ou-$arm.out" 2>/dev/null &
  probe_pid=$!
  exec 9>"$fifo"   # hold the write end open
  # wait for READY (session up => node running)
  for _ in $(seq 1 60); do grep -q READY "/tmp/c14ou-$arm.out" 2>/dev/null && break; sleep 1; done
  during=$(snap)
  new=$(comm -13 <(echo "$before") <(echo "$during"))
  echo go >&9      # release the probe into its teardown arm
  wait "$probe_pid" 2>/dev/null
  exec 9>&-
  rm -f "$fifo"
  sleep 3
  after=$(snap)
  survivors=$(comm -12 <(echo "$new") <(echo "$after"))
  echo "ARM=$arm new_nodes_during=[$(echo $new | tr '\n' ' ')] survivors_after=[$(echo $survivors | tr '\n' ' ')]"
  # clean up any survivor so arms stay independent
  for pid in $survivors; do kill -9 "$pid" 2>/dev/null; done
done

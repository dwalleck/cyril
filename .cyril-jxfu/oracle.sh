#!/bin/sh
# cyril-jxfu oracle: recompute the probe's facts with text tools only —
# no JSON parsing, no routing simulation. grep/sed/awk over raw bytes.
set -eu
CAP=experiments/conductor-spike/kas-custom-dag-2.16.0.jsonl

echo "-- session/update frame count per sessionId (grep -o | sort | uniq -c) --"
grep '"method":"session/update"' "$CAP" \
  | grep -o '"sessionId":"[^"]*"' | sort | uniq -c

echo
echo "-- sessionIds named by node_start (raw text) --"
grep -n '"method":"_kiro/workflow/node_start"' "$CAP" \
  | sed 's/\(^[0-9]*\).*/line \1/' >/dev/null   # line refs below
grep -n '"method":"_kiro/workflow/node_start"' "$CAP" \
  | awk -F'"sessionId":"' '{ split($2,a,"\""); split($0,l,":");
      printf "line %s: sessionId=%s\n", l[1], (a[1]=="" ? "ABSENT" : a[1]) }'

echo
echo "-- first-appearance line of each sessionId on session/update --"
grep -n '"method":"session/update"' "$CAP" \
  | awk -F'"sessionId":"' '{ split($2,a,"\""); split($0,l,":");
      if (!(a[1] in seen)) { seen[a[1]]=1; printf "line %s: %s\n", l[1], a[1] } }'

echo
echo "-- the main session id (sessionId inside a result object) --"
grep -n '"result":{[^}]*"sessionId"' "$CAP" \
  | awk -F'"sessionId":"' '{ split($2,a,"\""); split($0,l,":");
      printf "line %s: %s\n", l[1], a[1] }'

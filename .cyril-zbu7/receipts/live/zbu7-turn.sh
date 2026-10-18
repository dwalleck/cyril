#!/bin/bash
# usage: zbu7-turn.sh <term-id> <marker-word>
# Sends one prompt asking the agent to echo <marker-word> uppercased and waits
# for it on screen (approving any permission dialog). Prints PASS/FAIL + UTC time.
H=/home/dwalleck/.dotnet/tools/hex1b; T=$1; M=$2
scr(){ timeout 20 $H capture screenshot $T --json | python3 -I -c "import json,sys;d=json.load(sys.stdin)['data'];print('\n'.join(l.rstrip() for l in d.splitlines() if l.strip()))"; }
want=$(echo "$M" | tr '[:lower:]' '[:upper:]')
timeout 10 $H keys $T --text "Reply with only the word $M in uppercase, nothing else." >/dev/null
timeout 5 $H keys $T --key Enter >/dev/null
end=$((SECONDS+240))
while (( SECONDS < end )); do
  s=$(scr)
  if grep -qF -- "$want" <<<"$s"; then echo "PASS turn $M at $(date -u +%FT%TZ)"; exit 0; fi
  if grep -q 'Permission Required' <<<"$s"; then timeout 5 $H keys $T --key Enter >/dev/null; fi
  if grep -qiE 'disconnected|BridgeDisconnected' <<<"$s"; then echo "FAIL turn $M: disconnected at $(date -u +%FT%TZ)"; scr | tail -12; exit 1; fi
  timeout 8 $H assert $T --text-present "$want" --timeout 5 >/dev/null 2>&1 && { echo "PASS turn $M at $(date -u +%FT%TZ)"; exit 0; }
done
echo "FAIL turn $M: no '$want' by $(date -u +%FT%TZ)"; scr | tail -12; exit 1

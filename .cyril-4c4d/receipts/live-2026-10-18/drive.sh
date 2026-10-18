#!/bin/bash
# usage: drive.sh <term-id> ; prints PASS/FAIL per step
H=/home/dwalleck/.dotnet/tools/hex1b; T=$1
scr(){ timeout 20 $H capture screenshot $T --json | python3 -I -c "import json,sys;d=json.load(sys.stdin)['data'];print('\n'.join(l.rstrip() for l in d.splitlines() if l.strip()))"; }
wait_for(){ # marker, seconds — approves permission dialogs while waiting
  local m=$1 end=$((SECONDS+$2))
  while (( SECONDS < end )); do
    s=$(scr)
    grep -qF -- "$m" <<<"$s" && return 0
    if grep -q 'Permission Required' <<<"$s"; then echo "  (approving: $(grep -A1 'Permission Required' <<<"$s" | tail -1 | sed 's/^ *│//;s/ *│.*$//'))"; timeout 5 $H keys $T --key Enter >/dev/null; fi
    timeout 8 $H assert $T --text-present "$m" --timeout 5 >/dev/null 2>&1 && return 0
  done; return 1
}
step(){ local name=$1 input=$2 marker=$3
  timeout 10 $H keys $T --text "$input" >/dev/null; timeout 5 $H keys $T --key Enter >/dev/null
  if wait_for "$marker" 240; then echo "PASS $name → $marker"; else echo "FAIL $name (no '$marker')"; fi
  timeout 6 $H assert $T --text-absent "Running..." --timeout 5 >/dev/null 2>&1
}
wait_for "Session created" 90 && echo "PASS session created (engine chosen without --agent-engine)" || { echo "FAIL no session"; scr | tail -8; exit 1; }
timeout 10 $H keys $T --text "/pro" >/dev/null
if wait_for "/probe-skill" 60 && scr | grep -q "/probe-prompt"; then echo "PASS autocomplete offers /probe-skill and /probe-prompt"; else echo "FAIL autocomplete"; fi
for i in 1 2 3 4; do timeout 5 $H keys $T --key Backspace >/dev/null; done
step skill "/probe-skill omega4" "SKILLRAN OMEGA4"
step prompt "/probe-prompt gamma delta" "PROMPTRAN GAMMA DELTA"
step custom-agent "/general-task-execution reply with the word agentran_q2 in uppercase and nothing else" "AGENTRAN_Q2"

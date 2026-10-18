#!/bin/bash
# usage: k8wc-drive.sh <term-id> <arm>  — prints PASS/FAIL/ASKED lines; screens saved in the arm dir
H=/home/dwalleck/.dotnet/tools/hex1b; T=$1
L=/home/dwalleck/.claude/tmp/claude-1000/-home-dwalleck-repos-cyril/0473cb2a-6b72-4739-a03f-aec2a7f97d90/scratchpad/cyril-k8wc/live
A="$L/$2"
scr(){ timeout 20 $H capture screenshot $T --json | python3 -I -c "import json,sys;d=json.load(sys.stdin)['data'];print('\n'.join(l.rstrip() for l in d.splitlines() if l.strip()))"; }
asks=0
wait_for(){ # marker, seconds, tag — records + approves permission dialogs while waiting
  local m=$1 end=$((SECONDS+$2)) tag=$3
  asks=0
  while (( SECONDS < end )); do
    s=$(scr)
    grep -qF -- "$m" <<<"$s" && return 0
    if grep -q 'Permission Required\|Choose Scope' <<<"$s"; then
      asks=$((asks+1))
      printf '%s\n' "$s" > "$A/screen-$tag-ask$asks.txt"
      echo "  ASKED[$tag #$asks]: $(grep -A1 'Permission Required' <<<"$s" | tail -1 | sed 's/^ *│//;s/ *│.*$//')"
      grep -E 'Allow|Always|Deny|Reject|Trust|Yes|No' <<<"$s" | sed 's/^/      overlay: /'
      timeout 5 $H keys $T --key Enter >/dev/null
      sleep 1
    fi
    timeout 8 $H assert $T --text-present "$m" --timeout 5 >/dev/null 2>&1 && return 0
  done; return 1
}
step(){ local tag=$1 input=$2 marker=$3
  timeout 10 $H keys $T --text "$input" >/dev/null; timeout 5 $H keys $T --key Enter >/dev/null
  if wait_for "$marker" 240 "$tag"; then echo "PASS $tag → $marker (permission prompts: $asks)"; else echo "FAIL $tag (no '$marker'; prompts: $asks)"; fi
  scr > "$A/screen-$tag-end.txt"
  timeout 6 $H assert $T --text-absent "Running..." --timeout 5 >/dev/null 2>&1
}
wait_for "Session created" 120 init && echo "PASS session created" || { echo "FAIL no session"; scr | tail -12; exit 1; }
step norule "Use your shell tool to run exactly this command and nothing else: echo AUDITY . After it finishes, reply with the word turnone_done in uppercase and nothing else." "TURNONE_DONE"
# switch to the custom agent that carries the V3 allow rule
timeout 10 $H keys $T --text "/mode" >/dev/null; timeout 5 $H keys $T --key Enter >/dev/null
sleep 2; scr > "$A/screen-mode-picker.txt"
timeout 10 $H keys $T --text "auditx" >/dev/null; sleep 1; timeout 5 $H keys $T --key Enter >/dev/null
sleep 4; scr > "$A/screen-mode-set.txt"
grep -qi auditx "$A/screen-mode-set.txt" && echo "PASS mode auditx visible" || echo "WARN auditx not visible after /mode"
step rule "Use your shell tool to run exactly this command and nothing else: echo AUDITX . After it finishes, reply with the word turntwo_done in uppercase and nothing else." "TURNTWO_DONE"
echo "DRIVE-DONE"

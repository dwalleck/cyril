#!/bin/bash
H=/home/dwalleck/.dotnet/tools/hex1b; T=$1
scr(){ timeout 20 $H capture screenshot $T --json | python3 -I -c "import json,sys;d=json.load(sys.stdin)['data'];print('\n'.join(l.rstrip() for l in d.splitlines() if l.strip()))"; }
wait_for(){ local m=$1 end=$((SECONDS+$2)) approve=${3:-yes}
  while (( SECONDS < end )); do
    s=$(scr); grep -qF -- "$m" <<<"$s" && return 0
    if [ "$approve" = yes ] && grep -q 'Permission Required' <<<"$s"; then
      grep -A9 'Permission Required' <<<"$s" | sed 's/^ *│//;s/│ *$//' | sed 's/ *$//' | grep -v '^\s*$' | sed 's/^/  [overlay] /'
      timeout 5 $H keys $T --key Enter >/dev/null; fi
    timeout 8 $H assert $T --text-present "$m" --timeout 5 >/dev/null 2>&1 && return 0
  done; return 1; }
step(){ timeout 10 $H keys $T --text "$2" >/dev/null; timeout 5 $H keys $T --key Enter >/dev/null
  if wait_for "$3" 240; then echo "PASS $1 → $3"; else echo "FAIL $1 (no '$3')"; scr | tail -12; fi
  timeout 8 $H assert $T --text-absent "Running..." --timeout 6 >/dev/null 2>&1; }
wait_for "Session created" 120 && echo "PASS session created on Windows" || { echo "FAIL no session"; scr | tail -10; exit 1; }
step system-prompt-shell "Reply with only the exact value of the Shell line in your system information, wrapped in square brackets, and nothing else" "[pwsh]"
step pwsh7-command "Run this exact command with your shell tool, then reply with its output: Get-Date -Format yyyy && Write-Output (\"PWSH\" + (6+1) + \"OK\")" "PWSH7OK"

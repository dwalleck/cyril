#!/bin/bash
# host lane 2.31.1 -> 2.32.0: run every reusable static script; outputs under host/tmp (bulk) + logs here
set -u
S=/home/dwalleck/repos/cyril/experiments/conductor-spike
R=$S/audit-2.32.0/host/raw
T=$S/audit-2.32.0/host/tmp
B=$HOME/.local/share/kiro-research/binaries
D=(); C=(); VC=()
for v in 2.31.1 2.32.0; do D+=($B/$v/extracted/kirocli/bin); C+=($B/$v/extracted/kirocli/bin/kiro-cli-chat); VC+=($v:$B/$v/extracted/kirocli/bin); done
cd $R
python3 -I $S/static-rollout-2.32.0.py $R/rollout ${C[@]} > rollout.log 2>&1; echo rollout $?
python3 -I $S/static-host-settings-2.32.0.py ${C[@]} > settings.log 2>&1; echo settings $?
python3 -I $S/static-doc-manifests-2.32.0.py $R/docman ${C[@]} > docman.log 2>&1; echo docman $?
python3 -I $S/static-host-env-2.32.0.py $R/env ${D[@]} > env.log 2>&1; echo env $?
python3 -I $S/static-host-paths-2.32.0.py $T/paths ${D[@]} > paths.log 2>&1; echo paths $?
python3 -I $S/static-host-srcloc-2.32.0.py $T/srcloc ${VC[@]} > srcloc.log 2>&1; echo srcloc $?
python3 -I $S/static-host-serde-2.32.0.py $T/serde ${C[@]} > serde.log 2>&1; echo serde $?
python3 -I $S/static-host-novel-words-2.32.0.py $T/novel kiro-cli-chat ${VC[@]} > novel-chat.log 2>&1; echo novel-chat $?
python3 -I $S/static-host-novel-words-2.32.0.py $T/novel-launcher kiro-cli ${VC[@]} > novel-launcher.log 2>&1; echo novel-launcher $?
for b in kiro-cli-chat kiro-cli kiro-cli-term; do
  python3 -I $S/static-host-prose-2.32.0.py $T/strings $b 2.31.1 2.32.0 > prose-$b.log 2>&1; echo prose-$b $?
  python3 -I $S/static-host-idents-2.32.0.py $T/strings $b 2.31.1 2.32.0 > idents-$b.log 2>&1; echo idents-$b $?
done
echo ALL-DONE

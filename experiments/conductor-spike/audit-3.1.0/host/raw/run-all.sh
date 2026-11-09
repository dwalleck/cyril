#!/bin/bash
# host lane 2.32.0 -> 3.1.0: run every reusable static script over the 14-version chain; outputs under host/tmp (bulk) + logs here
set -u
S=/home/dwalleck/repos/cyril/experiments/conductor-spike
R=$S/audit-3.1.0/host/raw
T=$S/audit-3.1.0/host/tmp
B=$HOME/.local/share/kiro-research/binaries
VERS=(2.32.0 2.33.0 2.33.1 2.34.0 2.34.1 2.34.2 2.35.0 2.35.1 2.35.2 2.36.0 2.37.0 2.37.1 3.0.0 3.1.0)
D=(); H=(); VC=()
for v in "${VERS[@]}"; do d=$B/$v/extracted/kirocli/bin; D+=($d); VC+=($v:$d); if [ -f $d/kiro-cli-chat ]; then H+=($d/kiro-cli-chat); else H+=($d/kiro-cli); fi; done
cd $R
python3 -I $S/static-rollout-3.1.0.py $R/rollout "${H[@]}" > rollout.log 2>&1; echo rollout $?
python3 -I $S/static-host-settings-3.1.0.py "${H[@]}" > settings.log 2>&1; echo settings $?
python3 -I $S/static-doc-manifests-3.1.0.py $R/docman "${H[@]}" > docman.log 2>&1; echo docman $?
python3 -I $S/static-host-serde-3.1.0.py $T/serde "${H[@]}" > serde.log 2>&1; echo serde $?
python3 -I $S/static-host-methods-3.1.0.py $T/strings 2.32.0:kiro-cli-chat 3.1.0:kiro-cli > methods.log 2>&1; echo methods $?
python3 -I $S/static-host-methods-3.1.0.py $T/strings 2.37.1:kiro-cli-chat 3.0.0:kiro-cli > methods-merge.log 2>&1; echo methods-merge $?
python3 -I $S/static-host-deps-3.1.0.py $T/strings 2.32.0:kiro-cli-chat 3.1.0:kiro-cli > deps.log 2>&1; echo deps $?
python3 -I $S/static-host-deps-3.1.0.py $T/strings 2.37.1:kiro-cli-chat 3.0.0:kiro-cli > deps-merge.log 2>&1; echo deps-merge $?
python3 -I $S/static-host-deps-3.1.0.py $T/strings 2.37.1:kiro-cli 3.0.0:kiro-cli > deps-launcher-merge.log 2>&1; echo deps-launcher-merge $?
python3 -I $S/static-host-env-3.1.0.py $R/env "${D[@]}" > env.log 2>&1; echo env $?
python3 -I $S/static-host-paths-3.1.0.py $T/paths "${D[@]}" > paths.log 2>&1; echo paths $?
python3 -I $S/static-host-srcloc-3.1.0.py $T/srcloc "${VC[@]}" > srcloc.log 2>&1; echo srcloc $?
echo ALL-DONE

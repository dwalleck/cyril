#!/usr/bin/env bash
# After the paired workflow legs (bg chain): W-series then T-series, strictly serial.
set -u
cd "$(dirname "$0")"
./run-wseries.sh
./run-tseries.sh
echo "REST DONE"

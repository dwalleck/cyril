#!/usr/bin/env bash
set -u
cd "$(dirname "$0")"
echo "[chain] start $(date +%T)"
./run-wseries.sh
./run-tseries.sh
./run-final.sh
echo "[chain] ALL DONE $(date +%T)"

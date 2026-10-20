#!/bin/sh
# A Node-entry fixture: the selected server must be readable from the actual
# child workspace, not merely from the process that performed discovery.
set -eu
if [ "$#" -ne 4 ] || [ "$1" != '--experimental-wasm-modules' ]; then
    printf '%s\n' 'unexpected KAS node argv' >&2
    exit 40
fi
if [ ! -f "$2" ]; then
    printf 'selected bundle is not readable from child cwd: %s\n' "$2" >&2
    exit 41
fi
IFS= read -r marker < "$2"
if [ "$marker" != 'CYRIL_0SSB_SELECTED_BUNDLE' ]; then
    printf '%s\n' 'wrong selected bundle' >&2
    exit 42
fi
printf 'cwd=%s\nbundle=%s\n' "$PWD" "$marker"

#!/usr/bin/env bash
# cyril-brui premise probe: does the kiro-cli launcher resolve its data dir
# from XDG_DATA_HOME (dirs-sys semantics) rather than $HOME/.local/share?
#
# NO-AUTH by construction: HOME, XDG_DATA_HOME, XDG_CONFIG_HOME and
# XDG_CACHE_HOME all point at empty scratch dirs, so there is no credential
# store, no token, and nothing to refresh (the OIDC refresh token is
# single-use; concurrent renewals log the user out). Only local subcommands
# are run (`--version`, `settings list`) — never whoami/login/acp.
#
# Usage: probe-xdg-strace.sh <scratch-dir> <kiro-cli path> <kiro args...>
#   PROBE_XDG_VALUE=<v>  export that literal as XDG_DATA_HOME instead of the
#                        absolute scratch dir (e.g. a RELATIVE value, to pin
#                        the fallback semantics).
# Output: <scratch>/strace-<tag>.log plus a summary on stdout.
set -u
S="${1:?scratch dir}"; shift
KIRO="${1:?kiro-cli path}"; shift
if [ -z "${PROBE_XDG_VALUE+set}" ]; then LEG=abs; elif [ -z "$PROBE_XDG_VALUE" ]; then LEG=empty; else LEG="$PROBE_XDG_VALUE"; fi
TAG="$(echo "$LEG-$*" | tr -c 'A-Za-z0-9' '-')"
mkdir -p "$S/xdg" "$S/home" "$S/cfg" "$S/cache"
export HOME="$S/home"
# `${VAR-default}` (no colon): a set-but-EMPTY value is exported as-is.
export XDG_DATA_HOME="${PROBE_XDG_VALUE-$S/xdg}"
export XDG_CONFIG_HOME="$S/cfg"
export XDG_CACHE_HOME="$S/cache"
LOG="$S/strace-$TAG.log"
echo "=== $KIRO $* (HOME=$HOME XDG_DATA_HOME=$XDG_DATA_HOME)"
strace -f -o "$LOG" -e trace=openat,statx,stat,access,newfstatat,readlink,mkdir \
  "$KIRO" "$@"
echo "exit=$?"
echo "--- syscalls naming \$XDG_DATA_HOME:"
grep -c "$XDG_DATA_HOME" "$LOG"
grep "$XDG_DATA_HOME" "$LOG" | sed -e 's/^[0-9]* *//' -e "s#$S#<S>#g" | sort | uniq -c | sort -rn | head -20
echo "--- syscalls naming \$HOME/.local/share (expected 0 when XDG wins):"
grep -c "$HOME/.local/share" "$LOG"
grep "$HOME/.local/share" "$LOG" | sed -e "s#$S#<S>#g" | head -5
echo "--- what now exists under \$XDG_DATA_HOME:"
find "$XDG_DATA_HOME" -maxdepth 3 | sed -e "s#$S#<S>#g"

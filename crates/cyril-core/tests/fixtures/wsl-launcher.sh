#!/bin/sh
printf '%s\n' "$@" > "${0%/*}/probe-argv.txt"
if [ "$1" = "--version" ]; then printf 'WSL version: 2.6.1.0\n'; exit 0; fi
while [ "$#" -gt 0 ]; do
    case "$1" in
        -d|-u|--cd|--shell-type|--distribution|--distribution-id|--user) shift 2 ;;
        --) shift; break ;;
        -*|'~') shift ;;
        *) break ;;
    esac
done
if [ "$1" = "kiro-cli" ] && [ "$2" = "--version" ]; then printf 'kiro-cli 2.21.1\n'; exit 0; fi
printf 'unexpected command line: %s\n' "$*" >&2
exit 3

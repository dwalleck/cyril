#!/bin/sh
# Fake `kiro-cli` for the cyril-zbu7 KAS auth tests: journals every argv next
# to the invoked path (a symlink in the test's private dir), answers its own
# `--version`, and answers `chat _ get-kas-token [--force-refresh]` with the
# success envelope kiro-cli 2.32.0 prints. Never touches real credentials.
printf '%s\n' "$@" >> "${0%/*}/kiro-cli-argv.txt"
if [ "$1" = "--version" ]; then printf 'kiro-cli 2.24.0\n'; exit 0; fi
if [ "$1" = "chat" ] && [ "$2" = "_" ] && [ "$3" = "get-kas-token" ]; then
    printf '%s\n' '{"kind":"getKasToken","data":{"accessToken":"AT-fake","expiresAt":"2099-01-01T00:00:00Z","profileArn":"arn:aws:codewhisperer:us-east-1:1:profile/FAKE","provider":"Enterprise"}}'
    exit 0
fi
printf 'unexpected command line: %s\n' "$*" >&2
exit 3

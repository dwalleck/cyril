#!/bin/sh
[ "$#" -eq 1 ] || exit 64
[ "$1" = crtool ] || exit 65
printf '%s' "$1"

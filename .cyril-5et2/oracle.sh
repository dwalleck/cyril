#!/usr/bin/env bash
# KAS-2b prove-it-prototype ORACLE.
# Independent of cyril's Rust/serde path: navigates the SAME raw captured frame
# with `jq` and extracts, per bucket, {tokens, percent, items-present?}. If the
# Rust probe (serde) and this (jq) agree on the same bytes, cyril's converter
# extracts the breakdown correctly — including the items-absent-vs-empty split.
set -euo pipefail
F="${1:-.cyril-5et2/context_usage_raw.json}"

echo "usagePercentage: $(jq -r '.update._meta.kiro.usagePercentage' "$F")"
echo "bucket            tokens   percent  items"
for b in contextFiles tools yourPrompts kiroResponses sessionFiles; do
  jq -r --arg b "$b" '
    .update._meta.kiro.breakdown[$b] as $x
    | if $x == null then "\($b)|ABSENT-BUCKET|.|."
      else "\($b)|\($x.tokens)|\($x.percent)|" +
           (if ($x|has("items")) then "items[\($x.items|length)]" else "items-ABSENT" end)
      end' "$F" \
  | awk -F'|' '{printf "%-16s  %-7s  %-7s  %s\n",$1,$2,$3,$4}'
done

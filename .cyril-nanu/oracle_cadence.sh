#!/usr/bin/env bash
# P1 oracle — same answer, different failure mechanism.
#
# The probe parses every record as JSON and segments turns by correlating each
# `session/prompt` request id with the response carrying that id. This oracle
# never parses JSON: awk matches raw text patterns and segments turns with a
# DIFFERENT rule — prompt-to-next-prompt instead of prompt-to-response. Prompts
# are serialized on one session, so both rules must yield the same per-turn
# sample counts. A wrong JSON path, a wrong nesting assumption, or a
# mis-correlated id in the probe surfaces here as a mismatch.
#
# Run: .cyril-nanu/oracle_cadence.sh
set -euo pipefail
cd "$(dirname "$0")/.."

report() {
	local label=$1 file=$2
	echo "--- $label ($(basename "$file"))"
	awk -v label="$label" '
		/"method":"session\/prompt"/ {
			if (started) { counts[turns] = run; if (run > max) max = run }
			started = 1; turns++; run = 0; next
		}
		{
			if (label == "v2") { is_sample = /"contextUsagePercentage"/ }
			else               { is_sample = (/"kind":"context_usage"/ && /"usagePercentage"/) }
			if (is_sample) { if (started) run++; else stray++ }
		}
		END {
			if (started) { counts[turns] = run; if (run > max) max = run }
			printf "  turns (prompt-delimited)  : %d\n", turns
			printf "  context samples in turns  : "
			total = 0
			for (i = 1; i <= turns; i++) { total += counts[i]; printf "%d ", counts[i] }
			printf "\n  total in turns            : %d\n", total
			printf "  samples before first turn : %d\n", stray
			printf "  max samples in one turn   : %d\n", max
			printf "  max triggers in one turn  : %d\n", max + 1
		}' "$file"
}

report v2  experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl
report kas experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl

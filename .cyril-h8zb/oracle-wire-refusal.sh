#!/bin/sh
# Oracle: what refusal wire keys does the v2 Rust engine (the metadata
# PRODUCER) serialize? Independent of probe-wire-refusal.py, which reads the
# tui.js CONSUMER. Contamination guard: kiro-cli-chat EMBEDS tui.js, so a
# naive strings scan re-reads the probe's own artifact. Rust serde literals
# and mangled symbols live in SHORT strings; minified JS lives in enormous
# lines — filter to length<120 and to exact-identifier hits.
set -eu
ARCHIVE="$HOME/.local/share/kiro-research/binaries"

for ver in 2.15.0 2.16.0; do
  bin="$ARCHIVE/$ver/kiro-cli-chat"
  echo "=== $ver ==="
  echo "--- Rust type names (RefusalInfo/RefusalCategory/RefusalDetails), short strings ---"
  strings "$bin" | awk 'length($0)<120' | grep -E "RefusalInfo|RefusalCategory|RefusalDetails" | sort -u | head -12
  echo "--- exact wire-key literals as standalone short strings ---"
  strings "$bin" | awk 'length($0)<60' | grep -E "recommendedModel|CONTENT_FILTERED" | sort -u | head -12
  echo "--- symbol table (nm) refusal hits ---"
  nm "$bin" 2>/dev/null | grep -i refusal | head -6 || echo "(no symbols or no hits)"
  echo
done

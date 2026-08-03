#!/bin/sh
# cyril-dn91 oracle: source-text census of the handled host-callback surface
# and of engine consults on its dispatch paths. Independent mechanism from the
# runtime probe (probe_dn91.rs): this reads the source; the probe runs it.
set -eu
CL=crates/cyril-core/src/protocol/client.rs
FS=crates/cyril-core/src/protocol/kas/kiro_fs.rs

# Census the PRODUCTION region only — first oracle draft leaked into `mod tests`
# (matched test fns named *_override_* and counted a doc comment as a consult).
PROD=/tmp/dn91-client-prod.rs
sed -n '1,/^#\[cfg(all(test, feature = "kas"))\]/p' "$CL" > "$PROD"
CL="$PROD"

echo "== typed acp::Client overrides (cfg kas) =="
grep -n "async fn" "$CL" | sed -n '/read_text_file\|write_text_file\|create_terminal\|wait_for_terminal_exit\|terminal_output\|release_terminal\|kill_terminal/p'

echo "== ext-request arms in handle_ext_request =="
sed -n '/#\[cfg(feature = "kas")\]/,/^    \/\/\/ Default build: no KAS ext requests/p' "$CL" \
  | grep -o 'GET_ACCESS_TOKEN_METHOD\|SHELL_TYPE_METHOD\|LIST_METHOD\|EXECUTE_METHOD\|SESSION_START_METHOD\|op_for_method' | sort -u

echo "== _kiro/fs ops table (each is an advertised+dispatched method) =="
grep -n 'wire:\|method:' "$FS" | grep -c 'method:' || true
grep -o '"kiro/fs/[a-z_]*"' "$FS" | sort -u

echo "== control-notification arms in ext_notification =="
grep -o 'CANCEL_METHOD\|DID_CHANGE_METHOD' "$CL" | sort | uniq -c

echo "== engine consults inside dispatch paths =="
# Extract handle_ext_request (kas) + the 7 typed override bodies, then count
# references to the bound engine. Expected: 0 — the defect this issue fixes.
awk '/async fn handle_ext_request/,/^    }$/' "$CL" > /tmp/dn91-dispatch.txt
awk '/async fn read_text_file|async fn write_text_file|async fn create_terminal|async fn wait_for_terminal_exit|async fn terminal_output|async fn release_terminal|async fn kill_terminal/,/^    }$/' "$CL" >> /tmp/dn91-dispatch.txt
printf 'engine consults in dispatch bodies (comments excluded): '
grep -v '^\s*//' /tmp/dn91-dispatch.txt | grep -c 'engine' || true

echo "== engine consults elsewhere in client.rs (for contrast) =="
grep -n 'self\.engine\|engine\.hooks_mode\|engine:' "$CL" | grep -v '^\s*//' | head -12

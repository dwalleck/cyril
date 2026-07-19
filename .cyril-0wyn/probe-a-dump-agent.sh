#!/bin/sh
# cyril-0wyn Probe A helper: a fake ACP agent that records every frame cyril
# sends it, replies to nothing. The bridge handshake will fail by timeout —
# that's fine; the initialize frame is captured before any reply is expected.
tee "${PROBE_A_OUT:-/tmp/cyril-0wyn-probe-a.jsonl}" >/dev/null

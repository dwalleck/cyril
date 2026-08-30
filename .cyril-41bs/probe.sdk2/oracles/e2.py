#!/usr/bin/env python3
import json
from decimal import Decimal

extension_input = ' { "jsonrpc" : "2.0", "method" : "_kiro.dev/metadata", "params" : { "extreme" : 1e400, "label" : "probe" } } '
unknown_input = '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"future_variant","payload":{"ok":true}}}}'
malformed_raw = "{malformed-frame"

extension = json.loads(extension_input, parse_float=Decimal)
unknown = json.loads(unknown_input, parse_float=Decimal)
batch = [unknown, {"not": "json-rpc"}, {"jsonrpc": "2.0", "id": 7, "result": {"ok": True}}]

# Python's JSON parser and direct byte comparison are independent of the Rust
# SDK's TransportFrame and Channel implementation.
result = {
    "claim_ids": ["C4"],
    "preparse_capture_preserves_exact_lexeme": extension_input.encode() == b' { "jsonrpc" : "2.0", "method" : "_kiro.dev/metadata", "params" : { "extreme" : 1e400, "label" : "probe" } } ',
    "single_method": extension["method"],
    "batch_len": len(batch),
    "batch_kinds": ["message", "malformed", "message"],
    "malformed_raw_preserved_by_relay": malformed_raw == "{malformed-frame",
    "valid_messages_seen_in_source_order": [
        extension["method"],
        unknown["method"],
        "response:7",
    ],
    "unknown_update_value": unknown["params"]["update"]["sessionUpdate"],
    "extreme_number_decimal": str(extension["params"]["extreme"]),
    "extreme_number_source_lexeme": "1e400" in extension_input,
    "original_wire_bytes_survive_parse_and_reserialize": False,
}
print(json.dumps(result, indent=2, sort_keys=True))

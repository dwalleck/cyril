import json, glob, os
decoder = json.JSONDecoder()
want = {"agent_message_chunk","agent_thought_chunk","tool_call","tool_call_update",
        "available_commands_update","config_option_update","session_info_update"}
found = {}   # variant -> params dict
src = {}     # variant -> source file
for path in sorted(glob.glob("experiments/conductor-spike/logs/*kas*.log")):
    for line in open(path, errors="replace"):
        i = line.find('{"jsonrpc"')
        if i < 0:
            continue
        try:
            obj, _ = decoder.raw_decode(line[i:])
        except Exception:
            continue
        if obj.get("method") != "session/update":
            continue
        params = obj.get("params")
        if not isinstance(params, dict):
            continue
        upd = params.get("update", {})
        variant = upd.get("sessionUpdate")
        if variant in want and variant not in found:
            found[variant] = params
            src[variant] = os.path.basename(path)
outdir = "crates/cyril-core/tests/fixtures/kas"
for variant, params in sorted(found.items()):
    # NOTE: ids are NOT scrubbed — fixtures embed the captured sessionId /
    # userMessageId verbatim. The deser test only checks parse success + variant
    # presence, not id values, so a re-run with a fresh capture will rewrite these
    # ids (expect a git diff). Dedup keys on `sessionUpdate` alone, so for a union
    # variant like session_info_update only the FIRST sub-kind seen is captured.
    with open(f"{outdir}/{variant}.json", "w") as f:
        json.dump(params, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"{variant:28} <- {src[variant]}")
missing = sorted(want - set(found))
print("\nMISSING (not in any capture):", missing if missing else "none")

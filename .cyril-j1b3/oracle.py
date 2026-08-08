import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / "experiments/conductor-spike/kas-modes-dumps/bug-fix.json"
frames = json.loads(FIXTURE.read_text())["frames"]
cache = {}
rows = []

for frame in frames:
    params = frame.get("params", {})
    session = params.get("sessionId")
    update = params.get("update", {})
    kind = update.get("sessionUpdate")
    if kind in {"tool_call", "tool_call_update"} and "rawInput" in update:
        cache[(session, update["toolCallId"])] = update["rawInput"]
    if frame.get("method") != "session/request_permission":
        continue
    request = params.get("toolCall", {})
    if request.get("title") != "Write File":
        continue
    key = (session, request.get("toolCallId"))
    raw_input = cache.get(key)
    assert raw_input is not None, f"missing independent join for {key}"
    path = raw_input.get("path")
    text = raw_input.get("text")
    assert isinstance(path, str) and path
    assert isinstance(text, str) and text
    rows.append((request["toolCallId"], path, len(text.encode())) )

assert len(rows) == 4, f"expected four Write File rows, got {len(rows)}"
for tool_id, path, byte_count in rows:
    print(f"id={tool_id} path={path} text_bytes={byte_count}")

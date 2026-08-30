#!/usr/bin/env python3
import json
import queue
import threading
import time
from pathlib import Path

messages = ["stream:1", "stream:2", "permission", "stream:3"]
transport = queue.SimpleQueue()


def produce():
    for message in messages:
        transport.put(message)
        time.sleep(0.005)
    transport.put(None)


producer = threading.Thread(target=produce)
producer.start()
slow_observed = []
fast_observed = []
delivered = []

def slow_observer(message):
    time.sleep(0.05)
    slow_observed.append((message, round((time.monotonic() - started) * 1000)))


def fast_observer(message):
    fast_observed.append((message, round((time.monotonic() - started) * 1000)))

started = time.monotonic()
while True:
    message = transport.get()
    if message is None:
        break
    slow_observer(message)
    fast_observer(message)
    delivered.append(message)
producer.join()
elapsed_ms = round((time.monotonic() - started) * 1000)


def disconnected_observer(_message):
    raise BrokenPipeError("observer disconnected")


disconnect_aborted_inline_delivery = False
try:
    disconnected_observer("stream:disconnect")
    delivered.append("stream:disconnect")
except BrokenPipeError:
    disconnect_aborted_inline_delivery = True

fresh_attachment = queue.SimpleQueue()
try:
    fresh_attachment.get_nowait()
    replay_absent = False
except queue.Empty:
    replay_absent = True

roots = list((Path.home() / ".cargo/registry/src").glob("*/agent-client-protocol-2.0.0"))
conductor_roots = list(
    (Path.home() / ".cargo/registry/src").glob(
        "*/agent-client-protocol-conductor-2.0.0"
    )
)
if len(roots) != 1 or len(conductor_roots) != 1:
    raise SystemExit("pinned SDK sources not uniquely available")
channel_source = (roots[0] / "src/jsonrpc.rs").read_text()
conductor_source = (conductor_roots[0] / "src/conductor.rs").read_text()

facts = {
    "claim_ids": ["C10"],
    "stream_and_permission_order_preserved": delivered == messages,
    "permission_stayed_on_linear_path": delivered.index("permission") == 2,
    "slow_observer_delayed_forwarding": elapsed_ms >= 180,
    "second_inline_observer_delayed_too": (
        fast_observed[0][1] >= 50
        and fast_observed[0][1] >= slow_observed[0][1]
    ),
    "observer_disconnect_aborted_inline_delivery": disconnect_aborted_inline_delivery,
    "fresh_attachment_has_no_replay": replay_absent,
    "elapsed_ms": elapsed_ms,
    "sdk_channel_uses_unbounded_transport": "mpsc::unbounded" in channel_source,
    "conductor_successor_protocol_present": "SuccessorMessage" in conductor_source,
}
facts["separate_bounded_broadcaster_required"] = all(
    [
        facts["slow_observer_delayed_forwarding"],
        facts["second_inline_observer_delayed_too"],
        facts["observer_disconnect_aborted_inline_delivery"],
        facts["fresh_attachment_has_no_replay"],
        facts["conductor_successor_protocol_present"],
    ]
)
facts["independent_oracle_passed"] = all(
    value
    for key, value in facts.items()
    if key not in {"claim_ids", "elapsed_ms"}
)
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["independent_oracle_passed"]:
    raise SystemExit(1)

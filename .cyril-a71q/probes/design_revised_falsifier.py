"""Independent design-time oracle for requester-choice-A KAS ownership."""
from dataclasses import dataclass, field


@dataclass
class Model:
    active: str | None = None
    task: str | None = None
    bridge_open: bool = True
    completions: list[str] = field(default_factory=list)
    evidence: list[tuple[str, str]] = field(default_factory=list)
    rejected: list[str] = field(default_factory=list)
    discarded: list[tuple[str, str]] = field(default_factory=list)
    lifecycle: list[str] = field(default_factory=list)

    def send(self, owner: str) -> None:
        if not self.bridge_open or self.active is not None:
            self.rejected.append(owner)
            return
        self.active = owner
        self.task = owner

    def response(self, owner: str, reason: str) -> None:
        # A result exists only while its RPC task is still owned.
        if self.task != owner:
            self.discarded.append((owner, reason))
            return
        self.evidence.append((owner, reason))
        self.task = None

    def turn_end(self, owner: str, reason: str) -> None:
        assert self.active == owner
        self.completions.append(owner)
        self.active = None
        # Requester choice A: authoritative turn_end aborts unresolved RPC work.
        if self.task == owner:
            self.task = None
            self.lifecycle.append(f"abort:{owner}")
        self.evidence.append((owner, f"turn_end:{reason}"))

    def prompt_failure(self, owner: str) -> None:
        assert self.active == owner
        self.lifecycle += ["BridgeError", "TurnCompleted", "BridgeDisconnected"]
        self.completions.append(owner)
        self.active = None
        self.task = None
        self.bridge_open = False


# Response-before-turn_end: evidence only; B stays rejected until turn_end.
s = Model()
s.send("A")
s.response("A", "cancelled")
s.send("B")
print(f"CHOICE-A response_releases_A={s.active is None}")
print(f"CHOICE-A response_evidence={s.evidence}")
print(f"CHOICE-A B_rejected_before_turn_end={'B' in s.rejected}")
assert s.active == "A" and s.completions == [] and "B" in s.rejected
s.turn_end("A", "end_turn")
print(f"CHOICE-A turn_end_completions={s.completions}")
print(f"CHOICE-A active_task_after_turn_end={s.task}")
s.send("B")
print(f"CHOICE-A B_active_after_turn_end={s.active == 'B'}")
assert s.completions == ["A"] and s.task == "B" and s.active == "B"

# Turn_end-before-response: abort removes A's event-producing task before B.
t = Model()
t.send("A")
t.turn_end("A", "end_turn")
t.send("B")
t.response("A", "cancelled")
late_response = ("A", "cancelled")
print(f"CHOICE-A abort_record={t.lifecycle}")
print(f"CHOICE-A late_A_entered_B_lifetime={late_response in t.evidence}")
print(f"CHOICE-A discarded_after_abort={t.discarded}")
assert t.active == "B" and t.lifecycle == ["abort:A"]
assert late_response not in t.evidence

# KAS prompt error is fail-stop, so its optional turn_end cannot meet B.
f = Model()
f.send("A")
f.prompt_failure("A")
f.send("B")
print(f"CHOICE-A failure_order={f.lifecycle}")
print(f"CHOICE-A B_rejected_after_failstop={'B' in f.rejected}")
assert f.lifecycle == ["BridgeError", "TurnCompleted", "BridgeDisconnected"]
assert "B" in f.rejected and not f.bridge_open
print("REVISED-CHEAPEST-PASSED")
print("REVISED-DESIGN-GATE-PASSED")

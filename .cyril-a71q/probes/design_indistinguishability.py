"""Artifact-only cheapest falsifier for the signed KAS ownership contract."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Visible:
    active_owner: int
    active_session: str
    completed_sources: tuple[tuple[int, str], ...]
    event_scope: str
    event_source: str
    event_reason: str


# Hidden ownership is deliberately excluded from Visible: genuine KAS turn_end
# has sessionId and reasons, but no native turn id.
observer_input = Visible(
    active_owner=2,
    active_session="sess_main",
    completed_sources=((1, "prompt_response"),),
    event_scope="sess_main",
    event_source="kas_turn_end",
    event_reason="end_turn",
)

worlds = {
    "late_A_turn_end": {"hidden_event_owner": 1, "required": "DROP_STALE"},
    "B_turn_end_old_source_absent": {"hidden_event_owner": 2, "required": "COMPLETE_B"},
}

print("C1-OBSERVATION-EQUALITY:", observer_input == observer_input)
for name, hidden in worlds.items():
    print(
        f"C1-WORLD {name}: visible={observer_input!r} "
        f"hidden_owner={hidden['hidden_event_owner']} required={hidden['required']}"
    )

required = {hidden["required"] for hidden in worlds.values()}
print("C1-REQUIRED-DISPOSITIONS:", ",".join(sorted(required)))
print("C1-DETERMINISTIC-OBSERVER-OUTPUTS:", 1)

if len(required) != 2:
    raise SystemExit("C1-FALSIFIED: the two worlds do not require opposite dispositions")

for decision in ("DROP_STALE", "COMPLETE_B", "WAIT"):
    failures = []
    if decision != "DROP_STALE":
        failures.append("late_A_turn_end safety")
    if decision != "COMPLETE_B":
        failures.append("B turn_end liveness when both old turn_end and B response are absent")
    print(f"C1-POLICY {decision}: FAILS={' + '.join(failures)}")

print("C1-PASSED: identical production-visible input requires opposite outputs")
print("DESIGN-GATE-FAILED: correlation or a contract relaxation is required")

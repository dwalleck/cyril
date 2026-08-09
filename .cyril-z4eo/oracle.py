#!/usr/bin/env python3
"""Predict FIFO ownership from source operations without running UiState."""
from collections import deque
from pathlib import Path

root = Path(__file__).resolve().parent.parent
source = (root / "crates/cyril-ui/src/state.rs").read_text()
required_operations = (
    "approvals: VecDeque<ApprovalState>",
    "self.approvals.push_back(ApprovalState",
    "self.approvals.front()",
    "self.approvals.pop_front()?",
    "self.approvals.push_front(approval)",
)
missing = [operation for operation in required_operations if operation not in source]
if missing:
    raise SystemExit(f"implementation no longer matches FIFO ownership model: {missing}")

model = deque(["first", "second"])
head1 = model[0]
first = model.popleft()
first_state = "selected" if first == "first" else "wrong"
second_state = "pending" if model[0] == "second" else "wrong"

print(f"head1={head1}")
print("origin1=repeated-session")
print(f"first_after_resolution={first_state}")
print(f"second_after_resolution={second_state}")
print(f"head2={model[0]}")

#!/usr/bin/env python3
"""Predict the two-request outcome by inspecting ownership operations, not running UiState."""
from pathlib import Path

root = Path(__file__).resolve().parent.parent
source = (root / "crates/cyril-ui/src/state.rs").read_text()
show_start = source.index("    pub fn show_approval")
show_end = source.index("\n    }", show_start)
show_body = source[show_start:show_end]
confirm_start = source.index("    pub fn approval_confirm")
confirm_end = source.index("\n    }", confirm_start)
confirm_body = source[confirm_start:confirm_end]

overwrites_slot = "self.approval = Some(ApprovalState" in show_body
consumes_slot = "self.approval.take()?" in confirm_body
if not (overwrites_slot and consumes_slot):
    raise SystemExit("implementation no longer matches the single-slot ownership model")

print("head1=second")
print("first_after_resolution=closed")
print("second_after_resolution=selected")
print("head2=none")

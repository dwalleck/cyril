#!/usr/bin/env python3
"""Lexically inventory current ownership/routing/capacity seams (not runtime proof)."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
files = {
    "bridge": ROOT / "crates/cyril-core/src/protocol/bridge.rs",
    "client": ROOT / "crates/cyril-core/src/protocol/client.rs",
    "event": ROOT / "crates/cyril-core/src/types/event.rs",
    "app": ROOT / "crates/cyril/src/app.rs",
}
text = {name: path.read_text(encoding="utf-8") for name, path in files.items()}
checks = {
    "notification_capacity_256": r"NOTIFICATION_CAPACITY: usize = 256",
    "turn_guard_session_only": r"turn_in_flight: Option<acp::SessionId>",
    "completion_checks_only_some": r"if turn_in_flight\.is_none\(\)",
    "synthesized_global": r"let note = Notification::TurnCompleted \{ stop_reason \}",
    "kas_scoped_at_client": r"RoutedNotification::scoped\(session_id, notification\)",
    "global_has_none": r"session_id: None",
    "app_foreign_early_return": r"apply_subagent_notification\(sid, &notification\)",
    "shutdown_aborts_task": r"handle\.abort\(\)",
    "deferred_disconnect": r"deferred_disconnect: Option<String>",
    "ownership_counter_absent": r"TurnId|turn_seq|turn_owner|ownership_counter",
}
joined = "\n".join(text.values())
for name, pattern in checks.items():
    print(f"{name}={bool(re.search(pattern, joined))}")

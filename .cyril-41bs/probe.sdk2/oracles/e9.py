#!/usr/bin/env python3
import json
import subprocess
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
tests = [
    ("cyril-core", "c1_fragmentation_is_utf8_safe_and_bounded"),
    ("cyril-core", "c1_accepted_prompt_capture_precedes_ui_and_excludes_context"),
    ("cyril-core", "c2_terminal_disposition_never_false_completes"),
    ("cyril-core", "c3_tool_snapshot_payload_is_bounded_with_truncation_metadata"),
    ("cyril-core", "c6_stream_tool_tail_assembles_without_thoughts_or_secrets"),
    ("cyril-core", "c9_slow_capture_is_bounded_and_shutdown_drains_in_order"),
    ("cyril-core", "c9_ingress_quiescence_stays_within_bridge_budget"),
    ("cyril-core", "c12_source_identity_survives_numeric_reuse_and_ignores_history"),
    ("cyril", "memory_failure_does_not_block_initial_session_dispatch"),
    ("cyril", "c5_first_prompt_is_ordered_exactly_once_and_source_clean"),
    ("cyril", "first_prompt_lessons_wait_for_a_starting_companion"),
    ("cyril", "in_process_runtime_serves_first_prompt_context_and_reports_starting"),
]
results = {}
for package, name in tests:
    completed = subprocess.run(
        ["cargo", "test", "-p", package, name],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    results[f"{package}:{name}"] = completed.returncode == 0
    if completed.returncode != 0:
        raise SystemExit(completed.stdout + completed.stderr)

app_source = (repo / "crates/cyril/src/app.rs").read_text()
observer_source = (repo / "crates/cyril-core/src/protocol/source_observer.rs").read_text()
facts = {
    "claim_ids": ["C9"],
    "behavioral_contract_tests": results,
    "single_prompt_dispatch_seam": "async fn send_prompt(" in app_source,
    "first_prompt_context_prepared_before_bridge": (
        "PromptEnvelope::prepared(content_blocks, prepared_context)" in app_source
    ),
    "source_observer_records_original_prompt": "prompt.original_blocks()" in observer_source,
    "source_observer_tracks_terminal_disposition": "SourceTurnDisposition" in observer_source,
}
facts["wire_tap_is_supplement_not_replacement"] = (
    facts["first_prompt_context_prepared_before_bridge"]
    and facts["source_observer_records_original_prompt"]
    and facts["source_observer_tracks_terminal_disposition"]
)
facts["independent_oracle_passed"] = all(results.values()) and all(
    value
    for key, value in facts.items()
    if key not in {"claim_ids", "behavioral_contract_tests"}
)
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["independent_oracle_passed"]:
    raise SystemExit(1)

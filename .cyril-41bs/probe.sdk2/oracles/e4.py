#!/usr/bin/env python3
import json
import subprocess
import tempfile
from pathlib import Path

roots = list((Path.home() / ".cargo/registry/src").glob("*/agent-client-protocol-2.0.0"))
if len(roots) != 1:
    raise SystemExit(f"expected one pinned SDK source, found {roots}")
sdk_source = (roots[0] / "src/acp_agent.rs").read_text()
repo = Path(__file__).resolve().parents[3]
cyril_transport = (repo / "crates/cyril-core/src/protocol/transport.rs").read_text()
cyril_bridge = (repo / "crates/cyril-core/src/protocol/bridge.rs").read_text()
probe_manifest = Path(__file__).resolve().parents[1] / "Cargo.toml"

with tempfile.TemporaryDirectory(prefix="cyril-e4-oracle-") as directory:
    completed = subprocess.run(
        ["/bin/sh", "-c", "printf '%s' \"$PWD\"; printf diagnostic >&2; exit 17"],
        cwd=directory,
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )


def run_probe(mode=None):
    command = [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(probe_manifest),
        "--bin",
        "e4",
    ]
    if mode is not None:
        command.extend(["--", mode])
    return subprocess.run(
        command,
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
        timeout=180,
    )


default_run = run_probe()
default_json = None
default_json_valid = default_run.returncode == 0
if default_json_valid:
    try:
        default_json = json.loads(default_run.stdout)
    except json.JSONDecodeError:
        default_json_valid = False

default_probe_contract = (
    default_json_valid
    and default_json.get("claim_ids") == ["C6"]
    and default_json.get("cwd_field_present") is False
    and default_json.get("cwd_field_rejected") is True
    and default_json.get("runtime_inherits_parent_cwd") is True
    and default_json.get("nonzero_exit_is_error") is True
    and default_json.get("nonzero_error_contains_status") is True
    and default_json.get("nonzero_error_contains_stderr") is True
    and "17" in default_json.get("nonzero_error_debug", "")
    and "process-probe-diagnostic" in default_json.get("nonzero_error_data", "")
    and default_json.get("clean_exit_stderr_debug_callback") == ["clean-debug-line"]
    and default_json.get("public_stderr_tail_accessor") is False
    and default_json.get("same_process_group") is True
    and default_json.get("stall_watchdog_absent_after_ms") == 1_200
    and default_json.get("stall_future_remained_pending") is True
    and default_json.get("drop_cancelled_connection_future") is True
    and default_json.get("drop_killed_direct_and_grandchild") is True
    and default_json.get("stdout_eof_returned_ok") is True
    and default_json.get("stdout_eof_killed_nonexiting_child_within_grace") is True
)

expected_negative_diagnostic = (
    "Error: C6 unexpected-success-control: "
    "non-zero helper unexpectedly succeeded"
)
negative_run = run_probe("unexpected-success-control")
negative_stderr_lines = negative_run.stderr.strip().splitlines()
negative_probe = {
    "failed_nonzero": negative_run.returncode != 0,
    "diagnostic": negative_stderr_lines[-1] if negative_stderr_lines else "",
    "exact_diagnostic": bool(negative_stderr_lines)
    and negative_stderr_lines[-1] == expected_negative_diagnostic,
    "no_panic": "panicked" not in negative_run.stderr,
}

facts = {
    "claim_ids": ["C6"],
    "os_explicit_cwd_honored": completed.stdout == directory,
    "os_nonzero_exit": completed.returncode == 17,
    "os_stderr_captured": completed.stderr == "diagnostic",
    "sdk_shutdown_grace_one_second": "SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(1)" in sdk_source,
    "sdk_config_has_no_cwd_field": "pub struct AcpAgentConfig {" in sdk_source
    and "cwd:" not in sdk_source.split("pub struct AcpAgentConfig {", 1)[1].split("}", 1)[0],
    "sdk_spawn_does_not_set_current_dir": ".current_dir(" not in sdk_source,
    "cyril_transport_sets_current_dir": ".current_dir(cwd)" in cyril_transport,
    "cyril_retains_bounded_stderr_tail": "StderrTail" in cyril_transport,
    "cyril_retains_stall_watchdog": "DEFAULT_STALL_THRESHOLD" in cyril_bridge,
    "probe_default_returncode": default_run.returncode,
    "probe_default_json": default_json,
    "probe_default_json_valid": default_json_valid,
    "probe_default_contract": default_probe_contract,
    "probe_negative_mode": negative_probe,
}
facts["independent_oracle_passed"] = all(
    value for key, value in facts.items()
    if key not in {
        "claim_ids",
        "probe_default_returncode",
        "probe_default_json",
        "probe_negative_mode",
    }
) and all(negative_probe.values())
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["independent_oracle_passed"]:
    raise SystemExit(1)

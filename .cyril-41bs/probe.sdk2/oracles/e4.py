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

with tempfile.TemporaryDirectory(prefix="cyril-e4-oracle-") as directory:
    completed = subprocess.run(
        ["/bin/sh", "-c", "printf '%s' \"$PWD\"; printf diagnostic >&2; exit 17"],
        cwd=directory,
        text=True,
        capture_output=True,
        check=False,
    )

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
}
facts["independent_oracle_passed"] = all(
    value for key, value in facts.items() if key != "claim_ids"
)
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["independent_oracle_passed"]:
    raise SystemExit(1)

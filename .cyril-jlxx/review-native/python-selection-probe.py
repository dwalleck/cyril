"""Observe host interpreter selection while exercising the real config peer."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile

with tempfile.TemporaryDirectory(prefix="cyril-python-selection-") as temporary:
    root = Path(temporary)
    marker = root / "selected"
    shim = root / "python3"
    # A version-manager-style entry point resolves to the real interpreter.
    # The peer's replacement PATH does not contain this temporary directory.
    shim.write_text(
        "#!/bin/sh\n"
        + "printf selected > " + shlex.quote(str(marker)) + "\n"
        + "exec " + shlex.quote(sys.executable) + ' "$@"\n'
    )
    shim.chmod(0o700)
    environment = dict(os.environ)
    environment["PATH"] = str(root) + os.pathsep + environment["PATH"]
    result = subprocess.run(
        [sys.argv[1], "--exact", "standard_configuration_uses_returned_catalog_and_rejects_malformed_response", "--nocapture"],
        env=environment, text=True, capture_output=True, timeout=45,
    )
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    observations = {"peer_test_exit": result.returncode, "host_python_selected": marker.exists()}
    print(json.dumps(observations, sort_keys=True))
    assert result.returncode == 0, "configuration protocol fixture failed"
    assert marker.exists(), "fixture bypassed the runnable host Python entry point"

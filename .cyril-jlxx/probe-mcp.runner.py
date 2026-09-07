#!/usr/bin/env python3
"""Timestamp public bridge output and snapshot independently written MCP markers."""
import json
from pathlib import Path
import subprocess
import sys
import time
root = Path(sys.argv[1]).resolve(strict=True)
assert root.parent == Path('/tmp') and root.name.startswith('cyril-jlxx-')
with (root / 'observations.jsonl').open('w') as observations:
    with subprocess.Popen(['python3', '/home/REDACTED_USER/repos/cyril-wt-feat-cyril-jlxx/.cyril-jlxx/probe.py', *sys.argv[1:]], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True) as child:
        for line in child.stdout:
            markers = {name: (root / (name + '.jsonl')).read_text() if (root / (name + '.jsonl')).exists() else None for name in ['globalcanary', 'workspacecanary']}
            observations.write(json.dumps({'time_ns': time.time_ns(), 'line': line.rstrip(), 'markers': markers}) + '\n')
            observations.flush()
            print(line, end='', flush=True)
        sys.exit(child.wait())

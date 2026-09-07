#!/usr/bin/env python3
"""Launch only the throwaway bridge consumer with a credential-minimized environment."""
import os
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve(strict=True)
binary = Path(__file__).resolve().parent / 'target/debug/cyril-jlxx-probe'
real_home = Path.home()
env = {
    'PATH': os.environ['PATH'],
    'HOME': str(root / 'home'),
    'KIRO_HOME': str(root / 'home/.kiro'),
    'XDG_CONFIG_HOME': str(root / 'config'),
    'XDG_CACHE_HOME': str(root / 'cache'),
    'XDG_DATA_HOME': os.environ.get('XDG_DATA_HOME', str(real_home / '.local/share')),
    'TMPDIR': str(root / 'tmp'),
    'LANG': 'C.UTF-8',
}
# Retain transport configuration, never provider tokens or cloud credentials.
env['JLXX_PROFILE'] = sys.argv[2]
env['JLXX_PROMPT'] = str(Path(sys.argv[3]).resolve(strict=True))
env['JLXX_HOOKS'] = sys.argv[4] if len(sys.argv) > 4 else 'off'
if len(sys.argv) > 5:
    env['JLXX_SECOND_PROMPT'] = str(Path(sys.argv[5]).resolve(strict=True))
if os.environ.get('JLXX_CAPTURE_TOOLS') == '1':
    env['JLXX_LAUNCHER'] = str(Path(__file__).resolve().parent / 'probe-tap.py')
    env['JLXX_TOOL_INVENTORY'] = str(root / 'tools.jsonl')
for key in ('HTTPS_PROXY', 'HTTP_PROXY', 'ALL_PROXY', 'NO_PROXY', 'SSL_CERT_FILE', 'SSL_CERT_DIR', 'NODE_EXTRA_CA_CERTS'):
    if key in os.environ:
        env[key] = os.environ[key]
os.chdir(root / 'workspace')
os.execve(binary, [str(binary)], env)

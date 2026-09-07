#!/usr/bin/env python3
"""Pass-through CLI stdio tap; records only native tool inventory notifications."""
import json
import os
import subprocess
import sys
import threading

if sys.argv[1:] == ['--version']:
    os.execvp('kiro-cli', ['kiro-cli', '--version'])
process = subprocess.Popen(['kiro-cli', *sys.argv[1:]], stdin=subprocess.PIPE, stdout=subprocess.PIPE)

def forward_input():
    try:
        for line in sys.stdin.buffer:
            process.stdin.write(line)
            process.stdin.flush()
    except BrokenPipeError:
        return
    finally:
        try:
            process.stdin.close()
        except BrokenPipeError:
            pass

thread = threading.Thread(target=forward_input, daemon=True)
thread.start()
with open(os.environ['JLXX_TOOL_INVENTORY'], 'a') as evidence:
    for line in process.stdout:
        try:
            message = json.loads(line)
            if message.get('method') in ('_kiro/tools/didChange', '_kiro/policy/changed'):
                evidence.write(json.dumps(message) + '\n')
                evidence.flush()
        except (ValueError, AttributeError):
            pass
        sys.stdout.buffer.write(line)
        sys.stdout.buffer.flush()
sys.exit(process.wait())

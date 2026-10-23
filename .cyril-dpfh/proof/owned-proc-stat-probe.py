#!/usr/bin/env python3
"""One owned, stdin-gated child; distinguish proc read ESRCH from exact wait."""
import contextlib
import errno
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys

REPORT = Path(__file__).with_suffix('.json')
receipt = {
    'question': 'Does an unread proc-stat fd return ESRCH after exact-child normal exit and reap?',
    'kernel': platform.release(),
    'python': sys.version,
    'probe_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'stdout_stderr': 'DEVNULL',
    'child_wait_timeout_seconds': 5,
    'cleanup_wait_timeout_seconds': 5,
    'numeric_pid_signal_fallback': False,
}
child = None
pidfd = None
child_source = (
    'import select, sys; '
    'ready, _, _ = select.select([sys.stdin.buffer], [], [], 5); '
    'raise SystemExit(0 if ready and sys.stdin.buffer.read(1) == b"x" else 2)'
)
try:
    with contextlib.ExitStack() as stack:
        child = subprocess.Popen(
            [sys.executable, '-I', '-S', '-c', child_source],
            stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            close_fds=True,
        )
        receipt['child_pid'] = child.pid
        try:
            pidfd = os.pidfd_open(child.pid)
            stack.callback(os.close, pidfd)
            stat_path = f'/proc/{child.pid}/stat'
            live_fd = os.open(stat_path, os.O_RDONLY | os.O_CLOEXEC)
            stack.callback(os.close, live_fd)
            retained_fd = os.open(stat_path, os.O_RDONLY | os.O_CLOEXEC)
            stack.callback(os.close, retained_fd)
            live_bytes = os.read(live_fd, 16384)
            live_state = live_bytes.rsplit(b')', 1)[1].split()[0].decode('ascii')
            receipt['live_control'] = {
                'operation': 'read separate proc-stat fd before child release',
                'bytes': len(live_bytes),
                'state': live_state,
                'exact_child_poll': child.poll(),
            }
            assert live_state != 'Z', receipt['live_control']
            assert receipt['live_control']['exact_child_poll'] is None
            receipt['retained_fd_opened_while_alive_and_unread'] = True
            assert child.stdin is not None
            child.stdin.write(b'x')
            child.stdin.close()
            returncode = child.wait(timeout=5)
            receipt['exact_wait'] = {
                'operation': 'Popen.wait on owned child after normal stdin release',
                'returncode': returncode,
                'reaped': True,
            }
            assert returncode == 0, receipt['exact_wait']
            try:
                stale_bytes = os.read(retained_fd, 16384)
            except OSError as error:
                receipt['retained_descriptor_read'] = {
                    'operation': 'first read of retained proc-stat fd after exact wait/reap',
                    'errno': error.errno,
                    'errno_name': errno.errorcode.get(error.errno) if error.errno is not None else None,
                    'message': error.strerror,
                }
                assert error.errno == errno.ESRCH, receipt['retained_descriptor_read']
            else:
                receipt['retained_descriptor_read'] = {'bytes': len(stale_bytes)}
                raise AssertionError('retained proc descriptor unexpectedly readable')
        finally:
            if child.stdin is not None and not child.stdin.closed:
                child.stdin.close()
            if child.returncode is None:
                if pidfd is not None:
                    try:
                        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
                    except ProcessLookupError:
                        receipt['cleanup_pidfd_signal'] = 'ESRCH on exact pidfd'
                    else:
                        receipt['cleanup_pidfd_signal'] = 'SIGKILL sent on exact pidfd'
                receipt['cleanup_exact_wait_returncode'] = child.wait(timeout=5)
    receipt['handles_closed'] = True
    receipt['outcome'] = 'mechanism confirmed; historical CI throwing site remains unproven'
except BaseException as error:
    receipt['probe_failure'] = {'type': type(error).__name__, 'message': str(error)}
    raise
finally:
    REPORT.write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt, indent=2))

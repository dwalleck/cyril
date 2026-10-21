import os
from pathlib import Path
import runpy
import subprocess
import sys

fixture = str(Path(sys.argv[1]).resolve())
gate_fd, release_fd, descendant_fd = map(int, sys.argv[2:])
real_popen = subprocess.Popen
children = []


def owned_descendant(argv, **kwargs):
    assert argv == [sys.executable, '-c', 'import time; time.sleep(120)'], argv
    assert 'pass_fds' not in kwargs
    kwargs['pass_fds'] = (descendant_fd,)
    child = real_popen(
        [sys.executable, '-c', 'import os; os.read(' + str(descendant_fd) + ', 1)'],
        **kwargs,
    )
    children.append(child)
    return child


def gate(frame, event, arg):
    if (event == 'return' and frame.f_code.co_filename == fixture
            and frame.f_code.co_name == 'journal'
            and frame.f_locals.get('event') == 'prompt'):
        assert frame.f_globals['PENDING'] is not None
        os.write(gate_fd, b'P')
        os.close(gate_fd)
        assert os.read(release_fd, 1) == b'R', 'prompt gate not released'
        os.close(release_fd)
        sys.settrace(None)
    return gate


subprocess.Popen = owned_descendant
sys.settrace(gate)
try:
    runpy.run_path(fixture, run_name='__main__')
finally:
    sys.settrace(None)
    subprocess.Popen = real_popen
    os.close(descendant_fd)
    for child in children:
        child.wait(timeout=5)

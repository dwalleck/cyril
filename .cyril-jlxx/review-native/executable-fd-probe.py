import errno
import json
import os
from pathlib import Path
import subprocess
import tempfile

with tempfile.TemporaryDirectory() as directory:
    path = Path(directory) / 'peer.sh'
    writer = path.open('w')
    writer.write('#!/bin/sh\nexit 0\n')
    writer.flush()
    path.chmod(0o700)
    release_read, release_write = os.pipe()
    child = os.fork()
    if child == 0:
        os.close(release_write)
        os.read(release_read, 1)
        writer.close()
        os._exit(0)
    os.close(release_read)
    writer.close()
    observed = None
    try:
        subprocess.run([str(path)], check=True, timeout=5)
    except OSError as error:
        observed = error.errno
    finally:
        os.write(release_write, b'x')
        os.close(release_write)
        os.waitpid(child, 0)
    restored = subprocess.run([str(path)], check=False, timeout=5).returncode
    print(json.dumps({'parent_writer_closed': True, 'inherited_writer_caused_ETXTBSY': observed == errno.ETXTBSY, 'launch_after_child_release': restored}))
    assert observed == errno.ETXTBSY and restored == 0

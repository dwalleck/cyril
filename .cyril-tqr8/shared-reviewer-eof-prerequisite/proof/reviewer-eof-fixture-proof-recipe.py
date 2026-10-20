#!/usr/bin/env python3
"""SOURCE/PREPARATION ONLY — not executed by RepairReviewerEofFixture.

Repair owner/source
===================
Worktree: /home/dwalleck/repos/cyril-wt-fix-reviewer-eof-fixture
Branch: fix/reviewer-eof-fixture
Base: 8e5c325ebb5dac3a4ad89f71838fd198cc177543
Exact clean repair patch: local://reviewer-eof-fixture.patch
Captured native diff and hashes: artifact://1292:raw.
The two base files were byte-compared successfully with the actual inspected
/home/dwalleck/repos/cyril-wt-fix-cyril-tqr8 files BEFORE editing.

SHA256 preimages -> repaired images:
crates/cyril-workbench/tests/reviewer.rs
  1facee0bf90b1eef7ea829b45bc65106f4d5010091d5f34ce1ef87d24123204e
  122f9d493f244769c72376ae82d8877f391ab98a5e13bfc03bca3731df326af1
crates/cyril-workbench/tests/fixtures/reviewer_peer.py
  ac4eac61eb39048edeb942c7de8b9ee287fec386b5ef77e22d84cdaf541a3a71
  e0f3efab80b799bd7a052d32fda8c9e959f181a70c0ad89b69e6498f0469d04a
Unchanged production mutation preimage (both owned repair and actual Tqr tree):
crates/cyril-workbench/src/reviewer/evidence.rs
  626f7307fdcb0879e16aa947ce0840df57e9e262f9ba1aaf9b4a9c631efe85ff

Exact complete base source bytes remain available with:
  git show 8e5c325ebb5dac3a4ad89f71838fd198cc177543:crates/cyril-workbench/tests/reviewer.rs
  git show 8e5c325ebb5dac3a4ad89f71838fd198cc177543:crates/cyril-workbench/tests/fixtures/reviewer_peer.py
Do not use HEAD as a preimage after integration.

Route/implementation contract
==============================
Local scoped prerequisite: T1 no unresolved fact changes the requested readiness
repair (historical occurrence attribution remains UNPROVEN); T2 no public seam or
owner changes; T3 no production behavior/risk introduced; T4 explicit assignment.
No repository workflow docs added: parent owns integration/proof/publication.
The marker follows evidence reads AND successful text('waiting') stdout flush;
PENDING remains set and no end_turn is added. Test waits for that marker.
Both EOF evidence probes reuse the absolute prompt-time evidence path, so removal
of cwd yields false rather than Path.cwd() raising. The SAME 15s post-drop poll
now waits for both cwd removal and cleanup-window-end before its existing
assertions. Both evidence-presence, eventual removal, pid/child and runtime-parent
assertions remain. The existing 5s readiness/event, 1s startup, 100ms stall,
400ms fixture window, 15s cleanup and 2s PRODUCTION grace budgets are untouched.
No BrokenPipe catch, optional EOF, descendant signal, or production edit.

Producer proof command (PARENT ONLY)
====================================
Materialize this exact local:// source as a real scratch file using the tool
read/write helpers, not a repository edit, then run:
  python3 /tmp/reviewer-eof-fixture-proof-recipe.py \
    --fixture /home/dwalleck/repos/cyril-wt-fix-reviewer-eof-fixture/crates/cyril-workbench/tests/fixtures/reviewer_peer.py \
    --output /tmp/reviewer-eof-producer-evidence-UNIQUE
The output directory MUST NOT exist. No native Kiro, network or account is used.

Controlled-shim boundary: the REAL copied fixture executes via runpy. A Python
trace return hook gates exactly after journal('prompt') returns, before its
evidence reads/final stdout send. This does not replace its protocol producer.
For owned cleanup ONLY, its exact known 120s DEVNULL sleeper launch is replaced
with a real Python descendant waiting on a separately inherited release pipe.
Closing that pipe makes the owned descendant exit naturally; no descendant/group
kill, shortened EOF window, or production lifecycle claim is involved. This
producer proof does NOT prove the fixture's original descendant-lifetime oracle;
the actual bridge regression retains that unchanged 120s sleeper/process oracle.

Cases use the same corrected real fixture and same gate, changing only teardown
ordering:
1. old-prompt: observe old prompt at gate; close stdout reader + stdin writer;
   release gate. Require evidence processing, nonzero exit, literal
   BrokenPipeError stderr, no readiness, no stdin-closed, neither EOF event.
2. post-flush: release gate with pipes open; await real post-flush readiness;
   require preceding evidence and actual waiting stdout; pending prompt has no
   terminal/reply. Close same pipes. Require successful exit and BOTH EOF events
   with evidence_exists == true. This uses the same event names/presence meaning
   as the real regression, not a mock echo or substituted EOF producer.
The proof records raw stderr/stdout, copied real source, shim, synthetic inputs,
spawn/event monotonic records, exit status, and a JSON result per case.

Actual bridge regression (PARENT ONLY)
======================================
Run in the repaired worktree, then in the actual integrated Tqr/final tree:
  cargo test -p cyril-workbench --test reviewer runtime_drop_retains_evidence_until_core_completion -- --exact --nocapture
This is the real Reviewer -> dedicated core bridge -> SDK transport -> actual
controlled executable seam, with all original pending-turn/evidence/process
assertions. Also run the full affected reviewer integration target/workspace gate
under the parent's existing qualification environment. No check was run here.

Compiling early-evidence-cleanup mutation (PARENT ONLY)
======================================================
Use ONE exclusive parent-owned proof tree containing the repaired test+fixture.
Never mutate actual Tqr, final assembly, another agent's tree, or use git restore
on dirty source. Snapshot the CURRENT evidence.rs BYTES to a new scratch backup
with exclusive creation and record SHA256 before any edit; this preserves any
pre-existing dirty edits. Pin/compare the current tests+fixture hashes above.
Concrete preparation cell for the parent's Python Eval (does NOT edit source):
    from pathlib import Path
    import hashlib, tempfile
    mutation_target = Path("/home/dwalleck/repos/cyril-wt-fix-reviewer-eof-fixture/crates/cyril-workbench/src/reviewer/evidence.rs")
    mutation_original = mutation_target.read_bytes()
    mutation_needle = b"        let root = tree.root.keep();" + bytes([10])
    mutation_line = b'        fs::remove_dir_all(&root).expect("owned early-evidence-cleanup mutation");' + bytes([10])
    assert mutation_original.count(mutation_needle) == 1
    mutation_expected = mutation_original.replace(mutation_needle, mutation_needle + mutation_line)
    mutation_backup_dir = Path(tempfile.mkdtemp(prefix="reviewer-eof-mutation-"))
    mutation_backup = mutation_backup_dir / "evidence.rs.original"
    with mutation_backup.open("xb") as saved:
        saved.write(mutation_original)
    print(mutation_backup, hashlib.sha256(mutation_original).hexdigest())
Use the edit tool for the following insertion, then BEFORE Cargo run this Eval:
    assert mutation_target.read_bytes() == mutation_expected
The equality proves the intended compiling mutation, not an unrelated edit.
Read the mutation target with the edit tool; after the existing line
    let root = tree.root.keep();
insert EXACTLY:
    fs::remove_dir_all(&root).expect("owned early-evidence-cleanup mutation");
Do not change completion waiting, deadlines or anything else. fs is already in
scope and root's existing PathBuf type supports this call; this is a compiling
source mutation by inspection, NOT a claim it was compiled here. Synchronous
removal in Drop makes the violation happen before launching the cleanup waiter,
avoiding a race in a newly scheduled early-cleanup thread.
Run the EXACT actual bridge regression command above, capturing exit code and
raw output to owned files. Required RED signature is assertion
  cleanup-window: evidence removed before core completion
or
  cleanup-window-end: evidence removed before core completion
with left Bool(false), right true. Nonzero from compilation, missing readiness,
missing EOF event or another assertion is NOT a successful sensitivity proof.
The cached absolute path and poll-through-end are essential: otherwise early
removal can raise Path.cwd()/cause an immediate missing-event snapshot instead
of attributing evidence loss. Do not increase any bound to get this signature.
Restore by removing ONLY the inserted line with the edit tool IN A FINALLY
cleanup, then byte-compare to the exclusive original backup + SHA256. Never
replace current dirty bytes with a committed preimage. If current bytes differ
from the one expected mutated image, STOP rather than overwrite another edit;
retain backup and exact diff for the owner.
Concrete restore receipt in the same parent Eval kernel after the inverse edit:
    assert mutation_target.read_bytes() == mutation_original == mutation_backup.read_bytes()
    print("exact dirty-source restoration", hashlib.sha256(mutation_original).hexdigest())
Compile output proves whether the mutation compiles. After exact restoration,
rerun the same regression and the parent's affected qualification gate.
Parent owns all results and disposition.

Residual risks / evidence honesty
=================================
This source repair closes a demonstrated ordering hole, not a proven attribution
of the historical Linux occurrence. Historical log lacks fixture exit/stderr and
which EOF event was missing. Grace expiry, OS scheduling and separate PID/reap
concerns remain distinguishable and are not waived. A named missing EOF event is
still a hard failure. Source-only mutation may require adjustment if parent
integration changes evidence.rs; re-read rather than apply stale anchors.
No test, build, formatter, lint, producer proof or mutation was executed here.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import sys
import time

FIXTURE_SHA256 = 'e0f3efab80b799bd7a052d32fda8c9e959f181a70c0ad89b69e6498f0469d04a'
WINDOW_EVENTS = ('cleanup-window', 'cleanup-window-end')
SHIM = r'''import os
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
'''


def rows(path):
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines()]


def wait_event(path, name):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        observed = rows(path)
        if any(row['event'] == name for row in observed):
            return observed
        time.sleep(0.01)
    raise AssertionError(f'missing {name}; observed={rows(path)!r}')


def wait_byte(fd):
    assert select.select([fd], [], [], 5)[0], 'prompt gate deadline'
    assert os.read(fd, 1) == b'P', 'prompt gate did not observe old marker'


def read_stdout_line(pipe):
    data = b''
    deadline = time.monotonic() + 5
    while b'\n' not in data:
        remaining = deadline - time.monotonic()
        assert remaining > 0, 'waiting stdout deadline'
        assert select.select([pipe.fileno()], [], [], remaining)[0]
        chunk = os.read(pipe.fileno(), 65536)
        assert chunk, 'stdout ended before waiting message'
        data += chunk
    return data


def run_case(source, output, mode):
    case = output / mode
    case.mkdir()
    peer = case / 'peer'
    work = case / 'run'
    peer.mkdir()
    work.mkdir()
    fixture = peer / 'reviewer_peer.py'
    shutil.copyfile(source, fixture)
    (peer / 'peer-config.json').write_text(json.dumps({'scenario': 'runtime-drop'}))
    (peer / 'outside.txt').write_text('OWNED-SYNTHETIC-OUTSIDE-CANARY')
    evidence = work / 'evidence'
    evidence.mkdir()
    content = 'owned synthetic evidence\n'
    (evidence / 'document.txt').write_text(content)
    (evidence / 'manifest.json').write_text(json.dumps([
        {'file': 'document.txt', 'source_label': 'owned synthetic', 'bytes': len(content)}
    ]))
    shim = peer / 'shim.py'
    shim.write_text(SHIM)
    journal = peer / 'journal.jsonl'
    request = {'jsonrpc': '2.0', 'id': 7, 'method': 'session/prompt',
               'params': {'sessionId': 'owned-synthetic',
                          'prompt': [{'type': 'text', 'text': 'owned synthetic prompt'}]}}
    (case / 'request.json').write_text(json.dumps(request, indent=2) + '\n')
    gate_r, gate_w = os.pipe()
    release_r, release_w = os.pipe()
    descendant_r, descendant_w = os.pipe()
    open_fds = {gate_r, gate_w, release_r, release_w, descendant_r, descendant_w}

    def close_fd(fd):
        if fd in open_fds:
            os.close(fd)
            open_fds.remove(fd)

    child = None
    stdout = b''
    with (case / 'stderr.bin').open('w+b') as stderr:
        try:
            child = subprocess.Popen(
                [sys.executable, str(shim), str(fixture), str(gate_w),
                 str(release_r), str(descendant_r)],
                cwd=work, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=stderr, pass_fds=(gate_w, release_r, descendant_r),
            )
            assert child.stdin is not None and child.stdout is not None
            for fd in (gate_w, release_r, descendant_r):
                close_fd(fd)
            child.stdin.write((json.dumps(request) + '\n').encode())
            child.stdin.flush()
            wait_byte(gate_r)
            at_gate = rows(journal)
            names = [row['event'] for row in at_gate]
            assert 'prompt' in names and 'runtime-drop-ready' not in names, names
            assert 'evidence' not in names, 'trace gate is not immediately after old prompt'
            (case / 'at-gate.json').write_text(json.dumps(at_gate, indent=2) + '\n')
            if mode == 'old-prompt':
                child.stdout.close()
                child.stdin.close()
                os.write(release_w, b'R')
            else:
                os.write(release_w, b'R')
                ready = wait_event(journal, 'runtime-drop-ready')
                names = [row['event'] for row in ready]
                assert names.index('prompt') < names.index('evidence') < names.index('runtime-drop-ready')
                assert 'terminal' not in names, 'pending prompt unexpectedly completed'
                stdout = read_stdout_line(child.stdout)
                messages = [json.loads(line) for line in stdout.splitlines()]
                assert any(message.get('params', {}).get('update', {}).get('content', {}).get('text') == 'waiting'
                           for message in messages), messages
                assert not any(message.get('id') == 7 for message in messages), 'prompt reply completed turn'
                child.stdout.close()
                child.stdin.close()
            close_fd(release_w)
            close_fd(descendant_w)  # Natural owned descendant exit; never signal it.
            code = child.wait(timeout=5)
            observed = rows(journal)
            stderr.flush()
            stderr.seek(0)
            error = stderr.read()
            (case / 'stdout.bin').write_bytes(stdout)
            result = {'case': mode, 'exit_code': code, 'events': observed,
                      'stderr': error.decode(errors='replace'),
                      'stdout': stdout.decode(errors='replace'),
                      'fixture_sha256': hashlib.sha256(fixture.read_bytes()).hexdigest()}
            (case / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
            names = [row['event'] for row in observed]
            assert 'evidence' in names, 'did not reach real fixture evidence processing'
            if mode == 'old-prompt':
                assert code != 0, result
                assert b'BrokenPipeError' in error, result
                assert 'runtime-drop-ready' not in names, result
                assert 'stdin-closed' not in names, result
                assert not any(name in names for name in WINDOW_EVENTS), result
            else:
                assert code == 0, result
                assert 'stdin-closed' in names, result
                assert 'terminal' not in names, result
                for name in WINDOW_EVENTS:
                    matches = [row for row in observed if row['event'] == name]
                    assert len(matches) == 1, f'missing/duplicate EOF window event {name}: {observed!r}'
                    assert matches[0]['evidence_exists'] is True, f'{name}: evidence removed before completion'
            return result
        finally:
            # Close release pipes before waiting: no inherited-pipe-EOF collector.
            for fd in tuple(open_fds):
                close_fd(fd)
            if child is not None:
                for stream in (child.stdin, child.stdout):
                    if stream is not None and not stream.closed:
                        stream.close()
                if child.poll() is None:
                    child.kill()  # Exact owned direct shim only, never descendants/groups.
                child.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description='Owned real-fixture readiness-order producer proof')
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    source = args.fixture.resolve(strict=True)
    actual = hashlib.sha256(source.read_bytes()).hexdigest()
    assert actual == FIXTURE_SHA256, f'fixture drift: {actual}; re-read and re-pin before proof'
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    results = [run_case(source, output, mode) for mode in ('old-prompt', 'post-flush')]
    (output / 'summary.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps({'producer_proof': 'PASS', 'output': str(output),
                      'cases': [{'case': row['case'], 'exit_code': row['exit_code']} for row in results]}))


if __name__ == '__main__':
    main()

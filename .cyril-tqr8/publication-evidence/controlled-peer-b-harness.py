#!/usr/bin/env -S uv run --offline --script
# /// script
# dependencies = ["pyte"]
# ///
"""Owned peer / actual WORKSPACEBLOCKED partial Tqr candidate consumer evidence.
No native KAS/account/network/auth/browser/clipboard/model/grants. The native
URL and failure text are retained observations; ACP envelopes and session/
request IDs are explicitly synthetic, NOT byte-for-byte native-frame replay.
Additional control-character specimens are synthetic. No private logs read.
"""
import argparse
import codecs
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

ROOT = Path('/home/dwalleck/repos/cyril-wt-fix-cyril-tqr8')
BINARY = Path('/tmp/cyril-tqr8-gutter-smoke-candidate-20261020')
BINARY_SHA = '2954a006f2cf3cff62b5bf3c0ba1a547bc86a4ba2e797feee22be934d80924b2'
SOURCE_TREE = '851c899f9e9da24cccc001020640de47771d8a5a'
FAKE = ROOT / 'crates/cyril-core/tests/fixtures/fake-kiro-cli.sh'
FAKE_SHA = '61586cd278cb555d29f84b49805a1811705112713d0b6774feec42310d0ced6d'
SAFE_ROOT = Path('/home/dwalleck/.claude/tmp/cyril-mcp-consumer-ri0o3aod')
NATIVE_SCREEN = SAFE_ROOT / 'live-active-df5d9437/06-active-manual-url-visible-before-any-local-oauth.txt'
NATIVE_PROJECTION = SAFE_ROOT / 'live-active-df5d9437/live/wire.nonsecret.jsonl'
URL_SHA = '92961ea663eec802f2dc1a32a3893165cf170cfa2869f8588647d1c7e9346c5a'
SESSION = 'sess_tqr_owned_synthetic'
URL_REQUEST_ID = 10010
CONTROL_URL = 'http://127.0.0.1:1/authorize?state=apostrophe\'a\\b&hint=café&quote="exact"&control=\x1b[31m\n\r\t\u202e&tail=TQR_SAFE_TAIL'
CONTROL_DISPLAY = 'http://127.0.0.1:1/authorize?state=apostrophe\'a\\b&hint=café&quote="exact"&control=\\u{1b}[31m\\n\\r\\t\\u{202e}&tail=TQR_SAFE_TAIL'
ERROR = 'MCP server "fixture-failed" failed to connect: Error POSTing to endpoint: {"error":"synthetic_unavailable","error_description":"Deliberately unavailable synthetic MCP endpoint."}'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')


def journal(path, value):
    with path.open('a') as stream:
        stream.write(json.dumps(value, ensure_ascii=False) + '\n')


def native_values():
    lines = NATIVE_SCREEN.read_text().splitlines()
    session = next(line.removeprefix('Session created: ') for line in lines if line.startswith('Session created: '))
    index = lines.index('Sign-in requested. Open in browser:') + 1
    value = ''
    while lines[index].strip():
        value += lines[index].rstrip()
        index += 1
    assert hashlib.sha256(value.encode()).hexdigest() == URL_SHA
    projections = [json.loads(line) for line in NATIVE_PROJECTION.read_text().splitlines()]
    callback = next(frame for frame in projections if frame.get('method') == '_kiro/openExternalUrl')
    assert callback['url_sha256'] == URL_SHA and callback['id'] == 10
    observed_session_hashes = {frame['session_id_sha256'] for frame in projections if 'session_id_sha256' in frame}
    assert hashlib.sha256(session.encode()).hexdigest() in observed_session_hashes
    assert ERROR == lines[5].removeprefix('[error] ') + ' ' + lines[6]
    return SESSION, value


def agent():
    output = Path(os.environ['TQR_OWNED_OUTPUT'])
    session, url = native_values()
    buffer, sequence, session_request = b'', 0, None

    def send(frame):
        journal(output / 'agent-wire.jsonl', frame)
        print(json.dumps(frame, ensure_ascii=False), flush=True)

    def update(value):
        send({'jsonrpc': '2.0', 'method': 'session/update', 'params': {'sessionId': session, 'update': value}})

    while True:
        readable, _, _ = select.select([sys.stdin.fileno()], [], [], 0.02)
        if readable:
            chunk = os.read(sys.stdin.fileno(), 65536)
            if not chunk:
                return
            buffer += chunk
            while b'\n' in buffer:
                line, buffer = buffer.split(b'\n', 1)
                frame = json.loads(line)
                journal(output / 'client-wire.jsonl', frame)
                method, ident = frame.get('method'), frame.get('id')
                assert method not in ['session/prompt', 'session/request_permission', '_kiro/mcp/resetServer'], frame
                if method == 'initialize':
                    send({'jsonrpc': '2.0', 'id': ident, 'result': {'protocolVersion': 1,
                          'agentCapabilities': {'loadSession': True, '_meta': {'kiro': {}}},
                          'agentInfo': {'name': 'tqr-owned-source-shaped-peer-NOT-native-KAS', 'version': '0.66.43'}, 'authMethods': []}})
                elif method == 'session/new':
                    session_request = ident
                    send({'jsonrpc': '2.0', 'id': URL_REQUEST_ID, 'method': '_kiro/openExternalUrl', 'params': {'url': url}})
                elif method and ident is not None:
                    send({'jsonrpc': '2.0', 'id': ident, 'result': {}})
                elif ident == URL_REQUEST_ID:
                    assert frame.get('result') == {'success': True}, frame
                    send({'jsonrpc': '2.0', 'id': session_request, 'result': {'sessionId': session}})
        control = output / 'control.json'
        if not control.exists():
            continue
        command = json.loads(control.read_text())
        if command['sequence'] <= sequence:
            continue
        sequence = command['sequence']
        action = command['action']
        if action == 'error':
            update({'sessionUpdate': 'session_info_update', '_meta': {'kiro': {'kind': 'display_error',
                    'message': ERROR, 'errorType': 'mcp_connection_error'}}})
        elif action == 'control-url':
            send({'jsonrpc': '2.0', 'id': 20, 'method': '_kiro/openExternalUrl', 'params': {'url': CONTROL_URL}})
        elif action == 'fill':
            for index in range(50):
                update({'sessionUpdate': 'agent_message_chunk', 'content': {'type': 'text',
                        'text': f'OWNED_NAVIGATION_ROW_{index:02d} no action or authorization\n'}})
        else:
            raise AssertionError(action)


def main(args):
    import pyte
    assert sha(BINARY) == BINARY_SHA and sha(FAKE) == FAKE_SHA
    session, url = native_values()
    output = Path(args.output)
    output.mkdir(exist_ok=False)
    (output / 'harness.py').write_bytes(Path(__file__).read_bytes())
    save(output / 'reconstructed-inputs.json', {
        'controlled_peer_synthetic_session': session,
        'native_session_sha256_from_projection': 'c003e35c4b149f5c6ad9a3542fda9456e5d5682cdaa6cb6ee3bf66fb69b26ead',
        'native_url_sha256': URL_SHA,
        'controlled_native_url_callback_reconstructed_envelope': {'jsonrpc': '2.0', 'id': URL_REQUEST_ID, 'method': '_kiro/openExternalUrl', 'params': {'url': url}},
        'native_failure_message_observed_on_terminal': ERROR,
        'synthetic_control_url': CONTROL_URL,
        'synthetic_expected_visible_control_url': CONTROL_DISPLAY,
        'provenance': {'screen': str(NATIVE_SCREEN), 'screen_sha256': sha(NATIVE_SCREEN),
                       'projection': str(NATIVE_PROJECTION), 'projection_sha256': sha(NATIVE_PROJECTION)},
        'limit': 'Not raw native ACP frames. Supported envelopes and session/request IDs are synthetic; only URL/error text are retained native observations.'})
    receipt = {'result': 'FAIL', 'binary_sha256': BINARY_SHA, 'binary': str(BINARY),
               'fake_cli_sha256': FAKE_SHA, 'native_url_sha256': URL_SHA,
               'scope': 'Actual WORKSPACEBLOCKED partial Tqr candidate manual notice/durable error under owned ACP peer/PTy; not full qualification, PR readiness, native KAS/host, NK panel or caar acceptance',
               'source_tree': SOURCE_TREE, 'fully_qualified': False, 'pr_ready': False,
               'workspace_blocker': 'cyril-workbench reviewer runtime_drop_retains_evidence_until_core_completion: missing EOF window',
               'observations': []}
    with tempfile.TemporaryDirectory(prefix='cyril-tqr8-owned-private-') as temporary:
        scratch = Path(temporary)
        for name in ['bin', 'home', 'config', 'data', 'cache', 'tmp', 'ws']:
            (scratch / name).mkdir()
        (scratch / 'bin/kiro-cli').symlink_to(FAKE)
        node = Path(shutil.which('node')).resolve(strict=True)
        server = scratch / 'server.js'
        server.write_text("const {spawn}=require('node:child_process');" +
                          f"const c=spawn({json.dumps(sys.executable)},[{json.dumps(str(Path(__file__).resolve()))},'agent'],{{stdio:'inherit'}});" +
                          "c.on('exit',n=>process.exit(n??1));process.on('SIGTERM',()=>c.kill('SIGTERM'));")
        config = scratch / 'home/.config/cyril'
        config.mkdir(parents=True)
        (config / 'config.toml').write_text('[agent]\nengine = "kas"\nkas_hooks = "off"\n[memory]\nenabled = false\n')
        environment = {'TERM': 'xterm-256color', 'COLORTERM': 'truecolor', 'LANG': 'C.UTF-8',
                       'PATH': str(scratch / 'bin'), 'SHELL': '/bin/bash',
                       'HOME': str(scratch / 'home'), 'USERPROFILE': str(scratch / 'home'),
                       'XDG_CONFIG_HOME': str(scratch / 'config'), 'XDG_DATA_HOME': str(scratch / 'data'),
                       'XDG_CACHE_HOME': str(scratch / 'cache'), 'TMPDIR': str(scratch / 'tmp'),
                       'KIRO_HOME': str(scratch / 'home/.kiro'), 'KIRO_AGENT_PATH': str(node),
                       'KIRO_KAS_SERVER_PATH': str(server), 'TQR_OWNED_OUTPUT': str(output)}
        master, slave = pty.openpty()
        columns, rows = 140, 40
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
        command = [str(BINARY), '--cwd', str(scratch / 'ws'), '--agent-engine', 'kas']
        process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave, env=environment,
                                   cwd=scratch / 'ws', start_new_session=True)
        os.close(slave)
        screen = pyte.Screen(columns, rows)
        stream = pyte.Stream(screen)
        decoder = codecs.getincrementaldecoder('utf-8')('replace')
        raw = (output / 'terminal.ansi').open('wb')
        sequence, bytes_seen = 0, 0

        def drain(seconds=0.15):
            nonlocal bytes_seen
            until = time.monotonic() + seconds
            while time.monotonic() < until:
                ready, _, _ = select.select([master], [], [], max(0, until-time.monotonic()))
                if not ready:
                    return
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    return
                if not chunk:
                    return
                raw.write(chunk)
                raw.flush()
                bytes_seen += len(chunk)
                stream.feed(decoder.decode(chunk))

        def visible():
            return '\n'.join(screen.display)

        def wait(predicate, label):
            deadline = time.monotonic() + 20
            while not predicate():
                if process.poll() is not None or time.monotonic() > deadline:
                    raise AssertionError(f'Missing {label}; exit={process.poll()}\n{visible()}')
                drain()

        def wire():
            path = output / 'client-wire.jsonl'
            return [json.loads(line) for line in path.read_text().splitlines() if line] if path.exists() else []

        def content_cell_rows():
            # Actual fixed content rectangle, not blanking or reinserting a
            # character under chrome. Full 140-column screens remain captured.
            return [''.join(screen.buffer[y][x].data for x in range(columns - 1)).rstrip()
                    for y in range(rows)]

        def complete(value):
            lines = content_cell_rows()
            for index, line in enumerate(lines):
                if line.startswith(value[:20]):
                    result = ''
                    for row in lines[index:]:
                        if not row.strip():
                            break
                        result += row
                        if result == value:
                            return True
                        if not value.startswith(result):
                            break
            return False

        def capture(name, **facts):
            (output / (name + '.txt')).write_text(visible() + '\n')
            save(output / (name + '-content-cells.json'), {
                'screen_columns': columns, 'content_columns': columns - 1,
                'gutter_column': columns - 1, 'rows': content_cell_rows(),
                'padding_masking_or_character_reinsertion': False})
            receipt['observations'].append({'capture': name + '.txt', 'columns': columns, 'rows': rows,
                                            'raw_byte_offset': bytes_seen, **facts})

        def action(value):
            nonlocal sequence
            sequence += 1
            save(output / 'control.tmp', {'sequence': sequence, 'action': value})
            (output / 'control.tmp').replace(output / 'control.json')

        try:
            wait(lambda: complete(url) and any(frame.get('id') == URL_REQUEST_ID and frame.get('result') == {'success': True} for frame in wire()),
                 'full native URL and delivery ACK')
            capture('01-native-url-manual-ack-not-auth', full_native_url_exact=True, callback_delivery_ack=True,
                    no_connected_or_authorized_claim='connected' not in visible().lower() and 'authorized' not in visible().lower())
            assert 'Sign-in requested. Open in browser:' in visible()
            assert 'connected' not in visible().lower() and 'authorized' not in visible().lower()
            action('error')
            wait(lambda: 'Deliberately unavailable synthetic MCP endpoint.' in visible(), 'durable native-shaped failure')
            capture('02-durable-native-shaped-error', message_preserved=True)
            action('control-url')
            wait(lambda: complete(CONTROL_DISPLAY), 'printable URL exact and controls escaped visibly')
            assert '\x1b' not in visible() and '\u202e' not in visible()
            assert 'apostrophe\'a\\b' in visible() and 'café' in visible() and 'quote="exact"' in visible()
            wait(lambda: any(frame.get('id') == 20 and frame.get('result') == {'success': True} for frame in wire()), 'synthetic delivery ACK')
            capture('03-escaped-controls-printable-exact', full_synthetic_url_exact=True, apostrophe_quote_backslash_unicode_exact=True,
                    terminal_controls_and_bidi_visible_escapes=True)
            os.write(master, b'\x1b[200~DRAFT_NOT_SUBMITTED\x1b[201~')
            wait(lambda: 'DRAFT_NOT_SUBMITTED' in visible(), 'unsubmitted chat draft')
            action('fill')
            wait(lambda: 'OWNED_NAVIGATION_ROW_49' in visible(), 'unrelated subsequent content')
            capture('04-later-content-before-navigation', draft_preserved=True)
            for index in range(6):
                os.write(master, b'\x1b[5~')
                drain(0.2)
                if complete(url) and 'Deliberately unavailable synthetic MCP endpoint.' in visible():
                    break
            assert complete(url), 'full original URL not reachable by main-conversation PageUp'
            assert 'Deliberately unavailable synthetic MCP endpoint.' in visible(), 'error lost after unrelated notifications'
            assert complete(CONTROL_DISPLAY), 'escaped URL lost after unrelated notifications'
            assert 'DRAFT_NOT_SUBMITTED' in visible()
            gutter = [screen.buffer[y][columns - 1].data for y in range(1, rows - 6)]
            assert all(cell in {' ', '▲', '▼', '█', '║'} for cell in gutter), gutter
            assert any(cell != ' ' for cell in gutter), 'browse must exercise scrollbar chrome'
            capture('05-pageup-durable-full-urls-and-error', native_url_full_exact=True, synthetic_url_full_exact=True,
                    durable_error_after_other_notifications=True, draft_preserved=True)
            os.write(master, b'\x1b[6~' * 6)
            wait(lambda: 'OWNED_NAVIGATION_ROW_49' in visible(), 'return to recent conversation with PageDown')
            capture('06-pagedown-return-to-latest', draft_preserved='DRAFT_NOT_SUBMITTED' in visible())
            client = wire()
            assert not any(frame.get('method') in ['session/prompt', '_kiro/mcp/resetServer', 'session/request_permission'] for frame in client)
            initialize = next(frame for frame in client if frame.get('method') == 'initialize')
            kiro = initialize['params']['clientCapabilities']['_meta']['kiro']
            assert kiro.get('openExternalUrl') is True and not kiro.get('secretStorage', False)
            receipt.update(result='PASS', callback_during_new_wait=True, main_conversation_navigation='PageUp then PageDown',
                           full_native_url_exact=True, manual_display_ack_is_not_authorization=True,
                           durable_error_after_unrelated_notifications=True, no_prompt_reset_permission=True,
                           secret_storage_absent=True, native_frame_fidelity='synthetic envelopes/session/request IDs, retained native URL/error text; no raw-frame replay claim')
        except BaseException as error:
            receipt['error'] = repr(error)
            capture('FAILED-final-screen')
            raise
        finally:
            if process.poll() is None:
                os.write(master, b'\x11')
                drain(0.3)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=5)
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            raw.close()
            os.close(master)
            receipt['consumer_returncode'] = process.returncode
            receipt['owned_root_removed_on_exit'] = True
            receipt['no_private_logs_read_or_packaged'] = True
            receipt['launch'] = {'command': command, 'environment': environment,
                                 'boundary': 'fresh isolated roots, PATH contains only immutable fake kiro-cli; explicit Node->owned Python peer, no actual native KAS'}
            save(output / 'result.json', receipt)
    print(json.dumps({key: value for key, value in receipt.items() if key not in ['observations', 'launch']}, indent=2))


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == 'agent':
        agent()
    else:
        parser = argparse.ArgumentParser()
        parser.add_argument('--output', required=True)
        main(parser.parse_args())

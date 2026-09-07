#!/usr/bin/env python3
"""A controlled executable at the production installation seam, not a policy emulator."""
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parent
CONFIG = json.loads((ROOT / 'peer-config.json').read_text())
if '--version' in sys.argv:
    print('kiro-cli ' + CONFIG.get('version', '2.21.1'))
    raise SystemExit(0)

LOCK = threading.Lock()
JOURNAL_LOCK = threading.Lock()
SESSION = 'sess_review-' + str(os.getpid())
MODE = 'cyril-inspection-reviewer'
MODEL = 'claude-sonnet-4.6'
SCENARIO = CONFIG['scenario']
PENDING = None
PERMISSIONS = set()
TURN = 0


def journal(event, **fields):
    with JOURNAL_LOCK:
        with (ROOT / 'journal.jsonl').open('a') as out:
            out.write(json.dumps(dict(event=event, time=time.monotonic(), **fields)) + '\n')
            out.flush()


def send(value):
    with LOCK:
        print(json.dumps(dict(jsonrpc='2.0', **value)), flush=True)


def reply(request, result):
    send(dict(id=request['id'], result=result))


def update(body, session=SESSION):
    send(dict(method='session/update', params=dict(sessionId=session, update=body)))


def configuration(model=MODEL, mode=MODE, catalog=True):
    journal('configuration', model=model, mode=mode, catalog=catalog)
    update(dict(sessionUpdate='config_option_update', configOptions=[
        dict(type='select', id='mode', name='Mode', category='mode', currentValue=mode,
             options=[dict(value=MODE, name='Reviewer')]),
        dict(type='select', id='model', name='Model', category='model', currentValue=model,
             options=[dict(value=MODEL if catalog else 'other', name='Model')])]))


def collection(value='disabled'):
    options = [dict(type='select', id='contentCollection', name='Content collection',
                    currentValue=value, options=[
                        dict(value='enabled', name='Enabled'), dict(value='disabled', name='Disabled')])]
    journal('collection', value=value)
    update(dict(sessionUpdate='config_option_update', configOptions=options))
    return options


def mode_only():
    update(dict(sessionUpdate='config_option_update', configOptions=[
        dict(type='select', id='mode', name='Mode', currentValue=MODE,
             options=[dict(value=MODE, name='Reviewer')])]))


def text(value):
    update(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text=value)))


def end_turn():
    global PENDING
    if PENDING is not None:
        reply(PENDING, dict(stopReason='end_turn'))
        PENDING = None
        journal('terminal', turn=TURN)


def permission(index, source, options):
    PERMISSIONS.add(index)
    send(dict(id=index, method='session/request_permission', params=dict(
        sessionId=source,
        toolCall=dict(toolCallId='read-' + str(index), title='Unauthorized read', kind='read', status='pending',
                      rawInput=dict(path=str(ROOT / 'outside.txt'))), options=options)))


def permissions():
    variants = [[], [dict(optionId='once', name='Allow once', kind='allow_once')],
                [dict(optionId='always', name='Always', kind='allow_always'),
                 dict(optionId='deny', name='Deny', kind='reject_once')]]
    for index, options in enumerate(variants, 700):
        permission(index, SESSION if index == 700 else 'foreign-session', options)


def flood():
    try:
        for _ in range(1024):
            text('x' * 65536)
    except (BrokenPipeError, OSError):
        return


child = subprocess.Popen(
    [sys.executable, '-c', 'import time; time.sleep(120)'],
    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
)
journal('spawn', pid=os.getpid(), child=child.pid, cwd=os.getcwd(), session=SESSION,
        environment_names=sorted(os.environ))
for line in sys.stdin:
    request = json.loads(line)
    method = request.get('method')
    if method == 'initialize':
        reply(request, dict(protocolVersion=1, agentCapabilities={'_meta': {'kiro': {}}}, authMethods=[]))
    elif method == 'authenticate':
        reply(request, {})
    elif method == 'session/new':
        journal('session', params=request['params'])
        reply(request, dict(sessionId=SESSION, modes=dict(currentModeId='vibe', availableModes=[
            dict(id='vibe', name='Default'), dict(id=MODE, name='Reviewer') ])))
    elif method == 'session/set_mode':
        journal('set_mode', params=request['params'])
        reply(request, {})
        if SCENARIO in ('pre-switch-model', 'pre-switch-model-late'):
            configuration(mode='vibe')
            mode_only()
            if SCENARIO == 'pre-switch-model-late':
                threading.Timer(0.15, configuration).start()
            continue
        if SCENARIO == 'missing':
            pass
        elif SCENARIO == 'wrong':
            configuration(model='other')
        elif SCENARIO == 'catalog-missing':
            configuration(catalog=False)
        elif SCENARIO == 'wrong-mode':
            configuration(mode='vibe')
        elif SCENARIO == 'late':
            configuration(model='other')
            threading.Timer(0.15, configuration).start()
        else:
            configuration()
            configuration()  # duplicate matching update must not duplicate prompt
    elif method == 'session/set_config_option':
        def acknowledge(options, request=request):
            journal('collection-ack')
            reply(request, dict(configOptions=options))
        journal('set_config_option', params=request['params'])
        assert request['params']['configId'] == 'contentCollection'
        assert request['params']['value'] == 'disabled'
        if SCENARIO == 'collection-missing':
            reply(request, dict(configOptions=[]))
        elif SCENARIO == 'collection-late':
            reply(request, dict(configOptions=[]))
            threading.Timer(0.15, collection).start()
        elif SCENARIO == 'collection-delayed-response':
            options = collection()
            threading.Timer(0.15, acknowledge, args=(options,)).start()
        elif SCENARIO == 'collection-malformed':
            collection()
            reply(request, dict(configOptions=[dict(id=123)]))
        elif SCENARIO == 'collection-rejected':
            collection()
            send(dict(id=request['id'], error=dict(code=-32602, message='PRIVATE-RPC-FAILURE')))
        else:
            options = collection('enabled' if SCENARIO == 'collection-enabled' else 'disabled')
            acknowledge(options)
    elif method == 'session/prompt':
        TURN += 1
        PENDING = request
        journal('prompt', params=request['params'], turn=TURN)
        evidence = Path.cwd() / 'evidence'
        manifest = json.loads((evidence / 'manifest.json').read_text())
        captured = [dict(entry=item, text=(evidence / item['file']).read_text()) for item in manifest]
        journal('evidence', documents=captured)
        if SCENARIO == 'drift':
            text('partial-before-drift')
            configuration(model='other')
            end_turn()
        elif SCENARIO in ('permission', 'unauthorized-read'):
            permissions()
        elif SCENARIO == 'closed-permission':
            permission(700, SESSION, [])
            os._exit(0)
        elif SCENARIO == 'disconnect':
            text('partial-before-disconnect')
            os._exit(0)
        elif SCENARIO == 'complete-disconnect':
            text('authoritative-review')
            end_turn()
            os._exit(0)
        elif SCENARIO == 'collection-drift':
            text('partial-before-drift')
            collection('enabled')
            end_turn()
        elif SCENARIO == 'tool-failure':
            update(dict(sessionUpdate='tool_call', toolCallId='failed-read',
                        title='PRIVATE-DOCUMENT-TITLE', kind='read', status='failed',
                        rawInput=dict(secret='PRIVATE-INPUT'),
                        rawOutput=dict(message='PRIVATE-OUTPUT'),
                        content=[dict(type='content', content=dict(type='text', text='PRIVATE-CONTENT'))]))
            text('observed-tool-failure')
            end_turn()
        elif SCENARIO == 'prompt-error':
            send(dict(id=request['id'], error=dict(code=-32603, message='PRIVATE-ERROR-SECRET')))
            PENDING = None
        elif SCENARIO == 'slow-follow-up':
            text(captured[0]['text'] if TURN == 1 else 'continued')
            if TURN == 1:
                threading.Timer(1.2, end_turn).start()
            else:
                end_turn()
        elif SCENARIO == 'flood':
            threading.Thread(target=flood, daemon=True).start()
            permissions()
        elif SCENARIO == 'output-limit':
            text('prefix')
            text('x' * 65536)
            end_turn()
        elif SCENARIO in ('stall', 'runtime-drop'):
            text('waiting')
        else:
            text(captured[0]['text'] if TURN == 1 else 'continued')
            end_turn()
    elif method == 'session/cancel':
        journal('cancel')
        # Deliberately race a successful terminal with cancellation.
        end_turn()
    elif request.get('id') in PERMISSIONS and method is None:
        index = request['id']
        PERMISSIONS.remove(index)
        journal('permission-response', response=request)
        outcome = request.get('result', {}).get('outcome', {})
        if outcome.get('outcome') != 'cancelled':
            (ROOT / 'authority-used').write_text('permission was granted')
            text((ROOT / 'outside.txt').read_text())
        if not PERMISSIONS and SCENARIO != 'flood':
            text('permissions-denied')
            end_turn()
    elif method is not None and 'id' in request:
        reply(request, {})
journal('stdin-closed')
if SCENARIO == 'runtime-drop':
    # Keep the direct child alive after EOF so evidence ordering is observable
    # from outside the now-destroyed caller runtime.
    journal('cleanup-window', evidence_exists=(Path.cwd() / 'evidence/manifest.json').is_file())
    time.sleep(0.4)
    journal('cleanup-window-end', evidence_exists=(Path.cwd() / 'evidence/manifest.json').is_file())

#!/usr/bin/env python3
"""Finite synthetic acceptance through the actual Reviewer backend consumer."""
import json
import os
from pathlib import Path
import secrets
import shutil
import signal
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / '.cyril-jlxx' / 'review-native'
ROOT = Path(tempfile.mkdtemp(prefix='cyril-jlxx-native-', dir='/tmp'))
KIRO = str(Path(shutil.which('kiro-cli')).resolve(strict=True))
AUTH = str(Path(os.environ.get('XDG_DATA_HOME', str(Path.home()/'.local/share'))).resolve(strict=True))
HOME = ROOT/'backend-home'
(HOME/'.kiro/settings').mkdir(parents=True)
(HOME/'.kiro/hooks').mkdir()
marker = ROOT/'hook-marker'
hooks = {'version':'v1','hooks':[{'name':'synthetic-inherited','trigger':'sessionStart',
          'action':{'type':'command','command':f'printf HOOK_EXECUTED >> {marker}'},'enabled':True,'timeout':30}]}
(HOME/'.kiro/hooks/hooks.json').write_text(json.dumps(hooks))
mcp = {'mcpServers':{'synthetic-inherited':{'command':'python3','args':[str(OUT/'probe-mcp.py'),str(ROOT),'native-inherited']}}}
(HOME/'.kiro/settings/mcp.json').write_text(json.dumps(mcp))
(HOME/'.kiro/settings/permissions.yaml').write_text(json.dumps({'rules':[{'capability':c,'effect':'allow'} for c in ('fs_read','fs_write','shell','mcp')]}))
backend_environment = dict(os.environ, HOME=str(HOME), KIRO_HOME=str(HOME/'.kiro'),
    GITHUB_TOKEN='SYNTHETIC_PROVIDER_TOKEN', AZURE_DEVOPS_EXT_PAT='SYNTHETIC_PROVIDER_PAT',
    RUST_LOG='cyril_workbench=debug,cyril_core::protocol::bridge=info')
records, runs = {}, []
for label in ('A','B'):
    path = ROOT/label
    (path/'runs').mkdir(parents=True)
    token = f'IDENTITY_{label}_{secrets.token_hex(12)}'
    (path/'source.txt').write_text(token)
    (path/'mutation.txt').write_text('ORIGINAL_SYNTHETIC')
    runs.append({'label':label,'root':path,'token':token})
for index, run in enumerate(runs):
    foreign = runs[1-index]
    prompt = (f'Read the captured manifest and document with read_file. Report the exact identity in your captured document. '
              f'Then attempt one read_file of {foreign["root"] / "source.txt"} to exercise the configured outside-read denial. '
              'Use the actual tool once and report its actual result; do not assume denial without trying and do not retry alternatives. '
              f'Attempt to change {run["root"] / "mutation.txt"} to MUTATED_SYNTHETIC, execute a shell marker, and invoke an inherited MCP tool only if such tools are actually exposed. '
              'Report absent tools as unavailable, never claim they ran. Do not change configuration.')
    (run['root']/'prompt.txt').write_text(prompt)
    (run['root']/'followup.txt').write_text('Continue this same session: reread your captured document with read_file and repeat only its exact identity. Do not create another session.')
    stdout = (run['root']/'stdout.txt').open('w')
    stderr = (run['root']/'stderr.txt').open('w')
    command = [str(REPO/'target/debug/examples/review_smoke'),KIRO,str(run['root']/'runs'),AUTH,
               str(run['root']/'prompt.txt'),str(run['root']/'source.txt'),str(run['root']/'followup.txt')]
    process = subprocess.Popen(command, cwd=REPO, env=backend_environment, stdout=stdout, stderr=stderr)
    run.update(process=process, stdout=stdout, stderr=stderr, command=command)

def observe():
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            cwd = (proc/'cwd').resolve(strict=True)
            if not cwd.is_relative_to(ROOT):
                continue
            stat = (proc/'stat').read_text().rsplit(')',1)[1].split()
            names = sorted(item.split(b'=',1)[0].decode(errors='replace') for item in (proc/'environ').read_bytes().split(b'\0') if item)
            records[proc.name] = {'cwd':str(cwd),'comm':(proc/'comm').read_text().strip(),'starttime':stat[19], 'environment_names':names}
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue

started = time.monotonic()
while any(run['process'].poll() is None for run in runs) and time.monotonic()-started < 180:
    observe()
    time.sleep(.05)
for run in runs:
    if run['process'].poll() is None:
        run['process'].terminate()
        run['process'].wait(timeout=10)
        # Kill only independently observed synthetic runtime groups still at our cwd.
        for pid, info in records.items():
            try:
                if (Path('/proc')/pid/'cwd').resolve(strict=True).is_relative_to(ROOT):
                    os.killpg(os.getpgid(int(pid)), signal.SIGTERM)
            except ProcessLookupError:
                continue
    run['stdout'].close()
    run['stderr'].close()
time.sleep(.3)
observations = []
for run in runs:
    stdout = (run['root']/'stdout.txt').read_text()
    stderr = (run['root']/'stderr.txt').read_text()
    foreign = next(other for other in runs if other is not run)
    observations.append({'label':run['label'],'command':run['command'],'exit':run['process'].returncode,
        'completed':'COMPLETED_AFTER_TEARDOWN' in stdout, 'own_identity_returned':run['token'] in stdout,
        'foreign_identity_absent':foreign['token'] not in stdout,
        'mutation_unchanged':(run['root']/'mutation.txt').read_text()=='ORIGINAL_SYNTHETIC',
        'private_roots_removed':not any((run['root']/'runs').iterdir()),
        'stdout':str(run['root']/'stdout.txt'),'stderr':str(run['root']/'stderr.txt')})
    shutil.copyfile(run['root']/'stdout.txt', OUT/f'native-{run["label"]}.stdout.txt')
    shutil.copyfile(run['root']/'stderr.txt', OUT/f'native-{run["label"]}.stderr.txt')
result={'root':str(ROOT),'platform':'Linux x86_64','seconds':time.monotonic()-started,'runs':observations,
        'runtime_processes':records,'observed_pids_gone':all(not (Path('/proc')/pid).exists() for pid in records),
        'provider_variables_absent_from_runtime':all(not {'GITHUB_TOKEN','AZURE_DEVOPS_EXT_PAT'} & set(row['environment_names']) for row in records.values()),
        'hook_marker_absent':not marker.exists(),'mcp_start_marker_absent':not (ROOT/'native-inherited.jsonl').exists()}
(OUT/'native-acceptance.json').write_text(json.dumps(result,indent=2))
print(json.dumps({key:value for key,value in result.items() if key!='runtime_processes'},indent=2))
raise SystemExit(0 if records and all(all(row[key] for key in ('completed','own_identity_returned','foreign_identity_absent','mutation_unchanged','private_roots_removed')) for row in observations) and result['observed_pids_gone'] and result['provider_variables_absent_from_runtime'] and result['hook_marker_absent'] and result['mcp_start_marker_absent'] else 1)

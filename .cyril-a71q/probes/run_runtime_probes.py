#!/usr/bin/env python3
import os, pathlib, shutil, sqlite3, subprocess
ROOT = pathlib.Path(__file__).resolve().parents[2]
PROBES = ROOT / '.cyril-a71q' / 'probes'
RUNTIME = PROBES / 'runtime'
OUT = PROBES / 'output' / 'runtime'
HOME = PROBES / '.tmp-fake-home'
DB = HOME / '.local' / 'share' / 'kiro-cli' / 'data.sqlite3'

def store():
    DB.parent.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(DB)
    db.executescript("""
CREATE TABLE auth_kv (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT);
INSERT INTO auth_kv VALUES ('kirocli:odic:device-registration', '{"unrelated":true}');
INSERT INTO auth_kv VALUES ('kirocli:odic:token',
 '{"access_token":"AT-probe","expires_at":"2099-01-01T00:00:00Z","refresh_token":"RT-never-read"}');
INSERT INTO state VALUES ('aaa.first.row', '{"arn":"arn:aws:wrong"}');
INSERT INTO state VALUES ('api.codewhisperer.profile',
 '{"arn":"arn:aws:codewhisperer:us-east-1:1:profile/PROBE","profile_name":"p"}');
""")
    db.close()

def run(command, env=None):
    return subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=90)

def cleanup(path):
    try:
        shutil.rmtree(path, ignore_errors=True)
    except OSError as error:
        raise SystemExit(f'cleanup failed for {path}: {error}') from error

OUT.mkdir(parents=True, exist_ok=True)
cleanup(HOME)
build = run(['cargo', 'build', '--manifest-path', str(RUNTIME / 'Cargo.toml')])
(OUT / 'build-stdout.txt').write_text(build.stdout)
(OUT / 'build-stderr.txt').write_text(build.stderr)
if build.returncode: raise SystemExit(build.returncode)
exe = RUNTIME / 'target' / 'debug' / ('ownership-runtime-probe.exe' if os.name == 'nt' else 'ownership-runtime-probe')
for scenario in ('same', 'cross', 'response_only'):
    store()
    trace = OUT / f'{scenario}-mock-trace.txt'
    trace.unlink(missing_ok=True)
    env = os.environ.copy()
    env.update({'HOME': str(HOME), 'USERPROFILE': str(HOME),
      'KIRO_KAS_SERVER_PATH': str(PROBES / 'mock_kas_server.js'),
      'PROBE_SCENARIO': scenario, 'MOCK_TRACE': str(trace)})
    result = run([str(exe), scenario], env)
    (OUT / f'{scenario}-stdout.txt').write_text(result.stdout)
    (OUT / f'{scenario}-stderr.txt').write_text(result.stderr)
    print(f'{scenario}: exit={result.returncode} db_exists_after_run={DB.exists()}')
    DB.unlink(missing_ok=True)
    if result.returncode: raise SystemExit(result.returncode)
cleanup(HOME)
cleanup(RUNTIME / 'target')
print(f'fake_home_exists={HOME.exists()} target_exists={(RUNTIME / "target").exists()}')

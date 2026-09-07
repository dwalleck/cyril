#!/usr/bin/env python3
"""Native Windows constructor controls; no Kiro process or credential access."""
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'crates/cyril-workbench/src/reviewer/runtime.rs'
PREFIX = 'reviewer::runtime::tests::'
CONTROLS = [
    ('compatible-canonicalization',
     'constructor_accepts_windows_paths_with_native_auth_parent',
     'dunce::canonicalize', 'std::fs::canonicalize'),
    ('native-auth-parent',
     'constructor_rejects_windows_auth_parent_outside_known_folder',
     'if config.auth_data_home != dunce::canonicalize(native_auth_parent)? {',
     'if config.auth_data_home != dunce::canonicalize(native_auth_parent)? && false {'),
]


def run(test, *, compile_only=False):
    command = ['cargo', 'test', '-p', 'cyril-workbench', '--lib', PREFIX + test]
    command += ['--no-run'] if compile_only else ['--', '--exact']
    return subprocess.run(command, cwd=ROOT, timeout=600, check=False).returncode


def main():
    if os.name != 'nt':
        raise SystemExit('Windows constructor controls require native Windows')
    original = SOURCE.read_text(encoding='utf-8')
    try:
        for name, test, old, new in CONTROLS:
            SOURCE.write_text(original, encoding='utf-8')
            if run(test) != 0:
                raise SystemExit(f'{name}: fixed baseline failed')
            if old not in original:
                raise SystemExit(f'{name}: mutation anchor missing')
            SOURCE.write_text(original.replace(old, new), encoding='utf-8')
            # A compilation failure is not a successful behavioral control.
            if run(test, compile_only=True) != 0:
                raise SystemExit(f'{name}: mutant did not compile')
            if run(test) == 0:
                raise SystemExit(f'{name}: mutant escaped (or selected zero tests)')
            SOURCE.write_text(original, encoding='utf-8')
            if run(test) != 0:
                raise SystemExit(f'{name}: restored fence failed')
            print(f'{name}: baseline PASS, mutation RED, restored PASS', flush=True)
    finally:
        SOURCE.write_text(original, encoding='utf-8')


if __name__ == '__main__':
    main()

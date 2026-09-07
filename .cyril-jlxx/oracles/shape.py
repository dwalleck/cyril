#!/usr/bin/env python3
"""C7 standalone source/diff oracle; never imported by production or tests."""
import argparse
import difflib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

try:
    from tree_sitter import Language, Parser
    import tree_sitter_rust
except ImportError:
    # Tool-only pinned parser, isolated in uv's cache; no project dependency.
    raise SystemExit(subprocess.call([
        'uv', 'run', '--no-project', '--with', 'tree-sitter==0.25.2',
        '--with', 'tree-sitter-rust==0.24.0', 'python', __file__, *sys.argv[1:]
    ]))

ROOT = Path(__file__).resolve().parents[2]
PARSER = Parser(Language(tree_sitter_rust.language()))
REQUIRED = [
    'crates/cyril-workbench/src/lib.rs',
    'crates/cyril-workbench/src/reviewer.rs',
    'crates/cyril-workbench/src/reviewer/evidence.rs',
    'crates/cyril-workbench/src/reviewer/runtime.rs',
    'crates/cyril-workbench/src/reviewer/types.rs',
    'crates/cyril-core/src/types/spawn_environment.rs',
]
CORE = 'crates/cyril-core/src/'
FORBIDDEN_CORE = re.compile(r'\b(?:ReviewInput|ReviewRun|Reviewer|reviewer_profile|stage_review_evidence)\b')
FORBIDDEN_WORKBENCH = re.compile(r'\b(?:agent_client_protocol|agent_client_protocol_schema|agent_client_protocol_conductor)\b|cyril_core::protocol::(?:transport|domain_mediator|sdk_runtime|engine|kas)\b')
# Only listed bodies may change or be introduced in protected parents.
PROTECTED = {
    CORE+'types/mod.rs': set(),
    CORE+'protocol/bridge.rs': {'SpawnConfig::default', 'engine_for', 'resolve_spawn_command', 'run_bridge', 'BridgeHandle::sender', 'BridgeHandle::for_tests_with_command_rx', 'BridgeHandle::split', 'BridgeSender::from_sender', 'create_channel_pair'},
    CORE+'protocol/engine.rs': {'KasEngine::new', 'KasEngine::default', 'KasEngine::settings_extra'},
    'crates/cyril/src/main.rs': {'main'},
    CORE+'protocol/domain_mediator/mod.rs': {'DomainMediator::run'},
    'crates/cyril-workbench/src/lib.rs': set(),
    CORE+'types/event.rs': set(),
    CORE+'protocol/domain_mediator/commands/mod.rs': {'DomainMediator::handle_command'},
    CORE+'protocol/domain_mediator/commands/session.rs': {'DomainMediator::set_config_option'},
    CORE+'session.rs': {'SessionController::apply_notification'},
    'crates/cyril-ui/src/state.rs': {'UiState::apply_notification'},
}
# Growth tripwires from plan.md. Existing unchanged modules are also diff-guarded.
MAX_PREFIX = {
    CORE+'types/mod.rs':92, CORE+'types/spawn_environment.rs':150,
    CORE+'protocol/kas/version.rs':180,
    CORE+'protocol/bridge.rs':440, CORE+'protocol/transport.rs':345,
    CORE+'protocol/engine.rs':370, CORE+'protocol/kas/settings.rs':230,
    CORE+'protocol/kas/discovery.rs':345,
    'crates/cyril/src/main.rs':295,
    'crates/cyril-workbench/src/lib.rs':12,
    'crates/cyril-workbench/src/reviewer.rs':540,
    'crates/cyril-workbench/src/reviewer/evidence.rs':220,
    'crates/cyril-workbench/src/reviewer/runtime.rs':270,
    'crates/cyril-workbench/src/reviewer/types.rs':270,
    CORE+'types/event.rs':680,
    CORE+'protocol/domain_mediator/commands/mod.rs':190,
    CORE+'protocol/domain_mediator/commands/session.rs':550,
    CORE+'session.rs':380,
    'crates/cyril-ui/src/state.rs':2490,
}
# An early test-only enum variant makes a prefix cap unsuitable for this owner.
MAX_TOTAL = {CORE+'protocol/domain_mediator/mod.rs':735}
ALLOWED_PRODUCTION = set(MAX_PREFIX) | set(MAX_TOTAL)


def git(root, *args):
    return subprocess.run(['git', *args], cwd=root, text=True, capture_output=True, check=True).stdout.strip()


def walk(node):
    skip = False
    for child in node.children:
        if child.type == 'attribute_item' and re.search(rb'cfg\((?:all\()?test\b', child.text):
            skip = True
            continue
        if skip:
            if child.type not in ('line_comment', 'block_comment', 'attribute_item'):
                skip = False
            continue
        yield child
        yield from walk(child)


def tokens(node):
    return tuple((child.type, child.text.decode()) for child in walk(node)
                 if child.child_count == 0 and child.type not in ('line_comment','block_comment'))


def functions(source):
    tree = PARSER.parse(source.encode())
    if tree.root_node.has_error:
        raise ValueError('C7: Rust AST parse failure')
    result = {}
    for node in walk(tree.root_node):
        if node.type != 'function_item':
            continue
        name = node.child_by_field_name('name').text.decode()
        parent = node.parent
        owners = []
        while parent:
            if parent.type in ('impl_item', 'trait_item', 'mod_item'):
                field = parent.child_by_field_name('type' if parent.type == 'impl_item' else 'name')
                if field:
                    owners.append(field.text.decode())
            parent = parent.parent
        key = '::'.join([*reversed(owners), name])
        result[key] = tokens(node)
    return result


def dependency_sets(data):
    for section in ('dependencies', 'build-dependencies', 'dev-dependencies'):
        yield data.get(section, {})
    for target in data.get('target', {}).values():
        yield from dependency_sets(target)


def violations(root, complete=False, compare_git=False):
    failures, census = [], {}
    sources = {}
    for path in sorted((root / 'crates').glob('*/src/**/*.rs')):
        rel = path.relative_to(root).as_posix()
        source = path.read_text()
        sources[rel] = source
        pattern = FORBIDDEN_WORKBENCH if '/cyril-workbench/' in rel else FORBIDDEN_CORE
        if pattern.search(source):
            failures.append(f'C7: forbidden responsibility/dependency: {rel}')
        if rel in MAX_PREFIX:
            prefix = len(source.split('#[cfg(test)]', 1)[0].splitlines())
            census[rel] = {'prefix':prefix, 'maximum':MAX_PREFIX[rel]}
            if prefix > MAX_PREFIX[rel]:
                failures.append(f'C7: growth {rel}: {prefix} > {MAX_PREFIX[rel]}')
        if rel in MAX_TOTAL:
            lines = len(source.splitlines())
            census[rel] = {'lines':lines, 'maximum':MAX_TOTAL[rel]}
            if lines > MAX_TOTAL[rel]:
                failures.append(f'C7: total growth {rel}: {lines} > {MAX_TOTAL[rel]}')
    for path in sorted((root / 'crates').glob('*/Cargo.toml')):
        data = tomllib.loads(path.read_text())
        name = data.get('package', {}).get('name')
        for dependencies in dependency_sets(data):
            for dependency, value in dependencies.items():
                package = value.get('package', dependency) if isinstance(value, dict) else dependency
                if name != 'cyril-workbench' and package == 'cyril-workbench':
                    failures.append(f'C7: unapproved workbench dependency: {path.relative_to(root)}')
                if name == 'cyril-workbench' and package.startswith('agent-client-protocol'):
                    failures.append(f'C7: workbench imports ACP: {package}')
    if complete:
        for rel in REQUIRED:
            if not (root / rel).is_file():
                failures.append(f'C7: missing required owner: {rel}')
    if compare_git:
        upstream = git(root, 'symbolic-ref', 'refs/remotes/origin/HEAD')
        base = git(root, 'merge-base', 'HEAD', upstream)
        changed = git(root, 'diff', '--name-only', base, '--', 'crates').splitlines()
        # Include untracked new source without requiring staging as an oracle side effect.
        changed += git(root, 'ls-files', '--others', '--exclude-standard', '--', 'crates').splitlines()
        for rel in sorted(set(changed)):
            if (not rel.endswith('.rs') or '/src/' not in rel or '/tests/' in rel
                    or rel == CORE+'protocol/probe_dn91.rs'):
                continue
            try:
                before = git(root, 'show', f'{base}:{rel}')
            except subprocess.CalledProcessError as error:
                if 'does not exist' not in error.stderr and 'exists on disk, but not in' not in error.stderr:
                    raise
                before = ''
            after = sources.get(rel, '')
            before_tree, after_tree = PARSER.parse(before.encode()), PARSER.parse(after.encode())
            if tokens(before_tree.root_node) == tokens(after_tree.root_node):
                continue
            if rel not in ALLOWED_PRODUCTION:
                failures.append(f'C7: unapproved production delta: {rel}')
            if rel in PROTECTED:
                old, new = functions(before), functions(after)
                for name in sorted(old.keys() | new.keys()):
                    if old.get(name) != new.get(name) and name not in PROTECTED[rel]:
                        failures.append(f'C7: protected body changed: {rel}::{name}')
        census['upstream'] = upstream
        census['merge_base'] = base
    return failures, census


def mutation_check():
    with tempfile.TemporaryDirectory(prefix='jlxx-shape-mutation-') as directory:
        root = Path(directory)
        path = root / 'crates/cyril-core/src/protocol/transport.rs'
        path.parent.mkdir(parents=True)
        path.write_text('pub fn own_process() {}\n')
        assert not violations(root)[0], 'C7 positive control must pass'
        path.write_text('pub fn start_review(input: ReviewInput) -> ReviewRun { todo!() }\n')
        body = violations(root)[0]
        assert body and all('C7:' in item for item in body), body
        path.write_text('pub fn own_process() {}\n')
        manifest = root / 'crates/cyril-core/Cargo.toml'
        manifest.write_text('[package]\nname="cyril-core"\n[dev-dependencies]\ncyril-workbench="*"\n')
        dependency = violations(root)[0]
        assert dependency and 'unapproved workbench dependency' in dependency[0], dependency
        # Vocabulary-free renamed body is caught by AST ownership, not names.
        old = functions('pub fn main() {}')
        new = functions('pub fn main() {}\nfn renamed_operation() { loop {} }')
        changed = [name for name in new if old.get(name) != new[name] and name not in {'main'}]
        assert changed == ['renamed_operation'], changed
        return {'body':body, 'dev_dependency':dependency,
                'renamed_protected_body':f'C7: protected body changed: main.rs::{changed[0]}'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--complete', action='store_true')
    parser.add_argument('--mutation-check', action='store_true')
    args = parser.parse_args()
    failures, census = violations(args.root, args.complete, compare_git=True)
    result = {'claim':'C7', 'mode':'complete' if args.complete else 'slice',
              'status':'FAIL' if failures else 'PASS', 'violations':failures, 'census':census}
    if args.mutation_check:
        result['mutation_detection'] = mutation_check()
    print(json.dumps(result, indent=2))
    raise SystemExit(bool(failures))

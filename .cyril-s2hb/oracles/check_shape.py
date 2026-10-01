#!/usr/bin/env python3
"""C10: independent source/dependency census for the approved s2hb ledger."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

from rust_source import RustSource, SourceError, token_texts

ROOT = Path(__file__).resolve().parents[2]
REVIEW = "crates/cyril-review/"
LIB_PATH = "crates/cyril-core/src/lib.rs"
BRIDGE_PATH = "crates/cyril-core/src/protocol/bridge.rs"
HOST_SHELL_PATH = "crates/cyril-core/src/protocol/kas/host_shell.rs"

LIMITS = {
    REVIEW + "src/lib.rs": 180,
    REVIEW + "src/run.rs": 300,
    REVIEW + "src/clock.rs": 150,
    REVIEW + "src/git.rs": 290,
    REVIEW + "src/gather.rs": 350,
    REVIEW + "src/facts.rs": 620,
    "crates/cyril-core/src/review/mod.rs": 190,
    "crates/cyril/src/crtool.rs": 130,
}
DIAGNOSTICS = {
    REVIEW + "src/diagnostics/mod.rs": 280,
    REVIEW + "src/diagnostics/process.rs": 400,
    REVIEW + "src/diagnostics/command.rs": 270,
}
PARENTS = {
    "crates/cyril/src/main.rs": (20, 360),
    LIB_PATH: (2, 25),
    BRIDGE_PATH: (30, 490),
    HOST_SHELL_PATH: (15, 530),
}

BRIDGE_FIELD = "review_shell: Option<crate::review::ShellDialect>,"
APPROVED_REVIEW_SHELL_BODY = """use crate::review::ShellDialect;
match self.kind {
    ShellKind::Posix => ShellDialect::Posix,
    ShellKind::Fish => ShellDialect::Fish,
    ShellKind::Pwsh => ShellDialect::Pwsh,
    ShellKind::WindowsPowerShell => ShellDialect::WindowsPowerShell,
}"""
APPROVED_BRIDGE_ASSIGNMENT = (
    r"handle\.review_shell\s*=\s*\{\s*"
    r'#\[cfg\(feature = "kas"\)\]\s*\{\s*'
    r"host_shell\.as_ref\(\)\.map\(\|shell\| shell\.review_shell\(\)\)\s*"
    r"\}\s*"
    r'#\[cfg\(not\(feature = "kas"\)\)\]\s*\{\s*None\s*\}\s*'
    r"\};"
)
# Compare tokens only after finding the live, owned statement. These nested
# cfg arms are the approved KAS/non-KAS split, not conditional wiring.
APPROVED_BRIDGE_ASSIGNMENT_SOURCE = """handle.review_shell = {
    #[cfg(feature = "kas")]
    {
        host_shell.as_ref().map(|shell| shell.review_shell())
    }
    #[cfg(not(feature = "kas"))]
    {
        None
    }
};"""
EXPECTED_REVIEW_EXPORT = token_texts("pub mod review;")
EXPECTED_BRIDGE_FIELD = token_texts(BRIDGE_FIELD)
EXPECTED_BRIDGE_OWNER = token_texts("pub struct BridgeHandle {")
EXPECTED_BRIDGE_GETTER_HEADER = token_texts(
    "pub fn review_shell(&self) -> Option<crate::review::ShellDialect> {"
)
EXPECTED_BRIDGE_GETTER_BODY = token_texts("self.review_shell")
EXPECTED_ASSIGNMENT = token_texts(APPROVED_BRIDGE_ASSIGNMENT_SOURCE)
EXPECTED_HOST_GETTER_HEADER = token_texts(
    "pub(crate) fn review_shell(&self) -> crate::review::ShellDialect {"
)
EXPECTED_HOST_GETTER_BODY = token_texts(APPROVED_REVIEW_SHELL_BODY)


def git(*args):
    result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"git {args}: {result.stderr.decode(errors='replace')}")
    return result.stdout.decode("utf-8")


def production(text):
    return re.split(r"(?m)^#\[cfg\(test\)\]\nmod tests\b", text, maxsplit=1)[0]


def lines(text):
    return len(production(text).splitlines())


def normalized(text):
    return "\n".join(line.strip() for line in text.splitlines() if line.strip())


def balanced_body(text, opening, label):
    depth = 1
    end = opening
    while depth and end < len(text):
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    if depth:
        raise RuntimeError(f"unbalanced {label}")
    return text[opening:end - 1]


def remove_function(text, name):
    """Remove only the approved simple projection/getter, with balanced braces."""
    pattern = rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?fn {name}\b[^{{]*\{{"
    match = re.search(pattern, text)
    if not match:
        return text, None
    body = balanced_body(text, match.end(), f"approved function {name}")
    return text[:match.start()] + text[match.end() + len(body) + 1:], body


def _live_source(path, text, errors):
    try:
        return RustSource(text)
    except SourceError as error:
        errors.append(f"{path}: cannot inspect live Rust items: {error}")
        return None


def current_wiring_errors():
    """Check explicit live owners, not comments, literals or nested items.

    Read whole files: lexical scopes exclude test modules without mistaking a
    test-marker-looking literal for a production boundary. Required owners
    carrying cfg/cfg_attr are rejected; this does not evaluate cfg or macros.
    """
    errors = []

    lib_path = ROOT / LIB_PATH
    if not lib_path.is_file():
        errors.append(f"{LIB_PATH}: required owner missing")
    else:
        lib = _live_source(LIB_PATH, lib_path.read_text(encoding="utf-8"), errors)
        if lib is not None:
            exports = lib.top_level_mods("review")
            if not (
                len(exports) == 1
                and not exports[0].conditional
                and not lib.has_attribute_before(exports[0].keyword, {"path"})
                and lib.item_header(exports[0]) == EXPECTED_REVIEW_EXPORT
            ):
                errors.append(f"{LIB_PATH}: expected exactly one unconditional default-path `pub mod review;` export")

    bridge_path = ROOT / BRIDGE_PATH
    if not bridge_path.is_file():
        errors.append(f"{BRIDGE_PATH}: required owner missing")
    else:
        bridge = _live_source(BRIDGE_PATH, bridge_path.read_text(encoding="utf-8"), errors)
        if bridge is not None:
            owners = bridge.top_level_structs("BridgeHandle")
            if len(owners) != 1:
                errors.append(f"{BRIDGE_PATH}: BridgeHandle owner missing or duplicated")
            else:
                owner = owners[0]
                if bridge.item_header(owner) != EXPECTED_BRIDGE_OWNER:
                    errors.append(f"{BRIDGE_PATH}: expected the exact nongeneric public BridgeHandle owner")
                fields = bridge.direct_fields(owner, "review_shell")
                valid_fields = [
                    field for field in fields
                    if not field.conditional
                    and not owner.conditional
                    and not field.visible
                    and bridge.values(field.name, field.end) == EXPECTED_BRIDGE_FIELD
                ]
                if len(valid_fields) != 1 or len(fields) != 1:
                    errors.append(
                        f"{BRIDGE_PATH}: BridgeHandle must contain one private typed `{BRIDGE_FIELD}` field"
                    )
                if owner.conditional or any(field.conditional for field in fields):
                    errors.append(f"{BRIDGE_PATH}: BridgeHandle review_shell field must be unconditional")

            getters = [
                (impl, method)
                for impl in bridge.top_level_impls("BridgeHandle")
                for method in bridge.direct_methods(impl, "review_shell")
            ]
            valid_getters = [
                (impl, method) for impl, method in getters
                if not impl.conditional
                and not method.conditional
                and bridge.item_header(method) == EXPECTED_BRIDGE_GETTER_HEADER
                and bridge.item_body(method) == EXPECTED_BRIDGE_GETTER_BODY
            ]
            if len(valid_getters) != 1 or len(getters) != 1:
                errors.append(f"{BRIDGE_PATH}: expected one exact unconditional `review_shell` getter signature")
            if any(impl.conditional or method.conditional for impl, method in getters):
                errors.append(f"{BRIDGE_PATH}: review_shell getter must be an unconditional BridgeHandle method")

            functions = bridge.top_level_functions("spawn_bridge")
            assignments = [
                (function, statement)
                for function in functions
                for statement in bridge.direct_assignments(function, "handle", "review_shell")
            ]
            valid_assignments = [
                (function, statement) for function, statement in assignments
                if not function.conditional
                and not statement.conditional
                and statement.end is not None
                and bridge.values(statement.start, statement.end) == EXPECTED_ASSIGNMENT
            ]
            if len(valid_assignments) != 1 or len(assignments) != 1 or len(functions) != 1:
                errors.append(f"{BRIDGE_PATH}: expected one resolved-shell assignment to handle.review_shell")
            if any(function.conditional or statement.conditional for function, statement in assignments):
                errors.append(f"{BRIDGE_PATH}: resolved-shell assignment must be unconditional spawn_bridge code")
            if len(assignments) == 1:
                function, statement = assignments[0]
                if (
                    not function.conditional
                    and not statement.conditional
                    and statement.end is not None
                    and bridge.values(statement.start, statement.end) != EXPECTED_ASSIGNMENT
                ):
                    errors.append(
                        f"{BRIDGE_PATH}: resolved-shell assignment is not the approved single KAS/non-KAS projection"
                    )

    host_path = ROOT / HOST_SHELL_PATH
    if not host_path.is_file():
        errors.append(f"{HOST_SHELL_PATH}: required owner missing")
    else:
        host = _live_source(HOST_SHELL_PATH, host_path.read_text(encoding="utf-8"), errors)
        if host is not None:
            getters = [
                (impl, method)
                for impl in host.top_level_impls("HostShell")
                for method in host.direct_methods(impl, "review_shell")
            ]
            valid_getters = [
                (impl, method) for impl, method in getters
                if not impl.conditional
                and not method.conditional
                and host.item_header(method) == EXPECTED_HOST_GETTER_HEADER
                and host.item_body(method) == EXPECTED_HOST_GETTER_BODY
            ]
            if len(valid_getters) != 1 or len(getters) != 1:
                errors.append(
                    f"{HOST_SHELL_PATH}: expected one exact unconditional exhaustive review_shell projection"
                )
            if any(impl.conditional or method.conditional for impl, method in getters):
                errors.append(f"{HOST_SHELL_PATH}: HostShell review_shell projection must be unconditional")

    return errors


def parent_without_wiring(path, text):
    text = production(text)
    if path.endswith("/main.rs"):
        text = text.replace("mod crtool;\n", "")
        text = re.sub(r"\s*#\[command\(subcommand\)\]\s*command: Option<crtool::Command>,", "", text)
        text = re.sub(r"\s*if let Some\(command\) = cli.command \{\s*std::process::exit\(command.run\(cli\.cwd\)\);\s*\}", "", text)
    elif path == LIB_PATH:
        text = text.replace("pub mod review;\n", "")
    elif path.endswith("/bridge.rs"):
        text, body = remove_function(text, "review_shell")
        if body is not None and normalized(body) != "self.review_shell":
            raise RuntimeError(f"{path}: review_shell owns more than a projection")
        text = re.sub(r"(?m)^\s*/// Dialect of the same host shell resolved for this bridge's KAS session\.\n", "", text)
        text = re.sub(
            rf"(?m)^\s*(?:{re.escape(BRIDGE_FIELD)}|review_shell: None,)\n",
            "",
            text,
        )
        text = re.sub(APPROVED_BRIDGE_ASSIGNMENT, "", text)
        text = text.replace("let (mut handle, mut channels)", "let (handle, mut channels)")
    else:
        text, body = remove_function(text, "review_shell")
        if body is not None and normalized(body) != normalized(APPROVED_REVIEW_SHELL_BODY):
            raise RuntimeError(f"{path}: review_shell is not the approved exhaustive projection")
    return normalized(text)


def dependency_names(table):
    result = set()
    for key in ("dependencies", "build-dependencies"):
        for name, value in table.get(key, {}).items():
            result.add(value.get("package", name) if isinstance(value, dict) else name)
    for target in table.get("target", {}).values():
        result.update(dependency_names(target))
    return result



def diagnostics_owner_errors():
    """Narrow B placement/private-owner tripwires, not behavioral proof."""
    errors = []
    modules = [(REVIEW + "src/lib.rs", "diagnostics"),
               (REVIEW + "src/diagnostics/mod.rs", "command"),
               (REVIEW + "src/diagnostics/mod.rs", "process")]
    for path, name in modules:
        current = ROOT / path
        if not current.is_file():
            errors.append(f"{path}: required B owner missing")
            continue
        source = _live_source(path, current.read_text(encoding="utf-8"), errors)
        if source is None:
            continue
        declarations = source.top_level_mods(name)
        if not (len(declarations) == 1 and not declarations[0].conditional
                and not source.has_attribute_before(declarations[0].keyword, {"path"})
                and source.item_header(declarations[0]) == token_texts(f"mod {name};")):
            errors.append(f"{path}: `{name}` must have one private unconditional default-path owner")
    for path in (REVIEW + "src/diagnostics/command.rs", REVIEW + "src/diagnostics/process.rs"):
        current = ROOT / path
        if not current.is_file():
            continue
        source = _live_source(path, current.read_text(encoding="utf-8"), errors)
        if source is None:
            continue
        for index, token in enumerate(source.tokens):
            if source.depth_before[index] != 0 or token.text != "pub":
                continue
            # pub(crate)/pub(super) are private implementation cooperation.
            if index + 1 < len(source.tokens) and source.tokens[index + 1].text != "(":
                errors.append(f"{path}: unrestricted public helper/item outside the operation facade")
    facade = ROOT / REVIEW / "src/lib.rs"
    if facade.is_file():
        source = _live_source(REVIEW + "src/lib.rs", facade.read_text(encoding="utf-8"), errors)
        if source is not None:
            for index, token in enumerate(source.tokens):
                if source.depth_before[index] == 0 and token.text in {"Manifest", "ManifestFile"}:
                    errors.append(f"{REVIEW}src/lib.rs: private manifest representation exposed by facade")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("prefix", "gather", "diagnostics"))
    parser.add_argument("--integration-only", action="store_true")
    bases = parser.add_mutually_exclusive_group()
    bases.add_argument("--base", help="Review branch; compare from its merge base with HEAD")
    bases.add_argument("--exact-base", help="Exact before revision for a push endpoint comparison")
    args = parser.parse_args()
    if args.integration_only:
        if args.phase is not None:
            parser.error("--integration-only cannot be combined with --phase")
        errors = current_wiring_errors()
        if errors:
            for error in errors:
                print(f"FAIL C10: {error}", file=sys.stderr)
            return 1
        print("PASS C10: integration wiring only; core export, bridge projection, assignment, and HostShell mapping")
        return 0
    if args.phase is None:
        parser.error("--phase is required unless --integration-only is used")

    if args.exact_base is not None:
        base = git("rev-parse", "--verify", f"{args.exact_base}^{{commit}}").strip()
    else:
        base = args.base
        if not base:
            branch = os.environ.get("GITHUB_BASE_REF")
            base = f"refs/remotes/origin/{branch}" if branch else git("symbolic-ref", "refs/remotes/origin/HEAD").strip()
        base = git("merge-base", "HEAD", base).strip()
    errors = current_wiring_errors()
    limits = dict(LIMITS)
    if args.phase == "prefix":
        limits = {path: limit for path, limit in limits.items()
                  if path == "crates/cyril-core/src/review/mod.rs"}
    if args.phase == "diagnostics":
        limits.update(DIAGNOSTICS)
        errors.extend(diagnostics_owner_errors())
    expected_sources = {p for p in limits if p.startswith(REVIEW)}
    actual_sources = {p.relative_to(ROOT).as_posix() for p in (ROOT / REVIEW / "src").rglob("*.rs")}
    for missing in sorted(set(limits) - {p for p in limits if (ROOT / p).is_file()}):
        errors.append(f"{missing}: required owner missing")
    for extra in sorted(actual_sources - expected_sources):
        errors.append(f"{extra}: unapproved production module; return to design")
    for source_root in ("crates/cyril/src", "crates/cyril-core/src"):
        baseline_sources = set(git("ls-tree", "-r", "--name-only", base, "--", source_root).splitlines())
        for source in (ROOT / source_root).rglob("*.rs"):
            path = source.relative_to(ROOT).as_posix()
            if path not in baseline_sources and path not in limits:
                errors.append(f"{path}: unapproved new production owner; return to design")
    for path, limit in limits.items():
        if not (ROOT / path).is_file():
            continue
        text = (ROOT / path).read_text(encoding="utf-8")
        count = lines(text)
        if count > limit:
            errors.append(f"{path}: {count} production-region lines > {limit}; Length review required")
        print(f"C10 census {path}: {count}/{limit}")
    for path, (delta_limit, limit) in PARENTS.items():
        current_path = ROOT / path
        if not current_path.is_file():
            errors.append(f"{path}: required protected parent missing")
            continue
        current = current_path.read_text(encoding="utf-8")
        before = git("show", f"{base}:{path}")
        if args.phase == "prefix" and path == "crates/cyril/src/main.rs":
            if production(current) != production(before):
                errors.append(f"{path}: prefix increment must not change binary startup")
        delta = lines(current) - lines(before)
        if delta > delta_limit or lines(current) > limit:
            errors.append(f"{path}: production delta +{delta} (max +{delta_limit}), size {lines(current)} (max {limit}); Length review required")
        try:
            if parent_without_wiring(path, current) != parent_without_wiring(path, before):
                errors.append(f"{path}: protected-parent body changed outside approved wiring")
        except RuntimeError as error:
            errors.append(str(error))
    manifest_path = ROOT / REVIEW / "Cargo.toml"
    if manifest_path.is_file():
        if args.phase == "prefix":
            errors.append(f"{manifest_path.relative_to(ROOT)}: leaf belongs to the gather increment")
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        deps = dependency_names(manifest)
        # Exact selected packages; versions/features/linking are reviewed separately.
        expected = {"serde", "serde_json", "regex", "thiserror", "git2", "tokio",
                    "tracing", "interprocess", "subprocess"}
        if deps != expected:
            errors.append(f"{manifest_path.relative_to(ROOT)}: unapproved/missing runtime dependencies {sorted(deps)}")
        if manifest.get("lints") != {"workspace": True}:
            errors.append(f"{manifest_path.relative_to(ROOT)}: workspace lints must be inherited unchanged")
    elif args.phase != "prefix":
        errors.append(f"{manifest_path.relative_to(ROOT)}: required leaf manifest missing")
    lib_path = ROOT / REVIEW / "src/lib.rs"
    if lib_path.is_file():
        lib = production(lib_path.read_text(encoding="utf-8"))
        if re.search(r"(?m)^pub mod ", lib):
            errors.append(f"{lib_path.relative_to(ROOT)}: internal modules exposed instead of operation interface")
    if args.phase in ("gather", "diagnostics"):
        cli = (ROOT / "crates/cyril/src/crtool.rs").read_text(encoding="utf-8")
        if re.search(r"\bDiagnostics\b|cyril_core::|serde_json::|tokio::", production(cli)):
            errors.append("crates/cyril/src/crtool.rs: forbidden diagnostics verb or business/runtime ownership")
        main_source = (ROOT / "crates/cyril/src/main.rs").read_text(encoding="utf-8")
        dispatch = main_source.find("std::process::exit(command.run(cli.cwd));")
        startup = main_source.find("    setup_logging();")
        if dispatch < 0 or startup < 0 or dispatch > startup:
            errors.append("crates/cyril/src/main.rs: hidden dispatch must precede ordinary startup")
    if errors:
        for error in errors:
            print(f"FAIL C10: {error}", file=sys.stderr)
        return 1
    print(f"PASS C10: {args.phase} ledger, dependencies, protected parents; base={base}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, ValueError) as error:
        print(f"FAIL C10: cannot complete source census: {error}", file=sys.stderr)
        sys.exit(1)

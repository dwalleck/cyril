#!/usr/bin/env python3
"""C12/C14 repository-shape oracle for the conductor-first clean cutover."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LEDGER = json.loads((Path(__file__).with_name("module-shape.json")).read_text())


def rel(path: str) -> Path:
    return ROOT / path


def production_lines(path: Path) -> list[str]:
    lines = path.read_text().splitlines()
    if path.as_posix().endswith(("protocol/bridge.rs", "cyril/src/app.rs")):
        for index, line in enumerate(lines):
            if line == "#[cfg(test)]":
                return lines[:index]
    return lines


def cargo(path: str) -> dict:
    return tomllib.loads(rel(path).read_text())

DEPENDENCY_TABLES = {"dependencies", "dev-dependencies", "build-dependencies"}


def dependency_tables(document: dict) -> list[dict]:
    tables: list[dict] = []
    for name, value in document.items():
        if not isinstance(value, dict):
            continue
        if name in DEPENDENCY_TABLES:
            tables.append(value)
        tables.extend(dependency_tables(value))
    return tables


def dependency_names_for_package(document: dict, package: str) -> list[str]:
    return sorted(
        {
            name
            for dependencies in dependency_tables(document)
            for name, specification in dependencies.items()
            if name == package
            or (
                isinstance(specification, dict)
                and specification.get("package") == package
            )
        }
    )


def legacy_acp_aliases(document: dict) -> list[str]:
    aliases = set()
    for dependencies in dependency_tables(document):
        for name, specification in dependencies.items():
            package = (
                specification.get("package")
                if isinstance(specification, dict)
                else None
            )
            if name == "agent-client-protocol-legacy" or (
                package == "agent-client-protocol"
                and name != "agent-client-protocol"
            ):
                aliases.add(name)
    return sorted(aliases)




def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def check(phase: str) -> tuple[list[str], dict[str, object]]:
    failures: list[str] = []
    observations: dict[str, object] = {}

    for path in LEDGER["required_paths"]:
        if not rel(path).is_file():
            failures.append(f"C14 required path missing: {path}")
        elif git("ls-files", "--error-unmatch", "--", path).returncode != 0:
            failures.append(f"C14 required path is not tracked: {path}")

    for path, bounds in LEDGER["line_ranges"].items():
        count = len(production_lines(rel(path)))
        observations[f"lines:{path}"] = count
        low, high = bounds
        if not low <= count <= high:
            failures.append(f"C14 {path} production lines {count} outside {low}..{high}")

    for path, expected in LEDGER["exact_lines"].items():
        actual = len(rel(path).read_text().splitlines())
        observations[f"lines:{path}"] = actual
        if actual != expected:
            failures.append(f"C14 {path} lines {actual} != {expected}")

    for path, expected in LEDGER["unchanged_production_prefixes"].items():
        actual = len(production_lines(rel(path)))
        observations[f"production-lines:{path}"] = actual
        if actual != expected:
            failures.append(f"C14 {path} production lines {actual} != {expected}")
        merge_base = git("merge-base", "HEAD", "main")
        if merge_base.returncode != 0:
            failures.append(f"C14 cannot resolve main merge-base: {merge_base.stderr.strip()}")
        else:
            diff = git("diff", "--quiet", merge_base.stdout.strip(), "--", path)
            if diff.returncode != 0:
                failures.append(f"C14 protected parent changed: {path}")

    root_manifest = cargo("Cargo.toml")
    workspace_dependencies = root_manifest["workspace"]["dependencies"]
    sdk = workspace_dependencies.get("agent-client-protocol")
    conductor = workspace_dependencies.get("agent-client-protocol-conductor")
    observations["sdk-dependency"] = sdk
    observations["conductor-dependency"] = conductor
    if not isinstance(sdk, dict) or sdk.get("version") != "=2.0.0":
        failures.append("C12 agent-client-protocol must be pinned to =2.0.0")
    elif sdk.get("default-features") is not False:
        failures.append("C11 agent-client-protocol must disable default features explicitly")
    elif "unstable_protocol_v2" in sdk.get("features", []):
        failures.append("C11 unstable_protocol_v2 is enabled")
    if not isinstance(conductor, dict) or conductor.get("version") != "=2.0.0":
        failures.append("C12 agent-client-protocol-conductor must be pinned to =2.0.0")
    manifests = {
        "Cargo.toml": root_manifest,
        **{
            manifest.relative_to(ROOT).as_posix(): tomllib.loads(manifest.read_text())
            for manifest in rel("crates").glob("*/Cargo.toml")
        },
    }
    if phase == "final":
        for path, manifest in manifests.items():
            aliases = legacy_acp_aliases(manifest)
            if aliases:
                failures.append(
                    f"C12 legacy ACP dependency alias in {path}: {','.join(aliases)}"
                )
    for path, manifest in manifests.items():
        schema_dependencies = dependency_names_for_package(
            manifest, "agent-client-protocol-schema"
        )
        if schema_dependencies:
            failures.append(
                f"C11 schema dependency must remain transitive in {path}: "
                + ",".join(schema_dependencies)
            )

    core_manifest = manifests["crates/cyril-core/Cargo.toml"]
    core_dependencies = core_manifest.get("dependencies", {})
    if (
        "agent-client-protocol" not in core_dependencies
        or "agent-client-protocol-conductor" not in core_dependencies
    ):
        failures.append("C12 cyril-core must own both SDK2 dependencies")
    for manifest in rel("crates").glob("*/Cargo.toml"):
        if manifest.parent.name == "cyril-core":
            continue
        text = manifest.read_text()
        if "agent-client-protocol" in text:
            failures.append(f"C11 SDK dependency outside core: {manifest.relative_to(ROOT)}")

    lock = tomllib.loads(rel("Cargo.lock").read_text())
    packages = [(package["name"], package["version"]) for package in lock["package"]]
    acp_versions = sorted(version for name, version in packages if name == "agent-client-protocol")
    schema_versions = sorted(
        version for name, version in packages if name == "agent-client-protocol-schema"
    )
    conductor_versions = sorted(
        version for name, version in packages if name == "agent-client-protocol-conductor"
    )
    observations["lock"] = {
        "agent-client-protocol": acp_versions,
        "agent-client-protocol-schema": schema_versions,
        "agent-client-protocol-conductor": conductor_versions,
    }
    if acp_versions != ["2.0.0"]:
        failures.append(f"C12 ACP package family is {acp_versions}, expected ['2.0.0']")
    if schema_versions != ["1.5.0"]:
        failures.append(f"C12 schema package family is {schema_versions}, expected ['1.5.0']")
    if conductor_versions != ["2.0.0"]:
        failures.append(f"C12 conductor package family is {conductor_versions}, expected ['2.0.0']")

    protocol_root = rel("crates/cyril-core/src/protocol")
    rust_sources = {
        path.relative_to(ROOT).as_posix(): path.read_text()
        for path in rel("crates").glob("**/*.rs")
    }
    for path, text in rust_sources.items():
        if not path.startswith("crates/cyril-core/") and re.search(
            r"\bagent_client_protocol(?:_conductor)?\b", text
        ):
            failures.append(f"C11 SDK type/import outside core: {path}")

    protocol_sources = {
        path.relative_to(ROOT).as_posix(): path.read_text()
        for path in protocol_root.glob("**/*.rs")
    }
    repository_forbidden = {
        "ClientSideConnection": "C12 old connection symbol",
        "AgentEndpoint": "C14 custom endpoint symbol",
        "AcpAgent": "C14 forbidden direct process adapter",
        "trait AgentRuntime": "C14 custom runtime trait",
        "agent_client_protocol_legacy": "C12 legacy Rust import",
    }
    for symbol, label in repository_forbidden.items():
        for path, text in rust_sources.items():
            if symbol in text:
                failures.append(f"{label}: {symbol} in {path}")
    for path, text in protocol_sources.items():
        if "mpsc::unbounded_channel" in text:
            failures.append(f"C1 unbounded protocol queue: mpsc::unbounded_channel in {path}")

    sdk_runtime_path = "crates/cyril-core/src/protocol/sdk_runtime/mod.rs"
    sdk_runtime = rel(sdk_runtime_path).read_text()
    bridge = rel("crates/cyril-core/src/protocol/bridge.rs").read_text()
    client = rel("crates/cyril-core/src/protocol/client.rs").read_text()
    mediator = rel("crates/cyril-core/src/protocol/domain_mediator/mod.rs").read_text()
    bridge_production = "\n".join(
        production_lines(rel("crates/cyril-core/src/protocol/bridge.rs"))
    )
    conductor_sites = {
        path: text.count("ConductorImpl::new_agent")
        for path, text in rust_sources.items()
        if text.count("ConductorImpl::new_agent")
    }
    observations["conductor-constructor-sites"] = conductor_sites
    if conductor_sites != {sdk_runtime_path: 1}:
        failures.append(
            "C2 expected exactly one ConductorImpl::new_agent topology "
            f"owned by {sdk_runtime_path}: {conductor_sites}"
        )
    if "ProxiesAndAgent::new(agent).proxies(stages.into_vec())" not in sdk_runtime:
        failures.append("C2 StageChain is not wired through ProxiesAndAgent")
    for forbidden_body in ("BridgeCommand::", "match command", "Client::builder", "ConductorImpl"):
        if forbidden_body in bridge_production:
            failures.append(
                f"C14 protected bridge owns forbidden runtime body: {forbidden_body}"
            )
    if "run_client(" in bridge_production or ".connect_to(" in bridge_production:
        failures.append("C12 direct bridge runtime bypass detected")
    for marker in ("DomainMediator", "SourceObserver", "HostMediator", "Rc<", "RefCell"):
        if marker in client:
            failures.append(f"C14 client owns forbidden domain state: {marker}")
    for marker in (
        "BridgeCommand",
        "Notification",
        "DomainMediator",
        "SourceObserver",
        "HostMediator",
        "HostCallback",
    ):
        if marker in sdk_runtime:
            failures.append(f"C14 SDK runtime owns forbidden domain concern: {marker}")
    for marker in ("ConductorImpl", "Client::builder", "ProcessAdapter"):
        if marker in mediator:
            failures.append(f"C14 domain mediator owns forbidden SDK topology: {marker}")
    start = re.search(
        r"pub\(crate\) async fn start\(\s*process: AgentProcess,\s*domain_channels: DomainChannels,\s*stages: StageChain,\s*\)",
        sdk_runtime,
    )
    if start is None:
        failures.append("C10 SdkRuntime::start is not the exact three-argument interface")
    if "observer" in sdk_runtime.lower() or "inspection" in sdk_runtime.lower():
        failures.append("C10 observer/inspection marker in SDK runtime")
    if "ProtocolVersion::V1" not in mediator:
        failures.append("C11 stable wire v1 initialization is missing")
    host_start = mediator.find("let host_task = host::run")
    initialize = mediator.find("self.initialize(&connection)")
    observations["host-before-initialize"] = [host_start, initialize]
    if min(host_start, initialize) < 0 or not host_start < initialize:
        failures.append("C7 host drain must start before initialize can issue callbacks")

    unknown = client.find("if is_unknown_session_update")
    typed = client.find("message: acp::SessionNotification")
    extension = client.find("DomainWork::ExtensionNotification")
    observations["handler-order"] = [unknown, typed, extension]
    if min(unknown, typed, extension) < 0 or not unknown < typed < extension:
        failures.append("C5 client handler order is not unknown-session-update → typed → extension")

    docs = {
        "AGENTS.md": rel("AGENTS.md").read_text(),
        "docs/ROADMAP.md": rel("docs/ROADMAP.md").read_text(),
    }
    for path, text in docs.items():
        for required in ("agent-client-protocol` 2.0", "ConductorImpl"):
            if required not in text:
                failures.append(f"C12 stale topology documentation in {path}: missing {required}")
        if "default = direct spawn, today's behavior" in text:
            failures.append(f"C12 stale direct-spawn topology statement in {path}")

    return failures, observations


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("runtime", "final"), required=True)
    args = parser.parse_args()
    failures, observations = check(args.phase)
    claims = ["C1", "C2", "C3", "C7", "C10", "C11", "C14"]
    if args.phase == "final":
        claims.insert(6, "C12")
    result = {
        "phase": args.phase,
        "passed": not failures,
        "passed_claims": claims if not failures else [],
        "failures": failures,
        "observations": observations,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())

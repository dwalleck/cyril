#!/usr/bin/env python3
import json
import time
import tomllib
from pathlib import Path

started = time.monotonic()
repo = Path(__file__).resolve().parents[3]
artifact_dir = repo / ".cyril-41bs"
probe_dir = artifact_dir / "probe.sdk2"
adr_path = repo / "docs/adr/0012-conductor-first-acp-sdk-2-runtime.md"
old_adr_path = repo / "docs/adr/0003-defer-proxy-stack-for-host-callbacks.md"

required_probe_files = {
    ".gitignore",
    "Cargo.lock",
    "Cargo.toml",
    "oracles/c14.py",
    "oracles/e1.py",
    "oracles/e10.py",
    "oracles/e2.py",
    "oracles/e3.py",
    "oracles/e4.py",
    "oracles/e5.py",
    "oracles/e6.py",
    "oracles/e6_live_parity.py",
    "oracles/e7.py",
    "oracles/e8.py",
    "oracles/e9.py",
    "src/bin/e1.rs",
    "src/bin/e10.rs",
    "src/bin/e2.rs",
    "src/bin/e3.rs",
    "src/bin/e4.rs",
    "src/bin/e5.rs",
    "src/bin/e6.rs",
    "src/bin/e6_live.rs",
    "src/bin/e8.rs",
    "src/bin/e9.rs",
}
actual_probe_files = {
    path.relative_to(probe_dir).as_posix()
    for path in probe_dir.rglob("*")
    if path.is_file()
}
missing_probe_files = sorted(required_probe_files - actual_probe_files)
stale_probe_files = sorted(actual_probe_files - required_probe_files)

required_artifacts = [
    "route.md",
    "evidence.md",
    "design.md",
    "review-decisions.md",
    "plan.md",
    "checkpoints/C14.json",
    "checkpoints/C14-review-fix.json",
]
missing_artifacts = sorted(
    name for name in required_artifacts if not (artifact_dir / name).is_file()
)

def read_if_present(path: Path) -> str:
    return path.read_text() if path.is_file() else ""

def dependency_specs(manifest: dict) -> list[tuple[str, object]]:
    specs = []
    for key, value in manifest.items():
        if key in {"dependencies", "dev-dependencies", "build-dependencies"}:
            if isinstance(value, dict):
                specs.extend(value.items())
        if isinstance(value, dict):
            specs.extend(dependency_specs(value))
    return specs



adr = read_if_present(adr_path)
old_adr = read_if_present(old_adr_path)
design = read_if_present(artifact_dir / "design.md")
evidence = read_if_present(artifact_dir / "evidence.md")
route = read_if_present(artifact_dir / "route.md")
review_decisions = read_if_present(artifact_dir / "review-decisions.md")
normalized_adr = " ".join(adr.split())

adr_checks = {
    "accepted": "Status: accepted (2026-08-30)" in adr,
    "supersedes_adr_0003": (
        "Supersedes: [ADR-0003]" in adr
        and "0012-conductor-first-acp-sdk-2-runtime.md" in old_adr
        and "Status: superseded (2026-08-30)" in old_adr
    ),
    "supersedes_memory_move_promise": (
        "ADR-0003's promise to move persistent-memory adapters when a proxy "
        "stack is activated is also superseded."
        in normalized_adr
    ),
    "selects_conductor_first": "Option C" in adr and "conductor-first" in adr,
    "retains_process_adapter": "AgentProcess" in adr and "ConnectTo<Client>" in adr,
    "places_private_modules": "sdk_runtime" in adr and "domain_mediator" in adr,
    "keeps_stable_wire_v1": "stable wire v1" in adr,
    "has_no_observer_api": "no observer" in adr.lower(),
    "records_upstream_disposition": (
        "No blocking upstream gap" in normalized_adr
        and "no upstream issue or PR" in normalized_adr
    ),
    "links_follow_on_owners": all(
        issue_id in adr for issue_id in ("cyril-gl5s", "cyril-5g2o", "cyril-1ixa")
    ),
}

required_artifact_files = {
    *required_artifacts,
    *(f"probe.sdk2/{path}" for path in required_probe_files),
}
actual_artifact_files = {
    path.relative_to(artifact_dir).as_posix()
    for path in artifact_dir.rglob("*")
    if path.is_file()
}
missing_artifact_files = sorted(required_artifact_files - actual_artifact_files)
stale_artifact_files = sorted(actual_artifact_files - required_artifact_files)

production_sources = sorted((repo / "crates").glob("*/src/**/*.rs"))
workspace_manifest_path = repo / "Cargo.toml"
member_manifests = sorted((repo / "crates").glob("*/Cargo.toml"))
production_contract_files = [*production_sources, workspace_manifest_path, *member_manifests]
forbidden_production_markers = (
    "agent-client-protocol-conductor",
    "agent_client_protocol_conductor",
    "ConductorImpl",
    "sdk_runtime",
    "domain_mediator",
    "ConnectTo<",
)
unexpected_production_markers = sorted(
    f"{path.relative_to(repo).as_posix()}:{marker}"
    for path in production_contract_files
    for marker in forbidden_production_markers
    if marker in path.read_text()
)
manifest_sdk2_dependencies = []
for manifest_path in [workspace_manifest_path, *member_manifests]:
    manifest = tomllib.loads(manifest_path.read_text())
    for dependency_name, spec in dependency_specs(manifest):
        package_name = (
            spec.get("package", dependency_name)
            if isinstance(spec, dict)
            else dependency_name
        )
        version = spec.get("version") if isinstance(spec, dict) else spec
        if package_name == "agent-client-protocol-conductor" or (
            package_name == "agent-client-protocol"
            and isinstance(version, str)
            and version.startswith("2")
        ):
            manifest_sdk2_dependencies.append(
                f"{manifest_path.relative_to(repo).as_posix()}:{package_name}={version}"
            )
manifest_sdk2_dependencies.sort()
workspace_manifest = workspace_manifest_path.read_text()
pre_migration_manifest_intact = (
    'agent-client-protocol = { version = "0.10"' in workspace_manifest
    and not manifest_sdk2_dependencies
)
census_path_count = len(actual_artifact_files) + len(production_contract_files)

premise_statuses = {}
for line in evidence.splitlines():
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if cells and cells[0] in {f"P{number}" for number in range(1, 11)}:
        premise_statuses.setdefault(cells[0], cells[-1])

all_empirical_premises_pass = all(
    premise_statuses.get(f"P{number}", "").startswith("PASS")
    for number in range(1, 11)
)
c14_design_status = ""
for line in design.splitlines():
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if cells and cells[0] == "C14" and len(cells) >= 9:
        c14_design_status = cells[-1]
        break
design_c14_discharged = (
    c14_design_status.startswith("PASS")
    and "C14 owns the pending ADR/cleanup checkpoint" not in design
    and "Assigned to pending C14" not in design
)

expected_review_ids = {f"F{number}" for number in range(1, 15)}
valid_evidence_states = {"Verified", "Refuted", "Unverified", "Not-applicable"}
valid_decisions = {"Accept", "Modify", "Reject"}
review_rows = []
for line in review_decisions.splitlines():
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if cells and cells[0].startswith("F") and cells[0][1:].isdigit():
        review_rows.append(cells)
review_decisions_complete = (
    len(review_rows) == len(expected_review_ids)
    and {cells[0] for cells in review_rows} == expected_review_ids
    and all(
        len(cells) == 8
        and cells[3] in valid_evidence_states
        and cells[5].split(" (", maxsplit=1)[0] in valid_decisions
        and all(cells[index] for index in (1, 2, 4, 6, 7))
        and (
            not cells[5].startswith(("Accept", "Modify"))
            or not cells[6].startswith("N/A")
        )
        and (not cells[5].startswith("Reject") or cells[6].startswith("N/A"))
        for cells in review_rows
    )
)


facts = {
    "claim_ids": ["C14"],
    "adr_checks": adr_checks,
    "design_approved": (
        "- **Status:** APPROVED." in design
        and "- **Requester words:** “Approve conductor-first design”" in design
    ),
    "premise_statuses": premise_statuses,
    "all_empirical_premises_pass": all_empirical_premises_pass,
    "route_deliverables_present": (
        "## Required artifacts" in route and "**Deliverables:**" in route
    ),
    "c14_design_status": c14_design_status,
    "design_c14_discharged": design_c14_discharged,
    "review_decisions_complete": review_decisions_complete,
    "missing_artifacts": missing_artifacts,
    "missing_probe_files": missing_probe_files,
    "stale_probe_files": stale_probe_files,
    "missing_artifact_files": missing_artifact_files,
    "stale_artifact_files": stale_artifact_files,
    "unexpected_production_markers": unexpected_production_markers,
    "manifest_sdk2_dependencies": manifest_sdk2_dependencies,
    "pre_migration_manifest_intact": pre_migration_manifest_intact,
    "retained_path_count": census_path_count,
}
facts["census_within_500_paths"] = census_path_count <= 500
facts["elapsed_ms"] = round((time.monotonic() - started) * 1000, 3)
facts["within_one_second"] = facts["elapsed_ms"] <= 1000
facts["c14_passed"] = (
    all(adr_checks.values())
    and facts["design_approved"]
    and facts["all_empirical_premises_pass"]
    and facts["route_deliverables_present"]
    and not missing_artifacts
    and not missing_probe_files
    and not stale_probe_files
    and facts["design_c14_discharged"]
    and facts["review_decisions_complete"]
    and not missing_artifact_files
    and not stale_artifact_files
    and not unexpected_production_markers
    and not manifest_sdk2_dependencies
    and pre_migration_manifest_intact
    and facts["census_within_500_paths"]
    and facts["within_one_second"]
)

print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["c14_passed"]:
    raise SystemExit(1)

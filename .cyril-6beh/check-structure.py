#!/usr/bin/env python3
"""Persistent ownership/privacy fence for the cyril-6beh workflow seam."""

import re
import sys
import tempfile
from pathlib import Path


DOMAIN_REL = (
    Path("crates/cyril-core/src/types/workflow.rs"),
    Path("crates/cyril-core/src/workflow.rs"),
)
ADAPTER_REL = Path("crates/cyril-core/src/protocol/convert/kas/workflow.rs")
APP_REL = Path("crates/cyril/src/app.rs")
SESSION_REL = Path("crates/cyril-core/src/session.rs")
UI_REL = Path("crates/cyril-ui/src/state.rs")

FORBIDDEN_DOMAIN = {
    "ACP crate": r"\bagent_client_protocol\b|\bacp::",
    "protocol module": r"\b(?:crate|super)::protocol\b",
    "UI crate/type": r"\bcyril_ui\b|\bUiState\b|\bTuiState\b",
    "rendering crate": r"\bratatui\b|\bcrossterm\b",
    "async/runtime": r"\btokio\b|\basync\s+fn\b|\.await\b",
    "bridge/control": r"\bBridgeSender\b|\bBridgeCommand\b",
}


def source(root: Path, rel: Path) -> str:
    path = root / rel
    if not path.is_file():
        raise RuntimeError(f"missing required source: {rel}")
    return path.read_text(encoding="utf-8")


def public_struct_fields(text: str) -> list[str]:
    failures = []
    starts = list(
        re.finditer(
            r"(?m)^\s*pub(?:\([^)]*\))?\s+struct\s+([A-Za-z_][A-Za-z0-9_]*)[^;{(]*(?P<kind>[{(])",
            text,
        )
    )
    for match in starts:
        name = match.group(1)
        opener = match.group("kind")
        closer = "}" if opener == "{" else ")"
        depth = 1
        cursor = match.end()
        end = cursor
        while end < len(text) and depth:
            if text[end] == opener:
                depth += 1
            elif text[end] == closer:
                depth -= 1
            end += 1
        if depth:
            failures.append(f"unterminated public struct {name}")
            continue
        body = text[cursor : end - 1]
        if re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+[A-Za-z_][A-Za-z0-9_]*\s*:", body):
            failures.append(f"public field on {name}")
        if opener == "(" and re.search(r"(?:^|,)\s*pub(?:\([^)]*\))?\s+", body):
            failures.append(f"public tuple field on {name}")
    return failures


def check(root: Path) -> list[str]:
    failures = []
    try:
        domain = {rel: source(root, rel) for rel in DOMAIN_REL}
        adapter = source(root, ADAPTER_REL)
        app = source(root, APP_REL)
        session = source(root, SESSION_REL)
        ui = source(root, UI_REL)
    except RuntimeError as error:
        return [str(error)]

    for rel, text in domain.items():
        for label, pattern in FORBIDDEN_DOMAIN.items():
            if re.search(pattern, text):
                failures.append(f"{rel}: forbidden {label} dependency")
        failures.extend(f"{rel}: {failure}" for failure in public_struct_fields(text))

    all_domain = "\n".join(domain.values())
    if len(re.findall(r"\bstruct\s+WorkflowTracker\b", all_domain)) != 1:
        failures.append("WorkflowTracker must be declared exactly once in core workflow modules")
    if re.search(r"\bstruct\s+WorkflowTracker\b", domain[DOMAIN_REL[0]]):
        failures.append("WorkflowTracker state is owned by types/workflow.rs")

    if re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+(?:struct|enum|union|type)\b", adapter):
        failures.append("adapter wire data type is externally visible")
    if re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+use\b", adapter):
        failures.append("adapter re-exports a private wire item")

    tracker_fields = re.findall(
        r"(?m)^\s*(pub(?:\([^)]*\))?\s+)?workflow_tracker\s*:\s*(?:[A-Za-z0-9_:]+::)?WorkflowTracker\b",
        app,
    )
    if len(tracker_fields) != 1:
        failures.append("App must own exactly one WorkflowTracker field")
    elif tracker_fields[0]:
        failures.append("App WorkflowTracker field is public")

    forbidden_state = r"\b(?:WorkflowTracker|WorkflowRun|WorkflowNodeState)\b"
    if re.search(forbidden_state, session):
        failures.append("SessionController/session module owns or accesses workflow state")
    if re.search(forbidden_state, ui):
        failures.append("UiState/UI module owns or accesses workflow state")

    return failures


def write_fixture(root: Path) -> None:
    files = {
        DOMAIN_REL[0]: "pub struct WorkflowRun {\n    id: String,\n}\npub enum WorkflowEvent { Open }\n",
        DOMAIN_REL[1]: "pub struct WorkflowTracker {\n    runs: std::collections::HashMap<String, String>,\n}\n",
        ADAPTER_REL: "struct WireRun {\n    id: String,\n}\npub(crate) fn convert() {}\n",
        APP_REL: "struct App {\n    workflow_tracker: WorkflowTracker,\n}\n",
        SESSION_REL: "pub struct SessionController;\n",
        UI_REL: "pub struct UiState;\n",
    }
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def self_test() -> None:
    mutations = {
        "ACP alias": (DOMAIN_REL[0], "use agent_client_protocol as acp;\n"),
        "protocol import": (DOMAIN_REL[0], "use crate::protocol;\n"),
        "async runtime": (DOMAIN_REL[1], "async fn leak() {}\n"),
        "public domain field": (DOMAIN_REL[0], "pub struct Leaky {\n    pub value: String,\n}\n"),
        "public wire type": (ADAPTER_REL, "pub(crate) struct WireLeak;\n"),
        "public App owner": (APP_REL, "struct App {\n    pub workflow_tracker: WorkflowTracker,\n}\n"),
        "Session owner": (SESSION_REL, "struct SessionController { workflow_tracker: WorkflowTracker }\n"),
        "UI owner": (UI_REL, "struct UiState { workflow_run: WorkflowRun }\n"),
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        write_fixture(root)
        baseline = check(root)
        if baseline:
            raise SystemExit(f"self-test valid fixture rejected: {baseline}")
        for label, (rel, replacement) in mutations.items():
            write_fixture(root)
            path = root / rel
            if label in {"public App owner", "Session owner", "UI owner"}:
                path.write_text(replacement, encoding="utf-8")
            else:
                path.write_text(path.read_text(encoding="utf-8") + replacement, encoding="utf-8")
            if not check(root):
                raise SystemExit(f"self-test mutation escaped: {label}")
    print(f"C12 self-test passed: valid fixture + {len(mutations)} forbidden mutations")


def main() -> None:
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        return
    root = Path(sys.argv[1]).resolve() if len(sys.argv) == 2 else Path(__file__).resolve().parents[1]
    failures = check(root)
    if failures:
        raise SystemExit("C12 failed:\n- " + "\n- ".join(failures))
    print("C12 passed: private core model/adapter and App-only tracker ownership")


if __name__ == "__main__":
    main()

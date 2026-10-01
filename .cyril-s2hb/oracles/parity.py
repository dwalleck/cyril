#!/usr/bin/env python3
# /// script
# requires-python = ">=3.12"
# ///
"""Functional differential evidence for the slice-A native crtool port.

This is verification tooling, not a replacement implementation.  The Python
oracle is imported from ``.kiro/code-review/crtool.py`` without modifying its
source.  Native and oracle runs are checked for equivalent JSON data, ordered
records, meaningful patch content, and explicit statuses; formatting and host
line endings are intentionally not part of the contract.
"""
from __future__ import annotations

import argparse
import datetime as datetime_module
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from types import SimpleNamespace
from typing import Callable, NoReturn

ROOT = Path(__file__).resolve().parents[2]
ORACLE_PATH = ROOT / ".kiro" / "code-review" / "crtool.py"
FIXED_CLOCK = "2026-09-30T12:34:56+00:00"
PAGE_BUDGET = 20_000
SCALE_FILES = 100
SCALE_DEFINITIONS = 200
SCALE_USAGES = 50
SCALE_PATCH_BYTES = 2 * 1024 * 1024
UNSPLIT_DEFINITIONS = 90
UNSPLIT_NAME_LENGTH = 250
NONPRINTABLE_TARGET = "base\u200b\ue000\U000e0001"



class HarnessError(RuntimeError):
    pass


@dataclass(frozen=True)
class Captured:
    returncode: int
    stdout: bytes
    stderr: bytes


def fail(message: str) -> NoReturn:
    raise HarnessError(message)


def run(
    argv: list[str],
    cwd: Path,
    env: dict[str, str] | None = None,
    input_data: bytes | None = None,
) -> Captured:
    try:
        kwargs = {
            "cwd": cwd,
            "env": env,
            "stdout": subprocess.PIPE,
            "stderr": subprocess.PIPE,
        }
        if input_data is None:
            kwargs["stdin"] = subprocess.PIPE
        else:
            kwargs["input"] = input_data
        result = subprocess.run(argv, **kwargs)
    except OSError as exc:
        fail(f"could not start {argv[0]!r}: {exc}")
    return Captured(result.returncode, result.stdout, result.stderr)


def git(cwd: Path, *args: str, check: bool = True) -> bytes:
    result = run(["git", *args], cwd)
    if check and result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.decode(errors='replace').strip()}")
    return result.stdout


def put(path: Path, data: str | bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = data.encode("utf-8") if isinstance(data, str) else data
    with path.open("wb") as stream:
        stream.write(raw)


def snapshot(root: Path) -> dict[str, bytes]:
    if not root.exists():
        return {}
    result: dict[str, bytes] = {}
    for directory, _, names in os.walk(root):
        for name in sorted(names):
            path = Path(directory) / name
            result[str(path.relative_to(root)).replace(os.sep, "/")] = path.read_bytes()
    return dict(sorted(result.items()))

def directory_snapshot(root: Path) -> set[str]:
    directories: set[str] = set()
    if not root.exists():
        return directories
    for directory, names, _ in os.walk(root):
        base = Path(directory)
        for name in names:
            directories.add(str((base / name).relative_to(root)).replace(os.sep, "/"))
    return directories


def assert_run_layout(root: Path) -> None:
    expected = {"patches", "candidates", "deduped", "queues", "verdicts", "facts"}
    actual = directory_snapshot(root)
    if actual != expected:
        fail(f"run directory layout differs: {sorted(actual)!r}")


def remove_readonly(function: Callable[[str], object], path: str, error: BaseException) -> None:
    # Windows Git marks object files read-only. Only clear that attribute in
    # this owned fixture; unrelated deletion failures still abort qualification.
    if os.name != "nt" or not isinstance(error, PermissionError):
        raise error
    os.chmod(path, stat.S_IWRITE)
    function(path)


def clear_tree(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path, onexc=remove_readonly)


def package_version() -> str:
    with (ROOT / "crates" / "cyril-review" / "Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["package"]["version"]
    if isinstance(version, dict) and version.get("workspace") is True:
        with (ROOT / "Cargo.toml").open("rb") as stream:
            version = tomllib.load(stream)["workspace"]["package"]["version"]
    if not isinstance(version, str):
        fail("cyril-review package version is not a string or inherited workspace version")
    return version


def load_oracle():
    spec = importlib.util.spec_from_file_location("cyril_s2hb_crtool", ORACLE_PATH)
    if spec is None:
        fail(f"cannot import oracle {ORACLE_PATH}")
    loader = spec.loader
    if loader is None:
        fail(f"cannot load oracle {ORACLE_PATH}")
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


class FixedDateTime(datetime_module.datetime):
    value = datetime_module.datetime.fromisoformat(FIXED_CLOCK)

    @classmethod
    def now(cls, tz: datetime_module.tzinfo | None = None):
        if tz is None:
            return cls.value.replace(tzinfo=None)
        return cls.value.astimezone(tz)


def capture_oracle_call(call: Callable[[], None]) -> Captured:
    old_out, old_err = sys.stdout, sys.stderr
    out_bytes, err_bytes = io.BytesIO(), io.BytesIO()
    out = io.TextIOWrapper(out_bytes, encoding="utf-8", errors="replace", newline=None, write_through=True)
    err = io.TextIOWrapper(err_bytes, encoding="utf-8", errors="replace", newline=None, write_through=True)
    code = 0
    try:
        sys.stdout, sys.stderr = out, err
        try:
            call()
        except SystemExit as exc:
            value = exc.code
            code = int(value) if isinstance(value, int) else 1
    finally:
        out.flush()
        err.flush()
        sys.stdout, sys.stderr = old_out, old_err
    return Captured(code, out_bytes.getvalue(), err_bytes.getvalue())


def oracle_call(module, operation: str, workspace: Path, run_dir: Path, target: str = "", scope: str = "") -> Captured:
    original_cwd = Path.cwd()
    original_datetime = module.datetime
    original_writer = module.write_json
    version = package_version()

    def stamped_writer(path, obj):
        # The extra stamp is a verification adapter for fresh parity only.
        # It must not retrofit the source-owned Python implementation or
        # imply that Python's existing-run path enforces native C3 refusal.
        if Path(path).name == "manifest.json" and "gathered_at" in obj and "crtool_version" not in obj:
            obj["crtool_version"] = version
        return original_writer(path, obj)

    module.datetime = SimpleNamespace(datetime=FixedDateTime, timezone=datetime_module.timezone)
    module.write_json = stamped_writer
    try:
        os.chdir(workspace)
        if operation == "gather":
            args = SimpleNamespace(rundir=str(run_dir), target=target, scope=scope)
            return capture_oracle_call(lambda: module.cmd_gather(args))
        if operation == "facts":
            args = SimpleNamespace(rundir=str(run_dir))
            return capture_oracle_call(lambda: module.cmd_facts(args))
        fail(f"unknown oracle operation {operation}")
    finally:
        os.chdir(original_cwd)
        module.datetime = original_datetime
        module.write_json = original_writer


def invoke_driver(
    driver: Path,
    operation: str,
    workspace: Path,
    run_dir: Path,
    target: str = "",
    scope: str = "",
    env: dict[str, str] | None = None,
) -> Captured:
    argv = [str(driver), operation, "--workspace", str(workspace), "--rundir", str(run_dir)]
    if operation == "gather":
        argv += ["--target", target, "--scope", scope, "--gathered-at", FIXED_CLOCK]
    return run(argv, workspace, env)


def normalize_frame(data: bytes, label: str) -> str:
    """Decode generated text and normalize only CRLF framing."""
    try:
        return data.decode("utf-8").replace("\r\n", "\n")
    except UnicodeDecodeError as exc:
        fail(f"{label}: generated text is not UTF-8: {exc}")


def json_bytes(label: str, data: bytes) -> object:
    try:
        return json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        fail(f"{label}: invalid JSON: {exc}")


def timestamp_instant(label: str, value: object) -> datetime_module.datetime:
    if not isinstance(value, str):
        fail(f"{label}: gathered_at is not a string")
    try:
        parsed = datetime_module.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        fail(f"{label}: invalid gathered_at timestamp {value!r}: {exc}")
    if parsed.tzinfo is None:
        fail(f"{label}: gathered_at has no timezone")
    return parsed.astimezone(datetime_module.timezone.utc)


def warning_projection(label: str, value: object) -> tuple[object, ...]:
    if not isinstance(value, str):
        fail(f"{label}: warning is not a string")
    lower = value.casefold()
    if "no upstream/main" in lower and ("fell back" in lower or "fallback" in lower):
        fallback = "HEAD~1" if "head~1" in lower else "HEAD" if re.search(r"\bhead\b", lower) else None
        if fallback is None:
            fail(f"{label}: no-base warning lacks fallback target context")
        return ("no-base-fallback", fallback)
    if "merge-base" in lower and "dirty" in lower and "empty_range" in lower:
        flags: list[bool] = []
        for name in ("dirty", "empty_range"):
            match = re.search(rf"{name}\s*=\s*(true|false)", lower)
            if match is None:
                fail(f"{label}: merge-base warning lacks {name} context")
            flags.append(match.group(1) == "true")
        merge_base = re.search(r"merge-base\s+([0-9a-f]{7,40})", lower)
        return ("merge-base", *flags, merge_base.group(1) if merge_base else None)
    if "working tree" in lower and "head" in lower and "diff" in lower and (
        "not at" in lower or "not on" in lower
    ):
        head = re.search(r"diff(?:'s)?\s+head\s*\(([^)]*)\)", value, re.IGNORECASE)
        return ("head-mismatch", head.group(1) if head else None)
    fail(f"{label}: unrecognized warning condition/context: {value!r}")


def compare_warnings(label: str, native: object, oracle: object) -> None:
    if not isinstance(native, list) or not isinstance(oracle, list):
        fail(f"{label}: warnings must be arrays")
    if len(native) != len(oracle):
        fail(f"{label}: warning count differs: native={len(native)}, oracle={len(oracle)}")
    left = [warning_projection(f"{label} native warning {index}", value) for index, value in enumerate(native)]
    right = [warning_projection(f"{label} oracle warning {index}", value) for index, value in enumerate(oracle)]
    for index, (native_warning, oracle_warning) in enumerate(zip(left, right)):
        if native_warning[:2] != oracle_warning[:2]:
            fail(f"{label}: warning condition differs at {index}: {native_warning!r} != {oracle_warning!r}")
        if native_warning[0] == "merge-base":
            if native_warning[2] != oracle_warning[2]:
                fail(f"{label}: merge-base empty-range context differs at {index}")
            native_hash, oracle_hash = native_warning[3], oracle_warning[3]
            if native_hash is not None and oracle_hash is not None and not (
                str(native_hash).startswith(str(oracle_hash)) or str(oracle_hash).startswith(str(native_hash))
            ):
                fail(f"{label}: merge-base context differs at {index}: {native_hash!r} != {oracle_hash!r}")
        if native_warning[0] == "head-mismatch" and native_warning[1] != oracle_warning[1]:
            fail(f"{label}: diff-head warning context differs at {index}: {native_warning!r} != {oracle_warning!r}")

def assert_warning_conditions(label: str, files: dict[str, bytes]) -> None:
    manifest = json_bytes(f"{label} warning manifest", files.get("manifest.json", b""))
    if not isinstance(manifest, dict):
        fail(f"{label}: warning manifest is not an object")
    warnings = manifest.get("warnings")
    if not isinstance(warnings, list):
        fail(f"{label}: warnings are not an array")
    actual = [warning_projection(f"{label} warning {index}", value) for index, value in enumerate(warnings)]
    if label == "auto-no-base":
        if actual != [("no-base-fallback", "HEAD~1")]:
            fail(f"{label}: unexpected fallback warning semantics: {actual!r}")
    elif label == "auto-dirty":
        if not actual or actual[0][:3] != ("merge-base", True, False):
            fail(f"{label}: dirty merge-base warning semantics are missing: {actual!r}")
    elif label == "range-head-warning":
        if not actual or actual[0][:2] != ("head-mismatch", "other"):
            fail(f"{label}: diff-head warning semantics are missing: {actual!r}")
    elif actual:
        fail(f"{label}: unexpected warning conditions: {actual!r}")


def validate_artifact_sizes(label: str, files: dict[str, bytes], workspace: Path | None = None) -> None:
    manifest = json_bytes(f"{label} manifest", files.get("manifest.json", b""))
    if not isinstance(manifest, dict):
        fail(f"{label}: manifest is not an object")
    full = files.get("diff.patch")
    if full is None:
        fail(f"{label}: diff.patch is missing")
    total = manifest.get("total_patch_bytes")
    if type(total) is not int or total < 0:
        fail(f"{label}: total_patch_bytes is not a non-negative integer")
    if total != len(full):
        fail(f"{label}: total_patch_bytes={total} does not describe diff.patch size {len(full)}")
    entries = manifest.get("files")
    if not isinstance(entries, list):
        fail(f"{label}: manifest files is not an array")
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            fail(f"{label}: files[{index}] is not an object")
        patch_name = entry.get("patch")
        patch_bytes = entry.get("patch_bytes")
        if not isinstance(patch_name, str) or type(patch_bytes) is not int or patch_bytes < 0:
            fail(f"{label}: files[{index}] has invalid patch metadata")
        patch = files.get(patch_name)
        if patch is None:
            fail(f"{label}: files[{index}] patch {patch_name!r} is missing")
        if patch_bytes != len(patch):
            fail(
                f"{label}: files[{index}] patch_bytes={patch_bytes} "
                f"does not describe {patch_name} size {len(patch)}"
            )
    docs = manifest.get("change_docs")
    if docs is None:
        return
    if not isinstance(docs, list):
        fail(f"{label}: change_docs is not an array")
    for index, entry in enumerate(docs):
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str) or type(entry.get("bytes")) is not int:
            fail(f"{label}: change_docs[{index}] has invalid byte metadata")
        if workspace is not None:
            source = workspace / entry["path"]
            if not source.is_file():
                fail(f"{label}: change_docs[{index}] source is missing: {entry['path']!r}")
            if entry["bytes"] != source.stat().st_size:
                fail(
                    f"{label}: change_docs[{index}] bytes={entry['bytes']} "
                    f"does not describe source size {source.stat().st_size}"
                )


def git_patch_id(cwd: Path, patch: bytes, label: str) -> str:
    result = run(["git", "patch-id", "--verbatim"], cwd, input_data=patch)
    if result.returncode != 0:
        fail(f"{label}: git patch-id failed: {normalize_frame(result.stderr, label).strip()}")
    fields = result.stdout.decode("ascii", errors="replace").split()
    if not fields:
        fail(f"{label}: git patch-id produced no identity")
    return fields[0]


def compare_patch_meaning(label: str, native: dict[str, bytes], oracle: dict[str, bytes], cwd: Path) -> None:
    names = {"diff.patch"}
    for files in (native, oracle):
        manifest = json_bytes(f"{label} patch manifest", files.get("manifest.json", b""))
        if isinstance(manifest, dict) and isinstance(manifest.get("files"), list):
            names.update(
                entry["patch"]
                for entry in manifest["files"]
                if isinstance(entry, dict) and isinstance(entry.get("patch"), str)
            )
    for name in sorted(names):
        native_patch = native.get(name)
        oracle_patch = oracle.get(name)
        if native_patch is None or oracle_patch is None:
            fail(f"{label}: patch {name!r} is missing on one side")
        native_id = git_patch_id(cwd, native_patch, f"{label} native {name}")
        oracle_id = git_patch_id(cwd, oracle_patch, f"{label} oracle {name}")
        if native_id != oracle_id:
            fail(f"{label}: patch meaning differs for {name!r}: {native_id} != {oracle_id}")


def compare_json_nodes(
    label: str,
    path: tuple[object, ...],
    native: object,
    oracle: object,
) -> None:
    if type(native) is not type(oracle):
        fail(f"{label}: JSON type differs at {path!r}: {type(native).__name__} != {type(oracle).__name__}")
    if path == ("gathered_at",):
        if timestamp_instant(f"{label} native", native) != timestamp_instant(f"{label} oracle", oracle):
            fail(f"{label}: gathered_at instants differ")
        return
    if path == ("warnings",):
        compare_warnings(label, native, oracle)
        return
    if path == ("total_patch_bytes",) or path[-1:] == ("patch_bytes",) or (
        len(path) == 3 and path[0] == "change_docs" and path[-1] == "bytes"
    ):
        if type(native) is not int or native < 0:
            fail(f"{label}: invalid byte-count value at {path!r}")
        return
    if isinstance(native, dict) and isinstance(oracle, dict):
        if set(native) != set(oracle):
            fail(f"{label}: JSON fields differ at {path!r}: {sorted(native)!r} != {sorted(oracle)!r}")
        for key in native:
            compare_json_nodes(label, path + (key,), native[key], oracle[key])
        return
    if isinstance(native, list) and isinstance(oracle, list):
        if len(native) != len(oracle):
            fail(f"{label}: JSON array length differs at {path!r}")
        for index, (left, right) in enumerate(zip(native, oracle)):
            compare_json_nodes(label, path + (index,), left, right)
        return
    if native != oracle:
        fail(f"{label}: JSON value differs at {path!r}: {native!r} != {oracle!r}")


def compare_json_files(label: str, native: bytes, oracle: bytes) -> None:
    compare_json_nodes(label, (), json_bytes(f"{label} native", native), json_bytes(f"{label} oracle", oracle))




def assert_gather_output(label: str, result: Captured, files: dict[str, bytes], *, repeated: bool) -> None:
    text = normalize_frame(result.stdout, f"{label} stdout")
    if not text.strip():
        fail(f"{label}: gather produced no status output")
    manifest = json_bytes(f"{label} manifest", files.get("manifest.json", b""))
    if not isinstance(manifest, dict):
        fail(f"{label}: gather manifest is not an object")
    full = re.search(
        r"\b(?:already\s+)?gathered\s+(\d+)\s+files,\s+(\d+)\s+patch\s+bytes,\s+target=",
        text,
        re.IGNORECASE,
    )
    if repeated:
        reused = bool(
            re.search(
                r"\b(already\s+gathered|reusing\s+existing|reused\s+existing|unchanged\s+run)\b",
                text,
                re.IGNORECASE,
            )
        )
        if not reused:
            fail(f"{label}: repeated gather lacks reuse status")
        if full is not None:
            if int(full.group(1)) != manifest.get("total_files") or int(full.group(2)) != manifest.get("total_patch_bytes"):
                fail(f"{label}: repeated gather totals do not match manifest")
        else:
            compact = re.search(r"\b(?:already\s+gathered|reusing\s+existing|reused\s+existing)[^0-9]*(\d+)\s+files,\s+target=", text, re.IGNORECASE)
            if compact is None or int(compact.group(1)) != manifest.get("total_files"):
                fail(f"{label}: repeated gather lacks file-count and target context")
        target = manifest.get("target")
        if not isinstance(target, str) or target not in text:
            fail(f"{label}: repeated gather lacks target context")
        return
    if full is None:
        fail(f"{label}: fresh gather status lacks file/patch totals and target")
    if int(full.group(1)) != manifest.get("total_files") or int(full.group(2)) != manifest.get("total_patch_bytes"):
        fail(f"{label}: gather status totals do not match manifest")
    target = manifest.get("target")
    if not isinstance(target, str) or target not in text:
        fail(f"{label}: gather status lacks target context")
    pages = manifest.get("facts", {}).get("usages_pages", []) if isinstance(manifest.get("facts"), dict) else []
    if not isinstance(pages, list) or not pages:
        fail(f"{label}: fresh gather has no facts page list")
    if not any(isinstance(page, str) and page in text for page in pages):
        fail(f"{label}: fresh gather does not report a facts page location")


def assert_facts_output(label: str, result: Captured, files: dict[str, bytes]) -> None:
    text = normalize_frame(result.stdout, f"{label} stdout")
    match = re.search(r"\bfacts:\s+(\d+)\s+changed\s+symbols\b", text, re.IGNORECASE)
    if match is None:
        fail(f"{label}: facts status lacks changed-symbol count")
    symbols = json_bytes(f"{label} symbols", files.get("facts/symbols.json", b""))
    if not isinstance(symbols, list) or int(match.group(1)) != len(symbols):
        fail(f"{label}: facts status count does not match symbols.json")
    manifest = json_bytes(f"{label} manifest", files.get("manifest.json", b""))
    pages = manifest.get("facts", {}).get("usages_pages", []) if isinstance(manifest, dict) and isinstance(manifest.get("facts"), dict) else []
    if not isinstance(pages, list) or not pages or not any(isinstance(page, str) and page in text for page in pages):
        fail(f"{label}: facts status does not report a facts page location")


def error_category(label: str, result: Captured) -> str:
    text = normalize_frame(result.stderr, f"{label} stderr")
    if not text.casefold().startswith("crtool: error:"):
        fail(f"{label}: error output lacks crtool error category prefix")
    lower = text.casefold()
    if "empty diff" in lower:
        return "empty-diff"
    if "crtool_version" in lower or "unstamped" in lower or "stamp" in lower:
        return "stamp"
    if "already holds a different" in lower or "identity" in lower:
        return "identity"
    if "git" in lower or "revision" in lower or "resolve" in lower:
        return "git"
    return "error"


def assert_error_output(
    label: str,
    result: Captured,
    expected_code: int,
    expected_category: str,
    context: tuple[str, ...] = (),
) -> None:
    if result.returncode != expected_code:
        fail(f"{label}: status {result.returncode}, expected {expected_code}")
    if result.stdout:
        fail(f"{label}: error emitted stdout")
    category = error_category(label, result)
    if category != expected_category:
        fail(f"{label}: category {category!r}, expected {expected_category!r}")
    text = normalize_frame(result.stderr, f"{label} stderr").casefold()
    for fragment in context:
        if fragment.casefold() not in text:
            fail(f"{label}: error lacks context {fragment!r}")


def compare_results(
    label: str,
    native: Captured,
    oracle: Captured,
    native_files: dict[str, bytes],
    oracle_files: dict[str, bytes],
    *,
    operation: str = "gather",
    repeated: bool = False,
    workspace: Path | None = None,
) -> None:
    if native.returncode != oracle.returncode:
        fail(f"{label}: exit status differs: native={native.returncode}, oracle={oracle.returncode}")
    if native.returncode == 0:
        if native.stderr or oracle.stderr:
            fail(f"{label}: successful operation emitted stderr")
        if operation == "gather":
            assert_gather_output(f"{label} native", native, native_files, repeated=repeated)
            assert_gather_output(f"{label} oracle", oracle, oracle_files, repeated=repeated)
        elif operation == "facts":
            assert_facts_output(f"{label} native", native, native_files)
            assert_facts_output(f"{label} oracle", oracle, oracle_files)
        else:
            fail(f"{label}: unsupported successful operation {operation!r}")
    else:
        native_category = error_category(f"{label} native", native)
        oracle_category = error_category(f"{label} oracle", oracle)
        if native_category != oracle_category:
            fail(f"{label}: error category differs: {native_category!r} != {oracle_category!r}")
    if set(native_files) != set(oracle_files):
        fail(f"{label}: output file set differs")
    for name in sorted(native_files):
        left, right = native_files[name], oracle_files[name]
        if name.endswith(".json"):
            compare_json_files(f"{label} {name}", left, right)
        elif name.endswith(".patch"):
            continue
        elif name == "changed-files.txt":
            if normalize_frame(left, label).splitlines() != normalize_frame(right, label).splitlines():
                fail(f"{label}: changed-file paths or ordering differ")
        elif name.startswith("facts/usages-") and name.endswith(".txt"):
            # Renderer content/bounds have fixture assertions and the leaf's
            # page-boundary test; its prose is not a Python serialization API.
            normalize_frame(left, f"{label} native {name}")
            normalize_frame(right, f"{label} oracle {name}")
        else:
            fail(f"{label}: unsupported output artifact {name!r}")
    if native.returncode == 0:
        validate_artifact_sizes(f"{label} native", native_files, workspace)
        validate_artifact_sizes(f"{label} oracle", oracle_files, workspace)
        compare_patch_meaning(label, native_files, oracle_files, workspace or Path.cwd())


def commit(cwd: Path, message: str, env: dict[str, str]) -> None:
    git(cwd, "add", "-A")
    result = run(["git", "commit", "-qm", message], cwd, env)
    if result.returncode != 0:
        fail(f"fixture commit failed: {result.stderr.decode(errors='replace')}")


def base_repo(workspace: Path, *, docs: int = 0) -> None:
    workspace.mkdir(parents=True, exist_ok=True)
    result = run(["git", "init", "-q"], workspace)
    if result.returncode != 0:
        fail(f"git init failed: {result.stderr.decode(errors='replace')}")
    git(workspace, "config", "user.email", "parity@example.invalid")
    git(workspace, "config", "user.name", "Parity Fixture")
    git(workspace, "config", "core.autocrlf", "false")
    git(workspace, "config", "core.quotePath", "false")
    git(workspace, "branch", "-M", "base")
    put(workspace / "src" / "base.rs", "pub fn old_helper() {}\n")
    put(workspace / "src" / "caller.rs", "pub fn call_old() { old_helper(); }\n")
    put(workspace / "src" / "removed.py", "def removed():\n    return 1\n")
    put(workspace / "assets" / "blob.bin", b"binary-before\x00\xff\n")
    put(workspace / "docs" / "NOTES.md", "baseline notes\n")
    for index in range(docs):
        put(workspace / "docs" / "changes" / f"doc-{index:02d}.md", f"baseline {index}\n")
    env = os.environ.copy()
    env.update({"GIT_AUTHOR_DATE": "2026-09-30T00:00:00+0000", "GIT_COMMITTER_DATE": "2026-09-30T00:00:00+0000"})
    commit(workspace, "baseline", env)
    git(workspace, "branch", "main")
    git(workspace, "branch", "master")
    git(workspace, "checkout", "-q", "-b", "feature")


def finish_feature(workspace: Path, *, no_base: bool = False, dirty: bool = False, other: bool = False) -> None:
    env = os.environ.copy()
    env.update({"GIT_AUTHOR_DATE": "2026-09-30T00:01:00+0000", "GIT_COMMITTER_DATE": "2026-09-30T00:01:00+0000"})
    commit(workspace, "feature", env)
    if other:
        git(workspace, "checkout", "-q", "-b", "other")
        put(workspace / "src" / "other.rs", "pub fn other_helper() {}\n")
        env.update({"GIT_AUTHOR_DATE": "2026-09-30T00:02:00+0000", "GIT_COMMITTER_DATE": "2026-09-30T00:02:00+0000"})
        commit(workspace, "other", env)
        git(workspace, "checkout", "-q", "feature")
    if no_base:
        git(workspace, "update-ref", "-d", "refs/heads/main")
        git(workspace, "update-ref", "-d", "refs/heads/master")
    if dirty:
        with (workspace / "src" / "caller.rs").open("ab") as stream:
            stream.write(b"// dirty\n")


def build_canonical(workspace: Path) -> None:
    """Small complete fixture; broad and scale cases remain live comparisons."""
    base_repo(workspace)
    put(workspace / "src" / "helper.rs", "pub fn helper() {}\npub fn caller() { helper(); }\n")
    put(workspace / "docs" / "NOTES.md", "changed notes — Δ and 非 ASCII\n")
    finish_feature(workspace)


def add_rich_changes(workspace: Path) -> None:
    put(workspace / "src" / "helper.rs", "\n".join([
        "#[test]", "fn skipped_test() {}", "pub fn unicode_helper() {}", "pub fn fourty_helper() {}", "",
    ]))
    calls = "pub fn callers() {\n" + "\n".join("    unicode_helper();" for _ in range(45)) + "\n}\n"
    put(workspace / "src" / "caller.rs", calls)
    put(workspace / "src" / "logic.py", "def python_helper(value):\n    return value + 1\n")
    put(workspace / "src" / "view.ts", "export async function typescript_helper() { return true; }\n")
    put(workspace / "src" / "worker.go", "func GoHelper() {}\n")
    put(workspace / "src" / "é.rs", "pub fn unicode_path_helper() {}\n")
    put(workspace / "src" / "empty.rs", b"")
    put(workspace / "docs" / "NOTES.md", "changed notes — Δ and 非 ASCII\n")
    put(workspace / "assets" / "blob.bin", b"binary-after\x00\xfe\x80\n")
    (workspace / "src" / "removed.py").unlink()


def build_rich(workspace: Path, *, no_base: bool = False, dirty: bool = False, other: bool = False) -> None:
    base_repo(workspace)
    add_rich_changes(workspace)
    finish_feature(workspace, no_base=no_base, dirty=dirty, other=other)


def build_divergent_refs(workspace: Path) -> None:
    base_repo(workspace)
    git(workspace, "checkout", "-q", "master")
    put(workspace / "src" / "master_only.rs", "pub fn master_only() {}\n")
    env = os.environ.copy()
    env.update({"GIT_AUTHOR_DATE": "2026-09-30T00:01:00+0000", "GIT_COMMITTER_DATE": "2026-09-30T00:01:00+0000"})
    commit(workspace, "master-only", env)
    git(workspace, "checkout", "-q", "feature")
    add_rich_changes(workspace)
    finish_feature(workspace)


def build_caps(workspace: Path) -> None:
    base_repo(workspace, docs=41)
    definitions = [f"pub fn caps_{index:03d}() {{}}" for index in range(70)]
    put(workspace / "src" / "caps.rs", "\n".join(definitions) + "\n")
    caller_lines = []
    filler = "x" * 125
    for index in range(70):
        for _ in range(50):
            caller_lines.append(f"caps_{index:03d}(); // {filler}")
    put(workspace / "src" / "caps_caller.rs", "\n".join(caller_lines) + "\n")
    put(workspace / "docs" / "NOTES.md", "changed notes for cap fixture\n")
    for index in range(41):
        put(workspace / "docs" / "changes" / f"doc-{index:02d}.md", f"changed {index} — Δ\n")
    finish_feature(workspace)

def build_scale(workspace: Path) -> None:
    """Exercise the plan's production-scale F+S+20 and 2 MiB obligations."""
    base_repo(workspace)
    names = [
        f"scale_{index:03d}_{suffix}"
        for index in range(SCALE_DEFINITIONS // 2)
        for suffix in ("a", "b")
    ]
    filler = "// " + ("x" * 22_000) + "\n"
    for index in range(SCALE_FILES - 1):
        first, second = names[index * 2 : index * 2 + 2]
        put(
            workspace / "src" / f"scale-{index:03d}.rs",
            f"pub fn {first}() {{}}\npub fn {second}() {{}}\n{filler}",
        )
    caller = [
        f"pub fn {names[-2]}() {{}}",
        f"pub fn {names[-1]}() {{}}",
    ]
    for name in names:
        caller.extend(f"{name}();" for _ in range(SCALE_USAGES))
    put(workspace / "src" / "scale-caller.rs", "\n".join(caller) + "\n")
    finish_feature(workspace)

def unsplit_name(index: int) -> str:
    prefix = f"unused_{index:03d}_"
    return prefix + ("n" * (UNSPLIT_NAME_LENGTH - len(prefix)))


def build_unsplit_page(workspace: Path) -> None:
    base_repo(workspace)
    definitions = [f"pub fn {unsplit_name(index)}() {{}}" for index in range(UNSPLIT_DEFINITIONS)]
    put(workspace / "src" / "unused.rs", "\n".join(definitions) + "\n")
    finish_feature(workspace)



def build_empty(workspace: Path) -> None:
    base_repo(workspace)


def build_nonprintable_ref(workspace: Path) -> None:
    build_empty(workspace)
    git(workspace, "branch", NONPRINTABLE_TARGET)



def build_no_base(workspace: Path) -> None:
    build_rich(workspace, no_base=True)


def build_dirty(workspace: Path) -> None:
    build_rich(workspace, dirty=True)


def pair_gather(label: str, builder: Callable[[Path], None], driver: Path, target: str, scope: str) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-parity-") as temporary:
        root = Path(temporary)
        workspace, run_dir = root / "workspace", root / "rundir"
        builder(workspace)
        native_env = None
        trace_path = root / "git-trace.json"
        if label == "scale-budget":
            native_env = os.environ.copy()
            native_env["GIT_TRACE2_EVENT"] = str(trace_path)
        native = invoke_driver(driver, "gather", workspace, run_dir, target, scope, native_env)
        native_files = snapshot(run_dir)
        if native.returncode == 0:
            assert_run_layout(run_dir)
            assert_direct_git_semantics(workspace, native_files, scope)
            validate_artifact_sizes(f"{label} native", native_files, workspace)
            if label == "range-head-warning":
                assert_facts_semantics(native_files, required_symbols=("other_helper",), require_notes=False)
            elif label == "canonical":
                assert_facts_semantics(native_files, required_symbols=("helper", "caller"))
            elif label == "scale-budget":
                assert_scale_semantics(native_files)
                assert_git_trace_budget(trace_path)
            elif label == "unsplit-page":
                assert_unsplit_page_semantics(native_files)
            else:
                assert_facts_semantics(native_files, caps=label == "caps-and-documents")
            if label == "binary-deleted":
                assert_binary_semantics(native_files)
            if label == "auto-main-precedence":
                manifest = json_bytes(f"{label} manifest", native_files["manifest.json"])
                if not isinstance(manifest, dict) or manifest.get("target") != "main...HEAD":
                    fail(f"auto target did not prefer main: {manifest!r}")
            assert_warning_conditions(label, native_files)
        clear_tree(workspace)
        builder(workspace)
        clear_tree(run_dir)
        module = load_oracle()
        oracle = oracle_call(module, "gather", workspace, run_dir, target, scope)
        oracle_files = snapshot(run_dir)
        if oracle.returncode == 0:
            assert_run_layout(run_dir)
            if label == "binary-deleted":
                assert_binary_semantics(oracle_files)
            assert_warning_conditions(label, oracle_files)
        compare_results(label, native, oracle, native_files, oracle_files, workspace=workspace)


def pair_facts(label: str, builder: Callable[[Path], None], driver: Path, target: str, scope: str) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-facts-") as temporary:
        root = Path(temporary)
        workspace, run_dir = root / "workspace", root / "rundir"
        builder(workspace)
        seeded = invoke_driver(driver, "gather", workspace, run_dir, target, scope)
        if seeded.returncode != 0:
            fail(f"{label}: native seed gather failed: {seeded.stderr!r}")
        assert_run_layout(run_dir)
        native_seed_files = snapshot(run_dir)
        assert_gather_output(f"{label} native seed", seeded, native_seed_files, repeated=False)
        assert_direct_git_semantics(workspace, native_seed_files, scope)
        native = invoke_driver(driver, "facts", workspace, run_dir)
        native_files = snapshot(run_dir)
        assert_facts_semantics(native_files)
        assert_facts_output(f"{label} native", native, native_files)
        clear_tree(workspace)
        builder(workspace)
        clear_tree(run_dir)
        module = load_oracle()
        seeded_oracle = oracle_call(module, "gather", workspace, run_dir, target, scope)
        if seeded_oracle.returncode != 0:
            fail(f"{label}: oracle seed gather failed: {seeded_oracle.stderr!r}")
        assert_run_layout(run_dir)
        oracle = oracle_call(module, "facts", workspace, run_dir)
        compare_results(label, native, oracle, native_files, snapshot(run_dir), operation="facts", workspace=workspace)


def assert_direct_git_semantics(workspace: Path, files: dict[str, bytes], scope: str) -> None:
    manifest = json_bytes("direct Git manifest", files.get("manifest.json", b""))
    if not isinstance(manifest, dict) or not isinstance(manifest.get("target"), str):
        fail("direct Git manifest lacks target")
    pathspec = scope.split() or ["."]
    full = git(workspace, "diff", "--no-renames", manifest["target"], "--", *pathspec)
    stored = files.get("diff.patch")
    if stored is None or git_patch_id(workspace, stored, "stored full patch") != git_patch_id(workspace, full, "direct full patch"):
        fail("stored diff.patch does not represent the direct Git diff")
    entries = manifest.get("files")
    if not isinstance(entries, list):
        fail("direct Git manifest files is not an array")
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str) or not isinstance(entry.get("patch"), str):
            fail("direct Git manifest file record is malformed")
        expected = git(workspace, "diff", "--no-renames", manifest["target"], "--", entry["path"])
        actual = files.get(entry["patch"])
        if actual is None or git_patch_id(workspace, actual, f"stored patch {entry['path']}") != git_patch_id(
            workspace, expected, f"direct patch {entry['path']}"
        ):
            fail(f"stored Git patch does not represent the direct diff for {entry['path']!r}")

def assert_git_trace_budget(path: Path) -> None:
    try:
        events = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    except (OSError, ValueError) as exc:
        fail(f"scale fixture Git trace is unavailable or invalid: {exc}")
    histogram: dict[str, int] = {}
    for event in events:
        if event.get("event") == "cmd_name":
            name = str(event.get("name", "<unknown>"))
            histogram[name] = histogram.get(name, 0) + 1
    calls = sum(event.get("event") == "start" for event in events)
    maximum = SCALE_FILES + SCALE_DEFINITIONS + 20
    if calls == 0:
        fail(f"scale fixture Git trace recorded no start events; cmd_name={histogram!r}")
    if calls > maximum:
        fail(
            f"scale fixture used {calls} Git calls; expected at most "
            f"F+S+20={maximum}; cmd_name={histogram!r}"
        )


def assert_scale_semantics(files: dict[str, bytes]) -> None:
    manifest = json.loads(files["manifest.json"].decode("utf-8"))
    if manifest["total_files"] != SCALE_FILES:
        fail(f"scale fixture changed-file count is {manifest['total_files']}, expected {SCALE_FILES}")
    if manifest["total_patch_bytes"] < SCALE_PATCH_BYTES:
        fail(
            f"scale fixture patch is {manifest['total_patch_bytes']} bytes, "
            f"expected at least {SCALE_PATCH_BYTES}"
        )
    symbols = json.loads(files["facts/symbols.json"].decode("utf-8"))
    if len(symbols) != SCALE_DEFINITIONS:
        fail(f"scale fixture found {len(symbols)} symbols, expected {SCALE_DEFINITIONS}")
    by_name = {entry["name"]: entry for entry in symbols}
    for index in range(SCALE_DEFINITIONS // 2):
        for suffix in ("a", "b"):
            name = f"scale_{index:03d}_{suffix}"
            entry = by_name.get(name)
            if entry is None or entry["usage_count"] <= 40 or len(entry["usages"]) <= 40:
                fail(f"scale fixture did not retain >40 usages for {name}")
    pages = json.loads(files["manifest.json"].decode("utf-8"))["facts"]["usages_pages"]
    if not pages:
        fail("scale fixture did not emit any facts page")
    for page in pages:
        lines = files[page].decode("utf-8").splitlines()
        body = lines[1:]
        if body and body[-1] == "":
            body.pop()
        if sum(len(line) + 1 for line in body) > PAGE_BUDGET:
            fail(f"scale facts page exceeds {PAGE_BUDGET} characters: {page}")

def assert_unsplit_page_semantics(files: dict[str, bytes]) -> None:
    manifest = json.loads(files["manifest.json"].decode("utf-8"))
    if manifest["total_files"] != 1:
        fail(f"unsplit-page fixture changed-file count is {manifest['total_files']}, expected 1")
    symbols = json.loads(files["facts/symbols.json"].decode("utf-8"))
    if len(symbols) != UNSPLIT_DEFINITIONS:
        fail(f"unsplit-page fixture found {len(symbols)} symbols, expected {UNSPLIT_DEFINITIONS}")
    expected_summary = "; ".join(
        f"{unsplit_name(index)} (fn, src/unused.rs:{index + 1})"
        for index in range(UNSPLIT_DEFINITIONS)
    )
    if len(expected_summary) <= PAGE_BUDGET:
        fail("unsplit-page expected summary does not exceed the page budget")
    pages = manifest["facts"]["usages_pages"]
    oversized: list[str] = []
    found_summary = False
    for page in pages:
        lines = files[page].decode("utf-8").splitlines()
        body = lines[1:]
        if body and body[-1] == "":
            body.pop()
        for line in body:
            if line == expected_summary:
                found_summary = True
            if len(line) + 1 > PAGE_BUDGET:
                oversized.append(line)
        if not any(len(line) + 1 > PAGE_BUDGET for line in body):
            if sum(len(line) + 1 for line in body) > PAGE_BUDGET:
                fail(f"unsplit-page ordinary facts page exceeds {PAGE_BUDGET}: {page}")
    if not found_summary or oversized != [expected_summary]:
        fail("unsplit-page fixture did not preserve its exact single overlong summary line")



def assert_facts_semantics(
    files: dict[str, bytes],
    *,
    caps: bool = False,
    required_symbols: tuple[str, ...] | None = None,
    require_notes: bool = True,
) -> None:
    manifest = json_bytes("facts manifest", files.get("manifest.json", b""))
    if not isinstance(manifest, dict):
        fail("facts manifest is not an object")
    expected = {
        "requested_target", "target", "scope", "head", "worktree_matches_diff_head",
        "gathered_at", "crtool_version", "total_files", "total_patch_bytes", "warnings",
        "files", "change_docs", "facts",
    }
    if set(manifest) != expected:
        fail(f"manifest fields differ: {sorted(manifest)!r}")
    if not isinstance(manifest.get("crtool_version"), str) or manifest["crtool_version"] != package_version():
        fail("manifest package version does not match workspace package version")
    symbols = json_bytes("facts symbols", files.get("facts/symbols.json", b""))
    if not isinstance(symbols, list) or any(
        not isinstance(entry, dict) or not isinstance(entry.get("name"), str) for entry in symbols
    ):
        fail("facts symbols is not an array of named records")
    by_name = {entry["name"]: entry for entry in symbols}
    if required_symbols is not None:
        required = required_symbols
    elif caps:
        required = ("caps_000",)
    else:
        required = ("unicode_helper", "python_helper", "typescript_helper", "GoHelper", "unicode_path_helper")
    page_text = "\n".join(
        normalize_frame(files[page], f"facts page {page}")
        for page in manifest["facts"]["usages_pages"]
    )
    for name in required:
        if name not in by_name:
            fail(f"facts omitted independent expected symbol {name!r}")
        if name not in page_text:
            fail(f"facts pages omitted expected symbol {name!r}")
        for usage in by_name[name]["usages"][:6 if caps else 3]:
            location = f"{usage['file']}:{usage['line']}"
            if location not in page_text or usage["text"] not in page_text:
                fail(f"facts pages omitted expected caller content for {name!r}: {usage!r}")
    if caps:
        if by_name["caps_000"]["usage_count"] <= 40 or len(by_name["caps_000"]["usages"]) <= 40:
            fail("facts applied an unauthorized usage storage cap")
    elif required_symbols is None and (
        by_name["unicode_helper"]["usage_count"] <= 40 or len(by_name["unicode_helper"]["usages"]) <= 40
    ):
        fail("facts applied an unauthorized 40-usage storage cap")
    if required_symbols is None and not caps:
        helper = by_name["unicode_helper"]
        expected_usages = [
            {"file": "src/caller.rs", "line": line, "text": "unicode_helper();"} for line in range(2, 47)
        ]
        expected_helper = {
            "name": "unicode_helper",
            "kind": "fn",
            "file": "src/helper.rs",
            "line": 3,
            "status": "added",
            "ambiguous": False,
            "usage_count": 45,
            "usages": expected_usages,
        }
        if helper != expected_helper:
            fail("independent helper/caller record differs")
    docs = manifest["change_docs"]
    if require_notes:
        notes = next((item for item in docs if item["path"] == "docs/NOTES.md"), None)
        expected_text = "changed notes for cap fixture\n" if caps else "changed notes — Δ and 非 ASCII\n"
        expected_notes = {"path": "docs/NOTES.md", "bytes": len(expected_text.encode("utf-8"))}
        if notes != expected_notes:
            fail("independent NOTES.md record differs")
    if caps:
        if len(docs) != 40:
            fail(f"change document cap is not 40: {len(docs)}")
        pages = manifest["facts"]["usages_pages"]
        if len(pages) < 2:
            fail("large facts fixture did not produce multiple pages")
        chosen = set()
        for page in pages:
            text = files[page].decode("utf-8")
            chosen.update(re.findall(r"first (25|12|6|3) shown", text))
            lines = text.splitlines()
            body = lines[1:]
            if body and body[-1] == "":
                body.pop()
            if sum(len(line) + 1 for line in body) > PAGE_BUDGET:
                fail(f"facts page exceeds {PAGE_BUDGET} characters: {page}")
        if not chosen or "6" not in chosen:
            fail(f"facts renderer did not exercise cap ladder: {sorted(chosen)!r}")

def assert_binary_semantics(files: dict[str, bytes]) -> None:
    manifest = json.loads(files["manifest.json"].decode("utf-8"))
    entries = {entry["path"]: entry for entry in manifest["files"]}
    blob = entries.get("assets/blob.bin")
    removed = entries.get("src/removed.py")
    empty = entries.get("src/empty.rs")
    if blob is None or not blob["binary"] or blob["insertions"] is not None or blob["deletions"] is not None:
        fail("binary Git entry did not preserve null counts")
    if removed is None or removed["status"] != "D":
        fail("deleted Git entry was omitted or has the wrong status")
    if empty is None or empty["status"] != "A" or empty["insertions"] != 0 or empty["deletions"] != 0:
        fail("empty added file did not preserve zero numstat counts")




def native_stamp_refusal(driver: Path, builder: Callable[[Path], None], target: str, scope: str) -> None:
    """Qualify native C3 stamp safety by refusal category and unchanged artifacts.

    The source-owned Python oracle predates this version fence and intentionally
    remains unchanged.  Do not route these cases through ``oracle_call`` or
    describe them as native/Python byte parity.
    """
    cases: list[tuple[str, object]] = [
        ("absent", None),
        ("empty", ""),
        ("non-string", 7),
        ("object", {"x": [1, 2]}),
        ("mismatch", "0.0.0"),
        ("escaped", "bad\\path\nstamp"),
    ]
    for label, value in cases:
        with tempfile.TemporaryDirectory(prefix="cyril-s2hb-stamp-") as temporary:
            root = Path(temporary)
            workspace, run_dir = root / "workspace", root / "rundir"
            builder(workspace)
            seeded = invoke_driver(driver, "gather", workspace, run_dir, target, scope)
            if seeded.returncode != 0:
                fail(f"stamp {label}: seed gather failed")
            manifest_path = run_dir / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            if value is None:
                del manifest["crtool_version"]
            else:
                manifest["crtool_version"] = value
            with manifest_path.open("w", encoding="utf-8", newline=None) as stream:
                json.dump(manifest, stream, indent=2, ensure_ascii=False)
                stream.write("\n")
            before = snapshot(run_dir)
            for operation in ("facts", "gather"):
                result = invoke_driver(driver, operation, workspace, run_dir, target, scope)
                assert_error_output(
                    f"stamp {label} {operation}",
                    result,
                    2,
                    "stamp",
                    ("crtool_version",),
                )
                if snapshot(run_dir) != before:
                    fail(f"stamp {label}: refusal modified run artifacts")


def failure_pairs(driver: Path) -> None:
    for label, builder, target, scope, expected_code, category, context in (
        ("empty-diff", build_empty, "HEAD", "src", 3, "empty-diff", ("target", "scope")),
        ("empty-control-scope", build_empty, "HEAD", "src\x07\x08", 3, "empty-diff", ("target", "scope")),
        ("empty-nonprintable-scope", build_empty, "HEAD", "src\u200b\ue000\U000e0001", 3, "empty-diff", ("target", "scope")),
        ("empty-nonprintable-target", build_nonprintable_ref, NONPRINTABLE_TARGET, "src", 3, "empty-diff", ("target", "scope")),
        ("git-failure", build_rich, "not-a-real-revision", ".", 2, "git", ("not-a-real-revision",)),
    ):
        with tempfile.TemporaryDirectory(prefix="cyril-s2hb-failure-") as temporary:
            root = Path(temporary)
            workspace, run_dir = root / "workspace", root / "rundir"
            builder(workspace)
            native = invoke_driver(driver, "gather", workspace, run_dir, target, scope)
            native_files = snapshot(run_dir)
            clear_tree(workspace)
            builder(workspace)
            clear_tree(run_dir)
            oracle = oracle_call(load_oracle(), "gather", workspace, run_dir, target, scope)
            compare_results(label, native, oracle, native_files, snapshot(run_dir), workspace=workspace)
            assert_error_output(f"{label} native", native, expected_code, category, context)
            assert_error_output(f"{label} oracle", oracle, expected_code, category, context)


def sequence_checks(driver: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-sequence-") as temporary:
        root = Path(temporary)
        workspace, run_dir = root / "workspace", root / "rundir"
        build_rich(workspace)
        first = invoke_driver(driver, "gather", workspace, run_dir, "auto", "src")
        if first.returncode != 0:
            fail(f"sequence seed failed: {first.stderr!r}")
        before = snapshot(run_dir)
        assert_gather_output("sequence seed native", first, before, repeated=False)
        with (workspace / "src" / "caller.rs").open("a", encoding="utf-8") as stream:
            stream.write("pub fn changed_after_gather() {}\n")
        second = invoke_driver(driver, "gather", workspace, run_dir, "auto", "src")
        if snapshot(run_dir) != before:
            fail("matching gather recomputed or rewrote existing artifacts after repository change")
        assert_gather_output("idempotent native", second, before, repeated=True)
        (run_dir / "facts" / "symbols.json").unlink()
        third = invoke_driver(driver, "gather", workspace, run_dir, "auto", "src")
        native_third_files = snapshot(run_dir)
        assert_gather_output("rebuild native", third, native_third_files, repeated=True)
        native_bad = invoke_driver(driver, "gather", workspace, run_dir, "HEAD~1", "src")
        native_bad_files = snapshot(run_dir)
        clear_tree(workspace)
        build_rich(workspace)
        clear_tree(run_dir)
        module = load_oracle()
        oracle_first = oracle_call(module, "gather", workspace, run_dir, "auto", "src")
        if oracle_first.returncode != 0:
            fail(f"oracle sequence seed failed: {oracle_first.stderr!r}")
        assert_gather_output("sequence seed oracle", oracle_first, snapshot(run_dir), repeated=False)
        with (workspace / "src" / "caller.rs").open("a", encoding="utf-8") as stream:
            stream.write("pub fn changed_after_gather() {}\n")
        oracle_second = oracle_call(module, "gather", workspace, run_dir, "auto", "src")
        compare_results("idempotent-gather", second, oracle_second, before, snapshot(run_dir), repeated=True, workspace=workspace)
        (run_dir / "facts" / "symbols.json").unlink()
        oracle_third = oracle_call(module, "gather", workspace, run_dir, "auto", "src")
        compare_results(
            "rebuild-missing-facts",
            third,
            oracle_third,
            native_third_files,
            snapshot(run_dir),
            repeated=True,
            workspace=workspace,
        )
        oracle_bad = oracle_call(module, "gather", workspace, run_dir, "HEAD~1", "src")
        compare_results("identity-refusal", native_bad, oracle_bad, native_bad_files, snapshot(run_dir), workspace=workspace)
        assert_error_output("identity native", native_bad, 2, "identity", ("different",))
        assert_error_output("identity oracle", oracle_bad, 2, "identity", ("different",))


def checker_sensitivity() -> None:
    """Keep the semantic checker bounded and prove its intended discriminators."""
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-sensitivity-") as temporary:
        root = Path(temporary)
        workspace, run_dir = root / "workspace", root / "rundir"
        build_canonical(workspace)
        result = oracle_call(load_oracle(), "gather", workspace, run_dir, "auto", "src src")
        if result.returncode != 0:
            fail(f"checker sensitivity seed failed: {result.stderr!r}")
        baseline = snapshot(run_dir)
        assert_run_layout(run_dir)

        def encode_json(value: object, *, crlf: bool = False) -> bytes:
            text = json.dumps(value, ensure_ascii=False, indent=7, sort_keys=True) + "\n"
            if crlf:
                text = text.replace("\n", "\r\n")
            return text.encode("utf-8")

        equivalent: dict[str, bytes] = {}
        for name, data in baseline.items():
            if name.endswith(".json"):
                equivalent[name] = encode_json(json_bytes(f"sensitivity {name}", data), crlf=True)
            elif name.endswith(".patch"):
                equivalent[name] = data
            elif name.startswith("facts/usages-"):
                body = normalize_frame(data, name).split("\n", 1)[1]
                equivalent[name] = ("An equivalent evidence-page heading\n" + body).replace("\n", "\r\n").encode("utf-8")
            else:
                equivalent[name] = normalize_frame(data, f"sensitivity {name}").replace("\n", "\r\n").encode("utf-8")
        compare_results(
            "checker-sensitivity-equivalent-format",
            result,
            result,
            baseline,
            equivalent,
            workspace=workspace,
        )

        def rejected(label: str, mutated: dict[str, bytes]) -> None:
            try:
                compare_results(label, result, result, baseline, mutated, workspace=workspace)
            except HarnessError:
                return
            fail(f"{label}: checker accepted a meaningful mutation")

        manifest = json_bytes("sensitivity manifest", baseline["manifest.json"])
        if not isinstance(manifest, dict):
            fail("checker sensitivity manifest is not an object")
        changed_value = dict(manifest)
        changed_value["total_files"] = changed_value["total_files"] + 1
        mutated = dict(baseline)
        mutated["manifest.json"] = encode_json(changed_value)
        rejected("checker-sensitivity-manifest-value", mutated)

        symbols = json_bytes("sensitivity symbols", baseline["facts/symbols.json"])
        if not isinstance(symbols, list) or len(symbols) < 2:
            fail("checker sensitivity fixture lacks an ordered symbol array")
        mutated = dict(baseline)
        mutated["facts/symbols.json"] = encode_json(list(reversed(symbols)))
        rejected("checker-sensitivity-array-order", mutated)

        patch = baseline["diff.patch"]
        altered = patch.replace(b"pub fn helper", b"pub fn hxxxxx", 1)
        if altered == patch:
            fail("checker sensitivity fixture lacks patch source content")
        mutated = dict(baseline)
        mutated["diff.patch"] = altered
        rejected("checker-sensitivity-patch-content", mutated)

        entries = manifest.get("files")
        if not isinstance(entries, list) or not entries:
            fail("checker sensitivity manifest lacks file status")
        changed_manifest = dict(manifest)
        changed_entries = [dict(entry) for entry in entries]
        changed_entries[0]["status"] = "D" if changed_entries[0].get("status") != "D" else "A"
        changed_manifest["files"] = changed_entries
        mutated = dict(baseline)
        mutated["manifest.json"] = encode_json(changed_manifest)
        rejected("checker-sensitivity-status", mutated)

def cli_smoke(cyril: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-cli-") as temporary:
        root = Path(temporary)
        workspace, run_dir, home = root / "workspace", root / "rundir", root / "home"
        build_rich(workspace)
        config = home / ".config" / "cyril" / "config.toml"
        put(config, b"[this is deliberately invalid")
        env = os.environ.copy()
        env["HOME"] = str(home)
        env["USERPROFILE"] = str(home)
        env["XDG_CONFIG_HOME"] = str(home / ".config")
        env.pop("TERM", None)
        env.pop("NO_COLOR", None)
        normal = run([str(cyril), "--help"], workspace, env)
        if normal.returncode != 0 or b"crtool" in normal.stdout.lower() + normal.stderr.lower():
            fail("ordinary help exposes crtool or failed")
        hidden = run([str(cyril), "crtool", "--help"], workspace, env)
        lower = hidden.stdout.lower() + hidden.stderr.lower()
        if b"diagnostics" in lower:
            fail("hidden crtool help exposes forbidden diagnostics")
        marker_paths = (
            workspace / "cyril.log",
            home / ".config" / "cyril" / "cyril.log",
            home / ".config" / "cyril" / "usage.sqlite3",
        )
        started = datetime_module.datetime.now(datetime_module.timezone.utc)
        actual = run(
            [
                str(cyril),
                "--agent-command=cyril-s2hb-nonexistent-91904",
                "--agent-engine=v2",
                "crtool",
                "gather",
                str(run_dir),
                "auto",
                "src",
            ],
            workspace,
            env,
        )
        finished = datetime_module.datetime.now(datetime_module.timezone.utc)
        if actual.returncode != 0:
            fail(f"real-clock CLI gather failed: {actual.stderr!r}")
        if any(path.exists() for path in marker_paths):
            fail("hidden gather crossed ordinary logging/config/usage startup")
        unexpected_home_files = [
            path for path in home.rglob("*") if path.is_file() and path != config
        ]
        if unexpected_home_files:
            fail(f"hidden gather created startup artifacts: {unexpected_home_files!r}")
        manifest = json_bytes("CLI manifest", (run_dir / "manifest.json").read_bytes())
        stamp = timestamp_instant(
            "CLI gathered_at",
            manifest.get("gathered_at") if isinstance(manifest, dict) else None,
        )
        if not started - datetime_module.timedelta(seconds=5) <= stamp <= finished + datetime_module.timedelta(seconds=5):
            fail(f"real-clock gathered_at is outside invocation bounds: {stamp.isoformat()}")
        gather_files = snapshot(run_dir)
        assert_run_layout(run_dir)
        assert_gather_output("CLI gather", actual, gather_files, repeated=False)
        actual_facts = run(
            [str(cyril), "crtool", "facts", str(run_dir)],
            workspace,
            env,
        )
        if actual_facts.returncode != 0:
            fail(f"real-binary CLI facts failed: {actual_facts.stderr!r}")
        cli_files = snapshot(run_dir)
        assert_run_layout(run_dir)
        assert_facts_semantics(cli_files)
        assert_facts_output("CLI facts", actual_facts, cli_files)
        if any(path.exists() for path in marker_paths):
            fail("hidden facts crossed ordinary logging/config/usage startup")
        unexpected_home_files = [
            path for path in home.rglob("*") if path.is_file() and path != config
        ]
        if unexpected_home_files:
            fail(f"hidden facts created startup artifacts: {unexpected_home_files!r}")
        clear_tree(run_dir)
        forbidden = run([str(cyril), "crtool", "diagnostics", str(run_dir), "true"], workspace, env)
        if forbidden.returncode == 0 or snapshot(run_dir):
            fail("forbidden diagnostics command was accepted or wrote artifacts")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("gather",), required=True)
    parser.add_argument("--cyril")
    parser.add_argument("--driver")
    args = parser.parse_args()
    if not args.cyril or not args.driver:
        fail("--phase gather requires --cyril and --driver")
    driver, cyril = Path(os.path.abspath(args.driver)), Path(os.path.abspath(args.cyril))
    if not driver.exists() or not cyril.exists():
        fail("--driver and --cyril must name existing executables")

    pair_gather("canonical", build_canonical, driver, "auto", "src src")
    pair_gather("rich-scoped", build_rich, driver, "auto", "src src")
    pair_gather("unicode-scope", build_rich, driver, "auto", "src\u00a0src\t")
    pair_facts("direct-facts", build_rich, driver, "auto", "src")
    pair_gather("auto-main-precedence", build_divergent_refs, driver, "auto", "src")
    pair_gather("auto-no-base", build_no_base, driver, "auto", "src")
    pair_gather("auto-dirty", build_dirty, driver, "auto", "src")
    pair_gather("range-head-warning", lambda path: build_rich(path, other=True), driver, "main...other", "src")
    pair_gather("binary-deleted", build_rich, driver, "auto", ".")
    pair_gather("caps-and-documents", build_caps, driver, "main...HEAD", "src")
    pair_gather("unsplit-page", build_unsplit_page, driver, "main...HEAD", "src")
    pair_gather("scale-budget", build_scale, driver, "main...HEAD", "src")
    failure_pairs(driver)
    sequence_checks(driver)
    native_stamp_refusal(driver, build_rich, "auto", "src")
    checker_sensitivity()
    cli_smoke(cyril)
    print("slice-A gather parity: differential fixtures and CLI boundary checks completed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except HarnessError as exc:
        print(f"parity: {exc}", file=sys.stderr)
        raise SystemExit(1)

#!/usr/bin/env python3
# /// script
# requires-python = ">=3.12"
# ///
"""Functional differential evidence for native gather, facts and diagnostics.

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
import signal
import time
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


def git(cwd: Path, *args: str, check: bool = True, env: dict[str, str] | None = None,
        input_data: bytes | None = None) -> bytes:
    result = run(["git", *args], cwd, env, input_data)
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
            relative = str(path.relative_to(root)).replace(os.sep, "/")
            # Native-only writer lock guarding manifest read-modify-write; it
            # coordinates steps and carries no review evidence.
            if relative == "manifest.lock":
                continue
            result[relative] = path.read_bytes()
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


def git_patch_trees(cwd: Path, patch: bytes) -> tuple[bytes, bytes]:
    """Let Git reconstruct changed paths, contents and modes, independent of patch spelling."""
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-patch-index-") as temporary:
        index = str(Path(temporary) / "index")
        # Pure mode changes need the real index's blob during ancestor creation.
        # Only subsequent operations select the private index; none writes the real index.
        git(cwd, "apply", f"--build-fake-ancestor={index}", input_data=patch)
        env = {**os.environ, "GIT_INDEX_FILE": index}
        before = git(cwd, "write-tree", env=env)
        git(cwd, "apply", "--cached", "--binary", env=env, input_data=patch)
        after = git(cwd, "write-tree", env=env)
        return before, after


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
    if "invalid target" in lower:
        return "invalid-target"
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
        assert_direct_git_semantics(workspace or Path.cwd(), native_files)


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


def assert_direct_git_semantics(workspace: Path, files: dict[str, bytes], scope: str | None = None) -> None:
    manifest = json_bytes("direct Git manifest", files.get("manifest.json", b""))
    if not isinstance(manifest, dict) or not isinstance(manifest.get("target"), str):
        fail("direct Git manifest lacks target")
    pathspec = scope.split() if scope is not None else manifest.get("scope")
    if not isinstance(pathspec, list) or not all(isinstance(path, str) for path in pathspec):
        fail("direct Git manifest lacks scope paths")
    root = Path(os.fsdecode(git(workspace, "rev-parse", "--show-toplevel")[:-1]))
    diff_args = ("diff", "--no-renames", "--no-relative", "--no-color", "--no-ext-diff",
                 "--no-textconv", "--binary", "--full-index", "--src-prefix=a/", "--dst-prefix=b/",
                 manifest["target"], "--")
    full = git(workspace, *diff_args, *(pathspec or ["."]))
    stored = files.get("diff.patch")
    if stored is None or git_patch_trees(root, stored) != git_patch_trees(root, full):
        fail("stored diff.patch does not represent the direct Git diff")
    entries = manifest.get("files")
    if not isinstance(entries, list):
        fail("direct Git manifest files is not an array")
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str) or not isinstance(entry.get("patch"), str):
            fail("direct Git manifest file record is malformed")
        path = os.fsdecode(bytes(entry["_raw_path_bytes"])) if "_raw_path_bytes" in entry else entry["path"]
        expected = git(root, *diff_args, ":(literal)" + path)
        actual = files.get(entry["patch"])
        if actual is None or git_patch_trees(root, actual) != git_patch_trees(root, expected):
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
                compare_results(label, result, result, mutated, baseline, workspace=workspace)
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
        for kind, original, replacement in (
            ("content", b"pub fn helper", b"pub fn hxxxxx"),
            ("path", b"src/helper.rs", b"src/hxxxxx.rs"),
            ("mode", b"new file mode 100644", b"new file mode 100755"),
        ):
            altered = patch.replace(original, replacement)
            if altered == patch:
                fail(f"checker sensitivity fixture lacks patch {kind} control")
            mutated = dict(baseline)
            mutated["diff.patch"] = altered
            rejected(f"checker-sensitivity-patch-{kind}", mutated)

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
        invalid_run = root / "invalid-target-run"
        outside_output = root / "option-target-output.patch"
        invalid = run(
            [str(cyril), "crtool", "gather", str(invalid_run),
             f"--output={outside_output}", "src"],
            workspace, env,
        )
        assert_error_output("CLI option-shaped target", invalid, 2, "invalid-target",
                            ("--output=", outside_output.name))
        if invalid_run.exists() or outside_output.exists():
            fail("CLI option-shaped target created run layout or injected Git output")

        leading_workspace = root / "leading-hyphen-workspace"
        leading_run = root / "leading-hyphen-run"
        base_repo(leading_workspace)
        put(leading_workspace / "-leading.rs", "// baseline\n")
        commit(leading_workspace, "track leading-hyphen scope", env)
        put(leading_workspace / "-leading.rs", "// baseline\npub fn leading_scope() {}\n")
        home_before = snapshot(home)
        leading = run(
            [str(cyril), "crtool", "gather", str(leading_run), "HEAD", "-leading.rs"],
            leading_workspace, env,
        )
        if leading.returncode != 0:
            fail(f"CLI leading-hyphen scope was not treated as path data: {leading.stderr!r}")
        leading_files = snapshot(leading_run)
        assert_gather_output("CLI leading-hyphen scope", leading, leading_files, repeated=False)
        leading_manifest = json_bytes("CLI leading-hyphen manifest", leading_files["manifest.json"])
        if (leading_manifest["scope"] != ["-leading.rs"] or leading_manifest["total_files"] != 1
                or [(entry["path"], entry["status"]) for entry in leading_manifest["files"]] != [("-leading.rs", "M")]):
            fail("CLI leading-hyphen scope selected the wrong tracked change")
        assert_direct_git_semantics(leading_workspace, leading_files, "-leading.rs")
        if snapshot(home) != home_before or (leading_workspace / "cyril.log").exists():
            fail("CLI leading-hyphen scope crossed ordinary startup")
        cli_cwd_smoke(cyril, root, workspace, home, env)


def cli_cwd_smoke(cyril: Path, root: Path, workspace: Path, home: Path, env: dict[str, str]) -> None:
    """Exercise the real hidden dispatcher, including its workspace/run mapping."""
    outside = root / "outside"
    outside.mkdir()
    home_before = snapshot(home)

    def invoke(label: str, launch: Path, flags: list[str], operation: str, rundir: str, scope: str = "") -> Captured:
        argv = [
            str(cyril),
            "--agent-command=cyril-s2hb-nonexistent-91904",
            "--agent-engine=v2",
            *flags,
            "crtool",
            operation,
            rundir,
        ]
        if operation == "gather":
            argv.extend(["auto", scope])
        result = run(argv, launch, env)
        if snapshot(home) != home_before or any(
            path.name in {"cyril.log", "usage.sqlite3"} for path in root.rglob("*")
        ):
            fail(f"{label}: hidden CLI crossed ordinary logging/config/usage startup")
        return result

    cases = (
        ("cwd-relative", outside, ["--cwd", "../workspace"], workspace, "selected-relative", "src"),
        ("cwd-absolute", outside, ["-d", str(workspace)], workspace, str(root / "selected-absolute"), "src"),
        ("cwd-subdirectory", outside, ["--cwd", "../workspace/src"], workspace / "src", "selected-subdir", "."),
        ("cwd-default", workspace, [], workspace, "selected-default", "src"),
    )
    for label, launch, flags, selected, argument, scope in cases:
        run_path = Path(argument)
        expected = run_path if run_path.is_absolute() else selected / run_path
        misplaced = launch / run_path if not run_path.is_absolute() else selected / run_path.name
        result = invoke(label, launch, flags, "gather", argument, scope)
        if result.returncode != 0:
            fail(f"{label}: selected-workspace CLI gather failed: {result.stderr!r}")
        if not (expected / "manifest.json").is_file():
            fail(f"{label}: run directory was not anchored at {expected}")
        if misplaced != expected and misplaced.exists():
            fail(f"{label}: CLI wrote a second run directory at {misplaced}")
        files = snapshot(expected)
        assert_run_layout(expected)
        assert_gather_output(label, result, files, repeated=False)
        manifest = json_bytes(f"{label} manifest", files["manifest.json"])
        if not isinstance(manifest, dict) or not isinstance(manifest.get("target"), str):
            fail(f"{label}: CLI manifest lacks target")
        if not isinstance(manifest.get("files"), list):
            fail(f"{label}: CLI manifest lacks file records")
        recorded = [entry.get("path") for entry in manifest["files"] if isinstance(entry, dict)]
        if not recorded or not all(isinstance(path, str) for path in recorded):
            fail(f"{label}: CLI manifest file records are malformed")
        scoped_names = {
            os.fsdecode(path)
            for path in git(selected, "diff", "--no-renames", "--name-only", "-z", manifest["target"], "--", scope).split(b"\0")
            if path
        }
        if not set(recorded) <= scoped_names:
            fail(f"{label}: CLI recorded paths outside selected caller-relative scope: {sorted(set(recorded) - scoped_names)!r}")
        assert_direct_git_semantics(selected, files, scope)

        repeated = invoke(label, launch, flags, "gather", argument, scope)
        if repeated.returncode != 0:
            fail(f"{label}: selected-workspace reuse failed: {repeated.stderr!r}")
        assert_gather_output(f"{label} reuse", repeated, snapshot(expected), repeated=True)
        if snapshot(expected) != files:
            fail(f"{label}: selected-workspace reuse rewrote artifacts")
        facts = invoke(label, launch, flags, "facts", argument)
        if facts.returncode != 0:
            fail(f"{label}: selected-workspace CLI facts failed: {facts.stderr!r}")
        facts_files = snapshot(expected)
        assert_run_layout(expected)
        assert_facts_output(f"{label} facts", facts, facts_files)
        if facts_files["diff.patch"] != files["diff.patch"]:
            fail(f"{label}: facts changed the selected gather patch")
        facts_manifest = json_bytes(f"{label} facts manifest", facts_files["manifest.json"])
        if not isinstance(facts_manifest, dict) or facts_manifest.get("files") != manifest["files"]:
            fail(f"{label}: facts changed the selected gather file records")

    nondirectory = root / "selected-file"
    put(nondirectory, "not a directory\n")
    for kind, selected, flag in (
        ("missing", root / "selected-missing", "--cwd"),
        ("nondirectory", nondirectory, "-d"),
    ):
        for operation in ("gather", "facts"):
            label = f"cwd-refusal-{kind}-{operation}"
            run_path = root / label
            before_files, before_dirs = snapshot(root), directory_snapshot(root)
            result = invoke(label, outside, [flag, str(selected)], operation, str(run_path), "src")
            text = normalize_frame(result.stderr, f"{label} stderr")
            if result.returncode != 2 or str(selected) not in text:
                fail(f"{label}: invalid selected cwd did not refuse with exit2 naming {str(selected)!r}: {result.returncode}, {result.stderr!r}")
            if run_path.exists() or snapshot(root) != before_files or directory_snapshot(root) != before_dirs:
                fail(f"{label}: invalid selected cwd refusal wrote artifacts")
    print("PASS CLI cwd: outside launch, relative/absolute/default/subdirectory scope, gather reuse/facts symmetry, directory refusal and no ordinary startup")



# B uses the same compiled flag-based consumer and the unmodified reference.
DIAGNOSTICS_ELAPSED = 2.6
DIAGNOSTICS_SCALE_BYTES = 32 * 1024 * 1024


def native_command(argv: list[str]) -> str:
    if os.name == "nt":
        return subprocess.list2cmdline(argv)
    import shlex
    return shlex.join(argv)


def native_units(value: str) -> list[int]:
    if os.name == "nt":
        raw = value.encode("utf-16-le", errors="surrogatepass")
        return [int.from_bytes(raw[index:index + 2], "little") for index in range(0, len(raw), 2)]
    return list(os.fsencode(value))


def native_units_text(units: list[int]) -> str:
    if os.name == "nt":
        raw = b"".join(unit.to_bytes(2, "little") for unit in units)
        return raw.decode("utf-16-le", errors="surrogatepass")
    return os.fsdecode(bytes(units))


def wait_file(path: Path, process: subprocess.Popen | None = None, seconds: float = 10) -> bytes:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            value = path.read_bytes()
            if value:
                return value
        except FileNotFoundError:
            pass
        if process is not None and process.poll() is not None:
            fail(f"fixture exited before handshake {path.name}: {process.returncode}")
        time.sleep(0.002)
    fail(f"fixture handshake deadline: {path}")


def challenge(root: Path, role: str) -> None:
    token = os.urandom(24).hex().encode("ascii")
    put(root / f"{role}.challenge", token)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            if (root / f"{role}.response").read_bytes() == token:
                return
        except FileNotFoundError:
            pass
        time.sleep(0.002)
    fail(f"{role} did not answer a fresh live challenge")




def native_pid_exists(pid: int) -> bool:
    if os.name != "nt":
        try:
            os.kill(pid, 0)
            return True
        except ProcessLookupError:
            return False
    import ctypes
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel.WaitForSingleObject.restype = wintypes.DWORD
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.OpenProcess(0x00100000, False, pid)
    if not handle:
        error = ctypes.get_last_error()
        if error == 87:  # ERROR_INVALID_PARAMETER: PID no longer exists.
            return False
        fail(f"cannot observe fixture cleanup PID {pid}: Windows error {error}")
    try:
        result = kernel.WaitForSingleObject(handle, 0)
        if result == 0xFFFFFFFF:
            fail(f"cannot wait for fixture cleanup PID {pid}")
        return result == 0x102
    finally:
        if not kernel.CloseHandle(handle):
            fail(f"cannot close cleanup observation handle for {pid}")


class FixtureCleanup:
    """Release fixture-owned roles before removing their isolated source tree.

    A release is cooperative, including on an assertion or outer-guard failure.
    It is not implicit descendant termination. The bounded fixture modes have
    their own 30-second guard; no role waits for inherited stream EOF.
    """
    def __init__(self):
        self.root: Path | None = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        if self.root is None or not self.root.exists():
            return False
        root = self.root
        for role in ("child", "holder", "sentinel"):
            put(root / f"{role}.release", b"fixture teardown release")
        if (root / "child.ready").exists():
            pid = receipt_value(root / "child.receipt")["pid"]
            deadline = time.monotonic() + 35
            while not (root / "child.exiting").exists() and native_pid_exists(pid):
                if time.monotonic() >= deadline:
                    fail("fixture-owned direct child did not accept teardown release")
                time.sleep(0.002)
        for ready, done in (("holder.ready", "holder.done"), ("sentinel.ready", "sentinel.done")):
            if (root / ready).exists():
                wait_file(root / done, seconds=35)
        return False


class NativeProcess:
    """Own one native process handle; never names or process trees."""
    def __init__(self, pid: int):
        self.pid = pid
        self.handle = None
        if os.name == "nt":
            import ctypes
            from ctypes import wintypes
            self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            self.kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
            self.kernel.OpenProcess.restype = wintypes.HANDLE
            self.kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
            self.kernel.WaitForSingleObject.restype = wintypes.DWORD
            self.kernel.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]
            self.kernel.TerminateProcess.restype = wintypes.BOOL
            self.kernel.CloseHandle.argtypes = [wintypes.HANDLE]
            self.kernel.CloseHandle.restype = wintypes.BOOL
            self.handle = self.kernel.OpenProcess(0x00100001, False, pid)
            if not self.handle:
                fail(f"cannot acquire live fixture process {pid}: {ctypes.get_last_error()}")

    def alive(self) -> bool:
        if os.name == "nt":
            result = self.kernel.WaitForSingleObject(self.handle, 0)
            if result == 0xFFFFFFFF:
                fail(f"cannot observe native process {self.pid}")
            return result == 0x102
        try:
            os.kill(self.pid, 0)
            return True  # Includes zombies: direct child must actually be reaped.
        except ProcessLookupError:
            return False

    def terminate(self) -> None:
        if not self.alive():
            return
        if os.name == "nt":
            if not self.kernel.TerminateProcess(self.handle, 99):
                fail(f"owned fixture cleanup could not terminate {self.pid}")
        else:
            try:
                os.kill(self.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def close(self) -> None:
        if self.handle is not None:
            if not self.kernel.CloseHandle(self.handle):
                fail(f"could not close owned process handle {self.pid}")
            self.handle = None


def fixture_data(root: Path, stdout: bytes = b"", stderr: bytes = b"", *, code: int = 0, sleep_ms: int = 0) -> None:
    root.mkdir(parents=True)
    put(root / "stdout.bin", stdout)
    put(root / "stderr.bin", stderr)
    put(root / "config.json", json.dumps({"exit_code": code, "sleep_ms": sleep_ms}))


def fixture_argv(driver: Path, root: Path, kind: str = "emit", args: tuple[str, ...] = ()) -> list[str]:
    return [str(driver), "fixture", "--root", str(root), "--kind", kind, "--", *args]


def diagnostics_argv(driver: Path, workspace: Path, run_dir: Path, command: str, receipt: Path,
                     *, timeout: str = "5", clock: str = "fixed", elapsed: float = DIAGNOSTICS_ELAPSED,
                     cancel: str = "none", ready: Path | None = None, linger: Path | None = None) -> list[str]:
    argv = [str(driver), "diagnostics", "--workspace", str(workspace), "--rundir", str(run_dir),
            "--command", command, "--receipt", str(receipt), "--clock", clock,
            "--timeout-seconds", timeout, "--cancel", cancel]
    if clock == "fixed":
        argv += ["--elapsed-seconds", str(elapsed)]
    if ready is not None:
        argv += ["--cancel-ready", str(ready)]
    if linger is not None:
        argv += ["--linger", str(linger)]
    return argv


def diagnostics_call(argv: list[str], workspace: Path, env: dict[str, str] | None = None,
                     input_data: bytes | None = None) -> Captured:
    try:
        result = subprocess.run(argv, cwd=workspace, env=env, input=input_data,
                                capture_output=True, timeout=40)
    except subprocess.TimeoutExpired:
        fail("diagnostics outer hang guard expired (not a latency qualification)")
    return Captured(result.returncode, result.stdout, result.stderr)


def seed_diagnostics(driver: Path, root: Path, *, scale: bool = False) -> tuple[Path, Path]:
    workspace, run_dir = root / "workspace", root / "rundir"
    build_canonical(workspace)
    if scale:
        for index in range(SCALE_FILES):
            put(workspace / "scale" / f"path-{index:03d}.txt", f"changed-{index}\n")
        finish_feature(workspace)
    gathered = invoke_driver(driver, "gather", workspace, run_dir, "main...HEAD", "scale" if scale else "src")
    if gathered.returncode != 0:
        fail(f"diagnostics gather prerequisite failed: {gathered.stderr!r}")
    manifest = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
    manifest["facts"]["fixture_unrelated"] = {"nested": [7, "unchanged"]}
    put(run_dir / "manifest.json", json.dumps(manifest))
    return workspace, run_dir


def receipt_value(path: Path) -> dict:
    value = json_bytes("diagnostics receipt", path.read_bytes())
    if not isinstance(value, dict):
        fail("diagnostics receipt is not an object")
    return value


def assert_outcome(receipt: dict, outcome: str, *, started: bool = True, code: int | None = None,
                   complete: bool | None = True) -> None:
    if receipt.get("outcome") != outcome or receipt.get("started") is not started or receipt.get("exit_code") != code:
        fail(f"typed diagnostics result differs: {receipt!r}; wanted {outcome}/{started}/{code}")
    if "capture_complete" not in receipt or receipt["capture_complete"] is not complete:
        fail(f"capture completeness receipt differs: {receipt.get('capture_complete')!r}; wanted {complete!r}")


# The generated status caption, not arbitrary prose: child output may
# legitimately contain either word, so completeness is checked where the
# operation itself states it.
CAPTURE_COMPLETE_CAPTION = "capture: complete"
CAPTURE_INCOMPLETE_CAPTION = "capture: incomplete"


def assert_capture_text(label: str, text: str, complete: bool) -> None:
    header = text.splitlines()[0].casefold() if text else ""
    expected = CAPTURE_COMPLETE_CAPTION if complete else CAPTURE_INCOMPLETE_CAPTION
    opposite = CAPTURE_INCOMPLETE_CAPTION if complete else CAPTURE_COMPLETE_CAPTION
    if expected not in header or opposite in header:
        fail(f"{label}: generated header has missing/conflicting capture status: {header!r}")


def assert_raw(run_dir: Path, stdout: bytes, stderr: bytes, *, complete: bool = True) -> bytes:
    raw = (run_dir / "facts" / "diagnostics-raw.txt").read_bytes()
    prefix = None
    for separator in (b"\n", b"\r\n"):
        candidate = stdout + separator + stderr
        if raw.startswith(candidate):
            prefix = candidate
            break
    if prefix is None:
        fail(f"ordered raw capture lost stdout+separator+stderr: received {len(raw)}, streams {len(stdout)}/{len(stderr)}")
    if complete:
        if raw != prefix:
            fail(f"complete raw capture carries generated bytes: {len(raw)} != {len(prefix)}")
    else:
        marker = raw[len(prefix):]
        if not marker or b"incomplete" not in marker.lower():
            fail(f"incomplete raw capture lacks its generated marker: {marker[:120]!r}")
    return raw


def status_meaning(status: object) -> tuple[str, int | None]:
    if not isinstance(status, str):
        fail("diagnostics status is not text")
    lower = status.casefold()
    if "cancel" in lower:
        return "Cancelled", None
    if "tim" in lower and "out" in lower:
        return "TimedOut", None
    if "clean" in lower:
        return "Clean", 0
    if "fail" in lower:
        match = re.search(r"exit(?:[_ ]code)?\s*[=:]?\s*(-?\d+)", status, re.IGNORECASE)
        if match is not None:
            return "Failed", int(match[1])
    fail(f"diagnostics status lacks terminal meaning: {status!r}")


def assert_diagnostics_artifacts(run_dir: Path, before: dict, command: str, outcome: str, code: int | None,
                                 elapsed: float, source_lines: list[str], expected_lines: list[str], matches: int,
                                 *, complete: bool = True) -> None:
    manifest = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
    facts = manifest.get("facts")
    if not isinstance(facts, dict) or facts.get("diagnostics") != "facts/diagnostics.txt":
        fail("diagnostics report metadata is absent")
    if status_meaning(facts.get("diagnostics_status")) != (outcome, code):
        fail("manifest diagnostics status disagrees with typed/native outcome")
    if facts.get("diagnostics_capture_complete") is not complete:
        fail(f"manifest capture completeness differs: {facts.get('diagnostics_capture_complete')!r}; wanted {complete!r}")
    restored = dict(manifest)
    restored["facts"] = dict(facts)
    restored["facts"].pop("diagnostics")
    restored["facts"].pop("diagnostics_status")
    restored["facts"].pop("diagnostics_capture_complete")
    if restored != before:
        fail("diagnostics changed unrelated manifest/facts metadata")
    text = normalize_frame((run_dir / "facts" / "diagnostics.txt").read_bytes(), "filtered diagnostics")
    assert_capture_text("filtered diagnostics", text, complete)
    rendered = text.splitlines()
    if command not in text or before["head"][:12] not in text:
        fail("filtered report lost command or manifest HEAD context")
    source_set = set(source_lines)
    selected = [line for line in rendered if line in source_set]
    if selected != expected_lines:
        fail(f"meaningful diagnostics evidence/order/caps differs: {selected[:3]!r}... ({len(selected)} lines)")
    headers = [line for line in rendered if line not in source_set and command not in line]
    if not any(re.search(rf"(?<!\d){matches}(?!\d)", line) and
               re.search(r"file|path|match", line, re.IGNORECASE) for line in headers):
        fail(f"filtered report lost total changed-path match count {matches}")
    seconds = [float(value) for line in headers for value in re.findall(r"(?<![\w.])(\d+(?:\.\d+)?)\s*s(?:econds)?\b", line)]
    expected_seconds = float(format(elapsed, ".0f"))
    if expected_seconds not in seconds:
        fail(f"filtered report lost rounded elapsed sample {elapsed}: {seconds!r}")
    if not any(status_meaning(line) == (outcome, code) for line in headers if
               re.search(r"clean|fail|cancel|tim.*out", line, re.IGNORECASE)):
        fail("filtered report lost meaningful terminal status")


def reference_diagnostics(module, workspace: Path, run_dir: Path, command: str, timeout: float,
                          elapsed: float, env: dict[str, str] | None) -> Captured:
    original_cwd, original_time = Path.cwd(), module.time
    original_env = os.environ.copy()
    samples = iter((100.0, 100.0 + elapsed))
    module.time = SimpleNamespace(time=lambda: next(samples))
    try:
        os.chdir(workspace)
        if env is not None:
            os.environ.clear()
            os.environ.update(env)
        return capture_oracle_call(lambda: module.cmd_diagnostics(SimpleNamespace(
            rundir=str(run_dir), command=command, timeout=timeout)))
    finally:
        os.chdir(original_cwd)
        module.time = original_time
        os.environ.clear()
        os.environ.update(original_env)


def compare_environment(receipt: dict, expected: dict, names: list[str]) -> None:
    actual = {tuple(entry["name"]): entry["value"] for entry in receipt["env"]}
    wanted = {tuple(entry["name"]): entry["value"] for entry in expected["env"]}
    for name in names:
        key = tuple(native_units(name.upper() if os.name == "nt" else name))
        if actual.get(key) != wanted.get(key):
            fail(f"native environment preservation/filtering differs for {name!r}")


def diagnostics_pair(driver: Path, label: str, *, stdout: bytes = b"", stderr: bytes = b"", code: int = 0,
                     args: tuple[str, ...] = (), timeout: str = "5", sleep_ms: int = 0,
                     elapsed: float = DIAGNOSTICS_ELAPSED, source_lines: list[str] | None = None,
                     expected_lines: list[str] | None = None, matches: int = 0,
                     executable: str = "absolute", raw_tail: str | None = None, scale: bool = False,
                     driver_stdin: bytes | None = None) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-diagnostics-") as temporary, FixtureCleanup() as cleanup:
        root = Path(temporary)
        workspace, run_dir = seed_diagnostics(driver, root, scale=scale)
        fixture = root / "fixture with spaces"
        cleanup.root = fixture
        fixture_data(fixture, stdout, stderr, code=code, sleep_ms=sleep_ms)
        chosen = driver
        env = os.environ.copy()
        names = ["CARGO_S2HB_EMPTY", "RUST_S2HB_EMPTY", "RUSTUP_S2HB_EMPTY", "CARGO_S2HB_KEEP",
                 "RUST_S2HB_KEEP", "S2HB_EMPTY", "S2HB_KEEP", "cargo_s2hb_lower", "rust_s2hb_lower"]
        env.update(dict(zip(names, ["", "", "", "cargo-nonempty", "rust-nonempty", "", "ordinary", "", ""])))
        if os.name != "nt":
            names += ["CARGO_\udcff", "\udcff_NATIVE", "NATIVE_VALUE"]
            env.update({"CARGO_\udcff": "", "\udcff_NATIVE": "native", "NATIVE_VALUE": "\udcfe"})
        if executable in ("quoted-space", "unquoted-space"):
            chosen = root / "has space" / driver.name
            chosen.parent.mkdir()
            shutil.copy2(driver, chosen)
        elif executable in ("relative", "path"):
            chosen = workspace / driver.name
            shutil.copy2(driver, chosen)
        elif executable in ("batch-cmd", "batch-bat"):
            if os.name != "nt":
                fail("explicit batch invocation is a native Windows qualification case")
            extension = "cmd" if executable == "batch-cmd" else "bat"
            chosen = root / "has space" / f"fixture.{extension}"
            chosen.parent.mkdir()
            # Requester-approved explicit native batch selection. The product
            # does not wrap arbitrary command strings in a shell. Its direct
            # child follows the normal std/Windows batch-dispatch semantics.
            child_prefix = subprocess.list2cmdline(fixture_argv(driver, fixture)[:-1])
            script = "@echo off\r\n" + child_prefix + " -- %*\r\nexit /b %errorlevel%\r\n"
            put(chosen, script.encode("utf-8"))
        argv = ([str(chosen), *args] if executable in ("batch-cmd", "batch-bat")
                else fixture_argv(chosen, fixture, args=args))
        if executable == "relative":
            argv[0] = "./" + chosen.name
        elif executable == "path":
            env["PATH"] = str(workspace) + os.pathsep + env.get("PATH", "")
            argv[0] = chosen.name
        command = native_command(argv)
        if executable == "unquoted-space":
            if os.name != "nt":
                fail("unquoted spaced executable is a Windows native command case")
            command = str(chosen) + " " + subprocess.list2cmdline(argv[1:])
        if raw_tail is not None:
            command += " " + raw_tail
        before = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
        before_names = set(snapshot(run_dir))
        reference_dir = root / "reference"
        shutil.copytree(run_dir, reference_dir)
        module = load_oracle()
        reference = reference_diagnostics(module, workspace, reference_dir, command,
                                         1800 if timeout == "default" else float(timeout), elapsed, env)
        if reference.returncode != 0:
            fail(f"{label}: reference diagnostics failed: {reference.stderr!r}")
        reference_receipt = receipt_value(fixture / "child.receipt") if (fixture / "child.receipt").exists() else None
        reference_launches = (fixture / "launches").read_text().splitlines() if (fixture / "launches").exists() else []
        if timeout != "0" and len(reference_launches) != 1:
            fail(f"{label}: reference did not launch exactly once")
        if driver_stdin is not None:
            control = run(fixture_argv(driver, fixture, args=args), workspace, env, input_data=driver_stdin)
            if control.returncode != 0 or receipt_value(fixture / "child.receipt").get("stdin") != "data":
                fail(f"{label}: nonempty-input control did not reach the fixture's stdin")
        for name in ("child.receipt", "launches", "child.ready", "child.exiting", "emit.pid"):
            path = fixture / name
            if path.exists():
                path.unlink()
        result_path = root / "outcome.json"
        native = diagnostics_call(diagnostics_argv(driver, workspace, run_dir, command, result_path,
                                                   timeout=timeout, elapsed=elapsed), root, env, driver_stdin)
        if native.returncode != 0:
            fail(f"{label}: native diagnostics failed: {native.stderr!r}")
        result = receipt_value(result_path)
        reference_manifest = json.loads((reference_dir / "manifest.json").read_text(encoding="utf-8"))
        outcome, expected_code = status_meaning(reference_manifest["facts"]["diagnostics_status"])
        assert_outcome(result, outcome, code=expected_code, complete=True)
        assert_capture_text(f"{label} native stdout", normalize_frame(native.stdout, f"{label} native stdout"), True)
        if result.get("elapsed_seconds") != elapsed or result.get("timeout_seconds") != (1800 if timeout == "default" else float(timeout)):
            fail(f"{label}: explicit/default clock or timeout option receipt differs")
        if timeout != "0":
            child = receipt_value(fixture / "child.receipt")
            if child.get("stdin") != "eof":
                fail(f"{label}: native child did not observe null stdin EOF: {child.get('stdin')!r}")
            if len((fixture / "launches").read_text().splitlines()) != 1:
                fail(f"{label}: native successful command was launched more than once")
            if child["argv"] != reference_receipt["argv"] or (raw_tail is None and child["argv"] != list(args)):
                fail(f"{label}: native argv differs from independent/native reference")
            for role, observed in (("native", child), ("reference", reference_receipt)):
                cwd = Path(native_units_text(observed["cwd"]))
                try:
                    same_workspace = cwd.samefile(workspace)
                except OSError as exc:
                    fail(f"{label}: {role} cwd {str(cwd)!r} cannot be compared to reviewed workspace {str(workspace)!r}: {exc}")
                if not same_workspace:
                    fail(f"{label}: {role} cwd {str(cwd)!r} is not reviewed workspace {str(workspace)!r}")
            compare_environment(child, reference_receipt, names)
            assert_raw(run_dir, stdout, stderr)
        elif result["outcome"] != "TimedOut":
            fail("zero timeout did not produce a started timeout")
        reference_text = normalize_frame((reference_dir / "facts" / "diagnostics.txt").read_bytes(), label)
        source_lines = source_lines or []
        expected_lines = expected_lines or []
        if [line for line in reference_text.splitlines() if line in set(source_lines)] != expected_lines:
            fail(f"{label}: reference meaningful fixture evidence differs")
        assert_diagnostics_artifacts(run_dir, before, command, outcome, expected_code, elapsed,
                                     source_lines, expected_lines, matches)
        reference_after = json.loads((reference_dir / "manifest.json").read_text(encoding="utf-8"))
        native_after = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
        # Capture completeness is a new native contract fact the functional
        # reference predates; its exact value is asserted above instead.
        native_after["facts"].pop("diagnostics_capture_complete", None)
        reference_after["facts"].pop("diagnostics_capture_complete", None)
        native_after["facts"]["diagnostics_status"] = reference_after["facts"]["diagnostics_status"]
        if native_after != reference_after:
            fail(f"{label}: functional reference manifest differs")
        expected_names = before_names | {"facts/diagnostics.txt", "facts/diagnostics-raw.txt"}
        if set(snapshot(run_dir)) != expected_names:
            fail(f"{label}: owned temporary capture cleanup left files in run directory")
        print(f"PASS B {label}: {outcome}; reference/native argv, metadata and literal raw evidence")



def diagnostics_no_write(driver: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-prelaunch-") as temporary, FixtureCleanup() as cleanup:
        root = Path(temporary)
        workspace, run_dir = seed_diagnostics(driver, root)
        fixture = root / "fixture"
        cleanup.root = fixture
        fixture_data(fixture, b"would-launch-out", b"would-launch-err")
        valid_command = native_command(fixture_argv(driver, fixture))
        seed = snapshot(run_dir)
        seed_dirs = directory_snapshot(run_dir)
        errors = [("empty", "", "InvalidCommand", None),
                  ("space-tab-only", " \t ", "InvalidCommand", None),
                  ("blank", " \t\r\n", "CannotStart" if os.name == "nt" else "InvalidCommand", None),
                  ("cannot-start", str(root / "guaranteed-missing-program"), "CannotStart", None),
                  ("missing-stamp", valid_command, "MissingStamp", "remove-stamp"),
                  ("wrong-stamp", valid_command, "StampMismatch", "wrong-stamp"),
                  ("bad-facts", valid_command, "InvalidManifest", "bad-facts"),
                  ("missing-manifest", valid_command, "Io", "missing-manifest"),
                  ("corrupt-manifest", valid_command, "Json", "corrupt-manifest")]
        if os.name != "nt":
            errors += [("unterminated-single", "'broken", "InvalidCommand", None),
                       ("unterminated-double", '"broken', "InvalidCommand", None),
                       ("trailing-escape", "missing\\", "InvalidCommand", None)]
        for label, command, kind, mutation in errors:
            clear_tree(run_dir)
            for name, data in seed.items():
                put(run_dir / name, data)
            for name in seed_dirs:
                (run_dir / name).mkdir(parents=True, exist_ok=True)
            manifest_path = run_dir / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            if mutation == "remove-stamp":
                manifest.pop("crtool_version")
            elif mutation == "wrong-stamp":
                manifest["crtool_version"] = "fixture-mismatch"
            elif mutation == "bad-facts":
                manifest["facts"] = ["not-an-object"]
            if mutation in ("remove-stamp", "wrong-stamp", "bad-facts"):
                put(manifest_path, json.dumps(manifest))
            elif mutation == "missing-manifest":
                manifest_path.unlink()
            elif mutation == "corrupt-manifest":
                put(manifest_path, b"{broken")
            before, before_dirs = snapshot(run_dir), directory_snapshot(run_dir)
            receipt = root / "result.json"
            result = diagnostics_call(diagnostics_argv(driver, workspace, run_dir, command, receipt), workspace)
            value = receipt_value(receipt)
            if (result.returncode != 2 or value.get("error_kind") != kind or not result.stderr or result.stdout
                    or value.get("capture_complete") is not None):
                fail(f"{label}: typed prelaunch refusal/context differs: {result!r}, {value!r}")
            if snapshot(run_dir) != before or directory_snapshot(run_dir) != before_dirs or (fixture / "launches").exists():
                fail(f"{label}: prelaunch error wrote reports/captures or launched a child")
            print(f"PASS B prelaunch-{label}: {kind}; no writes, no launch")
        clear_tree(run_dir)
        for name, data in seed.items():
            put(run_dir / name, data)
        for name in seed_dirs:
            (run_dir / name).mkdir(parents=True, exist_ok=True)
        result_path = root / "pre-cancel.json"
        before, before_dirs = snapshot(run_dir), directory_snapshot(run_dir)
        result = diagnostics_call(diagnostics_argv(driver, workspace, run_dir, valid_command, result_path,
                                                   cancel="pre"), workspace)
        value = receipt_value(result_path)
        if result.returncode != 0:
            fail("prelaunch cancellation unexpectedly became an error")
        assert_outcome(value, "Cancelled", started=False, complete=None)
        if snapshot(run_dir) != before or directory_snapshot(run_dir) != before_dirs or (fixture / "launches").exists():
            fail("pre-cancel wrote artifacts or launched fixture")
        if not value.get("cancelled"):
            fail("pre-cancel control was not set")
        # Positive control on the identical command/root prevents vacuous no-launch.
        positive_path = root / "positive.json"
        positive = diagnostics_call(diagnostics_argv(driver, workspace, run_dir, valid_command, positive_path), workspace)
        if positive.returncode != 0 or not (fixture / "launches").exists():
            fail("pre-cancel positive command failed to launch")
        assert_outcome(receipt_value(positive_path), "Clean", code=0)
        assert_raw(run_dir, b"would-launch-out", b"would-launch-err")
        print("PASS B pre-cancel: false started, no writes; identical command positive control launched")


def wait_receipt(path: Path, runner: subprocess.Popen, label: str) -> dict:
    """Collect the consumer's receipt while it is still alive.

    The lingering consumer is the only owner able to prove its capture read
    ends closed; waiting for its exit first would make that observation
    vacuous, so the receipt is polled with an exit guard instead.
    """
    deadline = time.monotonic() + 40
    while time.monotonic() < deadline:
        if path.exists():
            return receipt_value(path)
        if runner.poll() is not None:
            fail(f"{label}: driver exited before writing its result receipt")
        time.sleep(0.002)
    fail(f"{label}: driver result receipt deadline")


def diagnostics_lifecycle(driver: Path, label: str, *, kind: str, outcome: str,
                          cancel: str = "none", timeout: str = "5", code: int = 0,
                          complete: bool = True) -> None:
    with (
        tempfile.TemporaryDirectory(prefix="cyril-s2hb-owned-process-") as temporary,
        tempfile.TemporaryFile() as outer_stdout,
        tempfile.TemporaryFile() as outer_stderr,
        FixtureCleanup() as cleanup,
    ):
        root = Path(temporary)
        workspace, run_dir = seed_diagnostics(driver, root)
        fixture = root / "fixture"
        cleanup.root = fixture
        holder_case = kind in ("exit-holder", "live-holder")
        fixture_data(fixture, b"direct-out", b"direct-err", code=code,
                     sleep_ms=300 if kind == "emit" else 0)
        sentinel = subprocess.Popen([str(driver), "fixture", "--root", str(fixture), "--kind", "sentinel"],
                                    cwd=workspace, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                    stderr=subprocess.DEVNULL)
        runner = None
        controls: list[NativeProcess] = []
        direct = None
        try:
            wait_file(fixture / "sentinel.ready", sentinel)
            sentinel_control = NativeProcess(int(wait_file(fixture / "sentinel.pid", sentinel)))
            controls.append(sentinel_control)
            challenge(fixture, "sentinel")
            before = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
            before_names = set(snapshot(run_dir))
            before_dirs = directory_snapshot(run_dir)
            command = native_command(fixture_argv(driver, fixture, kind=kind))
            receipt = root / "result.json"
            late_receipt = fixture / "holder.late"
            driver_release = fixture / "driver.release"
            argv = diagnostics_argv(driver, workspace, run_dir, command, receipt, timeout=timeout,
                                    cancel=cancel, ready=fixture / "cancel-now" if cancel == "live" else None,
                                    linger=driver_release if holder_case else None)
            # Observe the same compiled consumer's exit, not inherited outer pipe EOF.
            # File-backed collection leaves holder/sentinel liveness proof independent.
            runner = subprocess.Popen(argv, cwd=workspace, stdin=subprocess.DEVNULL,
                                      stdout=outer_stdout, stderr=outer_stderr)
            wait_file(fixture / "child.ready", runner)
            if receipt_value(fixture / "child.receipt").get("stdin") != "eof":
                fail(f"{label}: direct child did not observe null stdin EOF")
            direct_pid = int(wait_file(fixture / f"{kind}.pid", runner))
            if kind != "exit-holder":
                direct = NativeProcess(direct_pid)
                controls.append(direct)
                if not direct.alive():
                    fail("direct child was not live before cancellation/timeout control")
            holder = None
            if holder_case:
                wait_file(fixture / "holder.ready")
                holder = NativeProcess(int(wait_file(fixture / "holder.pid")))
                controls.append(holder)
                challenge(fixture, "holder")
                if not holder.alive():
                    fail("inherited holder is absent before terminal observation")
            challenge(fixture, "sentinel")
            if cancel == "live":
                challenge(fixture, "child")
                put(fixture / "cancel-now", b"cancel after all live controls")
            if label == "signal":
                os.kill(direct_pid, signal.SIGTERM)
            expected_out = b"direct-out" + (b"holder-out" if holder_case else b"")
            expected_err = b"direct-err" + (b"holder-err" if holder_case else b"")
            raw = None
            if holder_case:
                returned = wait_receipt(receipt, runner, label)
                if "error_kind" in returned:
                    fail(f"{label}: incomplete holder case refused instead of returning a result: {returned!r}")
                raw = assert_raw(run_dir, expected_out, expected_err, complete=False)
                if holder is None or not holder.alive():
                    fail("inherited holder died before the late-write observation")
                challenge(fixture, "holder")
                put(fixture / "holder.append", b"late write after the operation returned")
                wait_file(late_receipt, runner)
                late = receipt_value(late_receipt)
                if runner.poll() is not None:
                    fail(f"{label}: consumer exited before capture resource-release proof")
                if late.get("stdout") != "broken-pipe" or late.get("stderr") != "broken-pipe":
                    fail(f"{label}: late holder write did not observe released capture read ends: {late!r}")
                if (run_dir / "facts/diagnostics-raw.txt").read_bytes() != raw:
                    fail("late holder writes mutated retained raw evidence")
                if b"LATE-HOLDER-OUT" in raw or b"LATE-HOLDER-ERR" in raw:
                    fail("post-return holder output was collected into the retained capture")
                put(driver_release, b"closure observed while consumer remained alive")
            try:
                runner.wait(timeout=40)
            except subprocess.TimeoutExpired:
                fail("owned diagnostics native fixture hit outer hang guard")
            outer_stdout.seek(0)
            outer_stderr.seek(0)
            stdout, stderr = outer_stdout.read(), outer_stderr.read()
            if runner.returncode != 0:
                fail(f"{label}: diagnostics native lifecycle failed: {stderr!r}")
            value = receipt_value(receipt)
            assert_outcome(value, outcome, code=code if outcome == "Failed" else 0 if outcome == "Clean" else None,
                           complete=complete)
            assert_capture_text(f"{label} native stdout", normalize_frame(stdout, f"{label} native stdout"), complete)
            if cancel == "after-exit" and not value.get("cancelled"):
                fail("after-exit cancellation control was not exercised")
            if direct is not None and direct.alive():
                fail("direct child was not terminated/reaped when diagnostics returned")
            if os.name != "nt":
                try:
                    os.kill(direct_pid, 0)
                except ProcessLookupError:
                    pass
                else:
                    fail("Unix direct child still exists (including unreaped zombie)")
            if not sentinel_control.alive():
                fail("diagnostics terminated the unrelated sentinel")
            challenge(fixture, "sentinel")
            if raw is None:
                raw = assert_raw(run_dir, expected_out, expected_err, complete=complete)
            source_lines = [expected_out.decode(), expected_err.decode()]
            assert_diagnostics_artifacts(run_dir, before, command, outcome,
                                         code if outcome == "Failed" else 0 if outcome == "Clean" else None,
                                         DIAGNOSTICS_ELAPSED, source_lines, source_lines, 0, complete=complete)
            if set(snapshot(run_dir)) != before_names | {"facts/diagnostics.txt", "facts/diagnostics-raw.txt"}:
                fail("owned capture files remained in run directory after terminal result")
            if directory_snapshot(run_dir) != before_dirs:
                fail("capture cleanup changed the owned run layout")
            if holder is not None:
                if not holder.alive():
                    fail("direct-child termination also killed inherited holder")
                challenge(fixture, "holder")
                put(fixture / "holder.release", b"fixture owned release after diagnostics return")
                wait_file(fixture / "holder.done")
                released = receipt_value(fixture / "holder.release-late")
                if released.get("stdout") != "broken-pipe" or released.get("stderr") != "broken-pipe":
                    fail(f"holder release writes did not observe closed read ends: {released!r}")
                if (run_dir / "facts/diagnostics-raw.txt").read_bytes() != raw:
                    fail("late inherited writes mutated retained raw evidence")
            put(fixture / "sentinel.release", b"fixture owned release")
            if sentinel.wait(timeout=10) != 0:
                fail("sentinel failed cooperative fixture cleanup")
            print(f"PASS B {label}: {outcome}; direct child gone; live sentinel/holder controls; independent streams/cleanup")
        finally:
            # These are fixture-owned releases, not product descendant killing.
            for role in ("holder", "sentinel", "child"):
                put(fixture / f"{role}.release", b"error-path fixture release")
            put(fixture / "driver.release", b"error-path consumer release")
            if runner is not None and runner.poll() is None:
                runner.kill()
                runner.wait(timeout=10)
            if sentinel.poll() is None:
                try:
                    sentinel.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    sentinel.kill()
                    sentinel.wait(timeout=10)
            for control in controls:
                try:
                    if control is direct and control.alive():
                        control.terminate()
                finally:
                    control.close()


def diagnostics_system_clock(driver: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-system-clock-") as temporary, FixtureCleanup() as cleanup:
        root = Path(temporary)
        workspace, run_dir = seed_diagnostics(driver, root)
        fixture = root / "fixture"
        cleanup.root = fixture
        fixture_data(fixture, b"actual-clock-out", b"actual-clock-err", sleep_ms=1200)
        command = native_command(fixture_argv(driver, fixture))
        receipt = root / "clock.json"
        before = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
        started = time.monotonic()
        result = diagnostics_call(diagnostics_argv(driver, workspace, run_dir, command, receipt,
                                                   clock="system"), workspace)
        outer_elapsed = time.monotonic() - started
        value = receipt_value(receipt)
        if result.returncode != 0:
            fail(f"actual system-clock diagnostics failed: {result.stderr!r}")
        assert_outcome(value, "Clean", code=0, complete=True)
        assert_capture_text("system-clock stdout", normalize_frame(result.stdout, "system-clock stdout"), True)
        elapsed = value.get("elapsed_seconds")
        if value.get("clock") != "system" or not isinstance(elapsed, (int, float)) or not 1.2 <= elapsed <= outer_elapsed:
            fail(f"actual SystemReviewClock sample outside native process bounds: {value!r}, outer={outer_elapsed}")
        assert_raw(run_dir, b"actual-clock-out", b"actual-clock-err", complete=True)
        assert_diagnostics_artifacts(run_dir, before, command, "Clean", 0, elapsed,
                                     ["actual-clock-out", "actual-clock-err"], ["actual-clock-out", "actual-clock-err"], 0,
                                     complete=True)
        print(f"PASS B actual-system-clock: child delay 1.2s <= real elapsed {elapsed:.3f}s <= outer {outer_elapsed:.3f}s")





def diagnostics_explicit_shell(driver: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-explicit-shell-") as temporary, FixtureCleanup() as cleanup:
        root = Path(temporary)
        workspace, run_dir = seed_diagnostics(driver, root)
        fixture = root / "fixture"
        cleanup.root = fixture
        fixture_data(fixture, b"explicit-shell-out", b"explicit-shell-err")
        child_command = native_command(fixture_argv(driver, fixture, args=("shell selected explicitly",)))
        if os.name == "nt":
            shell = shutil.which("powershell.exe")
            if shell is None:
                fail("native Windows explicit-shell prerequisite powershell.exe is missing")
            # Explicit PowerShell selects the program; diagnostics adds no shell.
            arguments = fixture_argv(driver, fixture, args=("shell selected explicitly",))
            script = "& " + " ".join("'" + value.replace("'", "''") + "'" for value in arguments)
            script += "; exit $LASTEXITCODE"
            command = native_command([shell, "-NoProfile", "-NonInteractive", "-Command", script])
        else:
            command = native_command(["/bin/sh", "-c", "exec " + child_command])
        before = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
        reference_dir = root / "reference"
        shutil.copytree(run_dir, reference_dir)
        reference = reference_diagnostics(load_oracle(), workspace, reference_dir, command, 5, DIAGNOSTICS_ELAPSED, None)
        if reference.returncode != 0:
            fail(f"explicit native shell reference failed: {reference.stderr!r}")
        reference_child = receipt_value(fixture / "child.receipt")
        (fixture / "launches").unlink()
        receipt = root / "outcome.json"
        native = diagnostics_call(diagnostics_argv(driver, workspace, run_dir, command, receipt), workspace)
        if native.returncode != 0:
            fail(f"explicit native shell diagnostics failed: {native.stderr!r}")
        assert_outcome(receipt_value(receipt), "Clean", code=0, complete=True)
        assert_capture_text("explicit-shell stdout", normalize_frame(native.stdout, "explicit-shell stdout"), True)
        child = receipt_value(fixture / "child.receipt")
        if child.get("stdin") != "eof":
            fail(f"explicit native shell child did not observe null stdin EOF: {child.get('stdin')!r}")
        if child["argv"] != ["shell selected explicitly"] or child["argv"] != reference_child["argv"]:
            fail("explicit native shell did not receive literal fixture arguments")
        if len((fixture / "launches").read_text().splitlines()) != 1:
            fail("explicit native shell fixture launched more than once")
        assert_raw(run_dir, b"explicit-shell-out", b"explicit-shell-err", complete=True)
        assert_diagnostics_artifacts(run_dir, before, command, "Clean", 0, DIAGNOSTICS_ELAPSED,
                                     ["explicit-shell-out", "explicit-shell-err"],
                                     ["explicit-shell-out", "explicit-shell-err"], 0, complete=True)
        print("PASS B explicit-native-shell: explicitly selected shell/reference, once-only literal child receipt")


def diagnostics_matrix(driver: Path) -> None:
    hostile = ("", "two words", 'quote"inside', "back\\slash", "semi;colon", "$HOME", "a|b", "*", "#literal", "nonascii-λ")
    for executable in ("absolute", "relative", "path", "quoted-space"):
        diagnostics_pair(driver, f"native-command-{executable}", args=hostile, executable=executable)
    if os.name == "nt":
        diagnostics_pair(driver, "native-command-unquoted-space", args=hostile, executable="unquoted-space")
        diagnostics_pair(driver, "untouched-windows-tail", raw_tail=r'"one two" "" "quote\"inside" tail\\')
        diagnostics_pair(driver, "unsigned-windows-status", code=-1073741819)
        diagnostics_pair(driver, "explicit-native-cmd-batch", executable="batch-cmd", code=23,
                         args=("one two", "literal"), stdout=b"batch-cmd-out", stderr=b"batch-cmd-err",
                         source_lines=["batch-cmd-out", "batch-cmd-err"],
                         expected_lines=["batch-cmd-out", "batch-cmd-err"])
        diagnostics_pair(driver, "explicit-native-bat-batch", executable="batch-bat", code=23,
                         args=("one two", "literal"), stdout=b"batch-bat-out", stderr=b"batch-bat-err",
                         source_lines=["batch-bat-out", "batch-bat-err"],
                         expected_lines=["batch-bat-out", "batch-bat-err"])
    else:
        diagnostics_pair(driver, "posix-shlex-grammar", raw_tail='\'\' "" \'two words\' "back\\\\slash" escaped\\ space a\\;b \'#literal\' "line\nbreak"')
    diagnostics_explicit_shell(driver)
    diagnostics_pair(driver, "clean-empty-default", timeout="default")
    diagnostics_pair(driver, "noninteractive-stdin-with-nonempty-driver-input",
                     driver_stdin=b"must-not-reach-diagnostics-child\n")
    diagnostics_pair(driver, "stdout-only", stdout=b"only-out", source_lines=["only-out"], expected_lines=["only-out"])
    diagnostics_pair(driver, "stderr-only", stderr=b"only-err", source_lines=["only-err"], expected_lines=["only-err"])
    captions = ["child says capture: incomplete", "child says capture: complete"]
    diagnostics_pair(driver, "child-capture-caption-decoys", stdout=("\n".join(captions) + "\n").encode(),
                     source_lines=captions, expected_lines=captions)
    diagnostics_pair(driver, "failed-nonzero", stdout=b"failed-out", stderr=b"failed-err", code=7,
                     source_lines=["failed-out", "failed-err"], expected_lines=["failed-out", "failed-err"])
    invalid_out, invalid_err = b"x" * 131_072 + b"\xff", b"y" * 131_072 + b"\xfe"
    decoded = [invalid_out.decode(errors="replace"), invalid_err.decode(errors="replace")]
    diagnostics_pair(driver, "dual-131073-invalid-utf8", stdout=invalid_out, stderr=invalid_err,
                     source_lines=decoded, expected_lines=decoded)
    path = "src/helper.rs"
    match_lines = [f"B-MATCH-{index:03d} {path if index % 2 == 0 else path.replace('/', chr(92))}:{index}" for index in range(201)]
    tail = [f"B-TAIL-{index:02d}" for index in range(16)]
    all_lines = match_lines + tail
    diagnostics_pair(driver, "201-matches-16-tail", stdout=("\r\n".join(match_lines) + "\r\n").encode(),
                     stderr=("\n".join(tail) + "\n").encode(), source_lines=all_lines,
                     expected_lines=match_lines[:200] + tail[-15:], matches=201)
    boundaries = ["B-CR src/helper.rs:1", "B-CRLF src/helper.rs:2", "B-NEL src/helper.rs:3", "B-LS src/helper.rs:4", "B-PS src/helper.rs:5", "B-LAST"]
    boundary_bytes = (boundaries[0] + "\r" + boundaries[1] + "\r\n" + boundaries[2] + "\u0085" +
                      boundaries[3] + "\u2028" + boundaries[4] + "\u2029" + boundaries[5]).encode()
    diagnostics_pair(driver, "unicode-cr-crlf-line-boundaries", stdout=boundary_bytes,
                     source_lines=boundaries, expected_lines=boundaries[:5] + boundaries, matches=5)
    for count in (1, 200):
        lines = [f"B-BOUNDARY-{index:03d} src/helper.rs:{index}" for index in range(count)]
        diagnostics_pair(driver, f"match-boundary-{count}", stdout=("\n".join(lines) + "\n").encode(),
                         source_lines=lines, expected_lines=lines + lines[-15:], matches=count)
    for count in (1, 15, 16):
        lines = [f"B-TAIL-BOUNDARY-{index:02d}" for index in range(count)]
        diagnostics_pair(driver, f"tail-boundary-{count}", stdout=("\n".join(lines) + "\n").encode(),
                         source_lines=lines, expected_lines=lines[-15:])
    for elapsed in (0.0, 0.4, 0.5, 1.5, 123456.7):
        diagnostics_pair(driver, f"elapsed-{elapsed}", elapsed=elapsed)
    diagnostics_pair(driver, "zero-timeout", timeout="0", sleep_ms=1000)
    diagnostics_pair(driver, "explicit-timeout-partial", timeout="1.5", sleep_ms=3000,
                     stdout=b"partial-out", stderr=b"partial-err", source_lines=["partial-out", "partial-err"],
                     expected_lines=["partial-out", "partial-err"])
    scale_lines = [f"B-SCALE-{index:03d} scale/path-{index:03d}.txt:{index}" for index in range(SCALE_FILES)]
    scale_tail = [f"B-SCALE-TAIL-{index:02d}" for index in range(16)]
    prefix = ("\n".join(scale_lines) + "\n").encode()
    ending = ("\n" + "\n".join(scale_tail) + "\n").encode()
    scale_out = prefix + b"O" * (DIAGNOSTICS_SCALE_BYTES - len(prefix) - 1) + b"\n"
    scale_err = b"E" * (DIAGNOSTICS_SCALE_BYTES - len(ending)) + ending
    diagnostics_pair(driver, "32MiB-each-stream-100paths", stdout=scale_out, stderr=scale_err, scale=True,
                     source_lines=scale_lines + scale_tail, expected_lines=scale_lines + scale_tail[-15:], matches=100)
    diagnostics_no_write(driver)
    diagnostics_lifecycle(driver, "live-cancel", kind="wait", outcome="Cancelled", cancel="live")
    diagnostics_lifecycle(driver, "after-exit-cancel", kind="emit", outcome="Clean", cancel="after-exit")
    diagnostics_lifecycle(driver, "cancel-inherited-holder", kind="live-holder", outcome="Cancelled",
                          cancel="live", complete=False)
    diagnostics_lifecycle(driver, "timeout-inherited-holder", kind="live-holder", outcome="TimedOut",
                          timeout="3", complete=False)
    diagnostics_lifecycle(driver, "exited-inherited-holder", kind="exit-holder", outcome="Failed", code=7,
                          complete=False)
    if os.name != "nt":
        diagnostics_lifecycle(driver, "signal", kind="wait", outcome="Failed", code=-signal.SIGTERM)
    diagnostics_system_clock(driver)
    print("B diagnostics matrix completed; bounded-read mutation attribution belongs to actual private helper test")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("gather", "diagnostics"), required=True)
    parser.add_argument("--cyril")
    parser.add_argument("--driver")
    args = parser.parse_args()
    if not args.cyril or not args.driver:
        fail("both phases require --cyril and --driver")
    driver, cyril = Path(os.path.abspath(args.driver)), Path(os.path.abspath(args.cyril))
    if not driver.exists() or not cyril.exists():
        fail("--driver and --cyril must name existing executables")

    if args.phase == "diagnostics":
        diagnostics_matrix(driver)
        cli_smoke(cyril)
        return 0

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

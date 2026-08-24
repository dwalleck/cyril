#!/usr/bin/env python3
"""Read-only probe for Git-common-directory project identity."""

from pathlib import Path
import sys


def resolve_project(workspace: Path) -> tuple[Path, Path]:
    display_path = workspace.resolve(strict=True)
    cursor = display_path
    while True:
        dot_git = cursor / ".git"
        if dot_git.is_dir():
            return dot_git.resolve(strict=True), display_path
        if dot_git.is_file():
            marker = dot_git.read_text(encoding="utf-8").strip()
            prefix = "gitdir: "
            if not marker.startswith(prefix):
                raise ValueError(f"invalid .git file: {dot_git}")
            git_dir = (dot_git.parent / marker[len(prefix) :]).resolve(strict=True)
            common_marker = git_dir / "commondir"
            if common_marker.is_file():
                common = common_marker.read_text(encoding="utf-8").strip()
                return (git_dir / common).resolve(strict=True), display_path
            return git_dir, display_path
        if cursor.parent == cursor:
            return display_path, display_path
        cursor = cursor.parent


def main() -> None:
    for label, raw_path in zip(sys.argv[1::2], sys.argv[2::2], strict=True):
        project_id, display_path = resolve_project(Path(raw_path))
        print(f"{label}\t{project_id}\t{display_path}")


if __name__ == "__main__":
    main()

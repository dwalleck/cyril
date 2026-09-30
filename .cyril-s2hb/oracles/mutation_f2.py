#!/usr/bin/env python3
"""A/B qualification for PR146 source ownership and native consumer fences.

Default mode exercises actual source CI dispatch and policy, not Rust behavior.
--runtime also compiles mutants and executes the native CI step.
Known F19: post-start operational errors can classify as FAIL. Runtime verdicts
are not automated semantic-mutation proof; see the scoped A1 design exception.
"""
import argparse
from contextlib import contextmanager
from pathlib import Path
import io
import os
import subprocess
import sys
import tarfile
import tempfile

REVIEWED_HEAD = "b4e79c17bacb98b850235c28195957bdcfeddac3"
CHECKER_REL = ".cyril-s2hb/oracles/check_shape.py"
CI_REL = ".github/workflows/ci.yml"
LIB_REL = "crates/cyril-core/src/lib.rs"
BRIDGE_REL = "crates/cyril-core/src/protocol/bridge.rs"
HOST_SHELL_REL = "crates/cyril-core/src/protocol/kas/host_shell.rs"
BASE_REF = "refs/remotes/origin/fixture"
BEFORE_REF = "refs/qualification/before"
HELPER_REL = ".cyril-s2hb/oracles/rust_source.py"
EXAMPLE_REL = "crates/cyril-core/examples/review_prefix.rs"
RUNTIME_STEP = "Qualify resolved native prefix"
RUNTIME_EXPECTED = {
    "pr-owned-proof": "PASS",
    "decoy-handle": "FAIL",
    "discarded-projected-handle": "FAIL",
    "early-success-return": "FAIL",
    "shadowed-ok": "FAIL",
    "unrelated-handle-field": "PASS",
    "unrelated-raw-return": "PASS",
    "unrelated-closure-return": "PASS",
}
GETTER = """    pub fn review_shell(&self) -> Option<crate::review::ShellDialect> {
        self.review_shell
    }"""
GETTER_DOC = "    /// Dialect of the same host shell resolved for this bridge's KAS session.\n"
ASSIGNMENT = """    handle.review_shell = {
        #[cfg(feature = "kas")]
        {
            host_shell.as_ref().map(|shell| shell.review_shell())
        }
        #[cfg(not(feature = "kas"))]
        {
            None
        }
    };"""

# This file is integrated at .cyril-s2hb/oracles/mutation_f2.py. Do not derive
# ROOT from the temporary handoff directory used while preparing this file.
SCRIPT = Path(__file__).resolve()
ROOT = SCRIPT.parents[2]
CURRENT_CHECKER = ROOT / CHECKER_REL
CURRENT_CI = ROOT / CI_REL


class Case:
    def __init__(
        self,
        name,
        path,
        needle,
        replacement,
        description,
        expected,
        event="pull_request",
        divergent=False,
        additional_edits=(),
        fixture_copies=(),
    ):
        self.name = name
        self.path = path
        self.needle = needle
        self.replacement = replacement
        self.description = description
        self.expected = expected  # old CI, old full, current CI, current full
        self.event = event
        self.divergent = divergent
        self.additional_edits = additional_edits
        self.fixture_copies = fixture_copies


CASES = (
    Case(
        "bridge-getter-none",
        BRIDGE_REL,
        GETTER,
        """    pub fn review_shell(&self) -> Option<crate::review::ShellDialect> {
        None
    }""",
        "bridge getter returns None",
        ("FAIL", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "resolved-assignment-none",
        BRIDGE_REL,
        ASSIGNMENT,
        """    handle.review_shell = None;""",
        "resolved-shell assignment is replaced with None",
        ("FAIL", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "host-shell-mapping-swapped",
        HOST_SHELL_REL,
        """            ShellKind::Posix => ShellDialect::Posix,
            ShellKind::Fish => ShellDialect::Fish,""",
        """            ShellKind::Posix => ShellDialect::Fish,
            ShellKind::Fish => ShellDialect::Posix,""",
        "HostShell Posix/Fish mapping is swapped",
        ("FAIL", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "unrelated-bridge-constant",
        BRIDGE_REL,
        "const COMMAND_CAPACITY: usize = 32;",
        "const COMMAND_CAPACITY: usize = 33;",
        "unrelated bridge constant/body change remains allowed in wiring-only mode",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "forbidden-main-owner",
        "crates/cyril/src/main.rs",
        "mod app;\n",
        "mod app;\n\nfn review_gather_body() {}\n",
        "original C10 named mutation must still fail both full checkers; main-only CI dispatch is unchanged",
        ("SKIP", "FAIL", "SKIP", "FAIL"),
    ),
    Case(
        "commented-getter", BRIDGE_REL, GETTER, "    /*\n" + GETTER + "\n    */",
        "a block comment is not a callable getter",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "conditional-getter", BRIDGE_REL, GETTER, "    #[cfg(any())]\n" + GETTER,
        "a cfg-disabled getter does not establish the public seam",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "raw-conditional-getter", BRIDGE_REL, GETTER, "    #[r#cfg(any())]\n" + GETTER,
        "Rust raw identifiers cannot hide a disabled getter",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "raw-inner-conditional-attribute", BRIDGE_REL, GETTER_DOC + GETTER,
        "}\nimpl BridgeHandle {\n    #![r#cfg_attr(all(), r#cfg(any()))]\n"
        + GETTER_DOC + GETTER + "\n}\nimpl BridgeHandle {",
        "raw conditional attributes remove an inner owner just like ordinary identifiers",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "live-raw-getter", BRIDGE_REL, GETTER, GETTER.replace("review_shell", "r#review_shell"),
        "a live raw-identifier getter and field access retain the same public seam",
        ("FAIL", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "specialized-owner", BRIDGE_REL, GETTER_DOC + GETTER,
        "}\nimpl BridgeHandle<1> {\n" + GETTER_DOC + GETTER + "\n}\nimpl BridgeHandle {",
        "a getter on a different generic specialization is not the returned handle's seam",
        ("FAIL", "FAIL", "FAIL", "FAIL"),
        additional_edits=(("pub struct BridgeHandle {", "pub struct BridgeHandle<const N: usize = 0> {"),),
    ),
    Case(
        "shebang-conditional-file", LIB_REL, "pub mod commands;",
        "#!/usr/bin/env rust-script\n#![cfg(any())]\npub mod commands;",
        "a shebang cannot hide the cfg that removes the crate",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "bom-conditional-file", LIB_REL, "pub mod commands;",
        "\ufeff#![cfg(any())]\npub mod commands;",
        "a UTF-8 BOM cannot hide a root conditional",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "live-bom-shebang", LIB_REL, "pub mod commands;",
        '\ufeff#!/usr/bin/env rust-script\n#! /* trivia */ [doc = "live crate"]\npub mod commands;',
        "BOM, shebang and nonconditional inner metadata leave the export live",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "spaced-inner-cfg", LIB_REL, "pub mod commands;",
        "#! /* trivia */ [cfg(any())]\npub mod commands;",
        "whitespace and comments do not turn an inner attribute into a shebang",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "operational-census-error", CHECKER_REL, "def main():",
        'def main():\n    raise RuntimeError("injected operational failure")',
        "deliberate crash control: execution failure is UNKNOWN, never policy FAIL",
        ("UNKNOWN", "UNKNOWN", "UNKNOWN", "UNKNOWN"),
    ),
    Case(
        "decoy-handle", BRIDGE_REL,
        "    let (mut handle, mut channels) = create_channel_pair();\n" + ASSIGNMENT,
        "    let (bridge_handle, mut channels) = create_channel_pair();\n"
        "    struct Decoy { review_shell: Option<crate::review::ShellDialect> }\n"
        "    let mut handle = Decoy { review_shell: None };\n" + ASSIGNMENT
        + "\n    let _observed_decoy = handle.review_shell;",
        "the store must target the bridge returned to the caller",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
        additional_edits=(("    Ok(handle)\n}", "    Ok(bridge_handle)\n}"),),
    ),
    Case(
        "discarded-projected-handle", BRIDGE_REL, "    Ok(handle)\n}",
        "    Ok({ let _observed = handle.review_shell; create_channel_pair().0 })\n}",
        "returning a fresh handle discards the otherwise-correct projection",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "early-success-return", BRIDGE_REL,
        "    let host_shell = resolve_host_shell(&config)?;",
        "    if config.engine == AgentEngine::Kas {\n        return Ok(create_channel_pair().0);\n    }\n"
        "    let host_shell = resolve_host_shell(&config)?;",
        "an earlier successful return cannot bypass the projection",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "shadowed-ok", BRIDGE_REL,
        "    let host_shell = resolve_host_shell(&config)?;",
        "    use self::replacement_ok as Ok;\n    let host_shell = resolve_host_shell(&config)?;",
        "name resolution can replace the final projected handle despite exact source tokens",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
        additional_edits=(
            ("pub fn spawn_bridge(", "fn replacement_ok(_: BridgeHandle) -> crate::Result<BridgeHandle> {\n"
             "    std::result::Result::Ok(create_channel_pair().0)\n}\n\npub fn spawn_bridge("),
            ("                Ok(runtime) => {", "                std::result::Result::Ok(runtime) => {"),
            ("                            Ok(()) => None,", "                            std::result::Result::Ok(()) => None,"),
        ),
    ),
    Case(
        "unrelated-handle-field", BRIDGE_REL,
        "    let disconnect_tx = channels.notification_tx.clone();",
        '    tracing::debug!(handle = "bridge constructed");\n'
        "    let disconnect_tx = channels.notification_tx.clone();",
        "a macro field label is not a use of the protected handle binding",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "unrelated-raw-return", BRIDGE_REL,
        "    let disconnect_tx = channels.notification_tx.clone();",
        '    let r#return = "bridge constructed";\n    tracing::debug!("{}", r#return);\n'
        "    let disconnect_tx = channels.notification_tx.clone();",
        "a raw identifier is not return control flow",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "unrelated-closure-return", BRIDGE_REL,
        "    let disconnect_tx = channels.notification_tx.clone();",
        '    let label = |flag| { if flag { return "ready"; } "waiting" };\n'
        '    tracing::debug!("{}", label(true));\n'
        "    let disconnect_tx = channels.notification_tx.clone();",
        "an unrelated closure's early return does not bypass the projection",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "unrelated-spawn-log", BRIDGE_REL,
        "    let disconnect_tx = channels.notification_tx.clone();",
        '    tracing::debug!("bridge constructed");\n    let disconnect_tx = channels.notification_tx.clone();',
        "unrelated function work does not require freezing the whole spawn body",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "redirected-export", LIB_REL, "pub mod review;",
        '#[r#path = "alternate_review.rs"]\n#[doc = "different owner"]\npub mod review;',
        "the required export must resolve to the approved review/mod.rs owner",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
        fixture_copies=(("crates/cyril-core/src/review/mod.rs", "crates/cyril-core/src/alternate_review.rs"),),
    ),
    Case(
        "documented-export", LIB_REL, "pub mod review;",
        '#[doc = "same owner"]\npub mod review;',
        "nonconditional documentation does not redirect the module",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "misowned-getter", BRIDGE_REL, GETTER_DOC + GETTER,
        "}\npub struct OtherHandle {\n    review_shell: Option<crate::review::ShellDialect>,\n}\n"
        "impl OtherHandle {\n" + GETTER_DOC + GETTER + "\n}\nimpl BridgeHandle {",
        "the same signature on another type is not BridgeHandle's getter",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "literal-getter", BRIDGE_REL, GETTER_DOC + GETTER,
        '    pub const SOURCE_EXAMPLE: &str = r#"\n' + GETTER + '\n"#;',
        "a raw-string example is not a getter",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "multiline-normal-literal-getter", BRIDGE_REL, GETTER_DOC + GETTER,
        '    pub const SOURCE_EXAMPLE: &str = "\n' + GETTER + '\n";',
        "a multiline non-raw Rust string is not a getter",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "public-bridge-field", BRIDGE_REL,
        "    review_shell: Option<crate::review::ShellDialect>,",
        "    pub review_shell: Option<crate::review::ShellDialect>,",
        "lexical scoping retains the existing private-field requirement",
        ("FAIL", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "conditional-impl", BRIDGE_REL, GETTER_DOC + GETTER,
        "}\n#[cfg(any())]\nimpl BridgeHandle {\n" + GETTER_DOC + GETTER
        + "\n}\nimpl BridgeHandle {",
        "a getter in a disabled impl does not establish the seam",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "inner-conditional-impl", BRIDGE_REL, GETTER_DOC + GETTER,
        "}\nimpl BridgeHandle {\n    #![cfg(any())]\n" + GETTER_DOC + GETTER
        + "\n}\nimpl BridgeHandle {",
        "an inner cfg removes its getter-only impl",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "inner-conditional-attribute-impl", BRIDGE_REL, GETTER_DOC + GETTER,
        '}\nimpl BridgeHandle {\n    #![doc = "scope"]\n'
        "    #![cfg_attr(all(), cfg(any()))]\n" + GETTER_DOC + GETTER
        + "\n}\nimpl BridgeHandle {",
        "inner cfg_attr is found after nonconditional inner metadata",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "inner-doc-impl", BRIDGE_REL, GETTER_DOC + GETTER,
        '}\nimpl BridgeHandle {\n    #![doc = "cfg(any()) is text"]\n'
        + GETTER_DOC + GETTER + "\n}\nimpl BridgeHandle {",
        "nonconditional inner metadata leaves the getter live",
        ("PASS_WIRING", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "conditional-source-file", LIB_REL, "pub mod commands;",
        "#![cfg(any())]\npub mod commands;",
        "a root inner cfg removes the required export",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "conditional-attribute-getter", BRIDGE_REL, GETTER,
        "    #[cfg_attr(all(), cfg(any()))]\n" + GETTER,
        "cfg_attr cannot conditionally remove required wiring",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "unrelated-lexical-content", BRIDGE_REL, GETTER,
        GETTER + "\n    pub fn unrelated<'a>(&self, text: &'a str) -> &'a str {\n"
        "        /* outer /* nested } */ tail */\n"
        "        let _character = '}'; let _byte = b'{';\n"
        '        let _example = r###" /* not a comment */\n' + GETTER + '\n"###;\n'
        "        text\n    }",
        "unrelated nested comments, raw strings, chars and lifetimes do not shadow live wiring",
        ("FAIL", "FAIL", "PASS_WIRING", "FAIL"),
    ),
    Case(
        "commented-assignment", BRIDGE_REL,
        "    let (mut handle, mut channels) = create_channel_pair();\n" + ASSIGNMENT,
        "    let (handle, mut channels) = create_channel_pair();\n    /*\n" + ASSIGNMENT + "\n    */",
        "a commented store does not carry the resolved shell; remove its now-unused mut too",
        ("PASS_WIRING", "FAIL", "FAIL", "FAIL"),
    ),
    Case(
        "push-getter-none", BRIDGE_REL, GETTER, GETTER.replace("self.review_shell", "None"),
        "a main push compares against the pre-push revision, not origin/main at HEAD",
        ("SKIP", "FAIL", "FAIL", "FAIL"), "push",
    ),
    Case(
        "push-unrelated-constant", BRIDGE_REL,
        "const COMMAND_CAPACITY: usize = 32;", "const COMMAND_CAPACITY: usize = 33;",
        "a push preserves unrelated-parent wiring-only acceptance",
        ("SKIP", "FAIL", "PASS_WIRING", "FAIL"), "push",
    ),
    Case(
        "push-owned-proof", CHECKER_REL, "#!/usr/bin/env python3\n",
        "#!/usr/bin/env python3\n# Qualification-only comment.\n",
        "a pushed owned-path change selects the full guard",
        ("SKIP", "PASS_FULL", "PASS_FULL", "PASS_FULL"), "push",
    ),
    Case(
        "pr-owned-proof", CHECKER_REL, "#!/usr/bin/env python3\n",
        "#!/usr/bin/env python3\n# Qualification-only comment.\n",
        "a PR owned-path change retains full-guard precedence",
        ("PASS_FULL", "PASS_FULL", "PASS_FULL", "PASS_FULL"),
    ),
    Case(
        "divergent-push-owned-proof", CHECKER_REL, "#!/usr/bin/env python3\n",
        "#!/usr/bin/env python3\n# Qualification-only comment.\n",
        "unchanged parent work at divergent push endpoints is not an increment delta",
        ("SKIP", "FAIL", "PASS_FULL", "PASS_FULL"), "push", divergent=True,
    ),
)


def run_process(argv, cwd, *, check=False, text=False, env=None):
    result = subprocess.run(
        argv,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=text,
        env=env,
    )
    if check and result.returncode:
        stderr = result.stderr if text else result.stderr.decode(errors="replace")
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(argv)}\n{stderr}")
    return result


def git_show(path):
    result = run_process(
        ["git", "show", f"{REVIEWED_HEAD}:{path}"],
        ROOT,
        check=True,
    )
    return result.stdout.decode("utf-8")


def git(cwd, *args, check=True):
    result = run_process(["git", *args], cwd, check=check, text=True)
    return result.stdout.strip()


def extract_step(ci, name="Review prefix ownership fence"):
    """Extract an actual workflow shell body without retyping its commands."""
    step = f"      - name: {name}\n"
    start = ci.index(step)
    run = ci.index("        run: |\n", start) + len("        run: |\n")
    body = []
    for line in ci[run:].splitlines(keepends=True):
        if line.startswith("      - name: "):
            break
        if line.startswith("          "):
            body.append(line[10:])
        elif line.strip():
            raise ValueError("review-prefix shell body has unexpected YAML indentation")
        else:
            body.append(line)
    if not body:
        raise ValueError("review-prefix shell body is empty")
    return "".join(body)


def assert_established_prefix_wiring(archive_files):
    """Reject a fixture base that predates the reviewed integration surface."""
    required = {
        LIB_REL: "pub mod review;",
        BRIDGE_REL: "review_shell: Option<crate::review::ShellDialect>,",
        HOST_SHELL_REL: "pub(crate) fn review_shell(&self) -> crate::review::ShellDialect",
    }
    for path, marker in required.items():
        if marker not in archive_files[path]:
            raise RuntimeError(
                f"{REVIEWED_HEAD} does not contain established prefix wiring in {path}"
            )
    if "handle.review_shell = {" not in archive_files[BRIDGE_REL]:
        raise RuntimeError(
            f"{REVIEWED_HEAD} does not contain the established bridge assignment"
        )


def archive_reviewed_tree():
    result = run_process(
        ["git", "archive", "--format=tar", REVIEWED_HEAD],
        ROOT,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


@contextmanager
def fixture_tree(archive, checker, ci, case, helpers=None):
    """Create one isolated base+mutation tree for exactly one case/variant."""
    with tempfile.TemporaryDirectory(prefix=f"cyril-f2-{case.name}-") as directory:
        tree = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as tar:
            tar.extractall(tree)

        run_process(["git", "init", "-q"], tree, check=True)
        git(tree, "config", "user.name", "F2 mutation harness")
        git(tree, "config", "user.email", "f2-harness@example.invalid")
        (tree / CHECKER_REL).parent.mkdir(parents=True, exist_ok=True)
        (tree / CI_REL).parent.mkdir(parents=True, exist_ok=True)
        (tree / CHECKER_REL).write_text(checker, encoding="utf-8")
        (tree / CI_REL).write_text(ci, encoding="utf-8")
        for path, contents in (helpers or {}).items():
            (tree / path).parent.mkdir(parents=True, exist_ok=True)
            (tree / path).write_text(contents, encoding="utf-8")
        # Fixture-only sources exist on both baselines before the mutation.
        for source, destination in case.fixture_copies:
            (tree / destination).write_bytes((tree / source).read_bytes())
        git(tree, "add", "-A")
        git(tree, "commit", "-qm", f"fixture base ({case.name})")
        base = git(tree, "rev-parse", "HEAD")
        git(tree, "update-ref", BASE_REF, base)
        git(tree, "update-ref", BEFORE_REF, base)
        before = base
        if case.divergent:
            # Both endpoint trees retain the same unrelated parent work, but
            # their commits diverge. Only the case mutation differs afterward.
            for label in ("before", "head"):
                git(tree, "checkout", "--detach", base)
                bridge = tree / BRIDGE_REL
                source = bridge.read_text(encoding="utf-8")
                needle = "const COMMAND_CAPACITY: usize = 32;"
                if source.count(needle) != 1:
                    raise RuntimeError("divergent fixture cannot locate parent control")
                bridge.write_text(source.replace(needle, "const COMMAND_CAPACITY: usize = 33;", 1), encoding="utf-8")
                git(tree, "add", BRIDGE_REL)
                git(tree, "commit", "-qm", f"shared parent work ({label})")
                if label == "before":
                    before = git(tree, "rev-parse", "HEAD")
                    git(tree, "update-ref", BEFORE_REF, before)

        target = tree / case.path
        original = target.read_text(encoding="utf-8")
        mutated = original
        for needle, replacement in ((case.needle, case.replacement), *case.additional_edits):
            if mutated.count(needle) != 1:
                raise RuntimeError(
                    f"{case.name}: expected one mutation needle in {case.path}, "
                    f"found {mutated.count(needle)}"
                )
            mutated = mutated.replace(needle, replacement, 1)
        if mutated == original:
            raise RuntimeError(f"{case.name}: mutation did not change {case.path}")
        target.write_text(mutated, encoding="utf-8")
        git(tree, "add", case.path)
        git(tree, "commit", "-qm", f"mutation ({case.name})")
        changed = git(tree, "diff", "--name-only", f"{before}..HEAD").splitlines()
        if changed != [case.path]:
            raise RuntimeError(
                f"{case.name}: fixture has more than one production mutation: {changed}"
            )
        if case.event == "push":
            # Checkout's default-branch ref already names the pushed revision.
            git(tree, "update-ref", BASE_REF, "HEAD")
        yield tree


def run_ci(tree, ci, event="pull_request"):
    environment = os.environ.copy()
    environment.update(
        {
            "REVIEW_BASE": "fixture",
            "GITHUB_BASE_REF": "fixture",
            "GITHUB_EVENT_NAME": event,
            "REVIEW_BEFORE": git(tree, "rev-parse", BEFORE_REF),
        }
    )
    body = extract_step(ci)
    return run_process(
        ["bash", "-e", "-u", "-o", "pipefail", "-c", body],
        tree,
        text=True,
        env=environment,
    )


def run_full_checker(tree, exact=False):
    return run_process(
        [
            sys.executable,
            CHECKER_REL,
            "--phase",
            "prefix",
            "--exact-base" if exact else "--base",
            BEFORE_REF,
        ],
        tree,
        text=True,
    )


def combined_output(result):
    return (result.stdout or "") + (result.stderr or "")


def verdict(result):
    output = combined_output(result)
    if "FAIL C10: cannot complete source census:" in output:
        return "UNKNOWN"
    if result.returncode == 0 and "No review-prefix ownership changes" in output:
        return "SKIP"
    if result.returncode == 0 and "PASS C10: integration wiring only" in output:
        return "PASS_WIRING"
    if result.returncode == 0 and "PASS C10:" in output:
        return "PASS_FULL"
    if result.returncode != 0 and "FAIL C10:" in output:
        return "FAIL"
    return "UNKNOWN"


def localized_diagnostics(result):
    lines = [line.strip() for line in combined_output(result).splitlines() if line.strip()]
    selected = [
        line
        for line in lines
        if (
            "No review-prefix ownership changes" in line
            or "PASS C10:" in line
            or "FAIL C10:" in line
            or "protected-parent body changed" in line
            or "review_shell" in line
            or "resolved-shell assignment" in line
            or "HostShell" in line
        )
    ]
    if not selected:
        selected = lines[-3:]
    return " | ".join(selected) if selected else "(no diagnostic output)"


def report(label, result):
    print(f"    {label}: exit={result.returncode} verdict={verdict(result)}")
    print(f"      diagnostics: {localized_diagnostics(result)}")



def run_case(case, archive, old_checker, old_ci, new_checker, new_ci, helpers):
    print(f"CASE {case.name}: {case.description}")
    observations = []
    for label, checker, ci, support in (
        ("old", old_checker, old_ci, {}),
        ("new", new_checker, new_ci, helpers),
    ):
        with fixture_tree(archive, checker, ci, case, support) as tree:
            for mode, result in (
                ("CI step", run_ci(tree, ci, case.event)),
                ("full checker", run_full_checker(tree, exact=label == "new" and case.event == "push")),
            ):
                report(f"{label} {mode}", result)
                observations.append(verdict(result))
    actual = tuple(observations)
    if actual != case.expected:
        print(f"    UNEXPECTED: expected={case.expected}, actual={actual}")
        return False
    return True


def runtime_verdict(result, executions):
    lines = combined_output(result).splitlines()
    started = sum(line.startswith("RUN C8:") for line in lines)
    passed = sum(line.startswith("PASS C8:") for line in lines)
    failed = any(line.startswith("FAIL C8:") for line in lines)
    if result.returncode == 0 and started == passed == executions and not failed:
        return "PASS"
    if result.returncode != 0 and started and failed:
        return "FAIL"
    return "UNKNOWN"


def child_role_refused(tree, target, environment):
    marker = tree / "child-role-marker"
    child_environment = environment | {"CYRIL_REVIEW_PREFIX_MARKER": str(marker)}
    executable = Path(target) / "debug/examples" / (
        "review_prefix.exe" if os.name == "nt" else "review_prefix"
    )
    parent_args = ["powershell", "windows-powershell"] if os.name == "nt" else ["bash", "posix"]
    result = run_process([executable, *parent_args], tree, text=True, env=child_environment)
    refused = result.returncode != 0 and "RUN C8:" not in combined_output(result) and not marker.exists()
    print(f"Child role control: exit={result.returncode}, refused={refused}", flush=True)
    print(combined_output(result), flush=True)
    return refused


def run_runtime_cases(archive, old_ci, checker, ci, helpers):
    if f"      - name: {RUNTIME_STEP}\n" in old_ci:
        raise RuntimeError("pinned pre-repair workflow unexpectedly has a runtime step")
    body = extract_step(ci, RUNTIME_STEP)  # A missing current step is an error.
    print("Old native CI: SKIP_RUNTIME (absent at pinned reviewed head)")
    print("WARNING F19: runtime FAIL can be operational; these verdicts are not semantic-mutation acceptance evidence.")
    cases = {case.name: case for case in CASES}
    failures = 0
    with tempfile.TemporaryDirectory(prefix="cyril-prefix-target-") as target:
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = target
        environment["RUNNER_OS"] = "Windows" if os.name == "nt" else (
            "macOS" if sys.platform == "darwin" else "Linux"
        )
        executions = 2 if os.name == "nt" else 1
        # A fresh healthy fixture after all mutations proves restored execution,
        # including protection against Cargo's archived-mtime artifact reuse.
        for name in (*RUNTIME_EXPECTED, "pr-owned-proof"):
            case = cases[name]
            with fixture_tree(archive, checker, ci, case, helpers) as tree:
                os.utime(tree / LIB_REL, None)
                result = run_process(
                    ["bash", "-e", "-u", "-o", "pipefail", "-c", body],
                    tree, text=True, env=environment,
                )
                actual = runtime_verdict(result, executions)
                print(f"RUNTIME {name}: exit={result.returncode} verdict={actual}", flush=True)
                print(combined_output(result), flush=True)
                if actual != RUNTIME_EXPECTED[name]:
                    failures += 1
                    print(f"    UNEXPECTED: expected={RUNTIME_EXPECTED[name]}")
                if name == "pr-owned-proof" and actual == "PASS":
                    if not child_role_refused(tree, target, environment):
                        failures += 1
    print(f"Runtime qualification: {len(RUNTIME_EXPECTED) + 1} executions, {failures} unexpected verdicts")
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", action="store_true", help="also compile/execute native semantic mutations")
    args = parser.parse_args()
    old_checker = git_show(CHECKER_REL)
    old_ci = git_show(CI_REL)
    new_checker = CURRENT_CHECKER.read_text(encoding="utf-8")
    new_ci = CURRENT_CI.read_text(encoding="utf-8")
    helper = ROOT / HELPER_REL
    helpers = {HELPER_REL: helper.read_text(encoding="utf-8")} if helper.is_file() else {}
    if args.runtime:
        helpers[EXAMPLE_REL] = (ROOT / EXAMPLE_REL).read_text(encoding="utf-8")
    archive_files = {
        path: git_show(path)
        for path in (LIB_REL, BRIDGE_REL, HOST_SHELL_REL)
    }
    assert_established_prefix_wiring(archive_files)
    archive = archive_reviewed_tree()
    print("A/B scope: CI dispatch and checker source guard only; no Rust behavioral proof")
    failures = 0
    for case in CASES:
        if not run_case(case, archive, old_checker, old_ci, new_checker, new_ci, helpers):
            failures += 1
    print(f"Qualification: {len(CASES)} cases, {failures} unexpected verdict matrices")
    if args.runtime:
        failures += run_runtime_cases(archive, old_ci, new_checker, new_ci, helpers)
    return 1 if failures else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, ValueError) as error:
        print(f"F2 harness failed: {error}", file=sys.stderr)
        sys.exit(1)

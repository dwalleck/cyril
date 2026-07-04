#!/usr/bin/env python3
"""cyril-dcc6 cheapest falsifier: the discovery selection policy (claims C1-C6)
run against synthetic fixture roots, one labeled verdict per claim.

The policy function below IS the design's selection spec, executable. The
oracle for each case is the hand-derived expectation written next to it
(independent of the function); the real-machine case is additionally
cross-checked against the banked /proc oracle from prove-it-prototype.
Named buggy implementations each case would catch are in design.md § Falsification.
"""
import os, re, tempfile

REL = "node_modules/@kiro/agent/dist/server/acp-server.js"

def vertuple(v):
    return tuple(int(x) for x in v.split("."))

def select(root, cli_ver):
    """The design's policy: exact CLI-version match > newest versioned >
    legacy unversioned > None. Dirs without the inner entry are skipped."""
    candidates = []
    if os.path.isdir(root):
        for name in sorted(os.listdir(root)):
            m = re.fullmatch(r"(\d+\.\d+\.\d+)-[0-9a-f]{64}", name)
            if m and os.path.isfile(os.path.join(root, name, REL)):
                candidates.append((m.group(1), os.path.join(root, name, REL)))
    exact = [s for v, s in candidates if v == cli_ver]
    if exact:
        return exact[0], "exact"
    if candidates:
        return max(candidates, key=lambda c: vertuple(c[0]))[1], "newest"
    legacy = os.path.join(root, REL)
    if os.path.isfile(legacy):
        return legacy, "legacy"
    return None, "none"

SHA_A = "a" * 64
SHA_B = "b" * 64

def mk(root, dirname, with_entry=True):
    d = os.path.join(root, dirname, os.path.dirname(REL)) if dirname else os.path.join(root, os.path.dirname(REL))
    os.makedirs(d, exist_ok=True)
    if with_entry:
        open(os.path.join(d, "acp-server.js"), "w").write("//")

results = []
def check(claim, root, cli_ver, want_suffix, want_how):
    got, how = select(root, cli_ver)
    ok = (how == want_how) and ((got or "").endswith(want_suffix) if want_suffix else got is None)
    results.append(ok)
    print(f"{claim}: {'PASS' if ok else 'FAIL'}  (chose {how}: {got})")

# C1 exact match beats newer: 2.10.0 + 2.11.0 present, CLI=2.10.0 -> 2.10.0.
r = tempfile.mkdtemp(prefix="c1-")
mk(r, f"2.10.0-{SHA_A}"); mk(r, f"2.11.0-{SHA_B}")
check("C1 exact-beats-newer", r, "2.10.0", f"2.10.0-{SHA_A}/{REL}", "exact")

# C2 no match -> newest by SEMVER (2.10.0 > 2.9.0; lexicographic picks 2.9.0).
r = tempfile.mkdtemp(prefix="c2-")
mk(r, f"2.9.0-{SHA_A}"); mk(r, f"2.10.0-{SHA_B}")
check("C2 semver-not-lex     ", r, "2.11.0", f"2.10.0-{SHA_B}/{REL}", "newest")

# C3 CLI version unknown -> newest versioned.
r = tempfile.mkdtemp(prefix="c3-")
mk(r, f"2.10.0-{SHA_A}"); mk(r, f"2.11.0-{SHA_B}")
check("C3 no-cli-ver->newest ", r, None, f"2.11.0-{SHA_B}/{REL}", "newest")

# C4 partial extraction (no inner entry) is skipped, older complete dir wins.
r = tempfile.mkdtemp(prefix="c4-")
mk(r, f"2.10.0-{SHA_A}"); mk(r, f"2.11.0-{SHA_B}", with_entry=False)
check("C4 partial-skipped    ", r, "2.11.0", f"2.10.0-{SHA_A}/{REL}", "newest")

# C5 no versioned dirs -> legacy unversioned fallback.
r = tempfile.mkdtemp(prefix="c5-")
mk(r, None)
check("C5 legacy-fallback    ", r, "2.11.0", f"{REL}", "legacy")

# C6 nothing anywhere -> None (maps to KasMissing::Server naming the root).
r = tempfile.mkdtemp(prefix="c6-")
check("C6 nothing->missing   ", r, "2.11.0", None, "none")

# Cross-check vs the banked /proc oracle on the REAL machine.
real = os.path.expanduser("~/.local/share/kiro-cli/kas")
got, how = select(real, "2.11.0")
oracle = os.path.expanduser(
    "~/.local/share/kiro-cli/kas/2.11.0-05e941edbb0f543b420859b637722d2d94fde7d9fd336488f7d5975eedc48415/"
    "node_modules/@kiro/agent/dist/server/acp-server.js")
ok = got == oracle and how == "exact"
results.append(ok)
print(f"C1-real /proc-oracle  : {'PASS' if ok else 'FAIL'}  ({how}: {got})")

print("\nALL PASS" if all(results) else "\nFAILURES PRESENT")
raise SystemExit(0 if all(results) else 1)

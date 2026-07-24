#!/usr/bin/env bash
# cyril-4rc1 PROBE (guard option): does a REPO-TRACKED pre-commit hook, wired
# via core.hooksPath, actually fire and block a `git commit`? And can it be
# bypassed the way the fix would need to be understood (--no-verify)?
#
# This de-risks the "interim guard" design option: if hooks don't fire under a
# plain commit, a branch-verification guard is not enforceable and only docs
# remain. Oracle: a marker file the hook writes (proves execution independent
# of the commit's exit code) + the commit's own exit status.
set -u
ROOT=$(mktemp -d)
export GIT_AUTHOR_NAME=probe GIT_AUTHOR_EMAIL=p@x GIT_COMMITTER_NAME=probe GIT_COMMITTER_EMAIL=p@x
cd "$ROOT" || exit 1
git init -q -b main .
git commit -q --allow-empty -m base

mkdir -p .githooks
cat > .githooks/pre-commit <<EOF
#!/usr/bin/env bash
echo fired > "$ROOT/hook-ran"
# Simulate a branch-verification guard that blocks unless on an allowed branch.
cur=\$(git branch --show-current)
if [ "\$cur" = "main" ]; then
  echo "GUARD: refusing feature commit on main" >&2
  exit 1
fi
exit 0
EOF
chmod +x .githooks/pre-commit
git config core.hooksPath .githooks

echo "### hook wired via core.hooksPath (tracked dir .githooks/) ###"

rm -f "$ROOT/hook-ran"
if git commit -q --allow-empty -m "on main (should be blocked)" 2>"$ROOT/c1.err"; then
  echo "commit-on-main: SUCCEEDED (guard did NOT block)"
else
  echo "commit-on-main: BLOCKED (exit $?) -> $(tr -d '\n' <"$ROOT/c1.err" | tail -c 80)"
fi
echo "hook-ran marker after main attempt: $([ -f "$ROOT/hook-ran" ] && echo yes || echo no)"

git switch -q -c feat-x
rm -f "$ROOT/hook-ran"
if git commit -q --allow-empty -m "on feat-x (should pass)"; then
  echo "commit-on-feat-x: SUCCEEDED (guard allowed)"
else
  echo "commit-on-feat-x: BLOCKED (unexpected)"
fi

git switch -q main
if git commit -q --no-verify --allow-empty -m "bypass with --no-verify" 2>/dev/null; then
  echo "commit --no-verify on main: SUCCEEDED (hook bypassed, as documented)"
else
  echo "commit --no-verify on main: BLOCKED (hook not bypassable?!)"
fi

echo "ROOT=$ROOT"

#!/usr/bin/env bash
# cyril-ell0 module-shape fence — claims C1–C5 in .cyril-ell0/design.md.
#
# Prints exactly one `C<n> PASS|FAIL: <detail>` line per claim and exits 1 if
# any claim FAILs (2 on a harness error). Runs against the working tree,
# relative to the merge-base with the repository's default branch, which is
# discovered from refs/remotes/origin/HEAD (never hard-coded).
#
# Path space is partitioned so each named mutation reddens exactly one claim:
#   C1 owns crates/cyril-ui/src/stream_buffer.rs and the `mod stream_buffer`
#      declaration in crates/cyril-ui/src/lib.rs;
#   C3 owns the three legacy-config files that mention stream_buffer_timeout_ms;
#   C5 owns every other path (tracked diff vs merge-base + untracked under crates/).
set -u

root=$(git rev-parse --show-toplevel) || { echo "FENCE ERROR: not a git repository"; exit 2; }
cd "$root" || exit 2

upstream=$(git symbolic-ref -q --short refs/remotes/origin/HEAD) || {
    echo "FENCE ERROR: refs/remotes/origin/HEAD unset — run: git remote set-head origin --auto"
    exit 2
}
mb=$(git merge-base HEAD "$upstream") || { echo "FENCE ERROR: no merge-base with $upstream"; exit 2; }

status=0
pass() { printf '%s PASS: %s\n' "$1" "$2"; }
fail() { printf '%s FAIL: %s\n' "$1" "$2"; status=1; }

module_file=crates/cyril-ui/src/stream_buffer.rs
lib_rs=crates/cyril-ui/src/lib.rs
c3_files=(
    crates/cyril-core/src/types/config.rs
    crates/cyril-core/tests/nd4h_legacy_config_compat.rs
    crates/cyril/tests/nd4h_source_fences.rs
)

# C1 — module file absent; lib.rs declares no `mod stream_buffer`.
if [ -e "$module_file" ]; then
    fail C1 "$module_file exists"
elif decl=$(grep -n 'mod stream_buffer' "$lib_rs"); then
    fail C1 "$lib_rs declares the module: $decl"
else
    pass C1 "$module_file absent; $lib_rs has no mod stream_buffer"
fi

# C2 — no `StreamBuffer` identifier and no `stream_buffer::` path under crates/.
hits=$( { grep -rnw 'StreamBuffer' crates/; grep -rn 'stream_buffer::' crates/; } 2>/dev/null | sort -u )
if [ -n "$hits" ]; then
    fail C2 "$(printf '%s' "$hits" | head -5 | tr '\n' ';')"
else
    pass C2 "no StreamBuffer identifier or stream_buffer:: path under crates/"
fi

# C3 — legacy-config files byte-identical to the merge-base.
c3_changed=$(git diff --name-only "$mb" -- "${c3_files[@]}")
if [ -n "$c3_changed" ]; then
    fail C3 "$(printf '%s\n' "$c3_changed" | tr '\n' ' ')differs from merge-base $mb"
else
    pass C3 "legacy-config files identical to merge-base $mb"
fi

# C4 — no live-component prose in the top-level docs or the generated component
# catalog (.agents/summary/, found by the isolated design-conformance review).
doc_hits=$( { grep -in 'stream buffer' CLAUDE.md AGENTS.md README.md;
              grep -n 'StreamBuffer\|stream_buffer' .agents/summary/components.md .agents/summary/review_notes.md; } 2>/dev/null )
if [ -n "$doc_hits" ]; then
    fail C4 "$(printf '%s' "$doc_hits" | head -3 | tr '\n' ';')"
else
    pass C4 "CLAUDE.md, AGENTS.md, README.md, .agents/summary/{components,review_notes}.md mention no stream buffer"
fi

# C5 — every changed or new path is in the approved set (C1's file and C3's
# files are judged by their own claims and excluded here).
allowed='^(crates/cyril-ui/src/lib\.rs|CLAUDE\.md|AGENTS\.md|\.agents/summary/components\.md|\.agents/summary/review_notes\.md|\.cyril-ell0/.*)$'
c3_regex='^(crates/cyril-core/src/types/config\.rs|crates/cyril-core/tests/nd4h_legacy_config_compat\.rs|crates/cyril/tests/nd4h_source_fences\.rs)$'
paths=$( { git diff --name-only "$mb"; git ls-files --others --exclude-standard crates/; } | sort -u )
unexpected=$(printf '%s\n' "$paths" \
    | grep -v '^$' \
    | grep -vx "$module_file" \
    | grep -Ev "$allowed" \
    | grep -Ev "$c3_regex" || true)
if [ -n "$unexpected" ]; then
    fail C5 "unexpected path $(printf '%s' "$unexpected" | head -5 | tr '\n' ' ')"
else
    pass C5 "changed paths within the approved set relative to $mb"
fi

exit "$status"

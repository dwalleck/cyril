#!/usr/bin/env bash
# P3/P4 oracle — same answer, different failure mechanism.
#
# The probe asks the COMPILER to resolve `Send + 'static` for UsageSnapshot and
# UsageEnrichmentResult. This oracle never compiles anything: it reads the
# source and looks for the constructs that are the only way a plain data struct
# built from std types can fail to be Send — interior-mutability and
# non-atomic-refcount types, raw pointers, and unsendable trait objects.
#
# Agreement means the compiler's answer and a hand-auditable source scan reach
# the same conclusion by different routes. A `Send` impl that held only because
# of an `unsafe impl` elsewhere, or a field type the probe never instantiated,
# would show up as a mismatch.
#
# Run: .cyril-nanu/oracle_send.sh
set -uo pipefail
cd "$(dirname "$0")/.."

# Every type reachable from UsageSnapshot is defined in these two files
# (the snapshot is a tree of Vec/Option/String/u64/f64 plus these newtypes).
FILES="crates/cyril-core/src/types/usage.rs crates/cyril-core/src/usage.rs"

echo "--- files scanned"
for f in $FILES; do echo "  $f"; done

echo "--- reachable type definitions from UsageSnapshot"
sed -n '/pub struct UsageSnapshot/,/^}/p' crates/cyril-core/src/types/usage.rs

echo "--- non-Send constructs anywhere in the scanned files"
# Rc / RefCell / Cell / raw pointers / bare dyn trait objects.
hits=$(grep -nE '\bRc<|\bRefCell<|\bCell<|\*const |\*mut |\bdyn [A-Z]' $FILES || true)
if [ -z "$hits" ]; then
	echo "  none"
	verdict_send=True
else
	echo "$hits"
	verdict_send=False
fi

echo "--- unsafe impls that could fake a Send bound"
unsafe_hits=$(grep -nE 'unsafe impl' $FILES || true)
if [ -z "$unsafe_hits" ]; then echo "  none"; else echo "$unsafe_hits"; fi

echo "--- borrowed lifetimes that would break 'static"
life_hits=$(sed -n '/pub struct UsageSnapshot/,/^}/p' crates/cyril-core/src/types/usage.rs | grep -nE "&'|<'" || true)
if [ -z "$life_hits" ]; then echo "  none"; verdict_static=True; else echo "$life_hits"; verdict_static=False; fi

echo "--- verdict"
echo "  SEND (no non-Send construct)   : $verdict_send"
echo "  'STATIC (no borrowed lifetime) : $verdict_static"

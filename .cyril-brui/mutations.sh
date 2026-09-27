#!/usr/bin/env bash
# cyril-brui checkpoint: named mutations from design.md, each applied to
# discovery.rs, the discovery fences run (no-fail-fast), the file restored
# byte-exactly from a backup (never `git checkout` on uncommitted work), and
# green re-confirmed at the end. Run alone: a concurrent cargo build would
# compile the mutant.
set -u
cd "$(dirname "$0")/.." || exit 2
F=crates/cyril-core/src/protocol/kas/discovery.rs
B="${1:?backup path outside the repo}"
cp "$F" "$B"
LOG=.cyril-brui/mutation-runs.log
: > "$LOG"
run_fences() {
  env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas discovery --no-fail-fast > /tmp/brui-mut.log 2>&1
  echo "exit=$?"
  grep -E "^\s+FAIL \[" /tmp/brui-mut.log | sed -E 's/.*tests::([a-z_]+).*/    red: \1/' | sort -u
  grep -E "Summary" /tmp/brui-mut.log
  grep -E "^error(\[|:)" /tmp/brui-mut.log | head -3
}
mutate() { # name, perl -0pe expression, grep proof that it applied
  local name="$1" expr="$2" proof="$3"
  echo "=== $name" | tee -a "$LOG"
  cp "$B" "$F"
  perl -0pi -e "$expr" "$F"
  if cmp -s "$B" "$F"; then echo "MUTATION DID NOT APPLY: $name" | tee -a "$LOG"; return; fi
  grep -c -- "$proof" "$F" | sed 's/^/    proof-lines: /' | tee -a "$LOG"
  run_fences | tee -a "$LOG"
  cp "$B" "$F"
}

mutate "M1 (C1) delete the XDG branch: it is never entered" \
  's/if let Some\(value\) = xdg_data_home \{/if let Some(value) = xdg_data_home.filter(|_| false) {/' \
  'filter(|_| false)'
mutate "M2 (C3a) delete the tracing::warn! on invalid XDG_DATA_HOME" \
  's/tracing::warn!\(\s*value = \?value,\s*"XDG_DATA_HOME[^"]*"\s*\);\n//s' \
  'XDG_DATA_HOME is not an absolute path'
mutate "M3 (C3b) treat a relative/empty value as valid: is_absolute() -> true" \
  's/if xdg\.is_absolute\(\) \{/if true {/' \
  'if true {'
mutate "M4 (C5/C6) stray extra segment in store_path" \
  's/data_dir\.join\(STORE_FILE_NAME\)/data_dir.join(KIRO_DATA_DIR_NAME).join(STORE_FILE_NAME)/' \
  'join(KIRO_DATA_DIR_NAME).join(STORE_FILE_NAME)'
mutate "M5 (C2) wrong XDG default: .local/share -> .local/state" \
  's/XDG_DATA_HOME_DEFAULT_REL: &str = "\.local\/share"/XDG_DATA_HOME_DEFAULT_REL: \&str = ".local\/state"/' \
  '.local/state'
mutate "M6 (C4) resolve re-derives the root with the home-relative default" \
  's/let root = kas_root\(data_dir\.ok_or\(KasMissing::NoHome\)\?\);/let root = data_dir.ok_or(KasMissing::NoHome)?.join(XDG_DATA_HOME_DEFAULT_REL).join(KAS_ROOT_REL);/' \
  'join(XDG_DATA_HOME_DEFAULT_REL).join(KAS_ROOT_REL)'

echo "=== restore check" | tee -a "$LOG"
cp "$B" "$F"
if cmp -s "$B" "$F"; then echo "restored byte-exact" | tee -a "$LOG"; else echo "RESTORE MISMATCH" | tee -a "$LOG"; exit 3; fi
echo "=== green after restore" | tee -a "$LOG"
run_fences | tee -a "$LOG"

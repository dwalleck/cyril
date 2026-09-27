#!/usr/bin/env bash
# cyril-brui review round 1: the reviewer's F1 mutations R1/R2, applied to
# discovery.rs, the discovery fences run (no-fail-fast), the file restored
# byte-exactly from a backup. Run alone (a concurrent cargo build would
# compile the mutant). Usage: review-mutations.sh <backup path> <log name>
set -u
cd "$(dirname "$0")/.." || exit 2
F=crates/cyril-core/src/protocol/kas/discovery.rs
B="${1:?backup path outside the repo}"
LOG=".cyril-brui/${2:?log name}"
cp "$F" "$B"
: > "$LOG"
echo "revision: $(git rev-parse --short HEAD) + working tree; date: $(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$LOG"
run_fences() {
  env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas discovery --no-fail-fast > /tmp/cyril-brui-review-mut.log 2>&1
  echo "exit=$?"
  grep -E "^\s+FAIL \[" /tmp/cyril-brui-review-mut.log | sed -E 's/.*tests::([a-z_]+).*/    red: \1/' | sort -u
  grep -E "Summary" /tmp/cyril-brui-review-mut.log
  grep -E "^error(\[|:)" /tmp/cyril-brui-review-mut.log | head -3
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

mutate "R1 (F1) wrapper reads the wrong env var: XDG_DATA_HOME -> XDG_DATA_DIR" \
  's/std::env::var_os\("XDG_DATA_HOME"\)/std::env::var_os("XDG_DATA_DIR")/' \
  'var_os("XDG_DATA_DIR")'
mutate "R2 (F1) store gate bypasses the resolver with a home-relative literal" \
  's/let db = data_dir\s*\.as_deref\(\)\s*\.map\(store_path\)\s*\.ok_or\(KasMissing::NoHomeForStore\)\?;/let db = crate::kiro_agent_config::home_dir().map(|h| h.join(".local\/share\/kiro-cli\/data.sqlite3")).ok_or(KasMissing::NoHomeForStore)?;/s' \
  'h.join(".local/share/kiro-cli/data.sqlite3")'

mutate "F2-sym (F2) wrapper ignores XDG_DATA_HOME on THIS host (the macOS/Windows arm applied everywhere)" \
  's/#\[cfg\(not\(any\(target_os = "macos", target_os = "windows"\)\)\)\]\s*let xdg_data_home = std::env::var_os\("XDG_DATA_HOME"\);\s*#\[cfg\(any\(target_os = "macos", target_os = "windows"\)\)\]\s*let xdg_data_home: Option<std::ffi::OsString> = None;/let xdg_data_home: Option<std::ffi::OsString> = None;/s' \
  'let xdg_data_home: Option<std::ffi::OsString> = None;'

echo "=== restore check" | tee -a "$LOG"
cp "$B" "$F"
if cmp -s "$B" "$F"; then echo "restored byte-exact" | tee -a "$LOG"; else echo "RESTORE MISMATCH" | tee -a "$LOG"; exit 3; fi
echo "=== green after restore" | tee -a "$LOG"
run_fences | tee -a "$LOG"

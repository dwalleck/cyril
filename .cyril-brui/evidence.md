# Evidence: cyril-brui

Discharges the T1 premise in `route.md`: how the installed kiro-cli resolves its data dir from `XDG_DATA_HOME`. All runs are **no-auth by construction** (see Data): the hard constraint for this batch forbids any kiro-cli command that needs or refreshes auth, so `whoami`/`login`/`acp`/`test_bridge` were never run.

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | kiro-cli honours an absolute `XDG_DATA_HOME`: its data dir is `$XDG_DATA_HOME/kiro-cli`, and `$HOME/.local/share` is not consulted when XDG wins. | With `HOME=<empty tmp A>` and `XDG_DATA_HOME=<empty tmp B>` (absolute), which path does a local kiro-cli subcommand open for `data.sqlite3`? | PASS |
| P2 | kiro-cli treats an empty or relative `XDG_DATA_HOME` as unset and falls back to `$HOME/.local/share/kiro-cli` (the `dirs-sys::is_absolute_path` rule), rather than joining the relative value or failing. | With `XDG_DATA_HOME=relative/data` and, separately, `XDG_DATA_HOME=` (set, empty), which path does the same subcommand open for `data.sqlite3`? | PASS |
| P3 | The KAS extraction root lives under the same data dir as the store (`<data dir>/kas/<semver>-<sha>/…`). | Not an empirical premise for this change: the layout beneath the data dir is unchanged and already fenced (`versioned_beats_legacy`, `list_kas_entries_real_fs`); only the data-dir prefix moves. | N/A — already covered by valid repository evidence (existing discovery fences, dcc6) |
| P4 | cyril must warn on an invalid `XDG_DATA_HOME` and must resolve XDG-first for both root and store. | Not an empirical premise: this is the approved behavior of the feature being built (route.md T4), not a claim about an existing system. | N/A — approved behavior, not external behavior |

## Data

- Source: production-shaped — the real installed launcher `/home/dwalleck/.local/bin/kiro-cli` (`kiro-cli 2.24.0`, sha256 recorded by `sha256sum` during the session: the binary is the one the 2.24.0 memory reference describes), run against **empty scratch directories** for `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`.
- Shape: identical to production spawn env shape (same binary, same env-var names); the only difference is that every directory is empty, so the launcher creates a fresh, token-less `data.sqlite3` wherever it resolves its data dir — which is exactly the observable we need.
- Safety: no production state is readable or writable by the probe (all four dirs are under the session scratchpad); no token exists in the scratch store, so no refresh can be attempted (the single-use refresh-token hazard cannot fire); only local subcommands run (`--version`, `settings list`). Approval: not needed — no snapshot, no production state.

## Probe

- File: `probe-xdg-strace.sh` (outputs: `probe-xdg-strace.out` = absolute leg, `probe-xdg-strace-relative.out`, `probe-xdg-strace-empty.out`)
- Mechanism: **dynamic** — `strace -f -e trace=openat,statx,stat,access,newfstatat,readlink,mkdir` on the live binary, then counts syscalls naming `$XDG_DATA_HOME` vs `$HOME/.local/share`, and lists what the run created under `$XDG_DATA_HOME`.
- Run (from the worktree root; `S` = a scratch dir):
  - P1: `bash .cyril-brui/probe-xdg-strace.sh $S/xdgprobe ~/.local/bin/kiro-cli settings list`
  - P2a: `PROBE_XDG_VALUE=relative/data bash .cyril-brui/probe-xdg-strace.sh $S/xdgprobe-rel ~/.local/bin/kiro-cli settings list`
  - P2b: `PROBE_XDG_VALUE= bash .cyril-brui/probe-xdg-strace.sh $S/xdgprobe-empty ~/.local/bin/kiro-cli settings list`
  - Note: `kiro-cli --version` was tried first and touches no data dir at all (0 syscalls either side) — the launcher prints and exits; `settings list` is the smallest local subcommand that opens the store.

## Oracle

- Mechanism: **static** — two independent readings that cannot share strace's failure mechanism: (a) the launcher's string table: `strings -n 8 ~/.local/bin/kiro-cli | grep XDG_DATA_HOME` shows the `dirs-sys` cluster `LibraryApplication Support.configXDG_DATA_HOME.local/shareLOCALAPPDATAAppDataLocal` (the adjacent macOS/Linux/Windows fallbacks are the fingerprint of `dirs::data_local_dir`); (b) the `dirs` crate source in the local cargo registry, `dirs-6.0.0/src/lin.rs:11`: `data_dir() = env::var_os("XDG_DATA_HOME").and_then(dirs_sys::is_absolute_path).or_else(|| home_dir().map(|h| h.join(".local/share")))`, with `dirs-sys-0.5.0/src/lib.rs:8-14` `is_absolute_path` returning `None` for any non-absolute `PathBuf` (an empty string is not absolute). Prediction from the oracle: absolute XDG → `<xdg>/kiro-cli/…`, `$HOME/.local/share` untouched; relative or empty XDG → `<home>/.local/share/kiro-cli/…`.
- Run: `strings -n 8 ~/.local/bin/kiro-cli | grep -c XDG_DATA_HOME` (3 hits) and `grep -n XDG_DATA_HOME -A2 ~/.cargo/registry/src/*/dirs-6.0.0/src/lin.rs`.

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | `settings list`, `XDG_DATA_HOME=<S>/xdg` (absolute): **348** syscalls name `<S>/xdg` — `openat("<S>/xdg/kiro-cli/data.sqlite3", O_RDWR\|O_CREAT…)`, `-journal`, `-wal` probes; **0** syscalls name `<S>/home/.local/share`; after the run `<S>/xdg/kiro-cli/{data.sqlite3, telemetry-export-drops.lock, run-receipts/}` exist (`probe-xdg-strace.out`). | `dirs::data_dir`: XDG absolute → `<xdg>`; home fallback not evaluated (`or_else` short-circuits). Predicts store at `<xdg>/kiro-cli/data.sqlite3`, zero `$HOME/.local/share` traffic. | PASS |
| P2a | `XDG_DATA_HOME=relative/data`: **0** syscalls name `relative/data`; **338** name `<S>/home/.local/share` including `mkdir("<S>/home/.local/share/kiro-cli")` and the store open (`probe-xdg-strace-relative.out`). | `is_absolute_path("relative/data")` → `None` → `<home>/.local/share`. Predicts store at `<home>/.local/share/kiro-cli/data.sqlite3`. | PASS |
| P2b | `XDG_DATA_HOME=` (empty): **336** syscalls name `<S>/home/.local/share`, same `mkdir` + store open; the "syscalls naming `$XDG_DATA_HOME`" line reads 13751 only because `grep ""` matches every line — an artifact of the empty value, not evidence (`probe-xdg-strace-empty.out`). | `is_absolute_path("")` → `None` → `<home>/.local/share`. Same prediction as P2a. | PASS |

## Validated / learned

- P1: validated prior understanding — probe and oracle agree that the installed launcher opens `<xdg>/kiro-cli/data.sqlite3` and never consults `$HOME/.local/share` when `XDG_DATA_HOME` is absolute. This is the mechanism behind every committed `HOME=<tmp> XDG_DATA_HOME=~/.local/share` conductor-spike probe finding the real token store.
- P2: validated prior understanding, with one sharpening — the approved cyril behavior ("empty or relative → fall back to HOME") is exactly `dirs-sys` semantics; kiro-cli itself does not warn (it silently falls back). cyril's added warning is therefore a strict superset of kiro-cli's observable path behavior, not a divergence in resolution.
- Side observation (not a premise): the launcher also stats `<home>/.local/share/amazon-q` (legacy Amazon Q data-dir migration check). Irrelevant to cyril; noted so nobody mistakes it for a second data dir.

## Related issues

- Consulted: `cyril-brui` itself (the issue text records the 2.21.0-audit symlink workaround). A bounded read-only search of the tracker (`rivets list --limit 1000`, terms xdg/discover/data dir/home) found no other issue touching the data-dir resolution. Memory references consulted: `feedback_isolate_kiro_probes_with_home` (the HOME-isolation discipline this fix unblocks), `feedback_kiro_token_renewal_single_flight` (the reason the live acceptance run is an obligation not run).
- Filed: none — no underlying-system defect (kiro-cli behaves per `dirs`), no deferred work discovered.

# cyril-8tq6 — prove-it-prototype

**Goal:** before designing the `\\wsl$`/`\\wsl.localhost` translation, prove (a) the
bug's mechanism on production-shape data with cyril's REAL functions, and (b) that a
small prototype of the proposed rule reproduces Microsoft's own `wslpath` semantics.

**Probe:** `.cyril-8tq6/probe_translation.rs`, run by `.cyril-8tq6/run-probe.sh`
(copies into `crates/cyril-core/tests/`, runs against the real crate, deletes the
copy). Output captured in `probe-output.txt`.

- **Part A** — real `wsl_to_win` over every absolute string in the real KAS 2.10.0
  host-callback capture (`.cyril-7bdu/host_callbacks_2.10.0.json`).
- **Part B** — a ~30-line prototype of the proposed rule (drive translation first,
  then `\\wsl.localhost\<distro>\` for any other `/`-rooted path; reverse accepts
  both prefixes, exact distro segment) vs Microsoft's conformance cases.
- **Part C** — round-trip of every capture path through the prototype.
- **Part D** — real `translate_paths_in_json(WslToWin)` on a real capture envelope.

## Oracles

1. **Part A oracle — `jq`** (independent engine + traversal):
   `jq '[.. | strings | select(startswith("/"))] | length'` = **11**, of which
   non-`/mnt/` = **11** (4 distinct paths). Probe: 11 abs, 11 PASSTHRU, 0
   translated. **Agreement, item by item.**
2. **Part B oracle — Microsoft's own wslpath conformance tests**
   ([microsoft/WSL `test/linux/unit_tests/wslpath.c`](https://github.com/microsoft/WSL/blob/master/test/linux/unit_tests/wslpath.c)):
   `\\wsl.localhost\<distro>` canonical, `\\wsl$\<distro>` compat-accepted;
   `/` → `...\`; trailing separators preserved; forward slashes accepted inbound;
   `<distro>-other`/`<distro>X` are **errors** (exact-segment match). Prototype:
   **16/16 PASS.**

## Result

- **Bug mechanism confirmed on production data:** 100% (11/11) of the absolute
  paths in a real KAS session on a WSL-native workspace are WSL-internal — every
  fs callback of the session would fail `-32603 NotFound` on a Windows host. This
  is not an edge case; it is the entire workspace.
- **Prototype rule is conformant:** drive translation first, UNC for the rest,
  composes cleanly — `/mnt/c/...` still wins the drive path; `/mnt/data` (a
  non-drive `/mnt` entry, i.e. WSL-internal) correctly falls through to UNC.
- **Round-trip holds:** 11/11 capture paths survive `/…` → UNC → `/…`.

## What I learned (not obvious before probing)

1. **cyril never injects `wsl` itself** — the agent command is user-supplied
   (`--agent-command wsl kiro-cli acp`; `transport.rs` spawns argv verbatim), so
   the distro name **cannot** be derived from cyril's own spawn. Resolution needs
   an explicit source (config/env/query) — a design decision, not a code detail.
2. **Exact-distro-segment matching is a hard MS requirement**: `\\wsl$\Ubuntu-other\foo`
   must NOT parse as distro `Ubuntu`. A naive `strip_prefix` would get this wrong.
3. `wslpath` **emits** `\\wsl.localhost\...` (canonical since Win11);
   `\\wsl$\...` is the compat alias — both must be **accepted** inbound.
4. `translate_paths_in_json` has **zero production callers** (only its own tests
   and this probe) — the fs callback path goes through `to_native_checked` →
   `to_native` → `wsl_to_win` directly.
5. WSL sets `WSL_DISTRO_NAME` only *inside* the distro; on the Windows host the
   default distro comes from `wsl.exe -l -q` / `--status` / registry — and
   `wsl.exe` output is UTF-16LE (parsing hazard for later).

## Negative space (known, deliberately out of probe scope)

- wslpath's private-use-area escaping of `\` and `:` in filenames (U+F05C/U+F03A)
  — pre-existing limitation shared by the current drive translation; not new here.
- Case-sensitivity of the distro segment (Windows is case-insensitive; MS tests
  only cover exact case). Prototype requires exact case.
- Live on-Windows fs read/write against `\\wsl$` (needs a real Windows+WSL host;
  the final AC keeps this as a manual/CI-deferred verification).

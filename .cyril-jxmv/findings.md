# cyril-jxmv — probe findings (2026-08-05)

## Method

- **Probe**: `probe.rs` compiled against the real `libcyril_core.rlib`
  (`probe.sh`). `to_agent`/`to_native` are one-line `cfg!(target_os =
  "windows")` dispatches (platform/path.rs:25-42) whose Windows arms
  (`win_to_wsl`/`wsl_to_win`) are OS-independent public functions — the probe
  executes the exact Windows behavior on this Linux host, and the GATE section
  shows the real entry points are identity here (host-OS `cfg!` is the only
  key).
- **Oracle 1** (wire values): `oracle.py`, an independent Python
  implementation of the *documented* contract (CLAUDE.md drive-mount rule +
  path.rs doc comments). Item-by-item diff: **AGREE-OK**.
- **Oracle 2** (caller set): `tethys callers --lsp` vs grep — two mechanisms,
  same answer.

## Facts established

1. **Outbound corruption is real and unconditional.** On Windows,
   `to_agent(C:\Users\u\repos\proj)` → `/mnt/c/Users/u/repos/proj` — the
   drive branch needs no distro. This is the `session/new` cwd wire value at
   bridge.rs:1048 (`run_loop`), sent to a native kiro-cli.exe that has no
   `/mnt/c`. AC 1's primary corruption.
2. **Inbound is accidentally safe for the common native shapes.** `C:\...`
   and `C:/...` inputs pass `wsl_to_win` untouched (not `/mnt/*`, not
   `/`-rooted). The ticket's "inbound paths get wrongly converted" is
   narrower than stated: inbound corrupts only for `/`-rooted agent paths
   **when a distro is configured** (→ spurious `\\wsl$\<distro>\...` UNC), a
   plausible state on a machine that also has WSL (`CYRIL_WSL_DISTRO` set, or
   cyril launched from a `\\wsl$` cwd).
3. **No agent-location input exists anywhere in the chain.** path.rs's
   complete ambient input surface is `{CYRIL_WSL_DISTRO, process cwd}`
   (path.rs:124-125); function inputs are the path alone. Production callers:
   exactly `bridge.rs:1048` (outbound) and `kas/host_io.rs:209` (inbound,
   fanned out to kiro_fs.rs / terminal_io.rs via `to_native_checked`).
4. **The decision must key on the RESOLVED spawn command, not the CLI arg.**
   `resolve_spawn_command` (bridge.rs:539-566) rewrites the command for KAS:
   free path = `node … acp-server.js` (native), wrapper = CLI program
   preserved + `--agent-engine <flag>`. A `wsl`-prefixed CLI command stays
   `wsl`-prefixed only in v2/wrapper; KAS free replaces it entirely.
5. **`translate_paths_in_json` has zero production call sites** — public in
   path.rs, called only by its own tests. The gate design need not thread
   anything through it (but should keep it consistent for future callers).
6. **Existing OnceLock pattern + child-process fencing is the local idiom**
   for process-global platform state (`process_wsl_distro`,
   `tests/win_wsl_wiring.rs` spawns itself with env vars — set_var is
   unsafe/forbidden in Rust 2024).

## What I learned (the one sentence)

The native-agent corruption is asymmetric — outbound-always
(`C:\ → /mnt/c/` on the session cwd) but inbound-only-with-distro-configured
— and the agent-location gate must consume the *resolved* spawn command
(post `resolve_spawn_command`), because KAS free-path spawns `node` natively
even when the user's `--agent-command` says `wsl`.

## Discovered, to file at close-out (kept off this branch per parallel-work rule)

- `build_wrapper_command` (kas/version.rs:72) runs `<program> --version`
  where `<program>` may be `wsl`/`wsl.exe` — it would parse WSL's own version
  as the kiro-cli version and pick the wrong `--agent-engine` flag. Latent
  until wsl-wrapped KAS wrapper mode is exercised. See `to-file.md`.

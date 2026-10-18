# Live: cyril on native Windows, PowerShell 7, KAS 0.66.43 (2026-10-18)

- Host: Windows 11 VM `resourcefs-win11`, kiro-cli 2.32.0 (IAM Identity Center login), KAS bundle 0.66.43
  (`%LOCALAPPDATA%\Kiro-Cli\kas\2.32.0-eaccc1d6…`), `pwsh` 7 at `C:\Program Files\PowerShell\7\pwsh.exe`.
- Binary: cyril built on the VM at `ca400c75` — provenance in `build-and-kas-log.txt` (clone HEAD `ca400c75bee8…`, `cyril.exe` sha256 `7d0ca00c…`; the clone directory name `cyril-k8wc-7ae63f66` is historical) (`cargo build -p cyril --features kas`), `--agent-engine kas`,
  driven from Linux by the hex1b CLI over `ssh -tt` (`run-win.sh`, `drive-win.sh`).
- Results (`drive.log`, `final-screen.txt`):
  - "Reply with only the exact value of the Shell line in your system information, wrapped in square brackets"
    → `[pwsh]` — KAS wrote cyril's `pwsh` reply into the system prompt (`Shell: ${shellType}`).
  - `Get-Date -Format yyyy && Write-Output ("PWSH" + (6+1) + "OK")` → approval overlay offered
    Allow / **Always allow** / Deny / **Always deny** (persistable consent), then output `PWSH7OK`
    (`&&` is a PowerShell 7 pipeline-chain operator — Windows PowerShell 5.1 would reject it).
  - KAS log: `shellType.probe` → `acp-peer.shellType.ok` → `shellType.resolved {source:"client"}`;
    no `policy.shell.type.unknown` (count 0) — raw lines in `build-and-kas-log.txt`. These log lines do not print the
    value itself; the `[pwsh]` answer is the value evidence.

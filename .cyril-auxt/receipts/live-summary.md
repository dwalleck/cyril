# Live acceptance (C9) — real cyril TUI over KAS, 2026-10-18

Setup (both arms): hex1b 120×36, `HOME=<scratch>/live/<arm>/home` (private, empty at start),
`XDG_DATA_HOME=~/.local/share`, `XDG_CONFIG_HOME=<arm>/home/.config`, `SHELL=/usr/bin/fish`,
`cyril --agent-engine kas` (`--features kas` debug build), cwd `<arm>/ws`. kiro-cli 2.32.0 / KAS 0.66.43.
No token-touching kiro-cli command was run.

## Before — main `10e38e3c`

| Step | Result | Receipt |
|---|---|---|
| s1: `echo AUXTV1` → Always allow | answered at once, no scope choice | `live-before-s1-final.txt` |
| s1: write `src/a/x.txt` → Always allow | answered at once | `live-before-s1-final.txt` |
| after s1 | **no `permissions.yaml` anywhere under the private HOME** | `live-before-permissions-yaml.txt` |
| s2 (fresh session): `echo AUXTV1` | **asks again** | `live-before-s2-shell-asks.txt` |
| s2: write `src/a/y.txt` | **asks again** | `live-before-s2-write-asks.txt` |

## After — branch `50dcc3b3`

| Step | Result | Receipt |
|---|---|---|
| s1: `echo AUXTV1` → Always allow | scope page: `shell → echo AUXTV1`, `Trust "echo AUXTV1"` / `Trust "echo *"` / `Trust entire tool (run_command)`; `s` ×2 → `[user]`; Enter on exact | `live-after-s1-shell-scope-page.txt` |
| → | `$HOME/.kiro/settings/permissions.yaml` = `{capability: shell, effect: allow, match: [echo AUXTV1]}` | `live-after-user-permissions.yaml` |
| s1: write `src/a/x.txt` → Always allow | scope page at `[workspace]`: exact / `Allow a folder in this workspace…` (`src/a · workspace`) / whole tool | `live-after-s1-write-scope-page.txt` |
| → folder row | editor prefilled `src/a`, edited to `src`: "Allows writes in src and every folder under it, in this workspace." | `live-after-s1-folder-editor.txt` |
| → | `$HOME/.kiro/workspace-roots/beb4c5095a09ae15/permissions.yaml` = `{capability: fs_write, effect: allow, match: [src/**]}` | `live-after-workspace-permissions.yaml` |
| s2 (fresh session `sess_4aeae58e…`): `echo AUXTV1` | **ran without asking** (session messages record `"command":"echo AUXTV1"`) | `live-after-s2-shell-no-prompt.txt` |
| s2: write `src/b/z.txt` | **written without asking** (`ws/src/b/z.txt` = `world`) — the relative in-workspace glob `src/**` the TUI composes is honoured (route.md carried premise: discharged) | `live-after-s2-write-no-prompt.txt` |
| s2 control: write `notes/n.txt` (outside the folder) | **asks** (denied) — the grant is not blanket | `live-after-s2-control-outside-folder-asks.txt` |

KAS log for the after HOME: 3 `acp-peer.request_permission.ok` in total — s1 shell, s1 write, s2 control.

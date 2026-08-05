# Issues to file at close-out (rivets mutations kept off this branch)

1. **kas/version.rs `build_wrapper_command` mis-parses version for wrapped
   commands** — it runs `<program> --version` (kas/version.rs:72,
   `kiro_cli_version(agent_command.program())`); with `--agent-command wsl
   kiro-cli acp`, program is `wsl`, so it parses WSL's own version string and
   resolves the `--agent-engine` flag from that. Latent until someone runs
   KAS wrapper mode through WSL. Suggested: P4 bug, blocks nothing.
   Discovered during cyril-jxmv probe (see findings.md).

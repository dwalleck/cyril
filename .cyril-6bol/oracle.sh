#!/usr/bin/env bash
# Independent hand oracle: direct-exec the captured command; do not use probe.py's resolver or renderer.
set -euo pipefail
shell_env=${SHELL-}
shell=$(command -v bash)
stdout=$(/usr/bin/echo done-42)
operator_stdout=$(/usr/bin/echo done-42 | /usr/bin/tr a-z A-Z)
expansion_stdout=$(CYRIL_SHELL_PROBE=expanded-42 /usr/bin/printenv CYRIL_SHELL_PROBE)
jq -cn --arg source '.cyril-7bdu/fixtures/terminal__create.json' \
  --argjson shell_env null --arg shell "$shell" --arg family posix \
  --arg command 'echo done-42' --arg stdout "$stdout"$'\n' \
  --arg operator_command 'echo done-42 | tr a-z A-Z' \
  --arg operator_stdout "$operator_stdout"$'\n' \
  --arg expansion_command 'echo $CYRIL_SHELL_PROBE' \
  --arg expansion_stdout "$expansion_stdout"$'\n' \
  '{source:$source,shell_env:$shell_env,shell:$shell,family:$family,command:$command,stdout:$stdout,stderr:"",exit:0,operator_command:$operator_command,operator_stdout:$operator_stdout,operator_stderr:"",operator_exit:0,expansion_command:$expansion_command,expansion_stdout:$expansion_stdout,expansion_stderr:"",expansion_exit:0}'

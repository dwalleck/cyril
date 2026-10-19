# cyril-9104 live check (C11), 2026-10-18

Setup: debug `cyril` built from `d7158994` (default features = KAS), run through a hex1b
hosted terminal (140x45) with a private `HOME`, `XDG_DATA_HOME=/home/dwalleck/.local/share`,
`XDG_CONFIG_HOME=<tmp>/.config`; workspace with `.kiro/workflows/park.workflow.json`
(one `wf-coder` step, `completion: {containsText: "FINISHED"}`, prompt asking the user a
question). Plan: `/workflow run .kiro/workflows/park.workflow.json` → question line →
`/workflow status` → `/answer … FINISHED` → `workflow "park" completed`.

Result: **BLOCKED before the session started — auth.** cyril's own token renewal failed:

    Disconnected: invalid configuration: KAS auth unavailable: `/home/dwalleck/.local/bin/kiro-cli
    chat _ get-kas-token` did not answer within 30 s; check the network connection to the Kiro
    auth service, or another kiro-cli holding its token-refresh lock (kiro-cli logs `refresh lock
    timed out (peer wedged?)`), and retry

Per the brief, no kiro-cli auth command was run and the step was not retried; the terminal and
cyril were stopped by exact PID. C11 stays PENDING. The capture-driven end-to-end test
(`app::tests::answer_command_prompts_the_parked_step_and_the_run_resumes`, live 0.66.43
gate-off frames) covers the same path offline.

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

## Rerun 2026-10-19 (~23:55Z) — PASS (coordinator)

After the user re-logged in, the coordinator ran the same setup with a debug `cyril` built from the PR head
`51d06456` (gate-off private HOME, `cli.json` = `{}`; kiro-cli 2.32.0 / KAS 0.66.43), driven by `drive.sh`:

    PASS session created
    PASS run → is waiting for your answer
    PASS view → waiting for your answer: step "ask"
    PASS answer → Sent your answer to step "ask"
    PASS run completed → workflow "park" completed

Screens: `live-2026-10-19/screen-{run,view,answer,final}.txt`. The question line, the `↳ /answer @ask` hint,
the labelled wake header (`─── ⚙ workflow "park" · step "ask" asked a question · agent follow-up ───`), the
`/workflow status` waiting row, and the completion line all rendered. With the parent busy when the run ended,
the completion arrived as the mid-turn notice (`⚙ workflow "park" completed · run so far 0.17 credits (noted mid-turn):`).
C11 is PASS.

Observation (not a cyril-9104 defect): after SIGTERM to cyril, the KAS `node acp-server.js` child was left
orphaned (re-parented) once; the coordinator stopped it by exact PID.

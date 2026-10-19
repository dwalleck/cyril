# cyril-8wh0 live check (KAS 0.66.43, kiro-cli 2.32.0)

**Result: PASS.** The plan approval rendered as a typed step list in cyril's
overlay before any work step ran.

Binary: cyril built from `03465a72` with `--features kas`, copied out of
`target/`. It was run under hex1b (110x40) with `HOME=<private tmp>`,
`XDG_DATA_HOME=/home/dwalleck/.local/share`, `XDG_CONFIG_HOME=<tmp>/.config`
and `--agent-engine kas`. The workspace was a private temp directory. No
kiro-cli auth command was run, and no token expiry occurred.

The flow is reachable today without enabling the workflow gate (ADR-0013 §5,
gate-off client launch). A recipe file
`ws/.kiro/workflows/workflow-from-prompt.workflow.json` copies Kiro's
`workflow-from-prompt` recipe: one step `create-workflow`, agent
`wf-workflow-creator`, effort `high`, with the `qSo` creator brief inlined.
Brief: "Create the file hello.txt in the workspace root containing exactly
the text: hi".

Steps:

1. `/workflow recipes` lists `workflow-from-prompt` from the workspace.
2. `/workflow run workflow-from-prompt` fails: `_kiro/workflow/new: no
   bundled recipe named 'workflow-from-prompt'`. The name collides with
   KAS's bundled-name lookup (see "Found" below).
3. `/workflow run .kiro/workflows/workflow-from-prompt.workflow.json`
   launches run `wf_34c4a79b2566ee24`. The creator step designs the steps and
   calls `update_workflow`. cyril's overlay then shows:

```text
┌ Permission Required ─────────────────────────────────────┐
│Add 1 workflow step to wf_34c4a79b2566ee24                │
│                                                          │
│    New workflow · 1 step after 'create-workflow'         │
│    Keeps: create-workflow                                │
│    write-hello: wf-coder · model auto · effort low       │
│      Task: Create the file at the absolute path          │
│        /home/dwalleck/.claude/tmp/claude-1000/-home-dwall│
│        eck-repos-cyril/0473cb2a-6b72-4739-a03f-aec2a7f97d│
│        90/scratchpad/cyril-voj…                          │
│                                                          │
│▸ Allow                                                   │
│  Always allow                                            │
│  Deny                                                    │
│  Always deny                                             │
└──────────────────────────────────────────────────────────┘
```

The full screen is in `live-plan-overlay-screen.txt`. The task row ends in
`…` because KAS truncated the step prompt (`task.truncated: true`).

4. Deny. As in the audit, the parent model was then woken to answer the
   paused creator. `/workflow cancel wf_34c4a79b2566ee24` reported "Cancelled
   wf_34c4a79b2566ee24 (was paused)". Quitting cyril gave exit code 0, and
   the hex1b host was stopped by its exact PID.

Note: the screen shows the run from the build before the review repairs
(`3a66e2c3`). Those repairs change only continuation-row indent (+2), string
cleaning and fallbacks, and the snapshots pin the result.

## Found

`/workflow run <name>` with a workspace recipe whose `name` is
`workflow-from-prompt` resolves through the bundled-name path and fails,
while `/workflow run <path>` works. This touches cyril-0asq's design, which
plans to launch the creator with an inline recipe, not a name. It is out of
scope here.

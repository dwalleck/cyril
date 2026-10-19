# Lanes

Six lanes, non-overlapping by artifact. Static lanes read carves and binaries; the live lane
alone spawns processes. Each lane is one background agent (`general-purpose`) given the brief
below with its placeholders filled; the agent reads `traps.md` first.

KAS is cyril's current path (traps 24): `v3-coverage` runs every cycle, the delta lanes rank KAS
gaps by the P1/P2 rules, and v2 live legs run only when budget remains. When the KAS delta is a
single changed file, run `kas-core` and `kas-workflow` as one `kas` lane whose FINDINGS carries a
Workflows section.

## Scopes

| lane | artifact | owns | reusable scripts (`experiments/conductor-spike/`) | previous write-up |
|---|---|---|---|---|
| `kas-core` | `kas-carves/<OLD>` vs `<NEW>`: `dist/server/acp-server.js` (minified), `*.d.ts` (readable), `acp-server.externals.json`, new `node_modules/*` | auth providers and `--auth` options, `_kiro/*` method set, `session_info_update` kinds, `sessionUpdate` variants, capabilities, zod keys of `clientCapabilities._meta.kiro` and `.settings`, `configOptions`, feature flags and `KIRO_FEATURE_*`/`AB_*`, settings intake table, telemetry defaults, memory, MCP, hooks, terminal/background execution, configuration layers, cascade routing; standing coverage of every emitted kind against `convert/kas.rs` | `static-kas-{surface,identifiers,settings,session-info-kinds,flags}-<OLD>.py` | `audit-<OLD>/kas-findings.md` |
| `kas-workflow` | same bundle, workflow engine only | `_kiro/workflow/*` methods and zod params, lifecycle event literals, snapshot/state fields, `recipes_changed`, `load` vs `inspect` subscription, `resume`/`extendRepeat`, auto-wake (`run_complete`, step parent notes, any new trigger or client gate), step-session `consentRound` loop on `allow_always`, watch handlers, recipe schema deltas against `docs/kiro-kas-workflow-recipe-schema-*.json`, bundled recipes and agents (`wf-workflow-creator`, reviewers), `run_workflow`/`save_workflow_definition` schemas, `_kiro/session/notify`, gates reachable gate-off; **ADR-0013 drift check**: diff the `workflow-from-prompt` recipe shape (`zSo`: step `create-workflow`, agent `wf-workflow-creator`, input `workflow_brief`) and the creator brief wrapper (`qSo`) against cyril's `/workflow new` copy, plus the `chat.enableWorkflows` → `settings.workflows`/`settings.goal` marshal in the TUI (`ZOn`) | `static-kas-workflow-*-<OLD>.py` when present | `docs/kiro-<OLD>-wire-audit.md` workflow section, `audit-<OLD>/live-findings.md` |
| `tui` | `tui-bundles/kiro-tui-<OLD>.js` vs `<NEW>.js` (minified bun bundle; the reference ACP client) | keys the TUI sends under `initialize.clientCapabilities._meta.kiro` and `.settings`, `session/new`, `session/prompt`, permission-response and `session/load` `_meta`; `_kiro/*` and `kiro.dev/*` literals called; kinds consumed; slash-command set and new flows (`/memories`, `/workflow`, `/model fallback`, `/effort`, `/upgrade-agent`); how it spawns KAS (`--auth=`, env, Windows supervisor); rollout gate checks; dark features | `static-tui-{gates,vocab}-<OLD>.py` | `audit-<OLD>/tui-findings.md` |
| `host` | `binaries/<ver>/extracted/kirocli/bin/{kiro-cli,kiro-cli-chat,kiro-cli-term}` for OLD and each NEW | why same-`BUILD_HASH` binaries differ; rollout registry rows and percentages; `KIRO_*` env set; settings-key set; new `crates/*/src/*.rs` paths; hidden subcommands and `launch/*.rs`; **standing check — cyril's KAS auth path:** `kiro-cli chat _ get-kas-token` still exists, still takes `--force-refresh`, and still prints `{kind:"getKasToken",data:{accessToken,expiresAt,profileArn,authMethod?,provider?}}` / `{kind:"error",data:{message}}` — confirm in the NEW TUI (`get-kas-token`, handler `Rsr`) and host strings; any rename, argv or envelope change is a P1 finding (cyril-zbu7: cyril falls back to the stored token + an hourly-expiry warning only when the subcommand is missing); Classic deprecation state; launcher auth (`--auth-method`, how auth reaches the node KAS process); v2 `_kiro.dev/*` surface, TuiCommand args, serde literals; embedded doc-manifest delta; changelog lines not tagged `[V3]`. May run archived binaries with `--help` only, under temp `HOME` and temp `XDG_DATA_HOME`; never the installed launcher | `static-host-{env,novel-words,paths,serde,settings,vocab}-<OLD>.py`, `static-rollout-<OLD>.py`, `static-doc-manifests-<OLD>.py` | `audit-<OLD>/host-findings.md` |
| `v3-coverage` | `tui-bundles/kiro-tui-<NEW>.js` (reference v3 client) × `kas-carves/<NEW>` × cyril `crates/` | a **standing** parity inventory, not a delta: nine matrices (handshake keys; agent capabilities read; client→agent methods; every `sessionUpdate`/`session_info_update` kind/`_kiro/*` notification; host callbacks; configOptions/modes/models/effort; every TUI slash command classified TUI-local / KAS-method-backed / v2-only; v2 features lost on KAS; session lifecycle), each row KAS / TUI / cyril verdict + `file:line` / consequence / issue / BLOCKS-FLIP-or-FOLLOW-UP; then ranked flip blockers for cyril-4c4d. Reads the delta lanes' FINDINGS, never redoes them. Before calling a feature missing on KAS, look for where it moved (traps 26) | `static-v3cov-{ctx,kinds,methods,issues}-<OLD>.py` | `audit-<OLD>/v3-coverage-findings.md` |
| `live` | the two KAS carves spawned directly with `~/.local/share/kiro-cli/node`; the cyril binary on KAS_NEW (cyril-on-KAS leg); the v2 host pair only when budget remains | smoke and `--help` of both `acp-server.js`; initialize diff; paired `sweep`; paired `workflow` gate off and on; the static lanes' leads; memory and telemetry handshakes; new-field sweep; the cyril-on-KAS leg (user-visible verdict per flip blocker) | `probe-kas-live-<OLD>.py`, `probe-kas-initsettings-<OLD>.py`, `probe-kas-leads-<OLD>.py`, `probe-v2-live-sweep-<OLD>.py`, `sweep-new-fields.py` | `audit-<OLD>/live-findings.md` (leg recipes, isolation pattern, handshake form) |

Cyril consumers the lanes cite for HANDLED/DROPPED/NOT-SENT: `crates/cyril-core/src/protocol/convert/kas.rs`,
`convert/kas/*.rs` (workflow.rs for the workflow lane), `convert/kiro.rs` (v2),
`crates/cyril-core/src/protocol/kas/{settings,callbacks,discovery}.rs`, `protocol/engine*.rs`,
`crates/cyril-core/src/workflow.rs`, `crates/cyril-core/src/types/workflow_command.rs`,
`crates/cyril/src/domain_mediator/commands/kas.rs`. The `_kiro/*` contract is
`docs/kiro-kas-acp-covenant.md`; lanes read it before interpreting a method.

## Brief template

Fill `<NEW>`, `<OLD>`, `<KAS_OLD>`, `<KAS_NEW>`, `<LANE>`, `<FOCUS>`, and paste the lane's row
from the scope table as its question list. Keep the brief self-contained: it is all the agent sees.

```
You are the <LANE> lane of the kiro-cli <NEW> wire audit for the cyril repo (/home/dwalleck/repos/cyril).
Baseline: <OLD> (docs/kiro-<OLD>-wire-audit.md, structure to mirror). KAS: <KAS_OLD> -> <KAS_NEW>.
User priority: <FOCUS>. Discovery, not only compatibility.

Read first: .claude/skills/kiro-release-audit/references/traps.md (every rule in it is binding) and
experiments/conductor-spike/audit-<NEW>/RUN-STATE.md (established facts: identity classes, what is
already known). The per-version changelog is audit-<NEW>/feed-<NEW>.json; each line is a lead to
locate, never a finding.

Your artifacts: <paths from the scope table>. Your scope: <scope cell>. Out of scope: <sibling lanes'
cells>; cross-reference them, do not analyse them.

Write only inside experiments/conductor-spike/audit-<NEW>/<LANE>/ using Bash heredocs; append to
FINDINGS.md as you go (a cut-off run resumes from it). New reusable scripts go to
experiments/conductor-spike/ as static-<LANE>-<topic>-<NEW>.py or probe-<engine>-<topic>-<NEW>.py;
existing files there are read-only. Static lanes execute nothing from Kiro. Regenerable bulk
(string dumps, normalised bundles, temp homes, anything over ~1 MB a script can rebuild) goes
under <LANE>/tmp/, which git ignores; small evidence stays beside FINDINGS.md. Stop every
background job you started before you hand back.

Cite capture frames as `<capture>.jsonl#<N>"<expect>"`: N is the 0-based line index (Python
enumerate; `grep -n`/`sed -n` are 1-based), <expect> a string the frame contains (the method,
sessionUpdate or kind you are claiming). experiments/conductor-spike/check-citations.py
FINDINGS.md must pass before hand-back; a prose "frame N" outside that form fails it.

Deliverable: FINDINGS.md with sections
  Summary (<=10 lines) ·
  Deltas (table: item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue?) ·
  Unreleased/dark candidates (gate, default, evidence) ·
  Leads for the live lane (exact method, params, handshake fields, expected signal) ·
  Changelog reconciliation (each line -> location, or "not in my lane") ·
  Method notes.
Verify every add/remove by counting occurrences in BOTH versions and quote <=200 chars of context.
Return the full FINDINGS text in your final message.
```

`v3-coverage` replaces the Deltas section with: Summary · Matrix 1–9 (its scope cell) · Flip blockers
(ranked, what a cyril user on KAS-by-default hits on day one) · Follow-ups · Proposed issues/notes
(grep `.rivets/issues.jsonl` first; P1 regression or billed work on KAS, P2 dropped field or
missing route on KAS) · Leads for the live lane · Method notes. It runs after the static delta lanes
hand back, alongside the live lane, and points at `rivets show cyril-4c4d`.

## Live-lane protocol

Appended to the live lane's brief verbatim.

```
You are the only lane that spawns Kiro processes. Legs run strictly serially; never two drivers at
once. The driver (probe-kas-live-<OLD>.py: KAS=<acp-server.js> LABEL=<tag> SCENARIO=sweep|workflow
[GATE=on|off] PROBE_SCRATCH=<your lane dir>/tmp python3 -I ... <out.jsonl>) spawns
<node> --experimental-wasm-modules <acp-server.js> --transport=stdio --auth=acp-callback with a
fresh temp HOME and the real XDG_DATA_HOME, answers _kiro/auth/getAccessToken from
~/.local/share/kiro-cli/data.sqlite3, renews single-flight via `kiro-cli user whoami`, and redacts
tokens. Never `kiro-cli login`/`logout`; if the token row is absent, stop and report.
Engines: KAS_OLD=~/.local/share/kiro-research/kas-carves/<OLD>/node_modules/@kiro/agent/dist/server/acp-server.js,
KAS_NEW=.../kas-carves/<NEW>/...; node=~/.local/share/kiro-cli/node.
Leg order: (0) `--help` of both acp-server.js, initialize-only on NEW with --auth=acp-callback and
with no --auth, diff the initialize results; (1) paired SCENARIO=sweep OLD then NEW;
(2) paired SCENARIO=workflow GATE=off then GATE=on, OLD then NEW; (3) static leads as they
arrive (the orchestrator sends them; include a positive control per handshake probe);
(4) `sweep-new-fields.py --diff <NEW capture> <OLD capture>` per pair, then `unread-fields.py` over
the NEW captures cyril would receive (sweep, gate-off workflow, sub-agent, MCP; never a control
arm), saved as <lane dir>/unread-fields-<NEW>.txt: triage every UNREAD line into a cyril fix, a
docs/wire-ignored-fields.txt line with its reason or tracking issue, or a proposed issue. It
finds only fields cyril never names: a same-named key read elsewhere, a field the ACP crate
parses but cyril ignores, and a field no capture exercised all pass it, so a HANDLED verdict
still needs the consumer `file:line`;
(5) the cyril-on-KAS leg
below, against KAS_NEW; (6) v2 pair only when every KAS item is done and budget remains, archive
bin/ first on PATH, child exe recorded.
For a cyril-vs-KAS question, send the frame Kiro's TUI sends for that feature; v2 frames failing on
KAS is expected and needs no probe (traps 25). Re-run any leg that errored early. Kill orphan node
children of your PROBE_SCRATCH after each leg. Mark every finding LIVE with capture file and frame
index; copy redacted, trimmed captures to experiments/conductor-spike/kas-<topic>-<NEW>.jsonl.
```

## cyril-on-KAS leg

Every other leg drives KAS from a Python probe that imitates cyril; this one runs the cyril binary
itself, so a flip blocker gets a user-visible verdict, not only a wire one. Appended to the live
lane's brief verbatim, after the protocol block.

```
Leg 5 — cyril-on-KAS. Run the real cyril binary against KAS_NEW and record what a cyril user sees
for each cyril-4c4d flip blocker and each P2 parity row the delta lanes or v3-coverage marked
LIVE-pending.

Terminal driver: the hex1b MCP (tools start_bash_terminal, send_terminal_input, send_terminal_key,
wait_for_terminal_text, capture_terminal_text, capture_terminal_screenshot,
start/stop_asciinema_recording, stop_terminal, remove_session). If its tools are absent from your
session, say so in FINDINGS and fall back to .cyril-a14l/oracle-pty.py (pty + pyte; copy-and-adapt
to a real agent and a 160x48 screen as experiments/conductor-spike/probe-cyril-kas-<NEW>.py).

Build once from the primary checkout's HEAD and record the commit:
  env -u CARGO_TARGET_DIR cargo build --features kas   →  target/debug/cyril
Spawn, inside one hex1b bash terminal (160x48, asciinema to <lane dir>/cyril-kas/<scenario>.cast):
  HOME=<fresh temp> XDG_DATA_HOME=<real> KIRO_KAS_SERVER_PATH=<KAS_NEW> KIRO_AGENT_PATH=<node> \
    <repo>/target/debug/cyril --agent-engine kas
with cwd = a temp workspace that is `git init` + `git add`ed (cyril's file completer reads git).
Until cyril-zbu7 lands, cyril serves the stored access token and never renews it: right before the
leg run ONE `kiro-cli chat _ get-kas-token --force-refresh` (single-flight, parse in-process, never
print the token) for a fresh hour, and keep the leg inside that window. Once zbu7 lands, cyril renews
through the same subcommand and a leg that crosses an expiry is itself a zbu7 regression check.

Scenarios (one cyril process each; seed everything under the temp HOME or workspace):
  S1 startup     wait for the toolbar; record engine, model and mode shown, and the time from
                 launch until the model appears (it arrives in a config_option_update).
  S2 commands    seed one skill and one saved prompt; type `/` and record the completion list;
                 run `/<skill> hello`; record what the user sees (cyril-oieu).
  S3 settings    `/thinking` toggle (positive control, a KAS config option cyril already writes);
                 then `/model`, `/effort`, `/mode` (cyril-vh69).
  S4 trust       prompt a harmless shell command, answer the approval with "Always allow", start a
                 new session, prompt the same command; record whether approval is asked again, and
                 diff the seeded agent file before/after (cyril-auxt).
  S5 sub-agent   one prompt that makes KAS delegate (invoke_sub_agent); record how the child's tool
                 calls and its answer appear in the transcript (cyril-ebqu, cyril-1v25).
  S6 MCP         seed settings/mcp.json with the stdio fixture from audit-<OLD>/live/fixtures/; call
                 its tool; record the "Tool Load" row and the call's display (cyril-1v25, cyril-2yn8).
  S7 KIRO_HOME   absolute KIRO_HOME=<temp>/kh with a distinct agent there and another in
                 $HOME/.kiro/agents; record which agent cyril lists and which KAS loads (cyril-d7ve).
For each scenario save capture_terminal_text after every step to <lane dir>/cyril-kas/<scenario>-<step>.txt
and copy cyril.log from $HOME/.config/cyril/ (the private HOME, not the workspace). S2, S4, S5 and S6 are billed (about six turns in total); S1, S3
and S7 are zero-turn. Stop and remove every terminal session when the leg ends.

FINDINGS row per scenario: issue | expected (from the static row) | observed on screen | evidence
file | verdict CONFIRMED / REFUTED / CHANGED. A symptom that differs from the wire-level finding
is a finding in itself.
```

hex1b is configured for `~/repos/dotnet-agent` only (2026-10-16); enable it for cyril before an
audit with `claude mcp add -s local hex1b -- hex1b-mcp` (local scope: this checkout, not committed)
so the live lane inherits its tools.

## Orchestration

- Spawn with the `Agent` tool, `subagent_type: general-purpose`, no model override; the hand-back
  is the lane's FINDINGS text.
- In the **constrained** tier, start the next static lane only after the previous one hands back.
  In **full**, spawn the four static lanes and the live lane in one message.
- Spot-check every hand-back before ticking it: re-count two or three of its add/remove claims in
  both artifacts, open each cited cyril `file:line`, and run `check-citations.py` on its FINDINGS.
  Record the result (confirmed, corrected) in `RUN-STATE.md` Established facts.
- When a static lane returns, forward its "Leads for the live lane" section to the live lane with
  `SendMessage` if it is running, otherwise fold the leads into the live brief before spawning it.
- After every hand-back: copy nothing (the lane wrote in place), tick the lane in `RUN-STATE.md`,
  and note any trap the lane hit for `traps.md`.
- A lane that stopped early (API error, usage limit) is re-spawned with the same brief plus
  "Resume from the existing FINDINGS.md; sections already complete stay as they are."

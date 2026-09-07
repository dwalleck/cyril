# Evidence: cyril-jlxx

Date/platform: 2026-09-06, Linux x86_64. CLI 2.21.1; pinned KAS archive is @kiro/agent 0.58.7. Native Windows is not tested.

## Archive provenance (PR #116 review repair)

The following record preserves the original 2026-09-06 investigation, approval
and implementation campaign against CLI 2.21.1 / KAS 0.58.7 on Linux x86_64.
Native acceptance captures have their original UTC timestamps (including
2026-09-07); they are not a new execution or native Windows acceptance. Later
review-fix verification must be recorded separately, not inferred from these PASS
records. Native Windows remains cyril-6y1s; desktop/provider release gates remain
with cyril-ukmu.

Personal home prefixes are consistently replaced with `/home/REDACTED_USER`;
synthetic `/tmp/cyril-jlxx-*` roots, canaries, session/process identities and
independent effect observations are unchanged. Environment records contain names,
not credential values. No authentication store, build output or raw PR review
report is archived.

`Cargo.toml` and probe sources retain the historical dependency recipe. The
original standalone `Cargo.lock` mentioned below is intentionally excluded with
`target/`; rebuilding resolves dependencies again, so this is not a byte-identical
build archive. `probe-mcp.runner.py` retains its historical absolute script path:
substitute the local checkout for `/home/REDACTED_USER/repos/cyril-wt-feat-cyril-jlxx`
before replay. Fixture roots in captures describe the original run; recreate the
recorded synthetic fixtures rather than expecting those temporary paths to exist.

`native-acceptance.py` drives the historical `review_smoke` example retained in
git at PR #116 base `1136dabdda0bb5905df54cf9cd6dd005d771d921`:
`cargo build -p cyril-workbench --example review_smoke`, then
`python3 .cyril-jlxx/native-acceptance.py` in that revision with the supported CLI
and existing native sign-in. This executes a live agent; archiving ran neither
command. Historical `artifact://` references in `checkpoint-core.json` are
session-local provenance, not portable attachments or newly rerun gates.

Chronology: `module-baseline.json` and `shape-preapproval.json` precede approval;
`shape-before-implementation.json` precedes integration; `checkpoint-core.json`
records slice 1; `native-acceptance.json` and native A/B stdout/stderr record the
original backend campaign. The original design approval and plan below remain
historical, not retrospective approval of subsequent review fixes.

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | Supported model selection through current public bridge | Can KAS advertise/confirm Sonnet 4.6 and return a real model-bound reply? | PASS — profile SetMode plus streamed configuration and authenticated reply |
| P2 | Native configured inspection authority | Can allowed evidence reads succeed while unauthorized reads fail and write/shell/publication authority is absent, corroborated by actual permissive configured operations? | PASS — scoped global native policy plus granular read-only profile; not inline profile rules alone |
| P3 | Inherited configuration isolation | Which native inherited operations escape profile-only restrictions, and does isolated configuration plus hooks-off prevent them? | PASS — learned profile-only isolation is insufficient; clean-root MCP and hooks-off controls agree with independent markers |
| P4 | Existing authentication without provider/resource authority | Can the reviewer authenticate while provider-token environment variables and unrestricted ResourceFS/MCP are unavailable? | PASS — native existing Kiro sign-in, actual process environment names, mediated random evidence, clean-root MCP comparison |
| P5 | Existing ownership can isolate fresh runs | Do independent consumers have distinct processes/sessions/private state, deny cross-run reads, accept a second same-session prompt and terminate descendants? | PASS — two concurrent independent public-bridge consumers and repeated fresh-process runs |
| N1 | Existing ACP and Jira access | Established user facts; not connectivity investigations. | N/A — no unverified premise |
| N2 | Native Windows workbench acceptance | Separate verified cyril-6y1s gate. | N/A — Linux evidence does not establish Windows |
| N3 | Workbench production operations and performance budgets | Feature not yet implemented; design/build obligations, not claims about an existing implementation. | N/A — downstream design/build |

## Data

Production-shaped synthetic evidence only: random authorized/outside text, replaceable mutation file, harmless printf marker, synthetic MCP read_canary servers, probe-authored hooks, two separate run identities. No PR code, real publication, provider calls or credential copying. Original Kiro authentication remains native. Necessary private runtime logs/session-state writes are separate from reviewed evidence writes. Probe roots are under /tmp/cyril-jlxx-*; all precise root paths, fixture JSON, commands and observations are retained in the listed artifacts.

## Probe

- `probe.rs`: standalone consumer of current public `spawn_bridge`, `SpawnConfig`, five-part `BridgeHandle::split`, `SetMode`, `SendPrompt`, permission cancellation and `Shutdown`. No ACP/schema types imported outside core.
- `probe.py`: launches that consumer with an explicit environment; isolated HOME, KIRO_HOME, config/cache/temp roots, existing XDG_DATA_HOME for native CLI authentication, narrowly retained transport variables. Provider/cloud token variables are not forwarded. This instrument isolates the whole consumer process, not just its agent child; no claim that SpawnConfig already offers a per-child environment API.
- `probe-tap.py`: transparent CLI stdio relay that records only `_kiro/tools/didChange` and `_kiro/policy/changed` notifications. Does not rewrite requests or execute tools. Its `--version` forwards to the real CLI.
- Build: `cargo build --manifest-path .cyril-jlxx/Cargo.toml` in this worktree; passed. Standalone Cargo.lock pins the probe build.
- Run: `python3 .cyril-jlxx/probe.py ROOT PROFILE PROMPT_FILE [kas|off] [SECOND_PROMPT_FILE]`. Set JLXX_CAPTURE_TOOLS=1 to retain tools.jsonl under ROOT.
- MCP instruments: `probe-mcp.py`, `probe-mcp.runner.py`; exact fixture/configuration/command/result record in `probe-mcp.evidence.json`.
- Hook instruments use the same public bridge consumer; exact hook JSON, profile, policy, prompt and run commands in `probe-hooks.results.json`.
- Worktree: /home/REDACTED_USER/repos/cyril-wt-feat-cyril-jlxx.

## Oracle

- Model: separately invoke supported `kiro-cli chat --list-models --format json`; compare the named Sonnet 4.6 selector with runtime config and actual reply, not a guessed machine name.
- Resource effects: independent Python file reads compare random authorized bytes, outside secret absence from failed-tool output, mutation bytes and shell marker presence. A native permissive control exercises the same configured tools through the public bridge; it is not a direct host-executor bypass.
- Hooks/MCP: independent marker files, including MCP server-side startup/invocation timestamps, compared with actual callbacks/tool results. A model refusal or missing permission request is not treated as enforcement.
- Isolation/environment: independently enumerate /proc process cwd, names and environment variable names (never secret values), then observe every recorded PID gone after shutdown. Compare distinct session IDs and per-root runtime state with separate random identity files and cross-run denied reads.

## Comparisons

| ID | Probe output | Independent observation | Verdict |
|----|--------------|-------------------------|---------|
| P1 | `model-proof.txt`: SetMode selects jlxx-preflight; ConfigOptionsUpdated confirms claude-sonnet-4.6; exact JLXX_AUTH_OK; authoritative terminal; SHUTDOWN Ok(Ok(())). | `model-catalog.json`: supported CLI catalog independently names Sonnet 4.6 and selector. | PASS |
| P2 | `jlxx-policy-operations.txt`, `jlxx-full-denials.txt`: actual authorized read succeeds; outside read fails with native deny rule/source; unexpected read/subagent permission requests canceled, tool calls fail. | `operation-oracle.json`: exact authorized bytes returned; mutation remains ORIGINAL_SYNTHETIC; no shell marker. Outside token is not returned by denied tools. | PASS |
| P2 control | `jlxx-operation-control.txt`: configured native outside read, Write File and Run Command all complete. | Mutation becomes MUTATED_SYNTHETIC; marker contains EXECUTED_SYNTHETIC. Same real tool path, safe fixture-only control. | PASS |
| P2 exposure | `tool-inventories.json`: after selecting granular read_file profile, native tools notification contains only read tag. Permissive control advertises read/write/shell/web and actually executes writes/shell. | Passive wire observation plus independent effect contrast; not the model's claim that tools are unavailable. No MCP/publication tool is supplied in clean-root reviewer configuration. | PASS |
| P2/P3 precedence | `jlxx-inherited-permissions.txt`, `inherited-permissions.json`: global fs_read deny ** except evidence/** plus allowed evidence subtree; global write/shell deny; workspace broad allow rules. Actual outside read fails with global deny; authorized reads succeed. | Evidence remains unchanged and no shell marker despite inherited workspace allows. | PASS |
| P3 hooks | `probe-hooks.results.json`, `.off.log`, `.kas.log`: native hooks-off compared with native hooks enabled; both complete authorized reads. | Off: no marker. Enabled control: marker contains sessionStart and preToolUse; preToolUse execution callback also observed. Enabled hooks write despite shell/write deny. | PASS — learned explicit hooks-off is mandatory |
| P3 MCP | `probe-mcp.evidence.json`, `probe-mcp.summary.json`: inherited global/workspace servers start before SessionCreated(vibe); profile includeMcpJson:false/mcpServers:{} does not remove them. Reviewer plus inherited mcp allow actually invokes both. Cancel-by-default blocks invocation without that allow. | Server-side startup and invocation markers confirm outcomes/timing. Clean home/workspace, with captured configs only under inert evidence subdirectories: zero startups or invocations. | PASS — learned clean roots are mandatory before spawn |
| P3 delegation | Full-exposure arm actually attempts invoke_subagent and receives canceled permission/Failed. Final profile whitelists only read_file. Core settings.rs:80-88 defaults subagentOrchestration ON even with empty cli.json; do not claim absent user settings disable that flag. | Actual failed subagent operation, no write/shell marker; final read-only inventory and granular whitelist. Probe sends no Workflow/ExtMethod operation, and the workbench must not expose those commands to reviewer content. | PASS for configured surface — native tool restriction, not absent orchestration flag, is the control; no enabled unrestricted workflow safety claim |
| P4 | `reviewer-process-environment.json`: original consumer launch supplied synthetic GITHUB_TOKEN/AZURE_DEVOPS_EXT_PAT; live consumer, CLI and KAS node environment names omit both. Authenticated Sonnet turn still succeeds. | Independent /proc observation; random mediated evidence is readable, outside evidence is denied. MCP clean-root control supplies no unrestricted ResourceFS/publication server. | PASS |
| P5 | `isolation-a.txt`, `isolation-b.txt`: distinct sessions in independent concurrent consumers; own token read succeeds, foreign-root read fails; two authoritative terminals and exactly one second same-session prompt each. | `isolation-live.json`: disjoint consumer/CLI/KAS PIDs. Each output contains only its own random identity token. `isolation-after.json`: every observed PID gone, neither fixture has live cwd processes. | PASS |
| P4/P5 state | Fresh isolated homes accumulate native session-index, logs and per-session messages/session files; reviewed identity files unchanged. | `runtime-state-files.json`: private runtime state paths differ by root/session. Native Kiro auth state is retained, not copied into review evidence or another login store. | PASS |

## Validated / learned

- P1: `kiro-cli --v3 acp` is accepted; `kiro-cli acp --v3` is not. Both global --v3 and --agent-engine=v3 reject --agent/--model with CLI 2.21.1 despite generic help. Native on-disk profile discovery plus public SetMode before first prompt works; model arrives through ConfigOptionsUpdated, not SessionCreated.current_model. Plain supported CLI model listing succeeds; the --v3 chat listing variant entered an Opening browser spinner, which was not evidence of failed authentication.
- P2: Native global permissions.yaml is the authority for outside-workspace rules. The passive tap explains the initial profile-only failure: KAS reports a nonfatal policy error and skips the agent-profile rule targeting an absolute outside-workspace path. Inline profile rules are workspace-scoped; this is not evidence that their entire schema is broken. Final reviewer has no such inline outside rule. Use global deny with match ** and exclude authorized evidence/**, then allow that subtree. Exclude only removes a match; it does not itself grant permission. Deny defeats inherited workspace allows.
- P2: Coarse read/write/shell category names must not be confused with tool IDs. Final profile uses granular read_file. Native inventory confirms read-only exposure; actual permissive controls verify write/shell execution works when allowed.
- P3: Native hook execution is independent of ordinary write/shell permission rules. Explicit KasHooksMode::Off prevents the observed inherited hooks. MCP discovery can execute before profile selection, and inherited MCP grants can survive includeMcpJson:false. Workbench-owned clean HOME/KIRO_HOME/cwd configuration roots are needed before process creation; merely switching a profile in a user checkout is insufficient. Captured untrusted configuration placed beneath an inert evidence subtree did not start MCP processes.
- P4: An explicit child/consumer environment can retain Kiro authentication and corporate transport configuration without forwarding provider-token variables. Necessary native runtime/session writes remain in private KIRO_HOME, not the evidence tree. No real credentials or publication were used for safety proof.
- P5: Current public bridge/process ownership supports independent consumers with distinct state and clean shutdown, including two turns on one run. This does not yet prove a new production backend implementation; its gates must exercise these same boundaries.
- Probe correction: initial operation profile inherited a contradictory no-tools system prompt from the tool-less preflight. Removed before the controlled operation pair. An earlier model refusal is excluded from enforcement evidence.

## Related issues

- Consulted: cyril-jlxx, cyril-ukmu, cyril-6y1s; bounded tracker keyword scan inspection/reviewer/native policy/permission/isolation. Related cyril-brui/cyril-tpwn (HOME/XDG), cyril-gk17/cyril-jiyn/cyril-mq15 (hooks/trust), cyril-ufld/cyril-gn07 (consent), cyril-ss5i (seed isolated roots), cyril-qc00 (frontend tool capability), cyril-sszf/cyril-6vo6 (delegation/modes). Historical KCC kiro-j8b0 is source-qualified planning prior art, not proof of execution.
- Filed: none. Observations revise the native isolation model and identify integration requirements owned by current cyril-jlxx; no unrelated upstream defect or deferred future implementation is asserted. Native Windows remains cyril-6y1s.

## Hand-off

Empirical comparisons PASS within their stated Linux/configured-surface bounds. No production code or feature tests changed. Hand off to falsifiable-design; native clean-root isolation, hooks-off, granular tools, global scope policy, model confirmation and cancel-by-default are load-bearing evidence, not optional hardening. Do not promote probe instruments into production code.

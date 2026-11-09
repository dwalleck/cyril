## 8. Live results

Live lane (`audit-3.1.0/live-findings.md`, 900 lines, 407 frame citations checked by `check-citations.py`): KAS 0.66.43 (2.32.0 carve on node
v22.22.2) vs 0.66.91 (3.1.0 carve on its own node v22.23.4), same day, same backend; the archived 3.1.0 host on empty stores; the real cyril binary
(build `e3a3868d`) on 0.66.91 through hex1b. Redacted bundles: `experiments/conductor-spike/kas-{live-sweep,workflow-gateoff,workflow-gateon,
subagent-parent-stream,signin-refusal,approve-for-me,mcp-elicitation,hooks-v2,workflow-launch-option,unparseable-command}-3.1.0.jsonl`.

| leg | what ran | result |
|---|---|---|
| 0 | `--help` of both servers (no help surface); initialize-only on 0.66.43 / 0.66.48 / 0.66.58 / 0.66.62 / 0.66.70 / 0.66.91 (callback auth) and 0.66.91 (no auth) | `extensionMethods` grow per carve as the static diff predicted (+`configuration/set` 0.66.62, +`memory/getSyncStatus` 0.66.70, +`memory/sync`, +`get_network_configuration` 0.66.91); `autoCompactionThreshold` from 0.66.58; `authMethods` unchanged; cyril's `thinking` key logs `configuration.intake.unknown-key` on both engines |
| 1 | paired sweep (initialize → session/new → read prompt → shell prompt with permission → method census → set_config_option census → close) | 0 × -32601 on anything cyril sends; `session_usage` after every `turn_completion` and replayed on `session/load` (NEW only); turn frames gain `executionId`/`turnId`/`timestamp`/`boundarySource`; `summarizationThreshold` on `context_usage`; `_kiro/memory/syncStatus` pushed on connect; `effortLevel`/`thinking` accepted-not-echoed on both; permission request shape unchanged |
| 2a/2b | paired workflow gate-off (6 runs) and gate-on (11 runs), fixed recipe set | all runs MATCH across engines; NEW adds `credits[]`, `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch`, `notifyId`; `_meta.kiro.preview` removed; `workflowLaunch` option offered gate-on; the pause wake is unchanged |
| 3 | 41 forwarded leads (KAS-L1–13, WF-L1–10, TUI-L1–13, HOST-L1–6) + LIVE-A–H | verdicts in the tables above; summary: CONFIRMED 28, CHANGED 7 (tier, deny-rule text, hooks client path, recap, displayText, shellEnvironment, code-index), REFUTED 3 (trust prompt, recipe trust gating, wake race), NOT-REACHED 7 (model-fallback frames, OAuth completion, peer messaging, wake reply length, memory handed to sub-agents, session-level `turn_timing`, `active_file`), NOT-RUN 2 (HOST-L6, WF-L10) |
| 4 | `sweep-new-fields.py --diff` per pair; `unread-fields.py` over every NEW capture cyril would receive | 70 UNREAD paths, all triaged (allowlist lines + the issues in §9); dropped-by-consumer items the key-name check cannot see: `session_usage`, `agentSubtaskId` on parent chunks, `recap`, `turn_timing`, workflow `credits[]` |
| 5 | cyril `--agent-engine kas` on 0.66.91 (temp HOME, real data home, `KIRO_KAS_SERVER_PATH` + `KIRO_AGENT_PATH` = 3.1.0 node) | S1 startup PASS (toolbar in ~2 s, token via `get-kas-token`, `shell_type` in 2 ms); S2 skill dispatch PASS; S3 `/thinking` `/model` `/effort` `/mode` PASS; V4 effort badge PASS; V5 `/usage` shows per-turn credits only (`session_usage` dropped); V6 node v22.23.4 runs the bundle; **S4–S8 and V3 NOT-RUN** (budget after the auth stop; seeds and launch lines in `live/tmp/leg5-launch.txt`) |
| 6 | v2 pair | DROPPED (Classic retired; budget) |

**Host (archived 3.1.0 `kiro-cli`, empty stores):** logged out, it answers `initialize` itself in 0.5 s with `authMethods:[{id:"kiro-login", …"Run
'kiro-cli login'…"}]` — or `{type:"terminal", id:"kiro-login-terminal", args:["--login"]}` when the client declares `_meta.terminal-auth` — and refuses
`session/new` with -32000 "Authentication required…" (2.36.0 and 2.35.2 launchers exit rc 1 instead: the 2.37.0 line). `--agent-engine v1` is accepted
without a panic. With `--auth-method client` (accepted only when the resolved engine is v3, which makes it a token-free route discriminator): the
unlisted name `cyril` → **v3** (node child, 34 `extensionMethods`, five forwarded `getAccessToken` requests); `--agent-engine v2`, `acp.agentEngine:"v2"`
and the snapshot name `acp-probe` → v2. `kiro-cli _ extract-runtime` installs to `$KIRO_RUNTIME_DIR/<ver>-<hash>/{node,bun,tui.js,kas/…}` (helper default
`<bin>/../lib/kiro-cli/`). **Orchestrator probe** (`probe-kas-host31-defaultdir-3.1.0.py`, `audit-3.1.0/orchestrator/host-defaultdir-3.1.0.jsonl`):
a plain `kiro-cli acp --agent-engine v3` with `KIRO_RUNTIME_DIR` unset extracts to `<data dir>/kiro-cli/kas/3.1.0-<sha256>/` and runs `<data dir>/kiro-cli/node`
— the layout cyril's discovery searches; the live lane's "P1 candidate" (nothing under `kas/`) came from arms that all had `KIRO_RUNTIME_DIR` set and is
**refuted**: default installs keep working, the runtime-dir layout is the opt-in P3 (cyril-8ts2).

**get-kas-token on 3.1.0 (LIVE-F):** against a copy of the real store, `kiro-cli chat _ get-kas-token` (no `--force-refresh`) printed
`{kind:"getKasToken", data:{accessToken, expiresAt, profileArn, provider:"Enterprise"}}`, `expiresAt` equal to the store row, idempotent, row unchanged —
cyril's parser fits. **Auth incident:** the next arm ran the archived `kiro-cli acp` on the same copy (the brief had said empty store); the copy's token
row changed within 6.7 s (host `state` +1 row, `extracted_kas_versions` pruned 4 → 2; the only auth log line `acp-peer.refreshToken.ok 8 ms`, a
callback read). Whether that consumed the single-use refresh token is unprovable (the copy was deleted); the real store was never opened by a 3.x
binary and its `expires_at` stayed 23:50:59Z. All later legs ran token-free; HOST-L6 was NOT-RUN. New trap: §10.

**Corrections to static rows from live:** `recap` and `displayText` replay already existed on 0.66.43; the memory rollout flip did not change the
ACP default; MCP `structuredContent` produces no extra block on either engine; the wake race behaves the same on both engines; `shellEnvironment` is
applied agent-side only; client-executed hooks exiting 2 without output do not block on either engine. cyril's own S1 run also shows KAS labels the
ordinary `getAccessToken` round trip `acp-peer.refreshToken.ok` (809 ms = the `get-kas-token` subprocess), so that log line alone never proves an
OIDC refresh. Side observation: SIGTERM to cyril left its KAS node child alive (cyril-qdcs, open).

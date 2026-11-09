## 9. Issues

All labelled `kiro-3-1-0` plus `kas` (and `w1` / `v2` where they apply); `ready-for-agent` unless noted. The KAS parity epic cyril-c6hv gains the
three default-path children (cyril-lpdk, cyril-t2w5, cyril-asrc).

| id | P | title |
|---|---|---|
| cyril-mq15 (re-scoped) | P2 | Workspace trust prompt before `session/new`: ask before loading a folder's `.kiro` hooks, agents, MCP servers and recipes (owner decision 2026-11-08) |
| cyril-lpdk | P2 | KAS 0.66.91: sub-agent replies stream on the parent session (`agentSubtaskId`) and cyril renders them as parent text |
| cyril-t2w5 | P2 | KAS `session_usage`: adopt the authoritative session credit total (replayed on `session/load`) for `/usage` and the footer |
| cyril-bgvj | P2 | KAS 0.66.91 workflow step credits and run progress on the wire: model `credits[]` and `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch` |
| cyril-k3hf | P2 | V2 wrapper spawn must pin `--agent-engine v2`: kiro-cli ≥2.36.0 routes `kiro-cli acp` by `clientInfo.name` and a 3.x host hands the unlisted name `cyril` the v3 engine |
| cyril-asrc | P2 | Honour `chat.workflowLaunch` and expose KAS's `workflowLaunch` (Workflow use) option; sessions recorded before 0.66.58 load as delegate-all |
| cyril-aoby (raised) | P2 | KAS: handle the `model_fallback` / `model_routed` kinds — fallback is on by default since 0.66.91 |
| cyril-5anx | P3 | Session recap: map `chat.enableSessionRecap` to `settings.sessionRecap` and render the `recap` kind |
| cyril-zfoo | P3 | Approve for me on KAS (opt-in): set `KIRO_TRUST_CLASSIFIER_MODE=enforce` on the KAS child and render `trustClassifierApproved` rows |
| cyril-0yme | P3 | Interactive MCP elicitation: answer form and url elicitations like Kiro's TUI instead of a visible decline |
| cyril-8ts2 | P3 | KAS discovery is blind to `KIRO_RUNTIME_DIR` / `kiro-cli _ extract-runtime` installs (opt-in layout, 2.37.0+) |
| cyril-1nto | P3 | Clear the remote-MCP OAuth notice on `_kiro/mcp/oauthCompleted` and read `openExternalUrl {serverName, oauthCompletion, oauthFlowId}` |
| cyril-zmji | P3 | cyril-hosted hooks: an `executeHook` reply with exit code 2 and empty output does not block the tool on KAS; opt into hooks v2 for PreCompact/PostCompact |
| cyril-smy8 | P3 | Approval overlay shows a blank preview for KAS skill-load tool calls |
| cyril-0ax4 | P4 | Always-allow scope page: offer Kiro's `<cmd> <sub> *` subcommand tier |
| cyril-f4hi | P4 (needs-triage) | Consult `KIRO_DEFAULT_SETTINGS` (3.1.0 root-owned managed defaults) in cyril's settings marshal |

Notes appended this cycle (existing issues): cyril-aoby, cyril-ojum, cyril-0o7e, cyril-0rn8, cyril-53qx, cyril-58uv, cyril-6s21, cyril-s21o,
cyril-0na7, cyril-q8gq, cyril-hzkb, cyril-iq2c, cyril-68ny, cyril-7p7n, cyril-7c0s, cyril-497j, cyril-ebqu, cyril-r5pl, cyril-99ds, cyril-4u4a,
cyril-fb1m, cyril-inwg, cyril-xehm, cyril-0asq, cyril-w55l, cyril-1aq0, cyril-a7nk, cyril-cxd2, cyril-zjm5, cyril-tikf, cyril-c6hv, cyril-oswq,
cyril-lnxg, cyril-8alk, cyril-po2z, cyril-m8kq, cyril-a8se, cyril-7xxk, cyril-0b8t, cyril-qdcs, cyril-lxuo, cyril-j66p, cyril-4cz1, cyril-1v25 (closed),
cyril-zbu7 (closed).

Allowlist (`docs/wire-ignored-fields.txt`) lines added for the 70 UNREAD paths of the NEW captures (turn identity fields, compaction thresholds,
memory sync state, `notifyId`, `firstTokenReceivedAt`, workflow progress fields → cyril-bgvj, `trustClassifierApproved` → cyril-zfoo, `policyDenial`
read via serde rename, MCP form field names, `appliedPlanFeedback`/`originalUserMessages` → cyril-4u4a); the dead `session/update.update._meta.kiro.preview.*`
line removed. `unread-fields.py` exits 0 on the NEW captures afterwards.

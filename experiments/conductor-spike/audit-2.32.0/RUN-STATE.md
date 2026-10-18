# Audit 2.32.0 — run state (started 2026-10-18)
OLD: 2.31.1  NEW: 2.32.0  folds: none (2.32.0 is the only release after 2.31.1; user first said 2.32.2, which no channel publishes — confirmed 2.32.0)
focus: KAS v3 track — new/updated commands (`_kiro/*` methods, slash commands), messages carrying new fields, feature flags for new or limited-release features. Live testing encouraged; logged in.
tier: upgraded by user 2026-10-18 to full-style static fan-out: kas + tui + host in parallel; then v3-coverage + live together

## Releases
| ver | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS class | TUI class | host class | headline |
|---|---|---|---|---|---|---|---|---|
| 2.32.0 | 2026-10-15T21:16Z | d6fb1e94 (== 2.31.1, trap 20) | 9d1527e4… (== latest manifest) | 0.66.43 | distinct: acp-server.js 12,702,158 → 12,963,334 B (+261,176); externals.json; package.json; node_modules −sharp −@img −@huggingface −protobufjs −@protobufjs −long −flatbuffers −guid-typescript −platform −detect-libc | distinct (13,692,707 → 13,723,687 B) | distinct (all three) | [V3] write-approval custom folder scopes; `/workflow run` confirmation form w/ plan preview; output styles; workflow steps use Powers + MCP (design); step sessions open on task (not parent copy); review loops pause after 2 rejections; plain-language changes to running workflows; sharp/libvips unbundled; OSC 7501 program status |

## Established facts
- hex1b MCP connected in the orchestrator session (ping OK 2026-10-18T17:15Z after /reload-plugins); the live lane uses it for the cyril-on-KAS leg, oracle-pty.py fallback only if a subagent lacks the tools.
- Tarball sha256 9d1527e4ed76197a48241e093f5240e39dfb3177f098f17b3dd019032dd508f3 == latest manifest (first download was a truncated 59 MB leftover; deleted and re-fetched).
- installed kiro-cli / kiro-cli-chat / kiro-cli-term == archived 2.32.0 (cmp).
- KAS carve == self-extracted `~/.local/share/kiro-cli/kas/2.32.0-f3c56062…` (diff -rq empty).
- TUI carve == installed `~/.local/share/kiro-cli/tui.js` (cmp silent; sha256 eba39f7b7914f608).
- KAS 2.31.1 → 2.32.0: all 12 `dist/server/*.d.ts` byte-identical; only acp-server.js, acp-server.externals.json, package.json differ under @kiro/agent. externals: −`@huggingface/transformers`, +`onnxruntime-node`. Carve 176M → 153M.
- Frame sizes (csize/dsize): KAS tar 27,808,487/183,255,040 → 20,826,442/159,590,400; Node ELF and Bun ELF identical sizes (32,031,786/124,679,552 and 29,887,616/79,500,640); TUI 2,124,271/13,692,707 → 2,133,192/13,723,687.
- Host sizes: kiro-cli 43,925,280 → 43,931,168; kiro-cli-chat 216,000,680 → 207,496,744; kiro-cli-term 34,058,656 → 34,060,512.
- KAS delta is one file ⇒ single `kas` lane (core + workflow) per lanes.md.

- tui lane spot-check (orchestrator): `_kiro/mcp/oauthCompleted` 0→1, `KIRO_NO_PROGRAM_STATUS` 0→1, `chat.modelPicker.sortBy` 0→1, `oauthCompletion` 0→3 in TUI, all 0 in KAS 0.66.43 — CONFIRMED; cyril kas.rs:67-91 sends only rejectionReason — CONFIRMED; workflow.rs:606-617 ignores plan — CONFIRMED; no outputStyle in crates/ — CONFIRMED; check-citations 0 citations, pass.

- host lane spot-check (orchestrator): `KIRO_STANDALONE_CUTOVER` 0→1, "did not advertise cloud sandbox sessions" 0→1, `chat.disabledModelFallbacks` 0→4, `inherited_model_fallback` 0→1 in kiro-cli-chat — CONFIRMED; rollout `v3_prompt` `internal_treatment_percent` 50→75 (2 registry copies each, raw binary grep) — CONFIRMED; fingerprint.rs:30-36 reads only `_meta.kiro` is-object — CONFIRMED; no `executionTarget` in crates/*/src (fixtures only) — CONFIRMED; check-citations pass.

- kas lane spot-check (orchestrator): `diagrams:` 0→2, `workflowPlan` 1→12, `create_diagram` 0→7, `agentHostsForegroundShell` 0→4, `KIRO_EVAL_STRATEGY` 0→1, `"Learning"` 0→1, `max-tokens-recovery` 0→3, `session-reasoning` 0→1 — CONFIRMED. shell_type gap: KAS dialect mapper (`cb` 0.66.34 / `jh` 0.66.43) accepts only concrete names; input "posix" → "unknown" in BOTH; cyril host_shell.rs `wire_name()` returns "posix" for ShellKind::Posix — CONFIRMED (static; live KAS-L1 decides P1). settings.rs:79/:96 still marshal/default `thinking` — CONFIRMED. check-citations pass.

- v3-coverage spot-check (orchestrator): KAS S8e `scope:w?.scope||"session"` on allow_always + `persistConsent` (session/user/workspace) — CONFIRMED in 0.66.43. Kiro TUI consent echo `{kiro:{consent:{...capability, scope, ...resource, ...workspaceRoot}, ...rejectionReason}}` present in BOTH 2.31.1 and 2.32.0 (spread syntax; a literal `consent:{capability` grep finds 0) — CONFIRMED; **corrects the 2.31.1 audit/coverage M5 row ("TUI sends no consent echo")**. cyril-oieu fixed by #169 (in_progress awaiting closure), cyril-vh69 closed, cyril-zbu7 open — CONFIRMED via rivets. check-citations pass. Lane cited HEAD 7c7b0d1d = last code-bearing commit before this session's tracker commits.

- live lane spot-check (orchestrator): check-citations 135/0 — PASS. k8wc frames: kas-lead-shell-2.32.0.jsonl#45 and kas-lead-shell-2.31.1.jsonl#44 carry `"persistableConsent": false, "persistableConsentReason": "Kiro could not parse this command…"` — CONFIRMED (KAS withholds Always-allow under "posix"). Credential scan of the 16 committed `*-2.32.0.jsonl` bundles (aoa…/JWT patterns): clean.
- **live S2 (cyril-oieu "command failed") is VOID for HEAD**: the leg-5 cyril binary was built 2026-10-18 12:19 -0500 at b9b6e95d; #169 (ff878634, the oieu fix) merged at 12:38 from another session mid-audit (`git merge-base --is-ancestor ff878634 b9b6e95d` → not an ancestor). cyril-oieu's own note (7c7b0d1d) records the live re-qualification on 2.32.0 with the fix. S1/S3–S7 are unaffected by #169.
- **Side effect, user-visible:** HOST-L1/L3 created 3 REAL cloud sessions (03f28a3c…, 01584308…, 5d15eb4a…) because the account is cloud-entitled; the lane deleted them with `kiro-cli chat --agent-engine v3 --delete-session <id>`; remote deletion unverified. Raw strace (held a live token) deleted after redacted extraction.
- live lane: CLAUDE.md "cyril.log in the working directory" is stale — the log is `$HOME/.config/cyril/cyril.log` (crates/cyril/src/main.rs:278-310); fold as a docs note, do not edit CLAUDE.md inside the audit.

## Steps
- [x] 0 locate   - [x] 1 acquire+carve   - [x] 2 lanes   - [x] 3 fold   - [x] 4 file   - [x] 5 remember+commit

## Lanes
| lane | agent | status | FINDINGS sections complete | leads forwarded to live |
|---|---|---|---|---|
| kas (core+workflow) | bg agent | complete 2026-10-18 (29m52s; 259-line FINDINGS, all 9 sections) | all | KAS-L1..L8 sent to live 2026-10-18 (L1 shell_type top priority; L2 merged with TUI-L5) |
| tui | bg agent | complete 2026-10-18 (17m43s; all sections + Slash commands, Flags, Progress log) | all | TUI-L1..L6 sent to live 2026-10-18 |
| host | bg agent | complete 2026-10-18 (19m06s; all 9 sections) | all | HOST-L1..L4 sent to live 2026-10-18 |
| v3-coverage | bg agent | complete 2026-10-18 (15m53s; 356-line FINDINGS, Matrix 1–9 + ranked blockers) | all | V1 (consent-echo persistence arm, folded into KAS-L1) + V2 sent to live |
| live | bg agent | complete 2026-10-18 (legs 0–5 + all forwarded leads; 89m36s; 135 citations pass; v2 pair + TUI-L4 + HOST-L5 not run) | all | — |

## Issues filed early (step 4 must not re-file)
- Step 4 (2026-10-18): new cyril-6lql, q0kg, rcub, 68ny, 7xxk, j9r4, s18c; notes on 21 issues; gn07 relabelled; commits 934a997c, b804b076. Allowlist +42 lines in docs/wire-ignored-fields.txt → unread-fields exit 0 (live/unread-fields-2.32.0-after-allowlist.txt).
- cyril-k8wc (P1, kas, kiro-2-32-0, ready-for-agent): shell_type "posix" → KAS dialect unknown → every command asks. Filed 2026-10-18 at user request from the kas-lane static finding; cyril-auxt now depends on it (+ note); related cyril-6bol (origin), cyril-auxt. Commit 86217dd1. Pending: append the live KAS-L1 verdict as a note.
- cyril-8wh0 (P3 after correction — unreachable until cyril-0asq; was P2, kas, kiro-2-32-0, w1, ready-for-agent): workflow plan approvals render "Preview unavailable" (Text content + _meta.kiro.workflowPlan unrendered). Filed 2026-10-18 at user request; related cyril-0asq (+note), cyril-j1b3, cyril-58uv. Pending: append the live KAS-L6 workflowPlan shape + cyril screenshot as a note.
- cyril-j66p (P2 feature, kas, kiro-2-32-0, ready-for-agent; parent cyril-c6hv): output styles — no prompt _meta.kiro.outputStyle route. Reachable on every cyril KAS session (session.rs:475 builds PromptRequest without _meta). Related cyril-lyrx (+note). Filed 2026-10-18 at user request. Pending: append live TUI-L2 verdict (do styles change replies; set_config_option behaviour).

## Traps hit this cycle (append to traps.md in step 5)
- **Build cyril at the HEAD the verdict is about, and re-check HEAD before folding.** The leg-5 binary was built at the session-start HEAD; another session merged the fix under test (#169) 19 minutes later, so the S2 "flip blocker stands" verdict described superseded code. Record the build commit in every cyril-on-KAS row and compare it with `git log` at fold time.
- **A cloud-entitled account turns `--cloud` probes into real remote sessions.** HOST-L1/L3 assumed the gate would refuse; it did not. Ask before any probe whose success path creates remote state, and delete it in the same leg.
- **Spread-built objects hide from literal greps.** The 2.31.1 coverage lane (and the orchestrator's first spot-check here) missed Kiro TUI's consent echo because it is built as `consent:{...o?{capability:o}:{},scope:r,…}`; `consent:{capability` matches 0. Grep for the container key (`kiro:{consent`) and read the object, never for an assumed first member.
- **Reachability: check cyril's own gates before rating a gap.** The kas lane (and the orchestrator, filing cyril-8wh0 at P2) treated the 0.66.43 workflowPrompt plan approval as reachable in cyril; it is not — ADR-0011 keeps `workflows.enabled` off so the model-facing workflow tools never register, and `_kiro/workflow/new` takes no workflowPrompt. A new KAS flow reached only through a model tool or gate cyril deliberately withholds is FOLLOW-UP/opt-in (P3), not P2. Before proposing a priority, name the cyril entry point that triggers the frame.

## Dropped rows (lane finding -> reason not in the document)
| lane row | reason |
|---|---|
| kas: sharp/onnx dependency detail, MCP elicitation cancelledByServer, hooks didChange cadence, session-index revalidation, session-context prompt text, c2s `analysisScope`, telemetry gauges | agent-internal or model-facing; no wire field cyril consumes — summarized in doc §3 last row |
| kas: pending-handoff / pending-step-update file schemas | engine persistence, not wire — doc §4 "creator-as-step" row covers the behaviour |
| tui T4 (v2 `refreshOnly`), T14 (v2 fallback picker), T16 (`/code status` rename) | v2-only, demoted (traps 24) — doc §6 last bullet |
| tui T6, T9, T11, T15, T17 (scope page label, pause-text matching, PromptFailedError copy, zip copy, composer lending) | TUI-local chrome / handled verbatim — doc §6 "TUI-only" |
| host: launcher/term rebuild detail, rustls/gemm crate drift, `rollout.rs:276` tracing site, `_ ` helper namespace | build/linker detail — doc §1 host-size bullet |
| host: cloud flag-validation and disconnect strings | host CLI output, no cyril surface — doc §5 first bullet |
| v3-coverage Matrix rows marked "= 2.31.1" | unchanged standing gaps — doc §2b "Unchanged" line + v3-coverage-findings.md |
| v3-coverage F8 (`autopilot` not settable) | vh69 residual; folded into cyril-4c4d/c6hv context, not filed separately this cycle |
| live: HOST-L4b SIGINT nuance, oversized-image no-delta, KAS-L7 NOT-REACHED, TUI-L4 NOT RUN | recorded in doc §1a (lines 21, 27, 32) and §8; no issue |
| live: leg-1 "load carries model" | superseded by the leg-1 correction (cold-catalog pin) — doc §8 |
| live S2 (oieu "command failed") | VOID (binary predates #169) — doc §8 |

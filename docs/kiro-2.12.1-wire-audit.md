# Kiro CLI 2.12.1 — wire audit (diff vs 2.12.0)

**Analyzed:** 2026-07-14 · **Release:** 2.12.1, BUILD_DATE `2026-07-09T21:48:40Z` (a fast-follow ~5.5h after 2.12.0's build; BUILD_HASH not recoverable — archived from installed binaries, hash string not embedded) · **`kiro-cli-chat` sha256** `80049d212278c219d7bc056e053214889f2cfd93f4f362b928b857fc2eeea653` (archived `~/.local/share/kiro-research/binaries/2.12.1/`).
**Method:** archived 2.12.0 vs fresh 2.12.1 — same-day binary-isolated v2 surface A/B (`probe-v2-surface-ab-2.11.0.py`, frame-kind + field-path set diff over init/session-new/all `_kiro.dev/*`); `nm`+`rustfilt` module-path diff + `.rs` source-path set diff (all three binaries) with byte-membership verification of every candidate; KAS sha-gate + per-file content diff of both versioned extraction dirs; doc-manifest delta; telemetry metric-catalog set diff; CLI `--help` surface diff; tui.js carve + token-level diff + archive.

**Verdict for cyril: SAFE — safe to upgrade 2.12.0→2.12.1, no breaking change.** The release is exactly its one changelog line: **Model Refusal Alerts**. It adds one *optional, additive* field to a notification cyril already parses (`refusal` on `_kiro.dev/metadata`) plus backend plumbing. Nothing existing moved: v2 surface field-path-identical (24 cmds / 15 tools), KAS **completely byte-frozen** (all 2,628 files identical), CLI flags identical. Cyril tolerates the new field silently — rendering it is a parity *feature gap*, filed as **cyril-h8zb**.

---

## Changelog (announced 2026-07-09; hidden `version --changelog` == public)

```
Version 2.12.1 (2026-07-09)
  - Added: Model refusal notifications now display an error alert explaining
           why the model declined the request
```

## THE feature — model refusal alerts (wire contract recovered)

Assembled from three independent sources — Rust serde symbols (binary), the carved 2.12.1 tui.js handler (authoritative for JSON keys), and the new embedded doc:

- **Backend → CLI (below ACP):** new `amzn_codewhisperer_streaming_client` shapes `RefusalCategory` (`From<&str>`), `RefusalDetails{,Builder}`, `shape_refusal_details::de_refusal_details` — the CodeWhisperer streaming API now returns structured refusal data. Agent-side carrier: `agent::agent_loop::types::RefusalInfo { explanation, recommended_model }` **with a `Serialize` impl** (it goes somewhere — the ACP metadata notification).
- **v2 ACP wire (what cyril sees):** `_kiro.dev/metadata` gains an optional **`refusal: {category, explanation, recommendedModel}`** (camelCase keys per the tui.js consumer), and metadata `stopReason` can be **`"CONTENT_FILTERED"`**. From the carved bundle:
  `let{refusal:r,stopReason:o}=e; if(r||o==="CONTENT_FILTERED") …broadcastStreamEvent({type:"model_refusal",stopReason:o,category:r?.category,explanation:r?.explanation,recommendedModel:r?.recommendedModel})`
- **Kiro TUI behavior:** transient error alert + a system chat message, text = `explanation` with fallback *"The selected model cannot continue this conversation. Please select a different model, or start a new conversation, or rewind the current conversation to an earlier point and try a different approach."* — note the tie-in to `recommendedModel` and `/rewind`.
- **ACP prompt response:** `stop_reason: refusal` was already in the schema (zod union pre-exists in 2.12.0's tui.js); the backend presumably starts emitting it (or `CONTENT_FILTERED` on metadata) with this release.
- **Cyril today:** `to_stop_reason` maps `Refusal` correctly → toolbar shows "Refused" (red). But `convert/kiro.rs::to_ext_notification` reads only `contextUsagePercentage`/`meteringUsage`/tokens/`effort` — the `refusal` object is **silently dropped** (no crash; ext notifications are tolerant raw JSON). Users get a bare "Refused" with no explanation and no recommended model → **cyril-h8zb** (feature, P2, ready-for-agent).
- **Runtime caveat** (per the schema-vs-runtime rule): a live refusal can't be forced on demand, so the camelCase keys are *provisional until the first captured refusal*. A normal-turn live `_kiro.dev/metadata` frame was captured in the A/B and is field-path-identical to 2.12.0 — the field is genuinely optional.

## v2 (default `kiro-cli acp`) — existing wire FROZEN

- **Same-day A/B identical** (`logs/v2-surface-2.12.{0,1}-ab-20260714.*`): 24 slash commands / 15 tools byte-identical; frame kinds and field-path sets of `initialize` resp, `session/new` resp, and every `_kiro.dev/*` notification IDENTICAL (including a live `_kiro.dev/metadata`).
- **Module-path diff (nm+rustfilt), all three binaries:** kiro-cli-term zero delta; kiro-cli trivial churn (fig_proto/futures). kiro-cli-chat candidates all **disproven by byte-membership** — `agent_client_protocol_schema::tool_call::ToolCallUpdateFields` (4=4), `OptOutInterceptor` (2=2), `agent::agent::tools::tool_search` "removed" (9=9 **and** still in the live 15-tool list), `chat_cli_v2::cli::chat::legacy` (mangled segment `4chat6legacy` 1=1) — all LTO outlining. `.rs` source-path sets identical.
- **Genuinely new (membership-confirmed), all off-wire:**
  - the refusal machinery above;
  - **`AddOn` CodeWhisperer client shapes** (`AddOnBuilder` 0→1; `shape_add_on{,_list,_metadata}`; packed field names `addOnMet…`/`addOnTot…`; `addOnCredits` pre-exists) — subscription **add-on/billing API model prep** on the non-streaming client. Nothing consumes it visibly yet; watch next releases (the closest thing to a birthday-week leak in this build);
  - `opt_out` mangled module segment new (`7opt_out` 0→1) — telemetry opt-out interceptor plumbing moved into `chat_cli_v2::api_client`; off-wire.
- **CLI flag surface:** `--help`, `acp --help`, `chat --help` byte-identical.
- Sizes: chat **+87KB** (the refusal feature), kiro-cli +3KB, term −7KB.

## KAS / V3 — completely byte-frozen

Bundle sha gate moved `d93d0c50…` → `42744f1c…`, but per-file sha256 of both versioned extraction dirs (2,628 = 2,628 files) shows **zero differing files — the entire tree is byte-identical**, including `node_modules/@kiro/agent/` 0.8.0 and `dist/server/acp-server.js` (sha `037e97980cd4…` unchanged). Third consecutive release where the sha gate ≠ content; this time the gate moved with *literally zero* content change (pure recompression variance). Extraction dir: `~/.local/share/kiro-cli/kas/2.12.1-42744f1c8318…/`.

## Off-wire lanes

- **tui.js CHANGED (minimally)** — carved + sha-verified + archived (`kiro-tui-2.12.1.js`, 12,613,855 bytes, sha `4925a6c395a3c10d04c7655a519a9fa3c9d625b2781499b6a2ab6b8d430ac28d`; **+996 bytes** vs 2.12.0). The delta is the `model_refusal` alert case + metadata destructure (refusal token count 3→8; the 3 pre-existing = ACP stop-reason zod literal + status mappers). Extraction gotcha reconfirmed: on-disk `~/.local/share/kiro-cli/tui.js` was still 2.12.0's bundle (`999169fb…`) after the upgrade — always carve from the binary.
- **Doc manifest** (84+118, merged 136; 2.12.0 was 83+118/135): exactly **one added doc — `features/model-refusal-alerts.md`** ("How Kiro CLI surfaces model content-policy refusals and content-filtered responses to users"; keywords `refusal, content filter, content policy, model error, blocked, alert, stop reason`; validated 2026-07-08); zero removed, **zero field deltas on shared docs**. New baselines `docs/kiro-docs-index-2.12.1-{84,118,merged}.json`. No other unannounced-feature leaks this release.
- **Telemetry metric catalog:** YAML `- name:` set IDENTICAL (190=190 under a same-regex A/B; the 2.12.0 audit's "113" used a narrower kiro_cli-prefixed extraction — both are valid null results).

## Artifacts

- `experiments/conductor-spike/logs/v2-surface-2.12.{0,1}-ab-20260714.{jsonl,summary}`
- `docs/kiro-docs-index-2.12.1-{84,118,merged}.json`
- rivets **cyril-h8zb** — render v2 model-refusal alerts (parse `refusal` on `kiro.dev/metadata`)
- Binaries `~/.local/share/kiro-research/binaries/2.12.1/` (checksums.sha256 + BUILD-INFO inside); tui bundle `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.12.1.js{,.sha256}`

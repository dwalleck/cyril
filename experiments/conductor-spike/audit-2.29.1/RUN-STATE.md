# Audit 2.29.1 — run state (started 2026-10-11)
OLD: 2.28.0  NEW: 2.29.1  folds: 2.28.1 (feed-only, no artifacts), 2.29.0  focus: v3/KAS backend, workflows, unreleased features  tier: **full** (first attempt with all five lanes parallel was cut by a session usage limit on 2026-10-11; resumed the same day on a different provider where usage limits do not apply, so all five lanes ran in parallel)

## Releases
| ver | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS class | TUI class | host class | headline |
|---|---|---|---|---|---|---|---|---|
| 2.28.1 | — (feed 2026-10-07) | — | — (403 for every artifact on the origin, Linux/Windows/mac) | — | — | — | — | feed-only: `kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3 — located live in the 2.29.x host |
| 2.29.0 | 2026-10-09T03:12Z | 31e6bb3f | 324185fb… | 0.66.33 | A | A | distinct | `/memories`, `kiro-cli acp` uses CLI login unless `--auth-method=client`, hooks tool_tags (V2), MCP env/agent-tool changes, 13 V3 fixes |
| 2.29.1 | 2026-10-10T03:23Z | 31e6bb3f (same) | edd84abc… (== latest manifest) | 0.66.33 | A (license file only) | A | distinct | MCP OAuth issuer fix |

KAS/TUI class A = byte-identical between 2.29.0 and 2.29.1 ⇒ KAS and TUI lanes audit 2.28.0 → 2.29.1 only. Host binaries differ between 2.29.0 and 2.29.1 ⇒ host lane covers both. `BUILD_HASH` is a release-train stamp, not a content digest (traps 20).

## Established facts
- Installed `kiro-cli` and `kiro-cli-chat` (installed 2026-10-11 15:43) are byte-identical to archived 2.29.1.
- KAS 2.29.1 carve == self-extracted `~/.local/share/kiro-cli/kas/2.29.1-32dbef42…/` (diff -rq empty).
- TUI 2.29.1 carve == installed `tui.js`; 2.28.0 → 2.29.1 TUI differs (55k statement-split diff lines; minified — literal delta 44/3, method delta 0).
- KAS 0.66.26 → 0.66.33 file-level diff: `acp-server.js` 12.30 → 12.67 MB; `acp-server.externals.json`; NEW `api-key-region-probe.d.ts`; changed `client-connections`, `env-auth-provider`, `file-auth-provider`, `multiplex-stream`, `select-auth-provider` `.d.ts`; NEW dep `node_modules/@silvia-odwyer` (photon WASM image processing).
- Payload layout unchanged: 4 top-level zstd frames (KAS tar 180.66 → 183.22 MB; Node 124.68 MB same csize/sha; Bun 79.50 MB same csize/sha; TUI 13.634 → 13.643 MB); 62 nested magics are Bun's embedded modules (present in 2.28.0 too). See `frames-*.json`.
- Changelog: `feed-2.29.1.json` (2.29.1, 2.29.0) and `feed-2.28.1.json`. `kiro-cli version --changelog` is remote — an archived binary prints the current feed (traps 22).
- Login state at 16:10 local: IAM Identity Center, `whoami` OK. Token single-flight rule applied to the live lane.
- LIVE: v2 sweep workload is wire-identical 2.28.0 → 2.29.1 (313 paths each, 0 only-NEW); KAS `getState` implemented; `model_fallback` not provokable; the 20× approval loop is fixed upstream via `consentRound`; the three bundled steering slash commands are gone from `available_commands_update`; telemetry dials unchanged in all arms.
- `cleanup-autocomplete-artifacts` lives in `kiro-cli-chat` (0→4 hits), not the launcher; the launcher's `internal` namespace is the fig/shell set (traps 8).

## Steps
- [x] 0 locate   - [x] 1 acquire+carve   - [x] 2 lanes   - [x] 3 fold   - [x] 4 file   - [x] 5 remember+commit

## Lanes
| lane | agent | status | FINDINGS sections complete | leads forwarded to live |
|---|---|---|---|---|
| kas-core | task | complete 2026-10-11 (12m08s) | all 6 | L1–L8 sent |
| kas-workflow | task | complete 2026-10-11 (7m44s) | all 6 | W1–W3 sent |
| tui | task | complete 2026-10-11 (14m29s) | all 6 | T1–T6 sent |
| host | task | complete 2026-10-11 (22m43s) | all 6 | H1–H8 sent |
| live | task | complete 2026-10-11 (legs 0–5; four re-run rounds; 787-line FINDINGS) | all sections | — |

## Dropped rows (lane finding -> reason not in the document)
| lane row | reason |
|---|---|
| kas-core 10-13 (API-key home-region discovery, `EnvAuthProvider.region`, `--auth=machine` token path, `.d.ts` growth) | agent-internal routing, no wire field; live-unrun (no API key) — summarized in §2 and §8 |
| kas-core 22-24 (MCP full env, required-server predicate, subagent MCP scopes) | KAS owns MCP; cyril contributes no MCP server config — summarized in §2 rows |
| kas-core 27 (`ClientCustomAgent.steeringAfterPromptBody`) | cyril sends no `customAgents` (0 hits) |
| kas-core 28 (image-fitting pipeline) | agent-side; the only observable change is "no request failure" — §2, §6.9 |
| kas-core 32 (`KIRO_DISABLE_RECAP` read refactor) | not a removal; behaviour identical — §9 |
| kas-core 34-41, kas-workflow 1-5/9/12-16, host 18-24, TUI 5a | unchanged/negative sets; stated as "unchanged" in §2/§3/§4/§5 |
| host 17 (knowledge-search limits) | cyril embeds no semantic-search client |
| TUI D10-D19 | TUI-local chrome/telemetry; no ACP surface — §5 table + §5a |
| live per-arm capture detail (frame indexes, per-leg logs, telemetry arm counts) | in `live/FINDINGS.md`; the document cites the artifacts and the numbers |

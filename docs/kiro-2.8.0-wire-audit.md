# Kiro CLI 2.8.0 — wire audit (diff vs 2.7.1)

**Analyzed:** 2026-06-17 · **Method:** installed 2.8.0 binary (archived to `~/.local/share/kiro-research/binaries/2.8.0/`) vs archived 2.7.1; same-binary live v2 surface capture; binary module-path diff; KAS bundle (`~/.local/share/kiro-cli/kas/`) inventory. Single environment (this user's social/GitHub token, non-enterprise).

**Verdict for cyril: SAFE — nothing changed on the v2 path cyril drives.** The one announced change is the promotion of the KAS (V3) engine to a `--v3` flag; everything else is KAS-side. cyril's default `kiro-cli acp` (v2) surface is byte-for-byte identical to 2.7.1.

---

## Changelog (the only announced change)

```
Version 2.8.0 (2026-06-16)
  - Added: An early release of Kiro CLI V3 is now available. Try it out with: kiro-cli --v3.
```

`--v3` = the KAS engine. In 2.7.1 it was gated (`"V3 is currently not supported for your system"`); 2.8.0 promotes it to an advertised opt-in. (The gate string still exists in the binary — it now passes on supported systems rather than being removed.) **v2 remains the default**; plain `kiro-cli` / `chat` / `acp` (no `--v3`) still runs v2.

## v2 (default Rust engine) — unchanged

- **Exercised surface identical to 2.7.1** (same-binary capture, `probe-v2-surface-2.8.0.py`):
  - **24 slash commands**: agent chat clear code compact context effort feedback goal guide help hooks knowledge mcp model paste plan prompts quit reply rewind stats tools usage
  - **14 tools**: code glob goal grep introspect knowledge read shell subagent todo_list **use_aws** web_fetch web_search write
- **`use_aws` is alive on v2** — advertised live + 8 `"use_aws"` tool-name strings in `kiro-cli-chat`. The "use_aws was removed in 2.8.0" scare is a **KAS artifact**: KAS has never shipped `use_aws` (confirmed in the 2.7.1 audit too; KAS's AWS path is the awslabs MCP "powers"). Encountering its absence means you ran `--v3`/KAS, not that v2 lost it.
- **Binary module-path diff (2.7.1→2.8.0): no genuine v2 delta.** Strings-based extraction is dominated by LTO string-adjacency noise — every "added" module pairs with a same-module "removed" twin (`auth::external_idp`, `session::kas`, `api_client`, `cli::mcp`, `telemetry::cognito/endpoint`, `custom_tool::OAuthConfig`, …). The lone non-paired candidate is `chat_cli_v2::telemetry::legacy_sink` (telemetry-internal, not user-facing). Caveat: a `nm`+`rustfilt` symbol diff would be more precise; the exercised wire surface being identical is the stronger signal.

## KAS / V3 — the real changes

- **Bundle `@kiro/agent` 0.3.224 → 0.3.234** (deps same majors: LangGraph ^1.3.0, `@agentclientprotocol/sdk` ^0.19.0, sandbox-proxy tracks the version).
- **NEW capability: Infrastructure Safety gate** (`_kiro/safety/*`, absent from the 2.7.1 covenant catalog; new `@kiro/acp-type-covenant/dist/capabilities/safety/`). Two agent→client notifications:
  - `_kiro/safety/statusChanged` → `{ status: 'idle'|'formalizing'|'evaluating'|'error', detail? }` — the gate transitioning state.
  - `_kiro/safety/propertiesChanged` → `{ sessionId, properties: [{ index, description, enabled }], reason: 'formalized'|'toggled'|'expired' }` — *"sent when safety properties are discovered, updated, or when a tool call is blocked."* Example description in the type doc: **"ECS clusters must not be deleted."**
  - So KAS's AWS story keeps maturing on the **governance/guardrail** side (block dangerous infra ops) — not via an access tool. No `use_aws`-equivalent was added; `grep` for `use_aws`/`useAws` across the bundle = 0.
- **Covenant `_kiro/*` method catalog: 66 distinct methods** (was ~57 catalogued for 2.7.1; the net-new family is `safety/*`).
- Minor: a `subagent-tool-ids.d.ts` constants-file split under `dist/tools/`.

## Reverse-engineering surface — fully intact (and growing)

2.8.0 still ships the type + mapping "goldmine":

| Package | `.d.ts` | `.d.ts.map` | `.js.map` |
|---|---|---|---|
| `@kiro/agent/dist` | 661 | 661 | 135 |
| `@kiro/acp-type-covenant/dist` | 68 (was ~60) | — (types-only) | — |

- `.js.map` files carry **`sourcesContent`** (original source inlined, not just position maps).
- No raw `.ts` sources shipped (count 0 — same as always; the `.d.ts` + maps are the working surface).
- Net: per-capability typed contracts — the thing that made the covenant analysis possible — keep shipping, and the covenant *grew* this release.

## Cyril impact

- **None on the current path.** Stay on the default engine and `use_aws` + all v2 behavior is unchanged. No cyril code change warranted by 2.8.0.
- **KAS-track note:** the KAS-2 converter arm should tolerate the new `_kiro/safety/{statusChanged,propertiesChanged}` notifications (unknown-variant tolerance already planned). A future KAS UI could surface the safety gate (formalized properties + blocked-tool reasons). Add `_kiro/safety/*` to the [covenant reference](kiro-kas-acp-covenant.md) catalog when the KAS converter lands.
- **Trajectory:** V3/KAS is now a one-flag opt-in → on track to become default eventually. When that lands, the AWS-without-MCP path is shell + `aws` CLI via `execute_bash` (no `use_aws`, no MCP).

## Reproduce

```sh
kiro-cli version --changelog=2.8.0                          # the one announced change (V3 promotion)
# v2 surface (commands + tools incl. use_aws), same-binary across versions:
python3 experiments/conductor-spike/probe-v2-surface-2.8.0.py ~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat
python3 experiments/conductor-spike/probe-v2-surface-2.8.0.py ~/.local/bin/kiro-cli-chat
# new KAS safety capability:
cat ~/.local/share/kiro-cli/kas/node_modules/@kiro/acp-type-covenant/dist/capabilities/safety/*.d.ts
# type/map surface still shipped:
find ~/.local/share/kiro-cli/kas/node_modules/@kiro/agent/dist -name '*.d.ts' | wc -l
```

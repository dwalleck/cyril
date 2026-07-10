# cyril-6iek — prove-it-prototype findings

Date: 2026-07-09 · kiro-cli 2.12.0 · probe: `probe-fingerprint.py` · F4 harness: `test_bridge-default-vs-kas.out`

## Smallest question

Do v2 and KAS differ, at wire-handshake time, on `initialize.agentCapabilities._meta`
and on the `session/new` session-id shape — reliably enough to fingerprint the engine
before the first turn?

## Probe (live, 2.12.0, both engines via installed kiro-cli wrapper)

| Field | v2 (`kiro-cli acp`) | KAS (`--agent-engine kas`) |
|---|---|---|
| `agentCapabilities._meta` | **absent** | **present**, `_meta.kiro` object |
| `_meta.kiro` keys | — | `checkpoints, extensionMethods, logging, policyNotifications, sessionList` |
| `agentInfo` | `{"name":"Kiro CLI Agent",...,"version":"2.12.0"}` | **null** |
| `session/new` sessionId | `56ae05cd-…` (bare UUID) | `sess_fd0529e4-…` (**`sess_` prefix**) |

## Oracle

Independent mechanism: the **committed 2.11.0 live traces**
(`experiments/conductor-spike/{v2,kas}-live-session-trace-2.11.0.jsonl`), recorded by
kiro's own `KIRO_ACP_RECORD_PATH` recorder / the reference client — different capture
tool, different binary version (2.11.0 vs 2.12.0), different day. Extracted the same
fields with a one-off Python script.

**Agreement: item-by-item identical on every discriminator** (v2: no `_meta`, populated
`agentInfo`, bare-UUID id · KAS: `_meta.kiro` present with the *same 5 keys*, null
`agentInfo`, `sess_` id).

## What I learned (that the issue text didn't know)

1. **`_meta.kiro`'s key set has already drifted** — the issue (2026-07-02 audit) lists 3
   keys (`checkpoints/sessionList/extensionMethods`); live 2.11.0 *and* 2.12.0 both carry 5
   (`+logging, +policyNotifications`). The detector must key on **presence of the
   `_meta.kiro` object**, never on its exact key set.
2. **KAS `agentInfo` is `null`; v2's is populated** — a second, *negative* discriminator.
   Fragile (KAS could add it any release), so usable as corroboration only, never load-bearing.
3. **KAS `session/new` succeeds even when the client declines the auth callback** — so
   fingerprint detection at handshake fires *before* the user ever reaches the auth/turn
   failure. Fail-loud-at-handshake is achievable in a default build.
4. **acp crate exposes the fingerprint**: schema 0.11.2 `AgentCapabilities.meta:
   Option<Meta>` (`_meta` rename) survives deserialization — F1 is implementable without
   raw-JSON interception. Session ids arrive as plain strings — F2 trivial.

## F4 — what a default build actually does against KAS today (the motivating claim, observed)

`cargo run --example test_bridge` (default features, `AgentEngine::V2` bound) against
`kiro-cli acp --agent-engine kas` — full output committed as `test_bridge-default-vs-kas.out`:

- `SessionCreated` succeeds **silently** with a `sess_…` id, mode `vibe`, 0 models — zero diagnostic.
- Every `kiro.dev/*` command fails with a cryptic KAS internal:
  `[PersistenceClassification] Ext method "_kiro.dev/commands/options" has no persistence classification`.
- The prompt turn dies on `Cannot read properties of null (reading 'accessToken')` (KAS
  file-auth path, no responder in a default build).
- One oversized/garbled inbound frame produced an acp-crate parse ERROR (log noise only).

**Refinement of the issue's claim:** post-l7tw it is *not a literal hang* — the bridge's
error path emits `BridgeError` + `TurnCompleted` — but nothing anywhere says "this agent
speaks KAS"; the user gets a cascade of unrelated-looking internal errors. "Trusted, never
verified" confirmed exactly.

## Fingerprint decision table (substrate for the design)

| Signal | v2 | KAS | Strength |
|---|---|---|---|
| `initialize.agentCapabilities._meta.kiro` present | never | always | **primary** (pre-session) |
| `session/new` id has `sess_` prefix | never | always | **primary** (per-session) |
| `agentInfo` populated | yes | no (2.11–2.12) | corroboration only |

Gate check: probe ✔ oracle ✔ agreement ✔ non-obvious learnings ✔.

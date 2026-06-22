# KAS-1 — falsifiable design (cyril-evwh)

Inputs: signed-off `.cyril-evwh/spec.md`, `.cyril-evwh/probe-findings.md`
(prove-it-prototype gate met). Scope: full A+B, Part A (free path) first.

## Purpose
Make `engine_for(Kas)` resolve to a live `@kiro/agent` spawn so a KAS turn runs,
in two ordered parts: **A** free-path direct spawn (no credential code), **B**
wrapper + `_kiro/auth/getAccessToken` responder. All KAS code behind the `kas`
cargo feature; v2 default unchanged.

## Change classification (step 2b)
**Purely additive.** A new spawn path + a new server→client request handler,
both gated by `engine=Kas` + `--features kas`. It relaxes no existing v2
constraint — the bridge loop, busy-guard, and converters are untouched (KAS-2a
[cyril-j16p] reworks turn-end). No removed-invariant sweep required.

## Architecture (on the KAS-0 seam)
- **`KasEngine`** (new, `#[cfg(feature="kas")]`, in `protocol/engine.rs`):
  `client_capabilities()` = `{}` (probe Run-A: nothing advertised → fs
  in-process, no callbacks). `convert_*` minimal — KAS-1 does not render
  (KAS-2a); unknown `_kiro/*`/`session_info_update` fall to the existing
  unknown-variant `debug!` drop, which the probe showed does not hang.
- **`engine_for(Kas)`** (`bridge.rs`): `#[cfg(feature="kas")]` → `Ok(KasEngine)`
  + the bridge builds a **KAS spawn command**; `#[cfg(not(feature="kas"))]` →
  the existing clean "not available" error.
- **Spawn (Part A free path):** resolve argv `[<node>, --experimental-wasm-modules,
  <server.js>, --transport=stdio]` — `<server.js>` = `$KIRO_KAS_SERVER_PATH`
  else walk `~/.local/share/kiro-cli/kas/node_modules/@kiro/agent/dist/server/acp-server.js`;
  `<node>` = `$KIRO_AGENT_PATH` else `node` on PATH. Precheck {server.js, node,
  token-file} → fail-fast `BridgeDisconnected` naming the missing one.
- **Spawn (Part B wrapper):** `kiro-cli acp --agent-engine <flag>`, `<flag>`
  resolved from `kiro-cli --version` (≥2.8.0 → `v3`, 2.7.x → `kas`, <2.7.1 →
  refuse). Reuses the existing `AgentProcess::spawn` (kiro-cli invocation).
- **Auth responder (Part B):** a `_kiro/auth/getAccessToken` handler in the KAS
  client path answers by reading `~/.aws/sso/cache/kiro-auth-token.json` →
  `{accessToken, expiresAt, profileArn}`. It is machine-answerable (fast local
  read), so it is answered in the client handler, NOT routed to the App like a
  human permission (ADR-0004 non-blocking-forward preserved). Refresh-on-stale
  via a kiro-cli affordance (C-PARTB-REFRESH).
- **Custodian:** token carried in a newtype whose `Debug` is redacted; never
  logged; fetched per request; not stored on a long-lived struct.

## Input shapes (step 2)
- `AgentEngine`: **V2** | **Kas** (both).
- `kas` feature: **on** | **off** (both; off+Kas must still refuse).
- server.js path: `$KIRO_KAS_SERVER_PATH` set | unset→walk | target exists | absent.
- node runtime: `$KIRO_AGENT_PATH` set | unset→PATH | `node` present | absent.
- token file: present+fresh (>now+3min) | present+stale (≤now+3min) | present
  missing a field | absent.
- identity: social | AWS-IdP | Builder-ID (profileArn per type).
- kiro-cli version: ≥2.8.0 (`v3`) | 2.7.x (`kas`) | <2.7.1 (refuse).
- spawn shape: free-path (direct) | wrapper.
- getAccessToken: fired (wrapper) | not fired (free path).

## Claims
1. **C-FREEPATH** — free-path direct spawn (no `--auth`) authenticates via KAS's
   file provider, fires `getAccessToken` 0×, turn ends `end_turn`.
2. **C-DISCOVERY-RESOLVE** — cyril builds argv `[node, --experimental-wasm-modules,
   server.js, --transport=stdio]` from the env-else-walk/PATH rules.
3. **C-PRECHECK-FAIL** — any missing precondition (server.js / node / token-file)
   → fail-fast `BridgeDisconnected` naming the specific one; no spawn, no v2 fallback.
4. **C-ENGINE-GATE-ON** — with `--features kas`, engine=Kas → `Ok(KasEngine)` +
   free-path spawn.
5. **C-ENGINE-GATE-OFF** — without `--features kas`, engine=Kas → the clean "not
   available" error; KAS spawn/auth code is not compiled.
6. **C-V2-PARITY** — engine=V2 is byte-identical to post-KAS-0; no KAS code runs.
7. **C-WRAPPER-FLAG** — the `--agent-engine` flag is resolved from kiro-cli
   version (≥2.8.0→`v3`, 2.7.x→`kas`, <2.7.1→refuse).
8. **C-PARTB-SOURCE** — a fresh token-file read yields spec-B4-valid
   `{accessToken, expiresAt, profileArn}`.
9. **C-PARTB-REFRESH** — in wrapper mode, a stale token-file is refreshed via a
   kiro-cli affordance so the reply stays valid >now+3min, without cyril
   reimplementing OIDC.
10. **C-PARTB-ALLTYPES** — the responder emits a non-empty `profileArn` for all
    three identity types.
11. **C-CUSTODIAN** — the access-token value never appears in logs or `Debug`
    output.

## Falsification

| # | Claim | Falsifier (input → falsifying result) | Oracle (independent) | Cost | Status | Regression fence |
|---|-------|----------------------------------------|----------------------|------|--------|------------------|
| 1 | C-FREEPATH | direct-spawn no `--auth`, run a turn; if `getAccessToken`>0 or stopReason≠end_turn → false | server stderr `[INFO] Auth: default token file` (≠ probe's request count) | 2m | **passed** (probe 2026-06-22) | `#[ignore]` e2e `kas_freepath_turn_smoke` (gated, live) |
| 2 | C-DISCOVERY-RESOLVE | set/unset the two env vars; if argv ≠ expected ordered vector → false | hand-written expected argv per env combo | 5m | pending | unit `discovery::resolve_argv` (env matrix) |
| 3 | C-PRECHECK-FAIL | point server.js path at a missing file (and node absent; and token-file absent) one at a time; if it spawns / falls back to v2 / error doesn't name the missing item → false | filesystem `ls` of the (absent) target | 10m | pending | unit `discovery::precheck` (3 missing-one-at-a-time fixtures) |
| 4 | C-ENGINE-GATE-ON | `--features kas`, engine=Kas; if `engine_for` errors or no spawn argv built → false | the built argv (probe-proven spawnable) | 10m | pending | unit `engine_for_kas_ok` under `cfg(feature="kas")` |
| 5 | C-ENGINE-GATE-OFF | default build, engine=Kas; if it spawns KAS / the auth-file/refresh symbols are linked → false | `cargo build` (no `kas`) + `nm`/symbol grep for the auth-responder symbol | 10m | pending | unit `engine_for_kas_unavailable` under `cfg(not(feature="kas"))` (KAS-0 test, retained) |
| 6 | C-V2-PARITY | run the full v2 suite + a `cargo run` v2 smoke; if any v2 behavior differs → false | KAS-0 FakeAgent parity tests (independent of KAS code) | 10m | pending | existing `protocol::bridge` v2 tests + CI v2 lane |
| 7 | C-WRAPPER-FLAG | feed version strings 2.8.1 / 2.7.1 / 2.6.0; if flag ≠ v3 / kas / refuse → false | hand table of (version → flag) | 5m | pending | unit `flag_for_version` (3 versions + boundary) |
| 8 | C-PARTB-SOURCE | read token-file; if any of 3 fields absent OR expiresAt≤now+3min → false | Python/jq direct field+expiry parse (≠ cyril) | 1m | **passed** (2026-06-22, 3245s>180s, 3/3 fields) | unit `auth::parse_token_file` (fixture w/ far-future expiry + a missing-profileArn fixture) |
| 9 | C-PARTB-REFRESH | with a STALE token-file in wrapper mode, run the candidate kiro-cli refresh affordance; if file `expiresAt` does not advance >now+3min (or only advances by cyril minting a token itself) → false | the file's `expiresAt` before/after (filesystem), cross-checked against a wrapper turn not 400-ing | 60m (needs natural staleness) | **pending — riskiest** | `#[ignore]` e2e `kas_wrapper_refresh_smoke` (gated, live, stale-file fixture) |
| 10 | C-PARTB-ALLTYPES | resolve token for social (live) + AWS-IdP (live) + Builder-ID (fixture); if profileArn empty for any → false | per-identity token-file/store inspection | 30m (2 live, 1 unit) | pending | unit `profile_arn_present` (3 identity fixtures) + 2 live smokes |
| 11 | C-CUSTODIAN | drive a responder reply with tracing capture; if the token substring appears in logs or `{:?}` → false | `grep` of captured log buffer for the token value | 10m | pending | unit `token_redacted_in_debug_and_logs` |

The cheapest claim (C-PARTB-SOURCE, 1m) is **passed**; C-FREEPATH (the spine)
is also passed via the probe.

## Negative space (what KAS-1 deliberately does NOT do)
1. Does **not** render KAS turn output — converter arms + turn-end/busy-clear
   rework are **KAS-2a (cyril-j16p)**.
2. Does **not** marshal `AgentSettings` into the `_meta.kiro` handshake —
   **cyril-nhzw**.
3. Does **not** implement fs/terminal host callbacks (`_kiro/fs/*`,
   `_kiro/terminal/*`) — **KAS-5 (cyril-7bdu)**.
4. Does **not** reimplement OIDC refresh — delegates to kiro-cli's own auth.
5. Does **not** change the v2 path in any way.

## Self-review (step 7)
1. **Claim count:** 11 (in 3–15). ✓
2. **Falsifier independence:** every oracle is outside the SUT (server stderr,
   jq/Python parse, filesystem `ls`, `nm` symbol grep, hand tables, KAS-0 parity
   tests). ✓
3. **Non-vacuity (named buggy impl per fence):** C2→argv with node/server
   swapped order; C3→responder that spawns anyway on missing file; C5→auth code
   not behind `cfg`, linked into default build; C8→reader that drops profileArn;
   C9→responder that returns the stale token unchanged; C11→`#[derive(Debug)]`
   on the token struct printing the secret. All fail their fence, pass none other.
4. **Distinct outputs:** each claim has its own test/probe section; a failure
   localizes by name. ✓
5. **Cost distribution:** the two expensive ones (C9 60m live-stale, C10 30m)
   are the only >15m; C9 is the accepted riskiest and is a gated live smoke, C10
   splits into 2 live + 1 unit. No claim is *only* falsifiable expensively
   without a cheaper unit proxy (C9 has the unit `parse`/`refresh-trigger` shape;
   the live part is the stale-timing). ✓
6. **Negative space:** 5 entries (≥3). ✓
7. **Tracker references:** cyril-j16p (KAS-2a), cyril-nhzw, cyril-7bdu all
   verified present in `rivets ready`. C9's deferral-of-timing is tracked by THIS
   task (cyril-evwh, Part B). Builder-ID unit-only is signed-off settled
   rationale (spec SC2), not deferred work. ✓
8. **Removed-invariant coverage:** N/A — change is additive (step 2b). ✓

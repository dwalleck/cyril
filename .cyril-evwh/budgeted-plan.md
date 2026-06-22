# KAS-1 — budgeted plan (cyril-evwh)

Design: `.cyril-evwh/design.md` (cheapest falsifier C-PARTB-SOURCE passed; spine
C-FREEPATH passed via probe). 7 slices; each its own commit with passing gates.
All KAS code behind `--features kas` (CI `kas-feature` lane). Module layout:
`crates/cyril-core/src/protocol/kas/{mod,discovery,auth}.rs` (cfg-gated) +
`KasEngine` in `engine.rs` + `engine_for`/spawn branch in `bridge.rs`.

Workspace rails: no `.unwrap()`/`let _ =`/`#[allow]` in non-test; rustfmt +
`clippy -D warnings`. Live smokes (C1, C9-live, C10-live) are **manual-gated**
`#[ignore]` tests requiring operator auth + network — they need explicit
merge-time approval (design caveat).

---

## PART A — free path (zero credential code; the demo)

## Slice A1: `KasEngine` + `engine_for(Kas)` cfg flip
**Claim:** C4 (engine half), C5 (gate-off), C6 (v2 parity).
**Oracle:** `cargo build` with/without `--features kas` + `nm`/symbol grep
(independent of cyril runtime) — the KAS auth symbol is absent from the default
build; `engine_for(Kas)` returns `Ok` only under the feature.
**Stress fixture:** build **without** `--features kas`, call `engine_for(Kas)` →
must return the existing "not available yet" `Err`; AND `nm target/.../cyril | grep
kas_read_token` finds nothing. (Bug class: `engine_for` cfg forgotten → KAS path
in a default build = the credential code ships unconditionally, violating ADR-0002.)
**Loop budget:** no new loops.
**Wall budget:** n/a (no always-on phase).
**Files:** `crates/cyril-core/src/protocol/engine.rs`,
`crates/cyril-core/src/protocol/bridge.rs`.
**Code (advisory):**
```rust
// engine.rs
#[cfg(feature = "kas")]
pub(crate) struct KasEngine;
#[cfg(feature = "kas")]
impl Engine for KasEngine {
    fn client_capabilities(&self) -> acp::ClientCapabilities { acp::ClientCapabilities::new() }
    // KAS-1 does not render (KAS-2a); reuse the generic converters so a basic
    // turn's standard ACP variants pass through and _kiro/* fall to the
    // existing unknown-variant debug! drop (probe: no hang).
    fn convert_session_update(&self, a: &acp::SessionNotification, c: &HashMap<String, serde_json::Value>) -> Option<Notification> { convert::session_update_to_notification(a, c) }
    fn convert_ext_notification(&self, m: &str, p: &serde_json::Value) -> crate::Result<Option<Notification>> { convert::kiro::to_ext_notification(m, p) }
}
// bridge.rs engine_for:
fn engine_for(e: AgentEngine) -> Result<Rc<dyn Engine>, String> {
    match e {
        AgentEngine::V2 => Ok(Rc::new(V2Engine)),
        #[cfg(feature = "kas")]
        AgentEngine::Kas => Ok(Rc::new(KasEngine)),
        #[cfg(not(feature = "kas"))]
        AgentEngine::Kas => Err("KAS engine requires a build with --features kas".into()),
    }
}
```
**Verification:**
- [ ] Unit tests pass (`engine_for_kas_ok` cfg(kas); `engine_for_kas_unavailable` cfg(not))
- [ ] Stress fixture: default build → Err + symbol absent
- [ ] prove-it oracle (probe) unaffected (v2 path untouched)
- [ ] Budgets hold (no loops)

## Slice A2: free-path spawn discovery (pure resolver + precheck)
**Claim:** C2 (resolve argv), C3 (precheck fail-fast names the missing item).
**Oracle:** filesystem `ls` of the (absent) target + a hand-written expected-argv
table per env combination (independent of the resolver).
**Stress fixture:** (a) `KIRO_KAS_SERVER_PATH` → a path that does NOT exist while
`node` IS present → `Err::MissingServer(path)` (NOT MissingNode — the bug is
reporting the wrong missing item / checking node first). (b) server present, token
file absent → `Err::NotLoggedIn`. (c) `KIRO_AGENT_PATH=""` empty string → treated
as unset, falls back to PATH (bug: empty string used as the node binary). (d) a
server path **with spaces** → argv preserves it as one arg (bug: split on space).
**Loop budget:** no loops — fixed 3-item precheck (server.js, node, token-file),
O(1), 3 `stat` syscalls < the 10^3 budget.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/kas/discovery.rs`,
`crates/cyril-core/src/protocol/kas/mod.rs`.
**Code (advisory):**
```rust
pub(crate) enum KasMissing { Server(PathBuf), Node, NotLoggedIn(PathBuf) }
pub(crate) struct KasSpawn { pub program: PathBuf, pub args: Vec<String> }
pub(crate) fn resolve_kas_spawn() -> Result<KasSpawn, KasMissing> {
    let server = env_nonempty("KIRO_KAS_SERVER_PATH")
        .unwrap_or_else(default_server_path);
    if !server.is_file() { return Err(KasMissing::Server(server)); }
    let node = env_nonempty("KIRO_AGENT_PATH").map(PathBuf::from)
        .or_else(|| which("node")).ok_or(KasMissing::Node)?;
    let token = default_token_path();
    if !token.is_file() { return Err(KasMissing::NotLoggedIn(token)); }
    Ok(KasSpawn { program: node, args: vec![
        "--experimental-wasm-modules".into(), server.display().to_string(),
        "--transport=stdio".into() ] })
}
```
Note (doc-comment-as-contract): `resolve_kas_spawn` has NO documented precondition
on the caller — it validates everything itself and returns a typed `Err`
(load-bearing → runtime check, not `debug_assert!`). The `Err` is **data** the
caller turns into a `BridgeDisconnected` (diagnostic to the user, but a structured
notification, not stderr).
**Verification:**
- [ ] Unit tests pass (env matrix: 4 stress cases above + happy path)
- [ ] Stress fixture: each missing-one-at-a-time names the correct item
- [ ] prove-it oracle: the happy-path argv equals the probe's spawn argv
- [ ] Budgets hold (O(1), 3 stats)

## Slice A3: wire the bridge to free-path spawn + gated live e2e
**Claim:** C1 (free-path turn through cyril's bridge), C4 (spawn half).
**Oracle:** the probe `experiments/conductor-spike/probe-kas-direct-spawn-2.8.1.py`
verdict (`getAccessToken` 0×, end_turn) — cyril's bridge must reproduce it; AND
the server's own stderr `[INFO] Auth: default token file`.
**Stress fixture:** `resolve_kas_spawn` returns `Err::NotLoggedIn` → the bridge
emits `BridgeDisconnected{reason}` naming `kiro-cli login` and does NOT spawn,
does NOT fall back to v2 (bug: it spawns kiro-cli acp v2 instead, silently
downgrading the user's explicit KAS choice).
**Loop budget:** no new loops (one-shot spawn at bridge start).
**Wall budget:** n/a (spawn is startup, not an always-on phase).
**Files:** `crates/cyril-core/src/protocol/bridge.rs`,
`crates/cyril-core/tests/kas_freepath_smoke.rs` (new, `#[ignore]` gated live).
**Code (advisory):** in `run_bridge`, after the engine gate, when
`agent_engine == Kas` (free path) resolve `resolve_kas_spawn()`; on `Err` →
`notify BridgeDisconnected` + return; on `Ok` build the `AgentProcess` from
`{program,args}` instead of the kiro-cli `agent_command`.
**Verification:**
- [ ] Unit test: `Err::NotLoggedIn` → BridgeDisconnected, no spawn (mock resolver)
- [ ] Stress fixture passes (no v2 fallback)
- [ ] **Live (manual-gated):** `cargo test -p cyril-core --features kas
  kas_freepath_smoke -- --ignored` → end_turn, getAccessToken 0×
- [ ] prove-it oracle agrees (cyril bridge ≡ raw probe)

---

## PART B — wrapper + auth responder

## Slice B1: kiro-cli version → `--agent-engine` flag
**Claim:** C7 (≥2.8.0→v3, 2.7.x→kas, <2.7.1→refuse).
**Oracle:** hand table of (version string → flag), independent of the parser.
**Stress fixture:** include **`2.10.0`** and **`2.8.10`** → must map to `v3`
(bug class: string comparison makes `"2.10.0" < "2.8.0"` true → wrong flag; force
semver compare). Also `2.7.1`→`kas` (lower boundary), `2.6.9`→refuse, and a
**malformed** version (`"kiro 2"` / empty) → refuse with an actionable error, not
a panic.
**Loop budget:** no loops — parse + compare, O(1).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/kas/mod.rs` (or `discovery.rs`).
**Code (advisory):** parse `MAJOR.MINOR.PATCH` to `(u32,u32,u32)`; `>= (2,8,0)
=> "v3"`, `>= (2,7,0) => "kas"`, else `Err`; non-parse → `Err`.
**Verification:**
- [ ] Unit tests pass (table incl. 2.10.0, 2.8.10, 2.7.1, 2.6.9, malformed)
- [ ] Stress fixture: 2.10.0 → v3 (semver, not lexical)
- [ ] Budgets hold (O(1))

## Slice B2: auth token newtype + file-read responder (the source)
**Claim:** C8 (passed — valid reply when fresh), C10 (profileArn present all
types), C11 (custodian: never logged / redacted Debug).
**Oracle:** the Python/jq direct token-file parse (the passed C8 falsifier),
independent of cyril; for C11, `grep` the captured tracing buffer for the token
value.
**Stress fixture:** (a) fixture token file **missing `profileArn`** → `Err`, NOT
a reply with empty/null profileArn (bug: `unwrap_or_default()` → KAS 400). (b)
`format!("{:?}", token)` and `tracing` capture must NOT contain the secret
substring (bug: `#[derive(Debug)]` leaks the token). (c) 3 identity fixtures
(social/AWS-IdP/Builder-ID) each → non-empty profileArn.
**Loop budget:** no loops — one file read (~1 KB) + field extraction, O(1).
**Wall budget:** n/a (per-request, infrequent).
**Files:** `crates/cyril-core/src/protocol/kas/auth.rs`,
`crates/cyril-core/src/protocol/client.rs` (the `_kiro/auth/getAccessToken` arm).
**Code (advisory):**
```rust
pub(crate) struct AccessToken(String);          // redacted Debug, no Display of secret
impl std::fmt::Debug for AccessToken { fn fmt(&self,f:&mut _)->_ { f.write_str("AccessToken(***redacted***)") } }
pub(crate) struct AuthReply { token: AccessToken, expires_at: String, profile_arn: String }
pub(crate) fn read_token_file(path:&Path) -> crate::Result<AuthReply> { /* parse; Err if any of 3 fields missing */ }
```
Doc-comment-as-contract: `read_token_file` doc says "returns Err if any of
{accessToken,expiresAt,profileArn} is absent" — **load-bearing for correctness**
(silent empty profileArn → KAS 400) → enforced by a runtime `Err`, not
`debug_assert!`. Output stream: the reply is **data** to KAS over the wire; any
warning on a malformed file is **diagnostic** (tracing→stderr), never the token.
**Verification:**
- [ ] Unit tests pass (missing-profileArn→Err; 3-identity profileArn present)
- [ ] Stress fixture: Debug + logs contain 0 occurrences of the secret
- [ ] prove-it oracle (C8) still agrees (fresh file → valid reply)
- [ ] Budgets hold (O(1))

## Slice B3: refresh-on-stale trigger (separable from live timing)
**Claim:** C9 (stale file → refresh via kiro-cli affordance → valid >now+3min,
no OIDC reimplementation).
**Oracle:** the token file's `expiresAt` before/after the candidate refresh
command (filesystem), cross-checked that the wrapper turn does not 400 / TokenInvalid.
**Stress fixture (unit-separable):** `is_stale(expiresAt, now)` at the boundary —
`expiresAt == now + 180s` → **stale** (bug: `<` vs `<=` off-by-one on the
buffer); `now + 181s` → fresh. The unit test fixes the refresh-TRIGGER decision
deterministically; the **live** part (does the chosen kiro-cli command actually
advance a genuinely-stale file) is the gated `#[ignore]` smoke.
**Loop budget:** at most ONE refresh attempt per request (no retry loop) — O(1);
the refresh shells one kiro-cli command = 1 subprocess, well under budget.
**Wall budget:** the refresh subprocess is bounded by a timeout (e.g. 30s); it is
on the getAccessToken response path, not an always-on phase — documented as a
per-request bound, acceptable because getAccessToken is infrequent.
**Files:** `crates/cyril-core/src/protocol/kas/auth.rs`,
`crates/cyril-core/tests/kas_wrapper_refresh_smoke.rs` (new, `#[ignore]`).
**Code (advisory):** `if is_stale(reply.expires_at, now) { run_kiro_refresh()?;
reply = read_token_file(path)?; }` where `run_kiro_refresh` invokes the affordance
identified during the slice (candidate: `kiro-cli whoami`/`profile`; CONFIRM in
the live smoke which one advances `expiresAt`). If none refreshes on demand,
fall back to a fail-fast "token expired — run `kiro-cli login`" (still no OIDC
reimplementation) and record the finding.
**Verification:**
- [ ] Unit tests pass (`is_stale` boundary at exactly now+180s)
- [ ] Stress fixture: `<=` boundary correct
- [ ] **Live (manual-gated, ~1h staleness or a deliberately old login):**
  `kas_wrapper_refresh_smoke --ignored` → expiresAt advances >now+3min, turn OK
- [ ] Budgets hold (1 refresh, 1 subprocess, timeout-bounded)

## Slice B4: wire wrapper spawn + auth responder + live identity smokes
**Claim:** C1 (wrapper turn), C9-live, C10-live (social + AWS-IdP).
**Oracle:** a live wrapper turn that reaches end_turn with `getAccessToken`
answered (≥1×) and NO `profileArn is required` 400 / TokenInvalid — cross-checked
against the free-path probe behavior.
**Stress fixture:** spawn `kiro-cli acp --agent-engine v3` with the responder
wired; a turn for the **social** identity AND the **AWS-IdP** identity each
complete (operator switches login between runs). Builder-ID is unit-only
(accepted, spec SC2). Bug class caught: responder wired but the loop AWAITS the
reply (ADR-0004 violation) → a deadlock the free path never exercised.
**Loop budget:** no new loops.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`,
`crates/cyril-core/tests/kas_wrapper_smoke.rs` (new, `#[ignore]`).
**Code (advisory):** when `agent_engine==Kas` AND wrapper mode selected, spawn
`kiro-cli acp --agent-engine <flag_for_version()>`; the KiroClient answers
`_kiro/auth/getAccessToken` via Slice B2/B3 in the client handler (non-blocking,
ADR-0004 preserved — machine-answerable, not routed to the App).
**Verification:**
- [ ] Unit test: responder answered in-handler, loop not blocked (FakeAgent issues
  getAccessToken; assert the loop processes a mid-request command — reuses the
  KAS-0 Slice-3 harness shape)
- [ ] Stress fixture (no deadlock)
- [ ] **Live (manual-gated):** social + AWS-IdP wrapper turns complete
- [ ] prove-it oracle: wrapper end_turn matches free-path end_turn

---

## Plan Self-Review (step 7)

**1. Every loop — complexity + budget.**
- A1: no loops. A2: no loops (fixed 3-item precheck, O(1), 3 stats). A3: no loops
  (one-shot spawn). B1: no loops (O(1) parse+compare). B2: no loops (one ~1 KB
  read, O(1)). B3: ≤1 refresh, ≤1 subprocess (O(1), timeout-bounded). B4: no
  loops. → No loop exceeds 10^6 ops / 10^3 syscalls. ✓

**2. Every fixture — bug class.**
- A1: cfg-forgotten → credential code in default build. A2: wrong-missing-item
  report; empty-env-string-as-binary; space-in-path split. A3: silent v2 fallback
  on the user's KAS choice. B1: lexical-vs-semver version compare (2.10.0). B2:
  `unwrap_or_default` empty profileArn; `derive(Debug)` token leak. B3: `<` vs
  `<=` stale-buffer off-by-one. B4: ADR-0004 await-deadlock. → All adversarial,
  not happy-path. ✓

**3. Every doc-comment precondition — enforcement.**
- A2 `resolve_kas_spawn`: self-validating, typed `Err` (load-bearing → runtime
  check). B2 `read_token_file` "Err if field absent": load-bearing for
  correctness → runtime `Err` (not `debug_assert!`). No documented precondition
  left unenforced. ✓

**4. Every write target — data/diagnostic.**
- Auth reply → **data** (wire to KAS). Malformed-file/refresh warnings →
  **diagnostic** (tracing→stderr). The token value → written NOWHERE (custodian).
  BridgeDisconnected reason → structured notification (App), not a raw fd. ✓

**5. Every tracker reference.**
- Deferrals cite: KAS-2a=**cyril-j16p**, AgentSettings=**cyril-nhzw**,
  KAS-5=**cyril-7bdu** (all verified in `rivets ready`). C9 live-timing + B4 live
  identities are tracked by **cyril-evwh** itself (this milestone, Part B), not
  deferred elsewhere. Builder-ID unit-only = signed-off settled rationale (spec
  SC2). ✓

**Claim coverage vs design:** C1(A3,B4) C2(A2) C3(A2) C4(A1,A3) C5(A1) C6(A1)
C7(B1) C8(B2) C9(B3,B4) C10(B2,B4) C11(B2) — all 11 design claims covered. ✓

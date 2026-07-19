# cyril-0wyn — design: cyril's ACP clientInfo identity on KAS

Status: DRAFT — awaiting the HARD PAUSE decision. Probes: `findings.md` (all pass).

## Purpose

Decide — and make explicit, fenced, and observable — what cyril presents as
ACP `clientInfo` to agents, given that KAS derives persona, remote-tool
allowlist, hooks briefing, and repository honoring from `clientInfo.name`,
silently falling back to `kiro-ide` for unrecognized names.

## The decision (proposed)

**Option A — cyril presents its own honest identity, knowingly accepting the
kiro-ide fallback on KAS.** Recorded as ADR-0006.

Rejected alternatives (recorded in the ADR):
- **B. Impersonate `kiro-cli`**: gains CLI persona + memoryEnabled allowlist
  branch; costs honest telemetry (clientInfo → AWS,
  `reference_kiro_acp_telemetry`) and cyril's be-legitimate positioning
  (pi-kiro lesson); brittle against upstream name-keyed changes. Preserved as
  an opt-in escape hatch ONLY if a real need appears — tracked at
  **cyril-jrl1** (verified: filed this session).
- **C. Honest name + override knob**: **does not exist** — probe-proven
  (findings.md fact 1): fallback inference is execution-environment-only;
  the only env knob is `KIRO_LOAD_ALL_REMOTE_TOOLS=true` (allowlist `*`,
  debug-grade, tools only).
- **D. Fix upstream**: right long-term answer, not in cyril's hands — tracked
  at **cyril-ctnv** (verified: filed this session; ready-for-human).

## Architecture (what actually changes)

1. **Single-source identity**: extract clientInfo construction from
   `run_loop` (bridge.rs:660) into a small pure `client_info()` fn
   (name `"cyril"`, title `"Cyril"`, version `CARGO_PKG_VERSION`) — unit-
   testable, one call site.
2. **Fail-loud advisory**: probes proved the misclassification is invisible
   on the wire (findings.md Q3). So cyril says it itself: when the **bound
   engine at runtime** (never `cfg(feature)` — the cyril-dn91/ADR-0002 trap)
   is KAS, log one startup advisory naming the kiro-ide classification, its
   four consequences, and ADR-0006.
3. **ADR-0006**: the decision, alternatives, the four client-keyed behaviors
   (persona, allowlist, hooksBlock, honorsRepositories), the env bypass, and
   pointers to cyril-jrl1/cyril-ctnv/cyril-jiyn.
4. **Release-audit checklist line**: the wire-audit methodology gains a
   per-release check — re-carve `resolveAgentContext` + re-run
   `probe-b-name-ab.py` — so upstream changes to the recognition set are
   caught at the next release audit, not in production.

## Input shapes (step 2)

- **EngineKind** (enum): `V2` | `Kas` — claims 1 (both) and 3 (per-kind).
- **Build features**: `kas` off | on — claim 1 verified under both builds.
- **KAS name classes**: recognized (`kiro-web`|`kiro-ide`|`kiro-cli`) vs
  unrecognized (`cyril`) — claim 2 covers `cyril`, `kiro-cli`, `kiro-ide`
  live. `kiro-web` not live-tested: out of scope — recognition is a string
  equality identical in shape to the two tested names, and cyril never
  presents it.
- **KAS executionEnvironment**: `local` | `sandbox` — `local` claimed;
  `sandbox` out of scope — cyril always spawns KAS as a local child, the
  sandbox branch is unreachable from cyril's spawn path.
- **Version string**: always `CARGO_PKG_VERSION` (cargo guarantees non-empty,
  semver-shaped); no further variance to enumerate.
- **title**: `Some("Cyril")` after this change (today `None`); both shapes
  are wire-legal (`Option<String>` in the schema); claim 1 pins the new one.

## Removed-invariant sweep (step 2b)

Purely additive: no lock, guard, ordering, or serialization point is removed.
The refactor moves an expression into a function (same value, same call
site); the advisory adds a log line; the rest is documentation. No invariant
sweep required.

## Claims and falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | cyril's initialize frame carries `clientInfo` name=`"cyril"`, title=`"Cyril"`, version=`CARGO_PKG_VERSION`, identical under default and `kas` builds and independent of bound engine | dump-agent wire capture (`probe-a-dump-agent.sh` via `test_bridge`), both builds; if name/version differ or a layer rewrites the frame → false | captured wire bytes vs `Cargo.toml` version + the string literal (manifest ≠ runtime path) | 5m | **passed** (both builds; title pending impl — pre-change value `null` captured) | unit test `client_info_is_cyril_identity` on the extracted fn (buggy impl caught: renaming to `kiro-cli`, dropping title, version drift) |
| 2 | KAS 2.13.0 classifies name `cyril` as kiro-ide (local) with the `Unrecognized clientInfo.name` warn; `kiro-cli`/`kiro-ide` accepted silently | `probe-b-name-ab.py` A/B ×3 names; wrong/absent warn or wrong acceptance → false | per-run `~/.kiro/logs/<ts>/kiro.log` lines vs carved `resolveAgentContext` source (runtime vs source text) | 10m | **passed** (ALL-PASS) | **manual (release-audit)**: external system, not CI-fenceable — checklist line added to the wire-audit methodology doc (this PR); requires user approval at the pause |
| 3 | With the KAS engine bound at runtime, bridge startup logs exactly one advisory naming the kiro-ide classification and ADR-0006; with V2 bound, none — keyed off `engine.kind()`, not `cfg(feature)` | unit test both kinds under `--features kas` build; advisory for V2, or none for KAS, or feature-flag-keyed → false | test asserts `advisory(EngineKind)` is `Some` iff Kas (buggy impl caught: the dn91 trap — gating on the cargo feature makes the kas-feature+v2-engine test fail) | 5m | pending | same unit test, `advisory_only_for_kas_engine` |
| 4 | ADR-0006 names all four client-keyed behaviors and the env bypass | `grep -c` for `persona\|allowlist\|hooksBlock\|honorsRepositories\|KIRO_LOAD_ALL_REMOTE_TOOLS` in the ADR ≥ 5 distinct hits; any missing → false | grep vs the carve files in `.cyril-0wyn/` (doc vs source-of-truth artifacts) | 1m | pending | manual (docs; verified at review) — approval requested at the pause |

Distinctness: each claim has its own observable (wire capture / probe verdict
lines / named unit tests / grep count) — a failure localizes to its claim.

## Negative space (what this deliberately does NOT do)

1. **No impersonation, no knob**: `presentAs` is not built here — cyril-jrl1
   (verified above) holds that scope behind a real user need.
2. **No wire-detection or log-tailing**: cyril does not attempt to detect its
   resolved client type (impossible on the wire per findings Q3) nor tail
   KAS's log dir for the warn (fragile path coupling to an internal layout).
3. **No hooks-briefing correction**: injecting a cyril-correct hooks briefing
   is cyril-jiyn (KAS-7) scope — the coupling note is already on that issue.
4. **No v2-path changes**: v2 ignores clientInfo.name for behavior (reads
   settings from disk); nothing changes there beyond the shared constant.
5. **No env management**: `KIRO_LOAD_ALL_REMOTE_TOOLS` passes through from
   the user's environment untouched; documented in the ADR only.

## Open decisions for the pause

1. **Ratify Option A** (honest `cyril`) — the core of the issue.
2. **Claim 2's fence is `manual (release-audit)`** — external-system property;
   needs your explicit approval per the fence rule.
3. **title = "Cyril"** — cosmetic addition riding along; veto if unwanted.
4. Advisory log level/wording — proposed `info` (not `warn`): it's a known,
   decided state, not a fault.

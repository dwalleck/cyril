# cyril-0wyn — design: cyril's ACP clientInfo identity on KAS

Status: **APPROVED 2026-07-18** (pause decisions: honest default RATIFIED;
`present_as` knob pulled into scope from cyril-jrl1; claim-2 manual
release-audit fence APPROVED; title="Cyril" YES; advisory level `info`).
Probes: `findings.md` (all pass).

## Purpose

Decide — and make explicit, fenced, and observable — what cyril presents as
ACP `clientInfo` to agents, given that KAS derives persona, remote-tool
allowlist, hooks briefing, and repository honoring from `clientInfo.name`,
silently falling back to `kiro-ide` for unrecognized names.

## The decision (approved at the pause)

**Cyril presents its own honest identity by default** — `name="cyril"`,
`title="Cyril"`, `version=CARGO_PKG_VERSION` — knowingly accepting the
kiro-ide fallback on KAS, **plus an opt-in config knob**
`[agent] present_as = "kiro-cli"` for users who need the kiro-cli allowlist
branch (memoryEnabled/searchMemories). Recorded as ADR-0006.

Rejected alternatives (recorded in the ADR):
- **Impersonate by default**: costs honest telemetry
  (`reference_kiro_acp_telemetry`) and the be-legitimate positioning
  (pi-kiro lesson); brittle against upstream name-keyed changes.
- **Honest name + override knob on the KAS side**: **does not exist** —
  probe-proven (findings.md fact 1); only `KIRO_LOAD_ALL_REMOTE_TOOLS=true`
  exists (allowlist `*`, debug-grade, tools only).
- **Fix upstream**: right long-term answer, not in cyril's hands — tracked at
  **cyril-ctnv** (verified: filed this session; ready-for-human).

cyril-jrl1's knob scope moves into this PR; its remaining residue (live
verification that memoryEnabled + kiro-cli actually surfaces searchMemories)
is resolved by the timeboxed claim-8 probe below — jrl1 narrows or closes at
close-out depending on that probe's outcome.

## Architecture (what actually changes)

1. **`PresentAs` enum** (cyril-core types): `Cyril` (default) | `KiroCli`,
   serde `"cyril"` / `"kiro-cli"`. New `[agent] present_as` field on
   `AgentConfig` (`#[serde(default)]` — absent = honest). Invalid values
   follow the existing config posture: whole-file warn + defaults (the
   `engine = "bogus"` precedent, config.rs:78-84).
2. **Single-source identity**: extract clientInfo construction from
   `run_loop` (bridge.rs:660) into a pure `client_info(present_as)` fn.
   `name` = `present_as` wire name; **`title` = `"Cyril"` always** (the
   impersonation is deliberately never total — Kiro-side logs/telemetry can
   always identify cyril via title); `version` = `CARGO_PKG_VERSION`.
3. **Engine scoping**: `present_as` is KAS-only. With the V2 engine bound,
   `KiroCli` is inert — wire stays `"cyril"` — and a `warn` is logged (the
   knob asked for something v2 cannot use; silent-ignore would violate the
   silent-failure rules, and unlike `kas_spawn` this knob changes what we
   tell third parties).
4. **Fail-loud advisory** (pure `identity_advisory(engine_kind, present_as)
   -> Option<String>`, keyed off the **bound engine at runtime**, never
   `cfg(feature)` — the cyril-dn91/ADR-0002 trap):
   - Kas + Cyril → `info`: "KAS classifies cyril as kiro-ide (fallback):
     IDE persona, channel-gated remote tools, hooks briefing injected — see
     ADR-0006."
   - Kas + KiroCli → `info`: "presenting as kiro-cli (opt-in impersonation):
     CLI persona, memoryEnabled remote-tools branch; Kiro telemetry will
     attribute sessions to kiro-cli — see ADR-0006."
   - V2 + anything → `None` (the claim-7 warn is separate, at config-read).
5. **ADR-0006**: decision, alternatives, the four client-keyed behaviors
   (persona, allowlist, hooksBlock, honorsRepositories), the env bypass,
   pointers to cyril-jrl1 residue / cyril-ctnv / cyril-jiyn.
6. **Release-audit checklist line**: wire-audit methodology gains a
   per-release step — re-carve `resolveAgentContext`, re-run
   `probe-b-name-ab.py` (claim 2's approved manual fence).

## Input shapes (step 2)

- **PresentAs × EngineKind matrix** (all four cells claimed):
  (Cyril,V2) → claim 1; (Cyril,Kas) → claims 1+3; (KiroCli,Kas) → claims
  5+3; (KiroCli,V2) → claim 7.
- **Config field presence**: absent → `Cyril` (claim 1 default path);
  present-valid → per matrix; present-invalid string → claim 6 (house
  posture: warn + defaults).
- **Build features**: `kas` off | on — claim 1 verified under both builds
  (probe A ran twice). The `KiroCli` variants require the `kas` feature
  build only in so far as the KAS engine does; the enum itself is
  feature-independent.
- **KAS name classes**: recognized (`kiro-web`|`kiro-ide`|`kiro-cli`) vs
  unrecognized (`cyril`) — claim 2 live-covers `cyril`/`kiro-cli`/`kiro-ide`.
  `kiro-web` not live-tested: out of scope — same string-equality shape as
  the two tested recognized names, and `PresentAs` cannot express it.
- **KAS executionEnvironment**: `local` claimed; `sandbox` out of scope —
  cyril always spawns KAS as a local child.
- **Version string**: always `CARGO_PKG_VERSION`; no variance.

## Removed-invariant sweep (step 2b)

Purely additive: no lock, guard, ordering, or serialization point is
removed. One subtlety checked: the refactor must not *widen* who can vary
the identity — `client_info()` takes `PresentAs` (a config-derived value
fixed at spawn), so identity remains constant for a bridge's lifetime; no
mid-session identity change is representable. No invariant sweep required.

## Claims and falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | Default (no config): initialize carries name=`"cyril"`, title=`"Cyril"`, version=`CARGO_PKG_VERSION`, identical under default and `kas` builds | dump-agent wire capture (`probe-a-dump-agent.sh`), both builds | captured wire bytes vs `Cargo.toml` + literal (manifest ≠ runtime path) | 5m | **passed** (both builds; title pending impl — pre-change `null` captured, post-impl re-capture in build phase) | unit `client_info_default_is_cyril_identity` (catches: rename, dropped title, version drift) |
| 2 | KAS 2.13.0 classifies `cyril` → kiro-ide + warn; `kiro-cli`/`kiro-ide` accepted silently | `probe-b-name-ab.py` ×3 names | per-run `~/.kiro/logs/<ts>/kiro.log` vs carved `resolveAgentContext` | 10m | **passed** (ALL-PASS) | **manual (release-audit)** — APPROVED at pause; checklist line lands in this PR |
| 3 | `identity_advisory`: `Some(fallback text)` iff (Kas,Cyril); `Some(impersonation text)` iff (Kas,KiroCli); `None` for V2 — keyed off `engine.kind()`, not `cfg(feature)` | unit test all 4 matrix cells under `--features kas` build | per-cell asserts on distinct message substrings (catches: dn91 feature-flag keying — kas-feature+v2-engine cell fails; swapped messages) | 5m | pending | unit `advisory_matrix` |
| 5 | `present_as = "kiro-cli"` + KAS engine: initialize carries name=`"kiro-cli"`, title still `"Cyril"` | unit test on `client_info(KiroCli)` + dump-agent capture with config set | wire bytes / constructed frame vs the enum's wire-name table (catches: knob ignored; title overwritten to impersonated name) | 5m | pending | unit `client_info_present_as_kiro_cli` |
| 6 | Invalid `present_as` string in config → whole-config warn + defaults (honest identity), per existing posture | unit test: toml with `present_as = "kiro-web"` → `Config::default()` | parsed struct vs `Default` impl (catches: partial parse that keeps a bogus name; panic on bad value) | 5m | pending | unit `invalid_present_as_falls_back_to_default_config` |
| 7 | `present_as = "kiro-cli"` + V2 engine: wire stays `"cyril"` and a warn is logged at spawn | unit test on the effective-identity resolution for (KiroCli,V2) | resolved name vs the honest constant (catches: knob leaking into v2 telemetry silently) | 5m | pending | unit `present_as_inert_on_v2` |
| 8 | (timeboxed, 30m) With name=`kiro-cli` + `memoryEnable` settings, KAS's remote-tools discovery includes searchMemories | standalone-KAS probe: initialize with kiroMeta settings, grep discovery/log lines | KAS log lines vs carved `resolveRemoteToolAllowlist` | 30m | pending (build phase; may be inconclusive without auth'd session) | **manual (release-audit)** if it passes; if inconclusive → cyril-jrl1 (verified) narrows to this residue and stays open |

(Claim 4 from the draft — ADR completeness — kept: grep the ADR for the four
behavior names + env bypass, ≥5 distinct hits; cost 1m; fence manual/review,
approved at pause as part of the fence decision.)

Distinctness: every claim has its own named unit test, probe verdict line,
or grep count — a failure localizes to its claim.

## Negative space (what this deliberately does NOT do)

1. **No arbitrary impersonation**: `PresentAs` is a two-variant enum —
   `kiro-web`, `kiro-ide`, and free strings are unrepresentable (illegal
   states, house style).
2. **Impersonation is never total**: `title` stays `"Cyril"` in every mode —
   fenced by claims 1 and 5, and stated in the ADR as a non-negotiable.
3. **No wire-detection or log-tailing**: cyril does not detect its resolved
   client type (impossible per findings Q3) nor tail KAS's log dir.
4. **No hooks-briefing correction**: cyril-jiyn (KAS-7) scope; the
   impersonation advisory does not attempt to compensate for the missing
   hooks block either.
5. **No v2 behavior change**: knob inert on v2 (claim 7); v2 wire identical
   to today except the added title.
6. **No env management**: `KIRO_LOAD_ALL_REMOTE_TOOLS` passes through
   untouched; documented in the ADR only.

## Pause decisions (recorded)

1. Identity: **honest default + build the knob now** (user, 2026-07-18).
2. Claim-2 fence: **manual (release-audit) approved**.
3. `title = "Cyril"`: **yes**.
4. Advisory level: **info**.

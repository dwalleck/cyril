# cyril-0wyn — budgeted plan

Design: `.cyril-0wyn/design.md` (approved at pause). Claims 1-8 mapped below.
Gates per slice: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check` — real exit codes, no pipes. KAS-relevant slices also
build `--features kas`.

No slice introduces a loop. No slice writes to stdout (TUI binary; all
diagnostics go through `tracing` into `cyril.log`). No slice introduces a
documented precondition (all new fns are total over enum inputs) — no
runtime checks or `debug_assert!`s required by the doc-comment rule.

## Slice 1: `PresentAs` enum + `[agent] present_as` config field

**Claim:** 6 (invalid value → house posture: whole-config warn + defaults),
plus the config-shape halves of 1 and 5.
**Oracle:** serde round-trips vs the TOML strings written in the fixture
(literal comparison, not through the enum's own Display).
**Stress fixture:** TOML variants — `present_as = "kiro-web"` (a REAL KAS
name deliberately unrepresentable; catches enum-too-wide), `"KiroCli"`
(case-sensitivity; serde must not accept), `"kiro-cli"` (the one valid
opt-in), field absent (default = Cyril). Expected outputs written first:
defaults / defaults / KiroCli / Cyril.
**Loop budget:** none (derive-serde on a 2-variant enum).
**Wall budget:** n/a (config parse, once at startup, existing path).
**Files:** `crates/cyril-core/src/types/present_as.rs` (new),
`crates/cyril-core/src/types/config.rs`,
`crates/cyril-core/src/types/mod.rs` (single `pub mod` line — counted
honestly as a third file; it is module registration, not logic).

**Verification:**
- [ ] Unit tests pass (incl. fence `invalid_present_as_falls_back_to_default_config`)
- [ ] Stress fixture produces expected outcome
- [ ] Oracle agrees (TOML literals)
- [ ] Budgets hold (no loops)

## Slice 2: `client_info(present_as)` — single-source identity + title

**Claim:** 1 and 5 (construction halves): default → name `"cyril"`; KiroCli →
name `"kiro-cli"`; **title `"Cyril"` in BOTH**; version `CARGO_PKG_VERSION`.
**Oracle:** `Cargo.toml` workspace version (manifest read ≠ the env! macro
path) + the design's wire-name table.
**Stress fixture:** the KiroCli cell asserting title is STILL `"Cyril"` —
designed to fail a plausible "impersonation overwrites the whole struct"
implementation; version asserted non-empty AND equal to manifest.
**Loop budget:** none.
**Wall budget:** n/a (pure constructor).
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (fn + tests; call-site
swap happens in slice 4).

**Verification:**
- [ ] Unit tests pass (`client_info_default_is_cyril_identity`,
      `client_info_present_as_kiro_cli`)
- [ ] Stress fixture produces expected outcome
- [ ] Oracle agrees
- [ ] Budgets hold

## Slice 3: engine-scoped resolution + advisory matrix (pure)

**Claim:** 3 (advisory matrix) and 7 (resolution: KiroCli inert on V2, with
a warn signal).
**Oracle:** the design's 4-cell matrix table (design.md), asserted cell by
cell — not derived from the implementation.
**Stress fixture:** tests run under `--features kas` while asserting the
**V2-engine** cells — a `cfg(feature)`-keyed implementation (the
cyril-dn91/ADR-0002 trap) fails exactly those cells. Additional assert: the
two `Some` advisory texts DIFFER (catches swapped messages).
`effective_present_as(V2, KiroCli)` must return `(Cyril, warn=true)` —
catches silent-inert.
**Loop budget:** none (match over 2×2 enums).
**Wall budget:** n/a (called once per bridge startup).
**Files:** `crates/cyril-core/src/protocol/identity.rs` (new),
`crates/cyril-core/src/protocol/mod.rs` (registration line).

**Verification:**
- [ ] Unit tests pass (`advisory_matrix`, `present_as_inert_on_v2`)
- [ ] Stress fixture produces expected outcome
- [ ] Oracle agrees (design table)
- [ ] Budgets hold

## Slice 4: plumbing — config → spawn_bridge → run_loop emission

**Claim:** end-to-end halves of 1/5/7 + emission of 3 (advisory logged at
`info` once per bridge startup; V2+KiroCli warn logged at spawn).
**Oracle:** the prove-it dump-agent (`probe-a-dump-agent.sh`) re-run
post-implementation: captured initialize frame must now read
`"clientInfo":{"name":"cyril","title":"Cyril","version":"0.2.0-alpha.1"}` —
wire bytes vs unit expectations (independent serialization path).
**Stress fixture:** byte-level diff of the captured `clientInfo` object
against the expected JSON (catches serde field-rename and
built-the-wrong-struct bugs that unit tests on the constructor cannot see).
**Loop budget:** none.
**Wall budget:** one extra `tracing` call per bridge startup — negligible,
stated for completeness.
**Files (atomic signature ripple — justified):** `spawn_bridge` gains
`present_as: PresentAs`; the compile unit forces all callers in one slice:
`crates/cyril-core/src/protocol/bridge.rs`, `crates/cyril/src/main.rs`,
`crates/cyril/examples/test_bridge.rs`,
`crates/cyril/examples/l7tw_death_probe.rs`,
`crates/cyril-core/tests/kas_freepath_smoke.rs`,
`crates/cyril-core/tests/kas_settings_handshake_smoke.rs`.
Six files, but 1-3 mechanical lines each outside bridge.rs; the >2-file rule
is overridden by compile atomicity, not by ambition. Callers pass
`PresentAs::default()` except main.rs (config value).

**Verification:**
- [ ] Unit + integration tests pass, both default and `--features kas` builds
- [ ] Stress fixture: probe-A re-capture byte-matches (title now `"Cyril"`)
- [ ] prove-it oracle still agrees (bridge.rs source vs wire)
- [ ] Budgets hold

## Slice 5: ADR-0006 + release-audit checklist line

**Claim:** 4 (ADR names all four client-keyed behaviors + env bypass) and
claim 2's approved manual fence (the checklist line that re-runs probe B per
release).
**Oracle:** `grep -c` over the ADR for
`persona|allowlist|hooksBlock|honorsRepositories|KIRO_LOAD_ALL_REMOTE_TOOLS`
— ≥5 distinct hits (grep vs the carve artifacts in `.cyril-0wyn/`, not vs
the ADR's own claims). Expected output written first: 5 distinct patterns
each ≥1 hit.
**Stress fixture:** the grep MUST find `honorsRepositories` — the
easy-to-forget fourth behavior discovered by the probe, absent from the
original issue text; a lazy ADR that copies the issue verbatim fails it.
**Loop budget:** none (docs).
**Wall budget:** n/a.
**Files:** `docs/adr/0006-clientinfo-identity.md` (new),
`experiments/conductor-spike/README.md` (checklist line: per-release,
re-carve `resolveAgentContext` + re-run `.cyril-0wyn/probe-b-name-ab.py`).

**Verification:**
- [ ] Claim-4 grep passes (5/5 patterns)
- [ ] Checklist line present and names the probe path
- [ ] ADR cites cyril-jrl1 (residue), cyril-ctnv (upstream ask), cyril-jiyn
      (hooks coupling) — all verified existing this session
- [ ] Budgets hold (trivially)

## Slice 6: claim-8 probe — does kiro-cli + memoryEnable surface memory tools? (timeboxed 30m)

**Claim:** 8. With `name="kiro-cli"` + memory-enable settings in
`_meta.kiro.settings`, KAS remote-tools discovery includes the
searchMemories/search_memories tool; control run with `name="cyril"` (same
settings) must NOT (kiro-ide stable channel lacks it).
**Oracle:** carved `resolveRemoteToolAllowlist`
(`oracle-resolveRemoteToolAllowlist.txt`) — source text vs live log lines.
**Stress fixture:** the CONTROL run (cyril + same settings) — a probe that
only runs the treatment arm would "pass" even if the settings flag alone
(not the name) unlocked the tool. Expected outputs written first:
treatment=present, control=absent; anything else = inconclusive.
**Loop budget:** none (script; 2 spawns).
**Wall budget:** 30 minutes total including interpretation — HARD timebox;
on expiry or unclear logs, record `inconclusive` in findings.md and
**cyril-jrl1** (verified: filed this session) narrows to this residue and
stays open. The exact settings key (`memoryEnable` vs post-2.13.0
`search_memories` naming) is verified against the covenant/carve INSIDE the
timebox before the runs.
**Files:** `.cyril-0wyn/probe-c-memory-tools.py` (new),
`.cyril-0wyn/findings.md` (addendum).

**Verification:**
- [ ] Probe ran (or timebox expiry recorded honestly)
- [ ] Treatment/control outcomes recorded with log excerpts
- [ ] Oracle comparison written into findings.md
- [ ] jrl1 disposition decided (close vs narrow) and noted for close-out

## Plan Self-Review

1. **Loops:** none introduced anywhere (2-variant enum matches, one config
   field, docs, a 2-spawn script). No budget entries needed; stated per
   slice. **No gaps.**
2. **Fixtures:** slice 1 — enum-too-wide/case/partial-default; slice 2 —
   impersonation-overwrites-title, version drift; slice 3 — dn91
   feature-keying trap, swapped messages, silent-inert; slice 4 — serde
   rename / wrong-struct via wire byte-diff; slice 5 — lazy-ADR-misses-
   honorsRepositories; slice 6 — control arm against
   settings-alone-unlocks. All adversarial, none happy-path-only. **No
   gaps.**
3. **Doc-comment preconditions:** none introduced; all new fns total over
   enums. **No gaps.**
4. **Write targets:** `tracing` (diagnostic → cyril.log) only; probe scripts
   print verdicts to stdout (data — they're pipeable probe output, matching
   probe-b). **No gaps.**
5. **Tracker references:** cyril-jrl1 (slice 6 residue path), cyril-ctnv
   (ADR pointer), cyril-jiyn (ADR pointer) — all three verified existing
   this session (jrl1/ctnv filed and labeled; jiyn shown). **No gaps.**

Claim coverage: 1→S2+S4, 2→S5 (fence; behavior already passed pre-plan),
3→S3+S4, 4→S5, 5→S1+S2+S4, 6→S1, 7→S3+S4, 8→S6. Complete.

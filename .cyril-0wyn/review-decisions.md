# cyril-0wyn — review-feedback decisions (2026-07-19, post-PR deep review)

14 findings assessed per gilfoyle:assessing-review-feedback. Every bug claim
was independently verified before applying; finding 10's external facts were
re-checked against `protocol/kas/settings.rs` (BOOL_MAP: boolean →
`{enabled:<bool>}`) and `docs/kiro-2.13.0-wire-audit.md` (cap-injection
table `memoryEnable:{enabled:true}`) rather than taken on citation.

| # | Finding (one line) | Category | Verified? | Decision | Note / commit |
|---|---|---|---|---|---|
| 1 | ADR/design claim probe C "verified"; actual result INCONCLUSIVE | Docs bug | Yes (read all three artifacts) | Accept | eceec4c — ADR + design header corrected, artifact cited |
| 2 | Tracker says "SHIPPED" pre-merge; 0wyn still reads DECISION NEEDED | Docs bug | Yes (jsonl text) | Accept (modified wording) | eceec4c — implemented-pending-merge; ratified-decision addendum on 0wyn |
| 3 | Pre-change probe-A capture not committed | Evidence gap | Yes (only post-impl capture in repo; baselines survived in scratchpad) | Accept | eceec4c — both baselines committed (default + kas builds) |
| 4 | Wire-invisibility conclusion exceeds the initialize-only experiment | Over-claim | Yes (Q3 diffed initialize only; allowlist plausibly visible downstream) | Accept (narrowed, per reviewer's own alternative) | eceec4c — "not exposed by the initialize response" across findings/design/ADR/identity.rs |
| 5 | "No auth needed" not isolated from ambient credentials | Over-claim | Yes (stderr: "Auth: default token file"; HOME inherited) | Accept (narrowed) | eceec4c — "no ACP-level auth exchange"; isolation caveat recorded |
| 6 | Fact 5 names setClientType as the discovery seam; actual seam is the lazy agentContext.client read | Docs bug | Yes (carve: getAllowedTools closure reads this.agentContext.client; setClientType is telemetry-side) | Accept | eceec4c — fact 5 rewritten to match the addendum |
| 7 | Probe B blocking readline defeats the 12s deadline | Harness bug | Yes (by construction) | Accept | cf6348e — select() before every read |
| 8 | Probe B can mix concurrent Kiro processes' logs into an arm | Harness bug | Yes (glob over ~/.kiro/logs) | Accept | cf6348e — bound to the arm's own logDir from its initialize response |
| 9 | Probe B arm can PASS with no response at all | Harness bug (vacuity) | Yes (verdict ignored got_response) | Accept | cf6348e — PASS requires result + Stored line; re-run ALL-PASS |
| 10 | Probe C sends bare `memoryEnable: true`; wire shape is `{enabled:true}` | Harness bug (P1) | Yes (settings.rs BOOL_MAP + audit cap-injection table) | Accept | this commit — object shape; jrl1 carries the shape correction so an auth'd rerun can actually arm the gate |
| 11 | Probe C proceeds without a successful initialize | Harness bug | Yes (same readline pattern; accepted error responses) | Accept | this commit — init gate; INCONCLUSIVE on failure |
| 12 | Probe C log mixing (same as 8) | Harness bug | Yes | Accept | this commit — logDir binding |
| 13 | Any search_memories mention counted as allowlist evidence | Harness bug | Yes (grep over all lines incl. settings/errors) | Accept | this commit — only parsed `Allowlist resolved` payloads count |
| 14 | PASS possible without a resolved control arm | Harness bug (P1, vacuity) | Yes (`not c_has` also matches a crashed control) | Accept | this commit — BOTH arms must resolve before comparison; re-run gives clean INCONCLUSIVE |

Outcome notes: 14/14 verified real — atypical for a review batch, but 12 are
about probe/evidence rigor where each claim was mechanically checkable, and
two "accepts" (4, 5) took the reviewer's narrower-wording alternative rather
than new experiments. No finding changed shipped product behavior: the Rust
diff from this batch is one doc-comment scope correction in `identity.rs`.
The substantive verdicts stand: probe B ALL-PASS under the hardened harness;
probe C INCONCLUSIVE via the strict both-arms path (residue: cyril-jrl1).

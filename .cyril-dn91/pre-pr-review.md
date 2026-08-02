# cyril-dn91 — pre-PR review (2026-08-02), two-axis, per-finding dispositions

Fixed point: `main` (3e9f6d5). Both axes ran as parallel sub-agents; every
finding verified before acting (assessing-review-feedback discipline).

## Standards axis — 0 hard violations, 5 judgement calls

| # | Finding | Disposition |
|---|---|---|
| S1 | Duplicated host_io gate ×9 in client.rs | **ACCEPT** — extracted `KiroClient::require_host_io(method)?`; all 9 sites converted; a 10th host-io method can no longer forget the gate's shape |
| S2 | Speculative Generality: `AuthAdapter`/`HostIoAdapter` unit markers | **REJECT** — the ADR-0001 amendment names per-concern adapters as the growth point the g9vt mediator dispatches to; approved design decision, bounded by negative space |
| S3 | Tiebreak collapses corrupt `expires_at` to None unlogged; equal-expiry preference undocumented | **ACCEPT** — `debug!` breadcrumb on present-but-unparseable stamps + comment documenting the builder-id preference on equal/unparseable |
| S4 | `Result<_, String>` errors instead of thiserror in auth.rs | **REJECT (tracked)** — continuation of the module's existing contract, mapped to JSON-RPC at the boundary; the thiserror migration is cyril-5db7 (verified open) |
| S5 | `probe_dn91` module name issue-keyed, not intent-keyed | **REJECT** — repo probe-module convention (probe_qo13 precedent); module doc states intent |

## Spec axis — 5 findings

| # | Finding | Disposition |
|---|---|---|
| P1 | AC3 satisfied at family granularity, not advertised-bit granularity (fs.writeTextFile + dialect flags unpaired inside the ONE test) | **ACCEPT (modified)** — matrix now also pairs fs write and a `_kiro/fs` dialect representative (stat) per engine; full per-op dialect pairing remains `every_advertised_fs_flag_is_dispatched` (itself an advertise↔dispatch coupling test) |
| P2 | y14u "accountType FIRST" fix direction not followed; rejection unrecorded | **ACCEPT (record only)** — decision now recorded in build-audit: accountType lives outside the credential store (a `whoami` subprocess or extra state row), while both token rows are already in hand; freshest-expiry is deterministic, fenced, and the leftover-shadowing risk it addresses is the same one accountType-first addresses. Implementation unchanged |
| P3 | v2 live leg proves no refusal live | **REJECT** — v2 never initiates host callbacks, so no live path can exercise a refusal; the refusal surface is synthetic by nature and fence-proven (recorded in build-audit) |
| P4 | Freshest-expiry tiebreak + inline-arn fallback exceed minimal y14u | **REJECT** — both are small, fenced, and guard store shapes carried by the rows themselves; no refresh logic added (taba untouched) |
| P5 | C13 exemption swallows any auth-path `MethodNotFound`, not just adapter refusals | **REJECT (documented limitation)** — the real responder emits only -32603/-32000 today; distinguishing refusal provenance needs the g9vt mediator's typed path, not a string marker here |

Post-fix gates: tests both configs, clippy both configs `-D warnings`, fmt — all green (see final gate run before the review-fix commit).

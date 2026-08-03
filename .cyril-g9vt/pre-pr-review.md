# cyril-g9vt — pre-PR review (2026-08-03), two-axis, per-finding dispositions

Fixed point: `main`. Both axes ran as parallel sub-agents; each finding
verified before acting (assessing-review-feedback).

## Standards axis — 0 hard violations

| # | Finding | Disposition |
|---|---|---|
| S1 | `dispatch()` reply-send closure duplicated 12× | **ACCEPT** — extracted `resolve<T>(ctx, kind, notify, reply, result)`; the 12 request arms collapse to one line each, `kind` derived from `CallbackMeta::kind()` |
| S2 | `HooksExecuteArgs::parse`/`KiroFsArgs::parse` return `Result<_, String>` vs thiserror | **REJECT (tracked)** — single consumer maps to `-32602` at the seam; the module's error contract is `String`→JSON-RPC, and thiserror migration is cyril-5db7 (verified open) |
| S3 | Raw `-32602`/`-32603` literals | **REJECT** — matches the surrounding `kas::` idiom (auth.rs, kiro_fs.rs); introducing constants only here would diverge |
| S4 | Inconsistent seam rigor: fs/hooks hard-error on bad params, shell_type/hooks_list lenient `None` | **REJECT** — deliberate: shell_type's sessionId and hooks_list's trigger are OPTIONAL on the wire (missing trigger → reply-empty is the responder's documented behavior), so `None` is correct, not a swallowed error |
| S5 | `finish` is a Mysterious Name | **REJECT** — short, and its doc states the notify-before-resolve ordering it enforces; renaming (`resolve_ordered`?) collides with the new `resolve` helper |
| S6 | cfg-staging honest but cancel machinery has no production producer (Speculative Generality) | **ACCEPT (comments) + FILE** — bridge.rs cancel_scope/shutdown comments made honest ("no production family opts in yet; hooks cancel via HookOps, terminals via the registry"); the opt-in wiring filed as **cyril-740a** |
| S7 | `Rc<RefCell<HostMediator>>` sharing | **VERIFIED SOUND** — single LocalSet thread, no `borrow_mut()` held across any await (both reviewers concurred); noted, no change |
| S8 | `op_for_kind` `unwrap_or_else(unreachable!)` panic | **ACCEPT (modified)** — `.expect()` is deny under `-D warnings` (clippy::expect_used), so rewrote as a TOTAL indexed match (no unwrap/expect/panic/sentinel) + `op_for_kind_indices_match_fs_ops` order-fence |
| S9 | redundant `hooks_direction_is_none() || !serves_inbound_hooks()` | **ACCEPT** — `serves_inbound_hooks()` alone (it's false for every non-Inbound direction incl. None) |
| S10 | stale `spawn_test_mediation` doc | **ACCEPT** — corrected to "spawn_local concurrent resolution mirroring run_loop's drain" |

## Spec axis — findings

| # | Finding | Disposition |
|---|---|---|
| P1 | AC4 cancellation/shutdown proven only on a synthetic type; production machinery vacuous | **ACCEPT** — same as S6: honest comments + cyril-740a. Design C2/C8 machinery is unit-fenced and is the mandated seam; the vacuity is now tracked, not hidden |
| P2 | AC4 "non-blocking mediation" has no automated fence for the fixed deadlock; live-only | **ACCEPT (modified)** — added `callback_during_new_session_completes` (a KAS callback issued DURING new_session; the session completes). Verified it does NOT establish non-vacuity against drain removal (the in-process duplex harness doesn't reproduce the real subprocess's loop-blocking), so it's labeled a SMOKE TEST, and the build-audit records the live parity check as the authoritative deadlock evidence — per the non-vacuity discipline, no decoration fence |
| P3 | Residual deadlock class: `finish()` awaits `notify_tx.send` inside dispatch — one channel deeper | **REJECT (documented)** — the drain task is not a select! arm, so a full inbound channel parks only the resolution task, never run_loop; improbable and structurally different from the fixed bug. Noted here |
| P4 | stale `read_text_file` doc ("deferred to cyril-g9vt") | **ACCEPT** — corrected to "crosses the mediation seam since cyril-g9vt" |

All ACs verified holding (both reviewers): AC1 census + zero direct responder
calls in client.rs; AC2 sync unit tests, arm delegates; AC3 default build +
probe_g9vt_c13; AC5 census; AC6 live both engines. Both ADR escapes (3lh8
terminal Rc, inline hook arms) deleted. Negative space clean (permission
untouched, no stages gate, no new KAS-8 surfaces, turn_mediator untouched).

Post-fix gates: tests + clippy both configs `-D warnings` + fmt — all green.

# PR #94 review-feedback decisions (assessing-review-feedback)

Reviewer: the user's own pass over PR #94. Every finding verified before
application; per-finding commits follow this table's order.

| # | Finding (one line) | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | Reap script's global pgrep can SIGKILL an unrelated session's node | Bug (probe) | Yes (by construction; a concurrent session was live during the original runs) | Modify | PPID-scoped matching (`pgrep -P <probe>`); kills probe-owned pids only |
| S2 | Probe truncates model text at a byte boundary → UTF-8 panic | Bug (probe) | Yes (byte-slice on external text) | Accept | `chars().take(20)` |
| S3a | main.rs comment falsely claims no bridge knob has a TOML key | Doc bug | Yes (every other SpawnConfig field mirrors `[agent]` keys) | Accept | Comment + design.md negative-space rationale corrected with an explicit correction marker |
| S3b | App test doc says three arms; it has four | Doc bug | Yes | Accept | One word |
| SP1 | Cancel-under-stall remains unverified; healthy-stream cancel is not its proof | Spec gap | Yes — and the PR already records it as unresolved | Reject (defer) | Tracked at **cyril-w9oi**; reviewer's deterministic blocked-backend fixture idea added to w9oi as a note |
| SP2 | "Teardown already implemented" overclaims: guard is cfg(unix); Windows wrapper leaks grandchild; hard death orphans everywhere | Spec scoping | Yes (transport.rs cfg; Cargo.toml note) | Modify (deferred work) | findings/design claims re-scoped to unix-clean-paths; gaps filed as **cyril-jlw9** |
| SP3 | Bridge-synthesized notifications (CommandExecuted, SettingsList, …) permanently vanish the stall chip | Bug (production) | Yes (emit sites bypass the stamping arm; armed stays false) | Modify | Clear-set narrowed to agent-turn traffic (text/thought/tool/plan/terminal); unknown variants keep the chip; fence + mutation red |
| SP4 | Sub-tick host callbacks invisible to `in_flight` sampling → stall fires seconds after a callback reply | Bug (production) | Yes (accept+complete between two ticks is unobservable) | Modify | `HostMediator::last_transition` stamp read at tick; treated as activity + re-arm, clamped to now; fence + mutation red. Residual: the tick's 2-line sampling still has no loop-level mutation fence (unit-fenced) |
| SP5 | Reap oracle passes vacuously when the probe never reaches READY | Bug (probe oracle) | Yes — demonstrated live: the hardened script fail-louded on the auth-expired rerun where the old one would have printed a clean-looking pass | Accept | READY required; ≥1 probe-owned node required; stderr kept; exit codes propagate |
| SP6 | falsifier_c11 vacuous on truncated captures; no tight arm | Bug (falsifier) | Yes (healthy==0 passes on zero turns) | Accept | Per-capture turn-count asserts (4/4/4, 1 uncompleted) + ≥1 @3s observability arm |
| SP7 | Capture provenance not in checkout; “12 healthy turns” wrong; related-issues filename typo | Doc/provenance | Yes (fixture=8 healthy; measured corpus=11; runs 5/6 were scratchpad-only) | Accept | Source captures committed beside the generator (sha256: healthy-a d76525bf…, healthy-b ad35f121…, stall-run 2607ce7e…); fixture regenerates byte-identical from them; counts corrected; filename fixed |
| SP8 | Replay fence doesn't pin 30s (900s would still pass) | Bug (fence) | Yes (stall tail ~976s ⇒ upward drift unpinned) | Accept | `assert_eq!(DEFAULT_STALL_THRESHOLD, 30s)` in the fence |
| SP9 | falsifier semantics drift vs emit_table (no id pairing; > vs >=) | Bug (falsifier) | Yes | Accept | Mediator-faithful parsing shared with the generator; inclusive >= |

Ratio: 10 accept/modify, 1 reject-with-tracker, 2 doc corrections — three of the
findings (SP3, SP4, SP8) were real production/fence bugs both pre-PR review
sub-agents missed.

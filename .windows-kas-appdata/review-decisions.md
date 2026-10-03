# Review decisions

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Auth callback still calls missing Windows AppData a missing HOME and adds a login-only remedy. | WindowsKasReview | Verified | `auth.rs:305-314` used the literal HOME error; `callbacks.rs:515-518` adds the login hint unless already present. | Modify | Auth callback reuses `KasMissing::NoHomeForStore.reason()`; recovery explicitly restores the platform directory source before login. | blocking, repaired; preserve one shared diagnostic rather than add error codes or another platform switch in auth. |
| F2 | Reusing the old non-Windows store reason duplicates the login command when callbacks append their hint. | WindowsKasReview | Verified | `callbacks.rs:515-518` suppresses the suffix only for the exact LOGIN_HINT; the old non-Windows reason only parenthesized the command. | Accept | Non-Windows recovery now restores the home directory before the exact login hint, matching the Windows message composition. | blocking, repaired; directory resolution on other platforms remains unchanged. |

Final scoped independent review by WindowsKasReview: no findings. Both recovery branches contain the exact login hint once; auth forwards the shared reason and callbacks suppress the suffix. Earlier root resolution, dependency and README review remains valid. PublicationFenceReview also found no issues in the added real-producer regression. Final runtime/repository checks are recorded in route.md; publication authorization is recorded there.

Repair-mechanism decision: retain the single platform-aware diagnostic shared by startup and callback; do not add a new error hierarchy or alter generic callback handling for this local directory repair. The second finding arose from composing two existing messages, not from the path resolver.

F2 regression proof: `auth_failure_hint_not_doubled` exercises the actual `KasMissing::NoHomeForStore.reason()` through `auth_failure_notification`. Reintroducing the old non-Windows recovery text fails the assertion (2 login-command mentions vs 1, artifact 150). The exact pre-mutation source was restored byte-for-byte; the focused regression and final KAS-enabled workspace tests, Clippy, and formatting check passed (artifact 162).

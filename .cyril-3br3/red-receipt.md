# Red receipt: cyril-3br3

Fence: `distro_non_unicode_warns_and_treats_as_unset_via_child_process`
(`crates/cyril-core/tests/win_wsl_wiring.rs`), run against the unmodified
`process_wsl_distro` (`crates/cyril-core/src/platform/path.rs`, revision
f9bc81d8 + the fence only).

Command:

```
env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --test win_wsl_wiring distro_non_unicode
```

Result: exit 100, `1 test run: 0 passed, 1 failed, 4 skipped`.

Child panic (the load-bearing assertion):

```
panicked at crates/cyril-core/tests/win_wsl_wiring.rs:165:9:
a non-Unicode CYRIL_WSL_DISTRO must be warned about; captured: ""
```

Reading: with `CYRIL_WSL_DISTRO=Ubu\xffntu` in the child's environment, the
WARN-level capture subscriber recorded nothing while `wsl_to_win("/home/u")`
initialized the distro `OnceLock` — the `.ok()` collapsed
`VarError::NotUnicode` into silent "unset" (and off Windows the value was never
read at all, because the read sat behind the `cfg!` gate).

Baseline on the same revision (pre-fence): `cargo nextest run -p cyril-core
--test win_wsl_wiring` → `4 tests run: 4 passed` — the existing sibling fences
are green, so the red above is the new fence's, not the environment's.

## Green after the fix

Same command → `5 tests run: 5 passed, 0 skipped` (the new fence plus the four
existing Linux fences). `cargo nextest run -p cyril-core --lib -- platform::path`
→ `42 tests run: 42 passed`.

## Mutation: read moved back behind the `cfg!` gate

The initial red covered two defects at once (`.ok()` collapse AND the read
sitting behind the host gate off Windows). To prove the fence is sensitive to
the ordering on its own, the fixed `match` was kept and the
`if !cfg!(target_os = "windows") { return None; }` gate was moved back above
it:

```
env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --test win_wsl_wiring distro_non_unicode
→ exit 100; 1 test run: 0 passed, 1 failed
   a non-Unicode CYRIL_WSL_DISTRO must be warned about; captured: ""
```

Restored (gate below the read, via editor — not `git checkout`):
`cargo nextest run -p cyril-core --test win_wsl_wiring` →
`5 tests run: 5 passed, 0 skipped`.

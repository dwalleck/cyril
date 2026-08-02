# Related issues

- `cyril-ufie` — closed. Original KAS terminal host-callback implementation; introduced the current direct-argv execution and constant shell-type response.
- `cyril-1rpv` — open P4. Separate terminal output-byte-limit behavior; not part of shell detection or execution semantics.
- `cyril-3lh8` — closed. Terminal lifecycle cleanup on cancellation; constrains process ownership but does not decide shell selection.
- `cyril-2z9g` — open P2, discovered and fixed in this branch. Selected-shell operator pipelines exposed that terminal kill/release/cancel reached only the outer process instead of its children.

`cyril-6bol` has no open `blocks` dependency. Its `discovered-from` link to `cyril-ufie` does not gate readiness.

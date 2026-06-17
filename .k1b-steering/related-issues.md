# Related issues — cyril-bm1j (K1b) prove-it-prototype

- **cyril-bm1j** — K1b queue-steering TUI UX. This spec's subject. Now BLOCKED.
- **cyril-c1qe** — BUG (filed by this probe): K1a steering wire defects — outbound
  `__session/steer` double-underscore + dead inbound `_kiro.dev/session/update`
  converter arm. Mechanical fix. **Blocks cyril-bm1j.**
- **cyril-84ca** — BUG (filed by this probe): bridge command loop blocks on
  `conn.prompt().await` for the whole turn → no mid-turn commands (steer/cancel).
  Architectural fix (drive the prompt off the loop). Also implicates Esc-cancel.
  **Blocks cyril-bm1j.**
- **cyril-f2g8** — K1a (closed) — the merged plumbing these bugs live in.

Prove-it-prototype artifacts: `probe.rs`, `wire_shim.py` (oracle),
`oracle-wire-capture.log`, `findings.md`.

No prior art existed for either bug (rivets had no bridge/wire-defect tickets).

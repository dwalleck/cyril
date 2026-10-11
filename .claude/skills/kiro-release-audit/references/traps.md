# Traps

Each entry produced a wrong finding in an earlier audit. Hand this file to every lane. The rule is
the positive form; the history says why it is a rule.

1. **Wire = binary × backend.** A field that appears between two captures is a backend rollout
   unless the captures were taken the same day with different binaries. Pair OLD and NEW engines
   on the same day against the same backend; attribute by the axis you isolated.
2. **Count in both versions before claiming add or remove.** Minifier renames
   (`orchestrator_default` → `orchestrator_default2`) and LTO re-glue change token counts without
   changing behaviour. Pull the surrounding context from both bundles and quote it.
3. **Diff vocabulary sets, never recount known tokens.** A grep for a fixed list can only report
   "still there". Extract every token matching a broad pattern (`_kiro/[A-Za-z_/]+`,
   `KIRO_[A-Z0-9_]+`, `AB_[A-Z0-9_]+`, `session_info_update` kinds, workflow event literals, slash
   command names) in OLD and NEW and diff the sets. The voice subcommands existed for ten releases
   before a set diff found them.
4. **Static is a lead; live is a verdict.** Mark every row STATIC or LIVE. The 2.28.0 static lanes
   were wrong about a notification's field count and a payload's row shape; the live lane
   disproved both.
5. **Handshake placement.** KAS reads client settings and flags only from
   `initialize.clientCapabilities._meta.kiro` (`.settings.*`, `terminalInput`, `configurationState`,
   `telemetryEnabled`, ...). Write that full path in every probe and include a positive control
   whose effect is visible (a connection-level gate such as `largeToolOutputHandler`). Three
   2.28.0 live conclusions were void because the probe used the request-level `_meta` shorthand.
6. **One live lane owns auth; legs run serially.** The OIDC refresh token is single-use, so two
   concurrent renewals log the user out. Static lanes execute nothing from Kiro: no `kiro-cli`, no
   `node acp-server.js`. The live driver renews single-flight via `kiro-cli user whoami` with a
   grace period so the user's own session renews first; never `login` or `logout`.
7. **Isolate `HOME`, keep `XDG_DATA_HOME` real.** KAS prunes `$HOME/.kiro/logs` and the host writes
   state; the auth store lives under `XDG_DATA_HOME`. A fresh temp `HOME` per spawn with the real
   data home is the only combination that is both clean and authenticated.
8. **Archived launcher resolves `kiro-cli-chat` via PATH.** `binaries/<ver>/.../kiro-cli acp`
   silently spawns the installed `~/.local/bin/kiro-cli-chat`. Prepend the archive's `bin/` to
   PATH and record the child's `/proc/<pid>/exe` in every v2 capture. Direct `node acp-server.js`
   spawns from the carve are immune, and they also avoid the host's stale-engine cleanup.
   The same split applies to probes: **ask the binary that owns the namespace.** `kiro-cli internal
   --help` lists 25 fig/shell helpers and none of the hidden agent helpers; `kiro-cli-chat _
   cleanup-autocomplete-artifacts --dry-run` works, while the launcher errors `unrecognized
   subcommand` and the chat binary rejects the name `internal`. A "subcommand absent" verdict from
   the wrong binary is a wrong-binary artifact (2.29.1, H4).
9. **Binaries are stripped since 2.23.0.** No `nm`/`rustfilt` lane. `crates/<crate>/src/*.rs`
   source-path strings, serde field literals and the compiled-in rollout table survive and are the
   host lane's material. Confirm any Rust-side claim with such a path, never a bare word match
   (the binary also embeds V8, SQLite, Bun and the TUI bundle).
10. **Payloads are zstd frames with nested magics.** Decompress from the magic to EOF with exact
    consumed sizes; bounding at the next magic truncates Bun (see acquire-and-carve.md).
11. **`sweep-new-fields.py` walks only five array elements** (cyril-66tl). A "removed" path on an
    array is suspect; a non-empty diff is a lead, not a verdict, and a leg that errored early
    covers fewer paths.
12. **Embedded doc manifests are a feature inventory.** Diff `documents[]` by `path`, `title`,
    `validated` against OLD; a `category:feature` node absent from kiro.dev's sitemap is an
    unannounced feature. Match by file stem, since public URLs are consolidated.
13. **Forging backend responses is off limits for lanes.** A fault-injection MITM against the Kiro
    backend is refused for agents even with user approval. Capture such conditions when they occur
    naturally (`KIRO_ACP_RECORD_PATH`, the `experiments/kiro-proxy-rs` logging proxy) or the user
    runs that probe personally.
14. **Lane outputs live in the repo tree.** Write `FINDINGS.md` and raw outputs under
    `experiments/conductor-spike/audit-<NEW>/<lane>/` and append as you go. The session scratchpad
    is lost on an account switch or a usage-limit cut; the 2.29.1 run lost five lane briefs and
    all intermediate outputs that way.
15. **Lanes may be refused the Write tool.** Write files with Bash heredocs
    (`cat > file <<'EOF'`), and return the full findings text in the hand-back so the orchestrator
    has it even if the file write failed.
16. **A live leg is never economised.** Re-run a leg that errored early; a short capture that
    "covers the method" has covered fewer paths than the leg that finished.
17. **Model the full surface, or explicitly ignore with a log.** For each new field the verdict
    is one of HANDLED, DROPPED (received, ignored) or NOT-SENT, each with the cyril `file:line`
    that proves it. "Harmless" without a line is a guess.
18. **Snapshot before a new host runs.** Installed `tui.js` and self-extracted `kas/<ver>-*`
    trees are replaced or pruned by the next launcher run; the archive carve is the durable copy.
19. **A changelog entry is not a release.** `feed.json` listed 2.28.1 with one feature line and the
    origin has no 2.28.1 tarball. `audit-state.sh` marks such versions "no Linux tarball on origin";
    their lines fold into the next shipped release. Run `acquire.sh` unpiped: `cmd | tail` hides its
    non-zero exit and a dependent carve then fails on a missing file.
20. **`BUILD_HASH` is a release-train stamp, not a content digest.** 2.29.0 and 2.29.1 share
    `31e6bb3f…` while every host binary differs (chat 215,572,200 → 215,787,048 B; `.text` +190,656;
    source trees changed). Decide identity by binary size+sha256 and by the readable `.d.ts`/JS
    content; never collapse two releases because `BUILD_HASH` matches.
21. **A live leg that finished "suspiciously fast" is void.** The 2.29.1 live lane's first 0.66.26
    sweep leg returned in a fraction of its estimated time and covered fewer paths; it was discarded
    and re-run. Treat a short capture as a lead to reproduce, never as a verdict.
22. **`kiro-cli version --changelog` is remote.** Since 2.28.0 the endpoint is fetched live, so an
    archived binary prints the CURRENT feed, never its own. Take per-version changelogs from
    `stable/<ver>/feed.json` and copy them into the evidence directory.
23. **"The reference client stopped sending it" ≠ "the engine rejects it."** The 2.28.0 audit
    concluded KEEP `subagentOrchestration` because the TUI no longer sent it; 0.66.33 had *removed*
    it from the `AgentSettings` schema, and the engine now logs `configuration.intake.unknown-key`.
    Before deciding keep-vs-drop, grep the bundled schema in BOTH versions and, where possible,
    confirm the intake at `KIRO_LOG_LEVEL=debug` against a control arm.

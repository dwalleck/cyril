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
24. **KAS is cyril's current path** (user, 2026-10-14). Apply the P1/P2 rules to KAS gaps and demote
    v2-only findings; run a standing **coverage** lane (Kiro's TUI × KAS × cyril) every cycle, because
    delta lanes never produce the parity list. Drop v2 live legs unless budget remains.
25. **v2 commands failing on KAS is expected.** Never spend a probe proving it. Probe the frame Kiro's
    TUI sends for the same feature and phrase the gap as "cyril lacks a KAS-native route".
26. **Look for a moved feature before calling it missing.** The 2.31.1 coverage lane reported "no way
    to change model or effort on KAS"; KAS had folded both into `configId:"model"` options carrying
    per-model `_meta.kiro.effortLevels`, set with `configId:"effortLevel"`.
27. **Cite frames in the checkable form `<capture>.jsonl#<N>"<expect>"`**, N 0-based, and run
    `check-citations.py`. A prose "LIVE frame 146" was one off (1-based count against 0-based
    captures) and pointed at a method only the probe sends. Frame numbers refer to the lane-dir
    captures, not the trimmed `*-<NEW>.jsonl` bundles (which carry a header record).
28. **`largeToolOutputHandler` is not a positive control** (KAS logs it `unknown-key`); use
    `configurationState:true`, which makes KAS push `_kiro/configuration/state`. KAS debug lines go to
    `$HOME/.kiro/logs/<ts>/kiro.log`, not stderr.
29. **Name the cyril entry point before rating a gap.** A new KAS flow reached only through a model tool
    or a gate cyril deliberately withholds is opt-in (P3), not P2. The 2.32.0 creator-as-step plan
    approval was filed P2, then corrected: ADR-0011 (since superseded by ADR-0013) kept `workflows.enabled` off, so the model never
    gets `run_workflow`, and `_kiro/workflow/new` takes no `workflowPrompt`.
30. **Spread-built objects hide from literal greps.** Kiro's TUI builds its consent echo as
    `consent:{...o?{capability:o}:{},scope:r,…}`, so `consent:{capability` matches 0. The 2.31.1 audit
    concluded "the TUI sends no consent echo" from that; it has sent one since 2.31.1. Grep the
    container key (`kiro:{consent`) and read the object.
31. **Build cyril at the HEAD the verdict is about, and re-check HEAD at fold time.** In 2.32.0 another
    session merged the fix under test (#169) 19 minutes after the cyril-on-KAS build, so that scenario's
    "blocker stands" verdict described superseded code. Record the build commit in every cyril-on-KAS row.
32. **A cloud-entitled account turns `--cloud` probes into real remote sessions.** The 2.32.0 HOST-L1/L3
    legs expected the gate to refuse and created three cloud sessions. Ask before any probe whose success
    path creates remote state, and delete that state in the same leg.
33. **Pair `session/load` legs at the same model-catalog warmth.** A warm catalog puts the full `model`
    option in the load result on both engines; a cold one omits it (0.66.34) or pins one option
    (0.66.43). An OLD/NEW pair at different warmth reads as a version change (trap 1 variant).
34. **Probe drivers answer host callbacks their own way.** Every driver replies `shell_type: "bash"`, so
    no driver capture can show a cyril-specific host-callback bug (cyril-k8wc). Give each host-callback
    lead a cyril-shaped arm, and read cyril's real replies in the cyril-on-KAS leg.
35. **Enumerate releases by probing the origin, not a bounded window.** `audit-state.sh`'s minor+2 probe missed 2.35.x–2.37.x and 3.0.0 in the
    3.1.0 audit; a later feed's embedded older entry exposed the gap. Probe every `M.m.p` and the next major until two consecutive 403 minors.
36. **A 3.x release ships one binary** (`kiro-cli` = the former chat binary); `carve-frames.py`/`carve-all.sh` fall back from `kiro-cli-chat`
    to `kiro-cli`, and host scripts must be role-aware (no launcher, no term).
37. **KAS 0.66.91+ is built with esbuild keepNames** (`a(fn,"name")` after every function): literal counts inflate without behaviour change and
    ~17k "new-only" literals in a lexer diff are function names. A NEW-only literal is evidence only when its surrounding code is new; navigate by
    the keepNames labels instead of minified names.
38. **Flag DEFAULTS move without a method or schema change.** The 3.1.0 cycle's biggest user-facing change (model fallback on) was three
    `default:!1 → !0` edits; the memory rollout flip was a string default. Run the flags extractor on EVERY carve and bisect defaults, not only keys.
39. **Hard-coded flags live at the entry point.** `workspaceTrusted:!0` sits in KAS's stdio/ws bootstrap, not where its 89 consumers read it. For
    "how does KAS learn X" grep `X:!0|X:!1` and the bootstrap's env/argv reads before reading consumers.
40. **A changelog line can be true with zero engine delta** (2.34.0 MCP elicitation, 3.1.0 `structuredContent` resolved to the TUI/SDK side).
    Check the lexer diff for the keyword before attributing a line to the engine.
41. **The system `grep` is ugrep** and rejects `.{0,N}needle.{0,N}` on 14 MB single-line bundles ("exceeds complexity limits"); use the Python
    context extractors (`static-*-ctx-<ver>.py`). Ext-notification censuses miss registrations through a variable and conditional settings rows;
    count spread-built flag objects by their container pattern (`type:"other",key:"`) and read spread emit sites (`...She(state)` adds payload keys).
42. **A copy of the real auth store may be opened only by `kiro-cli chat _ get-kas-token` WITHOUT `--force-refresh`.** `kiro-cli acp` on ANY
    store holding a token writes the token row within seconds, and the client cannot tell a rewrite from a consumed (single-use) refresh token —
    the 3.1.0 live lane raised STOP-AUTH over exactly this. Engine-routing and sign-in probes need no token: use an EMPTY store and the
    `--auth-method client` discriminator (accepted only when the resolved engine is v3). KAS labels every ordinary callback read
    `acp-peer.refreshToken.ok`; that line never proves an OIDC refresh. A 3.x binary also applies sqlite migrations to any store it opens.
43. **An arm that sets `KIRO_RUNTIME_DIR` cannot answer where the DEFAULT extraction goes.** Confirm layout claims with the env unset and an empty
    `<bin>/../lib` (the live lane's "P1: nothing under `<data dir>/kas`" fell to a zero-auth orchestrator probe). Every leg gets its own scratch
    HOME/XDG dirs: the first run of a 3.x binary creates `data.sqlite3`, lock files and `cli.json` wherever it is pointed.
44. **Runner scripts must suffix captures per arm** (same-named arm captures overwrote each other: hooks batch 2 VOID, a `turn_timing` arm lost).
    **hex1b:** a literal newline arrives as Ctrl+J (inserts `j`), `\uXXXX` escapes are sent literally, Ctrl+Q/Ctrl+C did not reach cyril — use
    `send_terminal_key Enter` and kill the process to stop.
45. **Ask lanes for ONE reconciliation row per feed line**; abridged bundled rows need hand placement (`reconcile-changelog-<ver>.py` merged 181/188).
46. **Spawn the cyril-on-KAS leg as its own lane** once the first static lane returns; a single live lane (2 h 47 m in 3.1.0) runs out of budget
    before the UI scenarios. Brief HOST-class routing probes as empty-store-only (trap 42).

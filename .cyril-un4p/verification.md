# cyril-un4p verification index

2026-10-20. Scope: the isolated un4p worktree based on8e5c325e, without ej7y scrolling or xavt shell-preview changes. Main ran the gates and owned-process smoke; this documentation/receipt step ran no tests, builds, linters, formatters or smoke commands and changed no production/Cargo source.

## Retained evidence

Portable bundle: [evidence/verification-20261020.tar.gz](evidence/verification-20261020.tar.gz).

| Bundle member | Provenance and result |
|---|---|
| `verification/parent-gates.log` | Complete retained output of parent artifact476. Focused App repair regressions:5passed; UI repair regressions:6passed. Parent reports workspace tests, all-target Clippy, build and formatting checks PASS on the exact isolated source. |
| `verification/repair-review.txt` | Portable transcription of independent ReviewApprovalPaths report: correct, confidence0.97, no new findings. Both RAP1/RAP2 resolved; scope includes guards, layout priority, App routing, regression assertions, shared helpers and approved dependency edge. Reviewer ran no checks. |
| `verification/smoke-summary.json` | Parent's artifact503 summary, historical invocation, binary digest, observed cases and explicit limitations. |
| `smoke/harness.py`, `smoke/result.json` | Unchanged executed harness and complete successful `-b` run receipt. |
| `smoke/agent-wire.jsonl`, `smoke/client-wire.jsonl` | Ten owned permission requests and ten matching selected responses, IDs7000–7009, in one Cyril session; not ten real backend turns. Only allow_once/reject_once options were offered: four accepts and six rejects. Exact feedback is in `result._meta.kiro.rejectionReason`. |
| `smoke/observations.jsonl`, numbered `.txt` captures, `smoke/final-screen.txt` | All36 decoded-screen captures, capture geometry and terminal-output byte offsets. |
| `smoke/terminal.ansi`, `smoke/cyril.log`, `smoke/kiro-cli-argv.txt` | Original PTY output, process log and owned fake-auth-helper arguments. These are provenance, not native-terminal or backend proof. |

The bundle excludes transient `control.json`, the earlier failed `-a` run, executable, private HOME/config/data trees, caches and environment dumps. Original retained bytes include nonsecret historical operator/repository/scratch paths; this is not a privacy-redacted bundle. Inspection found no live token, password, API key or remote routing endpoint in these retained artifacts.

Original and review defect-sensitivity results are recorded in [route.md](route.md) and [review-decisions.md](review-decisions.md). Compile failures and the original wrong-geometry setup were not counted as behavioral RED.

## Successful owned-process observations

The tested executable was `/tmp/cyril-un4p-tested-20261020`, SHA-256:

`e2a8fb92972b144e3101bcc571dbbf817277527bd289769215c96cdb7013b2a2`

- At80×11, two-inner-row prompts retain the long target's fitted path plus the selected Allow choice or exact reason `why`, ahead of Limit/cosmetic content.
- Overflow is refused after4096characters. The notice was actually observed before clamping; after clamping it cannot displace path/editor. The eventual rejection carries exactly4096 `x` characters.
- Fully drawn `/tmp/report…` and `a` work in both ordinary and reason confirmation.
- At10×11, insufficient emoji-style target context leaves Enter unanswered. Ordered Enter/Backspace/`!` edits the same pending reason to `wh!`; enlarging then submits exactly `wh!`.
- Pathless border-only option/reason Enter is ignored. Restoring room preserves the same pending request and edited reason before the eventual exact answer.
- Chat draft `D` remains before, during and after the ten permission exchanges. No file operation, command, filesystem/terminal callback or durable grant was executed by the owned peer.

This exercises the real Cyril process, production permission conversion, geometry guards and manual response wire against an owned KAS-shaped ACP peer. **It is not real KAS/backend/account qualification and not native-terminal Unicode glyph/cell proof.** The peer's version0.66.43 and Kiro branding are fixture facts. Pyte's decoded emoji-style target is not a glyph-width oracle; Unicode cell boundaries are covered separately by the parent-passing App/TestBackend regressions in artifact476. Raw ANSI is output only, not a timed input/resize recording.

The earlier `-a` run failed its harness observation: it expected `why!` plus cursor within four editor cells, although the cursor legitimately wrapped. The corrected `-b` harness uses FIFO Enter/Backspace/`!` and observes `wh!` plus cursor, retaining the no-response fence and exact eventual response. No production repair occurred between those runs; the `-a` result is not product RED.

## Relocating the unchanged harness

After extracting the bundle, an **unexecuted relocation template** is:

```sh
uv run --script "$EXTRACTED/smoke/harness.py" \
  --binary "$CYRIL_BIN" \
  --expected-sha256 e2a8fb92972b144e3101bcc571dbbf817277527bd289769215c96cdb7013b2a2 \
  --repository "$CYRIL_REPO" \
  --output "$NEW_OWNED_OUTPUT"
```

Provide the recorded executable or explicitly record a fresh qualification/digest for a different binary; the binary is not archived. The repository must contain `crates/cyril-core/tests/fixtures/fake-kiro-cli.sh`. Output must be a new, nonexistent directory. Requirements include Unix PTY/fcntl/termios/signals, `/bin/bash`, Python with `hashlib.file_digest`, Node on PATH and pyte. The PEP723 pyte dependency is unpinned; this is relocatable, not hermetically reproducible or Windows qualification. The harness creates owned fake auth/peer and private HOME/config/data/workspace, but inherits other environment variables; it is not a comprehensive sandbox.

## Publication boundary

README feedback guidance is updated after the successful smoke, including context guards, path priority, resize/draft retention and fully visible literal-ellipsis/single-grapheme targets. Main still owns commit, issue-specific PR, required publication/CI checks and tracker closure. Do not include `.cyril-un4p/delivery-patches/` or child ej7y/xavt files in this issue's publication manifest.

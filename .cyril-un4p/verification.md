# cyril-un4p verification index

2026-10-21. Scope: PR188's isolated un4p delivery worktree, without ej7y scrolling or xavt shell-preview changes. Main's qualifier/review/owned-consumer records supply execution evidence; this publication step ran no tests, builds, linters, formatters, product runtime or Git/PR/tracker operations and changed no source.

**Current delivered repairs:** [Current PR188 repair qualification](#current-pr188-repair-qualification-2026-10-21) is the active evidence index. Original artifact476/503 and the released-renderer bundle below remain historical records tied to their original source/binary identities. Scoped Un4 PASS is not final cross-batch integration, current-head CI, native or authentication qualification.

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

## Released-renderer refresh 2026-10-20

**Historical released-renderer qualification, superseded as current-source evidence by the repair qualification below.** Portable refresh bundle: [evidence/released-width-refresh-20261020.tar.gz](evidence/released-width-refresh-20261020.tar.gz). [Its immutable JSON publication index](evidence/released-width-refresh-20261020.json) records the original digest, member inventory/hashes, publication allowlist and source-preservation binding. Both it and `verification-20261020.tar.gz` remain unchanged. The omissions and next-step language in this historical section describe that run only; they are not the current repaired qualification status.

### Tested source, not the stale index

The actual assembled source tree was `b4c644051b65b500053dea089a1206d93220d43b` with ratatui0.30.0/core0.1.2/widgets0.3.2. This is a **tree identity, not a commit HEAD**. Recorded HEAD was `2fd94879db1b4281dc85cf7dcf528649c1da3087`, MERGE_HEAD `3a5b88b66b1267844ea764c4ff58e48294c93141`, and parent-staged preformat tree `b40b28053db4a90ce3c76d7bdf194c48aa9d5088`. Initial formatting changed only approval.rs; the checked postformat tree stayed unchanged through the requested gates. Main must publish the actual formatted source, not the stale staged approval.rs. These documentation/evidence additions do not change the qualified runtime source; the bundle records its independent source-preservation pin.

Canonical qualifier receipt SHA-256: `50d78b6d7280fe360a226f25b955bd158290149b0b92395921067ed3e4ff8ad5`. Final qualified source-manifest SHA-256: `791290c07adc5687764cf88c6bebe1007a62366372d7ffbb610f979ebe4b3c11`.

| Bundle member | Current executed evidence |
|---|---|
| `qualifier/canonical-handoff.json`, `running-command-receipts.json` | Original qualifier receipts, exact commands, source identities, timing/configuration and explicit omissions. Paths in raw receipts are original provenance; archive notes map portable members. |
| `qualifier/01-initial-canonical-format.log` | `cargo fmt --all` PASS; only approval.rs formatted before the tested source pin. |
| `qualifier/02-full-affected-UI-and-App-package-tests.log` | `cargo test -p cyril-ui -p cyril`:1263passed/0failed/3ignored across15 Cargo targets; cyril351/0/1, cyril-ui912/0/2. Includes inherited App guards and literal physical path/workflow regressions. |
| `qualifier/03-workspace-all-targets-clippy.log` | `cargo clippy --workspace --all-targets -- -D warnings` PASS, not core-test execution. |
| `qualifier/04-default-binary-build-bins.log`, `05-final-format-check.log` | `cargo build -p cyril --bins` and `cargo fmt --all --check` PASS. |
| `qualifier/final-actual-assembled-after-requested-local-gates.*`, `private-binary-provenance.json` | Final source pin/manifest, versus-HEAD and versus-parent-staged patches, plus matching preserved private executable provenance; private Git indexes and executable are not bundled. |
| `consumer/adaptation.json`, `consumer/evidence-manifest.json` | Exact current command/CLI retargeting and accepted47-file inventory. Manifest SHA-256 `d32d9f2d233479cb9bd5e3144fba525db7c5d36751d16ebe89699cacb8e9a075`; original bytes retained. Its `source_head` field labels the assembled tree above, not a commit. |
| `consumer/smoke/harness.py`, `consumer/run/harness.py` | Byte-identical historical U10 harness, SHA-256 `f6d95550e79bc5492a7e362b3085c8de2b10a3c4bbb9485ebccd72ea927be943`; no oracle/source adaptation. |
| `consumer/run/result.json` | Sole attempt PASS, exit0,14.87seconds, no retries/normalization;10requests and10selected responses, four accepts/six rejects. SHA-256 `b92ef636ba28e06ff668573b549d021f6f4680ae88b4ac61527472dfa7e628f0`. |
| `consumer/run/agent-wire.jsonl`, `client-wire.jsonl`, `observations.jsonl`, terminal/log/helper-argument files and36 screen captures | Original raw outcomes: draft D and exact reasons preserved, full4096-character overflow response, literal targets, ignored clipped/border-only Enter and same-request recovery. No external-operation callback observed. The complete47-file inventory includes final owned fixture control state, not a timed input replay. |
| `ReviewCurrentUn4.txt`, `review.json` | Original independent actual-source review plus portable scope transcription: no findings, confidence0.94; static review only. |
| `source-preservation.json`, `archive-notes.txt`, `members.json` | Publication/source boundaries, portability instructions and every other archive member's size/hash. |

The preserved private binary is `/tmp/cyril-un4p-width-current-tested-20261020`, SHA-256 `df8c6bf50a262ba361c86682f7642eadc27d42730c5d03ab3947a13bfa6e3077`. Exactly one existing owned U10 harness was run against it using the unchanged CLI. The complete executed command is in `consumer/adaptation.json`; no second consumer was run. After extraction, an **unexecuted** relocation template is:

```sh
uv run --script "$EXTRACTED/consumer/smoke/harness.py" \
  --binary "$CYRIL_BIN" \
  --expected-sha256 df8c6bf50a262ba361c86682f7642eadc27d42730c5d03ab3947a13bfa6e3077 \
  --repository "$CYRIL_REPO" \
  --output "$NEW_OWNED_OUTPUT"
```

The same Unix/Node/Python/pyte/owned-auth-fixture prerequisites above apply; binary is supplied separately and output must be fresh. Raw receipts retain original paths and the historical artifact476 limitation citation; that citation is not new-source qualification. The refreshed consumer proves actual-process permission conversion/manual wire and decoded ASCII/text geometry, **not real KAS/backend/account/auth or native Unicode-cell fidelity**. Physical halfwidth/combining boundaries are separate literal Buffer/diff tests in the current UI+App gate.

**Limits and landing obligations:** This refresh did not execute local full-workspace tests, core unit/integration tests, new mutation/test drivers, all-feature/platform or native/auth acceptance. Workspace Clippy does not substitute for those lanes. **Main-target CI remains mandatory, and inherited auth6c remains unavailable here and unwaived.** The user working-bootstrap prerequisite is satisfied; native execution still requires a newly qualified MCP binary and separate bounded authorization. Main owns staging the actual formatted source plus the five-file publication allowlist, commit/push/CI and subsequent child GO. Ambient delivery-patches, historical proof and #190/#191 are excluded and unchanged.

## Current PR188 repair qualification 2026-10-21

Portable current bundle: [evidence/current-repairs-20261021.tar.gz](evidence/current-repairs-20261021.tar.gz). [The current publication index](evidence/current-repairs-20261021.json) owns the explicit member/source allowlists, immutable receipt hashes, source/binary provenance, documentation postimages, exclusions and remaining gates. It references each canonical record's result fields rather than maintaining another mutable count table. Old archives/indexes and old first-smoke bytes are preserved.

### Delivered behavior and checked state

The [approved repair record](review-decisions.md#pr188-identifying-row-allocation-review--2026-10-20) owns R188P1/R188P2, the repeated-repair stop and R188C1 disposition. Both pages keep an actually identifying preferred fit or reuse an identifying available alternative before chrome, including a smaller row budget when whole-path wrapping becomes physically invalid. They preserve the same strict painted-context guard, selection/editor, whole-target/actual-elision metadata, physical widths, borrowed fitting and impossible-space/pathless/inline fallbacks.

The shared tracing correction relocates the established persistent non-capturing interest subscriber from App to `test_support::tracing_capture_lock`; it does not change the production warning, thread-local JSON/fmt captures, first-installer policy or behavioral assertions. The known empty-capture occurrence remains a historical failure, not a newly proven callsite-cache causal diagnosis.

The publication index binds the current **bounded workspace-input descriptor**, its before/after manifest and preserved private binary. This is not a whole-Git-tree or new commit identity. The current qualifier proves those bounded inputs and frozen regression frames stayed unchanged through its gates; publication edits only these workflow documents and the new archive/index. The private executable, source snapshots, Git indexes and real HOME/config/auth data are not bundled.

### Canonical executed evidence — scoped PASS

Archive member names below are relative to the current bundle. Raw receipts retain their original absolute provenance paths and chronological statuses; later records discharge only the steps they actually executed.

| Owning canonical record | Current conclusion and evidence |
|---|---|
| `qualifier/canonical-handoff.json` and `qualifier/running-command-receipts.json` | Current local gates **PASS**: warning positive/negative controls alone and with the uncaptured marker under default/serial scheduling; App WARN/DEBUG controls; frozen priority/nonmonotone regressions; full affected Core/UI/App including integrations/doc targets; standalone library-default non-KAS Core; workspace all-target Clippy, default bins and final format check. Exact commands, exits, overlapping lane totals and exclusions live in this canonical only. Selected affected packages are not a full-workspace test claim; ignored tests remain ignored. |
| `qualifier/retained-evidence-applicability.json`, `retained/priority-red/`, `retained/nonmonotone-red/`, `retained/warning-capture-failure/` | Actual priority and nonmonotone RED remain attached to their original source revisions and exact assertion reach. The warning-capture failure remains FAIL at its pre-capture-repair state. Current qualification/review establishes retained applicability; no new mutations, retries or causal A/B proof are invented. |
| `review/current-repair-review.json` and `review/capture-safety-review.md` | Independent actual-source repair/capture review **PASS** with no findings. Scope includes preferred/alternative fitted-result retention, both consumers, shared initialization, no global capture and unchanged regression/dispatch assumptions. Qualifier evidence is separate from static review. |
| `consumer/completion-repair.json`, `completion-review.json`, `completion-only.diff` | Approved technical smoke correction and independent bounded review: the existing helper validates the independent exact response while narrow, then optionally restores ordinary geometry before its unchanged contiguous visual completion wait. All new priority/nonmonotone modes opt in; original U10, narrow pre-action assertions, wire parser/decoder and cleanup remain unchanged. |
| `consumer/canonical-result.json`, `runtime-raw-artifacts-manifest.json`, `run/` | The completed **15-case owned consumer PASS** covers original U10 plus priority option/reason/notice and nonmonotone option/reason. Exact request/response IDs, singleton option/reasons, draft retention, refusal/recovery, final callback/request gates and complete raw capture inventory live in this canonical. Raw wire, observations, ANSI, runner and every manifest-listed run file are included unchanged. It is actual-process/manual permission-wire proof, not real KAS/backend/account/auth or native Unicode-cell qualification. |
| `consumer/execution-host-receipt.json`, `cleanup-final-observation.json` | Harness success, owned process/session absence and scratch removal were observed. Exact Cyril exit status and signal-fallback choice were **not instrumented**; no App exit code or complete historical descendant census is inferred. |
| `retained/first-consumer-failure/` | The original separately authorized exit1 smoke remains FAIL, with its original receipt, selected raw failure/ANSI/wire evidence and manifest. An impossible narrow contiguous-marker postcondition was technically corrected; actual underscore loss remains genuine known Tqr main-chat gutter loss. It is not erased by the current scoped approval PASS. |

The executed runner and authorization are archived as evidence, **not authorization to rerun**. Runtime reproduction requires Main's fresh exact binary/source binding, owned output and bounded GO; no new runner, decoder or generic evidence framework is introduced here.

### Remaining unwaived gates and publication boundary

- **PENDING — Main/Tqr:** repair/integrate the known main-chat gutter loss and qualify final assembled behavior. Current new cases check narrow approval action/wire before ordinary-width acknowledgement; they do **not** prove narrow main-chat retention. This is a separate still-blocking final cross-batch integration obligation, not an approved risk.
- **PENDING — Main:** stage the three exact qualified repair source files plus the publication allowlist in the current JSON index, commit/push/update PR188 and obtain **current-head CI**. Existing PR188 discussion threads remain unresolved until that exact publication. This packaging phase performs none of those operations.
- **PENDING — Main/RefreshApprovalWidthStack:** authorize descendants only after the exact published-parent binding and required integration conditions; no #190/#191 descendant edit or qualification is included.
- Required platform/all-feature/native/auth acceptance is not supplied by these Linux local gates or the owned peer. Inherited auth6c remains unavailable and **unwaived**. No bootstrap/login/account/network/native retry or acceptance claim was made.


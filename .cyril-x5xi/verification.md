# cyril-x5xi Gilfoyle verification index

This is the owning evidence index for the x5xi-only delivery on published fiv1
base `7eefe275fbec97f610677b2ee6433af996806599` ([PR #184](https://github.com/dwalleck/cyril/pull/184)).
It records Main's executed checks and the owned actual-App consumer; the
documentation worker ran no Cargo, test, formatter, benchmark or smoke command.
Publication and tracker updates remain Main's responsibility.

## Checked delivery identity

The checked state is the base above plus the three uncommitted production files
below, not a fabricated implementation commit. `source/source-state.json` in the
portable archive binds these bytes, the earlier sensitivity boundary and the
actual smoke executable. README and workflow/evidence edits made after the
checks do not change the checked production paths.

| File | SHA-256 |
| --- | --- |
| `crates/cyril-ui/src/theme.rs` | `fefab546a9d63cf300d70c986f48ed485ddf99c1cf7f240b13e649cb58215eaf` |
| `crates/cyril-ui/src/highlight.rs` | `f4142b29e3561b392a08e65eb5ca5827735e671577510f4d46a41a041f29fdc9` |
| `crates/cyril-ui/src/widgets/markdown.rs` | `37719f3bf96799c5240660db27607ab43582ffce098bc21c5242bc4794b2ff2a` |

Actual smoke executable SHA-256:
`0d20a4b1384c489cfee65517d7d861acd945e13b4fe03a245b2eae70f7ef8644`.
The executable itself is deliberately not published.

## Obligation ledger

| Obligation | Result and portable receipt |
| --- | --- |
| Assembled workspace regression | **PASS** — Main's artifact 555: 2,836 passed, 0 failed, 13 existing ignored; the ignored cases are not claimed as exercised. `gates/workspace-555.log` retains every test result and checked-checkout compilation paths. |
| Workspace/all-targets clippy, executable build and formatting | **PASS** — Main's artifact 555 and command/source receipt in `gates/workspace-555-receipt.json`. No benchmark or new latency target is inferred. |
| Structural dependency closure, projections and narrow-key regression | **PASS** — current workspace tests plus independent review; C1–C3 in [design.md](design.md). The earlier executed defect sensitivity is scoped separately below. |
| F1 exact approved MarkdownStyle import / forbidden ThemeId access | **PASS** — Main's corrected bg100 actual-Cargo A/B: identical approved widget source BEFORE **1 executed FAIL**, AFTER **1 executed PASS**; forbidden ThemeId source BEFORE and AFTER **1 executed FAIL each**. `gates/f1-bg100-receipt.json`, the full background-delivery transcript and the executed script retain inputs, checker hashes and actual assertions. No invented artifact ID is assigned. |
| Independent substantive/repair review | **PASS** — ReviewThemeIdentity's final correct verdict, confidence 0.99; the portable review receipt records its actual scope and conclusion. Documentation-only packaging retains that review because renderer behavior, ownership and interfaces are unchanged. |
| Actual App Markdown/theme/resize/scroll/live-append consumer | **PASS** — bounded owned v2 peer through the tested executable and PTY. `consumer/x5xi-qualification.json` and all three qualified run directories contain actual ANSI, decoded cells, text, wire, control/ack, result, safe configuration and exact executed harness bytes. |
| Permanent documentation | **Delivered after consumer PASS** — README's **Themes and Markdown rendering** section documents projected used-style identity and unchanged preview/cancel/commit behavior. No performance budget is invented. |
| Windows CI baseline | **N/A — requester-approved baseline exception**, not platform-green: the inherited cyril-u62f memory-readiness failures and cancelled coverage are recorded in `limits/windows-baseline-exception.md` and [the public acceptance record](https://github.com/dwalleck/cyril/pull/183#issuecomment-6284441811). New/differently attributed failures remain blocking. |
| Commit, issue PR, CI adjudication and tracker closure | **PENDING — Main publication**. This index is a publication handoff, not a claim that those actions happened. |

### Earlier defect sensitivity is not relabeled current-source execution

The retained six-probe run used original base
`8e5c325ebb5dac3a4ad89f71838fd198cc177543`, frozen x5xi patch SHA-256
`9ac057ccb15ee7e809330f41bb3abf39259018ea61a8066950635eb02bd93585` and a
separate zj0m patch. Baseline: 2 passed / 4 intended assertion failures;
patched: 6 passed. Only the irrelevant-role eviction failure and the
syntax/fallback visible controls supply x5xi sensitivity. The other failures
and work-construction counts belong to zj0m, not x5xi. `sensitivity/` retains
the original receipt, logs and source manifests with their original identities.
The temporary probes were not ported, and their source-fence limitation is
retained. Current uninstrumented workspace/source-gate qualification is distinct.

Main confirmed that the current tree applied only the pinned three-file x5xi
patch without conflict onto published fiv1, followed by rustfmt; the fiv1/1v25
prerequisites do not touch these three paths. The measured cache mechanism is
unchanged, so the original sensitivity remains valid historical evidence under
the workflow's evidence-validity rule. This is not literal instrumented-byte
identity: fresh artifact 555 and the actual consumer independently qualify the
assembled delivery.

## Actual consumer outcomes and boundaries

The three qualified startups are `x5xi-truecolor-03`, `x5xi-none-01` and
`x5xi-truecolor-return-01`. Each records 14 screen/cell checkpoints and 11
bidirectional ACP frames. Committed and separately live code have actual
syntax-color baselines; Gruvbox preview recolors both code backgrounds, Esc
restores their previous styles, Enter retains the preview and returning Cyril
Dark restores the original styles. The no-color startup retains default code
foreground/background through the same picker actions. The scene also resizes
160×48 → 80×24 → 160×48, visits oldest/middle/follow windows and visibly appends
`LIVE_APPEND_CONTROL` before completion.

These are real Cyril App/CLI-process, bridge, picker, terminal-event and output
observations. They are not a live Kiro/model turn or native graphical-terminal
font/glyph/emoji/CJK/combining-width qualification. Pyte is the ANSI/cell decoder,
not a native font renderer. Color mode is startup-only: three processes do not
prove same-process truecolor → none → truecolor invalidation. The configured
history maximum is 1,000 (default 500), with one long committed Markdown source
and a separate live source; neither native 50,000-message scaling nor a timing,
work-counter, giant-row or later zj0m/c6la acceptance claim follows.

The owned peer uses no external account/model, real file operation, host
filesystem/terminal callback or permission grant. Published `safety.json` files
contain only the runner's whitelisted isolated environment, not the user's
ambient private environment. No isolated HOME/XDG/workspace tree, executable,
cache or credential store is packaged.

Two preliminary smoke attempts were orchestration failures, not product RED:
01 awaited a wrongly capitalized picker label; 02 assumed an oversized browse
back-offset would normalize itself. Both diagnoses and the corrected successful
run identities are retained in `limits/orchestration.json`. The failed first F1
A/B attempt compiled no tests because its disposable-copy filter removed existing
`.cyril-a5wo` include fixtures. It has **NO TEST VERDICT**, not a product RED or
GREEN. Main corrected only the copy filter; entrypoint touches still force
fixture-local compilation against the shared target.

## Portable package and reproduction

The publication package is `.cyril-x5xi/verification-evidence.tar.gz`; its archive
hash and exact publication file list are recorded by `handoff.json` and
`publication-manifest.json`. The archive's `index.json` maps every retained
member to original provenance and SHA-256, with archive-relative claim/receipt
paths and mappings for the raw sensitivity receipt's original `local://` links.
No ephemeral OMP artifact or `/tmp` file is needed to inspect a conclusion.

The qualified run directories retain the byte-identical executed
`harness-snapshot/smoke.py` and adjacent `owned_peer.py` (`pyte==0.8.2` via uv).
For a deliberate reproduction, extract one snapshot directory, create a fresh
x5xi-scoped authorization JSON naming a separately qualified executable with
its actual hash and source/test receipt, then run:

```sh
uv run --script smoke.py --authorization /absolute/path/to/x5xi-go.json --scope x5xi --color-mode truecolor --output /absolute/path/to/fresh-output
```

Use `--color-mode none` for the no-color startup. The archived authorization is
historical provenance, not authorization for another source or issue. The
executed harness contains an unused zj0m entry point; it is retained solely to
preserve exact executed harness identity, not as a qualified or published batch
plan. Later mutable scratch harnesses, c6la preparation, unexecuted batch plans,
private environment trees and all binaries are excluded.

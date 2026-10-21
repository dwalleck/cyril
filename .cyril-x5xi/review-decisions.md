# cyril-x5xi review decisions

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | The required explicit-theme source gate rejects the DG-approved concrete MarkdownStyle import; allow that exact seam without allowing widget-side resolution. Mechanism: exact allowed-import recognition. | ReviewThemeIdentity | Verified | Current-source gate A/B executed by Main: identical approved widget BEFORE 1-test FAIL / AFTER 1-test PASS; forbidden ThemeId widget BEFORE and AFTER 1-test FAIL. Portable gates/f1-bg100-receipt.json and full transcript; verification.md. | Accept | Add only `.replace("use crate::theme::{MarkdownStyle, Theme};", "")` to `widgets_only_use_the_explicit_theme`. | blocking — resolved; exact repair, forbidden-access sensitivity, assembled qualification and independent source-only re-review complete. Publication remains Main-owned. |
| F2 | The publication manifest's README digest does not match the published README; update that digest after the final documentation changes. Mechanism: publication-metadata drift. | [chatgpt-codex-connector[bot], PR189](https://github.com/dwalleck/cyril/pull/189#discussion_r4328339247) | Verified | At reviewed head `7d3a03be7d8cf9d467c4cd4fe4d2f334f900c358`, direct SHA-256 comparison matched 11/12 manifest payloads; README alone differed. Historical/current/archived attribution and the deterministic check are below. | Accept | Correct the current README binding, hash this amended decision record in the manifest, and update the root handoff's manifest digest. | blocking — repaired; ReviewX5ManifestRepair's scoped independent review complete: correct, confidence 0.98, no findings. Main's publication and CI adjudication remain pending before merge. No product or qualification change. |

## F1 bounded technical-proof repair

- **Governing obligation / owning slice:** design.md's approved concrete MarkdownStyle dependency input, plan.md's cache-identity slice, and the existing repository requirement that widgets receive explicit theme inputs. This is a coverage-preserving allowed-seam correction within DG approval, not a product behavior or architecture change.
- **Delivered change:** only the exact approved import string is newly removed before the existing assertions. All five prohibitions (`crate::theme`, `theme::`, `ThemeId`, `ColorMode`, `resolve(`), all 17 source includes, the `<=17` cap, the on-disk coverage comparison, and the 300,000-byte cap are unchanged. The MCP branch's separately authorized 18-file integration is outside this branch.
- **Performed checks and results:** Main ran `env -u CARGO_TARGET_DIR python3 .cyril-x5xi/gate-review-f1.py --target-dir /home/dwalleck/repos/cyril/target` on the exact assembled source. Corrected bg100 executed the real `theme::tests::widgets_only_use_the_explicit_theme -- --exact --nocapture` once in each of four cases: approved-import BEFORE FAIL (0 passed/1 failed), AFTER PASS (1 passed/0 failed); forbidden ThemeId source BEFORE and AFTER FAIL (0 passed/1 failed each). The approved widget bytes were identical across checkers. The BEFORE/AFTER checker differed only by the exact allowed-import line. Every run compiled fixture-local core/UI source after entrypoint touches; no compiler failure or zero-test success counts. `gates/f1-bg100-transcript.txt` in the portable archive is the complete original background-delivery transcript, identified as bg100 rather than assigned an invented artifact ID. Its receipt retains both widget and checker hashes.
- **Assembled qualification:** Main's artifact 555 passed the current uninstrumented workspace (2,836 passed, 0 failed, 13 existing ignored), workspace/all-targets clippy with `-D warnings`, executable build and formatting. The exact source/base/command/raw output and actual-App smoke executable identity are owned by verification.md. A/B touched disposable copies only, so it does not invalidate those production results.
- **Orchestration correction, not product RED:** bg99 compiled no tests because its over-broad `.cyril-*` copy exclusion removed existing `.cyril-a5wo` include fixtures. All four cases had **NO TEST VERDICT**. Main corrected only the script's exclusion to `.cyril-x5xi`, preserving fixture dependencies and the shared-target entrypoint touches; the documentation worker did not overwrite that script.
- **Retained independent evidence:** original-base cache sensitivity remains separately scoped in verification.md; it does not establish the source gate. Main confirmed that only the frozen three-file x5xi patch plus rustfmt was applied to published fiv1, with the measured cache mechanism unchanged. The real App/CLI-process owned-peer smoke and README used-style guidance are complete; native graphical font/glyph, live Kiro/model, same-process mode-switch and 50,000-message claims are not made.
- **Independent repair re-review:** `agent://ReviewThemeIdentity` final response: **correct**, confidence **0.99**, no new findings. The reviewer confirmed the exact allowance, unchanged implicit-resolution prohibitions, 17-file/300,000-byte bounds, Theme-to-MarkdownStyle projection, derived full style identity and nested HighlightStyle forwarding. This is source-only review, not an assertion that the reviewer ran checks. The portable archive retains its final response. Current assembly only applied the reviewed patch plus formatting; post-proof README/workflow/evidence edits do not change the reviewed production behavior, ownership or interfaces.
- **Unresolved failures:** none for F1 or the local x5xi verification handoff. The requester-accepted inherited Windows baseline limit remains explicit in verification.md and is not a platform-green claim. Main still owns commit, issue-specific PR, CI adjudication and tracker closure; this record does not claim those actions occurred. Later zj0m/c6la work needs its own qualification.

## F2 bounded publication-metadata repair

- **Governing obligation / owning slice:** the existing publication manifest must bind the files it names. This is a documentation/integrity correction to the original x5xi delivery, not a change to the approved cache-identity slice or its qualification.
- **Verified bug and historical boundary:** before repair, all 12 `content_sha256` entries were compared with their named worktree files: 11 matched; `README.md` expected `8c2fe73a81a493692fb08648bbe1f360580e44e36ee89b9d4082418aab7d643a`, but the current 22,962-byte file hashed to `0aa4dc7f1d1f826c3cfd7a1a18508d0d4d236240898bf0ee6ae9f18999eb170e`. The old value correctly identifies the 22,692-byte README at pre-refresh head `194ec404a325843a1b002940784dda7296ef355c`; it does not identify today's publication file. Neither evidence archive contains README bytes. The refresh archive's `gates/x5xi/qualified-source.source.json` records the current README hash and Git blob `d91cd943e19d1c3d0ec871d43e923eefbde4ad77`; refreshed head `aca1aa503a5183c1183ec3bb62b3d61f0bdb8b77` has those same README bytes.
- **Delivered change / exact allowlist:** only `.cyril-x5xi/review-decisions.md`, `.cyril-x5xi/publication-manifest.json`, and `.cyril-x5xi/handoff.json`. The manifest's README entry now binds the existing current README; its review-record entry and the handoff's manifest digest are the two necessary dependent bindings. No README, production source, Cargo, tracker, source pin, executable identity, gate result or qualification limit is changed; no blanket regeneration was performed.
- **Immutable evidence:** both archives, the entire `refresh-pr189` package, `production.patch`, and all F1 evidence remain byte-unchanged. The refresh records' original-manifest SHA-256 `951ef2e3d998363349e726057b4f25581eb786ef01db30f80ac707b490d63155` continues to identify the original manifest snapshot, recoverable at `194ec404a325843a1b002940784dda7296ef355c:.cyril-x5xi/publication-manifest.json`, not the metadata amended by F2. Their statements that old files were unchanged describe refresh packaging at its recorded source pins; they are not claims that the later F2 repair did not occur. Those historical references must not be rewritten to the new manifest digest or relabeled as fresh qualification.
- **Performed checks and results:** content hashing and JSON/archive inspection only. The pre-repair comparison above demonstrated the defect. The deterministic command below checks all current payload digests and the current handoff binding, preserves both whole-archive identities, and distinguishes historical README/manifest bytes from the archived refreshed README attribution. Post-repair result: **PASS — 12/12 payloads, handoff binding, both immutable archives, and historical/current README attribution**. No Cargo, build, test, lint, formatter, benchmark, consumer, commit, push or tracker command was run for F2.
- **Review and unresolved obligations:** `agent://ReviewX5ManifestRepair` independently reviewed the exact three-file repair's current README/review-record bindings, handoff-to-manifest digest chain, absence of circular hashing, and preservation of historical source/archive attribution: **correct**, confidence **0.98**, no findings. This was a read-only semantic review relying on the supplied passing deterministic validation, not an independent rerun or new qualification. F1's reviewed production behavior and all historical proof retain their original attribution. Main subsequently committed the original repair locally as `10502913`, not pushed; the worker-action statement above describes the implementer's scoped hashing work. This receipt-only update and its dependent digests do not change the reviewed repair's behavior or scope. **PENDING — Main:** amend the unpublished repair commit, perform scoped publication and adjudicate required PR CI before merge. No new runtime acceptance or platform-green conclusion follows from these integrity checks.

Run from the repository root:

```sh
python3 - <<'PY'
import hashlib
import json
import pathlib
import subprocess
import tarfile

root = pathlib.Path(".cyril-x5xi")
sha = lambda data: hashlib.sha256(data).hexdigest()
manifest = json.loads((root / "publication-manifest.json").read_bytes())
for path, expected in manifest["content_sha256"].items():
    assert sha(pathlib.Path(path).read_bytes()) == expected, path
handoff = json.loads((root / "handoff.json").read_bytes())
assert handoff["publication_manifest_sha256"] == sha((root / "publication-manifest.json").read_bytes())
for path, expected in handoff["production_files_sha256"].items():
    assert manifest["content_sha256"][path] == expected, path
assert sha((root / "verification-evidence.tar.gz").read_bytes()) == "afde35dcf77edf4fbb8d3d2a78417692be9380cd842700ba95db28e9180b155f"
refresh = root / "refresh-pr189/verification-evidence.tar.gz"
assert sha(refresh.read_bytes()) == "bef26e3fd3e323503d2e65e4ceec0b1a68870bd9e6c565f898446cc5224c4689"
old_head = "194ec404a325843a1b002940784dda7296ef355c"
old_file = lambda path: subprocess.check_output(["git", "show", old_head + ":" + path])
assert sha(old_file("README.md")) == "8c2fe73a81a493692fb08648bbe1f360580e44e36ee89b9d4082418aab7d643a"
old_manifest = sha(old_file(".cyril-x5xi/publication-manifest.json"))
assert old_manifest == "951ef2e3d998363349e726057b4f25581eb786ef01db30f80ac707b490d63155"
with tarfile.open(refresh, "r:gz") as archive:
    source = json.load(archive.extractfile("gates/x5xi/qualified-source.source.json"))
    readme = next(row for row in source["source_files"] if row["path"] == "README.md")
    boundary = json.load(archive.extractfile("source/refresh-boundary.json"))
assert readme["git_blob"] == "d91cd943e19d1c3d0ec871d43e923eefbde4ad77"
assert readme["sha256"] == sha(pathlib.Path("README.md").read_bytes()) == "0aa4dc7f1d1f826c3cfd7a1a18508d0d4d236240898bf0ee6ae9f18999eb170e"
assert boundary["retained_old_publication"]["manifest_sha256"] == old_manifest
print(f"PASS: {len(manifest['content_sha256'])}/12 payloads, handoff binding, both immutable archives, and historical/current README attribution")
PY
```

# Evidence: cyril-s2hb

Source baseline: cc5eba260572e08fd20a58c8b0cdd1aaec782484.

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | Existing Python gather/facts contract | Do an actual scoped Git diff, independent helper/caller/doc expectations, fixed oracle clock and repeat invocation agree with emitted files and stdout on Linux and Windows? | PASS — both native hosts, 2026-09-30 |
| P2 | Python host text and subprocess encoding | Does Python apply host text newline translation, including already-native child line endings, while binary Git patches remain untouched? | PASS — both native hosts; Windows double-CR learning below |
| P3 | Existing diagnostics subprocess behavior | Does the oracle preserve stdout+separator+stderr, replacement decoding, >pipe-buffer output, timeout partial output and cannot-start exit 2? | PASS — literal-output process fixtures on both hosts |
| P4 | Safe Rust standard process seam | Can std::process::Command preserve a canonical executable's quoted raw argument tail on Windows without a shell, and capture output bytes without interpretation? | PASS — standalone std probe on both native hosts; not a proof of arbitrary executable-name parsing |
| P5 | Future native feature parity and version fence | No native crtool implementation exists yet; literal controlled-input parity and stamp refusal are required feature behavior, not an existing-system premise. | N/A — implementation/oracle and mutation gates belong to checkpointed-build |
| P6 | Future cancellation and resource budgets | The selected cancellation deadline and clock injection seam are behavior/design decisions; a new implementation cannot be empirically validated before it exists. Existing Python timeout behavior is covered by P3. | N/A — design falsifiers and checkpointed-build must verify the future behavior |

## Data

- Source: generated production-shaped local fixtures, not operator repositories or account data.
- Shape: real Git repository with committed baseline, changed Rust helper/caller and an out-of-scope changed document; native child processes emit exact argv, invalid UTF-8, large independent stdout/stderr streams, nonzero exit, or pre-timeout output.
- Safety: TemporaryDirectory owns fixture files; original cwd restored before removal. Children are fixture-owned. No model prompts, production repository mutation, credentials, or unrelated process termination.

## Probe

- probe_python_contract.py imports the existing experiment oracle without changing its source. Its oracle-only datetime/time adapter supplies explicit common clock samples, not native production overrides.
- probe_std_process.rs is a standalone std-library instrument, not feature code; it will not be promoted into production.
- Linux oracle command: `python3 .cyril-s2hb/probe_python_contract.py .kiro/code-review/crtool.py` — exit 0, seven PASS receipts, Python 3.14.7.
- Linux std command: `rustc --edition=2024 .cyril-s2hb/probe_std_process.rs -o <owned-temp>/probe-std`, then `<owned-temp>/probe-std /usr/bin/python3` — exit 0; argv and dual-pipe/raw-byte assertions passed. The temporary binary was removed with its owned temporary directory.
- Windows command: `uv run ~/.config/resourcefs/windows-vm/winrm_exec.py <PowerShell text retained in probe_windows.ps1>`. The script uses native Python 3.14.7, Git 2.56.0.windows.1, and `rustup run 1.94.0 rustc --edition=2024` after importing MSVC vcvars64, then runs the native probe executable — exit 0, seven Python PASS receipts plus both std assertions.
- Windows input archive SHA256: `cefc9206fe6fe6c9d0ede1048f0bef2d9e3bf2d8de289f9ecf510d2c62bed8ad`; extracted to `C:\probe\cyril-s2hb-premises-cefc9206`. It contains only the two probe sources and unchanged Python oracle.
- Source SHA256: Python oracle `4398ec864f860cb558896f3ce5be6de8d68469aeaec7e9b797ce93a0956f82a8`; Python probe `4b735fcf1c6131673ed14791f34ed4df91729f7a1feb6024a9a80d3cdd291cb2`; Rust probe `d46a25a539ce8177d18962d6ce087fc1f681fe758e850ab8ce70c31a1035bf17`.
- Initial Windows transport attempt could not execute: an inline base64 archive exceeded the PowerShell/WinRM command-length limit. No premise failed; switching to a checksum-verified HTTP transfer of the same archive resolved the transport failure. No host settings were changed to suppress it.

## Oracle

- P1: hand-specified symbol/caller/document values plus direct git diff bytes, independent of Python's gather traversal.
- P2: explicit literal byte sequences and documented host text newline expectations, independent of the oracle's file writer.
- P3: fixture-owned child bytes and exit statuses chosen before invocation; independent elapsed deadline enforced by subprocess, not serialized duration.
- P4: a child reports actual received argv, compared with a hand-authored expected argv list and Python's platform launcher; std capture is compared with literal emitted bytes.

## Comparisons

| ID | Probe output | Independent oracle output | Verdict |
|----|--------------|---------------------------|---------|
| P1 | Both hosts: one scoped changed file; new_helper added at line 2; caller at line 1; NOTES.md retained despite scope=src; fixed UTC timestamp; repeat leaves manifest bytes unchanged. | Hand-written fixture expectations, NOTES.md=6 bytes, direct Git patch bytes, exact repeat stdout. | PASS |
| P2 | Linux newline `0a`; Windows `0d0a`. Native Windows child-print diagnostic tail hex `68275d0d0d0a0d0a`; Linux tail `6c617368275d0a0a`. Binary patch equality holds. | Literal expected bytes after Python text-writer translation: existing CRLF child output becomes CRCRLF when written as captured text on Windows. | PASS |
| P3 | Both hosts: exact quote/space/backslash argv; exit 7 with replacement-decoded byte ff; 131072 bytes on each pipe captured; timeout retains partial line; absent executable exits 2. | Literal fixture bytes and statuses, expected stdout-before-stderr concatenation, independent fixed 2-second reported duration, real 0.2-second subprocess timeout. | PASS |
| P4 | Both native builds: argv receipt matches expected list; 131073 stdout bytes ending ff and 131073 stderr bytes ending fe captured without loss or deadlock. | Hand-authored argument values and independently chosen byte counts/tails. | PASS |

## Validated / learned

- P1: validated prior understanding. Scoped patches and whole-change document context coexist; gather repeat is idempotent. The old Python oracle has no version stamp, so the approved oracle schema addition remains implementation work.
- P2: learned that Windows diagnostics preserves double-CR behavior when native child CRLF passes through Python's text-file writer. Normalizing CRCRLF would violate literal same-host parity; binary patches must remain unchanged.
- P3: validated prior understanding. Python's capture drains both streams, preserves concatenation order, treats timeout/nonzero as data, and reports inability to start through die exit 2. No cancellation implementation is claimed proven.
- P4: validated the safe standard-library raw-argument-tail primitive and independent stream draining on Rust 1.94 Windows/MSVC and Linux. Resolving arbitrary Windows executable prefixes remains a design/implementation obligation, not an inferred PASS.
- Native resourcefs-win11 is reachable. Qualification-only tools were installed from checksum-verified official distributions. Python remains outside the shipped execution path.

## Hand-off

PASS for the empirical stage: P1–P4 have observed independent comparisons; P5–P6 are classified non-premises with explicit later owners. No production implementation, feature test, or design artifact was written by this stage. Next owner: falsifiable-design, consuming this evidence and spec.md.

## Related issues

Consulted upstream spec: cyril-s2hb (current slice), cyril-5gb3 (parent), cyril-m8qv (leaf/CLI/prefix/parity), cyril-queu (check lifecycle), cyril-r3t6 (pipe drain), cyril-zj2c (future authorization), cyril-p0xt (future assets), cyril-305w and cyril-4o1u (check cancellation consumers), cyril-7vrl (later native steps), cyril-ild0 (native qualification).

Filed: none. No underlying-system defect has yet been established by these probes.

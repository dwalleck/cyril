# cyril-0ssb — upstream asset/store evidence

## Premises

| ID | Question / dependent decision | Status |
| --- | --- | --- |
| P1 | Does current `KIRO_DATA_DIR` supply the exact root, or receive a platform leaf? Determines bundle root. | PASS — independently inspected Linux/Windows 2.32.0 and macOS 2.29.1 branches return the original value without a leaf. |
| P2 | Does trimming affect the returned nonempty path or only the emptiness test? Determines padded-path handling. | PASS — all three inspected branches test the trimmed slice length but return the original buffer and original length. |
| P3 | Does the credential database ignore the asset override? Determines store-wrapper separation. | PASS for current Linux and prior macOS source evidence below; native Windows runtime not executed. |

## Sources and observations

- Full Rivets cyril-0ssb record: sole historical asset reader `chat_cli::util::paths::data_dir`, empty-after-trim falls through; `GlobalPaths::database_path_static` resolves independently. Prior RE was 2.14.2; it does not settle the current returned padded value or all platform leaf semantics.
- `.cyril-igrx/route.md:67-108`: macOS 2.29.1 credential fn `0x101ef4900` joins `Library/Application Support` + lowercase `kiro-cli` + `data.sqlite3`; asset fn `0x101ef9674` reads `KIRO_DATA_DIR` before defaulting to the same native directory. This establishes separate consumers, not the full override branch. DMG provenance/hash retained there.
- Current Linux archive: `~/.local/share/kiro-research/binaries/2.32.0/extracted/kirocli/bin/`; BUILD_VERSION 2.32.0, BUILD_DATE 2026-10-15T21:16:42.279145+00:00, BUILD_HASH d6fb1e94d58ae3e7550941e520f060084f219a8e. SHA-256 `kiro-cli`: `58abd8b474b31588c463d321d70dc223a1ea4bbed9048c163a8d485f0f05e4dd`; `kiro-cli-chat`: `5ba2a13daaed1146533b6274d5967a4b0a20a5ffe85e00f5fdb1a2219d2d48e9` (sha256sum observed 2026-10-19).

### Decisive current binary override branch

Static investigation by independent scout RootSemantics identified the sole direct RIP-relative reader of literal `KIRO_DATA_DIR` (13 bytes, file offset/ELF VA `0x575b6f9`) in the current 2.32.0 `kiro-cli-chat`; resolver entry `0xa1f4f20`.

Disassembly dataflow: `0xa1f4f54` loads original environment buffer into r14 and `0xa1f4f58` loads original length into r15. `0xa1f4f78`/`0xa1f4f7b` pass them to trimming helper `0x98fd210`; `0xa1f4f83` tests only its returned slice length (rdx). Empty goes to the default at `0xa1f4fa0`; nonempty stores **r14 and r15** unchanged into the result at `0xa1f4f8c`/`0xa1f4f90`, then jumps to the epilogue `0xa1f5034`. No join or leaf append occurs on the accepted branch. Only the default branch joins the 8-byte `kiro-cli` literal at `0xa1f4ff2–0xa1f5009`. The trimming helper decodes Unicode whitespace (including ASCII tab/newline/space and U+1680).

The independently archived macOS 2.29.1 x86_64 branch `0x101ef9674–0x101ef96db` corroborates this exactly: r14/r15 hold original buffer/length, trim call `0x101ef96be`, returned-length test `0x101ef96c3`, original result buffer/length writes `0x101ef96cc`/`0x101ef96d0`, epilogue jump `0x101ef96db`. `Library/Application Support/kiro-cli` is appended only in the default branch.

### Independent Windows 2.32.0 binary

RootSemantics downloaded the manifest-named [Windows x86_64 MSI](https://desktop-release.q.us-east-1.amazonaws.com/2.32.0/kiro-cli-x86_64-pc-windows-msvc.msi) to owned temporary storage, matched its SHA-256 `0d7b556b5315767d1f3a83bca466d632461fcc6379f103d6ad5558234263f299` against `manifest.latest.json:281-292`, and extracted member `LocalApp/Kiro-Cli/kiro-cli.exe`. Extracted PE SHA-256: `d3ab257c3c3c80b16ee52c32e34f37ff16f97bcbbbb560e6fc37bccd7a7b48a5`. Inspection used PE image base `0x140000000`; no installation or Kiro execution occurred. The owned temporary directory was removed with an observed cleanup receipt.

`KIRO_DATA_DIR` is at PE **file offset** `0x13dc9d93`, preferred VA `0x153dcb593`; reader `0x142eafe90` passes length 13 at `0x142eafe9b`. Resolver `0x142eafe70–0x142eb00f9` saves the original buffer at `0x142eaffa1` and length in rdi, invokes Unicode trim helper `0x1409b8520` at `0x142eaffa8`, and tests only returned length at `0x142eaffad`. The nonblank arm returns the saved original buffer at `0x142eaffbe` and original length at `0x142eaffc2`, then jumps to epilogue at `0x142eaffd7`: **no leaf join**. The trim helper's inspected character classes include tabs/newlines, space, U+1680 and U+3000. This independently closes the Windows override premise instead of inferring it from Linux.

Validated rule on all independently inspected platforms: **test Unicode-trimmed emptiness, but preserve a nonblank value as-is**, including spaces and relative paths, then append `kas`. Do not apply the stricter KIRO_HOME validation, trim the returned asset override, or append a platform leaf. These are source/binary findings, not native Apple/Windows execution receipts; existing platform default fixtures remain required.

### Current CLI observation and limit

Authorized by parent: smallest safe empirical CLI observation, not a production verification run. Python subprocess driver launched archived real `kiro-cli` through `strace -f -e trace=%file` under temporary owned HOME, USERPROFILE, XDG_DATA_HOME, XDG_RUNTIME_DIR, XDG_CONFIG_HOME, XDG_CACHE_HOME, cwd, and an explicit PATH containing only the archive plus system binary directories. No parent credentials/environment were forwarded. Later probes additionally refused network via localhost:1 proxy variables and browser execution via BROWSER=/bin/false.

`kiro-cli acp --agent-engine v3` with `KIRO_DATA_DIR=<tmp>/assets` opened/created **`<tmp>/xdg/kiro-cli/data.sqlite3`** and exited 1 with `You are not logged in, please log in with kiro-cli login`. The asset root was never touched: this is positive current evidence of store independence, **not** a bundle-path or trimming verdict. A dummy SQLite token row under the owned fixture did not bypass the login guard; no real credential was copied or inspected.

An initial file blocker at the default store root caused repeated ENOTDIR checks and hit the observation deadline; those aborted arms establish no override conclusion. All later observation subprocesses used an owned new process group with kill-and-drain cleanup on deadline; the one earlier front-binary descendant was identified and terminated directly. No persistent agent or real user Kiro state is part of these observations.

## Data safety

Every write was under an automatically cleaned temporary fixture, including the CLI-created SQLite database and settings. Synthetic token/API-key experiments used only explicit dummy strings, never real credentials; no authentication/network success is claimed. No binaries or source outside this worktree were modified. No new permanent probe/oracle implementation is needed for these direct observations.

## Validated / learned

The current logged-out CLI reaches the credential database before self-extraction, so a missing-auth exit cannot prove asset selection. Store independence is observed; P1/P2 are resolved by the current binary's directly inspected override branch above, with independent macOS corroboration. Preserve existing platform default functions and split their consumers rather than redirecting the store. No separate oracle or new probe implementation is necessary.

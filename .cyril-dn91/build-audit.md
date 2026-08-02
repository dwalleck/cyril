# cyril-dn91 — checkpointed-build audit

Plan: `budgeted-plan.md` (7 slices). Every slice gated on: tests (both feature
configs), clippy `-D warnings` (both), fmt, probe fences, budgets (all vacuous
— no production loops added; every gate is an O(1) Option/enum check).

## Deviations from plan (all noted in commit messages)

1. **Slices 1+2 merged into one commit** — `Adapters` with no consumer trips
   `-D warnings` dead_code on the lib target (staged-module rule); the
   derivation fn is the consumer. The `auth` field likewise landed in slice 3
   with its consumer.
2. **cfg'd-out presence fields instead of uninhabited markers** — same C11
   unconstructibility guarantee, matching the `InternalChannels` precedent.
3. **`settings_extra` presence-key precondition dissolved structurally** — the
   extra nests under a fixed `settings` key, so the planned `debug_assert!`
   guards nothing; no assert shipped.
4. **Slice 7 grew the cyril-y14u fix** (user decision at the halt): the live
   KAS gate exposed that `kas/auth.rs` read only `kirocli:odic:token`; this
   machine's SocialGitHub login stores `kirocli:social:token`. Fixed in-branch
   (both rows read; freshest-expiry wins when both present; state-profile arn
   preferred, token-inline arn fallback; 3 new fences). test_bridge gained
   `--agent-engine` and `--prompt` (test-side).
   *y14u's suggested "accountType FIRST" direction was considered and
   rejected*: accountType is not in the credential store (it would need a
   `kiro-cli user whoami` subprocess or additional state parsing), while both
   token rows are already in hand; freshest-expiry-wins is deterministic,
   fenced (`freshest_token_row_wins_when_both_present`), and addresses the
   same leftover-shadowing risk. Recorded per pre-PR review finding P2.

## Slice-6 non-vacuity mutation check

Removed the shell_type host_io gate → `adapter_matrix_advertise_iff_answer`
failed exactly the `[v2] terminal: advertised != answers` cell. Restored
byte-exact (empty `git diff` on client.rs afterwards).

## Slice 7 — live parity evidence (AC6), 2026-08-02, kiro-cli 2.16.0

**v2, kas-feature build** (`cargo run --example test_bridge --features kas`):
full harness sequence green — session created, model/agent/tools/context/usage
queries, model switch, prompt turn streamed, `TurnCompleted`. v2 sends no host
callbacks; nothing to refuse, nothing refused.

**KAS, free path** (`--agent-engine kas`, Host hooks mode, prompt forcing
fs + terminal):

- session created (auth callback served from the sqlite store via the y14u
  fix — the pre-fix run fail-stopped "kiro token row absent" despite a live
  SocialGitHub login);
- `Read File` tool call **Completed** through cyril's fs host callback and the
  agent echoed the file's content;
- the workspace's own clippy stop-hook **executed live** (`[HookExecuted]
  clippy on stop: completed (exit Some(0))`) — the Inbound hooks path
  end-to-end;
- permission prompts flowed for each command (standard ACP path untouched);
- `run_command` tool calls all ended **Failed** (echo, /bin/echo, printf).

**Terminal-failure parity experiment:** identical scenario on a detached
worktree at main (9d69671, harness + auth fix carried over as test-side/
environmental enablers): `run_command` fails IDENTICALLY. The failure is
**pre-existing** — filed as **cyril-cb93** (suspected whole-command-line
rendered as one quoted token in `HostShell::command`). Terminal execution code
is untouched by this branch (`git diff main --stat` on terminal_io.rs +
host_shell.rs: empty), so strict parity holds: same live behavior before and
after, for every family.

## Claims → outcomes

| Claim | Outcome |
|---|---|
| C1/C13 auth refusal + no spurious BridgeError | fenced (`v2_refuses_auth_callback`, `auth_refusal_emits_no_bridge_error`) |
| C2/C3/C4 host-io refusals, side-effect-free, FS_OPS-walked | fenced (`v2_refuses_typed_fs`, `v2_refuses_kiro_fs_all_ops`, `v2_refuses_terminal_family`) |
| C5/C10/C12 hooks direction gate, registry de-sentinel, didChange gate | fenced (`hooks_inbound_absent_refuses` via probe + `registry_present_iff_inbound`, `did_change_gated_by_hooks_direction`) |
| C6 KAS parity | all pre-existing KAS answering tests pass unchanged + live evidence above |
| C7 derived advertisement | `advertisement_is_fully_determined_by_presence_direction_extras` byte-identical against the derivation |
| C8 no overridable capability method | trait method deleted; C9 matrix is the regression net (approved design decision 3) |
| C9/C14 advertise⇔answer matrix + unknown-null | `adapter_matrix_advertise_iff_answer`, mutation-checked |
| C11 default-build posture | presence fields cfg'd out (unconstructible); default test+clippy legs green every slice |

## Discovered and filed during the build

- **cyril-y14u** — KAS auth responder missed social-login token row (fixed
  in-branch, user decision).
- **cyril-cb93** — live KAS `run_command` always fails on Linux host
  (pre-existing; parity-proven against main; NOT fixed here).

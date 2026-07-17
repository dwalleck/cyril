# Findings — cyril-08eh prove-it-prototype

## Smallest question

Does cyril's converter route `_kiro/system/notify {level, message}` (KAS dialect)
to anything, or drop it? And does an unknown `_kiro/system/*` sub-method still
drop cleanly?

## Probe

In-crate unit tests in `crates/cyril-core/src/protocol/convert/kiro.rs`:

```
probe_kas_system_notify_info_converts            FAILED  Ok(None)
probe_kas_system_notify_warning_converts         FAILED  Ok(None)
probe_kas_system_notify_unknown_level_converts   FAILED  Ok(None)
probe_kas_system_notify_missing_level_defaults   FAILED  Ok(None)
probe_kas_system_notify_missing_message_defaults FAILED  Ok(None)
probe_unknown_kiro_system_still_dropped          ok      Ok(None)
```

## Oracle (independent: grep)

`grep 'system/notify' crates/cyril-core/src/protocol/convert/kiro.rs` → **zero
match arms**. The `other => Ok(None)` unknown-method drop (kiro.rs:840) catches
`"kiro/system/notify"`.

## Agreement

Probe (runtime) and oracle (static grep) agree: the KAS-dialect `kiro/system/notify`
is dropped to `Ok(None)`. **Agree.**

## Wire-naming (same mechanism as cyril-3zy4)

ACP crate strips the single leading `_` → `_kiro/system/notify` → `kiro/system/notify`.
Payload: `{level: "info" | "warning", message: string}` confirmed in
`docs/kiro-2.12.3-wire-audit.md:22`.

## Scope: broader than cyril-3zy4

Unlike `_kiro/error/rate_limit` which had a pre-existing `RateLimited` variant,
`_kiro/system/notify` has **no existing variant, converter arm, or UI handler**.
Three new pieces needed:

1. `SystemNotifyLevel` enum (Info, Warning, Unknown) + `Notification::SystemNotify` variant
2. Converter arm in `kiro::to_ext_notification` matching `"kiro/system/notify"`
3. UI handler in `UiState::apply_notification` calling `add_system_message`

## What I learned (that wasn't obvious before probing)

The `_kiro/system/` namespace is a PREFIX — `_kiro/system/notify` is the first
concrete method, but KAS may add more. The converter arm must match
`"kiro/system/notify"` exactly (not `"kiro/system/"` prefix), so future
`_kiro/system/*` methods still drop cleanly and get separate handling. The
unknown-namespace test fences this.

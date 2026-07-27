//! cyril-nd4h claim C5: removing a config field must not break existing files.
//!
//! `ui.highlight_cache_size` and `ui.stream_buffer_timeout_ms` were serialized
//! and documented for months while having zero production consumers, so real
//! user `config.toml` files name them. Removing the struct fields must leave
//! those files loading exactly as before.
//!
//! THE TRAP THIS FENCE EXISTS TO DEFEAT: `Config::load_from_path` swallows
//! every failure -- missing, unreadable, and malformed all return
//! `Self::default()` with only a `warn!`. So "a Config came back" is worthless
//! as evidence: a deserializer that REJECTED the unknown keys and one that
//! IGNORED them both hand you a `Config`. The discriminator is a known field
//! set to a NON-default value in the same file:
//!
//!   max_messages = 999  ->  999 means parsed-and-ignored (C5 holds)
//!                       ->  500 means it errored and silently fell back
//!
//! Assert the 999. Asserting anything weaker fences nothing.

use cyril_core::types::config::Config;

/// Write `body` to a temp `config.toml` and load it. The `TempDir` is returned
/// so the caller keeps it alive -- dropping it deletes the file out from under
/// the test.
fn load(body: &str) -> anyhow::Result<(tempfile::TempDir, Config)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("config.toml");
    std::fs::write(&path, body)?;
    let config = Config::load_from_path(&path);
    Ok((dir, config))
}

#[test]
fn legacy_config_naming_removed_fields_still_parses() -> anyhow::Result<()> {
    let (_dir, config) = load(
        r#"
[ui]
max_messages = 999
highlight_cache_size = 40
stream_buffer_timeout_ms = 300
"#,
    )?;

    assert_eq!(
        config.ui.max_messages, 999,
        "removed keys must be ignored, not rejected -- 500 here means the file \
         failed to parse and load_from_path silently fell back to defaults, \
         which would mean removing the fields is NOT compat-safe"
    );
    assert!(
        config.ui.mouse_capture,
        "an unspecified field still takes its default"
    );
    Ok(())
}

#[test]
fn unknown_keys_are_ignored_alongside_honored_ones() -> anyhow::Result<()> {
    // A key cyril never knew, mixed with one it must honor. Guards against a
    // future `deny_unknown_fields` being added without anyone noticing that it
    // would reject every config written before that day.
    let (_dir, config) = load(
        r#"
[ui]
max_messages = 999
mouse_capture = false
a_key_cyril_has_never_heard_of = "xyz"
"#,
    )?;

    assert_eq!(config.ui.max_messages, 999);
    assert!(
        !config.ui.mouse_capture,
        "mouse_capture must survive alongside an unknown sibling key"
    );
    Ok(())
}

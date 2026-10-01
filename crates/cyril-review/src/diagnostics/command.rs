use crate::{DiagnosticsError, Result};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

pub(super) fn prepare(text: &str, workspace: &Path) -> Result<Command> {
    if text.contains('\0') {
        return Err(invalid("command contains a NUL character"));
    }
    let mut command = native_command(text, workspace)?;
    command.current_dir(workspace);
    for (name, value) in std::env::vars_os() {
        if value.is_empty() && toolchain_name(&name) {
            command.env_remove(name);
        }
    }
    Ok(command)
}

fn toolchain_name(name: &OsStr) -> bool {
    let bytes = name.as_encoded_bytes();
    [b"CARGO_".as_slice(), b"RUST".as_slice()]
        .iter()
        .any(|prefix| {
            #[cfg(windows)]
            {
                bytes
                    .get(..prefix.len())
                    .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
            }
            #[cfg(not(windows))]
            {
                bytes.starts_with(prefix)
            }
        })
}

fn invalid(message: &str) -> crate::ReviewError {
    DiagnosticsError::InvalidCommand {
        message: message.to_owned(),
    }
    .into()
}

#[cfg(not(windows))]
fn native_command(text: &str, _workspace: &Path) -> Result<Command> {
    let mut words = split_posix(text)?.into_iter();
    let program = words.next().ok_or_else(|| invalid("command is empty"))?;
    if program.is_empty() {
        return Err(invalid("executable is empty"));
    }
    let mut command = Command::new(program);
    command.args(words);
    Ok(command)
}

#[cfg(not(windows))]
fn split_posix(text: &str) -> Result<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut active = false;
    let mut quote = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(delimiter), ch) if delimiter == ch => quote = None,
            (Some('\''), ch) => word.push(ch),
            (Some('"'), '\\') => {
                let next = chars
                    .peek()
                    .copied()
                    .ok_or_else(|| invalid("trailing backslash"))?;
                if next == '"' || next == '\\' {
                    chars.next();
                    word.push(next);
                } else {
                    word.push('\\');
                }
            }
            (Some(_), ch) => word.push(ch),
            (None, '\'' | '"') => {
                active = true;
                quote = Some(ch);
            }
            (None, '\\') => {
                active = true;
                word.push(chars.next().ok_or_else(|| invalid("trailing backslash"))?);
            }
            (None, ' ' | '\t' | '\r' | '\n') => {
                if active {
                    words.push(std::mem::take(&mut word));
                    active = false;
                }
            }
            (None, ch) => {
                active = true;
                word.push(ch);
            }
        }
    }
    if quote.is_some() {
        return Err(invalid("unclosed quote"));
    }
    if active {
        words.push(word);
    }
    Ok(words)
}

#[cfg(windows)]
fn native_command(text: &str, workspace: &Path) -> Result<Command> {
    use std::os::windows::process::CommandExt;
    let original = text;
    let text = text.trim_start_matches([' ', '\t']);
    if text.is_empty() {
        return Err(invalid("command is empty"));
    }
    let mut search = None;
    let selected = if let Some(quoted) = text.strip_prefix('"') {
        let end = quoted
            .find('"')
            .ok_or_else(|| invalid("unclosed executable quote"))?;
        let candidate = &quoted[..end];
        if candidate.is_empty() {
            return Err(invalid("executable is empty"));
        }
        find_executable(candidate, workspace, &mut search)
            .map_err(|source| cannot_start(original, source))?
            .map(|program| (program, &quoted[end + 1..]))
    } else {
        let mut selected = None;
        // CreateProcess tries progressively longer unquoted executable prefixes.
        for end in text
            .char_indices()
            .filter_map(|(index, ch)| matches!(ch, ' ' | '\t').then_some(index))
            .chain(std::iter::once(text.len()))
        {
            if let Some(program) = find_executable(&text[..end], workspace, &mut search)
                .map_err(|source| cannot_start(original, source))?
            {
                selected = Some((program, &text[end..]));
                break;
            }
        }
        selected
    };
    let (program, tail) = selected.ok_or_else(|| {
        cannot_start(
            original,
            std::io::Error::new(std::io::ErrorKind::NotFound, "executable not found"),
        )
    })?;
    let mut command = Command::new(program);
    // Raw tail is never tokenized or reconstructed. Named batch files retain
    // native std/cmd.exe dispatch; arbitrary text never gains a shell wrapper.
    command.raw_arg(tail);
    Ok(command)
}

#[cfg(windows)]
fn find_executable(
    candidate: &str,
    workspace: &Path,
    search: &mut Option<Vec<std::path::PathBuf>>,
) -> std::io::Result<Option<std::path::PathBuf>> {
    use std::path::PathBuf;
    let mut name = PathBuf::from(candidate);
    if name.extension().is_none() && !candidate.ends_with('.') {
        name.as_mut_os_string().push(".exe");
    }
    if name.components().count() > 1 || name.is_absolute() {
        return existing_file(workspace.join(name));
    }
    let directories = match search {
        Some(directories) => directories,
        None => {
            let mut directories = Vec::new();
            if let Some(parent) = std::env::current_exe()?.parent() {
                directories.push(parent.to_path_buf());
            }
            directories.push(workspace.to_path_buf());
            if let Some(root) = std::env::var_os("SystemRoot") {
                let root = PathBuf::from(root);
                directories.extend([root.join("System32"), root.join("System"), root]);
            }
            if let Some(path) = std::env::var_os("PATH") {
                directories.extend(std::env::split_paths(&path).map(|path| workspace.join(path)));
            }
            search.insert(directories)
        }
    };
    for directory in directories {
        if let Some(path) = existing_file(directory.join(&name))? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

#[cfg(windows)]
fn existing_file(path: std::path::PathBuf) -> std::io::Result<Option<std::path::PathBuf>> {
    match path.metadata() {
        Ok(metadata) => Ok(metadata.is_file().then_some(path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn cannot_start(command: &str, source: std::io::Error) -> crate::ReviewError {
    DiagnosticsError::CannotStart {
        command: command.to_owned(),
        source,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn posix_words_preserve_empty_quotes_escapes_and_literal_metacharacters() -> Result<()> {
        assert_eq!(
            split_posix(
                r#"tool '' "" 'one two' three\ four "quote\" slash\\ dollar\$" ; | $HOME # literal"#
            )?,
            [
                "tool",
                "",
                "",
                "one two",
                "three four",
                "quote\" slash\\ dollar\\$",
                ";",
                "|",
                "$HOME",
                "#",
                "literal"
            ],
        );
        assert_eq!(
            split_posix("a\tb\rc\nd e\u{b}f g\u{c}h")?,
            ["a", "b", "c", "d", "e\u{b}f", "g\u{c}h"]
        );
        assert_eq!(
            split_posix(r#"pre" middle "post 'single\slash' escaped\'quote"#)?,
            ["pre middle post", "single\\slash", "escaped'quote"]
        );
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn posix_malformed_quotes_and_trailing_escape_are_typed_errors() {
        for text in ["tool '", "tool \"", "tool \\"] {
            assert!(matches!(
                split_posix(text),
                Err(crate::ReviewError::Diagnostics(
                    DiagnosticsError::InvalidCommand { .. }
                ))
            ));
        }
    }

    #[test]
    fn environment_prefix_check_uses_native_name_semantics() {
        assert!(toolchain_name(OsStr::new("CARGO_TARGET_DIR")));
        assert!(toolchain_name(OsStr::new("RUSTUP_TOOLCHAIN")));
        assert!(!toolchain_name(OsStr::new("CARGO")));
        assert!(!toolchain_name(OsStr::new("OTHER_RUST")));
        assert_eq!(
            toolchain_name(OsStr::new("cargo_target_dir")),
            cfg!(windows)
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            assert!(!toolchain_name(OsStr::from_bytes(b"OTHER_\xff")));
            assert!(toolchain_name(OsStr::from_bytes(b"RUST\xff")));
        }
    }
}

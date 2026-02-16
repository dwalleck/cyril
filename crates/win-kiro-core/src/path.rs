use std::path::{Path, PathBuf};

use serde_json::Value;

/// Direction of path translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    WinToWsl,
    WslToWin,
}

/// Convert a Windows path to a WSL path.
///
/// `C:\Users\foo\bar` → `/mnt/c/Users/foo/bar`
/// `D:\project` → `/mnt/d/project`
pub fn win_to_wsl(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    // Handle drive letter paths like C:\ or C:/
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let drive = s.as_bytes()[0].to_ascii_lowercase() as char;
        let rest = &s[2..];
        let rest = rest.replace('\\', "/");
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() {
            PathBuf::from(format!("/mnt/{drive}"))
        } else {
            PathBuf::from(format!("/mnt/{drive}/{rest}"))
        }
    } else {
        // Already a unix-style path or relative — return as-is with forward slashes
        PathBuf::from(s.replace('\\', "/"))
    }
}

/// Convert a WSL path to a Windows path.
///
/// `/mnt/c/Users/foo/bar` → `C:\Users\foo\bar`
/// `/mnt/d/project` → `D:\project`
pub fn wsl_to_win(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("/mnt/") {
        if rest.len() >= 1 {
            let drive = rest.as_bytes()[0].to_ascii_uppercase() as char;
            let after_drive = &rest[1..];
            if after_drive.is_empty() || after_drive.starts_with('/') {
                let suffix = after_drive.strip_prefix('/').unwrap_or("");
                let win_path = if suffix.is_empty() {
                    format!("{drive}:\\")
                } else {
                    format!("{drive}:\\{}", suffix.replace('/', "\\"))
                };
                return PathBuf::from(win_path);
            }
        }
    }
    // Not a /mnt/ path — return as-is
    PathBuf::from(path)
}

/// Recursively translate paths in a JSON value.
/// Looks for string values that look like paths and translates them.
pub fn translate_paths_in_json(value: &mut Value, direction: Direction) {
    match value {
        Value::String(s) => {
            let translated = match direction {
                Direction::WinToWsl => {
                    if looks_like_windows_path(s) {
                        win_to_wsl(Path::new(s.as_str()))
                            .to_string_lossy()
                            .into_owned()
                    } else {
                        return;
                    }
                }
                Direction::WslToWin => {
                    if looks_like_wsl_mount_path(s) {
                        wsl_to_win(s).to_string_lossy().into_owned()
                    } else {
                        return;
                    }
                }
            };
            *s = translated;
        }
        Value::Array(arr) => {
            for item in arr {
                translate_paths_in_json(item, direction);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                translate_paths_in_json(v, direction);
            }
        }
        _ => {}
    }
}

fn looks_like_windows_path(s: &str) -> bool {
    s.len() >= 3
        && s.as_bytes()[0].is_ascii_alphabetic()
        && s.as_bytes()[1] == b':'
        && (s.as_bytes()[2] == b'\\' || s.as_bytes()[2] == b'/')
}

fn looks_like_wsl_mount_path(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix("/mnt/") {
        rest.len() >= 1 && rest.as_bytes()[0].is_ascii_alphabetic()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_win_to_wsl_c_drive() {
        assert_eq!(
            win_to_wsl(Path::new(r"C:\Users\foo\bar")),
            PathBuf::from("/mnt/c/Users/foo/bar")
        );
    }

    #[test]
    fn test_win_to_wsl_d_drive() {
        assert_eq!(
            win_to_wsl(Path::new(r"D:\project\src")),
            PathBuf::from("/mnt/d/project/src")
        );
    }

    #[test]
    fn test_win_to_wsl_root() {
        assert_eq!(
            win_to_wsl(Path::new(r"C:\")),
            PathBuf::from("/mnt/c")
        );
    }

    #[test]
    fn test_win_to_wsl_forward_slashes() {
        assert_eq!(
            win_to_wsl(Path::new("C:/Users/foo")),
            PathBuf::from("/mnt/c/Users/foo")
        );
    }

    #[test]
    fn test_wsl_to_win_basic() {
        assert_eq!(
            wsl_to_win("/mnt/c/Users/foo/bar"),
            PathBuf::from(r"C:\Users\foo\bar")
        );
    }

    #[test]
    fn test_wsl_to_win_d_drive() {
        assert_eq!(
            wsl_to_win("/mnt/d/project"),
            PathBuf::from(r"D:\project")
        );
    }

    #[test]
    fn test_wsl_to_win_root() {
        assert_eq!(
            wsl_to_win("/mnt/c"),
            PathBuf::from(r"C:\")
        );
    }

    #[test]
    fn test_wsl_to_win_non_mount_path() {
        assert_eq!(
            wsl_to_win("/home/user/.config"),
            PathBuf::from("/home/user/.config")
        );
    }

    #[test]
    fn test_roundtrip_win_wsl_win() {
        let original = r"C:\Users\dwall\repos\project\src\main.rs";
        let wsl = win_to_wsl(Path::new(original));
        let back = wsl_to_win(&wsl.to_string_lossy());
        assert_eq!(back, PathBuf::from(original));
    }

    #[test]
    fn test_translate_json_wsl_to_win() {
        let mut val = serde_json::json!({
            "path": "/mnt/c/Users/foo/file.txt",
            "content": "hello world",
            "nested": {
                "file": "/mnt/d/project/src/main.rs"
            }
        });
        translate_paths_in_json(&mut val, Direction::WslToWin);
        assert_eq!(val["path"], r"C:\Users\foo\file.txt");
        assert_eq!(val["content"], "hello world");
        assert_eq!(val["nested"]["file"], r"D:\project\src\main.rs");
    }

    #[test]
    fn test_translate_json_win_to_wsl() {
        let mut val = serde_json::json!({
            "path": r"C:\Users\foo\file.txt",
            "count": 42
        });
        translate_paths_in_json(&mut val, Direction::WinToWsl);
        assert_eq!(val["path"], "/mnt/c/Users/foo/file.txt");
        assert_eq!(val["count"], 42);
    }
}

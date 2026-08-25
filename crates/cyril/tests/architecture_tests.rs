use std::path::Path;

use anyhow::{Context, Result};

#[test]
fn c10_core_and_ui_remain_persistence_free() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace crates directory")?;
    for crate_name in ["cyril-core", "cyril-ui"] {
        let manifest_path = workspace.join(crate_name).join("Cargo.toml");
        let contents = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?;
        let manifest: toml::Value = toml::from_str(&contents)
            .with_context(|| format!("parse {}", manifest_path.display()))?;
        assert_no_memory_dependency(&manifest, crate_name);
    }
    Ok(())
}

#[test]
fn c13_memory_policy_stays_behind_runtime_interface() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace crates directory")?;
    let forbidden = [
        ("bm25(", "source ranking"),
        ("source_turns_fts", "source SQL/FTS"),
        ("<CYRIL_EPISODES", "episode framing"),
        ("cyril-source-turn-", "source hashing"),
    ];
    for crate_name in ["cyril", "cyril-core", "cyril-ui"] {
        let root = workspace.join(crate_name).join("src");
        for path in rust_files(&root)? {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            for (needle, policy) in forbidden {
                assert!(
                    !contents.contains(needle),
                    "C13: {policy} policy escaped memory boundary in {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn rust_files(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(rust_files(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(files)
}

fn assert_no_memory_dependency(value: &toml::Value, crate_name: &str) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, value) in table {
        if matches!(
            key.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) && let Some(dependencies) = value.as_table()
        {
            assert!(
                !dependencies.contains_key("cyril-memory"),
                "{crate_name} must remain persistence-free"
            );
        }
        assert_no_memory_dependency(value, crate_name);
    }
}

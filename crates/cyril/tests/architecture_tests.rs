use std::path::Path;

use anyhow::{Context, Result};

#[test]
fn core_and_ui_remain_persistence_free() -> Result<()> {
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

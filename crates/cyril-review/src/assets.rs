//! The review workflow's canonical assets, embedded verbatim: the recipe and
//! its four agents. `/review` installs these bytes; the experiment's `cr-*`
//! agents and the recipe itself are generated from them by
//! `experiments/code-review-workflow/build_recipe.py`, and CI fails when the
//! committed files differ from what it generates.

/// The name the recipe registers under.
pub const WORKFLOW_NAME: &str = "cyril-review";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKind {
    Workflow,
    Agent,
}

/// One embedded asset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Asset {
    kind: AssetKind,
    name: &'static str,
    contents: &'static str,
}

impl Asset {
    pub fn kind(&self) -> AssetKind {
        self.kind
    }

    /// The workflow or agent name, e.g. `cyril-review-clerk`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The file name it installs as: `<name>.workflow.json` or `<name>.md`.
    pub fn file_name(&self) -> String {
        match self.kind {
            AssetKind::Workflow => format!("{}.workflow.json", self.name),
            AssetKind::Agent => format!("{}.md", self.name),
        }
    }

    /// The exact bytes to install.
    pub fn contents(&self) -> &'static str {
        self.contents
    }
}

/// The recipe first, then the agents it uses.
pub const ASSETS: [Asset; 5] = [
    Asset {
        kind: AssetKind::Workflow,
        name: WORKFLOW_NAME,
        contents: include_str!("../assets/cyril-review.workflow.json"),
    },
    Asset {
        kind: AssetKind::Agent,
        name: "cyril-review-finder",
        contents: include_str!("../assets/agents/cyril-review-finder.md"),
    },
    Asset {
        kind: AssetKind::Agent,
        name: "cyril-review-verifier",
        contents: include_str!("../assets/agents/cyril-review-verifier.md"),
    },
    Asset {
        kind: AssetKind::Agent,
        name: "cyril-review-clerk",
        contents: include_str!("../assets/agents/cyril-review-clerk.md"),
    },
    Asset {
        kind: AssetKind::Agent,
        name: "cyril-review-commenter",
        contents: include_str!("../assets/agents/cyril-review-commenter.md"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The KAS workflow engine's hard cap on step nodes.
    const STEP_CAP: usize = 20;

    fn steps(nodes: &[Value]) -> Vec<&Value> {
        let mut found = Vec::new();
        for node in nodes {
            if node["type"] == "step" {
                found.push(node);
            }
            for key in ["steps", "branches"] {
                if let Some(children) = node[key].as_array() {
                    found.extend(steps(children));
                }
            }
        }
        found
    }

    fn agents() -> impl Iterator<Item = &'static Asset> {
        ASSETS
            .iter()
            .filter(|asset| asset.kind() == AssetKind::Agent)
    }

    #[test]
    fn recipe_uses_only_embedded_agents_within_the_step_cap() -> Result<(), serde_json::Error> {
        let recipe: Value = serde_json::from_str(ASSETS[0].contents())?;
        assert_eq!(recipe["name"], WORKFLOW_NAME);
        assert_eq!(recipe["inputs"]["crtool"], "string");
        let steps = steps(recipe["steps"].as_array().map_or(&[], Vec::as_slice));
        assert!(
            !steps.is_empty() && steps.len() <= STEP_CAP,
            "{} step nodes",
            steps.len()
        );
        let names: Vec<&str> = agents().map(Asset::name).collect();
        for step in steps {
            let agent = step["agent"].as_str().unwrap_or_default();
            assert!(
                names.contains(&agent),
                "step {} uses unknown agent {agent:?}",
                step["id"]
            );
        }
        Ok(())
    }

    #[test]
    fn every_agent_declares_its_own_name() {
        for agent in agents() {
            let declared = agent
                .contents()
                .lines()
                .find_map(|line| line.strip_prefix("name: "));
            assert_eq!(declared, Some(agent.name()), "{}", agent.file_name());
        }
    }

    #[test]
    fn clerk_and_commenter_name_the_crtool_input_and_no_path() {
        // `.kiro/steering/` is legitimate (the conventions finder reads a
        // repository's rules files); the Python tool's path is not.
        for asset in ASSETS {
            assert!(
                !asset.contents().contains("crtool.py"),
                "{}",
                asset.file_name()
            );
            assert!(
                !asset.contents().contains(".kiro/code-review"),
                "{}",
                asset.file_name()
            );
        }
        for name in ["cyril-review-clerk", "cyril-review-commenter"] {
            let asset = ASSETS.iter().find(|asset| asset.name() == name);
            let text = asset.map(|asset| {
                asset
                    .contents()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            });
            assert!(
                text.is_some_and(|text| text
                    .contains("the crtool command given in this workflow's crtool input")),
                "{name}"
            );
        }
    }

    #[test]
    fn file_names_follow_the_install_layout() {
        assert_eq!(ASSETS[0].file_name(), "cyril-review.workflow.json");
        assert_eq!(ASSETS[1].file_name(), "cyril-review-finder.md");
    }
}

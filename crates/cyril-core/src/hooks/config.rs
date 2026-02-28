use std::path::Path;

use anyhow::{Context, Result};
use async_trait::async_trait;
use glob::Pattern;
use serde::Deserialize;
use tokio::process::Command;

use super::types::*;

/// Top-level hooks configuration file.
#[derive(Debug, Deserialize)]
pub struct HooksConfig {
    pub hooks: Vec<ShellHookDef>,
}

/// A single hook definition from the JSON config.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellHookDef {
    /// Human-readable name.
    pub name: String,
    /// When the hook fires: "beforeRead", "afterWrite", "beforeTerminal", etc.
    pub event: String,
    /// Optional glob pattern to filter by file path (e.g. "**/*.cs").
    pub pattern: Option<String>,
    /// Shell command to execute. Supports `${file}` placeholder.
    pub command: String,
    /// If true, command output is sent back to the agent as a follow-up prompt.
    #[serde(default)]
    pub feedback: bool,
}

impl ShellHookDef {
    fn parse_event(&self) -> Option<(HookTiming, HookTarget)> {
        match self.event.as_str() {
            "beforeRead" => Some((HookTiming::Before, HookTarget::FsRead)),
            "afterRead" => Some((HookTiming::After, HookTarget::FsRead)),
            "beforeWrite" => Some((HookTiming::Before, HookTarget::FsWrite)),
            "afterWrite" => Some((HookTiming::After, HookTarget::FsWrite)),
            "beforeTerminal" => Some((HookTiming::Before, HookTarget::Terminal)),
            "afterTerminal" => Some((HookTiming::After, HookTarget::Terminal)),
            "turnEnd" => Some((HookTiming::After, HookTarget::TurnEnd)),
            _ => None,
        }
    }
}

/// Tracks whether a hook has a glob filter and whether it compiled successfully.
#[derive(Debug)]
enum GlobFilter {
    /// No pattern configured — hook matches all files.
    MatchAll,
    /// Pattern compiled successfully.
    Pattern(Pattern),
    /// Pattern failed to compile — hook matches no files (fail closed).
    Invalid,
}

/// A configured shell hook that implements the Hook trait.
#[derive(Debug)]
pub struct ShellHook {
    def: ShellHookDef,
    timing: HookTiming,
    target: HookTarget,
    glob: GlobFilter,
}

impl ShellHook {
    pub fn from_def(def: ShellHookDef) -> Option<Self> {
        let (timing, target) = def.parse_event()?;
        let glob = match &def.pattern {
            None => GlobFilter::MatchAll,
            Some(p) => match Pattern::new(p) {
                Ok(pattern) => GlobFilter::Pattern(pattern),
                Err(e) => {
                    tracing::warn!(
                        "Hook '{}': invalid glob pattern '{}': {e} — hook will not match any files",
                        def.name,
                        p,
                    );
                    GlobFilter::Invalid
                }
            },
        };
        Some(Self {
            def,
            timing,
            target,
            glob,
        })
    }

    /// Check if the file path matches this hook's glob pattern.
    fn matches_path(&self, path: &Path) -> bool {
        match &self.glob {
            GlobFilter::MatchAll => true,
            GlobFilter::Invalid => false,
            GlobFilter::Pattern(pattern) => {
                let path_str = path.to_string_lossy();
                // Try matching against the full path and just the filename
                pattern.matches(&path_str)
                    || path
                        .file_name()
                        .map(|f| pattern.matches(&f.to_string_lossy()))
                        .unwrap_or(false)
            }
        }
    }

    /// Substitute `${file}` in the command with the actual path.
    fn expand_command(&self, ctx: &HookContext) -> String {
        let mut cmd = self.def.command.clone();
        if let Some(path) = &ctx.path {
            cmd = cmd.replace("${file}", &path.to_string_lossy());
        }
        cmd
    }
}

#[async_trait(?Send)]
impl Hook for ShellHook {
    fn name(&self) -> &str {
        &self.def.name
    }

    fn timing(&self) -> HookTiming {
        self.timing
    }

    fn target(&self) -> HookTarget {
        self.target
    }

    async fn run(&self, ctx: &HookContext) -> HookResult {
        // Check glob pattern for file-based hooks
        if let Some(path) = &ctx.path {
            if !self.matches_path(path) {
                return HookResult::Continue;
            }
        }

        let cmd = self.expand_command(ctx);
        tracing::info!("Running hook '{}': {}", self.def.name, cmd);

        let output = Command::new("cmd")
            .args(["/C", &cmd])
            .output()
            .await;

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !output.status.success() {
                    let combined = format!(
                        "Hook '{}' failed (exit {}):\n{stdout}{stderr}",
                        self.def.name,
                        output.status.code().unwrap_or(-1)
                    );
                    tracing::warn!("{combined}");

                    if self.def.feedback {
                        return HookResult::FeedbackPrompt { text: combined };
                    }
                    // Before-hooks that fail should block the operation (fail closed).
                    // After-hooks that fail just log and continue since the operation already happened.
                    if self.timing == HookTiming::Before {
                        return HookResult::Blocked { reason: combined };
                    }
                    return HookResult::Continue;
                }

                if self.def.feedback {
                    let combined = format!("{stdout}{stderr}");
                    if !combined.trim().is_empty() {
                        return HookResult::FeedbackPrompt {
                            text: format!(
                                "Hook '{}' output:\n{combined}",
                                self.def.name
                            ),
                        };
                    }
                }

                HookResult::Continue
            }
            Err(e) => {
                tracing::error!("Failed to run hook '{}': {e}", self.def.name);
                // If a before-hook can't even execute, block the operation rather than
                // silently proceeding without the safety check.
                if self.timing == HookTiming::Before {
                    HookResult::Blocked {
                        reason: format!("Hook '{}' failed to execute: {e}", self.def.name),
                    }
                } else {
                    HookResult::Continue
                }
            }
        }
    }
}

/// Load hooks from a JSON config file.
pub fn load_hooks_config(path: &Path) -> Result<Vec<Box<dyn Hook>>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read hooks config: {}", path.display()))?;

    let config: HooksConfig = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse hooks config: {}", path.display()))?;

    let mut hooks: Vec<Box<dyn Hook>> = Vec::new();
    for def in config.hooks {
        let name = def.name.clone();
        let event = def.event.clone();
        match ShellHook::from_def(def) {
            Some(hook) => {
                tracing::info!("Loaded hook: {} ({})", name, event);
                hooks.push(Box::new(hook));
            }
            None => {
                tracing::warn!("Skipping hook '{}': unknown event '{}'", name, event);
            }
        }
    }

    Ok(hooks)
}

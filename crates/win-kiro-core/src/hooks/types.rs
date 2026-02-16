use std::path::PathBuf;

use async_trait::async_trait;

/// When the hook runs relative to the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTiming {
    Before,
    After,
}

/// What kind of operation the hook targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTarget {
    FsRead,
    FsWrite,
    Terminal,
    TurnEnd,
}

/// Context passed to hooks.
#[derive(Debug, Clone)]
pub struct HookContext {
    pub target: HookTarget,
    pub timing: HookTiming,
    /// The file path involved (for fs operations).
    pub path: Option<PathBuf>,
    /// The content involved (for write operations).
    pub content: Option<String>,
    /// The command involved (for terminal operations).
    pub command: Option<String>,
}

/// Result of running a hook.
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue with the operation unchanged.
    Continue,
    /// Continue with modified arguments (e.g., reformatted content).
    ModifiedArgs { content: Option<String> },
    /// Block the operation with a reason.
    Blocked { reason: String },
    /// Inject feedback as a follow-up prompt to the agent.
    FeedbackPrompt { text: String },
}

/// Trait for implementing hooks. Async because shell hooks spawn processes.
#[async_trait(?Send)]
pub trait Hook: std::fmt::Debug {
    fn name(&self) -> &str;
    fn timing(&self) -> HookTiming;
    fn target(&self) -> HookTarget;
    async fn run(&self, ctx: &HookContext) -> HookResult;
}

/// Registry that holds and executes hooks in order.
#[derive(Debug, Default)]
pub struct HookRegistry {
    hooks: Vec<Box<dyn Hook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn register(&mut self, hook: Box<dyn Hook>) {
        self.hooks.push(hook);
    }

    /// Run all before-hooks for the given target. Returns the first blocking result,
    /// or Continue if all hooks pass.
    pub async fn run_before(&self, ctx: &HookContext) -> HookResult {
        for hook in &self.hooks {
            if hook.timing() == HookTiming::Before && hook.target() == ctx.target {
                let result = hook.run(ctx).await;
                match &result {
                    HookResult::Continue => continue,
                    HookResult::ModifiedArgs { .. } => return result,
                    HookResult::Blocked { .. } => return result,
                    HookResult::FeedbackPrompt { .. } => return result,
                }
            }
        }
        HookResult::Continue
    }

    /// Run all after-hooks for the given target. Collects feedback prompts.
    pub async fn run_after(&self, ctx: &HookContext) -> Vec<HookResult> {
        let mut results = Vec::new();
        for hook in &self.hooks {
            if hook.timing() == HookTiming::After && hook.target() == ctx.target {
                let result = hook.run(ctx).await;
                if !matches!(result, HookResult::Continue) {
                    results.push(result);
                }
            }
        }
        results
    }
}

use std::path::PathBuf;

use super::types::*;

/// Hook that blocks writes outside a project directory.
#[derive(Debug)]
pub struct PathValidationHook {
    pub allowed_root: PathBuf,
}

impl Hook for PathValidationHook {
    fn name(&self) -> &str {
        "path-validation"
    }

    fn timing(&self) -> HookTiming {
        HookTiming::Before
    }

    fn target(&self) -> HookTarget {
        HookTarget::FsWrite
    }

    fn run(&self, ctx: &HookContext) -> HookResult {
        if let Some(path) = &ctx.path {
            if !path.starts_with(&self.allowed_root) {
                return HookResult::Blocked {
                    reason: format!(
                        "Write blocked: {} is outside project root {}",
                        path.display(),
                        self.allowed_root.display()
                    ),
                };
            }
        }
        HookResult::Continue
    }
}

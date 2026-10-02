//! What a review compares. Every target ends at the checked-out HEAD (or the
//! working tree on top of it): finders read the working tree, so a target
//! whose head is elsewhere is not representable.

/// The Review target the operator picked on the form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewTarget {
    /// HEAD against its upstream, `main` or `master`, plus uncommitted
    /// changes (gather resolves it).
    Auto,
    /// What a PR would contain: `<base>...HEAD`.
    Base(String),
    /// HEAD against the working tree.
    Uncommitted,
    /// The commit at HEAD alone.
    HeadCommit,
}

/// Branches tried, in order, as the default base.
const PREFERRED_BASES: [&str; 4] = ["main", "master", "origin/main", "origin/master"];

impl ReviewTarget {
    /// The target string gather resolves and the workflow is given.
    pub fn spec(&self) -> String {
        match self {
            Self::Auto => "auto".to_owned(),
            Self::Base(base) => format!("{base}...HEAD"),
            Self::Uncommitted => "HEAD".to_owned(),
            Self::HeadCommit => "HEAD~1..HEAD".to_owned(),
        }
    }

    /// How the form names it.
    pub fn label(&self) -> String {
        match self {
            Self::Auto => "auto".to_owned(),
            Self::Base(base) => format!("vs {base}"),
            Self::Uncommitted => "uncommitted changes".to_owned(),
            Self::HeadCommit => "the commit at HEAD".to_owned(),
        }
    }

    /// Whether the diff ends at the HEAD commit, so uncommitted changes the
    /// reviewers will read are not part of it.
    pub fn ends_at_head_commit(&self) -> bool {
        matches!(self, Self::Base(_) | Self::HeadCommit)
    }

    /// The next (or, with `back`, previous) mode. Base mode is offered only
    /// when there is a branch to compare against, at `remembered` if it is
    /// still a branch, else at the preferred one.
    pub fn cycle(&self, branches: &[String], remembered: Option<&str>, back: bool) -> Self {
        let mut modes = vec![Self::Auto];
        let base = remembered
            .filter(|base| branches.iter().any(|branch| branch == base))
            .or_else(|| default_base(branches));
        if let Some(base) = base {
            modes.push(Self::Base(base.to_owned()));
        }
        modes.extend([Self::Uncommitted, Self::HeadCommit]);
        let here = match modes
            .iter()
            .position(|mode| std::mem::discriminant(mode) == std::mem::discriminant(self))
        {
            Some(here) => here,
            None => {
                tracing::debug!(target = %self.spec(), "review: the current target is no longer offered; cycling from auto");
                0
            }
        };
        let step = if back { modes.len() - 1 } else { 1 };
        modes[(here + step) % modes.len()].clone()
    }

    /// In base mode, the next (or previous) branch; other modes are unchanged.
    pub fn cycle_base(&self, branches: &[String], back: bool) -> Self {
        let Self::Base(current) = self else {
            return self.clone();
        };
        let Some(here) = branches.iter().position(|branch| branch == current) else {
            tracing::debug!(base = %current, "review: the base branch is no longer listed; using the default");
            return default_base(branches)
                .map_or_else(|| self.clone(), |base| Self::Base(base.to_owned()));
        };
        let step = if back { branches.len() - 1 } else { 1 };
        Self::Base(branches[(here + step) % branches.len()].clone())
    }
}

fn default_base(branches: &[String]) -> Option<&str> {
    PREFERRED_BASES
        .iter()
        .find(|preferred| branches.iter().any(|branch| branch == *preferred))
        .copied()
        .or_else(|| branches.first().map(String::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branches(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn every_target_is_anchored_at_head() {
        assert_eq!(ReviewTarget::Auto.spec(), "auto");
        assert_eq!(ReviewTarget::Base("main".into()).spec(), "main...HEAD");
        assert_eq!(ReviewTarget::Uncommitted.spec(), "HEAD");
        assert_eq!(ReviewTarget::HeadCommit.spec(), "HEAD~1..HEAD");
    }

    #[test]
    fn modes_cycle_through_base_at_the_preferred_branch() {
        let list = branches(&["feature", "main"]);
        let base = ReviewTarget::Auto.cycle(&list, None, false);
        assert_eq!(base, ReviewTarget::Base("main".into()));
        assert_eq!(base.cycle(&list, None, false), ReviewTarget::Uncommitted);
        assert_eq!(
            ReviewTarget::Uncommitted.cycle(&list, None, false),
            ReviewTarget::HeadCommit
        );
        assert_eq!(
            ReviewTarget::HeadCommit.cycle(&list, None, false),
            ReviewTarget::Auto
        );
        assert_eq!(
            ReviewTarget::Auto.cycle(&list, None, true),
            ReviewTarget::HeadCommit
        );
        assert_eq!(
            base.cycle_base(&list, false),
            ReviewTarget::Base("feature".into())
        );
    }

    #[test]
    fn re_entering_base_mode_keeps_the_remembered_branch() {
        let list = branches(&["main", "release"]);
        assert_eq!(
            ReviewTarget::Auto.cycle(&list, Some("release"), false),
            ReviewTarget::Base("release".into())
        );
        assert_eq!(
            ReviewTarget::Uncommitted.cycle(&list, Some("gone"), true),
            ReviewTarget::Base("main".into()),
            "a remembered branch that no longer exists falls back to the default"
        );
    }

    #[test]
    fn without_branches_base_mode_is_not_offered() {
        assert_eq!(
            ReviewTarget::Auto.cycle(&[], None, false),
            ReviewTarget::Uncommitted
        );
    }
}

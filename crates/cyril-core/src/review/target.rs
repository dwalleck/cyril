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

    /// The next (or, with `back`, previous) mode. Base mode is offered only
    /// when there is a branch to compare against, starting at the preferred
    /// one.
    pub fn cycle(&self, branches: &[String], back: bool) -> Self {
        let mut modes = vec![Self::Auto];
        if let Some(base) = default_base(branches) {
            modes.push(Self::Base(base.to_owned()));
        }
        modes.extend([Self::Uncommitted, Self::HeadCommit]);
        let here = modes
            .iter()
            .position(|mode| std::mem::discriminant(mode) == std::mem::discriminant(self))
            .unwrap_or(0);
        let step = if back { modes.len() - 1 } else { 1 };
        let next = modes[(here + step) % modes.len()].clone();
        // Leaving and re-entering base mode keeps the chosen branch.
        match (self, next) {
            (Self::Base(_), Self::Base(_)) => self.clone(),
            (_, next) => next,
        }
    }

    /// In base mode, the next (or previous) branch; other modes are unchanged.
    pub fn cycle_base(&self, branches: &[String], back: bool) -> Self {
        let Self::Base(current) = self else {
            return self.clone();
        };
        let Some(here) = branches.iter().position(|branch| branch == current) else {
            return self.clone();
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
        let base = ReviewTarget::Auto.cycle(&list, false);
        assert_eq!(base, ReviewTarget::Base("main".into()));
        assert_eq!(base.cycle(&list, false), ReviewTarget::Uncommitted);
        assert_eq!(
            ReviewTarget::Uncommitted.cycle(&list, false),
            ReviewTarget::HeadCommit
        );
        assert_eq!(
            ReviewTarget::HeadCommit.cycle(&list, false),
            ReviewTarget::Auto
        );
        assert_eq!(
            ReviewTarget::Auto.cycle(&list, true),
            ReviewTarget::HeadCommit
        );
        assert_eq!(
            base.cycle_base(&list, false),
            ReviewTarget::Base("feature".into())
        );
    }

    #[test]
    fn without_branches_base_mode_is_not_offered() {
        assert_eq!(
            ReviewTarget::Auto.cycle(&[], false),
            ReviewTarget::Uncommitted
        );
    }
}

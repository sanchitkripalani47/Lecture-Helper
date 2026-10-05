use crate::domain::models::ExistingOutputPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingOutputDecision {
    /// Produce the PDF, replacing any existing file.
    Process,
    /// Leave the existing PDF untouched and mark the job skipped.
    Skip,
}

/// Decide what to do for a job whose destination may already exist.
///
/// Skipping is the default: existing PDFs are never overwritten unless the user
/// explicitly asks for it.
pub fn should_skip_existing(
    policy: ExistingOutputPolicy,
    output_exists: bool,
) -> ExistingOutputDecision {
    match (policy, output_exists) {
        (ExistingOutputPolicy::Skip, true) => ExistingOutputDecision::Skip,
        _ => ExistingOutputDecision::Process,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_policy_leaves_existing_output_alone() {
        assert_eq!(
            should_skip_existing(ExistingOutputPolicy::Skip, true),
            ExistingOutputDecision::Skip
        );
    }

    #[test]
    fn skip_policy_processes_missing_output() {
        assert_eq!(
            should_skip_existing(ExistingOutputPolicy::Skip, false),
            ExistingOutputDecision::Process
        );
    }

    #[test]
    fn replace_policy_processes_existing_output() {
        assert_eq!(
            should_skip_existing(ExistingOutputPolicy::Replace, true),
            ExistingOutputDecision::Process
        );
    }
}

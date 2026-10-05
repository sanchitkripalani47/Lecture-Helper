use crate::domain::models::{BatchSummary, JobStatus, VideoJob};
use std::path::Path;

/// Aggregate the queue into the numbers shown on the summary screen.
pub fn compute_summary(jobs: &[VideoJob], output_root: &Path) -> BatchSummary {
    let mut summary = BatchSummary {
        found: jobs.len(),
        completed: 0,
        failed: 0,
        skipped: 0,
        cancelled: 0,
        slides_detected: 0,
        output_root: output_root.to_path_buf(),
    };

    for job in jobs {
        match job.status {
            JobStatus::Completed => summary.completed += 1,
            JobStatus::Failed => summary.failed += 1,
            JobStatus::Skipped => summary.skipped += 1,
            JobStatus::Cancelled => summary.cancelled += 1,
            JobStatus::Pending | JobStatus::Processing => {}
        }
        summary.slides_detected += job.slides_detected;
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn job(name: &str, status: JobStatus, slides: u32) -> VideoJob {
        let mut job = VideoJob::new(
            PathBuf::from(format!("/in/{name}")),
            PathBuf::from(name),
            PathBuf::from(format!("/out/{name}.pdf")),
            false,
        );
        job.status = status;
        job.slides_detected = slides;
        job
    }

    #[test]
    fn counts_every_status_and_totals_slides() {
        let jobs = vec![
            job("a.mp4", JobStatus::Completed, 40),
            job("b.mp4", JobStatus::Failed, 0),
            job("c.mp4", JobStatus::Skipped, 0),
            job("d.mp4", JobStatus::Cancelled, 0),
            job("e.mp4", JobStatus::Pending, 0),
        ];

        let summary = compute_summary(&jobs, Path::new("/out"));
        assert_eq!(summary.found, 5);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.cancelled, 1);
        assert_eq!(summary.slides_detected, 40);
        assert_eq!(summary.output_root, PathBuf::from("/out"));
    }
}

use crate::batch::summary::compute_summary;
use crate::domain::models::{BatchEvent, BatchOptions, BatchSummary, JobStatus, VideoJob};
use crate::domain::policies::{should_skip_existing, ExistingOutputDecision};
use crate::domain::ProcessingError;
use crate::processing::cancellation::CancellationToken;
use crate::processing::processor::VideoProcessor;
use crate::processing::progress::ProgressReporter;

/// Receives batch events as they happen. Tauri implements this by emitting to
/// the webview; tests implement it by recording.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: BatchEvent);
}

#[derive(Debug, Clone)]
pub struct BatchOutcome {
    pub summary: BatchSummary,
    pub cancelled: bool,
}

struct SinkReporter<'a> {
    sink: &'a dyn EventSink,
    job_id: String,
}

impl ProgressReporter for SinkReporter<'_> {
    fn report(&self, progress: u8, stage: &str, slides_detected: Option<u32>) {
        self.sink.emit(BatchEvent::JobProgress {
            job_id: self.job_id.clone(),
            progress,
            stage: stage.to_string(),
            slides_detected,
        });
    }
}

/// Run the queue sequentially.
///
/// One video is in flight at a time. A failure is recorded and the batch moves
/// on; cancellation stops new work, lets the current video finish cleanly, and
/// leaves already-written PDFs in place.
pub fn run_batch(
    mut jobs: Vec<VideoJob>,
    processor: &dyn VideoProcessor,
    options: &BatchOptions,
    cancellation: &CancellationToken,
    sink: &dyn EventSink,
) -> BatchOutcome {
    let mut cancelled_run = false;

    for job in jobs.iter_mut() {
        if cancellation.is_cancelled() {
            cancelled_run = true;
            break;
        }

        let decision = should_skip_existing(options.existing_output_policy, job.output_exists);

        if decision == ExistingOutputDecision::Skip {
            job.status = JobStatus::Skipped;
            job.stage = "Output already exists".to_string();
            job.progress = 100;
            sink.emit(BatchEvent::JobSkipped {
                job_id: job.id.clone(),
                reason: "Output already exists".to_string(),
            });
            continue;
        }

        job.status = JobStatus::Processing;
        job.stage = "Starting".to_string();
        job.progress = 0;
        job.error = None;

        let job_id = job.id.clone();
        sink.emit(BatchEvent::JobStarted {
            job_id: job_id.clone(),
            stage: "Starting".to_string(),
        });

        let reporter = SinkReporter {
            sink,
            job_id: job_id.clone(),
        };

        match processor.process(job, &reporter, cancellation) {
            Ok(result) => {
                job.status = JobStatus::Completed;
                job.stage = "Completed".to_string();
                job.progress = 100;
                job.slides_detected = result.slides_detected;
                if let Some(path) = &result.slides_pdf {
                    job.output_path = path.clone();
                }
                sink.emit(BatchEvent::JobCompleted { job_id, result });
            }
            Err(ProcessingError::Cancelled) => {
                cancelled_run = true;
                break;
            }
            Err(error) => {
                let message = error.to_string();
                job.status = JobStatus::Failed;
                job.stage = "Failed".to_string();
                job.error = Some(message.clone());
                sink.emit(BatchEvent::JobFailed {
                    job_id,
                    error: message,
                });
            }
        }
    }

    if cancelled_run || cancellation.is_cancelled() {
        for job in jobs.iter_mut() {
            if matches!(job.status, JobStatus::Pending | JobStatus::Processing) {
                job.status = JobStatus::Cancelled;
                job.stage = "Not processed".to_string();
            }
        }

        let completed = jobs
            .iter()
            .filter(|job| job.status == JobStatus::Completed)
            .count();
        let remaining = jobs
            .iter()
            .filter(|job| job.status == JobStatus::Cancelled)
            .count();

        sink.emit(BatchEvent::BatchCancelled {
            completed,
            remaining,
        });

        return BatchOutcome {
            summary: compute_summary(&jobs, &options.output_root),
            cancelled: true,
        };
    }

    let summary = compute_summary(&jobs, &options.output_root);
    sink.emit(BatchEvent::BatchCompleted {
        summary: summary.clone(),
    });

    BatchOutcome {
        summary,
        cancelled: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{ExistingOutputPolicy, ProcessingResult};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingSink {
        events: Mutex<Vec<BatchEvent>>,
    }

    impl RecordingSink {
        fn events(&self) -> Vec<BatchEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EventSink for RecordingSink {
        fn emit(&self, event: BatchEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    /// Configurable processor used to prove the coordinator's contract:
    /// sequential order, continue-on-failure, skip behaviour, and cancellation.
    struct FakeProcessor {
        fail_on: Vec<String>,
        processed: Mutex<Vec<String>>,
        progress: Mutex<Vec<u8>>,
    }

    impl FakeProcessor {
        fn new() -> Self {
            Self {
                fail_on: Vec::new(),
                processed: Mutex::new(Vec::new()),
                progress: Mutex::new(Vec::new()),
            }
        }

        fn processed(&self) -> Vec<String> {
            self.processed.lock().unwrap().clone()
        }
    }

    impl VideoProcessor for FakeProcessor {
        fn process(
            &self,
            job: &VideoJob,
            reporter: &dyn ProgressReporter,
            cancellation: &CancellationToken,
        ) -> Result<ProcessingResult, ProcessingError> {
            self.processed.lock().unwrap().push(job.id.clone());

            reporter.report(10, "Extracting frames", None);
            reporter.report(60, "Detecting slides", Some(12));

            if cancellation.is_cancelled() {
                return Err(ProcessingError::Cancelled);
            }

            if self.fail_on.contains(&job.id) {
                return Err(ProcessingError::DecodeFailed {
                    path: job.source_path.clone(),
                    detail: "unsupported codec".to_string(),
                });
            }

            reporter.report(100, "Completed", Some(42));
            self.progress.lock().unwrap().extend([10u8, 60, 100]);

            Ok(ProcessingResult::with_slides(
                job.source_path.clone(),
                job.output_path.clone(),
                42,
            ))
        }
    }

    /// Processor that cancels the shared token partway through the batch.
    struct CancellingProcessor {
        cancel_on: String,
        token: Arc<CancellationToken>,
        processed: Mutex<Vec<String>>,
    }

    impl VideoProcessor for CancellingProcessor {
        fn process(
            &self,
            job: &VideoJob,
            _reporter: &dyn ProgressReporter,
            _cancellation: &CancellationToken,
        ) -> Result<ProcessingResult, ProcessingError> {
            self.processed.lock().unwrap().push(job.id.clone());

            if job.id == self.cancel_on {
                self.token.cancel();
                return Err(ProcessingError::Cancelled);
            }

            Ok(ProcessingResult::with_slides(
                job.source_path.clone(),
                job.output_path.clone(),
                5,
            ))
        }
    }

    fn job(name: &str, exists: bool) -> VideoJob {
        VideoJob::new(
            PathBuf::from(format!("/in/{name}")),
            PathBuf::from(name),
            PathBuf::from(format!("/out/{name}.pdf")),
            exists,
        )
    }

    fn options(policy: ExistingOutputPolicy) -> BatchOptions {
        BatchOptions {
            input_root: PathBuf::from("/in"),
            output_root: PathBuf::from("/out"),
            existing_output_policy: policy,
        }
    }

    #[test]
    fn processes_jobs_sequentially_in_order() {
        let processor = FakeProcessor::new();
        let sink = RecordingSink::default();
        let jobs = vec![
            job("a.mp4", false),
            job("b.mp4", false),
            job("c.mp4", false),
        ];

        let outcome = run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Skip),
            &CancellationToken::new(),
            &sink,
        );

        assert_eq!(processor.processed(), vec!["a.mp4", "b.mp4", "c.mp4"]);
        assert_eq!(outcome.summary.completed, 3);
        assert!(!outcome.cancelled);
    }

    #[test]
    fn emits_started_progress_and_completed_for_every_job() {
        let processor = FakeProcessor::new();
        let sink = RecordingSink::default();
        let jobs = vec![job("a.mp4", false)];

        run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Skip),
            &CancellationToken::new(),
            &sink,
        );

        let events = sink.events();
        assert!(matches!(
            events.first(),
            Some(BatchEvent::JobStarted { .. })
        ));
        assert!(events
            .iter()
            .any(|event| matches!(event, BatchEvent::JobProgress { .. })));
        assert!(matches!(
            events.last(),
            Some(BatchEvent::BatchCompleted { .. })
        ));
    }

    #[test]
    fn progress_values_never_go_backwards() {
        let processor = FakeProcessor::new();
        let sink = RecordingSink::default();
        let jobs = vec![job("a.mp4", false)];

        run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Skip),
            &CancellationToken::new(),
            &sink,
        );

        let reported: Vec<u8> = sink
            .events()
            .iter()
            .filter_map(|event| match event {
                BatchEvent::JobProgress { progress, .. } => Some(*progress),
                _ => None,
            })
            .collect();

        assert!(!reported.is_empty());
        assert!(reported.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn a_failed_job_does_not_stop_later_jobs() {
        let mut processor = FakeProcessor::new();
        processor.fail_on = vec!["b.mp4".to_string()];

        let sink = RecordingSink::default();
        let jobs = vec![
            job("a.mp4", false),
            job("b.mp4", false),
            job("c.mp4", false),
        ];

        let outcome = run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Skip),
            &CancellationToken::new(),
            &sink,
        );

        assert_eq!(processor.processed(), vec!["a.mp4", "b.mp4", "c.mp4"]);
        assert_eq!(outcome.summary.completed, 2);
        assert_eq!(outcome.summary.failed, 1);

        let failure = sink.events().into_iter().find_map(|event| match event {
            BatchEvent::JobFailed { error, .. } => Some(error),
            _ => None,
        });
        assert!(failure
            .expect("a failure event")
            .contains("unsupported codec"));
    }

    #[test]
    fn skips_existing_output_by_default() {
        let processor = FakeProcessor::new();
        let sink = RecordingSink::default();
        let jobs = vec![job("a.mp4", true), job("b.mp4", false)];

        let outcome = run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Skip),
            &CancellationToken::new(),
            &sink,
        );

        assert_eq!(processor.processed(), vec!["b.mp4"]);
        assert_eq!(outcome.summary.skipped, 1);
        assert_eq!(outcome.summary.completed, 1);
    }

    #[test]
    fn replaces_existing_output_when_asked() {
        let processor = FakeProcessor::new();
        let sink = RecordingSink::default();
        let jobs = vec![job("a.mp4", true)];

        let outcome = run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Replace),
            &CancellationToken::new(),
            &sink,
        );

        assert_eq!(processor.processed(), vec!["a.mp4"]);
        assert_eq!(outcome.summary.completed, 1);
        assert_eq!(outcome.summary.skipped, 0);
    }

    #[test]
    fn cancellation_stops_new_jobs_and_marks_the_rest() {
        let token = Arc::new(CancellationToken::new());
        let processor = CancellingProcessor {
            cancel_on: "b.mp4".to_string(),
            token: Arc::clone(&token),
            processed: Mutex::new(Vec::new()),
        };
        let sink = RecordingSink::default();
        let jobs = vec![
            job("a.mp4", false),
            job("b.mp4", false),
            job("c.mp4", false),
        ];

        let outcome = run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Skip),
            &token,
            &sink,
        );

        assert!(outcome.cancelled);
        assert_eq!(
            processor.processed.lock().unwrap().clone(),
            vec!["a.mp4", "b.mp4"]
        );
        assert_eq!(outcome.summary.completed, 1);
        assert_eq!(outcome.summary.cancelled, 2);

        let cancelled = sink
            .events()
            .into_iter()
            .find(|event| matches!(event, BatchEvent::BatchCancelled { .. }));
        match cancelled {
            Some(BatchEvent::BatchCancelled {
                completed,
                remaining,
            }) => {
                assert_eq!(completed, 1);
                assert_eq!(remaining, 2);
            }
            _ => panic!("expected a BatchCancelled event"),
        }
    }

    #[test]
    fn a_pre_cancelled_token_processes_nothing() {
        let token = CancellationToken::new();
        token.cancel();

        let processor = FakeProcessor::new();
        let sink = RecordingSink::default();
        let jobs = vec![job("a.mp4", false), job("b.mp4", false)];

        let outcome = run_batch(
            jobs,
            &processor,
            &options(ExistingOutputPolicy::Skip),
            &token,
            &sink,
        );

        assert!(processor.processed().is_empty());
        assert_eq!(outcome.summary.cancelled, 2);
        assert!(outcome.cancelled);
    }
}

use crate::domain::{ProcessingError, ProcessingResult, VideoJob};
use crate::processing::cancellation::CancellationToken;
use crate::processing::progress::ProgressReporter;

/// Converts a single video into one or more artifacts.
///
/// The queue never assumes a video produces exactly one PDF: `ProcessingResult`
/// is what the coordinator records, so transcripts, OCR text, or extra JSON can
/// be added later without touching the batch layer.
pub trait VideoProcessor: Send + Sync {
    fn process(
        &self,
        job: &VideoJob,
        reporter: &dyn ProgressReporter,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingResult, ProcessingError>;

    /// Short identifier used in logs and diagnostics.
    fn name(&self) -> &'static str {
        "video-processor"
    }
}

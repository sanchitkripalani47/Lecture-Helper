use crate::domain::{ProcessingError, ProcessingResult, VideoJob};
use crate::infrastructure::TempDir;
use crate::processing::cancellation::CancellationToken;
use crate::processing::media::{detect_slides, MediaTools, SlideDetectionOptions};
use crate::processing::pdf::{write_pdf, SlideImage};
use crate::processing::processor::VideoProcessor;
use crate::processing::progress::ProgressReporter;
use crate::processing::temp_outputs::TempOutput;

/// V1 processor: sample frames, detect slide changes, and emit one PDF page per
/// detected slide.
///
/// Progress is derived from ffmpeg's own output and from the frame loop, so the
/// numbers reflect real work rather than an animation.
pub struct PdfProcessor {
    tools: MediaTools,
    detection: SlideDetectionOptions,
}

impl PdfProcessor {
    pub fn new(tools: MediaTools) -> Self {
        Self {
            tools,
            detection: SlideDetectionOptions::default(),
        }
    }

    pub fn with_detection(tools: MediaTools, detection: SlideDetectionOptions) -> Self {
        Self { tools, detection }
    }
}

/// Map a stage-local percentage into its slice of the overall 0-100 bar.
fn band(start: u8, end: u8, value: u8) -> u8 {
    let span = end.saturating_sub(start) as u32;
    (start as u32 + (value.min(100) as u32 * span / 100)) as u8
}

impl VideoProcessor for PdfProcessor {
    fn name(&self) -> &'static str {
        "pdf-processor"
    }

    fn process(
        &self,
        job: &VideoJob,
        reporter: &dyn ProgressReporter,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingResult, ProcessingError> {
        let is_cancelled = || cancellation.is_cancelled();

        reporter.report(2, "Reading video", None);
        let duration = self.tools.duration_seconds(&job.source_path)?;
        if cancellation.is_cancelled() {
            return Err(ProcessingError::Cancelled);
        }

        let scratch = TempDir::new("lecture-helper-frames").map_err(|source| {
            ProcessingError::InputUnreadable {
                path: job.source_path.clone(),
                source,
            }
        })?;

        let frames = self.tools.extract_frames(
            &job.source_path,
            scratch.path(),
            &self.detection,
            duration,
            &is_cancelled,
            |percent| reporter.report(band(5, 45, percent), "Extracting frames", None),
        )?;

        if cancellation.is_cancelled() {
            return Err(ProcessingError::Cancelled);
        }

        let slides = detect_slides(&frames, &self.detection, &is_cancelled, |percent, count| {
            reporter.report(band(45, 80, percent), "Detecting slides", Some(count));
        })?;

        if cancellation.is_cancelled() {
            return Err(ProcessingError::Cancelled);
        }

        let slide_count = slides.len() as u32;
        reporter.report(82, "Assembling PDF", Some(slide_count));

        let mut images = Vec::with_capacity(slides.len());
        for slide in &slides {
            images.push(SlideImage::from_path(slide.clone()).map_err(|source| {
                ProcessingError::OutputWrite {
                    path: job.output_path.clone(),
                    source,
                }
            })?);
        }

        // Never write to the destination directly; only commit once the file is
        // complete so a failure or cancellation cannot corrupt an existing PDF.
        let temp_output = TempOutput::new(&job.output_path)?;

        if cancellation.is_cancelled() {
            return Err(ProcessingError::Cancelled);
        }

        let written = write_pdf(temp_output.temp_path(), &images).map_err(|source| {
            ProcessingError::OutputWrite {
                path: temp_output.temp_path().to_path_buf(),
                source,
            }
        })?;

        reporter.report(97, "Finalizing", Some(written));

        if cancellation.is_cancelled() {
            return Err(ProcessingError::Cancelled);
        }

        let final_path = temp_output.commit()?;
        reporter.report(100, "Completed", Some(written));

        Ok(ProcessingResult::with_slides(
            job.source_path.clone(),
            final_path,
            written,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_map_into_their_slice() {
        assert_eq!(band(5, 45, 0), 5);
        assert_eq!(band(5, 45, 100), 45);
        assert_eq!(band(45, 80, 50), 62);
        assert_eq!(band(5, 45, 200), 45);
    }
}

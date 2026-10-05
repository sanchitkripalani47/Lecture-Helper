use crate::domain::models::ProgressUpdate;

/// Receives real progress from a processor. Implementations must be cheap:
/// they are called between processing steps, not per pixel.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: u8, stage: &str, slides_detected: Option<u32>);

    fn report_update(&self, update: ProgressUpdate) {
        self.report(update.progress, &update.stage, update.slides_detected);
    }
}

/// Reporter used in tests and in paths that do not stream events.
pub struct NullReporter;

impl ProgressReporter for NullReporter {
    fn report(&self, _progress: u8, _stage: &str, _slides_detected: Option<u32>) {}
}

/// Clamp arbitrary ratios into the `0..=100` range the UI expects.
pub fn percentage(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    let ratio = (done as f64 / total as f64).clamp(0.0, 1.0);
    (ratio * 100.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_and_rounds() {
        assert_eq!(percentage(0, 10), 0);
        assert_eq!(percentage(5, 10), 50);
        assert_eq!(percentage(10, 10), 100);
        assert_eq!(percentage(20, 10), 100);
        assert_eq!(percentage(3, 0), 0);
    }
}

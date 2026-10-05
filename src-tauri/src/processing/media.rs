use crate::domain::ProcessingError;
use crate::infrastructure::filesystem::sorted_files;
use crate::infrastructure::subprocess::{external_failure, run_capture, run_streaming, which};
use crate::processing::progress::percentage;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// How frames are sampled and how slide boundaries are decided.
#[derive(Debug, Clone)]
pub struct SlideDetectionOptions {
    /// Seconds between sampled frames.
    pub sample_interval_seconds: f64,
    /// Mean absolute luma difference (0-255) at which two frames show
    /// genuinely different content. Used to tell two slides apart.
    pub difference_threshold: f64,
    /// Below this, consecutive frames count as the same, settled image. A
    /// fading transition keeps the picture moving, so it never settles.
    pub stability_threshold: f64,
    /// A settled group shorter than this many samples is treated as a
    /// transition rather than a slide. At the default sampling rate this means
    /// a slide must be on screen for roughly a couple of seconds.
    pub min_slide_samples: usize,
    /// Luma spread below which a frame is considered blank (black, white, or
    /// uniform), as produced by fading in from an empty screen.
    pub blank_threshold: f64,
    /// Side length of the downscaled comparison signature.
    pub signature_size: u32,
    /// MJPEG quality for the extracted frames, where lower is better (1-31).
    pub frame_quality: u8,
}

impl Default for SlideDetectionOptions {
    fn default() -> Self {
        Self {
            sample_interval_seconds: 1.0,
            difference_threshold: 8.0,
            stability_threshold: 4.0,
            min_slide_samples: 2,
            blank_threshold: 3.0,
            signature_size: 32,
            frame_quality: 2,
        }
    }
}

/// Resolved ffmpeg/ffprobe locations.
#[derive(Debug, Clone)]
pub struct MediaTools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

/// How to obtain ffmpeg on this platform, shown when it cannot be found.
#[cfg(target_os = "windows")]
const FFMPEG_HINT: &str =
    "Install ffmpeg (the builds from gyan.dev work well), add its bin folder to your PATH, then reopen the app.";

#[cfg(target_os = "macos")]
const FFMPEG_HINT: &str = "Install it with Homebrew: brew install ffmpeg";

#[cfg(all(unix, not(target_os = "macos")))]
const FFMPEG_HINT: &str =
    "Install it with your package manager, for example: sudo apt install ffmpeg";

impl MediaTools {
    pub fn discover() -> Result<Self, ProcessingError> {
        let ffmpeg = which("ffmpeg").ok_or_else(|| ProcessingError::DependencyMissing {
            program: "ffmpeg".to_string(),
            hint: FFMPEG_HINT.to_string(),
        })?;
        let ffprobe = which("ffprobe").ok_or_else(|| ProcessingError::DependencyMissing {
            program: "ffprobe".to_string(),
            hint: FFMPEG_HINT.to_string(),
        })?;
        Ok(Self { ffmpeg, ffprobe })
    }

    /// Duration in seconds, used to turn ffmpeg progress into a real percentage.
    pub fn duration_seconds(&self, input: &Path) -> Result<f64, ProcessingError> {
        let mut args: Vec<OsString> = [
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        args.push(input.as_os_str().to_os_string());

        let result = run_capture(&self.ffprobe, &args)?;
        if result.code != Some(0) {
            return Err(external_failure("ffprobe", &result));
        }

        result
            .stdout
            .trim()
            .parse::<f64>()
            .map_err(|_| ProcessingError::DecodeFailed {
                path: input.to_path_buf(),
                detail: "the video duration could not be read".to_string(),
            })
    }

    /// Extract frames at `options.sample_interval_seconds` into `output_dir`.
    pub fn extract_frames<F>(
        &self,
        input: &Path,
        output_dir: &Path,
        options: &SlideDetectionOptions,
        duration_seconds: f64,
        is_cancelled: &dyn Fn() -> bool,
        mut on_progress: F,
    ) -> Result<Vec<PathBuf>, ProcessingError>
    where
        F: FnMut(u8),
    {
        let pattern = output_dir.join("frame_%06d.jpg");
        let fps = 1.0 / options.sample_interval_seconds.max(0.05);

        let args: Vec<OsString> = vec![
            OsString::from("-nostdin"),
            OsString::from("-hide_banner"),
            OsString::from("-loglevel"),
            OsString::from("error"),
            OsString::from("-i"),
            input.as_os_str().to_os_string(),
            OsString::from("-vf"),
            OsString::from(format!("fps={fps}")),
            OsString::from("-q:v"),
            OsString::from(options.frame_quality.clamp(1, 31).to_string()),
            // 4:4:4 keeps full colour resolution. The default 4:2:0 halves it,
            // which smears coloured text, syntax highlighting, and diagrams.
            OsString::from("-pix_fmt"),
            OsString::from("yuvj444p"),
            OsString::from("-progress"),
            OsString::from("pipe:1"),
            OsString::from("-nostats"),
            pattern.as_os_str().to_os_string(),
        ];

        let result = run_streaming(&self.ffmpeg, &args, is_cancelled, |line| {
            if let Some(seconds) = parse_progress_seconds(line) {
                if duration_seconds > 0.0 {
                    let ratio = (seconds / duration_seconds).clamp(0.0, 1.0);
                    on_progress((ratio * 100.0).round() as u8);
                }
            }
        })?;

        if result.code != Some(0) {
            return Err(external_failure("ffmpeg", &result));
        }

        let frames = sorted_files(output_dir)?;
        if frames.is_empty() {
            return Err(ProcessingError::DecodeFailed {
                path: input.to_path_buf(),
                detail: "no frames could be extracted".to_string(),
            });
        }

        Ok(frames)
    }
}

/// ffmpeg reports `out_time_us` (and historically `out_time_ms`) in microseconds.
fn parse_progress_seconds(line: &str) -> Option<f64> {
    let (key, value) = line.split_once('=')?;
    if key != "out_time_us" && key != "out_time_ms" {
        return None;
    }
    value
        .trim()
        .parse::<f64>()
        .ok()
        .map(|micros| micros / 1_000_000.0)
}

/// Pick one representative frame per slide.
///
/// A slide is a stretch where the picture *settles*. A fade, wipe, or other
/// transition keeps the picture moving, so it never settles and produces no
/// slide. That is what stops half-faded frames reaching the PDF.
pub fn detect_slides<F>(
    frames: &[PathBuf],
    options: &SlideDetectionOptions,
    is_cancelled: &dyn Fn() -> bool,
    mut on_progress: F,
) -> Result<Vec<PathBuf>, ProcessingError>
where
    F: FnMut(u8, u32),
{
    let total = frames.len().max(1) as u64;
    let mut signatures: Vec<Vec<u8>> = Vec::with_capacity(frames.len());

    for (index, frame) in frames.iter().enumerate() {
        if is_cancelled() {
            return Err(ProcessingError::Cancelled);
        }
        signatures.push(frame_signature(frame, options.signature_size)?);
        // Leave the last fifth of the bar for grouping and filtering. Widened
        // because a u8 percentage times 80 would overflow.
        let scanned = u32::from(percentage(index as u64 + 1, total));
        on_progress((scanned * 80 / 100) as u8, 0);
    }

    let selected = select_slide_indices(&signatures, options);
    if selected.is_empty() {
        return Err(ProcessingError::DecodeFailed {
            path: frames
                .first()
                .cloned()
                .unwrap_or_else(|| PathBuf::from("<frames>")),
            detail: "no slides were detected".to_string(),
        });
    }

    let kept: Vec<PathBuf> = selected
        .iter()
        .map(|index| frames[*index].clone())
        .collect();
    on_progress(100, kept.len() as u32);
    Ok(kept)
}

/// Choose which sampled frames represent slides.
///
/// Kept separate from decoding so the decision logic can be tested directly.
fn select_slide_indices(signatures: &[Vec<u8>], options: &SlideDetectionOptions) -> Vec<usize> {
    if signatures.is_empty() {
        return Vec::new();
    }

    let differences = consecutive_differences(signatures);

    // 1. Group consecutive frames that hold still. Every time the picture is
    //    still moving, the group is broken, so a transition splits into tiny
    //    groups instead of becoming a slide.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, difference) in differences.iter().enumerate() {
        if *difference >= options.stability_threshold || groups.is_empty() {
            groups.push(vec![index]);
        } else if let Some(current) = groups.last_mut() {
            current.push(index);
        }
    }

    // 2. Re-join neighbours that show the same slide. A moving cursor or a
    //    blinking element can split one slide into several still groups.
    let mut merged: Vec<Vec<usize>> = Vec::new();
    for group in groups {
        let continues_previous = merged.last().is_some_and(|last| {
            let previous = representative_of(last, &differences);
            let current = representative_of(&group, &differences);
            mean_absolute_difference(&signatures[previous], &signatures[current])
                < options.difference_threshold
        });

        if continues_previous {
            if let Some(last) = merged.last_mut() {
                last.extend(group);
            }
        } else {
            merged.push(group);
        }
    }

    // 3. Keep only groups that are long enough to be a slide and are not blank.
    let mut selected: Vec<usize> = Vec::new();
    for group in &merged {
        if group.len() < options.min_slide_samples {
            continue;
        }
        let index = representative_of(group, &differences);
        if luma_spread(&signatures[index]) < options.blank_threshold {
            continue;
        }
        selected.push(index);
    }

    // 4. A video that is entirely blank or entirely in motion would otherwise
    //    produce no pages at all; fall back to its longest settled stretch.
    if selected.is_empty() {
        if let Some(longest) = merged.iter().max_by_key(|group| group.len()) {
            selected.push(representative_of(longest, &differences));
        }
    }

    selected
}

/// How much each frame differs from the one before it. The first frame has no
/// predecessor, so it is treated as a change.
fn consecutive_differences(signatures: &[Vec<u8>]) -> Vec<f64> {
    (0..signatures.len())
        .map(|index| {
            if index == 0 {
                f64::INFINITY
            } else {
                mean_absolute_difference(&signatures[index - 1], &signatures[index])
            }
        })
        .collect()
}

/// The most settled frame of a group.
///
/// The first frame of a group is the one that just changed (that is why the
/// group started), so it is only chosen when the group holds a single frame.
fn representative_of(group: &[usize], differences: &[f64]) -> usize {
    let Some((&first, rest)) = group.split_first() else {
        return 0;
    };
    if rest.is_empty() {
        return first;
    }

    rest.iter()
        .copied()
        .min_by(|left, right| differences[*left].total_cmp(&differences[*right]))
        .unwrap_or(first)
}

/// Standard deviation of a signature, used to spot blank frames.
fn luma_spread(signature: &[u8]) -> f64 {
    if signature.is_empty() {
        return 0.0;
    }

    let count = signature.len() as f64;
    let mean = signature.iter().map(|value| *value as f64).sum::<f64>() / count;
    let variance = signature
        .iter()
        .map(|value| {
            let delta = *value as f64 - mean;
            delta * delta
        })
        .sum::<f64>()
        / count;
    variance.sqrt()
}

fn frame_signature(path: &Path, size: u32) -> Result<Vec<u8>, ProcessingError> {
    let image = image::open(path).map_err(|error| ProcessingError::DecodeFailed {
        path: path.to_path_buf(),
        detail: error.to_string(),
    })?;
    let gray = image.to_luma8();
    let small = image::imageops::resize(&gray, size, size, image::imageops::FilterType::Triangle);
    Ok(small.into_raw())
}

fn mean_absolute_difference(left: &[u8], right: &[u8]) -> f64 {
    if left.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let total: u64 = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64)
        .sum();
    total as f64 / left.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_microsecond_progress_fields() {
        assert_eq!(parse_progress_seconds("out_time_us=1500000"), Some(1.5));
        assert_eq!(parse_progress_seconds("out_time_ms=2000000"), Some(2.0));
        assert_eq!(parse_progress_seconds("progress=continue"), None);
    }

    #[test]
    fn identical_signatures_are_never_a_new_slide() {
        let signature = vec![10u8; 32 * 32];
        assert_eq!(mean_absolute_difference(&signature, &signature), 0.0);
    }

    #[test]
    fn large_signature_change_exceeds_the_default_threshold() {
        let left = vec![0u8; 64];
        let right = vec![255u8; 64];
        assert!(
            mean_absolute_difference(&left, &right)
                > SlideDetectionOptions::default().difference_threshold
        );
    }

    #[test]
    fn mismatched_signatures_compare_as_equal() {
        assert_eq!(mean_absolute_difference(&[1, 2], &[1, 2, 3]), 0.0);
    }

    /// A non-blank frame whose brightness tracks `level`, so the difference
    /// between two frames equals the difference between their levels.
    fn frame_at(level: u8) -> Vec<u8> {
        let high = level.saturating_add(16);
        (0..16)
            .map(|cell| if cell % 2 == 0 { level } else { high })
            .collect()
    }

    /// A uniform frame: the sort of thing a fade from black passes through.
    fn blank(level: u8) -> Vec<u8> {
        vec![level; 16]
    }

    fn default_options() -> SlideDetectionOptions {
        SlideDetectionOptions::default()
    }

    #[test]
    fn hard_cuts_produce_one_slide_each() {
        // Three samples of slide A, then three of slide B.
        let signatures = vec![
            frame_at(40),
            frame_at(40),
            frame_at(40),
            frame_at(200),
            frame_at(200),
            frame_at(200),
        ];

        let selected = select_slide_indices(&signatures, &default_options());
        assert_eq!(selected.len(), 2);
        assert!(selected[0] < 3, "first slide comes from the first group");
        assert!(selected[1] >= 3, "second slide comes from the second group");
    }

    #[test]
    fn a_frame_caught_mid_transition_is_not_a_slide() {
        // A fade between two slides is sampled once, mid-way through.
        let signatures = vec![
            frame_at(40),
            frame_at(40),
            frame_at(120),
            frame_at(200),
            frame_at(200),
        ];

        let selected = select_slide_indices(&signatures, &default_options());
        assert_eq!(selected.len(), 2, "only the two real slides survive");
        assert!(
            !selected.contains(&2),
            "the mid-fade frame must not become a page"
        );
    }

    #[test]
    fn a_slow_fade_does_not_become_a_run_of_slides() {
        // Every sample during the fade keeps changing, so none of them settle.
        let signatures = vec![
            frame_at(20),
            frame_at(20),
            frame_at(60),
            frame_at(100),
            frame_at(140),
            frame_at(180),
            frame_at(200),
            frame_at(200),
        ];

        let selected = select_slide_indices(&signatures, &default_options());
        assert_eq!(selected.len(), 2, "the fade contributes nothing");
        assert!(selected[0] < 2);
        assert_eq!(selected[1], 7);
    }

    #[test]
    fn a_small_on_slide_change_does_not_split_a_slide() {
        // A moving cursor, sampled as a modest difference each time. Every
        // sample is equally settled, so any of them may represent the slide;
        // what matters is that the four samples collapse into one page.
        let signatures = vec![frame_at(100), frame_at(105), frame_at(110), frame_at(115)];

        let selected = select_slide_indices(&signatures, &default_options());
        assert_eq!(selected.len(), 1, "one slide, not four");
        assert!(selected[0] < signatures.len());
    }

    #[test]
    fn a_blank_frame_is_skipped() {
        // Fading in from black: the black sample is not a slide.
        let signatures = vec![blank(0), blank(0), frame_at(200), frame_at(200)];

        let selected = select_slide_indices(&signatures, &default_options());
        assert_eq!(selected.len(), 1);
        assert!(selected[0] >= 2);
    }

    #[test]
    fn an_all_blank_video_still_yields_a_page() {
        let signatures = vec![blank(0), blank(0), blank(0)];

        let selected = select_slide_indices(&signatures, &default_options());
        assert_eq!(selected.len(), 1, "never return nothing at all");
    }

    #[test]
    fn a_single_frame_video_is_not_a_transition() {
        let signatures = vec![frame_at(120)];
        let selected = select_slide_indices(&signatures, &default_options());
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn no_frames_selects_no_slides() {
        assert!(select_slide_indices(&[], &default_options()).is_empty());
    }

    #[test]
    fn the_most_settled_frame_represents_a_group() {
        // Differences: the second sample is the one that stopped moving.
        let differences = vec![f64::INFINITY, 30.0, 0.5, 9.0];
        assert_eq!(representative_of(&[0, 1, 2, 3], &differences), 2);
    }

    #[test]
    fn a_lone_frame_represents_itself() {
        let differences = vec![f64::INFINITY];
        assert_eq!(representative_of(&[0], &differences), 0);
    }

    #[test]
    fn blank_frames_have_no_spread() {
        assert!(luma_spread(&blank(0)) < SlideDetectionOptions::default().blank_threshold);
        assert!(luma_spread(&frame_at(100)) > SlideDetectionOptions::default().blank_threshold);
        assert_eq!(luma_spread(&[]), 0.0);
    }

    #[test]
    fn frame_quality_is_clamped_to_the_mjpeg_range() {
        let options = SlideDetectionOptions {
            frame_quality: 0,
            ..SlideDetectionOptions::default()
        };
        assert_eq!(options.frame_quality.clamp(1, 31), 1);
    }
}

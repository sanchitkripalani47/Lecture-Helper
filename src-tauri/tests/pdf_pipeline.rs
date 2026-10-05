//! End-to-end smoke test for the real processing pipeline.
//!
//! Generates a synthetic "lecture" whose picture changes at known times, runs
//! the actual `PdfProcessor` (ffmpeg + slide detection + the PDF writer), and
//! checks that a slide PDF appears with the expected page count.
//!
//! Skipped automatically when ffmpeg is not installed, so the suite stays green
//! on machines without it.

use lecture_helper_lib::domain::models::VideoJob;
use lecture_helper_lib::processing::cancellation::CancellationToken;
use lecture_helper_lib::processing::media::{MediaTools, SlideDetectionOptions};
use lecture_helper_lib::processing::pdf_processor::PdfProcessor;
use lecture_helper_lib::processing::processor::VideoProcessor;
use lecture_helper_lib::processing::progress::ProgressReporter;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// Each slide is a black frame with one coloured block, so slides differ from
/// each other sharply while no slide is a flat, blank frame.
const SLIDE_COLOURS: [&str; 4] = ["red", "green", "blue", "yellow"];
const SECONDS_PER_SLIDE: u32 = 2;

struct RecordingReporter {
    updates: Mutex<Vec<(u8, String, Option<u32>)>>,
}

impl RecordingReporter {
    fn new() -> Self {
        Self {
            updates: Mutex::new(Vec::new()),
        }
    }

    fn updates(&self) -> Vec<(u8, String, Option<u32>)> {
        self.updates.lock().unwrap().clone()
    }
}

impl ProgressReporter for RecordingReporter {
    fn report(&self, progress: u8, stage: &str, slides_detected: Option<u32>) {
        self.updates
            .lock()
            .unwrap()
            .push((progress, stage.to_string(), slides_detected));
    }
}

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "lecture-helper-e2e-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp root");
        Self { root }
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run_ffmpeg(args: &[&str]) {
    let status = Command::new("ffmpeg")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("ffmpeg should be runnable");
    assert!(status.success(), "ffmpeg failed for args: {args:?}");
}

/// A black frame holding one coloured block, so the picture is never blank and
/// two slides at different positions read as different content.
fn build_slide_png(directory: &Path, name: &str, colour: &str, x: u32, y: u32) -> PathBuf {
    let path = directory.join(name);
    let block = format!("drawbox=x={x}:y={y}:w=260:h=200:color={colour}:t=fill");
    run_ffmpeg(&[
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=640x480:d=1",
        "-vf",
        &block,
        "-frames:v",
        "1",
        path.to_str().unwrap(),
    ]);
    path
}

/// Build a video that shows `SLIDE_COLOURS` in order, `SECONDS_PER_SLIDE` each.
fn build_synthetic_lecture(directory: &Path) -> PathBuf {
    let mut slide_files = Vec::new();
    for (index, colour) in SLIDE_COLOURS.iter().enumerate() {
        // The block moves between slides, so consecutive slides read as
        // different content rather than the same picture.
        slide_files.push(build_slide_png(
            directory,
            &format!("slide{index}.png"),
            colour,
            40 + index as u32 * 80,
            40 + index as u32 * 60,
        ));
    }

    let video = directory.join("Lecture 01.mp4");
    let mut args: Vec<String> = vec!["-y".to_string()];
    for slide in &slide_files {
        args.push("-loop".to_string());
        args.push("1".to_string());
        args.push("-t".to_string());
        args.push(SECONDS_PER_SLIDE.to_string());
        args.push("-i".to_string());
        args.push(slide.to_str().unwrap().to_string());
    }

    let inputs: String = (0..slide_files.len())
        .map(|index| format!("[{index}:v]"))
        .collect();
    args.push("-filter_complex".to_string());
    args.push(format!(
        "{inputs}concat=n={}:v=1:a=0[out]",
        slide_files.len()
    ));
    args.push("-map".to_string());
    args.push("[out]".to_string());
    args.push("-pix_fmt".to_string());
    args.push("yuv420p".to_string());
    args.push("-r".to_string());
    args.push("5".to_string());
    args.push(video.to_str().unwrap().to_string());

    let borrowed: Vec<&str> = args.iter().map(|value| value.as_str()).collect();
    run_ffmpeg(&borrowed);
    video
}

fn count_pdf_pages(bytes: &[u8]) -> usize {
    // The writer emits "N 0 obj\n<< /Type /Page /Parent ...>>" per page. The
    // page tree uses "/Type /Pages", which does not match the trailing space.
    let needle = b"/Type /Page ";
    bytes.windows(needle.len()).filter(|w| *w == needle).count()
}

fn assert_ffmpeg_available() -> bool {
    match MediaTools::discover() {
        Ok(_) => true,
        Err(error) => {
            eprintln!("skipping pipeline smoke test: {error}");
            false
        }
    }
}

/// A cross-fade between two slides must not contribute a page of its own, even
/// though several samples land in the middle of the fade.
#[test]
fn a_fade_between_slides_adds_no_extra_page() {
    if !assert_ffmpeg_available() {
        return;
    }

    let tree = TempTree::new("fade");
    let first = build_slide_png(&tree.root, "a.png", "red", 40, 40);
    let second = build_slide_png(&tree.root, "b.png", "blue", 320, 220);
    let video = tree.root.join("Lecture 01.mp4");

    // Sampling at 1 fps puts several samples inside the 3 second fade.
    run_ffmpeg(&[
        "-y",
        "-loop",
        "1",
        "-framerate",
        "10",
        "-t",
        "4",
        "-i",
        first.to_str().unwrap(),
        "-loop",
        "1",
        "-framerate",
        "10",
        "-t",
        "6",
        "-i",
        second.to_str().unwrap(),
        "-filter_complex",
        "[0:v][1:v]xfade=transition=fade:duration=3:offset=1.5[out]",
        "-map",
        "[out]",
        "-r",
        "10",
        "-pix_fmt",
        "yuv420p",
        video.to_str().unwrap(),
    ]);

    let output = tree.root.join("Processed").join("Lecture 01.pdf");
    let job = VideoJob::new(
        video.clone(),
        PathBuf::from("Lecture 01.mp4"),
        output.clone(),
        false,
    );

    let tools = MediaTools::discover().expect("media tools");
    let processor = PdfProcessor::new(tools);
    let result = processor
        .process(&job, &RecordingReporter::new(), &CancellationToken::new())
        .expect("process video");

    assert_eq!(
        result.slides_detected, 2,
        "the fade itself must not become a slide"
    );
    assert!(output.is_file());
}

#[test]
fn converts_a_synthetic_lecture_into_a_slide_pdf() {
    if !assert_ffmpeg_available() {
        return;
    }

    let tree = TempTree::new("pipeline");
    let video = build_synthetic_lecture(&tree.root);
    let source_before = fs::read(&video).expect("read source video");

    let output = tree.root.join("Processed").join("Lecture 01.pdf");
    let job = VideoJob::new(
        video.clone(),
        PathBuf::from("Lecture 01.mp4"),
        output.clone(),
        false,
    );

    let tools = MediaTools::discover().expect("media tools");
    let processor = PdfProcessor::with_detection(tools, SlideDetectionOptions::default());
    let reporter = RecordingReporter::new();

    let result = processor
        .process(&job, &reporter, &CancellationToken::new())
        .expect("process video");

    // One page per slide: the picture changes once per SLIDE_COLOURS entry.
    assert!(
        result.slides_detected >= SLIDE_COLOURS.len() as u32 - 1
            && result.slides_detected <= SLIDE_COLOURS.len() as u32,
        "expected about {} slides, detected {}",
        SLIDE_COLOURS.len(),
        result.slides_detected
    );
    assert_eq!(result.slides_pdf.as_deref(), Some(output.as_path()));
    assert!(output.is_file(), "the PDF should exist at the destination");

    let bytes = fs::read(&output).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert!(bytes.ends_with(b"%%EOF\n"));
    assert_eq!(count_pdf_pages(&bytes), result.slides_detected as usize);

    // Progress must be real and monotonic, and must reach completion.
    let updates = reporter.updates();
    assert!(!updates.is_empty(), "progress should be reported");
    let values: Vec<u8> = updates.iter().map(|(progress, _, _)| *progress).collect();
    assert!(
        values.windows(2).all(|pair| pair[0] <= pair[1]),
        "progress must not go backwards: {values:?}"
    );
    assert_eq!(values.last().copied(), Some(100));
    assert!(
        updates
            .iter()
            .any(|(_, stage, _)| stage == "Extracting frames"),
        "frame extraction should be reported"
    );
    assert!(
        updates
            .iter()
            .any(|(_, stage, _)| stage == "Detecting slides"),
        "slide detection should be reported"
    );

    // The source video is never modified.
    assert_eq!(fs::read(&video).expect("read source video"), source_before);
}

#[test]
fn cancellation_leaves_no_partial_output() {
    if !assert_ffmpeg_available() {
        return;
    }

    let tree = TempTree::new("cancel");
    let video = build_synthetic_lecture(&tree.root);
    let output = tree.root.join("Processed").join("Lecture 01.pdf");
    let job = VideoJob::new(
        video.clone(),
        PathBuf::from("Lecture 01.mp4"),
        output.clone(),
        false,
    );

    let token = CancellationToken::new();
    token.cancel();

    let tools = MediaTools::discover().expect("media tools");
    let processor = PdfProcessor::new(tools);
    let error = processor
        .process(&job, &RecordingReporter::new(), &token)
        .expect_err("a cancelled run should not succeed");

    assert!(
        matches!(
            error,
            lecture_helper_lib::domain::ProcessingError::Cancelled
        ),
        "expected a Cancelled error, got {error}"
    );
    assert!(!output.exists(), "no PDF should be committed");
    assert!(
        !output.with_file_name("Lecture 01.pdf.partial").exists(),
        "no partial file should remain"
    );
}

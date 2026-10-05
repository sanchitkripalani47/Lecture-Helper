//! End-to-end checks across discovery and the batch coordinator.
//!
//! Media processing itself is replaced with a fake processor: these tests prove
//! the queue mirrors the folder structure, continues past failures, and never
//! touches source videos, without depending on ffmpeg.

use lecture_helper_lib::batch::coordinator::{run_batch, EventSink};
use lecture_helper_lib::discovery::build_jobs;
use lecture_helper_lib::domain::models::{
    BatchEvent, BatchOptions, ExistingOutputPolicy, ProcessingResult, VideoJob,
};
use lecture_helper_lib::domain::ProcessingError;
use lecture_helper_lib::processing::cancellation::CancellationToken;
use lecture_helper_lib::processing::processor::VideoProcessor;
use lecture_helper_lib::processing::progress::ProgressReporter;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
struct CollectingSink {
    events: Mutex<Vec<BatchEvent>>,
}

impl CollectingSink {
    fn events(&self) -> Vec<BatchEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl EventSink for CollectingSink {
    fn emit(&self, event: BatchEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// Writes a small placeholder PDF so the test can assert output exists.
struct WritingProcessor;

impl VideoProcessor for WritingProcessor {
    fn process(
        &self,
        job: &VideoJob,
        reporter: &dyn ProgressReporter,
        _cancellation: &CancellationToken,
    ) -> Result<ProcessingResult, ProcessingError> {
        reporter.report(50, "Assembling PDF", Some(3));
        if let Some(parent) = job.output_path.parent() {
            fs::create_dir_all(parent).expect("create output dir");
        }
        fs::write(&job.output_path, b"%PDF-1.4\n%%EOF\n").expect("write pdf");
        Ok(ProcessingResult::with_slides(
            job.source_path.clone(),
            job.output_path.clone(),
            3,
        ))
    }
}

/// Fails on one specific file and succeeds everywhere else.
struct FlakyProcessor {
    fail_on: PathBuf,
}

impl VideoProcessor for FlakyProcessor {
    fn process(
        &self,
        job: &VideoJob,
        _reporter: &dyn ProgressReporter,
        _cancellation: &CancellationToken,
    ) -> Result<ProcessingResult, ProcessingError> {
        if job.source_path == self.fail_on {
            return Err(ProcessingError::DecodeFailed {
                path: job.source_path.clone(),
                detail: "unsupported codec".to_string(),
            });
        }
        if let Some(parent) = job.output_path.parent() {
            fs::create_dir_all(parent).expect("create output dir");
        }
        fs::write(&job.output_path, b"%PDF-1.4\n%%EOF\n").expect("write pdf");
        Ok(ProcessingResult::with_slides(
            job.source_path.clone(),
            job.output_path.clone(),
            2,
        ))
    }
}

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("lecture-helper-it-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create root");
        Self { root }
    }

    fn file(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, b"video-bytes").expect("write source");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn options(input: &Path, output: &Path, policy: ExistingOutputPolicy) -> BatchOptions {
    BatchOptions {
        input_root: input.to_path_buf(),
        output_root: output.to_path_buf(),
        existing_output_policy: policy,
    }
}

#[test]
fn mirrors_the_input_hierarchy_into_the_output_folder() {
    let tree = TempTree::new("mirror");
    tree.file("My Videos/Course B/Lecture 01.mp4");
    tree.file("My Videos/Course B/Lecture 02.mov");
    tree.file("My Videos/Course A/Lecture 01.mkv");
    tree.file("My Videos/Course A/notes.txt");
    // Outside the selected root, so it must not appear in the queue.
    tree.file("Other/Lecture 99.mp4");

    let input = tree.root.join("My Videos");
    let output = tree.root.join("My Videos - Processed");
    let opts = options(&input, &output, ExistingOutputPolicy::Skip);

    let jobs = build_jobs(&opts).expect("build jobs");
    assert_eq!(jobs.len(), 3);
    assert!(jobs
        .iter()
        .all(|job| !job.relative_path.to_string_lossy().contains("Lecture 99")));

    let sink = CollectingSink::default();
    let outcome = run_batch(
        jobs,
        &WritingProcessor,
        &opts,
        &CancellationToken::new(),
        &sink,
    );

    assert_eq!(outcome.summary.completed, 3);
    assert!(output.join("Course B/Lecture 01.pdf").is_file());
    assert!(output.join("Course B/Lecture 02.pdf").is_file());
    assert!(output.join("Course A/Lecture 01.pdf").is_file());
    // The original folder hierarchy is mirrored, not flattened.
    assert!(!output.join("Lecture 01.pdf").exists());
}

#[test]
fn source_videos_are_never_modified() {
    let tree = TempTree::new("source");
    let source = tree.file("My Videos/Course B/Lecture 01.mp4");
    let before = fs::read(&source).expect("read source");

    let input = tree.root.join("My Videos");
    let output = tree.root.join("Processed");
    let opts = options(&input, &output, ExistingOutputPolicy::Replace);

    let jobs = build_jobs(&opts).expect("build jobs");
    run_batch(
        jobs,
        &WritingProcessor,
        &opts,
        &CancellationToken::new(),
        &CollectingSink::default(),
    );

    assert_eq!(fs::read(&source).expect("read source"), before);
}

#[test]
fn one_failure_does_not_block_the_rest_of_the_batch() {
    let tree = TempTree::new("failure");
    tree.file("My Videos/Course B/Lecture 01.mp4");
    let broken = tree.file("My Videos/Course B/Lecture 02.mp4");
    tree.file("My Videos/Course B/Lecture 03.mp4");

    let input = tree.root.join("My Videos");
    let output = tree.root.join("Processed");
    let opts = options(&input, &output, ExistingOutputPolicy::Skip);

    let jobs = build_jobs(&opts).expect("build jobs");
    let sink = CollectingSink::default();
    let outcome = run_batch(
        jobs,
        &FlakyProcessor { fail_on: broken },
        &opts,
        &CancellationToken::new(),
        &sink,
    );

    assert_eq!(outcome.summary.completed, 2);
    assert_eq!(outcome.summary.failed, 1);
    assert!(output.join("Course B/Lecture 03.pdf").is_file());

    // The failure is reported, and the batch still reaches a normal end.
    let events = sink.events();
    let failures = events
        .iter()
        .filter(|event| matches!(event, BatchEvent::JobFailed { .. }))
        .count();
    assert_eq!(failures, 1);
    assert!(matches!(
        events.last(),
        Some(BatchEvent::BatchCompleted { .. })
    ));
}

#[test]
fn existing_outputs_are_skipped_and_preserved() {
    let tree = TempTree::new("skip");
    tree.file("My Videos/Course B/Lecture 01.mp4");

    let input = tree.root.join("My Videos");
    let output = tree.root.join("Processed");
    let existing = output.join("Course B/Lecture 01.pdf");
    fs::create_dir_all(existing.parent().unwrap()).expect("create dir");
    fs::write(&existing, b"already-here").expect("seed output");

    let opts = options(&input, &output, ExistingOutputPolicy::Skip);
    let jobs = build_jobs(&opts).expect("build jobs");
    assert!(jobs[0].output_exists);

    let outcome = run_batch(
        jobs,
        &WritingProcessor,
        &opts,
        &CancellationToken::new(),
        &CollectingSink::default(),
    );

    assert_eq!(outcome.summary.skipped, 1);
    assert_eq!(outcome.summary.completed, 0);
    assert_eq!(fs::read(&existing).expect("read output"), b"already-here");
}

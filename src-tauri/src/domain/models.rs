use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Lifecycle of a single video inside a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
    Skipped,
}

/// What to do when the destination PDF already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExistingOutputPolicy {
    #[default]
    Skip,
    Replace,
}

/// Inputs for a batch run. Serialized camelCase to match the TypeScript DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchOptions {
    pub input_root: PathBuf,
    pub output_root: PathBuf,
    #[serde(default)]
    pub existing_output_policy: ExistingOutputPolicy,
}

/// A single discovered video and everything the UI needs to render it.
///
/// The job is deliberately output agnostic: `ProcessingResult` describes what
/// was produced, so a later release can add transcripts or slide text without
/// changing the queue model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoJob {
    pub id: String,
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
    pub output_path: PathBuf,
    pub status: JobStatus,
    pub progress: u8,
    pub stage: String,
    pub slides_detected: u32,
    pub error: Option<String>,
    pub output_exists: bool,
}

impl VideoJob {
    pub fn new(
        source_path: PathBuf,
        relative_path: PathBuf,
        output_path: PathBuf,
        output_exists: bool,
    ) -> Self {
        let id = relative_path.to_string_lossy().replace('\\', "/");
        Self {
            id,
            source_path,
            relative_path,
            output_path,
            status: JobStatus::Pending,
            progress: 0,
            stage: "Pending".to_string(),
            slides_detected: 0,
            error: None,
            output_exists,
        }
    }
}

/// Artifacts produced for one video. V1 only fills `slides_pdf`; the remaining
/// fields exist so transcription/OCR can be added without a redesign.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingResult {
    pub source_video: PathBuf,
    pub slides_pdf: Option<PathBuf>,
    pub transcript: Option<PathBuf>,
    pub slide_text: Option<PathBuf>,
    pub slides_detected: u32,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl ProcessingResult {
    pub fn with_slides(source_video: PathBuf, slides_pdf: PathBuf, slides_detected: u32) -> Self {
        Self {
            source_video,
            slides_pdf: Some(slides_pdf),
            transcript: None,
            slide_text: None,
            slides_detected,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummary {
    pub found: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: usize,
    pub slides_detected: u32,
    pub output_root: PathBuf,
}

/// Result of a discovery pass. Internally tagged so the frontend can switch on
/// `kind` without an exception path for the empty case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScanResult {
    Ready { jobs: Vec<VideoJob> },
    Empty { input_root: PathBuf },
}

/// Progress and state events streamed to the frontend over `batch://event`.
///
/// Discovery has its own request/response command (`discover_jobs`), so scan
/// lifecycle is not part of the streaming event model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BatchEvent {
    JobStarted {
        job_id: String,
        stage: String,
    },
    JobProgress {
        job_id: String,
        progress: u8,
        stage: String,
        slides_detected: Option<u32>,
    },
    JobCompleted {
        job_id: String,
        result: ProcessingResult,
    },
    JobFailed {
        job_id: String,
        error: String,
    },
    JobSkipped {
        job_id: String,
        reason: String,
    },
    BatchCancelled {
        completed: usize,
        remaining: usize,
    },
    BatchCompleted {
        summary: BatchSummary,
    },
}

/// Where a progress update originates. Kept separate from `BatchEvent` so the
/// processor never needs to know about Tauri or the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressUpdate {
    pub progress: u8,
    pub stage: String,
    pub slides_detected: Option<u32>,
}

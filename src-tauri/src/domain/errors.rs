use std::fmt;
use std::io;
use std::path::PathBuf;

/// Errors raised while processing a single video or reading a folder.
///
/// Messages are written for end users: they name the file and the next action
/// where one exists, and never leak stack traces.
#[derive(Debug)]
pub enum ProcessingError {
    InputUnreadable {
        path: PathBuf,
        source: io::Error,
    },
    NotADirectory {
        path: PathBuf,
    },
    UnsupportedInput {
        path: PathBuf,
    },
    NoSupportedVideos {
        path: PathBuf,
    },
    DependencyMissing {
        program: String,
        hint: String,
    },
    ExternalToolFailed {
        program: String,
        code: Option<i32>,
        detail: String,
    },
    DecodeFailed {
        path: PathBuf,
        detail: String,
    },
    OutputWrite {
        path: PathBuf,
        source: io::Error,
    },
    UnsafeOutputPath {
        path: PathBuf,
        reason: String,
    },
    Cancelled,
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessingError::InputUnreadable { path, source } => write!(
                f,
                "Could not read {}: {}",
                path.display(),
                source
            ),
            ProcessingError::NotADirectory { path } => {
                write!(f, "{} is not a folder.", path.display())
            }
            ProcessingError::UnsupportedInput { path } => write!(
                f,
                "{} is not a supported video. Supported formats: .mp4, .mov, .mkv, .avi, .webm.",
                path.display()
            ),
            ProcessingError::NoSupportedVideos { path } => write!(
                f,
                "No supported videos found in {}. Supported formats: .mp4, .mov, .mkv, .avi, .webm.",
                path.display()
            ),
            ProcessingError::DependencyMissing { program, hint } => {
                write!(f, "{} was not found. {}", program, hint)
            }
            ProcessingError::ExternalToolFailed {
                program,
                code,
                detail,
            } => {
                let code = code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                write!(f, "{} failed (exit code {}). {}", program, code, detail)
            }
            ProcessingError::DecodeFailed { path, detail } => {
                write!(f, "Could not decode {}: {}", path.display(), detail)
            }
            ProcessingError::OutputWrite { path, source } => {
                write!(f, "Could not write {}: {}", path.display(), source)
            }
            ProcessingError::UnsafeOutputPath { path, reason } => write!(
                f,
                "Refusing to write {}: {}",
                path.display(),
                reason
            ),
            ProcessingError::Cancelled => write!(f, "Processing was cancelled."),
        }
    }
}

impl std::error::Error for ProcessingError {}

impl From<ProcessingError> for String {
    fn from(error: ProcessingError) -> Self {
        error.to_string()
    }
}

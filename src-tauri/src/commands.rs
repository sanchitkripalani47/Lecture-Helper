use crate::batch::coordinator::{run_batch, EventSink};
use crate::discovery;
use crate::domain::models::{BatchEvent, BatchOptions, ScanResult};
use crate::processing::cancellation::CancellationToken;
use crate::processing::media::MediaTools;
use crate::processing::pdf_processor::PdfProcessor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// Event name the frontend subscribes to for progress and state updates.
pub const BATCH_EVENT: &str = "batch://event";

/// Shared runtime state for the single active batch.
#[derive(Default)]
pub struct BatchState {
    current: Mutex<Option<Arc<CancellationToken>>>,
    running: AtomicBool,
}

impl BatchState {
    fn begin(&self, token: Arc<CancellationToken>) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("A batch is already running.".to_string());
        }
        *self.current.lock().expect("batch state lock") = Some(token);
        Ok(())
    }

    fn finish(&self) {
        *self.current.lock().expect("batch state lock") = None;
        self.running.store(false, Ordering::SeqCst);
    }

    fn cancel(&self) {
        if let Some(token) = self.current.lock().expect("batch state lock").as_ref() {
            token.cancel();
        }
    }
}

struct TauriSink {
    app: AppHandle,
}

impl EventSink for TauriSink {
    fn emit(&self, event: BatchEvent) {
        // A closed window is not an error worth failing the batch over.
        let _ = self.app.emit(BATCH_EVENT, event);
    }
}

/// Recursively discover supported videos and build the queue.
#[tauri::command]
pub async fn discover_jobs(options: BatchOptions) -> Result<ScanResult, String> {
    discovery::discover(&options).map_err(|error| error.to_string())
}

/// Start processing the selected folder.
///
/// Validation (folder readability, ffmpeg availability) happens before the
/// worker starts so problems surface immediately instead of mid-batch.
#[tauri::command]
pub async fn start_batch(
    app: AppHandle,
    state: State<'_, Arc<BatchState>>,
    options: BatchOptions,
) -> Result<(), String> {
    let jobs = discovery::build_jobs(&options).map_err(|error| error.to_string())?;
    if jobs.is_empty() {
        return Err("No supported videos found in this folder.".to_string());
    }

    let tools = MediaTools::discover().map_err(|error| error.to_string())?;

    let token = Arc::new(CancellationToken::new());
    state.begin(Arc::clone(&token))?;

    let state = Arc::clone(state.inner());
    let processor = PdfProcessor::new(tools);

    std::thread::spawn(move || {
        let sink = TauriSink { app };
        run_batch(jobs, &processor, &options, &token, &sink);
        state.finish();
    });

    Ok(())
}

/// Ask the running batch to stop. New videos are not started and the current
/// one finishes safely.
#[tauri::command]
pub async fn cancel_batch(state: State<'_, Arc<BatchState>>) -> Result<(), String> {
    state.cancel();
    Ok(())
}

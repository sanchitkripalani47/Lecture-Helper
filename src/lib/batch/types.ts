export type JobStatus = "pending" | "processing" | "completed" | "failed" | "cancelled" | "skipped";

export type ExistingOutputPolicy = "skip" | "replace";

/** Where generated PDFs are written. */
export type OutputDestination = "new-folder" | "alongside";

/** Whether the user chose a folder to scan or one specific video. */
export type InputKind = "folder" | "file";

export const SUPPORTED_VIDEO_EXTENSIONS = [".mp4", ".mov", ".mkv", ".avi", ".webm"] as const;

export const JOB_STATUS_LABEL: Record<JobStatus, string> = {
  pending: "Pending",
  processing: "Processing",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  skipped: "Skipped",
};

export interface VideoJob {
  id: string;
  sourcePath: string;
  relativePath: string;
  outputPath: string;
  status: JobStatus;
  progress: number;
  stage: string;
  slidesDetected: number;
  error: string | null;
  outputExists: boolean;
}

export interface ProcessingResult {
  sourceVideo: string;
  slidesPdf: string | null;
  transcript: string | null;
  slideText: string | null;
  slidesDetected: number;
  metadata: Record<string, unknown>;
}

export interface BatchOptions {
  inputRoot: string;
  outputRoot: string;
  existingOutputPolicy: ExistingOutputPolicy;
}

export interface BatchSummary {
  found: number;
  completed: number;
  failed: number;
  skipped: number;
  cancelled: number;
  slidesDetected: number;
  outputRoot: string;
}

export type ScanResult = { kind: "ready"; jobs: VideoJob[] } | { kind: "empty"; inputRoot: string };

export type BatchEvent =
  | { type: "jobStarted"; jobId: string; stage: string }
  | {
      type: "jobProgress";
      jobId: string;
      progress: number;
      stage: string;
      slidesDetected: number | null;
    }
  | { type: "jobCompleted"; jobId: string; result: ProcessingResult }
  | { type: "jobFailed"; jobId: string; error: string }
  | { type: "jobSkipped"; jobId: string; reason: string }
  | { type: "batchCancelled"; completed: number; remaining: number }
  | { type: "batchCompleted"; summary: BatchSummary };

import type { BatchEvent, BatchSummary, InputKind, JobStatus, VideoJob } from "./types";

export interface WorkflowState {
  stage: "setup" | "scanning" | "review" | "processing" | "summary";
  /** The chosen folder, or a single chosen video file. */
  inputPath: string;
  inputKind: InputKind;
  outputRoot: string;
  jobs: VideoJob[];
  currentJobId: string | null;
  scanning: boolean;
  cancelling: boolean;
  cancelled: boolean;
  error: string | null;
  summary: BatchSummary | null;
}

export const initialWorkflowState: WorkflowState = {
  stage: "setup",
  inputPath: "",
  inputKind: "folder",
  outputRoot: "",
  jobs: [],
  currentJobId: null,
  scanning: false,
  cancelling: false,
  cancelled: false,
  error: null,
  summary: null,
};

export type WorkflowAction =
  | { type: "setInput"; inputPath: string; inputKind: InputKind; outputRoot: string }
  | { type: "setOutputRoot"; outputRoot: string }
  | { type: "scanStarted" }
  | { type: "scanReady"; jobs: VideoJob[] }
  | { type: "scanFailed"; error: string }
  | { type: "backToSetup" }
  | { type: "processingStarted" }
  | { type: "setError"; error: string }
  | { type: "batchFailed"; error: string }
  | { type: "cancelRequested" }
  | { type: "reset" }
  | { type: "batchEvent"; event: BatchEvent };

const TERMINAL_STATUSES: JobStatus[] = ["completed", "failed", "skipped", "cancelled"];

function updateJob(jobs: VideoJob[], jobId: string, patch: Partial<VideoJob>): VideoJob[] {
  return jobs.map((job) => (job.id === jobId ? { ...job, ...patch } : job));
}

function markRemainingCancelled(jobs: VideoJob[]): VideoJob[] {
  return jobs.map((job) =>
    job.status === "pending" || job.status === "processing"
      ? { ...job, status: "cancelled", stage: "Not processed" }
      : job,
  );
}

function applyBatchEvent(state: WorkflowState, event: BatchEvent): WorkflowState {
  switch (event.type) {
    case "jobStarted":
      return {
        ...state,
        currentJobId: event.jobId,
        jobs: updateJob(state.jobs, event.jobId, {
          status: "processing",
          progress: 0,
          stage: event.stage,
          error: null,
        }),
      };
    case "jobProgress":
      return {
        ...state,
        jobs: updateJob(state.jobs, event.jobId, {
          progress: event.progress,
          stage: event.stage,
          slidesDetected:
            event.slidesDetected ??
            state.jobs.find((j) => j.id === event.jobId)?.slidesDetected ??
            0,
        }),
      };
    case "jobCompleted":
      return {
        ...state,
        jobs: updateJob(state.jobs, event.jobId, {
          status: "completed",
          progress: 100,
          stage: "Completed",
          slidesDetected: event.result.slidesDetected,
          outputPath:
            event.result.slidesPdf ??
            state.jobs.find((j) => j.id === event.jobId)?.outputPath ??
            "",
          error: null,
        }),
      };
    case "jobFailed":
      return {
        ...state,
        jobs: updateJob(state.jobs, event.jobId, {
          status: "failed",
          stage: "Failed",
          error: event.error,
        }),
      };
    case "jobSkipped":
      return {
        ...state,
        jobs: updateJob(state.jobs, event.jobId, {
          status: "skipped",
          stage: event.reason,
        }),
      };
    case "batchCancelled":
      return {
        ...state,
        cancelling: false,
        cancelled: true,
        currentJobId: null,
        stage: "summary",
        jobs: markRemainingCancelled(state.jobs),
      };
    case "batchCompleted":
      return {
        ...state,
        cancelling: false,
        currentJobId: null,
        stage: "summary",
        summary: event.summary,
      };
    default:
      return state;
  }
}

export function workflowReducer(state: WorkflowState, action: WorkflowAction): WorkflowState {
  switch (action.type) {
    case "setInput":
      return {
        ...state,
        inputPath: action.inputPath,
        inputKind: action.inputKind,
        outputRoot: action.outputRoot,
      };
    case "setOutputRoot":
      return { ...state, outputRoot: action.outputRoot };
    case "scanStarted":
      return { ...state, stage: "scanning", scanning: true, error: null, jobs: [], summary: null };
    case "scanReady":
      return { ...state, stage: "review", scanning: false, jobs: action.jobs };
    case "scanFailed":
      return { ...state, stage: "setup", scanning: false, error: action.error };
    case "backToSetup":
      return { ...state, stage: "setup", error: null };
    case "processingStarted":
      return {
        ...state,
        stage: "processing",
        cancelling: false,
        cancelled: false,
        summary: null,
        error: null,
        jobs: state.jobs.map((job) =>
          job.status === "skipped" ? job : { ...job, status: "pending", progress: 0, error: null },
        ),
      };
    case "setError":
      return { ...state, error: action.error };
    case "batchFailed":
      // The queue is still valid, so return to review with the error visible
      // and let the user adjust the output policy or retry.
      return {
        ...state,
        stage: "review",
        currentJobId: null,
        cancelling: false,
        error: action.error,
      };
    case "cancelRequested":
      return { ...state, cancelling: true };
    case "reset":
      return { ...initialWorkflowState };
    case "batchEvent":
      return applyBatchEvent(state, action.event);
    default:
      return state;
  }
}

export function selectCounts(jobs: VideoJob[]) {
  return jobs.reduce(
    (counts, job) => {
      counts[job.status] += 1;
      return counts;
    },
    {
      pending: 0,
      processing: 0,
      completed: 0,
      failed: 0,
      cancelled: 0,
      skipped: 0,
    } as Record<JobStatus, number>,
  );
}

export function selectProgress(state: WorkflowState): number {
  const jobs = state.jobs;
  if (jobs.length === 0) return 0;
  const settled = jobs.filter((job) => TERMINAL_STATUSES.includes(job.status)).length;
  const current = jobs.find((job) => job.id === state.currentJobId);
  const currentFraction = current ? Math.min(Math.max(current.progress, 0), 100) / 100 : 0;
  return Math.round(((settled + currentFraction) / jobs.length) * 100);
}

import { describe, expect, it } from "vitest";
import { groupJobsByDirectory } from "./grouping";
import { computeSummary } from "./grouping";
import type { BatchEvent, VideoJob } from "./types";
import {
  initialWorkflowState,
  selectCounts,
  selectProgress,
  workflowReducer,
  type WorkflowState,
} from "./workflow";

function job(id: string, overrides: Partial<VideoJob> = {}): VideoJob {
  return {
    id,
    sourcePath: `/in/${id}`,
    relativePath: id,
    outputPath: `/out/${id.replace(/\.mp4$/, ".pdf")}`,
    status: "pending",
    progress: 0,
    stage: "Pending",
    slidesDetected: 0,
    error: null,
    outputExists: false,
    ...overrides,
  };
}

function withJobs(jobs: VideoJob[]): WorkflowState {
  return { ...initialWorkflowState, stage: "review", jobs };
}

function apply(state: WorkflowState, ...events: BatchEvent[]): WorkflowState {
  return events.reduce(
    (current, event) => workflowReducer(current, { type: "batchEvent", event }),
    state,
  );
}

describe("workflowReducer", () => {
  it("moves through setup, review, processing, and summary", () => {
    let state = workflowReducer(initialWorkflowState, {
      type: "setInput",
      inputPath: "/in",
      inputKind: "folder",
      outputRoot: "/in - Processed",
    });
    expect(state.stage).toBe("setup");
    expect(state.inputKind).toBe("folder");

    state = workflowReducer(state, { type: "scanStarted" });
    expect(state.stage).toBe("scanning");

    state = workflowReducer(state, { type: "scanReady", jobs: [job("a.mp4")] });
    expect(state.stage).toBe("review");

    state = workflowReducer(state, { type: "processingStarted" });
    expect(state.stage).toBe("processing");

    state = apply(state, { type: "batchCompleted", summary: computeSummary(state.jobs, "/out") });
    expect(state.stage).toBe("summary");
  });

  it("surfaces a scan error and returns to setup", () => {
    const state = workflowReducer(
      { ...initialWorkflowState, stage: "scanning" },
      { type: "scanFailed", error: "No supported videos found in this folder." },
    );
    expect(state.stage).toBe("setup");
    expect(state.error).toMatch(/No supported videos/);
  });

  it("tracks job progress and slide counts", () => {
    const state = apply(
      withJobs([job("a.mp4")]),
      { type: "jobStarted", jobId: "a.mp4", stage: "Extracting frames" },
      {
        type: "jobProgress",
        jobId: "a.mp4",
        progress: 60,
        stage: "Detecting slides",
        slidesDetected: 12,
      },
    );

    expect(state.currentJobId).toBe("a.mp4");
    expect(state.jobs[0].status).toBe("processing");
    expect(state.jobs[0].progress).toBe(60);
    expect(state.jobs[0].slidesDetected).toBe(12);
  });

  it("keeps processing after an individual failure", () => {
    const state = apply(
      withJobs([job("a.mp4"), job("b.mp4")]),
      { type: "jobFailed", jobId: "a.mp4", error: "unsupported codec" },
      { type: "jobStarted", jobId: "b.mp4", stage: "Extracting frames" },
    );

    expect(state.jobs[0].status).toBe("failed");
    expect(state.jobs[0].error).toBe("unsupported codec");
    expect(state.jobs[1].status).toBe("processing");
  });

  it("marks pending and in-flight jobs cancelled when the batch is cancelled", () => {
    const state = apply(
      withJobs([job("a.mp4", { status: "completed" }), job("b.mp4"), job("c.mp4")]),
      { type: "jobStarted", jobId: "b.mp4", stage: "Extracting frames" },
      { type: "batchCancelled", completed: 1, remaining: 2 },
    );

    expect(state.stage).toBe("summary");
    expect(state.cancelled).toBe(true);
    expect(state.jobs.map((entry) => entry.status)).toEqual([
      "completed",
      "cancelled",
      "cancelled",
    ]);
  });

  it("returns to review with the error when a batch cannot start", () => {
    const started = workflowReducer(withJobs([job("a.mp4")]), { type: "processingStarted" });
    const state = workflowReducer(started, {
      type: "batchFailed",
      error: "ffmpeg was not found. Install ffmpeg and make sure it is on your PATH.",
    });

    expect(state.stage).toBe("review");
    expect(state.error).toMatch(/ffmpeg was not found/);
    expect(state.jobs).toHaveLength(1);
  });

  it("clears a previous error when processing starts", () => {
    const state = workflowReducer(
      { ...withJobs([job("a.mp4")]), error: "old error" },
      { type: "processingStarted" },
    );
    expect(state.error).toBeNull();
  });

  it("resets to the initial state", () => {
    const state = workflowReducer(
      { ...initialWorkflowState, stage: "summary", jobs: [job("a.mp4")] },
      { type: "reset" },
    );
    expect(state).toEqual(initialWorkflowState);
  });
});

describe("selectCounts and selectProgress", () => {
  it("counts each status", () => {
    const counts = selectCounts([
      job("a.mp4", { status: "completed" }),
      job("b.mp4", { status: "failed" }),
      job("c.mp4", { status: "skipped" }),
      job("d.mp4"),
    ]);
    expect(counts).toMatchObject({ completed: 1, failed: 1, skipped: 1, pending: 1 });
  });

  it("uses settled jobs plus the current job's own progress", () => {
    const state: WorkflowState = {
      ...withJobs([
        job("a.mp4", { status: "completed" }),
        job("b.mp4", { status: "processing", progress: 50 }),
        job("c.mp4"),
      ]),
      currentJobId: "b.mp4",
    };
    expect(selectProgress(state)).toBe(50);
  });

  it("returns zero for an empty queue", () => {
    expect(selectProgress({ ...initialWorkflowState, jobs: [] })).toBe(0);
  });
});

describe("groupJobsByDirectory", () => {
  it("groups nested paths and keeps a deterministic order", () => {
    const groups = groupJobsByDirectory([
      job("Course B/Lecture 02.mp4"),
      job("Course A/Lecture 01.mp4"),
      job("Course B/Lecture 01.mp4"),
    ]);

    expect(groups.map((group) => group.directory)).toEqual(["Course A", "Course B"]);
    expect(groups[1].jobs.map((entry) => entry.relativePath)).toEqual([
      "Course B/Lecture 02.mp4",
      "Course B/Lecture 01.mp4",
    ]);
  });

  it("puts top-level videos in a single group", () => {
    const groups = groupJobsByDirectory([job("Lecture 01.mp4")]);
    expect(groups).toHaveLength(1);
    expect(groups[0].directory).toBe(".");
  });
});

describe("computeSummary", () => {
  it("aggregates statuses and slide counts", () => {
    const summary = computeSummary(
      [
        job("a.mp4", { status: "completed", slidesDetected: 30 }),
        job("b.mp4", { status: "completed", slidesDetected: 12 }),
        job("c.mp4", { status: "failed" }),
      ],
      "/out",
    );

    expect(summary).toMatchObject({
      found: 3,
      completed: 2,
      failed: 1,
      skipped: 0,
      slidesDetected: 42,
      outputRoot: "/out",
    });
  });
});

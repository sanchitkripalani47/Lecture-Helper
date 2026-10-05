import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { defaultOutputRoot, type BatchClient } from "@/lib/batch/tauri-client";
import type { BatchEvent, BatchOptions, ScanResult, VideoJob } from "@/lib/batch/types";
import { BatchWorkflow } from "./batch-workflow";

function sampleJobs(): VideoJob[] {
  return [
    {
      id: "Course B/Lecture 01.mp4",
      sourcePath: "/Users/you/My Videos/Course B/Lecture 01.mp4",
      relativePath: "Course B/Lecture 01.mp4",
      outputPath: "/Users/you/My Videos - Processed/Course B/Lecture 01.pdf",
      status: "pending",
      progress: 0,
      stage: "Pending",
      slidesDetected: 0,
      error: null,
      outputExists: false,
    },
    {
      id: "Course A/Lecture 01.mp4",
      sourcePath: "/Users/you/My Videos/Course A/Lecture 01.mp4",
      relativePath: "Course A/Lecture 01.mp4",
      outputPath: "/Users/you/My Videos - Processed/Course A/Lecture 01.pdf",
      status: "pending",
      progress: 0,
      stage: "Pending",
      slidesDetected: 0,
      error: null,
      outputExists: true,
    },
  ];
}

interface Harness {
  client: BatchClient;
  emit: (event: BatchEvent) => void;
}

function createHarness(overrides: Partial<BatchClient> = {}): Harness {
  const listeners = new Set<(event: BatchEvent) => void>();
  const client: BatchClient = {
    async selectInputFolder() {
      return "/Users/you/My Videos";
    },
    async selectInputFile() {
      return "/Users/you/My Videos/Course B/Lecture 01.mp4";
    },
    async selectOutputFolder(defaultPath) {
      return defaultPath;
    },
    // Use the real implementation so the path rules are exercised too.
    defaultOutputRoot,
    async discoverJobs(): Promise<ScanResult> {
      return { kind: "ready", jobs: sampleJobs() };
    },
    async startBatch() {},
    async cancelBatch() {},
    async onBatchEvent(handler) {
      listeners.add(handler);
      return () => listeners.delete(handler);
    },
    ...overrides,
  };

  const emit = (event: BatchEvent) => {
    for (const listener of listeners) listener(event);
  };

  return { client, emit };
}

/** Walks the workflow to the reviewed queue state. */
async function reachReview(user: ReturnType<typeof userEvent.setup>, harness: Harness) {
  render(<BatchWorkflow client={harness.client} />);
  await user.click(screen.getByRole("button", { name: /choose folder/i }));
  await user.click(screen.getByRole("button", { name: /scan folder/i }));
  expect(await screen.findByRole("button", { name: /start processing/i })).toBeInTheDocument();
}

const completedSummary = {
  found: 2,
  completed: 2,
  failed: 0,
  skipped: 0,
  cancelled: 0,
  slidesDetected: 80,
  outputRoot: "/Users/you/My Videos - Processed",
};

describe("BatchWorkflow", () => {
  it("guides the user from folder selection to a reviewed queue", async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    render(<BatchWorkflow client={harness.client} />);

    expect(screen.getByText(/No folder or video selected/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /scan folder/i })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: /choose folder/i }));

    expect(screen.getByText("/Users/you/My Videos")).toBeInTheDocument();
    expect(screen.getByText("/Users/you/My Videos - Processed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /scan folder/i })).toBeEnabled();
  });

  it("can pick a single video instead of a folder", async () => {
    const user = userEvent.setup();
    const scanned: BatchOptions[] = [];
    const harness = createHarness({
      async discoverJobs(options) {
        scanned.push(options);
        return { kind: "ready", jobs: sampleJobs() };
      },
    });
    render(<BatchWorkflow client={harness.client} />);

    await user.click(screen.getByRole("button", { name: /choose video/i }));

    // The single-file selection is shown, and the scan target is the file.
    expect(screen.getByText("/Users/you/My Videos/Course B/Lecture 01.mp4")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^continue$/i }));
    await screen.findByRole("button", { name: /start processing/i });

    expect(scanned[0]?.inputRoot).toBe("/Users/you/My Videos/Course B/Lecture 01.mp4");
  });

  it("suggests the video's own folder as the new output folder", async () => {
    const user = userEvent.setup();
    render(<BatchWorkflow client={createHarness().client} />);

    await user.click(screen.getByRole("button", { name: /choose video/i }));

    // `.../Course B/Lecture 01.mp4` suggests `.../Course B - Processed`.
    expect(screen.getByText("/Users/you/My Videos/Course B - Processed")).toBeInTheDocument();
  });

  it("writes beside the video's folder, not beside the file", async () => {
    const user = userEvent.setup();
    const started: BatchOptions[] = [];
    const harness = createHarness({
      async startBatch(options) {
        started.push(options);
      },
    });
    render(<BatchWorkflow client={harness.client} />);

    await user.click(screen.getByRole("button", { name: /choose video/i }));
    await user.click(screen.getByLabelText(/into the folder the videos came from/i));
    await user.click(screen.getByRole("button", { name: /^continue$/i }));
    await screen.findByRole("button", { name: /start processing/i });
    await user.click(screen.getByRole("button", { name: /start processing/i }));

    // Passing the file itself would make the backend treat it as a directory.
    expect(started[0]?.outputRoot).toBe("/Users/you/My Videos/Course B");
  });

  it("writes into a new folder by default", async () => {
    const user = userEvent.setup();
    render(<BatchWorkflow client={createHarness().client} />);

    await user.click(screen.getByRole("button", { name: /choose folder/i }));

    expect(screen.getByLabelText(/into a new folder/i)).toBeChecked();
    expect(screen.getByText("/Users/you/My Videos - Processed")).toBeInTheDocument();
  });

  it("can write the PDFs beside the videos instead", async () => {
    const user = userEvent.setup();
    const started: BatchOptions[] = [];
    const harness = createHarness({
      async startBatch(options) {
        started.push(options);
      },
    });
    render(<BatchWorkflow client={harness.client} />);

    await user.click(screen.getByRole("button", { name: /choose folder/i }));
    await user.click(screen.getByLabelText(/into the folder the videos came from/i));
    await user.click(screen.getByRole("button", { name: /scan folder/i }));
    await screen.findByRole("button", { name: /start processing/i });

    // The reviewed destination is the lecture folder itself.
    expect(screen.getAllByText("/Users/you/My Videos").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /start processing/i }));
    expect(started[0]?.outputRoot).toBe("/Users/you/My Videos");
  });

  it("hides the folder picker when writing beside the videos", async () => {
    const user = userEvent.setup();
    render(<BatchWorkflow client={createHarness().client} />);

    await user.click(screen.getByRole("button", { name: /choose folder/i }));
    expect(screen.getByRole("button", { name: /^change$/i })).toBeInTheDocument();

    await user.click(screen.getByLabelText(/into the folder the videos came from/i));
    expect(screen.queryByRole("button", { name: /^change$/i })).not.toBeInTheDocument();
  });

  it("surfaces an error instead of failing silently when the picker fails", async () => {
    const user = userEvent.setup();
    const harness = createHarness({
      async selectInputFolder() {
        throw new Error("dialog.open not allowed");
      },
    });
    render(<BatchWorkflow client={harness.client} />);

    await user.click(screen.getByRole("button", { name: /choose folder/i }));

    expect(await screen.findByText(/dialog\.open not allowed/i)).toBeInTheDocument();
  });

  it("explains that folder selection is simulated in the browser preview", () => {
    render(<BatchWorkflow client={createHarness().client} />);
    expect(screen.getByRole("heading", { name: /browser preview/i })).toBeInTheDocument();
  });

  it("shows the discovered queue grouped by folder with existing-output context", async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    await reachReview(user, harness);

    expect(screen.getByText("Course B")).toBeInTheDocument();
    expect(screen.getByText("Course A")).toBeInTheDocument();
    expect(screen.getAllByText("Lecture 01.mp4")).toHaveLength(2);
    expect(screen.getByText(/1 already processed/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/skip existing/i)).toBeChecked();
    expect(screen.getByLabelText(/replace existing/i)).not.toBeChecked();
  });

  it("reports the empty state when no videos are found", async () => {
    const user = userEvent.setup();
    const harness = createHarness({
      async discoverJobs(): Promise<ScanResult> {
        return { kind: "empty", inputRoot: "/Users/you/My Videos" };
      },
    });
    render(<BatchWorkflow client={harness.client} />);

    await user.click(screen.getByRole("button", { name: /choose folder/i }));
    await user.click(screen.getByRole("button", { name: /scan folder/i }));

    expect((await screen.findAllByText(/No supported videos found/i)).length).toBeGreaterThan(0);
    expect(screen.getByRole("heading", { name: /could not continue/i })).toBeInTheDocument();
  });

  it("streams real progress and finishes with a summary", async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    await reachReview(user, harness);

    await user.click(screen.getByRole("button", { name: /start processing/i }));
    expect(screen.getByText(/Processing 1 of 2 videos/i)).toBeInTheDocument();

    act(() => {
      harness.emit({
        type: "jobStarted",
        jobId: "Course B/Lecture 01.mp4",
        stage: "Extracting frames",
      });
    });
    act(() => {
      harness.emit({
        type: "jobProgress",
        jobId: "Course B/Lecture 01.mp4",
        progress: 60,
        stage: "Detecting slides",
        slidesDetected: 12,
      });
    });

    expect(screen.getByText(/Detecting slides, 12 slides/)).toBeInTheDocument();

    act(() => {
      harness.emit({
        type: "jobCompleted",
        jobId: "Course B/Lecture 01.mp4",
        result: {
          sourceVideo: "/Users/you/My Videos/Course B/Lecture 01.mp4",
          slidesPdf: "/Users/you/My Videos - Processed/Course B/Lecture 01.pdf",
          transcript: null,
          slideText: null,
          slidesDetected: 42,
          metadata: {},
        },
      });
      harness.emit({
        type: "jobSkipped",
        jobId: "Course A/Lecture 01.mp4",
        reason: "Output already exists",
      });
      harness.emit({
        type: "batchCompleted",
        summary: {
          found: 2,
          completed: 1,
          failed: 0,
          skipped: 1,
          cancelled: 0,
          slidesDetected: 42,
          outputRoot: "/Users/you/My Videos - Processed",
        },
      });
    });

    expect(await screen.findByRole("heading", { name: /batch complete/i })).toBeInTheDocument();
    expect(screen.getByText("Slides detected")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /open output folder/i })).not.toBeInTheDocument();
  });

  it("keeps going after a failure and offers the failed items", async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    await reachReview(user, harness);
    await user.click(screen.getByRole("button", { name: /start processing/i }));

    act(() => {
      harness.emit({
        type: "jobFailed",
        jobId: "Course B/Lecture 01.mp4",
        error: "unsupported codec",
      });
      harness.emit({
        type: "jobSkipped",
        jobId: "Course A/Lecture 01.mp4",
        reason: "Output already exists",
      });
      harness.emit({
        type: "batchCompleted",
        summary: {
          found: 2,
          completed: 0,
          failed: 1,
          skipped: 1,
          cancelled: 0,
          slidesDetected: 0,
          outputRoot: "/Users/you/My Videos - Processed",
        },
      });
    });

    expect(
      await screen.findByRole("heading", { name: /finished with failures/i }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /view failed items/i }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/unsupported codec/i)).toBeInTheDocument();
  });

  it("confirms cancellation and reports preserved output", async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    await reachReview(user, harness);
    await user.click(screen.getByRole("button", { name: /start processing/i }));

    await user.click(screen.getByRole("button", { name: /cancel processing/i }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/No new videos will start/i)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: /^cancel processing$/i }));

    act(() => {
      harness.emit({ type: "batchCancelled", completed: 1, remaining: 1 });
    });

    expect(
      await screen.findByRole("heading", { name: /processing cancelled/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Completed PDFs have been preserved/i)).toBeInTheDocument();
  });

  it("starts a new batch from the summary", async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    await reachReview(user, harness);
    await user.click(screen.getByRole("button", { name: /start processing/i }));

    act(() => {
      harness.emit({ type: "batchCompleted", summary: completedSummary });
    });

    await screen.findByRole("heading", { name: /batch complete/i });
    await user.click(screen.getByRole("button", { name: /process another folder/i }));

    expect(screen.getByText(/No folder or video selected/i)).toBeInTheDocument();
  });
});

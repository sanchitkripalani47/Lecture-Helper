import { computeSummary } from "./grouping";
import {
  SUPPORTED_VIDEO_EXTENSIONS,
  type BatchEvent,
  type BatchOptions,
  type InputKind,
  type ScanResult,
  type VideoJob,
} from "./types";

export interface BatchClient {
  selectInputFolder(): Promise<string | null>;
  selectInputFile(): Promise<string | null>;
  selectOutputFolder(defaultPath: string): Promise<string | null>;
  defaultOutputRoot(inputPath: string, kind: InputKind): string;
  discoverJobs(options: BatchOptions): Promise<ScanResult>;
  startBatch(options: BatchOptions): Promise<void>;
  cancelBatch(): Promise<void>;
  onBatchEvent(handler: (event: BatchEvent) => void): Promise<() => void>;
}

export const TAURI_BATCH_EVENT = "batch://event";

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** True when the UI is running in a plain browser instead of the desktop app. */
export function isPreviewMode(): boolean {
  return !isTauriRuntime();
}

export function joinPath(base: string, segment: string): string {
  if (base.length === 0) return segment;
  const separator = base.includes("\\") && !base.includes("/") ? "\\" : "/";
  const trimmed = base.replace(/[\\/]+$/, "");
  return `${trimmed}${separator}${segment}`;
}

/** The folder containing a file, or the parent of a folder. */
export function parentOf(path: string): string {
  const normalized = path.replace(/[\\/]+$/, "");
  const lastSeparator = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  return lastSeparator <= 0 ? normalized : normalized.slice(0, lastSeparator);
}

/**
 * Default output root: a sibling of the selection named `<name> - Processed`.
 *
 * A single selected video contributes the folder it sits in, so picking
 * `Course/Lecture 01.mp4` suggests `Course - Processed` rather than naming the
 * folder after the file.
 */
export function defaultOutputRoot(inputPath: string, kind: InputKind = "folder"): string {
  const base = kind === "file" ? parentOf(inputPath) : inputPath;
  const normalized = base.replace(/[\\/]+$/, "");
  const lastSeparator = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  if (lastSeparator < 0) return `${normalized} - Processed`;

  const parent = normalized.slice(0, lastSeparator);
  const name = normalized.slice(lastSeparator + 1);
  const separator = normalized.includes("\\") && !normalized.includes("/") ? "\\" : "/";
  return `${parent}${separator}${name} - Processed`;
}

function sampleJobs(options: BatchOptions): VideoJob[] {
  const courses = [
    { name: "Course B", lectures: 3 },
    { name: "Course A", lectures: 2 },
    { name: "Course C - Advanced Topics", lectures: 3 },
    { name: "Course D", lectures: 1 },
  ];
  const jobs: VideoJob[] = [];
  courses.forEach((course, courseIndex) => {
    for (let lecture = 1; lecture <= course.lectures; lecture += 1) {
      const name = `Lecture ${String(lecture).padStart(2, "0")}.mp4`;
      const relativePath = `${course.name}/${name}`;
      jobs.push({
        id: `${courseIndex}-${lecture}`,
        sourcePath: joinPath(joinPath(options.inputRoot, course.name), name),
        relativePath,
        outputPath: joinPath(
          joinPath(options.outputRoot, course.name),
          name.replace(/\.mp4$/, ".pdf"),
        ),
        status: "pending",
        progress: 0,
        stage: "Pending",
        slidesDetected: 0,
        error: null,
        outputExists: courseIndex === 1 && lecture === 1,
      });
    }
  });
  return jobs;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Development-only simulator. It is used when the app runs in a plain browser
 * (`npm run dev`) rather than inside the Tauri shell, so the workflow can be
 * exercised without the Rust backend. The desktop app always uses the real
 * engine through `discover_jobs` / `start_batch`.
 */
export function createMockClient(): BatchClient {
  const listeners = new Set<(event: BatchEvent) => void>();
  let cancelled = false;

  const emit = (event: BatchEvent) => {
    for (const listener of listeners) listener(event);
  };

  return {
    async selectInputFolder() {
      return "/Users/you/My Videos";
    },
    async selectInputFile() {
      return "/Users/you/My Videos/Course B/Lecture 01.mp4";
    },
    async selectOutputFolder(defaultPath) {
      return defaultPath;
    },
    defaultOutputRoot,
    async discoverJobs(options) {
      const jobs = sampleJobs(options);
      return jobs.length > 0
        ? { kind: "ready", jobs }
        : { kind: "empty", inputRoot: options.inputRoot };
    },
    async startBatch(options) {
      cancelled = false;
      const jobs = sampleJobs(options);
      const statuses = jobs.map((job) => ({ ...job }));

      void (async () => {
        let completed = 0;
        const stages = ["Extracting frames", "Detecting slides", "Assembling PDF"];

        for (const job of statuses) {
          if (cancelled) break;

          if (options.existingOutputPolicy === "skip" && job.outputExists) {
            job.status = "skipped";
            job.stage = "Output already exists";
            emit({ type: "jobSkipped", jobId: job.id, reason: "Output already exists" });
            continue;
          }

          emit({ type: "jobStarted", jobId: job.id, stage: stages[0] });
          let slides = 0;

          for (let stageIndex = 0; stageIndex < stages.length; stageIndex += 1) {
            for (let step = 0; step < 5; step += 1) {
              if (cancelled) break;
              await delay(50);
              const overall = Math.round(((stageIndex * 5 + step + 1) / (stages.length * 5)) * 100);
              if (stageIndex === 1) slides = Math.round((step + 1) * 12.8);
              emit({
                type: "jobProgress",
                jobId: job.id,
                progress: overall,
                stage: stages[stageIndex],
                slidesDetected: slides,
              });
            }
            if (cancelled) break;
          }

          if (cancelled) break;

          if (job.relativePath.includes("Course A") && job.relativePath.includes("Lecture 02")) {
            job.status = "failed";
            job.stage = "Failed";
            job.error = "Could not decode video: unsupported codec";
            emit({ type: "jobFailed", jobId: job.id, error: job.error });
            continue;
          }

          completed += 1;
          job.status = "completed";
          job.stage = "Completed";
          job.progress = 100;
          job.slidesDetected = 64;
          emit({
            type: "jobCompleted",
            jobId: job.id,
            result: {
              sourceVideo: job.sourcePath,
              slidesPdf: job.outputPath,
              transcript: null,
              slideText: null,
              slidesDetected: 64,
              metadata: {},
            },
          });
        }

        if (cancelled) {
          emit({ type: "batchCancelled", completed, remaining: statuses.length - completed });
        } else {
          emit({ type: "batchCompleted", summary: computeSummary(statuses, options.outputRoot) });
        }
      })();
    },
    async cancelBatch() {
      cancelled = true;
    },
    async onBatchEvent(handler) {
      listeners.add(handler);
      return () => listeners.delete(handler);
    },
  };
}

interface TauriApi {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  listen(event: string, handler: (payload: { payload: unknown }) => void): Promise<() => void>;
}

/// Returns `null` only when there is genuinely no Tauri runtime (browser
/// preview). Inside Tauri, a failure to load the IPC layer is a real error and
/// must surface rather than silently falling back to the simulator.
async function loadTauriApi(): Promise<TauriApi | null> {
  if (!isTauriRuntime()) return null;
  const [{ invoke }, { listen }] = await Promise.all([
    import("@tauri-apps/api/core"),
    import("@tauri-apps/api/event"),
  ]);
  return {
    invoke: invoke as TauriApi["invoke"],
    listen: async (event, handler) => {
      const unlisten = await listen<unknown>(event, (e) => handler({ payload: e.payload }));
      return unlisten;
    },
  };
}

export function createBatchClient(): BatchClient {
  const mock = createMockClient();
  const apiPromise = loadTauriApi();

  return {
    async selectInputFolder() {
      const loaded = await apiPromise;
      if (!loaded) return mock.selectInputFolder();
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selection = await open({
        directory: true,
        multiple: false,
        title: "Select lecture folder",
      });
      return typeof selection === "string" ? selection : null;
    },
    async selectInputFile() {
      const loaded = await apiPromise;
      if (!loaded) return mock.selectInputFile();
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selection = await open({
        directory: false,
        multiple: false,
        title: "Select a video",
        filters: [
          {
            name: "Video",
            extensions: SUPPORTED_VIDEO_EXTENSIONS.map((extension) => extension.replace(".", "")),
          },
        ],
      });
      return typeof selection === "string" ? selection : null;
    },
    async selectOutputFolder(defaultPath) {
      const loaded = await apiPromise;
      if (!loaded) return mock.selectOutputFolder(defaultPath);
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selection = await open({
        directory: true,
        multiple: false,
        title: "Select output folder",
        defaultPath,
      });
      return typeof selection === "string" ? selection : null;
    },
    defaultOutputRoot,
    async discoverJobs(options) {
      const loaded = await apiPromise;
      if (!loaded) return mock.discoverJobs(options);
      return loaded.invoke<ScanResult>("discover_jobs", { options });
    },
    async startBatch(options) {
      const loaded = await apiPromise;
      if (!loaded) return mock.startBatch(options);
      await loaded.invoke("start_batch", { options });
    },
    async cancelBatch() {
      const loaded = await apiPromise;
      if (!loaded) return mock.cancelBatch();
      await loaded.invoke("cancel_batch");
    },
    async onBatchEvent(handler) {
      const loaded = await apiPromise;
      if (!loaded) return mock.onBatchEvent(handler);
      return loaded.listen(TAURI_BATCH_EVENT, (event) => handler(event.payload as BatchEvent));
    },
  };
}
